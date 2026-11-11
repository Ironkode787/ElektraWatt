// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On Golden A: the Properties rows of the layout records and their one
//! command each through `Session::layout_edit`: R1's and R2's rows with their free lengths, Length
//! lengthening R1, -102F1's Locked and Rotation rows, `layout.space.edit`, `layout.enclosure.edit` and
//! `layout.surface.edit`, the panel page's Surface view rows and the refusals in words.

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
    // The three rows sit under their own category, Surface view.
    let categories: Vec<ew_api::output::Category> = s
        .property_sheet(&[page])
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .filter(|r| r.key.starts_with("page/derived.") && view.iter().any(|v| v.0 == r.key))
        .map(|r| r.category)
        .collect();
    assert_eq!(categories, [ew_api::output::Category::SurfaceView; 3]);
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
    // An ID that names no surface: refused in words, without the ID.
    assert_eq!(
        refusal(edit(&s, "page", page, "surface", &Uuid::from_u128(7).to_string())),
        "That surface no longer exists; choose one of the list."
    );
    // An existing surface's ID in another form names it: upper case, braces, simple, padded.
    let plate = uuid::<MountingSurface>("mounting_surfaces", "-UC1");
    for form in [
        plate.to_string().to_uppercase(),
        format!("{{{plate}}}"),
        plate.simple().to_string(),
        format!("  {plate} "),
    ] {
        let cmd = edit(&s, "page", page, "surface", &form).unwrap_or_else(|e| panic!("{form}: {e:?}"));
        assert_eq!(cmd.params["surface"], json!(plate.to_string()), "{form}");
    }
    // An ID of no surface in another form is still refused.
    assert_eq!(
        refusal(edit(
            &s,
            "page",
            page,
            "surface",
            &format!("{{{}}}", Uuid::from_u128(7))
        )),
        "That surface no longer exists; choose one of the list."
    );
    assert_eq!(
        refusal(edit(&s, "page", page, "label_mode", "Short")),
        "Choose Abbreviated or Full."
    );
}

/// The page ID of folio `folio`.
fn page_of(s: &Session, folio: &str) -> Uuid {
    s.page_rows()
        .into_iter()
        .find(|r| r.folio == folio)
        .unwrap_or_else(|| panic!("no folio {folio}"))
        .id
}

/// The project's enclosure part copy (NSYPLA1074G).
fn enclosure_part(s: &Session) -> Uuid {
    s.layout_part_offers("enclosure")
        .into_iter()
        .find(|o| o.number == "NSYPLA1074G")
        .expect("the project's enclosure part")
        .part
}

/// E55 placed on -UC1's plate at (300, 400) mm; the placement's ID.
fn e55_on_the_plate(s: &mut Session) -> Uuid {
    let e55 = uuid::<ew_schema::Device>("devices", "E55");
    let plate = uuid::<MountingSurface>("mounting_surfaces", "-UC1");
    run(
        s,
        &AnyCommand {
            name: "layout.place".into(),
            params: json!({"device": e55.to_string(), "carrier": {"surface": plate.to_string()},
                "pose": {"t": [300_000_000, 400_000_000, 0], "q": [1_000_000_000, 0, 0, 0]}}),
        },
    );
    s.query()
        .table::<Placement>()
        .iter()
        .find(|(_, p)| p.device.as_uuid() == e55)
        .map(|(id, _)| id.as_uuid())
        .expect("E55's placement")
}

/// A second enclosure, at +FIELD, from the project's enclosure part, its tag frozen as -UC1 (as -UC1 of +J-E-2416 is);
/// its surface's ID.
fn field_enclosure(s: &mut Session) -> Uuid {
    let field = uuid::<ew_schema::StructureNode>("structure", "+FIELD");
    let before: Vec<Uuid> = s.surface_choices().into_iter().map(|c| c.0).collect();
    let part = enclosure_part(s);
    run(
        s,
        &AnyCommand {
            name: "layout.enclosure.create_from_part".into(),
            params: json!({"location": field.to_string(), "part": part.to_string()}),
        },
    );
    let surface = s
        .surface_choices()
        .into_iter()
        .map(|c| c.0)
        .find(|id| !before.contains(id))
        .expect("a new surface");
    let device = {
        let q = s.query();
        let enclosure = q
            .table::<MountingSurface>()
            .get(Id::from_uuid(surface))
            .unwrap()
            .enclosure;
        q.table::<Enclosure>().get(enclosure).unwrap().device
    };
    run(
        s,
        &AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "device", "targets": [device.as_uuid().to_string()],
                "path": "tag.frozen", "value": {"text": "UC1"}}),
        },
    );
    surface
}

