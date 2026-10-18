// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The mini golden project of doc 20 §4–§6 as typed records (placeholders only), shared by the in-crate tests.

#![allow(dead_code)]

use ew_ids::{Id, Uuid};
use ew_mltext::{Ident, MlText};
use ew_rds::Aspect;
use ew_schema::{
    Bond, BondKind, BridgeKind, BridgeSpec, Cable, Conductor, Connection, ConnectionKind, ContactKind, CpDef,
    Device, DeviceAspects, DeviceKind, DeviceState, Endpoint, Extra, FnDef, FnRole, Folio, FormDef, FormKind,
    Function, FunctionKind, IpDirection, IpSpec, Junction, JunctionStyle, JunctionWiring, LabelMode, LibCopy,
    LibDocument, LibRef, MasterData, NumberSpec, Open, Page, PageType, Part, PartRef, PartUse, PotentialDef,
    PotentialEnv, PotentialType, Project, ProjectKind, ProjectSettings, Record, RepType, Representation,
    Scale, Section, Sha256, SheetSpec, Side, Slot, Strip, StructureAssignment, StructureNode, SymbolDef,
    TagPrint, TagSpec, Terminal, TerminalGroup, WireOverrides, WireProps,
};
use ew_units::{Dec3, Grid, Nm, Pt2};

use crate::Model;

// ---------------------------------------------------------------------------------------------------------
// Identifiers

const fn id<T>(n: u128) -> Id<T> {
    Id::from_uuid(Uuid::from_u128(n))
}

pub(crate) const PROJECT: Id<Project> = id(0x01);
pub(crate) const LOC: Id<StructureNode> = id(0x10);
pub(crate) const FUNC_P01: Id<StructureNode> = id(0x11);
pub(crate) const SEC1: Id<Section> = id(0x20);
pub(crate) const PAGE_101: Id<Page> = id(0x30);
pub(crate) const PAGE_102: Id<Page> = id(0x31);

pub(crate) const FORM: Id<FormDef> = id(0x40);
pub(crate) const SYM: Id<SymbolDef> = id(0x41);
pub(crate) const FNDEF_MCB: Id<FnDef> = id(0x42);
pub(crate) const FNDEF_PSU: Id<FnDef> = id(0x43);
pub(crate) const FNDEF_MICO: Id<FnDef> = id(0x44);
pub(crate) const FNDEF_CONTACT: Id<FnDef> = id(0x45);
pub(crate) const FNDEF_STRIP: Id<FnDef> = id(0x46);
pub(crate) const FNDEF_TERMINAL: Id<FnDef> = id(0x47);
pub(crate) const FNDEF_IP: Id<FnDef> = id(0x48);
pub(crate) const PART_MCB: Id<Part> = id(0x50);
pub(crate) const PART_PSU: Id<Part> = id(0x51);

pub(crate) const DEV_101F1: Id<Device> = id(0x100);
pub(crate) const DEV_102G1: Id<Device> = id(0x101);
pub(crate) const DEV_102F2: Id<Device> = id(0x102);
pub(crate) const DEV_X1031: Id<Device> = id(0x103);
pub(crate) const DEV_T1: Id<Device> = id(0x104);
pub(crate) const DEV_T2: Id<Device> = id(0x105);
pub(crate) const DEV_T3: Id<Device> = id(0x106);
pub(crate) const DEV_3W1: Id<Device> = id(0x107);

pub(crate) const FN_101F1: Id<Function> = id(0x200);
pub(crate) const FN_102G1: Id<Function> = id(0x201);
pub(crate) const FN_102F2: Id<Function> = id(0x202);
pub(crate) const FN_102F2_AUX: Id<Function> = id(0x203);
pub(crate) const FN_X1031: Id<Function> = id(0x204);
pub(crate) const FN_T1: Id<Function> = id(0x205);
pub(crate) const FN_T2: Id<Function> = id(0x206);
pub(crate) const FN_T3: Id<Function> = id(0x207);
pub(crate) const FN_IP_101: Id<Function> = id(0x208);
pub(crate) const FN_IP_102: Id<Function> = id(0x209);

pub(crate) const REP_101F1: Id<Representation> = id(0x300);
pub(crate) const REP_102G1: Id<Representation> = id(0x301);
pub(crate) const REP_102F2: Id<Representation> = id(0x302);
pub(crate) const REP_102F2_AUX: Id<Representation> = id(0x303);
pub(crate) const REP_X1031: Id<Representation> = id(0x304);
pub(crate) const REP_T1: Id<Representation> = id(0x305);
pub(crate) const REP_IP_101: Id<Representation> = id(0x306);
pub(crate) const REP_IP_102: Id<Representation> = id(0x307);

