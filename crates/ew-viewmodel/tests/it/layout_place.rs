// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 3 through the shell on temporary copies of Golden A, folio 902 (the panel page of -UC1): the Unplaced
//! devices navigator (PW-1: rows, badges, empty states, the fetch counter); F55 pushed onto Rail 1 by a drag and from
//! the keyboard (PW-2: SN-3's poses, the ghost's predicted poses equal to the committed ones, one undo step); the
//! refusals of a TS 32 rail, of a rail device off every rail, and E55 on the plate (PW-3); target snapping, Shift and the
//! 1 mm grid (PW-4); -201U9's slot on Rail 3 against `layout.place_assembly` (PW-5, ruling 4); Tab and Shift+Tab, the
//! live region, the nudges, R, X, Delete, Lock and Unlock (PW-6).

use crate::common;
use ew_viewmodel::key;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::actions::ActionItem;
use ew_viewmodel::command_bar::{canvas_menu, hud};
use ew_viewmodel::ew_api::output::{PanelTables, Uuid};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

/// A headless shell with a writable copy of Golden A on folio 902.
fn writable(tmp: &Tmp) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, "902")
}

fn panel(shell: &ShellModel) -> std::sync::Arc<PanelTables> {
    shell.panel_tables().expect("902's panel tables")
}

/// The sheet point of a surface point of the page shown.
fn sheet_of(shell: &ShellModel, p: (i128, i128)) -> (i64, i64) {
    let t = panel(shell);
    shell.canvas().unwrap().page().snapper.page_to_sheet(t.to_page(p))
}

/// The ID of the carrier labelled `label`.
fn carrier(shell: &ShellModel, label: &str) -> Uuid {
    panel(shell)
        .carriers
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no carrier {label}"))
        .id
}

/// The sheet point of the clip point `s` (nm along the axis) of the rail labelled `rail`.
fn on_rail(shell: &ShellModel, rail: &str, s: i64) -> (i64, i64) {
    let t = panel(shell);
    let axis = t.carrier(carrier(shell, rail)).unwrap().axis.unwrap();
    sheet_of(shell, axis.point_at(i128::from(s)))
}

/// The unplaced device labelled `label`.
fn unplaced(shell: &ShellModel, label: &str) -> Uuid {
    panel(shell)
        .unplaced
        .iter()
        .find(|u| u.label == label)
        .unwrap_or_else(|| panic!("{label} is not unplaced"))
        .device
}

/// The placement of the device tagged `tag`.
fn placement(shell: &ShellModel, tag: &str) -> Uuid {
    panel(shell)
        .placements
        .iter()
        .find(|p| p.tag == tag)
        .unwrap_or_else(|| panic!("{tag} is not placed"))
        .id
}

/// The placements on the rail labelled `rail` in carrier order: (tag, clip point in mm, quarter turns).
fn rail_poses(shell: &ShellModel, rail: &str) -> Vec<(String, f64, u8)> {
    let t = panel(shell);
    let id = carrier(shell, rail);
    let mut on: Vec<_> = t.placements.iter().filter(|p| p.carrier == Some(id)).collect();
    on.sort_by_key(|p| i128::from(p.pose.0) - p.clip);
    on.iter()
        .map(|p| (p.tag.clone(), p.pose.0 as f64 / 1e6, p.pose.2))
        .collect()
}

fn rows(expected: &[(&str, f64)]) -> Vec<(String, f64, u8)> {
    expected.iter().map(|(t, s)| ((*t).to_owned(), *s, 0)).collect()
}

/// The committed pose of each predicted item (a device's new placement, or the placement itself), in the order of
/// the prediction.
fn committed(shell: &ShellModel, predicted: &[(Uuid, (i64, i64, u8))]) -> Vec<(Uuid, (i64, i64, u8))> {
    let t = panel(shell);
    predicted
        .iter()
        .map(|(id, _)| {
            let row = t
                .placements
                .iter()
                .find(|p| p.id == *id || p.device == *id)
                .unwrap_or_else(|| panic!("{id} is not placed"));
            (*id, row.pose)
        })
        .collect()
}

/// A click on the footprint of the placement of `tag` (Select on a panel page).
fn select_placement(shell: &mut ShellModel, tag: &str) {
    let t = panel(shell);
    let fp = t.placements.iter().find(|p| p.tag == tag).unwrap().footprint;
    let centre = (
        (i128::from(fp.min().x.0) + i128::from(fp.max().x.0)) / 2,
        (i128::from(fp.min().y.0) + i128::from(fp.max().y.0)) / 2,
    );
    let p = sheet_of(shell, centre);
    common::click(shell, p, json!({}));
    assert_eq!(shell.selected_placements(), 1, "a click on {tag} selects it");
    let id = placement(shell, tag);
    assert_eq!(state(shell)["canvas"]["selection"], json!([id.to_string()]));
}

fn ghost(shell: &ShellModel) -> Value {
    state(shell)["canvas"]["ghost"].clone()
}

/// Dispatches what the canvas's keyboard binding of `action` runs on the page shown (`shortcut_action`).
fn key(shell: &mut ShellModel, action: &'static str) -> Outcome {
    let (name, params) = shell.shortcut_action(action);
    let params = if params.is_null() { json!({}) } else { params };
    shell.dispatch(name, &params)
}

fn undo_all_once(shell: &mut ShellModel) {
    done(shell, "edit.undo", json!({}));
    assert!(!shell.can_undo(), "the gesture was one undo step");
}

const R1_BEFORE: [(&str, f64); 6] = [
    ("-102G1", 96.0),
    ("-101F1", 140.0),
    ("-102F1", 158.0),
    ("-102F2", 189.5),
    ("T55", 228.5),
    ("S51", 267.5),
];

/// SN-3: F55 pushed in between -101F1 and -102F1.
const R1_SN3: [(&str, f64); 7] = [
    ("-102G1", 96.0),
    ("-101F1", 140.0),
    ("F55", 158.0),
    ("-102F1", 176.0),
    ("-102F2", 207.5),
    ("T55", 246.5),
    ("S51", 285.5),
];

// ---------------------------------------------------------------------------------------------------------------
// PW-1

