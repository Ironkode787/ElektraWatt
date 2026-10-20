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
    TestProbe, edit, edit_device, fixture, mem_project, options, par_map, project_files, reopen, root, temps,
    try_reopen,
};
use ew_model::{AnyKey, Model};
use ew_store::ew_format::{FindingKind, Limits, Manifest, Severity, hash_text, sha256};
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
    let (vfs, root) = mem_project(mode);
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
    let mut s = setup(mode);
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

#[test]
fn save_call_trace() {
    // Documents the numbered calls of the fixture save, which the sweeps refer to.
    for mode in MODES {
        let (n, _, _, trace) = dry_run_traced(mode);
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
        let (n, final_files, _, trace) = dry_run_traced(mode);
        let g_files = fixture();
        println!("kill sweep {mode:?}: N = {n} calls");
        let results = par_map((1..=n).collect(), |k| {
            let mut failures: Vec<String> = Vec::new();
            let mut s = setup(mode);
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

#[test]
fn power_loss_at_every_call_never_mixes() {
    power_loss_sweep(false);
}

/// The strict form of the sweep: a project whose generation g is intact must never need the
/// EW-CORE-002 "mixed generation" finding after a power loss, which makes the session read-only and asks
/// the user to repair a project that has nothing wrong with it.
#[test]
fn power_loss_never_needs_the_mixed_generation_finding() {
    power_loss_sweep(true);
}

fn power_loss_sweep(strict: bool) {
    const ORPHAN: &str = "g1 + orphan of a removed file";
    for mode in MODES {
        let (n, _, dry_report, trace) = dry_run_traced(mode);
        let removed: Vec<String> = dry_report.removed.iter().map(|p| p.as_str().to_owned()).collect();
        println!("power-loss sweep {mode:?}: N = {n} calls");
        let cases: Vec<(u64, usize)> = (1..=n + 1)
            .flat_map(|k| [0usize, 1, 2, usize::MAX].map(move |keep| (k, keep)))
            .collect();
        let results = par_map(cases, |(k, keep)| {
            let mut failures: Vec<String> = Vec::new();
            let mut s = setup(mode);
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
            if mixed && !strict {
                println!(
                    "  EW-CORE-002 at {case} (`{}`): {}",
                    trace
                        .get(usize::try_from(k).expect("small") - 1)
                        .map_or("after the save", String::as_str),
                    describe(report).replace('\n', " ")
                );
            }
            if mixed && strict {
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
    let g_files = fixture();
    for mode in MODES {
        let (n, final_files, _, trace) = dry_run_traced(mode);
        let cases: Vec<(std::io::ErrorKind, u64)> = KINDS
            .iter()
            .flat_map(|&kind| (1..=n).map(move |k| (kind, k)))
            .collect();
        let results = par_map(cases, |(kind, k)| {
            let mut failures: Vec<String> = Vec::new();
            let mut s = setup(mode);
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

#[test]
fn vfs_trait_is_object_safe_for_the_harness() {
    // The harness hands the store an `Arc<dyn Vfs>`; keep that coupling visible.
    let vfs: Arc<dyn Vfs> = Arc::new(MemVfs::default());
    assert!(vfs.stat(Path::new("/nothing")).is_err());
}
