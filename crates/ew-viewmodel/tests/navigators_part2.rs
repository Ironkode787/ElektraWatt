// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The further navigators and highlighting of WP-S2.06 part 2 on Golden A (acceptance checks NP3's model half, NP4's
//! model half, NP5 and NP6's model half): highlighting 24VDC-1 from the Potentials navigator (slot 0, its members'
//! polylines on folios 102 and 103, the describe lines, one members call per revision, toggling off, a fifth highlight
//! replacing the first), a canvas click on W29 marking its row in Connections and Potentials, the New structure node
//! pre-fill (FT-461's second half), the six new words and the two new actions.

mod common;

use std::collections::BTreeSet;

use common::{Tmp, click, done, state};
use ew_viewmodel::highlight::{HighlightTarget, SLOT_COLOURS};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use ew_viewmodel::tools::OverlayItem;
use serde_json::{Value, json};

/// The key of the row labelled `label` with role `role` in an open navigator.
fn row_key(shell: &ShellModel, kind: NavKind, role: NavRole, label: &str) -> String {
    let t = shell
        .nav(kind)
        .and_then(|n| n.table())
        .expect("an open navigator with its table");
    t.nodes
        .iter()
        .find(|n| n.role == role && n.label == label)
        .map(|n| n.key.clone())
        .unwrap_or_else(|| panic!("no {role:?} row {label}"))
}

/// The records of every table row of navigator `kind` whose cell `col` is `text`.
fn records_where(
    shell: &ShellModel,
    kind: NavKind,
    col: usize,
    text: &str,
) -> Vec<ew_viewmodel::ew_api::output::Uuid> {
    let t = shell.nav(kind).and_then(|n| n.table()).unwrap();
    let mut v: Vec<_> = t
        .nodes
        .iter()
        .filter(|n| n.table_row && n.cells[col] == text)
        .flat_map(|n| n.records.clone())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The row lines under the header of the navigator titled `title`.
fn nav_rows(shell: &ShellModel, title: &str) -> Vec<String> {
    let d = shell.describe();
    let mut out = Vec::new();
    let mut inside = false;
    for l in d.lines() {
        if l.starts_with(&format!("navigator \"{title}\"")) {
            inside = true;
            continue;
        }
        if inside && !l.starts_with("  row ") {
            break;
        }
        if inside {
            out.push(l.to_owned());
        }
    }
    out
}

fn lines_with(shell: &ShellModel, prefix: &str) -> Vec<String> {
    shell
        .describe()
        .lines()
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

#[track_caller]
fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action}: {other:?}"),
    }
}

/// The drawn polylines of `members` on the shown page of group 0, in sheet nm.
fn drawn(shell: &ShellModel, members: &[ew_viewmodel::ew_api::output::Uuid]) -> Vec<Vec<(i64, i64)>> {
    let page = shell.canvas().unwrap().page();
    page.tables
        .connections
        .iter()
        .filter(|c| members.contains(&c.id))
        .map(|c| {
            c.polyline
                .iter()
                .map(|p| page.snapper.page_to_sheet(*p))
                .collect()
        })
        .collect()
}

