// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC exchange on Golden A (doc 10 §4, AC-30): the AML, tag table and PLCopen exports of the valve terminal
//! -201U1 with the symbol template `{target.process_tag}` (Golden A at 1.0), the file that comes back with its
//! three-way merge, ownership and the parking list, the drift rule and the instrument list.

use std::collections::BTreeSet;

use crate::golden_common;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::plc_exchange::{ApcVersion, PlcFormat, PlcImport, PlcOptions};
use ew_api::{AnyCommand, Origin, Session};
use ew_derive::{ChannelAddr, ChannelAddresses, PlcRows, Q};
use ew_ids::Id;
use ew_import_catalog::SourceFormat;
use ew_import_catalog::sheet::read_rows;
use ew_plc::ew_caex::{ExchangeImage, IoType, Limits, read_aml};
use ew_plc::{Choice, State};
use ew_schema::{Channel, ChannelState, Device, ExchangeDirection, ForeignId, PlcConfigProject};
use serde_json::{Value, json};

/// The symbols of the eight used channels (doc 20, AC-30; doc 10 §9.2).
const SYMBOLS: [&str; 8] = [
    "TA05_V_15",
    "TA05_V_33",
    "TA05_V_18",
    "TA05_V_17",
    "TA05_V_36",
    "TA05_V_38",
    "TA05_V_16",
    "TA05_V_37",
];

fn run(s: &mut Session, name: &str, params: Value) {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e}"));
}

fn refused(s: &mut Session, name: &str, params: Value) -> String {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .expect_err(name)
    .to_string()
}

/// A Bulk command previewed and committed, as a front end commits it.
fn bulk(s: &mut Session, cmd: &AnyCommand) -> ew_api::Receipt {
    let preview = s.preview(cmd).unwrap_or_else(|e| panic!("{}: {e}", cmd.name));
    s.commit(preview).unwrap_or_else(|e| panic!("{}: {e}", cmd.name))
}

fn device(s: &Session, tag: &str) -> Id<Device> {
    Id::from_uuid(s.resolve_reference(tag).unwrap_or_else(|e| panic!("{tag}: {e}")))
}

fn cfg(q: &Q) -> Id<PlcConfigProject> {
    q.table::<PlcConfigProject>().iter().next().expect("PLC1").0
}

/// Golden A with the symbol template of its 1.0 state.
fn golden() -> Session {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let c = cfg(&s.query());
    run(
        &mut s,
        "fields.set",
        json!({
            "kind": "plc_config_project",
            "targets": [c.to_string()],
            "path": "symbols",
            "value": {"template": "{target.process_tag}"},
        }),
    );
    s
}

/// The export of `format`, recorded in the project as the CLI records it.
fn export(s: &mut Session, format: PlcFormat) -> Vec<u8> {
    let e = s.plc_export(format, &PlcOptions::default()).expect("the export");
    if let Some(record) = &e.record {
        s.execute(record).expect("the export is recorded");
    }
    e.bytes
}

fn aml(s: &mut Session) -> String {
    String::from_utf8(export(s, PlcFormat::Aml(ApcVersion::V1_4_0))).expect("AML is UTF-8")
}

fn import(s: &Session, bytes: &[u8]) -> PlcImport {
    s.plc_import(bytes, &PlcOptions::default())
        .expect("the file is read")
}

fn image(text: &str) -> ExchangeImage {
    read_aml(text.as_bytes(), &Limits::default()).expect("the export is read back")
}

/// The channel whose determined symbol is `symbol`.
fn channel_of(s: &Session, symbol: &str) -> Id<Channel> {
    let q = s.query();
    q.get::<PlcRows>(device(s, "-201U1"))
        .iter()
        .find(|r| r.text.symbol.as_deref() == Some(symbol))
        .unwrap_or_else(|| panic!("no channel with the symbol {symbol}"))
        .channel
}

fn printed(q: &Q, module: Id<Device>) -> Vec<String> {
    q.get::<ChannelAddresses>(module)
        .iter()
        .map(|e| match &e.address {
            ChannelAddr::At { text, .. } => text.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn findings(s: &Session, rule: &str) -> Vec<ew_api::checks::Finding> {
    s.check(&PRE_DELIVERY, CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect()
}

/// AC-30, FR-PLC-029, FR-PLC-056, FR-EXP-009: the AML export of station -201U1 is AR APC 1.4.0 on CAEX 2.15 with the
/// seven modules on Q0 … Q6, 56 channels and the eight tags linked to their channels; it carries the project's own
/// IDs and IEC designations and no person field; the same model gives the same bytes; 1.2.0 differs in its version.
#[test]
fn the_aml_export_of_the_golden_station_is_ar_apc_on_caex_2_15() {
    let s = golden();
    let station = device(&s, "-201U1");
    let options = PlcOptions {
        stations: vec!["-201U1".to_owned()],
        ..PlcOptions::default()
    };
    let export = s
        .plc_export(PlcFormat::Aml(ApcVersion::V1_4_0), &options)
        .expect("the export");
    let text = String::from_utf8(export.bytes.clone()).unwrap();
    assert!(text.contains("SchemaVersion=\"2.15\""));
    assert!(text.contains("AutomationMLVersion=\"2.0\""));
    assert!(text.contains("<Document DocumentIdentifier=\"AR APC\" Version=\"1.4.0\"/>"));

    let image = image(&text);
    assert_eq!(image.project.name, "PLC1");
    assert_eq!(image.stations.len(), 1);
    let st = &image.stations[0];
    assert_eq!(st.id, station.to_string(), "the station's ID is its device's");
    assert_eq!(st.iec.designation.as_deref(), Some("-201U1"));
    assert_eq!(st.iec.location.as_deref(), Some("+J-E-2416"));
    assert_eq!(st.items.len(), 7);
    for (i, item) in st.items.iter().enumerate() {
        let i = u32::try_from(i).unwrap();
        assert_eq!(item.position, i + 1, "{}", item.name);
        let ranges: Vec<(u32, u32, IoType)> =
            item.addresses.iter().map(|a| (a.start, a.bits, a.io)).collect();
        assert_eq!(ranges, [(i, 8, IoType::Output)], "{}", item.name);
        assert_eq!(item.channels.len(), 8, "{}", item.name);
    }
    assert_eq!(st.items[0].name, "-201U3");
    assert_eq!(st.items[0].channels[1].name, "Channel_DO_1");
    let names: BTreeSet<&str> = image.tags.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, SYMBOLS.into_iter().collect());
    let ids: BTreeSet<&str> = st
        .items
        .iter()
        .flat_map(|i| &i.channels)
        .map(|c| c.id.as_str())
        .collect();
    for tag in &image.tags {
        assert!(
            tag.channel.as_deref().is_some_and(|c| ids.contains(c)),
            "{} is linked to a channel of the station",
            tag.name
        );
    }
    let first = image.tags.iter().find(|t| t.name == "TA05_V_15").unwrap();
    assert_eq!(first.address.as_deref(), Some("0.1"));
    assert_eq!(first.io, Some(IoType::Output));
    assert_eq!(export.counts, [1, 7, 56, 8]);

    // no person field: none of the project's party texts is in the file
    let parties = serde_json::to_value(&s.query().project().parties).unwrap();
    let mut texts = Vec::new();
    let mut stack = vec![&parties];
    while let Some(v) = stack.pop() {
        match v {
            Value::String(t) if t.chars().count() > 3 => texts.push(t.clone()),
            Value::Array(a) => stack.extend(a),
            Value::Object(o) => stack.extend(o.values()),
            _ => {}
        }
    }
    assert!(!texts.is_empty(), "Golden A names its parties");
    for t in texts {
        assert!(!text.contains(&t), "the file holds the party text `{t}`");
    }

    let again = s
        .plc_export(PlcFormat::Aml(ApcVersion::V1_4_0), &options)
        .expect("the export");
    assert_eq!(again.bytes, export.bytes, "the same model gives the same bytes");
    let older = s
        .plc_export(PlcFormat::Aml(ApcVersion::V1_2_0), &options)
        .expect("the export");
    assert_eq!(
        String::from_utf8(older.bytes).unwrap(),
        text.replace("Version=\"1.4.0\"", "Version=\"1.2.0\"")
    );
}

/// AC-30, FR-PLC-036, FR-PLC-037: the tag table has exactly the ten columns and one row per used channel with its
/// symbol and its TIA address; the PLCopen list has the same eight variables with IEC addresses.
#[test]
fn the_tag_table_and_the_gvl_hold_the_eight_used_channels() {
    let mut s = golden();
    let table = s
        .plc_export(PlcFormat::TiaXlsx, &PlcOptions::default())
        .expect("the tag table");
    assert!(
        table
            .warnings
            .iter()
            .any(|w| w.contains("not confirmed by the PLC configuration")),
        "{:?}",
        table.warnings
    );
    let rows = read_rows(
        &table.bytes,
        SourceFormat::Xlsx,
        ew_plc::tia::SHEET,
        &ew_import_catalog::table_limits(),
    )
    .expect("the sheet PLC Tags");
    assert_eq!(rows[0][..10], ew_plc::tia::COLUMNS.map(str::to_owned));
    let names: BTreeSet<&str> = rows[1..].iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, SYMBOLS.into_iter().collect());
    let row = rows.iter().find(|r| r[0] == "TA05_V_15").unwrap();
    assert_eq!(row[1], "EW_IO");
    assert_eq!(row[3], "%Q0.1");
    assert_eq!(row[4], "TA05-V-15", "the comment is the function text");
    assert_eq!(row[5..8], ["True", "True", "True"].map(str::to_owned));
    assert_eq!(
        row[10],
        channel_of(&s, "TA05_V_15").to_string(),
        "the hidden ID column"
    );

    let gvl = String::from_utf8(export(&mut s, PlcFormat::Plcopen)).unwrap();
    assert!(gvl.contains("<project xmlns=\"http://www.plcopen.org/xml/tc6_0201\">"));
    assert!(gvl.contains("<globalVars name=\"IO_ElektraWatt\">"));
    assert_eq!(gvl.matches("<variable ").count(), 8);
    for symbol in SYMBOLS {
        assert!(gvl.contains(&format!("<variable name=\"{symbol}\"")), "{symbol}");
    }
    assert!(gvl.contains("<variable name=\"TA05_V_15\" address=\"%QX0.1\">"));
    assert!(
        gvl.contains(">TA05-V-15 · "),
        "the documentation starts with the function text"
    );
}

/// FR-PLC-047: a manual symbol that is no IEC identifier blocks the AML and PLCopen exports under the strict policy
/// (EW-PLC-005) and not the tag table.
#[test]
fn a_symbol_that_is_no_identifier_blocks_the_export() {
    let mut s = golden();
    let channel = channel_of(&s, "TA05_V_15");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "channel", "targets": [channel.to_string()], "path": "symbol", "value": "TA05-V-15"}),
    );
    for format in [PlcFormat::Aml(ApcVersion::V1_4_0), PlcFormat::Plcopen] {
        let e = s.plc_export(format, &PlcOptions::default()).expect_err("blocked");
        assert!(e.to_string().contains("EW-PLC-005"), "{e}");
        assert!(e.to_string().contains("TA05-V-15"), "{e}");
    }
    s.plc_export(PlcFormat::TiaXlsx, &PlcOptions::default())
        .expect("the tag table takes any symbol");
}

