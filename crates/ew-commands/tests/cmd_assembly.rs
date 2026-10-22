// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `assembly.attach`, `valve.add`, `device.link`, `plc.config_project.create`, `plc.bind_channel`,
//! `project.supply.set`, `library.embed`, the `assembly`/`plc_station`/`plc_module`/`tag.scheme` paths of
//! `fields.set` and the channel instantiation normaliser (N1).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddValve, AttachChildren, BindChannel, ChildSpec, CreatePlcConfigProject, EmbedLibraryCopies,
    EndpointParam, KindName, LinkDevices, SetFields, SetProjectSupply,
};
use ew_commands::{CmdError, Engine, Receipt};
use ew_ids::Id;
use ew_model::{Entity, Model};
use ew_schema::{
    Channel, ChannelKind, Device, DeviceLink, DeviceLinkKind, Endpoint, ExternalRef, Extra, FnRole, FormDef,
    Function, FunctionKind, Kind, LibCopy, LibDocument, Part, PartRef, PartUse, PlcConfigProject, PlcSection,
    Record, SupplySpec, TagScheme,
};
use serde_json::{Value, json};

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

fn exists<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> bool {
    e.model().table::<T>().get(&id).is_some()
}

fn created<T>(r: &Receipt, kind: Kind) -> Vec<Id<T>> {
    r.created
        .iter()
        .filter(|k| k.0 == kind)
        .map(|k| typed(*k))
        .collect()
}

fn primary<T>(r: &Receipt) -> Id<T> {
    typed(r.outcome.primary.expect("primary outcome"))
}

fn rejected<T: std::fmt::Debug>(r: Result<T, CmdError>) {
    match r {
        Err(CmdError::Rejected { .. }) => {}
        other => panic!("expected a rejection, got {other:?}"),
    }
}

