// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew fmt` and `ew recover`, `ew fmt --repair` and
//! `ew upgrade`: the subcommands that work on the project's files and journals as
//! the store keeps them.

use std::path::Path;

use ew_api::cli_results as results;
use ew_api::fmt::{Backup, DiscardedLine, FmtError, KeptLine, RepairError, Repaired, Rewrite};

use super::{
    FmtArgs, RecoverAction, RecoverArgs, UpgradeArgs, journal_refusal, older_format, open_editing,
    open_editing_to, plural, printable, project_dir, read_only_words, reader_dir, typed_word,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The word that confirms `ew recover --discard`.
const DISCARD_WORD: &str = "discard";

/// The word that confirms `ew upgrade` on a terminal.
const UPGRADE_WORD: &str = "upgrade";

/// `ew fmt [--check]` (D9, FR-EXT-008): the files that are not in canonical form, listed (`--check`, a reader: no
/// lock, nothing written) or rewritten through the save protocol after the content check (`ew_api::fmt`). The lines the
/// open kept verbatim are listed on stderr in both modes.
pub(super) fn run_fmt(a: &FmtArgs) -> i32 {
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
    run_fmt_with(a, ew_api::StoreOptions::new(VERSION), (&mut out, &mut err))
}

/// [`run_fmt`] with the store options (a test's migration chain among them) and the writers of what it prints on a
/// project of an older format injected.
fn run_fmt_with(
    a: &FmtArgs,
    mut store: ew_api::StoreOptions,
    (out, err): (&mut dyn std::io::Write, &mut dyn std::io::Write),
) -> i32 {
    if a.repair {
        store.repair = true;
        return run_repair(a, store, err);
    }
    // `--check` is a reader and takes an `.ewz`; `fmt` writes and refuses one.
    let dir = if a.check {
        reader_dir(a.project.as_ref())
    } else {
        project_dir(a.project.as_ref())
    };
    let Some(dir) = dir else {
        return 2;
    };
    let opened = if a.check {
        // `Session::open_reader`, with the store options given.
        let reader = if ew_api::ewz::is_ewz(&ew_api::OsVfs, &dir) {
            ew_api::Session::open_ewz_with(&dir, store, "fmt", ew_api::MemoMode::default())
        } else {
            store.mode = ew_api::OpenMode::ReadOnly;
            store.state_dir = None;
            let options = ew_api::SessionOptions {
                origin: fmt_origin(),
                ..ew_api::SessionOptions::default()
            };
            ew_api::Session::open(&dir, store, options, ew_api::Recovery::Keep)
        };
        reader.map_err(|e| {
            eprintln!("error: {}: {e}", dir.display());
            2
        })
    } else {
        open_editing(&dir, fmt_origin(), store, ew_api::Recovery::Keep).and_then(|(session, report)| {
            if report.recovery.is_some() {
                // A save would hide the crashed session's unsaved changes (doc 05 section 8.3): every writer's text.
                eprintln!("{}", journal_refusal(&dir));
                session.close();
                return Err(2);
            }
            Ok((session, report))
        })
    };
    let (mut session, report) = match opened {
        Ok(r) => r,
        Err(code) => return code,
    };
    if let Some(from) = &report.migrated_from {
        session.close();
        let (code, text) = fmt_older(a.check, &dir, from);
        let _ = writeln!(err, "{text}");
        if a.check && a.json {
            let result = results::FmtResult {
                schema: results::FMT.to_owned(),
                check: true,
                files: Vec::new(),
                kept: Vec::new(),
                generation: report.generation,
                older_format: Some(from.clone()),
            };
            let _ = write!(out, "{}", results::to_text(&result));
        }
        return code;
    }
    let kept = ew_api::fmt::kept_lines(&report);
    for k in &kept {
        eprintln!("{}", kept_line(k));
    }
    let outcome = if a.check {
        ew_api::fmt::plan(&session).and_then(|plan| {
            // `--check` runs the content check too: it predicts what `ew fmt` does, a refusal included.
            ew_api::fmt::verify_content(&session, &plan)?;
            Ok((plan.files(), report.generation))
        })
    } else {
        ew_api::fmt::format(&mut session).map(|done| (done.files, done.generation))
    };
    // A format that wrote says so when its manifest is marked `unstable`.
    let unstable = !a.check
        && outcome.as_ref().is_ok_and(|(files, _)| !files.is_empty())
        && session.store().is_some_and(|s| s.writes_unstable());
    // A format's session holds no unsaved transaction: the clean close loses nothing.
    session.close();
    let (files, generation) = match outcome {
        Ok(done) => done,
        Err(e) => {
            eprintln!("{}", fmt_refusal(&e));
            return 2;
        }
    };
    for f in &files {
        let line = match (a.check, f.removed) {
            (true, false) => format!("would rewrite {}", f.file),
            (true, true) => format!("would remove {}", f.file),
            (false, false) => format!("rewrote {}", f.file),
            (false, true) => format!("removed {}", f.file),
        };
        if a.json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    }
    eprintln!("{}", fmt_summary(a.check, &files, generation));
    super::warn_unreleased(unstable, err);
    if a.json {
        print!(
            "{}",
            results::to_text(&results::FmtResult {
                schema: results::FMT.to_owned(),
                check: a.check,
                files: files
                    .iter()
                    .map(|f| results::FmtFile {
                        file: f.file.clone(),
                        removed: f.removed,
                    })
                    .collect(),
                kept: kept
                    .iter()
                    .map(|k| results::KeptRow {
                        file: k.file.clone(),
                        line: k.line,
                        code: k.code.clone(),
                        reason: k.reason.clone(),
                    })
                    .collect(),
                generation,
                older_format: None,
            })
        );
    }
    i32::from(a.check && !files.is_empty())
}

/// What `ew fmt` says on a project of an older format, which opens migrated and only `ew upgrade` writes,
/// with its exit code: `--check` a needed change (1), `ew fmt` a refusal (2).
fn fmt_older(check: bool, dir: &Path, from: &str) -> (i32, String) {
    if check {
        (1, format!("fmt: {}", older_format(dir, from)))
    } else {
        (2, format!("error: {}", older_format(dir, from)))
    }
}

/// One line the open kept verbatim, as `ew fmt` lists it on stderr.
fn kept_line(k: &KeptLine) -> String {
    match k.line {
        Some(line) => format!("kept {}:{line} ({}): {}", k.file, k.code, printable(&k.reason)),
        None => format!("kept {} ({}): {}", k.file, k.code, printable(&k.reason)),
    }
}

/// The last line of `ew fmt` on stderr.
fn fmt_summary(check: bool, files: &[Rewrite], generation: u64) -> String {
    match (check, files.len()) {
        (true, 0) => "fmt: every file is in canonical form".to_owned(),
        (true, n) => format!(
            "fmt: {} not in canonical form; nothing was written",
            plural(n, "file is", "files are")
        ),
        (false, 0) => "fmt: every file is in canonical form; nothing was written".to_owned(),
        (false, n) => format!(
            "fmt: rewrote {}; saved generation {generation}",
            plural(n, "file", "files")
        ),
    }
}

/// The origin of `ew fmt`'s sessions.
fn fmt_origin() -> ew_api::Origin {
    ew_api::Origin::Cli {
        subcommand: "fmt".to_owned(),
        confirmed: false,
    }
}

/// `ew fmt --repair`: the refusals in their order (an `.ewz`, a crashed session's
/// journal, any other finding that forces read-only, then the repair's own: a project that cannot be packed, the
/// backup's name or folder, the content check), each before anything is written; then the backup and the one save
/// (`ew_api::fmt::repair`), and the project opened again as an editor opens it: exit 0 when it opens editable
/// with no EW-CORE-005 finding, else 2, the new generation kept. The discarded lines and the backup go to stdout (stderr
/// with `--json`); the kept lines and the summary to stderr.
fn run_repair(a: &FmtArgs, store: ew_api::StoreOptions, err: &mut dyn std::io::Write) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let (mut session, report) = match open_editing(&dir, fmt_origin(), store, ew_api::Recovery::Keep) {
        Ok(r) => r,
        Err(code) => return code,
    };
    if report.recovery.is_some() {
        eprintln!("{}", journal_refusal(&dir));
        session.close();
        return 2;
    }
    if let Some(from) = &report.migrated_from {
        let _ = writeln!(err, "error: {}", older_format(&dir, from));
        session.close();
        return 2;
    }
    // `open_editing` lets through only a repair open that integrity findings other than broken lines made read-only.
    if report.access != ew_api::Access::Edit {
        eprintln!("{}", beyond_repair_refusal(&dir, &report));
        session.close();
        return 2;
    }
    let kept = ew_api::fmt::kept_after_repair(&report);
    for k in &kept {
        eprintln!("{}", kept_line(k));
    }
    let outcome = ew_api::fmt::repair(&mut session);
    // A repair that wrote says so when its manifest is marked `unstable`.
    let unstable = outcome.as_ref().is_ok_and(|r| r.backup.is_some())
        && session.store().is_some_and(|s| s.writes_unstable());
    // A repair's session holds no unsaved transaction: the clean close loses nothing.
    session.close();
    let (code, out, text) = repair_outcome(&dir, a.json, &kept, outcome, || reopen(&dir));
    print!("{out}");
    eprint!("{text}");
    super::warn_unreleased(unstable, err);
    code
}

