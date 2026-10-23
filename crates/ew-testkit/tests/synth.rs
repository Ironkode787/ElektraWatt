// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Acceptance tests of the synthetic fixture generator: counts, loadability, determinism and the
//! structural properties the benchmarks rely on.

use std::collections::{BTreeMap, BTreeSet};

use ew_api::{Session, SessionOptions};
use ew_commands::commands::{Connect, EndpointParam, MoveReps};
use ew_ids::{Id, Uuid};
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, FnDef, FnRole, Function, FunctionKind, InternalKind,
    IpDirection, Kind, LibDocument, Page, PotentialType, Record, Representation, SymbolDef,
};
use ew_testkit::synth::topology::{ConnPlan, ConnRole, DeviceRole, FnRolePlan};
use ew_testkit::synth::{self, Counts, Generated, Options, Plan, SizeClass};
use ew_units::{Nm, Pt2};

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

// ---------------------------------------------------------------------------------------------------------
// Signal wiring, routes and options (F9)

/// The signal wires of a generated project, in plan order, with the plan connection each one is.
fn signal_wires<'a>(g: &'a Generated, plan: &'a Plan) -> Vec<(&'a Connection, &'a ConnPlan)> {
    let connections = g.records.iter().filter_map(|r| match r {
        Record::Connection(c) => Some(c),
        _ => None,
    });
    connections
        .zip(&plan.connections)
        .filter(|(_, p)| p.role == ConnRole::Signal)
        .collect()
}

/// The device of every function (`None` for interruption points).
fn function_devices(g: &Generated) -> BTreeMap<Id<Function>, Option<Id<Device>>> {
    g.records
        .iter()
        .filter_map(|r| match r {
            Record::Function(f) => Some((f.id, f.device)),
            _ => None,
        })
        .collect()
}

fn cp_of(e: &Endpoint) -> (Id<Function>, u16) {
    match e {
        Endpoint::Cp(f, i) => (*f, *i),
        other => panic!("a signal wire ends at a CP, not at {other:?}"),
    }
}

#[test]
fn signal_wires_never_join_one_function_or_device() {
    for class in [SizeClass::S, SizeClass::M] {
        let g = synth::project(class, 1);
        let plan = Plan::build(class);
        let devices = function_devices(&g);
        let wires = signal_wires(&g, &plan);
        assert!(wires.len() > 500, "{} signal wires", wires.len());
        for (c, _) in wires {
            let (fa, fb) = (cp_of(&c.a).0, cp_of(&c.b).0);
            assert_ne!(fa, fb, "wire {} joins a function with itself", c.id);
            let (da, db) = (devices[&fa], devices[&fb]);
            assert!(da.is_some() && db.is_some());
            assert_ne!(da, db, "wire {} joins a device with itself", c.id);
        }
    }
}

/// Union-find over the CPs of the signal wires, with the internal `Short` links of the function definitions
/// (a terminal's sides `a` and `b`) applied to every CP a wire reaches.
struct Nets {
    index: BTreeMap<(Id<Function>, u16), usize>,
    parent: Vec<usize>,
}

impl Nets {
    fn node(&mut self, cp: (Id<Function>, u16)) -> usize {
        let next = self.parent.len();
        let n = *self.index.entry(cp).or_insert(next);
        if n == next {
            self.parent.push(next);
        }
        n
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        self.parent[a] = b;
    }
}

/// The internal `Short` links `(a, b)` of a function definition.
type ShortLinks = Vec<(u16, u16)>;

/// Per function: the category (third level) of its definition and the short links of it.
struct DefInfo {
    category: BTreeMap<Id<Function>, String>,
    shorts: BTreeMap<Id<Function>, ShortLinks>,
}

fn def_info(g: &Generated) -> DefInfo {
    let mut by_def: BTreeMap<Id<FnDef>, (String, ShortLinks)> = BTreeMap::new();
    for d in &g.lib {
        if let LibDocument::Fndef(c) = d {
            let shorts = c
                .item
                .internal
                .iter()
                .filter(|l| matches!(l.kind, InternalKind::Short))
                .map(|l| (l.a, l.b))
                .collect();
            by_def.insert(c.id, (c.item.category.2.as_str().to_owned(), shorts));
        }
    }
    let mut info = DefInfo {
        category: BTreeMap::new(),
        shorts: BTreeMap::new(),
    };
    for r in &g.records {
        if let Record::Function(f) = r {
            let (c, s) = &by_def[&f.def];
            info.category.insert(f.id, c.clone());
            info.shorts.insert(f.id, s.clone());
        }
    }
    info
}

