// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The set an import brings into Parts (doc 07 §5.9): a maker's table of his own parts mapped to the declared fields,
//! read against My parts (picture v9), then saved.

use crate::common;

use ew_api::catalog::{ColumnMap, ImportInput, RowRefusal};
use ew_api::parts_set::{SetRefusal, SetStatus};
use ew_api::{Session, SessionOptions};

/// A new project whose person has a user folder under `root`.
fn project(root: &std::path::Path) -> Session {
    Session::new_project(
        ew_api::ProjectTemplate::Builtin,
        ew_api::NewProjectKind::Schematic,
        SessionOptions {
            libraries: Some(common::tiers(root)),
            ..SessionOptions::default()
        },
    )
    .expect("a new project")
}

/// A maker's table as picture v9 has it: fourteen rows of his own parts, one of them without a part number and one
/// that My parts holds as it is.
const TABLE: &str = "Maker;Article;Kind;Text EN;Text NL;W;H;D;Mount;In;Cores;Price
Example;ENC-8-10-3;enclosure;Wall enclosure 800 x 1000 x 300 mm;Wandkast 800 x 1000 x 300 mm;800;1000;300;;;;412.00
Example;MP-8-10;mounting plate;Mounting plate 760 x 960 mm;Montageplaat 760 x 960 mm;760;960;;;;;88.00
Example;ENC-12-10-3-2D;enclosure;Wall enclosure 1200 x 1000 x 300 mm, two doors;Wandkast 1200 x 1000 x 300 mm, twee deuren;1200;1000;300;;;;690.00
Example;CON-18-24;device;Contactor 18 A, coil 24 V DC;;;;;;;;31.00
Example;CON-09-24;device;Contactor 9 A, coil 24 V DC;Magneetschakelaar 9 A, spoel 24 V DC;45;85;80;ts35x7_5;9 A;;24.00
Example;FT-4;terminal;Feed-through terminal 4 mm2, screw;Doorgangsklem 4 mm2, schroef;6.2;60;45;ts35x7_5;;;1.10
Example;DL-2.5;terminal;Two-level terminal 2.5 mm2, push-in;Etageklem 2,5 mm2, push-in;5.2;65;50;ts35x7_5;;;2.40
Example;EP-DL;accessory;End plate for DL 2.5;Eindplaat voor DL 2,5;;;;;;;0.30
Example;;rail;Rail 35 x 15 mm, 2 m;Rail 35 x 15 mm, 2 m;;;;;;;9.00
Example;WD-60x80;duct;Wiring duct 60 x 80 mm;Bedradingsgoot 60 x 80 mm;60;80;;;;;7.00
Example;CY-7x1.5;cable;Control cable, 7 cores 1.5 mm2, numbered;Stuurkabel, 7 aders 1,5 mm2, genummerd;;;;;;7;3.20
Example;H07-1.5-BU;wire;Wire 1.5 mm2, blue;Draad 1,5 mm2, blauw;;;;;;;0.40
Example;FER-1.5-BK;ferrule;Ferrule 1.5 mm2, insulated, black;Adereindhuls 1,5 mm2, geisoleerd, zwart;;;;;;;0.02
Example;CG-M25;gland;Cable gland M25;Kabelwartel M25;;;;;;;1.80
";

/// The map of picture v9: every column but the price to its field, the sizes by the word their kinds share.
fn map() -> ColumnMap {
    ColumnMap::new(
        None,
        [
            ("manufacturer", "Maker"),
            ("part_number", "Article"),
            ("kind", "Kind"),
            ("description.en", "Text EN"),
            ("description.nl", "Text NL"),
            ("width", "W"),
            ("height", "H"),
            ("depth", "D"),
            ("mech.mount_kind", "Mount"),
            ("tech_data.rated_current", "In"),
            ("cable.cores", "Cores"),
        ],
    )
    .expect("the map")
}

/// My parts holds the maker's contactor CON-18-24 as he made it on its sheet.
fn contactor_of_my_parts(s: &mut Session) {
    let mut sheet = s.new_part_draft().expect("a new part");
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", "CON-18-24"),
        ("description", "Contactor 18 A, coil 24 V DC"),
    ] {
        s.set_draft_field(&mut sheet, f, v).expect(f);
    }
    s.new_part_in_my_parts(&sheet).expect("saved into My parts");
}