/// PW-1 (FT-486): the rows in `Unplaced` order (ruling 9) with their badges and drag sources; placing F55 removes its
/// row in the same revision; the panel tables are fetched once per revision and folio, never for a schematic page.
#[test]
fn pw1_the_unplaced_devices_navigator_lists_the_five_devices() {
    let tmp = Tmp::new("layout-pw1");
    let mut shell = writable(&tmp);
    done(&mut shell, "navigator.show", json!({"kind": "unplaced"}));
    let shown: Vec<(String, usize, String, Vec<String>)> = shell
        .shown_rows(NavKind::Unplaced)
        .into_iter()
        .map(|r| (r.role.to_owned(), r.level, r.label, r.badges))
        .collect();
    let row = |role: &str, level, label: &str, badges: &[&str]| {
        (
            role.to_owned(),
            level,
            label.to_owned(),
            badges.iter().map(|b| (*b).to_owned()).collect::<Vec<_>>(),
        )
    };
    assert_eq!(
        shown,
        vec![
            row("location", 1, "+J-E-2416", &[]),
            row("device", 2, "-201U9", &["assembly"]),
            row("device", 2, "E55", &[]),
            row("device", 2, "E56", &["unknown size"]),
            row("device", 2, "F55", &[]),
            row("device", 2, "S56", &["unknown size"]),
        ]
    );
    let f55 = unplaced(&shell, "F55");
    let rows = shell.shown_rows(NavKind::Unplaced);
    assert_eq!(
        rows[4].drag,
        Some(ew_viewmodel::shell::DragSource::Device(f55.to_string()))
    );
    assert!(rows[4].openable, "Enter places the device");
    assert!(!rows[0].openable, "Enter toggles the location");

    // The fetch counter: none for a schematic page, one per revision and folio.
    let calls = shell.session_calls().panel_tables;
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    assert_eq!(shell.session_calls().panel_tables, calls);
    assert!(shell.panel_tables().is_none());
    done(&mut shell, "page.goto", json!({"folio": "902"}));
    let back = shell.session_calls().panel_tables;
    assert_eq!(back, calls, "the same revision and folio: no new fetch");
    done(&mut shell, "view.zoom_in", json!({}));
    assert_eq!(
        shell.session_calls().panel_tables,
        back,
        "no fetch without a new revision"
    );

    // Placing F55 removes its row in the same revision.
    let r1 = carrier(&shell, "Rail 1").to_string();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "layout.place", "params": {
            "device": f55.to_string(),
            "carrier": {"carrier": r1},
            "pose": {"t": [158 * MM, 0, 0], "q": [1_000_000_000, 0, 0, 0]},
            "mode": "push",
        }}),
    );
    assert_eq!(
        shell.session_calls().panel_tables,
        back + 1,
        "one fetch for the new revision"
    );
    let labels: Vec<String> = shell
        .shown_rows(NavKind::Unplaced)
        .into_iter()
        .map(|r| r.label)
        .collect();
    assert_eq!(labels, ["+J-E-2416", "-201U9", "E55", "E56", "S56"]);
    assert_eq!(rail_poses(&shell, "Rail 1"), rows_of(&R1_SN3));
    shell.shutdown();
}

fn rows_of(expected: &[(&str, f64)]) -> Vec<(String, f64, u8)> {
    rows(expected)
}

/// FT-882 (ID-1108: "`ui.describe` names a surface view \"surface view\""): folio 902's view is described as one,
/// never as an unknown item.
#[test]
fn ft882_describe_names_the_surface_view() {
    let tmp = Tmp::new("layout-ft882");
    let mut shell = writable(&tmp);
    done(&mut shell, "page.describe", json!({"folio": "902"}));
    let text = shell.take_page_text().expect("the page text");
    let items: Vec<&str> = text
        .lines()
        .filter(|l| l.trim_start().starts_with("item "))
        .map(|l| l.split(" cell ").next().unwrap_or(l))
        .collect();
    assert_eq!(items, ["  item surface view"]);
    for token in common::structural_tokens(items[0]) {
        assert!(
            ew_viewmodel::describe::VOCABULARY.contains(&token.as_str()),
            "`{token}` is a vocabulary word"
        );
    }
    shell.shutdown();
}

