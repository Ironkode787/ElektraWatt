// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the Session, the generic CPU placed from the picker gets
//! no bus port; the starter DP-port symbol placed onto it with `join_device` gets the CPU's DP port, typed, in the
//! placement's one undo step, and the checks report its TBD station address once.

use ew_api::checks::{CheckMode, Finding, PRE_DELIVERY};
use ew_api::{AnyCommand, NewProjectKind, Origin, ProjectTemplate, Session, SessionOptions};
use ew_commands::ChangeKind;
use ew_ids::{Id, Uuid};
use ew_schema::{AddrValue, BusPort, BusSystem, Function, Kind, PortRole};
use serde_json::{Value, json};

fn fresh(seed: u64) -> Session {
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(seed)),
        ..SessionOptions::default()
    };
    Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap()
}

/// The starter black box symbol.
const BLACKBOX: &str = "ew.gen.blackbox@qet10";
/// The starter DP-port symbol (FnDef `ew.fn.dp-port`, contact kind `bus_port`).
const DP_PORT: &str = "ew.bus.dp-port@qet10";
/// 7.9375 mm: three steps of the page's connection grid.
const STEP: i64 = 7_937_500;

fn library_part(s: &Session, number: &str) -> Uuid {
    s.library_parts()
        .iter()
        .find(|c| c.item.part_number.as_str() == number)
        .unwrap_or_else(|| panic!("{number} among the library parts"))
        .id
        .as_uuid()
}

fn first_page(s: &Session) -> Uuid {
    let q = s.query();
    let mut pages: Vec<Uuid> = q.pages().iter().map(|(id, _)| id.as_uuid()).collect();
    pages.sort();
    pages.first().copied().expect("the new project has a page")
}

/// Places EW-GEN-CPU-1 as the picker does (`box.black` from the part with its closure); the device and the notices.
fn place_cpu(s: &mut Session) -> (Uuid, Vec<String>) {
    let part = library_part(s, "EW-GEN-CPU-1");
    let copies = s
        .starter_embed_part(part, BLACKBOX)
        .expect("the closure resolves")
        .unwrap_or_else(|| json!({"copies": []}));
    let symbol = s.starter_symbol_copy(BLACKBOX).expect("a starter symbol");
    let page = first_page(s);
    let r = s
        .execute(&AnyCommand {
            name: "box.black".into(),
            params: json!({
                "page": page.to_string(),
                "symbol": symbol.to_string(),
                "pos": [12 * STEP, 12 * STEP],
                "box": {"outline": [[0, 0], [5 * STEP, 0], [5 * STEP, 8 * STEP], [0, 8 * STEP]]},
                "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "A"}},
                "copies": copies["copies"],
            }),
        })
        .unwrap_or_else(|e| panic!("box.black: {e:?}"));
    let device = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Device)
        .map(|k| k.1.as_uuid())
        .expect("a new device");
    (device, r.outcome.notices.clone())
}

/// The `device.place` of the starter DP-port symbol joining `device` as an auxiliary function, with the symbol's
/// closure in `copies`, on the bus topology page `page`.
fn place_port_command(s: &Session, page: Uuid, device: Uuid) -> AnyCommand {
    let copies = s
        .starter_embed(DP_PORT)
        .map_or_else(Vec::new, |c| c["copies"].as_array().cloned().unwrap_or_default());
    let symbol = s.starter_symbol_copy(DP_PORT).expect("a starter symbol");
    AnyCommand {
        name: "device.place".into(),
        params: json!({
            "page": page.to_string(),
            "symbol": symbol.to_string(),
            "pos": [24 * STEP, 12 * STEP],
            "target": {"join_device": {"device": device.to_string(), "role": "aux"}},
            "copies": Value::Array(copies),
        }),
    }
}

/// The bus ports of the functions of `device`, by function.
fn ports(s: &Session, device: Uuid) -> Vec<(Id<Function>, BusPort)> {
    let mut out: Vec<(Id<Function>, BusPort)> = s
        .query()
        .functions()
        .iter()
        .filter(|(_, f)| f.device.map(|d| d.as_uuid()) == Some(device))
        .filter_map(|(id, f)| f.bus_port.clone().map(|p| (id, p)))
        .collect();
    out.sort_by_key(|(id, _)| *id);
    out
}

