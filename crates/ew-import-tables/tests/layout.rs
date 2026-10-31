// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout tables of doc 20 §13's test geometry (U-50, WP-S4.01 D13): the records they give, the mechanical
//! columns of `parts.csv`, and the defective rows the emitter refuses.

mod common;

use ew_import_tables::ids::record_id;
use ew_interop_api::{Code, ImportBatch, ImportInput};
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, Enclosure, LayoutSpace, LibDocument, LibItem, MountKind,
    MountingSurface, Part, Placement, Record, SurfaceKind,
};
use ew_units::{Nm, Pt2};

const MM: i64 = 1_000_000;

fn mm(v: i64) -> Nm {
    Nm(v * MM)
}

fn pt(u: i64, v: i64) -> Pt2 {
    Pt2::new(mm(u), mm(v))
}

/// The golden inputs with the file `name` replaced by `text`.
fn golden_with(name: &str, text: &str) -> ImportBatch {
    let inputs: Vec<ImportInput> = common::golden_inputs()
        .into_iter()
        .map(|i| {
            if i.name() == name {
                ImportInput::new(name, text.as_bytes().to_vec()).unwrap()
            } else {
                i
            }
        })
        .collect();
    common::import(&inputs).expect("the import succeeds")
}

fn golden() -> ImportBatch {
    common::import(&common::golden_inputs()).expect("the golden tables import")
}

/// The messages of the findings with `code`.
fn messages(b: &ImportBatch, code: Code) -> Vec<String> {
    b.report.with_code(code).map(|e| e.message.clone()).collect()
}

fn part<'b>(b: &'b ImportBatch, key: &str) -> &'b Part {
    b.lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(p) if p.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) => {
                Some(&p.item)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("part {key} is embedded"))
}

fn placements(b: &ImportBatch) -> Vec<&Placement> {
    b.records
        .iter()
        .filter_map(|r| match r {
            Record::Placement(p) => Some(p),
            _ => None,
        })
        .collect()
}

/// The space, the enclosure and its plate surface carry the identities of the rule, the plate frame and outline of
/// §13 (plate 650 × 900 mm at (50, 50) mm in the 750 × 1000 mm enclosure) and `pe_bond`.
#[test]
fn the_space_enclosure_and_plate_are_those_of_doc_20() {
    let b = golden();
    let space = record_id::<LayoutSpace>("layout_spaces", "+J-E-2416");
    let enclosure = record_id::<Enclosure>("enclosures", "-UC1");
    let surface = record_id::<MountingSurface>("mounting_surfaces", "-UC1");
    let s = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::MountingSurface(s) => Some(s),
            _ => None,
        })
        .expect("the plate surface");
    assert_eq!(
        (s.id, s.enclosure, s.kind.clone()),
        (surface, enclosure, SurfaceKind::Plate)
    );
    assert_eq!(s.frame.t, [mm(50), mm(50), Nm::ZERO]);
    assert_eq!(s.usable, vec![pt(0, 0), pt(650, 0), pt(650, 900), pt(0, 900)]);
    assert!(s.pe_bond);
    assert!(s.device.is_some(), "the plate device -UC1-plate");
    assert!(
        b.records
            .iter()
            .any(|r| matches!(r, Record::Enclosure(e) if e.id == enclosure && e.space == space))
    );
    assert!(
        b.records
            .iter()
            .any(|r| matches!(r, Record::LayoutSpace(l) if l.id == space))
    );
}

/// R1 is a TS 35 × 7.5 rail from (40, 740) to (610, 740) mm with its part and `pe_bond`; D1 a 40 × 60 mm duct.
#[test]
fn the_carriers_are_those_of_doc_20() {
    let b = golden();
    let carrier = |key: &str| -> &Carrier {
        let id = record_id::<Carrier>("carriers", key);
        b.records
            .iter()
            .find_map(|r| match r {
                Record::Carrier(c) if c.id == id => Some(c),
                _ => None,
            })
            .unwrap_or_else(|| panic!("carrier {key}"))
    };
    let r1 = carrier("R1");
    assert_eq!(r1.kind, CarrierKind::Rail);
    assert_eq!(r1.path, vec![pt(40, 740), pt(610, 740)]);
    assert!(matches!(&r1.params, CarrierParams::Rail(p) if p.profile == MountKind::Ts35x7_5));
    assert!(r1.pe_bond && r1.part.is_some());
    let d1 = carrier("D1");
    assert_eq!(d1.kind, CarrierKind::Duct);
    assert!(matches!(&d1.params, CarrierParams::Duct(p) if p.width == mm(40) && p.height == mm(60)));
    assert!(!d1.pe_bond);
}