/// NP3 (FR-UI-018, FT-362; D14), the model half: Ctrl+Shift+H on 24VDC-1 in the Potentials navigator highlights it
/// in slot 0; on folios 102 and 103 the overlay holds exactly its members' polylines, and the members are the
/// connections of the wire-list rows whose potential is 24VDC-1; the describe lines; one `highlight_members` call per
/// highlight and revision; pressing it again removes it; a fifth highlight replaces the first; other rows refuse.
#[test]
fn np3_highlighting_24vdc_1_from_the_potentials_navigator() {
    let tmp = Tmp::new("nav2-hl");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "connections"}));
    done(&mut shell, "navigator.show", json!({"kind": "potentials"}));
    let key = row_key(&shell, NavKind::Potentials, NavRole::Potential, "24VDC-1");
    let calls = shell.session_calls().highlight_members;
    done(
        &mut shell,
        "navigator.highlight",
        json!({"kind": "potentials", "key": key}),
    );
    assert_eq!(shell.session_calls().highlight_members, calls + 1);
    let h = &shell.highlights().items()[0];
    assert_eq!(h.slot, 0);
    assert!(matches!(h.target, HighlightTarget::Potential(_)));
    let members = h.members.as_ref().clone();
    // Column 5 of the wire list is the potential.
    assert_eq!(
        members,
        records_where(&shell, NavKind::Connections, 5, "24VDC-1"),
        "the connections of the wire-list rows whose potential is 24VDC-1"
    );
    for folio in ["102", "103"] {
        done(&mut shell, "page.goto", json!({"folio": folio}));
        let overlay = shell.highlight_overlay(0);
        assert_eq!(overlay.len(), 1, "{folio}");
        let (OverlayItem::Highlight { polylines, slot }, n) = &overlay[0] else {
            panic!("a highlight item");
        };
        assert_eq!(*slot, 0);
        let want = drawn(&shell, &members);
        assert!(!want.is_empty(), "24VDC-1 is drawn on {folio}");
        assert_eq!(polylines, &want, "{folio}: exactly the members' polylines");
        assert_eq!(*n, want.len());
        assert_eq!(
            lines_with(&shell, "  highlight "),
            [format!(
                "  highlight slot 0 potential \"24VDC-1\" connections {n}"
            )]
        );
    }
    assert_eq!(
        shell.session_calls().highlight_members,
        calls + 1,
        "page switches read no members"
    );
    // The row line ends with its slot.
    let line = nav_rows(&shell, "Potentials")
        .into_iter()
        .find(|l| l.contains("row level 1 potential \"24VDC-1\""))
        .unwrap();
    assert!(line.ends_with(" highlight 0"), "{line}");
    let group = nav_rows(&shell, "Connections")
        .into_iter()
        .find(|l| l.contains("row level 1 potential \"24VDC-1\""))
        .unwrap();
    assert!(
        !group.contains("highlight"),
        "a group row names no potential: {group}"
    );
    // A new revision reads the members once more; the highlight survives it.
    done(
        &mut shell,
        "pages.reorder",
        json!({"pages": ["106"], "before": "103"}),
    );
    done(&mut shell, "navigator.apply", json!({}));
    assert_eq!(shell.session_calls().highlight_members, calls + 2);
    assert_eq!(shell.highlights().items().len(), 1);
    // The undo is a revision too (and gives the folios back).
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(shell.session_calls().highlight_members, calls + 3);
    // Pressing it again removes it.
    done(
        &mut shell,
        "navigator.highlight",
        json!({"kind": "potentials", "key": key}),
    );
    assert!(shell.highlights().is_empty());
    assert!(lines_with(&shell, "  highlight ").is_empty());
    // Four slots, then a fifth replaces the oldest.
    for (i, name) in ["24VDC-1", "24VDC-2", "24VDC-3", "24VDC-4", "0V"]
        .iter()
        .enumerate()
    {
        let k = row_key(&shell, NavKind::Potentials, NavRole::Potential, name);
        done(
            &mut shell,
            "navigator.highlight",
            json!({"kind": "potentials", "key": k}),
        );
        let slots: Vec<(u8, &str)> = shell
            .highlights()
            .items()
            .iter()
            .map(|h| (h.slot, h.name.as_str()))
            .collect();
        match i {
            0..=3 => assert_eq!(slots.last(), Some(&(u8::try_from(i).unwrap(), *name))),
            _ => assert_eq!(slots, [(1, "24VDC-2"), (2, "24VDC-3"), (3, "24VDC-4"), (0, "0V")]),
        }
    }
    // `off` only removes.
    let k = row_key(&shell, NavKind::Potentials, NavRole::Potential, "24VDC-1");
    done(
        &mut shell,
        "navigator.highlight",
        json!({"kind": "potentials", "key": k, "off": true}),
    );
    assert_eq!(shell.highlights().items().len(), 4);
    // A connection row highlights its net: W29's row 102.10 in Connections.
    done(&mut shell, "view.clear_highlights", json!({}));
    assert!(shell.highlights().is_empty());
    assert_eq!(
        shell.dispatch("view.clear_highlights", &json!({})),
        Outcome::Refused {
            reason_key: "reason-no-highlights"
        }
    );
    let w = row_key(&shell, NavKind::Connections, NavRole::Connection, "102.10");
    done(
        &mut shell,
        "navigator.highlight",
        json!({"kind": "connections", "key": w}),
    );
    let h = &shell.highlights().items()[0];
    assert!(matches!(h.target, HighlightTarget::Net(_)));
    assert_eq!(h.word(), "connection");
    // The row's first connection (by ID) is W29 on 103: its page-local net is drawn there.
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    let l = lines_with(&shell, "  highlight ");
    assert_eq!(l.len(), 1);
    assert!(
        l[0].starts_with("  highlight slot 0 connection \"102.10\" connections "),
        "{}",
        l[0]
    );
    // Any other row refuses; the bond row too (it names no potential).
    let bond = row_key(
        &shell,
        NavKind::Potentials,
        NavRole::Potential,
        "B1 (0V \u{21C4} PE)",
    );
    let m = failed(
        &mut shell,
        "navigator.highlight",
        json!({"kind": "potentials", "key": bond}),
    );
    assert_eq!(m, "Only potentials and connections can be highlighted.");
    let group = row_key(&shell, NavKind::Connections, NavRole::Potential, "24VDC-1");
    let m = failed(
        &mut shell,
        "navigator.highlight",
        json!({"kind": "connections", "key": group}),
    );
    assert_eq!(m, "Only potentials and connections can be highlighted.");
    assert_eq!(SLOT_COLOURS[0], (0xA8, 0x6F, 0x00), "FT-506: dark orange");
    shell.shutdown();
}