pub(crate) const CON_W11A: Id<Connection> = id(0x400);
pub(crate) const CON_W11B: Id<Connection> = id(0x401);
pub(crate) const CON_J5_IN: Id<Connection> = id(0x402);
pub(crate) const CON_W22: Id<Connection> = id(0x403);
pub(crate) const CON_W27: Id<Connection> = id(0x404);
pub(crate) const CON_W29: Id<Connection> = id(0x405);
pub(crate) const CON_BR1: Id<Connection> = id(0x406);
pub(crate) const CON_CORE_A: Id<Connection> = id(0x407);

pub(crate) const J5: Id<Junction> = id(0x500);
pub(crate) const COND_A: Id<Conductor> = id(0x600);
pub(crate) const COND_B: Id<Conductor> = id(0x601);
pub(crate) const COND_SH: Id<Conductor> = id(0x602);
pub(crate) const PD_24V: Id<PotentialDef> = id(0x700);
pub(crate) const BOND_PE: Id<Bond> = id(0x800);

// ---------------------------------------------------------------------------------------------------------
// Value helpers

fn ident(s: &str) -> Ident {
    s.parse().expect("valid identifier")
}

fn dec3(s: &str) -> Dec3 {
    s.parse().expect("valid decimal")
}

fn pt0() -> Pt2 {
    Pt2::new(Nm(0), Nm(0))
}

fn grid() -> Grid {
    Grid::new(7_937_500, 3).expect("valid grid")
}

fn lib_ref(item: u128) -> LibRef {
    LibRef {
        library: id(0x9100),
        item: id(item),
        version_req: "*".into(),
        extra: Extra::new(),
    }
}

// ---------------------------------------------------------------------------------------------------------
// Record helpers

pub(crate) fn structure_node(
    node_id: Id<StructureNode>,
    aspect: Aspect,
    code: &str,
    parent: Option<Id<StructureNode>>,
) -> StructureNode {
    StructureNode {
        id: node_id,
        v: 1,
        aspect: Open::Known(aspect),
        code: ident(code),
        aliases: vec![],
        parent,
        description: MlText::default(),
        order: 0,
        source_ref: None,
        extra: Extra::new(),
    }
}

