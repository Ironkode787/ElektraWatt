// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! AC-28 as one flow (doc 20 §21): on one copy of Golden A, "102-A1" followed from 101-H16 and back,
//! W29's chips, its colour changed through the editor and undone, and a terminal added to -X1031 with the ghost's
//! predicted tag shown before the click. `settle` runs the accessibility check after every step. The frame bound
//! of AC-28 is not measured here (owner's rule). The parts' own tests (PG5, PL6, the viewer's V12) stay.

use crate::support;

use egui::{Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::canvas::mm1;
use ew_ui::ew_viewmodel::properties::{Provenance, RowValue, find_row};
use ew_ui::ew_viewmodel::shell::Focus;
use ew_ui::ew_viewmodel::{key, strings};
use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, settle, wait_for_page};

/// W29 on folio 103 (`connections|W29`).
const W29: &str = "9eb2d865-fca4-59b1-87bf-c073d7190ceb";

fn press(h: &mut Harness<'_, App>, key: Key) {
    h.key_press(key);
    settle(h);
}

fn folio(h: &Harness<'_, App>) -> String {
    h.state().shell().current_folio().unwrap().to_owned()
}

/// The text of the grid cursor's live region.
fn live(h: &Harness<'_, App>) -> String {
    h.query_all_by_label("Canvas cursor")
        .filter_map(|n| n.value())
        .find(|v| !v.is_empty())
        .unwrap_or_default()
}

/// The value text and provenance of row `key` of the grid's sheet.
fn row(h: &Harness<'_, App>, key: &str) -> (String, Provenance) {
    let sheet = h.state().shell().properties().sheet().expect("a sheet");
    let (_, r) = find_row(sheet, key).unwrap_or_else(|| panic!("no row {key}"));
    let text = match &r.value {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    };
    (text, r.provenance.clone())
}

/// Every record of the open model as its canonical line: equal lines are byte-identical records.
fn model_digest(h: &Harness<'_, App>) -> Vec<(String, String)> {
    let world = h.state().shell().world_for_tests().expect("a project");
    ew_commands::rows::model_lines(&world.model)
        .expect("every record has a JSON form")
        .into_iter()
        .map(|(k, line)| (format!("{k:?}"), line))
        .collect()
}

