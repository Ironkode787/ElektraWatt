// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Turns a [`Plan`] and a seed into format-level records with deterministic IDs.
//!
//! The record builders are ports of the helpers of `ew-commands`' test fixture (`tests/common/fixture.rs`),
//! which in turn mirror the golden files; the emitter only decides IDs, positions and library contents.

use ew_ids::{Id, Uuid, V7Generator};
use ew_mltext::{Ident, MlText};
use ew_rds::Aspect;
use ew_schema::{
    Cable, Conductor, Connection, ConnectionKind, ContactKind, CpDef, Device, DeviceAspects, DeviceKind,
    DeviceState, Endpoint, Extra, FnDef, FnRole, Folio, FormDef, FormKind, Function, FunctionKind,
    IpDirection, IpSpec, Junction, JunctionStyle, JunctionWiring, LabelMode, LibCopy, LibDocument, LibRef,
    MasterData, NumberSpec, Open, Page, PageType, PotentialDef, PotentialEnv, PotentialType, Project,
    ProjectKind, ProjectSettings, Record, RepType, Representation, Scale, Section, Sha256, SheetSpec, Side,
    Slot, Strip, StructureAssignment, StructureNode, SymbolDef, TagPrint, TagSpec, Terminal, WireOverrides,
    WireProps,
};
use ew_units::{Dec3, Grid, Nm, Pt2};

use super::ids::SynthSource;
use super::topology::{ConnKind, DefKind, DeviceRole, End, FnRolePlan, IpDir, Plan, PotKind, RepKind};
use super::{Generated, Handles};

/// The library all embedded copies claim to come from.
const LIBRARY_ID: Uuid = Uuid::from_u128(0x5e17_0000_0000_0000_0000_0000_0000_0001);

/// Slots per row on a page.
const SLOTS_PER_ROW: usize = 24;

fn ident(s: &str) -> Ident {
    s.parse()
        .unwrap_or_else(|e| panic!("plan identifier {s:?} is not a valid Ident: {e:?}"))
}

fn dec3(s: &str) -> Dec3 {
    s.parse().expect("valid decimal")
}

fn page_grid() -> Grid {
    Grid::new(7_937_500, 3).expect("valid grid")
}

/// The grid position of a page-local slot (exact multiples of the grid).
fn slot_pos(grid: Grid, slot: usize) -> Pt2 {
    let col = (slot % SLOTS_PER_ROW) as i64;
    let row = (slot / SLOTS_PER_ROW) as i64;
    Pt2::new(
        grid.at(3 * (2 + 2 * col)).expect("slot x within range"),
        grid.at(3 * (2 + 2 * row)).expect("slot y within range"),
    )
}

fn lib_copy<T>(copy_id: Id<T>, item_id: Id<T>, item: T) -> LibCopy<T> {
    LibCopy {
        id: copy_id,
        v: 1,
        library_id: Id::from_uuid(LIBRARY_ID),
        item_id: Id::from_uuid(item_id.as_uuid()),
        version: "1.0.0".into(),
        sha256: Sha256([0; 32]),
        item,
    }
}

fn lib_ref() -> LibRef {
    LibRef {
        library: Id::from_uuid(LIBRARY_ID),
        item: Id::from_uuid(Uuid::from_u128(0x5e17_0000_0000_0000_0000_0000_0000_0002)),
        version_req: "*".into(),
        extra: Extra::new(),
    }
}

fn form_copy(copy_id: Id<FormDef>, item_id: Id<FormDef>) -> LibCopy<FormDef> {
    let item = FormDef {
        kind: FormKind::Frame,
        sheet: SheetSpec {
            width: Nm(420_000_000),
            height: Nm(297_000_000),
            extra: Extra::new(),
        },
        page_to_sheet: None,
        grid: None,
        title_block: None,
        graphics: vec![],
        cells: vec![],
        data: None,
        continuation: None,
        meta: Default::default(),
        extra: Extra::new(),
    };
    lib_copy(copy_id, item_id, item)
}

