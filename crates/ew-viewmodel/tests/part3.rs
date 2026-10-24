// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 3 in the view model (acceptance checks IB2 to IB6 and the view-model part of IB7): the IP tool's name
//! proposal, series, direction default and keys; a wire ending in an interruption point as one transaction and one
//! undo step; continue on another page (FR-XRF-014) and arrange by name (FR-XRF-013); the black box tool with a part's
//! connection points (FR-DEV-013) and the grips that reshape a box (FR-DEV-039); the picker's black-box rows; the
//! transcript members and describe lines with their words; the exact `Session` counts of the IP and black box tools.
//! Every test works on a writable temporary copy of Golden A.

mod common;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::ew_api::output::{PageTables, TableEnd, TableRep};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::tools::black_box::on_outline;
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`, at actual size.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

fn sheet(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    s.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The page point of grid index `k`.
fn grid_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// The grid index of a page point.
fn index_of(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.grid_index_of(p.0), sn.grid_index_of(p.1))
}

fn cursor(s: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(s, p);
    done(s, "cursor.set", at(q, json!({})));
}

fn enter_at(s: &mut ShellModel, p: (i64, i64)) {
    cursor(s, p);
    done(s, "canvas.activate", json!({}));
}

/// The grid cursor to grid index `k`.
fn cursor_k(s: &mut ShellModel, k: (i64, i64)) {
    let p = grid_point(s, k);
    cursor(s, p);
}

/// The grid cursor to grid index `k`, then Enter.
fn enter_k(s: &mut ShellModel, k: (i64, i64)) {
    let p = grid_point(s, k);
    enter_at(s, p);
}

fn canvas(s: &ShellModel) -> Value {
    state(s)["canvas"].clone()
}

fn describe_line(s: &ShellModel, prefix: &str) -> Option<String> {
    s.describe()
        .lines()
        .find(|l| l.starts_with(prefix))
        .map(str::to_owned)
}

/// The interruption point of the page with this name and direction.
fn ip(t: &PageTables, name: &str, direction: &str) -> TableRep {
    t.reps
        .iter()
        .find(|r| {
            r.ip.as_ref()
                .is_some_and(|i| i.name == name && i.direction == direction)
        })
        .unwrap_or_else(|| panic!("no IP {name} {direction}"))
        .clone()
}

/// The page's interruption points as (name, direction, position), sorted.
fn ips(t: &PageTables) -> Vec<(String, String, (i64, i64))> {
    let mut v: Vec<_> = t
        .reps
        .iter()
        .filter_map(|r| {
            r.ip.as_ref()
                .map(|i| (i.name.clone(), i.direction.clone(), r.pos))
        })
        .collect();
    v.sort();
    v
}

fn select_ids(s: &mut ShellModel, ids: &[String]) {
    done(s, "select.set", json!({ "ids": ids }));
}

/// Every connection of `t` at a CP of function `f`: (connection ID, the CP's index, the route's end at it).
fn ends_at(
    t: &PageTables,
    f: ew_viewmodel::ew_api::output::ew_dl::EntityRef,
) -> Vec<(String, u16, (i64, i64))> {
    let mut out = Vec::new();
    for c in &t.connections {
        for (e, end) in [(&c.a, c.polyline.first()), (&c.b, c.polyline.last())] {
            if let (TableEnd::Cp { function, idx }, Some(p)) = (e, end)
                && *function == f.0
            {
                out.push((c.id.to_string(), *idx, *p));
            }
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------------------------------------------------------
// IB2: the IP tool

#[test]
fn ib2_the_ip_tool_proposes_names_by_series_and_direction_and_d_and_star_toggle() {
    let tmp = Tmp::new("ip-tool");
    let mut s = shell(&tmp, "103");
    let before = ips(&tables(&s));
    done(&mut s, "tool.ip", json!({}));
    assert_eq!(s.tool().name(), "ip");
    assert_eq!(s.tool().state(), "floating");
    // A point in the left quarter of 103 proposes `target`; a typed name stays as typed even where the four 24VDC
    // targets of 103 use it (fix round, item 4: a star's second target shares its name on purpose).
    let left = grid_point(&s, (20, 40));
    cursor(&mut s, left);
    done(&mut s, "ip.name", json!({"text": "24VDC-1"}));
    let ghost = canvas(&s)["ghost"].clone();
    assert_eq!(ghost["kind"], "ip");
    assert_eq!(ghost["direction"], "target");
    assert_eq!(ghost["name"], "24VDC-1", "{ghost}");
    assert_eq!(ghost["star"], false);
    assert_eq!(ghost["at_nm"], json!([left.0, left.1]));
    // In the right quarter it is a source, and no source of 103 has the name yet.
    let area = s.canvas().unwrap().page().sheet;
    let right = s
        .canvas()
        .unwrap()
        .page()
        .snapper
        .sheet_to_page((area.0 * 15 / 16, area.1 / 2));
    let right = grid_point(&s, index_of(&s, right));
    cursor(&mut s, right);
    let ghost = canvas(&s)["ghost"].clone();
    assert_eq!(ghost["direction"], "source");
    assert_eq!(ghost["name"], "24VDC-1");
    // D cycles source → target → undirected; the star toggles.
    done(&mut s, "tool.ip_direction", json!({}));
    assert_eq!(canvas(&s)["ghost"]["direction"], "target");
    assert_eq!(canvas(&s)["ghost"]["name"], "24VDC-1", "typed: no series");
    done(&mut s, "tool.ip_direction", json!({}));
    assert_eq!(canvas(&s)["ghost"]["direction"], "undirected");
    done(&mut s, "tool.star_source", json!({}));
    let line = describe_line(&s, "canvas ghost ip ").expect("the ghost line");
    assert!(
        line.starts_with("canvas ghost ip name \"24VDC-1\" direction undirected star on at "),
        "{line}"
    );
    assert!(line.ends_with(" mm"), "{line}");
    // The preview is the very `ip.place`; the commit makes one IP; the tool stays floating and remembers the name.
    let rev = s.revision();
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    let preview = s.preview_state().expect("a preview");
    assert_eq!(preview.name, "ip.place");
    assert!(preview.result.is_ok(), "{:?}", preview.result.as_ref().err());
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    assert_eq!(s.tool().name(), "ip");
    let after = ips(&tables(&s));
    assert_eq!(after.len(), before.len() + 1);
    let placed = ip(&tables(&s), "24VDC-1", "undirected");
    assert_eq!(placed.pos, right);
    // The direction chosen with D and the star are cleared after the commit; the next proposal at the same point is
    // a source again, and the remembered name 24VDC-1 is free among the sources.
    assert_eq!(canvas(&s)["ghost"]["direction"], "source");
    assert_eq!(canvas(&s)["ghost"]["star"], false);
    assert_eq!(canvas(&s)["ghost"]["name"], "24VDC-1");
    // In the left quarter the remembered name is a proposal: the four 24VDC targets of 103 use the series up to 4.
    cursor(&mut s, left);
    assert_eq!(canvas(&s)["ghost"]["direction"], "target");
    assert_eq!(canvas(&s)["ghost"]["name"], "24VDC-5");
    // Escape ends the tool without a change.
    let rev = s.revision();
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
    assert_eq!(s.revision(), rev);
    // The IP tool's keys need the IP tool.
    assert_eq!(
        refused(&mut s, "tool.ip_direction", json!({})),
        "reason-ip-tool-only"
    );
    assert_eq!(
        refused(&mut s, "ip.name", json!({"text": "X"})),
        "reason-ip-tool-only"
    );
}

#[test]
fn ib2_the_name_proposal_is_the_potential_under_the_ghost_cp() {
    let tmp = Tmp::new("ip-potential");
    let mut s = shell(&tmp, "102");
    let t = tables(&s);
    // A CP with a derived potential, the IP's CP one grid step above its insertion point (ew.gen.ip, A).
    let cp = t
        .cps
        .iter()
        .find(|c| c.potential.is_some())
        .expect("a CP with a potential")
        .clone();
    let potential = cp.potential.clone().unwrap();
    let k = index_of(&s, cp.at);
    done(&mut s, "tool.ip", json!({}));
    cursor_k(&mut s, (k.0, k.1 + 1));
    let name = canvas(&s)["ghost"]["name"].as_str().unwrap().to_owned();
    assert!(
        name == potential || name.starts_with(&format!("{potential}-")),
        "{name} for {potential}"
    );
    // Elsewhere (no CP under the ghost's CP) and nothing committed yet: `IP`.
    cursor_k(&mut s, (60, 3));
    assert_eq!(canvas(&s)["ghost"]["name"], "IP");
}

#[test]
fn ib2_ending_a_wire_in_an_ip_is_one_transaction_and_one_undo_step() {
    let tmp = Tmp::new("wire-ip");
    let mut s = shell(&tmp, "102");
    let before = tables(&s);
    let start = before
        .cps
        .iter()
        .find(|c| c.label == "-102F2:13")
        .expect("-102F2:13")
        .clone();
    assert!(start.connections.is_empty());
    let rev = s.revision();
    done(&mut s, "tool.wire", json!({}));
    enter_at(&mut s, start.at);
    assert_eq!(s.tool().state(), "routing");
    // Down from the CP by four grid steps, then P.
    let k = index_of(&s, start.at);
    let end = grid_point(&s, (k.0, k.1 + 4));
    cursor(&mut s, end);
    assert_eq!(canvas(&s)["route"]["end"], Value::Null, "an empty grid point");
    done(&mut s, "tool.ip", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(s.tool().name(), "wire");
    assert_eq!(s.tool().state(), "armed");
    let after = tables(&s);
    assert_eq!(after.reps.len(), before.reps.len() + 1);
    assert_eq!(after.connections.len(), before.connections.len() + 1);
    let new_ip = after
        .reps
        .iter()
        .find(|r| r.ip.is_some() && !before.reps.iter().any(|b| b.id == r.id))
        .expect("the new IP")
        .clone();
    // The IP's CP lies where the wire ended and faces it: the wire arrives going down, the CP leaves up (A).
    let ip_cp = after
        .cps
        .iter()
        .find(|c| c.rep == new_ip.id)
        .expect("the IP's CP")
        .clone();
    assert_eq!(ip_cp.at, end);
    assert_eq!(new_ip.orient, 0);
    let wire = after
        .connections
        .iter()
        .find(|c| !before.connections.iter().any(|b| b.id == c.id))
        .unwrap();
    assert!(wire.stored_route);
    assert_eq!(wire.polyline.first(), Some(&start.at));
    assert_eq!(wire.polyline.last(), Some(&ip_cp.at));
    // Named from the start's potential, in its series.
    let name = &new_ip.ip.as_ref().unwrap().name;
    if let Some(p) = &start.potential {
        assert!(name == p || name.starts_with(&format!("{p}-")), "{name} for {p}");
    }
    // One undo step removes both.
    done(&mut s, "edit.undo", json!({}));
    let undone = tables(&s);
    assert_eq!(undone.reps.len(), before.reps.len());
    assert_eq!(undone.connections.len(), before.connections.len());
    // P while armed (not routing) arms the IP tool; P while routing on a CP is refused.
    done(&mut s, "tool.wire", json!({}));
    enter_at(&mut s, start.at);
    let other = before
        .cps
        .iter()
        .find(|c| c.label == "-102F2:14")
        .unwrap()
        .clone();
    cursor(&mut s, other.at);
    assert_eq!(refused(&mut s, "tool.ip", json!({})), "reason-ip-end-grid");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.ip", json!({}));
    assert_eq!(s.tool().name(), "ip");
}

// ---------------------------------------------------------------------------------------------------------------
// IB3, IB4: continue and arrange

/// The links of the page on show whose target is on `folio`.
fn links_to(s: &ShellModel, folio: &str) -> usize {
    s.canvas()
        .unwrap()
        .page()
        .links
        .iter()
        .filter(|l| l.target.as_ref().is_some_and(|t| t.folio == folio))
        .count()
}

#[test]
fn ib3_continue_on_page_103_places_the_paired_ip_at_the_default_position() {
    let tmp = Tmp::new("continue");
    let mut s = shell(&tmp, "102");
    let paired = links_to(&s, "103");
    // Delete 24VDC-2's partner on 103.
    done(&mut s, "page.goto", json!({"folio": "103"}));
    let partner = ip(&tables(&s), "24VDC-2", "target");
    select_ids(&mut s, &[partner.id.to_string()]);
    done(&mut s, "edit.delete_selection", json!({}));
    assert!(
        !tables(&s)
            .reps
            .iter()
            .any(|r| r.ip.as_ref().is_some_and(|i| i.name == "24VDC-2"))
    );
    done(&mut s, "page.goto", json!({"folio": "102"}));
    assert_eq!(links_to(&s, "103"), paired - 1, "the pairing is gone");
    // Continue on page 103 from 24VDC-2 on 102.
    let original = ip(&tables(&s), "24VDC-2", "source");
    assert_eq!(
        refused(&mut s, "ip.continue_on", json!({"folio": "103"})),
        "reason-select-one-ip"
    );
    select_ids(&mut s, &[original.id.to_string()]);
    let rev = s.revision();
    done(&mut s, "ip.continue_on", json!({"folio": "103"}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    // The pairing derives: 102 links to 103 as before the deletion.
    assert_eq!(links_to(&s, "103"), paired);
    done(&mut s, "page.goto", json!({"folio": "103"}));
    let t = tables(&s);
    let continued = ip(&t, "24VDC-2", "target");
    // The default position: the same y; the original lies in the right half of 102, so x is the leftmost grid point
    // inside 103's drawing area.
    assert_eq!(continued.pos.1, original.pos.1);
    let area = ew_viewmodel::tools::ip::drawing_area(s.canvas().unwrap().page()).expect("the drawing area");
    let k = index_of(&s, (area[0], 0)).0;
    let leftmost = if grid_point(&s, (k, 0)).0 < area[0] {
        k + 1
    } else {
        k
    };
    assert_eq!(continued.pos.0, grid_point(&s, (leftmost, 0)).0);
    assert_eq!(continued.symbol, original.symbol);
}

/// The cross-reference text (`102-A1`) that the source IP `name` on 101 prints for its partner on 102: the target cell
/// of 101's link nearest to that IP. Returns to 102.
fn xref_of_partner(s: &mut ShellModel, name: &str) -> String {
    done(s, "page.goto", json!({"folio": "101"}));
    let rep = ip(&tables(s), name, "source");
    let at = sheet(s, rep.pos);
    let cell = s
        .canvas()
        .unwrap()
        .page()
        .links
        .iter()
        .filter(|l| l.target.as_ref().is_some_and(|t| t.folio == "102"))
        .min_by_key(|l| {
            let c = ew_viewmodel::canvas::rect_centre(l.rect);
            let (dx, dy) = (i128::from(c.0 - at.0), i128::from(c.1 - at.1));
            dx * dx + dy * dy
        })
        .and_then(|l| l.target.as_ref().and_then(|t| t.cell.clone()))
        .expect("the partner's link");
    done(s, "page.goto", json!({"folio": "102"}));
    cell
}

#[test]
fn ib4_arrange_by_name_puts_l1_in_the_upper_slot() {
    let tmp = Tmp::new("arrange");
    let mut s = shell(&tmp, "102");
    let t = tables(&s);
    let (l1, n1) = (ip(&t, "L-1", "target"), ip(&t, "N-1", "target"));
    assert!(l1.pos.1 < n1.pos.1, "on Golden A L-1 is above N-1");
    // N-1 moved above L-1 (four grid steps up: two above L-1) and into the next column, so that the two slots lie in
    // different cells and the partners' cross-reference texts on 101 tell them apart.
    let rev = s.revision();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "rep.move", "params": {"reps": [n1.id.to_string()], "steps": [10, -4]}}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let t = tables(&s);
    let (l1, n1) = (ip(&t, "L-1", "target"), ip(&t, "N-1", "target"));
    assert!(n1.pos.1 < l1.pos.1);
    let before = xref_of_partner(&mut s, "L-1");
    let n_before = xref_of_partner(&mut s, "N-1");
    assert!(before.starts_with("102-"), "{before}");
    assert_ne!(before, n_before, "two cells");
    let l1_fn = ew_viewmodel::ew_api::output::ew_dl::EntityRef(l1.functions[0]);
    // Fewer than two selected: refused.
    select_ids(&mut s, &[l1.id.to_string()]);
    assert_eq!(
        refused(&mut s, "ip.arrange_selected", json!({"by": "name"})),
        "reason-select-ips"
    );
    select_ids(&mut s, &[n1.id.to_string(), l1.id.to_string()]);
    done(&mut s, "ip.arrange_selected", json!({"by": "name"}));
    let t = tables(&s);
    let (l1b, n1b) = (ip(&t, "L-1", "target"), ip(&t, "N-1", "target"));
    assert_eq!(l1b.pos, n1.pos, "L-1 takes the upper slot");
    assert_eq!(n1b.pos, l1.pos);
    // The connections at L-1's CP follow it: each route's end is the CP's new point.
    let cp_at = t.cps.iter().find(|c| c.rep == l1b.id).unwrap().at;
    for (_, _, end) in ends_at(&t, l1_fn) {
        assert_eq!(end, cp_at);
    }
    // The partner L-1 on 101 prints the new place of L-1 on 102 (FR-XRF-013: the cross-reference follows).
    let after = xref_of_partner(&mut s, "L-1");
    assert_ne!(after, before, "the partner's cross-reference text changes");
    assert_eq!(after, n_before, "L-1's partner now names N-1's former cell");
    // By potential with the chain order: one more commit.
    let rev = s.revision();
    done(
        &mut s,
        "ip.arrange_selected",
        json!({"by": "potential", "chain_order": true}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1));
}

// ---------------------------------------------------------------------------------------------------------------
// IB5, IB6: the black box tool and the grips

#[test]
fn ib5_the_black_box_tool_with_the_mico_part_gives_ten_cps_that_report_its_device() {
    let tmp = Tmp::new("box-tool");
    let mut s = shell(&tmp, "102");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "page.create", "params": {
            "number": 150, "title": "Black box", "kind": "multi_line", "grid": {"num": 7_937_500, "den": 3},
        }}),
    );
    done(&mut s, "page.goto", json!({"folio": "150"}));
    done(&mut s, "view.zoom_actual", json!({}));
    // Insert ▸ Black box lists the black-box rows only.
    done(&mut s, "tool.black_box", json!({}));
    let picker = s.picker().expect("the picker");
    assert_eq!(picker.mode.word(), "box");
    assert!(!picker.rows.is_empty());
    assert!(
        describe_line(&s, "  row part \"MICO 4.10\"").is_some(),
        "{}",
        s.describe()
    );
    done(&mut s, "ui.dismiss", json!({}));
    // A row that is no black box is refused by the black box tool.
    assert_eq!(
        refused(&mut s, "tool.black_box", json!({"symbol": "ew.iec.mcb-1p@qet10"})),
        "reason-not-black-box"
    );
    done(
        &mut s,
        "tool.black_box",
        json!({"part": "MICO 4.10", "template": 0}),
    );
    assert_eq!(s.tool().name(), "black_box");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(
        canvas(&s)["ghost"],
        json!({"kind": "box", "outline": 0, "cps": 10})
    );
    let rev = s.revision();
    // A rectangle two grid steps wide grows until five CPs fit on top.
    enter_k(&mut s, (30, 30));
    assert_eq!(s.tool().state(), "outline");
    cursor_k(&mut s, (32, 36));
    assert_eq!(
        describe_line(&s, "canvas ghost box ").as_deref(),
        Some("canvas ghost box outline 4 cps 10")
    );
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(s.preview_state().unwrap().name, "box.black");
    assert!(s.preview_state().unwrap().result.is_ok());
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    assert_eq!(s.tool().state(), "armed", "sticky");
    let t = tables(&s);
    let rep = t
        .reps
        .iter()
        .find(|r| r.black_box.is_some())
        .expect("the box")
        .clone();
    let b = rep.black_box.as_ref().unwrap();
    assert_eq!(b.outline.len(), 4);
    assert_eq!(index_of(&s, b.outline[1]).0 - index_of(&s, b.outline[0]).0, 6);
    assert_eq!(b.cps.len(), 10);
    let device = rep.device.expect("a device");
    let _ = device;
    let cps: Vec<_> = t.cps.iter().filter(|c| c.rep == rep.id).collect();
    assert_eq!(
        cps.len(),
        10,
        "every CP of the part reports the box in the page tables"
    );
    let tag = cps[0].label.split_once(':').unwrap().0.to_owned();
    let mut designations: Vec<String> = cps
        .iter()
        .map(|c| {
            let (t, d) = c.label.split_once(':').unwrap();
            assert_eq!(t, tag, "one device");
            assert_eq!(c.function, rep.functions[0]);
            d.split('#').next().unwrap().to_owned()
        })
        .collect();
    designations.sort();
    assert_eq!(
        designations,
        [
            "13", "14", "24VDC", "24VDC", "GND", "GND", "Out1", "Out2", "Out3", "Out4"
        ]
    );
    for c in &b.cps {
        assert!(on_outline(&b.outline, c.1));
    }
    done(&mut s, "ui.dismiss", json!({}));
}

/// -102G1's table row.
fn g1(t: &PageTables) -> TableRep {
    let f = t
        .cps
        .iter()
        .find(|c| c.label.starts_with("-102G1:"))
        .expect("-102G1")
        .rep;
    t.reps.iter().find(|r| r.id == f).unwrap().clone()
}

/// Checks the widened -102G1 against its row before: two grid steps wider, 7 CPs on the outline, every connection at
/// its CPs still ending at them.
fn check_widened(s: &ShellModel, before: &TableRep, connected: usize) {
    let t = tables(s);
    let after = g1(&t);
    let (b0, b1) = (
        before.black_box.as_ref().unwrap(),
        after.black_box.as_ref().unwrap(),
    );
    assert_eq!(
        index_of(s, b1.outline[1]).0 - index_of(s, b0.outline[1]).0,
        2,
        "two grid steps wider"
    );
    assert_eq!(b1.outline[0], b0.outline[0]);
    assert_eq!(b1.cps.len(), 7);
    for c in &b1.cps {
        assert!(on_outline(&b1.outline, c.1), "CP {} at {:?}", c.0, c.1);
    }
    // The CPs of the top and bottom edges stay where they were (the re-projection's anchor, fix round item 3).
    for (idx, p, _) in &b0.cps {
        let q = b1.cps.iter().find(|c| c.0 == *idx).unwrap().1;
        assert_eq!(q, *p, "CP {idx} stays");
    }
    let f = ew_viewmodel::ew_api::output::ew_dl::EntityRef(after.functions[0]);
    let ends = ends_at(&t, f);
    assert_eq!(ends.len(), connected, "all connected");
    for (_, idx, end) in ends {
        let cp = b1.cps.iter().find(|c| c.0 == idx).unwrap();
        assert_eq!(end, cp.1, "the route of CP {idx} ends at it");
    }
}

#[test]
fn ib6_widening_g1_by_two_grid_steps_keeps_its_cps_on_the_outline_and_connected() {
    let tmp = Tmp::new("grips");
    let mut s = shell(&tmp, "102");
    let t = tables(&s);
    let before = g1(&t);
    let f = ew_viewmodel::ew_api::output::ew_dl::EntityRef(before.functions[0]);
    let connected = ends_at(&t, f).len();
    assert_eq!(before.black_box.as_ref().unwrap().cps.len(), 7);
    assert_eq!(connected, 7, "Golden A connects all seven CPs of -102G1");
    // The grips need exactly one black box selected.
    assert_eq!(
        refused(&mut s, "box.grip", json!({"grip": 5})),
        "reason-one-black-box"
    );
    select_ids(&mut s, &[before.id.to_string()]);
    // The selected box shows its 8 grips and 7 CPs.
    assert_eq!(s.tool_overlay().len(), 15);
    assert_eq!(canvas(&s)["overlay"], 15);
    // The keyboard form: the grid cursor two steps right of the right edge, grip 5 (edge v1v2).
    let b = before.black_box.as_ref().unwrap();
    let right = b.outline[1];
    let k = index_of(&s, right);
    cursor_k(&mut s, (k.0 + 2, k.1 + 1));
    let rev = s.revision();
    done(&mut s, "box.grip", json!({"grip": 5}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    check_widened(&s, &before, connected);
    // A diagonal-free rectangle refuses no grip; an unknown one is refused.
    assert_eq!(refused(&mut s, "box.grip", json!({"grip": 8})), "reason-no-grip");
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(g1(&tables(&s)).black_box, before.black_box);
    // The pointer form: press on the right edge's midpoint, move two steps right, release.
    select_ids(&mut s, &[before.id.to_string()]);
    let mid = ew_viewmodel::tools::black_box::grip_points(&b.outline)[5];
    let from = sheet(&s, mid);
    let to = sheet(&s, grid_point(&s, (k.0 + 2, index_of(&s, mid).1)));
    done(&mut s, "canvas.press", at(from, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    done(&mut s, "canvas.pointer", at(to, json!({})));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(s.preview_state().unwrap().name, "box.reshape");
    let rev = s.revision();
    done(&mut s, "canvas.release", at(to, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    check_widened(&s, &before, connected);
}

#[test]
fn a_cp_is_added_on_the_outline_and_dragged_along_it() {
    let tmp = Tmp::new("box-cp");
    let mut s = shell(&tmp, "102");
    let before = g1(&tables(&s));
    let b = before.black_box.clone().unwrap();
    select_ids(&mut s, &[before.id.to_string()]);
    // Off the outline: refused.
    let k0 = index_of(&s, b.outline[0]);
    cursor_k(&mut s, (k0.0 + 1, k0.1 + 1));
    assert_eq!(refused(&mut s, "box.add_cp", json!({})), "reason-cp-off-outline");
    // On the left edge: a CP with the next free index (7), leaving left.
    let p = grid_point(&s, (k0.0, k0.1 + 2));
    cursor(&mut s, p);
    done(&mut s, "box.add_cp", json!({}));
    let added = g1(&tables(&s)).black_box.unwrap();
    assert_eq!(added.cps.len(), 8);
    let new = added.cps.iter().find(|c| c.0 == 7).expect("CP 7");
    assert_eq!(new.1, p);
    assert_eq!(new.2, ew_viewmodel::ew_api::geometry::Dir::Left);
    // Dragged one step down along the edge.
    let q = grid_point(&s, (k0.0, k0.1 + 3));
    let (sp, sq) = (sheet(&s, p), sheet(&s, q));
    done(&mut s, "canvas.press", at(sp, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    done(&mut s, "canvas.pointer", at(sq, json!({})));
    done(&mut s, "canvas.release", at(sq, json!({})));
    let moved = g1(&tables(&s)).black_box.unwrap();
    assert_eq!(moved.cps.iter().find(|c| c.0 == 7).unwrap().1, q);
    assert_eq!(moved.outline, added.outline);
}

// ---------------------------------------------------------------------------------------------------------------
// IB7: the picker, the transcript, the words and the counts

#[test]
fn the_pickers_black_box_rows_arm_the_black_box_tool() {
    let tmp = Tmp::new("box-picker");
    let mut s = shell(&tmp, "102");
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "picker.filter", json!({"text": "blackbox"}));
    let row = s
        .picker()
        .unwrap()
        .rows
        .iter()
        .position(|r| matches!(r, ew_viewmodel::tools::PickerRow::Symbol(_)))
        .expect("the black-box symbol row");
    done(&mut s, "picker.row", json!({"to": row}));
    done(&mut s, "picker.choose", json!({}));
    assert_eq!(s.tool().name(), "black_box");
    assert!(s.picker().is_none());
    assert_eq!(
        canvas(&s)["ghost"]["cps"],
        0,
        "a symbol without a part has no CPs"
    );
}

/// The structural tokens of a describe line: quoted text, numbers, aliases and punctuation out.
fn structural_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut quoted = false;
    let mut cur = String::new();
    for ch in line.chars() {
        if ch == '"' {
            quoted = !quoted;
            cur.clear();
            continue;
        }
        if quoted {
            continue;
        }
        if ch.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter()
        .filter(|t| {
            !ew_viewmodel::alias::is_alias(t)
                && t.parse::<f64>().is_err()
                && !t.contains('.')
                && !matches!(t.as_str(), "(" | ")" | "[" | "]" | "," | "|" | "%")
        })
        .collect()
}

#[test]
fn ib7_the_describe_lines_use_the_ten_words_and_the_tools_read_nothing_while_moving() {
    let tmp = Tmp::new("words");
    let mut s = shell(&tmp, "103");
    let mut seen = std::collections::BTreeSet::new();
    let scan = |s: &ShellModel, seen: &mut std::collections::BTreeSet<String>| {
        for l in s.describe().lines().filter(|l| l.starts_with("canvas ")) {
            for t in structural_tokens(l) {
                assert!(
                    ew_viewmodel::describe::VOCABULARY.contains(&t.as_str()),
                    "`{t}` is no vocabulary word; line: {l}"
                );
                seen.insert(t);
            }
        }
    };
    done(&mut s, "tool.ip", json!({}));
    let calls = s.session_calls();
    for k in 0..6 {
        cursor_k(&mut s, (20 + k, 30));
    }
    for _ in 0..3 {
        done(&mut s, "tool.ip_direction", json!({}));
        scan(&s, &mut seen);
    }
    done(&mut s, "ip.name", json!({"text": "X-1"}));
    let after = s.session_calls();
    assert_eq!(after.page_content, calls.page_content, "§12 item 4");
    assert_eq!(after.page_tables, calls.page_tables);
    assert_eq!(after.symbol_shape, calls.symbol_shape);
    assert_eq!(after.preview_with, calls.preview_with);
    scan(&s, &mut seen);
    done(&mut s, "ui.dismiss", json!({}));
    done(
        &mut s,
        "tool.black_box",
        json!({"part": "MICO 4.10", "template": 0}),
    );
    enter_k(&mut s, (40, 40));
    let calls = s.session_calls();
    cursor_k(&mut s, (44, 44));
    cursor_k(&mut s, (45, 46));
    let after = s.session_calls();
    assert_eq!(after.page_content, calls.page_content);
    assert_eq!(after.page_tables, calls.page_tables);
    assert_eq!(after.symbol_shape, calls.symbol_shape);
    scan(&s, &mut seen);
    let words = ew_viewmodel::describe::IP_WORDS;
    assert_eq!(words.len(), 10);
    for w in words {
        assert!(ew_viewmodel::describe::VOCABULARY.contains(w));
    }
    // `arrange`, `by` and `potential` name the parameters of `ip.arrange_selected` (no canvas line prints them).
    let missing: Vec<&&str> = words
        .iter()
        .filter(|w| !seen.contains(**w) && !matches!(**w, "arrange" | "by" | "potential"))
        .collect();
    assert!(missing.is_empty(), "words no state reached: {missing:?}");
}

#[test]
fn ip_external_refuses_a_printed_text_two_references_share_and_takes_an_id() {
    // A location node of Golden A, read from its external references (the copy keeps every ID).
    let location = {
        let (session, _) =
            ew_viewmodel::ew_api::Session::open_read_only(std::path::Path::new(common::GOLDEN_A), "test")
                .unwrap();
        let q = session.query();
        let id = q
            .external_refs()
            .iter()
            .next()
            .map(|(_, x)| x.location.to_string())
            .expect("Golden A has external references");
        drop(q);
        id
    };
    let tmp = Tmp::new("ip-external");
    let mut s = shell(&tmp, "102");
    for _ in 0..2 {
        done(
            &mut s,
            "command.run",
            json!({"cmd": "external_ref.create", "params": {
                "location": location, "device": "TWIN-DEV", "connection": "X",
            }}),
        );
    }
    done(&mut s, "tool.ip", json!({}));
    assert_eq!(
        refused(&mut s, "ip.external", json!({"external": "TWIN-DEV:X"})),
        "reason-external-ambiguous"
    );
    let twins: Vec<String> = s
        .external_choices()
        .into_iter()
        .filter(|(_, label)| label == "TWIN-DEV:X")
        .map(|(id, _)| id)
        .collect();
    assert_eq!(twins.len(), 2, "{twins:?}");
    done(&mut s, "ip.external", json!({"external": twins[0]}));
    assert_eq!(canvas(&s)["ghost"]["external"], "TWIN-DEV:X");
    // The empty text is the cleared form.
    done(&mut s, "ip.external", json!({"external": ""}));
    assert_eq!(canvas(&s)["ghost"]["external"], Value::Null);
}
