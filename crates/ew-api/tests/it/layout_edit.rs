// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On the sample project: the Properties rows of the layout records and their one
//! command each through `Session::layout_edit`: the rails' rows with their free lengths, Length
//! lengthening rail 1, -102F1's Locked and Rotation rows, `layout.space.edit`, `layout.enclosure.edit` and
//! `layout.surface.edit`, the panel page's Surface view rows and the refusals in words. The sample's cabinet
//! layout: the enclosure -UC1 and its plate, two rails and a duct drawn on 902, every device placed on the rails.

use crate::common;

use ew_api::output::{EditRoute, LayoutEdit, LayoutEditError, PropRow, RowValue};
use ew_api::{AnyCommand, Session};
use ew_ids::{Id, Uuid};
use ew_schema::{Carrier, CarrierKind, Enclosure, LayoutSpace, MountingSurface, Placement};
use serde_json::json;

/// The starter library of the sample's part copies.
const STARTER: &str = "d76f5765-c456-5393-935c-4cec32a87b01";

/// The copy ID of the accessory part ACC-1 of [`with_accessory`].
fn accessory_copy() -> Uuid {
    let item = Uuid::from_u128(0xacc1);
    ew_ids::v5::lib_copy_id::<()>(STARTER.parse().unwrap(), item).as_uuid()
}

/// The sample with a held copy of an accessory part, ACC-1, which mounts on its parent (no device uses it yet).
fn with_accessory() -> Session {
    let (project, records, mut lib) = common::sample_records();
    let maker = lib
        .iter()
        .find_map(|d| match d {
            ew_schema::LibDocument::Part(p) if p.item.part_number.as_str() == "EW-GEN-MCB-1" => {
                Some(serde_json::to_value(&p.item.manufacturer).unwrap())
            }
            _ => None,
        })
        .expect("the generic maker");
    let mut doc = json!({
        "$k": "part", "id": accessory_copy().to_string(), "library_id": STARTER,
        "item_id": Uuid::from_u128(0xacc1).to_string(), "version": "1.0.0",
        "sha256": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "manufacturer": maker, "part_number": "ACC-1", "description": "Auxiliary contact block",
        "mech": {"width": 9000000, "height": 45000000, "depth": 40000000, "mount_kind": "on_parent"}
    });
    let hash =
        ew_format::library_copy_hash(&ew_schema::LibDocument::from_value(doc.clone()).unwrap()).unwrap();
    doc["sha256"] = json!(ew_schema::Sha256(hash).to_string());
    lib.push(ew_schema::LibDocument::from_value(doc).unwrap());
    common::session_of(project, records, lib)
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
        others: Vec::new(),
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
        Err(e @ LayoutEditError::Refused(_)) => e.to_string(),
        Ok(c) => panic!("compiled {} {}", c.name, c.params),
    }
}

/// The `n`th carrier of `kind` in the order the script drew them (rail 1, rail 2; the duct).
fn carrier(s: &Session, kind: CarrierKind, n: usize) -> Uuid {
    let q = s.query();
    let mut ids: Vec<Uuid> = q
        .table::<Carrier>()
        .iter()
        .filter(|(_, c)| c.kind == kind)
        .map(|(id, _)| id.as_uuid())
        .collect();
    ids.sort();
    ids[n]
}

/// A carrier's stored path as (start x, start y, length) in whole mm.
fn rail_mm(s: &Session, id: Uuid) -> (i64, i64, i64) {
    let q = s.query();
    let c = q
        .table::<Carrier>()
        .get(Id::from_uuid(id))
        .expect("the carrier")
        .clone();
    let (a, b) = (c.path[0], c.path[c.path.len() - 1]);
    (a.x.0 / 1_000_000, a.y.0 / 1_000_000, (b.x.0 - a.x.0) / 1_000_000)
}

/// A length row's millimetres.
fn mm_of(text: &str) -> f64 {
    text.strip_suffix(" mm")
        .expect("a length")
        .parse()
        .expect("a number")
}

