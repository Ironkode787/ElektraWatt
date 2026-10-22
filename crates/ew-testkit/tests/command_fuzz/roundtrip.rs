// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The save-and-reload step: the session's model is written to a scratch project directory (the first time
//! with `Session::create`, afterwards the incremental `Session::save` of the changed files), the directory is
//! opened again by a reader (no lock, one generation, the way `ew check` opens a project), and
//!
//! - the reader's open report holds no integrity finding (a saved project passes the checks of loading);
//! - the reloaded model equals the live one record for record (`Model::content_eq`; on a mismatch the first
//!   differing row is named);
//! - the derived families agree: a sample of tags, the potential partition and the page order always, every
//!   family at every key (`ew_api::verify::sweep` of the live memoised read against the reader's cold one) on
//!   the deep cadence.
//!
//! At the end of a run, after the closing undo-all, the same round trip runs once more when the run saved
//! at all: the incremental save of the reverted entities (deleted files, restored rows) must give back the
//! initial model.

use std::path::PathBuf;

use ew_api::{MemoMode, Session, StoreOptions};
use ew_derive::{DeviceTag, PageOrder, PotentialPartition};
use ew_model::Model;

/// The scratch project of a run.
pub struct Disk {
    dir: PathBuf,
    created: bool,
    /// The number of round trips so far.
    pub trips: usize,
}

impl Disk {
    pub fn new(seed: u64) -> Disk {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "ew-fuzz-{}-{seed}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        Disk {
            dir,
            created: false,
            trips: 0,
        }
    }

    fn options() -> StoreOptions {
        let mut o = StoreOptions::new("ElektraWatt fuzz");
        // No journal: the run is not a crash test.
        o.state_dir = None;
        o
    }

    /// Saves `session` and reloads it: the findings are the error.
    pub fn round_trip(&mut self, session: &mut Session, deep: bool) -> Result<(), String> {
        self.trips += 1;
        let started = std::time::Instant::now();
        let lap = |what: &str| {
            if std::env::var_os("EW_FUZZ_TRACE").is_some() {
                eprintln!("  round trip, {what}: {:?}", started.elapsed());
            }
        };
        if self.created {
            session.save().map_err(|e| format!("save failed: {e:?}"))?;
        } else {
            std::fs::create_dir_all(&self.dir).map_err(|e| format!("scratch directory: {e}"))?;
            session
                .create(&self.dir, Disk::options())
                .map_err(|e| format!("create failed: {e:?}"))?;
            self.created = true;
        }
        lap("saved");
        let (reader, report) =
            Session::open_reader_with(&self.dir, "ElektraWatt fuzz", "check", MemoMode::Off)
                .map_err(|e| format!("reload failed: {e:?}"))?;
        lap("reloaded");
        if !report.findings.is_empty() {
            return Err(format!(
                "the saved project opens with {} integrity finding(s), first: {:?}",
                report.findings.len(),
                report.findings.first()
            ));
        }
        let (live, back) = (session.world().model, reader.world().model);
        same_model(&live, &back)?;
        lap("models compared");
        let r = derived_agree(session, &reader, deep);
        lap("derived compared");
        r
    }

    /// The closing round trip: the saved project, reloaded, equals `initial`.
    pub fn closing(&mut self, session: &mut Session, initial: &Model) -> Result<(), String> {
        if !self.created {
            return Ok(());
        }
        self.trips += 1;
        session.save().map_err(|e| format!("save failed: {e:?}"))?;
        let (reader, report) =
            Session::open_reader_with(&self.dir, "ElektraWatt fuzz", "check", MemoMode::Off)
                .map_err(|e| format!("reload failed: {e:?}"))?;
        if !report.findings.is_empty() {
            return Err(format!(
                "the project saved after undo-all opens with {} integrity finding(s), first: {:?}",
                report.findings.len(),
                report.findings.first()
            ));
        }
        same_model(initial, &reader.world().model)
            .map_err(|e| format!("after undo-all and a save, the reloaded model is not the initial one: {e}"))
    }
}

impl Drop for Disk {
    fn drop(&mut self) {
        // A failing run keeps its directory for the diagnosis; the next run of the same target dir does not
        // collide (the name has the process ID and a counter).
        if !std::thread::panicking() {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

/// `Model::content_eq`, and on a mismatch the first differing row.
fn same_model(live: &Model, back: &Model) -> Result<(), String> {
    if live.content_eq(back) {
        return Ok(());
    }
    let (a, b) = (
        ew_commands::rows::model_lines(live).map_err(|e| format!("{e:?}"))?,
        ew_commands::rows::model_lines(back).map_err(|e| format!("{e:?}"))?,
    );
    let (ma, mb): (std::collections::BTreeMap<_, _>, std::collections::BTreeMap<_, _>) =
        (a.into_iter().collect(), b.into_iter().collect());
    for (k, v) in &ma {
        match mb.get(k) {
            None => return Err(format!("the row {k:?} is gone after the reload")),
            Some(w) if w != v => {
                return Err(format!(
                    "the row {k:?} differs after the reload:\n live: {v}\n back: {w}"
                ));
            }
            Some(_) => {}
        }
    }
    for k in mb.keys() {
        if !ma.contains_key(k) {
            return Err(format!("the row {k:?} appears after the reload"));
        }
    }
    Err("the models differ (not in a row: an unknown record or an index entry)".to_owned())
}

/// The derived families of the live session and of its reload agree.
fn derived_agree(live: &Session, back: &Session, deep: bool) -> Result<(), String> {
    let (ql, qb) = (live.query(), back.query());
    for id in ql.devices().ids() {
        if *ql.get::<DeviceTag>(id) != *qb.get::<DeviceTag>(id) {
            return Err(format!("the tag of device {id} differs after the reload"));
        }
    }
    if *ql.get::<PotentialPartition>(()) != *qb.get::<PotentialPartition>(()) {
        return Err("the potential partition differs after the reload".to_owned());
    }
    if deep {
        // The page order derives the generated report pages: seconds in a debug build, so the cheap round
        // trip leaves it (and every other family) to the deep one.
        if *ql.get::<PageOrder>(()) != *qb.get::<PageOrder>(()) {
            return Err("the page order differs after the reload".to_owned());
        }
        let sweep = ew_api::verify::sweep(&ql, Some(&qb));
        if !sweep.differences.is_empty() {
            return Err(format!(
                "{} derived value(s) differ after the reload, first: {}",
                sweep.differences.len(),
                sweep.differences[0]
            ));
        }
    }
    Ok(())
}
