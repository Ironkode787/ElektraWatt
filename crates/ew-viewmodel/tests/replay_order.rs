// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-439's guard (decision log ID-640): no shell output is ordered by UUID. One creating script runs twice on equal
//! copies of Golden A, once with new IDs that ascend and once with new IDs that descend
//! (`ShellModel::set_id_source_for_tests`), so every pair of records the script creates sorts the other way round by ID
//! in the second run. The transcripts must be byte-identical with aliases in them, and each alias `@n` must name the
//! same record in both runs (compared by what a person sees of it: kind, title, place, label). An output ordered by
//! UUID fails one of the two: a list printed in another order changes the bytes, and a list of records printed for the
//! first time numbers its aliases the other way round.
//!
//! The script covers what FT-439 lists: three new devices with different part quantities in one property sheet
//! ("set all" prints the first target's list), two placements selected together by a band, and a wire split by a new
//! wire (the two halves and the junction) selected by a touching band.

mod common;

use std::path::Path;

use common::{GOLDEN_A, Tmp, copy_dir};
use ew_viewmodel::alias;
use ew_viewmodel::canvas::{HOVER_SLOP_PT, pick};
use ew_viewmodel::ew_api::{EntropyError, IdSource, Session, UnixTime};
use ew_viewmodel::script::{self, NoExtension};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// New IDs that ascend: the clock counts up from the epoch's first millisecond (every new ID sorts before Golden A's),
/// the random bits count up.
struct Ascending {
    millis: u64,
    n: u64,
}

/// New IDs that descend: the clock counts down from the largest 48-bit millisecond (every new ID sorts after Golden
/// A's), the random bits count down.
struct Descending {
    millis: u64,
    n: u64,
}

fn bits(n: u64, buf: &mut [u8]) {
    let b = n.to_be_bytes();
    for (i, x) in buf.iter_mut().enumerate() {
        *x = b[i % 8];
    }
}

impl IdSource for Ascending {
    fn now(&mut self) -> UnixTime {
        self.millis += 1;
        UnixTime {
            millis: self.millis,
            sub_ms_nanos: 0,
        }
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        self.n += 1;
        bits(self.n, buf);
        Ok(())
    }
}

impl IdSource for Descending {
    fn now(&mut self) -> UnixTime {
        self.millis -= 1;
        UnixTime {
            millis: self.millis,
            sub_ms_nanos: 0,
        }
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        self.n -= 1;
        bits(self.n, buf);
        Ok(())
    }
}

fn ascending() -> Box<dyn IdSource + Send> {
    Box::new(Ascending { millis: 0, n: 0 })
}

fn descending() -> Box<dyn IdSource + Send> {
    Box::new(Descending {
        millis: (1 << 48) - 1,
        n: u64::MAX,
    })
}

