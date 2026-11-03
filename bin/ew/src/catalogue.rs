// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CLI table (WP-S1.22 D4): one entry per subcommand path of the derive tree ([`crate::cli::command`]) with its
//! flags, its exit codes, its result schema and its stability.
//!
//! The paths and the flags (name, value, repetition) come from walking clap's tree in declaration order, skipping
//! clap's own help and version; the exit codes, the result and the stability are [`TABLE`], keyed by path. The tests
//! below tie the table to the tree (they name the same paths) and to the published result schemas
//! ([`ew_api::cli_results::RESULTS`]). It is the source of the CLI snapshot and the CLI reference.

/// One flag of a subcommand.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Flag {
    /// The flag, `--name`.
    pub name: String,
    /// Whether it takes a value (`--out <file>`).
    pub value: bool,
    /// Whether it may be given more than once (`--company <dir>`...).
    pub repeatable: bool,
}

/// One exit code of a subcommand and what it means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Exit {
    /// The code.
    pub code: u8,
    /// Its meaning for this subcommand.
    pub meaning: &'static str,
}

/// The stability of a surface (doc 17 §11): every surface is Preview before 1.0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stability {
    /// May change before 1.0 with a release-note line.
    Preview,
}

/// The hand-kept part of one subcommand path: what the tree cannot say.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Row {
    /// The words after `ew` (`["export", "pdf"]`).
    pub path: &'static [&'static str],
    /// Its exit codes.
    pub exits: &'static [Exit],
    /// The schema name of its `--json` result (`ew.<command>-result/1`), or `None` when it prints none.
    pub result: Option<&'static str>,
    /// Its stability.
    pub stability: Stability,
}

/// One subcommand path of the CLI: its words and flags from the tree, the rest from its [`Row`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Entry {
    /// The words after `ew`.
    pub path: Vec<String>,
    /// Its flags, in declaration order.
    pub flags: Vec<Flag>,
    /// Its exit codes.
    pub exits: &'static [Exit],
    /// The schema name of its `--json` result, or `None` when it prints none.
    pub result: Option<&'static str>,
    /// Its stability.
    pub stability: Stability,
}

const fn exit(code: u8, meaning: &'static str) -> Exit {
    Exit { code, meaning }
}

const OK: Exit = exit(0, "success");
const USAGE_ETC: Exit = exit(2, "usage, open, I/O and every other error");
const ROW_EXPORT_ERRORS: Exit = exit(
    2,
    "usage and open errors, a project without the block, an unknown or wrong block",
);
const HUMAN_EXITS: &[Exit] = &[
    exit(0, "confirmed, done and saved"),
    exit(1, "refused or not confirmed"),
    exit(2, "usage, no controlling terminal, open and save errors"),
];

