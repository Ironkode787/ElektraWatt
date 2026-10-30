// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Every starter symbol reaches a usable device from the picker (WP-S2.18 D7): in a new project each symbol of the
//! starter set is placed by its path, with a generic starter part where the starter parts (D5) give one, and each
//! auxiliary symbol joins a device by the join chooser (D2). The table [`PATHS`] must name every starter symbol, so a
//! symbol added later fails the test until it has a path. D3 (the device's "Add" menu) is not built; no path uses it.

mod common;

use std::collections::BTreeSet;

use common::{Tmp, at, click, done};
use ew_viewmodel::ew_api::Session;
use ew_viewmodel::ew_api::checks;
use ew_viewmodel::ew_api::library::starter::{self, StarterKind};
use ew_viewmodel::ew_api::output::{TableRep, Uuid};
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use ew_viewmodel::tools::PickerRow;
use serde_json::json;

/// How a starter symbol reaches a device from the picker.
#[derive(Clone, Copy, Debug)]
enum Path {
    /// `place.pick part=<number> template=<index>` on the page of that type: a new device with the starter part.
    Part(&'static str, usize, &'static str),
    /// A part-first strip terminal: `place.pick part=<number>`, the strip chooser's strip row, a click.
    Terminal(&'static str),
    /// A black box with the part's connection points: `place.pick part=<number>` arms the box tool, two clicks.
    BlackBox(&'static str),
    /// An auxiliary symbol: `place.pick symbol`, `place.join` and the chooser's row of the relay placed with
    /// `EW-GEN-RLY-1`, a click; the representation joins the relay's device.
    Join,
    /// No starter part names the symbol: `place.pick symbol=<key> letter=<letter>` on the page of that type, a click.
    Device(&'static str, &'static str),
    /// No starter part names the symbol, which is drawn as a box (an overview module): `place.pick symbol=<key>
    /// letter=<letter>` arms the box tool on the page of that type, two clicks.
    DeviceBox(&'static str, &'static str),
    /// A drawing aid, no device (a junction, a marker, a strip segment): it is not placed as a device.
    Aid,
}

const MULTI: &str = "multi_line";
const FLUID: &str = "fluid";
const OVERVIEW: &str = "overview";
const BUS: &str = "bus_topology";

/// Every starter symbol and its path, in key order.
const PATHS: &[(&str, Path)] = &[
    ("ew.bus.dp-connector@qet10", Path::Device("W", BUS)),
    ("ew.bus.dp-port@qet10", Path::Device("A", BUS)),
    ("ew.gen.blackbox@qet10", Path::BlackBox("EW-GEN-PSU-1")),
    ("ew.gen.cable-marker@qet10", Path::Aid),
    ("ew.gen.external-arrow@qet10", Path::Device("A", MULTI)),
    ("ew.gen.ip@qet10", Path::Device("A", MULTI)),
    ("ew.gen.junction-bevel@qet10", Path::Aid),
    ("ew.gen.junction-dot@qet10", Path::Aid),
    ("ew.gen.strip-segment@qet10", Path::Aid),
    ("ew.gen.wire-marker@qet10", Path::Aid),
    ("ew.iec.coil@qet10", Path::Part("EW-GEN-RLY-1", 0, MULTI)),
    ("ew.iec.contact-co@qet10", Path::Join),
    ("ew.iec.contact-nc@qet10", Path::Join),
    ("ew.iec.contact-no@qet10", Path::Join),
    ("ew.iec.fe@qet10", Path::Device("PE", MULTI)),
    ("ew.iec.fuse@qet10", Path::Part("EW-GEN-FUS-1", 0, MULTI)),
    ("ew.iec.heater@qet10", Path::Device("E", MULTI)),
    ("ew.iec.lamp@qet10", Path::Part("EW-GEN-LMP-1", 0, MULTI)),
    ("ew.iec.mcb-1p@qet10", Path::Part("EW-GEN-MCB-1", 0, MULTI)),
    ("ew.iec.mcb-1pn@qet10", Path::Part("EW-GEN-MCB-2", 0, MULTI)),
    ("ew.iec.motor-3ph@qet10", Path::Part("EW-GEN-MOT-1", 0, MULTI)),
    ("ew.iec.pe@qet10", Path::Device("PE", MULTI)),
    ("ew.iec.plug-pin@qet10", Path::Device("X", MULTI)),
    (
        "ew.iec.position-switch-no@qet10",
        Path::Part("EW-GEN-POS-1", 0, MULTI),
    ),
    ("ew.iec.pushbutton-no@qet10", Path::Part("EW-GEN-PB-1", 0, MULTI)),
    ("ew.iec.resistor@qet10", Path::Device("R", MULTI)),
    (
        "ew.iec.selector-switch-no@qet10",
        Path::Part("EW-GEN-SEL-1", 0, MULTI),
    ),
    ("ew.iec.switch-2p-manual@qet10", Path::Device("Q", MULTI)),
    ("ew.iec.terminal-pe@qet10", Path::Terminal("EW-GEN-TRM-2")),
    ("ew.iec.terminal@qet10", Path::Terminal("EW-GEN-TRM-1")),
    ("ew.iec.thermostat-nc@qet10", Path::Device("B", MULTI)),
    ("ew.iec.transformer-1ph@qet10", Path::Device("T", MULTI)),
    ("ew.iec.valve-coil@qet10", Path::Part("EW-GEN-VLV-1", 0, MULTI)),
    ("ew.iso.cylinder-double-acting@qet10", Path::Device("M", FLUID)),
    ("ew.iso.process-valve-2way@qet10", Path::Device("Q", FLUID)),
    (
        "ew.iso.valve-5-2-sol-spring@qet10",
        Path::Part("EW-GEN-VLV-1", 1, FLUID),
    ),
    ("ew.ovw.module@qet10", Path::DeviceBox("A", OVERVIEW)),
    ("ew.ovw.valve-top@qet10", Path::DeviceBox("Q", OVERVIEW)),
];

/// The starter symbols of the `@qet10` family, the family the picker lists on a page of the new project's grid.
///
/// The starter ships every symbol in two grid families, `<key>` and `<key>@qet10`, with the same function definition;
/// the two must name the same symbols, so a symbol added to either family needs a path here.
fn starter_symbols() -> BTreeSet<&'static str> {
    let symbols = || starter::items().iter().filter(|i| i.kind == StarterKind::Symbol);
    let qet: BTreeSet<&str> = symbols().filter_map(|i| i.key.strip_suffix("@qet10")).collect();
    let native: BTreeSet<&str> = symbols()
        .filter(|i| !i.key.contains('@'))
        .map(|i| i.key)
        .collect();
    assert_eq!(qet, native, "the two grid families name the same symbols");
    symbols()
        .map(|i| i.key)
        .filter(|k| k.ends_with("@qet10"))
        .collect()
}

/// A new project with one page of each type a path uses: 101 multi-line, 102 fluid, 103 overview, 104 bus topology,
/// and the strip `-X`.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    for (folio, ty) in [("101", MULTI), ("102", FLUID), ("103", OVERVIEW), ("104", BUS)] {
        done(
            &mut s,
            "page.new",
            json!({"folio": folio, "type": ty, "title": ty}),
        );
    }
    done(&mut s, "strip.new", json!({"letter": "X"}));
    s
}

