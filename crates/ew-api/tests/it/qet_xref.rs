// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cross-references an imported QET project prints (doc 14 §4.5, §4.6, §4.8): a small hand-written
//! `.qet` with a folio-report pair and a coil with two contacts, imported through the front door, and the
//! texts its pages print.

use std::path::Path;

use ew_api::Session;
use ew_api::import::{Imported, StarterLibrary, import_qet};
use ew_api::output::page_view;
use ew_derive::{PageOrder, PageRef};
use ew_schema::{FunctionKind, Record, SlotKind};

/// A definition `name` of link type `link` with kind `kind`: one terminal and a label text.
fn def(name: &str, link: &str, kind: &str) -> String {
    format!(
        r#"<element name="{name}.elmt"><definition type="element" version="0.80" link_type="{link}" width="20" height="20" hotspot_x="10" hotspot_y="10">
  <uuid uuid="{{00000000-0000-4000-8000-0000000e{n:04x}}}"/>
  <names><name lang="en">{name}</name></names>
  <kindInformations><kindInformation name="type">{kind}</kindInformation></kindInformations>
  <description>
   <dynamic_text text_from="ElementInfo" x="6" y="-12" font="Sans Serif,9,-1,5,50,0,0,0,0,0"><text/><info_name>label</info_name></dynamic_text>
   <terminal x="-10" y="0" orientation="w" uuid="{{00000000-0000-4000-9000-0000000e{n:04x}}}"/>
  </description>
 </definition></element>"#,
        n = name.len()
    )
}

