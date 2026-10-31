// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command-DSL builder of the mini golden model: the records of `fixture::mini_golden_parts()` (minus
//! the bond, which has no command in v0) constructed through commands only, from `empty_model()`, and
//! compared record for record with a hand-built expectation that uses the IDs the commands minted.

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddTerminals, ConductorSpec, Connect, ContinueIp, CreateCable, CreateDevice, CreatePage, CreateSection,
    CreateStructureNode, DefinePotential, EndpointParam, PlaceIp, PlaceSymbol, PlaceTarget, TerminalSpec,
};
use ew_commands::{Cost, DynCommand, Engine, Receipt};
use ew_ids::Id;
use ew_mltext::MlText;
use ew_model::Model;
use ew_rds::Aspect;
use ew_schema::{
    BridgeKind, BridgeSpec, Cable, Conductor, Connection, ConnectionKind, Device, DeviceKind, Extra, FnDef,
    FnRole, Function, IpDirection, Junction, Kind, Page, PageType, Part, PartRef, PartUse, PotentialDef,
    PotentialType, Record, RepType, Representation, Section, Slot, Strip, StructureAssignment, StructureNode,
    TerminalGroup,
};

/// The minted IDs of the DSL-built model.
#[derive(Clone, Debug)]
struct Ids {
    loc: Id<StructureNode>,
    func_p01: Id<StructureNode>,
    sec: Id<Section>,
    p101: Id<Page>,
    p102: Id<Page>,
    d101f1: Id<Device>,
    f101f1: Id<Function>,
    d102g1: Id<Device>,
    f102g1: Id<Function>,
    d102f2: Id<Device>,
    f102f2: Id<Function>,
    f102f2_aux: Id<Function>,
    x1031: Id<Device>,
    fx1031: Id<Function>,
    t: [Id<Device>; 3],
    ft: [Id<Function>; 3],
    cable: Id<Device>,
    cond: [Id<Conductor>; 3],
    rep_101f1: Id<Representation>,
    rep_102g1: Id<Representation>,
    rep_102f2: Id<Representation>,
    rep_102f2_aux: Id<Representation>,
    rep_x1031: Id<Representation>,
    rep_t1: Id<Representation>,
    ip101: Id<Function>,
    ip102: Id<Function>,
    rep_ip101: Id<Representation>,
    rep_ip102: Id<Representation>,
    w11a: Id<Connection>,
    w11b: Id<Connection>,
    j5_in: Id<Connection>,
    w22: Id<Connection>,
    w27: Id<Connection>,
    w29: Id<Connection>,
    br1: Id<Connection>,
    core_a: Id<Connection>,
    j5: Id<Junction>,
    pd: Id<PotentialDef>,
}

/// An engine over the library-only model, with typed helpers that run one command and return what it
/// created.
struct Dsl {
    e: Engine,
}

fn primary<T>(r: &Receipt) -> Id<T> {
    typed(r.outcome.primary.expect("the command has a primary outcome"))
}

/// The IDs of the records of `kind` that the command created, in insertion order.
fn created<T>(r: &Receipt, kind: Kind) -> Vec<Id<T>> {
    r.created
        .iter()
        .filter(|k| k.0 == kind)
        .map(|k| typed::<T>(*k))
        .collect()
}

fn at_loc(loc: Id<StructureNode>) -> StructureAssignment {
    StructureAssignment {
        location: Some(Slot::Node(loc)),
        ..Default::default()
    }
}

/// A `device.create` with the structure at `loc` and a main function.
fn new_device(loc: Id<StructureNode>, letter: &str, counter: u32, main: Id<FnDef>) -> CreateDevice {
    CreateDevice {
        letter: ident(letter),
        counter: Some(counter),
        kind: None,
        parts: vec![],
        function_text: MlText::default(),
        structure: at_loc(loc),
        strip: None,
        main_fndef: Some(main),
    }
}

fn part_use(part: Id<Part>) -> Vec<PartUse> {
    vec![PartUse {
        part: PartRef::new(part),
        qty: dec3("1"),
        extra: Extra::new(),
    }]
}

