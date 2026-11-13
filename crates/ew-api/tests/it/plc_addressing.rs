// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On Golden A (AC-29): the start command re-addresses
//! one module with one recompute of its channel addresses and one undo step; the profile command changes the printed
//! notation and nothing else in the files; EW-PLC-012 reports the 28 coil bindings under the part map H1 and none
//! under H2; the tag form of `plc.bind_channel`.

use crate::golden_common;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{AnyCommand, Origin, Session};
use ew_derive::{ChannelAddr, ChannelAddresses, ModuleOrder, Q};
use ew_ids::Id;
use ew_schema::{Channel, Device, Kind, LibDocument, PlcConfigProject, Record};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e:?}"));
}

fn device(s: &Session, tag: &str) -> Id<Device> {
    Id::from_uuid(s.resolve_reference(tag).unwrap_or_else(|e| panic!("{tag}: {e}")))
}

fn cfg(q: &Q) -> Id<PlcConfigProject> {
    let cfgs: Vec<Id<PlcConfigProject>> = q.table::<PlcConfigProject>().iter().map(|(id, _)| id).collect();
    assert_eq!(cfgs.len(), 1, "Golden A has one configuration project");
    cfgs[0]
}

/// The printed addresses of a module's channels, in number order.
fn texts(q: &Q, module: Id<Device>) -> Vec<String> {
    q.get::<ChannelAddresses>(module)
        .iter()
        .map(|e| match &e.address {
            ChannelAddr::At { text, .. } => text.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

/// Every module's printed addresses, in allocation order.
fn all_texts(q: &Q) -> Vec<(Id<Device>, Vec<String>)> {
    q.get::<ModuleOrder>(cfg(q))
        .iter()
        .map(|m| (m.module, texts(q, m.module)))
        .collect()
}

/// PA-1, PA-6 (D8, D9): `plc.set_start` gives -201U5 the output start Q10; its eight channels print Q10.0 … Q10.7,
/// the other modules keep their addresses, `channel_addresses` runs once (the module's own), and one undo restores
/// every address.
#[test]
fn a_start_re_addresses_one_module_in_one_step() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let u5 = device(&s, "-201U5");
    let before = all_texts(&s.query());
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [u5.to_string()], "dir": "output", "spec": "Q10"}),
    );
    let after = all_texts(&s.query());
    let stats = s.memo().expect("a memoising session").commit_stats();
    assert_eq!(
        stats.family("channel_addresses").re_executions,
        1,
        "only -201U5's channel addresses run again"
    );
    for ((m, b), (m2, a)) in before.iter().zip(&after) {
        assert_eq!(m, m2);
        if *m == u5 {
            let want: Vec<String> = (0..8).map(|n| format!("Q10.{n}")).collect();
            assert_eq!(a, &want);
            assert_ne!(a, b);
        } else {
            assert_eq!(a, b, "another module keeps its addresses");
        }
    }
    s.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(all_texts(&s.query()), before);
}

/// PA-1's refusals: a start of the other direction's area, and a text that is no start.
#[test]
fn a_start_of_the_wrong_area_is_refused() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let u5 = device(&s, "-201U5");
    for (spec, want) in [
        ("I10", "is not an output start"),
        ("Q10.3", "is not a start of profile"),
    ] {
        let err = s
            .execute(&AnyCommand {
                name: "plc.set_start".into(),
                params: json!({"modules": [u5.to_string()], "dir": "output", "spec": spec}),
            })
            .expect_err(spec);
        assert!(format!("{err}").contains(want), "{spec}: {err}");
    }
}

/// PA-3 (D8): `plc.set_profile` to IEC 61131-3 prints `%QX0.1` where TIA printed `Q0.1`; the model changes only in
/// the configuration project's `profile`.
#[test]
fn the_iec_profile_changes_the_notation_only() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let before_world = s.world();
    let before = all_texts(&s.query());
    let c = cfg(&s.query());
    run(
        &mut s,
        "plc.set_profile",
        json!({"config_project": c.to_string(), "profile": "iec-61131-3"}),
    );
    let after = all_texts(&s.query());
    assert!(before.iter().flat_map(|(_, t)| t).any(|t| t == "Q0.1"));
    for ((_, b), (_, a)) in before.iter().zip(&after) {
        for (b, a) in b.iter().zip(a) {
            let want = b
                .strip_prefix('Q')
                .map(|rest| format!("%QX{rest}"))
                .or_else(|| b.strip_prefix('I').map(|rest| format!("%IX{rest}")))
                .unwrap_or_else(|| b.clone());
            assert_eq!(a, &want);
        }
    }
    assert!(after.iter().flat_map(|(_, t)| t).any(|t| t == "%QX0.1"));
    let diff = s.diff_from(&before_world).expect("a diff");
    let changes: Vec<(Kind, Vec<String>)> = diff
        .model
        .changes
        .iter()
        .map(|c| (c.key.0, c.fields.clone()))
        .collect();
    assert_eq!(changes, [(Kind::PlcConfigProject, vec!["profile".to_owned()])]);
    let err = s
        .execute(&AnyCommand {
            name: "plc.set_profile".into(),
            params: json!({"config_project": c.to_string(), "profile": "codesys"}),
        })
        .expect_err("an unknown profile");
    assert!(format!("{err}").contains("iec-61131-3"), "{err}");
}