/// The choices of a row (its type's options).
fn options_of(s: &Session, id: Uuid, key: &str) -> Vec<String> {
    let sheet = s.property_sheet(&[id]);
    let row = sheet
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no row {key}"));
    match &row.ty {
        ew_api::output::FieldType::Enum { options } => options.clone(),
        other => panic!("{other:?}"),
    }
}

/// Two locations with an enclosure -UC1 each read alike by the short tag, so each surface reads by its
/// enclosure's full designation; choosing the second (by its value, the surface's ID) sets the second.
#[test]
fn two_uc1_plates_read_by_their_full_designations_and_the_choice_sets_the_one_chosen() {
    let mut s = golden_common::session(&golden_common::batch());
    let page = page_of(&s, "902");
    let first = uuid::<MountingSurface>("mounting_surfaces", "-UC1");
    let second = field_enclosure(&mut s);
    let choices = s.surface_choices();
    assert_eq!(
        choices,
        [
            (second, "+FIELD-UC1 plate".to_owned()),
            (first, "=P01+J-E-2416-UC1 plate".to_owned()),
        ]
    );
    assert_eq!(
        options_of(&s, page, "page/derived.surface"),
        ["+FIELD-UC1 plate", "=P01+J-E-2416-UC1 plate"]
    );
    assert_eq!(
        text_of(&s, page, "page/derived.surface"),
        "=P01+J-E-2416-UC1 plate"
    );
    // The choice by its value: the second surface, though the words differ from the shown ones only in the location.
    let cmd = edit(&s, "page", page, "surface", &second.to_string()).unwrap();
    assert_eq!(cmd.params["surface"], json!(second.to_string()));
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, page, "page/derived.surface"), "+FIELD-UC1 plate");
    // Typed words name one surface.
    let cmd = edit(&s, "page", page, "surface", "=P01+J-E-2416-UC1 plate").unwrap();
    assert_eq!(cmd.params["surface"], json!(first.to_string()));
    run(&mut s, &cmd);
    // A list chosen before the second enclosure was deleted: its ID is refused in words, without the ID.
    let enclosure = s
        .query()
        .table::<MountingSurface>()
        .get(Id::from_uuid(second))
        .unwrap()
        .enclosure;
    run(
        &mut s,
        &AnyCommand {
            name: "layout.enclosure.delete".into(),
            params: json!({"enclosure": enclosure.as_uuid().to_string()}),
        },
    );
    assert_eq!(
        refusal(edit(&s, "page", page, "surface", &second.to_string())),
        "That surface no longer exists; choose one of the list."
    );
}

/// A hostile file: two surfaces of one enclosure read alike after the full designation too, so each gains
/// its ordinal in page order: 902 shows -UC1's plate, which reads "(1)"; the other, shown by no page, "(2)".
#[test]
fn two_surfaces_of_one_enclosure_read_with_their_ordinals() {
    let plate = uuid::<MountingSurface>("mounting_surfaces", "-UC1");
    let twin = Uuid::from_u128(0x7717);
    let s = golden_common::session_with(&golden_common::batch(), |_, records| {
        let copy = records
            .iter()
            .find_map(|r| match r {
                ew_schema::Record::MountingSurface(m) if m.id.as_uuid() == plate => Some(m.clone()),
                _ => None,
            })
            .expect("-UC1's plate");
        records.push(ew_schema::Record::MountingSurface(MountingSurface {
            id: Id::from_uuid(twin),
            ..copy
        }));
    });
    assert_eq!(
        s.surface_choices(),
        [
            (plate, "=P01+J-E-2416-UC1 plate (1)".to_owned()),
            (twin, "=P01+J-E-2416-UC1 plate (2)".to_owned()),
        ]
    );
    assert_eq!(
        text_of(&s, page_of(&s, "902"), "page/derived.surface"),
        "=P01+J-E-2416-UC1 plate (1)"
    );
    assert_eq!(
        s.property_sheet(&[twin]).header.title,
        "=P01+J-E-2416-UC1 plate (2)"
    );
}

