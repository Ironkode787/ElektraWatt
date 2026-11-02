// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew fmt` and `ew recover` (WP-S1.22 part 3, D9 and D10), and `ew fmt --repair` (WP-S1.17, decision log ID-1185):
//! the subcommands that work on the project's files and journals as the store keeps them.

use std::path::Path;

use ew_api::cli_results as results;
use ew_api::fmt::{Backup, DiscardedLine, FmtError, KeptLine, RepairError, Repaired, Rewrite};

use super::{
    FmtArgs, RecoverAction, RecoverArgs, journal_refusal, open_editing, plural, printable, project_dir,
    read_only_words, reader_dir, typed_word,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The word that confirms `ew recover --discard`.
const DISCARD_WORD: &str = "discard";

/// `ew fmt [--check]` (D9, FR-EXT-008): the files that are not in canonical form, listed (`--check`, a reader: no
/// lock, nothing written) or rewritten through the save protocol after the content check (`ew_api::fmt`). The lines the
/// open kept verbatim are listed on stderr in both modes.
pub(super) fn run_fmt(a: &FmtArgs) -> i32 {
    if a.repair {
        return run_repair(a);
    }
    // `--check` is a reader and takes an `.ewz` (WP-S1.17 D9); `fmt` writes and refuses one.
    let dir = if a.check {
        reader_dir(a.project.as_ref())
    } else {
        project_dir(a.project.as_ref())
    };
    let Some(dir) = dir else {
        return 2;
    };
    let opened = if a.check {
        ew_api::Session::open_reader(&dir, VERSION, "fmt").map_err(|e| {
            eprintln!("error: {}: {e}", dir.display());
            2
        })
    } else {
        let origin = ew_api::Origin::Cli {
            subcommand: "fmt".to_owned(),
            confirmed: false,
        };
        open_editing(
            &dir,
            origin,
            ew_api::StoreOptions::new(VERSION),
            ew_api::Recovery::Keep,
        )
        .and_then(|(session, report)| {
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
            })
        );
    }
    i32::from(a.check && !files.is_empty())
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

/// `ew fmt --repair` (WP-S1.17, decision log ID-1185): the refusals in ruling 3's order (an `.ewz`, a crashed session's
/// journal, any other finding that forces read-only, then the repair's own: a project that cannot be packed, the
/// backup's name or folder, the content check), each before anything is written; then the backup and the one save
/// (`ew_api::fmt::repair`), and the project opened again as an editor opens it (ruling 6): exit 0 when it opens editable
/// with no EW-CORE-005 finding, else 2, the new generation kept. The discarded lines and the backup go to stdout (stderr
/// with `--json`); the kept lines and the summary to stderr.
fn run_repair(a: &FmtArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let mut store = ew_api::StoreOptions::new(VERSION);
    store.repair = true;
    let (mut session, report) = match open_editing(&dir, fmt_origin(), store, ew_api::Recovery::Keep) {
        Ok(r) => r,
        Err(code) => return code,
    };
    if report.recovery.is_some() {
        eprintln!("{}", journal_refusal(&dir));
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
    // A repair's session holds no unsaved transaction: the clean close loses nothing.
    session.close();
    let (code, out, err) = repair_outcome(&dir, a.json, &kept, outcome, || reopen(&dir));
    print!("{out}");
    eprint!("{err}");
    code
}

/// What `ew fmt --repair` prints after `ew_api::fmt::repair`, and its exit code: `(code, stdout, stderr)`. `reopen`
/// opens the repaired project again (ID-1185 ruling 6) and runs only after a save. Once the backup exists, `--json`
/// prints the result on every way out (C47-p2#2): after a failed save with `save_error` (exit 2), after a re-open that
/// found something wrong with `reopen_finding` (exit 2); a refusal before anything is written prints no result. A save
/// that failed before its commit point leaves `discarded` and `files` empty and the generation before; one incomplete
/// after it has taken effect, so they say what the repair decided, beside the decided generation (C47-p3#1). The human
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
/// characters, control characters escaped), then the backup's path (ID-1185 ruling 7) with, when the backup holds files
/// edited outside EW since the last save, how many (ID-1186 ruling 3: "as found: N files edited outside EW").
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

/// Opens the repaired project again as an editor opens it (ID-1185 ruling 6) and closes it: `Err` with what is still
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

/// The refusal of a repair open that a finding other than a broken line made read-only (ID-1185 ruling 3): the first
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
        // A file changed between the repair's open and its backup (ID-1186 ruling 4): the project is as it was, and a
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

/// `ew recover [--replay|--discard]` (D10, FT-252): the journal of the crashed session with the latest change, one at
/// a time (ruling 2 of cycle 41). Without a flag its summary; `--replay` replays it and saves; `--discard` deletes it
/// after the word typed on a controlling terminal (without one: exit 2 before the project is opened).
pub(super) fn run_recover(a: &RecoverArgs) -> i32 {
    use std::io::IsTerminal;
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    run_recover_with(
        a,
        terminal,
        &mut input,
        &mut output,
        ew_api::StoreOptions::new(VERSION),
    )
}

/// [`run_recover`] with the terminal test, the reader, the writer (everything `ew recover` prints on stdout) and the
/// store options injected (unit tests; ruling 3 of cycle 41).
fn run_recover_with(
    a: &RecoverArgs,
    terminal: bool,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    store: ew_api::StoreOptions,
) -> i32 {
    if a.action == RecoverAction::Discard && !terminal {
        eprintln!(
            "error: HumanOnly: needs an interactive terminal (`recover --discard` is confirmed by typing \
             `{DISCARD_WORD}`; `ew recover` prints the summary without one); nothing was discarded"
        );
        return 2;
    }
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let recovery = match a.action {
        RecoverAction::Replay => ew_api::Recovery::Apply,
        RecoverAction::Summary | RecoverAction::Discard => ew_api::Recovery::Keep,
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "recover".to_owned(),
        confirmed: a.action == RecoverAction::Discard,
    };
    // Ruling 2 of cycle 41: `ew recover` acts only with the lock; a read-only open is refused with `ew apply`'s text.
    let (mut session, opened) = match open_editing(&dir, origin, store, recovery) {
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
    let say = |output: &mut dyn std::io::Write, text: &str| {
        if a.json {
            eprint!("{text}");
        } else {
            let _ = output.write_all(text.as_bytes());
        }
    };
    let Some(info) = opened.recovery else {
        say(output, "no journal of a crashed session: nothing to recover\n");
        session.close();
        if a.json {
            let _ = output.write_all(result(None, false, generation).as_bytes());
        }
        return 0;
    };
    match a.action {
        RecoverAction::Summary => {
            say(output, &summary(&info));
            say(
                output,
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
            say(output, &summary(&info));
            if let Some(refusal) = session.recovery_refusal() {
                let (code, text) = replay_refusal(refusal);
                eprintln!("{text}");
                // The store kept the journal; the clean close deletes only this session's own journal.
                session.close();
                if a.json {
                    let _ = output.write_all(result(Some(&info), false, generation).as_bytes());
                }
                return code;
            }
            match session.save_and_close() {
                Ok(saved) => {
                    say(
                        output,
                        &format!(
                            "replayed {} of the crashed session's journal; saved generation {}\n",
                            plural(info.transactions, "transaction", "transactions"),
                            saved.generation
                        ),
                    );
                    say(output, &further(info.further));
                    if a.json {
                        let _ = output.write_all(result(Some(&info), true, saved.generation).as_bytes());
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: the save failed: {e}");
                    if !matches!(e, ew_api::SaveError::Incomplete { .. }) {
                        // `save_and_close` dropped the session without its clean close.
                        eprintln!(
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
                eprintln!("error: not confirmed; nothing was discarded");
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

/// How many further crashed sessions' journals remain (ruling 2 of cycle 41); empty when none.
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

    /// Runs `ew recover` on the project with `terminal` and the typed `answer`: the exit code and its stdout.
    fn recover(dir: &Path, action: RecoverAction, json: bool, terminal: bool, answer: &str) -> (i32, String) {
        let args = RecoverArgs {
            project: Some(dir.join("p.ew")),
            action,
            json,
        };
        let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
        let mut output = Vec::new();
        let code = run_recover_with(&args, terminal, &mut input, &mut output, store(dir));
        (code, String::from_utf8(output).unwrap())
    }

    #[test]
    fn recover_and_fmt_parse_their_flags() {
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string());
        let recover = |project: Option<&str>, action, json| {
            Ok(Command::Recover(RecoverArgs {
                project: project.map(PathBuf::from),
                action,
                json,
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
            Err("error: unexpected argument '--yes' found\n\n  tip: to pass '--yes' as a value, use '-- --yes'\n\n\
                 Usage: ew recover [OPTIONS] [project-dir]\n\nFor more information, try '--help'.\n"
                .to_owned())
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
        // ID-1185 ruling 7: `--repair` with `--check` is refused by the parser, before any open.
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

    /// FM-4 and ruling 3 of cycle 41 ("`--discard` without a terminal exits 2 and changes nothing, with a terminal and
    /// a wrong word exits 1"; the injected terminal of `run_assets_purge_with`): the summary names the transactions;
    /// off a terminal nothing is opened; a wrong word aborts; `discard` deletes the summarised journal only (ruling 2:
    /// "`discard_recovery` removes **only the summarised segment**"), and the other is offered next.
    #[test]
    fn the_discard_needs_the_typed_word_and_deletes_one_journal() {
        let dir = setup("discard");
        crash(&dir, 5);
        // The second crash is the later change: it is summarised first.
        std::thread::sleep(std::time::Duration::from_millis(20));
        crash(&dir, 6);
        let before = journals(&dir);
        assert_eq!(before.len(), 2);

        let (code, shown) = recover(&dir, RecoverAction::Summary, false, false, "");
        assert_eq!(code, 0);
        assert_eq!(
            shown,
            "a crashed session left 1 unsaved transaction in its journal:\n  1. page.create\nthe project is unchanged \
             since that session last opened or saved it\n1 more journal of a crashed session remains; run ew recover \
             again\n`ew recover --replay` replays and saves it; `ew recover --discard` deletes it\n"
        );
        assert_eq!(journals(&dir), before, "the summary changes nothing");

        let (code, shown) = recover(&dir, RecoverAction::Discard, false, false, "discard\n");
        assert_eq!(
            (code, shown.as_str()),
            (2, ""),
            "off a terminal nothing is opened"
        );
        assert_eq!(journals(&dir), before);

        let prompt = "a crashed session left 1 unsaved transaction in its journal:\n  1. page.create\nthe project is \
                      unchanged since that session last opened or saved it\n1 more journal of a crashed session \
                      remains; run ew recover again\nDiscarding deletes the journal: its unsaved changes are lost and \
                      do not come back. Type `discard` to delete it, anything else to abort: ";
        for wrong in ["", "\n", "y\n", "yes\n", "Discard\n", "discard it\n"] {
            let (code, shown) = recover(&dir, RecoverAction::Discard, false, true, wrong);
            assert_eq!((code, shown.as_str()), (1, prompt), "{wrong:?} must abort");
            assert_eq!(journals(&dir), before, "{wrong:?}: nothing is discarded");
        }

        let (code, shown) = recover(&dir, RecoverAction::Discard, false, true, "  discard \r\n");
        assert_eq!(code, 0, "{shown}");
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
        let (code, shown) = recover(&dir, RecoverAction::Replay, false, false, "");
        assert_eq!(code, 0, "{shown}");
        assert_eq!(
            shown,
            "a crashed session left 1 unsaved transaction in its journal:\n  1. page.create\nthe project is unchanged \
             since that session last opened or saved it\nreplayed 1 transaction of the crashed session's journal; \
             saved generation 2\n"
        );
        assert_eq!(saved_folios(&dir), ["1", "2", "5"]);
        assert!(journals(&dir).is_empty());
        let (code, shown) = recover(&dir, RecoverAction::Summary, false, false, "");
        assert_eq!(
            (code, shown.as_str()),
            (0, "no journal of a crashed session: nothing to recover\n")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ruling 2 of cycle 41: "`ew recover` checks that it got edit access ... Then it refuses with exit 2 and the same
    /// text `ew apply` uses for a held lock; it never acts read-only." An editor holds the lock: every action is
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
        for action in [
            RecoverAction::Summary,
            RecoverAction::Replay,
            RecoverAction::Discard,
        ] {
            let (code, shown) = recover(&dir, action, false, true, "discard\n");
            assert_eq!((code, shown.as_str()), (2, ""), "{action:?}");
            assert_eq!(journals(&dir), before, "{action:?}");
        }
        editor.close();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ruling 1 of the cycle-41 review's fix round: "`journal_refusal` names the command that now exists and no FT
    /// number ... add a whole-text test for one subcommand of each kind that prints it (`ew apply`, `ew import
    /// parts`, a human-only subcommand)." The human-only kind: `project assets purge`, confirmed on the injected
    /// terminal (ruling 3 of cycle 41), opens through `open_for_writing_with` and refuses a project with a crashed
    /// session's journal before it asks: exit 2, nothing prompted, the journal unchanged. It prints
    /// `journal_refusal`'s text on stderr, which a unit test does not capture, so the text is asserted whole here;
    /// `ew apply` and `ew import parts` assert the same text through the binary (`tests/cli.rs`).
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
    /// further journals; the collisions' text is asserted through the binary (`tests/cli.rs`).
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

    /// C47-p2#2: once the backup exists, `--json` prints the result on every way out, with exit 2 where it is a failure,
    /// and the human text goes to stderr: a re-open that still finds something wrong (with `reopen_finding`), a save
    /// incomplete after its commit point (with `save_error`, and `discarded` and `files` as the repair decided them,
    /// C47-p3#1) and a save that failed before it (with `save_error`, `discarded` and `files` empty), each with and
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
        // decided (C47-p3#1), and failed before it, which left the generation as it was.
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

    /// WP-S1.17 ID-1186 rulings 3 and 4: the backup's line counts the files edited outside EW, without the clause when
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
}
