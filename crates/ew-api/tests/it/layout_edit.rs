// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 4 (D12, rulings 3, 6 and 9) on Golden A: the Properties rows of the layout records and their one
//! command each through `Session::layout_edit`: R1's and R2's rows with their free lengths (PS-5), Length lengthening
//! R1, -102F1's Locked and Rotation rows, FT-840's `layout.space.edit`, `layout.enclosure.edit` and
//! `layout.surface.edit`, the panel page's Surface view rows (PS-4) and the refusals in words.

use crate::golden_common;

use ew_api::output::{EditRoute, LayoutEdit, LayoutEditError, PropRow, RowValue};
use ew_api::{AnyCommand, Session};
use ew_derive::ew_ids::{Id, Uuid};
use ew_import_tables::ids::record_id;
use ew_schema::{Carrier, Enclosure, LayoutSpace, MountingSurface, Placement};
use serde_json::json;

fn uuid<T>(table: &str, key: &str) -> Uuid {
    let id: Id<T> = record_id(table, key);
    id.as_uuid()
}

/// The rows of the record `id`'s Properties sheet: (key, text, the command it commits through).
fn rows(s: &Session, id: Uuid) -> Vec<(String, String, Option<String>)> {
    let sheet = s.property_sheet(&[id]);
    sheet
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .map(|r: &PropRow| {
            let text = match &r.value {
                RowValue::One { text, .. } => text.clone(),
                other => format!("{other:?}"),
            };
            let command = match &r.route {
                EditRoute::Command { command } => Some(command.clone()),
                _ => None,
            };
            (r.key.clone(), text, command)
        })
        .collect()
}

fn text_of(s: &Session, id: Uuid, key: &str) -> String {
    rows(s, id)
        .into_iter()
        .find(|r| r.0 == key)
        .unwrap_or_else(|| panic!("no row {key}"))
        .1
}

fn edit(s: &Session, kind: &str, target: Uuid, row: &str, text: &str) -> Result<AnyCommand, LayoutEditError> {
    s.layout_edit(&LayoutEdit {
        kind: kind.to_owned(),
        target,
        row: format!("derived.{row}"),
        text: text.to_owned(),
    })
}

fn run(s: &mut Session, cmd: &AnyCommand) {
    s.execute(cmd)
        .unwrap_or_else(|e| panic!("{} {}: {e:?}", cmd.name, cmd.params));
}

fn refusal(r: Result<AnyCommand, LayoutEditError>) -> String {
    match r {
        Err(LayoutEditError::Invalid(m)) => m,
        Ok(c) => panic!("compiled {} {}", c.name, c.params),
    }
}

#[test]
fn a_rails_rows_name_their_commands_and_read_the_free_length() {
    let s = golden_common::session(&golden_common::batch());
    let r1 = uuid::<Carrier>("carriers", "R1");
    let got: Vec<(String, String, Option<String>)> = rows(&s, r1)
        .into_iter()
        .filter(|r| r.0 != "carrier/derived.part")
        .collect();
    let edit = Some("layout.carrier.edit".to_owned());
    assert_eq!(
        got,
        [
            ("carrier/derived.profile".to_owned(), "TS 35×7.5".to_owned(), None),
            (
                "carrier/derived.start".to_owned(),
                "40, 740 mm".to_owned(),
                edit.clone()
            ),
            (
                "carrier/derived.length".to_owned(),
                "570 mm".to_owned(),
                edit.clone()
            ),
            (
                "carrier/derived.free_length".to_owned(),
                "341 mm".to_owned(),
                None
            ),
            ("carrier/derived.pe_bond".to_owned(), "true".to_owned(), edit),
        ]
    );
    let r2 = uuid::<Carrier>("carriers", "R2");
    assert_eq!(text_of(&s, r2, "carrier/derived.free_length"), "381.4 mm");
}

#[test]
fn length_lengthens_the_rail_and_a_bad_text_is_refused_in_words() {
    let mut s = golden_common::session(&golden_common::batch());
    let r1 = uuid::<Carrier>("carriers", "R1");
    let cmd = edit(&s, "carrier", r1, "length", "600").unwrap();
    assert_eq!(cmd.name, "layout.carrier.edit");
    assert_eq!(
        cmd.params,
        json!({"carrier": r1.to_string(), "path": [[40_000_000, 740_000_000], [640_000_000, 740_000_000]]})
    );
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, r1, "carrier/derived.length"), "600 mm");
    assert_eq!(text_of(&s, r1, "carrier/derived.free_length"), "371 mm");
    assert_eq!(
        refusal(edit(&s, "carrier", r1, "length", "long")),
        "Type a length above 0 mm."
    );
    assert_eq!(
        refusal(edit(&s, "carrier", r1, "start", "40")),
        "Type two lengths in millimetres, such as 40, 740."
    );
    assert_eq!(
        refusal(edit(&s, "carrier", r1, "pe_bond", "maybe")),
        "Choose yes or no."
    );
    assert_eq!(
        refusal(edit(&s, "carrier", r1, "free_length", "10")),
        "This row cannot be changed here."
    );
}

