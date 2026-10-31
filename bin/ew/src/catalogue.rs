// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CLI table (WP-S1.22 D4): one entry per subcommand path of [`crate::cli::USAGE`] with its flags, its exit codes,
//! its result schema and its stability.
//!
//! The parser stays hand-written; this table is the structured form of the surface. The tests below tie it to the
//! `USAGE` synopsis (each path's synopsis lines name exactly its flags, with their values and repetition), to the
//! parser (it accepts every flag of the table) and to the published result schemas
//! ([`ew_api::cli_results::RESULTS`]). From part 5 it is the source of the CLI snapshot and the CLI reference.

/// One flag of a subcommand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flag {
    /// The flag, `--name`.
    pub name: &'static str,
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

/// One subcommand path of the CLI.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry {
    /// The words after `ew` (`["export", "pdf"]`).
    pub path: &'static [&'static str],
    /// Its flags, in synopsis order.
    pub flags: &'static [Flag],
    /// Its exit codes.
    pub exits: &'static [Exit],
    /// The schema name of its `--json` result (`ew.<command>-result/1`), or `None` when it prints none.
    pub result: Option<&'static str>,
    /// Its stability.
    pub stability: Stability,
}

const fn flag(name: &'static str) -> Flag {
    Flag {
        name,
        value: false,
        repeatable: false,
    }
}

const fn valued(name: &'static str) -> Flag {
    Flag {
        name,
        value: true,
        repeatable: false,
    }
}

const fn repeated(name: &'static str) -> Flag {
    Flag {
        name,
        value: true,
        repeatable: true,
    }
}

const fn exit(code: u8, meaning: &'static str) -> Exit {
    Exit { code, meaning }
}

const PROJECT: Flag = valued("--project");
const JSON: Flag = flag("--json");
const COMPANY: Flag = repeated("--company");

const BLOCK: Flag = valued("--block");

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

