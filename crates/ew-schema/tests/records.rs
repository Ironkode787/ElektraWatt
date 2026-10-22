// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Canonical-line round trips: the doc 05 §3.5 examples byte for byte, one golden line per implemented
//! kind (doc 20 §4–§10 values), unknown members and variants, `$v`, and the rejection rules.

mod common;

use common::fill;
use common::lines::{DOC05_LINES, DOC05_PART, GOLDEN, GOLDEN_LIB};
use ew_schema::{ColourCode, ConnectionKind, Csa, Kind, LibDocument, PageType, Record, SchemaError, Unit};
use serde_json::Value;

/// Parses a line, serialises it compactly and returns the result.
fn round_trip(line: &str) -> String {
    let record: Record = serde_json::from_str(line).unwrap_or_else(|e| panic!("parse failed: {e}\n{line}"));
    serde_json::to_string(&record).unwrap()
}

/// Asserts that `line` is canonical: parse → serialise gives the same bytes, and the typed value equals
/// the line as a JSON value.
fn assert_canonical(line: &str) -> Record {
    assert_eq!(round_trip(line), line, "not canonical");
    let record: Record = serde_json::from_str(line).unwrap();
    let expected: Value = serde_json::from_str(line).unwrap();
    assert_eq!(serde_json::to_value(&record).unwrap(), expected);
    record
}

fn assert_lib_canonical(doc: &str) -> LibDocument {
    let parsed: LibDocument =
        serde_json::from_str(doc).unwrap_or_else(|e| panic!("parse failed: {e}\n{doc}"));
    assert_eq!(serde_json::to_string(&parsed).unwrap(), doc, "not canonical");
    parsed
}

// ---------------------------------------------------------------------------------------------------------
// Doc 05 §3.5, verbatim

#[test]
fn doc05_example_lines_round_trip_byte_for_byte() {
    let kinds: Vec<Option<Kind>> = DOC05_LINES
        .iter()
        .map(|line| assert_canonical(line).kind())
        .collect();
    assert_eq!(
        kinds,
        [
            Some(Kind::Page),
            Some(Kind::Representation),
            Some(Kind::Connection),
            Some(Kind::Device),
            Some(Kind::Function),
            Some(Kind::Device),
        ]
    );
}

#[test]
fn doc05_example_values_are_typed() {
    let Record::Connection(w7) = serde_json::from_str(DOC05_LINES[2]).unwrap() else {
        panic!("W7 is a connection");
    };
    assert_eq!(w7.kind, ConnectionKind::Wire);
    assert_eq!(w7.overrides.csa, Some(Csa::Mm2(ew_units::Dec3(2_500))));
    assert!(w7.overrides.show_marker);
    let Record::Device(g1) = serde_json::from_str(DOC05_LINES[3]).unwrap() else {
        panic!("-102G1 is a device");
    };
    assert_eq!(g1.tag.letter.as_str(), "G");
    let ew_schema::TechVal::Quantity(q) = &g1.tech_data[0].value else {
        panic!("i_out is a quantity");
    };
    assert_eq!(q.unit, Unit::A);
}

