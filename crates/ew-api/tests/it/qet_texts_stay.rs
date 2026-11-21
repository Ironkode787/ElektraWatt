// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An imported page's texts stay where the drawing has them (FT-1875; doc 06a §5.4, doc 14 §4.4.2): a small
//! QElectroTech file made for this test, a row of three coils with their labels and a contact that prints its coil's
//! place, imported with and without the symbol map beside it. Every text of an element is a text with a stored place
//! (a slot shown where the drawing has it, or a text of the element's symbol), so the text placer prints it there; a
//! move of an element and Arrange and route keep each text where it stands on its element.

use std::path::{Path, PathBuf};

use ew_api::Session;
use ew_api::import::{StarterLibrary, import_qet};
use ew_api::routing::RouteScope;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::Pt2;
use ew_derive::{PageRef, Q};
use ew_dl::Layer;
use ew_schema::{Page, PageItemKind, Representation};
use serde_json::json;

/// A definition `name` of link type `link` with `kind`, numbered `n`, its terminals up and down.
fn def(name: &str, link: &str, kind: &str, n: u32) -> String {
    let t = |i: u32, y: i32, o: &str| {
        format!(
            r#"<terminal x="0" y="{y}" orientation="{o}" name="" uuid="{{00000000-0000-4000-9000-0000{n:04}{i:04}}}"/>"#
        )
    };
    format!(
        r#"<element name="{name}.elmt"><definition type="element" version="0.80" link_type="{link}" width="20" height="20" hotspot_x="10" hotspot_y="10">
  <uuid uuid="{{00000000-0000-4000-8000-00000000{n:04}}}"/>
  <names><name lang="en">{name}</name></names>
  <kindInformations>{kind}</kindInformations>
  <description>
   <rect x="-8" y="-5" width="16" height="10" style="line-style:normal;line-weight:normal;filling:none;color:black" antialias="false"/>
   {}{}
  </description>
 </definition></element>"#,
        t(0, -10, "n"),
        t(1, 10, "s")
    )
}

/// The UUID of placed element `id`.
fn e(id: u32) -> String {
    format!("{{00000000-0000-4000-a000-{id:012}}}")
}