/// The exits, results and stability of every subcommand path of the tree.
pub const TABLE: &[Row] = &[
    Row {
        path: &["import", "qet"],
        exits: &[
            exit(0, "imported, even with findings"),
            exit(
                1,
                "the model check failed, or `--strict` with lost connections or error findings",
            ),
            USAGE_ETC,
        ],
        result: Some(ew_api::cli_results::IMPORT_QET),
        stability: Stability::Preview,
    },
    Row {
        path: &["import", "parts"],
        exits: &[exit(0, "imported and saved"), USAGE_ETC],
        result: Some(ew_api::cli_results::IMPORT_PARTS),
        stability: Stability::Preview,
    },
    Row {
        path: &["import", "csv"],
        exits: &[
            exit(0, "written, with its findings"),
            exit(
                1,
                "nothing written: `--strict` refused error findings, or the records do not make a project",
            ),
            exit(
                2,
                "usage, starter content, an unreadable table, an `--out` that holds something, write errors",
            ),
        ],
        result: Some(ew_api::cli_results::IMPORT_CSV),
        stability: Stability::Preview,
    },
    Row {
        path: &["import", "table"],
        exits: &[
            exit(0, "previewed, and with `--apply` applied and saved"),
            exit(
                2,
                "usage, open, read and save errors, a file without a table or of another project",
            ),
        ],
        result: Some(ew_api::cli_results::IMPORT_TABLE),
        stability: Stability::Preview,
    },
    Row {
        path: &["check"],
        exits: &[
            exit(0, "no unwaived error"),
            exit(1, "unwaived errors"),
            exit(2, "usage and open errors"),
        ],
        result: Some(ew_api::cli_results::CHECK),
        stability: Stability::Preview,
    },
    Row {
        path: &["sql"],
        exits: &[OK, exit(2, "usage, open and SQL errors, denials and limits")],
        result: Some(ew_api::cli_results::SQL),
        stability: Stability::Preview,
    },
    Row {
        path: &["sql", "dump"],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::SQL_DUMP),
        stability: Stability::Preview,
    },
    Row {
        path: &["sql", "schema"],
        exits: &[OK],
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "cables"],
        exits: &[OK, ROW_EXPORT_ERRORS],
        result: Some(ew_api::cli_results::EXPORT_CABLES),
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "bom"],
        exits: &[OK, ROW_EXPORT_ERRORS],
        result: Some(ew_api::cli_results::EXPORT_BOM),
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "wires"],
        exits: &[OK, ROW_EXPORT_ERRORS],
        result: Some(ew_api::cli_results::EXPORT_WIRES),
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "terminals"],
        exits: &[
            OK,
            exit(
                2,
                "usage and open errors, a project without the block, an unknown or wrong block, an unknown strip",
            ),
        ],
        result: Some(ew_api::cli_results::EXPORT_TERMINALS),
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "report"],
        exits: &[
            OK,
            exit(
                2,
                "usage, open, I/O and writer errors, an unknown block, a block without rows or pages",
            ),
        ],
        result: Some(ew_api::cli_results::EXPORT_REPORT),
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "table"],
        exits: &[
            OK,
            exit(
                2,
                "usage, open, I/O and writer errors, an unknown navigator or strip",
            ),
        ],
        result: Some(ew_api::cli_results::EXPORT_TABLE),
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "pdf"],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::EXPORT_PDF),
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "svg"],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::EXPORT_SVG),
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "png"],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::EXPORT_PNG),
        stability: Stability::Preview,
    },
    Row {
        path: &["export", "ewz"],
        exits: &[
            exit(0, "written"),
            exit(
                2,
                "usage, open and write errors, a project changed on disk or one that cannot be packed",
            ),
        ],
        result: Some(ew_api::cli_results::EXPORT_EWZ),
        stability: Stability::Preview,
    },
    Row {
        path: &["extract"],
        exits: &[
            exit(0, "extracted"),
            exit(
                2,
                "usage, an unreadable or refused archive, an existing folder, write errors",
            ),
        ],
        result: Some(ew_api::cli_results::EXTRACT),
        stability: Stability::Preview,
    },
    Row {
        path: &["print"],
        exits: &[
            exit(0, "printed, opened in the viewer or written"),
            exit(
                2,
                "usage, open, I/O and writer errors, an unknown folio or section, a refusal of the adapter",
            ),
        ],
        result: Some(ew_api::cli_results::PRINT),
        stability: Stability::Preview,
    },
    Row {
        path: &["render"],
        exits: &[
            OK,
            exit(2, "usage, open, I/O and writer errors, an unknown folio"),
        ],
        result: Some(ew_api::cli_results::RENDER),
        stability: Stability::Preview,
    },
    Row {
        path: &["apply"],
        exits: &[
            exit(0, "every line commits"),
            exit(1, "a prevent or domain rule refuses a line"),
            exit(
                2,
                "usage, a malformed log, an unresolvable reference, a refused command, open, lock and save errors",
            ),
        ],
        result: Some(ew_api::cli_results::APPLY),
        stability: Stability::Preview,
    },
    Row {
        path: &["fmt"],
        exits: &[
            exit(
                0,
                "every file is canonical, or the files were rewritten and saved; repaired, or nothing to repair",
            ),
            exit(1, "`--check` found files to rewrite"),
            exit(
                2,
                "usage, open, lock, a crashed session's journal, a content change and save errors; a repair refused \
                 before writing, its save failed, or its re-open found a finding (the new generation and the backup \
                 stay)",
            ),
        ],
        result: Some(ew_api::cli_results::FMT),
        stability: Stability::Preview,
    },
    Row {
        path: &["recover"],
        exits: &[
            exit(0, "summarised, replayed and saved, discarded, or no journal"),
            exit(1, "the replay refused or the discard not confirmed"),
            exit(2, "usage, no controlling terminal, lock, open and save errors"),
        ],
        result: Some(ew_api::cli_results::RECOVER),
        stability: Stability::Preview,
    },
    Row {
        path: &["upgrade"],
        exits: &[
            exit(0, "upgraded, backed up and saved, or nothing to upgrade"),
            exit(
                1,
                "`--check` found an upgrade needed, or the upgrade not confirmed",
            ),
            exit(
                2,
                "usage, no controlling terminal without `--yes`, lock, a crashed session's journal, open, backup and \
                 save errors",
            ),
        ],
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["project", "checks", "set"],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["project", "assets", "purge"],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["revision", "approve"],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["revision", "release"],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["revision", "withdraw"],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["verify-derived"],
        exits: &[
            exit(0, "no difference"),
            exit(1, "differences"),
            exit(2, "usage and open errors"),
        ],
        result: Some(ew_api::cli_results::VERIFY_DERIVED),
        stability: Stability::Preview,
    },
    Row {
        path: &["query"],
        exits: &[
            exit(0, "answered"),
            exit(
                2,
                "usage, an unknown query, open errors, a key that names no record or one of another kind",
            ),
        ],
        result: Some(ew_api::cli_results::QUERY),
        stability: Stability::Preview,
    },
    Row {
        path: &["queries"],
        exits: &[OK],
        result: Some(ew_api::cli_results::QUERIES),
        stability: Stability::Preview,
    },
    Row {
        path: &["lib", "list"],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_LIST),
        stability: Stability::Preview,
    },
    Row {
        path: &["lib", "search"],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_SEARCH),
        stability: Stability::Preview,
    },
    Row {
        path: &["lib", "index"],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_INDEX),
        stability: Stability::Preview,
    },
    Row {
        path: &["lib", "create"],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_CREATE),
        stability: Stability::Preview,
    },
    Row {
        path: &["lib", "import", "parts"],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_IMPORT_PARTS),
        stability: Stability::Preview,
    },
    Row {
        path: &["commands"],
        exits: &[OK],
        result: Some(ew_api::cli_results::COMMANDS),
        stability: Stability::Preview,
    },
    Row {
        path: &["schema"],
        exits: &[OK, exit(2, "usage, an unknown schema name")],
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["merge", "resolve"],
        exits: &[
            exit(0, "resolved and saved"),
            exit(
                1,
                "`merge.resolve` refused: theirs does not make a valid record, a file's lines do not read",
            ),
            exit(
                2,
                "usage (no conflict record selected), open, lock, a crashed session's journal and save errors",
            ),
        ],
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["merge-driver"],
        exits: &[
            exit(0, "merged cleanly into `%A`"),
            exit(
                1,
                "conflict records written, or a side or the merged file above the reader's limits (`%A` as it was)",
            ),
            exit(
                129,
                "an internal error or a usage error: a file that cannot be read or written, a failure in the merge, \
                 clap's usage text (`%A` as it was)",
            ),
        ],
        result: None,
        stability: Stability::Preview,
    },
    Row {
        path: &["diff"],
        exits: &[
            exit(0, "the sides are equal, or differ without `--exit-code`; `--textconv` printed"),
            exit(1, "the sides differ, with `--exit-code`"),
            exit(2, "usage and open errors, a git side that cannot be read"),
        ],
        result: Some(ew_api::cli_results::DIFF),
        stability: Stability::Preview,
    },
    Row {
        path: &["git", "setup"],
        exits: &[
            exit(0, "configured"),
            exit(
                2,
                "no consent (no terminal and no `--yes`, or declined), not in a git worktree, git missing or refusing, \
                 a post-merge hook ElektraWatt did not write, write errors",
            ),
        ],
        result: None,
        stability: Stability::Preview,
    },
];