/// The window position (points) of the sheet point `p` on the focused canvas.
fn screen(h: &Harness<'_, App>, p: (i64, i64)) -> Pos2 {
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = h.state().shell().canvas().unwrap();
    let k = c.points_per_nm();
    Pos2::new(
        bounds.x0 as f32 + ((p.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
        bounds.y0 as f32 + ((p.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
    )
}

#[test]
fn ac28_the_four_steps_on_one_copy_of_golden_a() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": "101" }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);

    // Step 1: on folio 101, Shift+F moves the cursor from link to link until the live region names "102-A1"; F
    // follows it to folio 102 with cell A1 in the view; Alt+Left comes back to the view of before.
    assert_eq!(folio(&h), "101");
    let mut found = false;
    for _ in 0..100 {
        h.key_press_modifiers(Modifiers::SHIFT, Key::F);
        settle(&mut h);
        let c = h.state().shell().canvas().unwrap();
        if c.cursor_link().and_then(|l| l.label()).as_deref() == Some("102-A1") {
            found = true;
            break;
        }
    }
    assert!(found, "Shift+F reaches the link to 102-A1");
    let (before, at, link_target) = {
        let c = h.state().shell().canvas().unwrap();
        let link = c.cursor_link().unwrap();
        (c.viewport(), c.cursor().unwrap(), link.target.clone().unwrap())
    };
    let cell = h.state().shell().canvas().unwrap().page().cell_text(at);
    assert_eq!(cell.as_deref(), Some("101-H16"));
    let mut expected = strings::format(key!("canvas-cursor-live"), &["101-H16", &mm1(at.0), &mm1(at.1)]);
    expected.push_str(&strings::format(key!("canvas-cursor-live-link"), &["102-A1"]));
    assert_eq!(live(&h), expected);
    press(&mut h, Key::F);
    wait_for_page(&mut h);
    settle(&mut h);
    assert_eq!(folio(&h), "102");
    {
        let c = h.state().shell().canvas().unwrap();
        let target = link_target.at_nm.expect("the link's target point");
        assert_eq!(c.page().cell_text(target).as_deref(), Some("102-A1"));
        let [x0, y0, x1, y1] = c.view_nm();
        assert!(
            (x0..=x1).contains(&target.0) && (y0..=y1).contains(&target.1),
            "cell A1 at {target:?} lies inside the view {:?}",
            c.view_nm()
        );
    }
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    assert_eq!(folio(&h), "101");
    assert_eq!(h.state().shell().canvas().unwrap().viewport(), before);

    // Step 2: W29 through `select.set` on folio 103; its chip line and colour line whole.
    dispatch(&mut h, "page.goto", json!({ "folio": "103" }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    dispatch(&mut h, "select.set", json!({"ids": [W29]}));
    settle(&mut h);
    assert_eq!(
        row(&h, "connection/derived.csa"),
        (
            "0.75 mm²".to_owned(),
            Provenance::Derived {
                via: "potential 24VDC-1 · size profile '0.75/1.5' · class control".into()
            }
        )
    );
    assert_eq!(
        row(&h, "connection/derived.colour"),
        (
            "DBU".to_owned(),
            Provenance::Derived {
                via: "potential 24VDC-1 · colour profile 'DBU/WH 24 V'".into()
            }
        )
    );

    // Step 3: the colour through the editor (E, the colour row, Enter, "BU", Down, Enter), then undone: the colour
    // and every record of the model are back.
    let model_before = model_digest(&h);
    press(&mut h, Key::E);
    assert_eq!(h.state().shell().focus(), Some(Focus::Properties));
    press(&mut h, Key::Home);
    for _ in 0..200 {
        if h.state().shell().properties_row().as_deref() == Some("connection/derived.colour") {
            break;
        }
        press(&mut h, Key::ArrowDown);
    }
    assert_eq!(
        h.state().shell().properties_row().as_deref(),
        Some("connection/derived.colour")
    );
    press(&mut h, Key::Enter);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    settle(&mut h);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Colour"))
        .expect("the colour field")
        .type_text("BU");
    settle(&mut h);
    press(&mut h, Key::ArrowDown);
    press(&mut h, Key::Enter);
    assert_eq!(
        row(&h, "connection/derived.colour"),
        (
            "BU".to_owned(),
            Provenance::Overridden {
                note: "override (CDP)".into()
            }
        )
    );
    assert_ne!(model_digest(&h), model_before);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    assert_eq!(
        row(&h, "connection/derived.colour"),
        (
            "DBU".to_owned(),
            Provenance::Derived {
                via: "potential 24VDC-1 · colour profile 'DBU/WH 24 V'".into()
            }
        )
    );
    assert_eq!(model_digest(&h), model_before);

    // Step 4: a terminal of -X1031 armed; the pointer rests on the page, and the ghost shows the predicted tag
    // before the click; the click adds it.
    dispatch(&mut h, "select.set", json!({"ids": []}));
    settle(&mut h);
    let reps = |h: &Harness<'_, App>| h.state().shell().canvas().unwrap().page().tables.reps.len();
    let count = reps(&h);
    dispatch(
        &mut h,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "30"}),
    );
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "place");
    let spot = {
        let c = h.state().shell().canvas().unwrap();
        let (cx, cy) = c.centre();
        c.grid_map().nearest((cx, cy), c.sheet())
    };
    let pos = screen(&h, spot);
    h.event(egui::Event::PointerMoved(pos));
    settle(&mut h);
    // The window's dwell timer, dispatched as the window dispatches it: no clock is read.
    dispatch(&mut h, "canvas.dwell", json!({"ms": 80}));
    settle(&mut h);
    let Some(ew_ui::ew_viewmodel::tools::Ghost::Place { tag: Some(tag), .. }) =
        h.state().shell().tool_ghost()
    else {
        panic!("no predicted tag before the click");
    };
    assert_eq!(tag, "-X1031:30");
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    settle(&mut h);
    assert_eq!(reps(&h), count + 1);
    h.state_mut().shell_mut().shutdown();
}
