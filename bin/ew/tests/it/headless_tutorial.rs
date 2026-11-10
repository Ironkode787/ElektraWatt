// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The headless tutorial of the user guide runs as its page says (DC-8): every `console` block of
//! `docs/book/src/tutorials/headless-workflow.md` is run in order in a folder that holds a copy of Golden A at
//! `golden/J-E-2416.ew`. An `ew` line runs the binary under test; `mkdir` makes the folder; `echo $?` shows the last
//! exit code. The lines the page shows below a command are its standard output and then its standard error, compared
//! whole; a line `...` stands for any lines, and a command without lines below it is only run.

use std::path::Path;
use std::process::{Command, Output};

use super::conventions::{bin_exe, copy_dir, isolate};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const PAGE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/book/src/tutorials/headless-workflow.md"
);

/// The words of a command line: blanks separate them, double quotes keep a word whole.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any {
                    out.push(std::mem::take(&mut word));
                    any = false;
                }
            }
            _ => {
                word.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(word);
    }
    out
}

/// The commands of the page's `console` blocks, each with the lines the page shows below it.
fn commands(page: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut in_console = false;
    for line in page.lines() {
        match (in_console, line.trim_end()) {
            (false, "```console") => in_console = true,
            (true, "```") => in_console = false,
            (true, l) => match l.strip_prefix("$ ") {
                Some(command) => out.push((command.to_owned(), Vec::new())),
                None => out.last_mut().expect("a command first").1.push(l.to_owned()),
            },
            _ => {}
        }
    }
    out
}

/// Whether `actual` is `shown`, where a shown `...` stands for any lines; a command the page shows no line of is not
/// compared.
fn shows(shown: &[String], actual: &[String]) -> bool {
    match shown.iter().position(|l| l == "...") {
        None => shown.is_empty() || shown == actual,
        Some(at) => {
            let (head, tail) = (&shown[..at], &shown[at + 1..]);
            actual.len() >= head.len() + tail.len() && actual.starts_with(head) && actual.ends_with(tail)
        }
    }
}

fn lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `ew` in `dir` with its user directories inside it, no starter content and no `SOURCE_DATE_EPOCH`.
fn ew(dir: &Path, args: &[String]) -> Output {
    let mut cmd = Command::new(bin_exe());
    cmd.args(args).current_dir(dir);
    isolate(&mut cmd, dir);
    cmd.output().unwrap()
}

#[test]
fn the_headless_tutorials_commands_print_what_its_page_shows() {
    let tmp = ew_testdir::fresh_dir("ew-headless-tutorial");
    copy_dir(Path::new(GOLDEN_A), &tmp.join("golden/J-E-2416.ew"));
    let page = std::fs::read_to_string(PAGE).unwrap().replace("\r\n", "\n");
    let commands = commands(&page);
    assert!(commands.iter().any(|(c, _)| c.starts_with("ew ")));
    let mut last = None;
    for (command, shown) in &commands {
        let words = words(command);
        match words[0].as_str() {
            "ew" => {
                let out = ew(&tmp, &words[1..]);
                let mut actual = lines(&out.stdout);
                actual.extend(lines(&out.stderr));
                assert!(
                    shows(shown, &actual),
                    "$ {command}\nthe page shows:\n{}\nthe run prints:\n{}",
                    shown.join("\n"),
                    actual.join("\n")
                );
                last = out.status.code();
            }
            "mkdir" => std::fs::create_dir_all(tmp.join(&words[1])).unwrap(),
            "echo" if words[1] == "$?" => {
                assert_eq!(
                    shown,
                    &[last.expect("an earlier command").to_string()],
                    "$ {command}"
                );
            }
            other => panic!("the page runs `{other}`, which the test does not know"),
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn the_page_parser_reads_commands_quotes_and_elisions() {
    assert_eq!(
        words("ew sql a.ew \"SELECT count(*) FROM t\""),
        ["ew", "sql", "a.ew", "SELECT count(*) FROM t"]
    );
    let shown = |l: &[&str]| l.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert!(shows(&shown(&["a", "...", "z"]), &shown(&["a", "b", "c", "z"])));
    assert!(!shows(&shown(&["a", "...", "z"]), &shown(&["a", "b"])));
    assert!(!shows(&shown(&["a"]), &shown(&["a", "b"])));
}