/// A placement sits at `t = (s, 0, 0)` on its rail; a strip has no part, a device with a part names its copy.
#[test]
fn the_placements_are_rail_poses() {
    let b = golden();
    let all = placements(&b);
    let by_key = |key: &str| {
        let id = record_id::<Placement>("placements", key);
        *all.iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("placement {key}"))
    };
    let x1031 = by_key("-X1031");
    assert_eq!(x1031.pose.t, [Nm(58_600_000), Nm::ZERO, Nm::ZERO]);
    assert_eq!(x1031.part, None, "a strip is placed whole");
    assert_eq!(x1031.carrier, CarrierRef::Carrier(record_id("carriers", "R2")));
    let g1 = by_key("-102G1");
    assert!(g1.part.is_some());
    assert!(all.iter().all(|p| !p.locked && p.function.is_none()));
}

/// The mechanical columns give `mech` and the carrier sections; the enclosure section names its plate part.
#[test]
fn the_parts_carry_their_mechanical_data() {
    let b = golden();
    let psu = part(&b, "2904602");
    let mech = psu.mech.as_ref().expect("2904602 has mech");
    assert_eq!(
        (mech.width, mech.height, mech.depth),
        (Some(mm(70)), Some(mm(130)), Some(mm(125)))
    );
    assert_eq!(mech.mount_kind, Some(MountKind::Ts35x7_5));
    let enc = part(&b, "NSYPLA1074G")
        .sections
        .enclosure
        .as_ref()
        .expect("an enclosure section");
    assert_eq!(
        (enc.width, enc.height, enc.depth),
        (Some(mm(750)), Some(mm(1000)), Some(mm(420)))
    );
    let plate = enc.plate.as_ref().expect("the plate part");
    assert_eq!(plate.item, record_id::<LibItem>("parts", "NSYPMM107"));
    let mp = part(&b, "NSYPMM107")
        .sections
        .mounting_plate
        .as_ref()
        .expect("a plate section");
    assert_eq!((mp.width, mp.height), (Some(mm(650)), Some(mm(900))));
    let rail = part(&b, "TBD DIN rail")
        .sections
        .rail
        .as_ref()
        .expect("a rail section");
    assert_eq!(rail.profile, MountKind::Ts35x7_5);
}

/// A placement on a duct, of a part the device does not use, or a second one of the same device and part is dropped
/// with a finding; an unknown carrier is unresolved.
#[test]
fn defective_placements_are_dropped_with_a_finding() {
    let text = "key,device,part,carrier,s\n\
                a,-102G1,2904602,D1,10\n\
                b,-102G1,195740,R1,10\n\
                c,-102G1,2904602,R1,10\n\
                d,-102G1,2904602,R1,90\n\
                e,-102F1,,R9,10\n\
                f,-102F2,,R1,-1\n";
    let b = golden_with("placements.csv", text);
    let malformed = messages(&b, Code::MALFORMED_ITEM);
    assert!(malformed.iter().any(|m| m.contains("is a duct")), "{malformed:?}");
    assert!(
        malformed.iter().any(|m| m.contains("does not use part `195740`")),
        "{malformed:?}"
    );
    assert!(
        malformed.iter().any(|m| m.contains("already placed")),
        "{malformed:?}"
    );
    assert!(
        malformed.iter().any(|m| m.contains("not negative")),
        "{malformed:?}"
    );
    let unresolved = messages(&b, Code::UNRESOLVED_REFERENCE);
    assert!(
        unresolved.iter().any(|m| m.contains("unknown carrier `R9`")),
        "{unresolved:?}"
    );
    assert_eq!(placements(&b).len(), 1, "only row c is kept");
}