/// The subcommand paths of `cmd` (a built command) with their flags: a command is a path when it has no subcommands
/// or does not require one (`sql` and its `dump` and `schema`); clap's `help` subcommand is skipped.
fn walk(cmd: &clap::Command, path: &mut Vec<String>, out: &mut Vec<(Vec<String>, Vec<Flag>)>) {
    let subcommands: Vec<&clap::Command> = cmd.get_subcommands().filter(|s| s.get_name() != "help").collect();
    if !path.is_empty() && (subcommands.is_empty() || !cmd.is_subcommand_required_set()) {
        let flags = cmd
            .get_arguments()
            .filter(|a| !a.is_positional() && !a.is_hide_set())
            // clap's injected help and version, by their action: a user flag may be called `--version`.
            .filter(|a| {
                !matches!(
                    a.get_action(),
                    clap::ArgAction::Help
                        | clap::ArgAction::HelpShort
                        | clap::ArgAction::HelpLong
                        | clap::ArgAction::Version
                )
            })
            .filter_map(|a| {
                a.get_long().map(|long| Flag {
                    name: format!("--{long}"),
                    value: a.get_action().takes_values(),
                    repeatable: matches!(a.get_action(), clap::ArgAction::Append),
                })
            })
            .collect();
        out.push((path.clone(), flags));
    }
    for sub in subcommands {
        path.push(sub.get_name().to_owned());
        walk(sub, path, out);
        path.pop();
    }
}