/// What `ew fmt --repair` prints after `ew_api::fmt::repair`, and its exit code: `(code, stdout, stderr)`. `reopen`
/// opens the repaired project again and runs only after a save. Once the backup exists, `--json`
/// prints the result on every way out: after a failed save with `save_error` (exit 2), after a re-open that
/// found something wrong with `reopen_finding` (exit 2); a refusal before anything is written prints no result. A save
/// that failed before its commit point leaves `discarded` and `files` empty and the generation before; one incomplete
/// after it has taken effect, so they say what the repair decided, beside the decided generation. The human
/// text goes to stderr with `--json`.
fn repair_outcome(
    dir: &Path,
    json: bool,
    kept: &[KeptLine],
    outcome: Result<Repaired, RepairError>,
    reopen: impl FnOnce() -> Result<(), String>,
) -> (i32, String, String) {
    let (mut out, mut err) = (String::new(), String::new());
    let repaired = match outcome {
        Ok(r) => r,
        Err(e) => {
            err.push_str(&format!("{}\n", repair_refusal(dir, &e)));
            if json
                && let RepairError::Save {
                    error,
                    backup,
                    decided,
                } = &e
            {
                let result = match (error, decided) {
                    // Incomplete after its commit point: the decided generation completes at the next open.
                    (ew_api::SaveError::Incomplete { generation, .. }, Some(decided)) => repair_result(
                        &Repaired {
                            generation: *generation,
                            discarded: decided.discarded.clone(),
                            files: decided.files.clone(),
                            backup: Some(backup.clone()),
                        },
                        kept,
                    ),
                    // Before it: the generation and its broken lines as they were.
                    _ => backup_result(Some(backup), backup.generation, kept),
                };
                out.push_str(&results::to_text(&results::FmtRepairResult {
                    save_error: Some(error.to_string()),
                    ..result
                }));
            }
            return (2, out, err);
        }
    };
    let Some(backup) = &repaired.backup else {
        err.push_str("Nothing to repair\n");
        if json {
            out.push_str(&results::to_text(&repair_result(&repaired, kept)));
        }
        return (0, out, err);
    };
    for line in repair_report(&repaired.discarded, backup) {
        let to = if json { &mut err } else { &mut out };
        to.push_str(&line);
        to.push('\n');
    }
    match reopen() {
        Ok(()) => {
            err.push_str(&format!(
                "Repaired: discarded {}; saved generation {}\n",
                plural(repaired.discarded.len(), "line", "lines"),
                repaired.generation
            ));
            if json {
                out.push_str(&results::to_text(&repair_result(&repaired, kept)));
            }
            (0, out, err)
        }
        Err(what) => {
            err.push_str(&format!(
                "error: {}: after the repair the project {what}; the repaired generation {} stays; the backup {} \
                 holds generation {}, and `ew extract` makes a project of it in a new folder\n",
                dir.display(),
                repaired.generation,
                backup.path.display(),
                backup.generation
            ));
            if json {
                out.push_str(&results::to_text(&results::FmtRepairResult {
                    reopen_finding: Some(what),
                    ..repair_result(&repaired, kept)
                }));
            }
            (2, out, err)
        }
    }
}

/// What `ew fmt --repair` prints when it repaired: each discarded line (`file:line` and its text as shown, cut after 80
/// characters, control characters escaped), then the backup's path with, when the backup holds files
/// edited outside EW since the last save, how many ("as found: N files edited outside EW").
fn repair_report(discarded: &[DiscardedLine], backup: &Backup) -> Vec<String> {
    let mut lines: Vec<String> = discarded
        .iter()
        .map(|d| format!("discarded {}:{}: {}", d.file, d.line, printable(&d.shown())))
        .collect();
    let mut about = format!("generation {}", backup.generation);
    if backup.edited > 0 {
        about.push_str(&format!(
            ", as found: {} edited outside EW",
            plural(backup.edited, "file", "files")
        ));
    }
    if backup.reused {
        about.push_str(", already there and reused");
    }
    lines.push(format!("backup {} ({about})", backup.path.display()));
    lines
}

/// `ew fmt --repair --json`'s result of a repair that ran: its discarded lines and files beside [`backup_result`]'s.
fn repair_result(repaired: &Repaired, kept: &[KeptLine]) -> results::FmtRepairResult {
    results::FmtRepairResult {
        discarded: repaired
            .discarded
            .iter()
            .map(|d| results::DiscardedRow {
                file: d.file.clone(),
                line: d.line,
                text: d.shown(),
            })
            .collect(),
        files: repaired
            .files
            .iter()
            .map(|f| results::FmtFile {
                file: f.file.clone(),
                removed: f.removed,
            })
            .collect(),
        ..backup_result(repaired.backup.as_ref(), repaired.generation, kept)
    }
}

/// `ew fmt --repair --json`'s result from the backup alone (`None`: nothing to repair), the generation on disk and the
/// kept lines that stay: no line discarded, no file written, nothing wrong.
fn backup_result(backup: Option<&Backup>, generation: u64, kept: &[KeptLine]) -> results::FmtRepairResult {
    results::FmtRepairResult {
        schema: results::FMT_REPAIR.to_owned(),
        discarded: Vec::new(),
        backup: backup.map(|b| results::BackupRow {
            path: b.path.display().to_string(),
            generation: b.generation,
            reused: b.reused,
            edited: b.edited,
        }),
        files: Vec::new(),
        kept: kept
            .iter()
            .map(|k| results::KeptRow {
                file: k.file.clone(),
                line: k.line,
                code: k.code.clone(),
                reason: k.reason.clone(),
            })
            .collect(),
        generation,
        reopen_finding: None,
        save_error: None,
    }
}

/// Opens the repaired project again as an editor opens it and closes it: `Err` with what is still
/// wrong, in words.
fn reopen(dir: &Path) -> Result<(), String> {
    let options = ew_api::SessionOptions {
        origin: fmt_origin(),
        ..ew_api::SessionOptions::default()
    };
    let (session, report) = ew_api::Session::open(
        dir,
        ew_api::StoreOptions::new(VERSION),
        options,
        ew_api::Recovery::Keep,
    )
    .map_err(|e| format!("does not open: {e}"))?;
    session.close();
    if let Some(f) = report.findings.iter().find(|f| f.code() == Some("EW-CORE-005")) {
        return Err(format!("still has {}", ew_api::fmt::finding_words(f)));
    }
    match &report.access {
        ew_api::Access::Edit => Ok(()),
        ew_api::Access::ReadOnly(reason) => Err(format!("opens read-only: {}", read_only_words(reason))),
    }
}

/// The refusal of a repair open that a finding other than a broken line made read-only: the first
/// such finding by its rule and place; one beyond the findings cap is named as such.
fn beyond_repair_refusal(dir: &Path, report: &ew_api::OpenReport) -> String {
    let what = ew_api::fmt::beyond_repair(report).map_or_else(
        || "a finding beyond those the open lists".to_owned(),
        ew_api::fmt::finding_words,
    );
    format!(
        "error: {}: {what} forces read-only, and `ew fmt --repair` discards only lines that are not JSON or not UTF-8 \
         (EW-CORE-005); nothing was written; resolve it first (`ew check` lists the findings), then run `ew fmt \
         --repair` again",
        dir.display()
    )
}

/// The refusal of a repair that `ew_api::fmt::repair` stopped: before the save nothing was written; a failed save
/// leaves the backup in place.
fn repair_refusal(dir: &Path, e: &RepairError) -> String {
    let d = dir.display();
    match e {
        // A file changed between the repair's open and its backup: the project is as it was, and a
        // new run finds it as it is now.
        RepairError::Pack(ew_api::ewz::PackError::ChangedSinceOpen { path }) => format!(
            "error: {d}: the backup cannot be packed: {path} changed on disk since the repair opened the project; \
             nothing was written; run `ew fmt --repair` again"
        ),
        RepairError::Pack(_) => format!(
            "error: {d}: {e}; nothing was written; keep a copy of the project folder and correct the broken lines by \
             hand"
        ),
        RepairError::BackupTaken(path) => format!(
            "error: {d}: {} already exists and does not hold this generation's backup; nothing was written; move it out \
             of the folder, then run `ew fmt --repair` again",
            path.display()
        ),
        RepairError::Backup { path, error } => format!(
            "error: {d}: the backup {} cannot be written: {error}; nothing was written; make its folder writable, then \
             run `ew fmt --repair` again",
            path.display()
        ),
        RepairError::Format(f @ FmtError::Content { .. }) => format!(
            "error: {d}: {f}; nothing was written; correct that line by hand, then run `ew fmt --repair` again"
        ),
        RepairError::Save {
            error: error @ ew_api::SaveError::Incomplete { .. },
            backup,
            ..
        } => format!(
            "error: {d}: the repair's save is incomplete: {error}; the repaired generation is decided and completes at \
             the next open; the backup is {}",
            backup.path.display()
        ),
        RepairError::Save { error, backup, .. } => format!(
            "error: {d}: the repair's save failed: {error}; the backup {} stays; run `ew fmt --repair` again",
            backup.path.display()
        ),
        other => format!("error: {d}: {other}; nothing was written"),
    }
}

