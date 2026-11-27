// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The SQL projection over the sample project (ADR-0017, doc 17 §6): every `api_v1_*` view is readable through the
//! sandbox with the columns of `ew_sqlview::schema`; the acceptance criteria of FR-EXT-006 and FR-EXT-007; the row
//! counts; a deterministic image; a module's valve positions (`api_v1_valve_position`) with the derivation's rows.

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::sql::{Profile, Projection, SqlError, schema};
use ew_derive::{ValveTerminalRows, VtSlot};
use ew_ids::Uuid;
use ew_schema::{DeviceState, Record};
use serde_json::{Value, json};

fn projection() -> Projection {
    let session = crate::common::sample();
    let report = session.check(&PRE_DELIVERY, CheckMode::Release);
    session
        .sql_projection(Some(&report))
        .expect("the projection builds")
}

#[test]
fn every_view_has_its_documented_columns_and_the_sample_counts() {
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    for v in schema::VIEWS {
        let set = reader.query(&format!("SELECT * FROM {}", v.name)).unwrap();
        let names: Vec<&str> = v.columns.iter().map(|c| c.name).collect();
        assert_eq!(set.columns, names, "{}", v.name);
        // Every value has its column's declared type or is NULL.
        for c in v.columns {
            let types = reader
                .query(&format!(
                    "SELECT DISTINCT typeof({}) FROM {} ORDER BY 1",
                    c.name, v.name
                ))
                .unwrap();
            let want = c.ty.sql().to_ascii_lowercase();
            for t in types.rows.iter().map(|r| r[0].text()) {
                assert!(
                    t == want || t == "null",
                    "{}.{}: a {t} value in a {want} column",
                    v.name,
                    c.name
                );
            }
        }
    }
    let count = |view: &str| {
        reader
            .query(&format!("SELECT count(*) FROM {view}"))
            .unwrap()
            .rows[0][0]
            .text()
    };
    // The sample's script: 18 terminals on the strips -X1011, -X1021 and -X1031, 3 cables with 13 cores (5, 4 and a
    // shield, 2 and a shield), the two bridge groups of -X1021, the one bond
    // (0 V to PE), a row per page, 7 tubes, no medium.
    assert_eq!(count("api_v1_terminal"), "18");
    assert_eq!(count("api_v1_cable"), "3");
    assert_eq!(count("api_v1_conductor"), "13");
    assert_eq!(
        reader
            .query("SELECT count(*) FROM api_v1_wire WHERE kind = 'bridge'")
            .unwrap()
            .rows[0][0]
            .text(),
        "2"
    );
    assert_eq!(count("api_v1_bond"), "1");
    // A row per page of the page order, drawn and generated.
    let pages = crate::common::sample().page_labels().len().to_string();
    assert_eq!(count("api_v1_page"), pages);
    assert_eq!(count("api_v1_meta"), "1");
    assert_eq!(count("api_v1_fluid_line"), "7");
    assert_eq!(count("api_v1_medium"), "0");
    // The 24 channels of the two cards and the valve terminal; 6 outputs used: two lamps and four valves.
    assert_eq!(count("api_v1_plc_channel"), "24");
    assert_eq!(
        reader
            .query("SELECT count(*) FROM api_v1_plc_channel WHERE state = 'used' AND address LIKE 'Q%'")
            .unwrap()
            .rows[0][0]
            .text(),
        "6"
    );
    // doc 10 §5: the channel view carries the I/O list's station bus address, CPU, passed terminals and F-address;
    // the valve terminal -201K1 is at DP address 3 under the CPU -104A1, its outputs pass no terminal, no F-module.
    let first = reader
        .query(
            "SELECT bus_address, cpu, passed_terminals, f_address FROM api_v1_plc_channel \
             WHERE module = '-201K1' LIMIT 1",
        )
        .unwrap();
    let first: Vec<String> = first.rows[0].iter().map(|v| v.text()).collect();
    assert_eq!(first, ["3", "-104A1", "[]", ""]);
    // The counts the projection reports equal the views'.
    for (view, n) in p.counts() {
        assert_eq!(count(view), n.to_string(), "{view}");
    }
}

#[test]
fn fr_ext_006_counts_terminals_and_denies_attach() {
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let set = reader.query("SELECT count(*) FROM api_v1_terminal").unwrap();
    assert_eq!(set.rows[0][0].text(), "18", "-X1011: 5, -X1021: 8, -X1031: 5");
    assert!(matches!(
        reader.query("ATTACH 'other.db' AS x"),
        Err(SqlError::NotAQuery(_))
    ));
    assert!(matches!(
        reader.query("SELECT 1; ATTACH 'other.db' AS x"),
        Err(SqlError::Denied(_) | SqlError::MultipleStatements)
    ));
}