#[test]
fn a_placements_locked_and_rotation_rows_commit_their_commands() {
    let mut s = golden_common::session(&golden_common::batch());
    let f1 = uuid::<Placement>("placements", "-102F1");
    let keys: Vec<String> = rows(&s, f1).into_iter().map(|r| r.0).collect();
    assert_eq!(
        keys,
        [
            "placement/derived.device",
            "placement/derived.realised_part",
            "placement/derived.carrier",
            "placement/derived.position",
            "placement/derived.rotation",
            "placement/derived.locked",
            "placement/derived.label",
            "placement/derived.footprint",
            "placement/derived.clearance",
        ]
    );
    assert_eq!(text_of(&s, f1, "placement/derived.carrier"), "Rail 1");
    assert_eq!(text_of(&s, f1, "placement/derived.position"), "158 mm");
    let lock = edit(&s, "placement", f1, "locked", "true").unwrap();
    assert_eq!(
        (lock.name.as_str(), &lock.params),
        (
            "layout.lock",
            &json!({"placement": f1.to_string(), "locked": true})
        )
    );
    run(&mut s, &lock);
    assert_eq!(text_of(&s, f1, "placement/derived.locked"), "true");
    let turn = edit(&s, "placement", f1, "rotation", "Flipped").unwrap();
    assert_eq!(turn.name, "layout.move");
    run(&mut s, &turn);
    assert_eq!(text_of(&s, f1, "placement/derived.rotation"), "Flipped");
    assert_eq!(
        refusal(edit(&s, "placement", f1, "rotation", "90°")),
        "Choose Normal or Flipped."
    );
    assert_eq!(
        refusal(edit(&s, "placement", f1, "realised_part", "XYZ")),
        "-102F1 uses no part XYZ; choose one of its parts."
    );
    assert_eq!(
        refusal(edit(&s, "placement", f1, "position", "far")),
        "Type a length in millimetres, such as 158.5."
    );
}

#[test]
fn the_space_enclosure_and_surface_rows_commit_ft840s_commands() {
    let mut s = golden_common::session(&golden_common::batch());
    let space = uuid::<LayoutSpace>("layout_spaces", "+J-E-2416");
    let enclosure = uuid::<Enclosure>("enclosures", "-UC1");
    let surface = uuid::<MountingSurface>("mounting_surfaces", "-UC1");
    let name = edit(&s, "layout_space", space, "name", "Main cabinet").unwrap();
    assert_eq!(name.name, "layout.space.edit");
    run(&mut s, &name);
    assert_eq!(text_of(&s, space, "layout_space/derived.name"), "Main cabinet");
    let pose = edit(&s, "enclosure", enclosure, "position", "10, 20").unwrap();
    assert_eq!(pose.name, "layout.enclosure.edit");
    run(&mut s, &pose);
    assert_eq!(
        text_of(&s, enclosure, "enclosure/derived.position"),
        "10, 20, 0 mm"
    );
    let bond = edit(&s, "mounting_surface", surface, "pe_bond", "false").unwrap();
    assert_eq!(bond.name, "layout.surface.edit");
    run(&mut s, &bond);
    assert_eq!(text_of(&s, surface, "mounting_surface/derived.pe_bond"), "false");
}

#[test]
fn the_panel_pages_surface_view_rows_set_the_view() {
    let mut s = golden_common::session(&golden_common::batch());
    let page = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == "902")
        .expect("902")
        .id;
    let view: Vec<(String, String, Option<String>)> = rows(&s, page)
        .into_iter()
        .filter(|r| r.2.as_deref() == Some("page.surface_view.set"))
        .collect();
    let set = Some("page.surface_view.set".to_owned());
    assert_eq!(
        view.iter()
            .map(|r| (r.0.as_str(), r.2.clone()))
            .collect::<Vec<_>>(),
        [
            ("page/derived.surface", set.clone()),
            ("page/derived.label_mode", set.clone()),
            ("page/derived.datum", set),
        ]
    );
    assert_eq!(view[0].1, "-UC1 plate");
    assert_eq!(view[1].1, "Abbreviated");
    let full = edit(&s, "page", page, "label_mode", "Full").unwrap();
    assert_eq!(full.name, "page.surface_view.set");
    run(&mut s, &full);
    assert_eq!(text_of(&s, page, "page/derived.label_mode"), "Full");
    let t = s.panel_tables("902").expect("902's panel");
    assert!(
        t.placements.iter().any(|p| p.label == "=P01+J-E-2416-X1031"),
        "{:?}",
        t.placements.iter().map(|p| &p.label).collect::<Vec<_>>()
    );
    assert_eq!(
        refusal(edit(&s, "page", page, "surface", "nowhere")),
        "The project has no surface nowhere; choose one of the list."
    );
    assert_eq!(
        refusal(edit(&s, "page", page, "label_mode", "Short")),
        "Choose Abbreviated or Full."
    );
}

