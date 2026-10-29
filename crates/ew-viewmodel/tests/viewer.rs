// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The read-only viewer through the shell on temporary copies of Golden A (WP-S2.03 part 1, acceptance checks V2 to
//! V5, V9 and the model half of V10): the canvas area, walks, jumps and the per-tab history, the page navigator, the
//! keyboard grid cursor, links, Go to, the transcript and describe members and the exact counts.

#[path = "common/fresh_dir.rs"]
mod fresh_dir;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;

use ew_viewmodel::area::MAX_TABS;
use ew_viewmodel::canvas::{AUTO_PAN_INSET, PageId, rect_centre};
use ew_viewmodel::navigator::{NavMode, NavSort, PageNavigator, RowKind};
use ew_viewmodel::script;
use ew_viewmodel::shell::{Focus, Mode, Outcome, RECENT_PAGES, ShellConfig, ShellModel};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        Tmp(fresh_dir::fresh_dir("ew-vm-viewer"))
    }

    /// A headless shell with a copy of Golden A open (read-only unless `writable`).
    fn shell(&self, writable: bool) -> ShellModel {
        let dir = self.0.join("J-E-2416.ew");
        if !dir.exists() {
            copy_dir(Path::new(GOLDEN_A), &dir);
        }
        let mut shell = ShellModel::new(
            Mode::Headless,
            ShellConfig::in_dir(&self.0.join("config"), "0.0.0-test"),
        );
        done(
            &mut shell,
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": !writable}),
        );
        shell
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[track_caller]
fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
    if let Some(a) = shell.area() {
        a.check().unwrap();
    }
}

#[track_caller]
fn refused(shell: &mut ShellModel, action: &str, params: Value) -> &'static str {
    match shell.dispatch(action, &params) {
        Outcome::Refused { reason_key } => reason_key,
        other => panic!("{action}: {other:?}"),
    }
}

fn mm(nm: i64) -> f64 {
    nm as f64 / 1e6
}

fn state(shell: &ShellModel) -> Value {
    serde_json::from_str(&script::state_json(shell)).unwrap()
}

fn view_nm(shell: &ShellModel) -> [i64; 4] {
    shell.canvas().unwrap().view_nm()
}

/// The centre of the link on the canvas whose target prints `cell`.
fn link_to(shell: &ShellModel, cell: &str) -> (i64, i64) {
    let c = shell.canvas().unwrap();
    let link = c
        .page()
        .links
        .iter()
        .find(|l| l.target.as_ref().and_then(|t| t.cell.as_deref()) == Some(cell))
        .unwrap_or_else(|| panic!("a link to {cell} on {}", c.page().folio));
    rect_centre(link.rect)
}

#[test]
fn v2_the_canvas_area_keeps_its_invariants_under_every_tab_action() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    let a = shell.area().unwrap();
    assert_eq!((a.groups.len(), a.tab_count(), a.current().id), (1, 1, 1));
    assert_eq!(shell.focus(), Some(Focus::Canvas(0)));
    assert_eq!(refused(&mut shell, "tab.close", json!({})), "reason-last-tab");
    assert_eq!(refused(&mut shell, "tab.next", json!({})), "reason-one-tab");
    assert_eq!(
        refused(&mut shell, "tab.focus_other", json!({})),
        "reason-one-group"
    );
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "view.zoom_to", json!({"percent": 200}));
    let view = view_nm(&shell);
    // A new tab is on the same page with the same view.
    done(&mut shell, "tab.new", json!({}));
    assert_eq!((shell.current_folio(), view_nm(&shell)), (Some("102"), view));
    assert_eq!(shell.area().unwrap().current().id, 2);
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    done(&mut shell, "tab.next", json!({}));
    assert_eq!(
        (shell.current_folio(), view_nm(&shell)),
        (Some("102"), view),
        "wraps to the first"
    );
    done(&mut shell, "tab.previous", json!({}));
    assert_eq!(shell.current_folio(), Some("103"));
    // Side by side: the page after the focused one, focused.
    done(&mut shell, "view.side_by_side", json!({}));
    assert_eq!(
        (shell.current_folio(), shell.focus()),
        (Some("106"), Some(Focus::Canvas(1)))
    );
    done(&mut shell, "tab.focus_other", json!({}));
    assert_eq!(shell.current_folio(), Some("103"));
    done(&mut shell, "tab.open", json!({"folio": "201", "group": 1}));
    assert_eq!(
        (shell.area().unwrap().focused, shell.current_folio()),
        (1, Some("201"))
    );
    assert!(matches!(
        shell.dispatch("tab.open", &json!({"folio": "nope"})),
        Outcome::Failed { .. }
    ));
    assert!(matches!(
        shell.dispatch("tab.activate", &json!({"group": 2, "index": 0})),
        Outcome::Failed { .. }
    ));
    done(&mut shell, "tab.activate", json!({"group": 0, "index": 0}));
    assert_eq!(shell.current_folio(), Some("102"));
    done(
        &mut shell,
        "tab.move",
        json!({"group": 1, "index": 0, "to_group": 0, "to_index": 0}),
    );
    assert_eq!(shell.current_folio(), Some("106"));
    // Off: the right group's tabs move to the end of the left group.
    done(&mut shell, "view.side_by_side", json!({}));
    let a = shell.area().unwrap();
    assert_eq!((a.groups.len(), a.tab_count()), (1, 4));
    let ids: Vec<u32> = a.tabs().map(|(_, _, t)| t.id).collect();
    assert_eq!(ids, [3, 1, 2, 4]);
    // Up to 32 tabs; IDs never reused after closing.
    while shell.area().unwrap().tab_count() < MAX_TABS {
        done(&mut shell, "tab.new", json!({}));
    }
    assert_eq!(refused(&mut shell, "tab.new", json!({})), "reason-tabs-full");
    assert_eq!(
        refused(&mut shell, "view.side_by_side", json!({})),
        "reason-tabs-full"
    );
    done(&mut shell, "tab.close", json!({}));
    done(&mut shell, "tab.new", json!({}));
    let ids: BTreeSet<u32> = shell.area().unwrap().tabs().map(|(_, _, t)| t.id).collect();
    assert_eq!(ids.len(), MAX_TABS);
    assert!(
        ids.contains(&33) && !ids.contains(&32),
        "the closed tab's ID is not reused: {ids:?}"
    );
    shell.shutdown();
}