/// The refusal text of a format that wrote nothing (or, after its commit point, completes at the next open).
fn fmt_refusal(e: &FmtError) -> String {
    match e {
        FmtError::Save(ew_api::SaveError::Incomplete { .. }) => {
            format!("error: {e}; the new generation is decided and completes at the next open")
        }
        _ => format!("error: {e}; nothing was written"),
    }
}

/// `ew recover [--replay|--discard]` (D10): the journal of the crashed session with the latest change, one at
/// a time. Without a flag its summary; `--replay` replays it and saves; `--discard` deletes it
/// after the word typed on a controlling terminal (without one: exit 2 before the project is opened). On a project of
/// an older format `--replay` asks the upgrade's typed word (or takes `--yes`) before it replays, and saves with the
/// upgrade's backup.
pub(super) fn run_recover(a: &RecoverArgs) -> i32 {
    use std::io::IsTerminal;
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut errors = std::io::stderr().lock();
    run_recover_with(
        a,
        terminal,
        &mut input,
        (&mut output, &mut errors),
        ew_api::StoreOptions::new(VERSION),
    )
}

/// [`run_recover`] with the terminal test, the reader, the writers (everything `ew recover` prints on stdout, and its
/// own errors on stderr) and the store options injected (unit tests).
fn run_recover_with(
    a: &RecoverArgs,
    terminal: bool,
    input: &mut dyn std::io::BufRead,
    (output, errors): (&mut dyn std::io::Write, &mut dyn std::io::Write),
    store: ew_api::StoreOptions,
) -> i32 {
    if a.action == RecoverAction::Discard && !terminal {
        let _ = writeln!(
            errors,
            "error: HumanOnly: needs an interactive terminal (`recover --discard` is confirmed by typing \
             `{DISCARD_WORD}`; `ew recover` prints the summary without one); nothing was discarded"
        );
        return 2;
    }
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "recover".to_owned(),
        confirmed: a.action == RecoverAction::Discard,
    };
    // `ew recover` acts only with the lock; a read-only open is refused with `ew apply`'s text.
    // The first open keeps the journal: `--replay` opens again to replay it once the upgrade has its consent.
    let opened = open_editing_to(
        &dir,
        origin.clone(),
        store.clone(),
        ew_api::Recovery::Keep,
        None,
        errors,
    );
    let (mut session, opened) = match opened {
        Ok(r) => r,
        Err(code) => return code,
    };
    let generation = opened.generation;
    let result = |journal: Option<&ew_api::RecoveryInfo>, replayed: bool, generation: u64| {
        results::to_text(&results::RecoverResult {
            schema: results::RECOVER.to_owned(),
            action: if a.action == RecoverAction::Replay {
                results::RecoverAction::Replay
            } else {
                results::RecoverAction::Summary
            },
            journal: journal.map(|info| results::JournalSummary {
                transactions: info.transactions,
                labels: info.labels.clone(),
                base_matches: info.base_matches,
                further: info.further,
            }),
            replayed,
            generation,
        })
    };
    // With `--json` stdout is the result: the human lines go to stderr.
    let say = |output: &mut dyn std::io::Write, errors: &mut dyn std::io::Write, text: &str| {
        let to: &mut dyn std::io::Write = if a.json { errors } else { output };
        let _ = to.write_all(text.as_bytes());
    };
    let Some(info) = opened.recovery else {
        say(
            output,
            errors,
            "no journal of a crashed session: nothing to recover\n",
        );
        session.close();
        if a.json {
            let _ = output.write_all(result(None, false, generation).as_bytes());
        }
        return 0;
    };
    match a.action {
        RecoverAction::Summary => {
            say(output, errors, &summary(&info));
            say(
                output,
                errors,
                &format!(
                    "`ew recover --replay` replays and saves {them}; `ew recover --discard` deletes {them}\n",
                    them = if info.transactions == 1 { "it" } else { "them" }
                ),
            );
            session.close();
            if a.json {
                let _ = output.write_all(result(Some(&info), false, generation).as_bytes());
            }
            0
        }
        RecoverAction::Replay => {
            say(output, errors, &summary(&info));
            // A project of an older format opens migrated and the journal replays onto it, as the
            // crashed session had it; its save converts the project, so the upgrade's consent comes first.
            // Without it the session closes with the journal kept as it was, nothing replayed.
            if let Some(from) = &opened.migrated_from
                && !a.yes
            {
                let format = ew_api::FORMAT;
                if !terminal {
                    let _ = writeln!(
                        errors,
                        "error: the project is in the older format {from}: saving the replay converts it, and \
                         confirming that needs an interactive terminal (type `{UPGRADE_WORD}` there) or `ew recover \
                         --replay --yes`; nothing was saved and the journal is kept"
                    );
                    session.close();
                    return 2;
                }
                let prompt = format!(
                    "The project is in the older format {from}. Saving the replay converts it to format {format}, \
                     which older versions of ElektraWatt cannot open, after a backup beside the project folder. Type \
                     `{UPGRADE_WORD}` to replay and upgrade it, anything else to abort: "
                );
                let asked: &mut dyn std::io::Write = if a.json { &mut *errors } else { &mut *output };
                if !typed_word(input, asked, &prompt, UPGRADE_WORD) {
                    let _ = writeln!(
                        errors,
                        "error: not confirmed; nothing was saved and the journal is kept"
                    );
                    session.close();
                    return 1;
                }
            }
            // The replay: the session opened again with the journal applied.
            session.close();
            let mut session =
                match open_editing_to(&dir, origin, store, ew_api::Recovery::Apply, None, errors) {
                    Ok((session, _)) => session,
                    Err(code) => return code,
                };
            if let Some(refusal) = session.recovery_refusal() {
                let (code, text) = replay_refusal(refusal);
                let _ = writeln!(errors, "{text}");
                // The store kept the journal; the clean close deletes only this session's own journal.
                session.close();
                if a.json {
                    let _ = output.write_all(result(Some(&info), false, generation).as_bytes());
                }
                return code;
            }
            session.confirm_upgrade();
            // `Session::save_and_close`, with the upgrade's backup read before the close.
            let saved = session.save();
            let backup = session.upgrade_backup().map(Path::to_path_buf);
            if saved.is_ok() || !session.is_dirty() {
                session.close();
            } else {
                drop(session);
            }
            match saved {
                Ok(saved) => {
                    super::warn_unreleased(saved.unstable, errors);
                    say(
                        output,
                        errors,
                        &format!(
                            "replayed {} of the crashed session's journal; saved generation {}\n",
                            plural(info.transactions, "transaction", "transactions"),
                            saved.generation
                        ),
                    );
                    if let (Some(from), Some(backup)) = (&opened.migrated_from, backup) {
                        say(
                            output,
                            errors,
                            &format!(
                                "upgraded the project from format {from} to format {}; backup: {}\n",
                                ew_api::FORMAT,
                                backup.display()
                            ),
                        );
                    }
                    say(output, errors, &further(info.further));
                    if a.json {
                        let _ = output.write_all(result(Some(&info), true, saved.generation).as_bytes());
                    }
                    0
                }
                Err(e) => {
                    let _ = writeln!(errors, "error: the save failed: {e}");
                    if !matches!(e, ew_api::SaveError::Incomplete { .. }) {
                        // The session was dropped without its clean close.
                        let _ = writeln!(
                            errors,
                            "the replayed transactions stay in this session's journal; `ew recover` offers them again"
                        );
                    }
                    2
                }
            }
        }
        RecoverAction::Discard => {
            let _ = output.write_all(summary(&info).as_bytes());
            let prompt = format!(
                "Discarding deletes the journal: its unsaved changes are lost and do not come back. Type \
                 `{DISCARD_WORD}` to delete it, anything else to abort: "
            );
            if !typed_word(input, output, &prompt, DISCARD_WORD) {
                let _ = writeln!(errors, "error: not confirmed; nothing was discarded");
                session.close();
                return 1;
            }
            session.discard_recovery();
            let _ = writeln!(
                output,
                "discarded the crashed session's journal ({} lost)",
                plural(info.transactions, "unsaved transaction", "unsaved transactions")
            );
            let _ = output.write_all(further(info.further).as_bytes());
            session.close();
            0
        }
    }
}

/// `ew upgrade [--check] [--yes]`: a project of an older format, which
/// opens migrated in memory, converted after the typed word or `--yes`: the session's consent, then its save, which
/// writes the backup beside the project folder first. `--check` is a reader: no lock, nothing written, and it takes an
/// `.ewz`, which the upgrade itself refuses as every writer does.
pub(super) fn run_upgrade(a: &UpgradeArgs) -> i32 {
    use std::io::IsTerminal;
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut errors = std::io::stderr().lock();
    run_upgrade_with(
        a,
        terminal,
        &mut input,
        (&mut output, &mut errors),
        ew_api::StoreOptions::new(VERSION),
    )
}

