// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Acceptance tests of the synthetic fixture generator: counts, loadability, determinism and the
//! structural properties the benchmarks rely on.

use std::collections::{BTreeMap, BTreeSet};

use ew_api::{Session, SessionOptions};
use ew_ids::{Id, Uuid};
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, FnDef, FnRole, Function, FunctionKind, IpDirection,
    LibDocument, Page, PotentialType, Record,
};
use ew_testkit::synth::{self, Counts, Generated, Plan, SizeClass};

/// The counts a generated project has, recomputed from its records and library.
struct Recount {
    counts: Counts,
    reps: usize,
    junctions: usize,
}

fn recount(g: &Generated) -> Recount {
    let cp_counts: BTreeMap<Id<FnDef>, (usize, bool)> = g
        .lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Fndef(c) => Some((c.id, (c.item.cps.len(), c.item.category.2.as_str() == "module"))),
            _ => None,
        })
        .collect();
    let (mut pages, mut devices, mut functions, mut connections) = (0, 0, 0, 0);
    let (mut terminals, mut cps, mut channels, mut reps, mut junctions) = (0, 0, 0, 0, 0);
    for r in &g.records {
        match r {
            Record::Page(_) => pages += 1,
            Record::Device(d) => {
                devices += 1;
                if d.aspects.terminal.is_some() {
                    terminals += 1;
                }
            }
            Record::Function(f) => {
                functions += 1;
                let (n, module) = cp_counts[&f.def];
                cps += n;
                if module {
                    channels += n - 2;
                }
            }
            Record::Connection(_) => connections += 1,
            Record::Representation(_) => reps += 1,
            Record::Junction(_) => junctions += 1,
            _ => {}
        }
    }
    Recount {
        counts: Counts {
            pages,
            devices,
            functions,
            connections,
            cps,
            terminals,
            channels,
        },
        reps,
        junctions,
    }
}

#[test]
fn s_counts_are_the_documented_ones() {
    let g = synth::project(SizeClass::S, 1);
    let documented = Counts {
        pages: 50,
        devices: 300,
        functions: 900,
        connections: 1_500,
        cps: 3_637,
        terminals: 120,
        channels: 60,
    };
    assert_eq!(g.counts, documented);
    let again = recount(&g);
    assert_eq!(again.counts, documented);
    assert_eq!(again.reps, 900);
    assert_eq!(again.junctions, 348);
    assert_eq!(g.records.len() + 1, 4_024);
}

#[test]
fn m_counts_are_the_documented_ones() {
    let g = synth::project(SizeClass::M, 1);
    let documented = Counts {
        pages: 300,
        devices: 2_500,
        functions: 7_500,
        connections: 15_000,
        cps: 37_178,
        terminals: 1_000,
        channels: 500,
    };
    assert_eq!(g.counts, documented);
    let again = recount(&g);
    assert_eq!(again.counts, documented);
    assert_eq!(again.reps, 7_500);
    assert_eq!(again.junctions, 2_960);
    assert_eq!(g.records.len() + 1, 35_884);
}

#[test]
fn s_loads_through_the_session() {
    let (project, records, lib) = synth::project(SizeClass::S, 1).into_parts();
    let session = Session::from_records(project, records, lib, SessionOptions::default());
    assert!(session.is_ok(), "{:?}", session.err());
}

#[test]
fn m_builds_a_model() {
    let _ = synth::model(SizeClass::M, 7);
}

fn json(g: &Generated) -> String {
    serde_json::to_string(&(&g.project, &g.records, &g.lib)).expect("serialisable")
}

#[test]
fn same_seed_same_records() {
    assert_eq!(
        json(&synth::project(SizeClass::S, 3)),
        json(&synth::project(SizeClass::S, 3))
    );
}

#[test]
fn different_seed_different_ids_same_counts() {
    let a = synth::project(SizeClass::S, 3);
    let b = synth::project(SizeClass::S, 4);
    assert_eq!(a.counts, b.counts);
    assert_eq!(a.records.len(), b.records.len());
    assert_ne!(json(&a), json(&b));
    assert_ne!(a.project.id, b.project.id);
}

/// The page of every function's representation (a function has at most one representation here).
fn function_pages(g: &Generated) -> BTreeMap<Id<Function>, Id<Page>> {
    g.records
        .iter()
        .filter_map(|r| match r {
            Record::Representation(rep) => Some((rep.functions[0], rep.page)),
            _ => None,
        })
        .collect()
}