/// FR-PLC-032, FR-PLC-033: an unchanged export comes back as an empty diff. A symbol renamed in the PLC software
/// and a function text changed in the project merge without a conflict, as one undoable transaction with its
/// snapshot; the same symbol renamed on both sides is one conflict.
#[test]
fn a_file_that_comes_back_merges_three_ways() {
    let mut s = golden();
    let exported = aml(&mut s);
    let unchanged = import(&s, exported.as_bytes());
    assert!(unchanged.plan.changes.is_empty(), "{:?}", unchanged.plan.changes);
    assert!(unchanged.plan.parking.is_empty() && unchanged.plan.removed.is_empty());

    // the PLC programmer renames the tag; the engineer gives the valve a function text
    let theirs = exported.replace("TA05_V_15", "TA05_V_15A");
    let valve = device(&s, "-201U3-Q1");
    run(
        &mut s,
        "fields.set",
        json!({
            "kind": "device",
            "targets": [valve.to_string()],
            "path": "function_text",
            "value": {"en-GB": "Feed valve"},
        }),
    );
    let channel = channel_of(&s, "TA05_V_15");
    let merged = import(&s, theirs.as_bytes());
    let name = merged
        .plan
        .changes
        .iter()
        .find(|c| c.field == "name")
        .expect("the renamed symbol");
    assert_eq!((name.state, name.default), (State::Apply, Choice::Theirs));
    assert_eq!(name.theirs.as_deref(), Some("TA05_V_15A"));
    assert!(
        merged.plan.changes.iter().all(|c| c.state != State::Conflict),
        "{:?}",
        merged.plan.changes
    );
    assert!(
        merged
            .plan
            .changes
            .iter()
            .any(|c| c.field.starts_with("comment") && c.state == State::Kept),
        "the project's new function text is kept: {:?}",
        merged.plan.changes
    );

    let receipt = bulk(&mut s, &merged.apply);
    assert!(
        receipt
            .outcome
            .notices
            .iter()
            .any(|n| n.contains("TA05_V_15A") && n.contains("taken from the file")),
        "{:?}",
        receipt.outcome.notices
    );
    let q = s.query();
    let symbol = |q: &Q| {
        q.table::<Channel>()
            .get(channel)
            .unwrap()
            .symbol
            .as_ref()
            .map(ToString::to_string)
    };
    assert_eq!(symbol(&q).as_deref(), Some("TA05_V_15A"));
    let stamp = |q: &Q| {
        q.table::<PlcConfigProject>()
            .get(cfg(q))
            .unwrap()
            .exchange
            .clone()
            .unwrap()
    };
    assert_eq!(stamp(&q).direction, ExchangeDirection::Import);
    // the snapshot is the file now: the same file again changes the name no more
    assert!(
        import(&s, theirs.as_bytes())
            .plan
            .changes
            .iter()
            .all(|c| c.field != "name")
    );
    s.undo(&Origin::Gui).expect("the import is one undo step");
    let q = s.query();
    assert_eq!(symbol(&q), None, "the symbol is the template's again");
    assert_eq!(
        stamp(&q).direction,
        ExchangeDirection::Export,
        "and the snapshot the export's"
    );

    // both sides rename the symbol
    run(
        &mut s,
        "fields.set",
        json!({"kind": "channel", "targets": [channel.to_string()], "path": "symbol", "value": "FEED_15"}),
    );
    let both = import(&s, theirs.as_bytes());
    let conflicts: Vec<_> = both
        .plan
        .changes
        .iter()
        .filter(|c| c.state == State::Conflict)
        .collect();
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert_eq!(conflicts[0].field, "name");
    assert_eq!(
        conflicts[0].default,
        Choice::Ours,
        "a shared field keeps the project's value"
    );
    // the choice `theirs` takes the file's name
    let options = PlcOptions {
        resolutions: [(conflicts[0].id.clone(), "theirs".to_owned())].into(),
        ..PlcOptions::default()
    };
    let taken = s
        .plc_import(theirs.as_bytes(), &options)
        .expect("the file is read");
    bulk(&mut s, &taken.apply);
    assert_eq!(symbol(&s.query()).as_deref(), Some("TA05_V_15A"));
}