/// [`run_upgrade`] with the terminal test, the reader, the writers (everything `ew upgrade` prints on stdout, and its
/// own errors on stderr) and the store options (a test's migration chain among them) injected.
fn run_upgrade_with(
    a: &UpgradeArgs,
    terminal: bool,
    input: &mut dyn std::io::BufRead,
    (output, errors): (&mut dyn std::io::Write, &mut dyn std::io::Write),
    mut store: ew_api::StoreOptions,
) -> i32 {
    let dir = if a.check {
        reader_dir(a.project.as_ref())
    } else {
        project_dir(a.project.as_ref())
    };
    let Some(dir) = dir else {
        return 2;
    };
    let format = ew_api::FORMAT;
    if a.check {
        store.mode = ew_api::OpenMode::ReadOnly;
        store.state_dir = None;
        let options = ew_api::SessionOptions {
            origin: ew_api::Origin::Cli {
                subcommand: "upgrade".to_owned(),
                confirmed: false,
            },
            ..ew_api::SessionOptions::default()
        };
        let ewz = ew_api::ewz::is_ewz(&ew_api::OsVfs, &dir);
        let opened = if ewz {
            ew_api::Session::open_ewz_with(&dir, store, "upgrade", ew_api::MemoMode::default())
        } else {
            ew_api::Session::open(&dir, store, options, ew_api::Recovery::Keep)
        };
        let opened = match opened {
            Ok((session, opened)) => {
                session.close();
                opened
            }
            Err(e) => {
                let _ = writeln!(errors, "error: {}: {e}", dir.display());
                return 2;
            }
        };
        return match opened.migrated_from {
            // `ew upgrade` refuses an `.ewz` as every writer does, so the way names `ew extract` first.
            Some(from) if ewz => {
                let _ = writeln!(
                    output,
                    "the project is in the older format {from}: `ew extract` makes a project folder of the archive, \
                     and `ew upgrade` converts that folder to format {format} after a backup"
                );
                1
            }
            Some(from) => {
                let _ = writeln!(
                    output,
                    "the project is in the older format {from}: `ew upgrade` converts it to format {format} after a \
                     backup"
                );
                1
            }
            None => {
                let _ = writeln!(output, "the project needs no upgrade to format {format}");
                0
            }
        };
    }
    let origin = ew_api::Origin::Cli {
        subcommand: "upgrade".to_owned(),
        confirmed: false,
    };
    let (mut session, opened) = match open_editing(&dir, origin, store, ew_api::Recovery::Keep) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let Some(from) = opened.migrated_from else {
        let _ = writeln!(output, "the project needs no upgrade to format {format}");
        session.close();
        return 0;
    };
    if opened.recovery.is_some() {
        let _ = writeln!(errors, "{}", journal_refusal(&dir));
        session.close();
        return 2;
    }
    if !a.yes {
        if !terminal {
            let _ = writeln!(
                errors,
                "error: the project is in the older format {from}: confirming its upgrade needs an interactive \
                 terminal (type `{UPGRADE_WORD}` there) or `ew upgrade --yes`; nothing was changed"
            );
            session.close();
            return 2;
        }
        let prompt = format!(
            "The project is in the older format {from}. Upgrading converts it to format {format}, which older \
             versions of ElektraWatt cannot open, after a backup beside the project folder. Type `{UPGRADE_WORD}` to \
             upgrade it, anything else to abort: "
        );
        if !typed_word(input, output, &prompt, UPGRADE_WORD) {
            let _ = writeln!(errors, "error: not confirmed; nothing was changed");
            session.close();
            return 1;
        }
    }
    session.confirm_upgrade();
    let result = session.save();
    let backup = session.upgrade_backup().map(Path::to_path_buf);
    match result {
        Ok(saved) => {
            super::warn_unreleased(saved.unstable, errors);
            let _ = writeln!(
                output,
                "upgraded the project from format {from} to format {format}; saved generation {}",
                saved.generation
            );
            if let Some(backup) = backup {
                let _ = writeln!(output, "backup: {}", backup.display());
            }
            session.close();
            0
        }
        Err(e) => {
            let _ = writeln!(errors, "error: the save failed: {e}");
            if let Some(backup) = backup {
                let _ = writeln!(errors, "the backup stays at {}", backup.display());
            }
            session.close();
            2
        }
    }
}

/// The summary of a crashed session's journal: its unsaved transactions with their commands in journal order, whether
/// the project was saved since, and how many more journals remain.
fn summary(info: &ew_api::RecoveryInfo) -> String {
    let mut out = format!(
        "a crashed session left {} in its journal:\n",
        plural(info.transactions, "unsaved transaction", "unsaved transactions")
    );
    for (i, label) in info.labels.iter().enumerate() {
        out.push_str(&format!("  {}. {}\n", i + 1, printable(label)));
    }
    out.push_str(if info.base_matches {
        "the project is unchanged since that session last opened or saved it\n"
    } else {
        "the project was saved by another session since; a replay is refused when it meets a field changed in that \
         save\n"
    });
    out.push_str(&further(info.further));
    out
}

/// How many further crashed sessions' journals remain; empty when none.
fn further(n: usize) -> String {
    match n {
        0 => String::new(),
        1 => "1 more journal of a crashed session remains; run ew recover again\n".to_owned(),
        n => format!("{n} more journals of crashed sessions remain; run ew recover again\n"),
    }
}