fn dp_master() -> BusPort {
    BusPort {
        interface: ew_mltext::Ident::from_input("DP").unwrap(),
        port_label: None,
        bus: BusSystem::ProfibusDp,
        role: PortRole::Master,
        address: AddrValue::Tbd,
        termination: None,
        logical_net: None,
        physical_net: None,
        mrp: None,
        extra: ew_schema::Extra::new(),
    }
}

/// BU-7f: the CPU from the picker has no bus port and N5 says nothing; the DP-port symbol joining it gets the typed
/// port `{DP, profibus_dp, master, tbd}` with no termination and no nets, listed by the placement's preview and
/// committed in its one undo step (undo removes the function and its port, redo restores identical IDs); the checks
/// then report one EW-CORE-044 finding with discriminator `bus_address` for that port.
#[test]
fn the_dp_port_symbol_on_the_generic_cpu_gets_its_port() {
    let mut s = fresh(41);
    let (cpu, notices) = place_cpu(&mut s);
    assert_eq!(ports(&s, cpu), [], "placing the CPU seeds no port");
    assert!(
        !notices.iter().any(|n| n.contains("bus port")),
        "no N5 notice: {notices:?}"
    );

    // The DP-port symbol is a topology representation: it goes on a bus topology page.
    let r = s
        .execute(&AnyCommand {
            name: "page.create".into(),
            params: json!({"kind": "bus_topology", "after": first_page(&s).to_string()}),
        })
        .expect("page.create bus_topology");
    let topology = r.outcome.primary.expect("the new page").1.as_uuid();
    let cmd = place_port_command(&s, topology, cpu);
    let preview = s.preview(&cmd).expect("the placement previews");
    let added: Vec<Id<Function>> = preview
        .diff()
        .changes
        .iter()
        .filter(|c| c.change == ChangeKind::Added && c.key.0 == Kind::Function)
        .map(|c| c.key.1.cast())
        .collect();
    let [function] = added.as_slice() else {
        panic!("one new function: {added:?}");
    };
    let candidate_port = s
        .query_over(preview.candidate())
        .functions()
        .get(*function)
        .and_then(|f| f.bus_port.clone());
    assert_eq!(candidate_port, Some(dp_master()), "the preview lists the port");
    assert!(
        !preview.outcome().notices.iter().any(|n| n.contains("bus port")),
        "{:?}",
        preview.outcome().notices
    );
    let cursor = s.history().cursor();
    s.commit(preview).expect("the placement commits");
    assert_eq!(s.history().cursor(), cursor + 1, "one undo step");
    assert_eq!(ports(&s, cpu), [(*function, dp_master())]);

    s.undo(&Origin::Gui).expect("undo");
    assert!(
        s.query().functions().get(*function).is_none(),
        "undo removes the function"
    );
    assert_eq!(ports(&s, cpu), []);
    s.redo(&Origin::Gui).expect("redo");
    assert_eq!(
        ports(&s, cpu),
        [(*function, dp_master())],
        "redo restores identical IDs"
    );

    let report = s.check(&PRE_DELIVERY, CheckMode::Online);
    let open: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-CORE-044" && f.discriminator == "bus_address" && f.is_open())
        .collect();
    assert_eq!(open.len(), 1, "{open:?}");
}

