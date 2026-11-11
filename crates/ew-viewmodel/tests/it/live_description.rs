// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 5 (D13, AX-1; FR-UI-026, NFR-A11Y-02): one selected record is described in the canvas's live region
//! and in `ui.describe`'s `canvas live` line: a device by its tag, kind and folio with its connection points and
//! wires, a wire by its number, potential and ends, a page item by its kind and text; two or more say how many. The
//! expected values are read from the model through other public reads (the Devices navigator, the property sheet,
//! the page tables), not written into the test.

use crate::common;

use common::{Tmp, done, tables};
use ew_viewmodel::ew_api::Session;
use ew_viewmodel::ew_api::output::{NavKind, NavRole, PageTables, TableEnd};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::status::canvas_node;
use serde_json::json;

/// What the focused canvas's live region says now.
fn live(shell: &ShellModel) -> String {
    canvas_node(shell, 0).expect("a canvas").live
}

/// The `canvas live` line of the describe text, if there is one.
fn live_line(shell: &ShellModel) -> Option<String> {
    shell
        .describe()
        .lines()
        .find(|l| l.starts_with("canvas live "))
        .map(str::to_owned)
}

/// The label of a connection's end on the shown page: its CP's label.
fn end_label(t: &PageTables, end: &TableEnd) -> Option<String> {
    match end {
        TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| c.label.clone()),
        TableEnd::Junction { .. } => None,
    }
}

#[test]
fn selecting_102f1_says_its_tag_kind_folio_connection_points_and_wires() {
    let tmp = Tmp::new("ax1-device");
    let mut shell = tmp.shell("102");
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    // The model's values: the kind of its main function in the Devices navigator, the folio of its main location in
    // the property sheet's header, its connection points and their connections in the page tables of folio 102.
    let (session, _) = Session::open_read_only(&tmp.golden_copy(), "0.0.0-test").unwrap();
    let devices = session.navigator_table(NavKind::Devices);
    let device = devices
        .nodes
        .iter()
        .position(|n| n.role == NavRole::Device && n.label == "-102F1")
        .expect("-102F1 in the Devices navigator");
    let kind = devices
        .nodes
        .iter()
        .find(|n| {
            n.parent == Some(device as u32)
                && n.role == NavRole::Function
                && n.badges.first().map(String::as_str) == Some("main")
        })
        .map(|n| n.label.clone())
        .expect("its main function");
    let header = session.property_sheet(&devices.nodes[device].records).header;
    assert_eq!(header.title, "-102F1");
    let location = header.location.expect("a main location");
    let (folio, _cell) = location.split_once('-').expect("<folio>-<cell>");
    assert_eq!(folio, "102");
    let t = tables(&shell);
    let cps: Vec<_> = t.cps.iter().filter(|c| c.label.starts_with("-102F1:")).collect();
    let mut wires: Vec<_> = cps.iter().flat_map(|c| c.connections.iter().copied()).collect();
    wires.sort();
    wires.dedup();
    assert!(!cps.is_empty() && !wires.is_empty());
    let expected = format!(
        "-102F1, {kind}, folio {folio}, {} connection points, {} wires",
        cps.len(),
        wires.len()
    );
    assert_eq!(live(&shell), expected);
    assert_eq!(live_line(&shell), Some(format!("canvas live \"{expected}\"")));
    shell.shutdown();
}

#[test]
fn a_wire_says_its_number_potential_and_ends() {
    let tmp = Tmp::new("ax1-wire");
    let mut shell = tmp.shell("102");
    let t = tables(&shell);
    let (session, _) = Session::open_read_only(&tmp.golden_copy(), "0.0.0-test").unwrap();
    // A wire between two connection points, with a number and a potential.
    let (conn, number, from, to, potential) = t
        .connections
        .iter()
        .filter(|c| c.kind == "wire")
        .find_map(|c| {
            let (from, to) = (end_label(&t, &c.a)?, end_label(&t, &c.b)?);
            let potential = t
                .cps
                .iter()
                .find(|cp| cp.label == from)
                .and_then(|cp| cp.potential.as_ref())
                .map(|p| p.name.clone())?;
            let number = session.property_sheet(&[c.id]).header.title;
            (number != "?").then_some((c.id, number, from, to, potential))
        })
        .expect("a numbered wire between two connection points on 102");
    done(&mut shell, "select.set", json!({"ids": [conn.to_string()]}));
    let expected = format!("{number}, {potential}, {from} to {to}");
    assert_eq!(live(&shell), expected);
    assert_eq!(live_line(&shell), Some(format!("canvas live \"{expected}\"")));
    shell.shutdown();
}