#[test]
fn v2_a_tab_keeps_its_page_by_identity_across_a_renumber() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(true);
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    let page = shell.current_page().unwrap();
    let id = page.uuid_text();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.renumber", "params": {"scope": {"pages": [id]}, "start": 150}}),
    );
    assert_eq!(shell.current_page(), Some(page));
    assert_eq!(shell.current_folio(), Some("150"));
    assert_eq!(shell.canvas().unwrap().page().folio, "150");
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(shell.current_folio(), Some("103"));
    done(&mut shell, "app.quit", json!({"discard": true}));
}

#[test]
fn v3_walks_jumps_back_and_forward_with_the_exact_view() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(true);
    // Walks record nothing.
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    assert_eq!(shell.area().unwrap().current().history.back_len(), 1);
    done(&mut shell, "page.next", json!({}));
    done(&mut shell, "page.next", json!({}));
    assert_eq!(shell.current_folio(), Some("103"));
    assert_eq!(
        shell.area().unwrap().current().history.back_len(),
        1,
        "walks record nothing"
    );
    // The per-page memory: a zoomed 102 comes back zoomed after walking away and back.
    done(&mut shell, "page.previous", json!({}));
    done(
        &mut shell,
        "view.zoom_to",
        json!({"percent": 400, "x_mm": 100, "y_mm": 80}),
    );
    let zoomed = view_nm(&shell);
    done(&mut shell, "page.next", json!({}));
    assert!(
        shell.canvas().unwrap().is_fitted(),
        "a page seen for the first time is fitted"
    );
    done(&mut shell, "page.previous", json!({}));
    assert_eq!(view_nm(&shell), zoomed);
    // A jump records one entry; Back restores the exact view; Forward returns.
    done(&mut shell, "nav.goto", json!({"text": "103-A1"}));
    let jumped = view_nm(&shell);
    assert_eq!(shell.area().unwrap().current().history.back_len(), 2);
    done(&mut shell, "nav.back", json!({}));
    assert_eq!((shell.current_folio(), view_nm(&shell)), (Some("102"), zoomed));
    done(&mut shell, "nav.forward", json!({}));
    assert_eq!((shell.current_folio(), view_nm(&shell)), (Some("103"), jumped));
    assert_eq!(
        refused(&mut shell, "nav.forward", json!({})),
        "reason-history-empty"
    );
    // A jump to the page and view already shown records nothing.
    done(&mut shell, "nav.goto", json!({"text": "103-A1"}));
    assert_eq!(shell.area().unwrap().current().history.back_len(), 2);
    // A fitted entry refits after a resize.
    done(&mut shell, "page.goto", json!({"folio": "201"}));
    done(&mut shell, "page.goto", json!({"folio": "202"}));
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 600, "height": 400}),
    );
    done(&mut shell, "nav.back", json!({}));
    let c = shell.canvas().unwrap();
    assert_eq!(c.page().folio, "201");
    assert!(c.is_fitted() && (c.zoom() - c.fit_zoom()).abs() < 1e-12 && c.pane() == (600.0, 400.0));
    // The jump view: max(current zoom, 2 × fit); a page destination arrives (fitted on a first visit).
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 1200, "height": 800}),
    );
    done(&mut shell, "nav.goto", json!({"text": "106-E1"}));
    let c = shell.canvas().unwrap();
    assert!((c.zoom() - 2.0 * c.fit_zoom()).abs() < 1e-9, "{}", c.zoom());
    done(&mut shell, "view.zoom_to", json!({"percent": 800}));
    done(&mut shell, "nav.goto", json!({"text": "103-B6"}));
    assert!((shell.canvas().unwrap().zoom() - 8.0).abs() < 1e-9);
    // A vanished page is skipped: a page made and visited, then undone.
    let after = shell
        .page_rows()
        .iter()
        .find(|r| r.folio == "106")
        .map(|r| PageId::of(r).uuid_text())
        .unwrap();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"after": after}}),
    );
    done(&mut shell, "page.goto", json!({"folio": "107"}));
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    done(&mut shell, "edit.undo", json!({}));
    assert!(!shell.page_labels().contains(&"107".to_owned()));
    // WP-S2.09 §8.5: the undo restores the page the step was made on (103) as a jump from 101.
    assert_eq!(shell.current_folio(), Some("103"), "the step's context");
    done(&mut shell, "nav.back", json!({}));
    assert_eq!(shell.current_folio(), Some("101"), "the restore's back entry");
    done(&mut shell, "nav.back", json!({}));
    assert_eq!(shell.current_folio(), Some("103"), "the entry of 107 was dropped");
    // The cap of 50.
    for i in 0..60 {
        let folio = if i % 2 == 0 { "101" } else { "102" };
        done(&mut shell, "page.goto", json!({"folio": folio}));
    }
    assert_eq!(shell.area().unwrap().current().history.back_len(), 50);
    done(&mut shell, "app.quit", json!({"discard": true}));
}

