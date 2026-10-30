// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 1 in the engine: normaliser N0 (`plc_aspects`, D2 to D4) and the channel members through
//! `fields.set` (D6, FT-226). The window path with the starter parts (PW-1 to PW-6) is `ew-api`'s `plc_window.rs`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AttachChildren, ChildSpec, CreateDevice, CreatePlcConfigProject, KindName, LinkDevices, SetFields,
};
use ew_commands::normalise::ORDER;
use ew_commands::{CmdError, Engine, Origin, Receipt};
use ew_ids::Id;
use ew_model::{Entity, Model};
use ew_schema::{
    AddrSpec, Channel, ChannelState, Device, DeviceLink, DeviceLinkKind, Kind, LibCopy, LibDocument, Part,
    PartRef, PartUse, PlcArea, PlcConfigProject, PlcSection, PlcSize, StationRole,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------------------------
// Fixture

/// A CPU part: a station without I/O ranges.
const CPU: Id<Part> = id(0x6f0);
/// A CPU part with on-board I/O: a station with an integrated module of 4 inputs.
const CPU_IO: Id<Part> = id(0x6f1);
/// A digital input card: a module of 8 channels.
const CARD: Id<Part> = id(0x6f2);
/// A second card part (another part with a PLC section).
const CARD_B: Id<Part> = id(0x6f3);

fn plc_part(part_id: Id<Part>, number: &str, plc: Value) -> LibCopy<Part> {
    let mut copy = part(part_id, number);
    copy.item.sections.plc = Some(serde_json::from_value::<PlcSection>(plc).expect("plc section"));
    copy
}

fn card(part_id: Id<Part>, number: &str) -> LibCopy<Part> {
    plc_part(
        part_id,
        number,
        json!({"role": "module", "io": [{"dir": "in", "bits": 8}], "channels": {"count": 8, "kind": "di"}}),
    )
}

fn model() -> Model {
    let (p, records, mut lib) = mini_golden_parts();
    lib.push(LibDocument::Part(plc_part(
        CPU,
        "CPU-1",
        json!({"role": "station"}),
    )));
    lib.push(LibDocument::Part(plc_part(
        CPU_IO,
        "CPU-2",
        json!({"role": "station", "io": [{"dir": "in", "bits": 8}], "channels": {"count": 4, "kind": "di"}}),
    )));
    lib.push(LibDocument::Part(card(CARD, "DI-1")));
    lib.push(LibDocument::Part(card(CARD_B, "DI-2")));
    Model::from_records(p, records, lib).expect("the PLC model is valid")
}

fn uses(part: Id<Part>) -> Vec<PartUse> {
    vec![PartUse {
        part: PartRef::new(part),
        qty: dec3("1"),
        extra: ew_schema::Extra::new(),
    }]
}

fn create(letter: &str, counter: u32, part: Id<Part>) -> CreateDevice {
    CreateDevice {
        letter: ident(letter),
        counter: Some(counter),
        kind: None,
        parts: uses(part),
        function_text: Default::default(),
        structure: Default::default(),
        strip: None,
        main_fndef: None,
    }
}

/// Creates a device with `part`; its ID and the receipt.
fn make(e: &mut Engine, letter: &str, counter: u32, part: Id<Part>) -> (Id<Device>, Receipt) {
    let r = e.execute(&create(letter, counter, part)).expect("device.create");
    (typed(r.outcome.primary.expect("the device")), r)
}

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

fn set(kind: KindName, target: ew_schema::AnyId, path: &str, value: Value) -> SetFields {
    SetFields {
        kind,
        targets: vec![target],
        path: path.into(),
        value,
    }
}

fn notices(r: &Receipt) -> &[String] {
    &r.outcome.notices
}

fn has_notice(r: &Receipt, needle: &str) -> bool {
    r.outcome.notices.iter().any(|n| n.contains(needle))
}

fn channels_of(e: &Engine, module: Id<Device>) -> Vec<std::sync::Arc<Channel>> {
    let mut out: Vec<_> = e
        .model()
        .table::<Channel>()
        .iter()
        .filter(|(_, c)| c.module == module)
        .map(|(_, c)| c.clone())
        .collect();
    out.sort_by_key(|c| c.number);
    out
}

fn invalid(r: Result<Receipt, CmdError>) -> (String, String) {
    match r {
        Err(CmdError::InvalidParams { path, message, .. }) => (path, message),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// N0: the registry row

#[test]
fn n0_is_the_first_row_of_the_registry_before_n1() {
    assert_eq!(ORDER[0], "plc_aspects");
    let n0 = ORDER.iter().position(|n| *n == "plc_aspects");
    let n1 = ORDER.iter().position(|n| *n == "channel_instantiation");
    assert!(n0 < n1, "{ORDER:?}");
}

// ---------------------------------------------------------------------------------------------------------
// D2, D3: stations and the configuration project on demand

/// PW-2's engine half: the first station creates `PLC1` with the TIA profile, the CPU as its `cpu` and the v5 ID.
#[test]
fn the_first_station_creates_plc1() {
    let mut e = engine(model());
    let (cpu, r) = make(&mut e, "A", 1, CPU);
    let cfg: Id<PlcConfigProject> = ew_ids::v5::plc_config_project_id(cpu);
    let station = row(&e, cpu).aspects.plc_station.clone().expect("a station");
    assert_eq!(station.config_project, cfg);
    assert_eq!(station.role, StationRole::Cpu);
    assert!(
        row(&e, cpu).aspects.plc_module.is_none(),
        "no I/O ranges, no module"
    );
    let c = row(&e, cfg);
    assert_eq!(c.name.as_str(), "PLC1");
    assert_eq!(c.profile.as_str(), "siemens-tia-intl@2027.1");
    assert_eq!(c.cpu, Some(cpu));
    assert!(r.created.contains(&(Kind::PlcConfigProject, cfg.cast())));
    assert!(
        has_notice(&r, "configuration project PLC1 created for station -A1"),
        "{:?}",
        notices(&r)
    );
}

/// D3: a second station joins the only configuration project, with a notice that names it.
#[test]
fn a_second_station_joins_plc1() {
    let mut e = engine(model());
    let (first, _) = make(&mut e, "A", 1, CPU);
    let (second, r) = make(&mut e, "A", 2, CPU);
    let cfg: Id<PlcConfigProject> = ew_ids::v5::plc_config_project_id(first);
    assert_eq!(
        row(&e, second)
            .aspects
            .plc_station
            .as_ref()
            .map(|s| s.config_project),
        Some(cfg)
    );
    assert_eq!(e.model().table::<PlcConfigProject>().iter().count(), 1);
    assert!(
        has_notice(&r, "station -A2 joins configuration project PLC1"),
        "{:?}",
        notices(&r)
    );
}

/// D3: with several configuration projects, a station joins the first by name, and the notice says so.
#[test]
fn with_several_configuration_projects_a_station_joins_the_first_by_name() {
    let mut e = engine(model());
    let mut ids = Vec::new();
    for name in ["PLC2", "PLC1"] {
        let r = e
            .execute(&CreatePlcConfigProject {
                name: ident(name),
                profile: ident("siemens-tia-intl@2027.1"),
                cpu: None,
            })
            .expect("config project");
        ids.push(typed::<PlcConfigProject>(r.outcome.primary.expect("cfg")));
    }
    let (cpu, r) = make(&mut e, "A", 1, CPU);
    assert_eq!(
        row(&e, cpu)
            .aspects
            .plc_station
            .as_ref()
            .map(|s| s.config_project),
        Some(ids[1])
    );
    assert!(
        has_notice(&r, "joins configuration project PLC1, the first of 2 by name"),
        "{:?}",
        notices(&r)
    );
}

/// D2: a station part with I/O ranges also gives the device an integrated module, whose channels N1 creates.
#[test]
fn a_station_with_io_carries_an_integrated_module() {
    let mut e = engine(model());
    let (cpu, _) = make(&mut e, "A", 1, CPU_IO);
    let d = row(&e, cpu);
    assert!(d.aspects.plc_station.is_some());
    assert!(d.aspects.plc_module.as_ref().is_some_and(|m| m.integrated));
    assert_eq!(channels_of(&e, cpu).len(), 4);
}

// ---------------------------------------------------------------------------------------------------------
// D2, D4: modules

/// PW-1's engine half: modules attach to the only station in placement order, positions 1 and 2, with the notice;
/// each module's channels exist with v5 IDs.
#[test]
fn modules_attach_to_the_only_station() {
    let mut e = engine(model());
    let (cpu, _) = make(&mut e, "A", 1, CPU);
    let (m1, r1) = make(&mut e, "A", 2, CARD);
    let (m2, r2) = make(&mut e, "A", 3, CARD_B);
    for (m, pos, r) in [(m1, 1, &r1), (m2, 2, &r2)] {
        let d = row(&e, m);
        assert_eq!((d.parent, d.position), (Some(cpu), Some(pos)));
        assert_eq!(
            d.aspects.plc_module.clone(),
            Some(ew_schema::PlcModule::default())
        );
        assert!(
            has_notice(r, &format!("attached to -A1 as module {pos}")),
            "{:?}",
            notices(r)
        );
        let chans = channels_of(&e, m);
        assert_eq!(chans.len(), 8);
        for c in &chans {
            assert_eq!(c.id, ew_ids::v5::plc_channel_id(m, c.number));
        }
    }
}

/// PW-3's engine half: without a station the module stays unattached with its channels and the notice; with two
/// stations it stays unattached with D4's other notice.
#[test]
fn a_module_without_one_station_stays_unattached() {
    let mut e = engine(model());
    let (m, r) = make(&mut e, "A", 1, CARD);
    assert_eq!(row(&e, m).parent, None);
    assert!(row(&e, m).aspects.plc_module.is_some());
    assert_eq!(channels_of(&e, m).len(), 8);
    assert!(
        notices(&r)
            .iter()
            .any(|n| n == "no PLC station: place a CPU first"),
        "{:?}",
        notices(&r)
    );

    let mut e = engine(model());
    make(&mut e, "A", 1, CPU);
    make(&mut e, "A", 2, CPU);
    let (m, r) = make(&mut e, "A", 3, CARD);
    assert_eq!(row(&e, m).parent, None);
    assert!(
        notices(&r).iter().any(|n| n == "attach the module to a station"),
        "{:?}",
        notices(&r)
    );
}

/// D4 applies only to a module without a station ancestor: a device already below the station keeps its place.
#[test]
fn a_module_below_a_station_keeps_its_place() {
    let mut e = engine(model());
    let (cpu, _) = make(&mut e, "A", 1, CPU);
    let r = e
        .execute(&CreateDevice {
            parts: Vec::new(),
            ..create("A", 2, CARD)
        })
        .expect("a device without parts");
    let m: Id<Device> = typed(r.outcome.primary.expect("the device"));
    e.execute(&AttachChildren {
        parent: cpu,
        children: vec![ChildSpec {
            device: m,
            position: Some(5),
        }],
    })
    .expect("attach");
    let r = e
        .execute(&set(
            KindName::Device,
            m.cast(),
            "parts",
            serde_json::to_value(uses(CARD)).unwrap(),
        ))
        .expect("parts");
    let d = row(&e, m);
    assert_eq!((d.parent, d.position), (Some(cpu), Some(5)));
    assert!(d.aspects.plc_module.is_some());
    assert!(!has_notice(&r, "attached to"), "{:?}", notices(&r));
}

/// D2: N0 never rewrites or re-adds an aspect: a removed aspect stays removed through unrelated edits and a part
/// list whose PLC part is unchanged; only another PLC part triggers it again.
#[test]
fn n0_never_rewrites_or_re_adds_an_aspect() {
    let mut e = engine(model());
    let (m, _) = make(&mut e, "A", 1, CARD);
    e.execute(&set(KindName::Device, m.cast(), "plc_module", Value::Null))
        .expect("remove the aspect");
    e.execute(&set(KindName::Device, m.cast(), "function_text", json!("inputs")))
        .expect("an unrelated edit");
    assert!(row(&e, m).aspects.plc_module.is_none());
    e.execute(&set(
        KindName::Device,
        m.cast(),
        "parts",
        json!([{"part": {"copy": CARD}, "qty": "2"}]),
    ))
    .expect("the same PLC part");
    assert!(row(&e, m).aspects.plc_module.is_none());
    e.execute(&set(
        KindName::Device,
        m.cast(),
        "parts",
        serde_json::to_value(uses(CARD_B)).unwrap(),
    ))
    .expect("another PLC part");
    assert!(row(&e, m).aspects.plc_module.is_some());
    // a device with an aspect is never touched: an explicit station keeps its role and project
    let mut e = engine(model());
    let r = e
        .execute(&CreatePlcConfigProject {
            name: ident("Line"),
            profile: ident("siemens-tia-intl@2027.1"),
            cpu: None,
        })
        .expect("cfg");
    let cfg: Id<PlcConfigProject> = typed(r.outcome.primary.expect("cfg"));
    let r = e
        .execute(&CreateDevice {
            parts: Vec::new(),
            ..create("A", 1, CPU)
        })
        .expect("device");
    let d: Id<Device> = typed(r.outcome.primary.expect("device"));
    e.execute(&set(
        KindName::Device,
        d.cast(),
        "plc_station",
        json!({"config_project": cfg, "role": "head_station"}),
    ))
    .expect("station");
    e.execute(&set(
        KindName::Device,
        d.cast(),
        "parts",
        serde_json::to_value(uses(CPU)).unwrap(),
    ))
    .expect("parts");
    let s = row(&e, d).aspects.plc_station.clone().unwrap();
    assert_eq!((s.config_project, s.role), (cfg, StationRole::HeadStation));
    assert_eq!(e.model().table::<PlcConfigProject>().iter().count(), 1);
}

/// PW-4's engine half: each placement is one undo step; undo of the first removes the station and `PLC1` and
/// nothing else, redo restores identical IDs; a second run with the same seed gives the same model (PW-8).
#[test]
fn undo_and_redo_of_a_placement_and_twice_run_determinism() {
    let run = || {
        let mut e = engine(model());
        let (cpu, _) = make(&mut e, "A", 1, CPU);
        let (m, _) = make(&mut e, "A", 2, CARD);
        (e, cpu, m)
    };
    let (mut e, cpu, m) = run();
    let (other, ..) = run();
    assert!(e.model().content_eq(other.model()), "twice-run determinism");
    let before_all = e.model().clone();
    e.undo(&Origin::Gui).expect("undo the card");
    assert!(e.model().table::<Device>().get(&m).is_none());
    assert!(channels_of(&e, m).is_empty());
    let after_cpu = e.model().clone();
    e.undo(&Origin::Gui).expect("undo the CPU");
    assert!(e.model().content_eq(&model()), "nothing else changes");
    e.redo(&Origin::Gui).expect("redo the CPU");
    assert!(e.model().content_eq(&after_cpu));
    let cfg: Id<PlcConfigProject> = ew_ids::v5::plc_config_project_id(cpu);
    assert!(e.model().table::<PlcConfigProject>().get(&cfg).is_some());
    e.redo(&Origin::Gui).expect("redo the card");
    assert!(e.model().content_eq(&before_all));
}

// ---------------------------------------------------------------------------------------------------------
// D6: the channel members through `fields.set`

fn station_with_card() -> (Engine, Id<Device>, Id<Channel>) {
    let mut e = engine(model());
    make(&mut e, "A", 1, CPU);
    let (m, _) = make(&mut e, "A", 2, CARD);
    (e, m, ew_ids::v5::plc_channel_id(m, 0))
}

/// PW-5's engine half: symbol, comment, state and every address form, each one undo step.
#[test]
fn channel_members_are_settable() {
    let (mut e, _, c0) = station_with_card();
    let steps: [(&str, Value); 10] = [
        ("symbol", json!("B1_Start")),
        ("comment", json!("Start button")),
        ("designation", json!("E0")),
        ("state", json!("spare")),
        ("address", json!("I4.0")),
        ("address", json!("excluded")),
        ("address", json!(" TBD ")),
        ("address", json!("auto")),
        ("target", json!(FN_102F2)),
        ("state", Value::Null),
    ];
    for (path, value) in steps {
        let before = row(&e, c0);
        e.execute(&set(KindName::Channel, c0.cast(), path, value.clone()))
            .unwrap_or_else(|err| panic!("{path} = {value}: {err:?}"));
        let after = row(&e, c0);
        assert_ne!(before, after, "{path} = {value}");
        e.undo(&Origin::Gui).expect("one undo step");
        assert_eq!(row(&e, c0), before, "{path}: undo restores");
        e.redo(&Origin::Gui).expect("redo");
    }
    let c = row(&e, c0);
    assert_eq!(c.symbol.as_ref().map(|s| s.as_str()), Some("B1_Start"));
    assert_eq!(c.designation.as_ref().map(|s| s.as_str()), Some("E0"));
    assert_eq!(c.address, AddrSpec::Auto);
    assert_eq!(c.target, Some(FN_102F2));
    assert_eq!(c.state, None);
    e.execute(&set(KindName::Channel, c0.cast(), "address", json!("I4.0")))
        .expect("manual");
    let AddrSpec::Manual(a) = &row(&e, c0).address else {
        panic!("a manual address")
    };
    assert_eq!(
        (&a.area, &a.size, a.byte, a.bit),
        (&PlcArea::I, &PlcSize::Bit, 4, Some(0))
    );
    e.execute(&set(KindName::Channel, c0.cast(), "state", json!("reserved")))
        .expect("state");
    assert_eq!(row(&e, c0).state, Some(ChannelState::Reserved));
}

/// D6: a text that the profile does not parse is refused with the profile's example; values of a newer format are
/// refused; a configuration project whose profile this build does not ship reads no address text.
#[test]
fn channel_values_outside_the_profile_and_this_build_are_refused() {
    let (mut e, _, c0) = station_with_card();
    let (path, message) = invalid(e.execute(&set(KindName::Channel, c0.cast(), "address", json!("I4.9"))));
    assert_eq!(path, "/value");
    assert!(message.contains("expected an address like Q0.1"), "{message}");
    for (p, v) in [
        ("state", json!("broken")),
        ("address", json!({"later": {"x": 1}})),
        ("address", json!({"manual": {"area": "z", "byte": 0, "bit": 0}})),
    ] {
        assert_eq!(
            invalid(e.execute(&set(KindName::Channel, c0.cast(), p, v.clone()))).0,
            "/value",
            "{p} = {v}"
        );
    }
    assert_eq!(row(&e, c0).address, AddrSpec::Auto);
    // a target that names no function is a dangling link, refused by the model
    assert!(
        e.execute(&set(
            KindName::Channel,
            c0.cast(),
            "target",
            json!(id::<()>(0xdead))
        ))
        .is_err()
    );
    assert_eq!(row(&e, c0).target, None);
    // a configuration project of a profile this build does not ship
    let r = e
        .execute(&CreatePlcConfigProject {
            name: ident("Vendor"),
            profile: ident("vendor-x@1"),
            cpu: None,
        })
        .expect("a foreign profile");
    let vendor: Id<PlcConfigProject> = typed(r.outcome.primary.expect("cfg"));
    let station = row(&e, row(&e, c0).module).parent.expect("attached");
    e.execute(&set(
        KindName::Device,
        station.cast(),
        "plc_station.config_project",
        json!(vendor),
    ))
    .expect("move the station");
    let (_, message) = invalid(e.execute(&set(KindName::Channel, c0.cast(), "address", json!("I4.0"))));
    assert!(message.contains("not shipped"), "{message}");
    e.execute(&set(KindName::Channel, c0.cast(), "address", json!("tbd")))
        .expect("a word needs no profile");
}

/// D6: a module without a station reads its address texts with the shipped default profile.
#[test]
fn a_module_without_a_station_reads_addresses_with_the_default_profile() {
    let mut e = engine(model());
    let (m, _) = make(&mut e, "A", 1, CARD);
    let c0 = ew_ids::v5::plc_channel_id::<_, Channel>(m, 0);
    e.execute(&set(KindName::Channel, c0.cast(), "address", json!("I2.3")))
        .expect("default profile");
    assert!(matches!(row(&e, c0).address, AddrSpec::Manual(_)));
}

/// D6: a configuration project's name is settable and unique; its profile is not settable here; a device link's
/// kind is settable, a kind of a newer format refused.
#[test]
fn configuration_project_name_and_device_link_kind() {
    let mut e = engine(model());
    let (cpu, _) = make(&mut e, "A", 1, CPU);
    let cfg: Id<PlcConfigProject> = ew_ids::v5::plc_config_project_id(cpu);
    e.execute(&set(
        KindName::PlcConfigProject,
        cfg.cast(),
        "name",
        json!("Line A"),
    ))
    .expect("rename");
    assert_eq!(row(&e, cfg).name.as_str(), "Line A");
    e.execute(&CreatePlcConfigProject {
        name: ident("Other"),
        profile: ident("siemens-tia-intl@2027.1"),
        cpu: None,
    })
    .expect("another");
    assert!(matches!(
        e.execute(&set(
            KindName::PlcConfigProject,
            cfg.cast(),
            "name",
            json!("Other")
        )),
        Err(CmdError::Rejected { .. })
    ));
    assert_eq!(
        invalid(e.execute(&set(
            KindName::PlcConfigProject,
            cfg.cast(),
            "profile",
            json!("siemens-tia-intl@2027.1")
        )))
        .0,
        "/path"
    );
    let r = e
        .execute(&LinkDevices {
            from: DEV_102F2,
            to: DEV_101F1,
            kind: DeviceLinkKind::Drives,
        })
        .expect("link");
    let link: Id<DeviceLink> = typed(r.outcome.primary.expect("link"));
    e.execute(&set(KindName::DeviceLink, link.cast(), "kind", json!("linkage")))
        .expect("kind");
    assert_eq!(row(&e, link).kind, DeviceLinkKind::Linkage);
    assert_eq!(
        invalid(e.execute(&set(KindName::DeviceLink, link.cast(), "kind", json!("pulls")))).0,
        "/value"
    );
    assert_eq!(
        invalid(e.execute(&set(KindName::DeviceLink, link.cast(), "from", json!(DEV_101F1)))).0,
        "/path"
    );
}

/// FT-226: a channel edit changes no page (no page draws a channel yet), so the affected pages are empty.
#[test]
fn a_channel_edit_affects_no_page() {
    let (mut e, _, c0) = station_with_card();
    e.drain_changes();
    e.execute(&set(KindName::Channel, c0.cast(), "symbol", json!("B1")))
        .expect("symbol");
    let changes = e.drain_changes();
    assert_eq!(changes.len(), 1);
    assert!(changes[0].touched_pages.is_empty(), "{changes:?}");
}
