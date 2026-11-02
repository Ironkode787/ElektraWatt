// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 4 through the shell on temporary copies of Golden A: Draw ▸ Layout enabled on 902 and disabled on 101
//! with its reason (PS-7); Draw rail giving R1's geometry with a diagonal second click (PS-2); Draw duct giving D1 and
//! D2 with Backspace, a double click and Enter (PS-3); a carrier dragged by Select; the Create enclosure dialog on a
//! new location of the project, its refusal on +J-E-2416, and a new panel page's Surface view centred (PS-1, the
//! project path); Properties rows committed from the shell (PS-4, PS-5); Auto-fill rail's dialog and toast (PS-6).

use crate::common;
use ew_viewmodel::key;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::command_bar::{Tab, command_bar_of};
use ew_viewmodel::ew_api::library::{LibManifest, LibraryTiers, STARTER_PARTS_KEY, starter_parts_library};
use ew_viewmodel::ew_api::output::{PanelCarrier, PanelTables, Uuid};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;
use std::path::Path;

const MM: i128 = 1_000_000;

fn panel(shell: &ShellModel) -> std::sync::Arc<PanelTables> {
    shell.panel_tables().expect("a panel page with a surface view")
}

/// The sheet point of a surface point (mm) of the page shown.
fn sheet_mm(shell: &ShellModel, u: f64, v: f64) -> (i64, i64) {
    let t = panel(shell);
    let p = ((u * 1e6).round() as i128, (v * 1e6).round() as i128);
    shell.canvas().unwrap().page().snapper.page_to_sheet(t.to_page(p))
}

/// A click at the surface point (mm).
fn click_mm(shell: &mut ShellModel, u: f64, v: f64) {
    let p = sheet_mm(shell, u, v);
    done(shell, "canvas.pointer", at(p, json!({})));
    common::click(shell, p, json!({}));
}

fn carrier<'a>(t: &'a PanelTables, label: &str) -> &'a PanelCarrier {
    t.carriers
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no carrier {label}"))
}

/// The carriers new since `before`.
fn new_carriers(shell: &ShellModel, before: &[Uuid]) -> Vec<PanelCarrier> {
    panel(shell)
        .carriers
        .iter()
        .filter(|c| !before.contains(&c.id))
        .cloned()
        .collect()
}

fn ids(shell: &ShellModel) -> Vec<Uuid> {
    panel(shell).carriers.iter().map(|c| c.id).collect()
}

/// The part number of the carrier labelled `label`, as the draw tools offer it.
fn part_number_of(shell: &ShellModel, kind: &str, label: &str) -> String {
    let part = carrier(&panel(shell), label).part.expect("a part");
    shell
        .draw_part_offers(kind)
        .into_iter()
        .find(|o| o.part == part)
        .expect("the carrier's part is offered")
        .number
}

fn mm_path(c: &PanelCarrier) -> Vec<(i128, i128)> {
    c.path
        .iter()
        .map(|p| (i128::from(p.0) / MM, i128::from(p.1) / MM))
        .collect()
}

/// The Draw tab's Layout group: (bar label, enabled, reason) per button.
fn layout_group(shell: &ShellModel) -> Vec<(String, bool, Option<String>)> {
    let bar = command_bar_of(shell, Tab::Draw);
    let group = bar
        .groups
        .iter()
        .find(|g| g.label == "Layout")
        .expect("Draw's Layout group");
    group
        .buttons
        .iter()
        .map(|b| (b.bar_label.clone(), b.enabled, b.reason.clone()))
        .collect()
}

#[test]
fn ps7_the_layout_group_is_enabled_on_a_panel_page_and_says_why_not_elsewhere() {
    let tmp = Tmp::new("ps7");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    let on = |b: &str| (b.to_owned(), true, None);
    assert_eq!(layout_group(&s), [on("Rail"), on("Duct"), on("Auto-fill rail")]);
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let off = |b: &str| (b.to_owned(), false, Some("Open a panel layout page".to_owned()));
    assert_eq!(
        layout_group(&s),
        [off("Rail"), off("Duct"), off("Auto-fill rail")]
    );
    assert_eq!(
        refused(&mut s, "tool.rail_draw", json!({})),
        "reason-no-panel-page"
    );
    // Project ▸ Project holds Enclosure…, enabled in a project that can be changed.
    let bar = command_bar_of(&s, Tab::Project);
    let enclosure = bar
        .groups
        .iter()
        .flat_map(|g| &g.buttons)
        .find(|b| b.id == "layout.enclosure_new")
        .expect("Enclosure… in the Project tab");
    assert_eq!(
        (
            enclosure.bar_label.as_str(),
            enclosure.label.as_str(),
            enclosure.enabled
        ),
        ("Enclosure", "Enclosure…", true)
    );
    s.shutdown();
}