#[test]
fn v4_the_navigator_on_golden_a() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    let rows = shell.page_rows().to_vec();
    assert_eq!(
        rows.len(),
        36,
        "FR-PAG-024: the 36 entries of the derived page order"
    );
    // Flat by folio: the rows equal the page labels.
    done(&mut shell, "pages.mode", json!({"mode": "flat"}));
    let flat: Vec<String> = shell
        .navigator_rows()
        .iter()
        .map(|r| match r.kind {
            RowKind::Page(i) => rows[i].folio.clone(),
            RowKind::Node { .. } => panic!("no node in flat mode"),
        })
        .collect();
    assert_eq!(flat, shell.page_labels());
    // By type: kind_rank, then page order.
    done(&mut shell, "pages.sort", json!({"by": "type"}));
    let by_type: Vec<usize> = shell
        .navigator_rows()
        .iter()
        .map(|r| match r.kind {
            RowKind::Page(i) => i,
            RowKind::Node { .. } => unreachable!(),
        })
        .collect();
    let mut oracle: Vec<usize> = (0..rows.len()).collect();
    oracle.sort_by_key(|&i| (rows[i].kind_rank, i));
    assert_eq!(by_type, oracle);
    // By section: a node per section in order of their first page, front matter first; every page once.
    done(&mut shell, "pages.sort", json!({"by": "folio"}));
    done(&mut shell, "pages.mode", json!({"mode": "section"}));
    let v = shell.navigator_rows();
    let nodes: Vec<&str> = v
        .iter()
        .filter(|r| matches!(r.kind, RowKind::Node { .. }))
        .map(|r| r.key.as_str())
        .collect();
    let mut seen = Vec::new();
    for r in &rows {
        let key = r
            .section
            .as_ref()
            .map_or("s:front".to_owned(), |(n, _)| format!("s:{n}"));
        if !seen.contains(&key) {
            seen.push(key);
        }
    }
    assert_eq!(nodes, seen.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(nodes[0], "s:front");
    assert_eq!(
        v.iter().filter(|r| matches!(r.kind, RowKind::Page(_))).count(),
        36
    );
    // Structure: function, location, section nodes; keys unique; every page once.
    done(&mut shell, "pages.mode", json!({"mode": "structure"}));
    let v = shell.navigator_rows();
    let keys: BTreeSet<&str> = v.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys.len(), v.len());
    assert_eq!(
        v.iter().filter(|r| matches!(r.kind, RowKind::Page(_))).count(),
        36
    );
    assert!(v.iter().all(|r| r.level <= 4));
    // The filter keeps ancestors of a match and expands them.
    done(&mut shell, "pages.mode", json!({"mode": "section"}));
    done(&mut shell, "pages.toggle", json!({"key": "s:1"}));
    assert!(shell.navigator_rows().iter().all(|r| !r.key.starts_with("s:1/")));
    done(&mut shell, "pages.filter", json!({"text": "power supply 2"}));
    let v = shell.navigator_rows();
    assert_eq!(v.len(), 2, "{v:?}");
    assert_eq!((v[0].key.as_str(), v[0].expanded), ("s:1", true));
    done(&mut shell, "pages.filter", json!({"text": ""}));
    done(&mut shell, "pages.toggle", json!({"key": "s:1"}));
    // The row cursor: on the current page until moved (a view change puts it back); Up, Down, Home, End.
    done(&mut shell, "pages.mode", json!({"mode": "section"}));
    let current = shell.navigator_row_key().unwrap();
    assert!(current.starts_with("s:front/p:"), "{current}");
    done(&mut shell, "pages.row", json!({"step": 1}));
    let next = shell.navigator_row_key().unwrap();
    assert_ne!(next, current);
    done(&mut shell, "pages.row", json!({"to": "last"}));
    assert_eq!(
        shell.navigator_row_key(),
        shell.navigator_rows().last().map(|r| r.key.clone())
    );
    done(&mut shell, "pages.row", json!({"to": "first"}));
    assert_eq!(shell.navigator_row_key().as_deref(), Some("s:front"));
    done(&mut shell, "pages.row", json!({"step": -1}));
    assert_eq!(shell.navigator_row_key().as_deref(), Some("s:front"), "clamped");
    // Open from the navigator is a jump; a new tab with new_tab.
    let row_102 = shell
        .navigator_rows()
        .into_iter()
        .find(|r| matches!(r.kind, RowKind::Page(i) if rows[i].folio == "102"))
        .unwrap();
    done(&mut shell, "pages.open", json!({"key": row_102.key}));
    assert_eq!(shell.current_folio(), Some("102"));
    assert_eq!(shell.area().unwrap().current().history.back_len(), 1);
    done(&mut shell, "pages.open", json!({"folio": "103", "new_tab": true}));
    assert_eq!(
        (shell.area().unwrap().tab_count(), shell.current_folio()),
        (2, Some("103"))
    );
    assert!(matches!(
        shell.dispatch("pages.open", &json!({"key": "s:1"})),
        Outcome::Failed { .. }
    ));
    // Accessible names: properties carry level and position, the name the folio, title, kind and badges.
    let v = shell.navigator_rows();
    let names: Vec<String> = v
        .iter()
        .map(|r| PageNavigator::accessible_name(r, &rows, shell.current_page()))
        .collect();
    assert!(names.contains(&"Front matter, section".to_owned()), "{names:?}");
    let row_103 = v
        .iter()
        .position(|r| matches!(r.kind, RowKind::Page(i) if rows[i].folio == "103"))
        .unwrap();
    assert!(names[row_103].starts_with("103 "), "{}", names[row_103]);
    assert!(
        names[row_103].ends_with(", page, QET grid, current"),
        "{}",
        names[row_103]
    );
    assert!(v.iter().all(|r| r.position >= 1 && r.position <= r.set_size));
    // Keys are stable across revisions (nothing but the revision changes here: a no-op settings write).
    let before: Vec<String> = v.iter().map(|r| r.key.clone()).collect();
    assert_eq!(NavMode::default(), NavMode::Section);
    assert_eq!(NavSort::default(), NavSort::Folio);
    let again: Vec<String> = shell.navigator_rows().iter().map(|r| r.key.clone()).collect();
    assert_eq!(before, again);
    shell.shutdown();
}

