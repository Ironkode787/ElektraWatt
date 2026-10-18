// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The topology of a generated project: a plan of pages, devices, functions, representations, junctions,
//! connections, conductors and potentials as plain indices, with no IDs and no records.
//!
//! The plan is pure index arithmetic, deterministic for a class (the seed only affects the IDs, which the
//! emitter mints in plan order). [`Plan::build`] replicates a Golden-B-shaped cabinet ("replica", 18 drawn
//! folios) until the page bound of the class is reached, the last replica possibly shorter, and then densifies
//! the replicas to the device, function, connection, terminal and channel bounds of NFR §2.1 exactly, and to
//! the CP bound from below.
//!
//! # One replica
//!
//! | Local page | Contents (besides loads, terminals and modules spread over all pages) |
//! |---|---|
//! | 0 (supply) | strip X1 with terminals 1 (L), 2 (N), 3 (PE); main switch Q; rails L, N, PE |
//! | 1 (PSU) | MCB F, PSU G; rails L, N, PE (continuing to the next replica); the 24 V and 0 V rails start here |
//! | 2 (distribution) | MCB F, distribution F (MICO-like); the segment of strip X2 |
//! | 3 (PLC) | the PLC station A; the bus cable W with three conductors |
//! | 0 … n−1 | loads (a main function with `m` CPs plus aux contact blocks on the next page), X2 terminals, PLC modules |
//!
//! **Rails.** A rail is one potential on one page, wired as a chain: its ordered nodes are the start (a device
//! CP or an incoming interruption point), the taps, and the end (an outgoing interruption point). With `k`
//! nodes it has `k − 2` junctions (each with the sequence in, tap, out) and `2k − 3` connections; two nodes
//! give one direct wire, one node none. L, N and PE run on pages 0 and 1 of every replica and continue through
//! interruption-point pairs from page 1 of replica `u` to page 0 of replica `u + 1`, so they are project-wide
//! potentials: the global stitch graph spans every replica. The 24 V and 0 V rails of a replica start at its
//! PSU and run through interruption-point pairs over pages 1 … n−1: one 24 V and one 0 V potential per
//! replica. Loads on page 0 tap L and N, all other loads 24 V and 0 V.
//!
//! **Bus.** The PLC stations form a daisy chain: the conductors of replica `u`'s cable connect the DP+, DP−
//! and FE points of station `u` with those of station `u + 1`.
//!
//! **Signals.** The remaining connection budget is spent on page-local signal wires between free CPs (PLC
//! channels, aux contacts, the field and internal sides of X2 terminals, spare CPs of loads), in pairs taken
//! from a round-robin merge of those categories, spread evenly over the pages. A signal wire never touches a
//! rail, so the power potentials stay apart.
//!
//! **Tags.** Every non-terminal device gets the next free counter of its letter on the page of its main
//! representation, so page-based tags (`{page}{letter}{counter}`) are unique. Terminals carry their strip's
//! letter and counter and are identified by strip and designation.

use std::collections::BTreeMap;

/// A size class of NFR §2.1.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SizeClass {
    /// Up to 50 pages, 300 devices.
    S,
    /// Up to 300 pages, 2,500 devices.
    M,
    /// Up to 1,500 pages, 12,000 devices.
    L,
    /// Up to 5,000 pages, 50,000 devices.
    XL,
}

impl SizeClass {
    /// Every class, smallest first.
    pub const ALL: [SizeClass; 4] = [SizeClass::S, SizeClass::M, SizeClass::L, SizeClass::XL];

    /// The upper bounds of the class (NFR §2.1 table).
    pub const fn bounds(self) -> Bounds {
        match self {
            SizeClass::S => Bounds {
                pages: 50,
                devices: 300,
                functions: 900,
                connections: 1_500,
                cps: 3_750,
                terminals: 120,
                channels: 60,
            },
            SizeClass::M => Bounds {
                pages: 300,
                devices: 2_500,
                functions: 7_500,
                connections: 15_000,
                cps: 37_500,
                terminals: 1_000,
                channels: 500,
            },
            SizeClass::L => Bounds {
                pages: 1_500,
                devices: 12_000,
                functions: 36_000,
                connections: 70_000,
                cps: 175_000,
                terminals: 4_800,
                channels: 2_400,
            },
            SizeClass::XL => Bounds {
                pages: 5_000,
                devices: 50_000,
                functions: 150_000,
                connections: 250_000,
                cps: 625_000,
                terminals: 20_000,
                channels: 10_000,
            },
        }
    }

    /// The class name as NFR §2.1 writes it (`"S"`, `"M"`, `"L"`, `"XL"`).
    pub const fn name(self) -> &'static str {
        match self {
            SizeClass::S => "S",
            SizeClass::M => "M",
            SizeClass::L => "L",
            SizeClass::XL => "XL",
        }
    }
}

/// The columns of the NFR §2.1 table that the generator controls. `devices` counts every device record
/// (terminals, strips, cables and PLC modules included); `cps` counts instantiated connection points (the CP
/// count of each function's definition, summed over the functions); `channels` counts the channel CPs of the
/// PLC module functions. 3D placements are not generated yet (no layout records in the schema).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Bounds {
    /// Pages.
    pub pages: usize,
    /// Devices.
    pub devices: usize,
    /// Functions.
    pub functions: usize,
    /// Connections.
    pub connections: usize,
    /// Instantiated connection points.
    pub cps: usize,
    /// Terminals.
    pub terminals: usize,
    /// PLC channels.
    pub channels: usize,
}

