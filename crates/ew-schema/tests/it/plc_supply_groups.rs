// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The supply groups of a part's PLC section (FR-PLC-024; PX-2): `PlcSection.supply_groups` types
//! `kind`, `cps {l_plus, m}` and `feeds`, keeps every other member in the group's own `extra` and every newer enum
//! value as `Unknown`, writes byte-equal after `pin_of` and before the section's unknown members, omits an empty list,
//! and refuses a group without one of its pins, naming the member.
//!
//! The orchestrator's ruling 1 of cycle 42, worded exactly: "`supply_groups` sits after `pin_of` and before the
//! flattened `extra` of `PlcSection` ... Each group ends with its own flattened `extra` (`de_extra`), as `PlcSection`
//! does, so an unknown member of a group round-trips".

use crate::common;

use common::fill;
use ew_schema::{LibDocument, Part, SupplyFeeds, SupplyKind};

const HEAD: &str = concat!(
    r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"1.3.0","sha256":"@HASH","#,
    r#""manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^1"},"part_number":"2904602","#
);

/// A part line whose PLC section is `plc` (a JSON object text).
fn part_line(plc: &str) -> String {
    fill(&format!(r#"{HEAD}"sections":{{"plc":{plc}}}}}"#))
}

fn part(doc: &LibDocument) -> &Part {
    let LibDocument::Part(copy) = doc else {
        panic!("a part");
    };
    &copy.item
}

/// PX-2: two groups, one feeding the outputs, one of a newer kind with an unknown member; the line reads typed and
/// writes byte-equal, the unknowns kept where they were.
#[test]
fn two_supply_groups_round_trip_with_their_unknowns() {
    let line = part_line(concat!(
        r#"{"role":"module","pin_of":"{n+1}","supply_groups":["#,
        r#"{"kind":"electronics","cps":{"l_plus":"4","m":"2"}},"#,
        r#"{"kind":"load","cps":{"l_plus":"5","m":"1"},"feeds":"outputs"},"#,
        r#"{"kind":"sensor","cps":{"l_plus":"7","m":"6","x_pin":"8"},"feeds":"odd","current":"2 A"}"#,
        r#"],"earth":{"pin":"3"}}"#
    ));
    let doc: LibDocument = serde_json::from_str(&line).unwrap();
    assert_eq!(serde_json::to_string(&doc).unwrap(), line, "not canonical");
    let plc = part(&doc).sections.plc.as_ref().expect("a PLC section");
    assert_eq!(
        plc.extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["earth"],
        "the earth pin stays in extra"
    );
    let [el, val, newer] = plc.supply_groups.as_slice() else {
        panic!("three groups: {:?}", plc.supply_groups);
    };
    assert_eq!(el.kind, SupplyKind::Electronics);
    assert_eq!((el.cps.l_plus.as_str(), el.cps.m.as_str()), ("4", "2"));
    assert_eq!(el.feeds, None);
    assert_eq!(el.feeds(), SupplyFeeds::All, "absent feeds all");
    assert_eq!(val.kind, SupplyKind::Load);
    assert_eq!((val.cps.l_plus.as_str(), val.cps.m.as_str()), ("5", "1"));
    assert_eq!(val.feeds, Some(SupplyFeeds::Outputs));
    assert_eq!(
        newer.kind,
        SupplyKind::Unknown(serde_json::json!("sensor")),
        "a newer kind is kept"
    );
    assert_eq!(newer.feeds, Some(SupplyFeeds::Unknown(serde_json::json!("odd"))));
    assert_eq!(
        newer.extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["current"]
    );
    assert_eq!(
        newer.cps.extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["x_pin"]
    );
    let again: LibDocument = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    assert_eq!(again, doc);
}

/// PX-2: `{"supply_groups": []}` writes as absent, and a section without groups writes the bytes it wrote before
/// (no copy's `sha256` moves).
#[test]
fn an_empty_group_list_writes_as_absent() {
    let doc: LibDocument =
        serde_json::from_str(&part_line(r#"{"role":"module","supply_groups":[]}"#)).unwrap();
    assert!(part(&doc).sections.plc.as_ref().unwrap().supply_groups.is_empty());
    assert_eq!(
        serde_json::to_string(&doc).unwrap(),
        part_line(r#"{"role":"module"}"#)
    );
    let before = part_line(r#"{"role":"module","pin_of":"{n+1}","earth":{"pin":"3"}}"#);
    let doc: LibDocument = serde_json::from_str(&before).unwrap();
    assert_eq!(serde_json::to_string(&doc).unwrap(), before);
}

/// PX-2: a group without `cps.l_plus` (or `cps.m`, `cps` or `kind`) fails to load, the error naming the member.
#[test]
fn a_group_without_a_required_member_fails_naming_it() {
    for (group, member) in [
        (r#"{"kind":"load","cps":{"m":"1"}}"#, "l_plus"),
        (r#"{"kind":"load","cps":{"l_plus":"5"}}"#, "m"),
        (r#"{"kind":"load"}"#, "cps"),
        (r#"{"cps":{"l_plus":"5","m":"1"}}"#, "kind"),
    ] {
        let line = part_line(&format!(r#"{{"role":"module","supply_groups":[{group}]}}"#));
        let err = serde_json::from_str::<LibDocument>(&line).expect_err(member);
        assert!(
            err.to_string().contains(&format!("missing field `{member}`")),
            "{member}: {err}"
        );
    }
}
