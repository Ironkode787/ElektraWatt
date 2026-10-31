// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The assembly table (doc 20 §14.2, D-36, D-41): the `assembly` aspect of -201U1 in the golden tables, and the
//! rows that are refused on small in-memory tables.

use crate::common;

use ew_ids::Id;
use ew_import_tables::ids::record_id;
use ew_interop_api::{Code, ImportBatch, Severity};
use ew_schema::{Assembly, BomBasis, Device, LibDocument, PartKind, PartRef, Record};

const ORDER_CODE_1: &str = "51E-F13GCQP-D+H2GG";
const ORDER_CODE_2: &str = "32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS";

fn device_id(key: &str) -> Id<Device> {
    record_id("devices", key)
}

fn device<'b>(b: &'b ImportBatch, key: &str) -> &'b Device {
    let id = device_id(key);
    b.records
        .iter()
        .find_map(|r| match r {
            Record::Device(d) if d.id == id => Some(d),
            _ => None,
        })
        .unwrap_or_else(|| panic!("device {key} is emitted"))
}

fn part_copy(b: &ImportBatch, key: &str) -> Id<ew_schema::Part> {
    b.lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(p) if p.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) => Some(p.id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("part {key}"))
}

fn aspect<'b>(b: &'b ImportBatch, key: &str) -> Option<&'b Assembly> {
    device(b, key).aspects.assembly.as_ref()
}

/// The golden batch gives -201U1 two order codes in order, named by part key, and no other device an assembly
/// aspect.
#[test]
fn the_golden_valve_terminal_carries_its_two_order_codes() {
    let b = common::import(&common::golden_inputs()).expect("the golden tables import");
    assert!(b.report.entries.is_empty(), "{:#?}", b.report.entries);
    let assembly = aspect(&b, "-201U1").expect("the assembly aspect");
    assert_eq!(assembly.bom_basis, BomBasis::Assembly);
    assert!(assembly.extra.is_empty());
    let configs = &assembly.configs;
    assert_eq!(configs.len(), 2);
    assert_eq!(
        configs.iter().map(|c| c.order_code.as_str()).collect::<Vec<_>>(),
        [ORDER_CODE_1, ORDER_CODE_2]
    );
    assert_eq!(
        configs.iter().map(|c| c.label.as_neutral()).collect::<Vec<_>>(),
        [Some("Order Code 1"), Some("Order Code 2")]
    );
    for (config, key) in configs.iter().zip([ORDER_CODE_1, ORDER_CODE_2]) {
        assert_eq!(config.part, Some(PartRef::new(part_copy(&b, key))), "{key}");
    }
    // The parts are configured products, one set each (doc 20 §14.2).
    for key in [ORDER_CODE_1, ORDER_CODE_2] {
        let part = b
            .lib
            .iter()
            .find_map(|d| match d {
                LibDocument::Part(p) if p.id == part_copy(&b, key) => Some(&p.item),
                _ => None,
            })
            .expect("the part");
        assert_eq!(part.kind, PartKind::Configured, "{key}");
        // WP-S1.19 D10: the reference names the project-local manufacturer copy of Festo, no longer a `key`.
        assert!(part.manufacturer.extra.is_empty(), "{key}");
        let festo = b
            .lib
            .iter()
            .find_map(|d| match d {
                LibDocument::Manufacturer(m) if m.item_id == part.manufacturer.item => Some(&m.item),
                _ => None,
            })
            .expect("the manufacturer copy");
        assert_eq!(festo.name.as_neutral(), Some("Festo"));
    }
    // A1 covers the head itself, A2 the 11 children and the 28 valves; the lists are disjoint.
    assert_eq!(configs[0].covers.as_slice(), [device_id("-201U1")]);
    let second: Vec<Id<Device>> = configs[1].covers.as_slice().to_vec();
    assert_eq!(second.len(), 11 + 28);
    assert!(!second.contains(&device_id("-201U1")));
    let children: Vec<Id<Device>> = (2..=9)
        .map(|k| format!("-201U{k}"))
        .chain((1..=3).map(|k| format!("-202U{k}")))
        .map(|key| device_id(&key))
        .collect();
    for child in &children {
        assert!(second.contains(child));
    }
    let valves: Vec<&Device> = b
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Device(d) if d.parent.is_some_and(|p| children.contains(&p)) => Some(d),
            _ => None,
        })
        .collect();
    assert_eq!(valves.len(), 28);
    for valve in valves {
        assert!(second.contains(&valve.id));
    }
    // No other device is an assembly.
    let assemblies = b
        .records
        .iter()
        .filter(|r| matches!(r, Record::Device(d) if d.aspects.assembly.is_some()))
        .count();
    assert_eq!(assemblies, 1);
}

// ---------------------------------------------------------------------------------------------------------
// Refused rows

const PARTS: &str =
    "key,manufacturer,part_number,kind,unit\nOC1,Acme,OC1,configured,set\nOC2,Acme,OC2,configured,set\n";