/// The doc 05 §3.5 part copy, compacted, with the abbreviated IDs and hash filled in. Licence and sources
/// are members of the part's `meta` provenance object (doc 05 §3.4, doc 07 §2.1).
#[test]
fn doc05_part_copy_round_trips() {
    let doc = fill(DOC05_PART);
    let LibDocument::Part(copy) = assert_lib_canonical(&doc) else {
        panic!("a part");
    };
    assert_eq!(copy.version, "1.2.0");
    assert_eq!(copy.item.cp_pattern.len(), 7);
    assert_eq!(copy.item.meta.licence.as_deref(), Some("CC0-1.0"));
    assert_eq!(copy.item.meta.sources, ["golden tables"]);
    assert!(copy.item.extra.is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// Golden lines, one or more per implemented kind

#[test]
fn golden_lines_are_canonical() {
    for template in GOLDEN {
        assert_canonical(&fill(template));
    }
}

#[test]
fn every_implemented_record_kind_has_a_golden_line() {
    let seen: std::collections::BTreeSet<Kind> = GOLDEN
        .iter()
        .map(|t| serde_json::from_str::<Record>(&fill(t)).unwrap().kind().unwrap())
        .collect();
    for kind in Kind::ALL {
        if kind.status() == ew_schema::KindStatus::Implemented && !kind.is_library() {
            assert!(seen.contains(&kind), "{kind} has no golden line");
        }
    }
}

#[test]
fn golden_lineage_namespace_is_derived_from_its_anchor() {
    let ns = ew_ids::v5::lineage_namespace("csv", &ew_ids::v5::LineageAnchor::Fixed("golden")).unwrap();
    let line = fill(r#"{"$k":"import_lineage","id":"@LIN","importer":"csv","source_name":"tables","anchor":{"fixed":"golden"},"namespace":"NS"}"#)
        .replace("NS", &ns.to_string());
    let Record::ImportLineage(l) = assert_canonical(&line) else {
        panic!("a lineage");
    };
    assert_eq!(l.namespace.as_uuid(), ns);
}

#[test]
fn golden_library_documents_are_canonical() {
    let kinds: Vec<Option<Kind>> = GOLDEN_LIB
        .iter()
        .map(|t| assert_lib_canonical(&fill(t)).kind())
        .collect();
    assert_eq!(
        kinds,
        [
            Some(Kind::Fndef),
            Some(Kind::Symbol),
            Some(Kind::Form),
            Some(Kind::Part)
        ]
    );
}

#[test]
fn a_text_slot_style_is_optional_and_round_trips() {
    // The golden symbol has two slots without a style; the member is written only when present (ID-197).
    let plain = fill(GOLDEN_LIB[1]);
    let LibDocument::Symbol(copy) = assert_lib_canonical(&plain) else {
        panic!("a symbol");
    };
    assert!(copy.item.text_slots.iter().all(|s| s.style.is_none()));
    assert!(!plain.contains("\"style\":\"xref.device\""));

    let styled = plain.replacen(
        r#""max_width":10000000}"#,
        r#""max_width":10000000,"style":"xref.device"}"#,
        1,
    );
    assert_ne!(styled, plain);
    let LibDocument::Symbol(copy) = assert_lib_canonical(&styled) else {
        panic!("a symbol");
    };
    let styles: Vec<Option<&str>> = copy
        .item
        .text_slots
        .iter()
        .map(|s| s.style.as_ref().map(|n| n.as_str()))
        .collect();
    assert_eq!(styles, [None, Some("xref.device")]);
}

// ---------------------------------------------------------------------------------------------------------
// Unknown content

#[test]
fn unknown_members_are_kept_and_written_last_in_byte_order() {
    // Unknown members anywhere in the object, including nested ones and plugin keys.
    let input = fill(
        r#"{"$k":"page","alpha":{"b":1,"a":2},"id":"@P103","folio":{"zz":true,"number":3},"kind":"bus_topology","plugin.acme.note":"x","title":"Bus Topology - Profibus","Zeta":[1,2],"grid":{"num":7937500,"den":3}}"#,
    );
    let expected = fill(
        r#"{"$k":"page","id":"@P103","folio":{"number":3,"zz":true},"kind":"bus_topology","title":"Bus Topology - Profibus","grid":{"num":7937500,"den":3},"Zeta":[1,2],"alpha":{"a":2,"b":1},"plugin.acme.note":"x"}"#,
    );
    assert_eq!(round_trip(&input), expected);
    assert_canonical(&expected);
}

#[test]
fn unknown_device_aspects_follow_the_known_ones() {
    // `assembly` is typed (cycle 15); `bus_node` is not implemented yet and stays in `extra`, after the known
    // aspects.
    let line = fill(
        r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"U","counter":1},"cable":{},"assembly":{"bom_basis":"components"},"bus_node":{"role":"slave"}}"#,
    );
    let Record::Device(d) = assert_canonical(&line) else {
        panic!("a device");
    };
    assert!(d.aspects.cable.is_some());
    assert_eq!(
        d.aspects.assembly.as_ref().map(|a| &a.bom_basis),
        Some(&ew_schema::BomBasis::Components)
    );
    assert_eq!(d.extra.len(), 1);
}

#[test]
fn the_assembly_aspect_is_typed() {
    // Doc 03 §7, doc 20 §14.2: two order codes on -201U1, the second covering two children.
    let line = fill(
        r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"U","counter":1},"assembly":{"configs":[{"label":{"en-GB":"Order Code 1"},"order_code":"51E-F13GCQP-D+H2GG","part":{"copy":"@PART_NEBU"},"covers":["@DEV_G1"]},{"order_code":"32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS","covers":["@DEV_S51","@DEV_X1031"]}]}}"#,
    );
    let Record::Device(d) = assert_canonical(&line) else {
        panic!("a device");
    };
    let a = d.aspects.assembly.as_ref().expect("the assembly aspect");
    assert_eq!(a.configs.len(), 2);
    assert_eq!(a.configs[0].order_code.as_str(), "51E-F13GCQP-D+H2GG");
    assert!(a.configs[0].part.is_some());
    assert_eq!(a.configs[1].covers.len(), 2);
    assert_eq!(a.bom_basis, ew_schema::BomBasis::Assembly);
    assert!(d.extra.is_empty());
}

