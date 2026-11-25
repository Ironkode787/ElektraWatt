// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid below the shell (acceptance checks PG1, PG2 and the `ew-api` half of PG8): the
//! field catalogue of every kind, the routes, the property sheet of the sample's records with its derived rows and
//! chips, and the editors' parse and display rules on the project's data.

use std::path::Path;

use ew_api::output::{
    Category, EditRoute, FieldReadOnly, FieldType, InputError, PropertySheet, Provenance, RowAction,
    RowValue, field_catalogue, field_catalogue_for, format_field_value,
};
use ew_api::{AnyCommand, CommandRegistry, Exposure, Session};
use ew_commands::commands::{SETTABLE, SETTABLE_SETTINGS};
use serde_json::{Value, json};

/// The kinds the catalogue covers: the implemented kinds that are not library documents, in kind-rank order.
fn catalogue_kinds() -> impl Iterator<Item = ew_schema::Kind> {
    ew_schema::Kind::ALL
        .into_iter()
        .filter(|k| k.status() == ew_schema::KindStatus::Implemented && !k.is_library())
}

/// The catalogue text: one line per node, `<kind> <path> <type> <category> <route>`.
fn catalogue_text() -> String {
    let mut out = String::new();
    for k in catalogue_kinds() {
        let catalogue = field_catalogue(k.name()).unwrap();
        for f in catalogue.iter() {
            let id = if f.identifier { " identifier" } else { "" };
            let header = f.ty.is_container()
                && f.route == EditRoute::ReadOnly(FieldReadOnly::NoCommand)
                && catalogue.iter().any(|g| {
                    g.route.is_editable()
                        && g.path.len() > f.path.len()
                        && g.path.starts_with(&f.path)
                        && matches!(g.path.as_bytes()[f.path.len()], b'.' | b'[')
                });
            let route = if header { "-".to_owned() } else { f.route.word() };
            out.push_str(&format!(
                "{} {} {} {} {route}{id}\n",
                f.kind,
                f.path,
                f.ty.word(),
                f.category.word(),
            ));
        }
    }
    out
}

/// The catalogue is built for every kind it covers, each node with its line. The lines are not compared with a kept
/// copy: a kept list of every field fails with every field a change adds and says nothing about the change; what a
/// field's route must be is tested where the route is used (the tests below).
#[test]
fn every_kind_has_its_catalogue() {
    let text = catalogue_text();
    for k in catalogue_kinds() {
        assert!(
            text.lines().any(|l| l.starts_with(&format!("{} ", k.name()))),
            "{} has no line",
            k.name()
        );
    }
    // A reserved, a library and an unknown kind have none.
    assert!(field_catalogue("comment").is_none());
    assert!(field_catalogue("part").is_none());
    assert!(field_catalogue("macro").is_none());
    assert!(field_catalogue("manufacturer").is_none());
    assert!(field_catalogue("no_such_kind").is_none());
    // An implemented kind has one: `waiver` (its members are read-only, the commands
    // check.waive and check.unwaive write it), `logical_net` and `physical_net`. The set
    // grows, so its total is not pinned (CONTRIBUTING.md).
    for kind in ["waiver", "logical_net", "physical_net"] {
        assert!(
            catalogue_kinds().any(|k| k.name() == kind),
            "{kind} has a catalogue"
        );
    }
}

#[test]
fn every_settable_path_maps_to_a_node_with_its_own_route() {
    for (kind, paths) in SETTABLE {
        let name = kind.kind().name();
        let catalogue = field_catalogue(name).unwrap_or_else(|| panic!("{name} has a catalogue"));
        for path in *paths {
            let node = catalogue
                .iter()
                .find(|f| f.path == *path)
                .unwrap_or_else(|| panic!("{name} {path}: no catalogue node"));
            // A settable path the grid shows read-only: narrowed, or set by the save (the edited date).
            let narrowed = matches!(
                node.route,
                EditRoute::ReadOnly(FieldReadOnly::Narrowed { .. } | FieldReadOnly::OnSave)
            );
            assert!(
                narrowed
                    || node.route
                        == EditRoute::FieldsSet {
                            kind: name.to_owned(),
                            path: (*path).to_owned()
                        },
                "{name} {path}: {:?}",
                node.route
            );
        }
    }
    let project = field_catalogue("project").unwrap();
    for s in SETTABLE_SETTINGS {
        let node = project
            .iter()
            .find(|f| f.path == format!("settings.{}", s.name))
            .unwrap();
        assert_eq!(
            node.route,
            EditRoute::SettingsMember {
                member: s.name.to_owned()
            }
        );
        // a member of the JSON shape, or a text member (`process_tag_scheme`)
        assert!(
            matches!(&node.ty, FieldType::Json)
                || matches!(&node.ty, FieldType::Optional(t) if **t == FieldType::Text),
            "{}: {:?}",
            s.name,
            node.ty
        );
    }
    // Each member `project.supply.set` takes is its row's route (FT-1928).
    for path in ew_commands::commands::SETTABLE_SUPPLY {
        let node = project
            .iter()
            .find(|f| f.path == format!("supply.{path}"))
            .unwrap_or_else(|| panic!("supply.{path}: no catalogue node"));
        assert_eq!(
            node.route,
            EditRoute::SupplyMember {
                path: (*path).to_owned()
            }
        );
    }
    let checks = project.iter().find(|f| f.path == "settings.checks").unwrap();
    assert!(matches!(
        &checks.route,
        EditRoute::ReadOnly(FieldReadOnly::HumanOnly { command, .. }) if command == "project.checks.set"
    ));
    let revision = field_catalogue("revision").unwrap();
    for path in ["author", "checker"] {
        let node = revision.iter().find(|f| f.path == path).unwrap();
        assert!(
            matches!(&node.route, EditRoute::ReadOnly(FieldReadOnly::Narrowed { why }) if why.contains("four-eyes")),
            "{path}: {:?}",
            node.route
        );
    }
    let device = field_catalogue("device").unwrap();
    let id = device.iter().find(|f| f.path == "id").unwrap();
    assert_eq!(id.route, EditRoute::ReadOnly(FieldReadOnly::Identity));
    let tag = device.iter().find(|f| f.path == "tag.counter").unwrap();
    assert!(tag.identifier && tag.category == Category::Identity);
    let qty = device.iter().find(|f| f.path == "parts[].qty").unwrap();
    assert_eq!(
        qty.route,
        EditRoute::FieldsSet {
            kind: "device".into(),
            path: "parts".into()
        }
    );
    assert_eq!(qty.ty, FieldType::Dec3);
}