#[test]
fn signal_nets_chain_channels_through_terminals() {
    for class in [SizeClass::S, SizeClass::M] {
        let g = synth::project(class, 1);
        let plan = Plan::build(class);
        let DefInfo { category, shorts } = def_info(&g);
        let page_section: BTreeMap<Id<Page>, Id<ew_schema::Section>> = g
            .records
            .iter()
            .filter_map(|r| match r {
                Record::Page(p) => Some((p.id, p.folio.section.expect("generated pages have sections"))),
                _ => None,
            })
            .collect();
        let wires = signal_wires(&g, &plan);

        let mut nets = Nets {
            index: BTreeMap::new(),
            parent: Vec::new(),
        };
        let mut section_of: BTreeMap<usize, Id<ew_schema::Section>> = BTreeMap::new();
        let mut channel_terminal: BTreeMap<Id<ew_schema::Section>, usize> = BTreeMap::new();
        let mut reached = Vec::new();
        for (c, _) in &wires {
            let (a, b) = (cp_of(&c.a), cp_of(&c.b));
            let section = page_section[&c.page.expect("wires are drawn on pages")];
            let (na, nb) = (nets.node(a), nets.node(b));
            nets.union(na, nb);
            section_of.insert(na, section);
            section_of.insert(nb, section);
            reached.extend([a, b]);
            let kinds = [category[&a.0].as_str(), category[&b.0].as_str()];
            if kinds.contains(&"module") && kinds.contains(&"terminal") {
                *channel_terminal.entry(section).or_default() += 1;
            }
        }
        // Internal short links of the functions a wire reaches.
        for (f, i) in reached {
            for &(x, y) in &shorts[&f] {
                if i == x || i == y {
                    let (nx, ny) = (nets.node((f, x)), nets.node((f, y)));
                    nets.union(nx, ny);
                    let section = section_of[&nets.index[&(f, i)]];
                    section_of.entry(nx).or_insert(section);
                    section_of.entry(ny).or_insert(section);
                }
            }
        }
        let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
        for n in 0..nets.parent.len() {
            let root = nets.find(n);
            *sizes.entry(root).or_default() += 1;
        }
        let mut big_per_section: BTreeMap<Id<ew_schema::Section>, usize> = BTreeMap::new();
        for (&root, &size) in &sizes {
            if size >= 3 {
                *big_per_section.entry(section_of[&root]).or_default() += 1;
            }
        }
        let replicas = plan.replicas.len();
        assert_eq!(
            channel_terminal.len(),
            replicas,
            "{class:?}: replicas with a channel-terminal wire"
        );
        assert_eq!(
            big_per_section.len(),
            replicas,
            "{class:?}: replicas with a net of 3+ CPs"
        );
        assert!(
            sizes.values().all(|&s| (2..=4).contains(&s)),
            "{class:?}: net sizes {:?}",
            sizes.values().collect::<BTreeSet<_>>()
        );
        assert!(sizes.values().any(|&s| s == 4), "{class:?}: no net of four CPs");
    }
}

/// The page position of every drawn function CP, computed from the representations and the embedded symbols
/// like the model does: the representation position plus the oriented symbol CP position.
fn cp_positions(g: &Generated) -> BTreeMap<(Id<Function>, u16), Pt2> {
    let symbols: BTreeMap<Id<SymbolDef>, Vec<(u16, Pt2)>> = g
        .lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Symbol(c) => Some((
                c.id,
                c.item
                    .cps
                    .as_slice()
                    .iter()
                    .map(|cp| (cp.fn_cp, cp.pos))
                    .collect(),
            )),
            _ => None,
        })
        .collect();
    let mut out = BTreeMap::new();
    for r in &g.records {
        if let Record::Representation(rep) = r {
            for &(fn_cp, pos) in &symbols[&rep.symbol] {
                let off = rep.orient.apply(pos);
                out.insert(
                    (rep.functions[0], fn_cp),
                    Pt2::new(Nm(rep.pos.x.0 + off.x.0), Nm(rep.pos.y.0 + off.y.0)),
                );
            }
        }
    }
    out
}

