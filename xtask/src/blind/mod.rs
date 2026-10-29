// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask blind`: the blind-drafter harness (WP-S6.13 part 1, ID-610 to ID-624).
//!
//! An agent that never sees pixels draws and edits in ElektraWatt through the shell's headless mode, with `ui.describe`
//! and `page.describe` as its only senses and the transcript as its evidence. This module is the tool between the agent
//! and the product: run folders outside the repository ([`mod@run`], [`start`]), the filter and the typed arguments
//! ([`filter`]), the protocol ([`protocol`]), the loopback keeper and the stateless replay ([`keeper`], [`stateless`]),
//! the composed prompt ([`compose`]), the stop with its epilogue ([`stop`]) and the recording, bless and replay
//! ([`replay`]). The harness drives the built `elektrawatt` and `ew` binaries as processes: it has no edge to any product
//! crate and calls no model (D2, D13).
//!
//! The agent runs only the run's copy of this program (`<run>/bin/xtask blind …`); the orchestrator runs the same code
//! from the repository.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub mod compose;
pub mod filter;
pub mod keeper;
pub mod normalise;
pub mod oracle;
pub mod protocol;
pub mod replay;
pub mod run;
pub mod score;
pub mod selftest;
pub mod session;
pub mod start;
pub mod stateless;
pub mod stop;
#[cfg(test)]
pub mod testkit;

use filter::{Reference, StepIn};
use run::{Kind, Run};
use session::{Backend, Lost, RealSpawner, Sent, Step};

const HELP: &str = "\
cargo xtask blind <subcommand> (the agent runs <run>/bin/xtask blind <subcommand>)

For the agent:
  help                                        this text, the protocol, the vocabulary and the actions
  do <run> <action> [key=value ...]           one step; values are typed by the action's signature
  do <run> -                                  a batch of 1 to 20 steps as JSON Lines on stdin
  look <run> [section ...] [--diff] [--menus] the interface as text, by sections
  look <run> --page [<folio>]                 the content of a page as text
  note <run> \"patience <0-10> flow <0-10> <text>\"
                                              the note of the last turn
  handback <run> -                            the fixed hand-back on stdin; it ends the run
For the orchestrator:
  start --kind free|gate|check --task <t> [--persona <p>] [--of <run>] [--folios <a,b>] --run <id>
        [--out <dir>] [--bin <dir>] [--stateless]
  stop <run> [--reason <r>]                   the epilogue, the exports, the report
  report <run> check|bom|score                what the stop wrote
  resume <run>                                a lost session: replay and go on
  compose --kind <k> --task <t> [--persona <p>] [--folios <a,b>] --run <id>
  bless <run> --as <name> [--ci pr|nightly|none]
  replay <recording|run> [--bin <dir>] [--twice]
  rescore <recording> [--bin <dir>] [--write] a recording scored again against its task's current oracle
  selftest [--no-build] [--bin <dir>] [--out <dir>]
  bless-oracle <task> [--bin <dir>]           the expected rows of a task's oracle, from a temporary copy of Golden A
  check-task <task>                           the task folder: files, prelude classes, gaps, the sheet against doc 20
";

/// Takes `--flag value` and `--flag` out of `args`.
struct Flags(Vec<String>);

impl Flags {
    fn take_value(&mut self, name: &str) -> Result<Option<String>> {
        match self.0.iter().position(|a| a == name) {
            None => Ok(None),
            Some(i) => {
                if i + 1 >= self.0.len() {
                    bail!("{name} needs a value");
                }
                self.0.remove(i);
                Ok(Some(self.0.remove(i)))
            }
        }
    }

    fn take_flag(&mut self, name: &str) -> bool {
        match self.0.iter().position(|a| a == name) {
            None => false,
            Some(i) => {
                self.0.remove(i);
                true
            }
        }
    }

    fn rest(self) -> Vec<String> {
        self.0
    }
}

/// The subcommands an agent may run from the run's copy of the harness; every other one is the orchestrator's.
pub const AGENT_COMMANDS: [&str; 8] = [
    "help", "do", "look", "note", "handback", "report", "resume", "serve",
];