/// FR-PLC-056: a file whose IDs the PLC software replaced still matches every object by its IEC designation and
/// position; applying it keeps the foreign IDs, by which the next file matches.
#[test]
fn replaced_ids_match_by_designation() {
    let mut s = golden();
    let exported = aml(&mut s);
    let uuid = regex::Regex::new("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
    let foreign = uuid.replace_all(&exported, "tia-$0").into_owned();
    assert_ne!(foreign, exported);
    let back = import(&s, foreign.as_bytes());
    assert!(back.plan.changes.is_empty(), "{:?}", back.plan.changes);
    assert!(back.plan.parking.is_empty(), "{:?}", back.plan.parking);
    assert_eq!(
        back.plan
            .matched
            .iter()
            .filter(|m| m.object.name() == "module")
            .count(),
        7
    );
    bulk(&mut s, &back.apply);
    let q = s.query();
    let kept = q
        .table::<ForeignId>()
        .iter()
        .filter(|(_, f)| f.foreign.starts_with("tia-"))
        .count();
    assert_eq!(
        kept,
        back.plan.matched.len(),
        "every matched object keeps its foreign ID"
    );
}

/// FR-PLC-032: a module of the file without an IEC designation at a position the station has not is parked, never
/// created silently; `create` creates it as an unplaced child of its station, and the next file finds it.
#[test]
fn a_module_the_project_lacks_is_parked_and_created_on_request() {
    let mut s = golden();
    let exported = aml(&mut s);
    let station = device(&s, "-201U1");
    let first = format!("<InternalElement ID=\"{}\"", device(&s, "-201U3"));
    let module = "<InternalElement ID=\"foreign-module\" Name=\"AI 8xU\">\
        <Attribute Name=\"PositionNumber\"><Value>42</Value></Attribute>\
        <Attribute Name=\"TypeIdentifier\"><Value>OrderNumber:6ES7 531-7KF00-0AB0</Value></Attribute>\
        <SupportedRoleClass RefRoleClassPath=\"AutomationProjectConfigurationRoleClassLib/DeviceItem\"/>\
        </InternalElement>";
    assert!(exported.contains(&first));
    let theirs = exported.replacen(&first, &format!("{module}{first}"), 1);
    let devices = |s: &Session| s.query().devices().len();
    let before = devices(&s);

    let parked = import(&s, theirs.as_bytes());
    assert!(parked.plan.changes.is_empty(), "{:?}", parked.plan.changes);
    assert_eq!(parked.plan.parking.len(), 1);
    let entry = &parked.plan.parking[0];
    assert_eq!(
        (entry.id.as_str(), entry.name.as_str()),
        ("foreign-module", "AI 8xU")
    );
    assert!(entry.creatable && !entry.ambiguous);
    bulk(&mut s, &parked.apply);
    assert_eq!(devices(&s), before, "a parked module is not created");

    let options = PlcOptions {
        resolutions: [("foreign-module".to_owned(), "create".to_owned())].into(),
        ..PlcOptions::default()
    };
    let create = s
        .plc_import(theirs.as_bytes(), &options)
        .expect("the file is read");
    bulk(&mut s, &create.apply);
    let q = s.query();
    assert_eq!(q.devices().len(), before + 1);
    let created = q
        .devices()
        .iter()
        .find(|(_, d)| {
            d.aspects
                .plc_module
                .as_ref()
                .and_then(|m| m.type_ident.as_ref())
                .and_then(|t| t.order_number.as_ref())
                .is_some_and(|n| n.as_str() == "6ES7 531-7KF00-0AB0")
        })
        .expect("the created module");
    assert_eq!(created.1.parent, Some(station));
    assert!(
        q.idx().functions_of_device(created.0).next().is_none(),
        "it is unplaced"
    );
    let again = import(&s, theirs.as_bytes());
    assert!(again.plan.parking.is_empty(), "{:?}", again.plan.parking);
}

/// FR-PLC-033, doc 10 §3.9: a start the PLC software changed is applied as a manual start, and from that import on the
/// starts are the PLC's: an edit is refused in words until `plc.unlock` lifts the protection.
#[test]
fn an_imported_start_is_the_plcs_until_unlocked() {
    let mut s = golden();
    let exported = aml(&mut s);
    let (u5, u6) = (device(&s, "-201U5"), device(&s, "-201U6"));
    // -201U5 moves from Q2 to Q10 in the PLC software
    let at = exported.find("Name=\"-201U5\"").expect("the module");
    let (head, tail) = exported.split_at(at);
    let theirs = format!(
        "{head}{}",
        tail.replacen("<Value>2</Value>", "<Value>10</Value>", 1)
    );
    let moved = import(&s, theirs.as_bytes());
    let start: Vec<_> = moved
        .plan
        .changes
        .iter()
        .filter(|c| c.field == "start.output")
        .collect();
    assert_eq!(start.len(), 1, "{:?}", moved.plan.changes);
    assert_eq!(start[0].state, State::Apply);
    assert_eq!(
        (start[0].ours.as_deref(), start[0].theirs.as_deref()),
        (Some("2"), Some("10"))
    );
    let set_start = |m: Id<Device>| json!({"modules": [m.to_string()], "dir": "output", "spec": "Q20"});
    run(&mut s, "plc.set_start", set_start(u6));
    s.undo(&Origin::Gui)
        .expect("before an import a start is an ordinary edit");

    bulk(&mut s, &moved.apply);
    let want: Vec<String> = (0..8).map(|n| format!("Q10.{n}")).collect();
    assert_eq!(printed(&s.query(), u5), want);
    let c = cfg(&s.query());
    let refusal = refused(&mut s, "plc.set_start", set_start(u6));
    assert!(
        refusal.contains("-201U6: the start address is owned by configuration project PLC1 (PLC import of "),
        "{refusal}"
    );
    let refusal = refused(
        &mut s,
        "fields.set",
        json!({
            "kind": "device",
            "targets": [u6.to_string()],
            "path": "plc_module.start",
            "value": {"output": "tbd"},
        }),
    );
    assert!(
        refusal.contains("owned by configuration project PLC1"),
        "{refusal}"
    );
    let refusal = s
        .preview(&AnyCommand {
            name: "plc.readdress".into(),
            params: json!({"config_project": c.to_string()}),
        })
        .expect_err("a re-address moves a PLC-owned start")
        .to_string();
    assert!(
        refusal.contains("owned by configuration project PLC1"),
        "{refusal}"
    );

    let unlock = json!({"config_project": c.to_string(), "group": "module_starts"});
    run(&mut s, "plc.unlock", unlock.clone());
    run(&mut s, "plc.set_start", set_start(u6));
    let refusal = refused(&mut s, "plc.unlock", unlock);
    assert!(
        refusal.contains("the start address is not protected"),
        "{refusal}"
    );
}

/// FR-PLC-035, EW-PLC-016: after an export the project is as exchanged; an edit of a symbol is reported as drift with
/// the export's date, and `plc_status` names the object and tells the exchanged file from another.
#[test]
fn an_edit_after_an_export_is_drift() {
    let mut s = golden();
    let exported = aml(&mut s);
    assert!(
        findings(&s, "EW-PLC-016").is_empty(),
        "the project is as exported"
    );
    let status = s.plc_status(None, Some(exported.as_bytes())).expect("the status");
    assert!(status.drift.is_empty());
    assert_eq!(status.same_file, Some(true));

    let channel = channel_of(&s, "TA05_V_15");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "channel", "targets": [channel.to_string()], "path": "symbol", "value": "FEED_15"}),
    );
    let found = findings(&s, "EW-PLC-016");
    assert_eq!(found.len(), 1, "{found:?}");
    let date = s
        .query()
        .table::<PlcConfigProject>()
        .get(cfg(&s.query()))
        .unwrap()
        .exchange
        .clone()
        .unwrap()
        .date;
    assert!(
        found[0].message.starts_with(&format!(
            "configuration project PLC1 changed since the AML export of {date}: 1 object(s)"
        )),
        "{}",
        found[0].message
    );
    let nl = ew_api::checks::render(&found[0], "nl-NL");
    assert!(
        nl.starts_with("configuratieproject PLC1 is gewijzigd sinds de AML-export van "),
        "{nl}"
    );
    let status = s.plc_status(None, Some(b"another file")).expect("the status");
    assert_eq!(status.drift.len(), 1);
    assert_eq!(status.drift[0].2, ["name".to_owned()]);
    assert_eq!(status.same_file, Some(false));
}

