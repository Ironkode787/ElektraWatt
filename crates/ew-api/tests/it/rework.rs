// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rework list (doc 13 §3.5) against a released copy of the sample.
//!
//! - `the_rework_list_since_v1_0_lists_the_fitters_work`: a copy of the sample released as V1.0, compared with the
//!   sample changed afterwards (-104E2 removed with its wires, 104.5 moved from the DI module's input 2 to 4, 101.7
//!   made 2.5 mm², 103.2's length 650 mm, a pushbutton -102S3 added with two wires): the rows are
//!   `tests/expected/rework.csv` (`EW_BLESS=1` writes it). 104.8, which takes 104.6's number, is no row.
//! - `the_release_is_found_by_its_revision`: the released copy finds V1.0's archive beside it, and a file of that name
//!   that is another archive is refused in words.

use crate::common;
use crate::release::{NoGit, options, store_options};

use std::path::{Path, PathBuf};

use ew_api::{
    AnyCommand, Approval, Origin, Recovery, ReleaseContext, ReworkBase, ReworkError, Session, SessionOptions,
    Submit,
};
use ew_derive::{FunctionCps, Q};
use ew_ids::{Id, Uuid};
use ew_schema::{Connection, Date, Device, Endpoint, Function, PartyRole, Record, Representation};
use ew_units::{Dec3, Nm};

fn expected() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/rework.csv"))
}

