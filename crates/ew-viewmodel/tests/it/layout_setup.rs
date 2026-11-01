// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 4 through the shell on temporary copies of Golden A: Draw ▸ Layout enabled on 902 and disabled on 101
//! with its reason (PS-7); Draw rail giving R1's geometry with a diagonal second click (PS-2); Draw duct giving D1 and
//! D2 with Backspace, a double click and Enter (PS-3); a carrier dragged by Select; the Create enclosure dialog on a
//! new location of the project, its refusal on +J-E-2416, and a new panel page's Surface view centred (PS-1, the
//! project path); Properties rows committed from the shell (PS-4, PS-5); Auto-fill rail's dialog and toast (PS-6).

use crate::common;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::command_bar::{Tab, command_bar_of};
use ew_viewmodel::ew_api::output::{PanelCarrier, PanelTables, Uuid};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use serde_json::json;

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
        "Location J-E-2416 already has enclosure -UC1; a location has one enclosure."
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
    s.shutdown();
}

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

/// PS-5 through the grid: -102F1's Locked row, its Position row's refusal in words, and R1's Length row.
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
    s.shutdown();
}

/// The Create enclosure dialog on a new project from the built-in template: no location and no enclosure part yet, and
/// the dialog says so in words (the fields then take typed text).
#[test]
fn the_create_enclosure_dialog_on_a_new_project_says_what_is_missing() {
    let tmp = Tmp::new("ps1-new");
    let config = ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test");
    let mut s = ShellModel::new(Mode::Gui, config);
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("n.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(&mut s, "layout.enclosure_new", json!({}));
    let spec = ew_viewmodel::dialogs::spec(s.dialog().unwrap(), &s);
    assert_eq!(
        (
            spec.title.as_str(),
            spec.body.clone(),
            spec.primary.label.as_str()
        ),
        (
            "Create enclosure",
            vec![
                "The project has no location yet; add one with Structure node… first.".to_owned(),
                "No enclosure part is in the project or an installed library; type a part number.".to_owned(),
            ],
            "Create"
        )
    );
    s.shutdown();
}