/// Removes the fixture directory at the end.
struct Dir(std::path::PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A golden library document (a part by number, or the manufacturer of one) moved into the library `lib`, with its
/// references (manufacturer, enclosure plate) into `lib` too.
fn golden_doc_in(lib: Uuid, pick: impl Fn(&ew_schema::LibDocument) -> bool) -> ew_schema::LibDocument {
    let batch = golden_common::batch();
    let doc = batch.lib.iter().find(|d| pick(d)).expect("a golden document");
    let mut v = serde_json::to_value(doc).unwrap();
    v["library_id"] = json!(lib.to_string());
    if v.get("manufacturer").is_some() {
        v["manufacturer"]["library"] = json!(lib.to_string());
        v["manufacturer"]["version_req"] = json!("^1");
    }
    if v["sections"]["enclosure"].get("plate").is_some() {
        v["sections"]["enclosure"]["plate"]["library"] = json!(lib.to_string());
        v["sections"]["enclosure"]["plate"]["version_req"] = json!("^1");
    }
    ew_schema::LibDocument::from_value(v).expect("a library document")
}

fn part_named(number: &'static str) -> impl Fn(&ew_schema::LibDocument) -> bool {
    move |d| matches!(d, ew_schema::LibDocument::Part(p) if p.item.part_number.as_str() == number)
}

/// PS-1's library path (ruling 8): a system library holding Golden A's enclosure part and its plate part; the offer
/// of `layout_part_offers` carries both through `copies` (the plate as the enclosure section's closure), and
/// `layout.enclosure.create_from_part` with them creates +FIELD's enclosure with its plate in one command.
#[test]
fn a_library_enclosure_part_is_offered_with_its_plate_and_creates_the_enclosure() {
    use ew_api::library::{LibraryTiers, Registry};
    use ew_api::{Session, SessionOptions};
    let root = Dir(ew_testdir::fresh_dir("layout-edit-library"));
    let tiers = |system: Vec<std::path::PathBuf>| LibraryTiers {
        system,
        community: root.0.join("data").join("packs"),
        company: Vec::new(),
        user: root.0.join("data").join("libraries"),
        cache: root.0.join("cache"),
    };
    let dir = root.0.join("system");
    {
        let t = LibraryTiers {
            company: vec![dir.clone()],
            ..tiers(Vec::new())
        };
        let mut reg = Registry::open(&t).unwrap();
        let lib = reg
            .create(&dir.join("cabinets.ewlib"), "cabinets", "Cabinets", "CC0-1.0")
            .unwrap();
        let schneider = golden_doc_in(
            lib,
            |d| matches!(d, ew_schema::LibDocument::Manufacturer(m) if m.item.key.as_str().to_ascii_lowercase().contains("schneider")),
        );
        let docs = vec![
            golden_doc_in(lib, part_named("NSYPLA1074G")),
            golden_doc_in(lib, part_named("NSYPMM107")),
            schneider,
        ];
        reg.write(lib, &docs).unwrap();
    }
    let batch = golden_common::batch();
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            ew_schema::Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let options = SessionOptions {
        libraries: Some(tiers(vec![dir])),
        ..SessionOptions::default()
    };
    let mut s = Session::from_records(project.unwrap(), records, batch.lib.clone(), options).unwrap();
    let offers = s.layout_part_offers("enclosure");
    let numbers: Vec<(&str, usize)> = offers
        .iter()
        .map(|o| (o.number.as_str(), o.copies.len()))
        .collect();
    let library = offers
        .iter()
        .find(|o| !o.copies.is_empty())
        .unwrap_or_else(|| panic!("a library offer: {numbers:?}"))
        .clone();
    assert_eq!(library.number, "NSYPLA1074G");
    let embedded: Vec<String> = library
        .copies
        .iter()
        .filter_map(|c| c["part_number"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(embedded, ["NSYPLA1074G", "NSYPMM107"], "the part and its plate");
    let field = uuid::<ew_schema::StructureNode>("structure", "+FIELD");
    let before = s.query().table::<MountingSurface>().iter().count();
    let cmd = AnyCommand {
        name: "layout.enclosure.create_from_part".into(),
        params: json!({"location": field.to_string(), "part": library.part.to_string(), "copies": library.copies}),
    };
    run(&mut s, &cmd);
    assert_eq!(
        s.query().table::<MountingSurface>().iter().count(),
        before + 1,
        "one plate more"
    );
}