/// A copy of the sample in `base`, its revision V1.0 approved by a checker and released, its archive in the default
/// root beside it; the copy's directory.
fn released_copy(base: &Path) -> PathBuf {
    let (dir, state) = (base.join("sample.ew"), base.join("state"));
    common::copy_dir(Path::new(ew_testdir::SAMPLE), &dir);
    let (mut s, report) = Session::open(
        &dir,
        store_options(&state),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .expect("the copy opens");
    let integrity = ew_api::integrity_items(&report);
    let mut parties = serde_json::to_value(&s.query().project().parties).expect("the parties");
    parties
        .as_array_mut()
        .expect("a list")
        .push(serde_json::json!({"role": "checker", "name": "Test office"}));
    s.execute_with(
        &AnyCommand {
            name: "fields.set".into(),
            params: serde_json::json!({
                "kind": "project", "targets": [s.query().project().id.to_string()], "path": "parties",
                "value": parties,
            }),
        },
        Submit::from(Origin::Gui),
    )
    .expect("a checker");
    let revision = s
        .query()
        .table::<ew_schema::Revision>()
        .ids()
        .next()
        .expect("V1.0");
    s.execute_with(
        &AnyCommand {
            name: "revision.set_author".into(),
            params: serde_json::json!({"revision": revision.to_string(), "role": "author"}),
        },
        Submit::from(Origin::Gui),
    )
    .expect("its author");
    s.save().expect("saved");
    let approval = Approval {
        label: "V1.0".into(),
        checker: PartyRole::Checker,
        date: Date::new(2026, 11, 20).expect("a date"),
        waivers_ack: Vec::new(),
    };
    s.approve(&approval, &integrity).expect("approved");
    s.save().expect("the sign-off saves");
    let context = ReleaseContext {
        label: "V1.0".into(),
        date: Date::new(2026, 11, 20).expect("a date"),
    };
    s.release(&context, &integrity, &options(&NoGit, None))
        .expect("released");
    s.close();
    dir
}

/// The archive a release of V1.0 writes beside the copy `dir`.
fn archive_of(dir: &Path) -> PathBuf {
    dir.with_extension("ew-archive").join("V1.0.ewarchive")
}

/// The device tagged `tag` of the sample.
fn device(s: &Session, tag: &str) -> Id<Device> {
    Id::from_uuid(s.resolve_reference(&format!("tag:{tag}")).expect("a device"))
}

/// The carrier connection of the sample's wire numbered `number`.
fn wire(s: &Session, number: &str) -> Id<Connection> {
    s.navigator_table(ew_api::output::NavKind::Connections)
        .nodes
        .iter()
        .find(|n| n.table_row && n.label == number)
        .map(|n| Id::from_uuid(n.records[0]))
        .unwrap_or_else(|| panic!("no wire {number}"))
}

/// The CP of device `d` whose designation is `designation`, of its first function that has one.
fn cp(q: &Q, d: Id<Device>, designation: &str) -> Endpoint {
    q.idx()
        .functions_of_device(d)
        .find_map(|f| {
            q.get::<FunctionCps>(f)
                .iter()
                .find(|c| c.designation == designation)
                .map(|c| Endpoint::Cp(f, c.idx))
        })
        .unwrap_or_else(|| panic!("no CP {designation}"))
}

/// The sample changed after V1.0 as the scenario says (see the module documentation).
fn changed_sample() -> Session {
    let s = common::sample();
    let q = s.query();
    let lamp = device(&s, "-104E2");
    let (moved, thicker, longer) = (wire(&s, "104.5"), wire(&s, "101.7"), wire(&s, "103.2"));
    let module = device(&s, "-104A1-A1");
    let (input_2, input_4) = (cp(&q, module, "2"), cp(&q, module, "4"));
    // Terminal 3 of -X1021, clamp a (side 1).
    let strip_3 = cp(&q, device(&s, "-X1021:3"), "1");
    let model = device(&s, "-102S1");
    let (project, mut records, lib) = common::sample_records();
    // A pushbutton -102S3, drawn as the pushbutton -102S1 is, three cells to the right, with a wire from -X1021:3 and one to input 2.
    let (s3, s3_function, s3_rep) = (
        Id::<Device>::from_uuid(Uuid::from_u128(0x019b_76da_ffff_7000_8000_0000_0000_0001)),
        Id::<Function>::from_uuid(Uuid::from_u128(0x019b_76da_ffff_7000_8000_0000_0000_0002)),
        Id::<Representation>::from_uuid(Uuid::from_u128(0x019b_76da_ffff_7000_8000_0000_0000_0003)),
    );
    let mut added = Vec::new();
    let mut model_function = None;
    for r in &records {
        match r {
            Record::Device(d) if d.id == model => {
                let mut d = d.clone();
                d.id = s3;
                d.tag.counter = Some(3);
                added.push(Record::Device(d));
            }
            Record::Function(f) if f.device == Some(model) && model_function.is_none() => {
                model_function = Some(f.id);
                let mut f = f.clone();
                f.id = s3_function;
                f.device = Some(s3);
                added.push(Record::Function(f));
            }
            _ => {}
        }
    }
    let model_function = model_function.expect("-102S1 function");
    let rep = records
        .iter()
        .find_map(|r| match r {
            Record::Representation(r) if r.functions == [model_function] => Some(r.clone()),
            _ => None,
        })
        .expect("-102S1 symbol");
    let page = rep.page;
    let mut new_rep = rep.clone();
    new_rep.id = s3_rep;
    new_rep.functions = vec![s3_function];
    new_rep.pos.x = Nm(rep.pos.x.0 + 3 * 7_937_500 * 2);
    added.push(Record::Representation(new_rep));
    let pins = q.get::<FunctionCps>(model_function);
    let pin = |designation: &str| {
        pins.iter()
            .find(|c| c.designation == designation)
            .map(|c| Endpoint::Cp(s3_function, c.idx))
            .unwrap_or_else(|| panic!("no pin {designation}"))
    };
    let new_wire = |n: u128, a: Endpoint, b: Endpoint| {
        let mut c: Connection = serde_json::from_value(serde_json::json!({
            "id": Uuid::from_u128(0x019b_76da_ffff_7000_8000_0000_0000_0010 + n).to_string(),
            "a": {"cp": [Uuid::nil().to_string(), 0]}, "b": {"cp": [Uuid::nil().to_string(), 0]},
        }))
        .expect("a connection");
        c.a = a;
        c.b = b;
        c.page = Some(page);
        Record::Connection(c)
    };
    added.push(new_wire(0, strip_3, pin("13")));
    added.push(new_wire(1, pin("14"), input_2.clone()));
    drop(q);
    for r in &mut records {
        if let Record::Connection(c) = r {
            if c.id == moved {
                if c.a == input_2 {
                    c.a = input_4.clone();
                } else {
                    c.b = input_4.clone();
                }
            } else if c.id == thicker {
                c.overrides.csa = Some(ew_schema::Csa::Mm2(Dec3(2_500)));
            } else if c.id == longer {
                c.overrides.length = Some(Nm(650_000_000));
            }
        }
    }
    records.extend(added);
    let mut s = common::session_of(project, records, lib);
    // -104E2 goes with its wires, as Delete takes it (its junction's sequence kept whole).
    let delete = AnyCommand {
        name: "edit.delete".into(),
        params: serde_json::json!({"targets": [{"kind": "device", "id": lamp.to_string()}]}),
    };
    match s.execute(&delete) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&delete).and_then(|p| s.commit(p)),
        other => other,
    }
    .expect("-104E2 is deleted");
    s
}

