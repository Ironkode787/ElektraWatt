// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fault injection over the save protocol (doc 05 §5.3, §5.6; NFR-REL-01, NFR-REL-03): a process kill and a
//! power loss at every `Vfs` call of a save, one failing call of several kinds, transient rename failures,
//! journal recovery and links in place of files.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use common::{
    TestProbe, edit, edit_device, file_kinds, fixture, mem_project, mem_project_of, options, par_map,
    project_files, reopen, root, small_fixture, temps, try_reopen,
};
use ew_model::{AnyKey, AnyRow, ApplyOptions, Model, Op, Rev};
use ew_store::ew_format::ew_schema::{FnDef, LibCopy, LibDocument, Sha256, SymbolDef};
use ew_store::ew_format::{FindingKind, Limits, Manifest, Severity, hash_text, library_copy_hash, sha256};
use ew_store::journal::{self, Collision, Entry, JournalOp, ReplayError};
use ew_store::mem::{DirSync, Fault, MemVfs};
use ew_store::{
    Access, BASE_MEMBER, Changes, OpenMode, OpenReport, Opened, ReadOnlyReason, SaveError, SaveReport,
    SaveStep, Store, StoreOptions, Vfs,
};

const MODES: [DirSync; 2] = [DirSync::Explicit, DirSync::Implicit];

struct Setup {
    vfs: Arc<MemVfs>,
    store: Store,
    g: Model,
    g1: Model,
    keys: BTreeSet<AnyKey>,
}

/// The golden project opened as pid 1, and the edit batch.
fn setup(mode: DirSync) -> Setup {
    setup_of(mode, fixture())
}

/// The small fixture ([`small_fixture`]: a file of every kind Golden A has) opened as pid 1, and the edit batch:
/// the per-case setup of the kill, power-loss and error sweeps, which open the project once per case (test audit
/// g7). Every call of the save is still swept, in both directory-sync modes.
fn sweep_setup(mode: DirSync) -> Setup {
    setup_of(mode, small_fixture())
}

/// `files` opened as pid 1, and the edit batch.
fn setup_of(mode: DirSync, files: BTreeMap<String, Vec<u8>>) -> Setup {
    let (vfs, root) = mem_project_of(mode, files);
    let Opened { store, model, report } = Store::open(&root, options(&vfs, 1)).expect("the fixture opens");
    assert_eq!(report.access, Access::Edit);
    assert_eq!(report.generation, 1);
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    let (g1, keys) = edit(&model);
    assert!(!g1.content_eq(&model));
    Setup {
        vfs,
        store,
        g: model,
        g1,
        keys,
    }
}

/// A successful save: the number of its `Vfs` calls, the files it leaves and its report.
fn dry_run(mode: DirSync) -> (u64, BTreeMap<String, Vec<u8>>, SaveReport) {
    let (n, files, report, _) = dry_run_traced(mode);
    (n, files, report)
}

/// [`dry_run`] with the call trace (`<number>: <operation> <paths>`).
fn dry_run_traced(mode: DirSync) -> (u64, BTreeMap<String, Vec<u8>>, SaveReport, Vec<String>) {
    dry_run_of(setup(mode))
}

/// [`dry_run_traced`] on the sweeps' fixture ([`sweep_setup`]).
fn sweep_dry_run(mode: DirSync) -> (u64, BTreeMap<String, Vec<u8>>, SaveReport, Vec<String>) {
    dry_run_of(sweep_setup(mode))
}

fn dry_run_of(mut s: Setup) -> (u64, BTreeMap<String, Vec<u8>>, SaveReport, Vec<String>) {
    s.vfs.set_trace(true);
    s.vfs.reset_calls();
    let report = s
        .store
        .save(&s.g1, Changes::Keys(&s.keys))
        .expect("the dry-run save succeeds");
    assert_eq!(report.generation, 2);
    assert!(
        !report.written.is_empty() && !report.removed.is_empty(),
        "{report:?}"
    );
    assert!(
        report.journal_error.is_none() && report.leftovers.is_empty(),
        "{report:?}"
    );
    (
        s.vfs.calls(),
        project_files(&s.vfs, &root()),
        report,
        s.vfs.trace(),
    )
}

/// The sweeps' fixture has a file of every kind Golden A has (and is small; `small_fixture` asserts the same when it
/// builds it), and the save of the edit batch on it makes the same sequence of `Vfs` operations as on Golden A, in
/// both directory-sync modes (the same files written and removed: two device shards and a page written, a page
/// removed): the sweeps over it reach every call that a sweep over Golden A would.
#[test]
fn the_sweep_fixture_has_every_file_kind_and_save_call_of_golden_a() {
    let small = small_fixture();
    assert_eq!(file_kinds(&small), file_kinds(&fixture()));
    assert!(small.len() < fixture().len());
    // `<number>: <operation> <paths>`: the operations, in order.
    let operations = |trace: Vec<String>| -> Vec<String> {
        trace
            .iter()
            .map(|line| line.split_whitespace().nth(1).unwrap_or_default().to_owned())
            .collect()
    };
    for mode in MODES {
        let (n_golden, _, golden_report, golden) = dry_run_traced(mode);
        let (n_small, _, small_report, small) = sweep_dry_run(mode);
        assert_eq!(n_small, n_golden, "{mode:?}: the number of calls");
        assert_eq!(operations(small), operations(golden), "{mode:?}: the operations");
        assert_eq!(
            (small_report.written, small_report.removed),
            (golden_report.written, golden_report.removed),
            "{mode:?}: the files written and removed"
        );
    }
}

#[test]
fn save_call_trace() {
    // Documents the numbered calls of the sweeps' fixture save, which the sweeps refer to.
    for mode in MODES {
        let (n, _, _, trace) = sweep_dry_run(mode);
        println!("save of the edit batch, {mode:?}: {n} calls");
        for line in trace {
            println!("  {line}");
        }
    }
}

fn describe(report: &OpenReport) -> String {
    let findings: Vec<String> = report
        .findings
        .iter()
        .map(|f| {
            format!(
                "{:?}({}) {:?}: {}",
                f.kind,
                f.kind.code().unwrap_or("-"),
                f.file,
                f.message
            )
        })
        .collect();
    format!(
        "generation {}, access {:?}, rolled_forward {:?}, removed {:?}, findings {findings:#?}",
        report.generation, report.access, report.rolled_forward, report.removed
    )
}

fn worst(report: &OpenReport) -> Option<Severity> {
    report.findings.iter().map(|f| f.kind.severity()).max()
}

/// Calls whose failure the save is allowed to swallow: pre-cleaning a stale temp, and the journal's own
/// housekeeping (the journal is crash protection, not project data).
fn best_effort(call: &str) -> bool {
    let (_, rest) = call.split_once(": ").expect("a numbered line");
    let (op, path) = rest.split_once(' ').expect("an operation and a path");
    (op == "remove_file" && path.contains(".tmp-")) || path.starts_with("/state")
}

fn no_temps(vfs: &MemVfs) -> Result<(), String> {
    let files = project_files(vfs, &root());
    let t = temps(&files);
    if t.is_empty() {
        Ok(())
    } else {
        Err(format!("temps left: {t:?}"))
    }
}

