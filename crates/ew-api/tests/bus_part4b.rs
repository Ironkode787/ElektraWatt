// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 4b in the Session (acceptance BU-7f; §8a.2, D16, D17): the generic CPU placed from the picker gets
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
        ids: Some(ew_commands::testkit::seeded_ids(seed)),
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
        extra: ew_schema::Extra::new(),
    }
}

/// BU-7f: the CPU from the picker has no bus port and N5 says nothing; the DP-port symbol joining it gets the typed
/// port `{DP, profibus_dp, master, tbd}` with no termination and no nets, listed by the placement's preview and
/// committed in its one undo step (undo removes the function and its port, redo restores identical IDs); the checks
/// then report one EW-CORE-044 finding with discriminator `bus_address` for that port.
#[test]
fn bu7f_the_dp_port_symbol_on_the_generic_cpu_gets_its_port() {
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
