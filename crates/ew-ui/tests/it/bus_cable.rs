// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A bus line gets its cable and its plugs (doc 10 §2.7): Cable… on a selected bus line opens the bus line's form,
//! never a refusal of a wire's cores. The window's own path on a new project.

use crate::own_parts::{new_project, place_box, press_cp};
use crate::persona_round::key;
use crate::support::Tmp;

use ew_ui::ew_viewmodel::ew_api::{BomMode, Export, Session};
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
use serde_json::{Value, json};

/// The value of the option of form field `name` whose words contain `words`.
/// A persona run read a bus line's kind as `{"bus":{}}`: a connection's Kind reads in words, Bus line.
#[test]
fn a_bus_lines_kind_reads_in_words() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    let cpu = place_box(&mut d, "EW-GEN-CPU-2", (20, 15), (30, 22));
    let rio = place_box(&mut d, "EW-GEN-RIO-1", (60, 15), (70, 22));
    let dp = |labels: &[String]| {
        labels
            .iter()
            .find(|l| !l.ends_with(":P1"))
            .cloned()
            .expect("a DP socket")
    };
    d.input(&key("W"));
    d.settle();
    press_cp(&mut d, &dp(&cpu));
    press_cp(&mut d, &dp(&rio));
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
    let line = d
        .shell_mut()
        .canvas()
        .expect("the page")
        .page()
        .tables
        .connections[0]
        .id
        .to_string();
    assert_eq!(
        d.shell_mut().dispatch("select.set", &json!({"ids": [line]})),
        Outcome::Done
    );
    assert_eq!(
        d.shell_mut().dispatch("props.show", &json!({"on": true})),
        Outcome::Done
    );
    d.settle();
    let sheet = d.shell_mut().properties().sheet().expect("the sheet").clone();
    let kind = ew_ui::ew_viewmodel::properties::find_row(&sheet, "connection/kind")
        .map(|(_, r)| ew_ui::ew_viewmodel::properties::value_text(r));
    assert_eq!(kind.as_deref(), Some("Bus line"));
}

/// A persona run met a banner about mdbook where the guide is not beside the program: in the offscreen visual driver
/// a guide link names the page's file under `book/` in a toast instead, with the section F1 chose (rc.3).
#[test]
fn a_guide_link_names_its_page_in_a_visual_run() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&key("F1"));
    d.settle();
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.starts_with("The guide's page: book/"), "{toast}");
    assert!(toast.contains(".html#"), "{toast}");
}

fn option(d: &mut ew_ui::visual::Driver, name: &str, words: &str) -> String {
    let Some(Dialog::Form(form)) = d.shell_mut().dialog() else {
        panic!("a form");
    };
    let field = form.fields.iter().find(|f| f.name == name).expect("the field");
    field
        .options
        .iter()
        .find(|(_, w)| w.contains(words))
        .unwrap_or_else(|| panic!("no {words} in {:?}", field.options))
        .0
        .clone()
}