/// A placed element of `path` at `(x, y)` labelled `label`, its label text left of it, linked to `links`.
fn placed(path: &str, id: u32, (x, y): (u32, u32), label: &str, links: &[u32]) -> String {
    let links: String = links
        .iter()
        .map(|l| format!(r#"<link_uuid uuid="{}"/>"#, e(*l)))
        .collect();
    format!(
        r#"<element type="embed://import/{path}" uuid="{}" x="{x}" y="{y}" orientation="0">
  <terminals><terminal x="0" y="0" orientation="0" id="{}"/><terminal x="0" y="0" orientation="0" id="{}"/></terminals>
  <elementInformations><elementInformation name="label">{label}</elementInformation></elementInformations>
  <dynamic_texts><dynamic_elmt_text text_from="ElementInfo" x="-34" y="-8" font="Sans Serif,7,-1,5,50,0,0,0,0,0"><text/><info_name>label</info_name></dynamic_elmt_text></dynamic_texts>
  <links_uuids>{links}</links_uuids>
 </element>"#,
        e(id),
        id * 10,
        id * 10 + 1
    )
}

/// A conductor from terminal `ta` of element `a` (definition `da`) to `tb` of `b` (`db`), straight.
fn wire(a: u32, da: u32, ta: u32, b: u32, db: u32, tb: u32) -> String {
    let t = |n: u32, i: u32| format!("{{00000000-0000-4000-9000-0000{n:04}{i:04}}}");
    format!(
        r#"<conductor element1="{}" terminal1="{}" element2="{}" terminal2="{}" num="" type="multi"/>"#,
        e(a),
        t(da, ta),
        e(b),
        t(db, tb)
    )
}

/// The project: a row of coils -1K1, -1K2, -1K3 (1, 2, 3) one column apart, each wired up to a contact of its own
/// over it (4, 5, 6; the first linked to -1K1, so it prints its coil's place, and -1K1 its contact list).
fn project() -> String {
    let coil = r#"<kindInformation name="type">coil</kindInformation>"#;
    let contact = r#"<kindInformation name="type">simple</kindInformation><kindInformation name="state">NO</kindInformation>"#;
    let defs = [
        def("coil", "master", coil, 1),
        def("contact", "slave", contact, 2),
    ]
    .concat();
    let elements = [
        placed("coil.elmt", 1, (100, 300), "-1K1", &[4]),
        placed("coil.elmt", 2, (160, 300), "-1K2", &[]),
        placed("coil.elmt", 3, (220, 300), "-1K3", &[]),
        placed("contact.elmt", 4, (100, 160), "", &[1]),
        placed("contact.elmt", 5, (160, 160), "-1S2", &[]),
        placed("contact.elmt", 6, (220, 160), "-1S3", &[]),
    ]
    .concat();
    let wires = [
        wire(4, 2, 1, 1, 1, 0),
        wire(5, 2, 1, 2, 1, 0),
        wire(6, 2, 1, 3, 1, 0),
    ]
    .concat();
    let xrefs = r#"<newdiagrams><xrefs><xref type="coil" master_label="%f-%l%c" slave_label="(%f-%l%c)" displayhas="contacts" snapto="label" xrefpos="AlignTop" slave_offset="7"/></xrefs></newdiagrams>"#;
    format!(
        r#"<project version="0.80" title="Stay">{xrefs}<diagram order="1" folio="1" title="F1" cols="17" colsize="60" rows="8" rowsize="80" height="660" displaycols="true" displayrows="true"><elements>{elements}</elements><conductors>{wires}</conductors></diagram><collection><category name="import">{defs}</category></collection></project>"#
    )
}

/// The QET file and, when `map` is not empty, the symbol map beside it, in a folder of their own.
fn folder(name: &str, map: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ew-qet-stay-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the folder");
    let file = dir.join("stay.qet");
    std::fs::write(&file, project()).expect("the QET file");
    if !map.is_empty() {
        std::fs::write(dir.join("symbol-map.csv"), map).expect("the map");
    }
    file
}

/// The session of the import of `file`, the folder removed.
fn import(file: &Path) -> Session {
    let lib = StarterLibrary::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content"))
        .expect("the starter content")
        .with_symbol_map_beside(file)
        .expect("the map reads");
    let imported = import_qet("stay.qet", std::fs::read(file).unwrap(), &lib).expect("an importable project");
    std::fs::remove_dir_all(file.parent().unwrap()).expect("the folder goes");
    imported.session.expect("the import opens")
}

const MAP: &str = "element,function,cps,tag,number,xref\ncoil.elmt,ew.fn.coil,A1 A2,,,\ncontact.elmt,ew.fn.contact-no,13 14,,,\n";

/// The one drawn page.
fn page(s: &Session) -> Id<Page> {
    s.query().pages().iter().next().map(|(id, _)| id).expect("a page")
}

/// Each text of an element on the page, by the record and slot that print it, as an offset from its element: a slot
/// text where it prints, a text of a symbol where it stands.
fn texts(q: &Q, page: Id<Page>) -> Vec<(Uuid, u16, (i64, i64))> {
    let layer = ew_api::output::page_layer(q, PageRef::Drawn(page), Layer::Labels);
    let reps = q.reps();
    let off = |rep: Id<Representation>, at: Pt2| {
        let p = reps.get(rep).expect("its symbol").pos;
        (at.x.0 - p.x.0, at.y.0 - p.y.0)
    };
    let mut out: Vec<(Uuid, u16, (i64, i64))> = layer
        .slot_texts
        .iter()
        .map(|t| (t.rep, t.slot, off(Id::from_uuid(t.rep), t.at)))
        .collect();
    for (id, item) in q.page_items().iter() {
        if let PageItemKind::FreeText(t) = &item.kind
            && let Some(rep) = t.of
        {
            out.push((id.as_uuid(), u16::MAX, off(rep, t.at)));
        }
    }
    out.sort();
    out
}

/// What the import stored for every text: each shown slot's override position, as `texts` gives it.
fn stored(q: &Q, page: Id<Page>) -> Vec<(Uuid, u16)> {
    let mut out: Vec<(Uuid, u16)> = Vec::new();
    for rid in q.idx().reps_by_page(page) {
        let rep = q.reps().get(rid).expect("a symbol");
        for o in rep.text_slots.iter().filter(|o| o.visible == Some(true)) {
            assert!(o.pos.is_some(), "a shown slot has its place from the drawing");
            out.push((rid.as_uuid(), o.slot));
        }
    }
    out.sort();
    out
}

fn stays(map: &str) {
    let mut s = import(&folder(if map.is_empty() { "plain" } else { "bound" }, map));
    let p = page(&s);
    let q = s.query();
    // Every slot the page prints is one the import gave a place (a contact without a partner prints its empty
    // cross-reference nowhere), and every element's other texts are its symbol's.
    let placed = stored(&q, p);
    let printed: Vec<(Uuid, u16)> = texts(&q, p)
        .into_iter()
        .filter(|t| t.1 != u16::MAX)
        .map(|t| (t.0, t.1))
        .collect();
    assert!(!printed.is_empty());
    assert!(
        printed.iter().all(|t| placed.contains(t)),
        "the page prints only the slots the drawing places: {printed:?} of {placed:?}"
    );
    let layer = ew_api::output::page_layer(&q, PageRef::Drawn(p), Layer::Labels);
    assert!(
        layer.slot_texts.iter().all(|t| t.moved),
        "no slot text is the placer's"
    );
    for t in &layer.slot_texts {
        let o = q
            .reps()
            .get(Id::from_uuid(t.rep))
            .and_then(|r| r.text_slots.iter().find(|o| o.slot == t.slot).and_then(|o| o.pos))
            .expect("its place");
        assert_eq!(
            t.at,
            t.frame.origin + t.frame.orient.apply(o),
            "printed where the drawing has it"
        );
    }
    let before = texts(&q, p);
    assert!(!before.is_empty());
    drop(q);
    // A move of one element: its texts go along.
    let coil = s
        .query()
        .reps()
        .iter()
        .find(|(_, r)| {
            r.page == p
                && r.source_ref
                    .as_ref()
                    .is_some_and(|x| x.key.contains("000000000002#rep"))
        })
        .map(|(id, _)| id)
        .expect("-1K2's symbol");
    s.execute(&ew_api::AnyCommand {
        name: "rep.move".into(),
        params: json!({"reps": [coil.to_string()], "steps": [2, 0]}),
    })
    .expect("moved");
    assert_eq!(
        texts(&s.query(), p),
        before,
        "each text keeps its place on its element"
    );
    // Arrange and route: whatever it moves, each text keeps its place on its element.
    let folio = s.page_rows()[0].folio.clone();
    if let Some(params) = s
        .arrange_plan(&folio, &RouteScope::Page, &[])
        .and_then(|plan| plan.params)
    {
        s.execute(&ew_api::AnyCommand {
            name: "layout.apply".into(),
            params: params.clone(),
        })
        .unwrap_or_else(|e| panic!("{e}: {params}"));
    }
    assert_eq!(texts(&s.query(), p), before, "Arrange and route keep them");
}

#[test]
fn an_imported_pages_texts_stay_where_the_drawing_has_them() {
    stays("");
}

#[test]
fn a_bound_imported_pages_texts_stay_where_the_drawing_has_them() {
    stays(MAP);
}