#[test]
fn ps2_draw_rail_gives_r1s_geometry_and_keeps_the_dominant_axis() {
    let tmp = Tmp::new("ps2");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    let before = ids(&s);
    let number = part_number_of(&s, "rail", "Rail 1");
    done(&mut s, "tool.rail_draw", json!({}));
    done(&mut s, "layout.draw_options", json!({"part": number}));
    assert_eq!(s.tool().name(), "rail_draw");
    click_mm(&mut s, 40.8, 741.3);
    assert_eq!(s.tool().state(), "drawing");
    // The second point, off the axis by 22 mm, stays on the horizontal.
    let p = sheet_mm(&s, 609.2, 762.0);
    done(&mut s, "canvas.pointer", at(p, json!({})));
    assert_eq!(
        state(&s)["canvas"]["ghost"],
        json!({"kind": "line", "carrier": "rail", "from_nm": [40_000_000, 740_000_000],
            "to_nm": [610_000_000, 740_000_000]})
    );
    let rev = s.revision();
    common::click(&mut s, p, json!({}));
    assert_ne!(s.revision(), rev, "one commit");
    let added = new_carriers(&s, &before);
    assert_eq!(added.len(), 1);
    let t = panel(&s);
    let r1 = carrier(&t, "Rail 1");
    assert_eq!(mm_path(&added[0]), [(40, 740), (610, 740)]);
    assert_eq!(added[0].path, r1.path);
    assert_eq!(added[0].profile, r1.profile);
    assert_eq!(added[0].part, r1.part);
    // One undo step takes it away.
    done(&mut s, "edit.undo", json!({}));
    assert!(new_carriers(&s, &before).is_empty());
    s.shutdown();
}

#[test]
fn ps3_draw_duct_gives_d1_and_d2_with_backspace_a_double_click_and_enter() {
    let tmp = Tmp::new("ps3");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    let before = ids(&s);
    let number = part_number_of(&s, "duct", "Duct 1");
    done(&mut s, "tool.duct_draw", json!({}));
    done(&mut s, "layout.draw_options", json!({"part": number}));
    // D1: under the plate's top edge, inset by half the width (40 mm).
    click_mm(&mut s, 1.5, 881.5);
    click_mm(&mut s, 300.0, 700.0);
    done(&mut s, "tool.back", json!({}));
    click_mm(&mut s, 648.5, 878.0);
    // A double click: the second click adds nothing, the double click finishes.
    click_mm(&mut s, 648.5, 878.0);
    done(&mut s, "tool.finish", json!({}));
    let t = panel(&s);
    let added = new_carriers(&s, &before);
    assert_eq!(added.len(), 1, "one duct");
    assert_eq!(mm_path(&added[0]), [(0, 880), (650, 880)]);
    assert_eq!(added[0].path, carrier(&t, "Duct 1").path);
    assert_eq!((added[0].width, added[0].height), (Some(40 * MM), Some(60 * MM)));
    // D2: from under D1's lower edge, down to the plate's bottom edge; Enter on the last vertex finishes.
    let before = ids(&s);
    click_mm(&mut s, 21.5, 858.5);
    click_mm(&mut s, 19.0, 1.5);
    done(&mut s, "tool.finish", json!({}));
    let added = new_carriers(&s, &before);
    assert_eq!(added.len(), 1, "a second duct");
    assert_eq!(mm_path(&added[0]), [(20, 860), (20, 0)]);
    // One point is not a duct.
    click_mm(&mut s, 300.0, 300.0);
    assert_eq!(
        refused(&mut s, "tool.finish", json!({})),
        "reason-duct-two-points"
    );
    s.shutdown();
}

#[test]
fn select_drags_a_rail_on_the_grid_with_its_placements() {
    let tmp = Tmp::new("drag-rail");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    let t = panel(&s);
    let r3 = carrier(&t, "Rail 3").clone();
    let on_r3 = t.placements.iter().filter(|p| p.carrier == Some(r3.id)).count();
    let a = r3.axis.expect("an axis");
    // A point of the rail no placement covers.
    let grab = (1..r3.length.expect("a length") / MM)
        .rev()
        .map(|s| a.point_at(s * MM))
        .find(|p| {
            !t.placements.iter().any(|x| {
                let f = x.footprint;
                (i128::from(f.min().x.0)..=i128::from(f.max().x.0)).contains(&p.0)
                    && (i128::from(f.min().y.0)..=i128::from(f.max().y.0)).contains(&p.1)
            })
        })
        .expect("a free point of rail 3");
    let from = s.canvas().unwrap().page().snapper.page_to_sheet(t.to_page(grab));
    let to = sheet_mm(&s, (grab.0 as f64) / 1e6 + 0.4, (grab.1 as f64) / 1e6 - 18.2);
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    done(&mut s, "canvas.release", at(to, json!({})));
    let t = panel(&s);
    let moved = carrier(&t, "Rail 3");
    let shifted: Vec<(i64, i64)> = r3.path.iter().map(|p| (p.0, p.1 - 20_000_000)).collect();
    assert_eq!(moved.path, shifted, "18.2 mm down is 20 mm on the 5 mm grid");
    assert_eq!(
        t.placements
            .iter()
            .filter(|p| p.carrier == Some(moved.id))
            .count(),
        on_r3,
        "its placements go with it"
    );
    s.shutdown();
}