/// PW-1's empty states (doc 16a §6.17), whole: every device placed names the location; a project without a layout
/// space says so.
#[test]
fn pw1_the_empty_states_name_the_location_or_the_missing_layout() {
    let tmp = Tmp::new("layout-pw1-empty");
    let mut shell = writable(&tmp);
    done(&mut shell, "navigator.show", json!({"kind": "unplaced"}));
    let surface = panel(&shell).surface.to_string();
    let r3 = carrier(&shell, "Rail 3").to_string();
    let r1 = carrier(&shell, "Rail 1").to_string();
    let one = [1_000_000_000_i64, 0, 0, 0];
    let place = |device: Uuid, carrier: Value, t: [i64; 3], mode: Option<&str>| {
        let mut p = json!({"device": device.to_string(), "carrier": carrier, "pose": {"t": t, "q": one}});
        if let Some(m) = mode {
            p["mode"] = json!(m);
        }
        json!({"cmd": "layout.place", "params": p})
    };
    let cmds = [
        place(
            unplaced(&shell, "F55"),
            json!({"carrier": r1}),
            [158 * MM, 0, 0],
            Some("push"),
        ),
        place(
            unplaced(&shell, "-201U9"),
            json!({"carrier": r3}),
            [338 * MM, 0, 0],
            Some("push"),
        ),
        place(
            unplaced(&shell, "E55"),
            json!({"surface": surface}),
            [520 * MM, 40 * MM, 0],
            None,
        ),
        place(
            unplaced(&shell, "E56"),
            json!({"surface": surface}),
            [560 * MM, 300 * MM, 0],
            None,
        ),
        place(
            unplaced(&shell, "S56"),
            json!({"surface": surface}),
            [600 * MM, 300 * MM, 0],
            None,
        ),
    ];
    for c in cmds {
        done(&mut shell, "command.run", c);
    }
    assert!(
        shell.shown_rows(NavKind::Unplaced).is_empty(),
        "the location row goes with its last device"
    );
    assert_eq!(
        shell.unplaced_empty(),
        (
            "All devices placed".to_owned(),
            "Every device of +J-E-2416 is placed.".to_owned()
        )
    );
    shell.shutdown();

    let fresh = Tmp::new("layout-pw1-none");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ew_viewmodel::shell::ShellConfig::in_dir(&fresh.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut shell,
        "file.new",
        json!({"dir": fresh.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(&mut shell, "navigator.show", json!({"kind": "unplaced"}));
    assert!(shell.shown_rows(NavKind::Unplaced).is_empty());
    assert_eq!(
        shell.unplaced_empty(),
        (
            "No panel layout yet".to_owned(),
            "Devices are listed here once the location has an enclosure.".to_owned()
        )
    );
    assert_eq!(
        refused(
            &mut shell,
            "tool.place_layout",
            json!({"device": "00000000-0000-0000-0000-000000000001"})
        ),
        "reason-no-panel-page"
    );
    assert_eq!(
        ew_viewmodel::strings::text(key!("reason-no-panel-page")),
        "Open a panel layout page"
    );
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// PW-2

/// PW-2: F55 dragged from the navigator onto Rail 1 between -101F1 and -102F1 with Ctrl held: the ghost moves four
/// neighbours, the predicted poses are the committed ones (SN-3), and one undo restores all five.
#[test]
fn pw2_f55_pushed_onto_rail_1_by_a_drag() {
    let tmp = Tmp::new("layout-pw2");
    let mut shell = writable(&tmp);
    let f55 = unplaced(&shell, "F55");
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": f55.to_string()}),
    );
    assert_eq!(shell.tool().name(), "layout_place");
    assert_eq!(
        hud(&shell).prompt,
        "Place F55 · Ctrl pushes · Alt swaps · Shift no snap · Esc done"
    );
    let drop = on_rail(&shell, "Rail 1", 158 * MM);
    done(&mut shell, "canvas.pointer", at(drop, json!({"ctrl": true})));
    assert_eq!(
        ghost(&shell),
        json!({"kind": "layout", "device": "F55", "carrier": "Rail 1", "mode": "push", "at_nm": [149 * MM],
            "moved": 4, "refused": null})
    );
    assert!(
        shell.layout_options_shown(),
        "the options card shows while placing"
    );
    assert_eq!(
        shell.layout_card_options().mode,
        ew_viewmodel::ew_api::output::SnapMode::Push,
        "the card shows the mode Ctrl holds"
    );
    assert_eq!(
        shell.layout_options().mode,
        ew_viewmodel::ew_api::output::SnapMode::Free,
        "the sticky mode stays"
    );
    let predicted = shell.layout_predicted_poses();
    let tags = ["-102F1", "-102F2", "T55", "S51"].map(|t| placement(&shell, t));
    let expected: Vec<(Uuid, (i64, i64, u8))> = std::iter::once((f55, (158 * MM, 0, 0)))
        .chain(
            tags.iter()
                .zip([176_000_000, 207_500_000, 246_500_000, 285_500_000])
                .map(|(p, s)| (*p, (s, 0, 0))),
        )
        .collect();
    assert_eq!(predicted, expected);
    done(&mut shell, "canvas.release", at(drop, json!({"ctrl": true})));
    assert_eq!(rail_poses(&shell, "Rail 1"), rows_of(&R1_SN3));
    assert_eq!(
        committed(&shell, &predicted),
        predicted,
        "the ghost and the commit agree (R1)"
    );
    assert_eq!(shell.tool().name(), "select", "the tool returns to Select");
    assert!(ghost(&shell).is_null());
    // FT-900 (ID-1147): the placement the drop made is the selection.
    let f55_placed = placement(&shell, "F55");
    assert_eq!(
        state(&shell)["canvas"]["selection"],
        json!([f55_placed.to_string()])
    );
    undo_all_once(&mut shell);
    assert_eq!(rail_poses(&shell, "Rail 1"), rows_of(&R1_BEFORE));
    assert_eq!(unplaced(&shell, "F55"), f55);
    shell.shutdown();
}

/// PW-2 from the keyboard: Enter on the F55 row floats it at the start of Rail 1, the arrows step it by snap step,
/// Enter drops it; with Push as the sticky mode the poses are SN-3's, in one undo step.
#[test]
fn pw2_f55_pushed_onto_rail_1_from_the_keyboard() {
    let tmp = Tmp::new("layout-pw2-keys");
    let mut shell = writable(&tmp);
    done(&mut shell, "navigator.show", json!({"kind": "unplaced"}));
    done(&mut shell, "layout.options", json!({"mode": "push"}));
    let key_of_f55 = shell
        .shown_rows(NavKind::Unplaced)
        .into_iter()
        .find(|r| r.label == "F55")
        .unwrap()
        .key;
    done(
        &mut shell,
        "navigator.open",
        json!({"kind": "unplaced", "key": key_of_f55}),
    );
    assert_eq!(shell.tool().name(), "layout_place");
    assert_eq!(
        ghost(&shell)["carrier"],
        "Rail 1",
        "it floats on the first rail it fits"
    );
    assert_eq!(ghost(&shell)["at_nm"], json!([0]));
    let f55 = unplaced(&shell, "F55");
    let target = (f55, (158 * MM, 0, 0));
    let mut steps = 0;
    while shell.layout_predicted_poses().first() != Some(&target) {
        assert_eq!(key(&mut shell, "cursor.right"), Outcome::Done);
        steps += 1;
        assert!(steps < 100, "the arrows never reached 149 mm: {}", ghost(&shell));
    }
    // One step back and forth lands on the same snap target.
    assert_eq!(key(&mut shell, "cursor.left"), Outcome::Done);
    assert_ne!(shell.layout_predicted_poses().first(), Some(&target));
    assert_eq!(key(&mut shell, "cursor.right"), Outcome::Done);
    let predicted = shell.layout_predicted_poses();
    assert_eq!(predicted.len(), 5, "four neighbours move");
    done(&mut shell, "canvas.activate", json!({}));
    assert_eq!(rail_poses(&shell, "Rail 1"), rows_of(&R1_SN3));
    assert_eq!(committed(&shell, &predicted), predicted);
    undo_all_once(&mut shell);
    assert_eq!(rail_poses(&shell, "Rail 1"), rows_of(&R1_BEFORE));
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// PW-3

/// PW-3: F55 over a TS 32 rail shows the command's refusal in the chip and commits nothing; F55 away from every rail
/// gets the tool's text; E55 (screw-mounted) lands on the plate at the snapped point.
#[test]
fn pw3_refused_drops_and_a_plate_placement() {
    let tmp = Tmp::new("layout-pw3");
    let mut shell = writable(&tmp);
    // The TS 32 rail fixture, below Rail 3.
    let surface = panel(&shell).surface.to_string();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "layout.carrier.add", "params": {
            "surface": surface,
            "kind": "rail",
            "path": [[40 * MM, 60 * MM], [610 * MM, 60 * MM]],
            "params": {"rail": {"profile": "ts32"}},
        }}),
    );
    let ts32 = panel(&shell)
        .carriers
        .iter()
        .find(|c| c.label == "Rail 4")
        .expect("the TS 32 rail")
        .label
        .clone();
    let revision = shell.revision();
    let f55 = unplaced(&shell, "F55");
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": f55.to_string()}),
    );
    let over = on_rail(&shell, &ts32, 100 * MM);
    done(&mut shell, "canvas.pointer", at(over, json!({})));
    let g = ghost(&shell);
    assert_eq!(g["carrier"], "Rail 4");
    assert_eq!(g["refused"], "F55 (TS 35×7.5) does not fit a TS 32 rail");
    assert_eq!(
        shell.tool_overlay().items.len(),
        1,
        "only the chip; the outline and cross are the conflict's"
    );
    assert_eq!(
        refused(&mut shell, "canvas.release", at(over, json!({}))),
        "reason-layout-refused"
    );
    assert_eq!(shell.revision(), revision, "nothing committed");

    // Away from every rail.
    let away = sheet_of(&shell, (300 * MM as i128, 640 * MM as i128));
    done(&mut shell, "canvas.pointer", at(away, json!({})));
    let g = ghost(&shell);
    assert_eq!(g["carrier"], "plate");
    assert_eq!(g["refused"], "F55 mounts on a TS 35×7.5 rail; drop it on a rail.");
    assert_eq!(
        refused(&mut shell, "canvas.release", at(away, json!({}))),
        "reason-layout-refused"
    );
    assert_eq!(shell.revision(), revision);
    done(&mut shell, "ui.dismiss", json!({}));

    // E55 on the plate.
    let e55 = unplaced(&shell, "E55");
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": e55.to_string()}),
    );
    let spot = sheet_of(&shell, (541_300_000, 77_700_000));
    done(&mut shell, "canvas.pointer", at(spot, json!({})));
    let g = ghost(&shell);
    assert_eq!(g["carrier"], "plate");
    assert_eq!(g["refused"], Value::Null);
    let predicted = shell.layout_predicted_poses();
    assert_eq!(predicted.len(), 1);
    let (u, v, _) = predicted[0].1;
    assert_eq!((u % (5 * MM), v % (5 * MM)), (0, 0), "on the 5 mm grid: {u}, {v}");
    done(&mut shell, "canvas.release", at(spot, json!({})));
    assert_eq!(committed(&shell, &predicted), predicted);
    let row = panel(&shell)
        .placements
        .iter()
        .find(|p| p.device == e55)
        .unwrap()
        .clone();
    assert_eq!(row.carrier, None, "a surface placement");
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// PW-4