fn symbol_copy(copy_id: Id<SymbolDef>, item_id: Id<SymbolDef>) -> LibCopy<SymbolDef> {
    let item = SymbolDef {
        meta: Default::default(),
        name: MlText::default(),
        keywords: Default::default(),
        rep_type: RepType::MultiLine,
        fndef: lib_ref(),
        grid: page_grid(),
        graphics: vec![],
        cps: Default::default(),
        text_slots: Default::default(),
        arrangements: vec![],
        anchor: Default::default(),
        link_points: vec![],
        extra_variants: vec![],
        iec60617_ref: None,
        iso1219_ref: None,
        extra: Extra::new(),
    };
    lib_copy(copy_id, item_id, item)
}

fn def_category(kind: &DefKind) -> &'static str {
    match kind {
        DefKind::Switch => "switch",
        DefKind::Mcb => "mcb",
        DefKind::Psu => "psu",
        DefKind::Distribution => "distribution",
        DefKind::Strip => "strip",
        DefKind::Terminal => "terminal",
        DefKind::Ip => "ip",
        DefKind::Station => "station",
        DefKind::Module(_) => "module",
        DefKind::Load(_) => "load",
        DefKind::Contact => "contact",
    }
}

fn fndef_copy(copy_id: Id<FnDef>, item_id: Id<FnDef>, kind: &DefKind) -> LibCopy<FnDef> {
    let item = FnDef {
        meta: Default::default(),
        category: (ident("ew"), ident("synth"), ident(def_category(kind))),
        contact_kind: ContactKind::Terminal,
        cps: ew_schema::KeyedList::from_vec(
            kind.cps()
                .iter()
                .enumerate()
                .map(|(i, d)| CpDef {
                    idx: i as u16,
                    designation: ident(d),
                    alias: None,
                    discipline: Default::default(),
                    potential_role: None,
                    source_output: false,
                    optional: false,
                    level: None,
                    side: None,
                    clamp: None,
                    physical: true,
                    extra: Extra::new(),
                })
                .collect(),
        )
        .expect("distinct CP indexes"),
        internal: vec![],
        flags: Default::default(),
        extra: Extra::new(),
    };
    lib_copy(copy_id, item_id, item)
}

fn structure_node(node_id: Id<StructureNode>, aspect: Aspect, code: &str) -> StructureNode {
    StructureNode {
        id: node_id,
        v: 1,
        aspect: Open::Known(aspect),
        code: ident(code),
        aliases: Default::default(),
        parent: None,
        description: MlText::default(),
        order: 0,
        source_ref: None,
        extra: Extra::new(),
    }
}

fn section(section_id: Id<Section>, number: u16, from: u32, to: u32, title: &str) -> Section {
    Section {
        id: section_id,
        v: 1,
        number,
        folio_range: (from, to),
        title: MlText::neutral(title),
        divider: false,
        source_ref: None,
        extra: Extra::new(),
    }
}

fn page(
    page_id: Id<Page>,
    section: Id<Section>,
    number: u32,
    title: &str,
    location: Id<StructureNode>,
    form: Id<FormDef>,
) -> Page {
    Page {
        id: page_id,
        v: 1,
        folio: Folio {
            section: Some(section),
            number,
            label: None,
            extra: Extra::new(),
        },
        kind: PageType::MultiLine,
        structure: StructureAssignment {
            location: Some(Slot::Node(location)),
            ..Default::default()
        },
        form: Some(form),
        title: MlText::neutral(title),
        description_override: None,
        scale: Scale::default(),
        grid: page_grid(),
        source_ref: None,
        extra: Extra::new(),
    }
}