/// The drawn folios of one replica, in page order: the 18 reproduced folio numbers of Golden B (doc 20 §5),
/// with the report and divider folios left to generation. A shorter last replica takes a prefix.
pub const REPLICA_FOLIOS: [u32; 18] = [
    1, 3, 101, 102, 103, 106, 201, 202, 203, 204, 205, 206, 301, 302, 303, 304, 305, 902,
];

/// Pages per full replica.
pub const PAGES_PER_REPLICA: usize = REPLICA_FOLIOS.len();

/// CPs of an aux contact block (`13`, `14`, `21`, `22`).
pub const CONTACT_CPS: usize = 4;

/// Channels of a full PLC module.
pub const CHANNELS_PER_MODULE: usize = 8;

/// Bounds of the load main function's CP count.
const LOAD_CPS_MIN: usize = 2;
const LOAD_CPS_MAX: usize = 32;

/// Index of a page in [`Plan::pages`].
pub type PageIx = usize;
/// Index of a device in [`Plan::devices`].
pub type DevIx = usize;
/// Index of a function in [`Plan::functions`].
pub type FnIx = usize;
/// Index of a junction in [`Plan::junctions`].
pub type JxIx = usize;
/// Index of a conductor in [`Plan::conductors`].
pub type CondIx = usize;
/// Index of a function definition in [`Plan::fndefs`].
pub type DefIx = usize;

/// One replica: its index and its pages.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Replica {
    /// The pages of the replica, in order (`pages.start` is local page 0).
    pub pages: std::ops::Range<PageIx>,
}

/// One page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PagePlan {
    /// The replica the page belongs to.
    pub replica: usize,
    /// The page's index within its replica.
    pub local: usize,
    /// The folio number within the replica's section ([`REPLICA_FOLIOS`]).
    pub folio: u32,
}

/// What a device is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceRole {
    /// Terminal strip (strip aspect); `supply` is X1, otherwise X2.
    Strip {
        /// X1 (L, N, PE) rather than X2 (distribution).
        supply: bool,
    },
    /// Terminal of a strip (terminal aspect).
    Terminal {
        /// The strip device.
        strip: DevIx,
        /// Position in the strip, from 1; also the designation.
        seq: u32,
    },
    /// Main switch.
    Switch,
    /// Miniature circuit breaker.
    Mcb,
    /// Power supply (device kind `Psu`).
    Psu,
    /// Electronic load distribution.
    Distribution,
    /// PLC station (bus node).
    Station,
    /// PLC I/O module.
    Module,
    /// Bus cable (cable aspect); has no function.
    Cable,
    /// A load: one main function and aux contact blocks.
    Load,
}

/// One device.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DevicePlan {
    /// What it is.
    pub role: DeviceRole,
    /// The replica it belongs to (its structure location).
    pub replica: usize,
    /// Tag letter.
    pub letter: &'static str,
    /// Tag counter: page-unique per letter for non-terminals; the strip's counter for terminals.
    pub counter: u32,
    /// The page of its main representation (for the cable: the page its conductors are drawn on).
    pub page: PageIx,
}

/// The function definitions the generator uses (embedded library copies).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DefKind {
    /// Two-pole main switch: `1`, `2`, `3`, `4`.
    Switch,
    /// MCB 1P+N: `1`, `2`, `N`, `N`.
    Mcb,
    /// PSU: `L`, `N`, `PE`, `+`, `+`, `-`, `-`.
    Psu,
    /// Load distribution: `24VDC`, `24VDC`, `GND`, `GND`, `13`, `14`, `Out1` … `Out4`.
    Distribution,
    /// Terminal strip (no CPs).
    Strip,
    /// Terminal: `a` (field side), `b` (internal side), `c` (bridge clamp).
    Terminal,
    /// Interruption point: `1`.
    Ip,
    /// PLC station: `24V`, `0V`, `FE`, `DP+`, `DP-`.
    Station,
    /// PLC module with `n` channels: `24V`, `0V`, `Ch0` … `Ch{n-1}`.
    Module(usize),
    /// Load main function with `m` CPs: `A1`, `A2`, `1` … `{m-2}`.
    Load(usize),
    /// Aux contact block: `13`, `14`, `21`, `22`.
    Contact,
}

impl DefKind {
    /// The CP designations, in index order.
    pub fn cps(&self) -> Vec<String> {
        let fixed = |names: &[&str]| names.iter().map(|s| (*s).to_owned()).collect();
        match self {
            DefKind::Switch => fixed(&["1", "2", "3", "4"]),
            DefKind::Mcb => fixed(&["1", "2", "N", "N"]),
            DefKind::Psu => fixed(&["L", "N", "PE", "+", "+", "-", "-"]),
            DefKind::Distribution => fixed(&[
                "24VDC", "24VDC", "GND", "GND", "13", "14", "Out1", "Out2", "Out3", "Out4",
            ]),
            DefKind::Strip => Vec::new(),
            DefKind::Terminal => fixed(&["a", "b", "c"]),
            DefKind::Ip => fixed(&["1"]),
            DefKind::Station => fixed(&["24V", "0V", "FE", "DP+", "DP-"]),
            DefKind::Module(n) => {
                let mut v: Vec<String> = fixed(&["24V", "0V"]);
                v.extend((0..*n).map(|k| format!("Ch{k}")));
                v
            }
            DefKind::Load(m) => {
                let mut v: Vec<String> = fixed(&["A1", "A2"]);
                v.extend((1..=m.saturating_sub(2)).map(|k| k.to_string()));
                v
            }
            DefKind::Contact => fixed(&["13", "14", "21", "22"]),
        }
    }