/// The CLI table: every subcommand path of `USAGE` (the meta lines `--version` and `help` aside).
pub const CATALOGUE: &[Entry] = &[
    Entry {
        path: &["import", "qet"],
        flags: &[
            valued("--content"),
            valued("--report"),
            valued("--report-out"),
            flag("--strict"),
            JSON,
        ],
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
    Entry {
        path: &["import", "parts"],
        flags: &[PROJECT, JSON, valued("--map"), valued("--as-of"), COMPANY],
        exits: &[exit(0, "imported and saved"), USAGE_ETC],
        result: Some(ew_api::cli_results::IMPORT_PARTS),
        stability: Stability::Preview,
    },
    Entry {
        path: &["import", "csv"],
        flags: &[
            valued("--out"),
            valued("--content"),
            flag("--strict"),
            flag("--lenient"),
            JSON,
        ],
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
    Entry {
        path: &["check"],
        flags: &[
            PROJECT,
            valued("--scheme"),
            valued("--mode"),
            valued("--format"),
            JSON,
            valued("--lang"),
            valued("--out"),
        ],
        exits: &[
            exit(0, "no unwaived error"),
            exit(1, "unwaived errors"),
            exit(2, "usage and open errors"),
        ],
        result: Some(ew_api::cli_results::CHECK),
        stability: Stability::Preview,
    },
    Entry {
        path: &["sql"],
        flags: &[
            PROJECT,
            valued("--file"),
            valued("--format"),
            JSON,
            valued("--timeout"),
            valued("--out"),
        ],
        exits: &[OK, exit(2, "usage, open and SQL errors, denials and limits")],
        result: Some(ew_api::cli_results::SQL),
        stability: Stability::Preview,
    },
    Entry {
        path: &["sql", "dump"],
        flags: &[PROJECT, JSON],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::SQL_DUMP),
        stability: Stability::Preview,
    },
    Entry {
        path: &["sql", "schema"],
        flags: &[],
        exits: &[OK],
        result: None,
        stability: Stability::Preview,
    },
    Entry {
        path: &["export", "cables"],
        flags: &[PROJECT, BLOCK, valued("--format"), valued("--out"), JSON],
        exits: &[OK, ROW_EXPORT_ERRORS],
        result: Some(ew_api::cli_results::EXPORT_CABLES),
        stability: Stability::Preview,
    },
    Entry {
        path: &["export", "bom"],
        flags: &[
            PROJECT,
            BLOCK,
            valued("--mode"),
            valued("--format"),
            valued("--out"),
            JSON,
        ],
        exits: &[OK, ROW_EXPORT_ERRORS],
        result: Some(ew_api::cli_results::EXPORT_BOM),
        stability: Stability::Preview,
    },
    Entry {
        path: &["export", "wires"],
        flags: &[PROJECT, BLOCK, valued("--format"), valued("--out"), JSON],
        exits: &[OK, ROW_EXPORT_ERRORS],
        result: Some(ew_api::cli_results::EXPORT_WIRES),
        stability: Stability::Preview,
    },
    Entry {
        path: &["export", "terminals"],
        flags: &[
            PROJECT,
            BLOCK,
            valued("--strip"),
            valued("--format"),
            valued("--out"),
            JSON,
        ],
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
    Entry {
        path: &["export", "report"],
        flags: &[PROJECT, BLOCK, valued("--format"), valued("--out"), JSON],
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
    Entry {
        path: &["export", "pdf"],
        flags: &[
            PROJECT,
            valued("--out"),
            valued("--pages"),
            valued("--threads"),
            JSON,
        ],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::EXPORT_PDF),
        stability: Stability::Preview,
    },
    Entry {
        path: &["export", "svg"],
        flags: &[
            PROJECT,
            valued("--out"),
            valued("--pages"),
            valued("--threads"),
            JSON,
        ],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::EXPORT_SVG),
        stability: Stability::Preview,
    },
    Entry {
        path: &["export", "png"],
        flags: &[
            PROJECT,
            valued("--out"),
            valued("--pages"),
            valued("--dpi"),
            valued("--threads"),
            JSON,
        ],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::EXPORT_PNG),
        stability: Stability::Preview,
    },
    Entry {
        path: &["render"],
        flags: &[
            PROJECT,
            valued("--page"),
            flag("--png"),
            valued("--dpi"),
            valued("--out"),
            JSON,
        ],
        exits: &[
            OK,
            exit(2, "usage, open, I/O and writer errors, an unknown folio"),
        ],
        result: Some(ew_api::cli_results::RENDER),
        stability: Stability::Preview,
    },
    Entry {
        path: &["apply"],
        flags: &[PROJECT, flag("--preview"), flag("--one-step"), JSON],
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
    Entry {
        path: &["fmt"],
        flags: &[PROJECT, flag("--check"), JSON],
        exits: &[
            exit(
                0,
                "every file is canonical, or the files were rewritten and saved",
            ),
            exit(1, "`--check` found files to rewrite"),
            exit(
                2,
                "usage, open, lock, a crashed session's journal, a content change and save errors",
            ),
        ],
        result: Some(ew_api::cli_results::FMT),
        stability: Stability::Preview,
    },
    Entry {
        path: &["recover"],
        flags: &[PROJECT, flag("--replay"), flag("--discard"), JSON],
        exits: &[
            exit(0, "summarised, replayed and saved, discarded, or no journal"),
            exit(1, "the replay refused or the discard not confirmed"),
            exit(2, "usage, no controlling terminal, lock, open and save errors"),
        ],
        result: Some(ew_api::cli_results::RECOVER),
        stability: Stability::Preview,
    },
    Entry {
        path: &["project", "checks", "set"],
        flags: &[PROJECT, valued("--active"), valued("--release")],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Entry {
        path: &["project", "assets", "purge"],
        flags: &[PROJECT, flag("--preview")],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Entry {
        path: &["revision", "approve"],
        flags: &[PROJECT, valued("--as")],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Entry {
        path: &["revision", "release"],
        flags: &[PROJECT, valued("--archive-dir")],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Entry {
        path: &["revision", "withdraw"],
        flags: &[valued("--reason"), PROJECT],
        exits: HUMAN_EXITS,
        result: None,
        stability: Stability::Preview,
    },
    Entry {
        path: &["verify-derived"],
        flags: &[PROJECT, JSON],
        exits: &[
            exit(0, "no difference"),
            exit(1, "differences"),
            exit(2, "usage and open errors"),
        ],
        result: Some(ew_api::cli_results::VERIFY_DERIVED),
        stability: Stability::Preview,
    },
    Entry {
        path: &["query"],
        flags: &[PROJECT, flag("--explain"), JSON],
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
    Entry {
        path: &["queries"],
        flags: &[JSON],
        exits: &[OK],
        result: Some(ew_api::cli_results::QUERIES),
        stability: Stability::Preview,
    },
    Entry {
        path: &["lib", "list"],
        flags: &[COMPANY, JSON],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_LIST),
        stability: Stability::Preview,
    },
    Entry {
        path: &["lib", "search"],
        flags: &[
            repeated("--kind"),
            flag("--all"),
            valued("--limit"),
            COMPANY,
            JSON,
        ],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_SEARCH),
        stability: Stability::Preview,
    },
    Entry {
        path: &["lib", "index"],
        flags: &[flag("--rebuild"), COMPANY, JSON],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_INDEX),
        stability: Stability::Preview,
    },
    Entry {
        path: &["lib", "create"],
        flags: &[
            valued("--key"),
            valued("--name"),
            valued("--licence"),
            COMPANY,
            JSON,
        ],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_CREATE),
        stability: Stability::Preview,
    },
    Entry {
        path: &["lib", "import", "parts"],
        flags: &[
            valued("--map"),
            valued("--as-of"),
            flag("--bump"),
            flag("--take-over"),
            COMPANY,
            JSON,
        ],
        exits: &[OK, USAGE_ETC],
        result: Some(ew_api::cli_results::LIB_IMPORT_PARTS),
        stability: Stability::Preview,
    },
    Entry {
        path: &["commands"],
        flags: &[JSON],
        exits: &[OK],
        result: Some(ew_api::cli_results::COMMANDS),
        stability: Stability::Preview,
    },
    Entry {
        path: &["schema"],
        flags: &[],
        exits: &[OK, exit(2, "usage, an unknown schema name")],
        result: None,
        stability: Stability::Preview,
    },
];

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    /// The synopsis of `USAGE`: one entry per line (continuation lines joined), the meta lines left out.
    fn synopsis() -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for line in crate::cli::USAGE.lines().skip(1) {
            if line.is_empty() {
                break;
            }
            if line.starts_with("      ") {
                let last = lines.last_mut().expect("a continuation follows a synopsis line");
                last.push(' ');
                last.push_str(line.trim());
            } else {
                lines.push(line.trim().to_owned());
            }
        }
        lines
            .into_iter()
            .filter(|l| !l.starts_with("ew --version") && !l.starts_with("ew help"))
            .collect()
    }

    /// The path and the flags one synopsis line names.
    fn parse_line(line: &str) -> (Vec<String>, Vec<Flag>) {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(tokens.first(), Some(&"ew"), "{line}");
        let path: Vec<String> = tokens[1..]
            .iter()
            .take_while(|t| t.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') && !t.starts_with('-'))
            .map(|t| (*t).to_owned())
            .collect();
        let mut flags = Vec::new();
        for (i, t) in tokens.iter().enumerate() {
            let bare = t.trim_start_matches('[');
            if !bare.starts_with("--") {
                continue;
            }
            let name: String = bare
                .chars()
                .take_while(|c| *c == '-' || c.is_ascii_lowercase())
                .collect();
            let rest = &bare[name.len()..];
            let next = tokens.get(i + 1).copied().unwrap_or("");
            let value =
                rest.is_empty() && !next.is_empty() && !next.starts_with('[') && !next.starts_with("--");
            let closing = if value { next } else { rest };
            flags.push(Flag {
                name: Box::leak(name.into_boxed_str()),
                value,
                repeatable: closing.ends_with("]..."),
            });
        }
        (path, flags)
    }

    /// D4: each `USAGE` synopsis line names only flags of its entry, and a path's lines together name exactly the
    /// entry's flags, with their values and repetition; every path of `USAGE` has an entry and every entry a line.
    #[test]
    fn usage_synopsis_names_exactly_the_flags_of_each_entry() {
        let mut by_path: BTreeMap<Vec<String>, BTreeSet<(String, bool, bool)>> = BTreeMap::new();
        for line in synopsis() {
            let (path, flags) = parse_line(&line);
            let entry = CATALOGUE
                .iter()
                .find(|e| e.path.iter().copied().eq(path.iter().map(String::as_str)))
                .unwrap_or_else(|| panic!("no catalogue entry for `{line}`"));
            let set = by_path.entry(path).or_default();
            for f in flags {
                assert!(
                    entry.flags.contains(&f),
                    "`{line}` names {f:?}, which its entry does not hold as such"
                );
                set.insert((f.name.to_owned(), f.value, f.repeatable));
            }
        }
        for e in CATALOGUE {
            let path: Vec<String> = e.path.iter().map(|w| (*w).to_owned()).collect();
            let named = by_path
                .get(&path)
                .unwrap_or_else(|| panic!("{:?} has no synopsis line in USAGE", e.path));
            let want: BTreeSet<(String, bool, bool)> = e
                .flags
                .iter()
                .map(|f| (f.name.to_owned(), f.value, f.repeatable))
                .collect();
            assert_eq!(*named, want, "{:?}", e.path);
        }
        assert_eq!(by_path.len(), CATALOGUE.len());
    }

    /// The hand-written parser accepts every flag of the table (none is an unknown option).
    #[test]
    fn the_parser_knows_every_flag_of_the_table() {
        for e in CATALOGUE {
            for f in e.flags {
                let mut args: Vec<String> = e.path.iter().map(|w| (*w).to_owned()).collect();
                args.push(f.name.to_owned());
                if f.value {
                    args.push("x".to_owned());
                }
                if let Err(msg) = crate::cli::parse(args.clone()) {
                    assert!(
                        !msg.contains(&format!("unknown option `{}`", f.name)),
                        "{args:?}: {msg}"
                    );
                }
            }
        }
    }

    /// Every result-printing entry takes `--json` and names a published schema whose subcommand is the entry's path;
    /// every published result belongs to one entry; every entry that takes a project takes `--project`.
    #[test]
    fn results_and_flags_follow_the_conventions() {
        let mut named = BTreeSet::new();
        for e in CATALOGUE {
            let has = |n: &str| e.flags.iter().any(|f| f.name == n);
            match e.result {
                Some(name) => {
                    assert!(has("--json"), "{:?} prints a result without `--json`", e.path);
                    let r = ew_api::cli_results::result(name)
                        .unwrap_or_else(|| panic!("{name} is not published"));
                    assert_eq!(r.command, e.path.join(" "), "{name}");
                    assert!(named.insert(name), "{name} twice");
                }
                None => assert!(!has("--json"), "{:?} takes `--json` without a result", e.path),
            }
            assert!(e.exits.iter().any(|x| x.code == 0), "{:?}", e.path);
            assert_eq!(e.stability, Stability::Preview);
        }
        let published: BTreeSet<&str> = ew_api::cli_results::RESULTS.iter().map(|r| r.name).collect();
        assert_eq!(named, published);
    }

    // -----------------------------------------------------------------------------------------------------
    // The CLI snapshot (WP-S1.22 D13, AG-1)
    //
    // The orchestrator's ruling 1 (cycle 42): "The CLI snapshot's generator lives in a binary": `CATALOGUE` is a
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
        name: &'static str,
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
                        name: f.name,
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

    /// AG-1 (D13): `bin/ew/api/cli.json` equals the snapshot of `CATALOGUE` (an equality snapshot, blessed with
    /// `EW_CLI_BLESS=1`).
    #[test]
    fn the_cli_snapshot_equals_the_table() {
        let generated = snapshot_text(CATALOGUE);
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
        let generated = snapshot_text(CATALOGUE);
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
        const FLAGS: &[Flag] = &[valued("--out"), JSON];
        let table = [
            Entry {
                path: &["export", "pdf"],
                flags: FLAGS,
                exits: &[OK, USAGE_ETC],
                result: Some("ew.export-pdf-result/1"),
                stability: Stability::Preview,
            },
            Entry {
                path: &["commands"],
                flags: &[],
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