fn goto(s: &mut ShellModel, ty: &str) {
    let folio = match ty {
        MULTI => "101",
        FLUID => "102",
        OVERVIEW => "103",
        _ => "104",
    };
    done(s, "page.goto", json!({ "folio": folio }));
    done(s, "view.zoom_actual", json!({}));
}

/// The sheet point of grid index `k`.
fn grid(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
}

/// A placement click with Alt (no snapping to a neighbour) at grid index `k`.
fn place_at(s: &mut ShellModel, k: (i64, i64)) {
    let p = grid(s, k);
    done(s, "canvas.press", at(p, json!({"alt": true})));
    done(s, "canvas.release", at(p, json!({"alt": true})));
}

fn reps(s: &ShellModel) -> Vec<TableRep> {
    s.canvas().unwrap().page().tables.reps.clone()
}

/// The representations of this page that are new since `before`.
fn new_reps(s: &ShellModel, before: &[TableRep]) -> Vec<TableRep> {
    reps(s)
        .into_iter()
        .filter(|r| before.iter().all(|b| b.id != r.id))
        .collect()
}

/// The tag a representation prints (its first CP's label before `:`).
fn tag_of(s: &ShellModel, rep: &TableRep) -> String {
    let t = &s.canvas().unwrap().page().tables;
    let label = &t.cps.iter().find(|c| c.rep == rep.id).unwrap().label;
    label.rsplit_once(':').unwrap().0.to_owned()
}