/// PW-4: near -102F2's end the left edge snaps to it (within 15 px, nearer than the grid point); Shift suspends the
/// snap (0.1 mm); the 1 mm option changes the grid step.
#[test]
fn pw4_target_snapping_shift_and_the_grid_option() {
    let tmp = Tmp::new("layout-pw4");
    let mut shell = writable(&tmp);
    let f55 = unplaced(&shell, "F55");
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": f55.to_string()}),
    );
    // The left edge at 212.4 mm: -102F2 ends at 212 mm, the grid point is 210 mm.
    let near = on_rail(&shell, "Rail 1", 221_400_000);
    done(&mut shell, "canvas.pointer", at(near, json!({"ctrl": true})));
    assert_eq!(ghost(&shell)["at_nm"], json!([212 * MM]));
    done(
        &mut shell,
        "canvas.pointer",
        at(near, json!({"ctrl": true, "shift": true})),
    );
    assert_eq!(ghost(&shell)["at_nm"], json!([212_400_000]));
    // On the free end of the rail, 402.4 mm: the 5 mm grid gives 400, the 1 mm grid 402.
    let free = on_rail(&shell, "Rail 1", 411_400_000);
    done(&mut shell, "canvas.pointer", at(free, json!({})));
    assert_eq!(ghost(&shell)["at_nm"], json!([400 * MM]));
    assert_eq!(ghost(&shell)["mode"], "free");
    done(&mut shell, "layout.options", json!({"grid": 1}));
    done(&mut shell, "canvas.pointer", at(free, json!({})));
    assert_eq!(ghost(&shell)["at_nm"], json!([402 * MM]));
    assert_eq!(shell.layout_options().grid_mm, 1);
    let predicted = shell.layout_predicted_poses();
    done(&mut shell, "canvas.release", at(free, json!({})));
    assert_eq!(committed(&shell, &predicted), predicted);
    assert_eq!(predicted, vec![(f55, (411 * MM, 0, 0))]);
    assert_eq!(
        shell.dispatch("layout.options", &json!({"grid": 2})),
        Outcome::Failed {
            message: shell_failed_text("grid")
        }
    );
    shell.shutdown();
}

fn shell_failed_text(param: &str) -> String {
    ew_viewmodel::strings::format(key!("error-bad-param"), &[param])
}

// ---------------------------------------------------------------------------------------------------------------
// PW-5

