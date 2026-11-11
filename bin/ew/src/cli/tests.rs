// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of the command line as a whole: parsing, the refusals, the commands that share helpers.

/// "error: <dir>: the project opens read-only: <reason>; nothing was
/// written", each reason in the window's words; text from the system is printed escaped. The held lock is
/// `tests/it/cli.rs`'s `recover_and_apply_refuse_a_held_lock_with_the_same_text`.
#[test]
fn a_read_only_refusal_words_each_reason() {
    use ew_api::ReadOnlyReason as R;
    let dir = std::path::Path::new("p");
    for (reason, text) in [
        (
            R::Requested,
            "error: p: the project opens read-only: it was opened read-only on request; nothing was written",
        ),
        (
            R::NotWritable("Access is denied.\u{1b}[2J".into()),
            "error: p: the project opens read-only: the project folder cannot be written (Access is \
                 denied.\\u{1b}[2J); nothing was written",
        ),
        (
            R::Integrity,
            "error: p: the project opens read-only: it has integrity findings; `ew check` lists them; nothing \
                 was written",
        ),
        (
            R::Format("written by a newer major format"),
            "error: p: the project opens read-only: it was written by a newer major format; nothing was written",
        ),
        (
            R::Format("written by a development build with an unreleased format"),
            "error: p: the project opens read-only: it was written by a development build with an unreleased \
                 format; nothing was written",
        ),
        (
            R::LockLost(None),
            "error: p: the project opens read-only: the lock was taken over; nothing was written",
        ),
    ] {
        assert_eq!(super::read_only_refusal(dir, &reason), text);
    }
}

/// The Unicode bidirectional controls print escaped, like the control characters; other text is kept.
#[test]
fn printable_escapes_the_bidirectional_controls() {
    assert_eq!(super::printable("a\u{202E}b"), "a\\u{202e}b");
    assert_eq!(
        super::printable("\u{061C}\u{200E}\u{200F}\u{202A}\u{2066}\u{2069}x"),
        "\\u{61c}\\u{200e}\\u{200f}\\u{202a}\\u{2066}\\u{2069}x"
    );
    assert_eq!(super::printable("Jürgen \u{1b}"), "Jürgen \\u{1b}");
}

#[test]
fn index_warning_prints_the_index_error_only_when_there_is_one() {
    let mut w = ew_api::library::WriteReport::default();
    assert_eq!(super::index_warning(&w), None);
    w.index_error = Some("disk full".into());
    let text = super::index_warning(&w).expect("a warning");
    assert!(text.starts_with("warning: "), "{text}");
    assert!(text.contains("disk full"), "{text}");
}

/// FIX40a (finding 8): the release run's record is kept per user, so the help names it as the person's own.
#[test]
fn the_help_names_the_release_record_as_the_users() {
    let mut cmd = super::command();
    cmd.build();
    let release = cmd
        .find_subcommand_mut("revision")
        .and_then(|r| r.find_subcommand_mut("release"))
        .expect("`revision release`");
    let help = release.render_long_help().to_string();
    let text = help.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        text.contains("`discard` is offered only when your record on this computer shows"),
        "the help text"
    );
    assert!(!text.contains("this computer's record"), "the help text");
}

use super::*;

/// The command of a line, or clap's rendering of its error.
fn p(args: &[&str]) -> Result<Command, String> {
    parse(args.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string())
}

/// clap's whole rendering of a usage error of the subcommand whose usage line is `usage`.
fn usage_error(message: &str, usage: &str) -> Result<Command, String> {
    Err(format!(
        "error: {message}\n\nUsage: {usage}\n\nFor more information, try '--help'.\n"
    ))
}

/// clap's whole rendering of a value that the flag's value parser refuses with `message`.
fn invalid(value: &str, flag: &str, message: &str) -> Result<Command, String> {
    Err(format!(
        "error: invalid value '{value}' for '{flag}': {message}\n\nFor more information, try '--help'.\n"
    ))
}

/// `ew print` lowers its flags, the platform's default adapter resolved (`lp` on Linux, `windows` on
/// Windows, else the viewer; ruling 4 of ID-1218); `--out` excludes `--via` and `--printer`; `--sections` reaches
/// `export png` too.
#[test]
fn print_lowers_its_flags_and_resolves_the_adapter() {
    use ew_api::output::print::Paper;
    #[cfg(windows)]
    let default = PrintVia::Windows;
    #[cfg(not(windows))]
    let default = if cfg!(target_os = "linux") {
        PrintVia::Lp
    } else {
        PrintVia::Viewer
    };
    assert_eq!(
        p(&[
            "print",
            "proj",
            "--sections",
            "1,2",
            "--paper",
            "A5",
            "--fit",
            "--mono",
            "--out",
            "p.pdf"
        ]),
        Ok(Command::Print(PrintArgs {
            project: Some(PathBuf::from("proj")),
            pages: PageSelection::Sections(vec!["1".to_owned(), "2".to_owned()]),
            paper: Paper::A5,
            fit: true,
            mono: true,
            printer: None,
            via: default,
            out: Some(PathBuf::from("p.pdf")),
            json: false,
        }))
    );
    let Ok(Command::Print(a)) = p(&["print", "--via", "lp", "--printer", "office"]) else {
        panic!("`--via lp --printer`");
    };
    assert_eq!(
        (a.via, a.printer.as_deref(), a.paper, a.pages),
        (PrintVia::Lp, Some("office"), Paper::Sheet, PageSelection::All)
    );
    let printer = p(&["print", "--printer", "office"]);
    if cfg!(any(target_os = "linux", windows)) {
        assert!(matches!(printer, Ok(Command::Print(_))));
    } else {
        assert_eq!(
            printer,
            usage_error(
                "`--printer` needs `--via lp` or `--via windows`; with the viewer, choose the printer in its print \
                     dialog",
                "ew print [OPTIONS] [project-dir]"
            )
        );
    }
    assert_eq!(
        p(&["print", "--via", "viewer", "--printer", "office"]),
        usage_error(
            "`--printer` needs `--via lp` or `--via windows`; with the viewer, choose the printer in its print \
                 dialog",
            "ew print [OPTIONS] [project-dir]"
        )
    );
    // Ruling 4 of ID-1218: `--via windows` with or without a printer on Windows, refused in words elsewhere.
    let windows = p(&["print", "--via", "windows", "--printer", "office"]);
    #[cfg(windows)]
    {
        let Ok(Command::Print(a)) = windows else {
            panic!("`--via windows --printer`: {windows:?}");
        };
        assert_eq!((a.via, a.printer.as_deref()), (PrintVia::Windows, Some("office")));
    }
    #[cfg(not(windows))]
    assert_eq!(
        windows,
        usage_error(
            "`--via windows` prints on Windows only",
            "ew print [OPTIONS] [project-dir]"
        )
    );
    assert_eq!(
        p(&["print", "--out", "p.pdf", "--via", "lp"]),
        usage_error(
            "the argument '--out <file>' cannot be used with '--via <adapter>'",
            "ew print --out <file> [project-dir]"
        )
    );
    let Ok(Command::ExportPages(png)) = p(&["export", "png", "--out", "d", "--sections", "9"]) else {
        panic!("`export png --sections`");
    };
    assert_eq!(png.pages, PageSelection::Sections(vec!["9".to_owned()]));
    assert_eq!(
        p(&["export", "svg", "--out", "d", "--sections", "1,,2"]),
        invalid(
            "1,,2",
            "--sections <numbers>",
            "page selection: `1,,2` has an empty item (section numbers separated by `,`)"
        )
    );
}