#[test]
fn the_assembly_aspect_is_written_before_the_plc_aspects() {
    // Doc 05 §3.1: strip, terminal, cable, assembly, plc_station, plc_module, then the later aspects.
    let line = fill(
        r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"U","counter":1},"assembly":{"bom_basis":"components"},"plc_station":{"config_project":"@PLC1","role":"valve_terminal"},"plc_module":{},"bus_node":{"role":"slave"}}"#,
    );
    assert_canonical(&line);
    let assembly = r#""assembly":{"bom_basis":"components"},"#;
    let shuffled = line
        .replace(assembly, "")
        .replace(r#""bus_node""#, &format!("{assembly}\"bus_node\""));
    assert_ne!(shuffled, line);
    assert_eq!(round_trip(&shuffled), line);
}

#[test]
fn unknown_enum_variants_are_kept() {
    let cases = [
        r#"{"$k":"page","id":"@P103","folio":{"number":3},"kind":"wiring_matrix","grid":{"num":7937500,"den":3}}"#,
        r#"{"$k":"connection","id":"@CON_W7","a":{"cp":["@FN_S51",0]},"b":{"port":["@FN_F1",4]},"kind":{"fluid":{"tube":{"od":8000000}}}}"#,
        r#"{"$k":"conductor","id":"@COND_BN","cable":"@DEV_W106_1","core":"X","colour":[{"foreign":"BR"},{"pantone":"7462"}],"csa":{"swg":12}}"#,
        r#"{"$k":"structure_node","id":"@FN_P01","aspect":"zone","code":"Z1"}"#,
        r#"{"$k":"page","id":"@P103","folio":{"number":3},"kind":"multi_line","structure":{"location":"later"},"grid":{"num":7937500,"den":3}}"#,
    ];
    for template in cases {
        assert_canonical(&fill(template));
    }
    let Record::Page(p) = serde_json::from_str(&fill(cases[0])).unwrap() else {
        panic!("a page");
    };
    assert!(matches!(p.kind, PageType::Unknown(_)));
    let Record::Conductor(c) = serde_json::from_str(&fill(cases[2])).unwrap() else {
        panic!("a conductor");
    };
    let codes = c.colour.unwrap();
    assert!(matches!(codes.codes()[0], ColourCode::Foreign(_)));
    assert!(matches!(codes.codes()[1], ColourCode::Unknown(_)));
    assert!(matches!(c.csa, Some(Csa::Unknown(_))));
}

/// Records of reserved and unknown kinds are kept as `Record::Unknown` with every member. Their members
/// re-emit in byte-wise key order; keeping an untouched line byte for byte is the format writer's
/// original-line rule (doc 05 §10.3).
#[test]
fn unknown_and_reserved_kinds_are_kept() {
    for line in [
        r#"{"$k":"waiver","id":"0192f3a0-0000-7000-8000-000000000001","reason":"accepted","rule":"EW-CORE-047"}"#,
        r#"{"$k":"hologram","beam":[1,2],"id":"0192f3a0-0000-7000-8000-000000000002"}"#,
    ] {
        let record: Record = serde_json::from_str(line).unwrap();
        assert!(matches!(record, Record::Unknown(_)));
        assert_eq!(record.kind(), None);
        assert!(record.kind_name().is_some());
        assert!(record.id().is_some());
        assert_eq!(record.anchor_ref(), ew_schema::AnchorRef::Unknown);
        assert_eq!(serde_json::to_string(&record).unwrap(), line);
    }
    let unsorted = r#"{"$k":"hologram","id":"0192f3a0-0000-7000-8000-000000000002","beam":[1,2]}"#;
    let record: Record = serde_json::from_str(unsorted).unwrap();
    let expected: Value = serde_json::from_str(unsorted).unwrap();
    assert_eq!(serde_json::to_value(&record).unwrap(), expected);
}