pub(crate) fn section(section_id: Id<Section>, number: u16, from: u32, to: u32, title: &str) -> Section {
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

pub(crate) fn page(page_id: Id<Page>, section: Option<Id<Section>>, number: u32, title: &str) -> Page {
    Page {
        id: page_id,
        v: 1,
        folio: Folio {
            section,
            number,
            label: None,
            extra: Extra::new(),
        },
        kind: PageType::MultiLine,
        structure: StructureAssignment {
            location: Some(Slot::Node(LOC)),
            ..Default::default()
        },
        form: Some(FORM),
        title: MlText::neutral(title),
        description_override: None,
        scale: Scale::default(),
        grid: grid(),
        source_ref: None,
        extra: Extra::new(),
    }
}

pub(crate) fn device(device_id: Id<Device>, letter: &str, counter: u32) -> Device {
    Device {
        id: device_id,
        v: 1,
        tag: TagSpec::new(ident(letter), Some(counter)),
        kind: None,
        parent: None,
        position: None,
        structure: StructureAssignment {
            location: Some(Slot::Node(LOC)),
            ..Default::default()
        },
        parts: vec![],
        function_text: MlText::default(),
        process_tag: None,
        tech_data: vec![],
        state: DeviceState::default(),
        source_ref: None,
        aspects: DeviceAspects::default(),
        extra: Extra::new(),
    }
}

pub(crate) fn strip(strip_id: Id<Device>, letter: &str, counter: u32) -> Device {
    let mut d = device(strip_id, letter, counter);
    d.aspects.strip = Some(Strip {
        label_mode: LabelMode::FunctionText,
        designation_scheme: None,
        accessories: vec![],
        tag_print: TagPrint::PerSegment,
        groups: vec![],
        field_side: Side::Side2,
        extra: Extra::new(),
    });
    d
}

pub(crate) fn terminal(terminal_id: Id<Device>, strip: Id<Device>, seq: u32, designation: &str) -> Device {
    let mut d = device(terminal_id, "X", seq);
    d.kind = Some(DeviceKind::Terminal);
    d.aspects.terminal = Some(Terminal {
        strip,
        seq,
        designation: ident(designation),
        levels: vec![],
        rail_bonded: false,
        extra: Extra::new(),
    });
    d
}

pub(crate) fn cable(cable_id: Id<Device>, letter: &str, counter: u32) -> Device {
    let mut d = device(cable_id, letter, counter);
    d.aspects.cable = Some(Cable::default());
    d
}

pub(crate) fn function(
    function_id: Id<Function>,
    device: Id<Device>,
    def: Id<FnDef>,
    role: FnRole,
) -> Function {
    Function {
        id: function_id,
        v: 1,
        device: Some(device),
        def,
        role,
        kind: FunctionKind::Normal,
        cp_overrides: vec![],
        source_ref: None,
        extra: Extra::new(),
    }
}

pub(crate) fn ip_function(function_id: Id<Function>, name: &str, direction: IpDirection) -> Function {
    Function {
        id: function_id,
        v: 1,
        device: None,
        def: FNDEF_IP,
        role: FnRole::Main,
        kind: FunctionKind::Ip(IpSpec {
            name: ident(name),
            direction,
            star_source: false,
            sort: None,
            external: None,
            extra: Extra::new(),
        }),
        cp_overrides: vec![],
        source_ref: None,
        extra: Extra::new(),
    }
}

pub(crate) fn rep(
    rep_id: Id<Representation>,
    page: Id<Page>,
    functions: Vec<Id<Function>>,
) -> Representation {
    Representation {
        id: rep_id,
        v: 1,
        functions,
        page,
        symbol: SYM,
        variant: 0,
        pos: pt0(),
        orient: Default::default(),
        rep_type: RepType::MultiLine,
        pin_map: vec![],
        text_slots: vec![],
        arrangement: None,
        box_: None,
        source_ref: None,
        extra: Extra::new(),
    }
}

pub(crate) fn cp(f: Id<Function>, idx: u16) -> Endpoint {
    Endpoint::Cp(f, idx)
}

pub(crate) fn at_junction(j: Id<Junction>) -> Endpoint {
    Endpoint::Junction(j)
}

pub(crate) fn connection(
    connection_id: Id<Connection>,
    page: Option<Id<Page>>,
    a: Endpoint,
    b: Endpoint,
) -> Connection {
    Connection {
        id: connection_id,
        v: 1,
        a,
        b,
        page,
        route: vec![],
        kind: ConnectionKind::Wire,
        overrides: WireOverrides::default(),
        number: NumberSpec::default(),
        source_ref: None,
        extra: Extra::new(),
    }
}

pub(crate) fn junction(junction_id: Id<Junction>, page: Id<Page>, sequence: Vec<Id<Connection>>) -> Junction {
    Junction {
        id: junction_id,
        v: 1,
        page,
        at: pt0(),
        style: JunctionStyle::Dot,
        wiring: Some(JunctionWiring::Star),
        sequence,
        source_ref: None,
        extra: Extra::new(),
    }
}

pub(crate) fn conductor(conductor_id: Id<Conductor>, cable: Id<Device>, core: &str) -> Conductor {
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

// ---------------------------------------------------------------------------------------------------------
// Library helpers

fn lib_copy<T>(copy_id: Id<T>, item: T) -> LibCopy<T> {
    let n = u128::from(copy_id.as_uuid().as_u128() as u8);
    LibCopy {
        id: copy_id,
        v: 1,
        library_id: id(0x9000),
        item_id: id(0x9001 + n),
        version: "1.0.0".into(),
        sha256: Sha256([0; 32]),
        item,
    }
}

pub(crate) fn fndef(fndef_id: Id<FnDef>, cps: &[&str]) -> LibCopy<FnDef> {
    let item = FnDef {
        meta: Default::default(),
        category: (ident("ew"), ident("test"), ident("fixture")),
        contact_kind: ContactKind::Terminal,
        cps: cps
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
        internal: vec![],
        flags: Default::default(),
        extra: Extra::new(),
    };
    lib_copy(fndef_id, item)
}

pub(crate) fn symbol(symbol_id: Id<SymbolDef>) -> LibCopy<SymbolDef> {
    let item = SymbolDef {
        meta: Default::default(),
        name: MlText::default(),
        keywords: vec![],
        rep_type: RepType::MultiLine,
        fndef: lib_ref(0x9101),
        grid: grid(),
        graphics: vec![],
        cps: vec![],
        text_slots: vec![],
        arrangements: vec![],
        anchor: Default::default(),
        link_points: vec![],
        extra_variants: vec![],
        iec60617_ref: None,
        iso1219_ref: None,
        extra: Extra::new(),
    };
    lib_copy(symbol_id, item)
}

pub(crate) fn form(form_id: Id<FormDef>) -> LibCopy<FormDef> {
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
    lib_copy(form_id, item)
}

pub(crate) fn part(part_id: Id<Part>, part_number: &str) -> LibCopy<Part> {
    let item = Part {
        meta: Default::default(),
        kind: Default::default(),
        manufacturer: lib_ref(0x9101),
        part_number: ident(part_number),
        type_designation: None,
        order_number: None,
        gtin: None,
        description: MlText::default(),
        category: None,
        tech_data: vec![],
        free_props: vec![],
        unit: Default::default(),
        cp_pattern: vec![],
        function_template: vec![],
        sections: Default::default(),
        extra: Extra::new(),
    };
    lib_copy(part_id, item)
}

// ---------------------------------------------------------------------------------------------------------
// The fixture

/// The mini golden project as records and library documents, in the order of the golden files.
pub(crate) fn mini_golden_parts() -> (Project, Vec<Record>, Vec<LibDocument>) {
    let project = Project {
        id: PROJECT,
        v: 1,
        master: MasterData {
            description: MlText::neutral("Expansion Control Cabinets"),
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

    let mut loc = structure_node(LOC, Aspect::Location, "J-E-2416", None);
    loc.aliases = vec![ident("JE2416")];

    let mut x1031 = strip(DEV_X1031, "X", 1031);
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

    let mut dev_101f1 = device(DEV_101F1, "F", 1);
    dev_101f1.parts = vec![PartUse {
        part: PartRef::new(PART_MCB),
        qty: dec3("1"),
        extra: Extra::new(),
    }];

    let mut dev_102g1 = device(DEV_102G1, "G", 1);
    dev_102g1.kind = Some(DeviceKind::Psu);
    dev_102g1.parts = vec![PartUse {
        part: PartRef::new(PART_PSU),
        qty: dec3("1"),
        extra: Extra::new(),
    }];

    let mut t1 = terminal(DEV_T1, DEV_X1031, 1, "1");
    let mut t2 = terminal(DEV_T2, DEV_X1031, 2, "2");
    let mut t3 = terminal(DEV_T3, DEV_X1031, 3, "3");
    for t in [&mut t1, &mut t2, &mut t3] {
        t.function_text = MlText::neutral("1+");
    }

    let mut rep_x1031 = rep(REP_X1031, PAGE_102, vec![FN_X1031]);
    rep_x1031.rep_type = RepType::StripSegment;

    let mut con_br1 = connection(CON_BR1, None, cp(FN_T1, 0), cp(FN_T3, 0));
    con_br1.kind = ConnectionKind::Bridge(BridgeSpec {
        kind: BridgeKind::PlugIn,
        group: 1,
        shaft: 1,
        extra: Extra::new(),
    });

    let mut con_core_a = connection(CON_CORE_A, Some(PAGE_102), cp(FN_T2, 1), cp(FN_T3, 1));
    con_core_a.kind = ConnectionKind::Core(COND_A);

    let mut cond_sh = conductor(COND_SH, DEV_3W1, "SH");
    cond_sh.shield = true;

    let records = vec![
        Record::StructureNode(loc),
        Record::StructureNode(structure_node(FUNC_P01, Aspect::Function, "P01", None)),
        Record::Section(section(SEC1, 1, 100, 199, "Wiring Definition")),
        Record::Page(page(PAGE_101, Some(SEC1), 101, "Power Supply 1 - Main Current")),
        Record::Page(page(
            PAGE_102,
            Some(SEC1),
            102,
            "Power Supply 2 - Control Current",
        )),
        Record::Device(dev_101f1),
        Record::Device(dev_102g1),
        Record::Device(device(DEV_102F2, "F", 2)),
        Record::Device(x1031),
        Record::Device(t1),
        Record::Device(t2),
        Record::Device(t3),
        Record::Device(cable(DEV_3W1, "W", 1)),
        Record::Function(function(FN_101F1, DEV_101F1, FNDEF_MCB, FnRole::Main)),
        Record::Function(function(FN_102G1, DEV_102G1, FNDEF_PSU, FnRole::Main)),
        Record::Function(function(FN_102F2, DEV_102F2, FNDEF_MICO, FnRole::Main)),
        Record::Function(function(FN_102F2_AUX, DEV_102F2, FNDEF_CONTACT, FnRole::Aux)),
        Record::Function(function(FN_X1031, DEV_X1031, FNDEF_STRIP, FnRole::Main)),
        Record::Function(function(FN_T1, DEV_T1, FNDEF_TERMINAL, FnRole::Main)),
        Record::Function(function(FN_T2, DEV_T2, FNDEF_TERMINAL, FnRole::Main)),
        Record::Function(function(FN_T3, DEV_T3, FNDEF_TERMINAL, FnRole::Main)),
        Record::Function(ip_function(FN_IP_101, "L-1", IpDirection::Source)),
        Record::Function(ip_function(FN_IP_102, "L-1", IpDirection::Target)),
        Record::Representation(rep(REP_101F1, PAGE_101, vec![FN_101F1])),
        Record::Representation(rep(REP_102G1, PAGE_102, vec![FN_102G1])),
        Record::Representation(rep(REP_102F2, PAGE_102, vec![FN_102F2])),
        Record::Representation(rep(REP_102F2_AUX, PAGE_102, vec![FN_102F2_AUX])),
        Record::Representation(rep_x1031),
        Record::Representation(rep(REP_T1, PAGE_102, vec![FN_T1])),
        Record::Representation(rep(REP_IP_101, PAGE_101, vec![FN_IP_101])),
        Record::Representation(rep(REP_IP_102, PAGE_102, vec![FN_IP_102])),
        Record::Connection(connection(
            CON_W11A,
            Some(PAGE_101),
            cp(FN_101F1, 1),
            cp(FN_IP_101, 0),
        )),
        Record::Connection(connection(
            CON_W11B,
            Some(PAGE_102),
            cp(FN_IP_102, 0),
            cp(FN_102G1, 0),
        )),
        Record::Connection(connection(
            CON_J5_IN,
            Some(PAGE_102),
            cp(FN_102G1, 5),
            at_junction(J5),
        )),
        Record::Connection(connection(
            CON_W22,
            Some(PAGE_102),
            at_junction(J5),
            cp(FN_102F2, 2),
        )),
        Record::Connection(connection(CON_W27, Some(PAGE_102), at_junction(J5), cp(FN_T2, 0))),
        Record::Connection(connection(CON_W29, Some(PAGE_102), cp(FN_102F2, 6), cp(FN_T1, 0))),
        Record::Connection(con_br1),
        Record::Connection(con_core_a),
        Record::Junction(junction(J5, PAGE_102, vec![CON_J5_IN, CON_W22, CON_W27])),
        Record::Conductor(conductor(COND_A, DEV_3W1, "A")),
        Record::Conductor(conductor(COND_B, DEV_3W1, "B")),
        Record::Conductor(cond_sh),
        Record::PotentialDef(PotentialDef {
            id: PD_24V,
            v: 1,
            at: cp(FN_102G1, 3),
            name: ident("24VDC"),
            ptype: Some(PotentialType::LPlus),
            voltage: Some(dec3("24")),
            env: PotentialEnv::default(),
            circuit_class: None,
            props: WireProps::default(),
            source_ref: None,
            extra: Extra::new(),
        }),
        Record::Bond(Bond {
            id: BOND_PE,
            v: 1,
            a: cp(FN_102G1, 5),
            b: cp(FN_102G1, 2),
            kind: BondKind::Pelv0vPe,
            via: Some(CON_W27),
            source_ref: None,
            extra: Extra::new(),
        }),
    ];

    let library = vec![
        LibDocument::Form(form(FORM)),
        LibDocument::Symbol(symbol(SYM)),
        LibDocument::Fndef(fndef(FNDEF_MCB, &["1", "2", "N", "N"])),
        LibDocument::Fndef(fndef(FNDEF_PSU, &["L", "N", "PE", "+", "+", "-", "-"])),
        LibDocument::Fndef(fndef(
            FNDEF_MICO,
            &[
                "24VDC", "24VDC", "GND", "GND", "13", "14", "Out1", "Out2", "Out3", "Out4",
            ],
        )),
        LibDocument::Fndef(fndef(FNDEF_CONTACT, &["13", "14"])),
        LibDocument::Fndef(fndef(FNDEF_STRIP, &[])),
        LibDocument::Fndef(fndef(FNDEF_TERMINAL, &["a", "b", "c"])),
        LibDocument::Fndef(fndef(FNDEF_IP, &["1"])),
        LibDocument::Part(part(PART_MCB, "A9P52610")),
        LibDocument::Part(part(PART_PSU, "2904602")),
    ];

    (project, records, library)
}

/// The mini golden project as a validated model snapshot.
pub(crate) fn mini_golden() -> Model {
    let (p, r, l) = mini_golden_parts();
    Model::from_records(p, r, l).expect("mini golden is valid")
}

#[test]
fn mini_golden_loads() {
    let m = mini_golden();
    assert_eq!(m.table::<Device>().len(), 8);
    assert_eq!(m.table::<Function>().len(), 10);
    assert_eq!(m.table::<Connection>().len(), 8);
    assert_eq!(m.indexes().functions_by_device().get(&DEV_102F2).count(), 2);
}