/// The sample's one layout space, enclosure and plate.
fn space(s: &Session) -> Uuid {
    s.query()
        .table::<LayoutSpace>()
        .ids()
        .next()
        .expect("the layout space")
        .as_uuid()
}

fn enclosure(s: &Session) -> Uuid {
    s.query()
        .table::<Enclosure>()
        .ids()
        .next()
        .expect("the enclosure")
        .as_uuid()
}

fn plate(s: &Session) -> Uuid {
    s.query()
        .table::<MountingSurface>()
        .ids()
        .next()
        .expect("-UC1's plate")
        .as_uuid()
}

/// The placement of the device printed as `tag`.
fn placement_of(s: &Session, tag: &str) -> Uuid {
    let device = s.resolve_reference(tag).expect(tag);
    s.query()
        .table::<Placement>()
        .iter()
        .find(|(_, p)| p.device.as_uuid() == device)
        .map(|(id, _)| id.as_uuid())
        .unwrap_or_else(|| panic!("{tag}'s placement"))
}

#[test]
fn a_rails_rows_name_their_commands_and_read_the_free_length() {
    let s = common::sample();
    let r1 = carrier(&s, CarrierKind::Rail, 0);
    let (x, y, length) = rail_mm(&s, r1);
    let got: Vec<(String, String, Option<String>)> = rows(&s, r1)
        .into_iter()
        .filter(|r| r.0 != "carrier/derived.part" && r.0 != "carrier/derived.free_length")
        .collect();
    let edit = Some("layout.carrier.edit".to_owned());
    assert_eq!(
        got,
        [
            ("carrier/derived.profile".to_owned(), "TS 35×7.5".to_owned(), None),
            (
                "carrier/derived.start".to_owned(),
                format!("{x}, {y} mm"),
                edit.clone()
            ),
            (
                "carrier/derived.length".to_owned(),
                format!("{length} mm"),
                edit.clone()
            ),
            ("carrier/derived.pe_bond".to_owned(), "false".to_owned(), edit),
        ]
    );
    // The free length: what the placed devices leave of the rail, read only.
    let free = rows(&s, r1)
        .into_iter()
        .find(|r| r.0 == "carrier/derived.free_length")
        .expect("the free length");
    assert_eq!(free.2, None);
    assert!((0.0..length as f64).contains(&mm_of(&free.1)), "{}", free.1);
    let r2 = carrier(&s, CarrierKind::Rail, 1);
    assert!(mm_of(&text_of(&s, r2, "carrier/derived.free_length")) >= 0.0);
}

#[test]
fn length_lengthens_the_rail_and_a_bad_text_is_refused_in_words() {
    let mut s = common::sample();
    let r1 = carrier(&s, CarrierKind::Rail, 0);
    let (x, y, length) = rail_mm(&s, r1);
    let free = mm_of(&text_of(&s, r1, "carrier/derived.free_length"));
    let longer = length + 130;
    let cmd = edit(&s, "carrier", r1, "length", &longer.to_string()).unwrap();
    assert_eq!(cmd.name, "layout.carrier.edit");
    assert_eq!(
        cmd.params,
        json!({"carrier": r1.to_string(),
            "path": [[x * 1_000_000, y * 1_000_000], [(x + longer) * 1_000_000, y * 1_000_000]]})
    );
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, r1, "carrier/derived.length"), format!("{longer} mm"));
    assert_eq!(
        mm_of(&text_of(&s, r1, "carrier/derived.free_length")),
        free + 130.0,
        "the 130 mm more are free"
    );
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
    let mut s = common::sample();
    let f1 = placement_of(&s, "-102F1");
    let along = s
        .query()
        .table::<Placement>()
        .get(Id::from_uuid(f1))
        .unwrap()
        .pose
        .t[0]
        .0
        / 1_000_000;
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
    // Auto-fill puts the schematic's first devices, 101's and 102's, on rail 1.
    assert_eq!(text_of(&s, f1, "placement/derived.carrier"), "Rail 1");
    assert_eq!(
        text_of(&s, f1, "placement/derived.position"),
        format!("{along} mm")
    );
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
    let mut s = common::sample();
    let (space, enclosure, surface) = (space(&s), enclosure(&s), plate(&s));
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
    let bond = edit(&s, "mounting_surface", surface, "pe_bond", "true").unwrap();
    assert_eq!(bond.name, "layout.surface.edit");
    run(&mut s, &bond);
    assert_eq!(text_of(&s, surface, "mounting_surface/derived.pe_bond"), "true");
}