#[test]
fn v4_navigator_keys_are_stable_across_a_revision() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(true);
    done(&mut shell, "pages.mode", json!({"mode": "structure"}));
    let before: Vec<String> = shell.navigator_rows().iter().map(|r| r.key.clone()).collect();
    let rev = shell.revision();
    let xref = json!({"key":"qet-style","ip":"{page}-{row}{column}","aux_to_main":"[{page}-{row}{column}]","main_to_aux":"∞ {list}","structure":"when_different","same_page":"full","matrix":{"per_row":3,"max":9,"overflow":"…"}});
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "project.settings.set", "params": {"member": "xref_format", "value": xref}}),
    );
    assert_ne!(shell.revision(), rev);
    let after: Vec<String> = shell.navigator_rows().iter().map(|r| r.key.clone()).collect();
    assert_eq!(before, after);
    done(&mut shell, "app.quit", json!({"discard": true}));
}

#[test]
fn v5_the_grid_cursor() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    let c = shell.canvas().unwrap();
    let centre = c.centre();
    let g = c.grid_map();
    let sheet = c.sheet();
    // First placement: the grid point nearest to the view centre.
    done(&mut shell, "cursor.right", json!({}));
    let first = shell.canvas().unwrap().cursor().unwrap();
    assert_eq!(first, g.nearest(centre, sheet));
    // Grid steps under the QET sheet map: every position is a grid point of the overlay.
    let on_grid = |p: (i64, i64)| g.point(g.index(p.0, 0), 0) == p.0 && g.point(g.index(p.1, 1), 1) == p.1;
    let mut last = first;
    for (action, dx, dy) in [
        ("cursor.right", 1, 0),
        ("cursor.down", 0, 1),
        ("cursor.left", -1, 0),
        ("cursor.up", 0, -1),
    ] {
        done(&mut shell, action, json!({}));
        let p = shell.canvas().unwrap().cursor().unwrap();
        assert!(on_grid(p), "{p:?}");
        assert_eq!(g.index(p.0, 0) - g.index(last.0, 0), dx);
        assert_eq!(g.index(p.1, 1) - g.index(last.1, 1), dy);
        last = p;
    }
    // Cell steps: one column width, then the nearest grid point.
    let pitch = shell.canvas().unwrap().page().cells.as_ref().unwrap().pitch_nm();
    done(&mut shell, "cursor.cell_right", json!({}));
    let p = shell.canvas().unwrap().cursor().unwrap();
    assert_eq!(p, g.nearest((last.0 + pitch.0, last.1), sheet));
    assert!(on_grid(p));
    // cursor.set and its hiding.
    done(&mut shell, "cursor.set", json!({"x_mm": 255.0, "y_mm": 172.5}));
    let p = shell.canvas().unwrap().cursor().unwrap();
    assert_eq!(p, g.nearest((255_000_000, 172_500_000), sheet));
    assert_eq!(
        shell.canvas().unwrap().page().cell_text(p).as_deref(),
        Some("102-H15")
    );
    let s = state(&shell);
    assert_eq!(s["canvas"]["cursor_nm"], json!([p.0, p.1]));
    assert_eq!(s["canvas"]["cell"], "102-H15");
    assert!(
        shell
            .status_fields(false)
            .iter()
            .any(|f| f.starts_with("102-H15 · x "))
    );
    done(&mut shell, "cursor.set", json!({}));
    assert_eq!(shell.canvas().unwrap().cursor(), None);
    // Auto-pan: zoomed in, a cursor run to the right pans by the least amount, never zooms.
    done(&mut shell, "view.zoom_to", json!({"percent": 800}));
    let zoom = shell.canvas().unwrap().zoom();
    done(&mut shell, "cursor.right", json!({}));
    for _ in 0..80 {
        done(&mut shell, "cursor.right", json!({}));
        let c = shell.canvas().unwrap();
        let k = c.points_per_nm();
        let sx = (c.cursor().unwrap().0 - c.centre().0) as f64 * k + c.pane().0 / 2.0;
        assert!(sx <= c.pane().0 * (1.0 - AUTO_PAN_INSET) + 1.0, "{sx}");
        assert_eq!(c.zoom(), zoom);
    }
    let c = shell.canvas().unwrap();
    let sx = (c.cursor().unwrap().0 - c.centre().0) as f64 * c.points_per_nm() + c.pane().0 / 2.0;
    assert!(
        (sx - c.pane().0 * (1.0 - AUTO_PAN_INSET)).abs() < 1.0,
        "the least pan puts it at the inset: {sx}"
    );
    // An arrival clears it; a jump to a cell places it on the grid point nearest the cell's centre.
    done(&mut shell, "page.next", json!({}));
    assert_eq!(shell.canvas().unwrap().cursor(), None);
    done(&mut shell, "nav.goto", json!({"text": "103-A1"}));
    let c = shell.canvas().unwrap();
    let p = c.cursor().unwrap();
    assert!(on_grid(p));
    assert_eq!(c.page().cell_text(p).as_deref(), Some("103-A1"));
    // The link and the entity at the cursor: on the "106-E1" text of folio 103.
    let at = link_to(&shell, "106-E1");
    done(
        &mut shell,
        "cursor.set",
        json!({"x_mm": mm(at.0), "y_mm": mm(at.1)}),
    );
    let c = shell.canvas().unwrap();
    assert!(
        c.cursor_link().is_some(),
        "the link at the cursor (its inflated rectangle holds the grid point)"
    );
    assert!(c.cursor_entity().is_some());
    let node = ew_viewmodel::status::canvas_node(&shell, 0).unwrap();
    assert!(node.description.contains(", cursor 103-"), "{}", node.description);
    assert!(node.live.contains(", link to 106-E1"), "{}", node.live);
    // Without a cursor the live region stays, empty; the description says nothing of a cursor.
    done(&mut shell, "cursor.set", json!({}));
    let node = ew_viewmodel::status::canvas_node(&shell, 0).unwrap();
    assert!(node.live.is_empty() && !node.description.contains("cursor"));
    // On a page without cells the description has no cell and no double space.
    let generated = shell
        .page_rows()
        .iter()
        .find(|r| r.generated)
        .unwrap()
        .folio
        .clone();
    done(&mut shell, "page.goto", json!({ "folio": generated }));
    done(&mut shell, "cursor.right", json!({}));
    let node = ew_viewmodel::status::canvas_node(&shell, 0).unwrap();
    assert!(node.description.contains(", cursor x "), "{}", node.description);
    assert!(!node.description.contains("  "));
    shell.shutdown();
}