/// FT-841: the DP-port symbol placed as a device of its own joins the CPU with `rep.join_device`, because the CPU's
/// bus part has a port no function carries yet; N5 then gives the joined function the CPU's DP port. A second port
/// symbol finds no free port and is refused in words.
#[test]
fn a_port_symbol_joins_a_device_with_a_free_bus_port() {
    let mut s = fresh(0x841);
    let (cpu, _) = place_cpu(&mut s);
    let r = s
        .execute(&AnyCommand {
            name: "page.create".into(),
            params: json!({"kind": "bus_topology", "after": first_page(&s).to_string()}),
        })
        .expect("page.create bus_topology");
    let page = r.outcome.primary.expect("the new page").1.as_uuid();
    let place_alone = |s: &mut Session, x: i64| {
        let copies = s
            .starter_embed(DP_PORT)
            .map_or_else(Vec::new, |c| c["copies"].as_array().cloned().unwrap_or_default());
        let symbol = s.starter_symbol_copy(DP_PORT).expect("a starter symbol");
        let r = s
            .execute(&AnyCommand {
                name: "device.place".into(),
                params: json!({
                    "page": page.to_string(),
                    "symbol": symbol.to_string(),
                    "pos": [x * STEP, 30 * STEP],
                    "target": {"new_device": {"letter": "X"}},
                    "copies": Value::Array(copies),
                }),
            })
            .unwrap_or_else(|e| panic!("device.place: {e:?}"));
        r.created
            .iter()
            .find(|k| k.0 == Kind::Representation)
            .map(|k| k.1.as_uuid())
            .expect("a representation")
    };
    let rep = place_alone(&mut s, 24);
    s.execute(&AnyCommand {
        name: "rep.join_device".into(),
        params: json!({"rep": rep.to_string(), "device": cpu.to_string()}),
    })
    .unwrap_or_else(|e| panic!("rep.join_device: {e:?}"));
    assert_eq!(
        ports(&s, cpu).into_iter().map(|(_, p)| p).collect::<Vec<_>>(),
        [dp_master()]
    );
    let second = place_alone(&mut s, 40);
    let err = s
        .execute(&AnyCommand {
            name: "rep.join_device".into(),
            params: json!({"rep": second.to_string(), "device": cpu.to_string()}),
        })
        .expect_err("no free port is left");
    assert!(
        format!("{err}").contains("no port that a function of it does not carry yet"),
        "{err}"
    );
}

/// Places starter part `number` as the picker does on `page`, at `x` steps; the device.
fn place_station(s: &mut Session, number: &str, page: Uuid, x: i64) -> Uuid {
    let part = library_part(s, number);
    let copies = s
        .starter_embed_part(part, BLACKBOX)
        .expect("the closure resolves")
        .unwrap_or_else(|| json!({"copies": []}));
    let symbol = s.starter_symbol_copy(BLACKBOX).expect("a starter symbol");
    let r = s
        .execute(&AnyCommand {
            name: "box.black".into(),
            params: json!({
                "page": page.to_string(),
                "symbol": symbol.to_string(),
                "pos": [x * STEP, 12 * STEP],
                "box": {"outline": [[0, 0], [5 * STEP, 0], [5 * STEP, 8 * STEP], [0, 8 * STEP]]},
                "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "A"}},
                "copies": copies["copies"],
            }),
        })
        .unwrap_or_else(|e| panic!("box.black {number}: {e:?}"));
    r.created
        .iter()
        .find(|k| k.0 == Kind::Device)
        .map(|k| k.1.as_uuid())
        .expect("a new device")
}

/// Places the port symbol `key` joined to `device` at `x` steps on `page`; the new function.
fn place_port(s: &mut Session, key: &str, page: Uuid, device: Uuid, x: i64) -> Id<Function> {
    let copies = s
        .starter_embed(key)
        .map_or_else(Vec::new, |c| c["copies"].as_array().cloned().unwrap_or_default());
    let symbol = s.starter_symbol_copy(key).expect("a starter symbol");
    let r = s
        .execute(&AnyCommand {
            name: "device.place".into(),
            params: json!({
                "page": page.to_string(),
                "symbol": symbol.to_string(),
                "pos": [x * STEP, 12 * STEP],
                "target": {"join_device": {"device": device.to_string(), "role": "aux"}},
                "copies": Value::Array(copies),
            }),
        })
        .unwrap_or_else(|e| panic!("device.place {key}: {e:?}"));
    r.created
        .iter()
        .find(|k| k.0 == Kind::Function)
        .map(|k| k.1.cast())
        .expect("a new function")
}

