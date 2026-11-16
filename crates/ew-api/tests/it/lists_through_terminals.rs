// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The lists tell what the drawing connects (rc.2 persona run, entries 33 and 34): the starter CPU and input card drawn
//! with the Black box tool, the card's pins wired through starter terminals to two starter push buttons, one of them
//! through a cable core. The terminal diagram names what is on each clamp; the PLC overview, the I/O list and the PLC
//! grid call both channels used by their push button, not spare.

use ew_api::output::{NavKind, RowExport, row_export};
use ew_api::{AnyCommand, Session};
use ew_derive::DeviceTag;
use ew_ids::{Id, Uuid};
use ew_schema::{Channel, Device};
use serde_json::{Value, json};

use crate::common::{first_page, library_part, new_project};
use crate::plc_window::place as place_box;

/// Three steps of the page's connection grid.
const STEP: i64 = 7_937_500;

fn run(s: &mut Session, name: &str, params: Value) {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e:?}"));
}

/// The one ID of `after` that `before` lacks.
fn made(before: Vec<Uuid>, after: Vec<Uuid>) -> Uuid {
    let new: Vec<Uuid> = after.into_iter().filter(|id| !before.contains(id)).collect();
    let [id] = new.as_slice() else {
        panic!("one new record: {new:?}")
    };
    *id
}

fn devices(s: &Session) -> Vec<Uuid> {
    s.query().devices().iter().map(|(id, _)| id.as_uuid()).collect()
}

fn connections(s: &Session) -> Vec<Uuid> {
    s.query().connections().iter().map(|(id, _)| id.as_uuid()).collect()
}

/// The one function of device `d`.
fn function_of(s: &Session, d: Uuid) -> Uuid {
    let q = s.query();
    let fs: Vec<Uuid> = q
        .idx()
        .functions_of_device(Id::from_uuid(d))
        .map(|f| f.as_uuid())
        .collect();
    let [f] = fs.as_slice() else {
        panic!("one function: {fs:?}")
    };
    *f
}

fn tag(s: &Session, d: Uuid) -> String {
    s.query()
        .get::<DeviceTag>(Id::<Device>::from_uuid(d))
        .as_ref()
        .as_ref()
        .map(|t| t.short())
        .expect("a tag")
}

fn cp(f: Uuid, idx: u16) -> Value {
    json!({"cp": {"function": f.to_string(), "idx": idx}})
}

/// Places starter part `number` drawn by the starter symbol `key` at `x` with `target` (its `part` filled in); the new
/// device.
fn place(s: &mut Session, number: &str, key: &str, mut target: Value, x: i64) -> Uuid {
    let page = first_page(s);
    let part = library_part(s, number);
    let copies = s
        .starter_embed_part(part, key)
        .expect("the closure resolves")
        .unwrap_or_else(|| json!({"copies": []}));
    let symbol = s.starter_symbol_copy(key).expect("a starter symbol");
    if let Some(t) = target.as_object_mut().and_then(|t| t.values_mut().next()) {
        t["part"] = json!(part.to_string());
    }
    let before = devices(s);
    run(
        s,
        "device.place",
        json!({"page": page.to_string(), "symbol": symbol.to_string(), "pos": [x, 30 * STEP],
            "target": target, "copies": copies["copies"]}),
    );
    made(before, devices(s))
}

/// The CSV rows of `csv` as cells by column name, the data rows only.
fn rows(csv: &str) -> Vec<Vec<(String, String)>> {
    let mut lines = csv.lines();
    let header: Vec<String> = lines.next().expect("a header").split(',').map(str::to_owned).collect();
    lines
        .filter(|l| l.starts_with("data,"))
        .map(|l| header.iter().cloned().zip(l.split(',').map(str::to_owned)).collect())
        .collect()
}

fn get<'r>(row: &'r [(String, String)], column: &str) -> &'r str {
    row.iter()
        .find(|(c, _)| c.starts_with(column))
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("no column {column}: {row:?}"))
}