#[test]
fn the_rework_list_since_v1_0_lists_the_fitters_work() {
    let base = ew_testdir::Tmp::new("ew-rework");
    let dir = released_copy(&base);
    let now = changed_sample();
    let rework = now
        .rework(&ReworkBase::File(archive_of(&dir)))
        .expect("the archive opens");
    let csv = now.rework_csv(&rework);
    if ew_testdir::bless() {
        std::fs::write(expected(), &csv).expect("rework.csv");
    }
    let want = std::fs::read_to_string(expected()).expect("rework.csv (EW_BLESS=1 writes it)");
    assert_eq!(csv, want.replace("\r\n", "\n"));
}

#[test]
fn the_release_is_found_by_its_revision() {
    let base = ew_testdir::Tmp::new("ew-rework-found");
    let dir = released_copy(&base);
    let (s, _) = Session::open_read_only(&dir, "test").expect("the released copy opens");
    let releases = s.rework_releases();
    let labels: Vec<&str> = releases.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, ["V1.0"]);
    let v1 = releases[0].revision;
    assert_eq!(s.release_archive(v1).expect("found"), archive_of(&dir));
    let rework = s.rework(&ReworkBase::Release(v1)).expect("compared");
    assert!(rework.set.rows.is_empty(), "nothing changed since the release");
    // Another file under the archive's name is not the release's archive.
    std::fs::write(archive_of(&dir), b"not the archive").expect("written");
    match s.release_archive(v1) {
        Err(e @ ReworkError::OtherArchive { .. }) => {
            assert!(e.to_string().contains("is not the archive of V1.0"), "{e}");
        }
        other => panic!("{other:?}"),
    }
}

/// The released copy `dir` with its release record naming the archive `name` where it named `was`: `revisions.jsonl`
/// and the manifest's hash of it rewritten, as a project file edited by hand or sent by someone else.
fn naming_archive(dir: &Path, was: &str, name: &str) {
    let path = dir.join("revisions.jsonl");
    let before = std::fs::read(&path).expect("revisions.jsonl");
    let text = String::from_utf8(before.clone()).expect("UTF-8");
    let stored = |n: &str| format!("\"name\":{}", serde_json::to_string(n).expect("a JSON string"));
    assert_eq!(text.matches(&stored(was)).count(), 1, "{text}");
    std::fs::write(&path, text.replace(&stored(was), &stored(name))).expect("revisions.jsonl written");
    let after = std::fs::read(&path).expect("revisions.jsonl");
    let manifest = dir.join("manifest.json");
    let m = std::fs::read_to_string(&manifest).expect("the manifest");
    let (old, new) = (
        ew_format::hash_text(&ew_format::sha256(&before)),
        ew_format::hash_text(&ew_format::sha256(&after)),
    );
    assert_eq!(
        m.matches(&old).count(),
        1,
        "the manifest lists revisions.jsonl once"
    );
    std::fs::write(&manifest, m.replace(&old, &new)).expect("the manifest written");
}

/// The released revision of the copy `dir`, opened read-only, and why its archive is not taken.
fn archive_refusal(dir: &Path) -> (ReworkError, ReworkError, usize) {
    let (s, _) = Session::open_read_only(dir, "test").expect("the copy opens");
    let v1 = s
        .query()
        .table::<ew_schema::Revision>()
        .ids()
        .next()
        .expect("V1.0");
    let refused = s.release_archive(v1).expect_err("not taken");
    (refused, s.why_no_release(), s.rework_releases().len())
}