fn diff(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) -> String {
    let mut out = Vec::new();
    for (n, v) in a {
        match b.get(n) {
            None => out.push(format!("only in left: {n}")),
            Some(w) if w != v => out.push(format!("differs: {n}")),
            Some(_) => {}
        }
    }
    for n in b.keys().filter(|n| !a.contains_key(*n)) {
        out.push(format!("only in right: {n}"));
    }
    format!("{out:?}")
}

#[test]
fn kill_at_every_call_yields_g_or_g1() {
    for mode in MODES {
        let (n, final_files, _, trace) = sweep_dry_run(mode);
        let g_files = small_fixture();
        println!("kill sweep {mode:?}: N = {n} calls");
        let results = par_map((1..=n).collect(), |k| {
            let mut failures: Vec<String> = Vec::new();
            let mut s = sweep_setup(mode);
            s.vfs.reset_calls();
            s.vfs.set_fault(Fault::Kill { at: k });
            let saved = s.store.save(&s.g1, Changes::Keys(&s.keys));
            s.store.crash();
            // After the manifest rename only best-effort steps remain (removals, the journal): a save that
            // returns Ok under a kill must then say so in its report, unless the killed call is one the
            // protocol does not care about.
            let call = &trace[usize::try_from(k).expect("small") - 1];
            if let Ok(r) = &saved {
                if r.leftovers.is_empty() && r.journal_error.is_none() && !best_effort(call) {
                    failures.push(format!(
                        "{mode:?} k={k}: Ok under a kill of `{call}`, with a clean report: {r:?}"
                    ));
                    return (failures, 0);
                }
            }
            s.vfs.restart();
            let mut opened = match try_reopen(&s.vfs, 2) {
                Ok(o) => o,
                Err(e) => {
                    failures.push(format!("{mode:?} k={k} (`{call}`): {e}"));
                    return (failures, 0);
                }
            };
            let report = &opened.report;
            let which = match report.generation {
                1 if opened.model.content_eq(&s.g) => 1,
                2 if opened.model.content_eq(&s.g1) => 2,
                gen_ => {
                    failures.push(format!(
                        "{mode:?} k={k} (`{call}`): generation {gen_} with a model of neither: {}",
                        describe(report)
                    ));
                    return (failures, 0);
                }
            };
            if worst(report) > Some(Severity::Info) {
                failures.push(format!(
                    "{mode:?} k={k} (`{call}`): findings above Info: {}",
                    describe(report)
                ));
            }
            if let Err(e) = no_temps(&s.vfs) {
                failures.push(format!("{mode:?} k={k} (`{call}`, gen {which}): {e}"));
            }
            let files = project_files(&s.vfs, &root());
            let expect = if which == 1 { &g_files } else { &final_files };
            if &files != expect {
                failures.push(format!(
                    "{mode:?} k={k} (`{call}`, gen {which}): files differ from the {} files: {}",
                    if which == 1 { "g" } else { "g1" },
                    diff(&files, expect)
                ));
            }
            if which == 1 {
                match opened.store.save(&s.g1, Changes::Keys(&s.keys)) {
                    Ok(r) if r.generation == 2 => {
                        if project_files(&s.vfs, &root()) != final_files {
                            failures.push(format!(
                                "{mode:?} k={k}: second save leaves other files than the dry run"
                            ));
                        }
                    }
                    other => failures.push(format!("{mode:?} k={k}: second save of g1 failed: {other:?}")),
                }
            }
            (failures, which)
        });
        let at_g = results.iter().filter(|r| r.1 == 1).count();
        let at_g1 = results.iter().filter(|r| r.1 == 2).count();
        println!("kill sweep {mode:?}: {at_g} cases open at g, {at_g1} at g1");
        let failures: Vec<String> = results.into_iter().flat_map(|r| r.0).collect();
        assert!(
            failures.is_empty(),
            "{} failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
        assert!(at_g > 0 && at_g1 > 0, "the sweep must reach both generations");
    }
}

/// A power loss at every call of a save (and after an acknowledged save), keeping 0, 1, 2 or all of the pending
/// writes: the project opens at g or g1, never mixed, and a project whose generation g is intact never needs the
/// EW-CORE-002 "mixed generation" finding, which makes the session read-only and asks the user to repair a project
/// that has nothing wrong with it. (Its lax twin `power_loss_at_every_call_never_mixes`, which only printed the
/// EW-CORE-002 cases, ran the same sweep and was removed by the test audit g7.)
#[test]
fn power_loss_never_needs_the_mixed_generation_finding() {
    const ORPHAN: &str = "g1 + orphan of a removed file";
    for mode in MODES {
        let (n, _, dry_report, trace) = sweep_dry_run(mode);
        let removed: Vec<String> = dry_report.removed.iter().map(|p| p.as_str().to_owned()).collect();
        println!("power-loss sweep {mode:?}: N = {n} calls");
        let cases: Vec<(u64, usize)> = (1..=n + 1)
            .flat_map(|k| [0usize, 1, 2, usize::MAX].map(move |keep| (k, keep)))
            .collect();
        let results = par_map(cases, |(k, keep)| {
            let mut failures: Vec<String> = Vec::new();
            let mut s = sweep_setup(mode);
            s.vfs.reset_calls();
            s.vfs.set_fault(Fault::PowerLoss { at: k, keep });
            let saved = s.store.save(&s.g1, Changes::Keys(&s.keys));
            if saved.is_ok() {
                // The loss hits the next call.
                let _ = s.vfs.stat(Path::new("/p"));
            }
            s.store.crash();
            s.vfs.restart();
            let case = format!("{mode:?} k={k} keep={keep:#x}");
            let opened = match try_reopen(&s.vfs, 2) {
                Ok(o) => o,
                Err(e) => {
                    failures.push(format!("{case}: {e}"));
                    return (failures, "open failed");
                }
            };
            let report = &opened.report;
            let mixed = report
                .findings
                .iter()
                .any(|f| f.kind == FindingKind::MixedGeneration);
            if mixed {
                failures.push(format!(
                    "{case} (`{}`): EW-CORE-002 although every file of generation 1 is intact: {}",
                    trace
                        .get(usize::try_from(k).expect("small") - 1)
                        .map_or("after the save", String::as_str),
                    describe(report).replace('\n', " ")
                ));
            }
            let outcome = if mixed {
                "mixed generation reported (EW-CORE-002)"
            } else if report.generation == 1 && opened.model.content_eq(&s.g) {
                "g"
            } else if report.generation == 2 && opened.model.content_eq(&s.g1) {
                // The removal of a file the new generation dropped may not have reached the disk: the file
                // then comes back as an orphan (EW-CORE-004, a warning), which is the only finding allowed.
                let orphans_of_removed = !report.findings.is_empty()
                    && report.findings.iter().all(|f| {
                        f.kind == FindingKind::Orphan
                            && f.file
                                .as_ref()
                                .is_some_and(|p| removed.iter().any(|r| r == p.as_str()))
                    });
                if orphans_of_removed { ORPHAN } else { "g1" }
            } else {
                failures.push(format!(
                    "{case}: neither g nor g1 and no EW-CORE-002: {}",
                    describe(report)
                ));
                "SILENT MIX"
            };
            if k > n && report.generation != 2 {
                failures.push(format!(
                    "{case}: an acknowledged save (Ok) is lost by a power loss"
                ));
            }
            if !mixed && outcome != ORPHAN && worst(report) > Some(Severity::Info) {
                failures.push(format!(
                    "{case}: outcome {outcome} with findings above Info: {}",
                    describe(report)
                ));
            }
            (failures, outcome)
        });
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for r in &results {
            *counts.entry(r.1).or_default() += 1;
        }
        println!("power-loss sweep {mode:?}: {counts:?}");
        let failures: Vec<String> = results.into_iter().flat_map(|r| r.0).collect();
        assert!(
            failures.is_empty(),
            "{} failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

const KINDS: [std::io::ErrorKind; 3] = [
    std::io::ErrorKind::StorageFull,
    std::io::ErrorKind::PermissionDenied,
    std::io::ErrorKind::ReadOnlyFilesystem,
];

#[test]
fn errors_before_the_commit_point_leave_g_intact() {
    let g_files = small_fixture();
    for mode in MODES {
        let (n, final_files, _, trace) = sweep_dry_run(mode);
        let cases: Vec<(std::io::ErrorKind, u64)> = KINDS
            .iter()
            .flat_map(|&kind| (1..=n).map(move |k| (kind, k)))
            .collect();
        let results = par_map(cases, |(kind, k)| {
            let mut failures: Vec<String> = Vec::new();
            let mut s = sweep_setup(mode);
            s.vfs.reset_calls();
            s.vfs.set_fault(Fault::Error { at: k, kind });
            let case = format!("{mode:?} {kind:?} k={k}");
            let mut leftovers: Vec<String> = Vec::new();
            let saved = s.store.save(&s.g1, Changes::Keys(&s.keys));
            s.vfs.set_fault(Fault::None);
            let tag = match saved {
                Err(SaveError::Failed { step, path, error }) => {
                    let files = project_files(&s.vfs, &root());
                    if files != g_files {
                        failures.push(format!(
                            "{case}: Failed at {step:?} ({}): {error}; disk differs from g: {}",
                            path.display(),
                            diff(&files, &g_files)
                        ));
                        return (failures, "Failed");
                    }
                    match s.store.save(&s.g1, Changes::Keys(&s.keys)) {
                        Ok(r) if r.generation == 2 => {}
                        other => {
                            failures.push(format!("{case}: retry save after Failed at {step:?}: {other:?}"));
                            return (failures, "Failed");
                        }
                    }
                    "Failed"
                }
                Err(SaveError::Incomplete {
                    generation,
                    step,
                    error,
                }) => {
                    if generation != 2 {
                        failures.push(format!("{case}: Incomplete with generation {generation}"));
                    }
                    match s.store.retry_pending() {
                        Ok(Some(2)) => {}
                        other => {
                            failures.push(format!(
                                "{case}: retry_pending after Incomplete at {step:?} ({error}): {other:?}"
                            ));
                            return (failures, "Incomplete");
                        }
                    }
                    "Incomplete"
                }
                Ok(report) => {
                    // Best-effort calls (pre-cleaning a stale temp, the journal's own housekeeping) may fail
                    // without the save noticing; any other failure must show in the report.
                    let call = &trace[usize::try_from(k).expect("small") - 1];
                    if report.journal_error.is_none() && report.leftovers.is_empty() && !best_effort(call) {
                        failures.push(format!(
                            "{case}: Ok although call `{call}` failed, with neither a journal error nor leftovers: {report:?}"
                        ));
                        return (failures, "Ok");
                    }
                    leftovers = report.leftovers.iter().map(|p| p.as_str().to_owned()).collect();
                    "Ok"
                }
                Err(other) => {
                    failures.push(format!("{case}: unexpected {other:?}"));
                    return (failures, "other");
                }
            };
            s.store.close();
            match try_reopen(&s.vfs, 2) {
                Ok(o) if o.report.generation == 2 && o.model.content_eq(&s.g1) => {
                    let mut files = project_files(&s.vfs, &root());
                    // A removal that failed leaves the file behind, reported as an orphan (EW-CORE-004).
                    for l in &leftovers {
                        let orphan = o.report.findings.iter().any(|f| {
                            f.kind == FindingKind::Orphan && f.file.as_ref().is_some_and(|p| p.as_str() == l)
                        });
                        if !orphan {
                            failures.push(format!("{case}: leftover {l} is not reported as an orphan"));
                        }
                        files.remove(l);
                    }
                    if files != final_files {
                        failures.push(format!(
                            "{case}: files after reopen differ from the dry run: {}",
                            diff(&files, &final_files)
                        ));
                    }
                }
                Ok(o) => failures.push(format!("{case}: reopen is not g1: {}", describe(&o.report))),
                Err(e) => failures.push(format!("{case}: reopen: {e}")),
            }
            (failures, tag)
        });
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for r in &results {
            *counts.entry(r.1).or_default() += 1;
        }
        println!("error sweep {mode:?}: N = {n}; {counts:?}");
        let failures: Vec<String> = results.into_iter().flat_map(|r| r.0).collect();
        assert!(
            failures.is_empty(),
            "{} failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn transient_rename_failures_are_retried() {
    for mode in MODES {
        let (_, final_files, report) = dry_run(mode);
        let first = report
            .written
            .first()
            .expect("the save writes files")
            .as_str()
            .to_owned();
        let target = ew_store::join(&root(), &first);

        // Three held renames: the backoff absorbs them.
        let mut s = setup(mode);
        s.vfs.hold(&target, 3);
        let r = s
            .store
            .save(&s.g1, Changes::Keys(&s.keys))
            .expect("three holds are retried");
        assert_eq!(r.generation, 2);
        assert_eq!(project_files(&s.vfs, &root()), final_files);
        s.store.close();
        let o = reopen(&s.vfs, 2);
        assert!(
            o.report.generation == 2 && o.model.content_eq(&s.g1),
            "{}",
            describe(&o.report)
        );

        // Ten: the save is incomplete, and the retry completes it once the holds are used up.
        let mut s = setup(mode);
        s.vfs.hold(&target, 10);
        match s.store.save(&s.g1, Changes::Keys(&s.keys)) {
            Err(SaveError::Incomplete { generation: 2, .. }) => {}
            other => panic!("{mode:?}: expected Incomplete, got {other:?}"),
        }
        assert!(s.store.is_incomplete());
        let mut done = None;
        for _ in 0..4 {
            match s.store.retry_pending() {
                Ok(g) => {
                    done = g;
                    break;
                }
                Err(SaveError::Incomplete { .. }) => {}
                Err(e) => panic!("{mode:?}: retry_pending: {e:?}"),
            }
        }
        assert_eq!(done, Some(2), "{mode:?}: retry_pending completes after the holds");
        assert!(!s.store.is_incomplete());
        assert_eq!(project_files(&s.vfs, &root()), final_files);
        s.store.close();
        let o = reopen(&s.vfs, 2);
        assert!(
            o.report.generation == 2 && o.model.content_eq(&s.g1),
            "{}",
            describe(&o.report)
        );
    }
}

#[test]
fn a_failed_retry_of_an_incomplete_save_is_not_reported_as_this_saves_incomplete() {
    // Review cycle 8, blocker 1. Save 1 is incomplete (a held rename). Save 2 carries new edits and meets the
    // same held rename while completing save 1: it wrote nothing of its own, so it must not answer with save
    // 1's `Incomplete` (on which a caller counts its model as saved and clears its dirty set). Save 3, with
    // the dirty set kept, completes save 1 and writes the edits of save 2; after a clean close (which
    // deletes the journal) a reopen has them.
    for mode in MODES {
        let (_, _, report) = dry_run(mode);
        let first = report
            .written
            .first()
            .expect("the save writes files")
            .as_str()
            .to_owned();
        let held = ew_store::join(&root(), &first);
        let mut s = setup(mode);
        s.vfs.hold(&held, 1000);
        match s.store.save(&s.g1, Changes::Keys(&s.keys)) {
            Err(SaveError::Incomplete { generation: 2, .. }) => {}
            other => panic!("{mode:?}: save 1: expected Incomplete of generation 2, got {other:?}"),
        }
        let (g2, keys2) = edit_device(&s.g1, 1, "second edit");
        let before = project_files(&s.vfs, &root());
        match s.store.save(&g2, Changes::Keys(&keys2)) {
            Err(SaveError::PendingIncomplete { generation: 2, .. }) => {}
            other => panic!("{mode:?}: save 2 must say that save 1 is still incomplete, got {other:?}"),
        }
        assert_eq!(
            project_files(&s.vfs, &root()),
            before,
            "{mode:?}: save 2 wrote nothing"
        );
        assert!(s.store.is_incomplete());
        // The caller keeps the dirty set of save 2.
        s.vfs.hold(&held, 0);
        let r = s
            .store
            .save(&g2, Changes::Keys(&keys2))
            .unwrap_or_else(|e| panic!("{mode:?}: save 3: {e:?}"));
        assert_eq!(r.generation, 3, "{mode:?}");
        assert!(
            !r.written.is_empty(),
            "{mode:?}: save 3 writes the edits of save 2"
        );
        assert!(!s.store.is_incomplete());
        s.store.close();
        let o = reopen(&s.vfs, 2);
        assert_eq!(o.report.generation, 3, "{}", describe(&o.report));
        assert!(
            o.model.content_eq(&g2),
            "{mode:?}: the edits of save 2 are on disk"
        );
        assert!(o.report.recovery.is_none());
        assert!(
            worst(&o.report) <= Some(Severity::Info),
            "{}",
            describe(&o.report)
        );
    }
}

#[test]
fn incomplete_save_rolls_forward_at_the_next_open() {
    // A held rename that never clears; the session dies; the next open finishes the save.
    for mode in MODES {
        let (_, final_files, report) = dry_run(mode);
        let first = report
            .written
            .first()
            .expect("the save writes files")
            .as_str()
            .to_owned();
        let mut s = setup(mode);
        s.vfs.hold(&ew_store::join(&root(), &first), 1000);
        assert!(matches!(
            s.store.save(&s.g1, Changes::Keys(&s.keys)),
            Err(SaveError::Incomplete { generation: 2, .. })
        ));
        s.store.crash();
        s.vfs.hold(&ew_store::join(&root(), &first), 0);
        let o = reopen(&s.vfs, 2);
        assert_eq!(o.report.rolled_forward, Some(2), "{}", describe(&o.report));
        assert!(o.model.content_eq(&s.g1));
        assert!(
            worst(&o.report) <= Some(Severity::Info),
            "{}",
            describe(&o.report)
        );
        assert_eq!(project_files(&s.vfs, &root()), final_files);
    }
}

#[test]
fn stale_lock_of_a_crashed_save_opens_read_only_not_as_an_error() {
    // A crash between the intent and the manifest rename leaves `manifest.json.tmp-2` and a stale lock. A
    // plain open must report the stale lock (so that the caller can offer the takeover), not fail.
    let mode = DirSync::Explicit;
    let mut s = setup(mode);
    let first = {
        let (_, _, report) = dry_run(mode);
        report.written.first().expect("files").as_str().to_owned()
    };
    s.vfs.hold(&ew_store::join(&root(), &first), 1000);
    assert!(matches!(
        s.store.save(&s.g1, Changes::Keys(&s.keys)),
        Err(SaveError::Incomplete { .. })
    ));
    s.store.crash();
    s.vfs.hold(&ew_store::join(&root(), &first), 0);
    assert!(project_files(&s.vfs, &root()).contains_key("manifest.json.tmp-2"));
    let opened = Store::open(&root(), options(&s.vfs, 2));
    match opened {
        Ok(o) => match &o.report.access {
            Access::ReadOnly(ReadOnlyReason::LockedBy(info)) => {
                assert!(info.stale);
                // The abandoned intent is read in memory: generation g1, exactly, and nothing written.
                assert_eq!(o.report.generation, 2);
                assert!(
                    o.model.content_eq(&s.g1),
                    "the read-only view is the intent's generation"
                );
                assert!(
                    o.report.findings.iter().all(|f| f.severity() == Severity::Info),
                    "{:#?}",
                    o.report.findings
                );
                assert!(project_files(&s.vfs, &root()).contains_key("manifest.json.tmp-2"));
            }
            other => panic!("expected a read-only session behind a stale lock, got {other:?}"),
        },
        Err(e) => panic!("a plain open behind a crashed save's stale lock fails: {e:?}"),
    }
}

/// The golden project after save 1 of the edit batch was interrupted past its commit point (a held rename)
/// and the session (pid 1) crashed with its lock in place: `manifest.json.tmp-2` and the temps on disk,
/// the manifest at generation 1. Returns the file system and the models of generations 1 and 2.
struct Crashed {
    vfs: Arc<MemVfs>,
    g: Model,
    g1: Model,
}

fn crashed_incomplete_save(mode: DirSync) -> Crashed {
    let first = {
        let (_, _, report) = dry_run(mode);
        report.written.first().expect("files").as_str().to_owned()
    };
    let mut s = setup(mode);
    s.vfs.hold(&ew_store::join(&root(), &first), 1000);
    assert!(matches!(
        s.store.save(&s.g1, Changes::Keys(&s.keys)),
        Err(SaveError::Incomplete { .. })
    ));
    s.store.crash();
    s.vfs.hold(&ew_store::join(&root(), &first), 0);
    assert!(project_files(&s.vfs, &root()).contains_key("manifest.json.tmp-2"));
    Crashed {
        vfs: s.vfs,
        g: s.g,
        g1: s.g1,
    }
}

#[test]
fn behind_a_lock_that_looks_alive_a_read_only_open_reads_the_intent_after_the_wait() {
    // Review cycle 8, major 4: where the process probe cannot tell that the writer died (Windows), its lock
    // stays non-stale for `lock.stale_after`. Openers then wait the 5 s for the save in progress, and read
    // the intent's generation in memory instead of failing with SaveInProgress. Nothing is written.
    for mode in MODES {
        let s = crashed_incomplete_save(mode);
        let before = project_files(&s.vfs, &root());
        for open_mode in [OpenMode::ReadOnly, OpenMode::Edit] {
            let mut o = options(&s.vfs, 2);
            // pid 1, the crashed writer, still looks alive: the lock is not stale.
            o.probe = Arc::new(TestProbe { alive: 1 });
            o.mode = open_mode;
            let opened = Store::open(&root(), o)
                .unwrap_or_else(|e| panic!("{mode:?} {open_mode:?}: the open fails: {e:?}"));
            match &opened.report.access {
                Access::ReadOnly(ReadOnlyReason::Requested) => assert_eq!(open_mode, OpenMode::ReadOnly),
                Access::ReadOnly(ReadOnlyReason::LockedBy(info)) => {
                    assert_eq!(open_mode, OpenMode::Edit);
                    assert!(!info.stale);
                }
                other => panic!("{mode:?} {open_mode:?}: unexpected access {other:?}"),
            }
            assert_eq!(opened.report.generation, 2, "{}", describe(&opened.report));
            assert!(
                opened.model.content_eq(&s.g1),
                "{mode:?} {open_mode:?}: the view is the intent's generation"
            );
            assert!(
                worst(&opened.report) <= Some(Severity::Info),
                "{}",
                describe(&opened.report)
            );
            drop(opened);
            assert_eq!(project_files(&s.vfs, &root()), before, "nothing written");
        }
    }
}

#[test]
fn a_read_only_open_ignores_an_intent_not_above_the_manifest() {
    // Review cycle 8, 5(a): `manifest.json.tmp-1` beside manifest generation 1 is a leftover, not a save in
    // progress. Before the fix the read-only opener read it in memory (here: an intent that drops a page).
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let limits = ew_store::ew_format::Limits::default();
    let mut stale = Manifest::parse(&files["manifest.json"], &limits).expect("the fixture's manifest");
    let page = stale
        .files
        .keys()
        .find(|p| p.as_str().starts_with("pages/"))
        .expect("a page file")
        .clone();
    stale.files.remove(&page);
    vfs.put(
        &ew_store::join(&root, "manifest.json.tmp-1"),
        stale.to_text().as_bytes(),
    );
    let mut o = options(&vfs, 1);
    o.mode = OpenMode::ReadOnly;
    let opened = Store::open(&root, o).expect("opens");
    assert_eq!(opened.report.generation, 1);
    assert!(opened.report.findings.is_empty(), "{}", describe(&opened.report));
    let fresh = Store::open(
        &root,
        StoreOptions {
            mode: OpenMode::ReadOnly,
            ..options(&vfs, 1)
        },
    )
    .expect("opens");
    assert!(opened.model.content_eq(&fresh.model));
    assert!(project_files(&vfs, &root).contains_key("manifest.json.tmp-1"));
}

/// The fixture's `manifest.json` with a git conflict on its `generation` line (doc 05 §11.4).
fn conflicted_manifest() -> Vec<u8> {
    let text = String::from_utf8(fixture()["manifest.json"].clone()).expect("a manifest is text");
    let conflicted = text.replace(
        "  \"generation\": 1,\n",
        "<<<<<<< ours\n  \"generation\": 1,\n=======\n  \"generation\": 2,\n>>>>>>> theirs\n",
    );
    assert_ne!(conflicted, text, "the generation line is found");
    conflicted.into_bytes()
}

/// `"base": "sha256:<hex>"` of `manifest` as the intent writes it.
fn base_member(manifest: &[u8]) -> String {
    format!("\"{BASE_MEMBER}\": \"{}\"", hash_text(&sha256(manifest)))
}

#[test]
fn a_crash_over_a_conflicted_manifest_rolls_forward() {
    // Review cycle 8, 5(j) rework: a manifest with git conflict markers is a supported state (EW-CORE-001,
    // edit mode). A save over it records the raw hash of those bytes as its base; after a crash past the
    // data renames, the reopen compares raw bytes and rolls forward (it used to refuse: "based on
    // generation 0, but manifest.json is unreadable", leaving the project mixed and read-only for ever).
    // In session, the retry completes the same way.
    for mode in MODES {
        for crash in [true, false] {
            let (vfs, root) = mem_project(mode);
            let conflicted = conflicted_manifest();
            vfs.put(&ew_store::join(&root, "manifest.json"), &conflicted);
            let Opened {
                mut store,
                model,
                report,
            } = Store::open(&root, options(&vfs, 1)).expect("opens");
            assert_eq!(report.access, Access::Edit, "{}", describe(&report));
            assert!(
                report
                    .findings
                    .iter()
                    .any(|f| f.kind == FindingKind::ManifestUnreadable),
                "{}",
                describe(&report)
            );
            let (edited, keys) = edit_device(&model, 0, "edited");
            // The data renames succeed; the manifest rename is held (antivirus): the save is decided.
            let held = ew_store::join(&root, "manifest.json");
            vfs.hold(&held, 1000);
            let generation = match store.save(&edited, Changes::Keys(&keys)) {
                Err(SaveError::Incomplete {
                    generation,
                    step: SaveStep::Manifest,
                    ..
                }) => generation,
                other => panic!("{mode:?}: expected Incomplete at the manifest, got {other:?}"),
            };
            let intent = format!("manifest.json.tmp-{generation}");
            let on_disk = project_files(&vfs, &root);
            let intent_text = String::from_utf8(on_disk[&intent].clone()).expect("text");
            assert!(
                intent_text.contains(&base_member(&conflicted)),
                "the intent records the raw hash of its base: {intent_text}"
            );
            assert_eq!(
                temps(&on_disk),
                [intent.as_str()],
                "{mode:?}: every data file is renamed"
            );
            assert_eq!(on_disk["manifest.json"], conflicted);
            vfs.hold(&held, 0);

            if crash {
                store.crash();
                let o = reopen(&vfs, 2);
                assert_eq!(
                    o.report.rolled_forward,
                    Some(generation),
                    "{}",
                    describe(&o.report)
                );
                assert_eq!(o.report.access, Access::Edit);
                assert!(o.model.content_eq(&edited), "{mode:?}: the save is complete");
                assert!(
                    worst(&o.report) <= Some(Severity::Info),
                    "{}",
                    describe(&o.report)
                );
            } else {
                assert_eq!(
                    store.retry_pending().expect("the retry completes"),
                    Some(generation)
                );
                store.close();
                let o = reopen(&vfs, 2);
                assert!(o.model.content_eq(&edited));
                assert!(
                    worst(&o.report) <= Some(Severity::Info),
                    "{}",
                    describe(&o.report)
                );
            }
            let files = project_files(&vfs, &root);
            assert!(temps(&files).is_empty(), "{:?}", temps(&files));
            let manifest = Manifest::parse(&files["manifest.json"], &Limits::default())
                .expect("the completed manifest parses");
            assert_eq!(manifest.generation, generation);
        }
    }
}

#[test]
fn an_intent_whose_base_bytes_changed_is_not_rolled_forward() {
    // Review cycle 8, 5(j) rework: the roll-forward compares the raw bytes of `manifest.json` with the
    // intent's recorded base. Different bytes (an external change after the save was decided), or no
    // recorded base while a manifest exists, are refused: EW-CORE-002, read-only, nothing touched (the
    // intent and its temps stay). In session the retry is refused the same way until the bytes are back.
    let changed = |original: &[u8]| {
        let text = String::from_utf8(original.to_vec()).expect("text");
        let other = text.replacen("\"generator\": \"", "\"generator\": \"another ", 1);
        assert_ne!(other, text);
        other.into_bytes()
    };
    for mode in MODES {
        for case in ["other manifest bytes", "no recorded base"] {
            let s = crashed_incomplete_save(mode);
            let vfs = &s.vfs;
            let manifest = ew_store::join(&root(), "manifest.json");
            let intent = ew_store::join(&root(), "manifest.json.tmp-2");
            let original = project_files(vfs, &root())["manifest.json"].clone();
            let intent_text =
                String::from_utf8(project_files(vfs, &root())["manifest.json.tmp-2"].clone()).expect("text");
            assert!(intent_text.contains(&base_member(&original)), "{intent_text}");
            if case == "other manifest bytes" {
                vfs.put(&manifest, &changed(&original));
            } else {
                let line = format!("  {},\n", base_member(&original));
                let mut without = intent_text.replace(&line, "");
                if without == intent_text {
                    // The last member: drop the comma of the one before it instead.
                    without = intent_text.replace(&format!(",\n  {}\n", base_member(&original)), "\n");
                }
                assert_ne!(without, intent_text, "the base member is removed");
                vfs.put(&intent, without.as_bytes());
            }
            let before = project_files(vfs, &root());
            let opened = reopen(vfs, 2);
            let report = &opened.report;
            assert_eq!(report.rolled_forward, None, "{case}: {}", describe(report));
            assert!(
                report
                    .findings
                    .iter()
                    .any(|f| f.kind == FindingKind::MixedGeneration
                        && f.file
                            .as_ref()
                            .is_some_and(|p| p.as_str() == "manifest.json.tmp-2")),
                "{case}: {}",
                describe(report)
            );
            assert!(
                matches!(report.access, Access::ReadOnly(ReadOnlyReason::Integrity)),
                "{case}: {:?}",
                report.access
            );
            assert!(
                opened.model.content_eq(&s.g),
                "{mode:?} {case}: generation 1 is loaded"
            );
            drop(opened);
            assert_eq!(
                project_files(vfs, &root()),
                before,
                "{mode:?} {case}: nothing is rolled forward or deleted"
            );
        }

        // In session: the manifest changes under an incomplete save.
        let (_, _, report) = dry_run(mode);
        let first = report.written.first().expect("files").as_str().to_owned();
        let mut s = setup(mode);
        let held = ew_store::join(&root(), &first);
        s.vfs.hold(&held, 1000);
        assert!(matches!(
            s.store.save(&s.g1, Changes::Keys(&s.keys)),
            Err(SaveError::Incomplete { generation: 2, .. })
        ));
        s.vfs.hold(&held, 0);
        let manifest = ew_store::join(&root(), "manifest.json");
        let original = project_files(&s.vfs, &root())["manifest.json"].clone();
        s.vfs.put(&manifest, &changed(&original));
        match s.store.retry_pending() {
            Err(SaveError::Incomplete { generation: 2, .. }) => {}
            other => panic!("{mode:?}: the retry over changed manifest bytes: {other:?}"),
        }
        assert!(s.store.is_incomplete());
        assert!(project_files(&s.vfs, &root()).contains_key("manifest.json.tmp-2"));
        s.vfs.put(&manifest, &original);
        assert_eq!(s.store.retry_pending().expect("the bytes are back"), Some(2));
    }
}

#[test]
fn journal_recovers_unsaved_commits() {
    for mode in MODES {
        // Variant 1: a commit that was never saved.
        let s = setup(mode);
        let Setup {
            vfs,
            mut store,
            g,
            g1,
            keys,
        } = s;
        let keys_vec: Vec<AnyKey> = keys.iter().copied().collect();
        store
            .journal_commit(&g, &g1, &keys_vec, "test.edit", "test")
            .expect("the journal accepts the commit");
        store.crash();
        let mut o = reopen(&vfs, 2);
        let info = o.report.recovery.clone().expect("a leftover journal is reported");
        assert_eq!(info.transactions, 1, "{info:?}");
        assert!(info.base_matches, "{info:?}");
        assert_eq!(o.report.generation, 1);
        let rec = o.store.recover(&o.model).expect("the journal replays");
        assert_eq!(rec.transactions, 1);
        assert!(
            rec.model.content_eq(&g1),
            "{mode:?}: the recovered model is not g1"
        );
        let report = o
            .store
            .save(&rec.model, Changes::Keys(&rec.keys))
            .expect("the recovered model saves");
        assert_eq!(report.generation, 2);
        o.store.close();
        let again = reopen(&vfs, 3);
        assert_eq!(again.report.generation, 2);
        assert!(again.report.recovery.is_none(), "{:?}", again.report.recovery);
        assert!(again.model.content_eq(&g1));
        assert!(
            worst(&again.report) <= Some(Severity::Info),
            "{}",
            describe(&again.report)
        );

        // Variant 2: a saved commit and one more after it: only the one after the save is replayed.
        let s = setup(mode);
        let Setup {
            vfs,
            mut store,
            g,
            g1,
            keys,
        } = s;
        let keys_vec: Vec<AnyKey> = keys.iter().copied().collect();
        store
            .journal_commit(&g, &g1, &keys_vec, "test.edit", "test")
            .expect("journal");
        store.save(&g1, Changes::Keys(&keys)).expect("the save succeeds");
        let (g2, keys2) = edit_device(&g1, 1, "second edit");
        let keys2_vec: Vec<AnyKey> = keys2.iter().copied().collect();
        store
            .journal_commit(&g1, &g2, &keys2_vec, "test.edit2", "test")
            .expect("journal");
        store.crash();
        let mut o = reopen(&vfs, 2);
        assert_eq!(o.report.generation, 2);
        assert!(o.model.content_eq(&g1));
        let info = o
            .report
            .recovery
            .clone()
            .expect("the commit after the save is reported");
        assert_eq!(info.transactions, 1, "{info:?}");
        assert!(info.base_matches, "{info:?}");
        let rec = o.store.recover(&o.model).expect("the journal replays");
        assert_eq!(rec.transactions, 1);
        assert!(
            rec.model.content_eq(&g2),
            "{mode:?}: the recovered model is not g2"
        );
        o.store
            .save(&rec.model, Changes::Keys(&rec.keys))
            .expect("the recovered model saves");
        o.store.close();
        let again = reopen(&vfs, 3);
        assert_eq!(again.report.generation, 3);
        assert!(again.report.recovery.is_none());
        assert!(again.model.content_eq(&g2));
    }
}

#[test]
fn journal_survives_a_power_loss() {
    // Doc 05 §8.1: every journalled commit is synced, so it must survive a power loss, including the
    // directory entries of the journal's own directories.
    for mode in MODES {
        let Setup {
            vfs,
            mut store,
            g,
            g1,
            keys,
        } = setup(mode);
        let keys_vec: Vec<AnyKey> = keys.iter().copied().collect();
        store
            .journal_commit(&g, &g1, &keys_vec, "test.edit", "test")
            .expect("journal");
        vfs.reset_calls();
        vfs.set_fault(Fault::PowerLoss { at: 1, keep: 0 });
        let _ = vfs.stat(Path::new("/p"));
        store.crash();
        vfs.restart();
        let mut o = reopen(&vfs, 2);
        let info = o
            .report
            .recovery
            .clone()
            .unwrap_or_else(|| panic!("{mode:?}: the synced journal is gone after a power loss"));
        assert_eq!(info.transactions, 1);
        let rec = o.store.recover(&o.model).expect("replays");
        assert!(rec.model.content_eq(&g1));
    }
}

#[test]
fn symlinked_page_file_is_not_followed() {
    for mode in MODES {
        let (vfs, root) = mem_project(mode);
        let page = fixture()
            .keys()
            .find(|n| n.starts_with("pages/"))
            .expect("the fixture has page files")
            .clone();
        vfs.put_link(&ew_store::join(&root, &page));
        let o = Store::open(&root, options(&vfs, 1)).expect("a project with a link opens read-only");
        assert!(
            o.report
                .findings
                .iter()
                .any(|f| f.kind == FindingKind::MissingFile
                    && f.file.as_ref().is_some_and(|p| p.as_str() == page)),
            "{}",
            describe(&o.report)
        );
        assert!(
            matches!(o.report.access, Access::ReadOnly(ReadOnlyReason::Integrity)),
            "{:?}",
            o.report.access
        );
        assert_eq!(FindingKind::MissingFile.code(), Some("EW-CORE-003"));
    }
}

// ---------------------------------------------------------------------------------------------------------
// FT-316: the `LibRef` keep-alive and the journal (EW-INV-021, ADR-0009 amendment 7)

/// The library and item of the FnDef copy of the FT-316 tests, and its copy ID `lib_copy_id(LIB_316,
/// ITEM_316)` = `v5(NS_LIBCOPY, "<library>\u{1F}<item>")`; every test checks through the referrer index that
/// a symbol's `LibRef` to the item resolves to it.
const LIB_316: &str = "0190f316-0000-7000-8000-00000000a001";
const ITEM_316: &str = "0190f316-0000-7000-8000-00000000a002";
const FNDEF_316: &str = "4370a04b-c31e-5381-b458-6669a77cc71c";

/// A symbol copy (the model's first, under the ID `0190f316-…-<n>`) whose `fndef` names the FT-316 item, and
/// that item's FnDef copy (the model's first FnDef under the FT-316 lineage); both with their true hash.
fn lib_ref_rows(model: &Model, n: u32) -> (AnyRow, AnyRow) {
    fn id<T: std::str::FromStr>(s: &str) -> T {
        s.parse().unwrap_or_else(|_| panic!("an ID: {s}"))
    }
    let hash = |doc: LibDocument| Sha256(library_copy_hash(&doc).expect("a known kind"));
    let (_, sym) = model
        .table::<LibCopy<SymbolDef>>()
        .iter()
        .next()
        .expect("a symbol copy");
    let mut sym = (**sym).clone();
    sym.id = id(&format!("0190f316-0000-7000-8000-{n:012x}"));
    sym.item.fndef.library = id(LIB_316);
    sym.item.fndef.item = id(ITEM_316);
    sym.sha256 = hash(LibDocument::Symbol(sym.clone()));
    let (_, def) = model
        .table::<LibCopy<FnDef>>()
        .iter()
        .next()
        .expect("an FnDef copy");
    let mut def = (**def).clone();
    (def.id, def.library_id, def.item_id) = (id(FNDEF_316), id(LIB_316), id(ITEM_316));
    def.sha256 = hash(LibDocument::Fndef(def.clone()));
    (AnyRow::of(sym), AnyRow::of(def))
}

/// `m` after the edit `ops` (the edit path, as a command commits).
fn edited(m: &Model, ops: &[Op]) -> Model {
    m.apply_with(ops, Rev(m.rev().0 + 1), &ApplyOptions::default())
        .expect("an edit")
}

/// `m` after the undo `ops` (the history path).
fn undone(m: &Model, ops: &[Op]) -> Model {
    m.apply_history(ops, Rev(m.rev().0 + 1)).expect("an undo")
}

/// The journal entry of the commit `before` → `after` of `keys`.
fn tx_entry(before: &Model, after: &Model, keys: &[AnyKey], cmd: &str) -> Entry {
    let ops = keys
        .iter()
        .filter_map(|k| JournalOp::diff(*k, before.row(k).as_ref(), after.row(k).as_ref()).expect("a diff"))
        .collect();
    Entry::Tx {
        seq: 1,
        t: 0,
        cmd: cmd.into(),
        origin: "test".into(),
        ops,
        assets: Vec::new(),
        damaged_assets: 0,
    }
}

/// Crash recovery of a journal that holds an undone `library.embed`: a symbol naming an FnDef the project
/// does not embed, the embedding of that FnDef, and its undo, which removes a copy the symbol names. The
/// recovered copy state is the state committed before the crash; also when the embedding was saved and only
/// the undo is journalled after the save.
#[test]
fn journal_recovers_an_undone_library_embed() {
    for mode in MODES {
        let Setup {
            vfs, mut store, g, ..
        } = setup(mode);
        let (sym, def) = lib_ref_rows(&g, 1);
        let (skey, fkey) = (sym.key(), def.key());
        let g1 = edited(&g, &[Op::insert(sym)]);
        let g2 = edited(&g1, &[Op::insert(def.clone())]);
        assert_eq!(
            g2.referrers(&fkey).len(),
            1,
            "the symbol's LibRef resolves to the copy"
        );
        let g3 = undone(&g2, &[Op::delete(def.clone())]);
        for (before, after, key, cmd) in [
            (&g, &g1, skey, "library.embed"),
            (&g1, &g2, fkey, "library.embed"),
            (&g2, &g3, fkey, "undo"),
        ] {
            store
                .journal_commit(before, after, &[key], cmd, "test")
                .expect("journal");
        }
        store.crash();
        let mut o = reopen(&vfs, 2);
        let info = o.report.recovery.clone().expect("a leftover journal");
        assert_eq!((info.transactions, info.base_matches), (3, true), "{info:?}");
        let rec = o
            .store
            .recover(&o.model)
            .expect("the journal replays, the undo included");
        assert!(rec.model.content_eq(&g3), "{mode:?}: not the committed state");
        assert!(rec.model.contains(&skey) && !rec.model.contains(&fkey));
        o.store.close();

        // The embedding saved, the undo journalled after the save.
        let Setup {
            vfs, mut store, g, ..
        } = setup(mode);
        let g1 = edited(&g, &[Op::insert(lib_ref_rows(&g, 1).0)]);
        let g2 = edited(&g1, &[Op::insert(def.clone())]);
        let saved: BTreeSet<AnyKey> = [skey, fkey].into();
        store
            .save(&g2, Changes::Keys(&saved))
            .expect("the embedding saves");
        let g3 = undone(&g2, &[Op::delete(def.clone())]);
        store
            .journal_commit(&g2, &g3, &[fkey], "undo", "test")
            .expect("journal");
        store.crash();
        let mut o = reopen(&vfs, 2);
        assert!(o.model.content_eq(&g2), "{}", describe(&o.report));
        let rec = o.store.recover(&o.model).expect("the undo replays");
        assert!(rec.model.content_eq(&g3), "{mode:?}: the undo is not recovered");
    }
}

/// Doc 05 §8.2 with a moved base (`check`): a journalled edit that deletes a symbol and the FnDef copy it named
/// collides with another writer's symbol that names the same copy (EW-INV-021 against the moved base), and
/// nothing is applied. On the base it was journalled on, the same edit replays. A journal that embeds the copy
/// and undoes the embedding is no collision on a moved base: the judgement is net.
#[test]
fn replay_onto_a_moved_base_collides_on_a_removed_copy_that_a_lib_ref_names() {
    let g = setup(DirSync::Explicit).g;
    let (s1, def) = lib_ref_rows(&g, 1);
    let (s2, _) = lib_ref_rows(&g, 2);
    let b0 = edited(&g, &[Op::insert(s1.clone()), Op::insert(def.clone())]);
    let ours = edited(&b0, &[Op::delete(s1.clone()), Op::delete(def.clone())]);
    let deletion = tx_entry(&b0, &ours, &[s1.key(), def.key()], "test.delete");
    let moved = edited(&b0, &[Op::insert(s2.clone())]);
    assert_eq!(moved.referrers(&def.key()).len(), 2, "both symbols name the copy");
    match journal::replay(&moved, &[&deletion], true) {
        Err(ReplayError::Collisions(c)) => assert_eq!(
            c,
            vec![Collision {
                key: def.key(),
                member: "symbol.fndef.item".into()
            }]
        ),
        other => panic!("expected a collision on the copy, got {other:?}"),
    }
    let (back, _) = journal::replay(&b0, &[&deletion], true).expect("no other writer names the copy");
    assert!(back.content_eq(&ours));

    let named = edited(&g, &[Op::insert(s1)]);
    let embedded = edited(&named, &[Op::insert(def.clone())]);
    let reverted = undone(&embedded, &[Op::delete(def.clone())]);
    let txs = [
        tx_entry(&named, &embedded, &[def.key()], "library.embed"),
        tx_entry(&embedded, &reverted, &[def.key()], "undo"),
    ];
    let moved = edited(&named, &[Op::insert(s2)]);
    let (back, _) = journal::replay(&moved, &txs.iter().collect::<Vec<_>>(), true)
        .expect("the journal leaves the copy as the moved base has it");
    assert!(!back.contains(&def.key()));
}

/// A placeholder logo (no company's artwork) for the asset sweeps.
const LOGO: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 20"><rect width="80" height="20"/></svg>"#;

/// The sweeps' small fixture ([`small_fixture`]) opened as pid 1, and `g1`: the project row naming a logo asset
/// that `g1` holds, journalled as committed (FT-337 fix round).
fn logo_setup(mode: DirSync) -> (Arc<MemVfs>, Store, Model, BTreeSet<AnyKey>, ew_model::Asset) {
    use ew_store::ew_format::ew_schema::{AssetRef, COMPANY_MEMBER, sniff};
    let (vfs, root) = mem_project_of(mode, small_fixture());
    let Opened { mut store, model, .. } = Store::open(&root, options(&vfs, 1)).expect("the fixture opens");
    let (format, size) = sniff(LOGO).expect("an SVG");
    let asset = ew_model::Asset::new(
        AssetRef {
            sha256: Sha256(sha256(LOGO)),
            format,
            size,
        },
        LOGO,
    );
    let before = Arc::clone(model.project());
    let mut after = (*before).clone();
    after.extra.insert(
        COMPANY_MEMBER.to_owned(),
        serde_json::json!({ "logo": serde_json::to_value(asset.reference()).unwrap() }),
    );
    let mut g1 = edited(
        &model,
        &[Op::update(
            AnyRow::Project(before),
            AnyRow::Project(Arc::new(after)),
        )],
    );
    g1.put_asset(asset.clone());
    let keys: BTreeSet<AnyKey> = std::iter::once(g1.project().id.cast())
        .map(|id| (ew_store::ew_format::ew_schema::Kind::Project, id))
        .collect();
    let keys_vec: Vec<AnyKey> = keys.iter().copied().collect();
    store
        .journal_commit(&model, &g1, &keys_vec, "project.company.set", "test")
        .expect("the journal accepts the commit");
    (vfs, store, g1, keys, asset)
}

/// Fix round, blocking item 1: a kill at every call of a save that writes an asset, the crash between the asset's
/// rename and the manifest rename included: the project opens at g with the journal restoring the asset's bytes, or
/// at g1 with the asset on disk; never named without its bytes.
#[test]
fn a_kill_at_every_call_of_an_asset_save_keeps_the_assets_bytes() {
    for mode in MODES {
        let (vfs, mut store, g1, keys, _) = logo_setup(mode);
        vfs.set_trace(true);
        vfs.reset_calls();
        store.save(&g1, Changes::Keys(&keys)).expect("the dry-run save");
        let n = vfs.calls();
        let trace = vfs.trace();
        assert!(
            trace.iter().any(|t| t.contains("assets")),
            "the save writes the asset: {trace:?}"
        );
        let failures: Vec<String> = par_map((1..=n).collect(), |k| {
            let (vfs, mut store, g1, keys, asset) = logo_setup(mode);
            vfs.reset_calls();
            vfs.set_fault(Fault::Kill { at: k });
            let _ = store.save(&g1, Changes::Keys(&keys));
            store.crash();
            vfs.restart();
            let mut o = match try_reopen(&vfs, 2) {
                Ok(o) => o,
                Err(e) => return vec![format!("{mode:?} k={k}: {e}")],
            };
            let model = match o.report.generation {
                1 => match o.store.recover(&o.model) {
                    Ok(r) => r.model,
                    Err(e) => return vec![format!("{mode:?} k={k}: at g without a recovery: {e:?}")],
                },
                2 => o.model.clone(),
                g => return vec![format!("{mode:?} k={k}: generation {g}")],
            };
            let mut out = Vec::new();
            if !model.content_eq(&g1) {
                out.push(format!("{mode:?} k={k}: the model is not g1"));
            }
            if model.asset(asset.sha256()) != Some(&asset) {
                out.push(format!(
                    "{mode:?} k={k} (gen {}): the logo is named without its bytes",
                    o.report.generation
                ));
            }
            if o.report.generation == 2
                && project_files(&vfs, &root())
                    .get(&asset.reference().path())
                    .map(Vec::as_slice)
                    != Some(LOGO)
            {
                out.push(format!("{mode:?} k={k}: at g1 without the asset file"));
            }
            out
        })
        .into_iter()
        .flatten()
        .collect();
        assert!(
            failures.is_empty(),
            "{} failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