/// A bus line between the CP `a` of one function and the CP `b` of another.
fn bus_line(s: &mut Session, a: (Id<Function>, u16), b: (Id<Function>, u16)) {
    s.execute(&AnyCommand {
        name: "connection.draw".into(),
        params: json!({
            "a": {"cp": {"function": a.0.to_string(), "idx": a.1}},
            "b": {"cp": {"function": b.0.to_string(), "idx": b.1}},
            "kind": {"bus": {}},
        }),
    })
    .unwrap_or_else(|e| panic!("connection.draw: {e:?}"));
}

/// The persona round, a bus from nothing: the CPU with PROFINET and PROFIBUS DP is a CPU and the remote I/O head a
/// head station (its ports lead no bus); the CPU's row offers both ports, and each port symbol takes its own bus's
/// port of the part (the PROFINET symbol the PROFINET controller, though the part lists its DP port first); a bus line
/// drawn between the two DP ports puts them in one new logical net "PROFIBUS DP" whose master is the CPU's port, and a
/// line to a third port that has no net joins that net.
#[test]
fn a_bus_line_between_two_stations_makes_their_net() {
    let mut s = fresh(0xB05);
    let sheet = first_page(&s);
    let cpu = place_station(&mut s, "EW-GEN-CPU-2", sheet, 12);
    let rio = place_station(&mut s, "EW-GEN-RIO-1", sheet, 30);
    let role = |s: &Session, d: Uuid| {
        s.query()
            .devices()
            .get(Id::from_uuid(d))
            .and_then(|d| d.aspects.plc_station.as_ref().map(|p| p.role.clone()))
    };
    assert_eq!(role(&s, cpu), Some(ew_schema::StationRole::Cpu));
    assert_eq!(role(&s, rio), Some(ew_schema::StationRole::HeadStation));
    let offered: Vec<String> = s.uncarried_bus_ports(cpu).into_iter().map(|(p, _)| p).collect();
    assert_eq!(offered, ["DP", "X1"], "the CPU's row offers both ports");
    let r = s
        .execute(&AnyCommand {
            name: "page.create".into(),
            params: json!({"kind": "bus_topology", "after": first_page(&s).to_string()}),
        })
        .expect("page.create bus_topology");
    let page = r.outcome.primary.expect("the new page").1.as_uuid();
    let port = |s: &Session, f: Id<Function>| s.query().functions().get(f).and_then(|f| f.bus_port.clone());
    let pn = place_port(&mut s, "ew.bus.pn-port@qet10", page, cpu, 12);
    let pn_port = port(&s, pn).expect("a port");
    assert_eq!(
        (pn_port.bus, pn_port.role),
        (BusSystem::Profinet, PortRole::Controller)
    );
    let dp_cpu = place_port(&mut s, DP_PORT, page, cpu, 24);
    let dp_rio = place_port(&mut s, DP_PORT, page, rio, 36);
    assert_eq!(port(&s, dp_cpu).map(|p| p.role), Some(PortRole::Master));
    assert_eq!(port(&s, dp_rio).map(|p| p.role), Some(PortRole::Slave));
    bus_line(&mut s, (dp_cpu, 0), (dp_rio, 0));
    let net = port(&s, dp_cpu).and_then(|p| p.logical_net).expect("a net");
    assert_eq!(port(&s, dp_rio).and_then(|p| p.logical_net), Some(net));
    let n = s
        .query()
        .table::<ew_schema::LogicalNet>()
        .get(net)
        .cloned()
        .expect("the net");
    assert_eq!(n.name.as_str(), "PROFIBUS DP");
    assert_eq!(
        n.master.as_ref().and_then(ew_schema::Endpoint::function),
        Some(dp_cpu)
    );
    let third = place_station(&mut s, "EW-GEN-RIO-1", sheet, 48);
    let dp_third = place_port(&mut s, DP_PORT, page, third, 48);
    bus_line(&mut s, (dp_rio, 1), (dp_third, 0));
    assert_eq!(port(&s, dp_third).and_then(|p| p.logical_net), Some(net));
    assert_eq!(
        s.query().table::<ew_schema::LogicalNet>().iter().count(),
        1,
        "no second net"
    );
}