/// PG1's registry half: the catalogue is a function of the registry. Over an empty registry no route is editable and
/// every command route is "no command"; the `HumanOnly` half (a registry in which `fields.set` is `HumanOnly`) is the
/// unit test `a_human_only_fields_set_makes_every_row_read_only` of `output::fields`, which needs no command type.
#[test]
fn the_catalogue_follows_the_registry() {
    let empty = CommandRegistry::new();
    for k in catalogue_kinds() {
        let built = field_catalogue(k.name()).unwrap();
        let bare = field_catalogue_for(&empty, k.name()).unwrap();
        assert_eq!(built.len(), bare.len());
        for (b, h) in built.iter().zip(bare.iter()) {
            assert_eq!(b.path, h.path);
            assert_eq!(b.ty, h.ty);
            match &b.route {
                EditRoute::FieldsSet { .. }
                | EditRoute::SettingsMember { .. }
                | EditRoute::SupplyMember { .. }
                | EditRoute::ReadOnly(FieldReadOnly::Command { .. } | FieldReadOnly::HumanOnly { .. }) => {
                    assert_eq!(
                        h.route,
                        EditRoute::ReadOnly(FieldReadOnly::NoCommand),
                        "{} {}",
                        h.kind,
                        h.path
                    );
                }
                other => assert_eq!(&h.route, other, "{} {}", h.kind, h.path),
            }
        }
    }
    let built = field_catalogue_for(CommandRegistry::builtin(), "device").unwrap();
    assert_eq!(&*built, &*field_catalogue("device").unwrap());
}

// ---------------------------------------------------------------------------------------------------------------
// PG2: the sheet on the sample

/// The UUID of the device whose tag prints `tag`.
fn device(s: &Session, tag: &str) -> ew_ids::Uuid {
    s.resolve_reference(&format!("tag:{tag}")).unwrap()
}

fn row<'s>(sheet: &'s PropertySheet, key: &str) -> &'s ew_api::output::PropRow {
    sheet
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .find(|r| r.key == key)
        .unwrap_or_else(|| {
            let keys: Vec<&str> = sheet
                .sections
                .iter()
                .flat_map(|s| &s.rows)
                .map(|r| r.key.as_str())
                .collect();
            panic!("no row {key}: {keys:#?}")
        })
}

fn one(r: &ew_api::output::PropRow) -> &str {
    match &r.value {
        RowValue::One { text, .. } => text,
        other => panic!("{}: {other:?}", r.key),
    }
}

/// A representation of a device (its main one).
fn main_rep(s: &Session, device: ew_ids::Uuid) -> ew_ids::Uuid {
    let q = s.query();
    let id = ew_ids::Id::from_uuid(device);
    q.get::<ew_derive::MainRep>(id).unwrap().as_uuid()
}

/// The wire numbered `number` (`102.1`: from the breaker -102F1 to the strip -X1021, 24 V).
fn wire(s: &Session, number: &str) -> ew_ids::Uuid {
    s.navigator_table(ew_api::output::NavKind::Connections)
        .nodes
        .iter()
        .find(|n| n.table_row && n.label == number)
        .map(|n| n.records[0])
        .unwrap_or_else(|| panic!("no wire {number}"))
}

/// The drawn page labelled `folio`.
fn page(s: &Session, folio: &str) -> ew_ids::Uuid {
    s.resolve_reference(&format!("folio:{folio}")).unwrap()
}

/// A record's own bookkeeping (`$k`, `$v`) is no row of Properties (FT-1939: a breaker showed a row `$v`).
#[test]
fn a_records_bookkeeping_members_are_no_rows() {
    for k in catalogue_kinds() {
        let catalogue = field_catalogue(k.name()).unwrap();
        let own: Vec<&str> = catalogue
            .iter()
            .map(|f| f.path.as_str())
            .filter(|p| p.starts_with('$'))
            .collect();
        assert!(own.is_empty(), "{}: {own:?}", k.name());
    }
    let s = crate::common::sample();
    let sheet = s.property_sheet(&[main_rep(&s, device(&s, "-101F1"))]);
    let keys: Vec<&str> = sheet
        .sections
        .iter()
        .flat_map(|x| &x.rows)
        .map(|r| r.key.as_str())
        .collect();
    assert!(keys.iter().all(|k| !k.starts_with('$')), "{keys:?}");
}

#[test]
fn a_representation_reaches_three_sections_with_derived_rows() {
    let s = crate::common::sample();
    let f1 = device(&s, "-101F1");
    let rep = main_rep(&s, f1);
    let sheet = s.property_sheet(&[rep]);
    let kinds: Vec<&str> = sheet.sections.iter().map(|x| x.kind.as_str()).collect();
    assert_eq!(kinds, ["device", "function", "representation"]);
    assert_eq!(sheet.header.title, "-101F1");
    assert_eq!(sheet.header.items, 1);
    assert_eq!(sheet.header.kinds, [("representation".to_owned(), 1)]);
    assert!(
        sheet
            .header
            .location
            .as_deref()
            .is_some_and(|l| l.starts_with("101-")),
        "{:?}",
        sheet.header
    );
    let tag = row(&sheet, "device/derived.tag");
    assert_eq!(one(tag), "-101F1");
    assert!(
        matches!(&tag.provenance, Provenance::Derived { via } if via.starts_with("numbering scheme '")),
        "{:?}",
        tag.provenance
    );
    // One device's Tag field is typed into.
    assert_eq!(tag.actions, [RowAction::Edit, RowAction::Freeze]);
    let part = row(&sheet, "device/parts[0].part");
    assert!(
        matches!(part.provenance, Provenance::Library { .. }),
        "{:?}",
        part.provenance
    );
    assert!(one(part).contains(" ("), "{}", one(part));
    assert_eq!(sheet.missing, 0);
    // Derived rows first in their category.
    let device = &sheet.sections[0];
    let first_identity = device
        .rows
        .iter()
        .find(|r| r.category == Category::Identity)
        .unwrap();
    assert_eq!(first_identity.key, "device/derived.tag");
}

