// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the view model: the IP tool's name
//! proposal, series, direction default and keys; a wire ending in an interruption point as one transaction and one
//! undo step; continue on another page (FR-XRF-014) and arrange by name (FR-XRF-013); the black box tool with a part's
//! connection points (FR-DEV-013) and the grips that reshape a box (FR-DEV-039); the picker's black-box rows; the
//! transcript members and describe lines with their words; the exact `Session` counts of the IP and black box tools.
//! Every test works on a writable temporary copy of the sample.

use crate::common;

use common::{
    Tmp, actual_size, at, canvas, cursor, describe_line, done, grid_point, refused, select_ids, sheet, tables,
};
use ew_api::output::{PageTables, TableEnd, TableRep};
use ew_viewmodel::shell::{Outcome, ShellModel};
use ew_viewmodel::tools::black_box::on_outline;
use serde_json::{Value, json};

/// The grid index of a page point.
fn index_of(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.grid_index_of(p.0), sn.grid_index_of(p.1))
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

/// The interruption point of the page with this name and direction.
fn ip(t: &PageTables, name: &str, direction: &str) -> TableRep {
    t.reps
        .iter()
        .find(|r| {
            r.ip.as_ref().is_some_and(|i| {
                i.name == name && ew_api::output::ip_direction_word(&i.direction) == direction
            })
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
            r.ip.as_ref().map(|i| {
                (
                    i.name.clone(),
                    ew_api::output::ip_direction_word(&i.direction),
                    r.pos,
                )
            })
        })
        .collect();
    v.sort();
    v
}

