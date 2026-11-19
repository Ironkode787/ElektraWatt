// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The persona runs of rc.5 (a whole cabinet on PROFINET), on the window's own path: the Terminal dialog adds to the
//! page's strip and keeps New strip first, a panel layout page says how its devices reach the plate, and a parts list
//! that runs on says where.

use crate::own_parts::{cursor_k, new_project};
use crate::persona_round::key;
use crate::support::Tmp;

use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
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
    let prompt = ew_ui::ew_viewmodel::command_bar::hud(d.shell_mut()).prompt;
    assert!(
        prompt.contains("+CAB1") && prompt.contains("Unplaced devices"),
        "{prompt}"
    );
    let out = d.shell_mut().dispatch("tool.place", &json!({}));
    let words = match out {
        Outcome::Refused { reason_key } => ew_ui::ew_viewmodel::strings::text(reason_key).to_owned(),
        other => format!("{other:?}"),
    };
    assert!(words.contains("Unplaced devices"), "{words}");
}

/// rc.5 (the cabinet run): the bus plugs were on the parts list's second page, which he did not find. A list that runs
/// on says on its page where it goes on; its last page does not.
#[test]
fn a_parts_list_that_runs_on_says_where() {
    let tmp = Tmp::new();
    let golden = tmp.golden("g.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, Outcome::Done);
    let prompt = |d: &mut Driver, folio: &str| {
        assert_eq!(
            d.shell_mut().dispatch("page.goto", &json!({"folio": folio})),
            Outcome::Done
        );
        d.settle();
        ew_ui::ew_viewmodel::command_bar::hud(d.shell_mut()).prompt
    };
    assert_eq!(prompt(&mut d, "901"), "This list continues on page 901.1");
    assert!(!prompt(&mut d, "901.1").contains("continues"));
    d.shell_mut().shutdown();
}