#[test]
fn a_maker_s_table_brings_new_parts_one_the_same_and_one_refused_with_its_reason() {
    let tmp = ew_testdir::Tmp::new("parts-set-v9");
    let mut s = project(&tmp.0);
    contactor_of_my_parts(&mut s);
    let input = ImportInput::new("example-parts.csv", TABLE.as_bytes().to_vec()).expect("an input");
    let set = s.table_set(&input, &map()).expect("the set");
    let (new, changed, same, refused) = set.counts();
    assert_eq!((new, changed, same, refused), (12, 0, 1, 1), "{:#?}", set.rows);
    let row = |number: &str| set.rows.iter().find(|r| r.number == number).expect(number);
    assert_eq!(row("CON-18-24").status, SetStatus::Same);
    let no_number = set
        .rows
        .iter()
        .find(|r| matches!(r.status, SetStatus::Refused(_)))
        .expect("the refused row");
    assert_eq!(no_number.line, 10);
    assert_eq!(
        no_number.status,
        SetStatus::Refused(SetRefusal::Read(RowRefusal::NoPartNumber))
    );
    // The cabinet's row has its outer size; the contactor its size in the cabinet, how it mounts and its current.
    let cabinet = row("ENC-8-10-3").part.clone().expect("a part");
    let enclosure = cabinet.sections.enclosure.expect("the cabinet's section");
    let mm = |n: i64| Some(ew_units::Nm(n * 1_000_000));
    assert_eq!(
        (enclosure.width, enclosure.height, enclosure.depth),
        (mm(800), mm(1000), mm(300))
    );
    assert_eq!(
        row("MP-8-10")
            .part
            .clone()
            .expect("a part")
            .sections
            .mounting_plate
            .expect("a plate")
            .width,
        mm(760)
    );
    let contactor = row("CON-09-24").part.clone().expect("a part");
    let mech = contactor.mech.clone().expect("its size");
    assert_eq!((mech.width, mech.height, mech.depth), (mm(45), mm(85), mm(80)));
    assert!(
        contactor
            .tech_data
            .iter()
            .any(|t| t.key.as_str() == "rated_current")
    );
    assert_eq!(
        serde_json::to_value(&contactor.description).unwrap()["nl"],
        "Magneetschakelaar 9 A, spoel 24 V DC"
    );
    // The cable's row has seven numbered cores, as its Cores form makes them.
    let cable = row("CY-7x1.5").part.clone().expect("a part");
    let cores: Vec<&str> = cable.sections.cable_cores.iter().map(|c| c.core.as_str()).collect();
    assert_eq!(cores, ["1", "2", "3", "4", "5", "6", "7"]);
    // Saved: the twelve new parts are in My parts, the one the same is not written again.
    assert_eq!(s.save_set(&set).expect("saved"), (12, 0));
    let again = s.table_set(&input, &map()).expect("the set read again");
    assert_eq!(again.counts(), (0, 0, 13, 1), "{:#?}", again.rows);
}

/// A value a row cannot give its field is refused at the row in the sheet's own words, its column named.
#[test]
fn a_row_whose_length_does_not_read_is_refused_at_its_column() {
    let tmp = ew_testdir::Tmp::new("parts-set-unread");
    let s = project(&tmp.0);
    let table = "Maker;Article;Kind;W\nExample;FT-6;terminal;six\n";
    let input = ImportInput::new("t.csv", table.as_bytes().to_vec()).expect("an input");
    let map = ColumnMap::new(
        None,
        [
            ("manufacturer", "Maker"),
            ("part_number", "Article"),
            ("kind", "Kind"),
            ("width", "W"),
        ],
    )
    .expect("the map");
    let set = s.table_set(&input, &map).expect("the set");
    match &set.rows[0].status {
        SetStatus::Refused(SetRefusal::Field { column, message }) => {
            assert_eq!(column, "W");
            assert_eq!(message.key, "error-part-length");
        }
        other => panic!("{other:?}"),
    }
}

/// A new row whose Like column names a part of My parts starts as that part: its pins and its size come with it, its
/// own columns over them.
#[test]
fn a_new_row_like_a_part_of_my_parts_takes_what_its_columns_do_not_give() {
    let tmp = ew_testdir::Tmp::new("parts-set-like");
    let mut s = project(&tmp.0);
    let mut sheet = s.new_part_draft().expect("a new part");
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", "FT-2.5"),
        ("kind", "terminal"),
        ("mech.width", "5.2"),
        ("mech.height", "60"),
    ] {
        s.set_draft_field(&mut sheet, f, v).expect(f);
    }
    s.new_part_in_my_parts(&sheet).expect("saved into My parts");
    let table = "Maker;Article;Like;W\nExample;FT-4;FT-2.5;6.2\n";
    let input = ImportInput::new("family.csv", table.as_bytes().to_vec()).expect("an input");
    let map = ColumnMap::new(
        None,
        [
            ("manufacturer", "Maker"),
            ("part_number", "Article"),
            ("like", "Like"),
            ("width", "W"),
        ],
    )
    .expect("the map");
    let set = s.table_set(&input, &map).expect("the set");
    assert_eq!(set.rows[0].status, SetStatus::New, "{:?}", set.rows[0]);
    let part = set.rows[0].part.clone().expect("a part");
    assert_eq!(part.kind, ew_schema::PartKind::Terminal);
    let mech = part.mech.expect("its size");
    let mm = |n: i64| Some(ew_units::Nm(n));
    assert_eq!((mech.width, mech.height), (mm(6_200_000), mm(60_000_000)));
    assert_eq!(part.part_number.as_str(), "FT-4");
}

/// A new cable whose row gives no cores has none; a count with a cross-section gives that many cores of it.
#[test]
fn a_cable_row_without_cores_has_none_and_a_count_gives_its_cores() {
    let tmp = ew_testdir::Tmp::new("parts-set-cores");
    let s = project(&tmp.0);
    let table = "Maker;Article;Kind;Cores;Size
Example;CY-5;cable;;
Example;CY-12;cable;12;0.75
";
    let input = ImportInput::new("cables.csv", table.as_bytes().to_vec()).expect("an input");
    let map = ColumnMap::new(
        None,
        [
            ("manufacturer", "Maker"),
            ("part_number", "Article"),
            ("kind", "Kind"),
            ("cable.cores", "Cores"),
            ("cable.csa", "Size"),
        ],
    )
    .expect("the map");
    let set = s.table_set(&input, &map).expect("the set");
    let part = |i: usize| set.rows[i].part.clone().expect("a part");
    assert!(part(0).sections.cable_cores.is_empty(), "{:?}", part(0).sections.cable_cores);
    let twelve = part(1).sections.cable_cores;
    assert_eq!(twelve.len(), 12);
    assert!(twelve.iter().all(|c| c.csa == Some(ew_schema::Csa::Mm2("0.75".parse().unwrap()))));
}