#[test]
fn v6_follow_back_forward_and_links_at_the_model() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "view.zoom_to", json!({"percent": 200}));
    done(&mut shell, "view.pan", json!({"dx": 40, "dy": -25}));
    let before = view_nm(&shell);
    let at = link_to(&shell, "103-A1");
    done(
        &mut shell,
        "canvas.pointer",
        json!({"x_mm": mm(at.0), "y_mm": mm(at.1)}),
    );
    let s = state(&shell);
    assert_eq!(s["canvas"]["link"]["folio"], "103");
    assert_eq!(s["status"][0], "Click or F: go to 103-A1");
    assert!(shell.describe().contains("canvas link \"103-A1\" folio \"103\""));
    done(&mut shell, "nav.follow", json!({}));
    let c = shell.canvas().unwrap();
    assert_eq!(c.page().folio, "103");
    let target = c.cursor().unwrap();
    assert_eq!(c.page().cell_text(target).as_deref(), Some("103-A1"));
    assert_eq!(c.centre(), target, "the target at the centre");
    done(&mut shell, "nav.back", json!({}));
    assert_eq!((shell.current_folio(), view_nm(&shell)), (Some("102"), before));
    done(&mut shell, "nav.forward", json!({}));
    assert_eq!(shell.current_folio(), Some("103"));
    // Next and previous link walk the reading order and wrap.
    done(&mut shell, "cursor.set", json!({}));
    let n = shell.canvas().unwrap().page().links.len();
    let mut seen = BTreeSet::new();
    for _ in 0..n {
        done(&mut shell, "nav.next_link", json!({}));
        seen.insert(shell.canvas().unwrap().cursor().unwrap());
    }
    done(&mut shell, "nav.next_link", json!({}));
    let wrapped = shell.canvas().unwrap().cursor().unwrap();
    assert!(seen.contains(&wrapped));
    done(&mut shell, "nav.previous_link", json!({}));
    done(&mut shell, "nav.next_link", json!({}));
    assert_eq!(shell.canvas().unwrap().cursor(), Some(wrapped));
    // No link: refused.
    done(&mut shell, "cursor.set", json!({"x_mm": 20, "y_mm": 20}));
    assert_eq!(refused(&mut shell, "nav.follow", json!({})), "reason-no-link");
    // Follow at a point: 102-A1 from 101-H16.
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    let at = link_to(&shell, "102-A1");
    done(
        &mut shell,
        "nav.follow",
        json!({"x_mm": mm(at.0), "y_mm": mm(at.1)}),
    );
    assert_eq!(shell.current_folio(), Some("102"));
    shell.shutdown();
}