    /// The number of CPs.
    pub fn cp_count(&self) -> usize {
        match self {
            DefKind::Switch | DefKind::Mcb => 4,
            DefKind::Psu => 7,
            DefKind::Distribution => 10,
            DefKind::Strip => 0,
            DefKind::Terminal => 3,
            DefKind::Ip => 1,
            DefKind::Station => 5,
            DefKind::Module(n) => 2 + n,
            DefKind::Load(m) => *m,
            DefKind::Contact => CONTACT_CPS,
        }
    }
}

/// The role of a function in its device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FnRolePlan {
    /// The device's main function.
    Main,
    /// An auxiliary function (aux contact block).
    Aux,
}

/// Direction of an interruption point.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IpDir {
    /// Where the potential leaves the page.
    Source,
    /// Where it arrives.
    Target,
}

/// An interruption point function (no device).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IpPlan {
    /// The pairing name; source and target of one hop share it, every hop has its own.
    pub name: String,
    /// Direction.
    pub direction: IpDir,
}

/// One function.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FunctionPlan {
    /// The device; `None` exactly for interruption points.
    pub device: Option<DevIx>,
    /// Its definition.
    pub def: DefIx,
    /// Main or aux.
    pub role: FnRolePlan,
    /// The interruption point, for IP functions.
    pub ip: Option<IpPlan>,
}

/// Representation type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RepKind {
    /// Multi-line symbol.
    MultiLine,
    /// Terminal-strip segment (the strip function).
    StripSegment,
}

/// One representation: exactly one function, on one page, at a page-local slot.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RepPlan {
    /// The page.
    pub page: PageIx,
    /// The function drawn.
    pub function: FnIx,
    /// Representation type.
    pub kind: RepKind,
    /// Page-local slot number (0, 1, 2, … per page, shared with junctions); the emitter maps it to a grid
    /// position.
    pub slot: usize,
}

/// One junction (style dot, star wiring) on a page, with its connection sequence.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JunctionPlan {
    /// The page.
    pub page: PageIx,
    /// The connections ending at the junction, in order (incoming chain segment, tap, outgoing segment).
    pub sequence: Vec<usize>,
    /// Page-local slot number.
    pub slot: usize,
}

/// A connection endpoint.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum End {
    /// CP `idx` of a function.
    Cp(FnIx, u16),
    /// A junction.
    Junction(JxIx),
}

/// The kind of a connection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnKind {
    /// A drawn wire.
    Wire,
    /// A cable core: the conductor.
    Core(CondIx),
}

/// What a connection is for (statistics and bench handles).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnRole {
    /// Part of a rail.
    Rail,
    /// A page-local signal wire.
    Signal,
    /// A bus cable core.
    Bus,
}

/// One connection.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConnPlan {
    /// The page it is drawn on.
    pub page: PageIx,
    /// First endpoint.
    pub a: End,
    /// Second endpoint.
    pub b: End,
    /// Wire or core.
    pub kind: ConnKind,
    /// What it is for.
    pub role: ConnRole,
}

/// One conductor of a cable.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConductorPlan {
    /// The cable device.
    pub cable: DevIx,
    /// Core designation.
    pub core: &'static str,
}

/// The type of a defined potential.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PotKind {
    /// Line.
    L,
    /// Neutral.
    N,
    /// Protective earth.
    Pe,
    /// DC positive.
    Plus24,
    /// DC negative.
    Zero,
}

/// One potential definition.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PotentialPlan {
    /// The name, project-unique (`L`, `N`, `PE`, `24V-<replica>`, `0V-<replica>`, replicas from 1).
    pub name: String,
    /// The type.
    pub kind: PotKind,
    /// Where it is defined.
    pub at: (FnIx, u16),
}

/// The plan of a generated project.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Plan {
    /// The class it was built for.
    pub class: SizeClass,
    /// The replicas.
    pub replicas: Vec<Replica>,
    /// The pages, replica by replica.
    pub pages: Vec<PagePlan>,
    /// The function definitions used.
    pub fndefs: Vec<DefKind>,
    /// The devices.
    pub devices: Vec<DevicePlan>,
    /// The functions.
    pub functions: Vec<FunctionPlan>,
    /// The representations.
    pub reps: Vec<RepPlan>,
    /// The junctions.
    pub junctions: Vec<JunctionPlan>,
    /// The connections.
    pub connections: Vec<ConnPlan>,
    /// The cable conductors.
    pub conductors: Vec<ConductorPlan>,
    /// The potential definitions.
    pub potentials: Vec<PotentialPlan>,
    /// The CP count of the load main function (`DefKind::Load(m)`).
    pub load_cps: usize,
    /// Per page, the signal CPs no connection uses, in pool order: targets for wires drawn by benchmarks.
    pub free_cps: Vec<Vec<(FnIx, u16)>>,
    /// The main representation of every load, in device order (rep indices): targets for moves.
    pub load_reps: Vec<usize>,
}