#[test]
fn ps1_create_enclosure_on_a_new_location_and_its_refusal_on_one_with_an_enclosure() {
    let tmp = Tmp::new("ps1");
    let mut s = tmp.shell_in(Mode::Gui, true, "902");
    // +J-E-2416 has -UC1: the refusal stays under the fields.
    done(&mut s, "layout.enclosure_new", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("{:?}", s.dialog())
    };
    assert_eq!(form.action, "layout.enclosure_new");
    let location = form.fields.iter().find(|f| f.name == "location").unwrap();
    let words = location
        .options
        .iter()
        .find(|(v, _)| *v == location.value)
        .map(|(_, w)| w.as_str());
    assert_eq!(words, Some("+J-E-2416"), "the focused page's location");
    let part = form.fields.iter().find(|f| f.name == "part").unwrap();
    assert!(!part.options.is_empty(), "the project's enclosure parts");
    let enclosure_part = part.value.clone();
    let rev = s.revision();
    let Outcome::Failed { message } = s.submit_form() else {
        panic!("refused")
    };
    assert_eq!(
        message,
        "Location +J-E-2416 already has enclosure -UC1; a location has one enclosure. \
         Choose another location, or edit -UC1."
    );
    let Some(Dialog::Form(form)) = s.dialog() else {
        panic!("the dialog stays")
    };
    assert_eq!(form.error.as_deref(), Some(message.as_str()));
    assert_eq!(s.revision(), rev);
    done(&mut s, "ui.dismiss", json!({}));
    // A new location: one undo step creates the enclosure with its plate.
    done(
        &mut s,
        "structure.new",
        json!({"kind": "location", "designation": "J-E-999"}),
    );
    let surfaces_before = surfaces(&s);
    assert_eq!(surfaces_before, ["-UC1 plate"]);
    let rev = s.revision();
    done(
        &mut s,
        "layout.enclosure_new",
        json!({"location": "+J-E-999", "part": enclosure_part}),
    );
    assert!(s.dialog().is_none());
    let plates = surfaces(&s);
    assert_eq!(plates.len(), 2, "one plate more: {plates:?}");
    let plate = plates.iter().find(|n| *n != "-UC1 plate").unwrap().clone();
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(surfaces(&s), ["-UC1 plate"], "one undo step");
    assert_ne!(s.revision(), rev);
    done(&mut s, "edit.redo", json!({}));
    // A new panel layout page shows the new plate centred in its frame.
    done(
        &mut s,
        "page.new",
        json!({"folio": "950", "type": "panel_layout", "title": "Panel"}),
    );
    assert_eq!(s.current_folio(), Some("950"));
    assert!(s.panel_tables().is_none(), "no surface view yet");
    done(&mut s, "select.set", json!({"ids": []}));
    done(
        &mut s,
        "props.set",
        json!({"row": "page/derived.surface", "value": plate}),
    );
    let t = panel(&s);
    let sheet = s.canvas().unwrap().page().sheet;
    let corners: Vec<(i64, i64)> = t
        .usable
        .iter()
        .map(|p| {
            s.canvas()
                .unwrap()
                .page()
                .snapper
                .page_to_sheet(t.to_page((i128::from(p.0), i128::from(p.1))))
        })
        .collect();
    let (xs, ys): (Vec<i64>, Vec<i64>) = corners.iter().copied().unzip();
    let (x0, x1) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
    let (y0, y1) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
    // The new page keeps its own scale (1:1 here, so the plate overhangs the sheet); its centre is the sheet's, less
    // the title block's share of the drawing area.
    let (cx, cy) = ((x0 + x1) / 2, (y0 + y1) / 2);
    assert!(
        (cx - sheet.0 / 2).abs() < sheet.0 / 10 && (cy - sheet.1 / 2).abs() < sheet.1 / 10,
        "centred: ({cx}, {cy}) on {sheet:?}"
    );
    // C44-p2#5: the datum exactly, the one D12 computes for this frame and plate.
    assert_eq!(row_text(&s, "page/derived.datum"), PS1_DATUM);
    s.shutdown();
}

/// PS-1's centred datum of the new plate on folio 950 (1:1): the frame's drawing-area centre on the page, less the
/// enclosure rectangle's centre in the plate's frame (x), plus it (y, the page's y grows downwards).
const PS1_DATUM: &str = "-192.746658, 531.283331 mm";

