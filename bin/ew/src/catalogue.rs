// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CLI table: one entry per subcommand path of the derive tree ([`crate::cli::command`]) with its
//! flags, its exit codes and its result schema.
//!
//! The paths and the flags (name, value, repetition) come from walking clap's tree in declaration order, skipping
//! clap's own help and version; the exit codes and the result of each path are its `Leaf` beside its run function
//! (`cli/tree.rs`), which the walk reads as it goes ([`crate::cli::entries`]). The tests below tie the entries to the
//! published result schemas ([`ew_api::cli_results::RESULTS`]).
//! It is the source of the CLI snapshot and the CLI reference.

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

/// One subcommand path of the CLI: its words and flags from the tree, its exits and result from its `Leaf`.
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
}

/// The exit code `code` with what it means for a subcommand.
pub const fn exit(code: u8, meaning: &'static str) -> Exit {
    Exit { code, meaning }
}

/// Success.
pub const OK: Exit = exit(0, "success");
/// Every error of a subcommand that tells none apart.
pub const USAGE_ETC: Exit = exit(2, "usage, open, I/O and every other error");
/// The errors of a row export of one block.
pub const ROW_EXPORT_ERRORS: Exit = exit(
    2,
    "usage and open errors, a project without the block, an unknown or wrong block",
);
/// The exits of a human-only subcommand.
pub const HUMAN_EXITS: &[Exit] = &[
    exit(0, "confirmed, done and saved"),
    exit(1, "refused or not confirmed"),
    exit(2, "usage, no controlling terminal, open and save errors"),
];

/// The subcommands of `cmd` (a built command) in declaration order; clap's `help` subcommand is skipped.
pub fn subcommands(cmd: &clap::Command) -> impl Iterator<Item = &clap::Command> {
    cmd.get_subcommands().filter(|s| s.get_name() != "help")
}

/// The entries of the node `N` whose clap command is `cmd`, a subcommand below `path`.
///
/// # Panics
///
/// When the group has fewer subcommands than variants: the tree and its enum are one declaration, so it does not.
pub fn below<N: crate::cli::tree::Node>(
    cmd: Option<&clap::Command>,
    path: &mut Vec<String>,
    out: &mut Vec<Entry>,
) {
    let cmd = cmd.expect("a variant of a group without its subcommand");
    path.push(cmd.get_name().to_owned());
    N::entries(cmd, path, out);
    path.pop();
}

/// The entry of the subcommand path `path` whose clap command is `cmd` (built), with its exits and result: its flags
/// in declaration order, clap's help and version and the hidden flags skipped; a parent's global flags (`lib
/// --company`, `--json`) follow its own.
pub fn entry(
    cmd: &clap::Command,
    path: &[String],
    exits: &'static [Exit],
    result: Option<&'static str>,
) -> Entry {
    Entry {
        path: path.to_vec(),
        flags: flags(cmd),
        exits,
        result,
    }
}

/// The flags of `cmd`, as [`entry`] lists them.
fn flags(cmd: &clap::Command) -> Vec<Flag> {
    cmd.get_arguments()
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
        .collect()
}

/// The CLI catalogue: every subcommand path of the tree in declaration order, with its exits and result.
pub fn catalogue() -> Vec<Entry> {
    crate::cli::entries()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The entries are the paths of clap's tree, each once: a command is a path when it has no subcommands or does
    /// not require one (`sql` and its `dump` and `schema`), so a structure with subcommands names them as its own
    /// (`Leaf::below`) and a group of required subcommands is no path.
    #[test]
    fn the_entries_are_the_paths_of_the_tree() {
        fn walk(cmd: &clap::Command, path: &mut Vec<String>, out: &mut BTreeSet<Vec<String>>) {
            let subs: Vec<&clap::Command> = subcommands(cmd).collect();
            if !path.is_empty() && (subs.is_empty() || !cmd.is_subcommand_required_set()) {
                out.insert(path.clone());
            }
            for sub in subs {
                path.push(sub.get_name().to_owned());
                walk(sub, path, out);
                path.pop();
            }
        }
        let mut cmd = crate::cli::command();
        cmd.build();
        let mut tree = BTreeSet::new();
        walk(&cmd, &mut Vec::new(), &mut tree);
        let entries = catalogue();
        let listed: BTreeSet<Vec<String>> = entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(listed.len(), entries.len(), "a path twice");
        assert_eq!(listed, tree);
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
                f("--underlays", false, false),
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

    /// The flags skip clap's help and version by their action, so a user flag with the id `version` and a
    /// `SetTrue` action stays in them, and clap's own `--help` and `--version` do not.
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
        let names: Vec<(String, Vec<String>)> = subcommands(&cmd)
            .map(|sub| {
                let flags = flags(sub).into_iter().map(|f| f.name).collect();
                (sub.get_name().to_owned(), flags)
            })
            .collect();
        assert_eq!(names, [("create".to_owned(), vec!["--version".to_owned()])]);
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
    // The CLI reference
    //
    // The catalogue is a module of the `ew` binary, which has no library target, so this unit test writes the
    // reference (`cli.json`: paths, flags, exits, results) where `EW_REFERENCE_DIR` asks for it, for the user guide.

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
            })
            .collect();
        cli.sort_by(|a, b| a.path.cmp(&b.path));
        let mut text = serde_json::to_string_pretty(&SnapshotFile { cli }).expect("the snapshot serialises");
        text.push('\n');
        text
    }

    // The variable only chooses whether the generated file is also written out.
    #[allow(clippy::disallowed_methods)]
    #[test]
    fn the_cli_reference_is_written_where_it_is_asked_for() {
        let Some(dir) = std::env::var_os("EW_REFERENCE_DIR").map(std::path::PathBuf::from) else {
            return;
        };
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cli.json"), snapshot_text(&catalogue())).unwrap();
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
            },
            Entry {
                path: vec!["commands".to_owned()],
                flags: Vec::new(),
                exits: &[OK],
                result: None,
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
      "result": null
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
      "result": "ew.export-pdf-result/1"
    }
  ]
}
"#
        );
    }
}