/// The counts of a plan in the columns of NFR §2.1.
pub type Counts = Bounds;

impl Plan {
    /// The counts of the plan: pages, devices, functions, connections, instantiated CPs, terminals and PLC
    /// channels.
    pub fn counts(&self) -> Counts {
        let cps = self.functions.iter().map(|f| self.fndefs[f.def].cp_count()).sum();
        let channels = self
            .functions
            .iter()
            .map(|f| match self.fndefs[f.def] {
                DefKind::Module(n) => n,
                _ => 0,
            })
            .sum();
        Counts {
            pages: self.pages.len(),
            devices: self.devices.len(),
            functions: self.functions.len(),
            connections: self.connections.len(),
            cps,
            terminals: self
                .devices
                .iter()
                .filter(|d| matches!(d.role, DeviceRole::Terminal { .. }))
                .count(),
            channels,
        }
    }

    /// Builds the plan of `class`.
    ///
    /// # Panics
    ///
    /// If the class bounds cannot be met by the replica shape (a generator bug: the shipped classes are
    /// covered by tests).
    pub fn build(class: SizeClass) -> Plan {
        Builder::new(class).build()
    }
}

// ---------------------------------------------------------------------------------------------------------
// Builder

/// The rails of a page: potential slot (0 L, 1 N, 2 PE, 3 24 V, 4 0 V) to its ordered nodes.
type Rails = BTreeMap<usize, Vec<End>>;

const RAIL_L: usize = 0;
const RAIL_N: usize = 1;
const RAIL_PE: usize = 2;
const RAIL_24: usize = 3;
const RAIL_0: usize = 4;
const RAIL_NAMES: [&str; 5] = ["L", "N", "PE", "P24", "P0"];

/// The signal-CP categories of a page, merged round-robin into the page's pool.
#[derive(Default)]
struct SignalPool {
    channels: Vec<(FnIx, u16)>,
    contact_a: Vec<(FnIx, u16)>,
    terminal_a: Vec<(FnIx, u16)>,
    contact_b: Vec<(FnIx, u16)>,
    terminal_b: Vec<(FnIx, u16)>,
    spares: Vec<(FnIx, u16)>,
}

impl SignalPool {
    fn merged(self) -> Vec<(FnIx, u16)> {
        let lists = [
            self.channels,
            self.contact_a,
            self.terminal_a,
            self.contact_b,
            self.terminal_b,
            self.spares,
        ];
        let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
        let mut out = Vec::with_capacity(lists.iter().map(Vec::len).sum());
        for i in 0..longest {
            for list in &lists {
                if let Some(cp) = list.get(i) {
                    out.push(*cp);
                }
            }
        }
        out
    }
}

struct Builder {
    class: SizeClass,
    bounds: Bounds,
    plan: Plan,
    /// Next tag counter per (page, letter).
    counters: BTreeMap<(PageIx, &'static str), u32>,
    /// Next slot per page.
    slots: Vec<usize>,
    /// Rails per page.
    rails: Vec<Rails>,
    /// Signal pools per page.
    pools: Vec<SignalPool>,
    /// Definition index per kind.
    defs: BTreeMap<String, DefIx>,
}

/// Per-replica spine handles.
struct Spine {
    station: FnIx,
    cable: DevIx,
}

impl Builder {
    fn new(class: SizeClass) -> Builder {
        Builder {
            class,
            bounds: class.bounds(),
            plan: Plan {
                class,
                replicas: Vec::new(),
                pages: Vec::new(),
                fndefs: Vec::new(),
                devices: Vec::new(),
                functions: Vec::new(),
                reps: Vec::new(),
                junctions: Vec::new(),
                connections: Vec::new(),
                conductors: Vec::new(),
                potentials: Vec::new(),
                load_cps: 0,
                free_cps: Vec::new(),
                load_reps: Vec::new(),
            },
            counters: BTreeMap::new(),
            slots: Vec::new(),
            rails: Vec::new(),
            pools: Vec::new(),
            defs: BTreeMap::new(),
        }
    }

    fn def(&mut self, kind: DefKind) -> DefIx {
        let key = format!("{kind:?}");
        if let Some(&ix) = self.defs.get(&key) {
            return ix;
        }
        let ix = self.plan.fndefs.len();
        self.plan.fndefs.push(kind);
        self.defs.insert(key, ix);
        ix
    }

    fn slot(&mut self, page: PageIx) -> usize {
        let s = self.slots[page];
        self.slots[page] += 1;
        s
    }

    fn counter(&mut self, page: PageIx, letter: &'static str) -> u32 {
        let c = self.counters.entry((page, letter)).or_insert(1);
        let out = *c;
        *c += 1;
        out
    }

    fn device(&mut self, role: DeviceRole, replica: usize, letter: &'static str, page: PageIx) -> DevIx {
        let counter = self.counter(page, letter);
        self.device_with_counter(role, replica, letter, counter, page)
    }

    fn device_with_counter(
        &mut self,
        role: DeviceRole,
        replica: usize,
        letter: &'static str,
        counter: u32,
        page: PageIx,
    ) -> DevIx {
        self.plan.devices.push(DevicePlan {
            role,
            replica,
            letter,
            counter,
            page,
        });
        self.plan.devices.len() - 1
    }

