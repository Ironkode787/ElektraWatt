// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew fmt` and `ew recover` (WP-S1.22 part 3, D9 and D10): the subcommands that work on the project's files and
//! journals as the store keeps them.

use ew_api::cli_results as results;
use ew_api::fmt::{FmtError, KeptLine, Rewrite};

use super::{FmtArgs, RecoverAction, RecoverArgs, open_editing, plural, printable, project_dir, typed_word};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The word that confirms `ew recover --discard`.
const DISCARD_WORD: &str = "discard";

/// `ew fmt [--check]` (D9, FR-EXT-008): the files that are not in canonical form, listed (`--check`, a reader: no
/// lock, nothing written) or rewritten through the save protocol after the content check (`ew_api::fmt`). The lines the
/// open kept verbatim are listed on stderr in both modes.
pub(super) fn run_fmt(a: &FmtArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
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
                // A save would hide the crashed session's unsaved changes (doc 05 section 8.3).
                eprintln!("{}", fmt_journal_refusal(&dir));
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

/// The refusal of `ew fmt` while a crashed session's journal holds unsaved changes: `ew recover` comes first.
fn fmt_journal_refusal(dir: &std::path::Path) -> String {
    format!(
        "error: {}: a crashed session left unsaved changes in its journal; replay or discard them first with `ew \
         recover`; nothing was written",
        dir.display()
    )
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
        let dir = std::env::temp_dir().join(format!("ew-cli-recover-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("state")).unwrap();
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
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned()));
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
            Err("`--replay` and `--discard` cannot be combined".to_owned())
        );
        assert_eq!(
            parse(&["recover", "--discard", "--json"]),
            Err(
                "`--discard` asks for a typed word on a terminal and prints no result: it takes no `--json`"
                    .to_owned()
            )
        );
        assert_eq!(
            parse(&["recover", "--yes"]),
            Err("unknown option `--yes`".to_owned())
        );
        assert_eq!(
            parse(&["fmt", "p.ew", "--check"]),
            Ok(Command::Fmt(FmtArgs {
                project: Some(PathBuf::from("p.ew")),
                check: true,
                json: false,
            }))
        );
        assert_eq!(
            parse(&["fmt", "a", "b"]),
            Err("unexpected argument `b`".to_owned())
        );
        assert_eq!(
            parse(&["fmt", "a", "--project", "b"]),
            Err("`--project` cannot be combined with a positional project directory".to_owned())
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
}