#[test]
fn the_panel_pages_surface_view_rows_set_the_view() {
    let mut s = common::sample();
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
            ("page/derived.labels", set.clone()),
            ("page/derived.datum", set),
        ]
    );
    assert_eq!(view[0].1, "-UC1 plate");
    assert_eq!(view[1].1, "Abbreviated");
    assert_eq!(
        view[2].1, "Tags or numbers",
        "the sample's 902, made by Enclosure…, prints its tags where they fit and balloons where they do not"
    );
    // The rows sit under their own category, Surface view.
    let categories: Vec<ew_api::output::Category> = s
        .property_sheet(&[page])
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .filter(|r| r.key.starts_with("page/derived.") && view.iter().any(|v| v.0 == r.key))
        .map(|r| r.category)
        .collect();
    assert_eq!(categories, [ew_api::output::Category::SurfaceView; 4]);
    let tags = edit(&s, "page", page, "labels", "Tags").unwrap();
    run(&mut s, &tags);
    let full = edit(&s, "page", page, "label_mode", "Full").unwrap();
    assert_eq!(full.name, "page.surface_view.set");
    run(&mut s, &full);
    assert_eq!(text_of(&s, page, "page/derived.label_mode"), "Full");
    let t = s.panel_tables("902").expect("902's panel");
    assert!(
        t.placements.iter().any(|p| p.label == "+CAB-102F1"),
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
    let plate = plate(&s);
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

/// The project's enclosure part copy (EW-GEN-ENC-1).
fn enclosure_part(s: &Session) -> Uuid {
    s.layout_part_offers("enclosure")
        .into_iter()
        .find(|o| o.number == "EW-GEN-ENC-1" && o.copies.is_empty())
        .expect("the project's enclosure part")
        .part
}

/// The lamp -102E1 taken off its rail and placed on -UC1's plate at (300, 400) mm; the placement's ID.
fn lamp_on_the_plate(s: &mut Session) -> Uuid {
    let lamp = s.resolve_reference("-102E1").expect("the lamp");
    let on_rail = placement_of(s, "-102E1");
    run(
        s,
        &AnyCommand {
            name: "layout.remove".into(),
            params: json!({"placement": on_rail.to_string()}),
        },
    );
    let plate = plate(s);
    run(
        s,
        &AnyCommand {
            name: "layout.place".into(),
            params: json!({"device": lamp.to_string(), "carrier": {"surface": plate.to_string()},
                "pose": {"t": [300_000_000, 400_000_000, 0], "q": [1_000_000_000, 0, 0, 0]}}),
        },
    );
    placement_of(s, "-102E1")
}

/// The ID of the sample's location node `code`.
fn location(s: &Session, code: &str) -> Uuid {
    s.query()
        .structure_nodes()
        .iter()
        .find(|(_, n)| n.code.as_str() == code)
        .map(|(id, _)| id.as_uuid())
        .unwrap_or_else(|| panic!("the location {code}"))
}

/// A second enclosure, at +FLD, from the project's enclosure part, its tag frozen as -UC1 (as -UC1 of +CAB is);
/// its surface's ID.
fn field_enclosure(s: &mut Session) -> Uuid {
    let field = location(s, "FLD");
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
    let mut s = common::sample();
    let page = page_of(&s, "902");
    let first = plate(&s);
    let second = field_enclosure(&mut s);
    let choices = s.surface_choices();
    assert_eq!(
        choices,
        [
            (first, "+CAB-UC1 plate".to_owned()),
            (second, "+FLD-UC1 plate".to_owned()),
        ]
    );
    assert_eq!(
        options_of(&s, page, "page/derived.surface"),
        ["+CAB-UC1 plate", "+FLD-UC1 plate"]
    );
    assert_eq!(text_of(&s, page, "page/derived.surface"), "+CAB-UC1 plate");
    // The choice by its value: the second surface, though the words differ from the shown ones only in the location.
    let cmd = edit(&s, "page", page, "surface", &second.to_string()).unwrap();
    assert_eq!(cmd.params["surface"], json!(second.to_string()));
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, page, "page/derived.surface"), "+FLD-UC1 plate");
    // Typed words name one surface.
    let cmd = edit(&s, "page", page, "surface", "+CAB-UC1 plate").unwrap();
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
    let twin = Uuid::from_u128(0x7717);
    let (project, mut records, lib) = common::sample_records();
    let copy = records
        .iter()
        .find_map(|r| match r {
            ew_schema::Record::MountingSurface(m) => Some(m.clone()),
            _ => None,
        })
        .expect("-UC1's plate");
    let plate = copy.id.as_uuid();
    records.push(ew_schema::Record::MountingSurface(MountingSurface {
        id: Id::from_uuid(twin),
        ..copy
    }));
    let s = common::session_of(project, records, lib);
    assert_eq!(
        s.surface_choices(),
        [
            (plate, "+CAB-UC1 plate (1)".to_owned()),
            (twin, "+CAB-UC1 plate (2)".to_owned()),
        ]
    );
    assert_eq!(
        text_of(&s, page_of(&s, "902"), "page/derived.surface"),
        "+CAB-UC1 plate (1)"
    );
    assert_eq!(s.property_sheet(&[twin]).header.title, "+CAB-UC1 plate (2)");
}