    /// A function of `device` with definition `kind`, drawn by one representation on `page`.
    fn function(
        &mut self,
        device: DevIx,
        kind: DefKind,
        role: FnRolePlan,
        page: PageIx,
        rep: RepKind,
    ) -> FnIx {
        let def = self.def(kind);
        self.plan.functions.push(FunctionPlan {
            device: Some(device),
            def,
            role,
            ip: None,
        });
        let f = self.plan.functions.len() - 1;
        let slot = self.slot(page);
        self.plan.reps.push(RepPlan {
            page,
            function: f,
            kind: rep,
            slot,
        });
        f
    }

    fn ip(&mut self, name: String, direction: IpDir, page: PageIx) -> FnIx {
        let def = self.def(DefKind::Ip);
        self.plan.functions.push(FunctionPlan {
            device: None,
            def,
            role: FnRolePlan::Main,
            ip: Some(IpPlan { name, direction }),
        });
        let f = self.plan.functions.len() - 1;
        let slot = self.slot(page);
        self.plan.reps.push(RepPlan {
            page,
            function: f,
            kind: RepKind::MultiLine,
            slot,
        });
        f
    }

    fn tap(&mut self, page: PageIx, rail: usize, end: End) {
        self.rails[page].entry(rail).or_default().push(end);
    }

    fn build(mut self) -> Plan {
        let b = self.bounds;
        // Pages: full replicas, then a shorter last one with the remainder.
        let full = b.pages / PAGES_PER_REPLICA;
        let rest = b.pages % PAGES_PER_REPLICA;
        let mut lengths = vec![PAGES_PER_REPLICA; full];
        if rest > 0 {
            assert!(
                rest >= 4,
                "a replica needs at least 4 pages (class {})",
                self.class.name()
            );
            lengths.push(rest);
        }
        for (u, &n) in lengths.iter().enumerate() {
            let start = self.plan.pages.len();
            for (local, &folio) in REPLICA_FOLIOS.iter().enumerate().take(n) {
                self.plan.pages.push(PagePlan {
                    replica: u,
                    local,
                    folio,
                });
            }
            self.plan.replicas.push(Replica {
                pages: start..start + n,
            });
        }
        let pages = self.plan.pages.len();
        let replicas = self.plan.replicas.len();
        self.slots = vec![0; pages];
        self.rails = vec![Rails::new(); pages];
        self.pools = (0..pages).map(|_| SignalPool::default()).collect();

        // Fixed devices, functions and CPs; the loads take the rest.
        let modules = b.channels.div_ceil(CHANNELS_PER_MODULE);
        let spine_devices = 9 * replicas; // X1, X2, Q, F, G, F, F (distribution), A (station), W (cable)
        let loads = b
            .devices
            .checked_sub(spine_devices + b.terminals + modules)
            .expect("device bound covers the spine");
        let ip_functions: usize =
            2 * (3 * replicas + 3 * (replicas - 1)) + lengths.iter().map(|&n| 4 * (n - 2)).sum::<usize>();
        let fixed_functions = 8 * replicas + b.terminals + modules + ip_functions;
        let load_functions = b
            .functions
            .checked_sub(fixed_functions)
            .expect("function bound covers the fixed functions");
        assert!(load_functions >= loads, "every load needs a main function");
        let contacts = load_functions - loads;
        let spine_cps = replicas
            * (DefKind::Switch.cp_count()
                + 2 * DefKind::Mcb.cp_count()
                + DefKind::Psu.cp_count()
                + DefKind::Distribution.cp_count()
                + DefKind::Station.cp_count());
        let fixed_cps = spine_cps
            + b.terminals * DefKind::Terminal.cp_count()
            + ip_functions
            + 2 * modules
            + b.channels
            + contacts * CONTACT_CPS;
        let load_cps = (b.cps.saturating_sub(fixed_cps) / loads.max(1)).clamp(LOAD_CPS_MIN, LOAD_CPS_MAX);
        self.plan.load_cps = load_cps;

        // Spine of every replica.
        let mut spines = Vec::with_capacity(replicas);
        for u in 0..replicas {
            let spine = self.spine(u, b.terminals, replicas);
            spines.push(spine);
        }
        // Bus: station u to station u + 1 through replica u's cable.
        for u in 0..replicas.saturating_sub(1) {
            let page = self.plan.replicas[u].pages.start + 3;
            for (core, cp) in [("A", 3u16), ("B", 4), ("SH", 2)] {
                self.plan.conductors.push(ConductorPlan {
                    cable: spines[u].cable,
                    core,
                });
                let cond = self.plan.conductors.len() - 1;
                self.plan.connections.push(ConnPlan {
                    page,
                    a: End::Cp(spines[u].station, cp),
                    b: End::Cp(spines[u + 1].station, cp),
                    kind: ConnKind::Core(cond),
                    role: ConnRole::Bus,
                });
            }
        }
        // The last cable has its conductors but no second station to reach.
        if let Some(last) = spines.last() {
            for core in ["A", "B", "SH"] {
                self.plan.conductors.push(ConductorPlan {
                    cable: last.cable,
                    core,
                });
            }
        }

        // PLC modules, round-robin over the replicas, on local pages 3 … n−1.
        for k in 0..modules {
            let u = k % replicas;
            let range = self.plan.replicas[u].pages.clone();
            let span = range.len() - 3;
            let page = range.start + 3 + (k / replicas) % span;
            let channels = if k + 1 == modules {
                b.channels - CHANNELS_PER_MODULE * (modules - 1)
            } else {
                CHANNELS_PER_MODULE
            };
            let d = self.device(DeviceRole::Module, u, "A", page);
            let f = self.function(
                d,
                DefKind::Module(channels),
                FnRolePlan::Main,
                page,
                RepKind::MultiLine,
            );
            self.power_taps(page, f, 0, 1);
            for ch in 0..channels {
                self.pools[page].channels.push((f, 2 + ch as u16));
            }
        }

        // Loads, round-robin over all pages; aux contacts on the next page of the replica.
        for i in 0..loads {
            let page = i % pages;
            let u = self.plan.pages[page].replica;
            let letter = ["K", "M", "H", "S", "B", "Y", "E"][i % 7];
            let d = self.device(DeviceRole::Load, u, letter, page);
            let f = self.function(
                d,
                DefKind::Load(load_cps),
                FnRolePlan::Main,
                page,
                RepKind::MultiLine,
            );
            self.plan.load_reps.push(self.plan.reps.len() - 1);
            self.power_taps(page, f, 0, 1);
            for cp in 2..load_cps {
                self.pools[page].spares.push((f, cp as u16));
            }
            let aux = contacts / loads + usize::from(i < contacts % loads);
            let range = self.plan.replicas[u].pages.clone();
            let aux_page = range.start + (self.plan.pages[page].local + 1) % range.len();
            for _ in 0..aux {
                let c = self.function(d, DefKind::Contact, FnRolePlan::Aux, aux_page, RepKind::MultiLine);
                let pool = &mut self.pools[aux_page];
                pool.contact_a.extend([(c, 0), (c, 1)]);
                pool.contact_b.extend([(c, 2), (c, 3)]);
            }
        }

        // Interruption points close the rails; then every rail is wired.
        self.interruption_points(&lengths);
        for page in 0..pages {
            let rails = std::mem::take(&mut self.rails[page]);
            for (_, nodes) in rails {
                self.wire_rail(page, &nodes);
            }
        }

        // Signals spend the remaining connection budget.
        let budget = b
            .connections
            .checked_sub(self.plan.connections.len())
            .expect("connection bound covers rails and bus");
        self.signals(budget);
        self.plan
    }