#[test]
fn page_based_tags_are_unique() {
    let g = synth::project(SizeClass::S, 1);
    // Devices are emitted in plan order; a cable with no drawn core falls back to its planned page.
    let plan = Plan::build(SizeClass::S);
    let fn_pages = function_pages(&g);
    let main_fn_of: BTreeMap<Id<Device>, Id<Function>> = g
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Function(f) if f.role == FnRole::Main => f.device.map(|d| (d, f.id)),
            _ => None,
        })
        .collect();
    // A cable has no function: its page is the page its core connections are drawn on.
    let conductor_cable: BTreeMap<_, _> = g
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Conductor(c) => Some((c.id, c.cable)),
            _ => None,
        })
        .collect();
    let mut cable_page: BTreeMap<Id<Device>, Id<Page>> = BTreeMap::new();
    for r in &g.records {
        if let Record::Connection(Connection {
            kind: ConnectionKind::Core(c),
            page: Some(p),
            ..
        }) = r
        {
            cable_page.entry(conductor_cable[c]).or_insert(*p);
        }
    }

    let mut seen: BTreeSet<(Id<Page>, String, u32)> = BTreeSet::new();
    let mut checked = 0;
    let devices = g.records.iter().filter_map(|r| match r {
        Record::Device(d) => Some(d),
        _ => None,
    });
    for (ix, d) in devices.enumerate() {
        if d.aspects.terminal.is_some() {
            continue;
        }
        let page = main_fn_of
            .get(&d.id)
            .and_then(|f| fn_pages.get(f))
            .or_else(|| cable_page.get(&d.id))
            .unwrap_or_else(|| &g.handles.pages[plan.devices[ix].page]);
        let counter = d.tag.counter.expect("generated tags carry a counter");
        assert!(
            seen.insert((*page, d.tag.letter.as_str().to_owned(), counter)),
            "duplicate page-based tag {}{counter}",
            d.tag.letter
        );
        checked += 1;
    }
    assert!(checked > 100, "only {checked} devices checked");
}

#[test]
fn interruption_points_pair() {
    let g = synth::project(SizeClass::S, 1);
    let mut by_name: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for r in &g.records {
        if let Record::Function(Function {
            kind: FunctionKind::Ip(ip),
            ..
        }) = r
        {
            let e = by_name.entry(ip.name.as_str().to_owned()).or_default();
            match ip.direction {
                IpDirection::Source => e.0 += 1,
                IpDirection::Target => e.1 += 1,
                _ => panic!("generated IPs are directed"),
            }
        }
    }
    assert!(!by_name.is_empty());
    for (name, counts) in by_name {
        assert_eq!(counts, (1, 1), "IP {name}");
    }
}

#[test]
fn potentials_are_defined() {
    let g = synth::project(SizeClass::S, 1);
    let mut names: BTreeMap<String, (PotentialType, usize)> = BTreeMap::new();
    for r in &g.records {
        if let Record::PotentialDef(p) = r {
            let (_, n) = names
                .entry(p.name.as_str().to_owned())
                .or_insert((p.ptype.clone().expect("typed"), 0));
            *n += 1;
        }
    }
    let replicas = Plan::build(SizeClass::S).replicas.len();
    for fixed in ["L", "N", "PE"] {
        assert_eq!(names[fixed].1, 1, "{fixed}");
    }
    for u in 1..=replicas {
        assert_eq!(names[&format!("24V-{u}")].1, 1, "24V-{u}");
        assert_eq!(names[&format!("0V-{u}")].1, 1, "0V-{u}");
    }
    assert_eq!(names.len(), 3 + 2 * replicas);
}

#[test]
fn handles_address_generated_records() {
    let g = synth::project(SizeClass::S, 1);
    let plan = Plan::build(SizeClass::S);
    assert_eq!(g.handles.pages.len(), plan.pages.len());
    assert_eq!(g.handles.load_reps.len(), plan.load_reps.len());
    assert_eq!(g.handles.free_cps.len(), plan.pages.len());
    let reps: BTreeSet<Uuid> = g
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Representation(rep) => Some(rep.id.as_uuid()),
            _ => None,
        })
        .collect();
    assert!(g.handles.load_reps.iter().all(|r| reps.contains(&r.as_uuid())));
    // Every free CP is on a function drawn on its page and used by no connection.
    let fn_pages = function_pages(&g);
    let used: BTreeSet<(Id<Function>, u16)> = g
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Connection(c) => Some([c.a.clone(), c.b.clone()]),
            _ => None,
        })
        .flatten()
        .filter_map(|e| match e {
            Endpoint::Cp(f, i) => Some((f, i)),
            _ => None,
        })
        .collect();
    for (page, free) in g.handles.pages.iter().zip(&g.handles.free_cps) {
        for cp in free {
            assert_eq!(fn_pages[&cp.0], *page);
            assert!(!used.contains(cp));
        }
    }
}

#[test]
#[ignore = "L/XL fixtures are generated with the performance spike (milestone rule)"]
fn l_and_xl_generate() {
    for class in [SizeClass::L, SizeClass::XL] {
        let g = synth::project(class, 1);
        assert_eq!(g.counts, Plan::build(class).counts());
        assert_eq!(recount(&g).counts, g.counts);
    }
}