fn not_found<T: std::fmt::Debug>(r: Result<T, CmdError>) {
    match r {
        Err(CmdError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

fn invalid_path<T: std::fmt::Debug>(r: Result<T, CmdError>, command: &str) -> String {
    match r {
        Err(CmdError::InvalidParams { command: c, path, .. }) => {
            assert_eq!(c, command);
            path
        }
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

fn child(device: Id<Device>, position: Option<u32>) -> ChildSpec {
    ChildSpec { device, position }
}

fn attach(parent: Id<Device>, children: Vec<ChildSpec>) -> AttachChildren {
    AttachChildren { parent, children }
}

fn set_device(device: Id<Device>, path: &str, value: Value) -> SetFields {
    SetFields {
        kind: KindName::Device,
        targets: vec![device.cast()],
        path: path.into(),
        value,
    }
}

// ---------------------------------------------------------------------------------------------------------
// assembly.attach

#[test]
fn attach_sets_parent_and_positions_and_undo_restores() {
    let mut e = engine(golden_model());
    let r = e
        .execute(&attach(
            DEV_102G1,
            vec![child(DEV_102F2, Some(1)), child(DEV_101F1, None)],
        ))
        .expect("assembly.attach");
    assert_eq!(r.outcome.primary, Some((Kind::Device, DEV_102G1.cast())));
    let a = row(&e, DEV_102F2);
    assert_eq!((a.parent, a.position), (Some(DEV_102G1), Some(1)));
    let b = row(&e, DEV_101F1);
    assert_eq!((b.parent, b.position), (Some(DEV_102G1), None));
    e.undo().expect("undo");
    for d in [DEV_102F2, DEV_101F1] {
        let r = row(&e, d);
        assert_eq!((r.parent, r.position), (None, None));
    }
}

#[test]
fn attach_rejects_malformed_and_conflicting_requests() {
    let mut e = engine(golden_model());
    let c = "assembly.attach";
    assert_eq!(
        invalid_path(e.execute(&attach(DEV_102G1, vec![])), c),
        "/children"
    );
    assert_eq!(
        invalid_path(
            e.execute(&attach(
                DEV_102G1,
                vec![child(DEV_102F2, Some(1)), child(DEV_102F2, Some(2))]
            )),
            c
        ),
        "/children/1/device"
    );
    assert_eq!(
        invalid_path(e.execute(&attach(DEV_102G1, vec![child(DEV_102G1, None)])), c),
        "/children/0/device"
    );
    assert_eq!(
        invalid_path(e.execute(&attach(DEV_102G1, vec![child(DEV_102F2, Some(0))])), c),
        "/children/0/position"
    );
    rejected(e.execute(&attach(
        DEV_102G1,
        vec![child(DEV_102F2, Some(1)), child(DEV_101F1, Some(1))],
    )));
    not_found(e.execute(&attach(DEV_102G1, vec![child(id(0xdead), None)])));
    // a position another child holds from an earlier attach
    e.execute(&attach(DEV_102G1, vec![child(DEV_102F2, Some(1))]))
        .expect("first attach");
    rejected(e.execute(&attach(DEV_102G1, vec![child(DEV_101F1, Some(1))])));
    // the parent's ancestor cannot become its child
    e.execute(&attach(DEV_101F1, vec![child(DEV_T1, None)]))
        .expect("A under B");
    rejected(e.execute(&attach(DEV_T1, vec![child(DEV_101F1, None)])));
}

#[test]
fn attach_moves_a_child_to_another_parent() {
    let mut e = engine(golden_model());
    e.execute(&attach(DEV_102G1, vec![child(DEV_102F2, Some(1))]))
        .expect("first");
    e.execute(&attach(DEV_101F1, vec![child(DEV_102F2, Some(1))]))
        .expect("move");
    assert_eq!(row(&e, DEV_102F2).parent, Some(DEV_101F1));
}

// ---------------------------------------------------------------------------------------------------------
// valve.add

fn mcb_use() -> PartUse {
    PartUse {
        part: PartRef::new(PART_MCB),
        qty: dec3("1"),
        extra: Extra::new(),
    }
}

fn valve(module: Id<Device>, position: Option<u32>, fndef: Option<Id<ew_schema::FnDef>>) -> AddValve {
    AddValve {
        module,
        position,
        part: mcb_use(),
        letter: None,
        fndef,
        structure: Default::default(),
    }
}

#[test]
fn valve_add_takes_the_lowest_free_position_and_creates_the_function() {
    let mut e = engine(golden_model());
    e.execute(&attach(DEV_102G1, vec![child(DEV_101F1, Some(1))]))
        .expect("attach");
    let r = e
        .execute(&valve(DEV_102G1, None, Some(FNDEF_MCB)))
        .expect("valve.add");
    let v: Id<Device> = primary(&r);
    let d = row(&e, v);
    assert_eq!(d.tag.letter, ident("Q"));
    assert_eq!(d.tag.counter, Some(2));
    assert_eq!(d.tag.scheme, TagScheme::Child);
    assert_eq!((d.parent, d.position), (Some(DEV_102G1), Some(2)));
    assert_eq!(d.parts.len(), 1);
    let fns: Vec<Id<Function>> = created(&r, Kind::Function);
    assert_eq!(fns.len(), 1);
    let f = row(&e, fns[0]);
    assert_eq!(f.device, Some(v));
    assert_eq!(f.def, FNDEF_MCB);
    assert_eq!(f.role, FnRole::Main);
    assert_eq!(f.kind, FunctionKind::Normal);
}

#[test]
fn valve_add_checks_positions_and_undo_removes_the_valve() {
    let mut e = engine(golden_model());
    let c = "valve.add";
    e.execute(&attach(DEV_102G1, vec![child(DEV_101F1, Some(1))]))
        .expect("attach");
    let r = e
        .execute(&valve(DEV_102G1, Some(5), Some(FNDEF_MCB)))
        .expect("free position");
    let v: Id<Device> = primary(&r);
    assert_eq!(row(&e, v).position, Some(5));
    let f: Id<Function> = created(&r, Kind::Function)[0];
    rejected(e.execute(&valve(DEV_102G1, Some(1), None)));
    rejected(e.execute(&valve(DEV_102G1, Some(5), None)));
    assert_eq!(
        invalid_path(e.execute(&valve(DEV_102G1, Some(0), None)), c),
        "/position"
    );
    not_found(e.execute(&valve(id(0xdead), None, None)));
    e.undo().expect("undo");
    assert!(!exists(&e, v));
    assert!(!exists(&e, f));
}

// ---------------------------------------------------------------------------------------------------------
// device.link

#[test]
fn device_link_creates_and_validates() {
    let mut e = engine(golden_model());
    let link = |from, to| LinkDevices {
        from,
        to,
        kind: DeviceLinkKind::Drives,
    };
    let r = e.execute(&link(DEV_102F2, DEV_101F1)).expect("device.link");
    let l = row(&e, primary::<DeviceLink>(&r));
    assert_eq!(
        (l.from, l.to, l.kind.clone()),
        (DEV_102F2, DEV_101F1, DeviceLinkKind::Drives)
    );
    assert_eq!(
        invalid_path(e.execute(&link(DEV_102F2, DEV_102F2)), "device.link"),
        "/to"
    );
    rejected(e.execute(&link(DEV_102F2, DEV_101F1)));
    not_found(e.execute(&link(DEV_102F2, id(0xdead))));
    not_found(e.execute(&link(id(0xdead), DEV_102F2)));
}

// ---------------------------------------------------------------------------------------------------------
// plc.config_project.create

fn config_project(name: &str, profile: &str, cpu: Option<Id<Device>>) -> CreatePlcConfigProject {
    CreatePlcConfigProject {
        name: ident(name),
        profile: ident(profile),
        cpu,
    }
}

#[test]
fn config_project_create_validates_name_profile_and_cpu() {
    let mut e = engine(golden_model());
    let c = "plc.config_project.create";
    let r = e
        .execute(&config_project("PLC1", "siemens-tia-intl@2027.1", None))
        .expect("create");
    let p = row(&e, primary::<PlcConfigProject>(&r));
    assert_eq!(p.name, ident("PLC1"));
    assert_eq!(p.profile, ident("siemens-tia-intl@2027.1"));
    assert_eq!(p.cpu, None);
    rejected(e.execute(&config_project("PLC1", "siemens-tia-intl@2027.1", None)));
    for bad in ["siemens", "@1", "a@"] {
        assert_eq!(
            invalid_path(e.execute(&config_project("PLC2", bad, None)), c),
            "/profile",
            "{bad}"
        );
    }
    not_found(e.execute(&config_project("PLC3", "a@1", Some(id(0xdead)))));
}

// ---------------------------------------------------------------------------------------------------------
// PLC fixture

const MODULE_PART: Id<Part> = id(0x58);
const XREF: Id<ExternalRef> = id(0x900);

fn module_part(part_id: Id<Part>) -> LibCopy<Part> {
    let mut copy = part(part_id, "533361");
    let plc: PlcSection =
        serde_json::from_value(json!({"role": "module", "channels": {"count": 8, "kind": "do"}}))
            .expect("plc section");
    copy.item.sections.plc = Some(plc);
    copy
}

fn plc_model() -> Model {
    let (mut p, mut records, mut lib) = mini_golden_parts();
    lib.push(LibDocument::Part(module_part(MODULE_PART)));
    let supply: SupplySpec =
        serde_json::from_value(json!({"phases": 1, "u_nom": "230", "frequency": "50"})).expect("supply");
    p.supply = Some(supply);
    records.push(Record::ExternalRef(ExternalRef {
        id: XREF,
        v: 1,
        project: None,
        location: LOC,
        device: ident("JE-N1000-03"),
        connection: None,
        source_ref: None,
        extra: Extra::new(),
    }));
    Model::from_records(p, records, lib).expect("plc model is valid")
}

fn module_use() -> Value {
    json!([{"part": {"copy": MODULE_PART}, "qty": "1"}])
}

fn channel_id(device: Id<Device>, n: u16) -> Id<Channel> {
    ew_ids::v5::plc_channel_id(device, n)
}

/// Gives `DEV_102F2` the module part and the `plc_module` aspect; returns the aspect step's receipt.
fn make_module(e: &mut Engine) -> Receipt {
    e.execute(&set_device(DEV_102F2, "parts", module_use()))
        .expect("parts");
    e.execute(&set_device(DEV_102F2, "plc_module", json!({})))
        .expect("plc_module")
}

#[test]
fn plc_module_aspect_instantiates_its_channels() {
    let mut e = engine(plc_model());
    let r = make_module(&mut e);
    let made: Vec<Id<Channel>> = created(&r, Kind::Channel);
    assert_eq!(made.len(), 8);
    for n in 0..8u16 {
        let id = channel_id(DEV_102F2, n);
        assert!(made.contains(&id));
        let c = row(&e, id);
        assert_eq!(c.module, DEV_102F2);
        assert_eq!(c.number, n);
        assert_eq!(c.kind, ChannelKind::Do);
        assert_eq!(c.binding, None);
    }
    assert_eq!(e.model().table::<Channel>().iter().count(), 8);
    // no change: no new channel
    let again = e
        .execute(&set_device(DEV_102F2, "plc_module", json!({})))
        .expect("again");
    assert!(created::<Channel>(&again, Kind::Channel).is_empty());
    assert_eq!(e.model().table::<Channel>().iter().count(), 8);
}

#[test]
fn undo_of_the_module_aspect_removes_the_channels() {
    let mut e = engine(plc_model());
    make_module(&mut e);
    e.undo().expect("undo");
    assert_eq!(e.model().table::<Channel>().iter().count(), 0);
}

#[test]
fn plc_module_without_a_channel_spec_creates_no_channel() {
    let mut e = engine(plc_model());
    let r = e
        .execute(&set_device(DEV_101F1, "plc_module", json!({})))
        .expect("plc_module");
    assert!(created::<Channel>(&r, Kind::Channel).is_empty());
    assert!(row(&e, DEV_101F1).aspects.plc_module.is_some());
}

#[test]
fn plc_station_aspect_is_set_whole_and_members_need_it() {
    let mut e = engine(plc_model());
    let r = e
        .execute(&config_project("PLC1", "siemens-tia-intl@2027.1", None))
        .expect("config project");
    let cp: Id<PlcConfigProject> = primary(&r);
    e.execute(&set_device(
        DEV_102F2,
        "plc_station",
        json!({"config_project": cp, "role": "valve_terminal"}),
    ))
    .expect("plc_station");
    assert!(row(&e, DEV_102F2).aspects.plc_station.is_some());
    assert_eq!(
        invalid_path(
            e.execute(&set_device(DEV_101F1, "plc_station.order", json!(3))),
            "fields.set"
        ),
        "/path"
    );
}

#[test]
fn assembly_aspect_covers_the_subtree_only() {
    let mut e = engine(plc_model());
    e.execute(&attach(DEV_102G1, vec![child(DEV_102F2, Some(1))]))
        .expect("attach");
    e.execute(&set_device(
        DEV_102G1,
        "assembly",
        json!({"configs": [{"order_code": "X1", "covers": [DEV_102G1, DEV_102F2]}]}),
    ))
    .expect("assembly");
    assert!(row(&e, DEV_102G1).aspects.assembly.is_some());
    assert_eq!(
        invalid_path(
            e.execute(&set_device(
                DEV_102G1,
                "assembly",
                json!({"configs": [{"order_code": "X1", "covers": [DEV_101F1]}]}),
            )),
            "fields.set"
        ),
        "/value"
    );
}

#[test]
fn tag_scheme_child_is_settable() {
    let mut e = engine(plc_model());
    e.execute(&set_device(DEV_102F2, "tag.scheme", json!("child")))
        .expect("tag.scheme");
    assert_eq!(row(&e, DEV_102F2).tag.scheme, TagScheme::Child);
}

// ---------------------------------------------------------------------------------------------------------
// plc.bind_channel

fn bind(channel: Id<Channel>, endpoint: Option<EndpointParam>) -> BindChannel {
    BindChannel { channel, endpoint }
}

fn cp_param(function: Id<Function>, idx: u16) -> Option<EndpointParam> {
    Some(EndpointParam::Cp { function, idx })
}

#[test]
fn bind_channel_binds_rebinds_and_unbinds() {
    let mut e = engine(plc_model());
    make_module(&mut e);
    let (c1, c3) = (channel_id(DEV_102F2, 1), channel_id(DEV_102F2, 3));
    // FNDEF_MICO defines CPs 0..=9
    let r = e
        .execute(&bind(c1, cp_param(FN_102F2, 4)))
        .expect("bind channel 1");
    assert_eq!(primary::<Channel>(&r), c1);
    assert!(r.outcome.notices.is_empty());
    assert_eq!(row(&e, c1).binding, Some(Endpoint::Cp(FN_102F2, 4)));
    let r = e
        .execute(&bind(c3, cp_param(FN_102F2, 4)))
        .expect("bind channel 3");
    assert_eq!(r.outcome.notices.len(), 1);
    assert_eq!(row(&e, c1).binding, None);
    assert_eq!(row(&e, c3).binding, Some(Endpoint::Cp(FN_102F2, 4)));
    e.execute(&bind(c3, None)).expect("unbind");
    assert_eq!(row(&e, c3).binding, None);
}

#[test]
fn bind_channel_rejects_bad_endpoints_and_unknown_channels() {
    let mut e = engine(plc_model());
    make_module(&mut e);
    let c1 = channel_id(DEV_102F2, 1);
    rejected(e.execute(&bind(c1, cp_param(FN_102F2, 99))));
    let junction = Some(EndpointParam::Junction { junction: J5 });
    assert_eq!(
        invalid_path(e.execute(&bind(c1, junction)), "plc.bind_channel"),
        "/endpoint"
    );
    not_found(e.execute(&bind(id(0xdead), None)));
}

// ---------------------------------------------------------------------------------------------------------
// project.supply.set

#[test]
fn supply_set_edits_one_member() {
    let mut e = engine(plc_model());
    e.execute(&SetProjectSupply {
        path: "origin.source".into(),
        value: json!(XREF),
    })
    .expect("origin.source");
    let origin = e
        .model()
        .project()
        .supply
        .as_ref()
        .and_then(|s| s.origin.clone())
        .expect("origin");
    assert_eq!(origin.source, Some(XREF));
    let c = "project.supply.set";
    let set = |path: &str, value: Value| SetProjectSupply {
        path: path.into(),
        value,
    };
    assert_eq!(invalid_path(e.execute(&set("bogus", json!(1))), c), "/path");
    assert_eq!(invalid_path(e.execute(&set("phases", json!("x"))), c), "/value");
    not_found(e.execute(&set("origin.source", json!(id::<ExternalRef>(0xdead)))));
}

#[test]
fn supply_set_is_rejected_without_a_supply() {
    let mut e = engine(golden_model());
    rejected(e.execute(&SetProjectSupply {
        path: "phases".into(),
        value: json!(3),
    }));
}

// ---------------------------------------------------------------------------------------------------------
// library.embed

fn hex_sha(copy: &LibCopy<FormDef>) -> String {
    use sha2::Digest;
    let text = serde_json::to_string(&copy.item).expect("item json");
    let digest = sha2::Sha256::digest(text.as_bytes());
    let mut s = String::from("sha256:");
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// A form copy with true lineage: library `lib`, item `item`.
fn embeddable(lib: u128, item: u128) -> (Id<FormDef>, Value) {
    let copy_id: Id<FormDef> = ew_ids::v5::lib_copy_id(uuid(lib), uuid(item));
    let mut copy = form(copy_id);
    copy.library_id = id(lib);
    copy.item_id = id(item);
    let sha = hex_sha(&copy);
    let mut v = serde_json::to_value(&copy).expect("copy json");
    v["$k"] = json!("form");
    v["sha256"] = json!(sha);
    (copy_id, v)
}

fn embed(copies: Vec<Value>) -> EmbedLibraryCopies {
    EmbedLibraryCopies { copies }
}

#[test]
fn library_embed_inserts_a_copy_once_and_undo_removes_it() {
    let mut e = engine(golden_model());
    let (id, json) = embeddable(0xa0001, 0xa0002);
    let r = e.execute(&embed(vec![json.clone()])).expect("library.embed");
    assert_eq!(r.outcome.primary, Some((Kind::Form, id.cast())));
    assert!(has_form(&e, id));
    rejected(e.execute(&embed(vec![json])));
    e.undo().expect("undo");
    assert!(!has_form(&e, id));
}

fn has_form(e: &Engine, id: Id<FormDef>) -> bool {
    e.model().table::<LibCopy<FormDef>>().get(&id).is_some()
}

#[test]
fn library_embed_validates_every_copy() {
    let mut e = engine(golden_model());
    let c = "library.embed";
    let (copy_id, good) = embeddable(0xa0001, 0xa0002);
    assert_eq!(
        invalid_path(e.execute(&embed(vec![good.clone(), good.clone()])), c),
        "/copies/1"
    );
    let mut bad_id = good.clone();
    bad_id["id"] = json!(id::<FormDef>(0xbeef));
    assert_eq!(invalid_path(e.execute(&embed(vec![bad_id])), c), "/copies/0/id");
    let mut bad_sha = good.clone();
    bad_sha["sha256"] = json!(format!("sha256:{}", "00".repeat(32)));
    assert_eq!(
        invalid_path(e.execute(&embed(vec![bad_sha])), c),
        "/copies/0/sha256"
    );
    // a project-local item (library = the project)
    let (_, local) = embeddable(PROJECT.as_uuid().as_u128(), 0xa0003);
    assert_eq!(
        invalid_path(e.execute(&embed(vec![local])), c),
        "/copies/0/library_id"
    );
    invalid_path(e.execute(&embed(vec![json!({"$k": "bogus"})])), c);
    assert_eq!(invalid_path(e.execute(&embed(vec![])), c), "/copies");
    assert!(!has_form(&e, copy_id));
}