/// The run whose copy of the harness is `exe`: `<dir>/bin/xtask` beside `<dir>/run.json`.
pub fn copy_of(exe: Option<&Path>) -> Option<Run> {
    exe.and_then(Path::parent)
        .filter(|p| p.file_name().is_some_and(|n| n == "bin"))
        .and_then(Path::parent)
        .filter(|d| d.join("run.json").is_file())
        .map(|d| Run { dir: d.to_path_buf() })
}

/// The refusal of `command` when it is run from a run's copy: the copy knows no repository (isolation, ID-621), so only
/// the agent's commands and the keeper's own `serve` run there (`--help` and `-h` are `help`).
pub fn refused_in_copy(command: &str, from_copy: bool) -> Option<String> {
    let command = if matches!(command, "--help" | "-h") {
        "help"
    } else {
        command
    };
    (from_copy && !AGENT_COMMANDS.contains(&command))
        .then(|| format!("harness: {command} is the orchestrator's"))
}

/// Whether `arg` names the folder of `own`, the run whose copy of the harness is running.
pub fn serves_own_run(own: &Run, arg: Option<&str>) -> bool {
    let Some(arg) = arg else { return false };
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    canon(Path::new(arg)) == canon(&own.dir)
}

/// The reference of a run (the copy in the run folder) or, from the repository, the committed one. A run's copy of the
/// harness (`repo` is `None`) never falls back to the repository.
fn load_reference(run: Option<&Run>, repo: Option<&Path>) -> Result<Reference> {
    let own = run
        .map(|r| r.path(".harness/reference/actions.txt"))
        .filter(|p| p.is_file());
    let path = match (own, repo) {
        (Some(p), _) => p,
        (None, Some(root)) => root.join("tools/blind/reference/actions.txt"),
        (None, None) => bail!("the run's copy of the action reference is missing"),
    };
    filter::parse_reference(
        &std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?,
    )
}

/// A backend that is never there: a run whose keeper does not answer.
struct Gone(String);

impl Backend for Gone {
    fn execute(&mut self, _: &Step) -> Result<Sent> {
        Err(Lost(self.0.clone()).into())
    }
    fn end_call(&mut self) -> Result<Option<i32>> {
        Ok(None)
    }
    fn finish(&mut self) -> Result<Option<i32>> {
        Ok(None)
    }
}

/// Runs `f` with the backend of `run`: the keeper's client, or the stateless replay.
fn with_backend<T>(run: &Run, f: impl FnOnce(&mut dyn Backend) -> Result<T>) -> Result<T> {
    let rj = run.load()?;
    if rj.stateless {
        let spawner = RealSpawner;
        let mut backend = stateless::Stateless::new(run, &spawner);
        f(&mut backend)
    } else {
        match keeper::KeeperClient::open(run) {
            Ok(mut client) => f(&mut client),
            Err(e) => f(&mut Gone(e.to_string())),
        }
    }
}