/// Every connection of `t` at a CP of function `f`: (connection ID, the CP's index, the route's end at it).
fn ends_at(t: &PageTables, f: ew_dl::EntityRef) -> Vec<(String, u16, (i64, i64))> {
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
fn the_ip_tool_proposes_names_by_series_and_direction_and_d_and_star_toggle() {
    let tmp = Tmp::new("ip-tool");
    let mut s = actual_size(&tmp, "103");
    let before = ips(&tables(&s));
    done(&mut s, "tool.ip", json!({}));
    assert_eq!(s.tool().name(), "ip");
    assert_eq!(s.tool().state(), "floating");
    // A typed name stays as typed, and 24V has its source on 101 (its targets are on 102 and 104): the tool starts its
    // point as a target.
    let left = grid_point(&s, (20, 40));
    cursor(&mut s, left);
    done(&mut s, "ip.name", json!({"text": "24V"}));
    let ghost = canvas(&s)["ghost"].clone();
    assert_eq!(ghost["kind"], "ip");
    assert_eq!(ghost["direction"], "target");
    assert_eq!(ghost["name"], "24V", "{ghost}");
    assert_eq!(ghost["star"], false);
    assert_eq!(ghost["at_nm"], json!([left.0, left.1]));
    // In the right quarter too: the direction is the tool's, wherever the click lands (EW-CORE-052 is project-wide).
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
    assert_eq!(ghost["direction"], "target");
    assert_eq!(ghost["name"], "24V", "typed: no series");
    // The direction is chosen in words too: undirected; the star toggles, and walks the point to source (a star
    // source is a source point).
    done(&mut s, "tool.ip_direction", json!({"direction": "undirected"}));
    assert_eq!(canvas(&s)["ghost"]["direction"], "undirected");
    done(&mut s, "tool.star_source", json!({}));
    let line = describe_line(&s, "canvas ghost ip ").expect("the ghost line");
    assert!(
        line.starts_with("canvas ghost ip name \"24V\" direction source star on at "),
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
    let placed = ip(&tables(&s), "24V", "source");
    assert_eq!(placed.pos, right);
    // The direction chosen stays after the commit (no flip after a placement), and when the name is erased; the
    // star left it alone and is cleared. The typed name stays after its source; erased, the remembered name's series
    // goes on to its first name that no point of the project has (no point of the sample is 24V-1), in either quarter.

    assert_eq!(canvas(&s)["ghost"]["direction"], "undirected");
    assert_eq!(canvas(&s)["ghost"]["star"], false);
    assert_eq!(canvas(&s)["ghost"]["name"], "24V");
    done(&mut s, "ip.name", json!({"text": ""}));
    assert_eq!(canvas(&s)["ghost"]["name"], "24V-1");
    cursor(&mut s, left);
    assert_eq!(canvas(&s)["ghost"]["direction"], "undirected");
    assert_eq!(canvas(&s)["ghost"]["name"], "24V-1");
    // Esc ends the tool without a change.
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
fn the_name_proposal_is_the_potential_under_the_ghost_cp() {
    let tmp = Tmp::new("ip-potential");
    let mut s = actual_size(&tmp, "102");
    let t = tables(&s);
    // A CP with a derived potential, the source form's CP one grid step left of its insertion point, the arrow's tip.
    let cp = t
        .cps
        .iter()
        .find(|c| c.potential.is_some())
        .expect("a CP with a potential")
        .clone();
    let potential = cp.potential.clone().unwrap().name;
    let k = index_of(&s, cp.at);
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "tool.ip_direction", json!({"direction": "source"}));
    cursor_k(&mut s, (k.0 + 1, k.1));
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
fn ending_a_wire_in_an_ip_is_one_transaction_and_one_undo_step() {
    let tmp = Tmp::new("wire-ip");
    let mut s = actual_size(&tmp, "102");
    let before = tables(&s);
    let start = before
        .cps
        .iter()
        .find(|c| c.label == "-X1021:2/b")
        .expect("the free lower side of terminal -X1021:2")
        .clone();
    assert!(start.connections.is_empty());
    let rev = s.revision();
    done(&mut s, "tool.wire", json!({}));
    enter_at(&mut s, start.at);
    assert_eq!(s.tool().state(), "routing");
    // Down from the CP by six grid steps (clear of the routed wires), then P.
    let k = index_of(&s, start.at);
    let end = grid_point(&s, (k.0, k.1 + 6));
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
    if let Some(p) = start.potential.as_ref().map(|p| &p.name) {
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
        .find(|c| c.label == "-X1021:2/b")
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
fn continue_on_page_103_places_the_paired_ip_at_the_default_position() {
    let tmp = Tmp::new("continue");
    let mut s = actual_size(&tmp, "101");
    let paired = links_to(&s, "103");
    // Delete 1L1's partner on 103.
    done(&mut s, "page.goto", json!({"folio": "103"}));
    let partner = ip(&tables(&s), "1L1", "target");
    select_ids(&mut s, &[partner.id.to_string()]);
    done(&mut s, "edit.delete_selection", json!({}));
    assert!(
        !tables(&s)
            .reps
            .iter()
            .any(|r| r.ip.as_ref().is_some_and(|i| i.name == "1L1"))
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    assert_eq!(links_to(&s, "103"), paired - 1, "the pairing is gone");
    // Continue on page 103 from 1L1 on 101.
    let original = ip(&tables(&s), "1L1", "source");
    assert_eq!(
        refused(&mut s, "ip.continue_on", json!({"folio": "103"})),
        "reason-select-one-ip"
    );
    select_ids(&mut s, &[original.id.to_string()]);
    let rev = s.revision();
    done(&mut s, "ip.continue_on", json!({"folio": "103"}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    // The pairing derives: 101 links to 103 as before the deletion.
    assert_eq!(links_to(&s, "103"), paired);
    done(&mut s, "page.goto", json!({"folio": "103"}));
    let t = tables(&s);
    let continued = ip(&t, "1L1", "target");
    // The default position: the same y; the original lies in the right half of 101, so x is the leftmost grid point
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
    // The partner of a source takes the target form.
    assert_eq!(
        (original.symbol_key.as_str(), continued.symbol_key.as_str()),
        ("ew.gen.ip@qet10", "ew.gen.ip-target@qet10")
    );
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
        // The point's cross-reference text stands below its arrow (FR-XRF-011's slot).
        .filter(|l| l.target.as_ref().is_some_and(|t| t.folio == "102") && l.rect.min().y.0 >= at.1)
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
fn arrange_by_name_puts_0v_in_the_upper_slot() {
    let tmp = Tmp::new("arrange");
    let mut s = actual_size(&tmp, "102");
    let t = tables(&s);
    let (zero, v24) = (ip(&t, "0V", "target"), ip(&t, "24V", "target"));
    assert!(v24.pos.1 < zero.pos.1, "on the sample 24V is above 0V");
    // 24V moved ten grid steps down, still above 0V, so that the two slots lie in different cells and the partners'
    // cross-reference texts on 101 tell them apart.
    let rev = s.revision();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "rep.move", "params": {"reps": [v24.id.to_string()], "steps": [0, 10]}}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let t = tables(&s);
    let (zero, v24) = (ip(&t, "0V", "target"), ip(&t, "24V", "target"));
    assert!(v24.pos.1 < zero.pos.1);
    let before = xref_of_partner(&mut s, "0V");
    let n_before = xref_of_partner(&mut s, "24V");
    assert!(before.starts_with("102-"), "{before}");
    assert_ne!(before, n_before, "two cells");
    let zero_fn = ew_dl::EntityRef(zero.functions[0]);
    // Fewer than two selected: refused.
    select_ids(&mut s, &[zero.id.to_string()]);
    assert_eq!(
        refused(&mut s, "ip.arrange_selected", json!({"by": "name"})),
        "reason-select-ips"
    );
    select_ids(&mut s, &[v24.id.to_string(), zero.id.to_string()]);
    done(&mut s, "ip.arrange_selected", json!({"by": "name"}));
    let t = tables(&s);
    let (zero_b, v24_b) = (ip(&t, "0V", "target"), ip(&t, "24V", "target"));
    assert_eq!(zero_b.pos, v24.pos, "0V takes the upper slot");
    assert_eq!(v24_b.pos, zero.pos);
    // The connections at 0V's CP follow it: each route's end is the CP's new point.
    let cp_at = t.cps.iter().find(|c| c.rep == zero_b.id).unwrap().at;
    for (_, _, end) in ends_at(&t, zero_fn) {
        assert_eq!(end, cp_at);
    }
    // The partner 0V on 101 prints the new place of 0V on 102 (FR-XRF-013: the cross-reference follows).
    let after = xref_of_partner(&mut s, "0V");
    assert_ne!(after, before, "the partner's cross-reference text changes");
    assert_eq!(after, n_before, "0V's partner now names 24V's former cell");
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
fn the_black_box_tool_with_the_power_supply_part_gives_seven_cps_that_report_its_device() {
    let tmp = Tmp::new("box-tool");
    let mut s = actual_size(&tmp, "102");
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
        describe_line(&s, "  row part \"EW-GEN-PSU-1\"").is_some(),
        "{}",
        s.describe()
    );
    done(&mut s, "ui.dismiss", json!({}));
    // A row that is no black box is refused by the black box tool: in words, with today's menu path and the
    // Symbol tool's key from the keymap.
    let key =
        ew_viewmodel::keymap::display(&s.shortcut_text(ew_viewmodel::actions::find("tool.place").unwrap()));
    assert_eq!(
        s.dispatch("tool.black_box", &json!({"symbol": "ew.iec.mcb-1p@qet10"})),
        Outcome::Failed {
            message: format!("This is not a black box: place it with Draw \u{25b8} Symbol\u{2026} ({key})")
        }
    );
    done(
        &mut s,
        "tool.black_box",
        json!({"part": "EW-GEN-PSU-1", "template": 0}),
    );
    assert_eq!(s.tool().name(), "black_box");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(
        canvas(&s)["ghost"],
        json!({"kind": "box", "outline": 0, "cps": 7})
    );
    let rev = s.revision();
    // A rectangle two grid steps wide grows until the four outputs fit on the bottom edge.
    enter_k(&mut s, (30, 30));
    assert_eq!(s.tool().state(), "outline");
    cursor_k(&mut s, (32, 36));
    assert_eq!(
        describe_line(&s, "canvas ghost box ").as_deref(),
        Some("canvas ghost box outline 4 cps 7")
    );
    // The HUD says what the count counts, never a bare "· 7".
    let prompt = ew_viewmodel::command_bar::hud(&s).prompt;
    assert!(prompt.ends_with(" \u{b7} 7 connection points"), "{prompt}");
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
    assert_eq!(index_of(&s, b.outline[1]).0 - index_of(&s, b.outline[0]).0, 5);
    assert_eq!(b.cps.len(), 7);
    let device = rep.device.expect("a device");
    let _ = device;
    let cps: Vec<_> = t.cps.iter().filter(|c| c.rep == rep.id).collect();
    assert_eq!(
        cps.len(),
        7,
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
    assert_eq!(designations, ["+", "+", "-", "-", "L", "N", "PE"]);
    for c in &b.cps {
        assert!(on_outline(&b.outline, c.1));
    }
    done(&mut s, "ui.dismiss", json!({}));
}

/// The first corner's click shows at once (a marker at the corner while the box has no area, the hint names
/// the opposite corner), the box follows the pointer from that corner, the first Escape drops the corner and says that
/// a second one leaves the tool, and the second Escape leaves it for Select.
#[test]
fn the_black_box_shows_its_first_corner_and_escape_leaves_in_two_steps() {
    let tmp = Tmp::new("box-escape");
    let mut s = actual_size(&tmp, "102");
    done(
        &mut s,
        "tool.black_box",
        json!({"part": "EW-GEN-PSU-1", "template": 0}),
    );
    assert_eq!(s.tool().name(), "black_box");
    // The armed hint names both corners, and the HUD's prompt is that hint with the box's pin count.
    assert_eq!(
        s.tool_hint(),
        "Click the box's first corner, then its opposite corner; Esc ends"
    );
    assert_eq!(
        ew_viewmodel::command_bar::hud(&s).prompt,
        "Click the box's first corner, then its opposite corner; Esc ends \u{b7} 7 connection points"
    );
    cursor_k(&mut s, (30, 30));
    // A chip at the pointer says how the box is drawn, not only the bottom bar.
    assert_eq!(canvas(&s)["overlay"], 2, "the grid marker and its chip");
    assert_eq!(
        s.tool_overlay().labels().collect::<Vec<_>>(),
        ["Black box: click two corners"]
    );
    enter_k(&mut s, (30, 30));
    assert_eq!(s.tool().state(), "outline");
    // No area yet: the corner marker and the target's label, so the click visibly registered.
    // The corner is a grid cross ringed by a CP marker, visible at 100 % zoom.
    assert_eq!(
        canvas(&s)["overlay"],
        3,
        "the first corner's cross and ring, and the label"
    );
    assert_eq!(
        s.tool_hint(),
        "Click or Enter at the opposite corner; Esc starts again"
    );
    // The pointer is the opposite corner: the rectangle (one outline, seven CPs, the label) and no corner marker.
    cursor_k(&mut s, (32, 36));
    assert_eq!(canvas(&s)["overlay"], 9);

    assert_eq!(canvas(&s)["ghost"]["outline"], 4);
    // The first Escape drops the corner and keeps the tool.
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "black_box");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(
        s.tool_hint(),
        "Click or Enter at the first corner; Esc again leaves the tool"
    );
    // The second leaves the tool.
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
}

/// -101G1's table row.
fn g1(t: &PageTables) -> TableRep {
    let f = t
        .cps
        .iter()
        .find(|c| c.label.starts_with("-101G1:"))
        .expect("-101G1")
        .rep;
    t.reps.iter().find(|r| r.id == f).unwrap().clone()
}

/// Checks the widened -101G1 against its row before: two grid steps wider, 7 CPs on the outline, every connection at
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
    // The CPs of the top and bottom edges move as a group: their spacing kept, the group centred on the wider edge.
    let (left, right) = (index_of(s, b1.outline[0]).0, index_of(s, b1.outline[1]).0);
    for y in [b1.outline[0].1, b1.outline[2].1] {
        let xs = |cps: &[(u16, (i64, i64), ew_api::geometry::Dir)]| -> Vec<i64> {
            let mut v: Vec<(u16, i64)> = cps
                .iter()
                .filter(|c| c.1.1 == y)
                .map(|c| (c.0, index_of(s, c.1).0))
                .collect();
            v.sort_unstable();
            v.into_iter().map(|c| c.1).collect()
        };
        let (old, new) = (xs(&b0.cps), xs(&b1.cps));
        assert_eq!(old.len(), new.len());
        let gaps = |v: &[i64]| v.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
        assert_eq!(gaps(&old), gaps(&new), "the spacing kept");
        let (first, last) = (*new.iter().min().unwrap(), *new.iter().max().unwrap());
        assert!(
            ((first - left) - (right - last)).abs() <= 1,
            "centred: {new:?} in {left}..{right}"
        );
    }
    let f = ew_dl::EntityRef(after.functions[0]);
    let ends = ends_at(&t, f);
    assert_eq!(ends.len(), connected, "all connected");
    for (_, idx, end) in ends {
        let cp = b1.cps.iter().find(|c| c.0 == idx).unwrap();
        assert_eq!(end, cp.1, "the route of CP {idx} ends at it");
    }
}

#[test]
fn widening_g1_by_two_grid_steps_keeps_its_cps_on_the_outline_and_connected() {
    let tmp = Tmp::new("grips");
    let mut s = actual_size(&tmp, "101");
    let t = tables(&s);
    let before = g1(&t);
    let f = ew_dl::EntityRef(before.functions[0]);
    let connected = ends_at(&t, f).len();
    assert_eq!(before.black_box.as_ref().unwrap().cps.len(), 7);
    assert_eq!(connected, 7, "the sample connects all seven CPs of -101G1");
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

/// A box's right-click offers Centre the pins, which centres the pins of each side without a resize (the
/// owner's box stored with a pin on its corner is put right so), one undo step; the wires stay at their pins.
#[test]
fn centre_the_pins_from_a_boxs_right_click() {
    let tmp = Tmp::new("centre-pins");
    let mut s = actual_size(&tmp, "101");
    // The sample's box has its pins centred: its top side's last pin (2) is dragged towards the right corner first.
    let drawn = g1(&tables(&s));
    select_ids(&mut s, &[drawn.id.to_string()]);
    let b = drawn.black_box.as_ref().unwrap();
    let (k0, k1) = (index_of(&s, b.outline[0]), index_of(&s, b.outline[1]));
    let pin = b.cps.iter().find(|c| c.0 == 2).unwrap().1;
    let (sp, sq) = (sheet(&s, pin), sheet(&s, grid_point(&s, (k1.0 - 1, k0.1))));
    done(&mut s, "canvas.press", at(sp, json!({})));
    done(&mut s, "canvas.pointer", at(sq, json!({})));
    done(&mut s, "canvas.release", at(sq, json!({})));
    let before = g1(&tables(&s));
    assert_ne!(before.black_box, drawn.black_box, "the pin moved");
    select_ids(&mut s, &[before.id.to_string()]);
    let menu = ew_viewmodel::command_bar::canvas_menu(&s, ew_viewmodel::command_bar::MenuSpot::Object, None);
    let item = menu
        .iter()
        .find(|i| i.id == "box.centre_pins")
        .expect("Centre the pins");
    assert_eq!(item.label, "Centre the pins");
    assert!(item.enabled);
    let rev = s.revision();
    done(&mut s, "box.centre_pins", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let t = tables(&s);
    let after = g1(&t);
    let b = after.black_box.as_ref().unwrap();
    assert_eq!(
        b.outline,
        before.black_box.as_ref().unwrap().outline,
        "the size kept"
    );
    let (left, right) = (index_of(&s, b.outline[0]).0, index_of(&s, b.outline[1]).0);
    let top: Vec<i64> = b
        .cps
        .iter()
        .filter(|c| c.1.1 == b.outline[0].1)
        .map(|c| index_of(&s, c.1).0)
        .collect();
    let (first, last) = (*top.iter().min().unwrap(), *top.iter().max().unwrap());
    assert!(
        ((first - left) - (right - last)).abs() <= 1,
        "{top:?} in {left}..{right}"
    );
    let f = ew_dl::EntityRef(after.functions[0]);
    for (_, idx, end) in ends_at(&t, f) {
        assert_eq!(
            end,
            b.cps.iter().find(|c| c.0 == idx).unwrap().1,
            "the wire of CP {idx}"
        );
    }
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(g1(&tables(&s)).black_box, before.black_box);
}

#[test]
fn an_inside_out_grip_drag_is_refused_and_leaves_the_box_alone() {
    let tmp = Tmp::new("grip-inside-out");
    let mut s = actual_size(&tmp, "101");
    let before = g1(&tables(&s));
    select_ids(&mut s, &[before.id.to_string()]);
    let grips = s.tool_overlay().clone();
    assert!(!grips.is_empty(), "the selected box shows its grips");
    let b = before.black_box.as_ref().unwrap();
    // Grip 5 is the right edge's midpoint; one grid step inward is valid, past the left edge is not.
    let mid = ew_viewmodel::tools::black_box::grip_points(&b.outline)[5];
    let from = sheet(&s, mid);
    let k = index_of(&s, mid);
    let left = index_of(&s, b.outline[0]).0;
    let inward = sheet(&s, grid_point(&s, (k.0 - 1, k.1)));
    let to = sheet(&s, grid_point(&s, (left - 2, k.1)));
    done(&mut s, "canvas.press", at(from, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    done(&mut s, "canvas.pointer", at(inward, json!({})));
    assert!(!s.tool_overlay().is_empty(), "the valid reshape ghost");
    assert_ne!(s.tool_overlay(), &grips, "the ghost is not the grips");
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert!(s.preview_state().is_some(), "the valid dwell preview");
    assert_eq!(
        refused(&mut s, "canvas.pointer", at(to, json!({}))),
        "reason-box-inside-out"
    );
    assert!(
        s.tool_overlay().is_empty(),
        "the refused move clears the reshape ghost"
    );
    assert!(s.preview_state().is_none(), "and its kept preview");
    assert_eq!(
        refused(&mut s, "canvas.release", at(to, json!({}))),
        "reason-box-inside-out"
    );
    assert!(
        s.tool_overlay().is_empty() || s.tool_overlay() == &grips,
        "the refused release leaves the grips, not a reshape ghost"
    );
    assert_eq!(g1(&tables(&s)).black_box, before.black_box, "the box stays");
    assert_eq!(s.tool().state(), "idle");
    assert_eq!(
        s.status_fields(false)[0],
        "A grip cannot pass the opposite side of the black box"
    );
}

/// A box of a part draws the part's pins; Add pin opens the part in Parts, where they are arranged: the sample's starter
/// power supply as it is, with the line that Make my own from it is the way; the box is unchanged.
#[test]
fn add_pin_on_a_part_s_box_opens_the_part_in_parts() {
    let tmp = Tmp::new("box-cp");
    let mut s = actual_size(&tmp, "101");
    let before = g1(&tables(&s));
    assert!(!before.black_box.as_ref().unwrap().own_pins, "a box of a part");
    select_ids(&mut s, &[before.id.to_string()]);
    done(&mut s, "box.add_cp", json!({}));
    assert!(
        s.parts_place().and_then(|p| p.sheet.as_ref()).is_some(),
        "the part in Parts"
    );
    assert!(
        s.symbol_editor().is_none(),
        "a starter part's box is not arranged"
    );
    assert!(
        crate::common::toast(&s).starts_with("The pins of this box are its part's"),
        "{}",
        crate::common::toast(&s)
    );
    assert_eq!(g1(&tables(&s)).black_box, before.black_box, "the box stays");
}

/// The sheet point of the page point `p`, as the menu's `x_mm`/`y_mm`.
fn sheet_mm(s: &ShellModel, p: (i64, i64)) -> Value {
    let q = sheet(s, p);
    json!({"x_mm": q.0 as f64 / 1e6, "y_mm": q.1 as f64 / 1e6})
}

/// The names the page prints for the pins of the box `rep` (the label after its colon), in index order.
fn pin_names(s: &ShellModel, rep: ew_api::output::Uuid) -> Vec<String> {
    let t = tables(s);
    let mut cps: Vec<_> = t.cps.iter().filter(|c| c.rep == rep).collect();
    cps.sort_by_key(|c| c.idx);
    cps.iter()
        .map(|c| c.label.rsplit(':').next().unwrap_or_default().to_owned())
        .collect()
}

/// A box drawn without a part takes pins where it stands. Add pin
/// at a point puts a pin on the edge nearest it and asks its name; Tab takes it and asks for the next on the same side;
/// a double click on a pin renames it; Remove pin takes it away while no wire holds it; a wire drawn to the box's edge
/// where no pin is asks for a pin there and ends at it. Each is one undo step.
/// A new page 150 on the QET grid of the sample's copy with a box drawn without a part from grid (30, 30) to (42, 38),
/// selected.
fn plain_box(tmp: &Tmp) -> (ShellModel, TableRep) {
    let mut s = actual_size(tmp, "102");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "page.create", "params": {
            "number": 150, "title": "Box", "kind": "multi_line", "grid": {"num": 7_937_500, "den": 3},
        }}),
    );
    done(&mut s, "page.goto", json!({"folio": "150"}));
    done(&mut s, "view.zoom_actual", json!({}));
    done(
        &mut s,
        "tool.black_box",
        json!({"symbol": "ew.gen.blackbox@qet10", "letter": "A"}),
    );
    enter_k(&mut s, (30, 30));
    cursor_k(&mut s, (42, 38));
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "tool.select", json!({}));
    let rep = tables(&s)
        .reps
        .iter()
        .find(|r| r.black_box.is_some())
        .expect("the box")
        .clone();
    assert!(rep.black_box.as_ref().unwrap().own_pins);
    select_ids(&mut s, &[rep.id.to_string()]);
    (s, rep)
}

/// A re-test of a drawn box's pins, as the person did it:
/// - "24V Tab 24V Tab 0V Tab 0V Enter" gives four pins 24V, 24V, 0V, 0V: two pins may share a name, as a part's do;
/// - an empty name is said under the field, whole, and the field stays open to type on (no banner);
/// - Add pin stands on the Draw tab's Arrange group, and a box without a part offers no dead "Arrange its box…";
/// - Add pin without a point lets the new pin follow the pointer along the nearest edge, the status line says so, and
///   a click puts it there;
/// - the box's height stops where the texts of its top and bottom sides would touch, as its width does;
/// - a press a few points beside a corner's handle takes the handle (the pointer shows resize first), not the box.
#[test]
fn a_drawn_box_s_pins_as_the_re_test_used_them() {
    let tmp = Tmp::new("box-pins-retest");
    let (mut s, rep) = plain_box(&tmp);
    let near_top = grid_point(&s, (33, 31));
    let point = sheet_mm(&s, near_top);
    done(&mut s, "box.add_cp", point);
    for (i, name) in ["24V", "24V", "0V", "0V"].into_iter().enumerate() {
        done(&mut s, "text.input", json!({ "text": name }));
        let next = if i < 3 {
            "tool.next_proposal"
        } else {
            "tool.finish"
        };
        done(&mut s, next, json!({}));
    }
    // Two pins of one name, as a part's: the page prints them apart as 24V#1 and 24V#2.
    assert_eq!(stored_names(&s, rep.id), ["24V", "24V", "0V", "0V"]);
    assert_eq!(s.tool().name(), "select");
    // An empty name: the line under the field says why, the field stays.
    select_ids(&mut s, &[rep.id.to_string()]);
    let point = sheet_mm(&s, grid_point(&s, (36, 38)));
    done(&mut s, "box.add_cp", point);
    done(&mut s, "text.input", json!({ "text": " " }));
    done(&mut s, "tool.finish", json!({}));
    let popup = s.text_popup().expect("the field stays open");
    assert_eq!(popup.error.as_deref(), Some("Type a name for the pin"));
    done(&mut s, "text.input", json!({ "text": "PE" }));
    assert!(
        s.text_popup().is_some_and(|p| p.error.is_none()),
        "typing clears the line"
    );
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(stored_names(&s, rep.id), ["24V", "24V", "0V", "0V", "PE"]);
    // Add pin on the Draw tab; no dead entry in the box's menu.
    let arrange = ew_viewmodel::command_bar::placed_in(ew_viewmodel::command_bar::Group::Arrange);
    assert!(arrange.iter().any(|a| a.id == "box.add_cp"));
    select_ids(&mut s, &[rep.id.to_string()]);
    let menu = ew_viewmodel::command_bar::canvas_menu(&s, ew_viewmodel::command_bar::MenuSpot::Object, None);
    assert!(menu.iter().any(|i| i.label == "Add pin"), "{menu:?}");
    assert!(!menu.iter().any(|i| i.label == "Arrange its box…"), "{menu:?}");
    // Add pin from the bar: the pin follows the pointer along the nearest edge before the click.
    done(&mut s, "box.add_cp", json!({}));
    assert_eq!(s.tool().state(), "armed");
    assert!(
        s.status_fields(false)[0].contains("edge nearest the pointer"),
        "{:?}",
        s.status_fields(false)
    );
    let right = grid_point(&s, (43, 34));
    let on_edge = grid_point(&s, (42, 34));
    let pointer = sheet(&s, right);
    done(&mut s, "canvas.pointer", at(pointer, json!({})));
    assert!(
        s.tool_overlay().items.iter().any(|i| matches!(i,
            ew_viewmodel::tools::OverlayItem::Marker { at, .. } if *at == sheet(&s, on_edge))),
        "the ghost pin stands on the right edge"
    );
    done(&mut s, "canvas.press", at(pointer, json!({})));
    assert_eq!(s.tool().state(), "typing");
    done(&mut s, "text.input", json!({ "text": "SIG" }));
    done(&mut s, "tool.finish", json!({}));
    let b = g1_box(&s, rep.id);
    assert!(
        b.cps.iter().any(|c| c.1 == on_edge),
        "the pin stands where the ghost was"
    );
    // The height stops where the top's and the bottom's pin texts would touch.
    let depth = b.room.1;
    assert!(depth > 0);
    select_ids(&mut s, &[rep.id.to_string()]);
    // The bottom-right corner dragged up to one step below the top.
    let (from, to) = (sheet(&s, b.outline[2]), sheet(&s, grid_point(&s, (42, 31))));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    assert!(
        s.status_fields(false)[0].starts_with("The box goes no smaller"),
        "{:?}",
        s.status_fields(false)
    );
    done(&mut s, "canvas.release", at(to, json!({})));
    let b = g1_box(&s, rep.id);
    let ys = b.outline.iter().map(|p| index_of(&s, *p).1);
    assert!(
        ys.clone().max().unwrap() - ys.min().unwrap() >= depth,
        "{:?}",
        b.outline
    );
    // A press a few points beside the top-left corner's handle takes it: the pointer shows resize first.
    let corner = sheet(&s, b.outline[0]);
    let pts = s.canvas().unwrap().points_per_nm();
    let beside = (corner.0 - (5.0 / pts) as i64, corner.1);
    done(&mut s, "canvas.pointer", at(beside, json!({})));
    assert_eq!(
        s.box_grip_under_pointer(),
        Some(ew_viewmodel::tools::black_box::GripResize::CornerNwSe)
    );
}

#[test]
fn a_drawn_box_takes_pins_where_it_stands() {
    let tmp = Tmp::new("box-own-pins");
    let (mut s, rep) = plain_box(&tmp);
    // Add pin near the top edge: the pin goes on it, its name asked in place.
    let near_top = grid_point(&s, (33, 31));
    let point = sheet_mm(&s, near_top);
    done(&mut s, "box.add_cp", point);
    assert_eq!(s.tool().name(), "box_pin");
    assert!(
        s.text_popup().is_some_and(|p| p.tab_next),
        "the name is asked; Tab adds the next"
    );
    let rev = s.revision();
    done(&mut s, "text.input", json!({"text": "L1"}));
    done(&mut s, "tool.next_proposal", json!({}));
    assert_eq!(
        s.tool().name(),
        "box_pin",
        "Tab asks for the next pin of the side"
    );
    done(&mut s, "text.input", json!({"text": "L2"}));
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().name(), "select");
    assert_eq!(pin_names(&s, rep.id), ["L1", "L2"]);
    let b = g1_box(&s, rep.id);
    assert!(
        b.cps.iter().all(|c| index_of(&s, c.1).1 == 30),
        "both on the top edge: {:?}",
        b.cps
    );
    assert_eq!(s.revision(), rev.map(|r| r + 2), "one step each");
    // The first Ctrl+Z puts an open name field away and undoes nothing.
    select_ids(&mut s, &[rep.id.to_string()]);
    let point = sheet_mm(&s, grid_point(&s, (42, 34)));
    done(&mut s, "box.add_cp", point);
    let rev = s.revision();
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.tool().name(), "select");
    assert_eq!(
        (s.revision(), pin_names(&s, rep.id)),
        (rev, vec!["L1".to_owned(), "L2".to_owned()])
    );
    // A name with a space is said under the field; nothing changes.
    select_ids(&mut s, &[rep.id.to_string()]);
    let point = sheet_mm(&s, grid_point(&s, (42, 34)));
    done(&mut s, "box.add_cp", point);
    done(&mut s, "text.input", json!({"text": "A 1"}));
    done(&mut s, "tool.finish", json!({}));
    assert!(
        s.text_popup()
            .and_then(|p| p.error)
            .is_some_and(|e| e.contains("no spaces"))
    );
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
    assert_eq!(pin_names(&s, rep.id), ["L1", "L2"]);
    // A double click on L2 renames it.
    let l2 = g1_box(&s, rep.id).cps[1].1;
    select_ids(&mut s, &[rep.id.to_string()]);
    let pointer = sheet(&s, l2);
    done(&mut s, "canvas.pointer", at(pointer, json!({})));
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().name(), "box_pin");
    done(&mut s, "text.input", json!({"text": "N"}));
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(pin_names(&s, rep.id), ["L1", "N"]);
    // A wire from L1 to the bottom edge, where no pin is: the pin's name is asked, the wire ends at it.
    let l1 = g1_box(&s, rep.id).cps[0].1;
    done(&mut s, "tool.wire", json!({}));
    cursor(&mut s, l1);
    done(&mut s, "canvas.activate", json!({}));
    for k in [(33, 26), (28, 26), (28, 42), (36, 42)] {
        cursor_k(&mut s, k);
        done(&mut s, "canvas.activate", json!({}));
    }
    cursor_k(&mut s, (36, 38));
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.tool().name(), "box_pin", "the wire asks for a pin there");
    done(&mut s, "text.input", json!({"text": "PE"}));
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().name(), "wire", "back to the wire");
    assert_eq!(pin_names(&s, rep.id), ["L1", "N", "PE"]);
    let t = tables(&s);
    let pe = t
        .cps
        .iter()
        .find(|c| c.rep == rep.id && c.label.ends_with(":PE"))
        .unwrap();
    assert_eq!(pe.connections.len(), 1, "the wire ends at the new pin");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    let wires = t.connections.len();
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(
        pin_names(&s, rep.id),
        ["L1", "N"],
        "one undo takes the pin and its wire back"
    );
    assert_eq!(tables(&s).connections.len(), wires - 1);
    done(&mut s, "edit.redo", json!({}));
    let t = tables(&s);
    let pe = t
        .cps
        .iter()
        .find(|c| c.rep == rep.id && c.label.ends_with(":PE"))
        .unwrap();
    // Remove pin: refused while a wire holds it, done for a free one.
    select_ids(&mut s, &[rep.id.to_string()]);
    let pe_idx = pe.idx;
    assert_eq!(
        refused(&mut s, "box.remove_pin", json!({"idx": pe_idx})),
        "reason-box-pin-wired"
    );
    let n_idx = t
        .cps
        .iter()
        .find(|c| c.rep == rep.id && c.label.ends_with(":N"))
        .unwrap()
        .idx;
    done(&mut s, "box.remove_pin", json!({"idx": n_idx}));
    assert_eq!(pin_names(&s, rep.id), ["L1", "PE"]);
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(
        pin_names(&s, rep.id),
        ["L1", "N", "PE"],
        "one undo brings it back"
    );
}

/// The names the box `rep` stores for its pins, in index order.
fn stored_names(s: &ShellModel, rep: ew_api::output::Uuid) -> Vec<String> {
    let mut names = g1_box(s, rep).names;
    names.sort();
    names.into_iter().map(|(_, n)| n).collect()
}

/// The box `rep` of the page shown.
fn g1_box(s: &ShellModel, rep: ew_api::output::Uuid) -> ew_api::output::TableBox {
    tables(s)
        .reps
        .iter()
        .find(|r| r.id == rep)
        .and_then(|r| r.black_box.clone())
        .unwrap()
}

/// A pin dragged along its side towards a corner stays on its side, one step clear of the corner, on a point
/// no other pin holds; dragged onto a point another pin holds, it stays where it was.
#[test]
fn a_pin_dragged_towards_a_corner_stays_on_its_side() {
    let tmp = Tmp::new("box-cp-corner");
    let mut s = actual_size(&tmp, "101");
    let before = g1(&tables(&s));
    let b = before.black_box.clone().unwrap();
    select_ids(&mut s, &[before.id.to_string()]);
    let (k0, k1) = (index_of(&s, b.outline[0]), index_of(&s, b.outline[1]));
    // The top side's last pin (2, at step 5), dragged to half a step short of the right edge.
    let pin = b.cps.iter().find(|c| c.0 == 2).unwrap().1;
    let right = grid_point(&s, (k1.0, k0.1)).0;
    let half = (grid_point(&s, (1, 0)).0 - grid_point(&s, (0, 0)).0) / 2;
    let (sp, sq) = (sheet(&s, pin), sheet(&s, (right - half, pin.1)));
    done(&mut s, "canvas.press", at(sp, json!({})));
    done(&mut s, "canvas.pointer", at(sq, json!({})));
    done(&mut s, "canvas.release", at(sq, json!({})));
    let moved = g1(&tables(&s)).black_box.unwrap();
    let to = moved.cps.iter().find(|c| c.0 == 2).unwrap();
    assert_eq!(
        to.1,
        grid_point(&s, (k1.0 - 1, k0.1)),
        "on its side, a step from the corner"
    );
    assert_eq!(to.2, ew_api::geometry::Dir::Up);
    // Onto the point pin 1 holds: it stays.
    let other = moved.cps.iter().find(|c| c.0 == 1).unwrap().1;
    let (sp, sq) = (sheet(&s, to.1), sheet(&s, other));
    done(&mut s, "canvas.press", at(sp, json!({})));
    done(&mut s, "canvas.pointer", at(sq, json!({})));
    done(&mut s, "canvas.release", at(sq, json!({})));
    let after = g1(&tables(&s)).black_box.unwrap();
    let pts: Vec<(i64, i64)> = after.cps.iter().map(|c| c.1).collect();
    let mut unique = pts.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), pts.len(), "no two pins on one point: {pts:?}");
}