/// FR-PLC-036: an unchanged tag table comes back as an empty diff; a renamed row without its ID column is found by
/// its address, and a row without a channel is listed and cannot be created.
#[test]
fn a_tag_table_comes_back() {
    let mut s = golden();
    let table = export(&mut s, PlcFormat::TiaXlsx);
    let unchanged = import(&s, &table);
    assert!(unchanged.plan.changes.is_empty(), "{:?}", unchanged.plan.changes);
    assert!(unchanged.plan.parking.is_empty());

    let limits = ew_import_catalog::table_limits();
    let mut rows = read_rows(&table, SourceFormat::Xlsx, ew_plc::tia::SHEET, &limits).unwrap();
    for row in &mut rows {
        row.truncate(10);
    }
    let renamed = rows.iter_mut().find(|r| r[0] == "TA05_V_15").unwrap();
    renamed[0] = "FEED_15".to_owned();
    rows.push(["GHOST", "EW_IO", "Bool", "%Q99.0"].map(str::to_owned).to_vec());
    let date = ew_reports::xlsx::DateTime {
        year: 2026,
        month: 1,
        day: 2,
        hour: 0,
        minute: 0,
        second: 0,
    };
    let edited = ew_reports::xlsx::plain_xlsx(ew_plc::tia::SHEET, &rows, 10, date).unwrap();
    let back = import(&s, &edited);
    assert_eq!(back.plan.changes.len(), 1, "{:?}", back.plan.changes);
    let change = &back.plan.changes[0];
    assert_eq!((change.field.as_str(), change.state), ("name", State::Apply));
    assert_eq!(change.target, channel_of(&s, "TA05_V_15").cast());
    assert_eq!(back.plan.parking.len(), 1);
    assert_eq!(back.plan.parking[0].name, "GHOST");
    assert!(!back.plan.parking[0].creatable);
    bulk(&mut s, &back.apply);
    channel_of(&s, "FEED_15");
}

/// FR-PLC-050, EW-PRC-002: ten instrument rows with eight DO tags reserve the eight spare channels of -201U5 and
/// -201U6; the two tags without a spare channel of their kind are open items; applying the list again changes nothing.
#[test]
fn an_instrument_list_reserves_spare_channels() {
    let mut s = golden();
    let mut list = String::from("Tag;Description;Signal type;Range;Field location;Preferred station\n");
    for n in 40..48 {
        list.push_str(&format!("TA05-V-{n};Valve {n};DO;;Hall 1;-201U1\n"));
    }
    list.push_str("TA05-S-01;Level switch;DI;;Hall 1;\nTA05-L-01;Level;AI;4..20 mA;Hall 1;\n");
    let command = s
        .instrument_list(None, list.as_bytes())
        .expect("the list is read");
    let devices = s.query().devices().len();
    let receipt = bulk(&mut s, &command);
    let notices = &receipt.outcome.notices;
    assert_eq!(
        notices.iter().filter(|n| n.contains("reserved")).count(),
        8,
        "{notices:?}"
    );
    assert_eq!(
        notices.iter().filter(|n| n.contains("open item")).count(),
        2,
        "{notices:?}"
    );

    let q = s.query();
    assert_eq!(q.devices().len(), devices + 10, "one placeholder per row");
    let (u5, u6) = (device(&s, "-201U5"), device(&s, "-201U6"));
    let reserved: Vec<_> = q
        .table::<Channel>()
        .iter()
        .filter(|(_, c)| c.state == Some(ChannelState::Reserved))
        .map(|(_, c)| (c.module, c.symbol.as_ref().map(ToString::to_string)))
        .collect();
    assert_eq!(reserved.len(), 8);
    assert!(reserved.iter().all(|(m, _)| *m == u5 || *m == u6), "{reserved:?}");
    let symbols: BTreeSet<String> = reserved.into_iter().filter_map(|(_, s)| s).collect();
    assert_eq!(symbols, (40..48).map(|n| format!("TA05_V_{n}")).collect());
    let placeholder = q
        .devices()
        .iter()
        .find(|(_, d)| d.process_tag.as_ref().is_some_and(|t| t.as_str() == "TA05-V-40"))
        .expect("the placeholder of TA05-V-40");
    assert_eq!(placeholder.1.function_text.as_neutral(), Some("Valve 40"));

    let open = findings(&s, "EW-PRC-002");
    let tags: BTreeSet<&str> = open
        .iter()
        .map(|f| f.args.iter().find(|(k, _)| *k == "tag").unwrap().1.as_str())
        .collect();
    assert_eq!(tags, ["TA05-L-01", "TA05-S-01"].into());
    assert!(
        open[0].message.contains("has no channel: no spare"),
        "{}",
        open[0].message
    );

    let again = s
        .instrument_list(None, list.as_bytes())
        .expect("the list is read");
    let preview = s.preview(&again).expect("the preview");
    assert_eq!(preview.op_count(), 0, "the same list changes nothing");
}

/// Doc 14 §11: what is no exchange file is refused in words, and the model stays as it was.
#[test]
fn a_file_that_is_no_exchange_file_is_refused() {
    let s = golden();
    for (bytes, want) in [
        (&b"<html/>"[..], "no AutomationML file"),
        (b"<!DOCTYPE x><CAEXFile SchemaVersion=\"2.15\"/>", "DOCTYPE"),
        (b"PK\x03\x04 not a workbook", "the file cannot be read"),
    ] {
        let e = s.plc_import(bytes, &PlcOptions::default()).expect_err(want);
        assert!(e.to_string().contains(want), "{e}");
    }
    let e = s
        .instrument_list(None, b"name,kind\nA,B\n")
        .expect_err("no instrument list");
    assert!(e.to_string().contains("it is not an instrument list"), "{e}");
    let e = s
        .instrument_list(None, b"tag,signal type\nTA05-V-40,relay\n")
        .expect_err("no signal type");
    assert!(
        e.to_string().contains("the signal type of TA05-V-40 is `relay`"),
        "{e}"
    );
}

/// The function of -201U1 that carries its bus port.
fn bus_function(s: &Session) -> Id<ew_schema::Function> {
    let q = s.query();
    q.idx()
        .functions_of_device(device(s, "-201U1"))
        .find(|f| q.functions().get(*f).is_some_and(|f| f.bus_port.is_some()))
        .expect("the DP port of -201U1")
}

/// FR-PLC-031, FR-PLC-033: the DP station address the PLC software sets is applied to the port, which closes its open
/// item, and from then on the address is the PLC's data.
#[test]
fn a_station_address_of_the_plc_is_applied_and_protected() {
    let mut s = golden();
    let exported = aml(&mut s);
    let port = bus_function(&s);
    assert!(
        exported.contains("<Value>Profibus</Value>"),
        "the DP node is in the file"
    );
    assert!(!exported.contains("NetworkAddress"), "its address is still TBD");
    let node = "Name=\"Node\">";
    let theirs = exported.replacen(
        node,
        &format!("{node}<Attribute Name=\"NetworkAddress\"><Value>3</Value></Attribute>"),
        1,
    );
    let set = import(&s, theirs.as_bytes());
    assert_eq!(set.plan.changes.len(), 1, "{:?}", set.plan.changes);
    let change = &set.plan.changes[0];
    assert_eq!((change.object.name(), change.field.as_str()), ("node", "address"));
    assert_eq!(
        (change.state, change.theirs.as_deref()),
        (State::Apply, Some("3"))
    );
    bulk(&mut s, &set.apply);
    let address = |s: &Session| {
        s.query()
            .functions()
            .get(port)
            .unwrap()
            .bus_port
            .clone()
            .unwrap()
            .address
    };
    assert_eq!(
        address(&s),
        ew_schema::AddrValue::Value(ew_schema::NetAddress::Profibus(3))
    );
    let refusal = refused(
        &mut s,
        "fields.set",
        json!({"kind": "function", "targets": [port.to_string()], "path": "bus_port.address", "value": "tbd"}),
    );
    assert!(
        refusal.contains(
            "-201U1: the PLC's data (firmware, bus address) is owned by configuration project PLC1"
        ),
        "{refusal}"
    );
    // a value that is no station address is refused with the value, and nothing is applied
    let bad = theirs.replacen(
        "NetworkAddress\"><Value>3</Value>",
        "NetworkAddress\"><Value>three</Value>",
        1,
    );
    let preview = import(&s, bad.as_bytes());
    let e = s
        .preview(&preview.apply)
        .expect_err("no station address")
        .to_string();
    assert!(
        e.contains("the file's value `three` for address of -201U1 DP cannot be written"),
        "{e}"
    );
}

