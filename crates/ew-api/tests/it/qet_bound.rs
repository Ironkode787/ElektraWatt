// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A QElectroTech set that becomes a project while it keeps its look (doc 14 §4.7, §4.11, §4.17): a small hand-written
//! `.qet` with a coil and its contact, a strip drawn over two folios, a T junction, an earth symbol and a cable drawn
//! as marks across its wires, imported through the front door with and without the symbol map beside it.
//!
//! FT-1869: the map binds the coil, the contact and the terminals to starter functions, keeping their drawing, and the
//! terminals' number texts print their designations. FT-1861: the junction element and the earth symbol are no
//! terminals, the strip goes on over the second folio, the marks make a cable with cores, and a stored route with a
//! corner between the grid's lines keeps its way.

use std::path::{Path, PathBuf};

use ew_api::import::{Imported, StarterLibrary, import_qet};
use ew_ids::Id;
use ew_schema::{ConnectionKind, Device, FnDef, FnRole, Function, LibDocument, Record, Representation};
use ew_units::{Nm, Pt2};

/// A definition `name` in category `cat` (`""` for none) of link type `link`, with `kind` (`kindInformation`s) and
/// `terminals` (`x y orientation name`), numbered `n` for its UUIDs, and a horizontal stroke.
fn def(cat: &str, name: &str, link: &str, kind: &str, n: u32, terminals: &[(i32, i32, &str, &str)]) -> String {
    let terminals: String = terminals
        .iter()
        .enumerate()
        .map(|(i, (x, y, o, t))| {
            format!(r#"<terminal x="{x}" y="{y}" orientation="{o}" name="{t}" uuid="{{00000000-0000-4000-9000-0000{n:04}{i:04}}}"/>"#)
        })
        .collect();
    let element = format!(
        r#"<element name="{name}.elmt"><definition type="element" version="0.80" link_type="{link}" width="20" height="20" hotspot_x="10" hotspot_y="10">
  <uuid uuid="{{00000000-0000-4000-8000-00000000{n:04}}}"/>
  <names><name lang="en">{name}</name></names>
  <kindInformations>{kind}</kindInformations>
  <description>
   <line x1="-5" y1="0" x2="5" y2="0" end1="none" end2="none" length1="1.5" length2="1.5" style="line-style:normal;line-weight:normal;filling:none;color:black" antialias="false"/>
   {terminals}
  </description>
 </definition></element>"#
    );
    if cat.is_empty() {
        element
    } else {
        format!(r#"<category name="{cat}">{element}</category>"#)
    }
}

/// The definition terminal UUID `i` of definition `n`.
fn t(n: u32, i: u32) -> String {
    format!("{{00000000-0000-4000-9000-0000{n:04}{i:04}}}")
}

/// The UUID of placed element `id`.
fn e(id: u32) -> String {
    format!("{{00000000-0000-4000-a000-{id:012}}}")
}

/// A placed element of definition `path` at `(x, y)` with `terminals` instance terminals, label `label`, its label text
/// when `label_text`, a user text `user` and links.
#[allow(clippy::too_many_arguments)]
fn placed(path: &str, id: u32, (x, y): (u32, u32), terminals: u32, label: &str, label_text: bool, user: &str, links: &[u32]) -> String {
    let terminals: String = (0..terminals)
        .map(|i| format!(r#"<terminal x="0" y="0" orientation="0" id="{}"/>"#, id * 10 + i))
        .collect();
    let label_text = if label_text {
        r#"<dynamic_elmt_text text_from="ElementInfo" x="-30" y="-5" font="Sans Serif,7,-1,5,50,0,0,0,0,0"><text/><info_name>label</info_name></dynamic_elmt_text>"#
    } else {
        ""
    };
    let user = if user.is_empty() {
        String::new()
    } else {
        format!(r#"<dynamic_elmt_text text_from="UserText" x="4" y="-5" font="Sans Serif,7,-1,5,50,0,0,0,0,0"><text>{user}</text></dynamic_elmt_text>"#)
    };
    let links: String = links.iter().map(|l| format!(r#"<link_uuid uuid="{}"/>"#, e(*l))).collect();
    format!(
        r#"<element type="embed://import/{path}" uuid="{}" x="{x}" y="{y}" orientation="0">
  <terminals>{terminals}</terminals>
  <elementInformations><elementInformation name="label">{label}</elementInformation></elementInformations>
  <dynamic_texts>{label_text}{user}</dynamic_texts>
  <links_uuids>{links}</links_uuids>
 </element>"#,
        e(id)
    )
}

/// A conductor from terminal `ta` of element `a` (definition `da`) to `tb` of `b` (`db`) through `segments`
/// (`orientation length`).
fn wire(a: u32, da: u32, ta: u32, b: u32, db: u32, tb: u32, segments: &[(&str, &str)]) -> String {
    let segments: String = segments
        .iter()
        .map(|(o, l)| format!(r#"<segment orientation="{o}" length="{l}"/>"#))
        .collect();
    format!(
        r#"<conductor element1="{}" terminal1="{}" element2="{}" terminal2="{}" num="" type="multi">{segments}</conductor>"#,
        e(a),
        t(da, ta),
        e(b),
        t(db, tb)
    )
}

fn diagram(order: u32, elements: &[String], wires: &[String]) -> String {
    format!(
        r#"<diagram order="{order}" folio="{order}" title="F{order}" cols="17" colsize="60" rows="8" rowsize="80" height="660" displaycols="true" displayrows="true"><elements>{}</elements><conductors>{}</conductors></diagram>"#,
        elements.concat(),
        wires.concat()
    )
}

// The definitions, by number: 1 coil, 2 contact, 3 terminal, 4 T junction, 5 earth, 6 cable mark.
const COIL: &str = "coil.elmt";
const CONTACT: &str = "contact.elmt";
const KLEM: &str = "klem.elmt";
const THRU: &str = "114_connections/thru.elmt";
const EARTH: &str = "110_network_supplies/terre.elmt";
const MARK: &str = "mark.elmt";

/// Folio 1: the coil -1K1 (5) at (500, 100) over the terminal -X1:1 (7) at (500, 200), its contact (6) at (530, 100)
/// over the terminal 2 (8); the cable -1W1 marked at (500, 150) and (530, 150) across those two wires; the T junction (9)
/// at (700, 300) joining the lower ends of -X1:1 and 2 with the earth symbol (10) below it; a wire from the contact's top round to
/// the coil's top whose corner QET stores at 63.33 px. Folio 2: the coil -2K1 (13) over the terminal 3 (14), without a
/// strip header, and a mark of the cable (15) across their wire.
fn project() -> String {
    let n = r#"<kindInformation name="type">coil</kindInformation>"#;
    let defs = [
        def("", "coil", "master", n, 1, &[(0, -10, "n", "A1"), (0, 10, "s", "A2")]),
        def(
            "",
            "contact",
            "slave",
            r#"<kindInformation name="type">simple</kindInformation><kindInformation name="state">NO</kindInformation>"#,
            2,
            &[(0, -10, "n", ""), (0, 10, "s", "")],
        ),
        def("", "klem", "terminal", "", 3, &[(0, -10, "n", ""), (0, 10, "s", "")]),
        def("114_connections", "thru", "terminal", "", 4, &[(-10, 0, "w", ""), (0, 10, "s", ""), (10, 0, "e", "")]),
        def("110_network_supplies", "terre", "terminal", "", 5, &[(0, -10, "n", "")]),
        def("", "mark", "simple", "", 6, &[]),
    ]
    .concat();
    let f1 = diagram(
        1,
        &[
            placed(COIL, 5, (500, 100), 2, "-1K1", true, "", &[6]),
            placed(CONTACT, 6, (530, 100), 2, "", true, "", &[5]),
            placed(KLEM, 7, (500, 200), 2, "-X1", true, "1", &[]),
            placed(KLEM, 8, (530, 200), 2, "", false, "2", &[]),
            placed(THRU, 9, (700, 300), 3, "", false, "", &[]),
            placed(EARTH, 10, (700, 330), 1, "", false, "", &[]),
            placed(MARK, 11, (500, 150), 0, "-1W1", true, "", &[]),
            placed(MARK, 12, (530, 150), 0, "", false, "", &[]),
        ],
        &[
            wire(5, 1, 1, 7, 3, 0, &[]),
            wire(6, 2, 1, 8, 3, 0, &[]),
            wire(7, 3, 1, 9, 4, 0, &[("vertical", "90"), ("horizontal", "190")]),
            wire(9, 4, 1, 10, 5, 0, &[]),
            wire(8, 3, 1, 9, 4, 2, &[("vertical", "140"), ("horizontal", "180"), ("vertical", "-50")]),
            wire(6, 2, 0, 5, 1, 0, &[("vertical", "-26.6667"), ("horizontal", "-30"), ("vertical", "26.6667")]),
        ],
    );
    let f2 = diagram(
        2,
        &[
            placed(COIL, 13, (300, 100), 2, "-2K1", true, "", &[]),
            placed(KLEM, 14, (300, 200), 2, "", false, "3", &[]),
            placed(MARK, 15, (300, 150), 0, "", false, "", &[]),
        ],
        &[wire(13, 1, 1, 14, 3, 0, &[])],
    );
    format!(r#"<project version="0.80" title="Bound">{f1}{f2}<collection><category name="import">{defs}</category></collection></project>"#)
}

/// The QET file and the symbol map `map` (none when empty) in a folder of their own, named for `name`.
fn folder(name: &str, map: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ew-qet-bound-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the folder");
    let file = dir.join("bound.qet");
    std::fs::write(&file, project()).expect("the QET file");
    if !map.is_empty() {
        std::fs::write(dir.join("symbol-map.csv"), map).expect("the map");
    }
    file
}

fn library() -> StarterLibrary {
    StarterLibrary::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")).expect("the starter content")
}

/// The import of the QET file `file` with the map beside it.
fn import(file: &Path) -> Imported {
    let lib = library().with_symbol_map_beside(file).expect("the map reads");
    let imported = import_qet("bound.qet", std::fs::read(file).unwrap(), &lib).expect("an importable project");
    std::fs::remove_dir_all(file.parent().unwrap()).expect("the folder goes");
    assert!(imported.session.is_ok(), "{:?}", imported.session.as_ref().err());
    imported
}

const MAP: &str = "element,function,cps,tag,number,xref\ncoil.elmt,ew.fn.coil,A1 A2,,,\ncontact.elmt,ew.fn.contact-no,13 14,,,\nklem.elmt,ew.fn.terminal,,,,\n";

fn records<T>(i: &Imported, f: impl Fn(&Record) -> Option<&T>) -> Vec<&T> {
    i.batch.records.iter().filter_map(f).collect()
}

/// The starter FnDef copy `key` of the batch's library.
fn starter(i: &Imported, key: &str) -> Id<FnDef> {
    i.batch
        .lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Fndef(c) if c.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) => Some(c.id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{key} travels with the batch"))
}

/// The function and the representation of the placed element `id`, by their source keys.
fn of(i: &Imported, id: u32) -> (&Function, &Representation) {
    let key = |suffix: &str| format!("qet:el:00000000-0000-4000-a000-{id:012}{suffix}");
    let f = records(i, |r| match r {
        Record::Function(f) if f.source_ref.as_ref().is_some_and(|s| s.key == key("#fn")) => Some(f),
        _ => None,
    });
    let r = records(i, |r| match r {
        Record::Representation(x) if x.source_ref.as_ref().is_some_and(|s| s.key == key("#rep")) => Some(x),
        _ => None,
    });
    (f[0], r[0])
}

fn device(i: &Imported, id: Id<Device>) -> &Device {
    records(i, |r| match r {
        Record::Device(d) if d.id == id => Some(d),
        _ => None,
    })[0]
}

#[test]
fn the_symbol_map_binds_coils_contacts_and_terminals_and_keeps_their_drawing() {
    let plain = import(&folder("plain", ""));
    let bound = import(&folder("bound", MAP));
    let (coil, coil_rep) = of(&bound, 5);
    assert_eq!(coil.def, starter(&bound, "ew.fn.coil"), "the coil is the starter coil");
    let (contact, _) = of(&bound, 6);
    assert_eq!(contact.def, starter(&bound, "ew.fn.contact-no"));
    assert_eq!((contact.device, contact.role.clone()), (coil.device, FnRole::Aux));
    let (terminal, terminal_rep) = of(&bound, 8);
    assert_eq!(terminal.def, starter(&bound, "ew.fn.terminal"));
    let designation = |f: &Function| {
        device(&bound, f.device.unwrap())
            .aspects
            .terminal
            .as_ref()
            .map(|t| t.designation.as_str().to_owned())
    };
    assert_eq!(designation(terminal).as_deref(), Some("2"));
    // The terminal's number text prints its designation: a slot shown where the text stands.
    assert!(terminal_rep.text_slots.iter().any(|o| o.visible == Some(true)), "{terminal_rep:?}");
    // The element keeps its drawing and its place.
    let symbol = |i: &Imported, r: &Representation| {
        i.batch
            .lib
            .iter()
            .find_map(|d| match d {
                LibDocument::Symbol(c) if c.id == r.symbol => Some(c.item.clone()),
                _ => None,
            })
            .expect("the symbol")
    };
    let (_, plain_rep) = of(&plain, 5);
    assert_eq!(symbol(&bound, coil_rep).graphics, symbol(&plain, plain_rep).graphics);
    assert_eq!((coil_rep.pos, coil_rep.orient), (plain_rep.pos, plain_rep.orient));
    assert_ne!(coil_rep.symbol, plain_rep.symbol, "a copy of its own, which a re-import adds");
    // The report says what each row bound.
    let report = bound.migration_report().to_csv();
    assert!(report.contains("symbol-map.csv, line 2: 2 elements bound to `ew.fn.coil`"), "{report}");
    // Without the map the coil draws a function definition of its own.
    let (plain_coil, _) = of(&plain, 5);
    assert_ne!(plain_coil.def, coil.def);
}

#[test]
fn a_wrong_row_refuses_the_map_in_words() {
    let refused = |map: &str| {
        let file = folder("refused", map);
        let r = library().with_symbol_map_beside(&file).map(|_| ()).err();
        std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
        r.expect("refused")
    };
    let e = refused("element,function\ncoil.elmt,ew.fn.motor-3ph\n");
    assert!(e.starts_with("symbol-map.csv, line 2, column `function`:"), "{e}");
    assert!(e.contains("the map binds coils, contacts, terminals and interruption points"), "{e}");
    assert!(refused("element,kind\n").contains("unknown column `kind`"));
    assert!(refused("function\new.fn.coil\n").contains("the column `element` is missing"));
    // A row whose connection points do not fit the element refuses the import.
    let file = folder("count", "element,function,cps\ncoil.elmt,ew.fn.coil,A1\n");
    let lib = library().with_symbol_map_beside(&file).expect("the map reads");
    let r = import_qet("bound.qet", std::fs::read(&file).unwrap(), &lib);
    std::fs::remove_dir_all(file.parent().unwrap()).unwrap();
    let e = r.err().expect("refused").to_string();
    assert!(e.contains("symbol-map.csv, line 2: coil.elmt has 2 terminals; the row names 1 connection points"), "{e}");
}

/// The px `v` in nm, as the importer places it.
fn px(v: i64) -> Nm {
    ew_units::Grid::QET_1PX.at(v).unwrap()
}

#[test]
fn junctions_earths_strips_and_cable_marks_make_the_wired_cabinet() {
    let i = import(&folder("cabinet", ""));
    // The T junction is a junction, the earth symbol an earth point: neither is a terminal.
    let junctions = records(&i, |r| match r {
        Record::Junction(j) => Some(j),
        _ => None,
    });
    assert_eq!(junctions.len(), 1);
    assert_eq!(junctions[0].at, Pt2::new(px(700), px(300)), "where its strokes meet");
    assert_eq!(junctions[0].sequence.len(), 3, "its three wires end at it");
    let (earth, _) = of(&i, 10);
    assert!(device(&i, earth.device.unwrap()).aspects.terminal.is_none(), "no terminal");
    let earth_def = i.batch.lib.iter().find_map(|d| match d {
        LibDocument::Fndef(c) if c.id == earth.def => Some(c.item.category.0.as_str().to_owned()),
        _ => None,
    });
    assert_eq!(earth_def.as_deref(), Some("EarthingBar"));
    // One strip, on over the second folio: the terminal there without a header is its third.
    let devices = records(&i, |r| match r {
        Record::Device(d) => Some(d),
        _ => None,
    });
    let strips: Vec<&&Device> = devices.iter().filter(|d| d.aspects.strip.is_some()).collect();
    assert_eq!(strips.len(), 1, "one strip");
    let on = |id: u32| device(&i, of(&i, id).0.device.unwrap()).aspects.terminal.clone().unwrap();
    assert_eq!(on(14).strip, strips[0].id);
    assert_eq!(on(14).designation.as_str(), "3");
    // The marks make one cable whose cores are the three wires they cross, in their order.
    let cables: Vec<&&Device> = devices.iter().filter(|d| d.aspects.cable.is_some()).collect();
    assert_eq!(cables.len(), 1, "one cable");
    let cores = records(&i, |r| match r {
        Record::Conductor(c) => Some(c),
        _ => None,
    });
    assert!(cores.iter().all(|c| c.cable == cables[0].id));
    // Each core and the x of the wire it is: the two of folio 1 left to right, then the one of folio 2.
    let mut wires: Vec<(String, Nm)> = i
        .batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Connection(c) => match c.kind {
                ConnectionKind::Core(k) => Some((k, c.route.first().map(|p| p.x))),
                _ => None,
            },
            _ => None,
        })
        .map(|(k, x)| {
            let core = cores.iter().find(|c| c.id == k).expect("its conductor");
            (core.core.as_str().to_owned(), x.expect("a stored route"))
        })
        .collect();
    wires.sort();
    assert_eq!(
        wires,
        [("1".to_owned(), px(500)), ("2".to_owned(), px(530)), ("3".to_owned(), px(300))]
    );
    // The route stored with a corner at 63.33 px keeps its way on the half grid: up to 65 px, over, down.
    let kept = records(&i, |r| match r {
        Record::Connection(c) if c.route.iter().any(|p| p.y == px(65)) => Some(c),
        _ => None,
    });
    assert_eq!(kept.len(), 1, "the stored route is kept");
    assert_eq!(kept[0].route.len(), 4);
}