/// The sample's power supply -101G1 has pins on its top
/// and bottom sides whose texts are turned a quarter and reach into the box; its bottom edge dragged up stops where
/// the longest top text and the longest bottom text would meet, the status line says why, and the box is no lower
/// than that.
#[test]
fn a_box_goes_no_lower_than_its_turned_pin_texts_need() {
    let tmp = Tmp::new("box-cp-names");
    let mut s = actual_size(&tmp, "101");
    let t = tables(&s);
    let psu = t
        .reps
        .iter()
        .find(|r| r.black_box.as_ref().is_some_and(|b| b.cps.len() == 7))
        .expect("-101G1")
        .clone();
    let b = psu.black_box.clone().unwrap();
    assert!(b.room.1 > 0, "the turned texts need a height");
    select_ids(&mut s, &[psu.id.to_string()]);
    // The lowest corner on the right, dragged up to one step below the top.
    let corner = *b.outline.iter().max_by_key(|p| (p.1, p.0)).unwrap();
    let top = b.outline.iter().map(|p| index_of(&s, *p).1).min().unwrap();
    let k = index_of(&s, corner);
    let from = sheet(&s, corner);
    let up = sheet(&s, grid_point(&s, (k.0, top + 1)));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(up, json!({})));
    assert!(
        s.tool_hint().starts_with("The box goes no smaller"),
        "{}",
        s.tool_hint()
    );
    done(&mut s, "canvas.release", at(up, json!({})));
    let after = tables(&s)
        .reps
        .iter()
        .find(|r| r.id == psu.id)
        .unwrap()
        .black_box
        .clone()
        .unwrap();
    let ys: Vec<i64> = after.outline.iter().map(|p| index_of(&s, *p).1).collect();
    let height = ys.iter().max().unwrap() - ys.iter().min().unwrap();
    assert!(height >= b.room.1, "{height} {}", b.room.1);
}

