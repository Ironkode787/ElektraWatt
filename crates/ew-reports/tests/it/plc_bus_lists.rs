// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC and bus lists of doc 10 §5 at the row level, on a small project that Golden A does not cover: an IO-Link
//! master `-A1` whose port 1 reads the IO-Link device `-B1` (`iolink_port_list`), an F-module `-A2` with a 1oo2 pair
//! (`safety_io_list`), and two PROFIBUS ports `-A3:X1` and `-A4:X1` that name no logical net, joined by the bus cable
//! `-W1` of 12.5 m (`network_address_list`, `bus_cable_list`).

use ew_derive::Q;
use ew_ids::Uuid;
use ew_schema::{LibDocument, Project, Record, ReportBlock};
use ew_model::Model;
use ew_reports::{Cell, Registry};
use serde_json::{Value, json};

fn uid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

const LIBRARY: u128 = 0x7000;
const PAGE: u128 = 0x100;
const CFG: u128 = 0x110;
const PNET: u128 = 0x120;
const DEF_CARD: u128 = 0x500;
const DEF_DEV: u128 = 0x501;
const DEF_PORT: u128 = 0x502;
const MASTER_PART: u128 = 0x600;
const DEV_PART: u128 = 0x601;
const F_PART: u128 = 0x602;
const A1: u128 = 0x1000;
const B1: u128 = 0x1100;
const A2: u128 = 0x1200;
const A3: u128 = 0x1300;
const A4: u128 = 0x1400;
const W1: u128 = 0x1500;

fn lib_item(k: &str, n: u128, item: Value) -> LibDocument {
    let mut doc = json!({
        "$k": k, "id": uid(n), "library_id": uid(LIBRARY), "item_id": uid(n + 0x1_0000), "version": "1.0.0",
        "sha256": format!("sha256:{}", "ab".repeat(32)),
    });
    for (key, v) in item.as_object().expect("an item") {
        doc[key] = v.clone();
    }
    serde_json::from_value(doc).unwrap_or_else(|e| panic!("{k} {n:#x}: {e}"))
}

fn fndef(n: u128, kind: &str, cps: &[&str]) -> LibDocument {
    let cps: Vec<Value> = cps
        .iter()
        .enumerate()
        .map(|(i, d)| json!({"idx": i, "designation": d}))
        .collect();
    lib_item(
        "fndef",
        n,
        json!({"meta": {}, "category": ["Misc", "Misc", "Plain"], "contact_kind": kind, "cps": cps}),
    )
}

fn part(n: u128, number: &str, sections: Value) -> LibDocument {
    lib_item(
        "part",
        n,
        json!({
            "meta": {}, "kind": "device", "part_number": number, "sections": sections,
            "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
        }),
    )
}

fn device(n: u128, letter: &str, counter: u32, parts: &[u128]) -> Value {
    let parts: Vec<Value> = parts
        .iter()
        .map(|p| json!({"part": {"copy": uid(*p)}, "qty": "1"}))
        .collect();
    json!({"$k": "device", "id": uid(n), "tag": {"letter": letter, "counter": counter}, "parts": parts})
}

fn channel(module: u128, n: u16, kind: &str) -> Value {
    let id = ew_ids::v5::plc_channel_id::<ew_schema::Device, ()>(
        ew_ids::Id::from_uuid(Uuid::from_u128(module)),
        n,
    );
    json!({"$k": "channel", "id": id.as_uuid().to_string(), "module": uid(module), "number": n, "kind": kind})
}