/// Device is a link (its tag the words); Realised part offers only the part uses `layout.move {part}`
/// takes (an accessory mounted on its parent is not one); Label is read from the page that shows the placement's
/// surface, and is absent when none does.
#[test]
fn a_placements_device_is_a_link_its_parts_mount_and_its_label_comes_from_the_showing_page() {
    let mut s = golden_common::session(&golden_common::batch());
    let f1 = uuid::<Placement>("placements", "-102F1");
    let sheet = s.property_sheet(&[f1]);
    let device = sheet
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .find(|r| r.key == "placement/derived.device")
        .expect("the Device row")
        .clone();
    assert_eq!(
        device.ty,
        ew_api::output::FieldType::Link {
            kind: "device".to_owned()
        }
    );
    let placement = s
        .query()
        .table::<Placement>()
        .get(Id::from_uuid(f1))
        .cloned()
        .unwrap();
    match &device.value {
        RowValue::One { json, text } => {
            assert_eq!(
                (json, text.as_str()),
                (&json!(placement.device.as_uuid().to_string()), "-102F1")
            );
        }
        other => panic!("{other:?}"),
    }
    let offered = options_of(&s, f1, "placement/derived.realised_part");
    // An accessory part use (532216, mounted on its parent) is not offered.
    let parts = s.query().devices().get(placement.device).unwrap().parts.clone();
    let mut uses = serde_json::to_value(&parts).unwrap();
    let accessory = "22ea7428-f5dd-5283-a359-ed71da1f5efc";
    uses.as_array_mut()
        .unwrap()
        .push(json!({"part": {"copy": accessory}, "qty": "1"}));
    run(
        &mut s,
        &AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "device", "targets": [placement.device.as_uuid().to_string()],
                "path": "parts", "value": uses}),
        },
    );
    assert_eq!(options_of(&s, f1, "placement/derived.realised_part"), offered);
    assert!(!offered.contains(&"532216".to_owned()), "{offered:?}");
    // The label comes from 902, which shows -UC1's plate; with 902 showing another plate no page shows it.
    assert_eq!(text_of(&s, f1, "placement/derived.label"), "-102F1");
    let page = page_of(&s, "902");
    let other = field_enclosure(&mut s);
    let cmd = edit(&s, "page", page, "surface", &other.to_string()).unwrap();
    run(&mut s, &cmd);
    let keys: Vec<String> = rows(&s, f1).into_iter().map(|r| r.0).collect();
    assert!(!keys.contains(&"placement/derived.label".to_owned()), "{keys:?}");
}

