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

/// An end kept on its CP but made another way is re-made: 101.1's To end at -101Q1:1, a ferrule in the release, set
/// bare by hand now.
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
        csv.lines()
            .any(|l| l.starts_with("data,Re-make end,101.1,")
                && l.ends_with("at -101Q1:1: ferrule 1.5 BK → bare")),
        "{csv}"
    );
}