#[test]
fn v7_go_to_and_its_errors_at_the_model() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    let fail = |shell: &mut ShellModel, text: &str| match shell.dispatch("nav.goto", &json!({"text": text})) {
        Outcome::Failed { message } => message,
        other => panic!("{other:?}"),
    };
    assert_eq!(fail(&mut shell, " "), "Type a folio or a cell.");
    assert_eq!(fail(&mut shell, "999-A1"), "No page or cell matches \"999-A1\".");
    assert_eq!(fail(&mut shell, "103-Z9"), "Page 103 has no cell Z9.");
    assert!(matches!(
        shell.dispatch("nav.goto", &json!({})),
        Outcome::Failed { .. }
    ));
    done(&mut shell, "nav.goto", json!({"text": "103"}));
    assert!(shell.canvas().unwrap().is_fitted());
    assert_eq!(
        shell.session_calls().resolve_goto,
        4,
        "every text read once; a missing text reads none"
    );
    shell.shutdown();
}

#[test]
fn v9_the_transcript_members_and_describe_lines() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    let text = script::state_json(&shell);
    let order = ["\"canvas\"", "\"tabs\"", "\"focus\"", "\"navigator\""];
    let at: Vec<usize> = order.iter().map(|k| text.find(k).unwrap()).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{text}");
    let canvas = &text[text.find("\"canvas\"").unwrap()..text.find("\"tabs\"").unwrap()];
    let order = ["\"renderer\"", "\"cursor_nm\"", "\"cell\"", "\"link\""];
    let at: Vec<usize> = order.iter().map(|k| canvas.find(k).unwrap()).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{canvas}");
    let s = state(&shell);
    assert_eq!(
        s["tabs"],
        json!({"focused": [0, 0], "groups": [[{"id": 1, "folio": "1", "back": 0, "forward": 0}]]})
    );
    assert_eq!(s["focus"], "canvas");
    assert_eq!(s["navigator"]["mode"], "section");
    assert_eq!(s["canvas"]["cursor_nm"], Value::Null);
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "view.side_by_side", json!({}));
    done(&mut shell, "ui.focus", json!({"area": "navigator"}));
    let d = shell.describe();
    assert!(
        d.contains("navigator \"Pages\" mode section sort folio filter \"\""),
        "{d}"
    );
    assert!(d.contains("  row level 1 \"Front matter\" expanded"), "{d}");
    assert!(d.contains("  row level 2 page \"1\" "), "{d}");
    assert!(d.contains("tabs group 1 of 2\n  tab \"102\" "), "{d}");
    assert!(d.contains(" selected history back 1 forward 0"), "{d}");
    assert!(d.contains("tabs group 2 of 2 focused\n  tab \"103\" "), "{d}");
    assert!(d.contains("\nfocus navigator\n"), "{d}");
    let lines: Vec<&str> = d.lines().collect();
    let tabs = lines.iter().position(|l| l.starts_with("tabs group 1")).unwrap();
    let pv = lines.iter().position(|l| l.starts_with("page view: ")).unwrap();
    let nav = lines.iter().position(|l| l.starts_with("navigator ")).unwrap();
    assert!(nav < tabs && tabs < pv);
    assert!(!d.contains("page list"));
    assert_eq!(state(&shell)["focus"], "navigator");
    // F6 cycles navigator, group 1, group 2 and the property grid (WP-S2.05: in the Schematic workspace), then the
    // command bar (WP-S2.17 part 2, doc 16a §5.9: first in the ring).
    done(&mut shell, "ui.focus_next", json!({}));
    assert_eq!(shell.focus(), Some(Focus::Canvas(0)));
    done(&mut shell, "ui.focus_next", json!({}));
    assert_eq!(shell.focus(), Some(Focus::Canvas(1)));
    done(&mut shell, "ui.focus_next", json!({}));
    assert_eq!(shell.focus(), Some(Focus::Properties));
    done(&mut shell, "ui.focus_next", json!({}));
    assert_eq!(shell.focus(), Some(Focus::CommandBar));
    assert_eq!(state(&shell)["focus"], "commandbar");
    done(&mut shell, "ui.focus_next", json!({}));
    assert_eq!(
        shell.focus(),
        Some(Focus::Navigator(ew_viewmodel::navigators::NavKind::Pages))
    );
    done(&mut shell, "ui.focus_previous", json!({}));
    assert_eq!(shell.focus(), Some(Focus::CommandBar));
    done(&mut shell, "ui.focus_previous", json!({}));
    assert_eq!(shell.focus(), Some(Focus::Properties));
    done(&mut shell, "ui.focus_previous", json!({}));
    assert_eq!(shell.focus(), Some(Focus::Canvas(1)));
    // The Window menu lists the focused group's tabs.
    let window = ew_viewmodel::actions::menu_model(&shell)
        .into_iter()
        .find(|m| m.menu == ew_viewmodel::actions::Menu::Window)
        .unwrap();
    let tabs: Vec<_> = window
        .entries
        .iter()
        .filter_map(|e| match e {
            ew_viewmodel::actions::MenuEntry::Item(i) if i.id == "tab.activate" => Some(i.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(tabs.len(), 1);
    assert!(tabs[0].checked && tabs[0].label.starts_with("103 "));
    // No project: null members.
    done(&mut shell, "file.close", json!({}));
    let s = state(&shell);
    assert_eq!(
        (s["tabs"].clone(), s["focus"].clone(), s["navigator"].clone()),
        (Value::Null, Value::Null, Value::Null)
    );
    shell.shutdown();
}

#[test]
fn v10_exact_counts_and_the_page_content_cache() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    // Two tabs (102, 103) in group 1, 104 in group 2; every page shown once.
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "tab.new", json!({}));
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    done(&mut shell, "view.side_by_side", json!({}));
    done(&mut shell, "tab.focus_other", json!({}));
    let calls = shell.session_calls();
    assert!(
        calls.page_content > 0 && calls.page_cells > 0 && calls.link_targets > 0,
        "{calls:?}"
    );
    let memo = shell.memo_totals().unwrap();
    let links_102_103 = {
        let c = shell.canvas().unwrap();
        c.page().links.len()
    };
    assert!(links_102_103 > 0);
    // 200 mixed steps between cached pages at one revision.
    for i in 0..200 {
        let step = match i % 20 {
            0 => ("tab.next", json!({})),
            1 => ("canvas.pointer", json!({"x_mm": 100 + i % 7, "y_mm": 80})),
            2 => ("cursor.right", json!({})),
            3 => ("cursor.cell_down", json!({})),
            4 => ("nav.next_link", json!({})),
            5 => ("tab.focus_other", json!({})),
            6 => ("canvas.pointer", json!({"x_mm": 120, "y_mm": 90, "group": 1})),
            7 => ("view.zoom_to", json!({"percent": 150 + i % 3})),
            8 => ("tab.focus_other", json!({})),
            9 => ("view.pan", json!({"dx": 3, "dy": -2})),
            10 => ("tab.previous", json!({})),
            11 => ("cursor.up", json!({})),
            12 => ("nav.previous_link", json!({})),
            13 => ("view.side_by_side", json!({})),
            14 => ("view.side_by_side", json!({})),
            15 => ("canvas.pointer", json!({})),
            16 => ("view.fit_page", json!({})),
            17 => ("cursor.set", json!({})),
            18 => ("ui.focus_next", json!({})),
            _ => ("tab.activate", json!({"group": 0, "index": 0})),
        };
        let out = shell.dispatch(step.0, &step.1);
        assert!(
            matches!(out, Outcome::Done | Outcome::Refused { .. }),
            "{step:?}: {out:?}"
        );
    }
    let after = shell.session_calls();
    // Every page the steps show (102, 103, and 106 or 103 beside them) was shown before the loop: not one call.
    assert_eq!(
        (
            after.page_content,
            after.page_cells,
            after.link_targets,
            after.page_rows
        ),
        (
            calls.page_content,
            calls.page_cells,
            calls.link_targets,
            calls.page_rows
        ),
        "{after:?} {calls:?}"
    );
    let shown: BTreeSet<String> = shell.cached_pages().into_iter().collect();
    assert!(shown.is_superset(&["102".to_owned(), "103".to_owned()].into_iter().collect()));
    let memo_after = shell.memo_totals().unwrap();
    assert_eq!(memo_after.1, memo.1, "no memo re-execution");
    // Back and forward and tab switches between cached pages make no Session call.
    let before = shell.session_calls();
    done(&mut shell, "tab.activate", json!({"group": 0, "index": 0}));
    done(&mut shell, "nav.goto", json!({"text": "103-A1"}));
    done(&mut shell, "nav.back", json!({}));
    done(&mut shell, "nav.forward", json!({}));
    done(&mut shell, "tab.next", json!({}));
    let mid = shell.session_calls();
    assert_eq!(mid.page_content, before.page_content);
    assert_eq!(mid.link_targets, before.link_targets);
    // The cache keeps the tabs' pages plus the 8 most recently shown others.
    for folio in [
        "1", "2", "3", "100", "101", "106", "200", "201", "202", "203", "204",
    ] {
        done(&mut shell, "page.goto", json!({"folio": folio}));
    }
    let cached = shell.cached_pages();
    let tab_pages: BTreeSet<String> = shell
        .area()
        .unwrap()
        .tabs()
        .filter_map(|(_, _, t)| shell.tab_row(t).map(|r| r.folio.clone()))
        .collect();
    assert!(tab_pages.iter().all(|f| cached.contains(f)));
    assert_eq!(
        cached.len(),
        tab_pages.len() + RECENT_PAGES,
        "{cached:?} {tab_pages:?}"
    );
    assert!(!cached.contains(&"1".to_owned()), "the least recently shown went");
    // Back to a page evicted from the cache makes exactly one page_content call.
    // Each step back costs one call exactly when its page is not cached, none when it is.
    let mut misses = 0;
    for _ in 0..10 {
        let cached = shell.cached_pages();
        let before = shell.session_calls().page_content;
        done(&mut shell, "nav.back", json!({}));
        let folio = shell.current_folio().unwrap().to_owned();
        let miss = u64::from(!cached.contains(&folio));
        assert_eq!(
            shell.session_calls().page_content - before,
            miss,
            "back to {folio}"
        );
        misses += miss;
    }
    assert_eq!(shell.current_folio(), Some("1"));
    assert!(misses >= 1, "the walk back reached an evicted page");
    shell.shutdown();
}