/// The Surface view row's choices on the page shown (the project's plates by their words).
fn surfaces(s: &ShellModel) -> Vec<String> {
    let sheet = s.properties().sheet().expect("the page's Properties");
    let row = sheet
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .find(|r| r.key == "page/derived.surface")
        .expect("the Surface row");
    match &row.ty {
        ew_viewmodel::ew_api::output::FieldType::Enum { options } => options.clone(),
        other => panic!("{other:?}"),
    }
}

/// The key of the Unplaced devices row labelled `label`.
fn unplaced_key(s: &ShellModel, label: &str) -> String {
    s.shown_rows(NavKind::Unplaced)
        .into_iter()
        .find(|r| r.label == label)
        .unwrap_or_else(|| panic!("{label} is not unplaced"))
        .key
}

/// PS-6 (AS-3 through the window): R1 emptied, the four rail devices and E55 marked in Unplaced devices, Auto-fill rail
/// in DT order gives -101F1, -102F1, -102F2 and -102G1 at clip 9, 27, 58.5 and 116 mm, and the toast names E55.
#[test]
fn ps6_auto_fill_rail_fills_an_emptied_rail_1_with_the_marked_devices() {
    let tmp = Tmp::new("ps6");
    let mut s = tmp.shell_in(Mode::Gui, true, "902");
    let t = panel(&s);
    let r1 = carrier(&t, "Rail 1").id;
    for p in t.placements.iter().filter(|p| p.carrier == Some(r1)) {
        done(
            &mut s,
            "command.run",
            json!({"cmd": "layout.remove", "params": {"placement": p.id.to_string()}}),
        );
    }
    done(&mut s, "navigator.show", json!({"kind": "unplaced"}));
    for d in ["-102G1", "-102F2", "-101F1", "-102F1", "E55"] {
        let key = unplaced_key(&s, d);
        done(&mut s, "navigator.mark", json!({"kind": "unplaced", "key": key}));
    }
    done(&mut s, "layout.auto_fill_rail", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("{:?}", s.dialog())
    };
    let fields: Vec<(&str, String, Vec<String>)> = form
        .fields
        .iter()
        .map(|f| {
            let words = f
                .options
                .iter()
                .find(|(v, _)| *v == f.value)
                .map(|(_, w)| w.clone())
                .unwrap_or_default();
            (f.name, words, f.options.iter().map(|(_, w)| w.clone()).collect())
        })
        .collect();
    assert_eq!(
        fields,
        [
            (
                "rail",
                "Rail 1".to_owned(),
                vec!["Rail 1".to_owned(), "Rail 2".to_owned(), "Rail 3".to_owned()]
            ),
            (
                "order",
                "Schematic".to_owned(),
                vec!["Schematic".to_owned(), "DT".to_owned()]
            ),
        ]
    );
    let spec = ew_viewmodel::dialogs::spec(s.dialog().unwrap(), &s);
    assert_eq!(
        (
            spec.title.as_str(),
            spec.body.clone(),
            spec.primary.label.as_str()
        ),
        (
            "Auto-fill rail",
            vec!["Fills with the 5 devices marked in Unplaced devices.".to_owned()],
            "Fill"
        )
    );
    s.set_form_field("order", "dt");
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(s.dialog().is_none());
    let t = panel(&s);
    let mut on: Vec<(String, i64)> = t
        .placements
        .iter()
        .filter(|p| p.carrier == Some(r1))
        .map(|p| (p.tag.clone(), p.pose.0 / 100_000))
        .collect();
    on.sort_by_key(|p| p.1);
    assert_eq!(
        on,
        [
            ("-101F1".to_owned(), 90),
            ("-102F1".to_owned(), 270),
            ("-102F2".to_owned(), 585),
            ("-102G1".to_owned(), 1160)
        ],
        "clip points in tenths of a millimetre"
    );
    let toast = s
        .notifications()
        .visible()
        .last()
        .map(|n| n.text.clone())
        .expect("a toast");
    assert_eq!(
        toast,
        "Placed 4 devices on rail 1; E55: screw-mounted, not on a rail."
    );
    s.shutdown();
}