/// The exit code and text of a replay the store refused; the journal is kept.
fn replay_refusal(e: &ew_api::RecoverError) -> (i32, String) {
    match e {
        ew_api::RecoverError::Replay(ew_api::ReplayError::Collisions(c)) => (
            1,
            format!(
                "error: the replay is refused: {} of the journal changed on disk too; the journal is kept and nothing \
                 was saved (`ew recover --discard` deletes it)",
                plural(c.len(), "field", "fields")
            ),
        ),
        ew_api::RecoverError::Replay(ew_api::ReplayError::Invalid(why)) => (
            1,
            format!(
                "error: the journal does not replay: {}; the journal is kept and nothing was saved",
                printable(why)
            ),
        ),
        ew_api::RecoverError::Io(e) => (
            2,
            format!("error: the replay cannot be journalled: {e}; the journal is kept and nothing was saved"),
        ),
        ew_api::RecoverError::Nothing => (2, "error: no journal to replay; nothing was saved".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    //! `ew recover` with the injected terminal on a project in a fresh directory, with its own state directory for the
    //! journals.

    use std::path::{Path, PathBuf};

    use super::*;
    use crate::cli::{Command, parse};

    const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

    /// A fresh directory with a saved project (`p.ew`, pages 1 and 2) and an empty state directory (`state`).
    fn setup(tag: &str) -> PathBuf {
        let dir = ew_testdir::fresh_dir(&format!("ew-cli-recover-{tag}"));
        std::fs::create_dir(dir.join("state")).unwrap();
        let mut s =
            ew_api::Session::from_records(serde_json::from_str(PROJECT).unwrap(), [], [], Default::default())
                .unwrap();
        for n in [1, 2] {
            s.execute(&page(n)).unwrap();
        }
        let mut o = ew_api::StoreOptions::new("test");
        o.state_dir = None;
        s.create(&dir.join("p.ew"), o).unwrap();
        s.close();
        dir
    }

    fn page(number: u32) -> ew_api::AnyCommand {
        ew_api::AnyCommand {
            name: "page.create".into(),
            params: serde_json::json!({ "number": number }),
        }
    }

    fn store(dir: &Path) -> ew_api::StoreOptions {
        let mut o = ew_api::StoreOptions::new("test");
        o.state_dir = Some(dir.join("state"));
        o
    }

    /// The journal files in the state directory with their bytes.
    fn journals(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn go(d: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    go(&p, out);
                } else if p.extension().is_some_and(|x| x == "jsonl") {
                    out.push((p.clone(), std::fs::read(&p).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        go(&dir.join("state"), &mut out);
        out.sort();
        out
    }

    /// A session that adds the page `number` and crashes (dropped without a save or a clean close: its journal stays).
    fn crash(dir: &Path, number: u32) {
        let (mut s, _) = ew_api::Session::open(
            &dir.join("p.ew"),
            store(dir),
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        s.execute(&page(number)).unwrap();
        drop(s);
    }

    /// The folio numbers of the saved project's pages.
    fn saved_folios(dir: &Path) -> Vec<String> {
        let (s, _) = ew_api::Session::open_reader(&dir.join("p.ew"), "test", "test").unwrap();
        let labels = s.page_labels().to_vec();
        s.close();
        labels
    }

    /// Runs `ew recover` on the project with `terminal` and the typed `answer`: the exit code, its stdout and its
    /// stderr.
    fn recover(
        dir: &Path,
        action: RecoverAction,
        json: bool,
        terminal: bool,
        answer: &str,
    ) -> (i32, String, String) {
        let args = RecoverArgs {
            project: Some(dir.join("p.ew")),
            action,
            json,
            yes: false,
        };
        let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
        let (mut output, mut errors) = (Vec::new(), Vec::new());
        let code = run_recover_with(
            &args,
            terminal,
            &mut input,
            (&mut output, &mut errors),
            store(dir),
        );
        (
            code,
            String::from_utf8(output).unwrap(),
            String::from_utf8(errors).unwrap(),
        )
    }

    #[test]
    fn recover_and_fmt_parse_their_flags() {
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string());
        let recover = |project: Option<&str>, action, json| {
            Ok(Command::Recover(RecoverArgs {
                project: project.map(PathBuf::from),
                action,
                json,
                yes: false,
            }))
        };
        assert_eq!(parse(&["recover"]), recover(None, RecoverAction::Summary, false));
        assert_eq!(
            parse(&["recover", "p.ew", "--replay", "--json"]),
            recover(Some("p.ew"), RecoverAction::Replay, true)
        );
        assert_eq!(
            parse(&["recover", "--discard", "--project", "p.ew"]),
            recover(Some("p.ew"), RecoverAction::Discard, false)
        );
        assert_eq!(
            parse(&["recover", "--replay", "--discard"]),
            Err("error: the argument '--replay' cannot be used with '--discard'\n\nUsage: ew recover --replay \
                 [project-dir]\n\nFor more information, try '--help'.\n"
                .to_owned())
        );
        assert_eq!(
            parse(&["recover", "--discard", "--json"]),
            Err("error: `--discard` asks for a typed word on a terminal and prints no result: it takes no \
                 `--json`\n\nUsage: ew recover [OPTIONS] [project-dir]\n\nFor more information, try '--help'.\n"
                .to_owned())
        );
        assert_eq!(
            parse(&["recover", "--yes"]),
            Err("error: the following required arguments were not provided:\n  --replay\n\nUsage: ew recover \
                 --replay --yes [project-dir]\n\nFor more information, try '--help'.\n"
                .to_owned())
        );
        assert_eq!(
            parse(&["recover", "--replay", "--yes"]),
            Ok(Command::Recover(RecoverArgs {
                project: None,
                action: RecoverAction::Replay,
                json: false,
                yes: true,
            }))
        );
        assert_eq!(
            parse(&["fmt", "p.ew", "--check"]),
            Ok(Command::Fmt(FmtArgs {
                project: Some(PathBuf::from("p.ew")),
                check: true,
                repair: false,
                json: false,
            }))
        );
        assert_eq!(
            parse(&["fmt", "p.ew", "--repair", "--json"]),
            Ok(Command::Fmt(FmtArgs {
                project: Some(PathBuf::from("p.ew")),
                check: false,
                repair: true,
                json: true,
            }))
        );
        // `--repair` with `--check` is refused by the parser, before any open.
        assert_eq!(
            parse(&["fmt", "p.ew", "--repair", "--check"]),
            Err(
                "error: the argument '--repair' cannot be used with '--check'\n\nUsage: ew fmt --repair \
                 <project-dir>\n\nFor more information, try '--help'.\n"
                    .to_owned()
            )
        );
        assert_eq!(
            parse(&["fmt", "a", "b"]),
            Err(
                "error: unexpected argument 'b' found\n\nUsage: ew fmt [OPTIONS] [project-dir]\n\nFor more \
                 information, try '--help'.\n"
                    .to_owned()
            )
        );
        assert_eq!(
            parse(&["fmt", "a", "--project", "b"]),
            Err(
                "error: the argument '[project-dir]' cannot be used with '--project <dir>'\n\nUsage: ew fmt \
                 <project-dir>\n\nFor more information, try '--help'.\n"
                    .to_owned()
            )
        );
    }

    /// `--discard` without a terminal exits 2 and changes nothing, with a terminal and
    /// a wrong word exits 1 (the injected terminal of `run_assets_purge_with`): the summary names the transactions;
    /// off a terminal nothing is opened; a wrong word aborts; `discard` deletes the summarised journal only, and the other is offered next.
    #[test]
    fn the_discard_needs_the_typed_word_and_deletes_one_journal() {
        let dir = setup("discard");
        crash(&dir, 5);
        // The second crash is the later change: it is summarised first.
        std::thread::sleep(std::time::Duration::from_millis(20));
        crash(&dir, 6);
        let before = journals(&dir);
        assert_eq!(before.len(), 2);

        let (code, shown, errors) = recover(&dir, RecoverAction::Summary, false, false, "");
        assert_eq!((code, errors.as_str()), (0, ""));
        assert_eq!(
            shown,
            "a crashed session left 1 unsaved transaction in its journal:\n  1. page.create\nthe project is unchanged \
             since that session last opened or saved it\n1 more journal of a crashed session remains; run ew recover \
             again\n`ew recover --replay` replays and saves it; `ew recover --discard` deletes it\n"
        );
        assert_eq!(journals(&dir), before, "the summary changes nothing");

        let (code, shown, errors) = recover(&dir, RecoverAction::Discard, false, false, "discard\n");
        assert_eq!(
            (code, shown.as_str(), errors.as_str()),
            (
                2,
                "",
                "error: HumanOnly: needs an interactive terminal (`recover --discard` is confirmed by typing \
                 `discard`; `ew recover` prints the summary without one); nothing was discarded\n"
            ),
            "off a terminal nothing is opened"
        );
        assert_eq!(journals(&dir), before);

        let prompt = "a crashed session left 1 unsaved transaction in its journal:\n  1. page.create\nthe project is \
                      unchanged since that session last opened or saved it\n1 more journal of a crashed session \
                      remains; run ew recover again\nDiscarding deletes the journal: its unsaved changes are lost and \
                      do not come back. Type `discard` to delete it, anything else to abort: ";
        for wrong in ["", "\n", "y\n", "yes\n", "Discard\n", "discard it\n"] {
            let (code, shown, errors) = recover(&dir, RecoverAction::Discard, false, true, wrong);
            assert_eq!(
                (code, shown.as_str(), errors.as_str()),
                (1, prompt, "error: not confirmed; nothing was discarded\n"),
                "{wrong:?} must abort"
            );
            assert_eq!(journals(&dir), before, "{wrong:?}: nothing is discarded");
        }

        let (code, shown, errors) = recover(&dir, RecoverAction::Discard, false, true, "  discard \r\n");
        assert_eq!((code, errors.as_str()), (0, ""), "{shown}");
        assert_eq!(
            shown,
            format!(
                "{prompt}discarded the crashed session's journal (1 unsaved transaction lost)\n1 more journal of a \
                 crashed session remains; run ew recover again\n"
            )
        );
        let left = journals(&dir);
        assert_eq!(left.len(), 1);
        assert!(before.contains(&left[0]), "the other journal is unchanged");

        // The other journal (page 5) is offered next and replays.
        let (code, shown, errors) = recover(&dir, RecoverAction::Replay, false, false, "");
        assert_eq!((code, errors.as_str()), (0, ""), "{shown}");
        assert_eq!(
            shown,
            "a crashed session left 1 unsaved transaction in its journal:\n  1. page.create\nthe project is unchanged \
             since that session last opened or saved it\nreplayed 1 transaction of the crashed session's journal; \
             saved generation 2\n"
        );
        assert_eq!(saved_folios(&dir), ["1", "2", "5"]);
        assert!(journals(&dir).is_empty());
        let (code, shown, errors) = recover(&dir, RecoverAction::Summary, false, false, "");
        assert_eq!(
            (code, shown.as_str(), errors.as_str()),
            (0, "no journal of a crashed session: nothing to recover\n", "")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `ew recover` checks that it got edit access; if not, it refuses with exit 2 and the same
    /// text `ew apply` uses for a held lock; it never acts read-only. An editor holds the lock: every action is
    /// refused and the journal stays; the refusal text is `read_only_refusal`'s, the one `ew apply` prints.
    #[test]
    fn recover_is_refused_while_another_session_holds_the_lock() {
        let dir = setup("locked");
        crash(&dir, 5);
        let before = journals(&dir);
        let mut held = ew_api::StoreOptions::new("test");
        held.state_dir = None;
        let (editor, opened) = ew_api::Session::open(
            &dir.join("p.ew"),
            held,
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        assert_eq!(opened.access, ew_api::Access::Edit);
        // The lock's holder in words (its user, host and start), as a second open reports it.
        let (probe, probed) = ew_api::Session::open(
            &dir.join("p.ew"),
            store(&dir),
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        probe.close();
        let ew_api::Access::ReadOnly(reason) = &probed.access else {
            panic!("the probe opened {:?}", probed.access);
        };
        let refusal = format!(
            "error: {}: the project opens read-only: {}; nothing was written\n",
            dir.join("p.ew").display(),
            read_only_words(reason)
        );
        for action in [
            RecoverAction::Summary,
            RecoverAction::Replay,
            RecoverAction::Discard,
        ] {
            let (code, shown, errors) = recover(&dir, action, false, true, "discard\n");
            assert_eq!(
                (code, shown.as_str(), errors.as_str()),
                (2, "", refusal.as_str()),
                "{action:?}"
            );
            assert_eq!(journals(&dir), before, "{action:?}");
        }
        editor.close();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `journal_refusal` names the command that now exists and no FT number; each kind of subcommand that prints it
    /// (`ew apply`, `ew import parts`, a human-only subcommand) has a whole-text test. The human-only kind: `project assets purge`, confirmed on the injected
    /// terminal, opens through `open_for_writing_with` and refuses a project with a crashed
    /// session's journal before it asks: exit 2, nothing prompted, the journal unchanged. It prints
    /// `journal_refusal`'s text on stderr, which a unit test does not capture, so the text is asserted whole here;
    /// `ew apply` and `ew import parts` assert the same text through the binary (`tests/it/cli.rs`).
    #[test]
    fn a_human_only_subcommand_refuses_a_crashed_session_journal() {
        let dir = setup("human-only");
        crash(&dir, 5);
        let before = journals(&dir);
        assert_eq!(before.len(), 1);
        let project = dir.join("p.ew");
        let args = crate::cli::AssetsPurgeArgs {
            project: Some(project.clone()),
            preview: false,
        };
        let mut input = std::io::Cursor::new(b"purge\n".to_vec());
        let mut output = Vec::new();
        let code = crate::cli::run_assets_purge_with(&args, true, &mut input, &mut output, store(&dir));
        assert_eq!((code, String::from_utf8(output).unwrap().as_str()), (2, ""));
        assert_eq!(journals(&dir), before, "the journal stays");
        assert_eq!(
            journal_refusal(&project),
            format!(
                "error: {}: a crashed session left unsaved changes in its journal; replay or discard them first with \
                 `ew recover`; nothing was written",
                project.display()
            )
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The texts of a replay the journal itself refuses (a control sequence from the journal is escaped) and of the
    /// further journals; the collisions' text is asserted through the binary (`tests/it/cli.rs`).
    #[test]
    fn refusal_and_further_texts() {
        assert_eq!(
            replay_refusal(&ew_api::RecoverError::Replay(ew_api::ReplayError::Invalid(
                "a\u{1b}[2J".into()
            ))),
            (
                1,
                "error: the journal does not replay: a\\u{1b}[2J; the journal is kept and nothing was saved"
                    .to_owned()
            )
        );
        assert_eq!(further(0), "");
        assert_eq!(
            further(2),
            "2 more journals of crashed sessions remain; run ew recover again\n"
        );
    }

    /// Once the backup exists, `--json` prints the result on every way out, with exit 2 where it is a failure,
    /// and the human text goes to stderr: a re-open that still finds something wrong (with `reopen_finding`), a save
    /// incomplete after its commit point (with `save_error`, and `discarded` and `files` as the repair decided them)
    /// and a save that failed before it (with `save_error`, `discarded` and `files` empty), each with and
    /// without `--json` (the faults the binary's tests cannot place).
    #[test]
    fn a_repair_with_a_backup_prints_its_result_on_every_way_out() {
        let dir = Path::new("p.ew");
        let backup = Backup {
            path: PathBuf::from("p.ew.before-repair-g1.ewz"),
            generation: 1,
            reused: false,
            edited: 1,
        };
        let kept = [KeptLine {
            file: "pages/1.jsonl".to_owned(),
            line: Some(4),
            code: "EW-CORE-007".to_owned(),
            reason: "a schema violation".to_owned(),
        }];
        let repaired = Repaired {
            generation: 2,
            discarded: vec![DiscardedLine {
                file: "pages/1.jsonl".to_owned(),
                line: 2,
                text: b"not json".to_vec(),
            }],
            files: vec![Rewrite {
                file: "pages/1.jsonl".to_owned(),
                removed: false,
            }],
            backup: Some(backup.clone()),
        };
        // The printer writes the members in byte order of their names.
        let kept_json = "  \"kept\": [\n    {\n      \"code\": \"EW-CORE-007\",\n      \"file\": \"pages/1.jsonl\",\n      \
                         \"line\": 4,\n      \"reason\": \"a schema violation\"\n    }\n  ],\n";
        let backup_json = "  \"backup\": {\n    \"edited\": 1,\n    \"generation\": 1,\n    \"path\": \
                           \"p.ew.before-repair-g1.ewz\",\n    \"reused\": false\n  },\n";

        // The re-open after the save finds a broken line still there.
        let report = "discarded pages/1.jsonl:2: not json\nbackup p.ew.before-repair-g1.ewz (generation 1, as found: 1 \
                      file edited outside EW)\n";
        let failed = "error: p.ew: after the repair the project still has EW-CORE-005 at pages/1.jsonl:3; the repaired \
                      generation 2 stays; the backup p.ew.before-repair-g1.ewz holds generation 1, and `ew extract` \
                      makes a project of it in a new folder\n";
        let reopen = || Err("still has EW-CORE-005 at pages/1.jsonl:3".to_owned());
        assert_eq!(
            repair_outcome(dir, false, &kept, Ok(repaired.clone()), reopen),
            (2, report.to_owned(), failed.to_owned())
        );
        // The result of the repair as it took effect, with `tail` (`"<member>": <value>`) after the kept lines.
        let repaired_json = |tail: &str| {
            format!(
                "{{\n  \"$schema\": \"ew.fmt-repair-result/1\",\n{backup_json}  \"discarded\": [\n    {{\n      \
                 \"file\": \"pages/1.jsonl\",\n      \"line\": 2,\n      \"text\": \"not json\"\n    }}\n  ],\n  \
                 \"files\": [\n    {{\n      \"file\": \"pages/1.jsonl\",\n      \"removed\": false\n    }}\n  ],\n  \
                 \"generation\": 2,\n{kept_json}  {tail}\n}}\n"
            )
        };
        assert_eq!(
            repair_outcome(dir, true, &kept, Ok(repaired.clone()), reopen),
            (
                2,
                repaired_json("\"reopen_finding\": \"still has EW-CORE-005 at pages/1.jsonl:3\""),
                format!("{report}{failed}")
            )
        );

        // The save after the backup: incomplete after its commit point, which took effect and carries what the repair
        // decided, and failed before it, which left the generation as it was.
        let incomplete = || RepairError::Save {
            error: ew_api::SaveError::Incomplete {
                generation: 2,
                step: ew_api::SaveStep::Renames,
                error: std::io::Error::other("injected error"),
            },
            backup: backup.clone(),
            decided: Some(Box::new(ew_api::fmt::Decided {
                discarded: repaired.discarded.clone(),
                files: repaired.files.clone(),
            })),
        };
        let incomplete_text = "error: p.ew: the repair's save is incomplete: save incomplete at Renames (generation 2): \
                               injected error; the repaired generation is decided and completes at the next open; the \
                               backup is p.ew.before-repair-g1.ewz\n";
        let save_failed = || RepairError::Save {
            error: ew_api::SaveError::Failed {
                step: ew_api::SaveStep::Temps,
                path: PathBuf::from("pages/1.jsonl.tmp-2"),
                error: std::io::Error::other("disk full"),
            },
            backup: backup.clone(),
            decided: None,
        };
        let failed_text = "error: p.ew: the repair's save failed: save failed at Temps (pages/1.jsonl.tmp-2): disk full; \
                           nothing changed on disk; the backup p.ew.before-repair-g1.ewz stays; run `ew fmt --repair` \
                           again\n";
        let never = || -> Result<(), String> { panic!("no re-open after a failed save") };
        assert_eq!(
            repair_outcome(dir, false, &kept, Err(save_failed()), never),
            (2, String::new(), failed_text.to_owned())
        );
        assert_eq!(
            repair_outcome(dir, true, &kept, Err(save_failed()), never),
            (
                2,
                format!(
                    "{{\n  \"$schema\": \"ew.fmt-repair-result/1\",\n{backup_json}  \"discarded\": [],\n  \"files\": \
                     [],\n  \"generation\": 1,\n{kept_json}  \"save_error\": \"save failed at Temps \
                     (pages/1.jsonl.tmp-2): disk full; nothing changed on disk\"\n}}\n"
                ),
                failed_text.to_owned()
            )
        );
        assert_eq!(
            repair_outcome(dir, false, &kept, Err(incomplete()), never),
            (2, String::new(), incomplete_text.to_owned())
        );
        assert_eq!(
            repair_outcome(dir, true, &kept, Err(incomplete()), never),
            (
                2,
                repaired_json(
                    "\"save_error\": \"save incomplete at Renames (generation 2): injected error\""
                ),
                incomplete_text.to_owned()
            )
        );

        // A refusal before anything is written prints no result.
        assert_eq!(
            repair_outcome(dir, true, &kept, Err(RepairError::Dirty), never),
            (
                2,
                String::new(),
                "error: p.ew: the session has unsaved changes; a repair writes only the saved project; nothing was \
                 written\n"
                    .to_owned()
            )
        );
    }

    /// The backup's line counts the files edited outside EW, without the clause when
    /// there are none, also for a backup reused; a file changed on disk between the repair's open and its backup
    /// refuses, saying to run the repair again (a race the binary's tests cannot place).
    #[test]
    fn the_repair_report_counts_the_files_edited_outside_ew() {
        let backup = |edited, reused| Backup {
            path: PathBuf::from("p.ew.before-repair-g3.ewz"),
            generation: 3,
            reused,
            edited,
        };
        let line = |b: &Backup| repair_report(&[], b).join("\n");
        assert_eq!(
            line(&backup(0, false)),
            "backup p.ew.before-repair-g3.ewz (generation 3)"
        );
        assert_eq!(
            line(&backup(1, false)),
            "backup p.ew.before-repair-g3.ewz (generation 3, as found: 1 file edited outside EW)"
        );
        assert_eq!(
            line(&backup(2, true)),
            "backup p.ew.before-repair-g3.ewz (generation 3, as found: 2 files edited outside EW, already there and \
             reused)"
        );
        let changed = RepairError::Pack(ew_api::ewz::PackError::ChangedSinceOpen {
            path: "pages/1.jsonl".to_owned(),
        });
        assert_eq!(
            repair_refusal(Path::new("p.ew"), &changed),
            "error: p.ew: the backup cannot be packed: pages/1.jsonl changed on disk since the repair opened the \
             project; nothing was written; run `ew fmt --repair` again"
        );
    }

    // ---- `ew upgrade` -------------------------------------------------

    fn unchanged(value: serde_json::Value) -> Result<serde_json::Value, ew_api::MigrationError> {
        Ok(value)
    }

    /// The test chain: the format step from "0.0", which changes no value.
    static UPGRADE_CHAIN: ew_api::Migrations = ew_api::Migrations {
        kinds: &[],
        formats: &[ew_api::FormatStep {
            from: "0.0",
            to: "0.1",
            manifest: unchanged,
            record: unchanged,
        }],
    };

    /// [`setup`]'s project with the stamps of the test format "0.0".
    fn setup_v0(tag: &str) -> PathBuf {
        let dir = setup(tag);
        let path = dir.join("p.ew").join("manifest.json");
        let text = std::fs::read_to_string(&path).unwrap();
        let format = ew_api::FORMAT;
        let older = text
            .replace(&format!("\"format\": \"{format}\""), "\"format\": \"0.0\"")
            .replace(
                &format!("\"min_reader\": \"{format}\""),
                "\"min_reader\": \"0.0\"",
            );
        assert_ne!(older, text);
        std::fs::write(&path, older).unwrap();
        dir
    }

    fn upgrade_store(dir: &Path) -> ew_api::StoreOptions {
        let mut o = store(dir);
        o.migrations = &UPGRADE_CHAIN;
        o
    }

    /// Runs `ew upgrade` on `project` with `terminal` and the typed `answer`: the exit code, its stdout and its stderr.
    fn upgrade(
        project: &Path,
        store: ew_api::StoreOptions,
        flags: (bool, bool),
        terminal: bool,
        answer: &str,
    ) -> (i32, String, String) {
        let args = UpgradeArgs {
            project: Some(project.to_path_buf()),
            check: flags.0,
            yes: flags.1,
        };
        let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
        let (mut output, mut errors) = (Vec::new(), Vec::new());
        let code = run_upgrade_with(&args, terminal, &mut input, (&mut output, &mut errors), store);
        (
            code,
            String::from_utf8(output).unwrap(),
            String::from_utf8(errors).unwrap(),
        )
    }

    /// Every file of the project folder and beside it (the backups), by name, without the lock.
    fn upgrade_files(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        fn go(d: &Path, base: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                let name = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                if p.is_dir() {
                    if name != "state" {
                        go(&p, base, out);
                    }
                } else if !name.ends_with(".~ew.lock") {
                    out.insert(name, std::fs::read(&p).unwrap());
                }
            }
        }
        let mut out = std::collections::BTreeMap::new();
        go(dir, dir, &mut out);
        out
    }

    /// The backup's path as the upgrade names it: beside the project folder, as the file system names that folder.
    fn backup_of(dir: &Path) -> PathBuf {
        ew_api::fmt::backup_path(&ew_api::OsVfs, &dir.join("p.ew"), 0)
            .unwrap()
            .with_file_name("p.ew.pre-0.0.ewz")
    }

    fn prompt() -> String {
        format!(
            "The project is in the older format 0.0. Upgrading converts it to format {}, which older versions of \
             ElektraWatt cannot open, after a backup beside the project folder. Type `upgrade` to upgrade it, anything \
             else to abort: ",
            ew_api::FORMAT
        )
    }

    #[test]
    fn upgrade_parses_its_flags() {
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string());
        let upgrade = |project: Option<&str>, check, yes| {
            Ok(Command::Upgrade(UpgradeArgs {
                project: project.map(PathBuf::from),
                check,
                yes,
            }))
        };
        assert_eq!(parse(&["upgrade"]), upgrade(None, false, false));
        assert_eq!(
            parse(&["upgrade", "p.ew", "--check"]),
            upgrade(Some("p.ew"), true, false)
        );
        assert_eq!(
            parse(&["upgrade", "--yes", "--project", "p.ew"]),
            upgrade(Some("p.ew"), false, true)
        );
        assert_eq!(
            parse(&["upgrade", "--check", "--yes"]),
            Err(
                "error: the argument '--check' cannot be used with '--yes'\n\nUsage: ew upgrade --check \
                 [project-dir]\n\nFor more information, try '--help'.\n"
                    .to_owned()
            )
        );
    }

    /// MG-5: `--check` writes nothing and exits 1 on the test fixture and 0 on Golden A (read only, no lock).
    #[test]
    fn upgrade_check_exits_1_on_an_older_project_and_0_on_golden_a() {
        let dir = setup_v0("upgrade-check");
        let before = upgrade_files(&dir);
        assert_eq!(
            upgrade(&dir.join("p.ew"), upgrade_store(&dir), (true, false), false, ""),
            (
                1,
                format!(
                    "the project is in the older format 0.0: `ew upgrade` converts it to format {} after a backup\n",
                    ew_api::FORMAT
                ),
                String::new()
            )
        );
        assert_eq!(upgrade_files(&dir), before);
        let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
        assert_eq!(
            upgrade(&golden, upgrade_store(&dir), (true, false), false, ""),
            (
                0,
                format!("the project needs no upgrade to format {}\n", ew_api::FORMAT),
                String::new()
            )
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The writers other than `ew upgrade` (`ew apply`, the purge, the release and the
    /// rest through `open_for_writing_items`; `ew fmt` and `--repair`) refuse a project of an older format before the
    /// unsaved-changes check, naming `ew upgrade`; nothing is written. `ew fmt --check` says so as a needed change.
    #[test]
    fn the_writers_refuse_an_older_project_naming_ew_upgrade() {
        let dir = setup_v0("older-writers");
        let project = dir.join("p.ew");
        let before = upgrade_files(&dir);
        let origin = ew_api::Origin::Cli {
            subcommand: "apply".to_owned(),
            confirmed: false,
        };
        assert!(matches!(
            super::super::open_for_writing_items(&project, origin, upgrade_store(&dir)),
            Err(2)
        ));
        assert_eq!(upgrade_files(&dir), before, "nothing was written");
        let older = format!(
            "{}: the project is in the older format 0.0; convert it with `ew upgrade <project>` first; nothing was \
             written",
            project.display()
        );
        // `ew fmt`, `--check` (with and without `--json`) and `--repair` through their runner, the whole
        // stderr and stdout and the exit codes.
        let fmt = |check: bool, repair: bool, json: bool| {
            let args = FmtArgs {
                project: Some(project.clone()),
                check,
                repair,
                json,
            };
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = run_fmt_with(&args, upgrade_store(&dir), (&mut out, &mut err));
            (
                code,
                String::from_utf8(out).unwrap(),
                String::from_utf8(err).unwrap(),
            )
        };
        assert_eq!(
            fmt(false, false, false),
            (2, String::new(), format!("error: {older}\n"))
        );
        assert_eq!(
            fmt(true, false, false),
            (1, String::new(), format!("fmt: {older}\n"))
        );
        assert_eq!(
            fmt(false, true, false),
            (2, String::new(), format!("error: {older}\n"))
        );
        let (code, json, err) = fmt(true, false, true);
        assert_eq!((code, err), (1, format!("fmt: {older}\n")));
        let json: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "$schema": "ew.fmt-result/1", "check": true, "files": [], "kept": [], "generation": 1,
                "older_format": "0.0"
            })
        );
        assert_eq!(upgrade_files(&dir), before, "nothing was written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// MG-5: without a terminal or `--yes` it exits 2, and a word other than `upgrade` exits 1: nothing changed either
    /// way; the typed word upgrades.
    #[test]
    fn upgrade_asks_on_a_terminal_and_refuses_without_one() {
        let dir = setup_v0("upgrade-ask");
        let before = upgrade_files(&dir);
        let project = dir.join("p.ew");
        assert_eq!(
            upgrade(&project, upgrade_store(&dir), (false, false), false, ""),
            (
                2,
                String::new(),
                "error: the project is in the older format 0.0: confirming its upgrade needs an interactive terminal \
                 (type `upgrade` there) or `ew upgrade --yes`; nothing was changed\n"
                    .to_owned()
            )
        );
        assert_eq!(upgrade_files(&dir), before);
        assert_eq!(
            upgrade(&project, upgrade_store(&dir), (false, false), true, "yes\n"),
            (
                1,
                prompt(),
                "error: not confirmed; nothing was changed\n".to_owned()
            )
        );
        assert_eq!(upgrade_files(&dir), before);
        assert_eq!(
            upgrade(&project, upgrade_store(&dir), (false, false), true, "upgrade\n"),
            (
                0,
                format!(
                    "{}upgraded the project from format 0.0 to format {}; saved generation 2\nbackup: {}\n",
                    prompt(),
                    ew_api::FORMAT,
                    backup_of(&dir).display()
                ),
                String::new()
            )
        );
        assert!(backup_of(&dir).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// MG-5: `--yes` writes the backup and saves; afterwards, and on a current project, there is nothing to upgrade.
    #[test]
    fn upgrade_yes_backs_up_and_saves() {
        let dir = setup_v0("upgrade-yes");
        let project = dir.join("p.ew");
        let before = upgrade_files(&dir);
        assert_eq!(
            upgrade(&project, upgrade_store(&dir), (false, true), false, ""),
            (
                0,
                format!(
                    "upgraded the project from format 0.0 to format {}; saved generation 2\nbackup: {}\n",
                    ew_api::FORMAT,
                    backup_of(&dir).display()
                ),
                String::new()
            )
        );
        let after = upgrade_files(&dir);
        let changed: Vec<&str> = after
            .iter()
            .filter(|(name, bytes)| before.get(*name) != Some(*bytes))
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(changed, ["p.ew.pre-0.0.ewz", "p.ew/manifest.json"]);
        let none = (
            0,
            format!("the project needs no upgrade to format {}\n", ew_api::FORMAT),
            String::new(),
        );
        assert_eq!(
            upgrade(&project, upgrade_store(&dir), (true, false), false, ""),
            none
        );
        assert_eq!(
            upgrade(&project, upgrade_store(&dir), (false, true), false, ""),
            none
        );
        assert_eq!(upgrade_files(&dir), after);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// With a released list that lacks this build's format, the save of `ew upgrade --yes`
    /// marks the manifest `unstable` and prints the warning on stderr, the exit code unchanged; with the format
    /// released, neither.
    #[test]
    fn a_save_in_an_unreleased_format_warns_on_stderr() {
        static OLDER: [&str; 1] = ["0.0"];
        static THIS: [&str; 1] = [ew_api::FORMAT];
        for (released, marked) in [(&OLDER[..], true), (&THIS[..], false)] {
            let dir = setup_v0("upgrade-unreleased");
            let project = dir.join("p.ew");
            let mut store = upgrade_store(&dir);
            store.released = released;
            let (code, output, errors) = upgrade(&project, store, (false, true), false, "");
            assert_eq!(
                (code, output),
                (
                    0,
                    format!(
                        "upgraded the project from format 0.0 to format {}; saved generation 2\nbackup: {}\n",
                        ew_api::FORMAT,
                        backup_of(&dir).display()
                    )
                )
            );
            let warning = "warning: saved in a format that is not released yet; released versions of ElektraWatt \
                           open this project read-only\n";
            assert_eq!(errors, if marked { warning } else { "" }, "{released:?}");
            let manifest = std::fs::read_to_string(project.join("manifest.json")).unwrap();
            assert_eq!(manifest.contains("\"unstable\": true"), marked, "{manifest}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// With a released list that lacks this build's format, `ew fmt` rewriting a page with CRLF
    /// line ends and `ew fmt --repair` discarding a broken line each mark the manifest `unstable` and print the warning
    /// on stderr after their save; with the format released, neither.
    #[test]
    fn fmt_and_its_repair_in_an_unreleased_format_warn_on_stderr() {
        static OLDER: [&str; 1] = ["0.0"];
        static THIS: [&str; 1] = [ew_api::FORMAT];
        let warning = "warning: saved in a format that is not released yet; released versions of ElektraWatt open \
                       this project read-only\n";
        for (released, marked) in [(&OLDER[..], true), (&THIS[..], false)] {
            for repair in [false, true] {
                let dir = setup(if repair {
                    "repair-unreleased"
                } else {
                    "fmt-unreleased"
                });
                let project = dir.join("p.ew");
                let page = std::fs::read_dir(project.join("pages"))
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .find(|p| p.extension().is_some_and(|x| x == "jsonl"))
                    .expect("a page file");
                let text = std::fs::read_to_string(&page).unwrap();
                let edited = if repair {
                    format!("{text}{{\"$k\":\"page\",\"id\":\n")
                } else {
                    text.replace('\n', "\r\n")
                };
                std::fs::write(&page, edited).unwrap();
                let args = FmtArgs {
                    project: Some(project.clone()),
                    check: false,
                    repair,
                    json: false,
                };
                let mut options = store(&dir);
                options.released = released;
                options.release_build = false;
                let (mut out, mut err) = (Vec::new(), Vec::new());
                let code = run_fmt_with(&args, options, (&mut out, &mut err));
                let err = String::from_utf8(err).unwrap();
                // The repair's own re-open has this build's options: a release build (this format
                // released) opens an unstable project read-only, exit 2 with the repaired generation kept; a
                // development build opens it.
                let release_build = ew_api::StoreOptions::new(VERSION).release_build;
                let code_after = if repair && marked && release_build { 2 } else { 0 };
                assert_eq!(
                    (code, err.as_str()),
                    (code_after, if marked { warning } else { "" }),
                    "repair {repair}, {released:?}"
                );
                let manifest = std::fs::read_to_string(project.join("manifest.json")).unwrap();
                assert_eq!(manifest.contains("\"unstable\": true"), marked, "{manifest}");
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
    }

    /// A session on a project of an older format (opened migrated) adds page 3 and crashes. `ew upgrade`
    /// refuses the journal, so `ew recover --replay` is the way out that keeps it: off a terminal without `--yes` it
    /// exits 2 and a wrong word exits 1, the journal kept and offered again each time; `--yes` replays, writes the
    /// upgrade's backup and saves in this version's format. The consent comes before the replay, so a
    /// declined run leaves the journal byte for byte as the crash left it, its labels offered again; stderr is
    /// asserted whole.
    #[test]
    fn recover_replays_the_journal_of_an_older_project_after_the_upgrade_consent() {
        let dir = setup_v0("recover-older");
        let (mut s, opened) = ew_api::Session::open(
            &dir.join("p.ew"),
            upgrade_store(&dir),
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        assert_eq!(opened.migrated_from.as_deref(), Some("0.0"));
        s.execute(&page(3)).unwrap();
        drop(s);
        let before = upgrade_files(&dir);
        let run = |yes: bool, terminal: bool, answer: &str| {
            let args = RecoverArgs {
                project: Some(dir.join("p.ew")),
                action: RecoverAction::Replay,
                json: false,
                yes,
            };
            let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
            let (mut output, mut errors) = (Vec::new(), Vec::new());
            let code = run_recover_with(
                &args,
                terminal,
                &mut input,
                (&mut output, &mut errors),
                upgrade_store(&dir),
            );
            (
                code,
                String::from_utf8(output).unwrap(),
                String::from_utf8(errors).unwrap(),
            )
        };
        let summary = "a crashed session left 1 unsaved transaction in its journal:\n  1. page.create\nthe project is \
                       unchanged since that session last opened or saved it\n";
        let crashed = journals(&dir);
        assert_eq!(
            run(false, false, ""),
            (
                2,
                summary.to_owned(),
                "error: the project is in the older format 0.0: saving the replay converts it, and confirming that \
                 needs an interactive terminal (type `upgrade` there) or `ew recover --replay --yes`; nothing was \
                 saved and the journal is kept\n"
                    .to_owned()
            )
        );
        assert_eq!(upgrade_files(&dir), before, "nothing was saved");
        assert_eq!(journals(&dir), crashed, "the journal is as the crash left it");
        let prompt = format!(
            "The project is in the older format 0.0. Saving the replay converts it to format {}, which older versions \
             of ElektraWatt cannot open, after a backup beside the project folder. Type `upgrade` to replay and \
             upgrade it, anything else to abort: ",
            ew_api::FORMAT
        );
        assert_eq!(
            run(false, true, "yes\n"),
            (
                1,
                format!("{summary}{prompt}"),
                "error: not confirmed; nothing was saved and the journal is kept\n".to_owned()
            )
        );
        assert_eq!(upgrade_files(&dir), before, "nothing was saved");
        assert_eq!(journals(&dir), crashed, "the journal is as the crash left it");
        let (code, shown, errors) = run(true, false, "");
        assert_eq!(
            (code, shown, errors),
            (
                0,
                format!(
                    "{summary}replayed 1 transaction of the crashed session's journal; saved generation 2\nupgraded the \
                     project from format 0.0 to format {}; backup: {}\n",
                    ew_api::FORMAT,
                    backup_of(&dir).display()
                ),
                String::new()
            )
        );
        assert!(backup_of(&dir).is_file());
        assert_eq!(saved_folios(&dir), ["1", "2", "3"]);
        assert!(journals(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `--check` on an `.ewz` of an older format opens it through the injected chain and exits 1;
    /// the backup the upgrade wrote holds the project in format 0.0. Nothing is written. The way names `ew extract`
    /// first, since `ew upgrade` refuses an `.ewz`.
    #[test]
    fn upgrade_check_exits_1_on_an_older_ewz() {
        let dir = setup_v0("upgrade-check-ewz");
        assert_eq!(
            upgrade(&dir.join("p.ew"), upgrade_store(&dir), (false, true), false, "").0,
            0
        );
        let before = upgrade_files(&dir);
        assert_eq!(
            upgrade(&backup_of(&dir), upgrade_store(&dir), (true, false), false, ""),
            (
                1,
                format!(
                    "the project is in the older format 0.0: `ew extract` makes a project folder of the archive, and \
                     `ew upgrade` converts that folder to format {} after a backup\n",
                    ew_api::FORMAT
                ),
                String::new()
            )
        );
        assert_eq!(upgrade_files(&dir), before);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