#[test]
fn fr_ext_007_sorts_designations_naturally_and_reads_multilingual_text() {
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let set = reader
        .query(
            "SELECT d FROM (SELECT 'X10' AS d UNION ALL SELECT 'X2' UNION ALL SELECT 'X1') \
             ORDER BY d COLLATE ew_natural",
        )
        .unwrap();
    let got: Vec<String> = set.rows.iter().map(|r| r[0].text()).collect();
    assert_eq!(got, ["X1", "X2", "X10"], "X2 before X10");
    let set = reader
        .query("SELECT ew_ml(title_ml, 'en'), title FROM api_v1_page WHERE folio = '101'")
        .unwrap();
    assert_eq!(set.rows[0][0], set.rows[0][1], "ew_ml reads the page title");
}

/// A cross-section from the rating of the protecting device names that device and its rating in
/// `prov_csa`, as Properties does: the sample's 102.1 behind the 16 A breaker -102F1. It said "size profile" and the
/// current table's key, which is no size profile.
#[test]
fn a_size_from_a_rating_names_the_device_in_its_provenance() {
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let set = reader
        .query("SELECT prov_csa FROM api_v1_wire WHERE number = '102.1'")
        .unwrap();
    assert_eq!(
        set.rows[0][0].text(),
        format!(
            "protected by -102F1, rated 16 A · {}",
            ew_derive::current_table_basis()
        )
    );
}

#[test]
fn two_builds_write_the_same_image() {
    let (a, b) = (projection(), projection());
    assert!(a.image() == b.image(), "the projection image is deterministic");
    assert!(a.image().starts_with(b"SQLite format 3\0"));
}

#[test]
fn messages_carry_the_check_run() {
    // The sample is clean; -101F1's tag frozen as F1 deviates from the scheme (EW-NUM-001, an info).
    let mut session = crate::common::sample();
    let f1 = session.resolve_reference("-101F1").expect("-101F1");
    session
        .execute(&ew_api::AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "device", "targets": [f1.to_string()], "path": "tag.frozen",
                "value": {"text": "F1"}}),
        })
        .expect("the frozen tag");
    let report = session.check(&PRE_DELIVERY, CheckMode::Release);
    let p = session
        .sql_projection(Some(&report))
        .expect("the projection builds");
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let set = reader
        .query(
            "SELECT rule, severity, state, target_tags FROM api_v1_message WHERE rule = 'EW-NUM-001' \
             ORDER BY target_tags",
        )
        .unwrap();
    let rows: Vec<Vec<String>> = set
        .rows
        .iter()
        .map(|r| r.iter().map(|v| v.text()).collect())
        .collect();
    assert_eq!(rows, [["EW-NUM-001", "info", "open", "+CAB-F1"]]);
}

