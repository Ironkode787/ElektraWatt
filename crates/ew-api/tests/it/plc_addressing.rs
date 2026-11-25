// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On the sample: the start command re-addresses one module with one recompute of its channel addresses and
//! one undo step; the profile command changes the printed notation and nothing else in the files; EW-PLC-012 reports
//! none of the valve terminal's four coil bindings under its part's map and all four under the swapped map; the tag
//! form of `plc.bind_channel`. The modules: the CPU -104A1's input card -104A1-A1 and output card -104A1-A2, and the
//! valve terminal -201K1 with its eight outputs.

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{AnyCommand, Origin, Session};
use ew_derive::{ChannelAddr, ChannelAddresses, ModuleOrder, Q};
use ew_ids::Id;
use ew_schema::{Channel, Device, Kind, LibDocument, PlcConfigProject};
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

/// Replays command-log text as origin `cmdlog:ft721`; the number of lines that committed.
fn run_log(s: &mut Session, text: &str) -> Result<usize, String> {
    let log = ew_api::CmdLog::parse(text).map_err(|e| e.to_string())?;
    s.run_script(&log, "ft721")
        .map(|r| r.len())
        .map_err(|e| e.to_string())
}

fn cfg(q: &Q) -> Id<PlcConfigProject> {
    let cfgs: Vec<Id<PlcConfigProject>> = q.table::<PlcConfigProject>().iter().map(|(id, _)| id).collect();
    assert_eq!(cfgs.len(), 1, "the sample has one configuration project");
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

/// `plc.set_start` gives -104A1-A2 the output start Q10; its eight channels print Q10.0 …
/// Q10.7, the other modules keep their addresses, `channel_addresses` runs once (the module's own), and one undo
/// restores every address.
#[test]
fn a_start_re_addresses_one_module_in_one_step() {
    let mut s = crate::common::sample();
    let u5 = device(&s, "-104A1-A2");
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
        "only -104A1-A2's channel addresses run again"
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

/// The refusals: a start of the other direction's area, and a text that is no start.
#[test]
fn a_start_of_the_wrong_area_is_refused() {
    let mut s = crate::common::sample();
    let u5 = device(&s, "-104A1-A2");
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

/// `plc.set_profile` to IEC 61131-3 prints `%QX0.1` where TIA printed `Q0.1`; the model changes only in
/// the configuration project's `profile`.
#[test]
fn the_iec_profile_changes_the_notation_only() {
    let mut s = crate::common::sample();
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
    let mut s = crate::common::sample();
    let c = cfg(&s.query());
    run(
        &mut s,
        "plc.set_profile",
        json!({"config_project": c.to_string(), "profile": "iec-61131-3"}),
    );
    let u5 = device(&s, "-104A1-A2");
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

/// The part of the sample's valve terminal.
const VALVE_TERMINAL: &str = "EW-GEN-VTM-1";

/// The valve terminal's solenoid-position map with solenoids 14 and 12 swapped: 14 on the odd channel of its position,
/// where the sample binds every valve's coil 14 to the even one.
fn swapped() -> Value {
    json!([{"solenoid": "14", "channel": "2p+1"}, {"solenoid": "12", "channel": "2p"}])
}

/// The sample's library with the solenoid-position map of the valve terminal's part replaced by `map`.
fn lib_with_map(lib: &[LibDocument], map: &Value) -> Vec<LibDocument> {
    lib.iter()
        .map(|d| {
            let LibDocument::Part(p) = d else {
                return d.clone();
            };
            if p.item.part_number.as_str() != VALVE_TERMINAL {
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

/// The sample over `lib`.
fn sample_over(lib: impl FnOnce(&[LibDocument]) -> Vec<LibDocument>) -> Session {
    let (project, records, sample_lib) = crate::common::sample_records();
    crate::common::session_of(project, records, lib(&sample_lib))
}

/// The sample with the valve terminal's map swapped.
fn swapped_sample() -> Session {
    sample_over(|lib| lib_with_map(lib, &swapped()))
}

/// The subject's channel of an EW-PLC-012 finding: what follows the last `-201K1:`.
fn on_channel(f: &ew_api::checks::Finding, number: u16) -> bool {
    f.subject_english().ends_with(&format!("-201K1:{number}"))
}

/// Under the part's own map (14 → 2p, 12 → 2p + 1) EW-PLC-012 reports none of the valve
/// terminal's coil bindings; under the swapped map (14 → 2p + 1) it reports the four, each with the rebind quick fix;
/// folio 908 prints the same rows either way. A `plc` section of confidence `test` marks the findings.
#[test]
fn ac29_ew_plc_012_follows_the_part_map() {
    let s = crate::common::sample();
    assert!(plc_012(&s).is_empty(), "{:#?}", plc_012(&s));
    let s2 = swapped_sample();
    let h2 = plc_012(&s2);
    assert_eq!(h2.len(), 4, "the four valves: {h2:#?}");
    assert!(h2.iter().all(|f| f.discriminator == "binding"
        && f.fix.as_ref().is_some_and(|x| x.command == "plc.bind_channel")
        && !f.unverified_data));
    let first = h2
        .iter()
        .find(|f| on_channel(f, 0))
        .expect("a finding on -201K1:0");
    assert!(first.message().contains("-201K1:1"), "{}", first.message());
    assert_eq!(
        all_texts(&s2.query()),
        all_texts(&s.query()),
        "the addresses do not follow the map"
    );

    // The same swapped data marked `test`
    let s3 = sample_over(|lib| {
        lib_with_map(lib, &swapped())
            .into_iter()
            .map(|d| match &d {
                LibDocument::Part(p) if p.item.part_number.as_str() == VALVE_TERMINAL => {
                    let mut v = serde_json::to_value(&d).expect("a document");
                    v["meta"]["confidence"] = json!({"plc": "test"});
                    LibDocument::from_value(v).expect("the marked part")
                }
                _ => d,
            })
            .collect()
    });
    let f = plc_012(&s3);
    assert_eq!(f.len(), 4);
    assert!(f.iter().all(|f| f.unverified_data));
}

/// The rebind quick fix of a finding commits, and the finding is gone; the binding moves to the map's channel.
#[test]
fn the_rebind_fix_clears_its_finding() {
    let mut s = swapped_sample();
    let f = plc_012(&s)
        .into_iter()
        .find(|f| on_channel(f, 0))
        .expect("the finding");
    let fix = f.fix.expect("a fix");
    run(&mut s, &fix.command, fix.params);
    let after = plc_012(&s);
    assert_eq!(after.len(), 3);
    assert!(after.iter().all(|f| !on_channel(f, 0)));
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
    let mut s = swapped_sample();
    let f = plc_012(&s)
        .into_iter()
        .find(|f| on_channel(f, 0))
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
    let u5 = device(&s, "-201K1");

    for (form, channel_text) in [("number", "-201K1#1"), ("designation", "-201K1:1")] {
        let log = format!(
            "{{\"$schema\":\"ew.cmdlog/1\"}}\n{{\"cmd\":\"plc.bind_channel\",\"params\":{{\"channel\":\"{channel_text}\",\
             \"endpoint\":{{\"cp\":{{\"function\":\"{valve_tag}\",\"designation\":\"14\"}}}}}}}}\n"
        );
        run_log(&mut s, &log).unwrap_or_else(|e| panic!("{form}: {e}"));
        let q = s.query();
        let bound = q
            .table::<Channel>()
            .get(channel(&q, u5, 1))
            .and_then(|c| c.binding.clone());
        assert_eq!(
            bound,
            Some(ew_schema::Endpoint::Cp(coil.cast(), 0)),
            "{form}: channel 1 binds the coil CP 14 (the coil function's CP 0)"
        );
        assert!(plc_012(&s).iter().all(|f| !on_channel(f, 0)));
        s.undo(&Origin::CmdLog { file: "ft721".into() }).expect("undo");
    }

    for (channel_text, function_text, want) in [
        ("-201K1#9", valve_tag.as_str(), "has no channel 9"),
        (
            "-201K1:X",
            valve_tag.as_str(),
            "no channel of -201K1 is designated `X`",
        ),
        ("-999U9#0", valve_tag.as_str(), "no device prints this tag"),
        ("-201K1#1", "-999Y9", "no device prints this tag"),
    ] {
        let log = format!(
            "{{\"$schema\":\"ew.cmdlog/1\"}}\n{{\"cmd\":\"plc.bind_channel\",\"params\":{{\"channel\":\"{channel_text}\",\
             \"endpoint\":{{\"cp\":{{\"function\":\"{function_text}\",\"designation\":\"14\"}}}}}}}}\n"
        );
        let err = run_log(&mut s, &log).expect_err(channel_text);
        assert!(err.contains(want), "{channel_text} {function_text}: {err}");
    }
}

/// FR-PLC-009: under the tag-based `rockwell-logix` profile a channel prints from its station and slot, and a slot
/// change re-renders the module's addresses: the input card moved to slot 9, the output card after it moves up.
#[test]
fn the_rockwell_profile_renders_from_the_station_and_the_slot() {
    let mut s = crate::common::sample();
    let c = cfg(&s.query());
    run(
        &mut s,
        "plc.set_profile",
        json!({"config_project": c.to_string(), "profile": "rockwell-logix"}),
    );
    let u5 = device(&s, "-104A1-A1");
    assert_eq!(texts(&s.query(), u5)[1], "Local:1:I.Data.1");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [u5.to_string()], "path": "plc_module.slot", "value": {"rack": {"rack": 0, "slot": 9}}}),
    );
    let q = s.query();
    assert_eq!(texts(&q, u5)[1], "Local:9:I.Data.1");
    assert_eq!(
        texts(&q, device(&s, "-104A1-A2"))[0],
        "Local:1:O.Data.0",
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

/// The findings of `rule` in a release check.
fn findings(s: &Session, rule: &str) -> Vec<ew_api::checks::Finding> {
    s.check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect()
}

/// The printed addresses of a module's channels whose address is `manual`, by channel number.
fn manual_texts(q: &Q, module: Id<Device>) -> Vec<(u16, String)> {
    q.get::<ChannelAddresses>(module)
        .iter()
        .filter(|e| {
            q.table::<Channel>()
                .get(e.channel)
                .is_some_and(|c| matches!(c.address, ew_schema::AddrSpec::Manual(_)))
        })
        .map(|e| match &e.address {
            ChannelAddr::At { text, .. } => (e.number, text.clone()),
            other => (e.number, format!("{other:?}")),
        })
        .collect()
}

/// FR-PLC-015, doc 10 §3.9: `plc.freeze` makes the derived address of every used or reserved channel manual and pins
/// every module start, so nothing prints differently and no spare channel gets EW-PLC-009's "manual address but no
/// connection". A later start change moves no frozen address; each frozen channel of the moved module then lies outside
/// its module's range, which EW-PLC-002 reports once per channel and EW-PLC-012 leaves to it (one defect, one finding),
/// while its spare channels follow the start. Undo restores `auto`.
#[test]
fn freezing_keeps_every_printed_address() {
    let mut s = crate::common::sample();
    let before = all_texts(&s.query());
    let c = cfg(&s.query());
    let receipt = bulk(&mut s, "plc.freeze", json!({"config_project": c.to_string()}));
    assert_eq!(receipt.outcome.notices.len(), 1, "{:?}", receipt.outcome.notices);
    assert_eq!(all_texts(&s.query()), before);
    assert_eq!(manual_deviations(&s), 0);
    let quiet = s.check(&PRE_DELIVERY, CheckMode::Online).findings;
    assert!(
        !quiet.iter().any(|f| f.rule == "EW-PLC-009"),
        "a spare channel stays quiet"
    );
    // An output module with used and spare channels.
    let (module, frozen) = {
        let q = s.query();
        q.get::<ModuleOrder>(c)
            .iter()
            .map(|m| (m.module, manual_texts(&q, m.module)))
            .find(|(m, f)| !f.is_empty() && f.len() < texts(&q, *m).len() && f[0].1.starts_with('Q'))
            .expect("a module with used and spare channels")
    };
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [module.to_string()], "dir": "output", "spec": "Q10"}),
    );
    assert_eq!(
        manual_texts(&s.query(), module),
        frozen,
        "a frozen address does not move"
    );
    assert_eq!(
        findings(&s, "EW-PLC-002").len(),
        frozen.len(),
        "the frozen channels of the module lie outside Q10"
    );
    assert_eq!(manual_deviations(&s), 0, "EW-PLC-002's, not also EW-PLC-012's");
    s.undo(&Origin::Gui).expect("undo the start");
    s.undo(&Origin::Gui).expect("undo the freeze");
    let q = s.query();
    assert!(
        q.table::<Channel>()
            .iter()
            .all(|(_, ch)| ch.address == ew_schema::AddrSpec::Auto)
    );
}

/// FR-PLC-014: `plc.readdress` turns -104A1-A2's manual start back to `auto`, its preview lists the eight changed
/// addresses with old and new text, and one undo restores the manual start; `plc.convert_manual_to_auto` drops a
/// manual start equal to the automatic one (the input card's I0) and keeps (and names) one that differs.
#[test]
fn readdress_and_convert_manual_to_auto() {
    let mut s = crate::common::sample();
    let before = all_texts(&s.query());
    let (u5, u6) = (device(&s, "-104A1-A2"), device(&s, "-104A1-A1"));
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
    let notices: Vec<String> = preview
        .outcome()
        .notices
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(notices.len(), 8, "{notices:?}");
    assert!(
        notices.contains(&"channel 3 of -104A1-A2: Q10.3 → Q1.3".to_owned()),
        "{notices:?}"
    );
    bulk(&mut s, "plc.readdress", json!({"config_project": c.to_string()}));
    assert_eq!(all_texts(&s.query()), before);
    s.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(texts(&s.query(), u5)[0], "Q10.0");

    // -104A1-A1 keeps I0 under auto, so a manual I0 is dropped; -104A1-A2's manual Q10 stays
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [u6.to_string()], "dir": "input", "spec": "I0"}),
    );
    let receipt = bulk(
        &mut s,
        "plc.convert_manual_to_auto",
        json!({"modules": [u5.to_string(), u6.to_string()]}),
    );
    assert_eq!(
        receipt.outcome.notices,
        ["module -104A1-A2 keeps its manual output start Q10 (automatic: Q1)"]
    );
    let q = s.query();
    let start = |m: Id<Device>| {
        q.devices()
            .get(m)
            .and_then(|d| d.aspects.plc_module.clone())
            .map(|p| p.start)
    };
    assert_eq!(start(u6).map(|s| s.input), Some(ew_schema::AddrSpec::Auto));
    assert!(matches!(
        start(u5).map(|s| s.output),
        Some(ew_schema::AddrSpec::Manual(_))
    ));
}

/// Under the tag-based `rockwell-logix` profile `plc.freeze` is refused in words (its addresses follow the station
/// and the slot); a manual address the profile inherits prints as the channel's own place, so a slot change that moves
/// the module's start neither moves it elsewhere nor makes it vanish.
#[test]
fn a_tag_based_profile_freezes_nothing_and_prints_an_inherited_manual_address_in_place() {
    let mut s = crate::common::sample();
    let c = cfg(&s.query());
    let u6 = device(&s, "-104A1-A2");
    let ch1 = channel(&s.query(), u6, 1);
    run(
        &mut s,
        "fields.set",
        json!({"kind": "channel", "targets": [ch1.to_string()], "path": "address", "value": "Q1.1"}),
    );
    run(
        &mut s,
        "plc.set_profile",
        json!({"config_project": c.to_string(), "profile": "rockwell-logix"}),
    );
    let Err(err) = s.preview(&AnyCommand {
        name: "plc.freeze".into(),
        params: json!({"config_project": c.to_string()}),
    }) else {
        panic!("a tag-based profile has nothing to freeze");
    };
    assert!(format!("{err}").contains("there is none to freeze"), "{err}");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [u6.to_string()], "path": "plc_module.slot", "value": {"rack": {"rack": 0, "slot": 9}}}),
    );
    assert_eq!(texts(&s.query(), u6)[1], "Local:9:O.Data.1");
}