/// The owner's job: a PROFIBUS line between a CPU and a remote I/O head is to be a DP cable with its plugs. Cable… on
/// the selected line opens its form, which offers the PROFIBUS cable first and only plugs made for PROFIBUS. Applied
/// with a plug with its termination switched on at one end and an M12 plug at the other, the line is linked to a new
/// cable with the part's cores, the parts list counts the cable and both plugs, and the cable overview lists the cable.
#[test]
fn a_bus_line_takes_a_dp_cable_and_a_plug_at_each_end() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    for (kind, folio) in [("parts_summary", "901"), ("cable_overview_diagrams", "902")] {
        let block = json!({"kind": kind, "folio": folio});
        assert_eq!(d.shell_mut().dispatch("report_block.new", &block), Outcome::Done);
    }
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    let cpu = place_box(&mut d, "EW-GEN-CPU-2", (20, 15), (30, 22));
    let rio = place_box(&mut d, "EW-GEN-RIO-1", (60, 45), (70, 52));
    let dp = |labels: &[String]| {
        labels
            .iter()
            .find(|l| !l.ends_with(":P1"))
            .cloned()
            .unwrap_or_else(|| panic!("a DP socket in {labels:?}"))
    };
    let (from, to) = (dp(&cpu), dp(&rio));
    d.input(&key("W"));
    d.settle();
    press_cp(&mut d, &from);
    press_cp(&mut d, &to);
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
    let line = {
        let c = d.shell_mut().canvas().expect("the page");
        let lines = &c.page().tables.connections;
        assert_eq!(lines.len(), 1, "one bus line");
        lines[0].id.to_string()
    };
    assert_eq!(
        d.shell_mut().dispatch("select.set", &json!({"ids": [line]})),
        Outcome::Done
    );
    d.settle();
    // The Draw tab's Cable… on the line: its own form.
    assert_eq!(
        d.shell_mut().dispatch("cable.from_wires", &json!({})),
        Outcome::Done
    );
    d.settle();
    {
        let Some(Dialog::Form(form)) = d.shell_mut().dialog() else {
            panic!("the bus line's Cable form");
        };
        assert_eq!(form.action, "bus.cable");
        let part = form
            .fields
            .iter()
            .find(|f| f.name == "part")
            .expect("the cable part");
        assert!(
            part.options[0].1.starts_with("EW-GEN-CBL-2"),
            "the PROFIBUS cable first: {:?}",
            part.options
        );
        assert_eq!(part.value, part.options[0].0, "and chosen");
        assert!(
            part.options
                .iter()
                .any(|(_, w)| w.contains("not made for PROFIBUS DP")),
            "another cable says it is not made for the bus: {:?}",
            part.options
        );
        let plugs = &form
            .fields
            .iter()
            .find(|f| f.name == "plug_a")
            .expect("a plug")
            .options;
        assert!(
            plugs.iter().all(|(_, w)| !w.contains("RJ45")),
            "only plugs for PROFIBUS: {plugs:?}"
        );
    }
    let switched = option(&mut d, "plug_a", "EW-GEN-PLG-1");
    let m12 = option(&mut d, "plug_b", "EW-GEN-PLG-5");
    for (name, value) in [
        ("plug_a", switched.as_str()),
        ("termination_a", "true"),
        ("plug_b", m12.as_str()),
        ("length", "12,5"),
    ] {
        d.shell_mut().set_form_field(name, value);
    }
    assert_eq!(d.shell_mut().submit_form(), Outcome::Done);
    d.settle();
    assert!(d.shell_mut().dialog().is_none(), "the form closes");
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.ends_with("carries the bus line."), "{toast}");

    assert_eq!(d.shell_mut().dispatch("file.save", &json!({})), Outcome::Done);
    let (saved, _) = Session::open_read_only(&tmp.0.join("p.ew"), "test").unwrap();
    let q = saved.query();
    let kind = q
        .connections()
        .iter()
        .map(|(_, c)| serde_json::to_value(&c.kind).unwrap())
        .next()
        .expect("the line");
    let cable = kind["bus"]["cable"]
        .as_str()
        .expect("the line has its cable")
        .to_owned();
    assert_eq!(kind["bus"]["plug_a"]["termination"], json!("on"));
    assert_eq!(
        kind["bus"]["plug_b"].get("termination"),
        None,
        "the M12 plug has no switch"
    );
    let cores = q
        .conductors()
        .iter()
        .filter(|(_, c)| c.cable.to_string() == cable)
        .count();
    assert!(cores > 0, "the cable has its part's cores");
    let length = q
        .devices()
        .iter()
        .find(|(id, _)| id.to_string() == cable)
        .and_then(|(_, dev)| serde_json::to_value(&dev.aspects.cable).ok())
        .unwrap_or(Value::Null);
    assert_eq!(length["length"], json!({"manual": 12_500_000_000_i64}));
    let bom = saved
        .export_csv(Export::Bom(BomMode::Component))
        .expect("the parts list");
    for number in ["EW-GEN-CBL-2", "EW-GEN-PLG-1", "EW-GEN-PLG-5"] {
        assert!(bom.contains(number), "{number} in the parts list:\n{bom}");
    }
    // rc.5: the cable counts by its length.
    assert!(
        bom.lines()
            .any(|l| l.contains("EW-GEN-CBL-2") && l.contains(",12.5,m,")),
        "{bom}"
    );
    let cables = saved.export_csv(Export::Cables).expect("the cable overview");
    assert!(cables.contains("EW-GEN-CBL-2"), "the cable overview:\n{cables}");
}