/// FR-PLC-031: a PROFINET station exported and read back keeps its IP address, mask, device name and port label; an
/// address the PLC software changes comes back into the port.
#[test]
fn a_profinet_station_keeps_its_ip_mask_and_port_labels() {
    let mut s = golden();
    let port = bus_function(&s);
    run(
        &mut s,
        "fields.set",
        json!({
            "kind": "function",
            "targets": [port.to_string()],
            "path": "bus_port",
            "value": {
                "interface": "X1",
                "port_label": "P1R",
                "bus": "profinet",
                "role": "device",
                "address": {"value": {"profinet": {"name": "cpx-1", "ip": "192.168.0.10", "mask": "255.255.255.0"}}},
            },
        }),
    );
    let exported = aml(&mut s);
    let read = image(&exported);
    let interface = &read.stations[0].interfaces[0];
    assert_eq!(interface.label, "X1");
    assert_eq!(interface.ports[0].label, "P1R");
    let node = interface.node.as_ref().expect("the node");
    assert_eq!(node.bus.as_deref(), Some("Ethernet"));
    assert_eq!(node.address.as_deref(), Some("192.168.0.10"));
    assert_eq!(node.mask.as_deref(), Some("255.255.255.0"));
    assert_eq!(node.device_name.as_deref(), Some("cpx-1"));
    assert!(import(&s, exported.as_bytes()).plan.changes.is_empty());

    let theirs = exported.replace("192.168.0.10", "192.168.0.20");
    let moved = import(&s, theirs.as_bytes());
    assert_eq!(moved.plan.changes.len(), 1, "{:?}", moved.plan.changes);
    bulk(&mut s, &moved.apply);
    let q = s.query();
    let ew_schema::AddrValue::Value(ew_schema::NetAddress::Profinet(a)) =
        q.functions().get(port).unwrap().bus_port.clone().unwrap().address
    else {
        panic!("a PROFINET address");
    };
    assert_eq!(a.ip.map(|ip| ip.to_string()).as_deref(), Some("192.168.0.20"));
    assert_eq!(a.mask.map(|ip| ip.to_string()).as_deref(), Some("255.255.255.0"));
    assert_eq!(a.name.as_str(), "cpx-1");
}

/// FR-PLC-033, doc 10 §2.2: the owner of a group decides the merge. With the starts given to the schematic, a start
/// the PLC software changed is a rejected PLC change that can be accepted; a start the project changed after an
/// unlock is kept, export pending.
#[test]
fn the_owner_of_a_group_decides_its_merge() {
    let mut s = golden();
    let exported = aml(&mut s);
    let c = cfg(&s.query());
    let u5 = device(&s, "-201U5");
    let at = exported.find("Name=\"-201U5\"").expect("the module");
    let (head, tail) = exported.split_at(at);
    let theirs = format!(
        "{head}{}",
        tail.replacen("<Value>2</Value>", "<Value>10</Value>", 1)
    );
    let owner = |s: &mut Session, value: Value| {
        run(
            s,
            "fields.set",
            json!({
                "kind": "plc_config_project",
                "targets": [c.to_string()],
                "path": "ownership.module_starts",
                "value": value,
            }),
        );
    };
    owner(&mut s, json!("ecad"));
    let rejected = import(&s, theirs.as_bytes());
    let start = rejected
        .plan
        .changes
        .iter()
        .find(|c| c.field == "start.output")
        .unwrap();
    assert_eq!((start.state, start.default), (State::Rejected, Choice::Ours));
    assert!(start.acceptable);
    bulk(&mut s, &rejected.apply);
    assert_eq!(
        printed(&s.query(), u5)[0],
        "Q2.0",
        "a rejected PLC change changes nothing"
    );
    let refusal = refused(
        &mut s,
        "fields.set",
        json!({
            "kind": "plc_config_project",
            "targets": [c.to_string()],
            "path": "ownership.module_starts",
            "value": "nobody",
        }),
    );
    assert!(
        refusal.contains("is not an owner; give ecad, plc or shared"),
        "{refusal}"
    );

    // back to the PLC: the same file applies now, and the starts are protected
    owner(&mut s, Value::Null);
    let applied = import(&s, exported.as_bytes());
    bulk(&mut s, &applied.apply);
    run(
        &mut s,
        "plc.unlock",
        json!({"config_project": c.to_string(), "group": "module_starts"}),
    );
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [u5.to_string()], "dir": "output", "spec": "Q30"}),
    );
    let pending = import(&s, exported.as_bytes());
    let start = pending
        .plan
        .changes
        .iter()
        .find(|c| c.field == "start.output")
        .unwrap();
    assert_eq!((start.state, start.default), (State::Pending, Choice::Ours));
    assert_eq!(
        (start.ours.as_deref(), start.theirs.as_deref()),
        (Some("30"), Some("2"))
    );
    // the import protects the starts again
    bulk(&mut s, &pending.apply);
    let refusal = refused(
        &mut s,
        "plc.set_start",
        json!({"modules": [u5.to_string()], "dir": "output", "spec": "Q31"}),
    );
    assert!(
        refusal.contains("owned by configuration project PLC1"),
        "{refusal}"
    );
}

