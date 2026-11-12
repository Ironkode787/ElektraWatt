// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The persona round on 0.2.0-rc.2, by the window's own path: the interruption point tool after a first point, New
//! page's type list, Title block fields…, Enclosure… in a project without a location, and the toasts' room.

use crate::support::{Scripted, Tmp, bar_button, dispatch, harness, hud_value, settle};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::script::state_json;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::visual::{Driver, Geometry, Input};
use serde_json::{Value, json};

fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

fn typed(text: &str) -> Input {
    Input::Type {
        text: text.to_owned(),
    }
}

/// Clicks the widget named `label` and lets the window settle.
fn press(d: &mut Driver, label: &str) {
    let at = d
        .find(label)
        .unwrap_or_else(|| panic!("no widget named {label:?}"));
    tap(d, at);
}

/// Clicks at `at`, lets the window settle and waits a moment, so the accessibility tree shows the result.
fn tap(d: &mut Driver, at: (i64, i64)) {
    d.input(&Input::Hover { at });
    d.input(&click(at));
    d.settle();
    d.input(&Input::Wait { ms: 100 });
}

/// A driver of a window of `geometry` over a project made as the play-test's prelude makes it: the wizard's company
/// standard, then a new project.
fn prelude(tmp: &Tmp, geometry: Geometry) -> Driver {
    let mut d = Driver::with_geometry(tmp.config(), None, tmp.0.clone(), geometry).unwrap();
    d.enable_accesskit();
    let company = tmp.0.join("company");
    let project = tmp.0.join("project");
    for (action, params) in [
        ("tools.company_standard", json!({})),
        ("wizard.finish", json!({"dir": company})),
        ("wizard.cancel", json!({})),
        ("file.new", json!({"dir": project})),
    ] {
        let out = d.shell_mut().dispatch(action, &params);
        assert_eq!(out, Outcome::Done, "{action}");
        d.settle();
    }
    // The window shows what the prelude made before the first look (the accessibility tree follows the frames).
    d.input(&Input::Wait { ms: 300 });
    d
}