/// A release record names its archive by a path that leaves the archive folder (`..`, absolute with either
/// separator, a UNC name, a drive): the name is refused in words that name the release and what its archive is called,
/// and nothing is read, although the path leads to the release's own archive, which would be taken if it were read.
#[test]
fn an_archive_name_that_leaves_its_folder_is_refused_and_nothing_is_read() {
    let base = ew_testdir::Tmp::new("ew-rework-name");
    let dir = released_copy(&base);
    let outside = base.join("outside.ewarchive");
    std::fs::copy(archive_of(&dir), &outside).expect("the archive copied outside its folder");
    let absolute = outside.to_string_lossy().into_owned();
    let mut was = "V1.0.ewarchive".to_owned();
    for name in [
        "../outside.ewarchive".to_owned(),
        "..\\outside.ewarchive".to_owned(),
        absolute.replace('\\', "/"),
        absolute.replace('/', "\\"),
        "\\\\host\\share\\outside.ewarchive".to_owned(),
        "C:outside.ewarchive".to_owned(),
        "NUL.ewarchive".to_owned(),
    ] {
        naming_archive(&dir, &was, &name);
        let (refused, why, found) = archive_refusal(&dir);
        match &refused {
            ReworkError::NotAFileName { label } => assert_eq!(label, "V1.0", "{name}"),
            other => panic!("{name}: {other:?}"),
        }
        assert!(
            refused
                .to_string()
                .contains("a release's archive is V1.0.ewarchive"),
            "{refused}"
        );
        assert!(matches!(why, ReworkError::NotAFileName { .. }), "{name}: {why:?}");
        assert_eq!(found, 0, "{name}: no release to compare with");
        was = name;
    }
}

/// A file under the archive's name larger than an archive may be (2 GiB) is refused without being read. A sparse
/// file, so the test writes nothing of that size (Unix only: Windows gives a file of that length its clusters).
#[cfg(unix)]
#[test]
fn an_archive_larger_than_an_archive_may_be_is_not_read() {
    let base = ew_testdir::Tmp::new("ew-rework-large");
    let dir = released_copy(&base);
    let file = std::fs::File::create(archive_of(&dir)).expect("the archive replaced");
    file.set_len(2 * 1024 * 1024 * 1024 + 1).expect("a sparse length");
    drop(file);
    let (refused, _, found) = archive_refusal(&dir);
    assert!(
        matches!(
            refused,
            ReworkError::Open(ew_api::ArchiveOpenError::TooLarge { .. })
        ),
        "{refused:?}"
    );
    assert_eq!(found, 0);
}

/// A release whose archive is not in the folder beside the project is said as what it is, not as no release: which
/// release, the archive's name and where it is missing.
#[test]
fn a_release_whose_archive_is_missing_is_said_so() {
    let base = ew_testdir::Tmp::new("ew-rework-missing");
    let dir = released_copy(&base);
    std::fs::remove_file(archive_of(&dir)).expect("the archive removed");
    let (_, why, found) = archive_refusal(&dir);
    assert_eq!(found, 0);
    match &why {
        ReworkError::NoArchive { label, name, .. } => {
            assert_eq!((label.as_str(), name.as_str()), ("V1.0", "V1.0.ewarchive"));
        }
        other => panic!("{other:?}"),
    }
}

/// An end kept on its CP but made another way is re-made: 101.1's To end at -101Q1:1, a ferrule in the release, set
/// bare by hand now. A person made it by name, so its Because cell is empty.
#[test]
fn an_end_made_another_way_is_re_made() {
    let base = ew_testdir::Tmp::new("ew-rework-remake");
    let dir = released_copy(&base);
    let mut now = common::sample();
    let row = {
        let t = now.navigator_table(ew_api::output::NavKind::Connections);
        let r = t
            .nodes
            .iter()
            .find(|n| n.table_row && n.label == "101.1")
            .expect("101.1");
        r.records.clone()
    };
    let (holder, at_a) = now.wire_end(&row, 1).expect("an end of the project");
    let cmd = AnyCommand {
        name: "fields.set".into(),
        params: serde_json::json!({"kind": "connection", "targets": [holder.to_string()],
            "path": if at_a { "overrides.end_a" } else { "overrides.end_b" }, "value": {"treatment": "bare"}}),
    };
    now.execute(&cmd).expect("an end set by hand");
    let rework = now
        .rework(&ReworkBase::File(archive_of(&dir)))
        .expect("the archive opens");
    let csv = now.rework_csv(&rework);
    assert!(
        csv.lines().any(|l| l.starts_with("data,Re-make end,101.1,")
            && l.ends_with("at -101Q1:1: ferrule 1.5 BK → bare,")),
        "{csv}"
    );
}