/// Doc 10 §2.5: under the IEC profile a channel address typed without its size (`%Q20.1`) is the bit
/// `%QX20.1`; a unit address without its size (`%Q20`) is refused with the profile's example.
#[test]
fn the_iec_profile_reads_a_bit_without_its_size() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let c = cfg(&s.query());
    run(
        &mut s,
        "plc.set_profile",
        json!({"config_project": c.to_string(), "profile": "iec-61131-3"}),
    );
    let u5 = device(&s, "-201U5");
    let ch = channel(&s.query(), u5, 1);
    run(
        &mut s,
        "fields.set",
        json!({"kind": "channel", "targets": [ch.to_string()], "path": "address", "value": "%Q20.1"}),
    );
    assert_eq!(texts(&s.query(), u5)[1], "%QX20.1");
    let err = s
        .execute(&AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "channel", "targets": [ch.to_string()], "path": "address", "value": "%Q20"}),
        })
        .expect_err("a unit address without its size");
    assert!(format!("{err}").contains("%QX0.1"), "{err}");
}

/// Golden A's library with the solenoid-position map of part 533361 replaced by `map` (H2 swaps solenoids 14 and 12).
fn lib_with_map(lib: &[LibDocument], map: &Value) -> Vec<LibDocument> {
    lib.iter()
        .map(|d| {
            let LibDocument::Part(p) = d else {
                return d.clone();
            };
            if p.item.part_number.as_str() != "533361" {
                return d.clone();
            }
            let mut v = serde_json::to_value(d).expect("a document");
            v["sections"]["plc"]["positions"]["map"] = map.clone();
            LibDocument::from_value(v).expect("the edited part")
        })
        .collect()
}

fn plc_012(s: &Session) -> Vec<ew_api::checks::Finding> {
    s.check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-PLC-012")
        .collect()
}

/// AC-29 (D10, EF-02): under map H1 (14 → 2p, 12 → 2p + 1) EW-PLC-012 reports the 28 coil bindings on the odd bits,
/// each with the rebind quick fix; under H2 (14 → 2p + 1) it reports none; folio 908 prints the same rows either way.
/// PA-5: a `plc` section of confidence `test` marks the findings.
#[test]
fn ac29_ew_plc_012_follows_the_part_map() {
    let batch = golden_common::batch();
    let s = golden_common::session(&batch);
    let h1 = plc_012(&s);
    assert_eq!(h1.len(), 28, "{h1:#?}");
    assert!(h1.iter().all(|f| f.discriminator == "binding"
        && f.fix.as_ref().is_some_and(|x| x.command == "plc.bind_channel")
        && !f.unverified_data));
    let first = h1
        .iter()
        .find(|f| f.subject == "=P01+J-E-2416-201U5:1")
        .expect("a finding on -201U5:1");
    assert!(
        first.message.contains("assigns to =P01+J-E-2416-201U5:0"),
        "{}",
        first.message
    );

    let session_over = |lib: Vec<LibDocument>| {
        let mut project = None;
        let mut records = Vec::new();
        for r in &batch.records {
            match r {
                Record::Project(p) => project = Some(p.clone()),
                other => records.push(other.clone()),
            }
        }
        Session::from_records(
            project.expect("the project"),
            records,
            lib,
            ew_api::SessionOptions::default(),
        )
        .expect("the edited batch")
    };
    let h2 = json!([{"solenoid": "14", "channel": "2p+1"}, {"solenoid": "12", "channel": "2p"}]);
    let s2 = session_over(lib_with_map(&batch.lib, &h2));
    assert!(plc_012(&s2).is_empty(), "{:#?}", plc_012(&s2));
    assert_eq!(
        all_texts(&s2.query()),
        all_texts(&s.query()),
        "the addresses do not follow the map"
    );

    // PA-5: the same H1 data marked `test`
    let marked: Vec<LibDocument> = batch
        .lib
        .iter()
        .map(|d| match d {
            LibDocument::Part(p) if p.item.part_number.as_str() == "533361" => {
                let mut v = serde_json::to_value(d).expect("a document");
                v["meta"]["confidence"] = json!({"plc": "test"});
                LibDocument::from_value(v).expect("the marked part")
            }
            other => other.clone(),
        })
        .collect();
    let s3 = session_over(marked);
    let f = plc_012(&s3);
    assert_eq!(f.len(), 28);
    assert!(f.iter().all(|f| f.unverified_data));
}