// ---------------------------------------------------------------------------------------------------------------
// IB7: the picker, the transcript, the words and the counts

/// Every way of arming the black box tool ends with the hint that names both corner clicks, its key
/// from the keymap (the keycap form): the picker `tool.black_box` opens, `tool.black_box` with a part, and
/// `place.pick` of a black-box symbol (which the Symbol tool's picker sends).
fn assert_armed_hint(s: &ShellModel) {
    assert_eq!(s.tool().name(), "black_box");
    assert_eq!(s.tool().state(), "armed");
    let esc = ew_viewmodel::keymap::short_display(
        &s.shortcut_text(ew_viewmodel::actions::find("ui.dismiss").expect("the dismiss action")),
    );
    assert_eq!(
        s.tool_hint(),
        format!("Click the box's first corner, then its opposite corner; {esc} ends")
    );
    assert_eq!(esc, "Esc", "the default keymap's key");
}

#[test]
fn the_black_box_picker_arms_the_tool_with_the_two_corner_hint() {
    let tmp = Tmp::new("box-hint-picker");
    let mut s = actual_size(&tmp, "102");
    done(&mut s, "tool.black_box", json!({}));
    let row = s
        .picker()
        .expect("the picker")
        .rows
        .iter()
        .position(|r| matches!(r, ew_viewmodel::tools::PickerRow::Part(..)))
        .expect("a part row");
    done(&mut s, "picker.row", json!({"to": row}));
    done(&mut s, "picker.choose", json!({}));
    assert_armed_hint(&s);
}

