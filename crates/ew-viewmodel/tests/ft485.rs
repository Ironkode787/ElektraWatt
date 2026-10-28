// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-485: the product findings of the first blind-drafter dry run (BD16), one regression test per fixed item. The
//! drafter drew folio 101 through the text interface only; each test repeats the step that went wrong there, on a
//! writable copy of Golden A or on a new project, and asserts what the text now says or does.

mod common;

use common::{Tmp, at, click, done, refused, state};
use ew_viewmodel::ew_api::output::PageTables;
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`, at actual size.
fn golden(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// A headless shell with a new project (the builtin template) and a new folio 101 shown.
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
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    s
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

fn page_text(s: &mut ShellModel, folio: &str) -> String {
    done(s, "page.describe", json!({ "folio": folio }));
    s.take_page_text().expect("page.describe leaves its text")
}

fn toast(s: &ShellModel) -> String {
    state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

/// `at X Y mm` of a line as a sheet point in nm.
fn at_of(line: &str) -> Option<(i64, i64)> {
    let words: Vec<&str> = line.split(' ').collect();
    let i = words.iter().rposition(|w| *w == "at")?;
    let mm = |w: &str| w.parse::<f64>().ok().map(|v| (v * 1e6).round() as i64);
    Some((mm(words.get(i + 1)?)?, mm(words.get(i + 2)?)?))
}

fn failed(s: &mut ShellModel, action: &str, params: Value) -> String {
    match s.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Item 5: the page text's positions are the positions a click or the grid cursor reaches.

#[test]
fn item5_page_text_positions_are_the_click_positions_of_the_connection_points() {
    let tmp = Tmp::new("ft485-5");
    let mut s = golden(&tmp, "101");
    let text = page_text(&mut s, "101");
    let page = std::sync::Arc::clone(s.canvas().unwrap().page());
    let mut checked = 0;
    for line in text.lines().filter(|l| l.starts_with("    cp ")) {
        let label = line.split('"').nth(1).unwrap();
        let p = at_of(line).unwrap();
        // The canvas's connection points are sheet points (what `canvas.press` and `cursor.set` take).
        assert!(
            page.cps.iter().any(|c| c.label == label
                && (c.at_nm.0 - p.0).abs() <= 500
                && (c.at_nm.1 - p.1).abs() <= 500),
            "{line}"
        );
        checked += 1;
    }
    assert!(checked > 30, "{checked}");
    // The grid cursor set to a printed position names that connection point.
    let line = text
        .lines()
        .find(|l| l.starts_with("    cp \"F55:2\""))
        .expect("F55:2");
    done(&mut s, "cursor.set", at(at_of(line).unwrap(), json!({})));
    assert_eq!(
        s.canvas().unwrap().cursor_cp().map(|c| c.label.as_str()),
        Some("F55:2")
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Item 4: a wire is picked from the text.

#[test]
fn item4_a_wire_is_selected_by_its_printed_point_or_by_its_ends() {
    let tmp = Tmp::new("ft485-4");
    let mut s = golden(&tmp, "101");
    let text = page_text(&mut s, "101");
    let wires: Vec<String> = text
        .lines()
        .filter(|l| l.starts_with("  wire "))
        .map(str::to_owned)
        .collect();
    assert_eq!(wires.len(), 21);
    // Folio 101's provisional routes overlap: eight wires run along another wire or through a symbol's body for their
    // whole length, so no point picks them and the line prints none.
    let pointless: Vec<&String> = wires.iter().filter(|l| at_of(l).is_none()).collect();
    assert_eq!(pointless.len(), 8, "{pointless:#?}");
    assert!(pointless.iter().all(|l| l.contains(" route provisional ")));
    let t = tables(&s);
    let ends_of = |id| {
        let c = t.connections.iter().find(|c| c.id == id).expect("a wire");
        let label = |e: &ew_viewmodel::ew_api::output::TableEnd| match e {
            ew_viewmodel::ew_api::output::TableEnd::Cp { function, idx } => t
                .cps
                .iter()
                .find(|x| x.function == *function && x.idx == *idx)
                .map(|x| format!("\"{}\"", x.label))
                .unwrap(),
            ew_viewmodel::ew_api::output::TableEnd::Junction { .. } => "junction".to_owned(),
        };
        (label(&c.a), label(&c.b))
    };
    for line in &wires {
        // The ends as the line names them: two quoted labels or `junction <n>`.
        let head = line
            .split(" route ")
            .next()
            .unwrap()
            .trim_start_matches("  wire ");
        let (a, b) = head.split_once(" -> ").unwrap();
        // By the ends, either way round, quotes optional.
        for refs in [
            format!("wire:{a} -> {b}"),
            format!("wire:{} -> {}", b.trim_matches('"'), a.trim_matches('"')),
        ] {
            done(&mut s, "select.set", json!({ "refs": [refs] }));
            let picked = s.selection().items.clone();
            assert_eq!(picked.len(), 1, "{line}");
            let (x, y) = ends_of(picked[0].0);
            assert!(
                (a.starts_with("junction") || x == a || y == a)
                    && (b.starts_with("junction") || x == b || y == b),
                "{line}: {x} -> {y}"
            );
        }
        // By the printed point: a click with Select.
        if let Some(p) = at_of(line) {
            done(&mut s, "ui.dismiss", json!({}));
            done(&mut s, "tool.select", json!({}));
            click(&mut s, p, json!({}));
            let picked = s.selection().items.clone();
            assert_eq!(picked.len(), 1, "{line}");
            let (x, y) = ends_of(picked[0].0);
            assert!(
                (a.starts_with("junction") || x == a || y == a)
                    && (b.starts_with("junction") || x == b || y == b),
                "{line}: clicked {x} -> {y}"
            );
        }
    }
    // A wire is deleted from the text: select it by its ends, Delete.
    let n = tables(&s).connections.len();
    done(&mut s, "select.set", json!({"refs": ["wire:-101F1:1 -> F55:1"]}));
    done(&mut s, "edit.delete_selection", json!({}));
    assert_eq!(tables(&s).connections.len(), n - 1);
    // No such wire, and the wrong form, fail with the form in the message.
    let m = failed(&mut s, "select.set", json!({"refs": ["wire:-101F1:1 -> F55:1"]}));
    assert!(m.contains("wire:<end> -> <end>"), "{m}");
    failed(&mut s, "select.set", json!({"refs": ["wire:F55:1"]}));
}

// ---------------------------------------------------------------------------------------------------------------
// Item 7: a wire end on a wire names the wire.

#[test]
fn item7_a_route_that_ends_on_a_wire_names_that_wire_by_its_ends() {
    let tmp = Tmp::new("ft485-7");
    let mut s = golden(&tmp, "101");
    let text = page_text(&mut s, "101");
    // The two wires of the drafter's mistake run side by side through one cell (`1X1:1/a` and `1X1:2/a` up to S51).
    let line = text
        .lines()
        .find(|l| l.starts_with("  wire \"1X1:1/a\" -> \"S51:2\""))
        .expect("the wire");
    let p = at_of(line).unwrap();
    let ends: Vec<&str> = line.split('"').skip(1).step_by(2).take(2).collect();
    done(&mut s, "tool.wire", json!({}));
    done(&mut s, "cursor.set", at(p, json!({})));
    done(&mut s, "canvas.activate", json!({}));
    let route = describe_line(&s, "canvas route ").expect("routing from the wire");
    assert!(
        route.contains(&format!(" {} -> {}\"", ends[0], ends[1])),
        "{route} names {ends:?}"
    );
}

fn describe_line(s: &ShellModel, prefix: &str) -> Option<String> {
    s.describe()
        .lines()
        .find(|l| l.starts_with(prefix))
        .map(str::to_owned)
}

// ---------------------------------------------------------------------------------------------------------------
// Items 9 and 10: Delete on a selection by reference.

#[test]
fn item10_delete_on_a_device_selected_by_tag_deletes_its_representations_on_the_page() {
    let tmp = Tmp::new("ft485-10");
    let mut s = golden(&tmp, "101");
    let before = tables(&s);
    let s51: Vec<_> = before
        .reps
        .iter()
        .filter(|r| {
            before
                .cps
                .iter()
                .any(|c| c.rep == r.id && c.label.starts_with("S51:"))
        })
        .map(|r| r.id)
        .collect();
    assert_eq!(s51.len(), 1);
    done(&mut s, "select.set", json!({"refs": ["tag:S51"]}));
    done(&mut s, "edit.delete_selection", json!({}));
    let after = tables(&s);
    assert_eq!(after.reps.len(), before.reps.len() - 1);
    assert!(!after.reps.iter().any(|r| r.id == s51[0]));
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(tables(&s).reps.len(), before.reps.len());
    // A device drawn on another page only is refused here, never deleted there.
    done(&mut s, "select.set", json!({"refs": ["tag:-102F1"]}));
    assert_eq!(
        refused(&mut s, "edit.delete_selection", json!({})),
        "reason-not-on-page"
    );
}

#[test]
fn item9_an_unused_strip_is_listed_with_its_alias_and_deleted_by_delete() {
    let tmp = Tmp::new("ft485-9");
    let mut s = new_project(&tmp);
    done(&mut s, "strip.new", json!({"letter": "X"}));
    // Item 1: the toast names the strip.
    let t = toast(&s);
    assert!(t.starts_with("Created terminal strip -"), "{t}");
    // The terminal symbol opens the strip picker, whose row ends with the strip's alias.
    done(&mut s, "place.pick", json!({"symbol": "ew.iec.terminal@qet10"}));
    let text = s.describe();
    let row = text
        .lines()
        .find(|l| l.starts_with("  row strip "))
        .unwrap_or_else(|| panic!("{text}"));
    let alias = row
        .split(' ')
        .find(|w| w.starts_with('@'))
        .unwrap_or_else(|| panic!("{row}"))
        .to_owned();
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "select.set", json!({ "ids": [alias] }));
    let rev = s.revision();
    done(&mut s, "edit.delete_selection", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one delete");
    // No strip is left for a terminal.
    assert_eq!(
        refused(&mut s, "place.pick", json!({"symbol": "ew.iec.terminal@qet10"})),
        "reason-no-strip"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Item 8: the external reference by the text the page prints.

#[test]
fn item8_ip_external_takes_the_printed_location_and_lists_the_labels_when_nothing_matches() {
    let tmp = Tmp::new("ft485-8");
    let mut s = new_project(&tmp);
    done(
        &mut s,
        "structure.new",
        json!({"kind": "location", "designation": "+J-E-206"}),
    );
    assert_eq!(toast(&s), "Created the location node J-E-206");
    for c in ["L", "N", "PE"] {
        done(
            &mut s,
            "external_ref.new",
            json!({"location": "+J-E-206", "device": "JE-N1000-03", "connection": c}),
        );
    }
    assert!(
        toast(&s).starts_with("Created the external reference JE-N1000-03:PE;"),
        "{}",
        toast(&s)
    );
    done(&mut s, "tool.ip", json!({}));
    // The drafter's spellings: the device alone matches no reference, and the message lists the labels.
    let m = failed(&mut s, "ip.external", json!({"external": "JE-N1000-03"}));
    assert!(m.contains("JE-N1000-03:L, JE-N1000-03:N, JE-N1000-03:PE"), "{m}");
    let m = failed(
        &mut s,
        "ip.external",
        json!({"external": "+J-E-206 / JE-N1000-03"}),
    );
    assert!(m.contains("The project's: "), "{m}");
    // The label, and the label after the printed location.
    done(&mut s, "ip.external", json!({"external": "JE-N1000-03:L"}));
    assert_eq!(state(&s)["canvas"]["ghost"]["external"], "JE-N1000-03:L");
    done(
        &mut s,
        "ip.external",
        json!({"external": "+J-E-206 / JE-N1000-03:PE"}),
    );
    assert_eq!(state(&s)["canvas"]["ghost"]["external"], "JE-N1000-03:PE");
    // A location that is not the reference's matches nothing.
    failed(
        &mut s,
        "ip.external",
        json!({"external": "+J-E-999 / JE-N1000-03:PE"}),
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Item 2: a frozen tag typed as it prints.

#[test]
fn item2_the_frozen_tag_takes_the_tag_as_the_page_prints_it() {
    let tmp = Tmp::new("ft485-2");
    let mut s = golden(&tmp, "101");
    done(&mut s, "select.set", json!({"refs": ["tag:-101F1"]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "device/tag.frozen", "value": "Q51"}),
    );
    assert!(
        page_text(&mut s, "101").contains("  rep \"Q51\" "),
        "no dash: the sign is hidden"
    );
    done(&mut s, "select.set", json!({"refs": ["tag:Q51"]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "device/tag.frozen", "value": "-Q52"}),
    );
    assert!(page_text(&mut s, "101").contains("  rep \"-Q52\" "));
    // The JSON string the drafter tried is the same text.
    done(&mut s, "select.set", json!({"refs": ["tag:-Q52"]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "device/tag.frozen", "value": "\"Q53\""}),
    );
    assert!(page_text(&mut s, "101").contains("  rep \"Q53\" "));
}

// ---------------------------------------------------------------------------------------------------------------
// Item 3: autoconnect says what it does.

#[test]
fn item3_the_hint_offers_alt_and_the_toast_names_the_wires_a_placement_drew() {
    let tmp = Tmp::new("ft485-3");
    let mut s = golden(&tmp, "101");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    // Two contacts one above the other, then a third between them: its CPs face both.
    let sn = s.canvas().unwrap().page().snapper.clone();
    let k = |x: i64, y: i64| sn.page_to_sheet((sn.page_of_index(x), sn.page_of_index(y)));
    done(&mut s, "cursor.set", at(k(60, 38), json!({})));
    assert_eq!(
        state(&s)["status"][0],
        "Click or Enter to place; R, X, Y and V turn it; Escape ends"
    );
    done(&mut s, "canvas.activate", json!({}));
    let span = 4;
    done(&mut s, "cursor.set", at(k(60, 38 + 3 * span + 2), json!({})));
    done(&mut s, "canvas.activate_alt", json!({}));
    done(&mut s, "cursor.set", at(k(60, 38 + span + 1), json!({})));
    let hints = state(&s)["canvas"]["ghost"]["hints"].as_u64().unwrap();
    assert!(hints > 0, "{}", s.describe());
    assert!(
        state(&s)["status"][0]
            .as_str()
            .unwrap()
            .contains("Alt+Click or Alt+Enter places without them"),
        "{}",
        state(&s)["status"]
    );
    done(&mut s, "canvas.activate", json!({}));
    let t = toast(&s);
    assert!(t.starts_with("Placed and connected "), "{t}");
    assert!(t.contains(" -> \"-101K"), "{t}");
    assert!(
        t.ends_with("Alt+Click or Alt+Enter places without connecting"),
        "{t}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Item 11: the export says what it wrote.

#[test]
fn item11_the_pdf_export_names_the_file_and_its_page_count() {
    let tmp = Tmp::new("ft485-11");
    let mut s = new_project(&tmp);
    let path = tmp.0.join("set.pdf");
    done(&mut s, "file.export_pdf", json!({"path": path.to_string_lossy()}));
    assert!(path.exists());
    let t = toast(&s);
    assert!(t.starts_with("Exported set.pdf: "), "{t}");
    assert!(t.contains(" pages, "), "{t}");
    assert!(
        !t.contains(&tmp.0.to_string_lossy().into_owned()),
        "no folder: {t}"
    );
}