#[test]
fn a_page_item_says_its_kind_and_text_and_two_records_say_how_many() {
    let tmp = Tmp::new("ax1-item");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    // Golden A has no page item: a free text placed on 102.
    let page = tables(&shell).page.to_string();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"text": "Spare", "at": [40_000_000, 30_000_000]}}}}),
    );
    let t = tables(&shell);
    let text = t
        .items
        .iter()
        .find(|i| i.kind == "free_text")
        .expect("the free text on 102");
    assert_eq!(text.text.as_deref(), Some("Spare"));
    done(&mut shell, "select.set", json!({"ids": [text.id.to_string()]}));
    let expected = "Text, Spare";
    assert_eq!(live(&shell), expected);
    assert_eq!(live_line(&shell), Some(format!("canvas live \"{expected}\"")));
    // Two or more records keep "<n> selected".
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:-102F1", "tag:-102F2"]}),
    );
    assert_eq!(live(&shell), "2 selected");
    assert_eq!(live_line(&shell), Some("canvas live \"2 selected\"".to_owned()));
    // Nothing selected: no line.
    done(&mut shell, "select.set", json!({"ids": []}));
    assert_eq!(live_line(&shell), None);
    shell.shutdown();
}

/// A selected free text of two lines that holds a `"` is quoted on one `canvas live`
/// line as the page items' texts are (a line break a space, a `"` a `'`); the live region keeps the text as it is.
#[test]
fn a_two_line_text_with_a_quote_stays_on_one_quoted_line() {
    let tmp = Tmp::new("ax1-quote");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let page = tables(&shell).page.to_string();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"text": "Spare \"A\"\nfeed", "at": [40_000_000, 30_000_000]}}}}),
    );
    let t = tables(&shell);
    let text = t
        .items
        .iter()
        .find(|i| i.kind == "free_text")
        .expect("the free text on 102");
    done(&mut shell, "select.set", json!({"ids": [text.id.to_string()]}));
    assert_eq!(live(&shell), "Text, Spare \"A\"\nfeed");
    assert_eq!(
        live_line(&shell),
        Some("canvas live \"Text, Spare 'A' feed\"".to_owned())
    );
    shell.shutdown();
}

/// Switches the window's language (View ▸ Language).
fn language(shell: &mut ShellModel, tag: &str) {
    done(shell, "view.language", json!({ "value": tag }));
}

/// A junction, an interruption point's representation and a structure box are described in words
/// of their own, not as "1 selected", in en-GB and nl-NL. The values are the page tables' and the Interruption points
/// navigator's.
#[test]
fn a_junction_an_interruption_point_and_a_structure_box_say_what_they_are() {
    let tmp = Tmp::new("ft1248");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let t = tables(&shell);
    let junction = t.junctions.first().expect("a junction on 102");
    let wires = junction.sequence.len();
    assert!(wires > 1);
    // An interruption point drawn on 102: its name is its group's label in the navigator.
    let (session, _) = Session::open_read_only(&tmp.golden_copy(), "0.0.0-test").unwrap();
    let ips = session.navigator_table(NavKind::Ips);
    let (name, rep) = ips
        .nodes
        .iter()
        .find_map(|n| {
            let d = n.drawables.iter().find(|d| d.folio == t.folio)?;
            let parent = &ips.nodes[n.parent? as usize];
            (parent.role == NavRole::Name).then(|| (parent.label.clone(), d.record))
        })
        .expect("an interruption point on 102");
    let page = t.page.to_string();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "box.structure", "params": {"page": page,
            "outline": [[10_000_000, 10_000_000], [60_000_000, 10_000_000], [60_000_000, 40_000_000]],
            "style": "dotted", "caption": "Feed", "assigns": {}}}),
    );
    let structure_box = tables(&shell)
        .structure_boxes
        .iter()
        .find(|b| b.caption == "Feed")
        .map(|b| b.id)
        .expect("the box");
    let folio = t.folio.clone();
    let mut said = Vec::new();
    for lang in ["en", "nl"] {
        language(&mut shell, lang);
        for id in [junction.id, rep, structure_box] {
            done(&mut shell, "select.set", json!({"ids": [id.to_string()]}));
            said.push(live(&shell));
        }
    }
    assert_eq!(
        said,
        [
            format!("Junction, folio {folio}, {wires} wires"),
            format!("Interruption point {name}, folio {folio}"),
            format!("Structure box Feed, folio {folio}"),
            format!("Aftakking, folio {folio}, {wires} draden"),
            format!("Onderbrekingspunt {name}, folio {folio}"),
            format!("Structuurkader Feed, folio {folio}"),
        ]
    );
    shell.shutdown();
}