#[test]
fn frozen_tags_wire_chips_and_the_page_description() {
    let mut s = crate::common::sample();
    // The enclosure's tag frozen as it prints (the sample freezes none).
    let uc1 = device(&s, "-UC1");
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [uc1.to_string()], "path": "tag.frozen",
            "value": {"text": "UC1"}}),
    })
    .expect("the frozen tag");
    let sheet = s.property_sheet(&[uc1]);
    let tag = row(&sheet, "device/derived.tag");
    assert_eq!(
        tag.provenance,
        Provenance::Frozen {
            note: "frozen".into()
        }
    );
    assert_eq!(tag.actions, [RowAction::Edit, RowAction::Unfreeze]);
    // 102.1 (D15): the rating of the breaker that protects it (its control class's 0.75 mm² carries less than 16 A,
    // FT-2055) and the colour profile name the supplying rows.
    let w29 = wire(&s, "102.1");
    let sheet = s.property_sheet(&[w29]);
    let csa = row(&sheet, "connection/derived.csa");
    assert_eq!(one(csa), "2.5 mm²");
    assert_eq!(
        csa.provenance,
        Provenance::Derived {
            via: format!(
                "potential 24V/102F1:2 · protected by -102F1, rated 16 A · {}",
                ew_derive::current_table_basis()
            )
        }
    );
    assert_eq!(csa.actions, [RowAction::Override]);
    let colour = row(&sheet, "connection/derived.colour");
    assert_eq!(one(colour), "DBU");
    assert_eq!(
        colour.provenance,
        Provenance::Derived {
            via: "potential 24V/102F1:2 · colour profile 'DBU/WH 24 V'".into()
        }
    );
    // The shield wire with its cross-section overridden on the connection (CDP).
    let w20 = crate::common::sample_shield(&s).as_uuid();
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "connection", "targets": [w20.to_string()], "path": "overrides.csa", "value": "4"}),
    })
    .expect("the override");
    let sheet = s.property_sheet(&[w20]);
    let csa = row(&sheet, "connection/derived.csa");
    assert_eq!(
        csa.provenance,
        Provenance::Overridden {
            note: "override (CDP)".into()
        }
    );
    assert!(csa.actions.contains(&RowAction::Reset));
    // Page 101: the description is the project's.
    let p101 = page(&s, "101");
    let sheet = s.property_sheet(&[p101]);
    let d = row(&sheet, "page/derived.description");
    assert_eq!(one(d), "Conveyor drive with a pneumatic clamp");
    assert_eq!(
        d.provenance,
        Provenance::Inherited {
            from: "project description".into()
        }
    );
    assert_eq!(d.path, "description_override");
    // The sample is clean under the checks: page 101's header counts no finding.
    assert!(sheet.header.findings.is_empty(), "{:?}", sheet.header.findings);
}

#[test]
fn mixed_and_missing_selections() {
    let s = crate::common::sample();
    let f1 = device(&s, "-101F1");
    let rep = main_rep(&s, f1);
    let w29 = wire(&s, "102.1");
    let sheet = s.property_sheet(&[rep, w29]);
    assert!(
        sheet.sections.is_empty(),
        "a representation and a connection share no section"
    );
    assert_eq!(sheet.header.items, 2);
    let gone = ew_ids::Uuid::from_u128(0x0123_4567_89ab_7def_8123_4567_89ab_cdef);
    let sheet = s.property_sheet(&[gone]);
    assert_eq!(sheet.missing, 1);
    assert!(sheet.sections.is_empty());
    // Two representations of devices: the device section has both devices, deduplicated.
    let f2 = device(&s, "-102F1");
    let sheet = s.property_sheet(&[main_rep(&s, f2), rep]);
    assert_eq!(sheet.sections[0].kind, "device");
    assert_eq!(sheet.sections[0].targets.len(), 2);
    // In label order, whatever order the selection named them in.
    assert_eq!(sheet.sections[0].targets, [f1, f2]);
    assert_eq!(
        s.property_sheet(&[rep, main_rep(&s, f2)]).sections[0].targets,
        [f1, f2]
    );
    let parts = row(&sheet, "device/parts");
    assert!(
        matches!(parts.value, RowValue::Varies { count: 2, .. }),
        "{:?}",
        parts.value
    );
    assert_eq!(parts.provenance, Provenance::Varies { count: 2 });
    assert_eq!(sheet.header.title, "2 devices");
}

/// An interruption point's Properties header names its kind in the navigators' words, not
/// the raw kind word "representation".
#[test]
fn an_interruption_points_header_names_its_kind_in_words() {
    let s = crate::common::sample();
    let tables = s.page_tables("102").unwrap();
    let ip = tables
        .reps
        .iter()
        .find(|r| r.ip.is_some() && r.device.is_none())
        .expect("an interruption point on 102");
    let sheet = s.property_sheet(&[ip.id]);
    assert_eq!(sheet.header.title, "Interruption point");
    assert_eq!(sheet.header.kinds, [("representation".to_owned(), 1)]);
}

