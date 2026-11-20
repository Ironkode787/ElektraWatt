// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New cable type… (rc.3): a cable part made where the cable is made, in a small form, by the window's own path.

use crate::support::Tmp;
use crate::wires_and_cables::{click, key, new_project, select_both, two_named_wires, two_wires};

use ew_viewmodel::shell::{Dialog, Outcome};
use ew_ui::visual::{Driver, Input};

/// Clicks the field named `label`, selects its text and types `text`.
fn fill(d: &mut Driver, label: &str, text: &str) {
    d.input(&click(
        d.find(label).unwrap_or_else(|| panic!("no field {label:?}")),
    ));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type {
        text: text.to_owned(),
    });
    d.settle();
}

/// A frame painted after a step the shell took without an input (a dispatch, a form field set).
fn shown(d: &mut Driver) {
    d.input(&Input::Hover { at: (4, 4) });
    d.settle();
}

/// The open dialog's title.
fn title(d: &mut Driver) -> String {
    match d.shell_mut().dialog() {
        Some(Dialog::PartEdit(e)) => e.title(),
        Some(Dialog::Form(f)) => ew_viewmodel::strings::text(f.title_key).to_owned(),
        other => panic!("no form: {other:?}"),
    }
}

/// G8: two wires selected, Cable…, and New cable type… at the end of the list of types: the small form takes the
/// series once (its maker, its type, four numbered cores with green-yellow, 1.5 mm²) and reads it as it will show;
/// Save and add another size keeps everything but the part number and the size; Save comes back to the Cable form with
/// the new type chosen as the type, and Apply makes the cable of that part. Escape in the small form returns to the
/// Cable form.
#[test]
fn a_cable_type_is_made_in_the_cable_form_and_chosen_there() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    select_both(&mut d, &pixel);
    d.input(&click(d.find("Cable").expect("Cable on the Draw tab")));
    shown(&mut d);
    let entry = match d.shell_mut().dialog() {
        Some(Dialog::Form(f)) => f.fields[0].options.last().cloned().expect("a list of types"),
        other => panic!("not the Cable form: {other:?}"),
    };
    assert_eq!(entry.1, "New cable type…", "the list's last entry");
    d.shell_mut().set_form_field("part", &entry.0);
    shown(&mut d);
    assert_eq!(title(&mut d), "New cable type");
    // Escape returns to the Cable form; New cable type… again.
    d.input(&key("escape"));
    shown(&mut d);
    assert!(matches!(d.shell_mut().dialog(), Some(Dialog::Form(f)) if f.action == "cable.from_wires"));
    d.shell_mut().set_form_field("part", &entry.0);
    shown(&mut d);
    fill(&mut d, "Manufacturer", "Generic");
    fill(&mut d, "Type (series)", "CTRL");
    fill(&mut d, "Part number", "CTRL-4G1.5");
    fill(&mut d, "Number of cores", "4");
    assert_eq!(
        d.shell_mut().dispatch(
            "part.field",
            &serde_json::json!({"field": "cable.marking", "value": "numbers"})
        ),
        Outcome::Done
    );
    shown(&mut d);
    let texts = d.texts();
    assert!(
        texts.iter().any(|t| t == "4G1.5 · 1 2 3 GNYE"),
        "the cable as it will read: {texts:?}"
    );
    d.input(&click(d.find("Save and add another size").expect("the button")));
    shown(&mut d);
    // The next size of the series: everything kept but the part number and the cross-section.
    assert_eq!(title(&mut d), "New cable type");
    match d.shell_mut().dialog() {
        Some(Dialog::PartEdit(e)) => {
            let t = |f: &str| e.draft.text(f).to_owned();
            assert_eq!(
                [
                    t("manufacturer"),
                    t("type_designation"),
                    t("part_number"),
                    t("cable.cores"),
                    t("cable.csa")
                ],
                ["Generic", "CTRL", "", "4", ""].map(str::to_owned)
            );
        }
        other => panic!("not the editor: {other:?}"),
    }
    fill(&mut d, "Part number", "CTRL-4G2.5");
    fill(&mut d, "Cross-section (mm²)", "2.5");
    // The dialog's Save, the last of that name (the command bar has one too).
    crate::own_parts::press(&mut d, "Save");
    // Back in the Cable form, the new type chosen and written as the type.
    match d.shell_mut().dialog() {
        Some(Dialog::Form(f)) => {
            let value = |n: &str| f.fields.iter().find(|x| x.name == n).map(|x| x.value.clone());
            assert_eq!(value("part").as_deref(), Some("CTRL-4G2.5"));
            assert_eq!(value("type").as_deref(), Some("CTRL-4G2.5"));
            assert!(
                f.fields[0].options.iter().any(|o| o.0 == "CTRL-4G1.5"),
                "the first size fits too"
            );
        }
        other => panic!("not the Cable form: {other:?}"),
    }
    d.input(&click(d.find("Apply").unwrap()));
    shown(&mut d);
    assert!(d.shell_mut().dialog().is_none(), "applied");
    let made = d.shell_mut().recorded_commands().last().cloned().expect("a step");
    assert_eq!(made.cmd, "cable.create");
    assert_eq!(
        made.params["conductors"]
            .as_array()
            .map(|c| c.iter().map(|c| c["csa"].clone()).collect::<Vec<_>>()),
        Some(vec![serde_json::json!("2.5"); 4])
    );
    d.shell_mut().shutdown();
}