/// Device is a link (its tag the words); Realised part offers only the part uses `layout.move {part}`
/// takes (an accessory mounted on its parent is not one); Label is read from the page that shows the placement's
/// surface, and is absent when none does.
#[test]
fn a_placements_device_is_a_link_its_parts_mount_and_its_label_comes_from_the_showing_page() {
    let mut s = with_accessory();
    let f1 = placement_of(&s, "-102F1");
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
    // An accessory part use (ACC-1, mounted on its parent) is not offered.
    let parts = s.query().devices().get(placement.device).unwrap().parts.clone();
    let mut uses = serde_json::to_value(&parts).unwrap();
    uses.as_array_mut()
        .unwrap()
        .push(json!({"part": {"copy": accessory_copy().to_string()}, "qty": "1"}));
    run(
        &mut s,
        &AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "device", "targets": [placement.device.as_uuid().to_string()],
                "path": "parts", "value": uses}),
        },
    );
    assert_eq!(options_of(&s, f1, "placement/derived.realised_part"), offered);
    assert!(!offered.contains(&"ACC-1".to_owned()), "{offered:?}");
    // The label comes from 902, which shows -UC1's plate; with 902 showing another plate no page shows it. 902
    // numbers its devices, so the label is -102F1's item number in the legend.
    assert!(!text_of(&s, f1, "placement/derived.label").is_empty());
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
    let mut s = common::sample();
    let f1 = placement_of(&s, "-102F1");
    let r1 = carrier(&s, CarrierKind::Rail, 0);
    let enclosure = enclosure(&s);
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
    let plate = lamp_on_the_plate(&mut s);
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
    let mut s = common::sample();
    let f1 = placement_of(&s, "-102F1");
    // Rail 1 is full: 100 mm more of it, and the breaker moves there.
    let r1 = carrier(&s, CarrierKind::Rail, 0);
    let (_, _, length) = rail_mm(&s, r1);
    let longer = edit(&s, "carrier", r1, "length", &(length + 100).to_string()).unwrap();
    run(&mut s, &longer);
    let to = length + 20;
    let cmd = edit(&s, "placement", f1, "position", &to.to_string()).unwrap();

    assert_eq!(cmd.name, "layout.move");
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, f1, "placement/derived.position"), format!("{to} mm"));

    // Realised part: -102F1's device gains -101F1's part (EW-GEN-MCB-4) as a second use, and the row commits it in
    // place of EW-GEN-MCB-1.
    assert_eq!(text_of(&s, f1, "placement/derived.realised_part"), "EW-GEN-MCB-1");
    let other = {
        let q = s.query();
        q.table::<ew_schema::LibCopy<ew_schema::Part>>()
            .iter()
            .find(|(_, c)| c.item.part_number.as_str() == "EW-GEN-MCB-4")
            .map(|(id, _)| id)
            .expect("EW-GEN-MCB-4's copy")
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
        ["EW-GEN-MCB-1", "EW-GEN-MCB-4"]
    );
    let cmd = edit(&s, "placement", f1, "realised_part", "EW-GEN-MCB-4").unwrap();
    assert_eq!(cmd.name, "layout.move");
    assert_eq!(cmd.params["part"], json!(other.as_uuid().to_string()));
    run(&mut s, &cmd);
    assert_eq!(text_of(&s, f1, "placement/derived.realised_part"), "EW-GEN-MCB-4");
    let plate = lamp_on_the_plate(&mut s);
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
    let d1 = carrier(&s, CarrierKind::Duct, 0);
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