/// The prelude's project on a new schematic page 101, as the play-test began.
fn on_page_101(tmp: &Tmp) -> Driver {
    let mut d = prelude(tmp, Geometry::DEFAULT);
    let page = json!({"folio": "101", "type": "multi_line", "title": "Control supply"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    d
}

fn state(d: &mut Driver) -> Value {
    serde_json::from_str(&state_json(d.shell_mut())).unwrap()
}

/// The window pixel of a page point (nm), from two pointer positions the canvas reports.
fn page_pixel(d: &mut Driver, p: (i64, i64)) -> (i64, i64) {
    let probe = |d: &mut Driver, at: (i64, i64)| {
        d.input(&Input::Hover { at });
        d.settle();
        let p = state(d)["canvas"]["pointer_nm"].clone();
        (p[0].as_i64().unwrap(), p[1].as_i64().unwrap())
    };
    let (f0, f1) = ((700, 400), (1000, 700));
    let (s0, s1) = (probe(d, f0), probe(d, f1));
    let sheet = d.shell_mut().canvas().unwrap().page().snapper.page_to_sheet(p);
    (
        f0.0 + (sheet.0 - s0.0) * (f1.0 - f0.0) / (s1.0 - s0.0),
        f0.1 + (sheet.1 - s0.1) * (f1.1 - f0.1) / (s1.1 - s0.1),
    )
}

/// The interruption points of the page shown: name, direction.
fn points(d: &mut Driver) -> Vec<(String, String)> {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    tables
        .reps
        .iter()
        .filter_map(|r| r.ip.as_ref().map(|ip| (ip.name.clone(), ip.direction.clone())))
        .collect()
}

/// The name and direction the IP tool's ghost shows.
fn ghost(d: &mut Driver) -> (String, String) {
    match d.shell_mut().tool_ghost() {
        Some(ew_ui::ew_viewmodel::tools::Ghost::Ip { name, direction, .. }) => (name, direction.to_owned()),
        other => panic!("no interruption point ghost: {other:?}"),
    }
}

fn toast(d: &mut Driver) -> String {
    d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default()
}

/// A page point in the left quarter of page 101's drawing area, `down` grid rows below its top quarter.
fn left_quarter(d: &mut Driver, down: i64) -> (i64, i64) {
    let page = d.shell_mut().canvas().unwrap().page().clone();
    let area = ew_ui::ew_viewmodel::tools::ip::drawing_area(&page).expect("a frame grid");
    let (w, h) = (area[2] - area[0], area[3] - area[1]);
    let p = (area[0] + w / 10, area[1] + h * 3 / 10 + down * 12_000_000);
    page.snapper.snap_grid(page.snapper.page_to_sheet(p)).page_nm
}

/// The centre of the widget named `label` with the greatest x (a form's field, right of its label of that name), or
/// with the least y (`top`: a command tab, over the navigator's heading of that name).
fn widget(d: &Driver, label: &str, top: bool) -> (i64, i64) {
    let all = d
        .labels()
        .into_iter()
        .filter(|(l, _)| l == label)
        .map(|(_, at)| at);
    let found = if top {
        all.min_by_key(|at| at.1)
    } else {
        all.max_by_key(|at| at.0)
    };
    found.unwrap_or_else(|| panic!("no widget named {label:?}"))
}

/// How many nodes of the window carry `text` as their name or value: a toast's text once, twice with the tooltip
/// that shows it whole because it is cut.
fn shown(d: &Driver, text: &str) -> usize {
    d.texts().iter().filter(|t| *t == text).count()
}

/// The play-test's way to a panel layout, in the prelude's new project, by the pointer: New page… lists every type
/// without a scroll, so Panel layout is clicked where the list shows it (it lay below the fold, t0055); Project ▸
/// Enclosure… is enabled without a location and its Create with the proposals makes the location, the enclosure and
/// shows its plate (it was greyed until a location existed, t0060); its toast is whole (t0062); Draw ▸ Layout ▸ Rail
/// is enabled and two clicks draw a rail.
#[test]
fn a_panel_layout_in_a_new_project_by_the_pointer() {
    let tmp = Tmp::new();
    let mut d = prelude(&tmp, Geometry::DEFAULT);
    press(&mut d, "New page");
    // The Type list, where the play-tester clicked it (a form's field is named by its label's relation, which the
    // driver does not read).
    let at = (868, 464);
    tap(&mut d, at);
    // The whole list stands in the window, its last type too.
    let last = d.find("External document").expect("the last type");
    assert!(last.1 < i64::from(d.geometry().frame_height()), "{last:?}");
    press(&mut d, "Panel layout");
    let kind = match d.shell_mut().dialog() {
        Some(ew_ui::ew_viewmodel::shell::Dialog::Form(f)) => f
            .fields
            .iter()
            .find(|f| f.name == "type")
            .map(|f| f.value.clone()),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        kind.as_deref(),
        Some("panel_layout"),
        "the click lands on Panel layout"
    );
    press(&mut d, "Create");
    let at = widget(&d, "Project", true);
    tap(&mut d, at);
    press(&mut d, "Enclosure");
    press(&mut d, "Create");
    assert!(d.shell_mut().dialog().is_none(), "{:?}", d.shell_mut().dialog());
    assert!(d.shell_mut().panel_tables().is_some(), "the page shows the plate");
    let said = toast(&mut d);
    assert!(said.ends_with("the page shows its mounting plate."), "{said:?}");
    let at = d.find("Notification").expect("the toast");
    d.input(&Input::Hover { at });
    d.input(&Input::Wait { ms: 1500 });
    d.settle();
    assert_eq!(shown(&d, &said), 1, "the toast is whole, no tooltip repeats it");
    let at = widget(&d, "Draw", true);
    tap(&mut d, at);
    press(&mut d, "Layout");
    press(&mut d, "Rail");
    assert_eq!(d.shell_mut().tool().name(), "rail_draw");
    for at in [(800, 400), (1000, 400)] {
        tap(&mut d, at);
    }
    let rails = d
        .shell_mut()
        .panel_tables()
        .map(|t| t.carriers.iter().filter(|c| c.axis.is_some()).count());
    assert_eq!(rails, Some(1), "one rail drawn");
    d.shell_mut().shutdown();
}

/// A new strip's toast, the longest a first-timer met (t0011, cut after "Draw ▸ …"), stands whole at the 1280 × 800
/// floor: under the pointer no tooltip repeats it.
#[test]
fn a_new_strips_toast_is_whole_at_the_floor() {
    let tmp = Tmp::new();
    let mut d = on_page_101(&tmp);
    assert_eq!(d.shell_mut().dispatch("strip.new", &json!({})), Outcome::Done);
    d.settle();
    let said = toast(&mut d);
    assert!(said.starts_with("New terminal strip"), "{said:?}");
    d.input(&Input::Wait { ms: 300 });
    let at = d.find("Notification").expect("the toast");
    d.input(&Input::Hover { at });
    d.input(&Input::Wait { ms: 1500 });
    d.settle();
    assert_eq!(shown(&d, &said), 1, "the toast is whole, no tooltip repeats it");
    d.shell_mut().shutdown();
}

/// The play-tester typed L1 into the tool's name field and clicked near the page's left edge without Enter: the
/// page's quarters made it a target, and the tool then offered "L1-1 target" ("L1 is taken, so this point is
/// L1-1"). A point without its partner leaves its name standing and the tool offers the partner; the toast says the
/// direction the click placed and the key that turns it before the click.
#[test]
fn after_a_lone_target_the_tool_offers_its_source_and_the_toast_says_the_direction() {
    let tmp = Tmp::new();
    let mut d = on_page_101(&tmp);
    press(&mut d, "Interruption");
    press(&mut d, "Interruption point name");
    d.input(&typed("L1"));
    let first = left_quarter(&mut d, 0);
    let at = page_pixel(&mut d, first);
    d.input(&Input::Hover { at });
    d.settle();
    d.input(&click(at));
    d.settle();
    assert_eq!(points(&mut d), [("L1".to_owned(), "target".to_owned())]);
    let said = toast(&mut d);
    assert!(
        said.starts_with("Placed L1 as a target at 101-")
            && said.ends_with("D before the click makes it a source."),
        "{said:?}"
    );
    // The next point is L1's source, in the same quarter too.
    let second = left_quarter(&mut d, 1);
    let at = page_pixel(&mut d, second);
    d.input(&Input::Hover { at });
    d.settle();
    assert_eq!(ghost(&mut d), ("L1".to_owned(), "source".to_owned()));
    assert!(
        !d.texts().iter().any(|t| t.contains("is taken")),
        "no new name offered"
    );
    d.input(&click(at));
    d.settle();
    let mut placed = points(&mut d);
    placed.sort();
    assert_eq!(
        placed,
        [
            ("L1".to_owned(), "source".to_owned()),
            ("L1".to_owned(), "target".to_owned())
        ]
    );
    assert!(toast(&mut d).starts_with("Placed L1 as a source at 101-"));
    // The pair is whole: the name is taken.
    assert_eq!(d.shell_mut().ip_typed_name(), None);
    d.shell_mut().shutdown();
}

/// The play-tester's L1 near the left edge came out a target with no source, and its wire's Potential read the device
/// pin ("-101K1:A1"), so the supply's wires had no name, no colour by name and no offer. A point names the potential
/// it carries whichever its direction: the wire from L1 to the relay's A1 reads L1 in Properties.
#[test]
fn a_wire_on_a_lone_target_point_carries_its_name() {
    let tmp = Tmp::new();
    let mut d = on_page_101(&tmp);
    press(&mut d, "Interruption");
    press(&mut d, "Interruption point name");
    d.input(&typed("L1"));
    let first = left_quarter(&mut d, 0);
    let at = page_pixel(&mut d, first);
    tap(&mut d, at);
    assert_eq!(points(&mut d), [("L1".to_owned(), "target".to_owned())]);
    d.input(&Input::Key {
        key: Some(egui::Key::Escape),
        mods: Modifiers::NONE,
        repeat: 2,
    });
    d.settle();
    // The relay of the device browser's Relays, right of the point.
    press(&mut d, "Device");
    d.input(&typed("relay"));
    d.settle();
    d.input(&Input::Wait { ms: 100 });
    let entry = d
        .labels()
        .into_iter()
        .find(|(l, _)| l.starts_with("Relay with four"))
        .expect("the relay in the browser")
        .1;
    d.input(&Input::DoubleClick {
        at: entry,
        mods: Modifiers::NONE,
    });
    let relay = page_pixel(&mut d, (first.0 + 80_000_000, first.1));
    tap(&mut d, relay);
    d.input(&Input::Key {
        key: Some(egui::Key::Escape),
        mods: Modifiers::NONE,
        repeat: 1,
    });
    d.settle();
    // A wire from the point's connection point to the coil's A1.
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let ip_rep = tables.reps.iter().find(|r| r.ip.is_some()).expect("the point").id;
    let from = tables
        .cps
        .iter()
        .find(|c| c.rep == ip_rep)
        .expect("the point's connection point")
        .at;
    let to = tables
        .cps
        .iter()
        .find(|c| c.label.ends_with(":A1"))
        .expect("the coil's A1")
        .at;
    press(&mut d, "Wire");
    for p in [from, to] {
        let at = page_pixel(&mut d, p);
        tap(&mut d, at);
    }
    d.input(&Input::Key {
        key: Some(egui::Key::Escape),
        mods: Modifiers::NONE,
        repeat: 2,
    });
    d.settle();
    // Select the wire by a click on its longest segment, and read its Potential row.
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let wire = tables.connections.last().expect("the wire").polyline.clone();
    let (p, q) = wire
        .windows(2)
        .map(|s| (s[0], s[1]))
        .max_by_key(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
        .expect("a segment");
    let at = page_pixel(&mut d, ((p.0 + q.0) / 2, (p.1 + q.1) / 2));
    tap(&mut d, at);
    let sheet = d
        .shell_mut()
        .properties()
        .sheet()
        .expect("the wire's properties")
        .clone();
    let potential = sheet
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .find(|r| r.key.ends_with("derived.potential"))
        .map(|r| format!("{:?}", r.value))
        .expect("the Potential row");
    assert!(potential.contains("\"L1\""), "{potential}");
    d.shell_mut().shutdown();
}

/// While the caret is in the tool's name field, D is a letter of the name: the bar says that Enter ends the name
/// first, not "D: target" (the play-tester pressed D after typing and read the bar's "D" as live). Tab gives the
/// keys back at once: the bar names D again, and the first D turns the point the bar shows.
#[test]
fn the_bar_names_d_only_while_the_tool_has_the_keys() {
    let tmp = Tmp::new();
    let mut h = prelude_window(&tmp);
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "102", "type": "multi_line", "title": "Control circuits"}),
    );
    settle(&mut h);
    bar_button(&h, "Interruption").click();
    settle(&mut h);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .type_text("+24V");
    settle(&mut h);
    let typing = hud_value(&h);
    assert!(typing.contains("Enter ends the name, then D:"), "{typing:?}");
    assert!(!typing.contains("\u{b7} D:"), "{typing:?}");
    h.key_press(egui::Key::Tab);
    settle(&mut h);
    let free = hud_value(&h);
    assert!(
        free.contains("\u{b7} D:") && !free.contains("Enter ends"),
        "{free:?}"
    );
    let before = ip_direction(&h);
    h.key_press(egui::Key::D);
    settle(&mut h);
    assert_ne!(
        ip_direction(&h),
        before,
        "the first D turns the point the bar shows"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The direction the IP tool's ghost shows.
fn ip_direction(h: &egui_kittest::Harness<'_, ew_ui::app::App>) -> &'static str {
    match h.state().shell().tool_ghost() {
        Some(ew_ui::ew_viewmodel::tools::Ghost::Ip { direction, .. }) => direction,
        other => panic!("no interruption point ghost: {other:?}"),
    }
}

/// The window at the 1280 x 800 floor over the prelude's project (the wizard's company standard, then a new project).
fn prelude_window(tmp: &Tmp) -> egui_kittest::Harness<'static, ew_ui::app::App> {
    let mut h = harness(tmp, Scripted::default(), None);
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    let company = tmp.0.join("company");
    let project = tmp.0.join("project");
    dispatch(&mut h, "tools.company_standard", json!({}));
    dispatch(&mut h, "wizard.finish", json!({"dir": company}));
    dispatch(&mut h, "wizard.cancel", json!({}));
    dispatch(&mut h, "file.new", json!({"dir": project}));
    settle(&mut h);
    h
}

/// Title block fields… on the prelude's first page, as the play-tester met it (t0051 to t0053): the Revision field
/// says the project's form and the next free label before Apply; a "B" refused leaves its line under the fields, and
/// when the field changes and the line goes, the dialog keeps its height, so Apply stays under the pointer; the word on
/// Checked stands at the foot of the form, under the last field, not over the first.
#[test]
fn title_block_fields_say_the_revision_rule_first_and_apply_stays_put() {
    let tmp = Tmp::new();
    let mut h = prelude_window(&tmp);
    h.get_by_label("Title block fields…").click();
    settle(&mut h);
    let field = |h: &egui_kittest::Harness<'_, ew_ui::app::App>, name: &str| {
        h.get_all_by_role(egui::Role::TextInput)
            .find(|n| n.accesskit_node().label().as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no field {name}"))
            .rect()
    };
    let hint = h
        .query_all_by_label_contains("The label takes the project's form, such as ")
        .next()
        .expect("the Revision field's rule")
        .rect();
    let revision = field(&h, "Revision");
    assert!(hint.top() >= revision.bottom(), "under the Revision field");
    let fields: Vec<egui::Rect> = h
        .get_all_by_role(egui::Role::TextInput)
        .map(|n| n.rect())
        .collect();
    let checked = h
        .query_all_by_label_contains("Checked prints the checker")
        .next()
        .expect("the word on Checked")
        .rect();
    assert!(
        fields.iter().all(|f| checked.top() >= f.bottom()),
        "the word on Checked stands under the last field"
    );
    // B, Apply: refused, its line under the fields.
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Revision"))
        .unwrap()
        .click();
    settle(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Revision"))
        .unwrap()
        .type_text("B");
    settle(&mut h);
    h.get_by_label("Apply").click();
    settle(&mut h);
    let refused = match h.state().shell().dialog() {
        Some(ew_ui::ew_viewmodel::shell::Dialog::Form(f)) => f.error.clone(),
        other => panic!("{other:?}"),
    };
    let refused = refused.expect("B is refused");
    assert!(refused.contains("the next free label is "), "{refused}");
    assert!(
        h.query_by_label(&refused).is_some(),
        "the refusal under the fields"
    );
    let apply = h.get_by_label("Apply").rect();
    // The field changes, the line goes, Apply stays.
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Revision"))
        .unwrap()
        .click();
    settle(&mut h);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Revision"))
        .unwrap()
        .type_text("2");
    settle(&mut h);
    assert!(
        h.query_by_label(&refused).is_none(),
        "the refusal goes with the change"
    );
    assert_eq!(
        h.get_by_label("Apply").rect(),
        apply,
        "Apply stays under the pointer"
    );
    h.state_mut().shell_mut().shutdown();
}