#[test]
fn routes_start_and_end_at_the_cps_on_the_grid() {
    let g = synth::project(SizeClass::S, 1);
    let plan = Plan::build(SizeClass::S);
    let cps = cp_positions(&g);
    let grid = g.handles.grid;
    let on_grid = |x: Nm| grid.snap_point(x) == Some(x);
    let signals = signal_wires(&g, &plan);
    let mut routed = 0;
    for (c, p) in &signals {
        assert_eq!(!c.route.is_empty(), p.routed, "wire {}", c.id);
        if c.route.is_empty() {
            continue;
        }
        routed += 1;
        assert!((2..=3).contains(&c.route.len()), "wire {}: {:?}", c.id, c.route);
        assert_eq!(c.route[0], cps[&cp_of(&c.a)], "wire {}: first vertex", c.id);
        assert_eq!(
            c.route[c.route.len() - 1],
            cps[&cp_of(&c.b)],
            "wire {}: last vertex",
            c.id
        );
        for v in &c.route {
            assert!(
                on_grid(v.x) && on_grid(v.y),
                "wire {}: {v:?} is off the grid",
                c.id
            );
        }
        for w in c.route.windows(2) {
            assert!(
                w[0].x == w[1].x || w[0].y == w[1].y,
                "wire {}: diagonal segment",
                c.id
            );
        }
    }
    // Every wire that ends at a load main function is routed; the fraction is at least a quarter.
    let fn_ids: Vec<Id<Function>> = g
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Function(f) => Some(f.id),
            _ => None,
        })
        .collect();
    let load_main: BTreeSet<Id<Function>> = plan
        .functions
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            f.role == FnRolePlan::Main && f.device.is_some_and(|d| plan.devices[d].role == DeviceRole::Load)
        })
        .map(|(i, _)| fn_ids[i])
        .collect();
    assert!(!load_main.is_empty());
    for (c, _) in &signals {
        if load_main.contains(&cp_of(&c.a).0) || load_main.contains(&cp_of(&c.b).0) {
            assert!(
                !c.route.is_empty(),
                "wire {} at a load main function has no route",
                c.id
            );
        }
    }
    assert!(
        routed * 4 >= signals.len(),
        "{routed} routed of {}",
        signals.len()
    );
    assert!(routed < signals.len(), "some signal wires stay unrouted");
    // Rails and bus cores have no route.
    let all_routed = g
        .records
        .iter()
        .filter(|r| matches!(r, Record::Connection(c) if !c.route.is_empty()))
        .count();
    assert_eq!(all_routed, routed);
}

#[test]
fn without_routes_only_the_routes_differ() {
    let with = synth::project(SizeClass::S, 1);
    let without = synth::project_with(SizeClass::S, 1, &Options { routes: false });
    assert_eq!(Options::default(), Options { routes: true });
    let routes = |g: &Generated| {
        g.records
            .iter()
            .filter(|r| matches!(r, Record::Connection(c) if !c.route.is_empty()))
            .count()
    };
    assert!(routes(&with) > 0);
    assert_eq!(routes(&without), 0);
    assert_eq!(with.project, without.project);
    assert_eq!(with.lib, without.lib);
    assert_eq!(with.counts, without.counts);
    assert_eq!(with.records.len(), without.records.len());
    let stripped: Vec<Record> = with
        .records
        .iter()
        .cloned()
        .map(|r| match r {
            Record::Connection(mut c) => {
                c.route.clear();
                Record::Connection(c)
            }
            other => other,
        })
        .collect();
    assert_eq!(stripped, without.records);
}

#[test]
fn generation_is_deterministic() {
    for class in [SizeClass::S, SizeClass::M] {
        assert_eq!(Plan::build(class), Plan::build(class));
    }
    let plan = Plan::build(SizeClass::S);
    assert_eq!(json(&synth::emit(&plan, 5)), json(&synth::emit(&plan, 5)));
    assert_eq!(
        json(&synth::emit(&plan, 5)),
        json(&synth::project(SizeClass::S, 5))
    );
    let off = Options { routes: false };
    assert_eq!(
        json(&synth::emit_with(&plan, 5, &off)),
        json(&synth::emit_with(&plan, 5, &off))
    );
}