/// A library document of the sample (a part by number, or the manufacturer of one) moved into the library `lib`, with
/// its references (manufacturer, enclosure plate) into `lib` too (the registry recomputes the copy ID and hash).
fn sample_doc_in(lib: Uuid, pick: impl Fn(&ew_schema::LibDocument) -> bool) -> ew_schema::LibDocument {
    let (_, _, docs) = common::sample_records();
    let doc = docs.iter().find(|d| pick(d)).expect("a document of the sample");
    let mut v = serde_json::to_value(doc).unwrap();
    v["library_id"] = json!(lib.to_string());
    if v.get("manufacturer").is_some() {
        v["manufacturer"]["library"] = json!(lib.to_string());
        v["manufacturer"]["version_req"] = json!("*");
    }
    if v["sections"]["enclosure"].get("plate").is_some() {
        v["sections"]["enclosure"]["plate"]["library"] = json!(lib.to_string());
        v["sections"]["enclosure"]["plate"]["version_req"] = json!("*");
    }
    ew_schema::LibDocument::from_value(v).expect("a library document")
}

fn part_named(number: &'static str) -> impl Fn(&ew_schema::LibDocument) -> bool {
    move |d| matches!(d, ew_schema::LibDocument::Part(p) if p.item.part_number.as_str() == number)
}