/// The part editor shows a cable part's section where a device part has its pins: a new part made a cable offers its
/// cores.
#[test]
fn a_new_part_of_kind_cable_has_the_cable_section() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    assert_eq!(
        d.shell_mut().dispatch("part.new", &serde_json::json!({})),
        Outcome::Done
    );
    shown(&mut d);
    assert!(d.find("Number of cores").is_none(), "a device part has none");
    assert_eq!(
        d.shell_mut().dispatch(
            "part.field",
            &serde_json::json!({"field": "kind", "value": "cable"})
        ),
        Outcome::Done
    );
    shown(&mut d);
    for label in ["Number of cores", "Cross-section (mm²)", "Outer diameter (mm)"] {
        assert!(d.find(label).is_some(), "{label}");
    }
    d.shell_mut().shutdown();
}

/// G8, G6: a cable made with a size typed has no part: its finding offers Choose a cable type…, whose part dialog lists the
/// cable parts that fit first and the others after them, greyed, each saying why (here: no green-yellow core for the PE
/// wire).
#[test]
fn a_cable_without_a_part_offers_its_cable_types_the_misfits_greyed() {
    use ew_viewmodel::navigators::NavKind;
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_named_wires(&mut d, ["L1", "PE"]);
    select_both(&mut d, &pixel);
    d.input(&click(d.find("Cable").expect("Cable on the Draw tab")));
    shown(&mut d);
    d.shell_mut().set_form_field("part", "");
    d.shell_mut().set_form_field("type", "2G1.5");
    shown(&mut d);
    d.input(&click(d.find("Apply").unwrap()));
    shown(&mut d);
    assert!(d.shell_mut().dialog().is_none(), "applied");
    assert_eq!(
        d.shell_mut()
            .dispatch("messages.show", &serde_json::json!({"on": true})),
        Outcome::Done
    );
    shown(&mut d);
    let row = d
        .shell_mut()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.cells.get(1).is_some_and(|c| c == "EW-CORE-041"))
                .cloned()
        })
        .expect("the cable has no part");
    let offer = d.shell_mut().finding_offer(&row.key).expect("an offer");
    assert_eq!(offer.label, "Choose a cable type…");
    assert_eq!(d.shell_mut().dispatch(offer.id, &offer.params), Outcome::Done);
    match d.shell_mut().dialog() {
        Some(Dialog::PartChoose(c)) => {
            assert!(c.offers.iter().all(|o| o.kind == "cable"), "cable parts only");
            let first_misfit = c
                .misfits
                .iter()
                .position(Option::is_some)
                .expect("a cable that does not fit");
            assert!(first_misfit > 0, "those that fit come first");
            assert!(
                c.misfits[first_misfit..].iter().all(Option::is_some),
                "then the others"
            );
            assert_eq!(
                c.misfits[first_misfit].as_deref(),
                Some("no protective conductor for its PE wire")
            );
        }
        other => panic!("no part dialog: {other:?}"),
    }
    // G6: the cable has no length yet, a warning (not an error) whose offer opens its Length row to type it.
    d.input(&key("escape"));
    shown(&mut d);
    let row = d
        .shell_mut()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.cells.get(1).is_some_and(|c| c == "EW-CORE-033"))
                .cloned()
        })
        .expect("the cable has no length");
    assert_eq!(row.cells[0], "warning");
    let offer = d.shell_mut().finding_offer(&row.key).expect("an offer");
    assert_eq!(offer.label, "Type the length…");
    assert_eq!(d.shell_mut().dispatch(offer.id, &offer.params), Outcome::Done);
    assert_eq!(
        d.shell_mut().properties().edit().map(|e| e.row.as_str()),
        Some("device/cable.length")
    );
    d.shell_mut().shutdown();
}