/// Entries 33 and 34: input pins 1 and 2 of the card (channels 0 and 1) wired to clamp b of terminals 1 and 2; clamp a
/// of terminal 1 wired to push button A's contact 13, clamp a of terminal 2 to push button B's contact 13 through core
/// BN of a cable. The terminal diagram names each button's contact (with the cable and core) on side 1 and the card's
/// pin on side 2; the PLC overview, the I/O list and the PLC grid call channels 0 and 1 used by their button, with its
/// function text, and the other channels spare.
#[test]
fn a_channel_wired_through_a_terminal_is_used_and_the_terminal_names_both_ends() {
    let mut s = new_project(3301);
    let page = first_page(&s).to_string();
    place_box(&mut s, "EW-GEN-CPU-1", 1);
    let card = place_box(&mut s, "EW-GEN-DI-1", 2);
    let card_fn = function_of(&s, card);
    let before = devices(&s);
    run(&mut s, "device.create", json!({"letter": "X", "strip": {}}));
    let strip = made(before, devices(&s));
    let mut ends = Vec::new();
    for (n, button) in [(1, "Start"), (2, "Stop")] {
        let terminal = place(
            &mut s,
            "EW-GEN-TRM-1",
            "ew.iec.terminal@qet10",
            json!({"new_terminal": {"strip": strip.to_string(), "designation": n.to_string()}}),
            (4 * n + 1) * STEP,
        );
        let button_dev = place(
            &mut s,
            "EW-GEN-PB-1",
            "ew.iec.pushbutton-no@qet10",
            json!({"from_part": {"letter": "S", "template": 0}}),
            (4 * n + 20) * STEP,
        );
        run(
            &mut s,
            "fields.set",
            json!({"kind": "device", "targets": [button_dev.to_string()], "path": "function_text",
                "value": button}),
        );
        let (terminal_fn, button_fn) = (function_of(&s, terminal), function_of(&s, button_dev));
        let idx = u16::try_from(n - 1).expect("a pin");
        run(
            &mut s,
            "connection.draw",
            json!({"a": cp(card_fn, idx), "b": cp(terminal_fn, 1), "page": page}),
        );
        let before = connections(&s);
        run(
            &mut s,
            "connection.draw",
            json!({"a": cp(terminal_fn, 0), "b": cp(button_fn, 0), "page": page}),
        );
        ends.push((button_dev, made(before, connections(&s))));
    }
    let before = devices(&s);
    run(
        &mut s,
        "cable.create",
        json!({"conductors": [{"core": "BN", "wires": [ends[1].1.to_string()]}]}),
    );
    let cable = made(before, devices(&s));
    let (card_tag, cable_tag) = (tag(&s, card), tag(&s, cable));
    let buttons: Vec<String> = ends.iter().map(|(d, _)| tag(&s, *d)).collect();

    // The terminal diagram, a block on the shipped frame as the new-project wizard makes it.
    let [frame, title]: [Value; 2] = [
        include_str!("../../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"),
        include_str!("../../../../content/forms/ew.titleblock.qet-a4.json"),
    ]
    .map(|t| serde_json::from_str(t).expect("a form"));
    run(
        &mut s,
        "report_block.create",
        json!({"source": {"kind": "terminal_diagram"}, "form": frame["id"], "form_copies": [frame, title],
            "first_folio": 903, "title": "Terminal diagram {n}"}),
    );
    let q = s.query();
    let terminals = row_export(&q, &RowExport::Terminals { strip: None }, None).expect("the terminal diagram");
    let sides: Vec<(String, String)> = rows(&terminals.csv)
        .iter()
        .map(|r| (get(r, "Side 1").to_owned(), get(r, "Side 2").to_owned()))
        .collect();
    assert_eq!(
        sides,
        [
            (format!("{}:13", buttons[0]), format!("{card_tag}:1")),
            (format!("{}:13 via {cable_tag} BN", buttons[1]), format!("{card_tag}:2")),
        ],
        "{}",
        terminals.csv
    );

    // The PLC overview and the I/O list: channels 0 and 1 used by their button, the others spare.
    for kind in ["plc_overview", "io_list"] {
        let out = row_export(&q, &RowExport::Kind(kind.into()), None).expect(kind);
        let card_rows: Vec<Vec<(String, String)>> = rows(&out.csv)
            .into_iter()
            .filter(|r| get(r, "Module") == card_tag)
            .collect();
        let seen: Vec<(&str, &str, &str, &str)> = card_rows
            .iter()
            .map(|r| (get(r, "Channel"), get(r, "State"), get(r, "Target"), get(r, "Function text")))
            .collect();
        let mut want = vec![
            ("0", "used", buttons[0].as_str(), "Start"),
            ("1", "used", buttons[1].as_str(), "Stop"),
        ];
        want.extend(["2", "3", "4", "5", "6", "7"].map(|n| (n, "spare", "", "")));
        assert_eq!(seen, want, "{kind}");
    }

    // The PLC grid: the same two channels used by their button.
    drop(q);
    let grid = s.navigator_table(NavKind::PlcGrid);
    let col = |key: &str| grid.columns.iter().position(|c| c.key == key).expect(key);
    for (n, button) in buttons.iter().enumerate() {
        let channel = ew_ids::v5::plc_channel_id::<Device, Channel>(Id::from_uuid(card), u16::try_from(n).expect("n"));
        let node = grid
            .nodes
            .iter()
            .find(|r| r.records.first() == Some(&channel.as_uuid()))
            .expect("the channel's row");
        assert_eq!(
            (node.cells[col("state")].as_str(), node.cells[col("target")].as_str()),
            ("used", button.as_str()),
            "channel {n}"
        );
    }
}