    /// The fixed devices of replica `u`.
    fn spine(&mut self, u: usize, terminals: usize, replicas: usize) -> Spine {
        let range = self.plan.replicas[u].pages.clone();
        let (p0, p1, p2, p3) = (range.start, range.start + 1, range.start + 2, range.start + 3);

        // X1: the supply strip with L, N, PE on page 0.
        let x1 = self.device(DeviceRole::Strip { supply: true }, u, "X", p0);
        self.function(x1, DefKind::Strip, FnRolePlan::Main, p0, RepKind::StripSegment);
        let x1_counter = self.plan.devices[x1].counter;
        for (seq, rail) in [(1u32, RAIL_L), (2, RAIL_N), (3, RAIL_PE)] {
            let t = self.device_with_counter(DeviceRole::Terminal { strip: x1, seq }, u, "X", x1_counter, p0);
            let f = self.function(t, DefKind::Terminal, FnRolePlan::Main, p0, RepKind::MultiLine);
            // The field side feeds the rail in replica 0 (its potential definition sits there); later replicas
            // are fed by the incoming interruption point, and the field side is one more tap.
            if u == 0 {
                self.plan.potentials.push(PotentialPlan {
                    name: RAIL_NAMES[rail].to_owned(),
                    kind: [PotKind::L, PotKind::N, PotKind::Pe][rail],
                    at: (f, 0),
                });
            }
            self.tap(p0, rail, End::Cp(f, 0));
            self.tap(p0, rail, End::Cp(f, 1));
        }
        // Main switch Q on page 0: pole inputs on L and N.
        let q = self.device(DeviceRole::Switch, u, "Q", p0);
        let fq = self.function(q, DefKind::Switch, FnRolePlan::Main, p0, RepKind::MultiLine);
        self.tap(p0, RAIL_L, End::Cp(fq, 0));
        self.tap(p0, RAIL_N, End::Cp(fq, 2));

        // MCB F and PSU G on page 1.
        let f1 = self.device(DeviceRole::Mcb, u, "F", p1);
        let ff1 = self.function(f1, DefKind::Mcb, FnRolePlan::Main, p1, RepKind::MultiLine);
        self.tap(p1, RAIL_L, End::Cp(ff1, 0));
        self.tap(p1, RAIL_N, End::Cp(ff1, 2));
        let g = self.device(DeviceRole::Psu, u, "G", p1);
        let fg = self.function(g, DefKind::Psu, FnRolePlan::Main, p1, RepKind::MultiLine);
        self.tap(p1, RAIL_L, End::Cp(fg, 0));
        self.tap(p1, RAIL_N, End::Cp(fg, 1));
        self.tap(p1, RAIL_PE, End::Cp(fg, 2));
        // The PSU starts the replica's 24 V and 0 V rails (first node of the rail on page 1).
        self.tap(p1, RAIL_24, End::Cp(fg, 3));
        self.tap(p1, RAIL_0, End::Cp(fg, 5));
        self.plan.potentials.push(PotentialPlan {
            name: format!("24V-{}", u + 1),
            kind: PotKind::Plus24,
            at: (fg, 3),
        });
        self.plan.potentials.push(PotentialPlan {
            name: format!("0V-{}", u + 1),
            kind: PotKind::Zero,
            at: (fg, 5),
        });

        // MCB F in +24 V and the distribution F on page 2; strip X2's segment.
        let f2 = self.device(DeviceRole::Mcb, u, "F", p2);
        let ff2 = self.function(f2, DefKind::Mcb, FnRolePlan::Main, p2, RepKind::MultiLine);
        self.tap(p2, RAIL_24, End::Cp(ff2, 0));
        let f3 = self.device(DeviceRole::Distribution, u, "F", p2);
        let ff3 = self.function(
            f3,
            DefKind::Distribution,
            FnRolePlan::Main,
            p2,
            RepKind::MultiLine,
        );
        self.power_taps(p2, ff3, 0, 2);
        let x2 = self.device(DeviceRole::Strip { supply: false }, u, "X", p2);
        self.function(x2, DefKind::Strip, FnRolePlan::Main, p2, RepKind::StripSegment);
        let x2_counter = self.plan.devices[x2].counter;
        // X2's terminals: this replica's share of the terminals beyond the X1 triples, spread over its pages.
        let rest = terminals - 3 * replicas;
        let share = rest / replicas + usize::from(u < rest % replicas);
        for j in 0..share {
            let page = range.start + j % range.len();
            let t = self.device_with_counter(
                DeviceRole::Terminal {
                    strip: x2,
                    seq: j as u32 + 1,
                },
                u,
                "X",
                x2_counter,
                page,
            );
            let f = self.function(t, DefKind::Terminal, FnRolePlan::Main, page, RepKind::MultiLine);
            self.pools[page].terminal_a.push((f, 0));
            self.pools[page].terminal_b.push((f, 1));
        }

        // PLC station A and the bus cable W on page 3.
        let a = self.device(DeviceRole::Station, u, "A", p3);
        let fa = self.function(a, DefKind::Station, FnRolePlan::Main, p3, RepKind::MultiLine);
        self.power_taps(p3, fa, 0, 1);
        let w = self.device(DeviceRole::Cable, u, "W", p3);
        Spine {
            station: fa,
            cable: w,
        }
    }