/// The rows' refusals in words, each whole; the enclosure's Position row asks for the three
/// lengths it shows.
#[test]
fn the_layout_rows_refusals_read_whole() {
    let mut s = golden_common::session(&golden_common::batch());
    let f1 = uuid::<Placement>("placements", "-102F1");
    let r1 = uuid::<Carrier>("carriers", "R1");
    let enclosure = uuid::<Enclosure>("enclosures", "-UC1");
    let page = page_of(&s, "902");
    let gone = Uuid::from_u128(7);
    assert_eq!(
        refusal(edit(&s, "placement", gone, "locked", "true")),
        "The placement is no longer in the project."
    );
    assert_eq!(
        refusal(edit(&s, "carrier", gone, "pe_bond", "true")),
        "The carrier is no longer in the project."
    );
    assert_eq!(
        refusal(edit(&s, "page", gone, "label_mode", "Full")),
        "The page is no longer in the project."
    );
    assert_eq!(
        refusal(edit(&s, "carrier", r1, "part", "NOPART")),
        "The project has no rail part NOPART."
    );
    assert_eq!(
        refusal(edit(&s, "enclosure", enclosure, "position", "10")),
        "Type three lengths in millimetres, such as 10, 20, 0."
    );
    // A plate placement's rotation takes a quarter turn.
    let plate = e55_on_the_plate(&mut s);
    assert_eq!(
        refusal(edit(&s, "placement", plate, "rotation", "45°")),
        "Choose 0°, 90°, 180° or 270°."
    );
    // A panel page without a surface view: its Label mode needs the surface first.
    run(
        &mut s,
        &AnyCommand {
            name: "page.create".into(),
            params: json!({"number": 950, "kind": "panel_layout"}),
        },
    );
    let p950 = page_of(&s, "950");
    assert_eq!(
        refusal(edit(&s, "page", p950, "label_mode", "Full")),
        "Choose the surface first."
    );
    // A row's text that reads but that the command refuses compiles; the refusal is the command's (the shell keeps
    // the old value, `layout_setup.rs`).
    assert!(edit(&s, "placement", f1, "position", "9999").is_ok());
    assert!(edit(&s, "page", page, "datum", "1, 2").is_ok());
}