/// A wire that ends at a junction names that end "a junction", in en-GB and
/// nl-NL; its other end is its connection point's label. A language switch words the live region again.
#[test]
fn a_wire_to_a_junction_names_that_end_a_junction() {
    let tmp = Tmp::new("ft1250");
    let mut shell = tmp.shell("102");
    let t = tables(&shell);
    let (session, _) = Session::open_read_only(&tmp.golden_copy(), "0.0.0-test").unwrap();
    let (conn, number, from, potential) = t
        .connections
        .iter()
        .find_map(|c| {
            let (TableEnd::Cp { .. }, TableEnd::Junction { .. }) = (&c.a, &c.b) else {
                return None;
            };
            let from = end_label(&t, &c.a)?;
            let potential = t
                .cps
                .iter()
                .find(|cp| cp.label == from)
                .and_then(|cp| cp.potential.as_ref())
                .map(|p| p.name.clone())?;
            let number = session.property_sheet(&[c.id]).header.title;
            Some((c.id, number, from, potential))
        })
        .expect("a wire from a connection point to a junction on 102");
    done(&mut shell, "select.set", json!({"ids": [conn.to_string()]}));
    assert_eq!(
        live(&shell),
        format!("{number}, {potential}, {from} to a junction")
    );
    language(&mut shell, "nl");
    assert_eq!(
        live(&shell),
        format!("{number}, {potential}, {from} naar een aftakking")
    );
    shell.shutdown();
}

/// A device that is not placed and has no kind (no function, so neither a
/// category text nor a key) says its tag and "Device", in en-GB and, after a language switch, nl-NL.
#[test]
fn an_unplaced_device_without_a_kind_says_device() {
    use ew_viewmodel::ew_api::{AnyCommand, StoreOptions};
    let tmp = Tmp::new("ft1252-device");
    // A copy of Golden A with one device more, made through the session.
    let (mut session, _) = Session::open_read_only(std::path::Path::new(common::GOLDEN_A), "test").unwrap();
    let receipt = session
        .execute(&AnyCommand {
            name: "device.create".into(),
            params: json!({"letter": "K"}),
        })
        .unwrap();
    let id = receipt
        .created
        .first()
        .expect("the new device")
        .1
        .as_uuid()
        .to_string();
    let dir = tmp.0.join("one-more.ew");
    let mut fresh = Session::new(session.world().model.clone(), Default::default());
    fresh.create(&dir, StoreOptions::new("test")).unwrap();
    fresh.close();
    let mut shell = ShellModel::new(
        Mode::Headless,
        ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(&mut shell, "file.open", json!({ "dir": dir.to_string_lossy() }));
    done(&mut shell, "page.goto", json!({ "folio": "102" }));
    done(&mut shell, "select.set", json!({ "ids": [id] }));
    let tag = shell
        .properties()
        .sheet()
        .expect("its sheet")
        .header
        .title
        .clone();
    assert_eq!(live(&shell), format!("{tag}, Device"));
    language(&mut shell, "nl");
    assert_eq!(live(&shell), format!("{tag}, Apparaat"));
    shell.shutdown();
}

/// An edit that keeps the selection (a new tag typed in Properties) words the live region again, as the
/// `canvas live` line is.
#[test]
fn a_rename_that_keeps_the_selection_words_the_live_region_again() {
    let tmp = Tmp::new("ft1251");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    let before = live(&shell);
    assert!(before.starts_with("-102F1, "), "{before}");
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/derived.tag", "value": "-102F9"}),
    );
    let after = before.replacen("-102F1", "-102F9", 1);
    assert_eq!(live(&shell), after);
    assert_eq!(live_line(&shell), Some(format!("canvas live \"{after}\"")));
    shell.shutdown();
}