fn device(device_id: Id<Device>, letter: &str, counter: u32, location: Id<StructureNode>) -> Device {
    Device {
        id: device_id,
        v: 1,
        tag: TagSpec::new(ident(letter), Some(counter)),
        kind: None,
        parent: None,
        position: None,
        structure: StructureAssignment {
            location: Some(Slot::Node(location)),
            ..Default::default()
        },
        parts: vec![],
        function_text: MlText::default(),
        process_tag: None,
        tech_data: Default::default(),
        state: DeviceState::default(),
        source_ref: None,
        aspects: DeviceAspects::default(),
        extra: Extra::new(),
    }
}

fn strip_aspect() -> Strip {
    Strip {
        label_mode: LabelMode::FunctionText,
        designation_scheme: None,
        accessories: vec![],
        tag_print: TagPrint::PerSegment,
        groups: vec![],
        field_side: Side::Side2,
        extra: Extra::new(),
    }
}

fn function(function_id: Id<Function>, device: Id<Device>, def: Id<FnDef>, role: FnRole) -> Function {
    Function {
        id: function_id,
        v: 1,
        device: Some(device),
        def,
        role,
        kind: FunctionKind::Normal,
        cp_overrides: Default::default(),
        source_ref: None,
        extra: Extra::new(),
    }
}

fn ip_function(function_id: Id<Function>, def: Id<FnDef>, name: &str, direction: IpDirection) -> Function {
    Function {
        id: function_id,
        v: 1,
        device: None,
        def,
        role: FnRole::Main,
        kind: FunctionKind::Ip(IpSpec {
            name: ident(name),
            direction,
            star_source: false,
            sort: None,
            external: None,
            extra: Extra::new(),
        }),
        cp_overrides: Default::default(),
        source_ref: None,
        extra: Extra::new(),
    }
}

fn connection(
    connection_id: Id<Connection>,
    page: Id<Page>,
    a: Endpoint,
    b: Endpoint,
    kind: ConnectionKind,
) -> Connection {
    Connection {
        id: connection_id,
        v: 1,
        a,
        b,
        page: Some(page),
        route: vec![],
        kind,
        overrides: WireOverrides::default(),
        number: NumberSpec::default(),
        source_ref: None,
        extra: Extra::new(),
    }
}

fn conductor(conductor_id: Id<Conductor>, cable: Id<Device>, core: &str) -> Conductor {
    Conductor {
        id: conductor_id,
        v: 1,
        cable,
        core: ident(core),
        colour: None,
        csa: None,
        shield: false,
        pair: None,
        source_ref: None,
        extra: Extra::new(),
    }
}

/// Mints `n` IDs.
fn mint<T>(ids: &mut V7Generator<SynthSource>, n: usize) -> Vec<Id<T>> {
    (0..n).map(|_| ids.next()).collect()
}

