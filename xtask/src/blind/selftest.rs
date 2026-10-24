// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `selftest` (WP-S6.13 §5.7, BD8 to BD11): a scripted fake agent runs through the real keeper and the copied binaries.
//!
//! The agent is data, `tools/blind/tasks/selftest/agent.jsonl`: one line per call of the harness (`do`, `kv`, `look`,
//! `note`, `handback`), each with the substrings its output must hold. The self-test starts the run, plays the agent by
//! running `<run>/bin/xtask blind …` as a child process exactly as a real agent's shell would, stops the run, blesses it as
//! the recording `tools/blind/recordings/selftest/` (`ci = "pr"`, never stale) and replays that recording twice in
//! fresh folders (R1 and R2). A second run with `--stateless` must leave the same files, the same transcript and the same
//! outcomes as the keeper's.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::replay::{self, load_inputs};
use super::run::{Kind, Run, canonical};
use super::start::{StartArgs, start};

/// The arguments of `selftest`.
pub struct Args {
    /// Build `elektrawatt` and `ew` first.
    pub build: bool,
    /// The folder of the built binaries (default `target/debug`).
    pub bin: Option<PathBuf>,
    /// Where the run folders go (default `<temp>/ew-blind-selftest`).
    pub out: Option<PathBuf>,
}

/// One call of the scripted agent.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    /// `do <run> -` with a batch on stdin.
    Do(Vec<Value>),
    /// `do <run> <action> key=value …`.
    Kv(Vec<String>),
    /// `look <run> …`: the sections, `--page` with its folio (`""` for none), `--diff`, `--menus`.
    Look {
        /// The sections.
        sections: Vec<String>,
        /// `--page [<folio>]`.
        page: Option<String>,
        /// `--diff`.
        diff: bool,
        /// `--menus`.
        menus: bool,
    },
    /// `note <run> "<text>"`.
    Note(String),
    /// `handback <run> -`.
    Handback(String),
}

/// Parses `agent.jsonl`: each line a call and the substrings its output must hold.
///
/// # Errors
///
/// A line that is not one of the five calls.
pub fn parse_agent(text: &str) -> Result<Vec<(Call, Vec<String>)>> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line).with_context(|| format!("agent.jsonl line {}", i + 1))?;
        let strings = |x: &Value| -> Vec<String> {
            x.as_array()
                .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
                .unwrap_or_default()
        };
        let call = if let Some(d) = v.get("do") {
            Call::Do(
                d.as_array()
                    .cloned()
                    .with_context(|| format!("line {}: do is a list", i + 1))?,
            )
        } else if let Some(k) = v.get("kv") {
            Call::Kv(strings(k))
        } else if let Some(l) = v.get("look") {
            Call::Look {
                sections: strings(&l["sections"]),
                page: l.get("page").and_then(Value::as_str).map(str::to_owned),
                diff: l["diff"].as_bool().unwrap_or(false),
                menus: l["menus"].as_bool().unwrap_or(false),
            }
        } else if let Some(n) = v.get("note").and_then(Value::as_str) {
            Call::Note(n.to_owned())
        } else if let Some(h) = v.get("handback").and_then(Value::as_str) {
            Call::Handback(h.to_owned())
        } else {
            bail!("agent.jsonl line {}: not a do, kv, look, note or handback", i + 1);
        };
        out.push((call, strings(&v["expect"])));
    }
    Ok(out)
}

/// The arguments of the harness command that plays `call` (after `blind` and the run ID), and its stdin.
fn command_of(call: &Call) -> (Vec<String>, Option<String>) {
    match call {
        Call::Do(steps) => (
            vec!["do".to_owned(), "-".to_owned()],
            Some(steps.iter().map(|s| format!("{}\n", canonical(s))).collect()),
        ),
        Call::Kv(args) => {
            let mut v = vec!["do".to_owned()];
            v.extend(args.iter().cloned());
            (v, None)
        }
        Call::Look {
            sections,
            page,
            diff,
            menus,
        } => {
            let mut v = vec!["look".to_owned()];
            v.extend(sections.iter().cloned());
            if let Some(p) = page {
                v.push("--page".to_owned());
                if !p.is_empty() {
                    v.push(p.clone());
                }
            }
            if *diff {
                v.push("--diff".to_owned());
            }
            if *menus {
                v.push("--menus".to_owned());
            }
            (v, None)
        }
        Call::Note(text) => (vec!["note".to_owned(), text.clone()], None),
        Call::Handback(text) => (vec!["handback".to_owned(), "-".to_owned()], Some(text.clone())),
    }
}