/// The rebind quick fix of a finding commits, and the finding is gone; the binding moves to the map's channel.
#[test]
fn the_rebind_fix_clears_its_finding() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let f = plc_012(&s)
        .into_iter()
        .find(|f| f.subject == "=P01+J-E-2416-201U5:1")
        .expect("the finding");
    let fix = f.fix.expect("a fix");
    run(&mut s, &fix.command, fix.params);
    let after = plc_012(&s);
    assert_eq!(after.len(), 27);
    assert!(after.iter().all(|f| f.subject != "=P01+J-E-2416-201U5:1"));
}

/// The ID of channel `number` of `module`.
fn channel(q: &Q, module: Id<Device>, number: u16) -> Id<Channel> {
    q.get::<ew_derive::ModuleChannelIds>(module)
        .iter()
        .find(|(n, _)| *n == number)
        .map(|(_, id)| *id)
        .expect("the channel")
}

/// A command log names the channel by `<module tag>#<number>` or `<module tag>:<designation>` and the coil by
/// `<device tag>` (or `<tag>:<slot>`) and its CP designation; the replay binds as the ID form does. An unknown or
/// ambiguous name stops the replay and lists candidates.
#[test]
fn bind_channel_takes_tags() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let f = plc_012(&s)
        .into_iter()
        .find(|f| f.subject == "=P01+J-E-2416-201U5:1")
        .expect("the finding");
    let coil = f
        .targets
        .iter()
        .find(|t| t.kind == Kind::Function)
        .expect("the coil function")
        .id;
    let valve = s
        .query()
        .functions()
        .get(coil.cast())
        .and_then(|f| f.device)
        .expect("the valve");
    let valve_tag = s
        .query()
        .get::<ew_derive::DeviceTag>(valve)
        .as_ref()
        .as_ref()
        .map(|t| t.short().to_owned())
        .expect("a tag");
    let u5 = device(&s, "-201U5");

    for (form, channel_text) in [("number", "-201U5#0"), ("designation", "-201U5:0")] {
        let log = format!(
            "{{\"$schema\":\"ew.cmdlog/1\"}}\n{{\"cmd\":\"plc.bind_channel\",\"params\":{{\"channel\":\"{channel_text}\",\
             \"endpoint\":{{\"cp\":{{\"function\":\"{valve_tag}\",\"designation\":\"14\"}}}}}}}}\n"
        );
        golden_common::run_log(&mut s, &log, "ft721").unwrap_or_else(|e| panic!("{form}: {e}"));
        let q = s.query();
        let bound = q
            .table::<Channel>()
            .get(channel(&q, u5, 0))
            .and_then(|c| c.binding.clone());
        assert_eq!(
            bound,
            Some(ew_schema::Endpoint::Cp(coil.cast(), 0)),
            "{form}: channel 0 binds the coil CP 14 (the coil function's CP 0, WP-S1.20 D5)"
        );
        assert!(plc_012(&s).iter().all(|f| f.subject != "=P01+J-E-2416-201U5:1"));
        s.undo(&Origin::CmdLog { file: "ft721".into() }).expect("undo");
    }

    for (channel_text, function_text, want) in [
        ("-201U5#9", valve_tag.as_str(), "has no channel 9"),
        (
            "-201U5:X",
            valve_tag.as_str(),
            "no channel of -201U5 is designated `X`",
        ),
        ("-999U9#0", valve_tag.as_str(), "no device prints this tag"),
        ("-201U5#0", "-999Y9", "no device prints this tag"),
    ] {
        let log = format!(
            "{{\"$schema\":\"ew.cmdlog/1\"}}\n{{\"cmd\":\"plc.bind_channel\",\"params\":{{\"channel\":\"{channel_text}\",\
             \"endpoint\":{{\"cp\":{{\"function\":\"{function_text}\",\"designation\":\"14\"}}}}}}}}\n"
        );
        let err = golden_common::run_log(&mut s, &log, "ft721").expect_err(channel_text);
        assert!(err.contains(want), "{channel_text} {function_text}: {err}");
    }
}

