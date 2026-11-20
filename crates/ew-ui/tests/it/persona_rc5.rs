// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The persona runs of rc.5 (a whole cabinet on PROFINET), on the window's own path: the Terminal dialog adds to the
//! page's strip and keeps New strip first, a panel layout page says how its devices reach the plate, and a parts list
//! that runs on says where.

use crate::own_parts::{cursor_k, new_project};
use crate::persona_round::key;
use crate::support::Tmp;

use ew_viewmodel::shell::{Dialog, Outcome};
use ew_ui::visual::Driver;
use serde_json::json;

/// One terminal of strip `strip` (`new` for a new one) placed at grid point `k` of the page shown.
fn place_terminal(d: &mut Driver, strip: &str, k: (i64, i64)) {
    let p = json!({"strip": strip, "count": 1});
    assert_eq!(d.shell_mut().dispatch("tool.place_terminal", &p), Outcome::Done);
    d.settle();
    cursor_k(d, k);
    assert_eq!(
        d.shell_mut().dispatch("canvas.activate", &json!({})),
        Outcome::Done
    );
    d.settle();
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
}

/// The Terminal dialog's strip field: its value's words and its options' words.
fn strip_field(d: &mut Driver) -> (String, Vec<String>) {
    let Some(Dialog::Form(form)) = d.shell_mut().dialog().cloned() else {
        panic!("the Terminal dialog");
    };
    let f = form.fields.iter().find(|f| f.name == "strip").expect("Strip");
    let words = |v: &str| {
        f.options
            .iter()
            .find(|(o, _)| o == v)
            .map(|(_, w)| w.clone())
            .unwrap_or_default()
    };
    (
        words(&f.value),
        f.options.iter().map(|(_, w)| w.clone()).collect(),
    )
}