/// The sample with a second copy of the strip -X1031 and its terminals under another counter, which no release has; the
/// new strip and its terminals.
fn sample_with_new_strip() -> (Session, Id<Device>, Vec<Id<Device>>) {
    let s = common::sample();
    let strip = device(&s, "-X1031");
    drop(s);
    let (project, mut records, lib) = common::sample_records();
    let fresh = |n: u128| Uuid::from_u128(0x019b_76da_fffe_7000_8000_0000_0000_0000 + n);
    let (new_strip, mut n) = (Id::<Device>::from_uuid(fresh(0)), 1);
    let mut terminals: Vec<(Id<Device>, Id<Device>)> = Vec::new();
    let mut added = Vec::new();
    for r in &records {
        if let Record::Device(d) = r {
            if d.id == strip {
                let mut d = d.clone();
                d.id = new_strip;
                d.tag.counter = Some(1041);
                added.push(Record::Device(d));
            } else if d.aspects.terminal.as_ref().is_some_and(|t| t.strip == strip) {
                let mut d = d.clone();
                let old = d.id;
                d.id = Id::from_uuid(fresh(n));
                n += 1;
                if let Some(t) = d.aspects.terminal.as_mut() {
                    t.strip = new_strip;
                }
                terminals.push((old, d.id));
                added.push(Record::Device(d));
            }
        }
    }
    for r in &records {
        if let Record::Function(f) = r
            && let Some((_, new)) = terminals.iter().find(|(old, _)| f.device == Some(*old))
        {
            let mut f = f.clone();
            f.id = Id::from_uuid(fresh(n));
            n += 1;
            f.device = Some(*new);
            added.push(Record::Function(f));
        }
    }
    records.extend(added);
    (
        common::session_of(project, records, lib),
        new_strip,
        terminals.into_iter().map(|(_, t)| t).collect(),
    )
}

/// Labels for these changes holds a new strip's marker (doc 13 §5.4): the rework list has no row of its own for a
/// strip the release did not have, only one per terminal it adds, and the strip's marker goes with their markers.
#[test]
fn the_labels_for_the_changes_hold_a_new_strips_marker() {
    let base = ew_testdir::Tmp::new("ew-rework-strip");
    let dir = released_copy(&base);
    let (now, strip, terminals) = sample_with_new_strip();
    let tag = ew_derive::short_tag(&now.query(), strip);
    let files = now
        .label_files(
            &ew_api::LabelScope::Changes(ReworkBase::File(archive_of(&dir))),
            Some(ew_api::FileForm::Csv),
            None,
        )
        .expect("the label files");
    let of = |set: ew_derive::LabelSet| files.iter().find(|f| f.set == set);
    let strips = of(ew_derive::LabelSet::StripMarkers).expect("the strip markers");
    assert_eq!(strips.count, 1, "the new strip's marker alone");
    assert!(String::from_utf8_lossy(&strips.bytes).contains(&tag), "{tag}");
    let added = of(ew_derive::LabelSet::TerminalMarkers).expect("the terminal markers");
    assert_eq!(added.count, terminals.len(), "the new terminals' markers");
}

/// A row a person did not make by name says why (doc 13 §3.5): 101.15 made 1.5 mm² on its own connection, as its
/// Size cell would, is a Replace without a reason, and 101.14, its twin ferrule's partner at -101G1:+#1, is re-made
/// because of it. 104.8 has no connection of its own: set on the one it shares with 104.4, the size reaches both, and
/// 104.8's row says so.
#[test]
fn a_consequence_says_why_it_is_there() {
    let base = ew_testdir::Tmp::new("ew-rework-reason");
    let dir = released_copy(&base);
    let sample = common::sample();
    let shared = wire(&sample, "104.8");
    let thicker = sample
        .query()
        .get::<ew_derive::WireRecordSet>(())
        .iter()
        .find(|r| r.number.as_deref() == Some("101.15"))
        .and_then(|r| r.own().first().copied())
        .expect("101.15 has a connection of its own");
    drop(sample);
    let (now, csv) = resized(&dir, thicker);
    drop(now);
    let row = |csv: &str, no: &str| {
        csv.lines()
            .find(|l| l.split(',').nth(2) == Some(no))
            .unwrap_or_else(|| panic!("no row of {no}: {csv}"))
            .to_owned()
    };
    let by_hand = row(&csv, "101.15");
    assert!(
        by_hand.starts_with("data,Replace,101.15,") && by_hand.ends_with(','),
        "{by_hand}"
    );
    let partner = row(&csv, "101.14");
    assert!(
        partner.starts_with("data,Re-make end,101.14,")
            && partner.ends_with(",its twin ferrule's partner 101.15 at -101G1:+#1 changed size"),
        "{partner}"
    );
    let (_, csv) = resized(&dir, shared);
    let follows = row(&csv, "104.8");
    assert!(
        follows.ends_with(",its size is set on a connection it shares with 104.4"),
        "{follows}"
    );
}