/// Emits the records of `plan`; see [`super::emit`].
pub(super) fn emit(plan: &Plan, seed: u64) -> Generated {
    let mut ids = V7Generator::new(SynthSource::new(seed));
    let class = plan.class;
    let grid = page_grid();
    let replicas = plan.replicas.len();

    // IDs, in the documented fixed order.
    let project_id: Id<Project> = ids.next();
    let func_node: Id<StructureNode> = ids.next();
    let loc_nodes: Vec<Id<StructureNode>> = mint(&mut ids, replicas);
    let section_ids: Vec<Id<Section>> = mint(&mut ids, replicas);
    let page_ids: Vec<Id<Page>> = mint(&mut ids, plan.pages.len());
    let form_id: Id<FormDef> = ids.next();
    let form_item: Id<FormDef> = ids.next();
    let symbol_id: Id<SymbolDef> = ids.next();
    let symbol_item: Id<SymbolDef> = ids.next();
    let fndef_ids: Vec<(Id<FnDef>, Id<FnDef>)> = (0..plan.fndefs.len())
        .map(|_| {
            let copy: Id<FnDef> = ids.next();
            let item: Id<FnDef> = ids.next();
            (copy, item)
        })
        .collect();
    let device_ids: Vec<Id<Device>> = mint(&mut ids, plan.devices.len());
    let function_ids: Vec<Id<Function>> = mint(&mut ids, plan.functions.len());
    let rep_ids: Vec<Id<Representation>> = mint(&mut ids, plan.reps.len());
    let junction_ids: Vec<Id<Junction>> = mint(&mut ids, plan.junctions.len());
    let connection_ids: Vec<Id<Connection>> = mint(&mut ids, plan.connections.len());
    let conductor_ids: Vec<Id<Conductor>> = mint(&mut ids, plan.conductors.len());
    let potential_ids: Vec<Id<PotentialDef>> = mint(&mut ids, plan.potentials.len());

    let project = Project {
        id: project_id,
        v: 1,
        master: MasterData {
            description: MlText::neutral(&format!("Synthetic {} fixture, seed {seed}", class.name())),
            ..Default::default()
        },
        parties: vec![],
        supply: None,
        control_voltage: None,
        rated_consumption: None,
        settings: ProjectSettings::default(),
        languages: None,
        derivation_profile: ident("2027.1"),
        kind: ProjectKind::Schematic,
        source_ref: None,
        extra: Extra::new(),
    };

    let ep = |e: End| match e {
        End::Cp(f, idx) => Endpoint::Cp(function_ids[f], idx),
        End::Junction(j) => Endpoint::Junction(junction_ids[j]),
    };

    let mut records: Vec<Record> = Vec::with_capacity(
        1 + replicas * 2
            + plan.pages.len()
            + plan.devices.len()
            + plan.functions.len()
            + plan.reps.len()
            + plan.junctions.len()
            + plan.connections.len()
            + plan.conductors.len()
            + plan.potentials.len(),
    );

    // Structure.
    records.push(Record::StructureNode(structure_node(
        func_node,
        Aspect::Function,
        "P01",
    )));
    for (u, node) in loc_nodes.iter().enumerate() {
        records.push(Record::StructureNode(structure_node(
            *node,
            Aspect::Location,
            &format!("C{:04}", u + 1),
        )));
    }

    // Sections.
    for (u, s) in section_ids.iter().enumerate() {
        records.push(Record::Section(section(
            *s,
            (u + 1) as u16,
            1,
            999,
            &format!("Cabinet {}", u + 1),
        )));
    }

    // Pages.
    for (p, pp) in plan.pages.iter().enumerate() {
        records.push(Record::Page(page(
            page_ids[p],
            section_ids[pp.replica],
            pp.folio,
            &format!("Cabinet {} folio {}", pp.replica + 1, pp.folio),
            loc_nodes[pp.replica],
            form_id,
        )));
    }

    // Devices.
    for (d, dp) in plan.devices.iter().enumerate() {
        let mut dev = device(device_ids[d], dp.letter, dp.counter, loc_nodes[dp.replica]);
        match dp.role {
            DeviceRole::Strip { .. } => dev.aspects.strip = Some(strip_aspect()),
            DeviceRole::Terminal { strip, seq } => {
                dev.kind = Some(DeviceKind::Terminal);
                dev.aspects.terminal = Some(Terminal {
                    strip: device_ids[strip],
                    seq,
                    designation: ident(&seq.to_string()),
                    levels: vec![],
                    rail_bonded: false,
                    extra: Extra::new(),
                });
            }
            DeviceRole::Cable => dev.aspects.cable = Some(Cable::default()),
            DeviceRole::Psu => dev.kind = Some(DeviceKind::Psu),
            _ => {}
        }
        records.push(Record::Device(dev));
    }

    // Functions.
    for (f, fp) in plan.functions.iter().enumerate() {
        let def = fndef_ids[fp.def].0;
        let rec = match (&fp.ip, fp.device) {
            (Some(ip), _) => ip_function(
                function_ids[f],
                def,
                &ip.name,
                match ip.direction {
                    IpDir::Source => IpDirection::Source,
                    IpDir::Target => IpDirection::Target,
                },
            ),
            (None, Some(d)) => function(
                function_ids[f],
                device_ids[d],
                def,
                match fp.role {
                    FnRolePlan::Main => FnRole::Main,
                    FnRolePlan::Aux => FnRole::Aux,
                },
            ),
            (None, None) => panic!("plan function {f} has neither a device nor an interruption point"),
        };
        records.push(Record::Function(rec));
    }

    // Representations.
    for (r, rp) in plan.reps.iter().enumerate() {
        records.push(Record::Representation(Representation {
            id: rep_ids[r],
            v: 1,
            functions: vec![function_ids[rp.function]],
            page: page_ids[rp.page],
            symbol: symbol_id,
            variant: 0,
            pos: slot_pos(grid, rp.slot),
            orient: Default::default(),
            rep_type: match rp.kind {
                RepKind::MultiLine => RepType::MultiLine,
                RepKind::StripSegment => RepType::StripSegment,
            },
            pin_map: Default::default(),
            text_slots: Default::default(),
            arrangement: None,
            box_: None,
            source_ref: None,
            extra: Extra::new(),
        }));
    }

    // Junctions.
    for (j, jp) in plan.junctions.iter().enumerate() {
        records.push(Record::Junction(Junction {
            id: junction_ids[j],
            v: 1,
            page: page_ids[jp.page],
            at: slot_pos(grid, jp.slot),
            style: JunctionStyle::Dot,
            wiring: Some(JunctionWiring::Star),
            sequence: jp.sequence.iter().map(|c| connection_ids[*c]).collect(),
            source_ref: None,
            extra: Extra::new(),
        }));
    }

    // Connections.
    for (c, cp) in plan.connections.iter().enumerate() {
        records.push(Record::Connection(connection(
            connection_ids[c],
            page_ids[cp.page],
            ep(cp.a),
            ep(cp.b),
            match cp.kind {
                ConnKind::Wire => ConnectionKind::Wire,
                ConnKind::Core(k) => ConnectionKind::Core(conductor_ids[k]),
            },
        )));
    }

    // Conductors.
    for (k, cp) in plan.conductors.iter().enumerate() {
        records.push(Record::Conductor(conductor(
            conductor_ids[k],
            device_ids[cp.cable],
            cp.core,
        )));
    }

    // Potential definitions.
    for (i, pp) in plan.potentials.iter().enumerate() {
        let (ptype, voltage) = match pp.kind {
            PotKind::L => (PotentialType::L, Some(dec3("230"))),
            PotKind::N => (PotentialType::N, None),
            PotKind::Pe => (PotentialType::Pe, None),
            PotKind::Plus24 => (PotentialType::LPlus, Some(dec3("24"))),
            PotKind::Zero => (PotentialType::LMinus, Some(dec3("0"))),
        };
        records.push(Record::PotentialDef(PotentialDef {
            id: potential_ids[i],
            v: 1,
            at: Endpoint::Cp(function_ids[pp.at.0], pp.at.1),
            name: ident(&pp.name),
            ptype: Some(ptype),
            voltage,
            env: PotentialEnv::default(),
            circuit_class: None,
            props: WireProps::default(),
            source_ref: None,
            extra: Extra::new(),
        }));
    }

    // Library.
    let mut lib = Vec::with_capacity(2 + plan.fndefs.len());
    lib.push(LibDocument::Form(form_copy(form_id, form_item)));
    lib.push(LibDocument::Symbol(symbol_copy(symbol_id, symbol_item)));
    for (kind, (copy, item)) in plan.fndefs.iter().zip(fndef_ids.iter().copied()) {
        lib.push(LibDocument::Fndef(fndef_copy(copy, item, kind)));
    }

    let handles = Handles {
        pages: page_ids,
        load_reps: plan.load_reps.iter().map(|r| rep_ids[*r]).collect(),
        free_cps: plan
            .free_cps
            .iter()
            .map(|page| page.iter().map(|(f, idx)| (function_ids[*f], *idx)).collect())
            .collect(),
        grid,
    };

    Generated {
        class,
        seed,
        project,
        records,
        lib,
        counts: plan.counts(),
        handles,
    }
}