/// A line drawn on the bus layout between the starter CPU's and the remote I/O head's sockets, DP (`dp`) or PROFINET,
/// and selected; its ID.
fn bus_line(d: &mut ew_ui::visual::Driver, cpu: &[String], rio: &[String], dp: bool) -> String {
    let pick = |labels: &[String]| {
        labels
            .iter()
            .find(|l| l.ends_with(":P1") != dp)
            .cloned()
            .unwrap_or_else(|| panic!("a socket in {labels:?}"))
    };
    let lines = |d: &mut ew_ui::visual::Driver| -> Vec<String> {
        d.shell_mut()
            .canvas()
            .expect("the page")
            .page()
            .tables
            .connections
            .iter()
            .map(|c| c.id.to_string())
            .collect()
    };
    let before = lines(d);
    d.input(&key("W"));
    d.settle();
    press_cp(d, &pick(cpu));
    press_cp(d, &pick(rio));
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
    let line = lines(d)
        .into_iter()
        .find(|c| !before.contains(c))
        .expect("the new line");
    assert_eq!(
        d.shell_mut().dispatch("select.set", &json!({"ids": [line]})),
        Outcome::Done
    );
    d.settle();
    line
}

/// The value of form field `name`.
fn field_value(d: &mut ew_ui::visual::Driver, name: &str) -> String {
    let Some(Dialog::Form(form)) = d.shell_mut().dialog() else {
        panic!("a form");
    };
    form.fields
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.value.clone())
        .expect("the field")
}

/// A new project with a bus layout page (folio 2) holding the starter CPU and remote I/O head; their sockets.
fn bus_page(tmp: &Tmp) -> (ew_ui::visual::Driver, Vec<String>, Vec<String>) {
    let mut d = new_project(tmp);
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    let cpu = place_box(&mut d, "EW-GEN-CPU-2", (20, 15), (30, 22));
    let rio = place_box(&mut d, "EW-GEN-RIO-1", (60, 45), (70, 52));
    (d, cpu, rio)
}

/// H3 (rc.5): a cable chosen for a line on a bus layout gets the next tag by the project's numbering, with the page as
/// a cable marked on a wiring page has it, and its mark at the line (a marker listing the line); a second line gets
/// the next tag. Each tag stands once on the page.
#[test]
fn a_bus_cable_takes_the_next_tag_of_its_page_and_its_mark() {
    let tmp = Tmp::new();
    let (mut d, cpu, rio) = bus_page(&tmp);
    let mut tags = Vec::new();
    for dp in [true, false] {
        let line = bus_line(&mut d, &cpu, &rio, dp);
        assert_eq!(
            d.shell_mut().dispatch("cable.from_wires", &json!({})),
            Outcome::Done
        );
        d.settle();
        let proposed = field_value(&mut d, "tag");
        assert_eq!(d.shell_mut().submit_form(), Outcome::Done);
        d.settle();
        let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
        assert!(toast.contains(&proposed), "{proposed}: {toast}");
        let marked = crate::own_parts::records(&mut d).into_iter().any(|r| {
            r["kind"]["wire_marker"]["connections"]
                .as_array()
                .is_some_and(|c| c.contains(&json!(line)))
        });
        assert!(marked, "the line {line} is in its cable's mark");
        tags.push(proposed);
    }
    assert_eq!(tags, ["-2W1", "-2W2"], "the page's next tags, one each");
    let texts = crate::own_parts::page_texts(&mut d);
    for tag in &tags {
        assert_eq!(
            texts.iter().filter(|t| *t == tag).count(),
            1,
            "{tag} once on the page: {texts:?}"
        );
    }
}

