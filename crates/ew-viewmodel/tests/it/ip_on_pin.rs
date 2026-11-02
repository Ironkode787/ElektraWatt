// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The delivery play-test of cycle 41 (rv-vis-5, FIXPT41): an interruption point dropped so that its connection
//! point lies on a pin connects to it (FR-CON-003, WP-S2.04 §9.1), in one undo step, as a symbol placed the same way
//! does; Alt places it free; a drop on another interruption point, and an external point's on a pin, are refused in
//! words.

use crate::common;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::ew_api::Session;
use ew_viewmodel::ew_api::checks;
use ew_viewmodel::ew_api::output::{
    PageTables, TableConnection, TableCp, TableEnd, TablePotential, TableRep,
};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::tools::{IpDir, MarkerKind, OverlayItem};
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

fn ghost(s: &ShellModel) -> Value {
    state(s)["canvas"]["ghost"].clone()
}

fn hud(s: &ShellModel) -> String {
    ew_viewmodel::command_bar::hud(s).prompt
}

fn toast(s: &ShellModel) -> String {
    state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

fn cp(s: &ShellModel, label: &str) -> TableCp {
    tables(s)
        .cps
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("{label}"))
        .clone()
}

/// Moves the grid cursor to the page point `p`.
fn cursor(s: &mut ShellModel, p: (i64, i64)) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "cursor.set", at(q, json!({})));
}

/// The page point of grid index `k`.
fn grid_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// The ghost's insertion point (page nm).
fn ghost_pos(s: &ShellModel) -> (i64, i64) {
    let g = ghost(s);
    (g["at_nm"][0].as_i64().unwrap(), g["at_nm"][1].as_i64().unwrap())
}

/// The insertion point whose ghost CP lies on the page point `target`, read from the armed tool's ghost
/// (`pos = target - orient(cp.at)`): its insertion point and its CP marker at a free grid point.
fn insertion_for(s: &mut ShellModel, target: (i64, i64)) -> (i64, i64) {
    let probe = grid_point(s, (40, 40));
    cursor(s, probe);
    let pos = ghost_pos(s);
    let marker = s
        .tool_overlay()
        .items
        .iter()
        .find_map(|i| match i {
            OverlayItem::Marker {
                at,
                kind: MarkerKind::Cp,
            } => Some(*at),
            _ => None,
        })
        .expect("the ghost's CP marker");
    let c = s.canvas().unwrap().page().snapper.sheet_to_page(marker);
    (target.0 - (c.0 - pos.0), target.1 - (c.1 - pos.1))
}

/// Arms the IP tool with the typed name `name` and puts the grid cursor where the ghost's CP lies on `pin`.
fn arm_on(s: &mut ShellModel, name: &str, pin: (i64, i64)) {
    done(s, "tool.ip", json!({}));
    done(s, "ip.name", json!({ "text": name }));
    let pos = insertion_for(s, pin);
    cursor(s, pos);
}

/// The HUD of the IP tool's ghost without the pin part: the name, the direction and D's next one.
fn hud_head(s: &ShellModel, name: &str) -> String {
    let direction = ghost(s)["direction"].as_str().unwrap().to_owned();
    let dir = IpDir::from_word(&direction).unwrap();
    // FT-899: the role in words.
    let role = match dir {
        IpDir::Source => "source: feeds its targets",
        IpDir::Target => "target: fed by its source",
        IpDir::Undirected => "undirected: pairs either way",
    };
    let next = dir.next().word();
    format!("{name} \u{b7} {role} \u{b7} D: {next}")
}

/// The EW-POT-009 subjects of the saved project (the drafting scheme, online), as `(severity, subject)` sorted.
fn pot009(tmp: &Tmp, s: &mut ShellModel) -> Vec<(String, String)> {
    done(s, "file.save", json!({}));
    let (session, _) = Session::open_read_only(&tmp.golden_copy(), "test").unwrap();
    let q = session.query();
    let mut out: Vec<(String, String)> = checks::run(&q, &checks::FULL, checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.is_open() && f.rule == "EW-POT-009")
        .map(|f| (f.severity.name().to_owned(), f.subject))
        .collect();
    out.sort();
    out
}