/// Plays the agent: runs the harness command of every call and checks its output; returns the failures.
fn play(
    run: &Run,
    id: &str,
    out: &Path,
    stateless: bool,
    agent: &[(Call, Vec<String>)],
) -> Result<Vec<String>> {
    let mut failures = Vec::new();
    for (n, (call, expect)) in agent.iter().enumerate() {
        let (mut args, stdin) = command_of(call);
        // The run ID is the first argument after the subcommand.
        args.insert(1, id.to_owned());
        if stateless && matches!(call, Call::Do(_) | Call::Kv(_) | Call::Look { .. }) {
            args.push("--stateless".to_owned());
        }
        args.push("--out".to_owned());
        args.push(out.to_string_lossy().into_owned());
        let mut child = Command::new(run.harness_exe())
            .arg("blind")
            .args(&args)
            .current_dir(&run.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("running the copied harness")?;
        if let Some(mut stdin_pipe) = child.stdin.take() {
            if let Some(text) = stdin {
                stdin_pipe.write_all(text.as_bytes())?;
            }
            drop(stdin_pipe);
        }
        let output = child.wait_with_output()?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.status.success() {
            failures.push(format!(
                "call {}: the harness exited with {:?}: {}",
                n + 1,
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
            continue;
        }
        for want in expect {
            if !text.contains(want.as_str()) {
                failures.push(format!(
                    "call {} ({}): the output lacks `{want}`:\n{text}",
                    n + 1,
                    args[0]
                ));
            }
        }
    }
    Ok(failures)
}

fn build(root: &Path) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    // `xtask` itself is running and is not rebuilt here: the copied harness is this very program.
    let status = Command::new(cargo)
        .args(["build", "-p", "elektrawatt", "-p", "ew"])
        .current_dir(root)
        .status()
        .context("running cargo build")?;
    if !status.success() {
        bail!("cargo build -p elektrawatt -p ew failed");
    }
    Ok(())
}

/// One run of the scripted agent: start, play, stop. Returns the run.
fn one_run(
    root: &Path,
    out: &Path,
    bin: &Path,
    id: &str,
    stateless: bool,
    agent: &[(Call, Vec<String>)],
) -> Result<Run> {
    let run = start(
        root,
        &StartArgs {
            kind: Kind::Free,
            task: "selftest".to_owned(),
            persona: None,
            of: None,
            folios: vec!["101".to_owned()],
            run_id: id.to_owned(),
            out: Some(out.to_path_buf()),
            bin: Some(bin.to_path_buf()),
            stateless,
        },
    )?;
    let failures = play(&run, id, out, stateless, agent)?;
    if !failures.is_empty() {
        bail!(
            "the scripted agent met {} problem(s):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
    let ok = super::run(
        &[
            "stop".to_owned(),
            id.to_owned(),
            "--out".to_owned(),
            out.to_string_lossy().into_owned(),
        ],
        root,
    )?;
    if !ok {
        bail!("stop failed");
    }
    Ok(run)
}

/// The normalised files two runs must share (the same files, the same transcript, the same outcomes).
fn comparable(run: &Run) -> Result<Vec<(String, String)>> {
    let norm = super::normalise::Normaliser::new(&run.dir);
    let mut files = Vec::new();
    for rel in [
        "prelude.uiscript",
        "steps.uiscript",
        "turns.jsonl",
        "drafter.md",
        "handback.md",
        "out/report.md",
    ] {
        let text = std::fs::read_to_string(run.path(rel))
            .with_context(|| rel.to_owned())?
            .replace("\r\n", "\n");
        // The report's title line names the run: the two runs have different IDs.
        let text = if rel == "out/report.md" {
            text.lines().skip(1).collect::<Vec<_>>().join("\n")
        } else {
            text
        };
        files.push((rel.to_owned(), norm.text(&text)));
    }
    let transcript = std::fs::read_to_string(run.path("transcript.jsonl"))?;
    files.push((
        "transcript.jsonl".to_owned(),
        transcript
            .lines()
            .map(|l| format!("{}\n", norm.text(l)))
            .collect(),
    ));
    files.push((
        "outcomes.jsonl".to_owned(),
        replay::outcomes_jsonl(&run.transcript()?)?,
    ));
    Ok(files)
}

/// `selftest`.
///
/// # Errors
///
/// A problem the scripted agent met, a replay that failed, or a difference between the two modes.
pub fn run(root: &Path, a: &Args) -> Result<bool> {
    if a.build {
        build(root)?;
    }
    let bin = a.bin.clone().unwrap_or_else(|| root.join("target/debug"));
    let out = a
        .out
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join("ew-blind-selftest"));
    if out.exists() {
        std::fs::remove_dir_all(&out).with_context(|| format!("clearing {}", out.display()))?;
    }
    let agent = parse_agent(&std::fs::read_to_string(
        root.join("tools/blind/tasks/selftest/agent.jsonl"),
    )?)?;

    // With the keeper: the run that is blessed.
    let keeper = one_run(root, &out, &bin, "selftest", false, &agent)?;
    let dest = replay::bless(root, &keeper, "selftest", "pr")?;
    println!("blessed {}", dest.strip_prefix(root).unwrap_or(&dest).display());
    let inputs = load_inputs(&dest)?;
    let exe = bin.join(format!("elektrawatt{}", std::env::consts::EXE_SUFFIX));
    let report = replay::replay(root, &exe, &inputs, true)?;
    print!("{}", replay::replay_text(&report));
    if !report.ok() {
        bail!("the blessed recording does not replay");
    }

    // Without it: the same files, transcript and outcomes.
    let stateless = one_run(root, &out, &bin, "selftest-stateless", true, &agent)?;
    let (a_files, b_files) = (comparable(&keeper)?, comparable(&stateless)?);
    for ((name, x), (_, y)) in a_files.iter().zip(&b_files) {
        if x != y {
            let at = x
                .lines()
                .zip(y.lines())
                .position(|(p, q)| p != q)
                .map_or(0, |i| i + 1);
            bail!("--stateless differs from the keeper in {name} (first difference at line {at})");
        }
    }
    println!("--stateless leaves the same files, transcript and outcomes");
    let _ = std::fs::remove_dir_all(&out);
    println!("selftest ok");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_agent_file_parses_and_its_calls_become_harness_commands() {
        let text =
            std::fs::read_to_string(crate::workspace_root().join("tools/blind/tasks/selftest/agent.jsonl"))
                .unwrap();
        let agent = parse_agent(&text).unwrap();
        assert!(agent.len() > 20);
        assert!(matches!(agent.last().unwrap().0, Call::Handback(_)));
        // Every call of the file is one of the harness's commands.
        for (call, _) in &agent {
            let (args, _) = command_of(call);
            assert!(
                ["do", "look", "note", "handback"].contains(&args[0].as_str()),
                "{args:?}"
            );
        }
        let (args, stdin) = command_of(&Call::Do(vec![
            json!({"params": {"b": 1, "a": 2}, "action": "x.y"}),
        ]));
        assert_eq!(args, ["do", "-"]);
        assert_eq!(
            stdin.unwrap(),
            "{\"action\":\"x.y\",\"params\":{\"a\":2,\"b\":1}}\n"
        );
        let (args, _) = command_of(&Call::Look {
            sections: vec!["canvas".to_owned()],
            page: Some(String::new()),
            diff: true,
            menus: false,
        });
        assert_eq!(args, ["look", "canvas", "--page", "--diff"]);
        assert!(parse_agent("{\"nothing\":1}").is_err());
    }
}