/// `rep.move` of a load representation updates exactly the routes of its routed wires and is never refused
/// because of them: `MoveReps` moves the route end at the moved CP (and keeps the neighbouring segment
/// orthogonal), it does not reject.
#[test]
fn moving_a_load_updates_the_routes_of_its_wires() {
    let g = synth::project(SizeClass::S, 1);
    let plan = Plan::build(SizeClass::S);
    let grid = g.handles.grid;
    let load_reps = g.handles.load_reps.clone();
    let rep_function: BTreeMap<Id<Representation>, Id<Function>> = g
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Representation(rep) => Some((rep.id, rep.functions[0])),
            _ => None,
        })
        .collect();
    let routed_at: BTreeMap<Id<Function>, BTreeSet<Uuid>> = {
        let mut m: BTreeMap<Id<Function>, BTreeSet<Uuid>> = BTreeMap::new();
        for (c, _) in signal_wires(&g, &plan) {
            if !c.route.is_empty() {
                for e in [&c.a, &c.b] {
                    m.entry(cp_of(e).0).or_default().insert(c.id.as_uuid());
                }
            }
        }
        m
    };
    let (project, records, lib) = g.into_parts();
    let mut session = Session::from_records(project, records, lib, SessionOptions::default()).expect("loads");
    let delta = Pt2::new(grid.at(6).expect("on grid"), grid.at(3).expect("on grid"));
    let mut with_routes = 0;
    for rep in load_reps.iter().take(30) {
        let cmd = MoveReps {
            steps: None,
            reps: vec![*rep],
            delta,
            page: None,
        };
        let preview = session.preview(&cmd).expect("the move is accepted");
        let changed_routes: BTreeSet<Uuid> = preview
            .diff()
            .changes
            .iter()
            .filter(|c| c.key.0 == Kind::Connection && c.fields.iter().any(|f| f == "route"))
            .map(|c| c.key.1.as_uuid())
            .collect();
        let expected = routed_at.get(&rep_function[rep]).cloned().unwrap_or_default();
        assert_eq!(changed_routes, expected, "routes updated by moving {rep}");
        if !expected.is_empty() {
            with_routes += 1;
        }
    }
    assert!(
        with_routes > 5,
        "only {with_routes} of the moved loads have routed wires"
    );
}

/// Consecutive free CPs of a page take a wire (what `connection.draw` of the benchmark does).
#[test]
fn consecutive_free_cps_take_a_wire() {
    let g = synth::project(SizeClass::S, 1);
    let free = g.handles.free_cps.clone();
    let pages = g.handles.pages.clone();
    let (project, records, lib) = g.into_parts();
    let mut session = Session::from_records(project, records, lib, SessionOptions::default()).expect("loads");
    let end = |(function, idx): (Id<Function>, u16)| EndpointParam::Cp { function, idx };
    let mut drawn = 0;
    for (page, cps) in pages.iter().zip(&free).take(3) {
        for w in cps.windows(2).take(12) {
            let cmd = Connect {
                a: end(w[0]),
                b: end(w[1]),
                page: Some(*page),
                route: Vec::new(),
                kind: Default::default(),
            };
            session.preview(&cmd).expect("the wire is accepted");
            drawn += 1;
        }
    }
    assert!(drawn >= 30, "{drawn} wires previewed");
}

#[test]
fn l_and_xl_plans_match_the_class_bounds() {
    // Plan-only: index arithmetic, cheap. The L/XL records are generated by the ignored test below.
    for class in [SizeClass::L, SizeClass::XL] {
        let b = class.bounds();
        let c = Plan::build(class).counts();
        assert_eq!(c.pages, b.pages);
        assert_eq!(c.devices, b.devices);
        assert_eq!(c.functions, b.functions);
        assert_eq!(c.connections, b.connections);
        assert_eq!(c.terminals, b.terminals);
        assert_eq!(c.channels, b.channels);
        assert!(c.cps <= b.cps && c.cps * 100 >= b.cps * 95, "{} CPs", c.cps);
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