#[test]
fn the_black_box_tool_with_a_part_arms_with_the_two_corner_hint() {
    let tmp = Tmp::new("box-hint-part");
    let mut s = actual_size(&tmp, "102");
    done(
        &mut s,
        "tool.black_box",
        json!({"part": "EW-GEN-PSU-1", "template": 0}),
    );
    assert_armed_hint(&s);
}

#[test]
fn place_pick_of_a_black_box_symbol_arms_with_the_two_corner_hint() {
    let tmp = Tmp::new("box-hint-pick");
    let mut s = actual_size(&tmp, "102");
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
    assert!(s.picker().is_none());
    assert_eq!(
        canvas(&s)["ghost"]["cps"],
        0,
        "a symbol without a part has no CPs"
    );
    assert_armed_hint(&s);
    // The same through `place.pick` itself, the action the Symbol tool's picker sends.
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "place.pick", json!({"symbol": "ew.gen.blackbox@qet10"}));
    assert_armed_hint(&s);
}

#[test]
fn the_tools_read_nothing_while_moving() {
    let tmp = Tmp::new("words");
    let mut s = actual_size(&tmp, "103");
    done(&mut s, "tool.ip", json!({}));
    let calls = s.session_calls();
    for k in 0..6 {
        cursor_k(&mut s, (20 + k, 30));
    }
    for _ in 0..3 {
        done(&mut s, "tool.ip_direction", json!({}));
    }
    done(&mut s, "ip.name", json!({"text": "X-1"}));
    let after = s.session_calls();
    assert_eq!(after.page_content, calls.page_content, "§12 item 4");
    assert_eq!(after.page_tables, calls.page_tables);
    assert_eq!(after.symbol_shape, calls.symbol_shape);
    assert_eq!(after.preview_with, calls.preview_with);
    done(&mut s, "ui.dismiss", json!({}));
    done(
        &mut s,
        "tool.black_box",
        json!({"part": "EW-GEN-PSU-1", "template": 0}),
    );
    enter_k(&mut s, (40, 40));
    let calls = s.session_calls();
    cursor_k(&mut s, (44, 44));
    cursor_k(&mut s, (45, 46));
    let after = s.session_calls();
    assert_eq!(after.page_content, calls.page_content);
    assert_eq!(after.page_tables, calls.page_tables);
    assert_eq!(after.symbol_shape, calls.symbol_shape);
}

