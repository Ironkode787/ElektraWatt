// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clipboard through the shell on temporary copies of Golden A (acceptance checks CB4, CB7 and
//! the shell half of CB8): FR-DEV-029's copy and paste of -102F1 on folio 102 (-102F3, part A9F78116, no wire); the
//! paste tool (ghost, dwell preview with the predicted tags, commit with the pasted representation selected, Escape);
//! duplicate (the clipboard unchanged); cut (one undo step, the clipboard keeps the fragment); plain text refused; the
//! transcript's `clipboard` and paste `ghost` members, the describe lines with the 6 words, and the exact counts.

use crate::common;
use ew_viewmodel::key;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::actions::{MenuEntry, find, menu_model, parse_signature};
use ew_viewmodel::ew_api::window::parse_fragment;
use ew_viewmodel::ew_api::{AnyCommand, CmdError, Origin, Session, Submit};
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`.
fn writable(tmp: &Tmp, folio: &str) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, folio)
}

/// Doc 06 §3.6 item 5: a copy that dropped a wire says so in the ghost also when the window's Ctrl+V brings the same
/// text back as `edit.paste_clip`'s `text`.
#[test]
fn the_dropped_count_survives_a_paste_of_the_same_text() {
    let tmp = Tmp::new("clip-dropped");
    let mut shell = writable(&tmp, "102");
    let rep = select_rep(&mut shell, "-102F1");
    let tables = shell.canvas().unwrap().page().tables.clone();
    let f1 = tables
        .reps
        .iter()
        .find(|r| r.id.to_string() == rep)
        .unwrap()
        .functions[0];
    let wire = tables
        .connections
        .iter()
        .find(|c| matches!(c.a, ew_viewmodel::ew_api::output::TableEnd::Cp { function, idx: 1 } if function == f1))
        .expect("-102F1:2's wire")
        .id
        .to_string();
    done(&mut shell, "select.set", json!({ "ids": [rep, wire] }));
    done(&mut shell, "edit.copy", json!({}));
    assert_eq!(shell.clipboard().dropped(), 1);
    let text = shell.clipboard().text().unwrap().to_owned();
    done(&mut shell, "edit.paste_clip", json!({ "text": text }));
    assert_eq!(state(&shell)["canvas"]["ghost"]["dropped"], 1);
    // Another text is another clipboard: nothing dropped.
    done(&mut shell, "ui.dismiss", json!({}));
    let other = text.replacen("\"letter\":\"F\"", "\"letter\":\"K\"", 1);
    done(&mut shell, "edit.paste_clip", json!({ "text": other }));
    assert_eq!(state(&shell)["canvas"]["ghost"]["dropped"], 0);
    shell.shutdown();
}

/// Window check, item 3: Ctrl+V on a page whose kind takes no paste says why and starts no paste tool; Edit ▸ Paste is
/// disabled with the same reason.
#[test]
fn a_paste_on_a_page_that_cannot_take_it_is_refused_with_its_reason() {
    let tmp = Tmp::new("clip-page-kind");
    let mut shell = writable(&tmp, "102");
    select_rep(&mut shell, "-102F1");
    done(&mut shell, "edit.copy", json!({}));
    for (folio, kind) in [("1", "title page"), ("901", "report page")] {
        done(&mut shell, "page.goto", json!({"folio": folio}));
        assert_eq!(
            shell.dispatch("edit.paste_clip", &json!({})),
            Outcome::Refused {
                reason_key: key!("reason-paste-page-kind")
            },
            "paste on folio {folio}"
        );
        let expected = format!("Nothing can be pasted on a {kind}");
        assert_ne!(shell.tool().name(), "paste", "no paste tool on folio {folio}");
        assert_eq!(shell.status_fields(false)[0], expected);
        let item = menu_model(&shell)
            .into_iter()
            .flat_map(|m| m.entries)
            .find_map(|e| match e {
                MenuEntry::Item(i) if i.id == "edit.paste_clip" => Some(i),
                _ => None,
            })
            .expect("Edit ▸ Paste");
        assert!(!item.enabled, "disabled on folio {folio}");
        assert_eq!(item.reason.as_deref(), Some(expected.as_str()));
    }
    shell.shutdown();
}

/// F2: `paste_page_kind`'s list must agree with `edit.paste`'s own page-kind check for every kind `page.new` offers
/// (plus a generated report page). A multi-line fragment may be refused on a schematic page for its representation
/// type, so the comparison is the page-level refusal (`/page`) itself, not any refusal.
#[test]
fn the_paste_page_kinds_agree_with_the_paste_command() {
    let tmp = Tmp::new("clip-page-kinds");
    let mut shell = writable(&tmp, "101");
    select_rep(&mut shell, "-101F1");
    done(&mut shell, "edit.copy", json!({}));
    let text = shell.clipboard().text().unwrap().to_owned();
    // The shell's own list of page kinds is the `type` enum of `page.new` (a test compares it to `PAGE_TYPES`).
    let sig = parse_signature(find("page.new").unwrap().params).unwrap();
    let ty = &sig.iter().find(|p| p.name == "type").unwrap().ty;
    let kinds: Vec<&str> = ty
        .strip_prefix("enum(")
        .and_then(|t| t.strip_suffix(')'))
        .unwrap()
        .split('|')
        .collect();
    let mut pages: Vec<(String, String)> = Vec::new();
    for (i, kind) in kinds.iter().enumerate() {
        let folio = (1000 + i).to_string();
        done(&mut shell, "page.new", json!({"folio": folio, "type": kind}));
        pages.push(((*kind).to_owned(), folio));
    }
    done(&mut shell, "file.save", json!({}));
    let (mut session, _) = Session::open_read_only(&tmp.0.join("J-E-2416.ew"), "0.0.0-test").unwrap();
    let rows = session.page_rows();
    let report = rows
        .iter()
        .find(|r| r.kind == "report")
        .expect("a generated report page");
    pages.push(("report".to_owned(), report.folio.clone()));
    let fragment = parse_fragment(&text).unwrap();
    let anchor = fragment.params["anchor"].clone();
    for (kind, folio) in pages {
        done(&mut shell, "page.goto", json!({"folio": folio}));
        let shell_refuses = shell.paste_page_kind().is_some();
        let row = rows
            .iter()
            .find(|r| r.folio == folio)
            .unwrap_or_else(|| panic!("no {kind} page {folio}"));
        let cmd = AnyCommand {
            name: "edit.paste".into(),
            params: json!({
                "page": row.id.to_string(),
                "at": anchor,
                "fragment": fragment.params.clone(),
            }),
        };
        let command_refuses_page = match session.preview_with(&cmd, Submit::from(Origin::Gui)) {
            Err(CmdError::InvalidParams { path, .. }) => path == "/page",
            _ => false,
        };
        assert_eq!(shell_refuses, command_refuses_page, "{kind} on folio {folio}");
    }
    shell.shutdown();
}

/// A cut whose copy is refused (several pages) leaves the selection as it was, also one made by tag.
#[test]
fn a_refused_cut_leaves_the_selection() {
    let tmp = Tmp::new("clip-cut-refused");
    let mut shell = writable(&tmp, "101");
    let rep101 = select_rep(&mut shell, "-101F1");
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    let device = shell.selection().items[0].0.to_string();
    done(&mut shell, "select.set", json!({ "ids": [device, rep101] }));
    let before = shell.selection().items.clone();
    assert_eq!(refused(&mut shell, "edit.cut", json!({})), "reason-copy-pages");
    assert_eq!(shell.selection().items, before);
    shell.shutdown();
}

/// A device selected by its tag (`select.set`, the scripts' and the blind drafter's way) is copied, and cut, as its
/// representations on the page shown.
#[test]
fn a_device_selected_by_tag_is_copied_as_its_representation() {
    let tmp = Tmp::new("clip-by-tag");
    let mut shell = writable(&tmp, "102");
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    done(&mut shell, "edit.copy", json!({}));
    assert_eq!(
        state(&shell)["clipboard"],
        json!({"kind": "fragment", "records": 4, "copies": 4})
    );
    let reps = shell.canvas().unwrap().page().tables.reps.len();
    done(&mut shell, "edit.cut", json!({}));
    assert_eq!(shell.canvas().unwrap().page().tables.reps.len(), reps - 1);
    shell.shutdown();
}

/// Selects the representation of the device printed `tag` on the page shown; its ID text.
fn select_rep(shell: &mut ShellModel, tag: &str) -> String {
    done(shell, "select.set", json!({"refs": [format!("tag:{tag}")]}));
    let dev = shell.selection().items[0].0;
    let rep = shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.device == Some(dev))
        .expect("the device is drawn here")
        .id
        .to_string();
    done(shell, "select.set", json!({ "ids": [rep] }));
    rep
}

/// The sheet point of the page point `p` on the page shown.
fn sheet(shell: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    shell.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The page position of the representation `rep` on the page shown.
fn rep_pos(shell: &ShellModel, rep: &str) -> (i64, i64) {
    shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.id.to_string() == rep)
        .unwrap()
        .pos
}

/// The QET grid step in nm (rounded): a sheet offset that moves the snapped anchor by one step.
const STEP: i64 = 2_645_833;

/// rc.5 (the owner: copy and paste between grids did not work, and a page's grid could not change once it held items):
/// -102G1 and -102F2 with the wires between them, copied on folio 102 (the QET grid) and pasted on a new page of the
/// 4 mm grid, are fitted to it and their wires routed anew, in one undo step, and a message says so. That page's grid
/// changed back to the QET grid asks first, saying what will happen; Change and fit fits its items and routes its
/// wires in one undo step, and says so.
#[test]
fn a_paste_onto_another_grid_and_a_grid_change_fit_the_items_and_say_so() {
    let tmp = Tmp::new("clip-fit");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    let g1 = select_rep(&mut shell, "-102G1");
    let f2 = select_rep(&mut shell, "-102F2");
    done(&mut shell, "select.set", json!({ "ids": [g1, f2] }));
    done(&mut shell, "edit.copy", json!({}));
    done(
        &mut shell,
        "page.new",
        json!({"folio": "150", "title": "Fit", "grid": "mm4"}),
    );
    assert_eq!(shell.current_folio(), Some("150"));
    done(&mut shell, "edit.paste_clip", json!({}));
    let target = sheet(&shell, (60_000_000, 60_000_000));
    done(&mut shell, "canvas.pointer", at(target, json!({})));
    let rev = shell.revision();
    done(&mut shell, "canvas.press", at(target, json!({})));
    assert_eq!(
        shell.revision(),
        rev.map(|r| r + 2),
        "the paste, then its wires routed"
    );
    let tables = |shell: &ShellModel| shell.canvas().unwrap().page().tables.clone();
    let t = tables(&shell);
    assert_eq!(t.reps.len(), 2);
    assert!(!t.connections.is_empty(), "the wires between them");
    assert!(
        t.connections.iter().all(|c| c.stored_route),
        "routed anew: {:?} {}",
        t.connections
            .iter()
            .map(|c| (c.stored_route, c.polyline.len()))
            .collect::<Vec<_>>(),
        common::toast(&shell)
    );
    let text = common::toast(&shell);
    assert!(
        text.starts_with("The paste was fitted to this page's 4 mm grid:"),
        "{text}"
    );
    // One undo step takes the paste and its routes back.
    done(&mut shell, "edit.undo", json!({}));
    assert!(tables(&shell).reps.is_empty(), "one step");
    done(&mut shell, "edit.redo", json!({}));
    assert_eq!(tables(&shell).reps.len(), 2);
    // The page's grid back to the QET grid: the question first.
    done(&mut shell, "select.set", json!({ "ids": [] }));
    done(&mut shell, "props.show", json!({"on": true}));
    let rev = shell.revision();
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/grid", "value": "QET 10 px"}),
    );
    assert_eq!(shell.revision(), rev, "nothing changed yet");
    let form = common::form(&shell);
    assert_eq!(form.action, "page.fit_grid");
    let foot = shell.form_foot(&form).expect("what will happen");
    assert!(foot.starts_with("Page 150 holds 2 items and "), "{foot}");
    assert!(foot.contains("On the QET 10 px grid"), "{foot}");
    assert_eq!(shell.submit_form(), Outcome::Done);
    assert_eq!(shell.canvas().unwrap().page().snapper.grid(), (7_937_500, 3));
    let t = tables(&shell);
    assert!(t.connections.iter().all(|c| c.stored_route), "routed anew");
    let text = common::toast(&shell);
    assert!(text.starts_with("Page 150 now has the QET 10 px grid:"), "{text}");
    // The message counts what the question counted.
    let moved = foot
        .split("grid, ")
        .nth(1)
        .and_then(|r| r.split(' ').next())
        .unwrap()
        .to_owned();
    assert!(
        text.contains(&format!(": {moved} items moved")),
        "{foot} / {text}"
    );
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(
        shell.canvas().unwrap().page().snapper.grid(),
        (4_000_000, 1),
        "one step"
    );
    shell.shutdown();
}

/// The page point `p` as a sheet point clicked: pointer, press, release.
fn click_at(shell: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(shell, p);
    done(shell, "canvas.pointer", at(q, json!({})));
    done(shell, "canvas.press", at(q, json!({})));
    done(shell, "canvas.release", at(q, json!({})));
}

/// Moves every representation of the page shown by a drag of two grid steps right and by the keyboard (Move, one
/// step down with the arrow key, Enter), each one commit that keeps its first pin on the grid `g`; turns the first
/// one; draws a wire from its first pin to a free grid point.
fn handle_fitted(shell: &mut ShellModel, g: (i64, i64)) {
    let grid = ew_viewmodel::ew_api::output::ew_dl::ew_units::Grid::new(g.0 as u64, g.1 as u64).unwrap();
    let on = |p: (i64, i64)| {
        grid.snap_point(ew_viewmodel::ew_api::output::ew_dl::ew_units::Nm(p.0))
            .map(|n| n.0)
            == Some(p.0)
            && grid
                .snap_point(ew_viewmodel::ew_api::output::ew_dl::ew_units::Nm(p.1))
                .map(|n| n.0)
                == Some(p.1)
    };
    let tables = |shell: &ShellModel| shell.canvas().unwrap().page().tables.clone();
    let pins = |shell: &ShellModel, id| -> Vec<(i64, i64)> {
        tables(shell)
            .cps
            .iter()
            .filter(|c| c.rep == id)
            .map(|c| c.at)
            .collect()
    };
    let ids: Vec<_> = tables(shell).reps.iter().map(|r| r.id).collect();
    assert!(!ids.is_empty());
    for id in &ids {
        let pos = |shell: &ShellModel| tables(shell).reps.iter().find(|r| r.id == *id).unwrap().pos;
        let before = pos(shell);
        // A drag from the symbol's middle (a box's outline's, else the middle of its pins).
        let t = tables(shell);
        let rep = t.reps.iter().find(|r| r.id == *id).unwrap();
        let points: Vec<(i64, i64)> = rep
            .black_box
            .as_ref()
            .map_or_else(|| pins(shell, *id), |b| b.outline.clone());
        let n = points.len() as i64;
        let mid = (
            points.iter().map(|p| p.0).sum::<i64>() / n,
            points.iter().map(|p| p.1).sum::<i64>() / n,
        );
        done(shell, "select.set", json!({ "ids": [] }));
        let from = sheet(shell, mid);
        let to = (
            from.0 + 2 * (grid.at(1).unwrap().0 - grid.at(0).unwrap().0),
            from.1,
        );
        let rev = shell.revision();
        done(shell, "canvas.press", at(from, json!({})));
        done(shell, "canvas.pointer", at(to, json!({})));
        done(shell, "canvas.release", at(to, json!({})));
        assert_eq!(shell.revision(), rev.map(|r| r + 1), "{id}: the drag moves it");
        assert_ne!(pos(shell), before, "{id}");
        assert!(pins(shell, *id).iter().any(|p| on(*p)), "{id}: a pin on the grid");
        // The keyboard: Move, one step down, Enter.
        done(shell, "select.set", json!({ "ids": [id.to_string()] }));
        let rev = shell.revision();
        done(shell, "tool.move", json!({}));
        done(shell, "cursor.down", json!({}));
        done(shell, "canvas.activate", json!({}));
        assert_eq!(
            shell.revision(),
            rev.map(|r| r + 1),
            "{id}: the arrow key moves it"
        );
        assert!(pins(shell, *id).iter().any(|p| on(*p)), "{id}: a pin on the grid");
    }
    // A turn keeps a pin on the grid (a symbol's: an interruption point turns by its direction).
    let first = tables(shell)
        .reps
        .iter()
        .find(|r| r.ip.is_none() && r.device.is_some())
        .map(|r| r.id)
        .unwrap();
    done(shell, "select.set", json!({ "ids": [first.to_string()] }));
    let rev = shell.revision();
    done(shell, "tool.rotate_cw", json!({}));
    assert_eq!(shell.revision(), rev.map(|r| r + 1), "the turn");
    assert!(
        pins(shell, first).iter().any(|p| on(*p)),
        "a pin on the grid after the turn"
    );
    // A wire from one of its free pins to a free pin of another symbol.
    done(shell, "select.set", json!({ "ids": [] }));
    let t = tables(shell);
    let free = |rep: ew_viewmodel::ew_api::output::Uuid, same: bool| {
        t.cps
            .iter()
            .find(|c| (c.rep == rep) == same && c.connections.is_empty())
            .map(|c| c.at)
    };
    let wired = t
        .cps
        .iter()
        .find(|c| c.connections.is_empty() && t.reps.iter().any(|r| r.id == c.rep && r.ip.is_none()))
        .map(|c| c.rep)
        .expect("a symbol with a free pin");
    // The other end: a free pin of another symbol, else a corner of a wire of the page (a junction there).
    let pin = free(wired, true).unwrap();
    let other = free(wired, false)
        .or_else(|| {
            t.connections
                .iter()
                .find(|c| c.polyline.len() > 2)
                .map(|c| c.polyline[1])
        })
        .expect("another end");
    let rev = shell.revision();
    let wires = t.connections.len();
    done(shell, "tool.wire", json!({}));
    click_at(shell, pin);
    click_at(shell, other);
    assert_eq!(shell.revision(), rev.map(|r| r + 1), "the wire");
    assert!(
        tables(shell).connections.len() > wires,
        "a new wire (a wire split at its junction counts twice)"
    );
    done(shell, "ui.dismiss", json!({}));
    done(shell, "ui.dismiss", json!({}));
}

/// rc.5 review: a symbol fitted to a page's grid (its pins on the grid, its position between grid points) moves by
/// drag and by the arrow key, turns and takes a wire, after a fitted paste and after a grid change.
#[test]
fn fitted_symbols_move_turn_and_take_a_wire() {
    let tmp = Tmp::new("clip-fit-move");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    let g1 = select_rep(&mut shell, "-102G1");
    let f1 = select_rep(&mut shell, "-102F1");
    let f2 = select_rep(&mut shell, "-102F2");
    done(&mut shell, "select.set", json!({ "ids": [g1, f1, f2] }));
    done(&mut shell, "edit.copy", json!({}));
    done(
        &mut shell,
        "page.new",
        json!({"folio": "150", "title": "Fit", "grid": "mm4"}),
    );
    done(&mut shell, "edit.paste_clip", json!({}));
    let target = sheet(&shell, (60_000_000, 60_000_000));
    done(&mut shell, "canvas.pointer", at(target, json!({})));
    done(&mut shell, "canvas.press", at(target, json!({})));
    assert_eq!(shell.canvas().unwrap().page().tables.reps.len(), 3);
    handle_fitted(&mut shell, (4_000_000, 1));
    // Copied and pasted again on the same page, the fitted symbols paste by grid steps.
    let ids: Vec<String> = shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.id.to_string())
        .collect();
    done(&mut shell, "select.set", json!({ "ids": ids }));
    done(&mut shell, "edit.copy", json!({}));
    done(&mut shell, "edit.paste_clip", json!({}));
    let target = sheet(&shell, (150_000_000, 100_000_000));
    done(&mut shell, "canvas.pointer", at(target, json!({})));
    done(&mut shell, "canvas.press", at(target, json!({})));
    assert_eq!(
        shell.canvas().unwrap().page().tables.reps.len(),
        6,
        "pasted again"
    );
    // Folio 102 changed to the 4 mm grid.
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "select.set", json!({ "ids": [] }));
    done(&mut shell, "page.fit_grid", json!({"grid": "mm4"}));
    assert_eq!(shell.canvas().unwrap().page().snapper.grid(), (4_000_000, 1));
    handle_fitted(&mut shell, (4_000_000, 1));
    shell.shutdown();
}

/// rc.5 review: a page that holds only a free text asks the same question before its grid changes, instead of a
/// refusal that sends the person to the Properties he is in; Change and fit says nothing moved.
#[test]
fn a_page_with_a_free_text_alone_asks_before_its_grid_changes() {
    let tmp = Tmp::new("clip-fit-text");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    done(
        &mut shell,
        "page.new",
        json!({"folio": "150", "title": "Notes", "grid": "mm4"}),
    );
    let page = shell.canvas().unwrap().page().tables.page.to_string();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page, "kind": {"free_text": {"text": "Note",
            "at": [40_000_000, 40_000_000], "angle": 0, "style": "text.normal", "align": "centre"}}}}),
    );
    done(&mut shell, "select.set", json!({ "ids": [] }));
    done(&mut shell, "props.show", json!({"on": true}));
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/grid", "value": "QET 10 px"}),
    );
    let form = common::form(&shell);
    assert_eq!(form.action, "page.fit_grid");
    let foot = shell.form_foot(&form).expect("what will happen");
    assert!(foot.starts_with("Page 150 holds 1 items and 0 wires."), "{foot}");
    assert_eq!(shell.submit_form(), Outcome::Done);
    assert_eq!(shell.canvas().unwrap().page().snapper.grid(), (7_937_500, 3));
    let text = common::toast(&shell);
    assert!(text.contains("every item stood on it already"), "{text}");
    shell.shutdown();
}

/// rc.5 persona run (he looked for the page's grid on the Page tab, then found it by filtering Properties for "grid",
/// and the filter word hid the next thing's rows): the Page tab has Connection grid…, which asks and fits as the
/// Properties row does; a filter typed for the page clears when a symbol is selected.
#[test]
fn the_page_tab_changes_the_grid_and_a_filter_clears_with_the_kind() {
    let tmp = Tmp::new("page-grid-tab");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    let bar = ew_viewmodel::command_bar::command_bar_of(&shell, ew_viewmodel::command_bar::Tab::Page);
    let button = bar
        .groups
        .iter()
        .flat_map(|g| g.buttons.iter())
        .find(|b| b.id == "page.grid")
        .expect("Connection grid… on the Page tab");
    assert!(button.enabled);
    done(&mut shell, "page.grid", json!({}));
    let form = common::form(&shell);
    assert_eq!(form.action, "page.fit_grid");
    assert_eq!(form.fields[0].value, "qet_10px", "the page's own grid");
    shell.set_form_field("grid", "mm4");
    let form = common::form(&shell);
    assert!(shell.form_foot(&form).is_some_and(|f| f.starts_with("Page 102 holds")));
    assert_eq!(shell.submit_form(), Outcome::Done);
    assert_eq!(shell.canvas().unwrap().page().snapper.grid(), (4_000_000, 1));
    // The filter.
    done(&mut shell, "select.set", json!({ "ids": [] }));
    done(&mut shell, "props.show", json!({"on": true}));
    done(&mut shell, "props.filter", json!({"text": "grid"}));
    assert_eq!(shell.properties_filter(), "grid");
    select_rep(&mut shell, "-102F1");
    assert_eq!(shell.properties_filter(), "", "another kind of thing: the filter clears");
    shell.shutdown();
}

/// rc.5 persona run (after Ctrl+V no ghost showed until the pointer moved; the click set the block down partly off the
/// sheet and nothing said so): the block hangs on the pointer at once, and a click where a part of it would lie outside
/// the drawing area is refused in the status line, nothing pasted.
#[test]
fn a_paste_shows_its_ghost_at_once_and_is_refused_off_the_drawing_area() {
    let tmp = Tmp::new("clip-ghost");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    select_rep(&mut shell, "-102F1");
    done(&mut shell, "edit.copy", json!({}));
    done(&mut shell, "select.set", json!({ "ids": [] }));
    done(&mut shell, "edit.paste_clip", json!({}));
    assert_eq!(shell.tool().name(), "paste");
    assert!(!shell.tool_overlay().is_empty(), "the ghost shows before the pointer moves");
    // At the sheet's top-left corner: outside the drawing area.
    let corner = sheet(&shell, (0, 0));
    let rev = shell.revision();
    done(&mut shell, "canvas.pointer", at(corner, json!({})));
    assert_eq!(
        shell.dispatch("canvas.press", &at(corner, json!({}))),
        Outcome::Refused {
            reason_key: key!("reason-paste-outside")
        }
    );
    assert_eq!(shell.revision(), rev, "nothing pasted");
    assert_eq!(shell.tool().name(), "paste", "the block still hangs on the pointer");
    shell.shutdown();
}

/// CB4 (FR-DEV-029): copying -102F1 on folio 102 and pasting it on 102 yields a device whose tag is -102F3 with part
/// A9F78116 and no connection (its wires had an end outside).
#[test]
fn copy_and_paste_of_102f1_is_102f3_with_its_part_and_no_wire() {
    let tmp = Tmp::new("clip-cb4");
    let mut shell = writable(&tmp, "102");
    let rep = select_rep(&mut shell, "-102F1");
    done(&mut shell, "edit.copy", json!({}));
    assert_eq!(
        state(&shell)["clipboard"],
        json!({"kind": "fragment", "records": 4, "copies": 4})
    );
    let wires = shell.canvas().unwrap().page().tables.connections.len();
    let reps = shell.canvas().unwrap().page().tables.reps.len();
    done(&mut shell, "edit.paste_clip", json!({}));
    let target = sheet(&shell, rep_pos(&shell, &rep));
    // Twelve grid steps right: an empty spot beside the original, on no wire. (Six steps down, the copy's pins would lie
    // on -102F1's own wire to the junction below it, which normaliser 7 splits into the copy.)
    done(
        &mut shell,
        "canvas.pointer",
        at((target.0 + 12 * STEP, target.1), json!({})),
    );
    let rev = shell.revision();
    done(
        &mut shell,
        "canvas.press",
        at((target.0 + 12 * STEP, target.1), json!({})),
    );
    assert_eq!(shell.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(shell.tool().name(), "select");
    assert_eq!(shell.selection().items.len(), 1, "the pasted representation");
    assert_ne!(shell.selection().items[0].0.to_string(), rep);
    assert_eq!(
        shell.canvas().unwrap().page().tables.connections.len(),
        wires,
        "no connection is pasted"
    );
    done(&mut shell, "props.show", json!({"on": true}));
    let st = state(&shell);
    assert_eq!(st["properties"]["title"], "-102F3");
    let sheet_ = shell.properties().sheet().unwrap();
    let part = find_row(sheet_, "device/parts[0].part").expect("the part row");
    match &part.1.value {
        RowValue::One { text, .. } => assert!(text.starts_with("A9F78116"), "{text}"),
        other => panic!("{other:?}"),
    }
    // One undo step removes the paste.
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(shell.canvas().unwrap().page().tables.reps.len(), reps);
    shell.shutdown();
}

/// CB7: the paste tool: the ghost follows the pointer (no `Session` call), a dwell previews `edit.paste` once with the
/// predicted tag, Escape ends the tool with the revision unchanged; the transcript, the describe lines and the counts.
#[test]
fn the_paste_tool_ghost_preview_and_escape() {
    let tmp = Tmp::new("clip-cb7");
    let mut shell = writable(&tmp, "102");
    let rep = select_rep(&mut shell, "-102F1");
    let calls = shell.session_calls();
    done(&mut shell, "edit.copy", json!({}));
    assert_eq!(shell.session_calls().copy_fragment, calls.copy_fragment + 1);
    let before = shell.session_calls();
    done(&mut shell, "edit.paste_clip", json!({}));
    let after = shell.session_calls();
    assert_eq!(after.parse_fragment, before.parse_fragment + 1, "one parse");
    assert_eq!(after.copy_fragment, before.copy_fragment);
    assert_eq!(shell.tool().name(), "paste");
    assert_eq!(
        state(&shell)["canvas"]["tool"],
        json!({"name": "paste", "state": "floating"})
    );
    let target = sheet(&shell, rep_pos(&shell, &rep));
    let p = (target.0, target.1 + 6 * STEP);
    let moved = shell.session_calls();
    done(&mut shell, "canvas.pointer", at(p, json!({})));
    done(&mut shell, "canvas.pointer", at((p.0 + 10, p.1), json!({})));
    assert_eq!(shell.session_calls(), moved, "ghost moves call nothing");
    let st = state(&shell);
    let ghost = &st["canvas"]["ghost"];
    assert_eq!(ghost["kind"], "paste");
    assert_eq!(ghost["records"], 4);
    assert_eq!(ghost["dropped"], 0);
    assert!(ghost["at_nm"].is_array());
    assert_eq!(st["canvas"]["overlay"], 2, "the ghost rectangles and the label");
    // The dwell: one preview with the predicted tag; a second dwell at the same anchor none.
    done(&mut shell, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(shell.session_calls().preview_with, moved.preview_with + 1);
    done(&mut shell, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(shell.session_calls().preview_with, moved.preview_with + 1);
    let st = state(&shell);
    assert_eq!(st["canvas"]["preview"]["command"], "edit.paste");
    assert_eq!(st["canvas"]["preview"]["state"], "ready");
    assert_eq!(st["canvas"]["preview"]["tags"], 1, "the new device's tag");
    assert!(
        shell.tool_overlay().labels().any(|l| l.contains("-102F3")),
        "the ghost shows the resulting tag: {:?}",
        shell.tool_overlay().labels().collect::<Vec<_>>()
    );
    // The describe lines and their words.
    let d = shell.describe();
    assert!(d.contains("clipboard fragment records 4 copies 4"), "{d}");
    let line = d
        .lines()
        .find(|l| l.starts_with("canvas ghost paste records 4 at "))
        .unwrap_or_else(|| panic!("{d}"));
    assert!(line.ends_with(" mm dropped 0"), "{line}");
    // Escape: the tool ends, nothing is committed.
    let rev = shell.revision();
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().name(), "select");
    assert_eq!(shell.revision(), rev);
    assert_eq!(state(&shell)["canvas"]["ghost"], Value::Null);
    shell.shutdown();
}

/// CB7: duplicate floats a copy two grid steps right and down of the selection's anchor and leaves the clipboard as
/// it was; Enter pastes it.
#[test]
fn duplicate_leaves_the_clipboard_and_starts_two_steps_off() {
    let tmp = Tmp::new("clip-dup");
    let mut shell = writable(&tmp, "102");
    done(&mut shell, "clipboard.text", json!({"text": "-101F1\n"}));
    let clip = state(&shell)["clipboard"].clone();
    assert_eq!(clip, json!({"kind": "text", "lines": 1}));
    select_rep(&mut shell, "-102F1");
    done(&mut shell, "edit.duplicate", json!({}));
    assert_eq!(
        state(&shell)["clipboard"],
        clip,
        "duplicate does not touch the clipboard"
    );
    assert_eq!(shell.tool().name(), "paste");
    let ghost = state(&shell)["canvas"]["ghost"].clone();
    // -102F1's anchor is [113770833, 60854167] (the fragment of fragment_102F1.txt); two QET steps on.
    assert_eq!(ghost["at_nm"], json!([119_062_500, 66_145_833]));
    let rev = shell.revision();
    let p = sheet(&shell, (119_062_500, 66_145_833));
    done(&mut shell, "canvas.press", at(p, json!({})));
    assert_eq!(shell.revision(), rev.map(|r| r + 1));
    assert_eq!(shell.tool().name(), "select");
    shell.shutdown();
}

/// CB7: cut copies and deletes in one undo step; the clipboard keeps the fragment, and undo restores the records.
#[test]
fn cut_is_one_undo_step_and_the_clipboard_keeps_the_fragment() {
    let tmp = Tmp::new("clip-cut");
    let mut shell = writable(&tmp, "102");
    let rep = select_rep(&mut shell, "-102F2");
    let rev = shell.revision();
    let steps = shell.recorded_commands().len();
    done(&mut shell, "edit.cut", json!({}));
    assert_eq!(shell.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(shell.recorded_commands().len(), steps + 1);
    assert_eq!(shell.recorded_commands()[steps].cmd, "edit.delete");
    assert!(
        !shell
            .canvas()
            .unwrap()
            .page()
            .tables
            .reps
            .iter()
            .any(|r| r.id.to_string() == rep),
        "the representation is gone"
    );
    assert_eq!(state(&shell)["clipboard"]["kind"], "fragment");
    done(&mut shell, "edit.undo", json!({}));
    assert!(
        shell
            .canvas()
            .unwrap()
            .page()
            .tables
            .reps
            .iter()
            .any(|r| r.id.to_string() == rep),
        "undo restores it"
    );
    assert_eq!(state(&shell)["clipboard"]["kind"], "fragment");
    shell.shutdown();
}

/// CB7: plain text and an empty clipboard are refused; a damaged fragment shows in the banner; copy without a selection
/// is refused; the tag list goes to the clipboard as text.
#[test]
fn plain_text_is_refused_and_the_refusals() {
    let tmp = Tmp::new("clip-refusals");
    let mut shell = writable(&tmp, "101");
    assert_eq!(
        refused(&mut shell, "edit.paste_clip", json!({})),
        "reason-clipboard-empty"
    );
    assert_eq!(state(&shell)["clipboard"], Value::Null);
    assert!(shell.describe().contains("\nclipboard none\n"));
    done(&mut shell, "clipboard.text", json!({"text": "hello\nworld\n"}));
    assert_eq!(
        refused(&mut shell, "edit.paste_clip", json!({})),
        "reason-not-a-fragment"
    );
    assert_eq!(shell.tool().name(), "select");
    match shell.dispatch(
        "edit.paste_clip",
        &json!({"text": "#ew-fragment v1\n{\"$fragment\":{}}\n"}),
    ) {
        Outcome::Failed { message } => {
            assert!(message.starts_with("Not pasted: fragment line 2"), "{message}");
            assert_eq!(shell.banner(), Some(message.as_str()));
        }
        other => panic!("{other:?}"),
    }
    done(&mut shell, "select.set", json!({"ids": []}));
    assert_eq!(
        refused(&mut shell, "edit.copy", json!({})),
        "reason-nothing-selected"
    );
    assert_eq!(
        refused(&mut shell, "edit.cut", json!({})),
        "reason-nothing-selected"
    );
    assert_eq!(
        refused(&mut shell, "edit.duplicate", json!({})),
        "reason-nothing-selected"
    );
    // Copy tag list (FR-UI-040): -101F1, S51 and F55.
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:S51", "tag:-101F1", "tag:F55"]}),
    );
    done(&mut shell, "edit.copy_tags", json!({}));
    assert_eq!(shell.clipboard().text(), Some("-101F1\nF55\nS51\n"));
    assert_eq!(state(&shell)["clipboard"], json!({"kind": "text", "lines": 3}));
    shell.shutdown();
}

/// CB8: the menu entries are disabled with their reason without a selection, and their chords are the canvas's own.
#[test]
fn menu_entries_and_chords() {
    use ew_viewmodel::actions::{Context, find};
    for (id, key) in [
        ("edit.cut", "Ctrl+X"),
        ("edit.copy", "Ctrl+C"),
        ("edit.paste_clip", "Ctrl+V"),
        ("edit.duplicate", "Ctrl+D"),
    ] {
        let a = find(id).unwrap();
        assert_eq!(a.shortcut, [key], "{id}");
        assert_eq!(a.context, Context::Canvas, "{id}");
    }
    assert!(find("clipboard.text").unwrap().headless_only);
    let tmp = Tmp::new("clip-menu");
    let mut shell = writable(&tmp, "101");
    let menus = ew_viewmodel::actions::menu_model(&shell);
    let edit = menus
        .iter()
        .find(|m| m.menu == ew_viewmodel::actions::Menu::Edit)
        .unwrap();
    let items: Vec<&ew_viewmodel::actions::ActionItem> = edit
        .entries
        .iter()
        .filter_map(|e| match e {
            ew_viewmodel::actions::MenuEntry::Item(i) => Some(i),
            _ => None,
        })
        .collect();
    for id in ["edit.cut", "edit.copy", "edit.duplicate", "edit.copy_tags"] {
        let item = items
            .iter()
            .find(|i| i.id == id)
            .unwrap_or_else(|| panic!("{id}"));
        assert!(!item.enabled, "{id} without a selection");
    }
    assert!(items.iter().find(|i| i.id == "edit.paste_clip").unwrap().enabled);
    shell.shutdown();
}

/// A refused Cut changes only the copy-call counter, including an already pending GUI clipboard write.
fn assert_incomplete_cut_unchanged(shell: &mut ShellModel, ids: &[String]) {
    let rep = select_rep(shell, "-101F1");
    done(shell, "select.set", json!({"ids":[rep]}));
    done(shell, "edit.copy", json!({}));
    // Keep a nonempty outbox pending while testing the refusal.
    let clip = shell.clipboard().text().unwrap().to_owned();
    let kind = shell.clipboard().kind().cloned();
    let dropped = shell.clipboard().dropped();
    done(shell, "select.set", json!({"ids":ids}));
    let selection = shell.selection().items.clone();
    let revision = shell.revision();
    let recorded = shell.recorded_commands().len();
    let announcement = shell.tool_announcement().map(str::to_owned);
    let calls = shell.session_calls();
    let before = shell.canvas().unwrap().page().tables.clone();
    assert_eq!(
        shell.dispatch("edit.cut", &json!({})),
        Outcome::Refused {
            reason_key: key!("reason-cut-incomplete-items")
        }
    );
    assert_eq!(shell.clipboard().text(), Some(clip.as_str()));
    assert_eq!(shell.clipboard().kind(), kind.as_ref());
    assert_eq!(shell.clipboard().dropped(), dropped);
    assert_eq!(shell.take_clipboard_outbox().as_deref(), Some(clip.as_str()));
    assert_eq!(shell.take_clipboard_outbox(), None);
    assert_eq!(shell.selection().items, selection);
    assert_eq!(shell.revision(), revision);
    assert_eq!(shell.recorded_commands().len(), recorded);
    assert_eq!(shell.tool_announcement(), announcement.as_deref());
    let mut expected_calls = calls;
    expected_calls.copy_fragment += 1;
    assert_eq!(
        shell.session_calls(),
        expected_calls,
        "exactly one copy preparation and no other Session calls"
    );
    assert_eq!(shell.canvas().unwrap().page().tables.as_ref(), before.as_ref());
}

fn cut_marker_fixture(shell: &mut ShellModel, partial: bool) -> (String, Vec<String>) {
    use ew_viewmodel::ew_api::output::TableEnd;
    let t = shell.canvas().unwrap().page().tables.clone();
    let funcs = |c: &ew_viewmodel::ew_api::output::TableConnection| match (&c.a, &c.b) {
        (TableEnd::Cp { function: a, .. }, TableEnd::Cp { function: b, .. }) => Some([*a, *b]),
        _ => None,
    };
    let (first, ends) = t
        .connections
        .iter()
        .filter_map(|c| funcs(c).map(|f| (c, f)))
        .find(|(_, f)| f.iter().all(|id| t.reps.iter().any(|r| r.functions.contains(id))))
        .unwrap();
    let other = t
        .connections
        .iter()
        .find(|c| funcs(c).is_some_and(|f| f.iter().any(|id| !ends.contains(id))))
        .unwrap();
    let connections = if partial {
        vec![first.id.to_string(), other.id.to_string()]
    } else {
        vec![first.id.to_string()]
    };
    done(
        shell,
        "command.run",
        json!({"cmd":"marker.place","params":{
        "page":t.page.to_string(),"connections":connections,"at":[0,0]}}),
    );
    let marker = shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .items
        .iter()
        .find(|i| i.kind == "wire_marker")
        .unwrap()
        .id
        .to_string();
    let reps = ends
        .iter()
        .map(|f| {
            t.reps
                .iter()
                .find(|r| r.functions.contains(f))
                .unwrap()
                .id
                .to_string()
        })
        .collect();
    (marker, reps)
}

fn cut_text_fixture(shell: &mut ShellModel) -> String {
    let page = shell.canvas().unwrap().page().tables.page.to_string();
    done(
        shell,
        "command.run",
        json!({"cmd":"item.place","params":{
        "page":page,"kind":{"free_text":{"text":"cut text","at":[0,0]}}}}),
    );
    shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .items
        .iter()
        .find(|i| i.text.as_deref() == Some("cut text"))
        .unwrap()
        .id
        .to_string()
}

#[test]
fn cut_marker_without_copied_wires_alone_and_with_text_is_refused_without_changes() {
    let tmp = Tmp::new("cut-empty-marker");
    let mut s = tmp.shell_in(Mode::Gui, true, "101");
    let (marker, _) = cut_marker_fixture(&mut s, false);
    assert_incomplete_cut_unchanged(&mut s, std::slice::from_ref(&marker));
    let text = cut_text_fixture(&mut s);
    assert_incomplete_cut_unchanged(&mut s, &[marker, text]);
    s.shutdown();
}

#[test]
fn cut_partially_copied_marker_is_refused_without_changes() {
    let tmp = Tmp::new("cut-partial-marker");
    let mut s = tmp.shell_in(Mode::Gui, true, "101");
    let (marker, mut selected) = cut_marker_fixture(&mut s, true);
    selected.push(marker);
    assert_incomplete_cut_unchanged(&mut s, &selected);
    // Compare the stored associations, not the page-table marker projection.
    let text = s.clipboard().text().unwrap().to_owned();
    let all: Vec<_> = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.id.to_string())
        .chain(selected)
        .collect();
    done(&mut s, "select.set", json!({"ids":all}));
    done(&mut s, "edit.copy", json!({}));
    let fragment = ew_viewmodel::ew_api::window::parse_fragment(s.clipboard().text().unwrap()).unwrap();
    let records = fragment.params["records"].as_array().unwrap();
    assert_eq!(
        records
            .iter()
            .find(|r| r["kind"]["wire_marker"].is_object())
            .unwrap()["kind"]["wire_marker"]["connections"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_ne!(s.clipboard().text(), Some(text.as_str()));
    s.shutdown();
}

#[test]
fn cut_unknown_page_item_alone_and_with_text_is_refused_without_changes() {
    // Load a forward-compatible variant through Session::from_records, then persist it in a temporary fixture.
    use ew_viewmodel::ew_api::{Session, SessionOptions, StoreOptions};
    let tmp = Tmp::new("cut-unknown");
    let mut s = tmp.shell_in(Mode::Gui, true, "101");
    let page = s.canvas().unwrap().page().tables.page.to_string();
    s.shutdown();
    let dir = tmp.golden_copy();
    let (source, _) = Session::open_read_only(&dir, "ElektraWatt test").unwrap();
    let project = (**source.query().project()).clone();
    // Retain Golden A's records and libraries using the existing project's persisted JSONL fixture.
    let mut records = Vec::new();
    fn read_records(dir: &std::path::Path, records: &mut Vec<Value>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() && path.file_name().unwrap() != "lib" {
                read_records(&path, records);
            } else if path.extension().is_some_and(|e| e == "jsonl") {
                for line in std::fs::read_to_string(path)
                    .unwrap()
                    .lines()
                    .filter(|l| !l.is_empty())
                {
                    let v: Value = serde_json::from_str(line).unwrap();
                    if v["$k"] != "project" {
                        records.push(v);
                    }
                }
            }
        }
    }
    read_records(&dir, &mut records);
    let unknown = "0192f3a0-0000-7000-8000-00000000a003";
    records.push(json!({"$k":"page_item","id":unknown,"page":page,"kind":{"macro_box":{"name":"later"}}}));
    let records = records
        .into_iter()
        .map(|v| serde_json::from_value(v).unwrap())
        .collect::<Vec<_>>();
    let mut libs = Vec::new();
    for folder in std::fs::read_dir(dir.join("lib")).unwrap() {
        for entry in std::fs::read_dir(folder.unwrap().path()).unwrap() {
            libs.push(
                serde_json::from_str(&std::fs::read_to_string(entry.unwrap().path()).unwrap()).unwrap(),
            );
        }
    }
    let mut fixture = Session::from_records(project, records, libs, SessionOptions::default()).unwrap();
    let target = tmp.0.join("unknown.ew");
    fixture.create(&target, StoreOptions::new("0.0.0-test")).unwrap();
    fixture.close();
    let mut s = ShellModel::new(
        Mode::Gui,
        ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("unknown-config"), "0.0.0-test"),
    );
    done(&mut s, "file.open", json!({"dir":target.to_string_lossy()}));
    done(&mut s, "page.goto", json!({"folio":"101"}));
    let text = cut_text_fixture(&mut s);
    assert_incomplete_cut_unchanged(&mut s, &[unknown.to_owned()]);
    assert_incomplete_cut_unchanged(&mut s, &[unknown.to_owned(), text]);
    s.shutdown();
}

#[test]
fn complete_page_item_cut_still_undoes_and_redoes_once() {
    let tmp = Tmp::new("cut-complete-items");
    let mut s = tmp.shell_in(Mode::Gui, true, "101");
    let (marker, _) = cut_marker_fixture(&mut s, true);
    let text = cut_text_fixture(&mut s);
    let page = s.canvas().unwrap().page().tables.page.to_string();
    done(
        &mut s,
        "command.run",
        json!({"cmd":"item.place","params":{"page":page,
        "kind":{"graphic":{"shape":{"line":{"a":[0,0],"b":[1000,1000]}}}}}}),
    );
    done(
        &mut s,
        "command.run",
        json!({"cmd":"box.structure","params":{"page":page,
        "outline":[[0,0],[1000,0],[1000,1000],[0,1000]],"style":"dotted","assigns":{}}}),
    );
    let before = s.canvas().unwrap().page().tables.clone();
    let mut ids: Vec<_> = before.reps.iter().map(|r| r.id.to_string()).collect();
    ids.extend([marker.clone(), text.clone()]);
    ids.extend(
        before
            .items
            .iter()
            .filter(|i| i.kind == "graphic")
            .map(|i| i.id.to_string()),
    );
    ids.extend(before.structure_boxes.iter().map(|b| b.id.to_string()));
    done(&mut s, "select.set", json!({"ids":ids}));
    let rev = s.revision();
    let recorded = s.recorded_commands().len();
    let calls = s.session_calls();
    done(&mut s, "edit.cut", json!({}));
    assert_eq!(s.session_calls().copy_fragment, calls.copy_fragment + 1);
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    assert_eq!(s.recorded_commands().len(), recorded + 1);
    let clipped = s.clipboard().text().unwrap().to_owned();
    let records = ew_viewmodel::ew_api::window::parse_fragment(&clipped)
        .unwrap()
        .params["records"]
        .clone();
    for id in [&marker, &text] {
        assert!(records.as_array().unwrap().iter().any(|r| r["id"] == id.as_str()));
    }
    assert!(
        records
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"]["graphic"].is_object())
    );
    assert!(
        records
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["$k"] == "structure_box")
    );
    let deleted = s.canvas().unwrap().page().tables.clone();
    assert!(deleted.items.iter().all(|i| !ids.contains(&i.id.to_string())));
    assert!(
        deleted
            .structure_boxes
            .iter()
            .all(|b| !ids.contains(&b.id.to_string()))
    );
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.canvas().unwrap().page().tables.as_ref(), before.as_ref());
    done(&mut s, "edit.redo", json!({}));
    assert_eq!(s.canvas().unwrap().page().tables.as_ref(), deleted.as_ref());
    assert_eq!(s.clipboard().text(), Some(clipped.as_str()));
    s.shutdown();
}
