// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The save-and-reload step: the session's model is written to a scratch project directory (the first time
//! with `Session::create`, afterwards the incremental `Session::save` of the changed files), the directory is
//! opened again by a reader (no lock, one generation, the way `ew check` opens a project), and
//!
//! - the reader's open report holds no integrity finding (a saved project passes the checks of loading);
//! - the reloaded model equals the live one record for record (`Model::content_eq`; on a mismatch
//!   `Model::content_diff` names the first difference: a row, an unknown record or library document, or an
//!   index entry);
//! - the derived families agree: a sample of tags, the potential partition and the page order always, every
//!   family at every key (`ew_api::verify::sweep` of the live memoised read against the reader's cold one) on
//!   the deep cadence.
//!
//! At the end of a run, after the closing undo-all, the same round trip runs once more when the run saved
//! at all: the incremental save of the reverted entities (deleted files, restored rows) must give back the
//! initial model.

use std::fmt::Write as _;
use std::path::PathBuf;

use ew_api::{MemoMode, Session, StoreOptions};
use ew_derive::{DeviceTag, PageOrder, PotentialPartition};
use ew_model::{ContentDiff, Model};

/// How much of the derived data a round trip compares.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Depth {
    /// The tags and the potential partition: cheap.
    Sample,
    /// And the title block of every drawn page and the page order they derive from (seconds in a debug build).
    TitleBlock,
    /// Every family at every key.
    Full,
}

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
        // The target directory's scratch space when cargo provides one (an integration test), else the system's
        // (the cargo-fuzz crate, FT-297, which includes this module).
        let base = option_env!("CARGO_TARGET_TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
        let dir = base.join(format!(
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
    pub fn round_trip(&mut self, session: &mut Session, depth: Depth) -> Result<(), String> {
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
        same_model("live", &live, &back)?;
        lap("models compared");
        let r = derived_agree(session, &reader, depth);
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
        same_model("initial", initial, &reader.world().model)
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

/// `Model::content_eq` between `first` (named `name` in the text: the live model, or the initial one at the closing
/// round trip) and the reloaded model; on a mismatch, the first difference `Model::content_diff` names (a row, an
/// unknown record or library document, an index entry), with the row's lines where `model_lines` renders its kind.
pub fn same_model(name: &str, first: &Model, back: &Model) -> Result<(), String> {
    if first.content_eq(back) {
        return Ok(());
    }
    let Some(diff) = first.content_diff(back) else {
        return Err(format!(
            "the models differ (first: {name}, second: reloaded), but Model::content_diff names no difference"
        ));
    };
    let mut text = format!("the models differ (first: {name}, second: reloaded): {diff}");
    if let ContentDiff::Row { key, .. } = &diff {
        for (side, model) in [(name, first), ("reloaded", back)] {
            let line = ew_commands::rows::model_lines(model)
                .map_err(|e| format!("{e:?}"))?
                .into_iter()
                .find_map(|(k, line)| (k == *key).then_some(line));
            if let Some(line) = line {
                let _ = write!(text, "\n {side}: {line}");
            }
        }
    }
    Err(text)
}

/// The derived families of the live session and of its reload agree.
fn derived_agree(live: &Session, back: &Session, depth: Depth) -> Result<(), String> {
    let (ql, qb) = (live.query(), back.query());
    for id in ql.devices().ids() {
        if *ql.get::<DeviceTag>(id) != *qb.get::<DeviceTag>(id) {
            return Err(format!("the tag of device {id} differs after the reload"));
        }
    }
    if *ql.get::<PotentialPartition>(()) != *qb.get::<PotentialPartition>(()) {
        return Err("the potential partition differs after the reload".to_owned());
    }
    if depth != Depth::Sample {
        // The title block of every drawn page (the revision label among its values) and the page order.
        for page in ql.pages().ids() {
            let key = ew_derive::PageRef::Drawn(page);
            if *ql.get::<ew_derive::TitleBlockValues>(key) != *qb.get::<ew_derive::TitleBlockValues>(key) {
                return Err(format!("the title block of page {page} differs after the reload"));
            }
        }
        // The page order derives the generated report pages: seconds in a debug build, so the cheap round
        // trip leaves it (and every other family) to the deep one.
        if *ql.get::<PageOrder>(()) != *qb.get::<PageOrder>(()) {
            return Err("the page order differs after the reload".to_owned());
        }
    }
    if depth == Depth::Full {
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