/// Draws a box from grid index `k` with the armed box tool: two clicks.
fn draw_box(s: &mut ShellModel, key: &str, (x, y): (i64, i64)) {
    assert_eq!(s.tool().name(), "black_box", "{key}");
    let (a, b) = (grid(s, (x, y)), grid(s, (x + 12, y + 8)));
    click(s, a, json!({}));
    click(s, b, json!({}));
}

/// What a path placed: the representation of the symbol, the part number its device must carry, and the device it
/// must join.
struct Placed {
    key: &'static str,
    rep: TableRep,
    part: Option<&'static str>,
    joins: Option<Uuid>,
}

#[test]
fn every_starter_symbol_reaches_a_usable_device_from_the_picker() {
    let tabled: BTreeSet<&str> = PATHS.iter().map(|(k, _)| *k).collect();
    assert_eq!(
        tabled,
        starter_symbols(),
        "every starter symbol needs a path in PATHS (WP-S2.18 D7)"
    );
    let tmp = Tmp::new("starter-symbols");
    let mut s = new_project(&tmp);
    let mut placed: Vec<Placed> = Vec::new();
    let mut k = 0i64;
    let mut next = || {
        let cell = (8 + (k % 6) * 24, 10 + (k / 6) * 24);
        k += 1;
        cell
    };
    // The relay the auxiliary contacts join.
    goto(&mut s, MULTI);
    let relay = {
        let before = reps(&s);
        done(
            &mut s,
            "place.pick",
            json!({"part": "EW-GEN-RLY-1", "template": 0}),
        );
        place_at(&mut s, next());
        done(&mut s, "ui.dismiss", json!({}));
        let [rep] = new_reps(&s, &before).try_into().expect("one relay coil");
        rep
    };
    let relay_tag = tag_of(&s, &relay);
    for &(key, path) in PATHS {
        let before = {
            let page = match path {
                Path::Part(_, _, page) | Path::Device(_, page) | Path::DeviceBox(_, page) => page,
                Path::Aid => continue,
                _ => MULTI,
            };
            goto(&mut s, page);
            reps(&s)
        };
        let (part, joins) = match path {
            Path::Part(number, template, _) => {
                done(
                    &mut s,
                    "place.pick",
                    json!({"part": number, "template": template}),
                );
                place_at(&mut s, next());
                (Some(number), None)
            }
            Path::Terminal(number) => {
                done(&mut s, "place.pick", json!({ "part": number }));
                let picker = s.picker().unwrap_or_else(|| panic!("{key}: the strip chooser"));
                let row = picker
                    .rows
                    .iter()
                    .position(|r| matches!(r, PickerRow::Strip(_)))
                    .unwrap_or_else(|| panic!("{key}: the strip -X in {:?}", picker.rows));
                done(&mut s, "picker.choose", json!({ "row": row }));
                place_at(&mut s, next());
                (Some(number), None)
            }
            Path::BlackBox(number) => {
                done(&mut s, "place.pick", json!({ "part": number }));
                draw_box(&mut s, key, next());
                (Some(number), None)
            }
            Path::DeviceBox(letter, _) => {
                done(&mut s, "place.pick", json!({"symbol": key, "letter": letter}));
                draw_box(&mut s, key, next());
                (None, None)
            }
            Path::Join => {
                done(&mut s, "place.pick", json!({"symbol": key, "letter": "K"}));
                done(&mut s, "place.join", json!({}));
                done(&mut s, "chooser.type", json!({ "text": relay_tag }));
                done(&mut s, "chooser.choose", json!({}));
                place_at(&mut s, next());
                (None, relay.device)
            }
            Path::Device(letter, _) => {
                done(&mut s, "place.pick", json!({"symbol": key, "letter": letter}));
                place_at(&mut s, next());
                (None, None)
            }
            Path::Aid => unreachable!(),
        };
        done(&mut s, "ui.dismiss", json!({}));
        done(&mut s, "ui.dismiss", json!({}));
        let new = new_reps(&s, &before);
        let rep = new
            .iter()
            .find(|r| r.symbol_key == key)
            .unwrap_or_else(|| panic!("{key} ({path:?}) placed nothing of its own: {new:?}"))
            .clone();
        assert!(rep.device.is_some(), "{key}: a device");
        if let Some(d) = joins {
            assert_eq!(rep.device, Some(d), "{key} joins the relay {relay_tag}");
        }
        placed.push(Placed {
            key,
            rep,
            part,
            joins,
        });
    }
    // The relay's own coil carries the relay part; the aids are no device symbol.
    placed.push(Placed {
        key: "ew.iec.coil@qet10",
        rep: relay.clone(),
        part: Some("EW-GEN-RLY-1"),
        joins: None,
    });
    done(&mut s, "file.save", json!({}));
    let (session, _) = Session::open_read_only(&tmp.0.join("new.ew"), "test").unwrap();
    let q = session.query();
    for p in &placed {
        let device = p.rep.device.unwrap();
        let (_, d) = q
            .devices()
            .iter()
            .find(|(id, _)| id.as_uuid() == device)
            .unwrap_or_else(|| panic!("{}: the device after reopening", p.key));
        let numbers: Vec<String> = d
            .parts
            .iter()
            .filter_map(|u| {
                q.parts()
                    .iter()
                    .find(|(id, _)| id.as_uuid() == u.part.copy.as_uuid())
                    .map(|(_, c)| c.item.part_number.as_str().to_owned())
            })
            .collect();
        assert_eq!(numbers.len(), d.parts.len(), "{}: every part use is held", p.key);
        match (p.part, p.joins) {
            (Some(n), _) => assert_eq!(numbers, [n], "{}", p.key),
            (None, Some(_)) => assert_eq!(numbers, ["EW-GEN-RLY-1"], "{}: the relay's part", p.key),
            (None, None) => assert!(numbers.is_empty(), "{}: {numbers:?}", p.key),
        }
    }
    let findings: Vec<String> = checks::run(&q, &checks::FULL, checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-CORE-042")
        .map(|f| f.message)
        .collect();
    assert!(
        findings.is_empty(),
        "the starter parts are complete: {findings:?}"
    );
}

/// FT-659: the letter a new project (identifier set EN 81346-2:2009) gives each generic starter part placed from the
/// picker without a letter of its own: the selector switch S (a manual command), the valve's coil M (a solenoid
/// actuator), not the switching device's Q or the relay's K.
#[test]
fn a_new_project_gives_each_starter_part_its_iec_81346_letter() {
    let tmp = Tmp::new("starter-letters");
    let mut s = new_project(&tmp);
    goto(&mut s, MULTI);
    for (k, (number, letter)) in (0i64..).zip([
        ("EW-GEN-RLY-1", "K"),
        ("EW-GEN-PB-1", "S"),
        ("EW-GEN-SEL-1", "S"),
        ("EW-GEN-POS-1", "B"),
        ("EW-GEN-LMP-1", "E"),
        ("EW-GEN-VLV-1", "M"),
    ]) {
        let before = reps(&s);
        done(&mut s, "place.pick", json!({"part": number, "template": 0}));
        place_at(&mut s, (8 + k * 24, 10));
        done(&mut s, "ui.dismiss", json!({}));
        done(&mut s, "ui.dismiss", json!({}));
        let new = new_reps(&s, &before);
        let rep = new.first().unwrap_or_else(|| panic!("{number} placed nothing"));
        let tag = tag_of(&s, rep);
        let got: String = tag
            .trim_start_matches('-')
            .chars()
            .skip_while(char::is_ascii_digit)
            .take_while(char::is_ascii_uppercase)
            .collect();
        assert_eq!(got, letter, "{number}: tag {tag}");
    }
}

/// A drawing aid is no device symbol: its function definition is no device function (no letter, no slot).
#[test]
fn the_aids_are_no_device_symbols() {
    let tmp = Tmp::new("starter-aids");
    let mut s = new_project(&tmp);
    goto(&mut s, MULTI);
    for &(key, path) in PATHS {
        if !matches!(path, Path::Aid) {
            continue;
        }
        let before = reps(&s);
        let _ = s.dispatch("place.pick", &json!({"symbol": key, "letter": "A"}));
        let p = grid(&s, (40, 40));
        let _ = s.dispatch("canvas.press", &at(p, json!({"alt": true})));
        let _ = s.dispatch("canvas.release", &at(p, json!({"alt": true})));
        let _ = s.dispatch("ui.dismiss", &json!({}));
        let _ = s.dispatch("ui.dismiss", &json!({}));
        assert!(
            new_reps(&s, &before).iter().all(|r| r.device.is_none()),
            "{key} is an aid, it gives no device"
        );
    }
}

/// The text of the selection's Parts row.
fn parts_text(s: &ShellModel) -> String {
    let sheet = s.properties().sheet().expect("a sheet");
    let (_, row) = find_row(sheet, "device/parts").expect("the Parts row");
    match &row.value {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    }
}

/// WP-S2.18 D6: the Parts row of a device in a new project offers the starter parts after the project's own (none
/// here); choosing one embeds it with its closure and assigns it in one undo step.
#[test]
fn the_parts_row_takes_a_starter_part_in_one_undo_step() {
    let tmp = Tmp::new("starter-parts-row");
    let mut s = new_project(&tmp);
    goto(&mut s, MULTI);
    let before = reps(&s);
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.lamp@qet10", "letter": "P"}),
    );
    place_at(&mut s, (20, 20));
    done(&mut s, "ui.dismiss", json!({}));
    let [lamp] = new_reps(&s, &before).try_into().expect("one lamp");
    let tag = tag_of(&s, &lamp);
    done(&mut s, "select.set", json!({ "refs": [format!("tag:{tag}")] }));
    let empty = parts_text(&s);
    done(&mut s, "props.edit", json!({"row": "device/parts"}));
    let options: Vec<String> = s
        .properties()
        .choices()
        .map(|c| c.items.iter().map(|(t, _)| t.clone()).collect())
        .unwrap_or_default();
    assert_eq!(options.len(), 16, "{options:?}");
    assert!(
        options.contains(&"EW-GEN-LMP-1 (Generic)".to_owned()),
        "{options:?}"
    );
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "select.set", json!({ "refs": [format!("tag:{tag}")] }));
    let rev = s.revision();
    done(
        &mut s,
        "props.set",
        json!({"row": "device/parts", "value": "EW-GEN-LMP-1"}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(parts_text(&s), "EW-GEN-LMP-1 × 1");
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(parts_text(&s), empty, "one undo step takes the part back");
    done(&mut s, "edit.redo", json!({}));
    assert_eq!(parts_text(&s), "EW-GEN-LMP-1 × 1");
}

/// FT-655: the starter relay templates its four changeover contacts. Placing the relay's coil makes the four
/// contacts as unplaced slots; each contact symbol joined to the relay fills the next slot, and every contact prints
/// its own designations (11-12-14, 21-22-24, ...) on the CPs its symbol draws, not the first contact's.
#[test]
fn the_starter_relays_contacts_are_its_template_slots_with_their_own_designations() {
    let tmp = Tmp::new("starter-relay-contacts");
    let mut s = new_project(&tmp);
    goto(&mut s, MULTI);
    let before = reps(&s);
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-RLY-1", "template": 0}),
    );
    place_at(&mut s, (8, 10));
    done(&mut s, "ui.dismiss", json!({}));
    let [relay] = new_reps(&s, &before).try_into().expect("one relay coil");
    let tag = tag_of(&s, &relay);
    let mut printed: Vec<Vec<String>> = Vec::new();
    for k in 0..4i64 {
        let before = reps(&s);
        done(
            &mut s,
            "place.pick",
            json!({"symbol": "ew.iec.contact-co@qet10", "letter": "K"}),
        );
        done(&mut s, "place.join", json!({}));
        done(&mut s, "chooser.type", json!({ "text": tag }));
        done(&mut s, "chooser.choose", json!({}));
        place_at(&mut s, (32 + 24 * k, 10));
        done(&mut s, "ui.dismiss", json!({}));
        done(&mut s, "ui.dismiss", json!({}));
        let [contact] = new_reps(&s, &before).try_into().expect("one contact");
        assert_eq!(contact.device, relay.device, "contact {k} joins the relay");
        let t = &s.canvas().unwrap().page().tables;
        let mut cps: Vec<(u16, String)> = t
            .cps
            .iter()
            .filter(|c| c.rep == contact.id)
            .map(|c| (c.idx, c.label.rsplit_once(':').unwrap().1.to_owned()))
            .collect();
        cps.sort();
        printed.push(cps.into_iter().map(|(_, d)| d).collect());
    }
    assert_eq!(
        printed,
        [
            ["11", "12", "14"],
            ["21", "22", "24"],
            ["31", "32", "34"],
            ["41", "42", "44"],
        ],
        "each contact fills its own template slot"
    );
}