    /// Taps CP `plus` on the page's positive rail (L on local page 0, else 24 V) and CP `minus` on its
    /// negative rail (N, else 0 V).
    fn power_taps(&mut self, page: PageIx, f: FnIx, plus: u16, minus: u16) {
        let (rp, rm) = if self.plan.pages[page].local == 0 {
            (RAIL_L, RAIL_N)
        } else {
            (RAIL_24, RAIL_0)
        };
        self.tap(page, rp, End::Cp(f, plus));
        self.tap(page, rm, End::Cp(f, minus));
    }

    /// Interruption-point pairs: L, N, PE from local page 0 to 1 and from page 1 of a replica to page 0 of
    /// the next; 24 V and 0 V from page `q` to `q + 1` for `q` in 1 … n−2. A target is the first node of its
    /// rail on the arriving page, a source the last node on the leaving page.
    fn interruption_points(&mut self, lengths: &[usize]) {
        let replicas = lengths.len();
        for (u, &len) in lengths.iter().enumerate() {
            let start = self.plan.replicas[u].pages.start;
            for rail in [RAIL_L, RAIL_N, RAIL_PE] {
                self.hop(
                    rail,
                    format!("{}-{}-0", RAIL_NAMES[rail], u + 1),
                    start,
                    start + 1,
                );
                if u + 1 < replicas {
                    let next = self.plan.replicas[u + 1].pages.start;
                    self.hop(rail, format!("{}-{}-x", RAIL_NAMES[rail], u + 1), start + 1, next);
                }
            }
            for rail in [RAIL_24, RAIL_0] {
                for q in 1..len - 1 {
                    self.hop(
                        rail,
                        format!("{}-{}-{q}", RAIL_NAMES[rail], u + 1),
                        start + q,
                        start + q + 1,
                    );
                }
            }
        }
    }

    fn hop(&mut self, rail: usize, name: String, from: PageIx, to: PageIx) {
        let source = self.ip(name.clone(), IpDir::Source, from);
        let target = self.ip(name, IpDir::Target, to);
        self.tap(from, rail, End::Cp(source, 0));
        self.rails[to]
            .entry(rail)
            .or_default()
            .insert(0, End::Cp(target, 0));
    }

    /// Wires one rail as a chain: first and last node are the ends, every middle node hangs off a junction.
    fn wire_rail(&mut self, page: PageIx, nodes: &[End]) {
        match nodes.len() {
            0 | 1 => {}
            2 => {
                self.wire(page, nodes[0], nodes[1], ConnRole::Rail);
            }
            k => {
                let mut prev = nodes[0];
                for (n, node) in nodes[1..k - 1].iter().enumerate() {
                    let j = self.plan.junctions.len();
                    let slot = self.slot(page);
                    self.plan.junctions.push(JunctionPlan {
                        page,
                        sequence: Vec::new(),
                        slot,
                    });
                    let incoming = self.wire(page, prev, End::Junction(j), ConnRole::Rail);
                    if n > 0 {
                        // This junction's incoming segment is the previous junction's outgoing one.
                        self.plan.junctions[j - 1].sequence.push(incoming);
                    }
                    let tap = self.wire(page, End::Junction(j), *node, ConnRole::Rail);
                    self.plan.junctions[j].sequence.extend([incoming, tap]);
                    prev = End::Junction(j);
                }
                let last = self.wire(page, prev, nodes[k - 1], ConnRole::Rail);
                let lj = self.plan.junctions.len() - 1;
                self.plan.junctions[lj].sequence.push(last);
            }
        }
    }