/// A rail of three points, a slanted duct and a rail without a profile are dropped with a finding.
#[test]
fn defective_carriers_are_dropped_with_a_finding() {
    let text = "key,surface,kind,part,path,profile,width,height,pe_bond\n\
                R1,-UC1,rail,,0 0;10 0;20 0,ts35x7_5,,,\n\
                D1,-UC1,duct,,0 0;10 10,,40,60,\n\
                R2,-UC1,rail,,0 0;10 0,,,,\n\
                R3,-UC9,rail,,0 0;10 0,ts32,,,\n";
    let b = golden_with("carriers.csv", text);
    let malformed = messages(&b, Code::MALFORMED_ITEM);
    assert!(
        malformed
            .iter()
            .any(|m| m.contains("a rail is two points, not 3")),
        "{malformed:?}"
    );
    assert!(
        malformed
            .iter()
            .any(|m| m.contains("neither horizontal nor vertical")),
        "{malformed:?}"
    );
    assert!(
        malformed.iter().any(|m| m.contains("a rail's profile")),
        "{malformed:?}"
    );
    let unresolved = messages(&b, Code::UNRESOLVED_REFERENCE);
    assert!(
        unresolved.iter().any(|m| m.contains("unknown enclosure `-UC9`")),
        "{unresolved:?}"
    );
    assert!(!b.records.iter().any(|r| matches!(r, Record::Carrier(_))));
}

/// A second layout space for one location and an unknown location are refused.
#[test]
fn a_location_has_one_layout_space() {
    let b = golden_with(
        "layout_spaces.csv",
        "key,location,name\n+J-E-2416,+J-E-2416,\nsecond,+J-E-2416,\nnowhere,+NOWHERE,\n",
    );
    let malformed = messages(&b, Code::MALFORMED_ITEM);
    assert!(
        malformed
            .iter()
            .any(|m| m.contains("a location has one layout space")),
        "{malformed:?}"
    );
    let unresolved = messages(&b, Code::UNRESOLVED_REFERENCE);
    assert!(
        unresolved
            .iter()
            .any(|m| m.contains("unknown structure node `+NOWHERE`")),
        "{unresolved:?}"
    );
    let spaces = b
        .records
        .iter()
        .filter(|r| matches!(r, Record::LayoutSpace(_)))
        .count();
    assert_eq!(spaces, 1);
}

/// WP-S4.01 part 3: folio 902's view columns give its surface view (the plate of -UC1, its origin at (25, 167) mm,
/// abbreviated labels); a view on a schematic page, an unknown enclosure and a view without its datum are refused.
#[test]
fn the_view_columns_give_902_its_surface_view_and_refuse_the_defective_ones() {
    let views = |b: &ImportBatch| -> Vec<(ew_schema::PageItem, ew_schema::SurfaceView)> {
        b.records
            .iter()
            .filter_map(|r| match r {
                Record::PageItem(i) => match &i.kind {
                    ew_schema::PageItemKind::SurfaceView(v) => Some((i.clone(), v.clone())),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    };
    let b = golden();
    let found = views(&b);
    assert_eq!(found.len(), 1);
    let (item, view) = &found[0];
    assert_eq!(item.page, record_id("pages", "902"));
    assert_eq!(
        (view.surface, view.datum_at, view.label_mode.clone()),
        (
            record_id::<MountingSurface>("mounting_surfaces", "-UC1"),
            pt(25, 167),
            ew_schema::SurfaceLabelMode::Abbreviated
        )
    );

    let golden_pages = std::fs::read_to_string(common::tables_dir().join("pages.csv")).unwrap();
    let text: String = golden_pages
        .lines()
        .map(|l| {
            if l.starts_with("101,") {
                l.replace(",qet10,,,", ",qet10,-UC1,0 0,full")
            } else if l.starts_with("902,") {
                l.replace("-UC1,25 167,", "-UC9,25 167,")
            } else if l.starts_with("102,") {
                l.replace(",qet10,,,", ",qet10,-UC1,,")
            } else {
                l.to_owned()
            }
        })
        .map(|l| l + "\n")
        .collect();
    let b = golden_with("pages.csv", &text);
    let malformed = messages(&b, Code::MALFORMED_ITEM);
    for expected in [
        "a surface view goes on a panel_layout page",
        "view_surface and view_datum go together",
    ] {
        assert!(
            malformed.iter().any(|m| m.contains(expected)),
            "{expected}: {malformed:?}"
        );
    }
    let unresolved = messages(&b, Code::UNRESOLVED_REFERENCE);
    assert!(
        unresolved
            .iter()
            .any(|m| m.contains("unknown enclosure `-UC9` in view_surface")),
        "{unresolved:?}"
    );
    assert!(views(&b).is_empty(), "no view is imported");
}