/// PS-5: Position on a rail and on the plate, Realised part, Rotation on the plate, a duct's Width and
/// Height and the page's Datum each commit their command, and the row then reads the new value.
#[test]
fn position_realised_part_rotation_duct_size_and_datum_commit() {
    let mut s = golden_common::session(&golden_common::batch());
    let f1 = uuid::<Placement>("placements", "-102F1");
    let cmd = edit(&s, "placement", f1, "position", "450").unwrap();
    assert_eq!(cmd.name, "layout.move");
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, f1, "placement/derived.position"), "450 mm");
    // Realised part: -102F1's device gains -101F1's part (A9P52610) as a second use, and the row commits it in place
    // of A9F78116.
    assert_eq!(text_of(&s, f1, "placement/derived.realised_part"), "A9F78116");
    let other = {
        let q = s.query();
        q.table::<ew_schema::LibCopy<ew_schema::Part>>()
            .iter()
            .find(|(_, c)| c.item.part_number.as_str() == "A9P52610")
            .map(|(id, _)| id)
            .expect("A9P52610's copy")
    };
    let device = s
        .query()
        .table::<Placement>()
        .get(Id::from_uuid(f1))
        .unwrap()
        .device;
    let mut uses = serde_json::to_value(&s.query().devices().get(device).unwrap().parts).unwrap();
    uses.as_array_mut()
        .unwrap()
        .push(json!({"part": {"copy": other.as_uuid().to_string()}, "qty": "1"}));
    run(
        &mut s,
        &AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "device", "targets": [device.as_uuid().to_string()],
                "path": "parts", "value": uses}),
        },
    );
    assert_eq!(
        options_of(&s, f1, "placement/derived.realised_part"),
        ["A9F78116", "A9P52610"]
    );
    let cmd = edit(&s, "placement", f1, "realised_part", "A9P52610").unwrap();
    assert_eq!(cmd.name, "layout.move");
    assert_eq!(cmd.params["part"], json!(other.as_uuid().to_string()));
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, f1, "placement/derived.realised_part"), "A9P52610");
    let plate = e55_on_the_plate(&mut s);
    let cmd = edit(&s, "placement", plate, "rotation", "180°").unwrap();
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, plate, "placement/derived.rotation"), "180°");
    let at = text_of(&s, plate, "placement/derived.position");
    let cmd = edit(&s, "placement", plate, "position", "300, 400").unwrap();
    run(&mut s, &cmd);
    assert_eq!(
        text_of(&s, plate, "placement/derived.position"),
        "300, 400 mm",
        "from {at}"
    );
    let d1 = uuid::<Carrier>("carriers", "D1");
    for (row, text) in [("width", "50"), ("height", "80")] {
        let cmd = edit(&s, "carrier", d1, row, text).unwrap();
        assert_eq!(cmd.name, "layout.carrier.edit");
        run(&mut s, &cmd);
        assert_eq!(
            text_of(&s, d1, &format!("carrier/derived.{row}")),
            format!("{text} mm")
        );
    }
    let page = page_of(&s, "902");
    let cmd = edit(&s, "page", page, "datum", "100, 200").unwrap();
    assert_eq!(cmd.name, "page.surface_view.set");
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, page, "page/derived.datum"), "100, 200 mm");
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
    // The library's enclosure by its part number: the project's held copy of it has no copies, so the offer with its
    // copies is the library's (the starter's enclosure is offered too, below).
    let library = offers
        .iter()
        .find(|o| o.number == "NSYPLA1074G" && !o.copies.is_empty())
        .unwrap_or_else(|| panic!("the library's enclosure: {numbers:?}"))
        .clone();
    assert_eq!(library.number, "NSYPLA1074G");
    let embedded: Vec<String> = library
        .copies
        .iter()
        .filter_map(|c| c["part_number"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(embedded, ["NSYPLA1074G", "NSYPMM107"], "the part and its plate");
    // The starter's enclosure is offered too, with its plate EW-GEN-PLT-1.
    let starter = offers
        .iter()
        .find(|o| o.number == "EW-GEN-ENC-1")
        .unwrap_or_else(|| panic!("the starter's enclosure: {numbers:?}"));
    let starter_embedded: Vec<String> = starter
        .copies
        .iter()
        .filter_map(|c| c["part_number"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(
        starter_embedded,
        ["EW-GEN-ENC-1", "EW-GEN-PLT-1"],
        "the starter's enclosure and its plate"
    );
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

/// The scale of the drawn page `page`.
fn scale_of(s: &Session, page: Uuid) -> ew_schema::Scale {
    s.query()
        .pages()
        .get(Id::from_uuid(page))
        .expect("the page")
        .scale
        .clone()
}

/// The Surface row of a new panel page (drawn not to scale) sends the fitting scale with the view
/// and the datum centred at that scale, in one command; the page then keeps it. 902's own 1:10 is kept.
#[test]
fn a_new_panel_page_takes_the_fitting_scale_with_its_surface() {
    let mut s = golden_common::session(&golden_common::batch());
    let p902 = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == "902")
        .expect("902")
        .id;
    let plate = uuid::<MountingSurface>("mounting_surfaces", "-UC1");
    assert_eq!(scale_of(&s, p902), ew_schema::Scale::OneTo(10));
    let keep = edit(&s, "page", p902, "surface", "-UC1 plate").unwrap();
    assert_eq!(keep.params.get("scale"), None, "a chosen scale is kept");

    let before: Vec<Uuid> = s.page_rows().into_iter().map(|r| r.id).collect();
    run(
        &mut s,
        &AnyCommand {
            name: "page.create".into(),
            params: json!({"after": p902.to_string(), "kind": "panel_layout"}),
        },
    );
    let new = s
        .page_rows()
        .into_iter()
        .find(|r| !before.contains(&r.id))
        .expect("the new page")
        .id;
    assert_eq!(scale_of(&s, new), ew_schema::Scale::NotToScale);
    let n = s.fitting_scale(new, plate).expect("the plate fits a scale");
    assert_eq!(n, 10, "-UC1's plate on the page frame's drawing area");
    let datum = s.centred_surface_datum(new, plate).expect("a centred datum");
    let cmd = edit(&s, "page", new, "surface", "-UC1 plate").unwrap();
    assert_eq!(cmd.name, "page.surface_view.set");
    assert_eq!(
        cmd.params,
        json!({"page": new.to_string(), "surface": plate.to_string(), "datum_at": [datum.0, datum.1],
            "label_mode": "abbreviated", "scale": {"one_to": n}})
    );
    run(&mut s, &cmd);
    assert_eq!(scale_of(&s, new), ew_schema::Scale::OneTo(n));
    // The page now has its scale: a later edit sends none.
    let again = edit(&s, "page", new, "label_mode", "Full").unwrap();
    assert_eq!(again.params.get("scale"), None);
    let again = edit(&s, "page", new, "surface", "-UC1 plate").unwrap();
    assert_eq!(again.params.get("scale"), None);
}