/// PS-5 through the grid: -102F1's Locked row, its Position row's refusals in words (a text that does not read, and one
/// the command refuses, the old value kept), and R1's Length row.
#[test]
fn ps5_rows_commit_from_the_properties_grid() {
    let tmp = Tmp::new("ps5");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    let t = panel(&s);
    let fp = t.placements.iter().find(|p| p.tag == "-102F1").unwrap().footprint;
    let f1 = t.placements.iter().find(|p| p.tag == "-102F1").unwrap().id;
    let centre = (
        (i128::from(fp.min().x.0) + i128::from(fp.max().x.0)) / 2,
        (i128::from(fp.min().y.0) + i128::from(fp.max().y.0)) / 2,
    );
    let p = s
        .canvas()
        .unwrap()
        .page()
        .snapper
        .page_to_sheet(t.to_page(centre));
    common::click(&mut s, p, json!({}));
    assert_eq!(s.selected_placements(), 1);
    done(
        &mut s,
        "props.set",
        json!({"row": "placement/derived.locked", "value": "true"}),
    );
    assert!(panel(&s).placement(f1).unwrap().locked);
    let Outcome::Failed { message } = s.dispatch(
        "props.set",
        &json!({"row": "placement/derived.position", "value": "far"}),
    ) else {
        panic!("refused")
    };
    assert_eq!(message, "Type a length in millimetres, such as 158.5.");
    // A text that reads but that the command refuses (off the rail's end): the refusal in words, the old value kept.
    let Outcome::Failed { message } = s.dispatch(
        "props.set",
        &json!({"row": "placement/derived.position", "value": "9999"}),
    ) else {
        panic!("refused")
    };
    assert_eq!(message, FAR_POSITION_REFUSAL);
    assert_eq!(row_text(&s, "placement/derived.position"), "158 mm");
    // R1's Length row lengthens the rail.
    let r1 = carrier(&panel(&s), "Rail 1").id;
    done(&mut s, "select.set", json!({"ids": [r1.to_string()]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "carrier/derived.length", "value": "600"}),
    );
    assert_eq!(row_text(&s, "carrier/derived.length"), "600 mm");
    s.shutdown();
}

/// What `layout.move` says of -102F1 moved to 9999 mm on rail 1 (570 mm long).
const FAR_POSITION_REFUSAL: &str = "the pose is off the rail: s = 9999 mm is outside [0, 570 mm]";

/// The text of the shown Properties row `key`.
fn row_text(s: &ShellModel, key: &str) -> String {
    let sheet = s.properties().sheet().expect("a Properties sheet");
    let row = sheet
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no row {key}"));
    match &row.value {
        ew_viewmodel::ew_api::output::RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    }
}

/// A new project from the built-in template (no location, no enclosure part).
fn new_project(tmp: &Tmp) -> ShellModel {
    new_project_with(tmp, None)
}

/// A new project's shell with the library tiers `libraries` (the compiled-in starter when `None`).
fn new_project_with(tmp: &Tmp, libraries: Option<LibraryTiers>) -> ShellModel {
    let config = ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test").with_libraries(libraries);
    let mut s = ShellModel::new(Mode::Gui, config);
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("n.ew").to_string_lossy(), "template": "builtin"}),
    );
    s
}

/// Project ▸ Enclosure…: (enabled, reason) on the command bar.
fn enclosure_entry(s: &ShellModel) -> (bool, Option<String>) {
    let bar = command_bar_of(s, Tab::Project);
    let b = bar
        .groups
        .iter()
        .flat_map(|g| &g.buttons)
        .find(|b| b.id == "layout.enclosure_new")
        .expect("Enclosure… in the Project tab");
    (b.enabled, b.reason.clone())
}