fn ep(f: Id<Function>, idx: u16) -> EndpointParam {
    EndpointParam::Cp { function: f, idx }
}

impl Dsl {
    fn new(seed: u64) -> Dsl {
        Dsl {
            e: engine_seeded(empty_model(), seed),
        }
    }

    /// Runs one command: bulk commands through preview and commit, the others through `execute`.
    fn run(&mut self, cmd: &dyn DynCommand) -> Receipt {
        if cmd.cost(self.e.model()) == Cost::Bulk {
            let p = self
                .e
                .preview(cmd)
                .unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name()));
            self.e
                .commit(p)
                .unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name()))
        } else {
            self.e
                .execute(cmd)
                .unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name()))
        }
    }

    fn node(&mut self, aspect: Aspect, code: &str, aliases: &[&str]) -> Id<StructureNode> {
        let r = self.run(&CreateStructureNode {
            aspect,
            code: ident(code),
            parent: None,
            aliases: aliases.iter().map(|a| ident(a)).collect(),
            description: MlText::default(),
        });
        primary(&r)
    }

    fn section(&mut self, number: u16, range: (u32, u32), title: &str) -> Id<Section> {
        let r = self.run(&CreateSection {
            number,
            folio_range: range,
            title: MlText::neutral(title),
            divider: false,
        });
        primary(&r)
    }

    fn page(&mut self, sec: Id<Section>, loc: Id<StructureNode>, number: u32, title: &str) -> Id<Page> {
        let mut c = CreatePage::at(Some(sec), number);
        c.kind = PageType::MultiLine;
        c.structure = at_loc(loc);
        c.form = Some(FORM);
        c.grid = grid();
        c.title = MlText::neutral(title);
        primary(&self.run(&c))
    }

    /// `device.create`; returns the device and its main function.
    fn device(&mut self, cmd: CreateDevice) -> (Id<Device>, Id<Function>) {
        let r = self.run(&cmd);
        let function = *created::<Function>(&r, Kind::Function)
            .first()
            .expect("the main function");
        (primary(&r), function)
    }

    /// `device.place` of an existing function; returns the representation.
    fn place_existing(
        &mut self,
        page: Id<Page>,
        function: Id<Function>,
        rep_type: Option<RepType>,
    ) -> Id<Representation> {
        let r = self.run(&PlaceSymbol {
            copies: Vec::new(),
            page,
            symbol: SYM,
            pos: pt0(),
            orient: Default::default(),
            rep_type,
            variant: 0,
            target: PlaceTarget::ExistingFunction { function },
            connect: vec![],
        });
        primary(&r)
    }

    /// `connection.draw`; returns the new connection and every other record the command created.
    fn draw(
        &mut self,
        a: EndpointParam,
        b: EndpointParam,
        page: Option<Id<Page>>,
        kind: ConnectionKind,
    ) -> Receipt {
        self.run(&Connect {
            a,
            b,
            page,
            route: vec![],
            kind,
        })
    }
}

