// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strip editor below the shell on the sample: the strips table's rows, columns and accessory rows
//! (SE7), the bridge groups (SE4), the part resolver's kinds and ambiguity (SE5), the group labels
//! (SE8) and the commands `Session::strip_edit` compiles.

use crate::common;

use ew_api::output::{NavKind, NavNode, NavRole, NavTable, RowValue, StripEdit, StripEditError, Uuid};
use ew_api::{AnyCommand, Origin, Session};
use ew_ids::Id;
use ew_schema::Device;
use serde_json::{Value, json};

fn children<'t>(t: &'t NavTable, parent: &NavNode) -> Vec<&'t NavNode> {
    let at = t.nodes.iter().position(|n| n.key == parent.key).unwrap();
    t.nodes
        .iter()
        .filter(|n| n.parent == Some(u32::try_from(at).unwrap()))
        .collect()
}

fn strip_node<'t>(t: &'t NavTable, label: &str) -> &'t NavNode {
    t.nodes
        .iter()
        .find(|n| n.role == NavRole::Strip && n.label == label)
        .unwrap_or_else(|| panic!("no strip {label}"))
}

/// The strip device of a printed strip tag.
fn strip(s: &Session, label: &str) -> Uuid {
    strip_node(&s.navigator_table(NavKind::Strips), label).records[0]
}

/// The terminal device `<strip>:<designation>`.
fn terminal(s: &Session, strip_label: &str, designation: &str) -> Uuid {
    let t = s.navigator_table(NavKind::Strips);
    children(&t, strip_node(&t, strip_label))
        .into_iter()
        .find(|n| n.role == NavRole::Terminal && n.label == designation)
        .unwrap_or_else(|| panic!("no terminal {strip_label}:{designation}"))
        .records[0]
}

