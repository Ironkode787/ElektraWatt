// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The part-level bus section (WP-S1.21 §8a.1, D15; BU-7a, BU-7b): `PartSections.bus` with its port list types the
//! members D15 names, keeps every other member in `extra` and every newer enum value as `Unknown`, writes byte-equal
//! in §8a.1's member order, omits an empty port list, and refuses a port without a required member.

mod common;

use common::fill;
use ew_schema::{BusSystem, LibDocument, OnOff, Part, PortRole};

const HEAD: &str = concat!(
    r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"1.3.0","sha256":"@HASH","#,
    r#""manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^1"},"part_number":"2904602","#
);

/// A part line whose `sections` member is `sections` (a JSON object text).
fn part_line(sections: &str) -> String {
    fill(&format!(r#"{HEAD}"sections":{sections}}}"#))
}

fn part(doc: &LibDocument) -> &Part {
    let LibDocument::Part(copy) = doc else {
        panic!("a part");
    };
    &copy.item
}

/// BU-7a: every typed member typed, the rest in `extra` or `Unknown`, and the line written byte-equal.
#[test]
fn a_bus_section_types_its_members_and_keeps_the_rest() {
    let line = part_line(concat!(
        r#"{"plc":{"role":"station"},"bus":{"ports":["#,
        r#"{"interface":"X1","port_label":"P1R","bus":"profinet","role":"device","termination":"off","address_range":[1,125],"connector":"rj45","signals":{"1":"TD+"}},"#,
        r#"{"interface":"DP","bus":"sercos","role":"observer","termination":"auto"}"#,
        r#"],"rates":[9600,12000000]}}"#
    ));
    let doc: LibDocument = serde_json::from_str(&line).unwrap();
    assert_eq!(serde_json::to_string(&doc).unwrap(), line, "not canonical");
    let p = part(&doc);
    assert!(
        p.sections.extra.is_empty(),
        "left in extra: {:?}",
        p.sections.extra
    );
    let bus = p.sections.bus.as_ref().expect("the bus section is typed");
    assert_eq!(bus.ports.len(), 2);
    assert_eq!(
        bus.extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["rates"]
    );
    let first = &bus.ports[0];
    assert_eq!(first.interface.as_str(), "X1");
    assert_eq!(first.port_label.as_ref().map(|l| l.as_str()), Some("P1R"));
    assert_eq!(first.bus, BusSystem::Profinet);
    assert_eq!(first.role, PortRole::Device);
    assert_eq!(first.termination, Some(OnOff::Off));
    assert_eq!(
        first.extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["address_range", "connector", "signals"]
    );
    let second = &bus.ports[1];
    assert_eq!(second.interface.as_str(), "DP");
    assert_eq!(second.port_label, None);
    assert!(matches!(second.bus, BusSystem::Unknown(_)), "{:?}", second.bus);
    assert!(matches!(second.role, PortRole::Unknown(_)), "{:?}", second.role);
    assert!(
        matches!(second.termination, Some(OnOff::Unknown(_))),
        "{:?}",
        second.termination
    );
    assert!(second.extra.is_empty());
    // The typed value reads back equal.
    let again: LibDocument = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    assert_eq!(again, doc);
}

/// BU-7a: a part without `bus` writes as before, and an empty port list is written `{}` (the canonical form).
#[test]
fn a_part_without_a_bus_section_and_an_empty_port_list() {
    let line = part_line(r#"{"plc":{"role":"station"}}"#);
    let doc: LibDocument = serde_json::from_str(&line).unwrap();
    assert_eq!(serde_json::to_string(&doc).unwrap(), line);
    assert_eq!(part(&doc).sections.bus, None);

    let empty = part_line(r#"{"bus":{"ports":[]}}"#);
    let doc: LibDocument = serde_json::from_str(&empty).unwrap();
    assert_eq!(part(&doc).sections.bus.as_ref().map(|b| b.ports.len()), Some(0));
    assert_eq!(serde_json::to_string(&doc).unwrap(), part_line(r#"{"bus":{}}"#));
}

/// §8a.1: the section is written after `mounting_plate` and before the unknown sections, where an older build,
/// which keeps `bus` in `PartSections.extra`, wrote it (an unknown section sorting after `bus`).
#[test]
fn the_bus_section_is_written_after_the_typed_sections_and_before_extra() {
    let line = part_line(concat!(
        r#"{"mounting_plate":{"width":500000000},"bus":{"ports":[{"interface":"DP","bus":"profibus_dp","role":"master"}]},"#,
        r#""zz_vendor":{"a":1}}"#
    ));
    let doc: LibDocument = serde_json::from_str(&line).unwrap();
    assert_eq!(serde_json::to_string(&doc).unwrap(), line);
    let p = part(&doc);
    assert_eq!(
        p.sections.extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["zz_vendor"]
    );
    assert!(p.sections.bus.is_some());
}

/// BU-7b: a port without `interface`, without `bus` or without `role` fails to load, the error naming the member.
#[test]
fn a_port_without_a_required_member_fails_naming_it() {
    for (port, member) in [
        (r#"{"bus":"profibus_dp","role":"master"}"#, "interface"),
        (r#"{"interface":"DP","role":"master"}"#, "bus"),
        (r#"{"interface":"DP","bus":"profibus_dp"}"#, "role"),
    ] {
        let line = part_line(&format!(r#"{{"bus":{{"ports":[{port}]}}}}"#));
        let err = serde_json::from_str::<LibDocument>(&line).expect_err(member);
        assert!(
            err.to_string().contains(&format!("missing field `{member}`")),
            "{member}: {err}"
        );
    }
}