/// Builds the mini golden project through commands only.
fn build(seed: u64) -> (Dsl, Ids) {
    let mut d = Dsl::new(seed);
    let loc = d.node(Aspect::Location, "J-E-2416", &["JE2416"]);
    let func_p01 = d.node(Aspect::Function, "P01", &[]);
    let sec = d.section(1, (100, 199), "Wiring Definition");
    let p101 = d.page(sec, loc, 101, "Power Supply 1 - Main Current");
    let p102 = d.page(sec, loc, 102, "Power Supply 2 - Control Current");

    let (d101f1, f101f1) = d.device(CreateDevice {
        parts: part_use(PART_MCB),
        ..new_device(loc, "F", 1, FNDEF_MCB)
    });
    let (d102g1, f102g1) = d.device(CreateDevice {
        kind: Some(DeviceKind::Psu),
        parts: part_use(PART_PSU),
        ..new_device(loc, "G", 1, FNDEF_PSU)
    });
    let (d102f2, f102f2) = d.device(new_device(loc, "F", 2, FNDEF_MICO));
    let strip_aspect = Strip {
        groups: vec![TerminalGroup {
            from_seq: 1,
            to_seq: 4,
            label: MlText::neutral("Pneumatics"),
            extra: Extra::new(),
        }],
        ..strip(DEV_X1031, "X", 1031).aspects.strip.expect("strip aspect")
    };
    let (x1031, fx1031) = d.device(CreateDevice {
        strip: Some(strip_aspect),
        ..new_device(loc, "X", 1031, FNDEF_STRIP)
    });

    let r = d.run(&AddTerminals {
        strip: x1031,
        fndef: FNDEF_TERMINAL,
        terminals: ["1", "2", "3"]
            .iter()
            .map(|des| TerminalSpec {
                designation: ident(des),
                seq: None,
                function_text: MlText::neutral("1+"),
                levels: vec![],
                rail_bonded: false,
            })
            .collect(),
    });
    let t: [Id<Device>; 3] = created::<Device>(&r, Kind::Device)
        .try_into()
        .expect("three terminals");
    let ft: [Id<Function>; 3] = created::<Function>(&r, Kind::Function)
        .try_into()
        .expect("three terminal functions");

    let spec = |core: &str, shield: bool| ConductorSpec {
        core: ident(core),
        colour: None,
        csa: None,
        shield,
        pair: None,
    };
    let r = d.run(&CreateCable {
        letter: ident("W"),
        counter: Some(1),
        parts: vec![],
        function_text: MlText::default(),
        structure: at_loc(loc),
        cable: Cable::default(),
        conductors: vec![spec("A", false), spec("B", false), spec("SH", true)],
    });
    let cable: Id<Device> = primary(&r);
    let cond: [Id<Conductor>; 3] = created::<Conductor>(&r, Kind::Conductor)
        .try_into()
        .expect("three conductors");

    let rep_101f1 = d.place_existing(p101, f101f1, None);
    let rep_102g1 = d.place_existing(p102, f102g1, None);
    let rep_102f2 = d.place_existing(p102, f102f2, None);
    let rep_x1031 = d.place_existing(p102, fx1031, Some(RepType::StripSegment));
    let rep_t1 = d.place_existing(p102, ft[0], None);
    let r = d.run(&PlaceSymbol {
        copies: Vec::new(),
        page: p102,
        symbol: SYM,
        pos: pt0(),
        orient: Default::default(),
        rep_type: None,
        variant: 0,
        target: PlaceTarget::JoinDevice {
            device: d102f2,
            fndef: Some(FNDEF_CONTACT),
            role: FnRole::Aux,
        },
        connect: vec![],
    });
    let rep_102f2_aux: Id<Representation> = primary(&r);
    let f102f2_aux: Id<Function> = *created::<Function>(&r, Kind::Function)
        .first()
        .expect("the aux function");

    let r = d.run(&PlaceIp {
        page: p101,
        symbol: SYM,
        fndef: Some(FNDEF_IP),
        pos: pt0(),
        orient: Default::default(),
        name: ident("L-1"),
        direction: IpDirection::Source,
        star_source: false,
        sort: None,
        external: None,
        copies: Vec::new(),
    });
    let ip101: Id<Function> = primary(&r);
    let rep_ip101: Id<Representation> = created::<Representation>(&r, Kind::Representation)[0];
    let r = d.run(&ContinueIp {
        ip: ip101,
        page: p102,
        pos: None,
    });
    let ip102: Id<Function> = primary(&r);
    let rep_ip102: Id<Representation> = created::<Representation>(&r, Kind::Representation)[0];

    let wire = ConnectionKind::Wire;
    let w11a: Id<Connection> = primary(&d.draw(ep(f101f1, 1), ep(ip101, 0), Some(p101), wire.clone()));
    let w11b: Id<Connection> = primary(&d.draw(ep(ip102, 0), ep(f102g1, 0), Some(p102), wire.clone()));
    let j5_in: Id<Connection> = primary(&d.draw(ep(f102g1, 5), ep(f102f2, 2), Some(p102), wire.clone()));
    let r = d.draw(
        EndpointParam::OnConnection {
            connection: j5_in,
            at: pt0(),
        },
        ep(ft[1], 0),
        Some(p102),
        wire.clone(),
    );
    let w27: Id<Connection> = primary(&r);
    let j5: Id<Junction> = created::<Junction>(&r, Kind::Junction)[0];
    let w22: Id<Connection> = *created::<Connection>(&r, Kind::Connection)
        .iter()
        .find(|c| **c != w27)
        .expect("the split part");
    let w29: Id<Connection> = primary(&d.draw(ep(f102f2, 6), ep(ft[0], 0), Some(p102), wire));
    let br1: Id<Connection> = primary(&d.draw(
        ep(ft[0], 0),
        ep(ft[2], 0),
        None,
        ConnectionKind::Bridge(BridgeSpec {
            kind: BridgeKind::PlugIn,
            group: 1,
            shaft: 1,
            extra: Extra::new(),
        }),
    ));
    let core_a: Id<Connection> = primary(&d.draw(
        ep(ft[1], 1),
        ep(ft[2], 1),
        Some(p102),
        ConnectionKind::Core(cond[0]),
    ));

    let r = d.run(&DefinePotential {
        at: ep(f102g1, 3),
        name: ident("24VDC"),
        ptype: Some(PotentialType::LPlus),
        voltage: Some(dec3("24")),
        env: Default::default(),
        circuit_class: None,
        props: Default::default(),
    });
    let pd: Id<PotentialDef> = primary(&r);

    let ids = Ids {
        loc,
        func_p01,
        sec,
        p101,
        p102,
        d101f1,
        f101f1,
        d102g1,
        f102g1,
        d102f2,
        f102f2,
        f102f2_aux,
        x1031,
        fx1031,
        t,
        ft,
        cable,
        cond,
        rep_101f1,
        rep_102g1,
        rep_102f2,
        rep_102f2_aux,
        rep_x1031,
        rep_t1,
        ip101,
        ip102,
        rep_ip101,
        rep_ip102,
        w11a,
        w11b,
        j5_in,
        w22,
        w27,
        w29,
        br1,
        core_a,
        j5,
        pd,
    };
    (d, ids)
}