/// PW-5: -201U9 dropped on Rail 3 without a modifier defaults to push at its member-order slot and gives AS-1's poses
/// in one undo step; ruling 4: the whole rail equals what `layout.place_assembly` gives on another copy.
#[test]
fn pw5_an_assembly_member_defaults_to_push_at_its_slot() {
    let tmp = Tmp::new("layout-pw5");
    let mut shell = writable(&tmp);
    let u9 = unplaced(&shell, "-201U9");
    done(&mut shell, "tool.place_layout", json!({"device": u9.to_string()}));
    let drop = on_rail(&shell, "Rail 3", 520 * MM);
    done(&mut shell, "canvas.pointer", at(drop, json!({})));
    let g = ghost(&shell);
    assert_eq!(g["mode"], "push");
    assert_eq!(g["moved"], 3);
    let predicted = shell.layout_predicted_poses();
    done(&mut shell, "canvas.release", at(drop, json!({})));
    assert_eq!(committed(&shell, &predicted), predicted);
    let r3 = rail_poses(&shell, "Rail 3");
    let tail: Vec<(String, f64, u8)> = r3[r3.len() - 4..].to_vec();
    assert_eq!(
        tail,
        rows_of(&[
            ("-201U9", 338.0),
            ("-202U1", 373.0),
            ("-202U2", 418.0),
            ("-202U3", 448.0)
        ])
    );
    undo_all_once(&mut shell);
    assert_eq!(unplaced(&shell, "-201U9"), u9);
    shell.shutdown();

    // Ruling 4: the same rail from `layout.place_assembly`.
    let other = Tmp::new("layout-pw5-assembly");
    let mut twin = writable(&other);
    let rail3 = carrier(&twin, "Rail 3").to_string();
    done(
        &mut twin,
        "command.run",
        json!({"cmd": "layout.place_assembly", "params": {
            "device": u9.to_string(),
            "carrier": {"carrier": rail3},
            "mode": "push",
        }}),
    );
    assert_eq!(rail_poses(&twin, "Rail 3"), r3);
    twin.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// PW-6

/// PW-6: Tab and Shift+Tab cycle Rail 1's placements in carrier order with the live region's text; the HUD of a
/// selected placement.
#[test]
fn pw6_tab_cycles_the_rail_with_the_live_region() {
    let tmp = Tmp::new("layout-pw6-tab");
    let mut shell = writable(&tmp);
    let mut seen = Vec::new();
    for _ in 0..7 {
        assert_eq!(key(&mut shell, "tool.next_proposal"), Outcome::Done);
        seen.push(shell.layout_live_text().unwrap());
    }
    assert_eq!(
        seen,
        [
            "-102G1, rail 1 at 96 mm, no findings",
            "-101F1, rail 1 at 140 mm, no findings",
            "-102F1, rail 1 at 158 mm, no findings",
            "-102F2, rail 1 at 189.5 mm, no findings",
            "T55, rail 1 at 228.5 mm, 2 findings",
            "S51, rail 1 at 267.5 mm, 1 finding",
            "-102G1, rail 1 at 96 mm, no findings",
        ]
    );
    assert_eq!(
        shell.tool_announcement(),
        Some("-102G1, rail 1 at 96 mm, no findings")
    );
    done(&mut shell, "layout.previous", json!({}));
    assert_eq!(
        shell.layout_live_text().as_deref(),
        Some("S51, rail 1 at 267.5 mm, 1 finding")
    );
    // FT-996: on a rail only X turns it, so R goes unnamed.
    assert_eq!(
        hud(&shell).prompt,
        "1 selected · arrows nudge · X flips · Delete deletes"
    );
    // Enter with a placement selected focuses Properties.
    done(&mut shell, "canvas.activate", json!({}));
    assert_eq!(shell.focus(), Some(ew_viewmodel::shell::Focus::Properties));
    shell.shutdown();
}

/// PW-6: the arrows nudge (free stops with "Blocked by …", push pushes); R is refused on a rail in words, X flips;
/// Lock and Unlock in the context menu; Delete removes the placement.
#[test]
fn pw6_nudge_turn_lock_and_delete_a_selected_placement() {
    let tmp = Tmp::new("layout-pw6-edit");
    let mut shell = writable(&tmp);
    let f1 = placement(&shell, "-102F1");
    select_placement(&mut shell, "-102F1");
    // Free: -102F1 touches -102F2.
    assert_eq!(
        key(&mut shell, "cursor.right"),
        Outcome::Refused {
            reason_key: key!("reason-layout-blocked")
        }
    );
    assert_eq!(shell.tool_announcement(), Some("Blocked by -102F2"));
    assert_eq!(
        key(&mut shell, "cursor.left"),
        Outcome::Refused {
            reason_key: key!("reason-layout-blocked")
        }
    );
    assert_eq!(shell.tool_announcement(), Some("Blocked by -101F1"));
    // S51 is free to its right.
    select_placement(&mut shell, "S51");
    assert_eq!(key(&mut shell, "cursor.right"), Outcome::Done);
    assert_eq!(
        shell.tool_announcement(),
        Some("S51, rail 1 at 272.5 mm, 1 finding")
    );
    // Push: -102F1 pushes -102F2, T55 and S51 by one grid step.
    done(&mut shell, "layout.options", json!({"mode": "push"}));
    select_placement(&mut shell, "-102F1");
    assert_eq!(key(&mut shell, "cursor.right"), Outcome::Done);
    assert_eq!(
        rail_poses(&shell, "Rail 1"),
        rows_of(&[
            ("-102G1", 96.0),
            ("-101F1", 140.0),
            ("-102F1", 163.0),
            ("-102F2", 194.5),
            ("T55", 233.5),
            ("S51", 272.5),
        ])
    );
    // R on a rail is refused in words with the keymap's keys; X flips.
    assert_eq!(
        key(&mut shell, "tool.rotate_cw"),
        Outcome::Refused {
            reason_key: key!("reason-layout-turn")
        }
    );
    assert_eq!(
        shell.tool_announcement(),
        Some("R turns a placement on the plate; X flips one on a rail.")
    );
    assert_eq!(key(&mut shell, "tool.mirror_x"), Outcome::Done);
    assert_eq!(panel(&shell).placement(f1).unwrap().pose.2, 2);
    // Lock, then the menu offers Unlock.
    let labels = |shell: &ShellModel| -> Vec<(String, &'static str)> {
        canvas_menu(shell, true)
            .into_iter()
            .map(|i: ActionItem| (i.label, i.id))
            .filter(|(_, id)| id.starts_with("layout.") || *id == "edit.delete_selection")
            .collect()
    };
    assert!(labels(&shell).contains(&("Lock".to_owned(), "layout.lock_selection")));
    done(&mut shell, "layout.lock_selection", json!({}));
    assert!(panel(&shell).placement(f1).unwrap().locked);
    assert!(labels(&shell).contains(&("Unlock".to_owned(), "layout.unlock_selection")));
    done(&mut shell, "layout.unlock_selection", json!({}));
    assert!(!panel(&shell).placement(f1).unwrap().locked);
    // Delete: the placement goes, its device is unplaced again.
    let device = panel(&shell).placement(f1).unwrap().device;
    done(&mut shell, "edit.delete_selection", json!({}));
    assert!(panel(&shell).placement(f1).is_none());
    assert_eq!(unplaced(&shell, "-102F1"), device);
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// Cycle-42 review part 3, finding 1

/// The sheet point of the centre of the footprint of the placement of `tag`.
fn footprint_centre(shell: &ShellModel, tag: &str) -> (i64, i64) {
    let t = panel(shell);
    let fp = t.placements.iter().find(|p| p.tag == tag).unwrap().footprint;
    sheet_of(
        shell,
        (
            (i128::from(fp.min().x.0) + i128::from(fp.max().x.0)) / 2,
            (i128::from(fp.min().y.0) + i128::from(fp.max().y.0)) / 2,
        ),
    )
}

/// A refused turn names the keymap's keys on every path: with `tool.rotate_cw` rebound to Q and `tool.mirror_x` to K,
/// the rotate key on a rail reads the same whole text from `layout.rotate` on the selected placement (the shell), from
/// the layout place tool's floating ghost and from Select's drag of a placement (the tools); the reason itself names
/// no keys.
#[test]
fn a_refused_turn_names_the_rebound_keys_on_every_path() {
    // ID-1106: the second key comes from `tool.mirror_x`, rebound here to K.
    const TEXT: &str = "Q turns a placement on the plate; K flips one on a rail.";
    let tmp = Tmp::new("layout-turn-keys");
    let mut shell = writable(&tmp);
    assert_eq!(
        ew_viewmodel::strings::text(key!("reason-layout-turn")),
        "Turn a placement on the plate; flip one on a rail"
    );
    done(&mut shell, "help.shortcuts", json!({}));
    done(
        &mut shell,
        "keymap.record_start",
        json!({"action": "tool.rotate_cw"}),
    );
    done(
        &mut shell,
        "keymap.record",
        json!({"action": "tool.rotate_cw", "keys": "Q"}),
    );
    done(
        &mut shell,
        "keymap.record_start",
        json!({"action": "tool.mirror_x"}),
    );
    done(
        &mut shell,
        "keymap.record",
        json!({"action": "tool.mirror_x", "keys": "K"}),
    );
    done(&mut shell, "ui.dismiss", json!({}));
    let turn_refused = Outcome::Refused {
        reason_key: key!("reason-layout-turn"),
    };

    // The shell: the selected placement -102F1 on Rail 1.
    select_placement(&mut shell, "-102F1");
    assert_eq!(key(&mut shell, "tool.rotate_cw"), turn_refused);
    assert_eq!(shell.tool_notice(), Some(TEXT));
    assert_eq!(shell.tool_announcement(), Some(TEXT));
    done(&mut shell, "ui.dismiss", json!({}));

    // The layout place tool: F55's ghost over Rail 1.
    let f55 = unplaced(&shell, "F55");
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": f55.to_string()}),
    );
    let over = on_rail(&shell, "Rail 1", 120 * MM);
    done(&mut shell, "canvas.pointer", at(over, json!({})));
    assert_eq!(ghost(&shell)["carrier"], "Rail 1");
    assert_eq!(key(&mut shell, "tool.rotate_cw"), turn_refused);
    assert_eq!(shell.tool_notice(), Some(TEXT));
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "tool.select", json!({}));
    assert_eq!(shell.tool().name(), "select");

    // The Select tool: -102F1 dragged along Rail 1.
    let revision = shell.revision();
    let from = footprint_centre(&shell, "-102F1");
    done(&mut shell, "canvas.press", at(from, json!({})));
    let to = on_rail(&shell, "Rail 1", 300 * MM);
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    assert_eq!(ghost(&shell)["kind"], "layout", "{}", ghost(&shell));
    assert_eq!(key(&mut shell, "tool.rotate_cw"), turn_refused);
    assert_eq!(shell.tool_notice(), Some(TEXT));
    assert_eq!(shell.revision(), revision, "nothing committed");
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// Cycle 46 (ID-1147): the placing tools' polish rows

/// The whole text of a refused turn with the default keymap.
const TURN_TEXT: &str = "R turns a placement on the plate; X flips one on a rail.";

/// Moves the pointer off every placement and rail, so the HUD names no pick stack.
fn rest_away(shell: &mut ShellModel) {
    let away = sheet_of(shell, (300 * MM as i128, 640 * MM as i128));
    done(shell, "canvas.pointer", at(away, json!({})));
    assert_eq!(hud(shell).stack, 0);
}

/// Presses `action` until it is refused (at most 200 times) and returns how many presses moved.
fn press_until_refused(shell: &mut ShellModel, action: &'static str) -> usize {
    for n in 0..200 {
        match key(shell, action) {
            Outcome::Done => {}
            Outcome::Refused { reason_key } => {
                assert_eq!(reason_key, key!("reason-layout-blocked"), "after {n} presses");
                return n;
            }
            other => panic!("{action}: {other:?}"),
        }
    }
    panic!("{action} never stopped: {}", ghost(shell));
}

/// FT-991 (ID-1147): a refused R keeps the layout ghost, its overlay and its gesture with the refusal's words, for the
/// layout place tool's ghost and for Select's drag of a placement alike; X then flips the same gesture. Other refusals
/// still end their ghost.
#[test]
fn ft991_a_refused_turn_keeps_the_ghost_and_its_gesture() {
    let tmp = Tmp::new("layout-ft991");
    let mut shell = writable(&tmp);
    let turn_refused = Outcome::Refused {
        reason_key: key!("reason-layout-turn"),
    };
    let f55 = unplaced(&shell, "F55");
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": f55.to_string()}),
    );
    let over = on_rail(&shell, "Rail 1", 400 * MM);
    done(&mut shell, "canvas.pointer", at(over, json!({})));
    let floating = ghost(&shell);
    let overlay = shell.tool_overlay().clone();
    assert!(!overlay.items.is_empty());
    assert_eq!(key(&mut shell, "tool.rotate_cw"), turn_refused);
    assert_eq!(shell.tool_notice(), Some(TURN_TEXT));
    assert_eq!(ghost(&shell), floating, "the ghost stays");
    assert_eq!(shell.tool_overlay(), &overlay, "and so does its overlay");
    assert_eq!(shell.tool().state(), "placing");
    assert_eq!(key(&mut shell, "tool.mirror_x"), Outcome::Done);
    assert!(!shell.tool_overlay().items.is_empty());
    assert_eq!(shell.tool_notice(), None);

    // Select's drag of -102F1 along Rail 1: R is refused, the drag goes on and its release commits.
    done(&mut shell, "tool.select", json!({}));
    let revision = shell.revision();
    let from = footprint_centre(&shell, "-102F1");
    done(&mut shell, "canvas.press", at(from, json!({})));
    let to = on_rail(&shell, "Rail 1", 400 * MM);
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    let dragged = ghost(&shell);
    assert_eq!(dragged["kind"], "layout");
    let overlay = shell.tool_overlay().clone();
    assert_eq!(key(&mut shell, "tool.rotate_cw"), turn_refused);
    assert_eq!(shell.tool_notice(), Some(TURN_TEXT));
    assert_eq!(ghost(&shell), dragged);
    assert_eq!(shell.tool_overlay(), &overlay);
    done(&mut shell, "canvas.release", at(to, json!({})));
    assert_eq!(shell.revision(), revision.map(|r| r + 1), "the drag committed");

    // Another refusal ends its ghost: F55 released away from every rail.
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": f55.to_string()}),
    );
    let away = sheet_of(&shell, (300 * MM as i128, 640 * MM as i128));
    done(&mut shell, "canvas.pointer", at(away, json!({})));
    assert!(!shell.tool_overlay().items.is_empty());
    assert_eq!(
        refused(&mut shell, "canvas.release", at(away, json!({}))),
        "reason-layout-refused"
    );
    assert!(
        shell.tool_overlay().items.is_empty(),
        "the refused drop's ghost goes"
    );
    shell.shutdown();
}