/// A section lists its targets in label order (devices by tag, representations by their
/// first CP's label, then place), never by ID alone, and a `‹varies›` value lists its texts in natural text order.
#[test]
fn targets_and_varied_texts_are_in_label_order() {
    let s = crate::common::sample();
    let tables = s.page_tables("101").unwrap();
    let mut devices: Vec<ew_ids::Uuid> = tables.reps.iter().filter_map(|r| r.device).collect();
    devices.dedup();
    let sheet = s.property_sheet(&devices);
    let section = sheet.sections.iter().find(|x| x.kind == "device").unwrap();
    assert!(section.targets.len() > 5, "{}", section.targets.len());
    let natural = |a: &str, b: &str| ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b));
    let titles: Vec<String> = section
        .targets
        .iter()
        .map(|t| s.property_sheet(&[*t]).header.title)
        .collect();
    assert!(
        titles.windows(2).all(|w| natural(&w[0], &w[1]).is_lt()),
        "{titles:?}"
    );
    let mut by_id = section.targets.clone();
    by_id.sort();
    assert_ne!(by_id, section.targets, "Golden A's IDs are in no visible order");
    // The derived tag varies; its texts (the first eight) in natural order.
    match &row(&sheet, "device/derived.tag").value {
        RowValue::Varies { texts, .. } => {
            assert!(texts.len() > 1);
            assert!(
                texts.windows(2).all(|w| natural(&w[0], &w[1]).is_lt()),
                "{texts:?}"
            );
        }
        other => panic!("{other:?}"),
    }
    // The representations of the folio's devices: by the label of their first CP, then by place.
    let reps: Vec<ew_ids::Uuid> = tables
        .reps
        .iter()
        .filter(|r| r.device.is_some())
        .map(|r| r.id)
        .collect();
    let sheet = s.property_sheet(&reps);
    let section = sheet
        .sections
        .iter()
        .find(|x| x.kind == "representation")
        .unwrap();
    let key = |id: &ew_ids::Uuid| {
        let r = tables.reps.iter().find(|r| r.id == *id).unwrap();
        let label = tables
            .cps
            .iter()
            .filter(|c| c.function == r.functions[0])
            .min_by_key(|c| c.idx)
            .map(|c| c.label.clone())
            .unwrap_or_default();
        (label, (r.pos.1, r.pos.0))
    };
    assert!(
        section.targets.windows(2).all(|w| {
            let (a, b) = (key(&w[0]), key(&w[1]));
            natural(&a.0, &b.0).then_with(|| a.1.cmp(&b.1)).is_le()
        }),
        "{:?}",
        section.targets.iter().map(key).collect::<Vec<_>>()
    );
}

#[test]
fn the_project_has_its_settings_rows_and_d06_shows_its_finding() {
    let mut s = crate::common::sample();
    let project = s.query().project().id.as_uuid();
    let sheet = s.property_sheet(&[project]);
    for member in [
        "numbering",
        "colour_profile",
        "size_profile",
        "xref_format",
        "checks",
    ] {
        let r = row(&sheet, &format!("project/settings.{member}"));
        assert_eq!(r.ty, FieldType::Json);
        if member == "xref_format" || member == "checks" {
            assert_eq!(r.value, RowValue::Absent, "{member}");
        }
    }
    let checks = row(&sheet, "project/settings.checks");
    assert!(checks.actions.is_empty());
    assert!(matches!(
        checks.route,
        EditRoute::ReadOnly(FieldReadOnly::HumanOnly { .. })
    ));
    // D-06: an override of page 101's description is shown and reported at info.
    let p101 = page(&s, "101");
    let cmd = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "page", "targets": [p101.to_string()], "path": "description_override",
            "value": "Control Cabinet Expansion"}),
    };
    s.execute(&cmd).unwrap();
    let sheet = s.property_sheet(&[p101]);
    let d = row(&sheet, "page/derived.description");
    assert_eq!(one(d), "Control Cabinet Expansion");
    assert_eq!(
        d.provenance,
        Provenance::Overridden {
            note: "override · project description is 'Conveyor drive with a pneumatic clamp'".into()
        }
    );
    assert_eq!(d.actions, [RowAction::Override, RowAction::Reset]);
    assert!(
        sheet
            .header
            .findings
            .iter()
            .any(|f| f.rule == "EW-CORE-023" && f.severity == "info"),
        "{:?}",
        sheet.header.findings
    );
    s.undo(&ew_api::Origin::Gui).unwrap();
    let sheet = s.property_sheet(&[p101]);
    assert!(sheet.header.findings.iter().all(|f| f.rule != "EW-CORE-023"));
}

/// The Supply rows stand while a schematic project has no supply, the first value set making one (FT-1928); a basic or
/// macro project has no supply to set (FR-PRJ-026), so it shows no Supply rows that could only refuse.
#[test]
fn only_a_schematic_project_shows_the_rows_of_a_supply_it_lacks() {
    use ew_schema::ProjectKind;
    let shows_phases = |kind: ProjectKind| {
        let (mut project, _, lib) = crate::common::sample_records();
        project.kind = kind;
        project.supply = None;
        let s = crate::common::session_of(project, Vec::new(), lib);
        let id = s.query().project().id.as_uuid();
        s.property_sheet(&[id])
            .sections
            .iter()
            .flat_map(|x| &x.rows)
            .any(|r| r.key == "project/supply.phases")
    };
    assert!(shows_phases(ProjectKind::Schematic), "a schematic project's");
    for kind in [ProjectKind::Basic, ProjectKind::Macro] {
        assert!(!shows_phases(kind.clone()), "{kind:?}");
    }
}