/// A placed element `def` at `(x, y)` with UUID number `id`, its label information, its own texts (the label
/// text in 7 pt and a user text) and links.
fn placed(def: &str, id: u32, (x, y): (u32, u32), label: &str, user: &str, links: &[u32]) -> String {
    let links: String = links
        .iter()
        .map(|l| format!(r#"<link_uuid uuid="{{00000000-0000-4000-8000-{l:012x}}}"/>"#))
        .collect();
    format!(
        r#"<element type="embed://import/{def}.elmt" uuid="{{00000000-0000-4000-8000-{id:012x}}}" x="{x}" y="{y}" orientation="0">
  <terminals><terminal x="-10" y="0" orientation="3" id="{id}"/></terminals>
  <elementInformations><elementInformation name="label">{label}</elementInformation></elementInformations>
  <dynamic_texts>
   <dynamic_elmt_text text_from="ElementInfo" x="-30" y="0" font="Sans Serif,7,-1,5,50,0,0,0,0,0"><text>stale</text><info_name>label</info_name></dynamic_elmt_text>
   <dynamic_elmt_text text_from="UserText" x="0" y="-10" font="Sans Serif,9,-1,5,50,0,0,0,0,0"><text>{user}</text></dynamic_elmt_text>
  </dynamic_texts>
  <links_uuids>{links}</links_uuids>
 </element>"#
    )
}

fn diagram(order: u32, elements: &[String]) -> String {
    format!(
        r#"<diagram order="{order}" folio="{order}" title="F{order}" cols="17" colsize="60" rows="8" rowsize="80" height="660" displaycols="true" displayrows="true"><elements>{}</elements><conductors/></diagram>"#,
        elements.concat()
    )
}

/// Folio 1: a going arrow `L-7` at 1-E8 and a coil `-K1` at 1-C3. Folio 2: the coming arrow `L-7` at 2-B2
/// and the coil's two contacts at 2-D5 and 2-F7.
fn project() -> String {
    let f1 = diagram(
        1,
        &[
            placed("going", 1, (500, 350), "", "L-7", &[2]),
            placed("coil", 3, (200, 190), "-K1", "", &[4, 5]),
        ],
    );
    let f2 = diagram(
        2,
        &[
            placed("coming", 2, (100, 110), "", "L-7", &[1]),
            placed("contact", 4, (300, 270), "", "", &[3]),
            placed("contact", 5, (400, 430), "", "", &[3]),
        ],
    );
    let defs = [
        def("going", "next_report", ""),
        def("coming", "previous_report", ""),
        def("coil", "master", "coil"),
        def("contact", "slave", "delayOn"),
    ]
    .concat();
    format!(
        r#"<project version="0.80" title="Xref">
 <newdiagrams>
  <report label="%F-%l%c"/>
  <xrefs><xref type="coil" master_label="%F-%l%c" slave_label="(%F-%l%c)" displayhas="contacts" snapto="label" xrefpos="AlignBottom"/></xrefs>
 </newdiagrams>
 {f1}{f2}
 <collection><category name="import">{defs}</category></collection>
</project>"#
    )
}

fn import() -> Imported {
    let lib = StarterLibrary::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content"))
        .expect("the starter content");
    let imported = import_qet("xref.qet", project().into_bytes(), &lib).expect("an importable project");
    assert!(imported.session.is_ok(), "{:?}", imported.session.as_ref().err());
    imported
}

/// The texts shown by the slots of `kind` on folio `folio`: the slot shown by its representation's override.
fn shown(s: &Session, folio: &str, kind: &SlotKind) -> Vec<String> {
    let q = s.query();
    let mut out = Vec::new();
    for entry in &q.get::<PageOrder>(()).entries {
        let PageRef::Drawn(page) = entry.page else {
            continue;
        };
        if q.pages().get(page).map(|p| ew_derive::folio_label(p)).as_deref() != Some(folio) {
            continue;
        }
        for rep in page_view(&q, entry).reps {
            let Some(symbol) = &rep.symbol else { continue };
            for v in &rep.slots {
                let of_kind = symbol
                    .text_slots
                    .get(usize::from(v.slot))
                    .is_some_and(|t| t.kind == *kind);
                let on = rep
                    .rep
                    .text_slots
                    .iter()
                    .any(|o| o.slot == v.slot && o.visible == Some(true));
                if of_kind && on {
                    out.push(v.text.clone());
                }
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_folio_report_pair_prints_its_name_and_its_partners_position_on_both_folios() {
    let imported = import();
    let s = imported.session.as_ref().unwrap();
    // QET's report formula `%F-%l%c` on QET's cells; the stale text stored in the file is not printed.
    for (folio, partner) in [("1", "2-B2"), ("2", "1-E8")] {
        let texts = shown(s, folio, &SlotKind::Xref);
        assert!(texts.contains(&partner.to_owned()), "folio {folio}: {texts:?}");
        assert!(!texts.contains(&"stale".to_owned()), "folio {folio}: {texts:?}");
    }
    for folio in ["1", "2"] {
        assert_eq!(
            shown(s, folio, &SlotKind::FunctionText),
            ["L-7".to_owned()],
            "folio {folio}"
        );
    }
}

#[test]
fn a_coil_and_its_contacts_print_each_others_positions() {
    let imported = import();
    let s = imported.session.as_ref().unwrap();
    // The coil lists its contacts one per line, as QET draws them; each contact prints the coil's position.
    let folio1 = shown(s, "1", &SlotKind::Xref);
    assert!(folio1.contains(&"2-D5\n2-F7".to_owned()), "{folio1:?}");
    let folio2 = shown(s, "2", &SlotKind::Xref);
    assert_eq!(folio2.iter().filter(|t| *t == "(1-C3)").count(), 2, "{folio2:?}");
}

#[test]
fn the_report_label_and_the_xref_formulas_become_the_projects_format() {
    let imported = import();
    let project = imported
        .batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::Project(p) => Some(p),
            _ => None,
        })
        .unwrap();
    let format = &project.settings.extra["xref_format"];
    assert_eq!(format["ip"], "%F-%l%c");
    assert_eq!(format["aux_to_main"], "(%F-%l%c)");
    assert_eq!(format["main_to_aux"], "{list}");
    // The pair keeps one name on both ends.
    let names: Vec<&str> = imported
        .batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Function(f) => match &f.kind {
                FunctionKind::Ip(ip) => Some(ip.name.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(names, ["L-7", "L-7"]);
}