/// The (from, to, label) of a strip's groups.
fn groups(s: &Session, strip: Uuid) -> Vec<(u32, u32, String)> {
    let q = s.query();
    q.devices()
        .get(Id::<Device>::from_uuid(strip))
        .and_then(|d| d.aspects.strip.as_ref())
        .map(|a| {
            a.groups
                .iter()
                .map(|g| {
                    (
                        g.from_seq,
                        g.to_seq,
                        g.label.as_neutral().unwrap_or_default().to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A group as (from, to, label).
type Group = (u32, u32, String);

fn g(from: u32, to: u32, label: &str) -> Group {
    (from, to, label.to_owned())
}

/// The part copy ID of a part number and kind.
fn copy_of(s: &Session, number: &str, kind: &str) -> Option<String> {
    let q = s.query();
    q.parts()
        .iter()
        .find(|(_, p)| {
            p.item.part_number.as_str() == number
                && serde_json::to_value(&p.item.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .as_deref()
                    == Some(kind)
        })
        .map(|(id, _)| id.as_uuid().to_string())
}

/// A TBD part copy as the blind task's prelude embeds it.
fn tbd_copy(id: &str, item: &str, kind: &str, description: &str, key: &str) -> Value {
    json!({
        "$k": "part", "description": description, "id": id, "item_id": item, "kind": kind,
        "library_id": "d76f5765-c456-5393-935c-4cec32a87b01",
        "manufacturer": {"item": "9ec896f8-f264-52e9-9bc2-cefa41e0435f", "key": "TBD",
            "library": "d76f5765-c456-5393-935c-4cec32a87b01", "version_req": "*"},
        "meta": {"key": key}, "part_number": "TBD", "version": "1.0.0"
    })
}

/// The copy with its copy identity and the hash of its item (EW-CORE-014), so that `library.embed` takes it.
fn consistent(mut v: Value) -> Value {
    // Another company library than the project's own (a project-local item is not embedded).
    v["library_id"] = json!("0b6e3a2c-0000-5000-8000-000000000003");
    let lib: ew_ids::Uuid = v["library_id"].as_str().unwrap().parse().unwrap();
    let item: ew_ids::Uuid = v["item_id"].as_str().unwrap().parse().unwrap();
    let id: Id<()> = ew_ids::v5::lib_copy_id(lib, item);
    v["id"] = json!(id.as_uuid().to_string());
    v["sha256"] = json!(format!("sha256:{}", "0".repeat(64)));
    let doc = ew_schema::LibDocument::from_json(&v.to_string()).expect("a part copy");
    let hash = ew_format::library_copy_hash(&doc).expect("a hash");
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    v["sha256"] = json!(format!("sha256:{hex}"));
    v
}

/// Embeds the two TBD copies of the blind task's prelude (an end bracket and a plug-in bridge) when the project lacks them.
fn with_tbd_parts(s: &mut Session) -> (String, String) {
    if copy_of(s, "TBD", "end_bracket").is_none() {
        let copy = |id: &str, item: &str, kind: &str, description: &str, key: &str| {
            consistent(tbd_copy(id, item, kind, description, key))
        };
        s.execute(&AnyCommand {
            name: "library.embed".into(),
            params: json!({"copies": [
                copy("d838425d-a9e4-57fc-9107-7b66ce4a6ac6", "05d17a0f-8f9c-5a23-9226-465f95f4c2c3", "accessory",
                    "Plug-in bridge", "TBD plug-in bridge"),
                copy("c236ee7d-ff98-5754-9e02-64260efbfabf", "f55cd650-f081-5fad-a791-375bfb4d9a11", "end_bracket",
                    "End bracket", "TBD end bracket"),
            ]}),
        })
        .expect("library.embed of the TBD parts");
    }
    (
        copy_of(s, "TBD", "end_bracket").expect("the end bracket"),
        copy_of(s, "TBD", "accessory").expect("the plug-in bridge"),
    )
}

fn run(s: &mut Session, cmd: &AnyCommand) {
    s.execute(cmd).unwrap_or_else(|e| panic!("{}: {e}", cmd.name));
}

/// The terminals `from` to `to` of -X1021.
fn ts(s: &Session, from: u32, to: u32) -> Vec<Uuid> {
    (from..=to)
        .map(|n| terminal(s, "-X1021", &n.to_string()))
        .collect()
}

/// The sample with -X1021's terminals grouped: 1 to 4 "Supply", 5 to 8 "Spare" (the sample's strips have no groups).
fn with_groups() -> Session {
    let mut s = common::sample();
    for (from, to, label) in [(1, 4, "Supply"), (5, 8, "Spare")] {
        let cmd = s
            .strip_edit(&StripEdit::GroupLabel {
                terminals: ts(&s, from, to),
                label: label.to_owned(),
            })
            .expect("compiles");
        run(&mut s, &cmd);
    }
    s
}

/// SE8: the three examples on -X1021, each one `fields.set` of `strip.groups` and one undo step.
#[test]
fn group_labels_rewrite_the_groups() {
    let mut s = with_groups();
    let x = strip(&s, "-X1021");
    let before = groups(&s, x);
    assert_eq!(before, [g(1, 4, "Supply"), g(5, 8, "Spare")]);
    let cases: [(Vec<Uuid>, &str, Vec<Group>); 3] = [
        (
            ts(&s, 7, 8),
            "Reserve",
            vec![g(1, 4, "Supply"), g(5, 6, "Spare"), g(7, 8, "Reserve")],
        ),
        (ts(&s, 5, 8), "Supply", vec![g(1, 8, "Supply")]),
        (ts(&s, 5, 8), "", vec![g(1, 4, "Supply")]),
    ];
    for (terminals, label, want) in cases {
        let cmd = s
            .strip_edit(&StripEdit::GroupLabel {
                terminals,
                label: label.to_owned(),
            })
            .expect("compiles");
        assert_eq!(cmd.name, "fields.set", "{label:?}: one fields.set");
        assert_eq!(cmd.params["path"], "strip.groups");
        assert_eq!(cmd.params["targets"], json!([x.to_string()]));
        run(&mut s, &cmd);
        assert_eq!(groups(&s, x), want, "{label:?}");
        s.undo(&Origin::Gui).unwrap();
        assert_eq!(groups(&s, x), before, "{label:?}: one undo step");
    }
}

/// A terminal's Properties list "Printed label" and "Group" after Designation; the Group row shows the label.
#[test]
fn terminal_properties_show_the_printed_label_and_the_group() {
    let s = with_groups();
    let t7 = terminal(&s, "-X1021", "7");
    let sheet = s.property_sheet(&[t7]);
    let device = sheet
        .sections
        .iter()
        .find(|x| x.kind == "device")
        .expect("a device section");
    let keys: Vec<&str> = device.rows.iter().map(|r| r.key.as_str()).collect();
    let at = keys
        .iter()
        .position(|k| *k == "device/terminal.designation")
        .expect("Designation");
    assert_eq!(
        &keys[at + 1..at + 3],
        ["device/derived.printed_label", "device/derived.group"]
    );
    let printed = &device.rows[at + 1];
    let text = device
        .rows
        .iter()
        .find(|r| r.key == "device/function_text")
        .unwrap();
    assert_eq!(
        (&printed.path, &printed.route, &printed.value),
        (&text.path, &text.route, &text.value)
    );
    let group = &device.rows[at + 2];
    assert_eq!(
        group.value,
        RowValue::One {
            json: Value::String("Spare".into()),
            text: "Spare".into()
        }
    );
    assert!(group.route.is_editable());
    // Other devices: unchanged.
    let x = strip(&s, "-X1021");
    let sheet = s.property_sheet(&[x]);
    assert!(
        sheet
            .sections
            .iter()
            .flat_map(|x| &x.rows)
            .all(|r| r.key != "device/derived.group" && r.key != "device/derived.printed_label")
    );
}

/// SE4: `bridge_groups` names the sample's two bridge groups BR1 and BR2 as the wire list does: -X1021's terminals 1
/// to 4 (24 V) and 5 to 8 (0 V).
#[test]
fn bridge_groups_are_br1_and_br2() {
    let s = common::sample();
    let q = s.query();
    let groups = ew_derive::bridge_groups(&q);
    let labels: Vec<&str> = groups.iter().map(|(l, _, _)| l.as_str()).collect();
    assert_eq!(labels, ["BR1", "BR2"]);
    let x1021 = strip(&s, "-X1021");
    let on: Vec<usize> = groups
        .iter()
        .filter(|(_, st, _)| st.as_uuid() == x1021)
        .map(|(_, _, g)| g.terminals.len())
        .collect();
    assert_eq!(on, [4, 4], "-X1021: BR1 and BR2");
    let numbers: Vec<String> = groups
        .iter()
        .map(|(_, _, g)| g.part.as_ref().map(|p| ew_derive::printed_number(&q, p)).unwrap_or_default())
        .collect();
    assert_eq!(numbers, ["EW-GEN-BRG-4", "EW-GEN-BRG-4"], "the sample's bridges' parts");
}

/// SE5: the resolver's kind filter, the long form and the ambiguity.
#[test]
fn the_part_resolver_filters_by_kind_and_lists_every_match() {
    let mut s = common::sample();
    let (bracket, bridge) = with_tbd_parts(&mut s);
    let x = strip(&s, "-X1021");
    let t1 = terminal(&s, "-X1021", "1");
    // TBD among the end brackets: the end bracket.
    let cmd = s
        .strip_edit(&StripEdit::AddAccessory {
            strip: x,
            after_seq: 8,
            kind: "end_clamp".into(),
            part: Some("TBD".into()),
        })
        .expect("compiles");
    assert_eq!(cmd.name, "fields.set");
    assert_eq!(cmd.params["path"], "strip.accessories");
    assert_eq!(
        cmd.params["value"],
        json!([{"after_seq": 8, "kind": "end_clamp", "part": {"copy": bracket}}])
    );
    // TBD among the accessories: the plug-in bridge (on a bridged terminal's carrier).
    let cmd = s
        .strip_edit(&StripEdit::BridgePart {
            terminals: vec![t1],
            part: "TBD".into(),
        })
        .expect("compiles");
    assert_eq!(
        (cmd.name.as_str(), &cmd.params["path"]),
        ("fields.set", &json!("overrides.wire_part"))
    );
    assert_eq!(cmd.params["kind"], "connection");
    assert_eq!(cmd.params["value"], json!({"copy": bridge}));
    // The long form parses back.
    let cmd = s
        .strip_edit(&StripEdit::BridgePart {
            terminals: vec![t1],
            part: "TBD (TBD) · Plug-in bridge".into(),
        })
        .expect("the long form");
    assert_eq!(cmd.params["value"], json!({"copy": bridge}));
    // A partition plate takes accessories: TBD resolves to the bridge. A marker carrier takes markers and accessories.
    let cmd = s
        .strip_edit(&StripEdit::AddAccessory {
            strip: x,
            after_seq: 0,
            kind: "partition".into(),
            part: Some("TBD".into()),
        })
        .unwrap();
    assert_eq!(cmd.params["value"][0]["part"]["copy"], json!(bridge));
    // Unknown and unfit texts.
    let err = s
        .strip_edit(&StripEdit::BridgePart {
            terminals: vec![t1],
            part: "533356".into(),
        })
        .unwrap_err();
    assert!(
        matches!(err, StripEditError::Invalid(ref m) if m.to_string().contains("533356")),
        "{err:?}"
    );
    // Two TBD copies of one kind beside the starter's end bracket (the sample's terminals bring it): ambiguous, each listed
    // with the long form.
    let second = consistent(tbd_copy(
        "",
        "0b6e3a2c-0000-5000-8000-000000000002",
        "end_bracket",
        "End bracket, wide",
        "TBD end bracket wide",
    ));
    run(
        &mut s,
        &AnyCommand {
            name: "library.embed".into(),
            params: json!({ "copies": [second] }),
        },
    );
    let err = s
        .strip_edit(&StripEdit::AddAccessory {
            strip: x,
            after_seq: 8,
            kind: "end_clamp".into(),
            part: None,
        })
        .unwrap_err();
    let StripEditError::Ambiguous(choices) = err else {
        panic!("ambiguous: {err:?}")
    };
    let texts: Vec<&str> = choices.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "EW-GEN-EB-1 (Generic) · End bracket for TS 35",
            "TBD (TBD) · End bracket",
            "TBD (TBD) · End bracket, wide"
        ]
    );
    assert_eq!(choices[1].value, json!(format!("id:{bracket}")));
    // The choice's value resolves.
    let cmd = s
        .strip_edit(&StripEdit::AddAccessory {
            strip: x,
            after_seq: 8,
            kind: "end_clamp".into(),
            part: Some(format!("id:{bracket}")),
        })
        .unwrap();
    assert_eq!(cmd.params["value"][0]["part"]["copy"], json!(bracket));
}

/// No part of the kind refuses `reason-no-accessory-part` (a new project's strip); the one copy is taken without a
/// text (the sample holds the starter's end bracket, which its terminals bring with them).
#[test]
fn add_accessory_takes_the_one_copy_or_refuses() {
    let mut fresh = common::new_project(7);
    let receipt = common::run_cmd(&mut fresh, "device.create", json!({"letter": "X", "strip": {}}));
    let x = receipt.outcome.primary.expect("the strip").1.as_uuid();
    let err = fresh
        .strip_edit(&StripEdit::AddAccessory {
            strip: x,
            after_seq: 0,
            kind: "end_clamp".into(),
            part: None,
        })
        .unwrap_err();
    assert_eq!(err, StripEditError::Refused("reason-no-accessory-part"));
    let s = common::sample();
    let x = strip(&s, "-X1021");
    let bracket = copy_of(&s, "EW-GEN-EB-1", "end_bracket").expect("the starter's end bracket");
    let cmd = s
        .strip_edit(&StripEdit::AddAccessory {
            strip: x,
            after_seq: 8,
            kind: "end_clamp".into(),
            part: None,
        })
        .unwrap();
    assert_eq!(cmd.params["value"][0]["part"]["copy"], json!(bracket));
}

/// Accessory rows after_seq 0, 8 and two at 4, placed and numbered; the remove and part edits.
#[test]
fn accessory_rows_are_placed_and_numbered() {
    let mut s = common::sample();
    let (bracket, bridge) = with_tbd_parts(&mut s);
    let x = strip(&s, "-X1021");
    for (after_seq, kind, part) in [
        (8, "end_clamp", &bracket),
        (4, "partition", &bridge),
        (0, "end_clamp", &bracket),
        (4, "end_plate", &bridge),
    ] {
        let cmd = s
            .strip_edit(&StripEdit::AddAccessory {
                strip: x,
                after_seq,
                kind: kind.into(),
                part: Some(format!("id:{part}")),
            })
            .unwrap();
        run(&mut s, &cmd);
    }
    let t = s.navigator_table(NavKind::Strips);
    // The strip's own rows; what its terminals' parts bring stands beside them in rows of its own (`derived:`).
    let rows: Vec<(String, String, NavRole)> = children(&t, strip_node(&t, "-X1021"))
        .iter()
        .filter(|n| !n.key.contains("/derived:"))
        .map(|n| (n.cells[1].clone(), n.label.clone(), n.role))
        .collect();
    let row = |pos: &str, label: &str, role: NavRole| (pos.to_owned(), label.to_owned(), role);
    let mut want = vec![row("0.1", "End bracket", NavRole::Accessory)];
    for n in 1..=4 {
        want.push(row(&n.to_string(), &n.to_string(), NavRole::Terminal));
    }
    want.push(row("4.1", "Partition plate", NavRole::Accessory));
    want.push(row("4.2", "End plate", NavRole::Accessory));
    for n in 5..=8 {
        want.push(row(&n.to_string(), &n.to_string(), NavRole::Terminal));
    }
    want.push(row("8.1", "End bracket", NavRole::Accessory));
    assert_eq!(rows, want);
    // The accessory row: the strip, its part number, the facets `kind=accessory` and `strip`.
    let acc = children(&t, strip_node(&t, "-X1021"))
        .into_iter()
        .find(|n| n.role == NavRole::Accessory)
        .unwrap();
    assert!(acc.records.is_empty());
    assert!(acc.facets.contains(&("kind".to_owned(), "accessory".to_owned())));
    assert!(acc.facets.contains(&("strip".to_owned(), "-x1021".to_owned())));
    let part = t.columns.iter().position(|c| c.key == "part").unwrap();
    assert_eq!(acc.cells[part], "TBD");
    // The strip columns' strip-editor cells.
    let cell = |key: &str| t.columns.iter().find(|c| c.key == key).and_then(|c| c.strip_cell);
    assert_eq!(cell("group"), Some(ew_api::output::StripCell::Group));
    assert_eq!(cell("bridge_part"), Some(ew_api::output::StripCell::BridgePart));
    assert_eq!(cell("part"), Some(ew_api::output::StripCell::AccessoryPart));
    assert_eq!(cell("terminal"), None);
    // Removing the two at 4 (the list is kept sorted by `after_seq`: indexes 1 and 2): one fields.set of the rest.
    let cmd = s
        .strip_edit(&StripEdit::RemoveAccessories {
            strip: x,
            indexes: vec![1, 2],
        })
        .unwrap();
    assert_eq!(
        cmd.params["value"],
        json!([
            {"after_seq": 0, "kind": "end_clamp", "part": {"copy": bracket}},
            {"after_seq": 8, "kind": "end_clamp", "part": {"copy": bracket}},
        ])
    );
    // The part of accessory 0 among end brackets; a bridge copy does not fit.
    let err = s
        .strip_edit(&StripEdit::AccessoryPart {
            strip: x,
            index: 0,
            part: "TBD (TBD) · Plug-in bridge".into(),
        })
        .unwrap_err();
    assert!(matches!(err, StripEditError::Invalid(_)), "{err:?}");
}

/// SE7: the strips table's nodes are in the provider's order (the `Order` sort): -X1011, -X1021 1 to 8, -X1031;
/// every node of a strip carries the `strip` facet.
#[test]
fn the_provider_order_and_the_strip_facet() {
    let s = common::sample();
    let t = s.navigator_table(NavKind::Strips);
    let strips: Vec<&str> = t
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Strip)
        .map(|n| n.label.as_str())
        .collect();
    // Natural tag order (the sign sorts before the digits).
    assert_eq!(strips, ["-X1011", "-X1021", "-X1031"]);
    let x = strip_node(&t, "-X1021");
    let labels: Vec<String> = children(&t, x)
        .iter()
        .filter(|n| n.role == NavRole::Terminal)
        .map(|n| n.label.clone())
        .collect();
    let want: Vec<String> = (1..=8).map(|n| n.to_string()).collect();
    assert_eq!(labels, want);
    for n in children(&t, x) {
        assert!(n.facets.contains(&("strip".to_owned(), "-x1021".to_owned())));
    }
}

/// Insert compiles to `terminal.insert` with the reference terminal's definition and parts and the next whole
/// number; move to `terminal.move`; both alone.
#[test]
fn insert_and_move_compile_to_one_command() {
    let mut s = common::sample();
    let x = strip(&s, "-X1021");
    let t5 = terminal(&s, "-X1021", "5");
    let cmd = s
        .strip_edit(&StripEdit::Insert {
            strip: x,
            before: Some(t5),
            count: 2,
            designation: None,
        })
        .unwrap();
    assert_eq!(cmd.name, "terminal.insert");
    assert_eq!(cmd.params["before"], json!(t5.to_string()));
    assert_eq!(
        cmd.params["terminals"],
        json!([{"designation": "9"}, {"designation": "10"}])
    );
    assert!(cmd.params["fndef"].is_string());
    assert_eq!(
        cmd.params["parts"].as_array().map(Vec::len),
        Some(1),
        "EW-GEN-TRM-1"
    );
    run(&mut s, &cmd);
    let t = s.navigator_table(NavKind::Strips);
    let labels: Vec<String> = children(&t, strip_node(&t, "-X1021"))
        .iter()
        .filter(|n| n.role == NavRole::Terminal)
        .map(|n| n.label.clone())
        .collect();
    assert_eq!(&labels[3..7], ["4", "9", "10", "5"]);
    let err = s
        .strip_edit(&StripEdit::Insert {
            strip: x,
            before: None,
            count: 2,
            designation: Some("A".into()),
        })
        .unwrap_err();
    assert!(matches!(err, StripEditError::Invalid(_)));
    let t13 = terminal(&s, "-X1021", "8");
    let t10 = terminal(&s, "-X1021", "6");
    let cmd = s
        .strip_edit(&StripEdit::Move {
            terminals: vec![t13],
            before: Some(t10),
        })
        .unwrap();
    assert_eq!(
        (cmd.name.as_str(), &cmd.params),
        (
            "terminal.move",
            &json!({"terminals": [t13.to_string()], "before": t10.to_string()})
        )
    );
    // A move among other edits: not one gesture.
    let err = s
        .strip_edits(&[
            StripEdit::Move {
                terminals: vec![t13],
                before: Some(t10),
            },
            StripEdit::GroupLabel {
                terminals: vec![t13],
                label: "Air".into(),
            },
        ])
        .unwrap_err();
    assert!(matches!(err, StripEditError::Invalid(_)));
}

/// Several cell gestures compile to one `fields.set_many`; a bridge part on a terminal without a bridge refuses.
#[test]
fn several_cells_compile_to_one_set_many() {
    let mut s = common::sample();
    let (_, bridge) = with_tbd_parts(&mut s);
    let x1031 = strip(&s, "-X1021");
    let t1 = terminal(&s, "-X1021", "1");
    let t2 = terminal(&s, "-X1021", "2");
    let t11 = terminal(&s, "-X1021", "5");
    let cmd = s
        .strip_edits(&[
            StripEdit::GroupLabel {
                terminals: vec![t1, t2],
                label: "Air".into(),
            },
            StripEdit::BridgePart {
                terminals: vec![t1, t2, t11],
                part: "TBD".into(),
            },
        ])
        .unwrap();
    assert_eq!(cmd.name, "fields.set_many");
    let edits = cmd.params["edits"].as_array().unwrap();
    assert_eq!(edits.len(), 2, "{edits:?}");
    assert_eq!(edits[0]["targets"], json!([x1031.to_string()]));
    assert_eq!(
        edits[1]["targets"].as_array().map(Vec::len),
        Some(2),
        "BR1's and BR2's carriers"
    );
    assert_eq!(edits[1]["value"], json!({"copy": bridge}));
    run(&mut s, &cmd);
    let q = s.query();
    let groups = ew_derive::bridge_groups(&q);
    let parts: Vec<(&str, bool)> = groups
        .iter()
        .map(|(l, _, g)| (l.as_str(), g.part.is_some()))
        .collect();
    assert_eq!(parts, [("BR1", true), ("BR2", true)]);
    drop(q);
    // The bridge lines of the BOM: one per group with a part.
    let q = s.query();
    let lines = q.get::<ew_derive::DevicePartLines>(Id::from_uuid(x1031));
    let bridges = lines
        .iter()
        .filter(|l| matches!(l.source, ew_derive::LineSource::Bridge))
        .count();
    assert_eq!(bridges, 2);
    drop(q);
    // A terminal without a bridge.
    let unbridged = s
        .navigator_table(NavKind::Strips)
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Terminal)
        .map(|n| n.records[0])
        .find(|t| {
            let q = s.query();
            !ew_derive::bridge_groups(&q)
                .iter()
                .any(|(_, _, g)| g.terminals.contains(&Id::from_uuid(*t)))
        })
        .expect("an unbridged terminal");
    let err = s
        .strip_edit(&StripEdit::BridgePart {
            terminals: vec![unbridged],
            part: "TBD".into(),
        })
        .unwrap_err();
    assert_eq!(err, StripEditError::Refused("table-no-bridge"));
}

/// The strip editor lists what the terminals' parts bring (doc 07 §5): -X1031 starts and ends with its end brackets
/// "with EW-GEN-TRM-1" and has its end plate after the last terminal whose part names it; Delete on the end plate's row
/// is the strip's choice, which leaves it off (its row then says so) and the parts list with it; the Part cell of a
/// terminal sets its part's variant.
#[test]
fn what_the_parts_bring_stands_in_the_strip_and_is_left_off_by_its_choice() {
    let mut s = common::sample();
    let t = s.navigator_table(NavKind::Strips);
    let col = |key: &str| t.columns.iter().position(|c| c.key == key).unwrap();
    let rows = children(&t, strip_node(&t, "-X1031"));
    let derived: Vec<&&NavNode> = rows.iter().filter(|n| n.key.contains("/derived:")).collect();
    assert_eq!(rows.first().map(|n| n.label.as_str()), Some("End bracket"));
    assert_eq!(rows.last().map(|n| n.label.as_str()), Some("End bracket"));
    assert_eq!(rows[0].cells[col("potential")], "with EW-GEN-TRM-1");
    let plate = derived
        .iter()
        .find(|n| n.label == "End plate")
        .expect("the end plate's row");
    let of: Uuid = ew_api::output::table_edit::derived_of(plate).expect("its accessory");
    let x = strip(&s, "-X1031");
    let cmd = s
        .strip_edit(&StripEdit::Choice {
            strip: x,
            of,
            part: None,
        })
        .unwrap();
    assert_eq!(
        (cmd.name.as_str(), &cmd.params["path"]),
        ("fields.set", &json!("accessory_choices"))
    );
    run(&mut s, &cmd);
    let t = s.navigator_table(NavKind::Strips);
    let left = children(&t, strip_node(&t, "-X1031"))
        .into_iter()
        .find(|n| n.key.contains("/derived:") && n.label == "End plate")
        .expect("the row left off");
    assert!(left.badges.contains(&"left off".to_owned()), "{:?}", left.badges);
    // The Part cell of a terminal whose part has variants chooses one: the blue terminal of -X1011 back to grey.
    let n = terminal(&s, "-X1011", "N");
    let cmd = s
        .strip_edit(&StripEdit::Variant {
            terminal: n,
            text: String::new(),
        })
        .unwrap();
    run(&mut s, &cmd);
    let q = s.query();
    let device = q.devices().get(Id::<Device>::from_uuid(n)).unwrap();
    assert!(device.parts.iter().all(|u| u.part.variant.is_none()));
}

/// Properties of -X1031 list what its terminals' parts bring, one row per kind: emptying the End plate row leaves it
/// off the strip (its choice), typing its number takes it again.
#[test]
fn a_strips_properties_take_or_leave_its_accessories() {
    let mut s = common::sample();
    let x = strip(&s, "-X1031");
    let leave = s
        .layout_edit(&ew_api::output::LayoutEdit {
            kind: "device".into(),
            target: x,
            others: Vec::new(),
            row: "derived.accessory.end_plate".into(),
            text: String::new(),
        })
        .expect("leave it off");
    run(&mut s, &leave);
    let q = s.query();
    let device = q.devices().get(Id::<Device>::from_uuid(x)).unwrap();
    assert!(device.accessory_choices.iter().any(|c| c.part.is_none()));
    drop(q);
    let take = s
        .layout_edit(&ew_api::output::LayoutEdit {
            kind: "device".into(),
            target: x,
            others: Vec::new(),
            row: "derived.accessory.end_plate".into(),
            text: "EW-GEN-EP-1".into(),
        })
        .expect("take it again");
    run(&mut s, &take);
    let q = s.query();
    let device = q.devices().get(Id::<Device>::from_uuid(x)).unwrap();
    assert!(device.accessory_choices.iter().all(|c| c.part.is_some()));
}

/// The held copy of the part `number`.
fn held(s: &Session, number: &str) -> Uuid {
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(id, _)| id.as_uuid())
        .unwrap_or_else(|| panic!("{number} is held"))
}

/// The library part `number` forked as the project's own (its description changed), as Edit part… saves it.
fn fork(s: &mut Session, number: &str) {
    let mut d = s.part_draft(held(s, number)).expect("its draft");
    s.set_draft_field(&mut d, "description", "corrected").unwrap();
    let plan = s.part_edit_plan(&d).expect("the fork");
    assert!(plan.fork.is_some(), "{number} is forked");
    let preview = s.preview(&plan.command).expect("its preview");
    s.commit(preview).expect("the fork");
}

/// A choice names the accessory as the declaring part names it, so no change of a copy's identity loses it: -X1031's
/// end plate left off stays off when the end plate's part is forked, when its terminals' part is, and when both are,
/// and the strip's accessories hold no end plate each time.
#[test]
fn a_left_off_end_plate_stays_off_when_its_parts_are_forked() {
    for forks in [
        &["EW-GEN-EP-1"][..],
        &["EW-GEN-TRM-1"],
        &["EW-GEN-EP-1", "EW-GEN-TRM-1"],
    ] {
        let mut s = common::sample();
        let x = strip(&s, "-X1031");
        let of = held(&s, "EW-GEN-EP-1");
        let cmd = s
            .strip_edit(&StripEdit::Choice {
                strip: x,
                of,
                part: None,
            })
            .unwrap();
        run(&mut s, &cmd);
        for number in forks {
            fork(&mut s, number);
        }
        let q = s.query();
        let device = q.devices().get(Id::<Device>::from_uuid(x)).unwrap();
        let plates = ew_derive::strip_accessories(&q, device)
            .into_iter()
            .filter(|a| a.kind == Some(ew_schema::AccessoryKind::EndPlate))
            .count();
        assert_eq!(plates, 0, "{forks:?}: the end plate stays off");
        assert_eq!(
            device
                .accessory_choices
                .iter()
                .map(|c| c.of.as_uuid())
                .collect::<Vec<_>>(),
            [of],
            "{forks:?}: the choice still names the declared end plate"
        );
    }
}