/// FR-PLC-009: under the tag-based `rockwell-logix` profile a channel prints from its station and slot (Golden A has no
/// CPU, so -201U1 is a remote adapter named by its tag), and a slot change re-renders the module's addresses.
#[test]
fn the_rockwell_profile_renders_from_the_station_and_the_slot() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let c = cfg(&s.query());
    run(
        &mut s,
        "plc.set_profile",
        json!({"config_project": c.to_string(), "profile": "rockwell-logix"}),
    );
    let u5 = device(&s, "-201U5");
    assert_eq!(texts(&s.query(), u5)[1], "_201U1:3:O.Data.1");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [u5.to_string()], "path": "plc_module.slot", "value": {"rack": {"rack": 0, "slot": 9}}}),
    );
    let q = s.query();
    assert_eq!(texts(&q, u5)[1], "_201U1:9:O.Data.1");
    assert_eq!(
        texts(&q, device(&s, "-201U6"))[0],
        "_201U1:3:O.Data.0",
        "the next positional module moves up"
    );
    let err = s
        .execute(&AnyCommand {
            name: "plc.set_start".into(),
            params: json!({"modules": [u5.to_string()], "dir": "output", "spec": "Q10"}),
        })
        .expect_err("a tag-based profile reads no start");
    assert!(
        format!("{err}").contains("renders its addresses from the station and the slot"),
        "{err}"
    );
}

/// A Bulk command previewed and committed, as the window commits it.
fn bulk(s: &mut Session, name: &str, params: Value) -> ew_api::Receipt {
    let preview = s
        .preview(&AnyCommand {
            name: name.into(),
            params,
        })
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    s.commit(preview).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

/// The EW-PLC-012 findings of discriminator `manual`.
fn manual_deviations(s: &Session) -> usize {
    plc_012(s).iter().filter(|f| f.discriminator == "manual").count()
}

/// FR-PLC-015: `plc.freeze` makes every derived channel address manual, so a later start change moves no printed
/// address and EW-PLC-012 reports each channel of the moved module as a manual deviation; undo restores `auto`.
#[test]
fn freezing_keeps_every_printed_address() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let before = all_texts(&s.query());
    let c = cfg(&s.query());
    let receipt = bulk(&mut s, "plc.freeze", json!({"config_project": c.to_string()}));
    assert_eq!(receipt.outcome.notices.len(), 1, "{:?}", receipt.outcome.notices);
    assert_eq!(all_texts(&s.query()), before);
    assert_eq!(manual_deviations(&s), 0);
    let u5 = device(&s, "-201U5");
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [u5.to_string()], "dir": "output", "spec": "Q10"}),
    );
    assert_eq!(all_texts(&s.query()), before, "a frozen address does not move");
    assert_eq!(
        manual_deviations(&s),
        8,
        "the eight channels of -201U5 deviate from Q10.x"
    );
    s.undo(&Origin::Gui).expect("undo the start");
    s.undo(&Origin::Gui).expect("undo the freeze");
    let q = s.query();
    assert!(
        q.table::<Channel>()
            .iter()
            .all(|(_, ch)| ch.address == ew_schema::AddrSpec::Auto)
    );
}

/// FR-PLC-014: `plc.readdress` turns -201U5's manual start back to `auto`, its preview lists the eight changed
/// addresses with old and new text, and one undo restores the manual start; `plc.convert_manual_to_auto` drops a
/// manual start equal to the automatic one and keeps (and names) one that differs.
#[test]
fn readdress_and_convert_manual_to_auto() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let before = all_texts(&s.query());
    let (u5, u6) = (device(&s, "-201U5"), device(&s, "-201U6"));
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [u5.to_string()], "dir": "output", "spec": "Q10"}),
    );
    let c = cfg(&s.query());
    let preview = s
        .preview(&AnyCommand {
            name: "plc.readdress".into(),
            params: json!({"config_project": c.to_string()}),
        })
        .expect("a preview");
    let notices = preview.outcome().notices.clone();
    assert_eq!(notices.len(), 8, "{notices:?}");
    assert!(
        notices.contains(&"channel 3 of -201U5: Q10.3 → Q2.3".to_owned()),
        "{notices:?}"
    );
    bulk(&mut s, "plc.readdress", json!({"config_project": c.to_string()}));
    assert_eq!(all_texts(&s.query()), before);
    s.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(texts(&s.query(), u5)[0], "Q10.0");

    // -201U6 keeps Q3 under auto, so a manual Q3 is dropped; -201U5's manual Q10 stays
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [u6.to_string()], "dir": "output", "spec": "Q3"}),
    );
    let receipt = bulk(
        &mut s,
        "plc.convert_manual_to_auto",
        json!({"modules": [u5.to_string(), u6.to_string()]}),
    );
    assert_eq!(
        receipt.outcome.notices,
        ["module -201U5 keeps its manual output start Q10 (automatic: Q2)"]
    );
    let q = s.query();
    let start = |m: Id<Device>| {
        q.devices()
            .get(m)
            .and_then(|d| d.aspects.plc_module.clone())
            .map(|p| p.start.output)
    };
    assert_eq!(start(u6), Some(ew_schema::AddrSpec::Auto));
    assert!(matches!(start(u5), Some(ew_schema::AddrSpec::Manual(_))));
}