/// FT-992 (ID-1147): the arrows stop a floating ghost as they stop a selected placement: at the rail's start and end
/// with "At the rail end" and at a neighbour with "Blocked by -102G1", the ghost staying on Rail 1; a selected
/// placement stops at the rail end in the same words.
#[test]
fn ft992_the_arrows_stop_a_floating_ghost_at_the_rail_end_and_at_a_neighbour() {
    let tmp = Tmp::new("layout-ft992");
    let mut shell = writable(&tmp);
    let f55 = unplaced(&shell, "F55");
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": f55.to_string()}),
    );
    // Armed from the keyboard, it floats at the start of Rail 1.
    assert_eq!(ghost(&shell)["carrier"], "Rail 1");
    assert_eq!(ghost(&shell)["at_nm"], json!([0]));
    assert_eq!(press_until_refused(&mut shell, "cursor.left"), 0);
    assert_eq!(shell.tool_notice(), Some("At the rail end"));
    assert_eq!(shell.tool_announcement(), Some("At the rail end"));
    assert_eq!(ghost(&shell)["at_nm"], json!([0]), "the ghost stays");
    // Right: up to -102G1, then blocked by it.
    assert!(press_until_refused(&mut shell, "cursor.right") > 0);
    assert_eq!(shell.tool_notice(), Some("Blocked by -102G1"));
    assert_eq!(ghost(&shell)["carrier"], "Rail 1");
    assert_eq!(ghost(&shell)["refused"], Value::Null, "it stops touching -102G1");
    // From the free end of the rail: right to the end, never onto the plate.
    let free = on_rail(&shell, "Rail 1", 400 * MM);
    done(&mut shell, "canvas.pointer", at(free, json!({})));
    assert!(press_until_refused(&mut shell, "cursor.right") > 0);
    assert_eq!(shell.tool_notice(), Some("At the rail end"));
    assert_eq!(ghost(&shell)["carrier"], "Rail 1");
    assert_eq!(ghost(&shell)["refused"], Value::Null);
    let predicted = shell.layout_predicted_poses();
    done(&mut shell, "canvas.activate", json!({}));
    assert_eq!(
        committed(&shell, &predicted),
        predicted,
        "it drops where it stopped"
    );

    // The selected placement S51 stops at the rail end in the same words (F55's drop undone).
    done(&mut shell, "edit.undo", json!({}));
    select_placement(&mut shell, "S51");
    assert!(press_until_refused(&mut shell, "cursor.right") > 0);
    assert_eq!(shell.tool_notice(), Some("At the rail end"));
    assert_eq!(shell.tool_announcement(), Some("At the rail end"));
    shell.shutdown();
}