/// The frozen tag is typed as the page prints it.
#[test]
fn the_frozen_tag_takes_the_printed_tag_as_text() {
    let s = crate::common::sample();
    let value = |text: &str| s.parse_field_input("device", "tag.frozen", text).unwrap().value;
    assert_eq!(value("S51"), json!({"text": "S51", "sign": "hidden"}));
    assert_eq!(value("-S51"), json!({"text": "S51"}));
    assert_eq!(value(" \"S51\" "), json!({"text": "S51", "sign": "hidden"}));
    assert_eq!(value(""), Value::Null);
    // The object form is parsed as before.
    assert_eq!(
        value(r#"{"text":"S51","sign":"hidden"}"#),
        json!({"text": "S51", "sign": "hidden"})
    );
    assert!(matches!(
        s.parse_field_input("device", "tag.frozen", "S\t51"),
        Err(InputError::Invalid(_))
    ));
}

/// FT-429: a cable's length is typed in metres or as its words, and shows as it is typed.
#[test]
fn the_cable_length_is_typed_in_metres_or_words() {
    let s = crate::common::sample();
    let value = |text: &str| s.parse_field_input("device", "cable.length", text).unwrap().value;
    assert_eq!(value("12.5 m"), json!({"manual": 12_500_000_000_i64}));
    assert_eq!(value("12.5m"), json!({"manual": 12_500_000_000_i64}));
    assert_eq!(value("3"), json!({"manual": 3_000_000_000_i64}));
    assert_eq!(value("0.000000001 m"), json!({"manual": 1}));
    assert_eq!(value("missing"), json!("missing"));
    assert_eq!(value("Length missing"), json!("missing"));
    assert_eq!(value("from part"), json!("from_part"));
    // The JSON form is parsed as before.
    assert_eq!(value(r#"{"manual": 7}"#), json!({"manual": 7}));
    for bad in ["1.2.3 m", "1.0000000001 m", ".5 m"] {
        assert!(
            matches!(
                s.parse_field_input("device", "cable.length", bad),
                Err(InputError::Invalid(_))
            ),
            "{bad}"
        );
    }
    // The value reads back in the words the editor takes.
    let ty = ew_api::output::field_catalogue("device")
        .unwrap()
        .iter()
        .find(|f| f.path == "cable.length")
        .unwrap()
        .ty
        .clone();
    let show = |v: Value| ew_api::output::format_field_value(&ty, &v);
    assert_eq!(show(json!({"manual": 12_500_000_000_i64})), "12.5 m");
    assert_eq!(show(json!({"manual": 3_000_000_000_i64})), "3 m");
    assert_eq!(show(json!("from_part")), "from part");
    assert_eq!(show(json!("missing")), "missing");
}

// ---------------------------------------------------------------------------------------------------------------
// PG8: the editors on the project's data

#[test]
fn editors_parse_and_format_on_the_sample() {
    let s = crate::common::sample();
    // Colours: the project's company code DBU, `BR` refused with the suggestion BN.
    assert_eq!(
        s.parse_field_input("connection", "overrides.colour", "dbu")
            .unwrap()
            .value,
        json!(["DBU"])
    );
    let Err(InputError::Invalid(m)) = s.parse_field_input("connection", "overrides.colour", "BR") else {
        panic!("BR is not a code");
    };
    let m = m.to_string();
    assert!(m.contains("BN brown"), "{m}");
    let Err(InputError::Ambiguous { choices }) = s.parse_field_input("connection", "overrides.csa", "1,500")
    else {
        panic!("ambiguous");
    };
    assert_eq!(choices.len(), 2);
    assert_eq!(
        s.parse_field_input("connection", "overrides.csa", "1.0")
            .unwrap()
            .value,
        json!("1")
    );
    // An Ident with a Cyrillic letter parses with the note.
    let p = s.parse_field_input("device", "process_tag", "F\u{410}1").unwrap();
    assert!(p.notes.iter().any(|n| n.contains("U+0410")), "{:?}", p.notes);
    // The part picker by part number; the part's choices in natural order.
    let p = s
        .parse_field_input("device", "parts[0].part", "EW-GEN-MCB-1")
        .unwrap();
    assert!(p.value.get("copy").is_some(), "{}", p.value);
    let choices = s.field_choices("device", "parts[0].part", "gen-mcb");
    assert!(!choices.is_empty() && choices.iter().all(|c| c.text.to_lowercase().contains("gen-mcb")));
    let colours = s.field_choices("connection", "overrides.colour", "");
    assert_eq!(colours.first().map(|c| c.text.as_str()), Some("BK"));
    assert_eq!(colours.last().map(|c| c.text.as_str()), Some("DBU"));
    // Enum words.
    assert_eq!(
        s.parse_field_input("device", "state", "Spare").unwrap().value,
        json!("spare")
    );
    assert_eq!(s.field_choices("device", "state", "").len(), 4);
    // A slot by its printed code path.
    let loc = s.field_choices("device", "structure.location", "");
    assert!(!loc.is_empty(), "the sample has location nodes");
    let first = &loc[0];
    assert_eq!(
        s.parse_field_input("device", "structure.location", &first.text)
            .unwrap()
            .value,
        first.value
    );
    assert_eq!(format_field_value(&FieldType::Bool, &Value::Bool(true)), "yes");
}

#[test]
fn a_counter_collision_names_the_next_free_counter() {
    let mut s = crate::common::sample();
    // -102S2 holds counter 2 of the push buttons on 102.
    let s1 = device(&s, "-102S1");
    let cmd = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [s1.to_string()], "path": "tag.counter", "value": 2}),
    };
    let err = s.execute(&cmd).unwrap_err();
    let text = format!("{err:?}");
    assert!(text.contains("next free counter"), "{text}");
}

#[test]
fn the_command_meta_and_the_reference_resolver() {
    let s = crate::common::sample();
    assert_eq!(s.command_meta("fields.set").unwrap().exposure, Exposure::Edit);
    assert_eq!(
        s.command_meta("project.checks.set").unwrap().exposure,
        Exposure::HumanOnly
    );
    assert_eq!(
        s.command_meta("project.settings.set").unwrap().cost,
        ew_api::Cost::Bulk
    );
    assert!(s.command_meta("no.such").is_none());
    let err = s.resolve_reference("tag:-999Z9").unwrap_err().to_string();
    assert!(err.contains("no device prints this tag"), "{err}");
    assert!(s.resolve_reference("folio:101").is_ok());
    let err = s.resolve_reference("bogus").unwrap_err().to_string();
    assert!(err.contains("a reference is a tag"), "{err}");
}