/// Library tiers in which the compiled-in starter parts are replaced by an empty library of their ID, so that no
/// enclosure part is installed: a directory of the starter's ID replaces the compiled-in library (D6, ID-1109).
fn tiers_without_starter_parts(root: &Path) -> LibraryTiers {
    let dir = root.join("company").join("ew-starter-parts.ewlib");
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = LibManifest::new(
        starter_parts_library(),
        STARTER_PARTS_KEY,
        "ElektraWatt starter parts",
        "CC0-1.0",
    );
    std::fs::write(dir.join("library.json"), manifest.to_text()).unwrap();
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: vec![root.join("company")],
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// W44.2 F1: on a new project without a location, Project ▸ Enclosure… is disabled with its reason, and the action
/// refuses with it; with a location the dialog opens, and Create is disabled while the part (or the location) is
/// empty, each with its reason; Create then sends nothing. No enclosure part is installed here (ID-1109).
#[test]
fn enclosure_is_disabled_without_a_location_and_create_without_a_part() {
    let tmp = Tmp::new("ps1-new");
    let mut s = new_project_with(&tmp, Some(tiers_without_starter_parts(&tmp.0)));
    let no_location = "The project has no location yet; add one with Structure node… first.";
    assert_eq!(enclosure_entry(&s), (false, Some(no_location.to_owned())));
    assert_eq!(
        refused(&mut s, "layout.enclosure_new", json!({})),
        "enclosure-new-no-location"
    );
    assert_eq!(s.tool_notice(), Some(no_location));
    assert!(s.dialog().is_none());
    done(
        &mut s,
        "structure.new",
        json!({"kind": "location", "designation": "CAB1"}),
    );
    assert_eq!(enclosure_entry(&s), (true, None));
    done(&mut s, "layout.enclosure_new", json!({}));
    let spec = ew_viewmodel::dialogs::spec(s.dialog().unwrap(), &s);
    assert_eq!(
        (
            spec.title.as_str(),
            spec.body.clone(),
            spec.primary.label.as_str(),
            spec.primary.disabled.as_deref()
        ),
        (
            "Create enclosure",
            vec![
                "No enclosure part is in the project or an installed library; type a part number.".to_owned()
            ],
            "Create",
            Some("Type or choose a part number")
        )
    );
    // C44-p4#2: the description names the reason whole.
    assert_eq!(
        create_line(&s),
        "  button \"Create\" disabled: Type or choose a part number"
    );
    let rev = s.revision();
    let Outcome::Failed { message } = s.submit_form() else {
        panic!("refused")
    };
    assert_eq!(message, "Type or choose a part number");
    assert_eq!(s.revision(), rev, "no command");
    s.set_form_field("part", "NSY-1");
    let spec = ew_viewmodel::dialogs::spec(s.dialog().unwrap(), &s);
    assert_eq!(spec.primary.disabled, None);
    s.set_form_field("location", "");
    let spec = ew_viewmodel::dialogs::spec(s.dialog().unwrap(), &s);
    assert_eq!(spec.primary.disabled.as_deref(), Some("Choose a location"));
    assert_eq!(create_line(&s), "  button \"Create\" disabled: Choose a location");
    let Outcome::Failed { message } = s.submit_form() else {
        panic!("refused")
    };
    assert_eq!(message, "Choose a location");
    assert_eq!(s.revision(), rev, "no command");
    s.shutdown();
}

/// ID-1109 (WP-S4.02 D14): with the compiled-in starter parts, the Create enclosure dialog offers the starter's
/// enclosure: its body names no refusal, and Create is enabled.
#[test]
fn the_starter_enclosure_is_offered_by_the_create_dialog() {
    let tmp = Tmp::new("ps1-starter");
    let mut s = new_project(&tmp);
    done(
        &mut s,
        "structure.new",
        json!({"kind": "location", "designation": "CAB1"}),
    );
    done(&mut s, "layout.enclosure_new", json!({}));
    let spec = ew_viewmodel::dialogs::spec(s.dialog().unwrap(), &s);
    assert_eq!(
        (
            spec.title.as_str(),
            spec.body.clone(),
            spec.primary.label.as_str(),
            spec.primary.disabled.as_deref()
        ),
        ("Create enclosure", Vec::<String>::new(), "Create", None)
    );
    // C45-p1#6: the part line names the starter's enclosure, whole.
    let describe = s.describe();
    let part_line: Vec<&str> = describe
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("field \"Enclosure part\""))
        .collect();
    assert_eq!(
        part_line,
        ["field \"Enclosure part\": \"EW-GEN-ENC-1 · Wall enclosure 600 × 800 × 250 mm\""]
    );
    s.shutdown();
}

/// The description's line of the dialog's Create button.
fn create_line(s: &ShellModel) -> String {
    s.describe()
        .lines()
        .find(|l| l.trim_start().starts_with("button \"Create\""))
        .expect("the Create button's line")
        .to_owned()
}

/// C44-p2#1: on 902 at a scale of 1:4 000 000 000 with its datum 10⁹ mm away, every click lies far beyond the `i64`
/// range on the surface; Draw rail's two clicks and Draw duct's finish are refused by the tool in words, before the
/// command, and nothing panics (ID-1106: "The draw tools refuse a point outside the drawable range before they send
/// the command, with \"That point lies outside the drawing area.\"").
#[test]
fn far_points_of_the_draw_tools_are_refused_in_words() {
    let tmp = Tmp::new("far");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [{"ref": "folio:902"}],
            "path": "scale", "value": {"one_to": 4_000_000_000_u32}}}),
    );
    done(&mut s, "select.set", json!({"ids": []}));
    done(
        &mut s,
        "props.set",
        json!({"row": "page/derived.datum", "value": "1000000000, 1000000000"}),
    );
    let rev = s.revision();
    let far_refused = Outcome::Refused {
        reason_key: key!("reason-point-outside-drawing"),
    };
    let a = (100_000_000, 100_000_000);
    let b = (300_000_000, 100_000_000);
    done(&mut s, "tool.rail_draw", json!({}));
    common::click(&mut s, a, json!({}));
    done(&mut s, "canvas.pointer", at(b, json!({})));
    let out = s.dispatch("canvas.press", &at(b, json!({})));
    assert_eq!(out, far_refused);
    assert_eq!(s.tool_notice(), Some(FAR_REFUSAL));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.duct_draw", json!({}));
    common::click(&mut s, a, json!({}));
    common::click(&mut s, b, json!({}));
    let out = s.dispatch("tool.finish", &json!({}));
    assert_eq!(out, far_refused);
    assert_eq!(s.tool_notice(), Some(FAR_REFUSAL));
    assert_eq!(s.revision(), rev, "nothing drawn");
    s.shutdown();
}

/// The draw tools' refusal of a point beyond the drawable range (ID-1106), whole.
const FAR_REFUSAL: &str = "That point lies outside the drawing area.";