/// FT-993 (ID-1147): Escape on a layout ghost drops it and returns to Select, as the schematic Place tool does; the
/// hint says so.
#[test]
fn ft993_escape_drops_the_layout_ghost_and_returns_to_select() {
    let tmp = Tmp::new("layout-ft993");
    let mut shell = writable(&tmp);
    assert_eq!(
        ew_viewmodel::strings::format(key!("hint-layout-place"), &["Escape"]),
        "Click or release to place; Escape ends"
    );
    let f55 = unplaced(&shell, "F55");
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": f55.to_string()}),
    );
    let over = on_rail(&shell, "Rail 1", 400 * MM);
    done(&mut shell, "canvas.pointer", at(over, json!({})));
    assert_eq!(ghost(&shell)["carrier"], "Rail 1");
    let revision = shell.revision();
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().name(), "select");
    assert!(ghost(&shell).is_null());
    assert!(shell.tool_overlay().items.is_empty());
    assert_eq!(shell.revision(), revision, "nothing placed");
    assert_eq!(unplaced(&shell, "F55"), f55);
    shell.shutdown();
}

/// FT-994 (ID-1147): at grid 1 mm an arrow moves a plate ghost exactly one grid step from where it is drawn, at every
/// turn: E55 floating 30 mm right of -201U7's, -102G1's and -X1032's footprints, level with their tops, where an edge
/// of a neighbour snaps it off the grid (a step from the pointer re-snapped it 0.5 mm further, or short).
#[test]
fn ft994_an_arrow_steps_a_turned_plate_ghost_one_grid_step() {
    let tmp = Tmp::new("layout-ft994");
    let mut shell = writable(&tmp);
    done(&mut shell, "layout.options", json!({"grid": 1}));
    let e55 = unplaced(&shell, "E55");
    let t = panel(&shell);
    for tag in ["-201U7", "-102G1", "-X1032"] {
        let fp = t.placements.iter().find(|p| p.tag == tag).unwrap().footprint;
        let start = (
            i128::from(fp.max().x.0) + i128::from(30 * MM),
            i128::from(fp.min().y.0),
        );
        for turn in 0..4u8 {
            done(
                &mut shell,
                "tool.place_layout",
                json!({"device": e55.to_string()}),
            );
            let spot = sheet_of(&shell, start);
            done(&mut shell, "canvas.pointer", at(spot, json!({})));
            for _ in 0..turn {
                assert_eq!(key(&mut shell, "tool.rotate_cw"), Outcome::Done);
            }
            for (action, (du, dv)) in [
                ("cursor.right", (MM, 0)),
                ("cursor.up", (0, MM)),
                ("cursor.left", (-MM, 0)),
                ("cursor.down", (0, -MM)),
                ("cursor.right", (MM, 0)),
            ] {
                let (_, (u, v, k)) = shell.layout_predicted_poses()[0];
                assert_eq!(k, turn);
                assert_eq!(key(&mut shell, action), Outcome::Done);
                let (_, (u2, v2, _)) = shell.layout_predicted_poses()[0];
                assert_eq!(
                    (u2 - u, v2 - v),
                    (du, dv),
                    "{action} beside {tag} at {turn} quarter turns"
                );
            }
            // The drop commits what the ghost shows.
            let predicted = shell.layout_predicted_poses();
            done(&mut shell, "canvas.activate", json!({}));
            assert_eq!(committed(&shell, &predicted), predicted);
            done(&mut shell, "edit.undo", json!({}));
        }
    }
    shell.shutdown();
}