#[test]
fn v10_a_revision_change_empties_the_cache() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(true);
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    assert!(shell.cached_pages().contains(&"102".to_owned()));
    let xref = json!({"key":"qet-style","ip":"{page}-{row}{column}","aux_to_main":"[{page}-{row}{column}]","main_to_aux":"∞ {list}","structure":"when_different","same_page":"full","matrix":{"per_row":3,"max":9,"overflow":"…"}});
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "project.settings.set", "params": {"member": "xref_format", "value": xref}}),
    );
    assert_eq!(
        shell.cached_pages(),
        ["103"],
        "only the page on show, rebuilt at the new revision"
    );
    assert_eq!(shell.canvas().unwrap().page().rev, shell.revision().unwrap());
    done(&mut shell, "app.quit", json!({"discard": true}));
}

#[test]
fn a_tab_moved_across_the_groups_takes_its_new_groups_pane() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 900, "height": 700}),
    );
    done(&mut shell, "view.side_by_side", json!({}));
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 600, "height": 400, "group": 1}),
    );
    assert_eq!(shell.canvas_of(1).unwrap().pane(), (600.0, 400.0));
    // tab.move: the moved tab is active in the left group, with the left group's pane.
    done(
        &mut shell,
        "tab.move",
        json!({"group": 1, "index": 0, "to_group": 0, "to_index": 1}),
    );
    assert_eq!(shell.area().unwrap().groups.len(), 1);
    assert_eq!(shell.canvas_of(0).unwrap().pane(), (900.0, 700.0));
    // Side by side off: the right group's tab joins the left group and takes its pane when shown.
    done(&mut shell, "view.side_by_side", json!({}));
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 500, "height": 300, "group": 1}),
    );
    done(&mut shell, "view.side_by_side", json!({}));
    let last = shell.area().unwrap().groups[0].tabs.len() - 1;
    done(&mut shell, "tab.activate", json!({"group": 0, "index": last}));
    assert_eq!(shell.canvas_of(0).unwrap().pane(), (900.0, 700.0));
    shell.shutdown();
}