/// A modular valve terminal. The sample's -201K1 holds its four valve positions in its head; the set-up puts a module
/// under it (on the head's own part, four positions), moves the four valves onto the module at their positions and
/// marks the fourth spare. Each valve carries EW-GEN-VLV-3, which declares solenoid 14, and its coil function is bound
/// to one of the head's outputs (channels 0, 2, 4 and 6). `valve_terminal_rows()` gives the terminal, the module and
/// one row per position with the valve, its solenoids, the channel bound to its coil, the address of that output and
/// the valve's state; `api_v1_valve_position` gives the module's four rows with the same facts.
#[test]
fn a_modules_valve_positions_are_rows_of_the_derivation_and_of_the_view() {
    let (project, records, lib) = crate::common::sample_records();
    let mut values: Vec<Value> = records
        .iter()
        .map(|r| serde_json::to_value(r).expect("a record serialises"))
        .collect();
    let terminal = values
        .iter()
        .find(|v| v["$k"] == "device" && v["plc_station"]["role"] == "valve_terminal")
        .expect("the sample's valve terminal")
        .clone();
    let terminal_id = terminal["id"].as_str().unwrap().to_owned();
    let module_id = Uuid::from_u128(0x4d4f_d001).to_string();
    // The valves by position, each moved onto the module; the fourth is spare.
    let mut valves: Vec<(u64, String, String)> = Vec::new();
    for v in values
        .iter_mut()
        .filter(|v| v["$k"] == "device" && v["parent"] == terminal_id.as_str())
    {
        let position = v["position"].as_u64().expect("a valve's position");
        v["parent"] = json!(module_id);
        if position == 4 {
            v["state"] = json!("spare");
        }
        let part = v["parts"][0]["part"]["copy"].as_str().unwrap().to_owned();
        valves.push((position, v["id"].as_str().unwrap().to_owned(), part));
    }
    valves.sort();
    assert_eq!(valves.iter().map(|v| v.0).collect::<Vec<_>>(), [1, 2, 3, 4]);
    values.push(json!({
        "$k": "device", "id": module_id, "tag": {"letter": "A", "counter": 1, "scheme": "child"},
        "parent": terminal_id, "position": 1, "parts": terminal["parts"].clone(), "plc_module": {},
    }));
    // The channel bound to each valve's coil, and its number.
    let channel_of = |valve: &str| -> (String, u64) {
        let functions: Vec<&str> = values
            .iter()
            .filter(|f| f["$k"] == "function" && f["device"] == valve)
            .map(|f| f["id"].as_str().unwrap())
            .collect();
        let bound: Vec<(String, u64)> = values
            .iter()
            .filter(|c| c["$k"] == "channel" && c["module"] == terminal_id.as_str())
            .filter(|c| {
                c["binding"]["cp"][0]
                    .as_str()
                    .is_some_and(|f| functions.contains(&f))
            })
            .map(|c| {
                (
                    c["id"].as_str().unwrap().to_owned(),
                    c["number"].as_u64().unwrap(),
                )
            })
            .collect();
        let [one] = bound.as_slice() else {
            panic!("one channel bound to {valve}'s coil: {bound:?}")
        };
        one.clone()
    };
    let channels: Vec<(String, u64)> = valves.iter().map(|v| channel_of(&v.1)).collect();
    let records: Vec<Record> = values
        .into_iter()
        .map(|v| Record::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}")))
        .collect();
    let s = crate::common::session_of(project, records, lib);

    let q = s.query();
    let rows: Vec<_> = q
        .get::<ValveTerminalRows>(())
        .iter()
        .filter(|r| r.terminal.to_string() == terminal_id)
        .cloned()
        .collect();
    assert_eq!(
        rows.len(),
        2 + 4,
        "the terminal, the module and its four positions: {rows:#?}"
    );
    assert_eq!(
        (rows[0].module.to_string(), rows[0].slot.clone(), rows[0].position),
        (terminal_id.clone(), VtSlot::Module, None)
    );
    assert_eq!(
        (
            rows[1].module.to_string(),
            rows[1].module_position,
            rows[1].slot.clone()
        ),
        (module_id.clone(), Some(1), VtSlot::Module)
    );
    for ((row, (position, valve, _)), (channel, number)) in rows[2..].iter().zip(&valves).zip(&channels) {
        let state = if *position == 4 {
            DeviceState::Spare
        } else {
            DeviceState::Used
        };
        assert_eq!(row.module.to_string(), module_id);
        assert_eq!(row.position, Some(u32::try_from(*position).unwrap()));
        assert_eq!(row.slot, VtSlot::Valve);
        assert_eq!(row.device.map(|d| d.to_string()).as_deref(), Some(valve.as_str()));
        assert_eq!(row.solenoids, ["14"], "EW-GEN-VLV-3 declares solenoid 14");
        assert_eq!(
            row.channels.iter().map(ToString::to_string).collect::<Vec<_>>(),
            std::slice::from_ref(channel)
        );
        assert_eq!(row.addresses.as_deref(), Some(format!("Q0.{number}").as_str()));
        assert_eq!(row.state.as_ref(), Some(&state));
    }

    let p = s.sql_projection(None).expect("the projection builds");
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let set = reader
        .query(&format!(
            "SELECT position, occupancy, valve_id, valve_part, solenoids, channel_ids, addresses, state \
             FROM api_v1_valve_position WHERE module_id = '{module_id}' ORDER BY position"
        ))
        .unwrap();
    let got: Vec<Vec<String>> = set
        .rows
        .iter()
        .map(|r| r.iter().map(|c| c.text()).collect())
        .collect();
    let want: Vec<Vec<String>> = valves
        .iter()
        .zip(&channels)
        .map(|((position, valve, _), (channel, number))| {
            vec![
                position.to_string(),
                "valve".to_owned(),
                valve.clone(),
                "EW-GEN-VLV-3".to_owned(),
                json!(["14"]).to_string(),
                json!([channel]).to_string(),
                format!("Q0.{number}"),
                if *position == 4 { "spare" } else { "used" }.to_owned(),
            ]
        })
        .collect();
    assert_eq!(got, want);
}

/// A wire's length and where it comes from read in `api_v1_wire` as the wire list prints them (doc 13 §3.1, doc 08
/// §10), from the wire record: the sample's 101.1 typed, 101.2 routed, a core of -101W1 with its cable, 104.4 made on
/// site. Both columns were NULL for every wire.
#[test]
fn a_wires_length_and_its_source_are_the_wire_lists() {
    let session = crate::common::sample();
    let q = session.query();
    let records = q.get::<ew_derive::WireRecordSet>(());
    let length = |number: &str| {
        let r = records
            .iter()
            .find(|r| r.number.as_deref() == Some(number))
            .unwrap_or_else(|| panic!("{number}"));
        r.length.value().map(|n| n.0.to_string()).expect("a length")
    };
    let p = projection();
    let reader = p.reader(Profile::EW_SQL).unwrap();
    let row = |filter: &str| {
        let set = reader
            .query(&format!(
                "SELECT length_nm, length_source FROM api_v1_wire WHERE {filter} LIMIT 1"
            ))
            .unwrap();
        (set.rows[0][0].text(), set.rows[0][1].text())
    };
    assert_eq!(row("number = '101.1'"), ("400000000".to_owned(), "typed".to_owned()));
    assert_eq!(row("number = '101.2'"), (length("101.2"), "routed".to_owned()));
    assert_eq!(row("cable_tag = '-101W1'").1, "cable");
    assert_eq!(row("number = '104.4'").1, "site");
}