/// The stand-in of the fuzz targets while the fuzz workspace is cut (doc 14 §11): a damaged tag workbook and a
/// damaged instrument list are read or refused in words; the read never panics.
#[test]
fn a_damaged_workbook_or_list_never_panics() {
    let mut s = golden();
    let table = export(&mut s, PlcFormat::TiaXlsx);
    let panicked = |e: &ew_api::plc_exchange::PlcExchangeError| {
        matches!(e, ew_api::plc_exchange::PlcExchangeError::Panicked)
    };
    for cut in (0..table.len()).step_by(37) {
        if let Err(e) = s.plc_import(&table[..cut], &PlcOptions::default()) {
            assert!(!panicked(&e), "the workbook cut at {cut}");
        }
    }
    for (i, byte) in (0..table.len()).step_by(41).zip(b"\0\xffPK<>\"=".iter().cycle()) {
        let mut damaged = table.clone();
        damaged[i] = *byte;
        if let Err(e) = s.plc_import(&damaged, &PlcOptions::default()) {
            assert!(!panicked(&e), "the workbook with byte {i} replaced");
        }
        if let Err(e) = s.instrument_list(None, &damaged) {
            assert!(
                !panicked(&e),
                "the workbook as an instrument list, byte {i} replaced"
            );
        }
    }
    let list = b"tag;signal type;description\nTA05-V-40;DO;\"Valve 40\"\n";
    for (i, byte) in (0..list.len()).zip(b"\0\xff\";\n,".iter().cycle()) {
        let mut damaged = list.to_vec();
        damaged[i] = *byte;
        if let Err(e) = s.instrument_list(None, &damaged) {
            assert!(!panicked(&e), "the list with byte {i} replaced");
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// The merge of doc 10 §4.4: matching, the merge table, ownership and the snapshot

/// `exported` with the output start of `module` moved from byte `from` to byte `to`.
fn with_start(exported: &str, module: &str, from: u32, to: u32) -> String {
    let at = exported.find(&format!("Name=\"{module}\"")).expect("the module");
    let (head, tail) = exported.split_at(at);
    format!(
        "{head}{}",
        tail.replacen(
            &format!("<Value>{from}</Value>"),
            &format!("<Value>{to}</Value>"),
            1
        )
    )
}

/// Gives the module starts to `owner` (`ecad`, `plc`, `shared`).
fn starts_owner(s: &mut Session, owner: &str) {
    let c = cfg(&s.query());
    run(
        s,
        "fields.set",
        json!({
            "kind": "plc_config_project",
            "targets": [c.to_string()],
            "path": "ownership.module_starts",
            "value": owner,
        }),
    );
}

/// What the merge makes of a change: its state, its default and whether the file's value can be taken.
type Outcome = (State, Choice, bool);

/// The output start of -201U5 in the merge of Golden A's recorded AML export: the project's start set to
/// `project` (a start spec), the file's moved from byte 2 to `file`; per owner of the starts, the state, the default
/// and whether the file's value can be taken, `None` for no change.
fn start_row(project: Option<&str>, file: Option<u32>) -> Vec<(&'static str, Option<Outcome>)> {
    let mut s = golden();
    let exported = aml(&mut s);
    if let Some(spec) = project {
        let u5 = device(&s, "-201U5");
        run(
            &mut s,
            "plc.set_start",
            json!({"modules": [u5.to_string()], "dir": "output", "spec": spec}),
        );
    }
    let theirs = file.map_or_else(|| exported.clone(), |to| with_start(&exported, "-201U5", 2, to));
    let u5 = device(&s, "-201U5");
    ["ecad", "plc", "shared"]
        .into_iter()
        .map(|owner| {
            starts_owner(&mut s, owner);
            let plan = import(&s, theirs.as_bytes()).plan;
            let change = plan
                .changes
                .iter()
                .find(|c| c.field == "start.output" && c.target == u5.cast())
                .map(|c| (c.state, c.default, c.acceptable));
            (owner, change)
        })
        .collect()
}

/// Doc 10 §4.4, the merge table's first row: a change of the file alone is a rejected PLC change that can be accepted
/// when the schematic owns the field, and applied when the PLC or both own it.
#[test]
fn the_merge_table_a_change_of_the_file_alone() {
    assert_eq!(
        start_row(None, Some(10)),
        [
            ("ecad", Some((State::Rejected, Choice::Ours, true))),
            ("plc", Some((State::Apply, Choice::Theirs, true))),
            ("shared", Some((State::Apply, Choice::Theirs, true))),
        ]
    );
}

/// Doc 10 §4.4, the second row: a change of the project alone is kept, flagged export pending when the PLC owns it.
#[test]
fn the_merge_table_a_change_of_the_project_alone() {
    assert_eq!(
        start_row(Some("Q20"), None),
        [
            ("ecad", Some((State::Kept, Choice::Ours, true))),
            ("plc", Some((State::Pending, Choice::Ours, true))),
            ("shared", Some((State::Kept, Choice::Ours, true))),
        ]
    );
}

/// Doc 10 §4.4, the third row: a change of both sides is a conflict that keeps the project's value, unless the PLC
/// owns the field.
#[test]
fn the_merge_table_a_change_of_both_sides() {
    assert_eq!(
        start_row(Some("Q20"), Some(10)),
        [
            ("ecad", Some((State::Conflict, Choice::Ours, true))),
            ("plc", Some((State::Conflict, Choice::Theirs, true))),
            ("shared", Some((State::Conflict, Choice::Ours, true))),
        ]
    );
}

/// Doc 10 §4.4, the fourth row: both sides with the same value is no change, whoever owns the field.
#[test]
fn the_merge_table_the_same_change_on_both_sides() {
    assert_eq!(
        start_row(Some("Q10"), Some(10)),
        [("ecad", None), ("plc", None), ("shared", None)]
    );
}

/// Doc 10 §4.4: without a snapshot (an export that was never recorded) every difference of an owned field is a
/// conflict with the owner's default, also one the project has no value for.
#[test]
fn without_a_snapshot_a_difference_is_a_conflict_with_the_owners_default() {
    let s = golden();
    let exported = String::from_utf8(
        s.plc_export(PlcFormat::Aml(ApcVersion::V1_4_0), &PlcOptions::default())
            .expect("the export")
            .bytes,
    )
    .unwrap();
    let node = "Name=\"Node\">";
    let theirs = exported.replacen(
        node,
        &format!("{node}<Attribute Name=\"NetworkAddress\"><Value>3</Value></Attribute>"),
        1,
    );
    let plan = import(&s, theirs.as_bytes()).plan;
    let change = plan
        .changes
        .iter()
        .find(|c| c.object.name() == "node" && c.field == "address")
        .expect("the station address");
    assert_eq!(change.ours, None, "the project's address is still TBD");
    assert_eq!((change.state, change.default), (State::Conflict, Choice::Theirs));
}

/// Doc 10 §4.4 (decided for the review of WP-S3.16): a value the file leaves out is no deletion. It is listed as a
/// rejected PLC change that keeps the project's value, and only a resolution takes the file's side.
#[test]
fn a_value_the_file_leaves_out_is_kept() {
    let mut s = golden();
    let channel = channel_of(&s, "TA05_V_15");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "channel", "targets": [channel.to_string()], "path": "comment", "value": "Feed"}),
    );
    let exported = aml(&mut s);
    assert!(exported.contains("<Value>Feed</Value>"));
    let theirs = exported.replace("<Value>Feed</Value>", "<Value></Value>");
    let back = import(&s, theirs.as_bytes());
    let change = back
        .plan
        .changes
        .iter()
        .find(|c| c.field == "comment")
        .expect("the comment the file leaves out");
    assert_eq!(
        (change.state, change.default, change.theirs.as_deref()),
        (State::Rejected, Choice::Ours, None)
    );
    bulk(&mut s, &back.apply);
    let comment = |s: &Session| s.query().table::<Channel>().get(channel).unwrap().comment.clone();
    assert_eq!(comment(&s).as_neutral(), Some("Feed"));
}

/// FR-PLC-032, A6: matching runs in passes over the whole file. A module the PLC software inserted at -201U3's slot
/// with -201U3's order number, listed first, does not take -201U3, whose own entry carries its ID; a module whose
/// designation names nothing in the project is parked, not paired by its slot.
#[test]
fn a_weaker_key_never_takes_the_object_of_a_later_entry() {
    let mut s = golden();
    let exported = aml(&mut s);
    let order = image(&exported).stations[0].items[0]
        .order_number
        .clone()
        .expect("-201U3's order number");
    let first = format!("<InternalElement ID=\"{}\"", device(&s, "-201U3"));
    let module = |id: &str, iec: &str, slot: u32| {
        format!(
            "<InternalElement ID=\"{id}\" Name=\"DQ {slot}\">{iec}\
             <Attribute Name=\"PositionNumber\"><Value>{slot}</Value></Attribute>\
             <Attribute Name=\"TypeIdentifier\"><Value>OrderNumber:{order}</Value></Attribute>\
             <Attribute Name=\"Address\"><Attribute Name=\"1\">\
             <Attribute Name=\"StartAddress\"><Value>9</Value></Attribute>\
             <Attribute Name=\"Length\"><Value>8</Value></Attribute>\
             <Attribute Name=\"IoType\"><Value>Output</Value></Attribute></Attribute></Attribute>\
             <SupportedRoleClass RefRoleClassPath=\"AutomationProjectConfigurationRoleClassLib/DeviceItem\"/>\
             </InternalElement>"
        )
    };
    let inserted = format!(
        "{}{}{first}",
        module("tia-new", "", 1),
        module(
            "tia-k99",
            "<Attribute Name=\"ProductDesignation IEC\"><Value>-K99</Value></Attribute>",
            2
        )
    );
    let theirs = exported.replacen(&first, &inserted, 1);
    let back = import(&s, theirs.as_bytes());
    assert!(back.plan.changes.is_empty(), "{:?}", back.plan.changes);
    let parked: BTreeSet<&str> = back.plan.parking.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(parked, ["tia-k99", "tia-new"].into());
}