fn subjects(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

/// The representations and connections of the page on show that `before` does not hold.
fn added(before: &PageTables, s: &ShellModel) -> (Vec<TableRep>, Vec<TableConnection>) {
    let now = tables(s);
    (
        now.reps
            .iter()
            .filter(|r| !before.reps.iter().any(|b| b.id == r.id))
            .cloned()
            .collect(),
        now.connections
            .iter()
            .filter(|c| !before.connections.iter().any(|b| b.id == c.id))
            .cloned()
            .collect(),
    )
}

/// Asserts the one new connection runs from `pin` to the new interruption point's CP with no route.
#[track_caller]
fn assert_connects(s: &ShellModel, before: &PageTables, pin: &TableCp, name: &str) {
    let (reps, conns) = added(before, s);
    assert_eq!(reps.len(), 1, "one new representation");
    assert_eq!(
        reps[0].ip.as_ref().map(|ip| ip.name.as_str()),
        Some(name),
        "an interruption point"
    );
    assert_eq!(conns.len(), 1, "one new connection");
    let ip_cp = tables(s)
        .cps
        .iter()
        .find(|c| c.rep == reps[0].id)
        .unwrap()
        .clone();
    assert_eq!(ip_cp.at, pin.at, "the point's CP lies on the pin");
    let ends = [conns[0].a, conns[0].b];
    let want = [
        TableEnd::Cp {
            function: pin.function,
            idx: pin.idx,
        },
        TableEnd::Cp {
            function: ip_cp.function,
            idx: ip_cp.idx,
        },
    ];
    assert!(ends == want || ends == [want[1], want[0]], "{ends:?}");
    assert!(!conns[0].stored_route, "no route");
    assert_eq!(
        conns[0].polyline,
        vec![pin.at],
        "a wire of length zero: a single point, which the scene does not draw"
    );
}

/// The play-test's drop (ruling 1: option (c)): the "0V" point's CP on the free pin -102F2:14 (EF-09). The HUD says it
/// before the click, the dwell previews the connection, the click adds the point and one route-less connection in one
/// undo step, the toast says it, and EW-POT-009 no longer reports the pin.
#[test]
fn a_point_dropped_on_a_free_pin_connects_to_it() {
    let tmp = Tmp::new("ip-pin-free");
    let mut s = shell(&tmp, "102");
    let pin = cp(&s, "-102F2:14");
    arm_on(&mut s, "0V", pin.at);
    assert_eq!(ghost(&s)["connects"], "-102F2:14");
    // The describe line ends with the pin, a word of `Reach::IpPin`.
    let line = s
        .describe()
        .lines()
        .find(|l| l.starts_with("canvas ghost ip "))
        .unwrap()
        .to_owned();
    assert!(line.ends_with(" connects \"-102F2:14\""), "{line}");
    for w in ew_viewmodel::describe::words(ew_viewmodel::describe::Reach::IpPin) {
        assert!(line.split_whitespace().any(|t| t == w), "{w}");
    }
    assert_eq!(
        hud(&s),
        format!("{} \u{b7} connects to -102F2:14", hud_head(&s, "0V"))
    );
    done(&mut s, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(state(&s)["canvas"]["preview"]["command"], "connection.draw_to_ip");
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    assert_connects(&s, &before, &pin, "0V");
    let cell = {
        let (reps, _) = added(&before, &s);
        let page = s.canvas().unwrap().page();
        page.cell_text(page.snapper.page_to_sheet(reps[0].pos)).unwrap()
    };
    assert_eq!(
        toast(&s),
        format!(
            "Placed the interruption point 0V at {cell} and connected it to -102F2:14. Alt+Click or Ctrl+Enter places \
             it without connecting"
        )
    );
    // The control pin stays reported; -102F2:14 is connected now.
    assert_eq!(pot009(&tmp, &mut s), subjects(&[("warning", "-102F2:13")]));
    // One undo step removes both.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(*tables(&s), *before, "the page tables as before");
}

/// Alt places the point free (`ip.place`, no connection): Alt held on the hover shows no "connects to" and previews
/// `ip.place` (feasibility read, item 3); `canvas.activate_alt` commits it; EW-POT-009 still reports -102F2:14.
#[test]
fn alt_places_the_point_without_connecting() {
    let tmp = Tmp::new("ip-pin-alt");
    let mut s = shell(&tmp, "102");
    let pin = cp(&s, "-102F2:14");
    arm_on(&mut s, "0V", pin.at);
    assert_eq!(ghost(&s)["connects"], "-102F2:14", "without Alt it connects");
    let sheet = s.canvas().unwrap().page().snapper.page_to_sheet(ghost_pos(&s));
    done(&mut s, "canvas.pointer", at(sheet, json!({"alt": true})));
    assert_eq!(ghost(&s)["connects"], Value::Null);
    assert_eq!(hud(&s), hud_head(&s, "0V"));
    done(&mut s, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(state(&s)["canvas"]["preview"]["command"], "ip.place");
    // `canvas.activate_alt` moves the grid cursor (which releases a hovered Alt) and presses with Alt.
    let before = tables(&s);
    done(&mut s, "canvas.activate_alt", json!({}));
    let (reps, conns) = added(&before, &s);
    assert_eq!(reps.len(), 1);
    assert_eq!(reps[0].ip.as_ref().map(|ip| ip.name.as_str()), Some("0V"));
    assert!(conns.is_empty(), "no connection");
    assert_eq!(
        pot009(&tmp, &mut s),
        subjects(&[("warning", "-102F2:13"), ("warning", "-102F2:14")])
    );
}

/// Ruling 2 (a): "the click means 'this point is on this pin' either way" — a pin that already has a wire gets a
/// second connection, and its wire is unchanged.
#[test]
fn a_point_on_a_wired_pin_joins_it() {
    let tmp = Tmp::new("ip-pin-wired");
    let mut s = shell(&tmp, "102");
    let t = tables(&s);
    let pin = t
        .cps
        .iter()
        .find(|c| {
            c.connections.len() == 1
                && t.reps.iter().any(|r| r.id == c.rep && r.ip.is_none())
                && t.cps.iter().filter(|o| o.at == c.at).count() == 1
        })
        .expect("a wired pin on 102")
        .clone();
    arm_on(&mut s, "24V", pin.at);
    let words = ghost(&s)["connects"].as_str().unwrap_or_default().to_owned();
    assert_eq!(words, pin.label, "the pin in words");
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    assert_connects(&s, &before, &pin, "24V");
    let wire = |t: &PageTables| t.connections.iter().find(|c| c.id == pin.connections[0]).cloned();
    assert_eq!(wire(&tables(&s)), wire(&before), "the existing wire is unchanged");
}

/// A second drop on the "0V" just placed (its CP and the pin's coincide): refused in words, nothing committed, the
/// ghost kept.
#[test]
fn a_point_on_another_point_is_refused() {
    let tmp = Tmp::new("ip-pin-on-ip");
    let mut s = shell(&tmp, "102");
    let pin = cp(&s, "-102F2:14");
    arm_on(&mut s, "0V", pin.at);
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(
        ghost(&s)["connects"],
        Value::Null,
        "the HUD offers no connection on a point"
    );
    let before = tables(&s);
    let rev = s.revision();
    let items = s.tool_overlay().items.len();
    assert!(items > 0);
    assert_eq!(
        refused(&mut s, "canvas.activate", json!({})),
        "reason-ip-not-placed"
    );
    let words =
        "Not placed: the interruption point 0V is already here; place it on a pin or a free grid point";
    assert_eq!(s.revision(), rev, "nothing committed");
    assert_eq!(*tables(&s), *before);
    assert_eq!(s.tool().name(), "ip");
    assert_eq!(s.tool_overlay().items.len(), items, "the ghost stays");
    assert_eq!(s.tool_notice(), Some(words));
    assert_eq!(toast(&s), words);
}

/// The page point of the armed IP tool's ghost CP marker.
fn ghost_cp(s: &ShellModel) -> (i64, i64) {
    let marker = s
        .tool_overlay()
        .items
        .iter()
        .find_map(|i| match i {
            OverlayItem::Marker {
                at,
                kind: MarkerKind::Cp,
            } => Some(*at),
            _ => None,
        })
        .expect("the ghost's CP marker");
    s.canvas().unwrap().page().snapper.sheet_to_page(marker)
}

/// FT-948 (ID-1150): a point whose connection point lies on a junction, or strictly inside a wire, would sit on it
/// without joining it: refused in words, nothing committed, the ghost kept; the HUD offers no connection there.
#[test]
fn ft948_a_point_on_a_junction_or_inside_a_wire_is_refused() {
    const WORDS: &str = "Not placed: the point would sit on a wire without joining it; place it on a pin or a free grid point";
    let tmp = Tmp::new("ip-on-wire");
    let mut s = shell(&tmp, "102");
    let t = tables(&s);
    // A junction on no pin (on a pin, the pin wins and the point connects to it) where the ghost's CP can lie (the
    // insertion point is on the grid).
    let junction = t
        .junctions
        .iter()
        .map(|j| j.at)
        .filter(|at| !t.cps.iter().any(|c| c.at == *at))
        .find(|at| {
            arm_on(&mut s, "W948", *at);
            ghost_cp(&s) == *at
        })
        .expect("a junction on 102 off the pins");
    // A grid point strictly inside a horizontal segment of a wire, on no connection point.
    let inside = {
        let sn = &s.canvas().unwrap().page().snapper;
        let on_grid = |v: i64| sn.page_of_index(sn.grid_index_of(v)) == v;
        t.connections
            .iter()
            .flat_map(|c| {
                let line = c.polyline.clone();
                let ends = (line[0], line[line.len() - 1]);
                line.windows(2)
                    .filter(|w| w[0].1 == w[1].1 && on_grid(w[0].1))
                    .flat_map(|w| {
                        let (a, b) = (sn.grid_index_of(w[0].0), sn.grid_index_of(w[1].0));
                        (a.min(b) + 1..a.max(b)).map(|k| (sn.page_of_index(k), w[0].1))
                    })
                    .filter(move |p| *p != ends.0 && *p != ends.1)
                    .collect::<Vec<_>>()
            })
            .find(|p| !t.cps.iter().any(|c| c.at == *p) && !t.junctions.iter().any(|j| j.at == *p))
            .expect("a grid point inside a wire on 102")
    };
    for (name, target) in [("W948A", junction), ("W948B", inside)] {
        arm_on(&mut s, name, target);
        assert_eq!(ghost_cp(&s), target, "{name}: the ghost's CP on the target");
        assert_eq!(
            ghost(&s)["connects"],
            Value::Null,
            "{name}: no connection offered"
        );
        let before = tables(&s);
        let rev = s.revision();
        assert_eq!(
            refused(&mut s, "canvas.activate", json!({})),
            "reason-ip-not-placed",
            "{name}"
        );
        assert_eq!(s.revision(), rev, "{name}: nothing committed");
        assert_eq!(*tables(&s), *before);
        assert_eq!(s.tool().name(), "ip");
        assert_eq!(s.tool_notice(), Some(WORDS), "{name}");
        assert_eq!(toast(&s), WORDS, "{name}");
        done(&mut s, "ui.dismiss", json!({}));
        done(&mut s, "ui.dismiss", json!({}));
    }
}

/// FT-946 (ID-1146, ID-1019's ruling 3 b): an external connection point dropped on a pin connects to it as any point
/// does: the HUD says so, the click commits `connection.draw_to_ip` with the point's `external`, in one undo step, and
/// the saved point is external; on a free grid point it still places.
#[test]
fn an_external_point_on_a_pin_connects_to_it() {
    let tmp = Tmp::new("ip-pin-external");
    let mut s = shell(&tmp, "102");
    let pin = cp(&s, "-102F2:13");
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "ip.external", json!({"external": "JE-N1000-03:N"}));
    let pos = insertion_for(&mut s, pin.at);
    cursor(&mut s, pos);
    assert_eq!(ghost(&s)["connects"], "-102F2:13");
    done(&mut s, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(state(&s)["canvas"]["preview"]["command"], "connection.draw_to_ip");
    let before = tables(&s);
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    let name = added(&before, &s).0[0].ip.as_ref().unwrap().name.clone();
    assert_connects(&s, &before, &pin, &name);
    let function = added(&before, &s).0[0].functions[0];
    // The saved point is external, to JE-N1000-03:N.
    done(&mut s, "file.save", json!({}));
    {
        let (session, _) = Session::open_read_only(&tmp.golden_copy(), "test").unwrap();
        let q = session.query();
        let want = q
            .external_refs()
            .iter()
            .find(|(_, x)| {
                x.device.as_str() == "JE-N1000-03" && x.connection.as_ref().map(|c| c.as_str()) == Some("N")
            })
            .map(|(id, _)| id.to_string())
            .expect("JE-N1000-03:N");
        let f = q
            .functions()
            .iter()
            .find(|(id, _)| id.as_uuid() == function)
            .map(|(_, f)| serde_json::to_value(&**f).unwrap())
            .expect("the new point");
        assert_eq!(f["kind"]["ip"]["external"], json!(want));
    }
    // One undo step removes the point and its connection.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(*tables(&s), *before, "the page tables as before");
    // On a free grid point the external arrow places as before.
    let free = grid_point(&s, (40, 40));
    cursor(&mut s, free);
    done(&mut s, "canvas.activate", json!({}));
    let (reps, conns) = added(&before, &s);
    assert_eq!(reps.len(), 1, "the external point is placed");
    assert!(conns.is_empty());
}

/// FT-822's refusal holds for a drop on a pin: a source name that already has a source, dropped on a pin with the
/// direction source, is refused by EW-CORE-052 in its words; nothing committed, the ghost kept.
#[test]
fn a_second_source_on_a_pin_keeps_ft822s_refusal() {
    let tmp = Tmp::new("ip-pin-source");
    let mut s = shell(&tmp, "102");
    let source = tables(&s)
        .reps
        .iter()
        .filter_map(|r| r.ip.as_ref())
        .find(|ip| ip.direction == "source")
        .map(|ip| ip.name.clone())
        .expect("a source point on 102");
    let pin = cp(&s, "-102F2:14");
    arm_on(&mut s, &source, pin.at);
    done(&mut s, "tool.ip_direction", json!({"direction": "source"}));
    assert_eq!(ghost(&s)["connects"], "-102F2:14");
    let before = tables(&s);
    let items = s.tool_overlay().items.len();
    assert_eq!(
        refused(&mut s, "canvas.activate", json!({})),
        "reason-ip-not-placed"
    );
    let words = format!(
        "Not placed: {source} already has a source on page 102; press D for a target, or mark one source as the star"
    );
    assert_eq!(*tables(&s), *before, "nothing committed");
    assert_eq!(s.tool_overlay().items.len(), items, "the ghost stays");
    assert_eq!(s.tool_notice(), Some(words.as_str()));
    assert_eq!(toast(&s), words);
}

/// The probe of the feasibility read (item 5): a route-less connection to an interruption point whose CP lies on a
/// pin draws no polyline of its own, checks the pin as connected, and saves and reloads byte-stable.
#[test]
fn a_route_less_connection_to_a_point_on_a_pin_saves_and_reloads_byte_stable() {
    let tmp = Tmp::new("ip-pin-probe");
    let mut s = shell(&tmp, "102");
    let pin = cp(&s, "-102F2:14");
    arm_on(&mut s, "0V", pin.at);
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    let (_, conns) = added(&before, &s);
    assert_eq!(conns.len(), 1);
    assert_eq!(pot009(&tmp, &mut s), subjects(&[("warning", "-102F2:13")]));
    let snapshot = |dir: &std::path::Path| {
        let mut files = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let e = e.unwrap();
                if e.file_type().unwrap().is_dir() {
                    stack.push(e.path());
                } else {
                    files.push((e.path(), std::fs::read(e.path()).unwrap()));
                }
            }
        }
        files.retain(|(p, _)| {
            let p = p.to_string_lossy();
            !p.contains("lock") && !p.contains("journal")
        });
        files.sort();
        files
    };
    let first = snapshot(&tmp.golden_copy());
    drop(s);
    let mut s2 = shell(&tmp, "102");
    assert!(tables(&s2).connections.iter().any(|c| c.id == conns[0].id));
    done(&mut s2, "file.save", json!({}));
    drop(s2);
    let second = snapshot(&tmp.golden_copy());
    assert_eq!(
        first.iter().map(|f| &f.0).collect::<Vec<_>>(),
        second.iter().map(|f| &f.0).collect::<Vec<_>>()
    );
    for (a, b) in first.iter().zip(&second) {
        assert!(a.1 == b.1, "{} changed on a re-save", a.0.display());
    }
}

/// FT-950 (ID-1146): the next name after a connecting drop. A target "FT950" dropped on the free pin -102F2:14 makes a
/// potential whose name is only the pin's fallback; the ghost kept there proposes the next of the name just placed (as
/// `propose` counts among the targets of the page), not the fallback name. The Wire tool's P, from that pin, names its
/// end from the name just placed the same way.
#[test]
fn ft950_after_a_connecting_drop_the_next_name_goes_on_from_the_name_placed() {
    let tmp = Tmp::new("ip-pin-next-name");
    let mut s = shell(&tmp, "102");
    let pin = cp(&s, "-102F2:14");
    arm_on(&mut s, "FT950", pin.at);
    done(&mut s, "tool.ip_direction", json!({"direction": "target"}));
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    assert_connects(&s, &before, &pin, "FT950");
    // A target names no potential: the pin's potential has its fallback name.
    let start = cp(&s, "-102F2:14");
    assert_eq!(
        start.potential,
        Some(TablePotential {
            name: "-102F2:14".to_owned(),
            fallback: true
        })
    );
    // The ghost stays where it was dropped: its CP on the pin and the new point's.
    let pos = ghost_pos(&s);
    cursor(&mut s, pos);
    assert_eq!(
        (ghost(&s)["name"].as_str(), ghost(&s)["direction"].as_str()),
        (Some("FT950-1"), Some("target"))
    );
    // The Wire tool's P from the pin: the end is named from the name just placed.
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.wire", json!({}));
    cursor(&mut s, start.at);
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.tool().state(), "routing");
    let k = {
        let sn = &s.canvas().unwrap().page().snapper;
        (sn.grid_index_of(start.at.0), sn.grid_index_of(start.at.1))
    };
    let end = grid_point(&s, (k.0, k.1 + 4));
    cursor(&mut s, end);
    let before = tables(&s);
    done(&mut s, "tool.ip", json!({}));
    let (reps, _) = added(&before, &s);
    let ip = reps[0].ip.clone().expect("the end's interruption point");
    assert_eq!((ip.name.as_str(), ip.direction.as_str()), ("FT950-1", "target"));
}
