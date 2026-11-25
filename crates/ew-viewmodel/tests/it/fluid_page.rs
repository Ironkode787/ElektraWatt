// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tools and the overview generator on the sample's fluid page 201, through the shell on a writable copy: a
//! silencer placed facing a free fluid port autoconnects with a fluid line, the interruption-point tool places the
//! fluid form, a drawn overview is generated again without a change, and the PLC and tube panels speak Dutch. Not
//! here: the Fluid workspace's panes, menus and cells in the window (`ew-ui`'s `fluid_workspace.rs`); the fluid forms'
//! words (`fluid_forms.rs`).

use crate::common::{Tmp, cursor, done, grid_point};

use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::tools::Ghost;
use serde_json::json;

/// The fluid lines of the page on show.
fn fluid_lines(s: &ShellModel) -> usize {
    s.canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .iter()
        .filter(|c| c.kind == "fluid")
        .count()
}

/// On fluid page 201 a silencer placed facing a valve's free fluid port autoconnects to it, and the committed
/// connection is a fluid line, not a wire.
#[test]
fn a_silencer_autoconnects_to_a_free_fluid_port_with_a_fluid_line() {
    let tmp = Tmp::new("fluid-silencer");
    let mut s = tmp.sample_in(Mode::Gui, true, "201");
    let (free, step) = {
        let t = &s.canvas().unwrap().page().tables;
        let free: Vec<(i64, i64)> = t
            .cps
            .iter()
            .filter(|p| p.fluid && p.connections.is_empty() && p.dir.is_some())
            .map(|p| p.at)
            .collect();
        (free, t.grid.0 / t.grid.1.max(1))
    };
    assert!(!free.is_empty(), "free fluid ports");
    let before = fluid_lines(&s);
    done(&mut s, "tool.place", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iso.silencer@qet10", "letter": "R"}),
    );
    let mut found = false;
    'search: for at in &free {
        for dy in (-10..=10).filter(|d| *d != 0) {
            for dx in -3..=3 {
                cursor(&mut s, (at.0 + dx * step, at.1 + dy * step));
                if matches!(s.tool_ghost(), Some(Ghost::Place { hints, .. }) if hints > 0) {
                    found = true;
                    break 'search;
                }
            }
        }
    }
    assert!(found, "a place where the silencer faces a free fluid port");
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(fluid_lines(&s), before + 1, "the hint is a fluid line");
    s.shutdown();
}

/// The members Generate overview… previews on each page, by folio, from the Preview pane's notices
/// (`page <folio>: <tag>, <tag>, …`; the mark of a page it adds, `<folio> (new page)`, left out).
fn overview_pages(s: &ShellModel) -> Vec<(String, Vec<String>)> {
    s.preview_pane()
        .notices()
        .iter()
        .filter_map(|n| n.words.strip_prefix("page "))
        .filter_map(|n| n.split_once(": "))
        .map(|(folio, members)| {
            (
                folio.split(' ').next().unwrap_or_default().to_owned(),
                members.split(", ").map(str::to_owned).collect(),
            )
        })
        .collect()
}

/// Doc 11 §11: Generate overview… on a valve terminal whose overview is drawn keeps every member where it
/// stands: the preview names the same members on the same pages, and changes no record.
#[test]
fn a_drawn_overview_is_refreshed_without_a_change() {
    let tmp = Tmp::new("fluid-overview");
    let mut s = tmp.sample_in(Mode::Gui, true, "201");
    let generate = json!({"station": "tag:-201K1"});
    done(&mut s, "fluid.generate_overview", generate.clone());
    let drawn = overview_pages(&s);
    assert!(!drawn.is_empty(), "the first overview's pages");
    done(&mut s, "preview.apply", json!({}));
    done(&mut s, "fluid.generate_overview", generate);
    assert_eq!(overview_pages(&s), drawn);
    let pane = s.preview_pane();
    assert!(pane.entities().is_empty(), "{:?}", pane.entities());
    s.shutdown();
}

/// On a fluid page (the sample's 201) the interruption-point tool places the fluid line's form `ew.iso.ip`, not
/// the wire's.
#[test]
fn the_interruption_tool_places_the_fluid_form_on_a_fluid_page() {
    let tmp = Tmp::new("fluid-ip");
    let mut s = tmp.sample_in(Mode::Gui, true, "201");
    let forms = |s: &ShellModel| {
        s.canvas()
            .unwrap()
            .page()
            .tables
            .reps
            .iter()
            .filter(|r| r.ip.is_some())
            .map(|r| r.symbol_key.clone())
            .collect::<Vec<_>>()
    };
    let before = forms(&s).len();
    done(&mut s, "tool.ip", json!({}));
    let p = grid_point(&s, (60, 6));
    cursor(&mut s, p);
    done(&mut s, "canvas.activate_alt", json!({}));
    done(&mut s, "tool.select", json!({}));
    let after = forms(&s);
    assert_eq!(after.len(), before + 1, "{after:?}");
    assert!(after.iter().all(|k| k.starts_with("ew.iso.ip")), "{after:?}");
    s.shutdown();
}

/// No English in a Dutch run of the fluid workspace: the valve terminal's role badge, the tube table's port roles,
/// line kinds and length sources are Dutch, and its colour column is a tube's colour, not a core's.
#[test]
fn the_fluid_workspace_speaks_dutch() {
    use ew_viewmodel::navigators::{NavKind, NavRole};
    let tmp = Tmp::new("fluid-dutch");
    let mut s = tmp.sample_in(Mode::Gui, true, "201");
    done(&mut s, "view.language", json!({ "value": "nl" }));
    done(&mut s, "rail.show", json!({"pane": "plc"}));
    done(&mut s, "navigator.show", json!({"kind": "tubes"}));
    let rows = s.shown_rows(NavKind::Plc);
    let terminal = rows
        .iter()
        .find(|r| r.role == NavRole::Station.word() && r.label == "-201K1")
        .expect("the valve terminal");
    assert!(
        terminal.badges.iter().any(|b| b == "ventieleiland"),
        "{:?}",
        terminal.badges
    );
    let table = s
        .nav(NavKind::Tubes)
        .and_then(|n| n.table())
        .cloned()
        .expect("the tube table");
    let column = |key: &str| table.columns.iter().position(|c| c.key == key).expect(key);
    assert_eq!(table.columns[column("colour")].label, "Kleur");
    let first = &table.nodes[0].cells;
    assert_eq!(first[column("line_kind")], "werkleiding", "{first:?}");
    assert_eq!(first[column("from_role")], "werkpoort", "{first:?}");
    assert_eq!(first[column("length_source")], "ontbreekt", "{first:?}");
    s.shutdown();
}