/// A bare tag resolves like `tag:<tag>` when one device prints it, a terminal as `TAG:pin` included; an
/// ambiguous one is refused with a sentence naming the candidates, never guessed.
#[test]
fn a_bare_tag_is_a_reference() {
    let s = crate::common::sample();
    assert_eq!(
        s.resolve_reference("-101F1").unwrap(),
        s.resolve_reference("tag:-101F1").unwrap()
    );
    assert_eq!(
        s.resolve_reference("-X1031:U").unwrap(),
        s.resolve_reference("tag:-X1031:U").unwrap()
    );
    let err = s.resolve_reference("-999Z9").unwrap_err().to_string();
    assert!(err.contains("no device prints this tag"), "{err}");
    // Two devices frozen to the same text print the same tag: refused, and the sentence names the candidates.
    let mut s = s;
    let ids = [device(&s, "-102F1"), device(&s, "-103F1")];
    for id in ids {
        let cmd = AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "device", "targets": [id.to_string()], "path": "tag.frozen",
                "value": {"text": "K9", "sign": "shown"}}),
        };
        s.execute(&cmd).unwrap();
    }
    let err = s.resolve_reference("-K9").unwrap_err().to_string();
    assert!(
        err.starts_with("`-K9` is ambiguous: 2 devices print this tag ("),
        "{err}"
    );
    for id in ids {
        assert!(err.contains(&format!("`id:{id}`")), "{err}");
    }
    assert_eq!(err.matches("+CAB-K9 as `id:").count(), 2, "{err}");
}

/// The page item and structure box placement members read their narrowing reasons (a page item's kind and a
/// marker's wires are replaced, not changed in place; an item stays on its page), while the settable members of a
/// kind keep their `fields.set` route.
#[test]
fn page_item_and_structure_box_members_read_their_reasons() {
    const KIND: &str = "a page item's kind is not changed in place; delete the item and place another";
    const GROUP: &str = "a marker's wires are changed by deleting the marker and placing another";
    const PAGE: &str = "stays on its page; delete it and place it on the other page";
    let route = |kind: &str, path: &str| {
        field_catalogue(kind)
            .unwrap()
            .iter()
            .find(|f| f.path == path)
            .unwrap_or_else(|| panic!("{kind} {path}"))
            .route
            .clone()
    };
    let why = |route: &EditRoute| match route {
        EditRoute::ReadOnly(FieldReadOnly::Narrowed { why }) => why.clone(),
        other => panic!("not narrowed: {other:?}"),
    };
    for path in [
        "kind",
        "kind.graphic",
        "kind.free_text",
        "kind.wire_marker",
        "kind.image",
        "kind.image.asset",
        "kind.image.rect",
    ] {
        assert_eq!(why(&route("page_item", path)), KIND, "{path}");
    }
    for path in [
        "kind.wire_marker.connections",
        "kind.wire_marker.connections[]",
        "kind.wire_marker.fields",
        "kind.wire_marker.fields[]",
    ] {
        assert_eq!(why(&route("page_item", path)), GROUP, "{path}");
    }
    assert_eq!(why(&route("page_item", "page")), PAGE);
    assert_eq!(why(&route("structure_box", "page")), PAGE);
    for path in ["kind.graphic.dash", "kind.free_text.text"] {
        assert_eq!(
            route("page_item", path),
            EditRoute::FieldsSet {
                kind: "page_item".into(),
                path: path.into()
            },
            "{path}"
        );
    }
}

/// Place through the command path and keep the actual primary ID, rather than a catalogue-only fixture.
fn placed(s: &mut Session, name: &str, params: Value) -> ew_ids::Uuid {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e}"))
    .outcome
    .primary
    .expect("the placed record")
    .1
    .as_uuid()
}

#[test]
fn page_items_and_structure_boxes_have_property_sheets() {
    let mut s = crate::common::sample();
    let page = &page(&s, "101").to_string();
    let w4 = wire(&s, "101.2").to_string();
    let text = placed(
        &mut s,
        "item.place",
        json!({"page": page,
        "kind": {"free_text": {"text": "40x60", "at": [100_000_000, 100_000_000]}}}),
    );
    let line = placed(
        &mut s,
        "item.place",
        json!({"page": page,
        "kind": {"graphic": {"shape": {"line": {"a": [100_000_000, 110_000_000], "b": [120_000_000, 110_000_000]}}}}}),
    );
    let marker = placed(
        &mut s,
        "marker.place",
        json!({"page": page,
        "connections": [w4], "at": [100_000_000, 120_000_000]}),
    );
    let box_id = placed(
        &mut s,
        "box.structure",
        json!({"page": page,
        "outline": [[100_000_000, 130_000_000], [120_000_000, 130_000_000], [120_000_000, 150_000_000]],
        "style": "dotted", "caption": "Spare", "assigns": {}}),
    );
    for (id, kind) in [
        (text, "page_item"),
        (line, "page_item"),
        (marker, "page_item"),
        (box_id, "structure_box"),
    ] {
        let sheet = s.property_sheet(&[id]);
        assert_eq!(sheet.header.items, 1, "{kind} {id}");
        assert_eq!(sheet.missing, 0);
        assert_eq!(sheet.header.kinds, [(kind.to_owned(), 1)]);
        assert_eq!(sheet.sections.len(), 1);
        assert_eq!(sheet.sections[0].kind, kind);
        assert_eq!(sheet.sections[0].targets, [id]);
        assert!(!sheet.sections[0].rows.is_empty());
    }
    let narrowed = |sheet: &PropertySheet, key: &str, expected: &str| {
        assert_eq!(
            row(sheet, key).route,
            EditRoute::ReadOnly(FieldReadOnly::Narrowed { why: expected.into() }),
            "{key}"
        );
    };
    let editable = |sheet: &PropertySheet, kind: &str, path: &str| {
        let r = row(sheet, &format!("{kind}/{path}"));
        assert_eq!(
            r.route,
            EditRoute::FieldsSet {
                kind: kind.into(),
                path: path.into()
            }
        );
        assert!(r.actions.contains(&RowAction::Edit));
    };
    const KIND: &str = "a page item's kind is not changed in place; delete the item and place another";
    const GROUP: &str = "a marker's wires are changed by deleting the marker and placing another";
    const PAGE: &str = "stays on its page; delete it and place it on the other page";
    for id in [text, line, marker] {
        let sheet = s.property_sheet(&[id]);
        narrowed(&sheet, "page_item/kind", KIND);
        narrowed(&sheet, "page_item/page", PAGE);
    }
    let sheet = s.property_sheet(&[text]);
    assert_eq!(one(row(&sheet, "page_item/kind.free_text.text")), "40x60");
    editable(&sheet, "page_item", "kind.free_text.text");
    let sheet = s.property_sheet(&[line]);
    editable(&sheet, "page_item", "kind.graphic.shape");
    let sheet = s.property_sheet(&[marker]);
    for path in ["connections", "connections[0]", "fields", "fields[0]"] {
        narrowed(&sheet, &format!("page_item/kind.wire_marker.{path}"), GROUP);
    }
    let sheet = s.property_sheet(&[box_id]);
    narrowed(&sheet, "structure_box/page", PAGE);
    assert_eq!(one(row(&sheet, "structure_box/caption")), "Spare");
    editable(&sheet, "structure_box", "caption");
}