/// The expected records: the fixture helpers with the minted IDs, field by field what the commands'
/// doc comments say they create.
fn expected_records(i: &Ids) -> Vec<Record> {
    let structure = at_loc(i.loc);
    let dev = |mut d: Device| {
        d.structure = structure.clone();
        d
    };
    let pg = |id: Id<Page>, number: u32, title: &str| {
        let mut p = page(id, Some(i.sec), number, title);
        p.structure = structure.clone();
        p
    };
    let func =
        |id: Id<Function>, device: Id<Device>, def, role| Record::Function(function(id, device, def, role));
    let con = |id, page, a, b| connection(id, page, a, b);

    let mut loc = structure_node(i.loc, Aspect::Location, "J-E-2416", None);
    loc.aliases = vec![ident("JE2416")].into();

    let mut x1031 = strip(i.x1031, "X", 1031);
    x1031
        .aspects
        .strip
        .as_mut()
        .expect("strip aspect")
        .groups
        .push(TerminalGroup {
            from_seq: 1,
            to_seq: 4,
            label: MlText::neutral("Pneumatics"),
            extra: Extra::new(),
        });
    let mut d101f1 = device(i.d101f1, "F", 1);
    d101f1.parts = vec![PartUse {
        part: PartRef::new(PART_MCB),
        qty: dec3("1"),
        extra: Extra::new(),
    }];
    let mut d102g1 = device(i.d102g1, "G", 1);
    d102g1.kind = Some(DeviceKind::Psu);
    d102g1.parts = vec![PartUse {
        part: PartRef::new(PART_PSU),
        qty: dec3("1"),
        extra: Extra::new(),
    }];
    let mut terminals: Vec<Device> = Vec::new();
    for (n, des) in ["1", "2", "3"].iter().enumerate() {
        let mut t = terminal(i.t[n], i.x1031, n as u32 + 1, des);
        t.function_text = MlText::neutral("1+");
        terminals.push(dev(t));
    }
    let mut cond_sh = conductor(i.cond[2], i.cable, "SH");
    cond_sh.shield = true;

    let mut rep_x1031 = rep(i.rep_x1031, i.p102, vec![i.fx1031]);
    rep_x1031.rep_type = RepType::StripSegment;

    let mut br1 = con(i.br1, None, cp(i.ft[0], 0), cp(i.ft[2], 0));
    br1.kind = ConnectionKind::Bridge(BridgeSpec {
        kind: BridgeKind::PlugIn,
        group: 1,
        shaft: 1,
        extra: Extra::new(),
    });
    let mut core_a = con(i.core_a, Some(i.p102), cp(i.ft[1], 1), cp(i.ft[2], 1));
    core_a.kind = ConnectionKind::Core(i.cond[0]);

    // The junction of `connection.draw`: style dot, the style's default wiring, at the drawn point.
    let mut j5 = junction(i.j5, i.p102, vec![i.j5_in, i.w22, i.w27]);
    j5.wiring = None;
    j5.at = pt0();

    let mut third = terminals.into_iter();
    let (t1, t2, t3) = (
        third.next().expect("t1"),
        third.next().expect("t2"),
        third.next().expect("t3"),
    );

    let ip101 = ip_function(i.ip101, "L-1", IpDirection::Source);
    let ip102 = ip_function(i.ip102, "L-1", IpDirection::Target);

    vec![
        Record::StructureNode(loc),
        Record::StructureNode(structure_node(i.func_p01, Aspect::Function, "P01", None)),
        Record::Section(section(i.sec, 1, 100, 199, "Wiring Definition")),
        Record::Page(pg(i.p101, 101, "Power Supply 1 - Main Current")),
        Record::Page(pg(i.p102, 102, "Power Supply 2 - Control Current")),
        Record::Device(dev(d101f1)),
        Record::Device(dev(d102g1)),
        Record::Device(dev(device(i.d102f2, "F", 2))),
        Record::Device(dev(x1031)),
        Record::Device(t1),
        Record::Device(t2),
        Record::Device(t3),
        Record::Device(dev(cable(i.cable, "W", 1))),
        func(i.f101f1, i.d101f1, FNDEF_MCB, FnRole::Main),
        func(i.f102g1, i.d102g1, FNDEF_PSU, FnRole::Main),
        func(i.f102f2, i.d102f2, FNDEF_MICO, FnRole::Main),
        func(i.f102f2_aux, i.d102f2, FNDEF_CONTACT, FnRole::Aux),
        func(i.fx1031, i.x1031, FNDEF_STRIP, FnRole::Main),
        func(i.ft[0], i.t[0], FNDEF_TERMINAL, FnRole::Main),
        func(i.ft[1], i.t[1], FNDEF_TERMINAL, FnRole::Main),
        func(i.ft[2], i.t[2], FNDEF_TERMINAL, FnRole::Main),
        Record::Function(ip101),
        Record::Function(ip102),
        Record::Representation(rep(i.rep_101f1, i.p101, vec![i.f101f1])),
        Record::Representation(rep(i.rep_102g1, i.p102, vec![i.f102g1])),
        Record::Representation(rep(i.rep_102f2, i.p102, vec![i.f102f2])),
        Record::Representation(rep(i.rep_102f2_aux, i.p102, vec![i.f102f2_aux])),
        Record::Representation(rep_x1031),
        Record::Representation(rep(i.rep_t1, i.p102, vec![i.ft[0]])),
        Record::Representation(rep(i.rep_ip101, i.p101, vec![i.ip101])),
        Record::Representation(rep(i.rep_ip102, i.p102, vec![i.ip102])),
        Record::Connection(con(i.w11a, Some(i.p101), cp(i.f101f1, 1), cp(i.ip101, 0))),
        Record::Connection(con(i.w11b, Some(i.p102), cp(i.ip102, 0), cp(i.f102g1, 0))),
        Record::Connection(con(i.j5_in, Some(i.p102), cp(i.f102g1, 5), at_junction(i.j5))),
        Record::Connection(con(i.w22, Some(i.p102), at_junction(i.j5), cp(i.f102f2, 2))),
        Record::Connection(con(i.w27, Some(i.p102), at_junction(i.j5), cp(i.ft[1], 0))),
        Record::Connection(con(i.w29, Some(i.p102), cp(i.f102f2, 6), cp(i.ft[0], 0))),
        Record::Connection(br1),
        Record::Connection(core_a),
        Record::Junction(j5),
        Record::Conductor(conductor(i.cond[0], i.cable, "A")),
        Record::Conductor(conductor(i.cond[1], i.cable, "B")),
        Record::Conductor(cond_sh),
        Record::PotentialDef(PotentialDef {
            id: i.pd,
            v: 1,
            at: cp(i.f102g1, 3),
            name: ident("24VDC"),
            ptype: Some(PotentialType::LPlus),
            voltage: Some(dec3("24")),
            env: Default::default(),
            circuit_class: None,
            props: Default::default(),
            source_ref: None,
            extra: Extra::new(),
        }),
    ]
}