/// The texts of the draw tools' overlay labels.
fn overlay_labels(s: &ShellModel) -> Vec<String> {
    s.tool_overlay()
        .items
        .iter()
        .filter_map(|i| match i {
            ew_viewmodel::tools::OverlayItem::Label { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// A click at the surface point (mm) with Shift (the snap lines suspended, the grid kept).
fn free_click_mm(shell: &mut ShellModel, u: f64, v: f64) -> Outcome {
    let p = sheet_mm(shell, u, v);
    let mods = json!({"shift": true});
    done(shell, "canvas.pointer", at(p, mods.clone()));
    let out = shell.dispatch("canvas.press", &at(p, mods.clone()));
    done(shell, "canvas.release", at(p, mods));
    out
}

/// C44-p2#5: the draw tools' hints, chips and refusals, each whole: the rail's two hints and its length chip, the
/// duct's two hints and its size chip, the options without a draw tool, the 256-point cap, the one-point duct.
#[test]
fn the_draw_tools_texts_read_whole() {
    let tmp = Tmp::new("draw-texts");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    assert_eq!(
        refused(&mut s, "layout.draw_options", json!({"part": "x"})),
        "reason-no-draw-tool"
    );
    assert_eq!(
        ew_viewmodel::strings::text(key!("reason-no-draw-tool")),
        "Choose Rail or Duct first"
    );
    done(&mut s, "tool.rail_draw", json!({}));
    assert_eq!(s.tool_hint(), "Click the rail's start; Escape cancels");
    click_mm(&mut s, 40.0, 740.0);
    assert_eq!(
        s.tool_hint(),
        "Click the rail's end; Backspace drops the start; Escape cancels"
    );
    let p = sheet_mm(&s, 610.0, 740.0);
    done(&mut s, "canvas.pointer", at(p, json!({})));
    assert_eq!(overlay_labels(&s), ["570 mm"]);
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.duct_draw", json!({}));
    assert_eq!(s.tool_hint(), "Click the duct's first point; Escape cancels");
    assert_eq!(free_click_mm(&mut s, 100.0, 100.0), Outcome::Done);
    assert_eq!(
        s.tool_hint(),
        "Click the next point; a double click or Enter finishes; Backspace removes the last point"
    );
    let p = sheet_mm(&s, 300.0, 100.0);
    done(&mut s, "canvas.pointer", at(p, json!({"shift": true})));
    // Golden A's duct part gives no size: the card asks for it, 40 × 60 mm (C44-p4#3: the numbers written out).
    assert_eq!(
        s.draw_card().and_then(|c| c.size),
        Some(("40".to_owned(), "60".to_owned()))
    );
    // The card's texts (`tool_view.rs`), whole.
    assert_eq!(
        [
            ew_viewmodel::strings::text(key!("draw-options-no-part")),
            ew_viewmodel::strings::text(key!("draw-options-width")),
            ew_viewmodel::strings::text(key!("draw-options-height")),
        ],
        ["No part", "Width (mm)", "Height (mm)"]
    );
    assert_eq!(overlay_labels(&s), [DUCT_CHIP]);
    // 256 points in a staircase; the 257th is refused.
    for i in 1..256 {
        let (u, v) = (
            100.0 + 5.0 * f64::from(i / 2 + i % 2),
            100.0 + 5.0 * f64::from(i / 2),
        );
        assert_eq!(free_click_mm(&mut s, u, v), Outcome::Done, "point {i}");
    }
    let out = free_click_mm(&mut s, 900.0, 740.0);
    assert_eq!(
        out,
        Outcome::Refused {
            reason_key: key!("reason-duct-vertices")
        }
    );
    assert_eq!(s.tool_notice(), Some("A duct takes at most 256 points"));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.duct_draw", json!({}));
    assert_eq!(free_click_mm(&mut s, 100.0, 100.0), Outcome::Done);
    assert_eq!(
        refused(&mut s, "tool.finish", json!({})),
        "reason-duct-two-points"
    );
    assert_eq!(s.tool_notice(), Some("A duct needs two points"));
    // The refusals of a rail tool without a usable part (no fixture holds such a project).
    assert_eq!(
        ew_viewmodel::strings::text(key!("reason-no-rail-part")),
        "Add a rail part to the project first"
    );
    assert_eq!(
        ew_viewmodel::strings::text(key!("reason-rail-no-profile")),
        "The rail part has no profile; choose a rail part with a rail section."
    );
    s.shutdown();
}

/// The size chip of Draw duct with Golden A's duct part, which gives no size: the card's default 40 × 60 mm.
const DUCT_CHIP: &str = "40 \u{d7} 60 mm";

/// C44-p2#5: Auto-fill rail with one marked device: the dialog's line for one device and the toast without notices;
/// a rail that is not on the page is refused in words.
#[test]
fn auto_fill_one_marked_device_says_so_and_toasts_without_notices() {
    let tmp = Tmp::new("fill-one");
    let mut s = tmp.shell_in(Mode::Gui, true, "902");
    let t = panel(&s);
    let p = t.placements.iter().find(|p| p.tag == "-101F1").unwrap().id;
    done(
        &mut s,
        "command.run",
        json!({"cmd": "layout.remove", "params": {"placement": p.to_string()}}),
    );
    done(&mut s, "navigator.show", json!({"kind": "unplaced"}));
    let key = unplaced_key(&s, "-101F1");
    done(&mut s, "navigator.mark", json!({"kind": "unplaced", "key": key}));
    let Outcome::Failed { message } =
        s.dispatch("layout.auto_fill_rail", &json!({"rail": "Rail 9", "order": "dt"}))
    else {
        panic!("refused")
    };
    assert_eq!(message, "The page has no rail; draw one first.");
    done(&mut s, "layout.auto_fill_rail", json!({}));
    let spec = ew_viewmodel::dialogs::spec(s.dialog().unwrap(), &s);
    assert_eq!(spec.body, ["Fills with the device marked in Unplaced devices."]);
    assert_eq!(s.submit_form(), Outcome::Done);
    let toast = s
        .notifications()
        .visible()
        .last()
        .map(|n| n.text.clone())
        .expect("a toast");
    assert_eq!(toast, "Placed 1 device on rail 1.");
    s.shutdown();
}

/// C44-p2#2: the draw tools' part offers are computed when the tool is armed, and a second frame with the tool armed
/// (the options card read again) reuses them; a new revision computes them once more.
#[test]
fn the_draw_tools_part_offers_are_kept_between_frames() {
    let tmp = Tmp::new("offers");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    let calls = |s: &ShellModel| s.session_calls().layout_part_offers;
    let before = calls(&s);
    done(&mut s, "tool.rail_draw", json!({}));
    assert_eq!(calls(&s), before + 1, "armed");
    let card = s.draw_card().expect("the options card");
    assert_eq!(s.draw_card(), Some(card), "a second frame");
    assert_eq!(calls(&s), before + 1, "the frames reuse the offers");
    let number = s.draw_card().unwrap().parts[0].clone();
    done(&mut s, "layout.draw_options", json!({"part": number}));
    assert_eq!(calls(&s), before + 2, "the options changed");
    s.draw_card();
    assert_eq!(calls(&s), before + 2);
    // A new revision (an edit, here a page's title) computes them once more, once.
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [{"ref": "folio:101"}],
            "path": "title", "value": "Offers"}}),
    );
    s.draw_card();
    assert_eq!(calls(&s), before + 3, "a new revision");
    s.draw_card();
    assert_eq!(calls(&s), before + 3);
    s.shutdown();
}

