// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A declared accessory counts the project's own copy of its item (doc 07 §2.5): the sample's end plate, from a
//! library of one's own, corrected in the project (another number, another width) is what the parts list counts, the assembly list names, the strips on
//! the panel page are as wide by and EW-MFG-020's fix adds; and a terminal of one's own declares an end plate of one's
//! own.

use crate::common;

use ew_api::Session;
use ew_api::output::{RowExport, row_export};
use ew_checks::{CheckMode, FULL};
use ew_derive::layout::PlacementGeometry;
use serde_json::{Value, json};

/// The CSV lines of the export by kind `kind`.
fn lines(s: &Session, kind: &str) -> Vec<String> {
    let out = row_export(&s.query(), &RowExport::Kind(kind.to_owned()), None).expect(kind);
    out.csv.lines().map(str::to_owned).collect()
}

/// The project copy numbered `number`.
fn copy(s: &Session, number: &str) -> String {
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(id, _)| id.to_string())
        .unwrap_or_else(|| panic!("no copy {number}"))
}

/// The rows of the strip `tag` in the assembly lists, its heading first.
fn strip_rows(s: &Session, tag: &str) -> Vec<String> {
    let all = lines(s, "assembly_list");
    let at = all
        .iter()
        .position(|l| l.starts_with(&format!("heading,{tag} ")))
        .unwrap_or_else(|| panic!("no heading of {tag}: {all:#?}"));
    let mut rows = vec![all[at].clone()];
    rows.extend(
        all[at + 1..]
            .iter()
            .take_while(|l| !l.starts_with("heading,"))
            .cloned(),
    );
    rows
}

/// The width of the strip `tag`'s footprint on its rail, in nm.
fn strip_width(s: &Session, tag: &str) -> i64 {
    let g = s.query().get::<PlacementGeometry>(common::placement_of(s, tag));
    let g = g.as_ref().as_ref().expect("a geometry");
    g.footprint.width().0
}

/// The sample with its end plate EW-GEN-EP-1 from a library of the person's own (a starter part is never corrected,
/// doc 07 §5.8), which its terminals' parts name.
fn plate_from_own_library() -> Session {
    let own = ew_ids::Uuid::from_u128(0x5e47_0000_0000_4000_8000_0000_0000_0e90);
    let (project, records, mut lib) = common::sample_records();
    let plate = lib
        .iter()
        .find_map(|d| match d {
            ew_schema::LibDocument::Part(c) if c.item.part_number.as_str() == "EW-GEN-EP-1" => {
                Some(c.item_id)
            }
            _ => None,
        })
        .expect("the end plate");
    for d in &mut lib {
        let ew_schema::LibDocument::Part(c) = d else {
            continue;
        };
        if c.item_id == plate {
            c.library_id = ew_ids::Id::from_uuid(own);
            c.id = ew_ids::v5::lib_copy_id(own, plate.as_uuid());
        }
        for a in c.item.accessories.iter_mut().filter(|a| a.part.item == plate) {
            a.part.library = ew_ids::Id::from_uuid(own);
        }
    }
    common::session_of(project, records, lib)
}

/// [`plate_from_own_library`] with its end plate corrected in the project: number OWN-EP-3, 3 mm wide (the library's
/// is 2.2 mm).
fn corrected() -> Session {
    let mut s = plate_from_own_library();
    let plate = copy(&s, "EW-GEN-EP-1");
    let mut item: Value = {
        let q = s.query();
        let held = q
            .parts()
            .get(ew_ids::Id::from_uuid(plate.parse().expect("an ID")))
            .expect("held");
        serde_json::to_value(&held.item).expect("JSON")
    };
    item["part_number"] = json!("OWN-EP-3");
    item["mech"]["width"] = json!(3_000_000);
    let fork = ew_ids::Uuid::from_u128(0x5e47_0000_0000_4000_8000_0000_0000_0e93).to_string();
    common::run_cmd(
        &mut s,
        "part.edit_copy",
        json!({"copy": plate, "part": item, "fork": fork}),
    );
    s
}