    fn wire(&mut self, page: PageIx, a: End, b: End, role: ConnRole) -> usize {
        self.plan.connections.push(ConnPlan {
            page,
            a,
            b,
            kind: ConnKind::Wire,
            role,
        });
        self.plan.connections.len() - 1
    }

    /// Spends `budget` signal wires over the pages: an even share per page first (capped by what the page
    /// offers), then the shortfall from the pages with room left, in page order.
    fn signals(&mut self, budget: usize) {
        let pages = self.plan.pages.len();
        let pools: Vec<Vec<(FnIx, u16)>> = std::mem::take(&mut self.pools)
            .into_iter()
            .map(SignalPool::merged)
            .collect();
        let caps: Vec<usize> = pools.iter().map(|p| p.len() / 2).collect();
        let mut used: Vec<usize> = (0..pages)
            .map(|q| (budget / pages + usize::from(q < budget % pages)).min(caps[q]))
            .collect();
        let mut short = budget - used.iter().sum::<usize>();
        for q in 0..pages {
            if short == 0 {
                break;
            }
            let more = (caps[q] - used[q]).min(short);
            used[q] += more;
            short -= more;
        }
        assert_eq!(
            short,
            0,
            "class {}: not enough free CPs for the signal wires",
            self.class.name()
        );
        for (q, pool) in pools.into_iter().enumerate() {
            for i in 0..used[q] {
                let (fa, ca) = pool[2 * i];
                let (fb, cb) = pool[2 * i + 1];
                self.wire(q, End::Cp(fa, ca), End::Cp(fb, cb), ConnRole::Signal);
            }
            self.plan.free_cps.push(pool[2 * used[q]..].to_vec());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_shape(plan: &Plan) {
        let b = plan.class.bounds();
        let c = plan.counts();
        assert_eq!(c.pages, b.pages);
        assert_eq!(c.devices, b.devices);
        assert_eq!(c.functions, b.functions);
        assert_eq!(c.connections, b.connections);
        assert_eq!(c.terminals, b.terminals);
        assert_eq!(c.channels, b.channels);
        assert!(c.cps <= b.cps, "{} CPs above the bound {}", c.cps, b.cps);
        assert!(c.cps * 100 >= b.cps * 95, "{} CPs below 95 % of {}", c.cps, b.cps);

        // Every junction has exactly the connections that end at it.
        let mut at: BTreeMap<JxIx, Vec<usize>> = BTreeMap::new();
        for (i, c) in plan.connections.iter().enumerate() {
            assert_ne!(c.a, c.b);
            for e in [c.a, c.b] {
                if let End::Junction(j) = e {
                    at.entry(j).or_default().push(i);
                }
            }
        }
        for (j, junction) in plan.junctions.iter().enumerate() {
            let mut seq = junction.sequence.clone();
            seq.sort_unstable();
            assert_eq!(seq.len(), 3, "junction {j}");
            assert_eq!(Some(&seq), at.get(&j), "junction {j}");
        }
        // Wires are page-local: CP endpoints are drawn on the connection's page (bus cores excepted).
        let rep_page: BTreeMap<FnIx, PageIx> = plan.reps.iter().map(|r| (r.function, r.page)).collect();
        for c in &plan.connections {
            if c.role == ConnRole::Bus {
                continue;
            }
            for e in [c.a, c.b] {
                match e {
                    End::Cp(f, idx) => {
                        assert_eq!(rep_page[&f], c.page);
                        assert!(usize::from(idx) < plan.fndefs[plan.functions[f].def].cp_count());
                    }
                    End::Junction(j) => assert_eq!(plan.junctions[j].page, c.page),
                }
            }
        }
        // Page-based tags are unique.
        let mut tags = std::collections::BTreeSet::new();
        for d in &plan.devices {
            if !matches!(d.role, DeviceRole::Terminal { .. }) {
                assert!(tags.insert((d.page, d.letter, d.counter)), "duplicate tag {d:?}");
            }
        }
        // Interruption points pair up: one source and one target per name.
        let mut ips: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for f in &plan.functions {
            if let Some(ip) = &f.ip {
                let e = ips.entry(ip.name.as_str()).or_default();
                match ip.direction {
                    IpDir::Source => e.0 += 1,
                    IpDir::Target => e.1 += 1,
                }
            }
        }
        assert!(ips.values().all(|&n| n == (1, 1)));
    }

    #[test]
    fn s_plan_meets_the_class_bounds() {
        let plan = Plan::build(SizeClass::S);
        check_shape(&plan);
        assert_eq!(plan.replicas.len(), 3);
        assert_eq!(plan, Plan::build(SizeClass::S), "deterministic");
    }

    #[test]
    fn m_plan_meets_the_class_bounds() {
        check_shape(&Plan::build(SizeClass::M));
    }

    #[test]
    #[ignore = "L/XL generation runs with the performance spike (milestone rule)"]
    fn l_and_xl_plans_meet_the_class_bounds() {
        check_shape(&Plan::build(SizeClass::L));
        check_shape(&Plan::build(SizeClass::XL));
    }
}