/// `plc.convert_manual_to_auto` changes nothing that prints: the valve terminal -201K1's manual start Q1 keeps its own
/// place as automatic (-104A1-A2's manual Q0 holds Q0), but an output card added after them would move from Q2 to Q3,
/// so the start stays manual and the notice says why.
#[test]
fn converting_a_start_that_would_move_another_module_keeps_it() {
    let mut s = crate::common::sample();
    let (u3, u4) = (device(&s, "-201K1"), device(&s, "-104A1-A2"));
    let part = s
        .query()
        .devices()
        .get(u4)
        .map(|d| serde_json::to_value(&d.parts[0]).expect("a part use"))
        .expect("-104A1-A2");
    let station = device(&s, "-104A1");
    run(
        &mut s,
        "plc.add_module",
        json!({"station": station.to_string(), "part": part}),
    );
    for (m, spec) in [(u3, "Q1"), (u4, "Q0")] {
        run(
            &mut s,
            "plc.set_start",
            json!({"modules": [m.to_string()], "dir": "output", "spec": spec}),
        );
    }
    let before = all_texts(&s.query());
    let receipt = bulk(
        &mut s,
        "plc.convert_manual_to_auto",
        json!({"modules": [u3.to_string()]}),
    );
    assert_eq!(all_texts(&s.query()), before, "nothing prints differently");
    assert_eq!(
        receipt.outcome.notices,
        [
            "module -201K1 keeps its manual output start Q1: as an automatic start it would move the addresses of other \
          modules"
        ]
    );
}

/// After `plc.freeze` a module added in rack slot 9 (free: the positional cards hold 1 and 2), which A1 allocates
/// before every positional module, is placed around the frozen modules (their starts are frozen too), so the batch
/// commits and no frozen address moves.
#[test]
fn a_module_added_before_frozen_ones_commits_and_moves_nothing() {
    let mut s = crate::common::sample();
    let c = cfg(&s.query());
    bulk(&mut s, "plc.freeze", json!({"config_project": c.to_string()}));
    let before = all_texts(&s.query());
    let part = s
        .query()
        .devices()
        .get(device(&s, "-104A1-A2"))
        .map(|d| serde_json::to_value(&d.parts[0]).expect("a part use"))
        .expect("-104A1-A2");
    let station = device(&s, "-104A1");
    run(
        &mut s,
        "plc.add_module",
        json!({"station": station.to_string(), "part": part, "slot": 9}),
    );
    let after = all_texts(&s.query());
    for (m, texts) in &before {
        let now = after.iter().find(|(x, _)| x == m).map(|(_, t)| t);
        assert_eq!(now, Some(texts), "a frozen module keeps its addresses");
    }
}