/// What `ew print` says when a spooler took the set: through `lp`, or the Windows print spooler (ruling 4 of
/// ID-1218), to the printer named or the default one.
#[test]
fn a_spooled_print_names_its_printer_and_its_way() {
    assert_eq!(
        super::spooled_line(4, None, "lp"),
        "4 pages sent to the default printer through `lp`"
    );
    assert_eq!(
        super::spooled_line(5, Some("Microsoft Print to PDF"), "windows"),
        "5 pages sent to printer Microsoft Print to PDF through the Windows print spooler"
    );
}

/// Cycle 43, ruling 4: a line without its subcommand prints the help on stderr and exits 2; `--help` and
/// `--version` print on stdout and exit 0, the version as `ew <version>`.
#[test]
fn help_and_version_are_clap_s() {
    let e = |args: &[&str]| parse(args.iter().map(|s| (*s).to_owned())).unwrap_err();
    for args in [&[][..], &["lib"], &["import"], &["project", "checks"]] {
        let err = e(args);
        assert_eq!(
            err.kind(),
            clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
            "{args:?}"
        );
        assert_eq!(err.exit_code(), 2, "{args:?}");
    }
    for args in [
        &["--help"][..],
        &["-h"],
        &["help"],
        &["export", "pdf", "--help"],
        &["help", "lib", "search"],
    ] {
        let err = e(args);
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp, "{args:?}");
        assert_eq!(err.exit_code(), 0, "{args:?}");
    }
    for args in [&["--version"][..], &["-V"]] {
        let err = e(args);
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion, "{args:?}");
        assert_eq!(err.exit_code(), 0, "{args:?}");
        assert_eq!(
            err.render().to_string(),
            format!("ew {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
    assert_eq!(
        p(&["frobnicate"]),
        usage_error("unrecognized subcommand 'frobnicate'", "ew <COMMAND>")
    );
}

/// A positional that takes hyphen values (`query`, `lib search`, `lib create`, `lib import parts`) takes
/// a word starting with one `-`, but refuses one starting with `--` as a mistyped option (exit 2), as the old
/// parser did.
#[test]
fn a_double_dash_word_in_a_hyphen_positional_is_a_mistyped_option() {
    let e = |args: &[&str]| parse(args.iter().map(|s| (*s).to_owned())).unwrap_err();
    for (args, value, positional) in [
        (&["lib", "search", "--alll"][..], "--alll", "<text>"),
        (
            &["query", "device_tag", "tag:-101F1", "--explian"],
            "--explian",
            "[key]",
        ),
        (&["query", "n", "k", "--bogus"], "--bogus", "[key]"),
        (
            &[
                "lib",
                "create",
                "--frob",
                "--key",
                "k",
                "--name",
                "n",
                "--licence",
                "MIT",
            ],
            "--frob",
            "<dir>",
        ),
        (
            &[
                "lib",
                "import",
                "parts",
                "--lib",
                "t.csv",
                "--map",
                "m.toml",
                "--as-of",
                "2026-01-01",
            ],
            "--lib",
            "<library>",
        ),
    ] {
        assert_eq!(
            p(args),
            invalid(value, positional, &format!("unknown option `{value}`")),
            "{args:?}"
        );
        assert_eq!(e(args).exit_code(), 2, "{args:?}");
    }
    let Ok(Command::Lib(l)) = p(&["lib", "search", "-X1"]) else {
        panic!("a search text starting with one `-`");
    };
    let LibAction::Search { text, .. } = l.action else {
        panic!("`lib search`");
    };
    assert_eq!(text, "-X1");
}

/// Decision log ID-1273: `merge-driver`'s help quotes doc 05 §11.1's three `.gitattributes` lines in full, as
/// `ew git setup` will write them, and says its exits.
#[test]
fn merge_driver_s_help_quotes_the_attribute_lines_and_its_exits() {
    let cmd = command();
    let help = cmd
        .find_subcommand("merge-driver")
        .and_then(clap::Command::get_long_about)
        .map(ToString::to_string);
    assert_eq!(
        help.as_deref(),
        Some(
            "Merge one project file three-way: git's merge driver (plumbing).\n\nGit runs it for a file changed on \
                 both sides, with `%O` (the ancestor), `%A` (ours, where the result goes), `%B` (theirs), `%L` (the \
                 marker size, unused: conflicts are written as records, not markers) and `%P` (the file's path in the \
                 repository); the older form `%O %A %B %P` gives the same result. `%P` is taken relative to the \
                 deepest folder above it that holds a `manifest.json` in the current directory, the working tree, and \
                 selects the merge: a record file per record, a library document, `manifest.json`; any other file \
                 keeps ours. The three files are read under the reader's limits, and `%A` is replaced whole through a \
                 temporary file beside it. Git takes a driver only from its own configuration, so it is registered by \
                 two lines, `git config merge.elektrawatt.name \"ElektraWatt record merge\"` and `git config \
                 merge.elektrawatt.driver \"ew merge-driver %O %A %B %L %P\"`, and selected by three lines of the \
                 project's `.gitattributes`, `*.jsonl text eol=lf merge=elektrawatt diff=elektrawatt`, \
                 `lib/**/*.json text eol=lf merge=elektrawatt diff=elektrawatt` and `manifest.json text eol=lf \
                 merge=elektrawatt` (`eol=lf` keeps the manifest's hashes true under `core.autocrlf`). Exit 0 when \
                 merged cleanly, 1 when conflict records were written or a side or the merged file is larger than \
                 the reader's limits (`%A` then as it was), 129 on an internal error (a file that cannot be read or \
                 written, a failure in the merge) or a usage error, with `%A` as it was: git reads every code up to \
                 128 as conflicts written."
        )
    );
}

/// Decision log ID-1273 (ADR-0012 amendment 2): a usage error of `merge-driver` exits 129, which git reads as a
/// failure rather than as conflicts written; a usage error of every other command keeps 2, the help 0.
#[test]
fn a_usage_error_of_merge_driver_exits_129_and_of_another_command_2() {
    let code = |args: &[&str]| {
        let args: Vec<std::ffi::OsString> = args.iter().map(Into::into).collect();
        let e = parse(args.iter().cloned()).expect_err("a usage error or the help");
        error_exit_code(&args, &e)
    };
    assert_eq!(code(&["merge-driver", "o", "a", "b"]), 129);
    assert_eq!(code(&["merge-driver"]), 129);
    assert_eq!(code(&["merge-driver", "--help"]), 0);
    assert_eq!(code(&["schema"]), 2);
    assert_eq!(code(&["check", "--frob"]), 2);
    assert_eq!(code(&["frobnicate", "merge-driver"]), 2);
}

/// Decision log ID-1273: `merge-driver`'s positionals take a leading `-` (a `%P` under a folder such as `-old.ew`),
/// in both forms; three arguments are refused naming `<P>`, the argument the usage line shows as required.
#[test]
fn merge_driver_takes_a_leading_hyphen_and_names_a_missing_p() {
    let driver = |path: &str| {
        Ok(Command::MergeDriver(merge_driver::MergeDriverArgs {
            base: PathBuf::from("-o"),
            ours: PathBuf::from("-a"),
            theirs: PathBuf::from("-b"),
            path: path.to_owned(),
        }))
    };
    assert_eq!(
        p(&["merge-driver", "-o", "-a", "-b", "7", "-old.ew/pages/x.jsonl"]),
        driver("-old.ew/pages/x.jsonl")
    );
    assert_eq!(
        p(&["merge-driver", "-o", "-a", "-b", "-old.ew/manifest.json"]),
        driver("-old.ew/manifest.json")
    );
    assert_eq!(
        p(&["merge-driver", "o", "a", "b"]),
        usage_error(
            "the following required arguments were not provided:\n  <P>",
            "ew merge-driver <O> <A> <B> [L] <P>"
        )
    );
}

/// Cycle 43, ruling 3: a valued flag takes the next word even when it starts with `-`; a second `--project` is an
/// error, `--company` accumulates, another flag is last-wins; a bare `--` ends the options.
#[test]
fn the_grammar_of_values_repeats_and_the_option_end() {
    let Ok(Command::Export(t)) = p(&["export", "terminals", "--strip", "--json"]) else {
        panic!("a value starting with `-`");
    };
    assert_eq!(
        (t.kind, t.json),
        (ExportKind::Terminals(Some("--json".to_owned())), false)
    );
    assert_eq!(
        p(&["fmt", "--project", "a", "--project", "b"]),
        usage_error(
            "the argument '--project <dir>' cannot be used multiple times",
            "ew fmt [OPTIONS] [project-dir]"
        )
    );
    let Ok(Command::Check(c)) = p(&["check", "--out", "a", "--out", "b", "--json", "--json"]) else {
        panic!("last-wins");
    };
    assert_eq!((c.out, c.format), (Some(PathBuf::from("b")), CheckFormat::Json));
    let Ok(Command::Check(c)) = p(&["check", "--lang", "NL-nl"]) else {
        panic!("--lang in any case");
    };
    assert_eq!(c.lang, "nl-NL");
    let Ok(Command::Fmt(f)) = p(&["fmt", "--", "--check"]) else {
        panic!("`--` ends the options");
    };
    assert_eq!((f.project, f.check), (Some(PathBuf::from("--check")), false));
    let Ok(Command::Query(q)) = p(&["query", "device_tag", "-X1031:5", "--json"]) else {
        panic!("a key starting with `-`");
    };
    assert_eq!((q.key.as_str(), q.json), ("-X1031:5", true));
    let Ok(Command::Lib(l)) = p(&["lib", "search", "-X1", "--company", "a", "--company", "b"]) else {
        panic!("a search text starting with `-`");
    };
    assert_eq!(l.company, [PathBuf::from("a"), PathBuf::from("b")]);
    let Ok(Command::Query(q)) = p(&["query", "proj", "device_tag", "-"]) else {
        panic!("`-` for a query without a key");
    };
    assert_eq!((q.project, q.key.as_str()), (Some(PathBuf::from("proj")), "-"));
    // `import csv --strict` names the default: the run reads only `lenient`.
    let Ok(Command::ImportCsv(strict)) = p(&["import", "csv", "t", "--out", "o", "--strict"]) else {
        panic!("`--strict` is the explicit default");
    };
    assert_eq!((strict.strict, strict.lenient), (true, false));
    assert_eq!(
        Ok(Command::ImportCsv(ImportCsvArgs {
            strict: false,
            ..strict
        })),
        p(&["import", "csv", "t", "--out", "o"])
    );
    assert_eq!(
        p(&["import", "csv", "t", "--out", "o", "--strict", "--lenient"]),
        usage_error(
            "the argument '--strict' cannot be used with '--lenient'",
            "ew import csv --out <dir> --strict <tables-dir>"
        )
    );
    // The refused `--lang` takes no flag as its value: `--lang --out o` is still the `--lang` refusal.
    for args in [
        &["export", "pdf", "--out", "o", "--lang", "de"][..],
        &["export", "pdf", "--lang", "--out", "o"],
    ] {
        assert_eq!(
            p(args),
            usage_error(
                "`--lang` is not built yet",
                "ew export pdf [OPTIONS] --out <path> [project-dir]"
            ),
            "{args:?}"
        );
    }
    assert_eq!(
        p(&["export", "png", "--out", "o", "--omit-personal-data"]),
        usage_error(
            "`--omit-personal-data` is not built yet",
            "ew export png [OPTIONS] --out <dir> [project-dir]"
        )
    );
    assert_eq!(
        p(&["import", "qet", "a.qet", "--json", "--report", "md"]),
        usage_error(
            "`--json` prints the result on stdout: give the report a file with `--report-out <file>`",
            "ew import qet [OPTIONS] <file.qet>"
        )
    );
    assert_eq!(
        p(&["check", "--json", "--format", "md"]),
        usage_error(
            "`--json` cannot be combined with `--format md` (`--json` is `--format json`)",
            "ew check [OPTIONS] [project-dir]"
        )
    );
    assert_eq!(
        p(&["sql", "--project", "p", "--file", "q", "proj"]),
        usage_error(
            "the argument '[project-dir]' cannot be used with '--project <dir>'",
            "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       \
                 ew sql <COMMAND>"
        )
    );
    assert_eq!(
        p(&["sql", "SELECT 1", "--bogus"]),
        invalid("--bogus", "[select]", "unknown option `--bogus`")
    );
}

#[test]
fn lib_commands() {
    let lib = |action, company: &[&str], json| {
        Ok(Command::Lib(LibArgs {
            action,
            company: company.iter().map(PathBuf::from).collect(),
            json,
        }))
    };
    assert_eq!(p(&["lib", "list"]), lib(LibAction::List, &[], false));
    assert_eq!(
        p(&["lib", "list", "--company", "a", "--json", "--company", "b"]),
        lib(LibAction::List, &["a", "b"], true)
    );
    assert_eq!(
        p(&["lib", "search", "A9P52610"]),
        lib(
            LibAction::Search {
                text: "A9P52610".into(),
                kinds: Vec::new(),
                all: false,
                limit: 50
            },
            &[],
            false
        )
    );
    assert_eq!(
        p(&[
            "lib", "search", "--kind", "symbol", "mcb", "--kind", "fndef", "--all", "--limit", "0", "--json"
        ]),
        lib(
            LibAction::Search {
                text: "mcb".into(),
                kinds: vec!["symbol".into(), "fndef".into()],
                all: true,
                limit: 0
            },
            &[],
            true
        )
    );
    assert_eq!(
        p(&["lib", "index", "--rebuild"]),
        lib(LibAction::IndexRebuild, &[], false)
    );
    assert_eq!(
        p(&[
            "lib",
            "create",
            "d",
            "--key",
            "k",
            "--name",
            "N",
            "--licence",
            "CC0-1.0",
            "--company",
            "c"
        ]),
        lib(
            LibAction::Create {
                dir: PathBuf::from("d"),
                key: "k".into(),
                name: "N".into(),
                licence: "CC0-1.0".into()
            },
            &["c"],
            false
        )
    );
    let create = "ew lib create --key <key> --name <name> --licence <spdx> <dir>";
    for (args, error) in [
            (
                &["lib", "frob"][..],
                usage_error("unrecognized subcommand 'frob'", "ew lib [OPTIONS] <COMMAND>"),
            ),
            (
                &["lib", "list", "x"],
                usage_error("unexpected argument 'x' found", "ew lib list [OPTIONS]"),
            ),
            (
                &["lib", "search"],
                usage_error(
                    "the following required arguments were not provided:\n  <text>",
                    "ew lib search <text>",
                ),
            ),
            (
                &["lib", "search", "a", "b"],
                usage_error("unexpected argument 'b' found", "ew lib search [OPTIONS] <text>"),
            ),
            (
                &["lib", "search", "a", "--kind", "device"],
                invalid(
                    "device",
                    "--kind <kind>",
                    "unknown kind `device` (part, symbol, fndef, form, macro, manufacturer)",
                ),
            ),
            (
                &["lib", "search", "a", "--limit", "x"],
                invalid("x", "--limit <n>", "`--limit` needs a number, not `x`"),
            ),
            (
                &["lib", "index"],
                usage_error(
                    "the following required arguments were not provided:\n  --rebuild",
                    "ew lib index --rebuild",
                ),
            ),
            (
                &["lib", "create", "d", "--key", "k", "--name", "N"],
                usage_error(
                    "the following required arguments were not provided:\n  --licence <spdx>",
                    create,
                ),
            ),
            (
                &["lib", "create", "--key", "k"],
                usage_error(
                    "the following required arguments were not provided:\n  --name <name>\n  --licence <spdx>\n  <dir>",
                    create,
                ),
            ),
            (
                &["lib", "list", "--company"],
                Err("error: a value is required for '--company <dir>' but none was supplied\n\nFor more information, \
                     try '--help'.\n"
                    .to_owned()),
            ),
        ] {
            assert_eq!(p(args), error, "{args:?}");
        }
}

#[test]
fn import_qet_options() {
    let Ok(Command::ImportQet(a)) = p(&[
        "import",
        "qet",
        "a.qet",
        "--content",
        "c",
        "--report",
        "md",
        "--report-out",
        "r.md",
    ]) else {
        panic!("parse failed");
    };
    assert_eq!(a.file, PathBuf::from("a.qet"));
    assert_eq!(a.content, Some(PathBuf::from("c")));
    assert_eq!(a.report, Some(ReportFormat::Markdown));
    assert_eq!(a.report_out, Some(PathBuf::from("r.md")));
    let Ok(Command::ImportQet(b)) = p(&["import", "qet", "--report", "json", "x.qet"]) else {
        panic!("parse failed");
    };
    assert_eq!(b.report, Some(ReportFormat::Json));
    assert_eq!(b.content, None);
    assert!(!b.strict);
    let Ok(Command::ImportQet(c)) = p(&["import", "qet", "--strict", "x.qet"]) else {
        panic!("parse failed");
    };
    assert!(c.strict);
}

#[test]
fn check_defaults_options_and_errors() {
    let Ok(Command::Check(a)) = p(&["check"]) else {
        panic!("parse failed");
    };
    assert_eq!(a.project, None);
    assert_eq!(a.scheme, None);
    assert_eq!(a.mode, CheckMode::Release);
    assert_eq!(a.format, CheckFormat::Text);
    assert_eq!(a.out, None);
    let Ok(Command::Check(b)) = p(&[
        "check", "proj", "--scheme", "drafting", "--mode", "online", "--format", "json", "--out", "r.json",
    ]) else {
        panic!("parse failed");
    };
    assert_eq!(b.project, Some(PathBuf::from("proj")));
    assert_eq!(b.scheme.as_deref(), Some("drafting"));
    assert_eq!(b.mode, CheckMode::Online);
    assert_eq!(b.format, CheckFormat::Json);
    assert_eq!(b.out, Some(PathBuf::from("r.json")));
    let Ok(Command::Check(c)) = p(&["check", "--format", "md", "proj"]) else {
        panic!("parse failed");
    };
    assert_eq!(c.format, CheckFormat::Markdown);
    assert!(p(&["check", "--scheme", "bogus"]).is_err());
    assert!(p(&["check", "--mode", "x"]).is_err());
    assert!(p(&["check", "--format", "xml"]).is_err());
    assert!(p(&["check", "a", "b"]).is_err());
    assert!(p(&["check", "--bogus"]).is_err());
    assert!(p(&["check", "--scheme"]).is_err());
}

fn sql(args: &[&str]) -> SqlArgs {
    let mut v = vec!["sql"];
    v.extend_from_slice(args);
    match p(&v) {
        Ok(Command::Sql(a)) => a,
        other => panic!("parse failed: {other:?}"),
    }
}

#[test]
fn sql_defaults_and_positionals() {
    let a = sql(&["SELECT 1"]);
    assert_eq!(a.project, None);
    assert_eq!(a.query, SqlSource::Text("SELECT 1".to_owned()));
    assert_eq!(a.format, SqlFormat::Csv);
    assert_eq!(a.timeout, None);
    assert_eq!(a.out, None);
    let b = sql(&[
        "proj",
        "SELECT 1",
        "--format",
        "json",
        "--timeout",
        "5",
        "--out",
        "r.json",
    ]);
    assert_eq!(b.project, Some(PathBuf::from("proj")));
    assert_eq!(b.query, SqlSource::Text("SELECT 1".to_owned()));
    assert_eq!(b.format, SqlFormat::Json);
    assert_eq!(b.timeout, Some(5));
    assert_eq!(b.out, Some(PathBuf::from("r.json")));
    let c = sql(&["--file", "q.sql"]);
    assert_eq!(c.project, None);
    assert_eq!(c.query, SqlSource::File(PathBuf::from("q.sql")));
    let d = sql(&["proj", "--file", "q.sql"]);
    assert_eq!(d.project, Some(PathBuf::from("proj")));
    let e = sql(&["--project", "proj", "SELECT 1"]);
    assert_eq!(e.project, Some(PathBuf::from("proj")));
    assert_eq!(e.query, SqlSource::Text("SELECT 1".to_owned()));
    let f = sql(&["--file", "q.sql", "--project", "proj"]);
    assert_eq!(f.project, Some(PathBuf::from("proj")));
}

#[test]
fn sql_dump_and_schema_forms() {
    assert_eq!(
        p(&["sql", "dump", "o.sqlite"]),
        Ok(Command::SqlDump(SqlDumpArgs {
            project: None,
            out: PathBuf::from("o.sqlite"),
            json: false,
        }))
    );
    assert_eq!(
        p(&["sql", "dump", "proj", "o.sqlite"]),
        Ok(Command::SqlDump(SqlDumpArgs {
            project: Some(PathBuf::from("proj")),
            out: PathBuf::from("o.sqlite"),
            json: false,
        }))
    );
    assert_eq!(
        p(&["sql", "dump", "--project", "proj", "o.sqlite"]),
        Ok(Command::SqlDump(SqlDumpArgs {
            project: Some(PathBuf::from("proj")),
            out: PathBuf::from("o.sqlite"),
            json: false,
        }))
    );
    assert_eq!(p(&["sql", "schema"]), Ok(Command::SqlSchema));
}

#[test]
fn sql_errors() {
    // `--format xlsx` writes a file, and the CSV options apply to CSV only.
    assert_eq!(
        p(&["sql", "SELECT 1", "--format", "xlsx"]),
        usage_error(
            "`--format xlsx` writes a file: give `--out <file.xlsx>`",
            "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       \
                 ew sql <COMMAND>"
        )
    );
    assert_eq!(
        p(&["sql", "SELECT 1", "--json", "--preset", "excel-nl"]),
        usage_error(
            "`--preset` applies to CSV only; drop it, or give `--format csv`",
            "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       \
                 ew sql <COMMAND>"
        )
    );
    assert_eq!(
        p(&[
            "sql",
            "SELECT 1",
            "--format",
            "xlsx",
            "--out",
            "q.xlsx",
            "--line-end",
            "lf"
        ]),
        usage_error(
            "`--line-end` applies to CSV only; drop it, or give `--format csv`",
            "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       \
                 ew sql <COMMAND>"
        )
    );
    assert_eq!(
        p(&["sql", "SELECT 1", "--format", "x"]),
        invalid("x", "--format <format>", "unknown format `x` (csv, json, xlsx)")
    );
    assert!(p(&["sql", "SELECT 1", "--timeout", "0"]).is_err());
    assert!(p(&["sql", "SELECT 1", "--timeout", "abc"]).is_err());
    assert!(p(&["sql", "SELECT 1", "--timeout", "-1"]).is_err());
    assert!(p(&["sql", "SELECT 1", "--timeout"]).is_err());
    // The timeout is capped at one day: no overflow of the clock.
    assert!(p(&["sql", "SELECT 1", "--timeout", "18446744073709551615"]).is_err());
    assert!(p(&["sql", "SELECT 1", "--timeout", "86401"]).is_err());
    assert!(matches!(
        p(&["sql", "SELECT 1", "--timeout", "86400"]),
        Ok(Command::Sql(SqlArgs {
            timeout: Some(86_400),
            ..
        }))
    ));
    // An inline query may start with an SQL comment.
    let Ok(Command::Sql(c)) = p(&[
        "sql",
        "-- count
SELECT 1",
    ]) else {
        panic!("a query starting with a comment is not an option");
    };
    assert_eq!(
        c.query,
        SqlSource::Text(
            "-- count
SELECT 1"
                .to_owned()
        )
    );
    assert!(
        p(&[
            "sql",
            "proj",
            "-- c
SELECT 1"
        ])
        .is_ok()
    );
    assert!(p(&["sql", "proj", "SELECT 1", "--file", "q.sql"]).is_err());
    assert!(p(&["sql", "a", "b", "c"]).is_err());
    assert!(p(&["sql"]).is_err());
    assert!(p(&["sql", "schema", "extra"]).is_err());
    assert!(p(&["sql", "dump"]).is_err());
    assert!(p(&["sql", "dump", "a", "b", "c"]).is_err());
    assert!(p(&["sql", "dump", "a", "--bogus"]).is_err());
    assert!(p(&["sql", "--project", "p", "proj", "SELECT 1"]).is_err());
    assert!(p(&["sql", "--project", "p", "proj", "--file", "q.sql"]).is_err());
    assert!(p(&["sql", "dump", "--project", "p", "a", "b"]).is_err());
}

#[test]
fn import_parts_options_and_errors() {
    let Ok(Command::ImportParts(a)) = p(&["import", "parts", "a.csv"]) else {
        panic!("import parts");
    };
    assert_eq!(a.file, PathBuf::from("a.csv"));
    assert_eq!(a.project, None);
    let Ok(Command::ImportParts(b)) = p(&["import", "parts", "--project", "d", "a.csv"]) else {
        panic!("import parts --project");
    };
    assert_eq!(b.project, Some(PathBuf::from("d")));
    assert!(p(&["import", "parts"]).is_err());
    assert!(p(&["import", "parts", "a", "b"]).is_err());
    assert!(p(&["import", "parts", "a", "--bogus"]).is_err());
    assert!(p(&["import", "parts", "a", "--project"]).is_err());
}

/// `import parts --map` and `lib import parts`.
#[test]
fn mapped_parts_imports() {
    let Ok(Command::ImportParts(a)) = p(&[
        "import",
        "parts",
        "list.xlsx",
        "--map",
        "m.toml",
        "--as-of",
        "2026-10-29",
        "--company",
        "c",
    ]) else {
        panic!("import parts --map");
    };
    assert_eq!(a.map, Some(PathBuf::from("m.toml")));
    assert_eq!(a.as_of, Some("2026-10-29".parse().unwrap()));
    assert_eq!(a.company, [PathBuf::from("c")]);
    let Ok(Command::ImportParts(b)) = p(&["import", "parts", "parts.csv"]) else {
        panic!("import parts");
    };
    assert_eq!((b.map, b.as_of, b.company), (None, None, Vec::new()));
    assert_eq!(
        p(&[
            "lib",
            "import",
            "parts",
            "std",
            "list.csv",
            "--map",
            "m.toml",
            "--as-of",
            "2026-10-29",
            "--bump",
            "--take-over",
            "--company",
            "c"
        ]),
        Ok(Command::Lib(LibArgs {
            action: LibAction::ImportParts {
                library: "std".into(),
                file: PathBuf::from("list.csv"),
                map: PathBuf::from("m.toml"),
                as_of: "2026-10-29".parse().unwrap(),
                bump: true,
                take_over: true,
            },
            company: vec![PathBuf::from("c")],
            json: false,
        }))
    );
    let lib_import = "ew lib import parts --map <map.toml> --as-of <date> <library> <table>";
    let missing = "the following required arguments were not provided:";
    for (args, error) in [
            (
                &["import", "parts", "a.csv", "--as-of", "2026-10-29"][..],
                usage_error(
                    &format!("{missing}\n  --map <map.toml>"),
                    "ew import parts --map <map.toml> --as-of <date> <table>",
                ),
            ),
            (
                &["import", "parts", "a.csv", "--map", "m", "--as-of", "29.10.2026"],
                invalid(
                    "29.10.2026",
                    "--as-of <date>",
                    "`--as-of` needs a date YYYY-MM-DD, not `29.10.2026`",
                ),
            ),
            (
                &["lib", "import", "macros"],
                Err("error: unrecognized subcommand 'macros'\n\n  tip: a similar subcommand exists: 'parts'\n\nUsage: \
                     ew lib import [OPTIONS] <COMMAND>\n\nFor more information, try '--help'.\n"
                    .to_owned()),
            ),
            (
                &["lib", "import", "parts", "std"],
                usage_error(
                    &format!("{missing}\n  --map <map.toml>\n  --as-of <date>\n  <table>"),
                    lib_import,
                ),
            ),
            (
                &["lib", "import", "parts", "std", "a.csv", "--as-of", "2026-10-29"],
                usage_error(&format!("{missing}\n  --map <map.toml>"), lib_import),
            ),
            (
                &["lib", "import", "parts", "std", "a.csv", "--map", "m"],
                usage_error(&format!("{missing}\n  --as-of <date>"), lib_import),
            ),
            (
                &["lib", "import", "parts", "std", "a.csv", "b.csv"],
                usage_error(
                    "unexpected argument 'b.csv' found",
                    "ew lib import parts [OPTIONS] --map <map.toml> --as-of <date> <library> <table>",
                ),
            ),
            (
                &["lib", "list", "--bump"],
                usage_error("unexpected argument '--bump' found", "ew lib list [OPTIONS]"),
            ),
        ] {
            assert_eq!(p(args), error, "{args:?}");
        }
}

#[test]
fn import_qet_errors() {
    assert!(p(&["import"]).is_err());
    assert!(p(&["import", "dxf", "a"]).is_err());
    assert!(p(&["import", "qet"]).is_err());
    assert!(p(&["import", "qet", "a", "b"]).is_err());
    assert!(p(&["import", "qet", "a", "--bogus"]).is_err());
    assert!(p(&["import", "qet", "a", "--report"]).is_err());
    assert!(p(&["import", "qet", "a", "--report", "xml"]).is_err());
}
#[test]
fn export_forms() {
    let Ok(Command::Export(a)) = p(&["export", "cables"]) else {
        panic!("parse failed");
    };
    assert_eq!(a.kind, ExportKind::Cables);
    assert_eq!((a.project, a.out), (None, None));
    let Ok(Command::Export(b)) = p(&["export", "cables", "proj", "--format", "csv", "--out", "c.csv"]) else {
        panic!("parse failed");
    };
    assert_eq!(b.project, Some(PathBuf::from("proj")));
    assert_eq!(b.out, Some(PathBuf::from("c.csv")));
    // The parts list is the default of `export bom`; `component` is the doc 17 name of `parts`.
    for (args, mode) in [
        (&["export", "bom"][..], BomMode::Component),
        (&["export", "bom", "--mode", "parts"][..], BomMode::Component),
        (&["export", "bom", "--mode", "component"][..], BomMode::Component),
        (
            &["export", "bom", "proj", "--mode", "assembly"][..],
            BomMode::Assembly,
        ),
    ] {
        let Ok(Command::Export(c)) = p(args) else {
            panic!("{args:?}: parse failed");
        };
        assert_eq!(c.kind, ExportKind::Bom(mode), "{args:?}");
    }
}

#[test]
fn export_errors() {
    assert!(p(&["export"]).is_err());
    assert!(p(&["export", "nets"]).is_err());
    assert!(p(&["export", "cables", "a", "b"]).is_err());
    assert!(p(&["export", "cables", "--bogus"]).is_err());
    assert!(p(&["export", "cables", "--out"]).is_err());
    // `--mode` belongs to `bom`; the other formats are not built.
    assert!(p(&["export", "cables", "--mode", "parts"]).is_err());
    assert!(p(&["export", "bom", "--mode", "everything"]).is_err());
    assert!(p(&["export", "bom", "--mode"]).is_err());
    // `--strip` belongs to `terminals`, `--block` needs a value, `report` needs a block and PDF a file.
    assert!(p(&["export", "wires", "--strip", "-X1"]).is_err());
    assert!(p(&["export", "wires", "--block"]).is_err());
    assert!(p(&["export", "report"]).is_err());
    assert!(p(&["export", "report", "--block", "908", "--format", "pdf"]).is_err());
    assert!(p(&["export", "report", "--block", "908", "--format", "svg"]).is_err());
    // `--format xlsx` needs `--out` and takes no CSV option; the value refusals name the choices.
    for what in ["bom", "cables", "wires", "terminals"] {
        let usage = format!("ew export {what} [OPTIONS] [project-dir]");
        assert_eq!(
            p(&["export", what, "--format", "xlsx"]),
            usage_error("`--format xlsx` writes a file: give `--out <file.xlsx>`", &usage)
        );
        assert_eq!(
            p(&[
                "export", what, "--format", "xlsx", "--out", "x.xlsx", "--preset", "excel-nl"
            ]),
            usage_error(
                "`--preset` applies to CSV only; drop it, or give `--format csv`",
                &usage
            )
        );
        assert_eq!(
            p(&[
                "export", what, "--format", "xlsx", "--out", "x.xlsx", "--bom", "on"
            ]),
            usage_error(
                "`--bom` applies to CSV only; drop it, or give `--format csv`",
                &usage
            )
        );
        assert_eq!(
            p(&["export", what, "--preset", "excel"]),
            invalid(
                "excel",
                "--preset <preset>",
                "unknown preset `excel` (ew, excel-nl, excel-en, rfc4180)"
            )
        );
        assert_eq!(
            p(&["export", what, "--bom", "yes"]),
            invalid(
                "yes",
                "--bom <on|off>",
                "unknown byte-order mark setting `yes` (on, off)"
            )
        );
        assert_eq!(
            p(&["export", what, "--line-end", "cr"]),
            invalid("cr", "--line-end <crlf|lf>", "unknown line end `cr` (crlf, lf)")
        );
        assert_eq!(
            p(&["export", what, "--format", "tsv"]),
            invalid("tsv", "--format <format>", "unknown format `tsv` (csv, xlsx)")
        );
    }
    let report = "ew export report [OPTIONS] <--block <block>|--kind <kind>> [project-dir]";
    assert_eq!(
        p(&["export", "report", "--block", "908", "--format", "xlsx"]),
        usage_error("`--format xlsx` writes a file: give `--out <file.xlsx>`", report)
    );
    assert_eq!(
        p(&[
            "export", "report", "--block", "908", "--format", "pdf", "--out", "r.pdf", "--preset", "rfc4180",
        ]),
        usage_error(
            "`--preset` applies to CSV only; drop it, or give `--format csv`",
            report
        )
    );
    assert_eq!(
        p(&["export", "report", "--block", "908", "--format", "svg"]),
        invalid(
            "svg",
            "--format <format>",
            "unknown format `svg` (csv, xlsx, pdf)"
        )
    );
    // Cycle-40 review, part 3, finding 5: the row exports write rows, and the refusal points to the report PDF.
    for what in ["bom", "cables", "wires", "terminals"] {
        assert_eq!(
            p(&["export", what, "--format", "pdf"]),
            usage_error(
                &format!(
                    "`export {what}` writes rows as CSV or XLSX; a report block's pages go to PDF with `ew export report --block <block> --format pdf`"
                ),
                &format!("ew export {what} [OPTIONS] [project-dir]")
            )
        );
    }
}

/// `--format xlsx` with `--out`, `--preset`, `--bom` and `--line-end` lower to the row format;
/// the default is the golden dialect.
#[test]
fn row_export_formats() {
    use ew_api::output::{CsvOptions, LineEnd, Preset};
    let Ok(Command::Export(a)) = p(&["export", "wires", "--format", "xlsx", "--out", "w.xlsx"]) else {
        panic!("parse failed");
    };
    assert_eq!(
        (a.format, a.out),
        (RowFormat::Xlsx, Some(PathBuf::from("w.xlsx")))
    );
    let Ok(Command::Export(a)) = p(&[
        "export",
        "bom",
        "--preset",
        "excel-nl",
        "--bom",
        "off",
        "--line-end",
        "lf",
    ]) else {
        panic!("parse failed");
    };
    assert_eq!(
        a.format,
        RowFormat::Csv(CsvOptions {
            preset: Preset::ExcelNl,
            bom: Some(false),
            line_end: Some(LineEnd::Lf),
        })
    );
    let Ok(Command::Export(a)) = p(&[
        "export", "report", "--kind", "io_list", "--format", "xlsx", "--out", "k.xlsx",
    ]) else {
        panic!("parse failed");
    };
    assert_eq!(
        (a.kind, a.format),
        (ExportKind::ReportKind("io_list".to_owned()), RowFormat::Xlsx)
    );
    let Ok(Command::Export(a)) = p(&["export", "terminals"]) else {
        panic!("parse failed");
    };
    assert_eq!(a.format, RowFormat::default());
    let Ok(Command::Sql(q)) = p(&["sql", "SELECT 1", "--format", "xlsx", "--out", "q.xlsx"]) else {
        panic!("parse failed");
    };
    assert_eq!((q.format, q.csv), (SqlFormat::Xlsx, CsvOptions::default()));
    let Ok(Command::Sql(q)) = p(&["sql", "SELECT 1", "--preset", "rfc4180"]) else {
        panic!("parse failed");
    };
    assert_eq!(q.csv, CsvOptions::preset(Preset::Rfc4180));
}

/// D8: `wires`, `terminals [--strip]`, `report --block … --format csv|pdf` and `--block` on the row exports.
#[test]
fn row_export_forms() {
    let Ok(Command::Export(a)) = p(&["export", "wires", "proj", "--block", "907"]) else {
        panic!("parse failed");
    };
    assert_eq!(
        (a.kind, a.block, a.project),
        (
            ExportKind::Wires,
            Some("907".to_owned()),
            Some(PathBuf::from("proj"))
        )
    );
    let Ok(Command::Export(t)) = p(&["export", "terminals", "--strip", "-X1031"]) else {
        panic!("parse failed");
    };
    assert_eq!(t.kind, ExportKind::Terminals(Some("-X1031".to_owned())));
    let Ok(Command::Export(t)) = p(&["export", "terminals"]) else {
        panic!("parse failed");
    };
    assert_eq!((t.kind, t.block), (ExportKind::Terminals(None), None));
    let Ok(Command::Export(b)) = p(&["export", "bom", "--block", "x", "--mode", "assembly"]) else {
        panic!("parse failed");
    };
    assert_eq!(
        (b.kind, b.block),
        (ExportKind::Bom(BomMode::Assembly), Some("x".to_owned()))
    );
    let Ok(Command::Export(r)) = p(&["export", "report", "--block", "908"]) else {
        panic!("parse failed");
    };
    assert_eq!(r.kind, ExportKind::Report { pdf: false });
    let Ok(Command::Export(r)) = p(&[
        "export", "report", "--block", "908", "--format", "pdf", "--out", "r.pdf",
    ]) else {
        panic!("parse failed");
    };
    assert_eq!(
        (r.kind, r.out),
        (ExportKind::Report { pdf: true }, Some(PathBuf::from("r.pdf")))
    );
}

/// WP-S3.06 D4 (the orchestrator's): `export report --kind <kind>` is CSV only and excludes
/// `--block`; each refusal is the report layer's text, whole, given before the project opens.
#[test]
fn report_kind_forms_and_refusals() {
    let Ok(Command::Export(r)) = p(&[
        "export",
        "report",
        "proj",
        "--kind",
        "device_list",
        "--format",
        "csv",
    ]) else {
        panic!("parse failed");
    };
    assert_eq!(
        (r.kind, r.block, r.project),
        (
            ExportKind::ReportKind("device_list".to_owned()),
            None,
            Some(PathBuf::from("proj"))
        )
    );
    let report = "ew export report <--block <block>|--kind <kind>> [project-dir]";
    // WP-S3.06 D4's refusal, before the pages refusal.
    for args in [
        &["export", "report", "--kind", "device_list", "--block", "910"][..],
        &[
            "export",
            "report",
            "--block",
            "910",
            "--kind",
            "device_list",
            "--format",
            "pdf",
            "--out",
            "r.pdf",
        ],
    ] {
        assert_eq!(
            p(args),
            usage_error(
                "give `--block` or `--kind`, not both",
                "ew export report [OPTIONS] <--block <block>|--kind <kind>> [project-dir]"
            ),
            "{args:?}"
        );
    }
    assert_eq!(
        p(&[
            "export",
            "report",
            "--kind",
            "device_list",
            "--format",
            "pdf",
            "--out",
            "r.pdf"
        ]),
        usage_error(
            "`--kind` exports rows: a kind has pages only through a block, use `--block <block> --format pdf`",
            "ew export report [OPTIONS] <--block <block>|--kind <kind>> [project-dir]"
        )
    );
    assert_eq!(
        p(&["export", "report"]),
        usage_error(
            "the following required arguments were not provided:\n  <--block <block>|--kind <kind>>",
            report
        )
    );
    assert_eq!(
            p(&["export", "wires", "--kind", "device_list"]),
            Err("error: unexpected argument '--kind' found\n\n  tip: to pass '--kind' as a value, use '-- --kind'\n\n\
                 Usage: ew export wires [OPTIONS] [project-dir]\n\nFor more information, try '--help'.\n"
                .to_owned())
        );
}

/// D7: `render --page <folio> --png [--dpi] [--out]` and `export png --dpi`.
#[test]
fn render_and_png_forms() {
    assert_eq!(
        p(&["render", "proj", "--page", "103", "--png"]),
        Ok(Command::Render(RenderArgs {
            project: Some(PathBuf::from("proj")),
            page: "103".to_owned(),
            dpi: 150,
            out: None,
            json: false,
        }))
    );
    let Ok(Command::Render(r)) = p(&[
        "render", "--page", "2.1", "--png", "--dpi", "300", "--out", "x.png", "--json",
    ]) else {
        panic!("parse failed");
    };
    assert_eq!((r.dpi, r.out, r.json), (300, Some(PathBuf::from("x.png")), true));
    assert!(p(&["render", "--page", "103"]).is_err(), "--png is required");
    assert!(p(&["render", "--png"]).is_err(), "--page is required");
    for bad in ["71", "601", "x", "0"] {
        assert!(
            p(&["render", "--page", "1", "--png", "--dpi", bad]).is_err(),
            "{bad}"
        );
    }
    assert!(p(&["render", "--page", "1", "--png", "--dpi", "72"]).is_ok());
    assert!(p(&["render", "--page", "1", "--png", "--dpi", "600"]).is_ok());
    let Ok(Command::ExportPages(e)) = p(&["export", "png", "--out", "o", "--dpi", "300", "--threads", "4"])
    else {
        panic!("parse failed");
    };
    assert_eq!((e.format, e.dpi, e.threads), (PageFormat::Png, 300, Some(4)));
    assert!(p(&["export", "png"]).is_err(), "--out is required");
    // `--dpi` is the raster's: PDF and SVG refuse it.
    assert!(p(&["export", "svg", "--out", "o", "--dpi", "300"]).is_err());
    assert!(p(&["export", "pdf", "--out", "o", "--dpi", "300"]).is_err());
}

#[test]
fn export_pages_forms_and_errors() {
    let Ok(Command::ExportPages(a)) = p(&["export", "pdf", "proj", "--out", "o.pdf"]) else {
        panic!("parse failed");
    };
    assert_eq!(a.format, PageFormat::Pdf);
    assert_eq!(a.project, Some(PathBuf::from("proj")));
    assert_eq!(a.out, PathBuf::from("o.pdf"));
    assert_eq!((a.pages, a.threads), (PageSelection::All, None));
    let Ok(Command::ExportPages(b)) = p(&[
        "export",
        "svg",
        "--out",
        "svg",
        "--pages",
        "101-106,1",
        "--threads",
        "4",
    ]) else {
        panic!("parse failed");
    };
    assert_eq!(b.format, PageFormat::Svg);
    assert_eq!(b.project, None);
    assert_eq!(
        b.pages,
        PageSelection::Folios(vec!["101-106".to_owned(), "1".to_owned()])
    );
    assert_eq!(b.threads, Some(4));
    assert!(p(&["export", "pdf"]).is_err(), "--out is required");
    assert!(p(&["export", "svg", "proj"]).is_err());
    assert!(p(&["export", "pdf", "--out"]).is_err());
    assert!(p(&["export", "pdf", "--out", "o", "--pages", ""]).is_err());
    assert!(p(&["export", "pdf", "--out", "o", "--threads", "0"]).is_err());
    assert!(p(&["export", "pdf", "--out", "o", "--threads", "x"]).is_err());
    assert!(p(&["export", "pdf", "--out", "o", "a", "b"]).is_err());
    assert!(p(&["export", "png", "--out", "o", "--dpi", "1000"]).is_err());
}

#[test]
fn apply_forms() {
    assert_eq!(
        p(&["apply", "proj", "x.ewcmd", "--preview"]),
        Ok(Command::Apply(ApplyArgs {
            project: Some(PathBuf::from("proj")),
            log: PathBuf::from("x.ewcmd"),
            preview: true,
            one_step: false,
            json: false,
        }))
    );
    assert_eq!(
        p(&["apply", "--one-step", "x.ewcmd"]),
        Ok(Command::Apply(ApplyArgs {
            project: None,
            log: PathBuf::from("x.ewcmd"),
            preview: false,
            one_step: true,
            json: false,
        }))
    );
    assert!(p(&["apply"]).is_err());
    assert!(p(&["apply", "a", "b", "c"]).is_err());
    assert!(p(&["apply", "x.ewcmd", "--bogus"]).is_err());
}

#[test]
fn verify_derived_forms() {
    assert_eq!(
        p(&["verify-derived"]),
        Ok(Command::VerifyDerived(VerifyArgs {
            project: None,
            json: false
        }))
    );
    assert_eq!(
        p(&["verify-derived", "proj"]),
        Ok(Command::VerifyDerived(VerifyArgs {
            project: Some(PathBuf::from("proj")),
            json: false,
        }))
    );
    assert!(p(&["verify-derived", "a", "b"]).is_err());
    assert!(p(&["verify-derived", "--bogus"]).is_err());
}

#[test]
fn layout_forms() {
    assert_eq!(
        p(&["layout"]),
        Ok(Command::Layout(layout::LayoutArgs {
            project: None,
            page: None,
            json: false,
        }))
    );
    assert_eq!(
        p(&["layout", "proj", "--page", "101", "--json"]),
        Ok(Command::Layout(layout::LayoutArgs {
            project: Some(PathBuf::from("proj")),
            page: Some("101".to_owned()),
            json: true,
        }))
    );
    assert_eq!(
        p(&["layout", "--project", "proj", "--page", "-1"]),
        Ok(Command::Layout(layout::LayoutArgs {
            project: Some(PathBuf::from("proj")),
            page: Some("-1".to_owned()),
            json: false,
        }))
    );
    assert!(p(&["layout", "a", "b"]).is_err());
    assert!(p(&["layout", "a", "--project", "b"]).is_err());
    assert!(p(&["layout", "--page"]).is_err());
}