/// Whether the project holds the relay EW-GEN-RLY-1 and the coil symbol, as the picker's catalogue lists them (a
/// library part the project does not hold has `library`, a starter symbol it does not hold no `copy`).
fn relay_held(s: &mut ShellModel) -> (bool, bool) {
    done(s, "tool.place", json!({}));
    let cat = std::sync::Arc::clone(&s.picker().expect("the picker").catalogue);
    done(s, "ui.dismiss", json!({}));
    let part = cat
        .parts
        .iter()
        .find(|p| p.part_number == "EW-GEN-RLY-1")
        .expect("the relay row");
    let symbol = cat
        .symbols
        .iter()
        .find(|r| r.key == "ew.iec.coil@qet10")
        .expect("the coil row");
    (part.library.is_none(), symbol.copy.is_some())
}

/// FT-656: placing EW-GEN-RLY-1 from the picker in a new project is one commit, so one undo step: the placement
/// carries the relay's closure (the part, its FnDefs and manufacturer, the coil symbol), and undo takes the device
/// and the copies back together. The recorded command alone places the relay in another new project.
#[test]
fn a_library_part_from_the_picker_is_one_undo_step_and_its_command_replays() {
    let tmp = Tmp::new("ft656-relay");
    let mut s = new_project(&tmp);
    goto(&mut s, MULTI);
    assert_eq!(relay_held(&mut s), (false, false), "a new project holds neither");
    let before = reps(&s);
    let rev = s.revision();
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-RLY-1", "template": 0}),
    );
    assert_eq!(s.revision(), rev, "the pick commits nothing");
    place_at(&mut s, (8, 10));
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    let placed = s.recorded_commands().last().expect("the placement").clone();
    assert_eq!(placed.cmd, "device.place");
    let kinds: Vec<&str> = placed.params["copies"]
        .as_array()
        .expect("copies")
        .iter()
        .map(|c| c["$k"].as_str().unwrap_or_default())
        .collect();
    for kind in ["symbol", "fndef", "part", "manufacturer"] {
        assert!(kinds.contains(&kind), "{kind} in {kinds:?}");
    }
    assert_eq!(new_reps(&s, &before).len(), 1, "the coil");
    assert_eq!(relay_held(&mut s), (true, true));
    // One undo: the device and the copies go together.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 2), "the undo is one step");
    assert!(new_reps(&s, &before).is_empty(), "the coil is gone");
    assert_eq!(relay_held(&mut s), (false, false), "and so are the copies");
    // The recorded command, on another new project's page, places the relay without a library lookup.
    let tmp2 = Tmp::new("ft656-replay");
    let mut t = new_project(&tmp2);
    goto(&mut t, MULTI);
    let mut params = placed.params.clone();
    params["page"] = json!(t.canvas().unwrap().page().tables.page.to_string());
    let before = reps(&t);
    done(
        &mut t,
        "command.run",
        json!({"cmd": "device.place", "params": params}),
    );
    let new = new_reps(&t, &before);
    assert_eq!(new.len(), 1, "the replayed coil");
    assert_eq!(new[0].symbol_key, "ew.iec.coil@qet10");
    assert_eq!(relay_held(&mut t), (true, true));
}