/// H4 (rc.5): New bus cable… and New plug… in the bus line's Cable form make the parts in a short form, the cable's
/// cores as PROFIBUS has them, the plug angled with its resistor; each is chosen in the form at once, and Apply puts
/// both into the parts list (H15), the cable by its length.
#[test]
fn a_new_bus_cable_and_plug_are_made_in_the_cable_form() {
    let tmp = Tmp::new();
    let (mut d, cpu, rio) = bus_page(&tmp);
    let block = json!({"kind": "parts_summary", "folio": "901"});
    assert_eq!(d.shell_mut().dispatch("report_block.new", &block), Outcome::Done);
    bus_line(&mut d, &cpu, &rio, true);
    assert_eq!(
        d.shell_mut().dispatch("cable.from_wires", &json!({})),
        Outcome::Done
    );
    d.settle();
    let part_field = |d: &mut ew_ui::visual::Driver, field: &str, value: &str| {
        let p = json!({"field": field, "value": value});
        assert_eq!(d.shell_mut().dispatch("part.field", &p), Outcome::Done);
    };
    let new_cable = option(&mut d, "part", "New bus cable");
    d.shell_mut().set_form_field("part", &new_cable);
    {
        let Some(Dialog::PartEdit(e)) = d.shell_mut().dialog() else {
            panic!("the New bus cable form");
        };
        assert_eq!(e.title(), "New bus cable");
        assert_eq!(e.draft.text("cable.bus"), "profibus_dp");
        assert_eq!(
            e.draft.cable.table,
            [
                ("A".to_owned(), "GN".to_owned()),
                ("B".to_owned(), "RD".to_owned())
            ]
        );
        assert_eq!(e.draft.text("cable.shield"), "true");
    }
    for (f, v) in [
        ("manufacturer", "Maker"),
        ("type_designation", "DP trunk"),
        ("part_number", "BUS-CBL-9"),
        ("cable.diameter", "8"),
    ] {
        part_field(&mut d, f, v);
    }
    assert_eq!(d.shell_mut().dispatch("part.save", &json!({})), Outcome::Done);
    d.settle();
    let cable = field_value(&mut d, "part");
    assert_eq!(
        option(&mut d, "part", "BUS-CBL-9"),
        cable,
        "the new cable is chosen"
    );
    let new_plug = option(&mut d, "plug_b", "New plug");
    d.shell_mut().set_form_field("plug_b", &new_plug);
    {
        let Some(Dialog::PartEdit(e)) = d.shell_mut().dialog() else {
            panic!("the New plug form");
        };
        assert_eq!(e.title(), "New bus plug");
    }
    for (f, v) in [
        ("manufacturer", "Maker"),
        ("part_number", "BUS-PLG-9"),
        ("plug.angled", "true"),
        ("bus.0.termination", "off"),
    ] {
        part_field(&mut d, f, v);
    }
    {
        let Some(Dialog::PartEdit(e)) = d.shell_mut().dialog() else {
            panic!("the New plug form");
        };
        let words = e.draft.text("description");
        assert!(
            words.contains("PROFIBUS DP")
                && words.contains("angled")
                && words.contains("terminating resistor"),
            "{words}"
        );
    }
    assert_eq!(d.shell_mut().dispatch("part.save", &json!({})), Outcome::Done);
    d.settle();
    let plug = field_value(&mut d, "plug_b");
    assert_eq!(
        option(&mut d, "plug_b", "BUS-PLG-9"),
        plug,
        "the new plug is chosen"
    );
    d.shell_mut().set_form_field("length", "7.5");
    assert_eq!(d.shell_mut().submit_form(), Outcome::Done);
    d.settle();
    assert_eq!(d.shell_mut().dispatch("file.save", &json!({})), Outcome::Done);
    let (saved, _) = Session::open_read_only(&tmp.0.join("p.ew"), "test").unwrap();
    let bom = saved
        .export_csv(Export::Bom(BomMode::Component))
        .expect("the parts list");
    let row = |number: &str| {
        bom.lines()
            .find(|l| l.contains(number))
            .unwrap_or_else(|| panic!("{number} in the parts list:\n{bom}"))
            .to_owned()
    };
    assert!(
        row("BUS-CBL-9").contains(",7.5,m,"),
        "the cable by its length: {bom}"
    );
    assert!(row("BUS-PLG-9").contains(",1,pc,"), "{bom}");
}