/// A location node of the sample, read from its external references (the copy keeps every ID).
fn external_location() -> String {
    let (session, _) = ew_api::Session::open_read_only(std::path::Path::new(common::SAMPLE), "test").unwrap();
    let q = session.query();
    let id = q
        .external_refs()
        .iter()
        .find_map(|(_, x)| x.location)
        .map(|l| l.to_string())
        .expect("the sample has external references");
    drop(q);
    id
}

#[test]
fn ip_external_refuses_a_printed_text_two_references_share_and_takes_an_id() {
    let location = external_location();
    let tmp = Tmp::new("ip-external");
    let mut s = actual_size(&tmp, "102");
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

/// An external connection point placed after an interruption point does not inherit the IP's name; it
/// proposes its own (`IP`), and committing it leaves the IP series where it was.
#[test]
fn an_external_point_after_an_ip_does_not_inherit_its_name() {
    let location = external_location();
    let tmp = Tmp::new("ip-external-name");
    let mut s = actual_size(&tmp, "102");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "external_ref.create", "params": {
            "location": location, "device": "NAME-DEV", "connection": "Y",
        }}),
    );
    done(&mut s, "tool.ip", json!({}));
    cursor_k(&mut s, (60, 3));
    done(&mut s, "ip.name", json!({"text": "PE"}));
    done(&mut s, "tool.ip_direction", json!({"direction": "target"}));
    done(&mut s, "canvas.activate", json!({}));
    // Off the placed point (its CP now carries a potential), the name in the field: one more target of PE, whose source is on 101.
    cursor_k(&mut s, (60, 9));
    assert_eq!(canvas(&s)["ghost"]["name"], "PE", "the name in the field");
    let id = s
        .external_choices()
        .into_iter()
        .find(|(_, label)| label == "NAME-DEV:Y")
        .map(|(id, _)| id)
        .expect("the new external reference");
    // The field erased: a name in it would be the external point's too.
    done(&mut s, "ip.name", json!({"text": ""}));
    done(&mut s, "ip.external", json!({ "external": id }));
    let name = canvas(&s)["ghost"]["name"].as_str().unwrap().to_owned();
    assert!(name != "PE", "the external point's own proposal: {name}");
    assert_eq!(name, "IP");
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "the external point is placed");
    // Back to an ordinary point: a target goes on from the IP's source, not from the external point.
    done(&mut s, "ip.external", json!({"external": ""}));
    done(&mut s, "tool.ip_direction", json!({"direction": "target"}));
    cursor_k(&mut s, (60, 15));
    assert_eq!(canvas(&s)["ghost"]["name"], "PE");
}