/// The script text of `steps`.
fn script_text(steps: &[(String, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"replay order\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// `{"x_mm", "y_mm"}` of a sheet point.
fn at(p: (i64, i64)) -> Value {
    json!({"x_mm": p.0 as f64 / 1e6, "y_mm": p.1 as f64 / 1e6})
}

/// What the script needs from Golden A, read once: a part copy, the sheet points of folio 101's placements and band,
/// folio 102's CP `-102F2:14`, the split point on the PE wire and a touching band around it.
struct Points {
    part: String,
    place: [(i64, i64); 2],
    band_101: [(i64, i64); 2],
    wire_from: (i64, i64),
    split: (i64, i64),
    band_102: [(i64, i64); 2],
}

/// Whether a press at the sheet point `p` of the focused canvas hits no entity and no link (twice the pick's slop).
fn empty(s: &ShellModel, p: (i64, i64)) -> bool {
    let c = s.canvas().unwrap();
    let slop = 2.0 * HOVER_SLOP_PT / c.points_per_nm();
    pick(&c.page().entities, Some(p), slop).is_none() && c.link_at(p).is_none()
}

fn points(tmp: &Tmp) -> Points {
    let part = {
        let (s, _) = Session::open_read_only(Path::new(GOLDEN_A), "test").unwrap();
        let q = s.query();
        q.parts().iter().next().map(|(id, _)| id.to_string()).unwrap()
    };
    let mut s = tmp.shell("101");
    let sn = |s: &ShellModel, k: (i64, i64)| {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
    };
    let place = [sn(&s, (60, 30)), sn(&s, (66, 30))];
    // An enclosing band (left to right) from an empty point above and left of the placements.
    let corner = (3..14)
        .map(|k| sn(&s, (60 - k, 30 - k)))
        .find(|p| empty(&s, *p))
        .expect("an empty point above and left of folio 101's placements");
    let band_101 = [corner, sn(&s, (74, 42))];
    assert!(s.dispatch("page.goto", &json!({"folio": "102"})) == ew_viewmodel::shell::Outcome::Done);
    let t = std::sync::Arc::clone(&s.canvas().unwrap().page().tables);
    let page_to_sheet = |s: &ShellModel, p: (i64, i64)| s.canvas().unwrap().page().snapper.page_to_sheet(p);
    let from = t
        .cps
        .iter()
        .find(|c| c.label == "-102F2:14")
        .expect("-102F2:14")
        .at;
    let x = s.canvas().unwrap().page().snapper.page_of_index(76);
    let split_page = (x, 13_229_167);
    let wire_from = page_to_sheet(&s, from);
    let split = page_to_sheet(&s, split_page);
    // A touching band (right to left) around the split point, starting on an empty point.
    let sheet_grid = |s: &ShellModel, dx: i64, dy: i64| {
        let sn = &s.canvas().unwrap().page().snapper;
        let (ix, iy) = (sn.grid_index_of(split_page.0), sn.grid_index_of(split_page.1));
        sn.page_to_sheet((sn.page_of_index(ix + dx), sn.page_of_index(iy + dy)))
    };
    let start = (1..12)
        .flat_map(|k| [(k, k), (k, k + 1), (k + 1, k)])
        .map(|(dx, dy)| sheet_grid(&s, dx, dy))
        .find(|p| empty(&s, *p))
        .expect("an empty point right of and below the split point");
    let band_102 = [start, sheet_grid(&s, -1, -1)];
    s.shutdown();
    Points {
        part,
        place,
        band_101,
        wire_from,
        split,
        band_102,
    }
}

fn steps(dir: &str, p: &Points) -> Vec<(String, Value)> {
    let device = |counter: u32, qty: &str| {
        json!({"cmd": "device.create", "params": {"letter": "K", "counter": counter,
            "function_text": format!("Fan {counter}"),
            "parts": [{"part": {"copy": p.part}, "qty": qty}]}})
    };
    let list: Vec<(&str, Value)> = vec![
        ("file.open", json!({ "dir": dir })),
        ("page.goto", json!({"folio": "101"})),
        // Three new devices; one sheet; "set all" of the varying part list prints the first target's list.
        ("command.run", device(91, "1")),
        ("command.run", device(92, "2")),
        ("command.run", device(93, "3")),
        (
            "select.set",
            json!({"refs": ["tag:-?K93", "tag:-?K91", "tag:-?K92"]}),
        ),
        ("props.show", json!({"on": true})),
        ("props.edit", json!({"row": "device/parts"})),
        ("ui.describe", json!({})),
        ("props.cancel", json!({})),
        // Two placements, selected together by an enclosing band.
        (
            "place.pick",
            json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
        ),
        ("canvas.pointer", at(p.place[0])),
        ("canvas.press", at(p.place[0])),
        ("canvas.release", at(p.place[0])),
        ("canvas.pointer", at(p.place[1])),
        ("canvas.press", at(p.place[1])),
        ("canvas.release", at(p.place[1])),
        ("ui.dismiss", json!({})),
        ("tool.select", json!({})),
        ("canvas.press", at(p.band_101[0])),
        ("canvas.pointer", at(p.band_101[1])),
        ("canvas.release", at(p.band_101[1])),
        ("ui.describe", json!({})),
        // A wire split by a new wire: the two halves and the junction, selected by a touching band.
        ("page.goto", json!({"folio": "102"})),
        ("view.zoom_actual", json!({})),
        ("tool.wire", json!({})),
        ("cursor.set", at(p.wire_from)),
        ("canvas.activate", json!({})),
        ("cursor.set", at(p.split)),
        ("canvas.activate", json!({})),
        ("ui.dismiss", json!({})),
        ("tool.select", json!({})),
        ("canvas.press", at(p.band_102[0])),
        ("canvas.pointer", at(p.band_102[1])),
        ("canvas.release", at(p.band_102[1])),
        ("ui.describe", json!({})),
        // The new junction alone, its editor: the wires through it listed by printed number, then ends.
        ("canvas.press", at(p.split)),
        ("canvas.release", at(p.split)),
        ("junction.open", json!({})),
        ("ui.describe", json!({})),
        ("ui.dismiss", json!({})),
        ("file.save", json!({})),
    ];
    list.into_iter().map(|(a, v)| (a.to_owned(), v)).collect()
}

/// What a person sees of the record `id` in the saved project: the sheet's kinds and title, and on folios 101 and
/// 102 a representation's symbol and insertion point, a junction's point, a connection's drawn polyline, a CP's
/// label; a part copy's part number.
fn fingerprint(s: &Session, id: &str) -> String {
    let u = id.parse().unwrap();
    let sheet = s.property_sheet(&[u]);
    let mut out = format!("{:?} {:?}", sheet.header.kinds, sheet.header.title);
    for folio in ["101", "102"] {
        let t = s.page_tables(folio).unwrap();
        if let Some(r) = t.reps.iter().find(|r| r.id == u) {
            out.push_str(&format!(" rep {folio} {} {:?}", r.symbol_key, r.pos));
        }
        if let Some(j) = t.junctions.iter().find(|j| j.id == u) {
            out.push_str(&format!(" junction {folio} {:?}", j.at));
        }
        if let Some(c) = t.connections.iter().find(|c| c.id == u) {
            out.push_str(&format!(" connection {folio} {:?}", c.polyline));
        }
        if let Some(c) = t.cps.iter().find(|c| c.id == u) {
            out.push_str(&format!(" cp {}", c.label));
        }
    }
    let q = s.query();
    if let Some((_, p)) = q.parts().iter().find(|(pid, _)| pid.to_string() == id) {
        out.push_str(&format!(" part {}", p.item.part_number.as_str()));
    }
    out
}

/// One run of the script under `ids`: the transcript (the directory replaced), the fingerprint of every alias in
/// order, and the IDs of `-K91` and `-K93`.
fn run(
    tag: &str,
    ids: fn() -> Box<dyn IdSource + Send>,
    p: &Points,
) -> (String, Vec<String>, (String, String)) {
    let tmp = Tmp::new(tag);
    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let dir = dir.to_string_lossy().into_owned();
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    shell.set_id_source_for_tests(Some(ids));
    let parsed = script::parse(&script_text(&steps(&dir, p))).unwrap();
    let mut lines = Vec::new();
    script::run(&mut shell, &mut NoExtension, &parsed, &mut |l| {
        lines.push(l.to_owned());
        Ok(())
    })
    .unwrap();
    for l in lines.iter().skip(1) {
        let step: Value = serde_json::from_str(l).unwrap();
        assert_eq!(step["outcome"], "done", "{tag}: {l}");
    }
    let aliases: Vec<String> = (1..=shell.alias_count())
        .map(|n| shell.alias_uuid(n).unwrap())
        .collect();
    shell.shutdown();
    let (s, _) = Session::open_read_only(Path::new(&dir), "test").unwrap();
    let prints = aliases.iter().map(|id| fingerprint(&s, id)).collect();
    let devices: Vec<String> = {
        let q = s.query();
        q.devices().iter().map(|(id, _)| id.to_string()).collect()
    };
    let by_tag = |t: &str| {
        devices
            .iter()
            .find(|id| s.property_sheet(&[id.parse().unwrap()]).header.title == t)
            .cloned()
            .unwrap_or_else(|| panic!("{tag}: no device {t}"))
    };
    let k = (by_tag("-?K91"), by_tag("-?K93"));
    (lines.join("\n").replace(&dir, "DIR"), prints, k)
}

/// Acceptance 1 and 4 of FT-439: under ascending and descending new IDs one creating script gives byte-identical
/// transcripts with aliases, and every alias names the same record in both runs.
#[test]
fn one_script_under_ascending_and_descending_ids_prints_the_same_bytes_and_records() {
    let probe = Tmp::new("order-probe");
    let p = points(&probe);
    let (a, prints_a, k_a) = run("order-asc", ascending, &p);
    let (b, prints_b, k_b) = run("order-desc", descending, &p);
    // The two runs minted their IDs in opposite orders.
    assert!(k_a.0 < k_a.1, "ascending: -K91 before -K93 by ID");
    assert!(k_b.0 > k_b.1, "descending: -K91 after -K93 by ID");
    assert_eq!(alias::find_uuid(&a), None, "no UUID in the transcript");
    // What the script prints names new records by alias: the "set all" buffer's part copy, the band selections (the
    // second and third `ui.describe` follow the bands of folios 101 and 102).
    let described: Vec<Value> = a
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|v| v["action"] == "ui.describe")
        .map(|v| v["state"]["canvas"]["selection"].clone())
        .collect();
    let len = |v: &Value| v.as_array().map_or(0, Vec::len);
    assert!(
        len(&described[1]) >= 2,
        "folio 101's band selects the two placements: {described:?}"
    );
    assert!(
        len(&described[2]) >= 3,
        "folio 102's band selects the split's records: {described:?}"
    );
    assert!(
        a.contains("editing \\\"device/parts\\\" buffer"),
        "the set-all buffer is described"
    );
    // The junction editor of the split's junction lists the wires through it (`  wire "<n>" from … end …`).
    let editor: Vec<&str> = a
        .lines()
        .skip(1)
        .filter(|l| l.contains("\"action\":\"ui.describe\"") && l.contains("  wire \\\""))
        .collect();
    assert_eq!(editor.len(), 1, "the junction editor is described once");
    assert!(editor[0].matches("  wire \\\"").count() >= 2, "{}", editor[0]);
    assert_eq!(a, b, "byte-identical transcripts");
    assert_eq!(prints_a.len(), prints_b.len());
    for (n, (x, y)) in prints_a.iter().zip(&prints_b).enumerate() {
        assert_eq!(x, y, "alias @{} names another record", n + 1);
    }
}
