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
    let cables = saved.export_csv(Export::Cables).expect("the cable overview");
    assert!(cables.contains("EW-GEN-CBL-2"), "the cable overview:\n{cables}");
}