#[test]
fn a_refused_grip_drag_keeps_its_notice_through_dwell_and_release() {
    let tmp = Tmp::new("grip-notice");
    let mut s = actual_size(&tmp, "101");
    let before = g1(&tables(&s));
    select_ids(&mut s, &[before.id.to_string()]);
    let grips = s.tool_overlay().clone();
    let b = before.black_box.as_ref().unwrap();
    let mid = ew_viewmodel::tools::black_box::grip_points(&b.outline)[5];
    let from = sheet(&s, mid);
    let k = index_of(&s, mid);
    let left = index_of(&s, b.outline[0]).0;
    let inward = sheet(&s, grid_point(&s, (k.0 - 1, k.1)));
    let to = sheet(&s, grid_point(&s, (left - 2, k.1)));
    done(&mut s, "canvas.press", at(from, json!({})));
    assert_eq!(
        refused(&mut s, "canvas.pointer", at(to, json!({}))),
        "reason-box-inside-out",
        "one move straight inside out"
    );
    assert_eq!(
        s.status_fields(false)[0],
        "A grip cannot pass the opposite side of the black box"
    );
    // A valid move in the same drag ends the refusal and brings the reshape ghost back (R1).
    done(&mut s, "canvas.pointer", at(inward, json!({})));
    assert_eq!(
        s.status_fields(false)[0],
        "Release, click or Enter to move; Esc cancels",
        "the valid move clears the notice and shows the drag hint"
    );
    assert!(!s.tool_overlay().is_empty(), "the ghost is back");
    assert_ne!(s.tool_overlay(), &grips, "the reshape ghost, not the grips");
    // Inside out again sets it again; the dwell is a pointer rest in the refused drag and must not clear it.
    assert_eq!(
        refused(&mut s, "canvas.pointer", at(to, json!({}))),
        "reason-box-inside-out"
    );
    let _ = s.dispatch("canvas.dwell", &json!({"ms": 80}));
    assert_eq!(
        refused(&mut s, "canvas.release", at(to, json!({}))),
        "reason-box-inside-out"
    );
    assert_eq!(
        s.status_fields(false)[0],
        "A grip cannot pass the opposite side of the black box",
        "the notice stays after the release"
    );
    // A hover is still the same gesture: it keeps the notice (only the next press, key, page or tool change clears it).
    let elsewhere = sheet(&s, (0, 0));
    done(&mut s, "canvas.pointer", at(elsewhere, json!({})));
    assert_eq!(
        s.status_fields(false)[0],
        "A grip cannot pass the opposite side of the black box",
        "the notice stays over the hover"
    );
    // The next primary press clears it like every other refused canvas input.
    done(&mut s, "canvas.press", at(elsewhere, json!({})));
    assert_eq!(
        s.status_fields(false)[0],
        "Click to select; drag for a rubber band"
    );
}

