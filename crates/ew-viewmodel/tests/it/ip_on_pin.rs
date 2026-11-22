// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An interruption point dropped so that its connection
//! point lies on a pin connects to it (FR-CON-003), in one undo step, as a symbol placed the same way
//! does; Alt places it free; a drop on another interruption point, and an external point's on a pin, are refused in
//! words.

use crate::common;

use common::{Tmp, at, done, refused, state, tables, toast};
use ew_api::Session;
use ew_api::checks;
use ew_api::output::{PageTables, TableConnection, TableCp, TableEnd, TablePotential, TableRep};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::tools::{IpDir, MarkerKind, OverlayItem};
use serde_json::{Value, json};

/// A headless shell with a writable copy of the sample on `folio`, at actual size.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.sample_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// [`shell`] on folio 102 with a push button placed in its empty right half: -102S3, whose pins 13 and 14 no wire
/// reaches, so that EW-POT-009 reports both (the sample has no free pin of a device).
fn with_a_free_button(tmp: &Tmp) -> ShellModel {
    let mut s = shell(tmp, "102");
    done(&mut s, "cursor.set", json!({"x_mm": 200.0, "y_mm": 80.0}));
    done(&mut s, "tool.place", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-PB-1", "letter": "S"}),
    );
    done(&mut s, "canvas.activate", json!({"alt": true}));
    done(&mut s, "ui.dismiss", json!({}));
    s
}

fn ghost(s: &ShellModel) -> Value {
    state(s)["canvas"]["ghost"].clone()
}

