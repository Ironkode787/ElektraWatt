// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A copy of pages reproduces what the project holds (doc 06 §3.6): a small project of its own, the supply page with
//! its strip and two of the strip's terminals both designated 12 (a state a lenient import leaves, which the project
//! opens with and EW-TERM-002 reports), is copied as it is and the check reports the copy's strip too; a frozen strip
//! tag keeps where the copy stands in a structure node of its own and is cleared where it would print its original's
//! designation; a name of interruption points that are targets only, with no source in the project (a supply coming
//! from outside the drawing), continues.

use crate::common;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{AnyCommand, CmdError, CopyFolios, PageCopySpec, Session};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, Record};
use serde_json::json;

/// `page.copy` (a Bulk command) previewed and committed.
fn run(s: &mut Session, cmd: &AnyCommand) -> Result<(), CmdError> {
    let preview = s.preview(cmd)?;
    s.commit(preview).map(|_| ())
}

/// The small project: the sample's supply page 101 alone with its section and location, its strip and the strip's
/// terminals L1 and L2, both designated 12; with `frozen`, the strip's tag frozen as X5. Returns the session, the page,
/// the strip and the location node.
fn small(frozen: bool) -> (Session, Uuid, Uuid, Uuid) {
    let (project, records, lib) = common::sample_records();
    let page = records
        .iter()
        .find_map(|r| match r {
            Record::Page(p) if p.folio.number == 101 => Some(p.clone()),
            _ => None,
        })
        .expect("page 101");
    let terminals: Vec<Device> = records
        .iter()
        .filter_map(|r| match r {
            Record::Device(d)
                if d.aspects
                    .terminal
                    .as_ref()
                    .is_some_and(|t| ["L1", "L2"].contains(&t.designation.as_str())) =>
            {
                Some(d.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(terminals.len(), 2, "the fixture: -X1011's terminals L1 and L2");
    let strip = terminals[0].aspects.terminal.as_ref().unwrap().strip;
    let location = match page.structure.location {
        Some(ew_schema::Slot::Node(n)) => n,
        _ => panic!("page 101 stands in a location"),
    };
    let devices: Vec<Id<Device>> = terminals.iter().map(|d| d.id).chain([strip]).collect();
    let functions: Vec<Uuid> = records
        .iter()
        .filter_map(|r| match r {
            Record::Function(f) if f.device.is_some_and(|d| devices.contains(&d)) => Some(f.id.as_uuid()),
            _ => None,
        })
        .collect();
    let kept: Vec<Record> = records
        .into_iter()
        .filter_map(|r| match r {
            Record::Section(s) if Some(s.id) == page.folio.section => Some(Record::Section(s)),
            Record::StructureNode(n) if n.id == location => Some(Record::StructureNode(n)),
            Record::Page(p) if p.id == page.id => Some(Record::Page(p)),
            Record::Device(mut d) if devices.contains(&d.id) => {
                if let Some(t) = &mut d.aspects.terminal {
                    t.designation = "12".parse().unwrap();
                }
                if frozen && d.id == strip {
                    d.tag.frozen = Some(serde_json::from_value(json!({"text": "X5"})).unwrap());
                }
                Some(Record::Device(d))
            }
            Record::Function(f) if functions.contains(&f.id.as_uuid()) => Some(Record::Function(f)),
            Record::Representation(rep)
                if rep.page == page.id && rep.functions.iter().all(|f| functions.contains(&f.as_uuid())) =>
            {
                Some(Record::Representation(rep))
            }
            _ => None,
        })
        .collect();
    let s = common::session_of(project, kept, lib);
    (s, page.id.as_uuid(), strip.as_uuid(), location.as_uuid())
}

fn spec(page: Uuid, structure: Vec<(Uuid, String)>) -> PageCopySpec {
    PageCopySpec {
        pages: vec![page],
        folios: CopyFolios::Offset(10),
        structure,
        rename: Vec::new(),
    }
}

/// The strips and their terminals' designations.
fn strips(s: &Session) -> Vec<(Id<Device>, Vec<String>)> {
    let q = s.query();
    q.devices()
        .iter()
        .filter(|(_, d)| d.aspects.strip.is_some())
        .map(|(id, _)| {
            let designations = q
                .idx()
                .terminals_of_strip(id)
                .filter_map(|t| {
                    Some(
                        q.devices()
                            .get(t)?
                            .aspects
                            .terminal
                            .as_ref()?
                            .designation
                            .to_string(),
                    )
                })
                .collect();
            (id, designations)
        })
        .collect()
}

/// A: two terminals of one designation copy as the source strip holds them, and EW-TERM-002 reports both strips; the
/// commit validator judged the copy's two new terminals a duplicate the batch makes ("designation 12 is taken …; use
/// 26") and refused it.
#[test]
fn a_strip_with_two_terminals_of_one_designation_copies_as_it_is() {
    let (mut s, page, strip, _) = small(false);
    let plan = s.copy_pages(&spec(page, Vec::new())).expect("composed");
    run(&mut s, &plan.command).expect("the copy commits");
    let strips = strips(&s);
    assert_eq!(strips.len(), 2, "the strip and its copy");
    let copy = strips
        .iter()
        .find(|(id, _)| id.as_uuid() != strip)
        .expect("the copy");
    assert_eq!(copy.1, ["12", "12"], "the copy's terminals as the source's");
    let duplicates = s
        .check(&PRE_DELIVERY, CheckMode::Online)
        .findings
        .iter()
        .filter(|f| f.rule == "EW-TERM-002" && f.is_open())
        .count();
    assert_eq!(duplicates, 2, "reported in the strip and in its copy");
}

/// The rules a copy may reproduce are shipped prevent rules.
#[test]
fn the_reproduced_rules_are_prevent_rules() {
    for id in ew_commands::commands::REPRODUCED {
        assert!(
            ew_api::checks::rules()
                .iter()
                .any(|r| r.meta().id == id && r.meta().modes.prevent),
            "{id} is a shipped prevent rule"
        );
    }
}

/// C: a frozen strip tag is cleared where the copy would print its original's designation, and kept where the copy
/// stands in a location of its own; the card counts only the cleared one.
#[test]
fn a_frozen_tag_keeps_in_a_node_of_its_own_and_is_cleared_without_one() {
    let frozen_of = |s: &Session, original: Uuid| -> Option<String> {
        let q = s.query();
        let (_, copy) = q
            .devices()
            .iter()
            .find(|(id, d)| d.aspects.strip.is_some() && id.as_uuid() != original)
            .map(|(id, d)| (id, d.clone()))
            .expect("the copied strip");
        copy.tag.frozen.as_ref().map(|f| f.text.to_string())
    };
    let (mut s, page, strip, _) = small(true);
    let plan = s.copy_pages(&spec(page, Vec::new())).expect("composed");
    assert_eq!(plan.cleared_frozen, ["X5"]);
    run(&mut s, &plan.command).expect("the copy commits");
    assert_eq!(
        frozen_of(&s, strip),
        None,
        "the copy's -X5 would be its original's: cleared"
    );
    let (mut s, page, strip, location) = small(true);
    let plan = s
        .copy_pages(&spec(page, vec![(location, "CAB2".to_owned())]))
        .expect("composed");
    assert!(plan.cleared_frozen.is_empty(), "{:?}", plan.cleared_frozen);
    run(&mut s, &plan.command).expect("the copy commits");
    assert_eq!(
        frozen_of(&s, strip).as_deref(),
        Some("X5"),
        "+CAB2-X5 differs from +CAB-X5: kept"
    );
}

/// B: the sample's 1L1 made a target on 101 as on 103, so the name has no source in the project: copying 101 to 103
/// with the other supply names renamed continues 1L1 (it was refused as joining its original), and the card names it.
#[test]
fn an_incoming_supply_of_targets_only_continues() {
    let (project, records, lib) = common::sample_records();
    let mut s = common::session_of(project, records, lib);
    let source = {
        let q = s.query();
        q.functions()
            .iter()
            .find(|(_, f)| {
                matches!(&f.kind, ew_schema::FunctionKind::Ip(ip)
                    if ip.name.as_str() == "1L1" && ip.direction == ew_schema::IpDirection::Source)
            })
            .map(|(id, _)| id.as_uuid())
            .expect("1L1's source on 101")
    };
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "function", "targets": [source.to_string()], "path": "kind.ip.direction",
            "value": "target"}),
    })
    .expect("1L1 becomes a target");
    let page = |folio: &str| -> Uuid { s.page_tables(folio).expect("a page").page };
    let rename = [
        ("1L2", "2L2"),
        ("1L3", "2L3"),
        ("24V", "24V.2"),
        ("0V", "0V.2"),
        ("PE", "PE.2"),
    ]
    .map(|(a, b)| (a.to_owned(), b.to_owned()))
    .to_vec();
    let spec = PageCopySpec {
        pages: vec![page("101"), page("102"), page("103")],
        folios: CopyFolios::Offset(10),
        structure: Vec::new(),
        rename,
    };
    let plan = s.copy_pages(&spec).expect("1L1 continues");
    assert_eq!(plan.continued, ["1L1"]);
    run(&mut s, &plan.command).expect("the copy commits");
}