/// C44-p4#3: the Surface row's choices are the surfaces by their IDs (the shell's choose-by-ID arm): each item is a
/// surface's words with its ID as the value, and choosing one commits `page.surface_view.set` with that ID.
#[test]
fn the_surface_row_chooses_a_surface_by_its_id() {
    let tmp = Tmp::new("surface-choice");
    let mut s = tmp.shell_in(Mode::Gui, true, "902");
    done(&mut s, "layout.enclosure_new", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("{:?}", s.dialog())
    };
    let part = form
        .fields
        .iter()
        .find(|f| f.name == "part")
        .unwrap()
        .value
        .clone();
    done(&mut s, "ui.dismiss", json!({}));
    done(
        &mut s,
        "structure.new",
        json!({"kind": "location", "designation": "J-E-999"}),
    );
    done(
        &mut s,
        "layout.enclosure_new",
        json!({"location": "+J-E-999", "part": part}),
    );
    done(&mut s, "page.goto", json!({"folio": "902"}));
    done(&mut s, "select.set", json!({"ids": []}));
    done(&mut s, "props.edit", json!({"row": "page/derived.surface"}));
    let items = s.properties().choices().expect("the surfaces").items.clone();
    let words: Vec<&str> = items.iter().map(|(w, _)| w.as_str()).collect();
    assert_eq!(words, SURFACE_WORDS);
    for (_, v) in &items {
        let id = v.as_str().expect("an ID");
        assert!(Uuid::parse_str(id).is_ok(), "{id}");
    }
    let i = items.iter().position(|(w, _)| w != "-UC1 plate").unwrap();
    let chosen = items[i].1.clone();
    done(&mut s, "props.choose", json!({"index": i}));
    let step = s.recorded_commands().last().unwrap();
    assert_eq!(step.cmd, "page.surface_view.set");
    assert_eq!(step.params["surface"], chosen);
    assert_eq!(row_text(&s, "page/derived.surface"), items[i].0);
    s.shutdown();
}

/// The Surface row's choices on Golden A with the enclosure of +J-E-999 added.
const SURFACE_WORDS: [&str; 2] = ["-UC1 plate", "-UC2 plate"];