fn model() -> Model {
    let lib = vec![
        fndef(DEF_CARD, "plc_io", &["1", "2"]),
        fndef(DEF_DEV, "black_box", &["1", "2"]),
        fndef(DEF_PORT, "bus_port", &["A", "B"]),
        part(
            MASTER_PART,
            "IOL-2",
            json!({"plc": {"role": "module", "channels": {"count": 2, "kind": "multi_function"},
                           "iolink": {"input_bytes": 2, "output_bytes": 2, "class": "a"}}}),
        ),
        part(
            DEV_PART,
            "SENSOR-IOL",
            json!({"bus": {"ports": [{"interface": "IOL", "bus": "io_link", "role": "device",
                    "iolink": {"class": "a", "input_bits": 16, "vendor_id": 26, "device_id": 1001}}]}}),
        ),
        part(
            F_PART,
            "F-DI-2",
            json!({"plc": {"role": "module", "io": [{"dir": "in", "bits": 8}], "channels": {"count": 2, "kind": "di"}}}),
        ),
    ];
    let mut master = device(A1, "A", 1, &[MASTER_PART]);
    master["plc_station"] = json!({"config_project": uid(CFG), "role": "io_device"});
    master["plc_module"] = json!({});
    let mut f_module = device(A2, "A", 2, &[F_PART]);
    f_module["parent"] = json!(uid(A1));
    f_module["position"] = json!(1);
    f_module["plc_module"] = json!({"safety": {"f_dest": 7, "f_source": 1}});
    let mut port_ch = channel(A1, 0, "multi_function");
    port_ch["binding"] = json!({"cp": [uid(A1 + 1), 0]});
    let mut f0 = channel(A2, 0, "di");
    let mut f1 = channel(A2, 1, "di");
    f0["safety"] = json!({"evaluation": "1oo2", "pair": f1["id"]});
    f1["safety"] = json!({"evaluation": "1oo2", "pair": f0["id"]});
    let mut cable = device(W1, "W", 1, &[]);
    cable["cable"] = json!({"length": {"manual": 12_500_000_000_i64}});
    let mut records = vec![
        json!({"$k": "page", "id": uid(PAGE), "folio": {"number": 1}, "kind": "multi_line", "title": {"en": "Page"},
               "grid": {"num": 4_000_000, "den": 1}}),
        json!({"$k": "plc_config_project", "id": uid(CFG), "name": "PLC1", "profile": "siemens-tia-intl@2027.1"}),
        json!({"$k": "physical_net", "id": uid(PNET), "name": "DP", "medium": "copper"}),
        master,
        device(B1, "B", 1, &[DEV_PART]),
        f_module,
        device(A3, "A", 3, &[]),
        device(A4, "A", 4, &[]),
        cable,
        json!({"$k": "function", "id": uid(A1 + 1), "device": uid(A1), "def": uid(DEF_CARD)}),
        json!({"$k": "function", "id": uid(B1 + 1), "device": uid(B1), "def": uid(DEF_DEV)}),
        port_ch,
        channel(A1, 1, "multi_function"),
        f0,
        f1,
        json!({"$k": "connection", "id": uid(0x2000), "page": uid(PAGE),
               "a": {"cp": [uid(A1 + 1), 0]}, "b": {"cp": [uid(B1 + 1), 0]}}),
        json!({"$k": "connection", "id": uid(0x2001), "page": uid(PAGE),
               "a": {"cp": [uid(A3 + 1), 0]}, "b": {"cp": [uid(A4 + 1), 0]}, "kind": {"bus": {"cable": uid(W1)}}}),
    ];
    for (n, address) in [(A3, json!({"value": {"profibus": 5}})), (A4, json!("tbd"))] {
        records.push(json!({
            "$k": "function", "id": uid(n + 1), "device": uid(n), "def": uid(DEF_PORT),
            "bus_port": {"interface": "X1", "bus": "profibus_dp", "role": "slave", "address": address,
                         "physical_net": uid(PNET)},
        }));
    }
    let project: Project =
        serde_json::from_value(json!({"id": uid(1), "derivation_profile": "2027.1"})).expect("a project");
    let records: Vec<Record> = records
        .into_iter()
        .map(|r| serde_json::from_value(r.clone()).unwrap_or_else(|e| panic!("{r}: {e}")))
        .collect();
    Model::from_records(project, records, lib).expect("a valid model")
}

/// The rows of report kind `kind`, each cell as text; an undrawn device's tag (`-?A1`, its place unresolved) prints as
/// `-A1`, since the place is not what these lists are tested for.
fn rows(kind: &str) -> Vec<Vec<String>> {
    let m = model();
    let q = Q::new(&m);
    let block: ReportBlock =
        serde_json::from_value(json!({"id": uid(0x50), "source": {"kind": kind}, "form": uid(0x51)}))
            .expect("a report block");
    let def = Registry::builtin().by_name(kind).expect("a registered kind");
    def.rows(&q, &block)
        .rows
        .iter()
        .map(|r| {
            r.cells
                .iter()
                .map(|c| match c {
                    Cell::Ident(s) => s.replace("-?", "-"),
                    Cell::Text(t) => t.as_neutral().unwrap_or_default().to_owned(),
                    Cell::Number(n) => n.to_string(),
                    Cell::Bool(b) => b.to_string(),
                    Cell::Tbd => "TBD".to_owned(),
                    Cell::Empty => String::new(),
                    other => format!("{other:?}"),
                })
                .collect()
        })
        .collect()
}

/// FR-BUS-014: one row per used master port: master, port, mode, class, device, vendor and device ID, process data.
#[test]
fn the_iolink_port_list_has_a_row_per_used_port() {
    assert_eq!(
        rows("iolink_port_list"),
        [["-A1", "1", "IO-Link", "A", "-B1", "26", "1001", "16", "0", ""]]
    );
}

/// FR-PLC-041: one row per safety channel: F-destination and F-source, module, channel and its pair.
#[test]
fn the_safety_io_list_has_a_row_per_safety_channel() {
    assert_eq!(
        rows("safety_io_list"),
        [
            ["7", "1", "-A2", "0", "1", "", "", ""],
            ["7", "1", "-A2", "1", "0", "", "", ""],
        ]
    );
}

/// FR-BUS-008: a port with an address that names no logical net is listed, after the nets, with an empty net.
#[test]
fn the_network_address_list_lists_ports_without_a_net() {
    let rows = rows("network_address_list");
    let stations: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|r| (r[0].as_str(), r[1].as_str(), r[3].as_str()))
        .collect();
    assert_eq!(stations, [("", "-A3", "5"), ("", "-A4", "TBD")]);
}

/// A bus cable's length prints in metres to the millimetre without trailing zeros: 12.5 m.
#[test]
fn the_bus_cable_list_prints_the_length_in_metres() {
    let rows = rows("bus_cable_list");
    assert_eq!(rows.len(), 1, "{rows:?}");
    // A bus cable takes the page of its line.
    assert_eq!(rows[0][0], "-1W1");
    assert_eq!(rows[0][3], "12.5 m");
}