/// The library path: a system library holding an enclosure part and its plate part (the sample's own, moved into
/// it); the offer of `layout_part_offers` carries both through `copies` (the plate as the enclosure section's
/// closure), beside the project's held copy, which brings none; and `layout.enclosure.create_from_part` with them
/// creates +FLD's enclosure with its plate in one command.
#[test]
fn a_library_enclosure_part_is_offered_with_its_plate_and_creates_the_enclosure() {
    use ew_api::SessionOptions;
    use ew_api::library::{LibraryTiers, Registry};
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
        let docs = vec![
            sample_doc_in(lib, part_named("EW-GEN-ENC-1")),
            sample_doc_in(lib, part_named("EW-GEN-PLT-1")),
            sample_doc_in(lib, |d| matches!(d, ew_schema::LibDocument::Manufacturer(_))),
        ];
        reg.write(lib, &docs).unwrap();
    }
    let (project, records, lib) = common::sample_records();
    let options = SessionOptions {
        libraries: Some(tiers(vec![dir])),
        ..SessionOptions::default()
    };
    let mut s = Session::from_records(project, records, lib, options).unwrap();
    let offers = s.layout_part_offers("enclosure");
    let numbers: Vec<(&str, usize)> = offers
        .iter()
        .map(|o| (o.number.as_str(), o.copies.len()))
        .collect();
    // The project's held copy brings no copies; the library's, with its copies, is offered beside it.
    assert!(
        offers
            .iter()
            .any(|o| o.number == "EW-GEN-ENC-1" && o.copies.is_empty()),
        "the held enclosure: {numbers:?}"
    );
    let library = offers
        .iter()
        .find(|o| o.number == "EW-GEN-ENC-1" && !o.copies.is_empty())
        .unwrap_or_else(|| panic!("the library's enclosure: {numbers:?}"))
        .clone();
    let embedded: Vec<String> = library
        .copies
        .iter()
        .filter_map(|c| c["part_number"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(
        embedded,
        ["EW-GEN-ENC-1", "EW-GEN-PLT-1"],
        "the part and its plate"
    );
    let field = location(&s, "FLD");
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
/// and the datum centred at that scale, in one command; the page then keeps it. 902's own 1:5 (the scale
/// Enclosure… chose for the plate) is kept.
#[test]
fn a_new_panel_page_takes_the_fitting_scale_with_its_surface() {
    let mut s = common::sample();
    let p902 = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == "902")
        .expect("902")
        .id;
    let plate = plate(&s);
    assert_eq!(scale_of(&s, p902), ew_schema::Scale::OneTo(5));
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
    assert_eq!(n, 5, "-UC1's plate on the page frame's drawing area, as on 902");
    let datum = s.centred_surface_datum(new, plate).expect("a centred datum");
    let cmd = edit(&s, "page", new, "surface", "-UC1 plate").unwrap();
    assert_eq!(cmd.name, "page.surface_view.set");
    assert_eq!(
        cmd.params,
        json!({"page": new.to_string(), "surface": plate.to_string(), "datum_at": [datum.0, datum.1],
            "label_mode": "abbreviated", "scale": {"one_to": n}, "labels": "tags_or_numbers"}),
        "a new view prints a tag where it fits and a balloon where it does not"
    );
    run(&mut s, &cmd);
    assert_eq!(scale_of(&s, new), ew_schema::Scale::OneTo(n));
    // The page now has its scale: a later edit sends none.
    let again = edit(&s, "page", new, "label_mode", "Full").unwrap();
    assert_eq!(again.params.get("scale"), None);
    let again = edit(&s, "page", new, "surface", "-UC1 plate").unwrap();
    assert_eq!(again.params.get("scale"), None);
}

/// Fit the layout to the sheet: an older layout page, its plate small in a corner, takes the fitting scale
/// and the centred datum in one command that keeps its labels and moves no stored pose; a fitted page offers nothing.
#[test]
fn fit_the_layout_to_the_sheet_changes_the_scale_and_the_datum_only() {
    let mut s = common::sample();
    let p902 = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == "902")
        .expect("902")
        .id;
    let plate = plate(&s);
    let n = s.fitting_scale(p902, plate).expect("the plate fits a scale");
    let older = if n == 10 { 20 } else { 10 };
    run(
        &mut s,
        &AnyCommand {
            name: "page.surface_view.set".into(),
            params: json!({"page": p902.to_string(), "surface": plate.to_string(),
                "datum_at": [20_000_000, 150_000_000], "label_mode": "abbreviated", "scale": {"one_to": older}}),
        },
    );
    let poses = |s: &Session| -> Vec<String> {
        let q = s.query();
        q.table::<Placement>()
            .iter()
            .map(|(_, p)| format!("{p:?}"))
            .collect()
    };
    let before = poses(&s);
    let cmd = s.sheet_fit(p902).expect("an older layout is fitted");
    assert_eq!(cmd.name, "page.surface_view.set");
    assert_eq!(cmd.params["scale"], json!({"one_to": n}));
    assert_eq!(cmd.params.get("labels"), None, "the view keeps its labels");
    run(&mut s, &cmd);
    assert_eq!(scale_of(&s, p902), ew_schema::Scale::OneTo(n));
    let centred = s.centred_surface_datum(p902, plate).expect("a centred datum");
    assert_eq!(cmd.params["datum_at"], json!([centred.0, centred.1]));
    assert_eq!(poses(&s), before, "no stored pose moves");
    assert!(s.sheet_fit(p902).is_none(), "a fitted page has nothing to fit");
}