#[test]
fn a_refused_grip_drag_leaves_no_band_on_the_next_hover() {
    let tmp = Tmp::new("grip-no-band");
    let mut s = actual_size(&tmp, "101");
    let before = g1(&tables(&s));
    select_ids(&mut s, &[before.id.to_string()]);
    let b = before.black_box.as_ref().unwrap();
    let mid = ew_viewmodel::tools::black_box::grip_points(&b.outline)[5];
    let from = sheet(&s, mid);
    let k = index_of(&s, mid);
    let left = index_of(&s, b.outline[0]).0;
    let to = sheet(&s, grid_point(&s, (left - 2, k.1)));
    done(&mut s, "canvas.press", at(from, json!({})));
    assert_eq!(
        refused(&mut s, "canvas.pointer", at(to, json!({}))),
        "reason-box-inside-out"
    );
    assert_eq!(
        refused(&mut s, "canvas.release", at(to, json!({}))),
        "reason-box-inside-out"
    );
    // The next hover must not turn the stale press into a band.
    let elsewhere = sheet(&s, (0, 0));
    done(&mut s, "canvas.pointer", at(elsewhere, json!({})));
    assert_eq!(s.tool().state(), "idle");
    let band = s.tool_overlay().items.iter().any(|i| {
        matches!(i, ew_viewmodel::tools::OverlayItem::Rect { style, .. }
            if style.token == ew_viewmodel::tools::Token::RubberBand)
    });
    assert!(!band, "the hover draws no band");
}

/// A potential could not be named where a device's pins touch terminals and no
/// wire is there to select. A device's right-click at a pin offers Define potential at that pin…: it opens the form for
/// that pin, and the name typed defines the potential there.
#[test]
fn a_pin_is_offered_define_potential_on_its_devices_right_click() {
    let tmp = Tmp::new("pin-potential");
    let mut s = actual_size(&tmp, "101");
    let g = g1(&tables(&s));
    let cp = tables(&s)
        .cps
        .iter()
        .find(|c| c.rep == g.id && c.label.ends_with(":L"))
        .expect("a pin of -101G1")
        .clone();
    select_ids(&mut s, &[g.id.to_string()]);
    let at = sheet(&s, cp.at);
    let menu =
        ew_viewmodel::command_bar::canvas_menu(&s, ew_viewmodel::command_bar::MenuSpot::Object, Some(at));
    let item = menu
        .iter()
        .find(|i| i.label == format!("Define potential at {}…", cp.label))
        .unwrap_or_else(|| panic!("{:?}", menu.iter().map(|i| &i.label).collect::<Vec<_>>()));
    assert!(item.enabled, "{:?}", item.reason);
    assert_eq!(item.params, json!({ "at": cp.label }));
    // Away from the pins the device's menu offers none.
    let far = ew_viewmodel::command_bar::canvas_menu(
        &s,
        ew_viewmodel::command_bar::MenuSpot::Object,
        Some((at.0 + 50_000_000, at.1 + 50_000_000)),
    );
    assert!(!far.iter().any(|i| i.label.starts_with("Define potential at ")));
    // The form opens for the pin; its name defines the potential there.
    done(&mut s, "potential.draw", item.params.clone());
    done(
        &mut s,
        "potential.draw",
        json!({ "name": "PIN-POT", "at": cp.label }),
    );
    let last = s.recorded_commands().last().expect("a command").clone();
    assert_eq!(last.cmd, "potential.define");
    assert_eq!(
        last.params["at"],
        json!({"cp": {"function": cp.function.to_string(), "idx": cp.idx}})
    );
}

/// A re-test (two wires thin and dashed after a grid change, with no word why): a wire without a route says in
/// its tooltip what its thin dashed line means and what to do.
#[test]
fn an_unrouted_wire_says_why_it_is_dashed() {
    let tmp = Tmp::new("unrouted-tip");
    let (mut s, _) = plain_box(&tmp);
    for k in [(60, 20), (60, 40)] {
        done(
            &mut s,
            "place.pick",
            json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        );
        let p = grid_point(&s, k);
        cursor(&mut s, p);
        done(&mut s, "canvas.activate_alt", json!({}));
        done(&mut s, "tool.select", json!({}));
    }
    let t = tables(&s);
    let end = |label: &str| {
        t.cps
            .iter()
            .find(|c| c.label.ends_with(label))
            .expect(label)
            .clone()
    };
    let (a, b) = (end("K1:A2"), end("K2:A1"));
    let draw = json!({"cmd": "connection.draw", "params": {
        "a": {"cp": {"function": a.function.to_string(), "idx": a.idx}},
        "b": {"cp": {"function": b.function.to_string(), "idx": b.idx}},
        "page": t.page.to_string()}});
    done(&mut s, "command.run", draw);
    let middle = sheet(&s, (a.at.0, (a.at.1 + b.at.1) / 2));
    done(&mut s, "canvas.pointer", at(middle, json!({})));
    assert_eq!(
        s.canvas().unwrap().hovered_line_tip().as_deref(),
        Some(
            "This wire has no route, so it is drawn as a thin dashed line: move what is in its way and choose Route."
        )
    );
}