/// NP4 (§6.5), the model half: with Connections and Potentials open, selecting W29 on folio 103 from the canvas marks its
/// wire row in both (in Potentials under 24VDC-1), reveals them and leaves the focus on the canvas. On Golden A, W29's
/// provisional route on 103 lies on other wires' routes end to end, so the first click there picks the first wire of
/// the pick stack and repeated clicks walk to W29 (FT-500, which closes deviation N30-3); every click marks the row of
/// the wire it selects.
#[test]
fn np4_selecting_w29_on_the_canvas_marks_its_rows_in_both_navigators() {
    let tmp = Tmp::new("nav2-sync");
    let mut shell = tmp.shell_in(Mode::Headless, false, "103");
    done(&mut shell, "navigator.show", json!({"kind": "connections"}));
    done(&mut shell, "navigator.show", json!({"kind": "potentials"}));
    let row = row_key(&shell, NavKind::Connections, NavRole::Connection, "102.10");
    let (_, node) = shell.nav(NavKind::Connections).unwrap().node_of(&row).unwrap();
    let w29 = node
        .drawables
        .iter()
        .find(|d| d.folio == "103")
        .map(|d| d.record)
        .expect("W29 is drawn on 103");
    // Collapse 24VDC-1 in Potentials, so that the selection must reveal the row.
    let pot = row_key(&shell, NavKind::Potentials, NavRole::Potential, "24VDC-1");
    let under = format!("{pot}/");
    done(
        &mut shell,
        "navigator.toggle",
        json!({"kind": "potentials", "key": pot}),
    );
    done(&mut shell, "ui.focus", json!({"area": "canvas"}));
    let marked = |shell: &ShellModel, kind| {
        shell
            .nav(kind)
            .unwrap()
            .marked()
            .iter()
            .cloned()
            .collect::<BTreeSet<String>>()
    };
    // A pointer click on the middle of W29's route on 103.
    let page = shell.canvas().unwrap().page();
    let line = page
        .tables
        .connections
        .iter()
        .find(|x| x.id == w29)
        .unwrap()
        .polyline
        .clone();
    let p = page
        .snapper
        .page_to_sheet(((line[1].0 + line[2].0) / 2, (line[1].1 + line[2].1) / 2));
    click(&mut shell, p, json!({}));
    let picked = shell.selection().items.clone();
    assert_eq!(picked.len(), 1, "the click selects one wire");
    let k = marked(&shell, NavKind::Connections);
    assert_eq!(k.len(), 1);
    let (_, n) = shell
        .nav(NavKind::Connections)
        .unwrap()
        .node_of(k.iter().next().unwrap())
        .unwrap();
    assert!(n.records.contains(&picked[0].0));
    let in_pots = marked(&shell, NavKind::Potentials);
    assert_eq!(in_pots.len(), 1);
    assert!(in_pots.iter().next().unwrap().contains("/connection:"));
    assert_eq!(state(&shell)["focus"], "canvas");
    // W29 itself: the same click repeated walks the pick stack to it.
    let stack = {
        let c = shell.canvas().unwrap();
        let page = c.page();
        ew_viewmodel::tools::pick_stack(
            page,
            &page.snapper,
            p,
            ew_viewmodel::canvas::HOVER_SLOP_PT / c.points_per_nm(),
        )
    };
    assert_eq!(
        picked[0], stack[0],
        "the first click picks the first of the stack"
    );
    let at_w29 = stack
        .iter()
        .position(|e| e.0 == w29)
        .expect("W29 is in the pick stack at its route");
    for (k, next) in stack.iter().enumerate().take(at_w29 + 1).skip(1) {
        click(&mut shell, p, json!({}));
        assert_eq!(shell.selection().items, vec![*next], "click {}", k + 1);
    }
    assert_eq!(shell.selection().items[0].0, w29);
    assert_eq!(
        marked(&shell, NavKind::Connections),
        BTreeSet::from([row.clone()])
    );
    let in_pots = marked(&shell, NavKind::Potentials);
    assert_eq!(in_pots.len(), 1);
    let k = in_pots.into_iter().next().unwrap();
    assert!(k.starts_with(&under), "{k} under 24VDC-1");
    let segment = &row[row.rfind('/').unwrap()..];
    assert!(k.ends_with(segment), "{k}: the same wire row");
    assert!(
        shell.shown_rows(NavKind::Potentials).iter().any(|r| r.key == k),
        "the collapsed potential was expanded"
    );
    assert_eq!(shell.nav_cursor(NavKind::Potentials).as_deref(), Some(k.as_str()));
    assert_eq!(state(&shell)["focus"], "canvas", "the focus stays on the canvas");
    shell.shutdown();
}