/// FR-PLC-056, A6: a file whose IDs the PLC software replaced and whose modules carry no IEC designation matches each
/// module by its slot and order number. Once applied, the foreign IDs match the next file, in which a renamed module
/// is a rejected change of the schematic's designation.
#[test]
fn modules_without_ids_or_designations_match_by_slot_and_order_number() {
    let mut s = golden();
    let exported = aml(&mut s);
    let uuid = regex::Regex::new("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
    let foreign = uuid.replace_all(&exported, "tia-$0").into_owned();
    let designation = regex::Regex::new(
        "<Attribute Name=\"ProductDesignation IEC\"[^>]*>\\s*<Value>-20[12]U[2-9]</Value>\\s*</Attribute>",
    )
    .unwrap();
    let bare = designation.replace_all(&foreign, "").into_owned();
    assert_ne!(bare, foreign, "the modules' designations are gone");
    let back = import(&s, bare.as_bytes());
    assert!(back.plan.changes.is_empty(), "{:?}", back.plan.changes);
    assert!(back.plan.parking.is_empty(), "{:?}", back.plan.parking);
    let modules = back
        .plan
        .matched
        .iter()
        .filter(|m| m.object.name() == "module")
        .count();
    assert_eq!(modules, 7);
    bulk(&mut s, &back.apply);

    let renamed = foreign.replace("<Value>-201U4</Value>", "<Value>-201U4X</Value>");
    let next = import(&s, renamed.as_bytes());
    assert!(next.plan.parking.is_empty(), "{:?}", next.plan.parking);
    let name = next
        .plan
        .changes
        .iter()
        .find(|c| c.field == "name" && c.target == device(&s, "-201U4").cast())
        .expect("the renamed designation");
    assert_eq!((name.state, name.acceptable), (State::Rejected, false));
}

/// Doc 10 §3.9, §4.4: an import under the PLC's ownership makes every start of the file a manual start, also one
/// equal to the project's, so that a module inserted in front of the others afterwards moves none of them; the same
/// file again writes nothing.
#[test]
fn an_import_freezes_the_starts_so_an_inserted_module_moves_nothing() {
    let mut s = golden();
    let exported = aml(&mut s);
    let back = import(&s, exported.as_bytes());
    assert!(back.plan.changes.is_empty(), "{:?}", back.plan.changes);
    bulk(&mut s, &back.apply);
    let modules = [
        "-201U3", "-201U4", "-201U5", "-201U6", "-201U7", "-201U8", "-202U2",
    ]
    .map(|t| device(&s, t));
    for m in modules {
        let q = s.query();
        let start = q
            .devices()
            .get(m)
            .unwrap()
            .aspects
            .plc_module
            .clone()
            .unwrap()
            .start
            .output;
        assert!(matches!(start, ew_schema::AddrSpec::Manual(_)), "{start:?}");
    }
    let again = import(&s, exported.as_bytes());
    let preview = s.preview(&again.apply).expect("the preview");
    assert_eq!(preview.op_count(), 0, "the same file again writes nothing");

    let before: Vec<Vec<String>> = modules.iter().map(|m| printed(&s.query(), *m)).collect();
    let copy = s
        .module_parts()
        .into_iter()
        .find(|p| p.text.starts_with("533361"))
        .expect("the module part")
        .copy;
    let station = device(&s, "-201U1");
    run(
        &mut s,
        "plc.add_module",
        json!({"station": station.to_string(),
            "part": {"part": {"copy": copy.to_string()}, "qty": "1"}, "slot": 0}),
    );
    let after: Vec<Vec<String>> = modules.iter().map(|m| printed(&s.query(), *m)).collect();
    assert_eq!(after, before, "the inserted module moves no start");
}

/// Doc 10 §4.4 (review of WP-S3.16): an export records only what its format carries. A PLCopen export, which never
/// comes back, records nothing, and a tag table records no start: a start changed in the project since the AML export
/// stays a change of the project when the PLC software's file comes back.
#[test]
fn an_export_records_only_what_its_format_carries() {
    let mut s = golden();
    let exported = aml(&mut s);
    let channel = channel_of(&s, "TA05_V_15");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "channel", "targets": [channel.to_string()], "path": "symbol", "value": "FEED_15"}),
    );
    let gvl = s
        .plc_export(PlcFormat::Plcopen, &PlcOptions::default())
        .expect("the PLCopen list");
    assert!(gvl.record.is_none(), "a PLCopen export is not recorded");
    let name = import(&s, exported.as_bytes())
        .plan
        .changes
        .into_iter()
        .find(|c| c.field == "name")
        .expect("the renamed symbol");
    assert_eq!((name.state, name.default), (State::Kept, Choice::Ours));

    let u5 = device(&s, "-201U5");
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [u5.to_string()], "dir": "output", "spec": "Q20"}),
    );
    export(&mut s, PlcFormat::TiaXlsx);
    let start = import(&s, exported.as_bytes())
        .plan
        .changes
        .into_iter()
        .find(|c| c.field == "start.output")
        .expect("the moved start");
    assert_eq!(
        (start.state, start.default, start.base.as_deref()),
        (State::Pending, Choice::Ours, Some("2"))
    );
}

/// The rows of a tag workbook, edited by `edit`, as a workbook again (the hidden ID column kept).
fn edited_table(table: &[u8], edit: impl FnOnce(&mut Vec<Vec<String>>)) -> Vec<u8> {
    let limits = ew_import_catalog::table_limits();
    let mut rows = read_rows(table, SourceFormat::Xlsx, ew_plc::tia::SHEET, &limits).unwrap();
    edit(&mut rows);
    let date = ew_reports::xlsx::DateTime {
        year: 2026,
        month: 1,
        day: 2,
        hour: 0,
        minute: 0,
        second: 0,
    };
    ew_reports::xlsx::plain_xlsx(ew_plc::tia::SHEET, &rows, 10, date).unwrap()
}

fn row_of<'a>(rows: &'a mut [Vec<String>], symbol: &str) -> &'a mut Vec<String> {
    rows.iter_mut().find(|r| r[0] == symbol).expect("the row")
}

/// Doc 10 §4.5 (review of WP-S3.16): a comment of several languages is never replaced whole by one value. A table's
/// comment without a language is a conflict that keeps the languages and cannot be taken; a table in a language
/// gives its comment to that language only.
#[test]
fn a_comment_in_one_language_never_replaces_the_others() {
    let mut s = golden();
    let channel = channel_of(&s, "TA05_V_15");
    run(
        &mut s,
        "fields.set",
        json!({
            "kind": "channel",
            "targets": [channel.to_string()],
            "path": "comment",
            "value": {"en-GB": "Feed", "nl-NL": "Toevoer"},
        }),
    );
    let comment = |s: &Session| s.query().table::<Channel>().get(channel).unwrap().comment.clone();
    let table = export(&mut s, PlcFormat::TiaXlsx);
    let neutral = edited_table(&table, |rows| row_of(rows, "TA05_V_15")[4] = "Changed".to_owned());
    let back = import(&s, &neutral);
    let change = back
        .plan
        .changes
        .iter()
        .find(|c| c.field == "comment")
        .expect("the table's comment");
    assert_eq!(
        (change.state, change.default, change.acceptable),
        (State::Conflict, Choice::Ours, false)
    );
    let taken = PlcOptions {
        resolutions: [(change.id.clone(), "theirs".to_owned())].into(),
        ..PlcOptions::default()
    };
    let e = s
        .preview(&s.plc_import(&neutral, &taken).unwrap().apply)
        .expect_err("a comment without a language over languages")
        .to_string();
    assert!(e.contains("do not replace each other"), "{e}");
    bulk(&mut s, &back.apply);
    assert_eq!(comment(&s).get(&"en-GB".parse().unwrap()), Some("Feed"));

    let german = PlcOptions {
        lang: Some("de-DE".to_owned()),
        ..PlcOptions::default()
    };
    let e = s
        .plc_export(PlcFormat::TiaXlsx, &german)
        .expect("the German table");
    s.execute(e.record.as_ref().unwrap()).unwrap();
    let edited = edited_table(&e.bytes, |rows| {
        row_of(rows, "TA05_V_15")[4] = "Zufuhr".to_owned()
    });
    let applied = s.plc_import(&edited, &german).unwrap().apply;
    bulk(&mut s, &applied);
    let c = comment(&s);
    for (lang, text) in [("en-GB", "Feed"), ("nl-NL", "Toevoer"), ("de-DE", "Zufuhr")] {
        assert_eq!(c.get(&lang.parse().unwrap()), Some(text), "{lang}");
    }
}