#[test]
fn a_corrected_end_plate_is_counted_listed_and_drawn_by_its_own_copy() {
    let before = plate_from_own_library();
    let width_before = strip_width(&before, "-X1031");
    let mut s = corrected();
    // The parts list counts the project's copy for every strip whose terminals name the end plate, and not the
    // library's.
    let parts = lines(&s, "parts_list");
    assert!(
        parts
            .iter()
            .any(|l| l.starts_with("data,-X1031,1,pc,Generic,OWN-EP-3,")),
        "{parts:#?}"
    );
    assert!(!parts.iter().any(|l| l.contains("EW-GEN-EP-1")), "{parts:#?}");
    // The assembly list names it with its width, and the strip is as long by it.
    let rows = strip_rows(&s, "-X1031");
    assert!(
        rows.iter()
            .any(|l| l.starts_with("data,7,End plate,,,OWN-EP-3,") && l.contains(",3,,")),
        "{rows:#?}"
    );
    // The strip on the panel page: 0.8 mm wider, the corrected plate's 3 mm in place of 2.2 mm.
    assert_eq!(strip_width(&s, "-X1031") - width_before, 800_000);
    // EW-MFG-020's fix adds the corrected plate where the end plate was left off.
    let strip = s.resolve_reference("tag:-X1031").expect("-X1031").to_string();
    let declared = copy(&before, "EW-GEN-EP-1");
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [strip], "path": "accessory_choices", "value": [{"of": declared}]}),
    );
    let found: Vec<_> = ew_checks::run(&s.query(), &FULL, CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-MFG-020")
        .collect();
    let fix = found
        .first()
        .and_then(|f| f.fix.clone())
        .expect("EW-MFG-020 with its fix");
    assert_eq!(
        ew_checks::messages::fix_label(&fix, "en-GB"),
        "Add the end plate OWN-EP-3 after SH"
    );
}

/// A new part of the project's own: `number`, of kind `kind`, `width` mm wide, with the end plate `plate` per strip when
/// given.
fn own_part(s: &mut Session, number: &str, kind: &str, width: &str, plate: Option<&str>) {
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    for (f, v) in [
        ("manufacturer", "Acme"),
        ("part_number", number),
        ("kind", kind),
        ("mech.width", width),
        ("mech.height", "45"),
        ("mech.mount_kind", "ts35x7_5"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    if let Some(p) = plate {
        assert!(d.add_table_row("accessories"));
        for (f, v) in [("part", p), ("qty", "1"), ("per", "strip"), ("kind", "end_plate")] {
            s.set_draft_field(&mut d, &format!("accessories[0].{f}"), v)
                .expect(f);
        }
    }
    let made = s.new_part_command(&d).expect("the new part's command");
    common::run_cmd(s, &made.command.name, made.command.params.clone());
}

/// A terminal of one's own that names an end plate of one's own: the strip of those terminals takes that end plate
/// after its last terminal, in the parts list and the assembly list.
#[test]
fn a_terminal_of_ones_own_brings_an_end_plate_of_ones_own() {
    let mut s = common::sample();
    own_part(&mut s, "OWN-EP", "accessory", "1.5", None);
    own_part(&mut s, "OWN-TRM", "terminal", "6", Some("OWN-EP"));
    let terminal = copy(&s, "OWN-TRM");
    let strip = s.resolve_reference("tag:-X1031").expect("-X1031");
    let terminals: Vec<String> = s
        .query()
        .devices()
        .iter()
        .filter(|(_, d)| {
            d.aspects
                .terminal
                .as_ref()
                .is_some_and(|t| t.strip.as_uuid() == strip)
        })
        .map(|(id, _)| id.to_string())
        .collect();
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": terminals, "path": "parts",
            "value": [{"part": {"copy": terminal}, "qty": "1"}]}),
    );
    let rows = strip_rows(&s, "-X1031");
    assert!(
        rows.iter()
            .any(|l| l.contains(",End plate,,,OWN-EP,") && l.contains(",1.5,,")),
        "{rows:#?}"
    );
    assert!(!rows.iter().any(|l| l.contains("EW-GEN-EP-1")), "{rows:#?}");
    let parts = lines(&s, "parts_list");
    assert!(
        parts
            .iter()
            .any(|l| l.starts_with("data,-X1031,1,pc,Acme,OWN-EP,")),
        "{parts:#?}"
    );
}