/// NP5 (FT-461's second half, §7.1): Structure ▸ New node with +J-E-2416 marked and kind `location` opens with the
/// parent filled in; with kind `function` it is left empty.
#[test]
fn np5_new_structure_node_prefills_the_marked_parent() {
    let tmp = Tmp::new("nav2-struct");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "structure"}));
    let k = row_key(&shell, NavKind::Structure, NavRole::StructureNode, "+J-E-2416");
    done(
        &mut shell,
        "navigator.select",
        json!({"kind": "structure", "keys": [k]}),
    );
    assert_eq!(shell.new_structure_parent("location"), "+J-E-2416");
    assert_eq!(shell.new_structure_parent("function"), "");
    shell.dispatch("structure.new", &json!({"kind": "location"}));
    let Some(Dialog::Form(f)) = shell.dialog() else {
        panic!("the New structure node dialog");
    };
    let field = |name: &str| f.fields.iter().find(|x| x.name == name).unwrap().value.clone();
    assert_eq!(field("kind"), "location");
    assert_eq!(field("parent"), "+J-E-2416");
    shell.shutdown();
}

/// NP6 (§7.4, D15), the model half: the 6 words are in the vocabulary (291 words) and reached by the describe text,
/// and the registry has 217 actions with Ctrl+Shift+H in the navigator context.
#[test]
fn np6_the_six_words_and_the_two_actions() {
    use ew_viewmodel::describe::{NAV2_WORDS, VOCABULARY};
    assert_eq!(NAV2_WORDS.len(), 6);
    for w in NAV2_WORDS {
        assert!(VOCABULARY.contains(w), "{w}");
    }
    // WP-S2.09 part 2 adds the message panel's sixteen words and five actions; part 3 the Preview tab's and the
    // history panel's thirteen words and eight actions; WP-S2.06 part 3 table mode's six words and fourteen actions;
    // WP-S2.17 part 1 View ▸ Theme and View ▸ Density; part 3 the rails' six words and two actions; FT-541
    // `props.filter`; the G-PP triage's Draw ▸ Bond (`bond.draw`), the tag proposal
    // (`props.accept_tag_proposal`) a terminal CP's side `internal` and
    // View ▸ Bottom dock (`view.bottom_dock`).
    // WP-S2.07 part 1: the strip editor's word `accessory` and six actions.
    assert_eq!(VOCABULARY.len(), 356);
    assert_eq!(ew_viewmodel::actions::ACTIONS.len(), 268);
    let hl = ew_viewmodel::actions::ACTIONS
        .iter()
        .find(|a| a.id == "navigator.highlight")
        .unwrap();
    assert_eq!(hl.shortcut, ["Ctrl+Shift+H"]);
    let tmp = Tmp::new("nav2-words");
    let mut shell = tmp.shell_in(Mode::Headless, false, "102");
    for k in ["cables", "plc", "parts", "potentials"] {
        done(&mut shell, "navigator.show", json!({"kind": k}));
    }
    let key = row_key(&shell, NavKind::Potentials, NavRole::Potential, "24VDC-1");
    done(
        &mut shell,
        "navigator.highlight",
        json!({"kind": "potentials", "key": key}),
    );
    let text = shell.describe();
    let words: BTreeSet<&str> = text
        .lines()
        .flat_map(|l| {
            let mut out = Vec::new();
            let mut quoted = false;
            for part in l.split('"') {
                if !quoted {
                    out.extend(part.split_whitespace());
                }
                quoted = !quoted;
            }
            out
        })
        .collect();
    for w in NAV2_WORDS {
        assert!(words.contains(w), "`{w}` is reached");
    }
    shell.shutdown();
}