fn hud(s: &ShellModel) -> String {
    ew_viewmodel::command_bar::hud(s).prompt
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
/// (`pos = target - orient(cp.at)`): its insertion point and its CP marker with the ghost at the target, so the
/// ghost shows the form of the direction there (a source's CP is one grid step behind its tip, a
/// target's at its tip).
fn insertion_for(s: &mut ShellModel, target: (i64, i64)) -> (i64, i64) {
    cursor(s, target);
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
    // The role in words.
    let role = match dir {
        IpDir::Source => "source: where the potential comes from, one for its name",
        IpDir::Target => "target: where the potential goes on, fed by its source",
        IpDir::Undirected => "undirected: pairs either way",
    };
    let next = dir.next().word();
    format!("{name} \u{b7} {role} \u{b7} D: {next}")
}

/// The EW-POT-009 subjects of the saved project (the drafting scheme, online), as `(severity, subject)` sorted.
fn pot009(tmp: &Tmp, s: &mut ShellModel) -> Vec<(String, String)> {
    done(s, "file.save", json!({}));
    let (session, _) = Session::open_read_only(&tmp.sample_copy(), "test").unwrap();
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

/// The play-test's drop: the "5V" point's CP on the free pin -102S3:14 (EF-09). The HUD says it
/// before the click, the dwell previews the connection, the click adds the point and one route-less connection in one
/// undo step, the toast says it, and EW-POT-009 no longer reports the pin.
#[test]
fn a_point_dropped_on_a_free_pin_connects_to_it() {
    let tmp = Tmp::new("ip-pin-free");
    let mut s = with_a_free_button(&tmp);
    let pin = cp(&s, "-102S3:14");
    arm_on(&mut s, "5V", pin.at);
    assert_eq!(ghost(&s)["connects"], "-102S3:14");
    // The describe line ends with the pin.
    let line = s
        .describe()
        .lines()
        .find(|l| l.starts_with("canvas ghost ip "))
        .unwrap()
        .to_owned();
    assert!(line.ends_with(" connects \"-102S3:14\""), "{line}");
    assert_eq!(
        hud(&s),
        format!("{} \u{b7} connects to -102S3:14", hud_head(&s, "5V"))
    );
    done(&mut s, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(state(&s)["canvas"]["preview"]["command"], "connection.draw_to_ip");
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    assert_connects(&s, &before, &pin, "5V");
    let cell = {
        let (reps, _) = added(&before, &s);
        let page = s.canvas().unwrap().page();
        page.cell_text(page.snapper.page_to_sheet(reps[0].pos)).unwrap()
    };
    assert_eq!(
        toast(&s),
        format!(
            "Placed the interruption point 5V at {cell} and connected it to -102S3:14. Alt+Click or Ctrl+Enter places \
             it without connecting. 5V at {cell} goes on nowhere yet: place where 5V goes on, its targets."
        )
    );
    // The control pin stays reported; -102S3:14 is connected now.
    assert_eq!(pot009(&tmp, &mut s), subjects(&[("warning", "-102S3:13")]));
    // One undo step removes both.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(*tables(&s), *before, "the page tables as before");
}

/// Alt places the point free (`ip.place`, no connection): Alt held on the hover shows no "connects to" and previews
/// `ip.place` (feasibility read, item 3); `canvas.activate_alt` commits it; EW-POT-009 still reports -102S3:14.
#[test]
fn alt_places_the_point_without_connecting() {
    let tmp = Tmp::new("ip-pin-alt");
    let mut s = with_a_free_button(&tmp);
    let pin = cp(&s, "-102S3:14");
    arm_on(&mut s, "5V", pin.at);
    assert_eq!(ghost(&s)["connects"], "-102S3:14", "without Alt it connects");
    let sheet = s.canvas().unwrap().page().snapper.page_to_sheet(ghost_pos(&s));
    done(&mut s, "canvas.pointer", at(sheet, json!({"alt": true})));
    assert_eq!(ghost(&s)["connects"], Value::Null);
    assert_eq!(hud(&s), hud_head(&s, "5V"));
    done(&mut s, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(state(&s)["canvas"]["preview"]["command"], "ip.place");
    // `canvas.activate_alt` moves the grid cursor (which releases a hovered Alt) and presses with Alt.
    let before = tables(&s);
    done(&mut s, "canvas.activate_alt", json!({}));
    let (reps, conns) = added(&before, &s);
    assert_eq!(reps.len(), 1);
    assert_eq!(reps[0].ip.as_ref().map(|ip| ip.name.as_str()), Some("5V"));
    assert!(conns.is_empty(), "no connection");
    assert_eq!(
        pot009(&tmp, &mut s),
        subjects(&[("warning", "-102S3:13"), ("warning", "-102S3:14")])
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

/// A second drop on the "5V" just placed (its CP and the pin's coincide): refused in words, nothing committed, the
/// ghost kept.
#[test]
fn a_point_on_another_point_is_refused() {
    let tmp = Tmp::new("ip-pin-on-ip");
    let mut s = with_a_free_button(&tmp);
    let pin = cp(&s, "-102S3:14");
    arm_on(&mut s, "5V", pin.at);
    done(&mut s, "canvas.activate", json!({}));
    // Without the typed name (it stays for a source's target, whose form has its CP elsewhere) the ghost is the next
    // point of the same form, on the point just placed.
    done(&mut s, "ip.name", json!({"text": ""}));
    // A source again: the waiting source's partner, a target, would have its CP elsewhere.
    done(&mut s, "tool.ip_direction", json!({"direction": "source"}));
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
        "Not placed: the interruption point 5V is already here; place it on a pin or a free grid point";
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

/// A point whose connection point lies on a junction, or strictly inside a wire, would sit on it
/// without joining it: refused in words, nothing committed, the ghost kept; the HUD offers no connection there.
#[test]
fn a_point_on_a_junction_or_inside_a_wire_is_refused() {
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

/// A drawing of an older build: a wire of folio 102 stored 1 nm beside the grid's number of its line (the pins' place
/// plus offset). The page reads it on the grid's number, so a point dropped strictly inside it is on it and refused as
/// above, not placed free beside it.
#[test]
fn a_point_inside_an_older_drawing_s_wire_is_on_it() {
    let tmp = Tmp::new("ip-older-wire");
    let s = shell(&tmp, "102");
    let t = tables(&s);
    let sn = s.canvas().unwrap().page().snapper.clone();
    let on_grid = |v: i64| sn.page_of_index(sn.grid_index_of(v)) == v;
    // A stored wire with a horizontal stretch on the grid, and a grid point strictly inside it on no pin or junction.
    let (wire, y, inside) = t
        .connections
        .iter()
        .filter(|c| c.stored_route)
        .find_map(|c| {
            let line = &c.polyline;
            line.windows(2)
                .filter(|w| w[0].1 == w[1].1 && on_grid(w[0].1))
                .find_map(|w| {
                    let (a, b) = (sn.grid_index_of(w[0].0), sn.grid_index_of(w[1].0));
                    (a.min(b) + 1..a.max(b))
                        .map(|k| (sn.page_of_index(k), w[0].1))
                        .find(|p| {
                            !t.cps.iter().any(|c| c.at == *p) && !t.junctions.iter().any(|j| j.at == *p)
                        })
                })
                .map(|p| (c.id, p.1, p))
        })
        .expect("a wire with a free grid point inside on 102");
    drop(s);
    // Store that wire as an older build did: every vertex on its line's y 1 nm beside it.
    let file = tmp.sample_copy().join("pages").join(format!("{}.jsonl", t.page));
    let text = std::fs::read_to_string(&file).expect("the page's file");
    let id = format!("\"id\":\"{wire}\"");
    let older: Vec<String> = text
        .lines()
        .map(|l| {
            let Some(start) = l.find("\"route\":[").filter(|_| l.contains(&id)) else {
                return l.to_owned();
            };
            let end = start + l[start..].find("]]").expect("the route's end") + 2;
            let route = l[start..end].replace(&format!(",{y}]"), &format!(",{}]", y + 1));
            format!("{}{route}{}", &l[..start], &l[end..])
        })
        .collect();
    assert_ne!(older.join("\n") + "\n", text, "the wire is rewritten");
    std::fs::write(&file, older.join("\n") + "\n").expect("written");
    let mut s = shell(&tmp, "102");
    arm_on(&mut s, "W949", inside);
    assert_eq!(ghost_cp(&s), inside);
    assert_eq!(ghost(&s)["connects"], Value::Null, "no connection offered");
    assert_eq!(
        refused(&mut s, "canvas.activate", json!({})),
        "reason-ip-not-placed"
    );
    assert_eq!(
        s.tool_notice(),
        Some(
            "Not placed: the point would sit on a wire without joining it; place it on a pin or a free grid point"
        )
    );
}

/// An external connection point dropped on a pin connects to it as any point
/// does: the HUD says so, the click commits `connection.draw_to_ip` with the point's `external`, in one undo step, and
/// the saved point is external; on a free grid point it still places.
#[test]
fn an_external_point_on_a_pin_connects_to_it() {
    let tmp = Tmp::new("ip-pin-external");
    let mut s = with_a_free_button(&tmp);
    let pin = cp(&s, "-102S3:13");
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "ip.external", json!({"external": "-Q0"}));
    let pos = insertion_for(&mut s, pin.at);
    cursor(&mut s, pos);
    assert_eq!(ghost(&s)["connects"], "-102S3:13");
    done(&mut s, "canvas.dwell", json!({"ms": 100}));
    assert_eq!(state(&s)["canvas"]["preview"]["command"], "connection.draw_to_ip");
    let before = tables(&s);
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    let name = added(&before, &s).0[0].ip.as_ref().unwrap().name.clone();
    assert_connects(&s, &before, &pin, &name);
    // The toast names the external point so; both sentences end with a full stop.
    let cell = {
        let (reps, _) = added(&before, &s);
        let page = s.canvas().unwrap().page();
        page.cell_text(page.snapper.page_to_sheet(reps[0].pos)).unwrap()
    };
    assert_eq!(
        toast(&s),
        format!(
            "Placed the external point {name} at {cell} and connected it to -102S3:13. Alt+Click or Ctrl+Enter \
             places it without connecting."
        )
    );
    let function = added(&before, &s).0[0].functions[0];
    // The saved point is external, to the upstream supply -Q0, the sample's external reference.
    done(&mut s, "file.save", json!({}));
    {
        let (session, _) = Session::open_read_only(&tmp.sample_copy(), "test").unwrap();
        let q = session.query();
        let want = q
            .external_refs()
            .iter()
            .find(|(_, x)| {
                x.device.as_ref().is_some_and(|d| d.as_str().ends_with("Q0")) && x.connection.is_none()
            })
            .map(|(id, _)| id.to_string())
            .expect("-Q0");
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

/// The IP tool's refusal holds for a drop on a pin: a source name that already has a source, dropped on a pin with the
/// direction source, is refused by EW-CORE-052 in its words; nothing committed, the ghost kept.
#[test]
fn a_second_source_on_a_pin_keeps_ft822s_refusal() {
    let tmp = Tmp::new("ip-pin-source");
    let mut s = with_a_free_button(&tmp);
    // 24V has its source, on 101.
    let source = "24V".to_owned();
    let pin = cp(&s, "-102S3:14");
    arm_on(&mut s, &source, pin.at);
    done(&mut s, "tool.ip_direction", json!({"direction": "source"}));
    // The name has its source, so the tool offered its target; the source's form has its CP at the tail: the ghost
    // goes where that CP lies on the pin.
    let pos = insertion_for(&mut s, pin.at);
    cursor(&mut s, pos);
    assert_eq!(ghost(&s)["connects"], "-102S3:14");
    let before = tables(&s);
    let items = s.tool_overlay().items.len();
    assert_eq!(
        refused(&mut s, "canvas.activate", json!({})),
        "reason-ip-not-placed"
    );
    let words = format!(
        "Not placed: {source} already has a source on page 101; press D for a target, or mark one source as the star"
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
    let mut s = with_a_free_button(&tmp);
    let pin = cp(&s, "-102S3:14");
    arm_on(&mut s, "5V", pin.at);
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    let (_, conns) = added(&before, &s);
    assert_eq!(conns.len(), 1);
    assert_eq!(pot009(&tmp, &mut s), subjects(&[("warning", "-102S3:13")]));
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
    let first = snapshot(&tmp.sample_copy());
    drop(s);
    let mut s2 = shell(&tmp, "102");
    assert!(tables(&s2).connections.iter().any(|c| c.id == conns[0].id));
    done(&mut s2, "file.save", json!({}));
    drop(s2);
    let second = snapshot(&tmp.sample_copy());
    assert_eq!(
        first.iter().map(|f| &f.0).collect::<Vec<_>>(),
        second.iter().map(|f| &f.0).collect::<Vec<_>>()
    );
    for (a, b) in first.iter().zip(&second) {
        assert!(a.1 == b.1, "{} changed on a re-save", a.0.display());
    }
}

/// The next name after a connecting drop. A target "FT950" dropped on the free pin -102S3:14 names the pin's
/// potential (doc 08 §3.5 step 2c); the ghost kept there keeps the name and the chosen direction (the typed name
/// stays while no source feeds it). The Wire tool's P, from that pin,
/// names its end the same way: the source of FT950.
#[test]
fn after_a_connecting_drop_the_next_point_is_the_targets_source() {
    let tmp = Tmp::new("ip-pin-next-name");
    let mut s = with_a_free_button(&tmp);
    let pin = cp(&s, "-102S3:14");
    arm_on(&mut s, "FT950", pin.at);
    done(&mut s, "tool.ip_direction", json!({"direction": "target"}));
    // The target's form has its CP at the tip: the ghost goes where that CP lies on the pin.
    let pos = insertion_for(&mut s, pin.at);
    cursor(&mut s, pos);
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    assert_connects(&s, &before, &pin, "FT950");
    // A target no source feeds names the potential it carries (doc 08 §3.5 step 2c).
    let start = cp(&s, "-102S3:14");
    assert_eq!(
        start.potential,
        Some(TablePotential {
            name: "FT950".to_owned(),
            fallback: false
        })
    );
    // The ghost stays where it was dropped: its CP on the pin and the new point's. Target was chosen, and a chosen
    // direction holds until the person chooses another (a persona run's marks all turned source after a target).
    let pos = ghost_pos(&s);
    cursor(&mut s, pos);
    assert_eq!(
        (ghost(&s)["name"].as_str(), ghost(&s)["direction"].as_str()),
        (Some("FT950"), Some("target"))
    );
    // The Wire tool's P from the pin: the end is the name's partner too, its source (the typed name still stands).
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
    assert_eq!((ip.name.as_str(), ip.direction.as_str()), ("FT950", "source"));
}

/// A pin on a junction wins: a free contact moved so that its pin lies on a junction of
/// 102, the point dropped there connects to the pin as on any pin.
#[test]
fn a_pin_on_a_junction_wins() {
    let tmp = Tmp::new("ip-pin-junction");
    let mut s = shell(&tmp, "102");
    let t = tables(&s);
    let junction = t
        .junctions
        .iter()
        .map(|j| j.at)
        .filter(|at| !t.cps.iter().any(|c| c.at == *at))
        .find(|at| {
            arm_on(&mut s, "J1074", *at);
            ghost_cp(&s) == *at
        })
        .expect("a junction on 102 off the pins where the ghost's CP can lie");
    done(&mut s, "tool.select", json!({}));
    // A contact placed free (Alt), then moved by a command so that its first pin lies on the junction.
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
    );
    let free = grid_point(&s, (40, 40));
    cursor(&mut s, free);
    done(&mut s, "canvas.activate_alt", json!({}));
    done(&mut s, "tool.select", json!({}));
    let (reps, conns) = added(&t, &s);
    assert_eq!((reps.len(), conns.len()), (1, 0), "a contact, unconnected");
    let pin = tables(&s)
        .cps
        .iter()
        .find(|c| c.rep == reps[0].id)
        .expect("the contact's pin")
        .clone();
    let steps = {
        let sn = &s.canvas().unwrap().page().snapper;
        [
            sn.grid_index_of(junction.0) - sn.grid_index_of(pin.at.0),
            sn.grid_index_of(junction.1) - sn.grid_index_of(pin.at.1),
        ]
    };
    done(
        &mut s,
        "command.run",
        json!({"cmd": "rep.move", "params": {"reps": [pin.rep.to_string()], "steps": steps}}),
    );
    let pin = cp(&s, &pin.label);
    assert_eq!(pin.at, junction, "the pin lies on the junction");
    assert!(
        tables(&s).junctions.iter().any(|j| j.at == junction),
        "the junction stays"
    );
    arm_on(&mut s, "J1074", junction);
    assert_eq!(ghost_cp(&s), junction, "the ghost's CP on the junction");
    assert_eq!(
        ghost(&s)["connects"].as_str(),
        Some(pin.label.as_str()),
        "the pin wins"
    );
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    assert_connects(&s, &before, &pin, "J1074");
}

/// A wire's bare end is no wire's inside: on a page where the scene draws a wire
/// whose coil lies off the sheet along the sheet's edge, the drawn end on the edge has no pin and no junction, and a
/// point dropped there places free.
#[test]
fn a_wires_bare_end_places_the_point_free() {
    let tmp = Tmp::new("ip-bare-end");
    let mut s = crate::off_sheet_wire::fixture(&tmp);
    let t = tables(&s);
    let bare: Vec<(i64, i64)> = t
        .connections
        .iter()
        .flat_map(|w| [w.polyline[0], w.polyline[w.polyline.len() - 1]])
        .filter(|p| !t.cps.iter().any(|c| c.at == *p) && !t.junctions.iter().any(|j| j.at == *p))
        .collect();
    assert_eq!(bare.len(), 1, "one bare end: {bare:?}");
    let end = bare[0];
    arm_on(&mut s, "E1074", end);
    assert_eq!(ghost_cp(&s), end, "the ghost's CP on the bare end");
    assert_eq!(ghost(&s)["connects"], Value::Null, "no connection offered");
    let before = tables(&s);
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    let (reps, conns) = added(&before, &s);
    assert_eq!(
        reps.iter()
            .map(|r| r.ip.as_ref().map(|ip| ip.name.as_str()))
            .collect::<Vec<_>>(),
        [Some("E1074")],
        "the point is placed"
    );
    assert!(conns.is_empty(), "free: no connection");
}

/// On the sample's grid of thirds of a nm a point's place plus its pin's offset, each rounded, can add up to 1 nm beside
/// the grid's number of the pin's position; the page draws a pin, and the tool's ghost its point's pin, on that number
/// (docs/design/06a-drafting.md §2.1). So a point dropped on any pin of a routed folio is on it: the ghost connects to
/// it, and the point's pin and the wire's one point are the pin's own number.
#[test]
fn a_point_dropped_on_a_pin_connects_to_it_on_the_pin_s_own_number() {
    let tmp = Tmp::new("ip-pin-one-number");
    let mut s = shell(&tmp, "102");
    done(&mut s, "view.zoom_actual", json!({}));
    let t = tables(&s);
    let pins: Vec<TableCp> = t
        .cps
        .iter()
        .filter(|c| {
            // (A pin whose designation repeats is named otherwise by the ghost.)
            !c.label.contains('#')
                && t.reps.iter().any(|r| r.id == c.rep && r.ip.is_none())
                && t.cps.iter().filter(|o| o.at == c.at).count() == 1
        })
        .cloned()
        .collect();
    assert!(!pins.is_empty(), "pins to drop a point on");
    for pin in &pins {
        arm_on(&mut s, "T1", pin.at);
        assert_eq!(
            ghost(&s)["connects"],
            pin.label.as_str(),
            "the pin under the point"
        );
        done(&mut s, "tool.select", json!({}));
    }
    let pin = &pins[0];
    arm_on(&mut s, "T1", pin.at);
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    let (reps, conns) = added(&before, &s);
    assert_eq!((reps.len(), conns.len()), (1, 1), "the point and its wire");
    let ip_cp = tables(&s)
        .cps
        .iter()
        .find(|c| c.rep == reps[0].id)
        .unwrap()
        .clone();
    assert_eq!(ip_cp.at, pin.at, "the point's pin on the pin's own number");
}