/// FR-PLC-036, doc 10 §4.4: a tag the table no longer has is listed as removed only when the table is complete, and
/// it is never deleted.
#[test]
fn a_tag_the_complete_table_lacks_is_removed_and_kept() {
    let mut s = golden();
    let table = export(&mut s, PlcFormat::TiaXlsx);
    let short = edited_table(&table, |rows| rows.retain(|r| r[0] != "TA05_V_15"));
    assert!(import(&s, &short).plan.removed.is_empty());
    let complete = PlcOptions {
        complete: true,
        ..PlcOptions::default()
    };
    let back = s.plc_import(&short, &complete).unwrap();
    let removed: Vec<&str> = back.plan.removed.iter().map(|(_, _, n)| n.as_str()).collect();
    assert_eq!(removed, ["TA05_V_15"]);
    bulk(&mut s, &back.apply);
    channel_of(&s, "TA05_V_15");
}

/// Doc 10 §3.9 (review of WP-S3.16): when symbols are given to the PLC, an import protects them as it protects the
/// starts, until `plc.unlock` lifts the protection.
#[test]
fn symbols_given_to_the_plc_are_protected() {
    let mut s = golden();
    let c = cfg(&s.query());
    run(
        &mut s,
        "fields.set",
        json!({"kind": "plc_config_project", "targets": [c.to_string()], "path": "ownership.symbols", "value": "plc"}),
    );
    let exported = aml(&mut s);
    let applied = import(&s, exported.as_bytes()).apply;
    bulk(&mut s, &applied);
    let channel = channel_of(&s, "TA05_V_15");
    let rename =
        json!({"kind": "channel", "targets": [channel.to_string()], "path": "symbol", "value": "FEED_15"});
    let refusal = refused(&mut s, "fields.set", rename.clone());
    assert!(
        refusal.contains("-201U3: a channel's symbol or comment is owned by configuration project PLC1"),
        "{refusal}"
    );
    run(
        &mut s,
        "plc.unlock",
        json!({"config_project": c.to_string(), "group": "symbols"}),
    );
    run(&mut s, "fields.set", rename);
}

/// Doc 10 §2.2 (review of WP-S3.16): only an import that gives a value of a PLC-owned group starts the protection; a
/// tag table carries symbols, which are shared, so the starts stay ordinary edits.
#[test]
fn a_tag_table_import_protects_no_start() {
    let mut s = golden();
    let table = export(&mut s, PlcFormat::TiaXlsx);
    let applied = import(&s, &table).apply;
    bulk(&mut s, &applied);
    let imported = s
        .query()
        .table::<PlcConfigProject>()
        .get(cfg(&s.query()))
        .unwrap()
        .ownership
        .as_ref()
        .and_then(|o| o.imported);
    assert_eq!(imported, None);
    let u6 = device(&s, "-201U6");
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [u6.to_string()], "dir": "output", "spec": "Q20"}),
    );
}

/// FR-PLC-032: values that only differ in leading or trailing white space, which the readers drop, are no change.
#[test]
fn white_space_around_a_value_is_no_change() {
    let mut s = golden();
    let channel = channel_of(&s, "TA05_V_15");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "channel", "targets": [channel.to_string()], "path": "comment", "value": " Feed "}),
    );
    let stored = s.query().table::<Channel>().get(channel).unwrap().comment.clone();
    assert_eq!(
        stored.as_neutral(),
        Some(" Feed "),
        "the comment keeps its spaces"
    );
    let exported = aml(&mut s);
    let back = import(&s, exported.as_bytes());
    assert!(back.plan.changes.is_empty(), "{:?}", back.plan.changes);
    let table = export(&mut s, PlcFormat::TiaXlsx);
    let back = import(&s, &table);
    assert!(back.plan.changes.is_empty(), "{:?}", back.plan.changes);
}

/// FR-PLC-036: a tag table from a German TIA Portal writes `%A0.1` for `%Q0.1` and `%E` for `%I`; it is no change.
#[test]
fn german_mnemonics_are_the_international_addresses() {
    let mut s = golden();
    let table = export(&mut s, PlcFormat::TiaXlsx);
    let german = edited_table(&table, |rows| {
        for row in rows.iter_mut().skip(1) {
            row[3] = row[3].replace("%Q", "%A");
        }
    });
    let back = import(&s, &german);
    assert!(back.plan.changes.is_empty(), "{:?}", back.plan.changes);
    assert!(back.plan.parking.is_empty(), "{:?}", back.plan.parking);
}

/// Doc 10 §4.4: a file's tag on a channel that has a symbol but no address (so no tag of its own) is compared with
/// that symbol: without a snapshot it is a conflict that keeps it, never a value the file silently replaces.
#[test]
fn a_tag_on_a_channel_without_an_address_meets_its_symbol() {
    let mut s = golden();
    let exported = String::from_utf8(
        s.plc_export(PlcFormat::Aml(ApcVersion::V1_4_0), &PlcOptions::default())
            .expect("the export")
            .bytes,
    )
    .unwrap();
    let channel = channel_of(&s, "TA05_V_15");
    let u3 = device(&s, "-201U3");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "channel", "targets": [channel.to_string()], "path": "symbol", "value": "MINE"}),
    );
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [u3.to_string()], "path": "plc_module.start", "value": {"output": "tbd"}}),
    );
    let back = import(&s, exported.as_bytes());
    let name = back
        .plan
        .changes
        .iter()
        .find(|c| c.field == "name" && c.target == channel.cast())
        .expect("the symbol");
    assert_eq!(
        (name.ours.as_deref(), name.theirs.as_deref()),
        (Some("MINE"), Some("TA05_V_15"))
    );
    assert_eq!((name.state, name.default), (State::Conflict, Choice::Ours));
}

/// Doc 10 §3.9: after an import a bus port added to a function is no edit of an address the PLC set.
#[test]
fn a_bus_port_can_be_added_after_an_import() {
    let mut s = golden();
    let exported = aml(&mut s);
    let applied = import(&s, exported.as_bytes()).apply;
    bulk(&mut s, &applied);
    let q = s.query();
    let function = q
        .idx()
        .functions_of_device(device(&s, "-201U3"))
        .find(|f| q.functions().get(*f).is_some_and(|f| f.bus_port.is_none()))
        .expect("a function of -201U3 without a bus port");
    run(
        &mut s,
        "fields.set",
        json!({
            "kind": "function",
            "targets": [function.to_string()],
            "path": "bus_port",
            "value": {"interface": "X2", "bus": "profinet", "role": "device", "address": "tbd"},
        }),
    );
}

/// Doc 10 §4.2: the foreign ID of an object the file no longer has leaves the snapshot with its record.
#[test]
fn the_foreign_id_of_a_removed_object_goes() {
    let mut s = golden();
    let exported = aml(&mut s);
    let uuid = regex::Regex::new("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
    let foreign = uuid.replace_all(&exported, "tia-$0").into_owned();
    let applied = import(&s, foreign.as_bytes()).apply;
    bulk(&mut s, &applied);
    let channel = channel_of(&s, "TA05_V_15");
    let named = |s: &Session| {
        s.query()
            .table::<ForeignId>()
            .iter()
            .any(|(_, f)| f.target == channel.cast() && f.object.name() == "tag")
    };
    assert!(named(&s), "the tag's foreign ID is kept");
    let interface =
        regex::Regex::new("(?s)<ExternalInterface ID=\"[^\"]*\" Name=\"TA05_V_15\".*?</ExternalInterface>")
            .unwrap();
    let link = regex::Regex::new("<InternalLink [^>]*TA05_V_15\"[^>]*/>").unwrap();
    let without = link
        .replace_all(&interface.replace_all(&foreign, ""), "")
        .into_owned();
    let back = import(&s, without.as_bytes());
    assert!(
        back.plan
            .removed
            .iter()
            .any(|(o, _, n)| o.name() == "tag" && n == "TA05_V_15"),
        "{:?}",
        back.plan.removed
    );
    bulk(&mut s, &back.apply);
    assert!(!named(&s), "the removed tag's foreign ID is gone");
}