#[test]
fn page_item_property_sheet_multi_selection_keeps_members_and_missing_counts() {
    let mut s = crate::common::sample();
    let page = page(&s, "101").to_string();
    let mut texts = Vec::new();
    for (text, x) in [("40x60", 100_000_000), ("Updated", 120_000_000)] {
        texts.push(placed(
            &mut s,
            "item.place",
            json!({"page": page,
            "kind": {"free_text": {"text": text, "at": [x, 100_000_000]}}}),
        ));
    }
    let box_id = placed(
        &mut s,
        "box.structure",
        json!({"page": page,
        "outline": [[100_000_000, 130_000_000], [120_000_000, 130_000_000], [120_000_000, 150_000_000]],
        "style": "dotted", "caption": "Spare", "assigns": {}}),
    );
    let sheet = s.property_sheet(&texts);
    assert_eq!(sheet.header.items, 2);
    assert_eq!(sheet.missing, 0);
    assert_eq!(sheet.sections.len(), 1);
    assert_eq!(sheet.sections[0].kind, "page_item");
    assert_eq!(sheet.sections[0].targets.len(), 2);
    assert!(texts.iter().all(|id| sheet.sections[0].targets.contains(id)));
    assert!(matches!(
        row(&sheet, "page_item/kind.free_text.text").value,
        RowValue::Varies { count: 2, .. }
    ));
    let sheet = s.property_sheet(&[texts[0], box_id]);
    assert_eq!(sheet.header.items, 2);
    assert_eq!(sheet.missing, 0);
    assert!(sheet.sections.is_empty(), "the two kinds share no section");
    let gone = ew_ids::Uuid::from_u128(0x0123_4567_89ab_7def_8123_4567_89ab_cdef);
    let sheet = s.property_sheet(&[gone]);
    assert_eq!(sheet.header.items, 0);
    assert_eq!(sheet.missing, 1);
    assert!(sheet.sections.is_empty());
    let sheet = s.property_sheet(&[texts[0], gone]);
    assert_eq!(sheet.header.items, 1);
    assert_eq!(sheet.missing, 1);
    assert_eq!(sheet.sections.len(), 1);
    assert_eq!(sheet.sections[0].kind, "page_item");
    assert_eq!(sheet.sections[0].targets, [texts[0]]);
    assert_eq!(one(row(&sheet, "page_item/kind.free_text.text")), "40x60");
}

/// A templated function's `template_entry` row is read-only, labelled "Part function" (its label key), its
/// value the entry's function and designations as the part names them, never the integer; a function without the
/// member (every one of the sample) has no such row.
#[test]
fn a_templated_function_shows_its_part_function() {
    let mut s = crate::common::sample();
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected/two_starter_fndef_part.json"),
        )
        .expect("the fixture"),
    )
    .expect("JSON");
    let part = fixture["id"].as_str().expect("the part copy's ID").to_owned();
    s.execute(&AnyCommand {
        name: "library.embed".into(),
        params: json!({"copies": [fixture]}),
    })
    .expect("library.embed");
    let symbol_key = "ew.iec.contact-no@qet10";
    let copies = s
        .starter_embed_part(part.parse().expect("a UUID"), symbol_key)
        .expect("the starter closure")
        .expect("the symbol and both FnDefs");
    let symbol = s.starter_symbol_copy(symbol_key).expect("a starter symbol");
    let before: Vec<ew_ids::Uuid> =
        s.query().functions().iter().map(|(id, _)| id.as_uuid()).collect();
    let step = 7_937_500i64;
    s.execute(&AnyCommand {
        name: "device.place".into(),
        params: json!({
            "page": page(&s, "101").to_string(),
            "symbol": symbol.to_string(),
            "pos": [30 * step, 30 * step],
            "target": {"from_part": {"part": part, "template": 0, "letter": "K"}},
            "copies": copies["copies"],
        }),
    })
    .expect("device.place");
    let functions: Vec<ew_ids::Uuid> = s
        .query()
        .functions()
        .iter()
        .map(|(id, _)| id.as_uuid())
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(functions.len(), 2, "the part's two template functions");
    let texts: Vec<String> = functions
        .iter()
        .map(|f| {
            let sheet = s.property_sheet(&[*f]);
            let r = row(&sheet, "function/template_entry");
            assert_eq!(r.ty, FieldType::Text);
            assert_eq!(r.route, EditRoute::ReadOnly(FieldReadOnly::NoCommand));
            assert!(r.actions.is_empty());
            one(r).to_owned()
        })
        .collect();
    // Entry 0, the main NO contact on pattern CPs 13 and 14; entry 1, the NC contact on Out1 and Out2 (the part's
    // designations, not the FnDef's 21 and 22).
    assert!(texts.contains(&"NO contact 13-14".to_owned()), "{texts:?}");
    assert!(texts.contains(&"NC contact Out1-Out2".to_owned()), "{texts:?}");
    // A function without `template_entry` (the sample's): no row.
    let plain = s
        .query()
        .functions()
        .iter()
        .find(|(_, f)| f.template_entry.is_none() && f.device.is_some())
        .map(|(id, _)| id.as_uuid())
        .expect("a function without the member");
    let sheet = s.property_sheet(&[plain]);
    assert!(
        sheet
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .all(|r| r.key != "function/template_entry"),
        "no Part function row without the member"
    );
}