/// FT-995 (ID-1147): after a layout drop and after Select's drag of a placement, the HUD names no pick stack where the
/// drop left the pointer; once the pointer moves it does again.
#[test]
fn ft995_after_a_drop_the_hud_names_no_stack_until_the_pointer_moves() {
    let tmp = Tmp::new("layout-ft995");
    let mut shell = writable(&tmp);
    let f55 = unplaced(&shell, "F55");
    done(
        &mut shell,
        "tool.place_layout",
        json!({"device": f55.to_string()}),
    );
    let drop = on_rail(&shell, "Rail 1", 400 * MM);
    done(&mut shell, "canvas.pointer", at(drop, json!({})));
    done(&mut shell, "canvas.release", at(drop, json!({})));
    assert_eq!(shell.tool().name(), "select");
    let h = hud(&shell);
    assert_eq!(h.stack, 0);
    assert_eq!(h.prompt, "1 selected · arrows nudge · X flips · Delete deletes");
    let near = on_rail(&shell, "Rail 1", 401 * MM);
    done(&mut shell, "canvas.pointer", at(near, json!({})));
    let h = hud(&shell);
    assert_eq!(h.stack, 2, "F55 and Rail 1");
    assert_eq!(
        h.prompt,
        "2 objects overlap here · click again to select the next"
    );

    // Select's drag of F55 further along the rail.
    let from = footprint_centre(&shell, "F55");
    done(&mut shell, "canvas.press", at(from, json!({})));
    let to = on_rail(&shell, "Rail 1", 450 * MM);
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    done(&mut shell, "canvas.release", at(to, json!({})));
    let h = hud(&shell);
    assert_eq!(h.stack, 0);
    assert_eq!(h.prompt, "1 selected · arrows nudge · X flips · Delete deletes");
    let near = on_rail(&shell, "Rail 1", 451 * MM);
    done(&mut shell, "canvas.pointer", at(near, json!({})));
    assert_eq!(hud(&shell).stack, 2);
    shell.shutdown();
}

/// FT-996 (ID-1147): the HUD of a selected placement names only the keys it takes: X on a rail, R on the plate, Delete
/// always.
#[test]
fn ft996_the_selected_placements_hud_names_only_its_keys() {
    let tmp = Tmp::new("layout-ft996");
    let mut shell = writable(&tmp);
    select_placement(&mut shell, "S51");
    rest_away(&mut shell);
    assert_eq!(
        hud(&shell).prompt,
        "1 selected · arrows nudge · X flips · Delete deletes"
    );
    let surface = panel(&shell).surface.to_string();
    let e55 = unplaced(&shell, "E55");
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "layout.place", "params": {
            "device": e55.to_string(),
            "carrier": {"surface": surface},
            "pose": {"t": [520 * MM, 300 * MM, 0], "q": [1_000_000_000, 0, 0, 0]},
        }}),
    );
    select_placement(&mut shell, "E55");
    rest_away(&mut shell);
    assert_eq!(
        hud(&shell).prompt,
        "1 selected · arrows nudge · R rotates · Delete deletes"
    );
    assert_eq!(
        ew_viewmodel::strings::format(key!("hud-layout-selected"), &["2", "Delete"]),
        "2 selected · arrows nudge · Delete deletes"
    );
    shell.shutdown();
}

/// FT-997 (ID-1147): a nudge's "Blocked by T55" ends with the next action: Lock, Unlock and Enter each clear it.
#[test]
fn ft997_blocked_by_clears_on_lock_unlock_and_enter() {
    let tmp = Tmp::new("layout-ft997");
    let mut shell = writable(&tmp);
    select_placement(&mut shell, "-102F2");
    rest_away(&mut shell);
    let selected = "1 selected · arrows nudge · X flips · Delete deletes";
    let blocked = |shell: &mut ShellModel| {
        assert_eq!(
            key(shell, "cursor.right"),
            Outcome::Refused {
                reason_key: key!("reason-layout-blocked")
            }
        );
        assert_eq!(shell.tool_notice(), Some("Blocked by T55"));
        assert_eq!(hud(shell).prompt, "Blocked by T55");
    };
    blocked(&mut shell);
    done(&mut shell, "layout.lock_selection", json!({}));
    assert_eq!(shell.tool_notice(), None);
    assert_eq!(hud(&shell).prompt, selected);
    blocked(&mut shell);
    done(&mut shell, "layout.unlock_selection", json!({}));
    assert_eq!(shell.tool_notice(), None);
    assert_eq!(hud(&shell).prompt, selected);
    blocked(&mut shell);
    done(&mut shell, "canvas.activate", json!({}));
    assert_eq!(shell.tool_notice(), None);
    assert_eq!(hud(&shell).prompt, selected);
    shell.shutdown();
}