/// rc.5 (the cabinet run, twice): the dialog chose the strip used last on another page, and New strip moved in the
/// list. Now it chooses the strip of this page's terminals, New strip first, and a page without terminals a new strip.
#[test]
fn the_terminal_dialog_adds_to_the_pages_strip_with_new_strip_first() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    place_terminal(&mut d, "new", (10, 10));
    let page = json!({"folio": "2", "type": "multi_line", "title": "Field"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    place_terminal(&mut d, "new", (10, 10));
    assert_eq!(
        d.shell_mut().dispatch("page.goto", &json!({"folio": "1"})),
        Outcome::Done
    );
    d.settle();
    assert_eq!(
        d.shell_mut().dispatch("tool.place_terminal", &json!({})),
        Outcome::Done
    );
    d.settle();
    let (chosen, options) = strip_field(&mut d);
    assert!(options[0].starts_with("New strip"), "{options:?}");
    assert!(chosen.contains("X1"), "page 1's strip: {chosen} of {options:?}");
    d.input(&key("Escape"));
    d.settle();
    let page = json!({"folio": "3", "type": "multi_line", "title": "Spare"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    assert_eq!(
        d.shell_mut().dispatch("tool.place_terminal", &json!({})),
        Outcome::Done
    );
    d.settle();
    let (chosen, options) = strip_field(&mut d);
    assert_eq!(chosen, options[0], "a page without terminals: a new strip");
}

/// rc.5 (the cabinet run): on the layout page with its enclosure he did not find how the devices reach the plate, and
/// trying to place one said to draw on a schematic page. Now the page's line says the next step, and the refusal says
/// what to do instead.
#[test]
fn a_panel_layout_page_says_how_the_devices_reach_the_plate() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let _ = crate::own_parts::place_box(&mut d, "EW-GEN-CPU-2", (20, 15), (30, 22));
    let page = json!({"folio": "301", "type": "panel_layout", "title": "Cabinet layout"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let enclosure = json!({"location": "new", "new_location": "CAB1", "part": "EW-GEN-ENC-1"});
    assert_eq!(
        d.shell_mut().dispatch("layout.enclosure_new", &enclosure),
        Outcome::Done
    );
    d.settle();
    // The CPU has no location yet: Unplaced devices lists none, so the location comes first.
    let prompt = ew_viewmodel::command_bar::hud(d.shell_mut()).prompt;
    assert!(
        prompt.contains("+CAB1") && prompt.contains("Unplaced devices"),
        "{prompt}"
    );
    let out = d.shell_mut().dispatch("tool.place", &json!({}));
    let words = match out {
        Outcome::Refused { reason_key } => ew_viewmodel::strings::text(reason_key).to_owned(),
        other => format!("{other:?}"),
    };
    assert!(words.contains("Unplaced devices"), "{words}");
}

/// rc.5 (the cabinet run, twice): the bus plugs were on the parts list's second page, and the line at the window's foot
/// that said so did not stop him reading the first page as the whole list. Now the page's tab says "page 1 of 2", the
/// HUD says it with a button to the next page, and under the table's last row a row of the table says that the list
/// continues there and goes there on a click; the last page has no such row and a button back.
#[test]
fn a_parts_list_that_runs_on_says_where() {
    let tmp = Tmp::new();
    let golden = tmp.golden("g.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, Outcome::Done);
    assert_eq!(
        d.shell_mut().dispatch("page.goto", &json!({"folio": "901"})),
        Outcome::Done
    );
    d.input(&ew_ui::visual::Input::Wait { ms: 50 });
    let hud = |d: &mut Driver| {
        let h = ew_viewmodel::command_bar::hud(d.shell_mut());
        (
            h.prompt,
            h.buttons.into_iter().map(|b| b.label).collect::<Vec<_>>(),
        )
    };
    assert_eq!(
        hud(&mut d),
        (
            "Page 1 of 2 of this list".to_owned(),
            vec!["Next page 901.1".to_owned()]
        )
    );
    let labels: Vec<String> = d.labels().into_iter().map(|(l, _)| l).collect();
    assert!(
        labels
            .iter()
            .any(|l| l.contains("901") && l.contains("page 1 of 2")),
        "the tab: {labels:?}"
    );
    crate::own_parts::press(&mut d, "The list continues on page 901.1 ›");
    d.input(&ew_ui::visual::Input::Wait { ms: 50 });
    assert_eq!(
        hud(&mut d),
        (
            "Page 2 of 2 of this list".to_owned(),
            vec!["Previous page 901".to_owned()]
        )
    );
    assert!(
        d.shell_mut().list_foot().is_none(),
        "the last page runs on nowhere"
    );
    d.shell_mut().shutdown();
}

/// rc.5 (the cabinet run, again): the layout page's line sent him to "Unplaced devices (left rail)" among icons
/// without words, and to Properties for each of thirty devices. Now the line carries its ways on as buttons: Give
/// location +CAB1… gives the devices drawn on the wiring pages the cabinet's location in one step, after a preview that
/// leaves a device wired only to the terminals' field side unticked; then Unplaced devices opens the panel, whose rail
/// icon's tooltip has that very name.
///
/// rc.5 (the cabinet run, a third time): after Give location every tag of the wiring page printed "+Supply-" in front,
/// as the field devices beside them had no location. A drawn tag stays as it was.
#[test]
fn a_layout_page_gives_the_cabinets_devices_their_location_in_one_step() {
    use crate::own_parts::press;
    let tmp = Tmp::new();
    let mut d = cabinet_project(&tmp);
    let buttons = |d: &mut Driver| -> Vec<String> {
        let hud = ew_viewmodel::command_bar::hud(d.shell_mut());
        hud.buttons.into_iter().map(|b| b.label).collect()
    };
    assert_eq!(buttons(&mut d), ["Give location +CAB1…", "Unplaced devices"]);
    let before = tags_on(&mut d, "1");
    let back = json!({"folio": "301"});
    assert_eq!(d.shell_mut().dispatch("page.goto", &back), Outcome::Done);
    d.input(&ew_ui::visual::Input::Wait { ms: 50 });
    press(&mut d, "Give location +CAB1…");
    let rows = preview_rows(&mut d);
    for (tag, ticked) in [("-1A1", true), ("-1A2", false), ("-1X1", true)] {
        assert!(
            rows.contains(&(tag.to_owned(), ticked)),
            "{tag} {ticked}: {rows:?}"
        );
    }
    press(&mut d, "Give location");
    assert!(
        d.shell_mut().dialog().is_none(),
        "Give location closes the preview"
    );
    assert_eq!(tags_on(&mut d, "1"), before, "every drawn tag as it was");
    assert_eq!(d.shell_mut().dispatch("page.goto", &back), Outcome::Done);
    d.settle();
    let unplaced: Vec<String> = d
        .shell_mut()
        .panel_tables()
        .expect("the plate")
        .unplaced
        .iter()
        .map(|u| u.label.clone())
        .collect();
    assert!(
        unplaced.iter().any(|l| l.contains("1A1")) && !unplaced.iter().any(|l| l.contains("1A2")),
        "{unplaced:?}"
    );
    assert_eq!(buttons(&mut d), ["Unplaced devices", "Auto-fill rail…"]);
    d.input(&ew_ui::visual::Input::Wait { ms: 50 });
    press(&mut d, "Unplaced devices");
    let rail = ew_viewmodel::rail::rail(d.shell_mut());
    let entry = rail
        .left
        .iter()
        .find(|b| b.word == "unplaced")
        .expect("the rail's entry");
    assert!(entry.active, "the button shows the panel");
    assert_eq!(entry.label, "Unplaced devices", "the rail icon's tooltip");
}

/// rc.5: where a prefix has to appear (the page draws a device of a second location), the preview says so on the
/// devices it concerns before Apply, the tags then print it, and one Undo takes the step back.
#[test]
fn give_location_announces_a_location_its_tags_will_print() {
    use crate::own_parts::press;
    let tmp = Tmp::new();
    let mut d = cabinet_project(&tmp);
    // A second cabinet, +FIELD, takes the card below alone, through its own Give location.
    let page = json!({"folio": "302", "type": "panel_layout", "title": "Field box"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let enclosure = json!({"location": "new", "new_location": "FIELD", "part": "EW-GEN-ENC-1"});
    assert_eq!(
        d.shell_mut().dispatch("layout.enclosure_new", &enclosure),
        Outcome::Done
    );
    d.input(&ew_ui::visual::Input::Wait { ms: 50 });
    press(&mut d, "Give location +FIELD…");
    for (i, (tag, _)) in preview_rows(&mut d).iter().enumerate() {
        let tick = if tag == "-1A2" {
            ew_viewmodel::shell::TICKED
        } else {
            ""
        };
        d.shell_mut().set_form_table_cell(i, Some(0), tick);
    }
    press(&mut d, "Give location");
    let before = tags_on(&mut d, "1");
    assert_eq!(
        d.shell_mut().dispatch("page.goto", &json!({"folio": "301"})),
        Outcome::Done
    );
    d.input(&ew_ui::visual::Input::Wait { ms: 50 });
    press(&mut d, "Give location +CAB1…");
    let rows = preview_rows(&mut d);
    let said = "-1A1: its tag will print +CAB1 in front, as its page draws another location";
    assert!(rows.iter().any(|(k, _)| k == said), "{rows:?}");
    press(&mut d, "Give location");
    let after = tags_on(&mut d, "1");
    assert!(after.iter().any(|t| t == "+CAB1-1A1"), "{after:?}");
    assert_eq!(d.shell_mut().dispatch("edit.undo", &json!({})), Outcome::Done);
    assert_eq!(tags_on(&mut d, "1"), before, "one Undo takes the step back");
}

/// A new project like his: a card above wired to a terminal's inner side, one below wired to its field side (side 2,
/// `b`) alone, none with a location, and a layout page 301 with the enclosure of +CAB1, on show.
fn cabinet_project(tmp: &Tmp) -> Driver {
    use crate::own_parts::place_box;
    let mut d = new_project(tmp);
    let _ = place_box(&mut d, "EW-GEN-DI-1", (20, 10), (30, 16));
    let _ = place_box(&mut d, "EW-GEN-DI-1", (20, 40), (30, 46));
    place_terminal(&mut d, "new", (10, 28));
    // The card above wired to the terminal's inner side, the one below to its field side (side 2, `b`) alone.
    for (from, to) in [("-1A1:1", "-1X1:1/a"), ("-1A2:1", "-1X1:1/b")] {
        d.input(&key("W"));
        d.settle();
        crate::own_parts::press_cp(&mut d, from);
        crate::own_parts::press_cp(&mut d, to);
        d.input(&key("Escape"));
        d.input(&key("Escape"));
        d.settle();
    }
    let page = json!({"folio": "301", "type": "panel_layout", "title": "Cabinet layout"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let enclosure = json!({"location": "new", "new_location": "CAB1", "part": "EW-GEN-ENC-1"});
    assert_eq!(
        d.shell_mut().dispatch("layout.enclosure_new", &enclosure),
        Outcome::Done
    );
    // A frame of the layout page: settling alone paints none after a dispatch.
    d.input(&ew_ui::visual::Input::Wait { ms: 50 });
    d
}

/// The rows of the open Give location preview: each row's words and whether it is ticked.
fn preview_rows(d: &mut Driver) -> Vec<(String, bool)> {
    let Some(Dialog::Form(form)) = d.shell_mut().dialog().cloned() else {
        panic!("the preview");
    };
    form.table
        .expect("the devices")
        .rows
        .into_iter()
        .map(|r| (r.key, r.cells[0] == ew_viewmodel::shell::TICKED))
        .collect()
}

/// The device tags page `folio` prints (its texts that name a device of page 1), the page shown after.
fn tags_on(d: &mut Driver, folio: &str) -> Vec<String> {
    let at = json!({ "folio": folio });
    assert_eq!(d.shell_mut().dispatch("page.goto", &at), Outcome::Done);
    d.settle();
    let mut tags: Vec<String> = crate::own_parts::page_texts(d)
        .into_iter()
        .filter(|t| t.contains("-1A") || t.contains("-1X"))
        .collect();
    tags.sort();
    tags
}