/// FT-524, FT-538 (the `v3` sighting): a killed test process leaves its directory behind with the project's lock in
/// it; a later test process with the same PID used to adopt that directory (`create_dir_all`, and `Tmp::shell` skips
/// the copy when the project exists), open the copy read-only and have its edits refused. A test directory is now
/// always one this process created; the leftover is skipped and left alone.
#[test]
fn ft_524_a_leftover_of_a_killed_run_with_the_same_pid_is_not_adopted() {
    let parent = Tmp::new();
    // The killed run: its copy of Golden A open for editing, then gone without a shutdown (the lock stays).
    let killed = Tmp(fresh_dir::fresh_dir_in(&parent.0, "run", &AtomicU32::new(0)));
    Box::leak(Box::new(killed.shell(true)));
    assert!(killed.0.join("J-E-2416.ew").join(".~ew.lock").is_file());
    // Adopting the leftover (the old behaviour): the copy is read-only.
    assert!(
        !killed.shell(true).is_writable(),
        "a leftover lock makes the copy read-only"
    );
    // The next process starts its numbers again: the leftover's name is skipped, the new directory is empty and its
    // copy is writable.
    let fresh = Tmp(fresh_dir::fresh_dir_in(&parent.0, "run", &AtomicU32::new(0)));
    assert_ne!(fresh.0, killed.0);
    assert_eq!(std::fs::read_dir(&fresh.0).unwrap().count(), 0);
    assert!(fresh.shell(true).is_writable());
    assert!(
        killed.0.join("J-E-2416.ew").join(".~ew.lock").is_file(),
        "the leftover is not touched"
    );
}