/// The sample with the connection `c` made 1.5 mm², and its rework list against the release in `dir` as CSV.
fn resized(dir: &Path, thicker: Id<Connection>) -> (Session, String) {
    let (project, mut records, lib) = common::sample_records();
    for r in &mut records {
        if let Record::Connection(c) = r
            && c.id == thicker
        {
            c.overrides.csa = Some(ew_schema::Csa::Mm2(Dec3(1_500)));
        }
    }
    let now = common::session_of(project, records, lib);
    let rework = now
        .rework(&ReworkBase::File(archive_of(dir)))
        .expect("the archive opens");
    let csv = now.rework_csv(&rework);
    (now, csv)
}

/// Labels for these changes name each label they hold, what it is for and what it says (doc 13 §5.4), so that a person
/// reads them before he prints: the device label of the new pushbutton -102S3 and the wire markers of its new wires.
#[test]
fn the_labels_for_the_changes_are_named() {
    let base = ew_testdir::Tmp::new("ew-rework-named");
    let dir = released_copy(&base);
    let now = changed_sample();
    let files = now
        .label_files(
            &ew_api::LabelScope::Changes(ReworkBase::File(archive_of(&dir))),
            Some(ew_api::FileForm::Csv),
            None,
        )
        .expect("the label files");
    let names = |set: ew_derive::LabelSet| {
        files
            .iter()
            .find(|f| f.set == set)
            .map(|f| f.names(None))
            .unwrap_or_default()
    };
    let devices = names(ew_derive::LabelSet::DeviceLabels);
    assert!(devices.iter().any(|n| n == "-102S3 · -102S3"), "{devices:?}");
    let wires = names(ew_derive::LabelSet::WireMarkers);
    assert!(
        wires.iter().any(|n| n == "102.10 at -X1021:3/a · -102S3:13"),
        "{wires:?}"
    );
}

/// What only a workshop setting changed says that setting by its name (doc 13 §3.5): a routed length made with another
/// Reserve per end says so, not that Route laid its way again, and a wire part the workshop's stock now gives a wire
/// of its size and colour says that.
#[test]
fn a_change_a_workshop_setting_made_names_it() {
    let base = ew_testdir::Tmp::new("ew-rework-setting");
    let dir = released_copy(&base);
    let mut now = common::sample();
    let (number, csa, colour) = now
        .query()
        .get::<ew_derive::WireRecordSet>(())
        .iter()
        .find_map(|r| match (&r.csa, &r.colour, &r.number) {
            (Some(c), Some(k), Some(n)) => match &c.value {
                ew_schema::Csa::Mm2(d) => Some((n.clone(), d.to_string(), serde_json::to_value(&k.value).ok()?)),
                _ => None,
            },
            _ => None,
        })
        .expect("a wire with a size and a colour");
    let (part, doc) = common::wire_part(0x7102, "WIRE-FITS", &csa, &colour);
    let mut stock: Vec<serde_json::Value> = now
        .query()
        .project()
        .settings
        .workshop
        .stock
        .iter()
        .map(|r| serde_json::json!({"copy": r.copy.to_string()}))
        .collect();
    stock.push(serde_json::json!({"copy": part.to_string()}));
    common::run_cmd(
        &mut now,
        "workshop.stock",
        serde_json::json!({"stock": stock, "copies": [doc]}),
    );
    common::run_cmd(
        &mut now,
        "project.settings.set",
        serde_json::json!({"member": "workshop.reserve", "value": 60_000_000}),
    );
    let rework = now
        .rework(&ReworkBase::File(archive_of(&dir)))
        .expect("the archive opens");
    let csv = now.rework_csv(&rework);
    let lengths: Vec<&str> = csv
        .lines()
        .filter(|l| l.starts_with("data,Check length,"))
        .collect();
    assert!(!lengths.is_empty(), "{csv}");
    for row in lengths {
        assert!(row.ends_with(",the workshop's Reserve per end changed"), "{row}");
    }
    let replaced = csv
        .lines()
        .find(|l| l.starts_with(&format!("data,Replace,{number},")))
        .unwrap_or_else(|| panic!("no Replace row of {number}: {csv}"));
    assert!(
        replaced.contains("the workshop's Stock gives its wire part by its size and colour"),
        "{replaced}"
    );
    assert!(!csv.contains("Route laid its way again"), "{csv}");
}