/// An external interruption point's External row reads as the IP tool prints the reference, its device alone where
/// the reference names no connection (the sample's supply marks name the upstream breaker -Q0 only), and
/// `<device>:<connection>`, not the record kind's internal name, once it names one.
#[test]
fn an_external_points_row_names_its_reference() {
    let s = crate::common::sample();
    let q = s.query();
    let (point, reference) = q
        .functions()
        .iter()
        .find_map(|(id, f)| {
            let v = serde_json::to_value(&**f).ok()?;
            let external = v["kind"]["ip"]["external"].as_str()?.to_owned();
            Some((id.as_uuid(), external))
        })
        .expect("an external interruption point in the sample");
    let x = q
        .external_refs()
        .iter()
        .find(|(id, _)| id.to_string() == reference)
        .map(|(_, x)| x.clone())
        .expect("its reference");
    assert_eq!(
        (
            x.device.as_ref().map(|d| d.as_str()),
            x.connection.as_ref().map(|c| c.as_str())
        ),
        (Some("-Q0"), None)
    );
    let text = |s: &Session| match &s
        .property_sheet(&[point])
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .find(|r| r.key == "function/kind.ip.external")
        .expect("the External row")
        .value
    {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    };
    assert_eq!(text(&s), "-Q0");
    // A reference with a connection reads `<device>:<connection>`.
    let mut s = s;
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "external_ref", "targets": [reference], "path": "connection", "value": "2"}),
    })
    .expect("the connection is set");
    assert_eq!(text(&s), "-Q0:2");
}

/// `Session::stored_field` gives what the record's property row holds, without a sheet: every top-level row of a
/// device's, a wire's and a page's own section, a value or none; and nothing for a record of another kind.
#[test]
fn a_stored_field_is_what_the_record_s_row_holds() {
    let s = crate::common::sample();
    let records = [
        ("device", device(&s, "-101F1")),
        ("connection", wire(&s, "102.1")),
        ("page", page(&s, "103")),
    ];
    for (kind, id) in records {
        let sheet = s.property_sheet(&[id]);
        let section = sheet
            .sections
            .iter()
            .find(|x| x.kind == kind)
            .unwrap_or_else(|| panic!("no section {kind}"));
        let mut values = 0;
        for r in section
            .rows
            .iter()
            .filter(|r| r.depth == 0 && !r.key.contains("/derived."))
        {
            let stored = s.stored_field(kind, id, &r.path);
            match &r.value {
                RowValue::One { json, .. } => {
                    values += 1;
                    assert_eq!(stored.as_ref(), Some(json), "{}", r.key);
                }
                RowValue::Absent => assert_eq!(stored, None, "{}", r.key),
                other => panic!("{}: {other:?}", r.key),
            }
        }
        assert!(values > 0, "{kind}: no row with a value");
        assert_eq!(s.stored_field("junction", id, "id"), None, "{kind}");
    }
    assert_eq!(
        s.source_language().as_deref(),
        s.property_sheet(&[]).languages.first().map(String::as_str)
    );
}

/// A fluid line's sheet has the row Line kind with its word and no Kind row of raw JSON; a wire
/// has no Line kind row.
#[test]
fn a_fluid_line_shows_its_line_kind_row_and_a_wire_none() {
    let mut s = crate::common::sample();
    let valves: Vec<ew_ids::Uuid> = {
        let q = s.query();
        let fndefs = q.fndefs();
        q.functions()
            .iter()
            .filter(|(_, f)| {
                fndefs
                    .get(f.def)
                    .is_some_and(|d| d.item.contact_kind == ew_schema::ContactKind::Valve)
            })
            .map(|(id, _)| id.as_uuid())
            .take(2)
            .collect()
    };
    let line = placed(
        &mut s,
        "connection.draw",
        json!({"a": {"cp": {"function": valves[0].to_string(), "idx": 1}},
            "b": {"cp": {"function": valves[1].to_string(), "idx": 1}}, "kind": {"fluid": {"line_kind": "drain"}}}),
    );
    let sheet = s.property_sheet(&[line]);
    assert_eq!(one(row(&sheet, "connection/line_kind")), "drain");
    let keys: Vec<&str> = sheet
        .sections
        .iter()
        .flat_map(|x| &x.rows)
        .map(|r| r.key.as_str())
        .collect();
    assert!(!keys.contains(&"connection/kind"), "{keys:?}");
    // The fluid rows only, none of a wire's, under the line's kind in words.
    assert_eq!(
        keys,
        [
            "connection/id",
            "connection/line_kind",
            "connection/derived.medium",
            "connection/derived.tube",
            "connection/tube_part",
            "connection/tube_length",
            "connection/page",
            "connection/a",
            "connection/b",
            "connection/computed",
            "connection/route",
            "connection/source_ref"
        ]
    );
    assert_eq!(sheet.header.title, "drain line");
    let w29 = wire(&s, "102.1");
    let sheet = s.property_sheet(&[w29]);
    let keys: Vec<&str> = sheet
        .sections
        .iter()
        .flat_map(|x| &x.rows)
        .map(|r| r.key.as_str())
        .collect();
    assert!(!keys.contains(&"connection/line_kind"), "{keys:?}");
}

/// What a device is comes from what the model knows of it. On the sample the valve terminal -201K1 is a PLC
/// station of role valve terminal with a bus port, the MCB -101F1 is electrical and nothing else, a terminal of -X1031
/// is a terminal; the sheet's header carries the same natures.
#[test]
fn a_devices_natures_come_from_its_aspects_and_its_points() {
    let s = crate::common::sample();
    let natures = |tag: &str| s.device_natures(device(&s, tag)).expect("a device");
    let vt = natures("-201K1");
    assert_eq!(vt.station, Some(ew_schema::StationRole::ValveTerminal));
    assert!(vt.bus_device() && !vt.mechanical_only(), "{vt:?}");
    let mcb = natures("-101F1");
    assert!(
        mcb.electrical() && !mcb.fluid() && !mcb.bus_device() && !mcb.terminal,
        "{mcb:?}"
    );
    let terminal = natures("-X1031:U");
    assert!(terminal.terminal && !terminal.fluid(), "{terminal:?}");
    let header = s.property_sheet(&[device(&s, "-101F1")]).header;
    assert_eq!(header.what.map(|w| w.natures), Some(mcb));
}