const DEVICES: &str = "key,letter,counter,parent,position\n-1U1,U,1,,\n-1U2,U,2,-1U1,1\n-1U3,U,3,-1U1,2\n-1U4,U,4,-1U2,1\n-2U1,U,5,,\n";
const HEADER: &str = "key,assembly,n,label,order_code,part,covers\n";

fn run(configs: &str) -> ImportBatch {
    let text = format!("{HEADER}{configs}");
    let files = [
        ("project.csv", common::PROJECT_CSV),
        ("parts.csv", PARTS),
        ("devices.csv", DEVICES),
        ("assembly_configs.csv", text.as_str()),
    ];
    common::import(&common::inputs(&files)).expect("the import succeeds")
}

fn errors(b: &ImportBatch, code: u16) -> Vec<&str> {
    b.report
        .with_code(Code::imp(code))
        .inspect(|e| assert_eq!(e.severity, Severity::Error))
        .map(|e| e.message.as_str())
        .collect()
}

/// Descendants at any depth, the head itself and a row without part, label or covers are all fine; the configs
/// are written in `n` order whatever the row order.
#[test]
fn rows_are_sorted_by_n_and_covers_may_reach_any_depth() {
    let b = run("a:2,-1U1,2,,OC2,,\na:1,-1U1,1,Order 1,OC1,OC1,-1U1;-1U4;-1U3\n");
    assert!(b.report.entries.is_empty(), "{:#?}", b.report.entries);
    let a = aspect(&b, "-1U1").expect("the aspect");
    assert_eq!(a.configs.len(), 2);
    assert_eq!(a.configs[0].order_code.as_str(), "OC1");
    assert_eq!(a.configs[0].label.as_neutral(), Some("Order 1"));
    assert_eq!(a.configs[0].covers.len(), 3);
    assert_eq!(a.configs[1].order_code.as_str(), "OC2");
    assert!(a.configs[1].label.is_empty());
    assert_eq!(a.configs[1].part, None);
    assert!(a.configs[1].covers.is_empty());
    assert!(aspect(&b, "-1U2").is_none());
}

/// An unknown assembly, part or covered device is EW-IMP-003; the row is dropped and the aspect stays out.
#[test]
fn an_unknown_key_drops_the_row() {
    for (row, what) in [
        (
            "a:1,-9U9,1,,OC1,OC1,",
            "unknown device `-9U9` in column `assembly`",
        ),
        ("a:1,-1U1,1,,OC1,NOPE,", "unknown part `NOPE` in column `part`"),
        (
            "a:1,-1U1,1,,OC1,OC1,-1U1;-9U9",
            "unknown device `-9U9` in column `covers`",
        ),
    ] {
        let b = run(&format!("{row}\n"));
        let found = errors(&b, 3);
        assert_eq!(found.len(), 1, "{row}: {:#?}", b.report.entries);
        assert!(
            found[0].starts_with("row dropped: ") && found[0].contains(what),
            "{row}: {found:?}"
        );
        assert!(aspect(&b, "-1U1").is_none(), "{row}");
        assert_eq!(b.report.stats.dropped, 1, "{row}");
    }
}

/// A repeated `n` of one assembly drops the later row, and the same `n` of another assembly is fine.
#[test]
fn a_repeated_n_drops_the_later_row() {
    let b = run("a:1,-1U1,1,,OC1,OC1,\na:2,-1U1,1,,OC2,OC2,\na:3,-1U2,1,,OC2,OC2,\n");
    let found = errors(&b, 2);
    assert_eq!(found.len(), 1, "{:#?}", b.report.entries);
    assert!(
        found[0].contains("already has an order code with n = 1"),
        "{found:?}"
    );
    let a = aspect(&b, "-1U1").expect("the aspect");
    assert_eq!(a.configs.len(), 1);
    assert_eq!(a.configs[0].order_code.as_str(), "OC1");
    assert_eq!(aspect(&b, "-1U2").expect("the other assembly").configs.len(), 1);
}

/// A covered device that is neither the assembly nor one of its descendants is EW-IMP-002; so is a device
/// listed twice. Nothing is repaired.
#[test]
fn a_covered_device_outside_the_assembly_drops_the_row() {
    // -1U3 is a sibling of -1U2, the parent -1U1 is above it, -2U1 is another tree.
    for covers in ["-1U3", "-1U1", "-2U1", "-1U4;-1U3"] {
        let b = run(&format!("a:1,-1U2,1,,OC1,OC1,{covers}\n"));
        let found = errors(&b, 2);
        assert_eq!(found.len(), 1, "{covers}: {:#?}", b.report.entries);
        assert!(
            found[0].contains("neither `-1U2` nor one of its descendants"),
            "{covers}: {found:?}"
        );
        assert!(aspect(&b, "-1U2").is_none(), "{covers}");
    }
    let b = run("a:1,-1U1,1,,OC1,OC1,-1U2;-1U2\n");
    let found = errors(&b, 2);
    assert_eq!(found.len(), 1, "{:#?}", b.report.entries);
    assert!(found[0].contains("`-1U2` is listed twice"), "{found:?}");
    assert!(aspect(&b, "-1U1").is_none());
}