/// R30 F9: opening a paged report-block row goes to its first folio's page and selects the block; an embedded block
/// (no generated pages) has no open target and leaves the page unchanged.
#[test]
fn r30_opening_a_paged_report_block_goes_to_its_first_folio() {
    let tmp = Tmp::new("nav2-blocks-open");
    let mut shell = tmp.shell_in(Mode::Headless, false, "102");
    done(&mut shell, "navigator.show", json!({"kind": "blocks"}));
    let t = shell.nav(NavKind::Blocks).and_then(|n| n.table()).unwrap();
    let wire = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::ReportBlock && n.cells[2] == "907")
        .cloned()
        .expect("a paged block at folio 907");
    let embedded = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::ReportBlock && n.cells[4] == "0")
        .cloned()
        .expect("an embedded block");
    done(
        &mut shell,
        "navigator.open",
        json!({"kind": "blocks", "key": wire.key}),
    );
    assert_eq!(shell.current_folio(), Some("907"));
    let selected: Vec<_> = shell.selection().items.iter().map(|e| e.0).collect();
    assert_eq!(selected, wire.records, "the block is selected");
    assert!(embedded.open.is_none(), "an embedded block has no open target");
    let _ = shell.dispatch("navigator.open", &json!({"kind": "blocks", "key": embedded.key}));
    assert_eq!(shell.current_folio(), Some("907"), "the page is unchanged");
    shell.shutdown();
}