// ---------------------------------------------------------------------------------------------------------
// $v

#[test]
fn schema_version_is_omitted_when_one_and_read_when_present() {
    let with_v = fill(r#"{"$k":"section","id":"@SEC1","$v":1,"number":1,"folio_range":[100,199]}"#);
    let without = fill(r#"{"$k":"section","id":"@SEC1","number":1,"folio_range":[100,199]}"#);
    assert_eq!(round_trip(&with_v), without);
    let newer = fill(r#"{"$k":"section","id":"@SEC1","$v":2,"number":1,"folio_range":[100,199]}"#);
    let err = serde_json::from_str::<Record>(&newer).unwrap_err().to_string();
    assert!(err.contains("$v 2"), "{err}");
}

// ---------------------------------------------------------------------------------------------------------
// Rejections

#[test]
fn floats_are_rejected_everywhere() {
    for template in [
        // a typed integer
        r#"{"$k":"section","id":"@SEC1","number":1.0,"folio_range":[100,199]}"#,
        // a length in nm
        r#"{"$k":"junction","id":"@JN_J1","page":"@P101","at":[1e3,0],"style":"dot","sequence":[]}"#,
        // an unknown member
        r#"{"$k":"section","id":"@SEC1","number":1,"folio_range":[100,199],"x":0.5}"#,
        // inside an unknown enum value
        r#"{"$k":"page","id":"@P103","folio":{"number":3},"kind":{"future":1.5},"grid":{"num":7937500,"den":3}}"#,
        // an AWG size
        r#"{"$k":"conductor","id":"@COND_BN","cable":"@DEV_W106_1","core":"X","csa":{"awg":18.0}}"#,
    ] {
        let line = fill(template);
        assert!(serde_json::from_str::<Record>(&line).is_err(), "accepted: {line}");
    }
    assert_eq!(
        ew_schema::check_value(&serde_json::json!([1, {"a": 2.5}])),
        Err(SchemaError::Float)
    );
}

#[test]
fn unsafe_integers_and_bad_decimals_are_rejected() {
    for template in [
        r#"{"$k":"connection","id":"@CON_W7","a":{"cp":["@FN_S51",0]},"b":{"cp":["@FN_F1",0]},"x":9007199254740992}"#,
        r#"{"$k":"conductor","id":"@COND_BN","cable":"@DEV_W106_1","core":"X","csa":"2.50"}"#,
        r#"{"$k":"conductor","id":"@COND_BN","cable":"@DEV_W106_1","core":"X","colour":[]}"#,
        r#"{"$k":"conductor","id":"@COND_BN","cable":"@DEV_W106_1","core":"X","colour":["BK","BU","WH"]}"#,
    ] {
        let line = fill(template);
        assert!(serde_json::from_str::<Record>(&line).is_err(), "accepted: {line}");
    }
}

#[test]
fn reversed_rects_are_rejected() {
    let ok = fill(
        r#"{"$k":"report_block","id":"@RB_PD201","source":{"kind":"part_data"},"form":"@FORM_TOC","first_folio":201,"embedded":{"page":"@P201","rect":[[5,5],[5,5]]}}"#,
    );
    assert_canonical(&ok);
    let reversed = ok.replace("[[5,5],[5,5]]", "[[5,5],[4,6]]");
    let err = serde_json::from_str::<Record>(&reversed).unwrap_err().to_string();
    assert!(err.contains("reversed"), "{err}");
}

#[test]
fn library_kinds_are_not_records_and_vice_versa() {
    let doc = fill(GOLDEN_LIB[0]);
    assert!(serde_json::from_str::<Record>(&doc).is_err());
    let line = fill(GOLDEN[1]);
    assert!(serde_json::from_str::<LibDocument>(&line).is_err());
    assert!(serde_json::from_str::<Record>(r#"{"id":"x"}"#).is_err());
}

// ---------------------------------------------------------------------------------------------------------
// revision (FT-165, doc 03 §18)

#[test]
fn a_revision_is_typed_and_lives_in_the_revisions_file() {
    let line = fill(
        r#"{"$k":"revision","id":"@REV_V2","label":"V2.0","date":"2025-04-14","author":"engineer","checker":"checker","description":{"en-GB":"Changes on 101 and 106"},"state":"open","page_notes":[{"page":"@P1"},{"page":"@P103","description":{"en-GB":"Supply moved"}}]}"#,
    );
    let record = assert_canonical(&line);
    assert_eq!(record.kind(), Some(Kind::Revision));
    assert_eq!(record.anchor_ref(), ew_schema::AnchorRef::Root);
    assert_eq!(
        record.file_rule(),
        Some(ew_schema::FileRule::RootFile("revisions.jsonl"))
    );
    let Record::Revision(r) = record else {
        panic!("a revision");
    };
    assert_eq!(r.label.as_str(), "V2.0");
    assert!(r.is_open());
    assert_eq!(r.date, Some(ew_schema::Date::new(2025, 4, 14).unwrap()));
    assert_eq!(r.author, Some(ew_schema::PartyRole::Engineer));
    assert_eq!(r.checker, Some(ew_schema::PartyRole::Checker));
    assert_eq!(r.page_notes.len(), 2);
    assert!(r.page_notes[0].description.is_empty());
    assert!(!r.page_notes[1].description.is_empty());
}

/// The historic released revision of FT-165: no date is required, no release record, no archive.
#[test]
fn a_historic_released_revision_needs_only_label_and_state() {
    let line = fill(r#"{"$k":"revision","id":"@REV_V1","label":"V1.0","state":"released"}"#);
    let Record::Revision(r) = assert_canonical(&line) else {
        panic!("a revision");
    };
    assert_eq!(r.state, ew_schema::RevisionState::Released);
    assert!(r.date.is_none() && r.author.is_none() && r.page_notes.is_empty() && r.extra.is_empty());
}

/// The members of doc 03 §18 that later commands write (`purpose`, `sign_off`, `release`, `reason`) are kept
/// in `extra` and written after the known members in byte-wise order, nested unknown members of a page note
/// too; `withdraws` (the ID of the withdrawn revision) is typed.
#[test]
fn revision_members_of_later_commands_stay_in_extra() {
    let line = fill(
        r#"{"$k":"revision","id":"@REV_V2","label":"V2.0","state":"withdrawn","page_notes":[{"page":"@P1","cloud":true}],"withdraws":"@REV_V1","purpose":"approval","reason":{"en-GB":"wrong customer"},"release":{"changed_pages":["@P1"],"generation":3},"sign_off":{"checker":"checker"}}"#,
    );
    let Record::Revision(r) = assert_canonical(&line) else {
        panic!("a revision");
    };
    let keys: Vec<&str> = r.extra.keys().map(String::as_str).collect();
    assert_eq!(keys, ["purpose", "reason", "release", "sign_off"]);
    assert_eq!(r.withdraws.map(|w| w.to_string()), Some(fill("@REV_V1")));
    assert_eq!(
        r.page_notes[0].extra.keys().next().map(String::as_str),
        Some("cloud")
    );
    assert_eq!(r.state, ew_schema::RevisionState::Withdrawn);
}

/// A state of a newer format is newer data, kept verbatim (ADR-0011 amendment 1); the stored form is
/// lower-case snake case, so `"Released"` is such a value too.
#[test]
fn a_revision_state_of_a_newer_format_is_kept_verbatim() {
    for state in ["superseded", "Released"] {
        let line = fill(&format!(
            r#"{{"$k":"revision","id":"@REV_V1","label":"V1.0","state":"{state}"}}"#
        ));
        let Record::Revision(r) = assert_canonical(&line) else {
            panic!("a revision");
        };
        assert_eq!(
            r.state,
            ew_schema::RevisionState::Unknown(Value::String(state.into()))
        );
        assert!(!r.is_open());
    }
}

#[test]
fn a_revision_without_label_or_state_or_with_a_bare_page_id_is_rejected() {
    for line in [
        r#"{"$k":"revision","id":"@REV_V1","state":"open"}"#,
        r#"{"$k":"revision","id":"@REV_V1","label":"V1.0"}"#,
        r#"{"$k":"revision","id":"@REV_V1","label":"V1.0","state":"open","page_notes":["@P1"]}"#,
        r#"{"$k":"revision","id":"@REV_V1","label":"V1.0","state":"open","date":"2025-02-30"}"#,
    ] {
        let line = fill(line);
        assert!(serde_json::from_str::<Record>(&line).is_err(), "accepted: {line}");
    }
}