fn expected_model(i: &Ids) -> Model {
    Model::from_records(project(), expected_records(i), library()).expect("the expectation is a valid model")
}

fn count(model: &Model, kind: Kind) -> usize {
    lines(model).iter().filter(|(k, _)| k.0 == kind).count()
}

/// Asserts element-wise equality of the canonical lines, naming the first differing key.
fn assert_same_lines(actual: &Model, expected: &Model) {
    let a = lines(actual);
    let b = lines(expected);
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(
            x.0, y.0,
            "the key lists diverge: command-built {:?} against expected {:?}",
            x.0, y.0
        );
        assert_eq!(
            x.1, y.1,
            "record {:?} differs\n  commands: {}\n  expected: {}",
            x.0, x.1, y.1
        );
    }
    assert_eq!(a.len(), b.len(), "record counts differ");
}

#[test]
fn commands_build_the_mini_golden_records() {
    let (d, ids) = build(SEED);
    assert_same_lines(d.e.model(), &expected_model(&ids));
}

#[test]
fn record_counts_match_the_fixture() {
    let (d, _) = build(SEED);
    let fixture_model = golden_model();
    for (kind, n) in [
        (Kind::Device, 8),
        (Kind::Function, 10),
        (Kind::Representation, 8),
        (Kind::Connection, 8),
        (Kind::Junction, 1),
        (Kind::Conductor, 3),
        (Kind::PotentialDef, 1),
        (Kind::Page, 2),
        (Kind::Section, 1),
        (Kind::StructureNode, 2),
    ] {
        assert_eq!(count(d.e.model(), kind), n, "{kind:?} built by commands");
        assert_eq!(count(&fixture_model, kind), n, "{kind:?} in the fixture");
    }
    assert_eq!(
        count(d.e.model(), Kind::Bond),
        0,
        "no command creates a bond in v0"
    );
}

#[test]
fn the_dsl_is_deterministic_per_seed() {
    let (a, _) = build(SEED);
    let (b, _) = build(SEED);
    assert_eq!(lines(a.e.model()), lines(b.e.model()));

    let (c, _) = build(SEED + 1);
    assert_eq!(lines(c.e.model()).len(), lines(a.e.model()).len());
    assert_ne!(
        lines(c.e.model()),
        lines(a.e.model()),
        "another seed mints other IDs"
    );
}

#[test]
fn undoing_every_step_returns_to_the_empty_model() {
    let (mut d, _) = build(SEED);
    let mut steps = 0;
    while d.e.undo(&ew_commands::Origin::Gui).is_ok() {
        steps += 1;
    }
    assert!(steps > 20, "every command is one undo step, got {steps}");
    let empty = empty_model();
    assert_eq!(lines(d.e.model()), lines(&empty));
    assert!(d.e.model().content_eq(&empty));
}