/// `cargo xtask blind …`. Returns whether the command succeeded.
///
/// # Errors
///
/// Usage and I/O errors.
pub fn run(args: &[String], root: &Path) -> Result<bool> {
    let Some(command) = args.first().map(String::as_str) else {
        print!("{HELP}");
        return Ok(true);
    };
    let rest = args[1..].to_vec();
    let exe = std::env::current_exe().ok();
    // The run's copy of the harness knows no repository: it runs the agent's commands only, from its own folder.
    let copy = copy_of(exe.as_deref());
    if let Some(refusal) = refused_in_copy(command, copy.is_some()) {
        println!("{refusal}");
        return Ok(false);
    }
    let repo: Option<&Path> = if copy.is_some() { None } else { Some(root) };
    let locate = |arg: &str, out: Option<&str>| run::locate(arg, out.map(Path::new), exe.as_deref());
    match command {
        "help" | "--help" | "-h" => {
            print!("{}", help_text(repo, copy.as_ref())?);
            Ok(true)
        }
        "serve" => {
            // From a run's copy the keeper serves that run only (the binding `locate` applies to every other command).
            if let Some(own) = &copy
                && !serves_own_run(own, rest.first().map(String::as_str))
            {
                println!(
                    "harness: serve of {} is the orchestrator's",
                    rest.first().map_or("another folder", String::as_str)
                );
                return Ok(false);
            }
            keeper::serve_command(&rest)
        }
        "start" => {
            let mut f = Flags(rest);
            let a = start::StartArgs {
                kind: Kind::parse(&f.take_value("--kind")?.context("--kind is needed")?)?,
                task: f.take_value("--task")?.context("--task is needed")?,
                persona: f.take_value("--persona")?,
                of: f.take_value("--of")?,
                folios: f
                    .take_value("--folios")?
                    .map(|s| {
                        s.split(',')
                            .map(|x| x.trim().to_owned())
                            .filter(|x| !x.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
                run_id: f.take_value("--run")?.context("--run is needed")?,
                out: f.take_value("--out")?.map(PathBuf::from),
                bin: f.take_value("--bin")?.map(PathBuf::from),
                stateless: f.take_flag("--stateless"),
            };
            if let Some(extra) = f.rest().first() {
                bail!("start: unexpected `{extra}`");
            }
            let run = start::start(root, &a)?;
            println!("run {} started in {}", a.run_id, run.dir.display());
            println!("the agent's prompt: {}", run.path("prompt.md").display());
            println!("the agent's command: {}", run.blind_command());
            Ok(true)
        }
        "do" => {
            let mut f = Flags(rest);
            let stateless_flag = f.take_flag("--stateless");
            let out = f.take_value("--out")?;
            let mut rest = f.rest().into_iter();
            let run_arg = rest.next().context("do needs the run")?;
            let run = locate(&run_arg, out.as_deref())?;
            check_stateless_flag(&run, stateless_flag)?;
            let reference = load_reference(Some(&run), repo)?;
            let first = rest.next().context("do needs an action or `-` for a batch")?;
            let steps: Vec<StepIn> = if first == "-" {
                let mut text = String::new();
                std::io::stdin().read_to_string(&mut text)?;
                match filter::parse_batch(&text) {
                    Ok(s) => s,
                    Err(e) => {
                        println!("{e}");
                        return Ok(true);
                    }
                }
            } else {
                let args: Vec<String> = rest.collect();
                match filter::parse_args(&reference, &first, &args) {
                    Ok(s) => vec![s],
                    Err(e) => {
                        println!("{e}");
                        return Ok(true);
                    }
                }
            };
            let text = with_backend(&run, |b| protocol::do_steps(&run, &reference, b, &steps))?;
            print!("{text}");
            Ok(true)
        }
        "look" => {
            let mut f = Flags(rest);
            let stateless_flag = f.take_flag("--stateless");
            let out = f.take_value("--out")?;
            let diff = f.take_flag("--diff");
            let menus = f.take_flag("--menus");
            let page_at = f.0.iter().position(|a| a == "--page");
            let mut args = f.rest();
            let run_arg = if args.is_empty() {
                bail!("look needs the run")
            } else {
                args.remove(0)
            };
            let mut opts = protocol::LookOpts {
                diff,
                menus,
                ..protocol::LookOpts::default()
            };
            if let Some(i) = page_at {
                // `--page [<folio>]`: the word after the flag is the folio when it is not another flag.
                let i = i - 1;
                args.remove(i);
                let folio = (i < args.len()).then(|| args.remove(i));
                opts.page = Some(folio);
            }
            opts.sections = args;
            let run = locate(&run_arg, out.as_deref())?;
            check_stateless_flag(&run, stateless_flag)?;
            let text = with_backend(&run, |b| protocol::look(&run, b, &opts))?;
            print!("{text}");
            Ok(true)
        }
        "note" => {
            let mut f = Flags(rest);
            let out = f.take_value("--out")?;
            let args = f.rest();
            let [run_arg, text] = args.as_slice() else {
                bail!("note <run> \"patience <0-10> flow <0-10> <text>\"");
            };
            let run = locate(run_arg, out.as_deref())?;
            print!("{}", protocol::note(&run, text)?);
            Ok(true)
        }
        "handback" => {
            let mut f = Flags(rest);
            let out = f.take_value("--out")?;
            let args = f.rest();
            let [run_arg, dash] = args.as_slice() else {
                bail!("handback <run> -");
            };
            if dash != "-" {
                bail!("handback <run> - (the hand-back on stdin)");
            }
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text)?;
            let run = locate(run_arg, out.as_deref())?;
            print!("{}", protocol::handback(&run, &text)?);
            Ok(true)
        }
        "report" => {
            let mut f = Flags(rest);
            let out = f.take_value("--out")?;
            let args = f.rest();
            let [run_arg, which] = args.as_slice() else {
                bail!("report <run> check|bom|score");
            };
            let run = locate(run_arg, out.as_deref())?;
            print!("{}", protocol::report(&run, which)?);
            Ok(true)
        }
        "stop" => {
            let mut f = Flags(rest);
            let out = f.take_value("--out")?;
            let reason = f.take_value("--reason")?.unwrap_or_else(|| "handback".to_owned());
            let args = f.rest();
            let [run_arg] = args.as_slice() else {
                bail!("stop <run> [--reason <r>]");
            };
            let run = locate(run_arg, out.as_deref())?;
            let text = with_backend(&run, |b| stop::stop(root, &run, b, &reason))?;
            print!("{text}");
            Ok(true)
        }
        "resume" => {
            let mut f = Flags(rest);
            let out = f.take_value("--out")?;
            let args = f.rest();
            let [run_arg] = args.as_slice() else {
                bail!("resume <run>");
            };
            let run = locate(run_arg, out.as_deref())?;
            println!("{}", resume(&run)?);
            Ok(true)
        }
        "compose" => {
            let mut f = Flags(rest);
            let kind = Kind::parse(&f.take_value("--kind")?.context("--kind is needed")?)?;
            let task = f.take_value("--task")?.context("--task is needed")?;
            let persona = f.take_value("--persona")?;
            let run_id = f.take_value("--run")?.context("--run is needed")?;
            let folios: Vec<String> = f
                .take_value("--folios")?
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_owned())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let c = compose::compose(&compose::ComposeIn {
                root,
                kind,
                task: &task,
                persona: persona.as_deref(),
                run_id: &run_id,
                blind: "<run>/bin/xtask blind",
                caps: kind.caps(),
                folios: &folios,
            })?;
            print!("{}", c.text);
            Ok(true)
        }
        "bless" => {
            let mut f = Flags(rest);
            let out = f.take_value("--out")?;
            let name = f.take_value("--as")?.context("--as <name> is needed")?;
            let ci = f.take_value("--ci")?.unwrap_or_else(|| "none".to_owned());
            let args = f.rest();
            let [run_arg] = args.as_slice() else {
                bail!("bless <run> --as <name> [--ci pr|nightly|none]");
            };
            let run = locate(run_arg, out.as_deref())?;
            let dest = replay::bless(root, &run, &name, &ci)?;
            println!(
                "blessed into {}",
                dest.strip_prefix(root).unwrap_or(&dest).display()
            );
            Ok(true)
        }
        "replay" => {
            let mut f = Flags(rest);
            let bin = f.take_value("--bin")?.map(PathBuf::from);
            let out = f.take_value("--out")?;
            let twice = f.take_flag("--twice");
            let args = f.rest();
            let [what] = args.as_slice() else {
                bail!("replay <recording|run> [--bin <dir>] [--twice]");
            };
            let recording = replay::recordings_dir(root).join(what);
            let dir = if recording.join("run.json").is_file() {
                recording
            } else {
                locate(what, out.as_deref())?.dir
            };
            let inputs = replay::load_inputs(&dir)?;
            if let Some(reason) = &inputs.stale {
                println!("stale: {reason} (skipped)");
                return Ok(true);
            }
            let exe = bin
                .unwrap_or_else(|| root.join("target/debug"))
                .join(format!("elektrawatt{}", std::env::consts::EXE_SUFFIX));
            let report = replay::replay(root, &exe, &inputs, twice)?;
            print!("{}", replay::replay_text(&report));
            Ok(report.ok())
        }
        "rescore" => {
            let mut f = Flags(rest);
            let bin = f
                .take_value("--bin")?
                .map_or_else(|| root.join("target/debug"), PathBuf::from);
            let write = f.take_flag("--write");
            let args = f.rest();
            let [name] = args.as_slice() else {
                bail!("rescore <recording> [--bin <dir>] [--write]");
            };
            let (score, old) = replay::rescore(root, &bin, name, write)?;
            let outcome = |json: &str| -> String {
                serde_json::from_str::<serde_json::Value>(json)
                    .ok()
                    .and_then(|v| v["outcome"].as_str().map(str::to_owned))
                    .unwrap_or_else(|| "none".to_owned())
            };
            let new = score::score_json(&score)?;
            println!("recorded outcome {}, now {}", outcome(&old), outcome(&new));
            print!("{}", score::score_markdown(&score));
            if write {
                println!("written: tools/blind/recordings/{name}/out/score.json and out/report.md");
            }
            Ok(true)
        }
        "selftest" => {
            let mut f = Flags(rest);
            let a = selftest::Args {
                build: !f.take_flag("--no-build"),
                bin: f.take_value("--bin")?.map(PathBuf::from),
                out: f.take_value("--out")?.map(PathBuf::from),
            };
            if let Some(extra) = f.rest().first() {
                bail!("selftest: unexpected `{extra}`");
            }
            selftest::run(root, &a)
        }
        "bless-oracle" => {
            let mut f = Flags(rest);
            let bin = f.take_value("--bin")?.map(PathBuf::from);
            let args = f.rest();
            let [task] = args.as_slice() else {
                bail!("bless-oracle <task> [--bin <dir>]");
            };
            let (before, after) = oracle::bless_oracle(
                root,
                &oracle::BlessArgs {
                    task: task.clone(),
                    bin,
                },
            )?;
            println!("oracle of `{task}` written from a temporary copy of the golden project");
            println!("golden/ digest before {before}");
            println!("golden/ digest after  {after}");
            println!("golden/ unchanged: {}", before == after);
            Ok(before == after)
        }
        "check-task" => {
            let args = Flags(rest).rest();
            let [task] = args.as_slice() else {
                bail!("check-task <task>");
            };
            let problems = oracle::check_task(root, task)?;
            if problems.is_empty() {
                println!("task `{task}` is sound");
                Ok(true)
            } else {
                for p in &problems {
                    println!("{p}");
                }
                println!("task `{task}`: {} problem(s)", problems.len());
                Ok(false)
            }
        }
        other => {
            eprint!("blind: unknown subcommand `{other}`\n\n{HELP}");
            Ok(false)
        }
    }
}

/// `--stateless` names the run's mode: given for a run with a keeper it is a mistake.
fn check_stateless_flag(run: &Run, flag: bool) -> Result<()> {
    if flag && !run.load()?.stateless {
        bail!("this run has a keeper: leave out --stateless");
    }
    Ok(())
}

/// `resume <run>`: replays the prelude and the accepted steps in a fresh product under a fresh keeper.
///
/// # Errors
///
/// The folder cannot be reset or the keeper cannot start.
pub fn resume(run: &Run) -> Result<String> {
    let rj = run.load()?;
    if rj.stateless {
        return Ok("this run is stateless: every call replays, there is no keeper to bring back".to_owned());
    }
    if matches!(
        rj.end.as_deref(),
        Some("handback" | "cap" | "quit" | "orchestrator")
    ) {
        return Ok(format!(
            "run `{}` has ended ({}): nothing to resume",
            rj.run,
            rj.end.unwrap_or_default()
        ));
    }
    // A keeper that still answers is stopped first.
    if let Ok(client) = keeper::KeeperClient::open(run) {
        let _ = client.shutdown();
    }
    for name in ["work", "config"] {
        let p = run.path(name);
        if p.parent() != Some(run.dir.as_path()) {
            bail!("refusing to delete {} outside the run folder", p.display());
        }
        match std::fs::remove_dir_all(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("deleting {}", p.display())),
        }
    }
    // A check run's project is a copy in `work/`: it goes back with it.
    start::reseed_check_project(run)?;
    match keeper::spawn_keeper(run, true, keeper::DEFAULT_IDLE_MINUTES) {
        Ok(_) => {
            run.update(|j| {
                if j.end.as_deref() == Some("lost") {
                    j.end = None;
                }
            })?;
            Ok(format!("run `{}` resumed: the outcomes repeat", rj.run))
        }
        Err(e) => {
            if let Some(d) = e.downcast_ref::<session::Diverged>() {
                let step = d.step;
                run.update(|j| {
                    j.end = Some("diverged".to_owned());
                    j.diverged_at = Some(step);
                })?;
                return Ok(format!("run `{}` diverged at step {step}: {}", rj.run, d.what));
            }
            Err(e)
        }
    }
}