/// The parts list page's part numbers and, with `nav` (it takes the keyboard), the Parts navigator's part rows, read in
/// the window as they stand.
fn parts_now(d: &mut ew_ui::visual::Driver, nav: bool) -> (Vec<String>, Vec<String>) {
    assert_eq!(
        d.shell_mut().dispatch("page.goto", &json!({"folio": "901"})),
        Outcome::Done
    );
    d.settle();
    let page = crate::own_parts::page_texts(d)
        .into_iter()
        .filter(|t| t.starts_with("EW-GEN-"))
        .collect();
    if !nav {
        assert_eq!(
            d.shell_mut().dispatch("page.goto", &json!({"folio": "2"})),
            Outcome::Done
        );
        d.settle();
        return (page, Vec::new());
    }
    assert_eq!(
        d.shell_mut().dispatch("navigator.show", &json!({ "kind": "parts" })),
        Outcome::Done
    );
    d.settle();
    let nav = d
        .shell_mut()
        .nav(ew_ui::ew_viewmodel::ew_api::output::NavKind::Parts)
        .and_then(|n| n.table())
        .map(|t| {
            t.nodes
                .iter()
                .map(|n| n.label.clone())
                .filter(|l| l.starts_with("EW-GEN-"))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        d.shell_mut().dispatch("page.goto", &json!({"folio": "2"})),
        Outcome::Done
    );
    d.settle();
    (page, nav)
}

/// H15 (rc.5): with the parts list page open before, a bus line's cable and plugs chosen in the Cable form are on that
/// page and in the Parts navigator at once, without a save; a plug at an end without a device (an interruption point)
/// counts too, with the cable.
#[test]
fn the_parts_list_shows_a_bus_lines_plugs_at_once() {
    let tmp = Tmp::new();
    let (mut d, cpu, rio) = bus_page(&tmp);
    let block = json!({"kind": "parts_summary", "folio": "901"});
    assert_eq!(d.shell_mut().dispatch("report_block.new", &block), Outcome::Done);
    d.settle();
    let (page, _) = parts_now(&mut d, false);
    assert!(!page.iter().any(|t| t.contains("PLG")), "{page:?}");
    bus_line(&mut d, &cpu, &rio, true);
    assert_eq!(
        d.shell_mut().dispatch("cable.from_wires", &json!({})),
        Outcome::Done
    );
    d.settle();
    let (a, b) = (
        option(&mut d, "plug_a", "EW-GEN-PLG-5"),
        option(&mut d, "plug_b", "EW-GEN-PLG-1"),
    );
    d.shell_mut().set_form_field("plug_a", &a);
    d.shell_mut().set_form_field("plug_b", &b);
    assert_eq!(d.shell_mut().submit_form(), Outcome::Done);
    d.settle();
    let (page, _) = parts_now(&mut d, false);
    for number in ["EW-GEN-CBL-2", "EW-GEN-PLG-5", "EW-GEN-PLG-1"] {
        assert!(page.iter().any(|t| t == number), "{number} on the open page: {page:?}");
    }
    // A line from the CPU's DP to an interruption point: its plug there is the cable's.
    let before = crate::own_parts::cp_labels(&mut d);
    assert_eq!(d.shell_mut().dispatch("tool.ip", &json!({})), Outcome::Done);
    d.settle();
    crate::own_parts::cursor_k(&mut d, (45, 15));
    assert_eq!(
        d.shell_mut().dispatch("canvas.activate", &json!({})),
        Outcome::Done
    );
    d.settle();
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
    let ip = crate::own_parts::cp_labels(&mut d)
        .into_iter()
        .find(|l| !before.contains(l))
        .expect("the interruption point");
    d.input(&key("W"));
    d.settle();
    press_cp(&mut d, &ip);
    press_cp(&mut d, cpu.iter().find(|l| !l.ends_with(":P1")).expect("DP"));
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
    let lines: Vec<String> = d
        .shell_mut()
        .canvas()
        .expect("the page")
        .page()
        .tables
        .connections
        .iter()
        .map(|c| c.id.to_string())
        .collect();
    assert_eq!(lines.len(), 2, "two bus lines");
    let new_line = lines
        .into_iter()
        .find(|l| {
            !crate::own_parts::records(&mut d)
                .iter()
                .any(|r| r["id"] == json!(l) && r["kind"]["bus"]["cable"].is_string())
        })
        .expect("the line without a cable");
    assert_eq!(
        d.shell_mut().dispatch("select.set", &json!({"ids": [new_line]})),
        Outcome::Done
    );
    d.settle();
    assert_eq!(
        d.shell_mut().dispatch("cable.from_wires", &json!({})),
        Outcome::Done
    );
    d.settle();
    let plug = option(&mut d, "plug_a", "EW-GEN-PLG-2");
    d.shell_mut().set_form_field("plug_a", &plug);
    assert_eq!(d.shell_mut().submit_form(), Outcome::Done);
    d.settle();
    let (page, nav) = parts_now(&mut d, true);
    for number in ["EW-GEN-CBL-2", "EW-GEN-PLG-5", "EW-GEN-PLG-1", "EW-GEN-PLG-2"] {
        assert!(page.iter().any(|t| t == number), "{number} on the open page: {page:?}");
        assert!(nav.iter().any(|t| t == number), "{number} in the navigator: {nav:?}");
    }
}