/// The CLI catalogue: every subcommand path of the tree in declaration order, with its row of [`TABLE`].
///
/// # Panics
///
/// When a path of the tree has no row (the test `the_table_and_the_tree_name_the_same_paths` fails first).
pub fn catalogue() -> Vec<Entry> {
    let mut cmd = crate::cli::command();
    cmd.build();
    let mut paths = Vec::new();
    walk(&cmd, &mut Vec::new(), &mut paths);
    paths
        .into_iter()
        .map(|(path, flags)| {
            let row = TABLE
                .iter()
                .find(|r| r.path.iter().copied().eq(path.iter().map(String::as_str)))
                .unwrap_or_else(|| panic!("`ew {}` has no row in the CLI table", path.join(" ")));
            Entry {
                path,
                flags,
                exits: row.exits,
                result: row.result,
                stability: row.stability,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// Cycle 43, ruling 5: the table and the tree name the same paths, each once; the paths are those of the tree.
    #[test]
    fn the_table_and_the_tree_name_the_same_paths() {
        let mut cmd = crate::cli::command();
        cmd.build();
        let mut walked = Vec::new();
        walk(&cmd, &mut Vec::new(), &mut walked);
        let tree: BTreeSet<Vec<String>> = walked.into_iter().map(|(path, _)| path).collect();
        let table: BTreeSet<Vec<String>> = TABLE
            .iter()
            .map(|r| r.path.iter().map(|w| (*w).to_owned()).collect())
            .collect();
        assert_eq!(table.len(), TABLE.len(), "a path twice in the table");
        assert_eq!(tree, table);
        assert!(
            tree.contains(&vec!["sql".to_owned()]),
            "`sql` takes a query and has subcommands"
        );
        assert!(tree.contains(&vec!["lib".to_owned(), "import".to_owned(), "parts".to_owned()]));
        assert!(
            !tree.iter().any(|p| p.iter().any(|w| w == "help")),
            "clap's `help` is no path"
        );
    }

    /// The walk reads each flag's value and repetition from the tree, skips clap's help and version and the hidden
    /// flags, and lists a parent's global flags (`lib --company`, `--json`) on each subcommand after its own.
    #[test]
    fn the_walk_reads_values_repetition_and_global_flags() {
        let all = catalogue();
        let flags = |path: &str| -> Vec<(String, bool, bool)> {
            let entry = all.iter().find(|e| e.path.join(" ") == path).expect("the path");
            entry
                .flags
                .iter()
                .map(|f| (f.name.clone(), f.value, f.repeatable))
                .collect()
        };
        let f = |name: &str, value: bool, repeatable: bool| (name.to_owned(), value, repeatable);
        assert_eq!(
            flags("lib search"),
            [
                f("--kind", true, true),
                f("--all", false, false),
                f("--limit", true, false),
                f("--company", true, true),
                f("--json", false, false),
            ]
        );
        assert_eq!(
            flags("export png"),
            [
                f("--project", true, false),
                f("--out", true, false),
                f("--pages", true, false),
                f("--sections", true, false),
                f("--dpi", true, false),
                f("--threads", true, false),
                f("--json", false, false),
            ]
        );
        assert_eq!(
            flags("import csv"),
            [
                f("--out", true, false),
                f("--content", true, false),
                f("--strict", false, false),
                f("--lenient", false, false),
                f("--json", false, false),
            ]
        );
        assert_eq!(flags("schema"), []);
    }

    /// C43-p4#8b: the walk skips clap's help and version by their action, so a user flag with the id `version` and a
    /// `SetTrue` action stays in it, and clap's own `--help` and `--version` do not.
    #[test]
    fn the_walk_keeps_a_user_flag_named_version() {
        let mut cmd = clap::Command::new("t").version("1").subcommand(
            clap::Command::new("create").arg(
                clap::Arg::new("version")
                    .long("version")
                    .action(clap::ArgAction::SetTrue),
            ),
        );
        cmd.build();
        let mut walked = Vec::new();
        walk(&cmd, &mut Vec::new(), &mut walked);
        let names: Vec<(Vec<String>, Vec<String>)> = walked
            .into_iter()
            .map(|(path, flags)| (path, flags.into_iter().map(|f| f.name).collect()))
            .collect();
        assert_eq!(names, [(vec!["create".to_owned()], vec!["--version".to_owned()])]);
    }

    /// Every result-printing entry takes `--json` and names a published schema whose subcommand is the entry's path;
    /// every published result belongs to one entry: as its result, or (a result of a flag, `fmt --repair`'s) as the
    /// result of an entry of that path that takes the flag and `--json`; every entry that takes a project takes
    /// `--project`.
    #[test]
    fn results_and_flags_follow_the_conventions() {
        let entries = catalogue();
        let mut named = BTreeSet::new();
        for e in &entries {
            let has = |n: &str| e.flags.iter().any(|f| f.name == n);
            match e.result {
                Some(name) => {
                    assert!(has("--json"), "{:?} prints a result without `--json`", e.path);
                    let r = ew_api::cli_results::result(name)
                        .unwrap_or_else(|| panic!("{name} is not published"));
                    assert_eq!(r.command, e.path.join(" "), "{name}");
                    assert_eq!(r.flag, None, "{name} is a flag's result");
                    assert!(named.insert(name), "{name} twice");
                }
                None => assert!(!has("--json"), "{:?} takes `--json` without a result", e.path),
            }
            assert!(e.exits.iter().any(|x| x.code == 0), "{:?}", e.path);
            assert_eq!(e.stability, Stability::Preview);
        }
        for r in ew_api::cli_results::RESULTS {
            let Some(flag) = r.flag else { continue };
            let takes = entries
                .iter()
                .filter(|e| e.path.join(" ") == r.command)
                .filter(|e| e.flags.iter().any(|f| f.name == flag))
                .filter(|e| e.flags.iter().any(|f| f.name == "--json"))
                .count();
            assert_eq!(
                takes, 1,
                "{}: no entry `{} {flag}` with `--json`",
                r.name, r.command
            );
            assert!(named.insert(r.name), "{} twice", r.name);
        }
        let published: BTreeSet<&str> = ew_api::cli_results::RESULTS.iter().map(|r| r.name).collect();
        assert_eq!(named, published);
    }

    // -----------------------------------------------------------------------------------------------------
    // The CLI snapshot (WP-S1.22 D13, AG-1)
    //
    // The orchestrator's ruling 1 (cycle 42): "The CLI snapshot's generator lives in a binary": the catalogue is a
    // module of the `ew` binary, which has no library target, so this unit test writes `bin/ew/api/cli.json`,
    // blessed with `EW_CLI_BLESS=1` (the flag of the result schemas).

    /// The committed CLI snapshot.
    const SNAPSHOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/api/cli.json");

    /// The command that rewrites the snapshot.
    const BLESS: &str =
        "EW_CLI_BLESS=1 cargo nextest run -p ew --all-features --bin ew -E 'test(/^catalogue::/)'";

    #[derive(serde::Serialize)]
    struct SnapshotFile {
        cli: Vec<SnapshotEntry>,
    }

    #[derive(serde::Serialize)]
    struct SnapshotEntry {
        path: String,
        flags: Vec<SnapshotFlag>,
        exits: Vec<SnapshotExit>,
        result: Option<&'static str>,
        stability: &'static str,
    }

    #[derive(serde::Serialize)]
    struct SnapshotFlag {
        name: String,
        value: bool,
        repeatable: bool,
    }

    #[derive(serde::Serialize)]
    struct SnapshotExit {
        code: u8,
        meaning: &'static str,
    }

    /// The snapshot text of `table`: its entries sorted by path (the words joined by one space), flags and exit codes
    /// in table order; pretty JSON with a two-space indent and one trailing newline.
    fn snapshot_text(table: &[Entry]) -> String {
        let mut cli: Vec<SnapshotEntry> = table
            .iter()
            .map(|e| SnapshotEntry {
                path: e.path.join(" "),
                flags: e
                    .flags
                    .iter()
                    .map(|f| SnapshotFlag {
                        name: f.name.clone(),
                        value: f.value,
                        repeatable: f.repeatable,
                    })
                    .collect(),
                exits: e
                    .exits
                    .iter()
                    .map(|x| SnapshotExit {
                        code: x.code,
                        meaning: x.meaning,
                    })
                    .collect(),
                result: e.result,
                stability: match e.stability {
                    Stability::Preview => "preview",
                },
            })
            .collect();
        cli.sort_by(|a, b| a.path.cmp(&b.path));
        let mut text = serde_json::to_string_pretty(&SnapshotFile { cli }).expect("the snapshot serialises");
        text.push('\n');
        text
    }

    /// The comparison of the committed snapshot with the generated text: `Err` with the whole message when they
    /// differ or the file is missing.
    fn compare(committed: Option<&str>, generated: &str) -> Result<(), String> {
        if committed == Some(generated) {
            Ok(())
        } else {
            Err(format!(
                "bin/ew/api/cli.json is out of date; run `{BLESS}` and commit the result"
            ))
        }
    }

    /// Whether `EW_CLI_BLESS=1` asks for the snapshot to be rewritten.
    // The flag only chooses whether the committed file is rewritten; the compared result never depends on it.
    #[allow(clippy::disallowed_methods)]
    fn bless() -> bool {
        std::env::var_os("EW_CLI_BLESS").is_some_and(|v| v == "1")
    }

    /// AG-1 (D13): `bin/ew/api/cli.json` equals the snapshot of the catalogue (an equality snapshot, blessed with
    /// `EW_CLI_BLESS=1`).
    #[test]
    fn the_cli_snapshot_equals_the_table() {
        let generated = snapshot_text(&catalogue());
        if bless() {
            std::fs::create_dir_all(std::path::Path::new(SNAPSHOT).parent().unwrap()).unwrap();
            std::fs::write(SNAPSHOT, &generated).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(SNAPSHOT)
            .ok()
            .map(|t| t.replace("\r\n", "\n"));
        if let Err(message) = compare(committed.as_deref(), &generated) {
            panic!("{message}");
        }
    }

    /// AG-1: a one-character edit of the snapshot fails the comparison with the bless command in its message; a
    /// missing file fails the same way.
    #[test]
    fn an_edited_cli_snapshot_fails_naming_the_bless_command() {
        let generated = snapshot_text(&catalogue());
        assert_eq!(compare(Some(&generated), &generated), Ok(()));
        let edited = generated.replacen("\"--json\"", "\"--jsom\"", 1);
        assert_ne!(edited, generated);
        let want = "bin/ew/api/cli.json is out of date; run `EW_CLI_BLESS=1 cargo nextest run -p ew --all-features \
                    --bin ew -E 'test(/^catalogue::/)'` and commit the result";
        assert_eq!(compare(Some(&edited), &generated), Err(want.to_owned()));
        assert_eq!(compare(None, &generated), Err(want.to_owned()));
    }

    /// The snapshot's form: entries sorted by path, flags and exits in table order, the result name or `null`.
    #[test]
    fn the_cli_snapshot_lists_paths_flags_exits_and_results() {
        let flag = |name: &str, value: bool| Flag {
            name: name.to_owned(),
            value,
            repeatable: false,
        };
        let table = [
            Entry {
                path: vec!["export".to_owned(), "pdf".to_owned()],
                flags: vec![flag("--out", true), flag("--json", false)],
                exits: &[OK, USAGE_ETC],
                result: Some("ew.export-pdf-result/1"),
                stability: Stability::Preview,
            },
            Entry {
                path: vec!["commands".to_owned()],
                flags: Vec::new(),
                exits: &[OK],
                result: None,
                stability: Stability::Preview,
            },
        ];
        assert_eq!(
            snapshot_text(&table),
            r#"{
  "cli": [
    {
      "path": "commands",
      "flags": [],
      "exits": [
        {
          "code": 0,
          "meaning": "success"
        }
      ],
      "result": null,
      "stability": "preview"
    },
    {
      "path": "export pdf",
      "flags": [
        {
          "name": "--out",
          "value": true,
          "repeatable": false
        },
        {
          "name": "--json",
          "value": false,
          "repeatable": false
        }
      ],
      "exits": [
        {
          "code": 0,
          "meaning": "success"
        },
        {
          "code": 2,
          "meaning": "usage, open, I/O and every other error"
        }
      ],
      "result": "ew.export-pdf-result/1",
      "stability": "preview"
    }
  ]
}
"#
        );
    }
}