/// The text of `help`: the harness help, the vocabulary and the actions, and what the copied binaries print.
fn help_text(repo: Option<&Path>, run: Option<&Run>) -> Result<String> {
    let read = |own: &str, rel: &str| -> Result<String> {
        let own = run.map(|r| r.path(own)).filter(|p| p.is_file());
        let path = match (own, repo) {
            (Some(p), _) => p,
            (None, Some(root)) => root.join(rel),
            (None, None) => bail!("the run's copy of {rel} is missing"),
        };
        Ok(std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?
            .replace("\r\n", "\n"))
    };
    let mut out = read(".harness/HELP.md", "tools/blind/HELP.md")?;
    out.push_str("\n== vocabulary ==\n");
    out.push_str(&read(
        ".harness/reference/vocabulary.txt",
        "tools/blind/reference/vocabulary.txt",
    )?);
    out.push_str("\n== actions ==\n");
    out.push_str(&read(
        ".harness/reference/actions.txt",
        "tools/blind/reference/actions.txt",
    )?);
    if let Some(r) = run {
        for (title, name, args) in [
            ("elektrawatt --help", "elektrawatt", ["--help"]),
            ("ew help", "ew", ["help"]),
        ] {
            out.push_str(&format!("\n== {title} ==\n"));
            match session::run_tool(r, &r.product_exe(name), &args, &r.dir) {
                Ok((_, stdout, _)) => out.push_str(&stdout),
                Err(_) => out.push_str("(not available)\n"),
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::testkit::{Tmp, test_run};

    /// ID-621: the run's copy of the harness runs the agent's commands only.
    #[test]
    fn a_runs_copy_refuses_the_orchestrators_subcommands() {
        for sub in [
            "stop",
            "bless",
            "selftest",
            "start",
            "compose",
            "replay",
            "bless-oracle",
            "rescore",
            "check-task",
            "nonsense",
        ] {
            assert_eq!(
                refused_in_copy(sub, true),
                Some(format!("harness: {sub} is the orchestrator's")),
                "{sub}"
            );
            assert_eq!(refused_in_copy(sub, false), None, "{sub} from the repository");
        }
        for sub in AGENT_COMMANDS.iter().chain(&["--help", "-h"]) {
            assert_eq!(refused_in_copy(sub, true), None, "{sub}");
        }
    }

    #[test]
    fn a_copy_is_the_executable_beside_a_run_json() {
        let tmp = Tmp::new("copy-of");
        let run = test_run(&tmp);
        std::fs::create_dir_all(run.path("bin")).unwrap();
        let exe = run.path("bin/xtask.exe");
        assert_eq!(copy_of(Some(&exe)).map(|r| r.dir), Some(run.dir.clone()));
        // The build's own xtask (target/debug) and a bin folder without a run.json are no copy.
        assert!(copy_of(Some(&tmp.0.join("target/debug/xtask.exe"))).is_none());
        std::fs::create_dir_all(tmp.0.join("other/bin")).unwrap();
        assert!(copy_of(Some(&tmp.0.join("other/bin/xtask.exe"))).is_none());
        assert!(copy_of(None).is_none());
    }

    /// ID-621: from a run's copy, `serve` serves that run's folder only: not another run, not a repository.
    #[test]
    fn a_copy_serves_its_own_run_only() {
        let tmp = Tmp::new("serve-own");
        let (drafter, check) = crate::blind::testkit::check_pair(&tmp);
        let arg = |r: &Run| r.dir.to_string_lossy().into_owned();
        assert!(serves_own_run(&check, Some(&arg(&check))));
        // Another spelling of the same folder is the same folder.
        let dotted = check.dir.join("..").join("check");
        assert!(serves_own_run(&check, Some(&dotted.to_string_lossy())));
        assert!(!serves_own_run(&check, Some(&arg(&drafter))), "another run");
        assert!(
            !serves_own_run(&check, Some(&tmp.0.to_string_lossy())),
            "a repository-like folder"
        );
        assert!(!serves_own_run(&check, Some("")));
        assert!(!serves_own_run(&check, None));
        // Nothing was created in the folders it refused.
        assert!(!drafter.path("session.json").exists() && !tmp.0.join("config").exists());
    }

    /// `resume` deletes `work/` and gives a check run its project back before the keeper replays the prelude.
    #[test]
    fn resume_puts_a_check_runs_project_back() {
        let tmp = Tmp::new("resume-check");
        let (_drafter, check) = crate::blind::testkit::check_pair(&tmp);
        std::fs::create_dir_all(check.path("work")).unwrap();
        std::fs::write(check.path("work/leftover.txt"), "x").unwrap();
        // There is no copied harness in the fixture, so the keeper cannot start: the folder is what is checked.
        assert!(resume(&check).is_err());
        assert!(check.path("work/project/marker.txt").is_file());
        assert!(!check.path("work/leftover.txt").exists());
    }

    /// ID-621: from a copy, the help and the reference come from the run folder and never from a repository.
    #[test]
    fn a_copy_reads_its_own_files_and_never_falls_back_to_the_repository() {
        let tmp = Tmp::new("copy-help");
        let run = test_run(&tmp);
        // A repository that has the files, and a run folder that does not: the copy must fail, not read the repository.
        let repo = tmp.0.join("repo");
        for (rel, text) in [
            ("tools/blind/HELP.md", "repo help\n"),
            ("tools/blind/reference/vocabulary.txt", "# v\n"),
            (
                "tools/blind/reference/actions.txt",
                "a View \"x\" menu none shortcut none context global params \"\"\n",
            ),
        ] {
            let p = repo.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        assert!(help_text(None, Some(&run)).is_err());
        assert!(load_reference(Some(&run), None).is_err());
        // The orchestrator, with the repository, reads the repository's.
        assert!(help_text(Some(&repo), None).unwrap().starts_with("repo help"));
        assert!(load_reference(None, Some(&repo)).unwrap().find("a").is_some());
        // With its own files the copy reads them, and says so in its output.
        std::fs::create_dir_all(run.path(".harness/reference")).unwrap();
        std::fs::write(run.path(".harness/HELP.md"), "own help\n").unwrap();
        std::fs::write(run.path(".harness/reference/vocabulary.txt"), "# own\n").unwrap();
        std::fs::write(
            run.path(".harness/reference/actions.txt"),
            "b View \"y\" menu none shortcut none context global params \"\"\n",
        )
        .unwrap();
        let text = help_text(None, Some(&run)).unwrap();
        assert!(
            text.starts_with("own help") && text.contains("# own") && !text.contains("repo help"),
            "{text}"
        );
        assert!(load_reference(Some(&run), None).unwrap().find("b").is_some());
        // Even with a repository at hand, the run's own file wins.
        assert!(
            load_reference(Some(&run), Some(&repo))
                .unwrap()
                .find("a")
                .is_none()
        );
    }
}
