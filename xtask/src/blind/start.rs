// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `start` (WP-S6.13 §5.1, §5.4): a run folder outside the repository with the copies the run needs, the composed
//! prompt, the prelude and, unless `--stateless`, a keeper.
//!
//! The agent never needs the repository: the harness, the product binaries, the user guide, the starter content and
//! the reference files are copied into the folder. The sample project is not: `EW_SAMPLE_DIR` points at an empty
//! folder, and `golden/` is hashed here and again at `stop` (D16).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::compose::{ComposeIn, REFERENCE_FILES, compose, run_files};
use super::run::{
    Counts, HARNESS_VERSION, Kind, RUN_SCHEMA, Run, RunJson, copy_dir, script_header, tree_sha, turns_header,
    valid_run_id,
};

/// The arguments of `start`.
#[derive(Clone, Debug)]
pub struct StartArgs {
    /// `free`, `gate` or `check`.
    pub kind: Kind,
    /// The task (`gpp-101-103`, `free`, `selftest`).
    pub task: String,
    /// The drafter's persona.
    pub persona: Option<String>,
    /// For a check run: the drafter run's ID.
    pub of: Option<String>,
    /// The folios the epilogue describes.
    pub folios: Vec<String>,
    /// The run's ID.
    pub run_id: String,
    /// The folder the run folders live in (default `<temp>/ew-blind`).
    pub out: Option<PathBuf>,
    /// The folder of the built binaries (default `target/debug`).
    pub bin: Option<PathBuf>,
    /// Whether the run has no keeper.
    pub stateless: bool,
}

/// The folder a run `id` gets under `out`.
pub fn run_dir(out: Option<&Path>, id: &str) -> PathBuf {
    out.map_or_else(|| Run::default_dir(id), |o| o.join(id))
}

/// Whether `dir` lies inside the repository at `root` (even where `dir` does not exist yet).
pub fn inside_repo(root: &Path, dir: &Path) -> bool {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut rest = Vec::new();
    let mut at = dir.to_path_buf();
    while !at.exists() {
        match (at.file_name().map(std::ffi::OsStr::to_owned), at.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                at = parent.to_path_buf();
            }
            _ => break,
        }
    }
    let mut full = at.canonicalize().unwrap_or(at);
    for part in rest.iter().rev() {
        full.push(part);
    }
    full.starts_with(&root)
}

/// The SHA-256 over `golden/` and a check that two such digests agree.
///
/// # Errors
///
/// The hash cannot be taken, or `golden/` changed.
pub fn check_golden(root: &Path, expected: &str) -> Result<()> {
    let now = tree_sha(&root.join("golden"))?;
    if now != expected {
        bail!(
            "golden/ changed during the run (digest {now}, at the start {expected}): the fixture is read through copies only"
        );
    }
    Ok(())
}

fn exe_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

fn copy_file(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from, to).with_context(|| format!("copying {} (build it first)", from.display()))?;
    Ok(())
}

/// Copies the stopped drafter run's project into the check run's `work/project`.
fn seed_from(drafter: &Run, run: &Run) -> Result<()> {
    let project = drafter.path("work/project");
    if !project.is_dir() {
        bail!("run `{}` has no project to check", drafter.dir.display());
    }
    copy_dir(&project, &run.path("work/project"))
}

/// Puts a check run's copy of the drafter's project back after `work/` was deleted (`resume`, a stateless call): the
/// check prelude opens `project`. A run of another kind needs nothing.
///
/// # Errors
///
/// The drafter run or its project is gone.
pub fn reseed_check_project(run: &Run) -> Result<()> {
    let rj = run.load()?;
    if rj.kind != "check" {
        return Ok(());
    }
    let of = rj.of.context("a check run names its drafter")?;
    let drafter = super::run::locate(&of, run.dir.parent(), None)?;
    seed_from(&drafter, run)
}

/// The parts of `start` the tests reach: everything but the keeper. `version` reads `elektrawatt --version`.
///
/// # Errors
///
/// A folder inside the repository, an existing run, a missing binary or file, a drafter run that is not stopped.
pub fn prepare(root: &Path, a: &StartArgs, version: &dyn Fn(&Path) -> Result<String>) -> Result<Run> {
    if !valid_run_id(&a.run_id) {
        bail!("`{}` is not a run ID (letters, digits, `.`, `_`, `-`)", a.run_id);
    }
    let run = Run {
        dir: run_dir(a.out.as_deref(), &a.run_id),
    };
    if inside_repo(root, &run.dir) {
        bail!(
            "the run folder {} is inside the repository: a run lives outside it",
            run.dir.display()
        );
    }
    if run.dir.exists() && std::fs::read_dir(&run.dir)?.next().is_some() {
        bail!("the run folder {} exists and is not empty", run.dir.display());
    }
    if a.kind == Kind::Check && a.of.is_none() {
        bail!("a check run needs `--of <drafter run>`");
    }
    if a.kind != Kind::Check && a.of.is_some() {
        bail!("`--of` belongs to a check run");
    }
    // A check run audits a stopped drafter run.
    let drafter = if a.kind == Kind::Check {
        let of = a.of.as_deref().unwrap_or_default();
        let d = super::run::locate(of, a.out.as_deref(), None)?;
        if !d.path("out/report.md").exists() {
            bail!("run `{of}` is not stopped: `stop` it first");
        }
        Some(d)
    } else {
        None
    };
    let folios = run_folios(root, a, drafter.as_ref())?;
    let golden = tree_sha(&root.join("golden"))?;
    let bin = a.bin.clone().unwrap_or_else(|| root.join("target/debug"));
    let own = std::env::current_exe().ok();
    // The copied harness is the build's `xtask` next to the product binaries, else the one that is running.
    let xtask_from = [bin.join(exe_name("xtask")), own.clone().unwrap_or_default()]
        .into_iter()
        .find(|p| p.is_file())
        .context("no xtask binary to copy")?;
    for sub in [
        "bin",
        ".harness/bin",
        ".harness/empty-sample",
        ".harness/reference",
        ".harness/looks",
        "work",
        "config",
        "out",
    ] {
        std::fs::create_dir_all(run.path(sub))?;
    }
    copy_file(&xtask_from, &run.harness_exe())?;
    for name in ["elektrawatt", "ew"] {
        copy_file(&bin.join(exe_name(name)), &run.product_exe(name))?;
    }
    // The user guide as the build made it; a run without a built guide has an empty one (the guide is a courtesy).
    let book = root.join("target/book");
    if book.is_dir() {
        copy_dir(&book, &run.path("book"))?;
    } else {
        std::fs::create_dir_all(run.path("book"))?;
    }
    // The starter content (symbols, function definitions, forms) the product looks for next to its executable.
    copy_dir(&root.join("content"), &run.path(".harness/content"))?;
    for rel in REFERENCE_FILES {
        copy_file(
            &root.join(rel),
            &run.path(&format!(
                ".harness/reference/{}",
                rel.rsplit('/').next().unwrap_or(rel)
            )),
        )?;
    }
    copy_file(&root.join("tools/blind/HELP.md"), &run.path(".harness/HELP.md"))?;

    let blind = run.blind_command();
    let inp = compose_input(root, a, &blind, &folios);
    let composed = compose(&inp)?;
    let files = run_files(&inp, &composed)?;
    std::fs::write(run.path("prompt.md"), &composed.text)?;

    // The prelude: the task's own, or the harness's for a check run (the drafter's project, opened).
    let prelude = if let Some(drafter) = &drafter {
        seed_from(drafter, &run)?;
        copy_reports(drafter, &run)?;
        format!(
            "{}\n{}\n",
            script_header("check prelude"),
            super::run::step_line("file.open", &serde_json::json!({"dir": "project"}))
        )
    } else {
        let task_prelude = root.join(format!("tools/blind/tasks/{}/prelude.uiscript", a.task));
        if task_prelude.is_file() {
            std::fs::read_to_string(&task_prelude)?.replace("\r\n", "\n")
        } else {
            format!("{}\n", script_header(&format!("{} prelude", a.task)))
        }
    };
    std::fs::write(run.path("prelude.uiscript"), prelude)?;
    std::fs::write(
        run.path("steps.uiscript"),
        format!("{}\n", script_header("steps")),
    )?;
    std::fs::write(run.path("turns.jsonl"), format!("{}\n", turns_header()))?;
    std::fs::write(run.path("drafter.md"), "")?;
    let app_version = version(&run.product_exe("elektrawatt"))?;
    let json = RunJson {
        schema: RUN_SCHEMA.to_owned(),
        run: a.run_id.clone(),
        kind: a.kind.word().to_owned(),
        task: a.task.clone(),
        persona: a.persona.clone(),
        of: a.of.clone(),
        folios,
        caps: a.kind.caps(),
        files,
        golden,
        app_version,
        harness_version: HARNESS_VERSION.to_owned(),
        stateless: a.stateless,
        counts: Counts::default(),
        end: None,
        handed_back: false,
        diverged_at: None,
        ci: "none".to_owned(),
        stale: None,
        clean_before_epilogue: None,
        child_exit: None,
        golden_changed: false,
    };
    run.save(&json)?;
    Ok(run)
}

fn compose_input<'a>(
    root: &'a Path,
    a: &'a StartArgs,
    blind: &'a str,
    folios: &'a [String],
) -> ComposeIn<'a> {
    ComposeIn {
        root,
        kind: a.kind,
        task: &a.task,
        persona: a.persona.as_deref(),
        run_id: &a.run_id,
        blind,
        caps: a.kind.caps(),
        folios,
    }
}

/// The folios of a run: those given with `--folios`; else, for a check run, the drafter's; else those of the task's
/// oracle (`oracle/counts.toml`); a task without an oracle has none (the sheet is shown whole).
///
/// # Errors
///
/// A malformed `counts.toml`, or a drafter run that cannot be read.
pub fn run_folios(root: &Path, a: &StartArgs, drafter: Option<&Run>) -> Result<Vec<String>> {
    if !a.folios.is_empty() {
        return Ok(a.folios.clone());
    }
    if let Some(d) = drafter {
        return Ok(d.load()?.folios);
    }
    let counts = super::score::task_dir(root, &a.task).join("oracle/counts.toml");
    if counts.is_file() {
        let file: super::score::CountsFile = super::score::read_toml(&counts)?;
        return Ok(file.folio.into_iter().map(|c| c.folio).collect());
    }
    Ok(Vec::new())
}

/// Copies the drafter run's reports (`out/check.txt`, `bom.csv`, `score.json`, `report.md`) into the check run's
/// `reports/`: what `report <run> check|bom|score` of the check run prints. The drafter's notes and hand-back stay
/// where they are.
fn copy_reports(drafter: &Run, run: &Run) -> Result<()> {
    std::fs::create_dir_all(run.path("reports"))?;
    for name in ["check.txt", "bom.csv", "score.json", "report.md"] {
        let from = drafter.path(&format!("out/{name}"));
        if from.is_file() {
            std::fs::copy(&from, run.path(&format!("reports/{name}")))
                .with_context(|| format!("copying {}", from.display()))?;
        }
    }
    Ok(())
}

/// Reads `elektrawatt --version` of the copied binary: the text after the program's name.
///
/// # Errors
///
/// The program does not run or prints nothing.
pub fn read_version(exe: &Path) -> Result<String> {
    let out = std::process::Command::new(exe)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .output()
        .with_context(|| format!("running {} --version", exe.display()))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let v = text.trim().strip_prefix("elektrawatt ").unwrap_or(text.trim());
    if v.is_empty() {
        bail!("{} --version printed nothing", exe.display());
    }
    Ok(v.to_owned())
}

/// `start`: prepares the folder and, unless `--stateless`, starts the keeper.
///
/// # Errors
///
/// See [`prepare`]; the keeper's failure.
pub fn start(root: &Path, a: &StartArgs) -> Result<Run> {
    let run = prepare(root, a, &read_version)?;
    if !a.stateless {
        super::keeper::spawn_keeper(&run, false, super::keeper::DEFAULT_IDLE_MINUTES)?;
    }
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::testkit::Tmp;

    /// A repository root of a test: golden, content, tools/blind and a `target/debug` of dummy binaries.
    fn fake_repo(tmp: &Tmp) -> PathBuf {
        let root = tmp.0.join("repo");
        let w = |rel: &str, text: &str| {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        w("golden/J-E-2416.ew/project.json", "{}\n");
        w("content/symbols/x.json", "{}\n");
        w("tools/blind/HELP.md", "Help {blind}\n");
        w(
            "tools/blind/reference/actions.txt",
            "a View \"x\" menu none shortcut none context global params \"\"\n",
        );
        w("tools/blind/reference/vocabulary.txt", "# v\n");
        w("tools/blind/tasks/selftest/task.md", "Selftest\n");
        w(
            "tools/blind/tasks/selftest/prelude.uiscript",
            "{\"$schema\":\"ew.uiscript/1\",\"label\":\"p\"}\n",
        );
        for name in ["xtask", "elektrawatt", "ew"] {
            w(&format!("target/debug/{}", exe_name(name)), "binary");
        }
        root
    }

    fn args(tmp: &Tmp, id: &str) -> StartArgs {
        StartArgs {
            kind: Kind::Free,
            task: "selftest".to_owned(),
            persona: None,
            of: None,
            folios: vec!["101".to_owned()],
            run_id: id.to_owned(),
            out: Some(tmp.0.join("runs")),
            bin: None,
            stateless: true,
        }
    }

    /// BD9: the run folder is outside the repository and holds the copies of §5.1; no time stamp or host name.
    #[test]
    fn a_run_folder_holds_the_copies_and_lives_outside_the_repository() {
        let tmp = Tmp::new("start");
        let root = fake_repo(&tmp);
        let run = prepare(&root, &args(&tmp, "r1"), &|_| Ok("0.0.0".to_owned())).unwrap();
        assert!(!inside_repo(&root, &run.dir));
        for rel in [
            "bin",
            ".harness/bin",
            ".harness/empty-sample",
            ".harness/content/symbols/x.json",
            ".harness/reference/actions.txt",
            ".harness/reference/vocabulary.txt",
            ".harness/HELP.md",
            "book",
            "run.json",
            "prompt.md",
            "prelude.uiscript",
            "steps.uiscript",
            "turns.jsonl",
            "drafter.md",
            "work",
            "config",
            "out",
        ] {
            assert!(run.path(rel).exists(), "{rel}");
        }
        assert!(run.harness_exe().is_file());
        assert!(run.product_exe("elektrawatt").is_file() && run.product_exe("ew").is_file());
        assert!(
            std::fs::read_dir(run.path(".harness/empty-sample"))
                .unwrap()
                .next()
                .is_none()
        );
        let rj = run.load().unwrap();
        assert_eq!(rj.golden, tree_sha(&root.join("golden")).unwrap());
        assert_eq!(rj.app_version, "0.0.0");
        assert!(rj.files.contains_key("tools/blind/HELP.md"));
        let prompt = std::fs::read_to_string(run.path("prompt.md")).unwrap();
        assert!(prompt.contains(&run.blind_command()), "{prompt}");
        // No time stamp, host or user in anything the harness wrote (the folder's own path is only in the prompt).
        for rel in ["run.json", "turns.jsonl", "steps.uiscript", "prelude.uiscript"] {
            let text = std::fs::read_to_string(run.path(rel)).unwrap();
            for word in ["host", "user", "timestamp", "2026"] {
                assert!(!text.contains(word), "{rel}: {word}");
            }
        }
    }

    #[test]
    fn a_folder_inside_the_repository_or_an_existing_run_is_refused() {
        let tmp = Tmp::new("start-refuse");
        let root = fake_repo(&tmp);
        let mut a = args(&tmp, "r1");
        a.out = Some(root.join("runs"));
        let err = prepare(&root, &a, &|_| Ok("0".to_owned())).unwrap_err();
        assert!(err.to_string().contains("inside the repository"), "{err}");
        assert!(inside_repo(&root, &root.join("a/b/c")));
        assert!(!inside_repo(&root, &tmp.0.join("elsewhere")));
        let a = args(&tmp, "r2");
        prepare(&root, &a, &|_| Ok("0".to_owned())).unwrap();
        let err = prepare(&root, &a, &|_| Ok("0".to_owned())).unwrap_err();
        assert!(err.to_string().contains("not empty"), "{err}");
        assert!(prepare(&root, &args(&tmp, "../x"), &|_| Ok("0".to_owned())).is_err());
        let mut check = args(&tmp, "r3");
        check.kind = Kind::Check;
        assert!(
            prepare(&root, &check, &|_| Ok("0".to_owned())).is_err(),
            "a check run needs --of"
        );
    }

    /// The folios of a run: `--folios`, else the drafter's (a check run), else the task's oracle, else none.
    #[test]
    fn the_folios_of_a_run_default_to_the_oracles_or_the_drafters() {
        let tmp = Tmp::new("start-folios");
        let root = fake_repo(&tmp);
        let w = |rel: &str, text: &str| {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        w(
            "tools/blind/tasks/gpp/oracle/counts.toml",
            "[[folio]]\nfolio = \"101\"\nreps = 1\nconnections = 0\njunctions = 0\nsource = \"t\"\n\n[[folio]]\nfolio = \"102\"\nreps = 1\nconnections = 0\njunctions = 0\nsource = \"t\"\n",
        );
        let mut a = args(&tmp, "r1");
        a.task = "gpp".to_owned();
        a.folios = Vec::new();
        assert_eq!(run_folios(&root, &a, None).unwrap(), ["101", "102"]);
        a.folios = vec!["102".to_owned()];
        assert_eq!(run_folios(&root, &a, None).unwrap(), ["102"], "--folios wins");
        // A task without an oracle has none: the sheet is shown whole.
        a.task = "selftest".to_owned();
        a.folios = Vec::new();
        assert!(run_folios(&root, &a, None).unwrap().is_empty());
        // A check run takes the drafter's, unless it is given its own.
        let (drafter, _check) = crate::blind::testkit::check_pair(&tmp);
        drafter
            .update(|j| j.folios = vec!["101".to_owned(), "103".to_owned()])
            .unwrap();
        a.task = "gpp".to_owned();
        assert_eq!(run_folios(&root, &a, Some(&drafter)).unwrap(), ["101", "103"]);
    }

    /// A check run gets the drafter's reports in `reports/` (and nothing of its notes), and `report` of the check run
    /// prints them; the drafter's folios are the check run's.
    #[test]
    fn a_check_run_reads_the_drafters_reports_and_not_its_notes() {
        let tmp = Tmp::new("start-reports");
        let root = fake_repo(&tmp);
        std::fs::write(root.join("tools/blind/CHECKER.md"), "You check {run}.\n").unwrap();
        let mut a = args(&tmp, "d1");
        a.folios = vec!["101".to_owned()];
        let drafter = prepare(&root, &a, &|_| Ok("0".to_owned())).unwrap();
        // The drafter is not stopped yet: no check run.
        let mut c = args(&tmp, "c1");
        c.kind = Kind::Check;
        c.of = Some("d1".to_owned());
        c.folios = Vec::new();
        let err = prepare(&root, &c, &|_| Ok("0".to_owned())).unwrap_err();
        assert!(err.to_string().contains("is not stopped"), "{err}");
        std::fs::create_dir_all(drafter.path("work/project")).unwrap();
        std::fs::create_dir_all(drafter.path("out")).unwrap();
        for (name, text) in [
            ("report.md", "# Run d1\n"),
            ("score.json", "{\"outcome\":\"pass\"}\n"),
            ("check.txt", "no findings\n"),
            ("bom.csv", "row,Qty\n"),
            ("project-t1.pdf", "pdf"),
        ] {
            std::fs::write(drafter.path(&format!("out/{name}")), text).unwrap();
        }
        std::fs::write(drafter.path("drafter.md"), "## Turn 1\nsecret notes\n").unwrap();
        std::fs::write(drafter.path("handback.md"), "1. Keep or drop\n").unwrap();
        let check = prepare(&root, &c, &|_| Ok("0".to_owned())).unwrap();
        assert_eq!(check.load().unwrap().folios, ["101"], "the drafter's folios");
        for name in ["report.md", "score.json", "check.txt", "bom.csv"] {
            assert!(check.path(&format!("reports/{name}")).is_file(), "{name}");
        }
        assert!(!check.path("reports/project-t1.pdf").exists(), "no PDF");
        // Nothing of the drafter's notes reaches the check run's folder.
        for rel in [
            "drafter.md",
            "handback.md",
            "reports/drafter.md",
            "reports/handback.md",
        ] {
            let text = std::fs::read_to_string(check.path(rel)).unwrap_or_default();
            assert!(
                !text.contains("secret") && !text.contains("Keep or drop"),
                "{rel}"
            );
        }
        use crate::blind::protocol::report;
        assert_eq!(report(&check, "check").unwrap(), "no findings\n");
        assert_eq!(report(&check, "bom").unwrap(), "row,Qty\n");
        let score = report(&check, "score").unwrap();
        assert!(
            score.contains("\"outcome\":\"pass\"") && score.contains("# Run d1"),
            "{score}"
        );
        assert!(
            report(&check, "nonsense")
                .unwrap()
                .starts_with("harness: report check, bom or score")
        );
        // A drafter run reads its own `out/`.
        assert_eq!(report(&drafter, "check").unwrap(), "no findings\n");
        std::fs::remove_file(drafter.path("out/check.txt")).unwrap();
        assert_eq!(
            report(&drafter, "check").unwrap(),
            "harness: out/check.txt does not exist yet\n"
        );
    }

    /// A check run's project is a copy of the drafter's in `work/`: it comes back after `work/` was deleted.
    #[test]
    fn a_check_runs_project_is_put_back() {
        let tmp = Tmp::new("start-reseed");
        let (_drafter, check) = crate::blind::testkit::check_pair(&tmp);
        assert!(!check.path("work/project").exists());
        reseed_check_project(&check).unwrap();
        assert_eq!(
            std::fs::read_to_string(check.path("work/project/marker.txt")).unwrap(),
            "the drafter's project"
        );
        // A run of another kind needs nothing; a drafter that is gone is an error.
        let plain_tmp = Tmp::new("start-reseed-plain");
        let plain = crate::blind::testkit::test_run(&plain_tmp);
        reseed_check_project(&plain).unwrap();
        assert!(!plain.path("work/project").exists());
        std::fs::remove_dir_all(tmp.0.join("runs/drafter")).unwrap();
        assert!(reseed_check_project(&check).is_err());
    }

    /// BD10: `golden/` is hashed at the start and compared at the stop; a changed byte fails the run.
    #[test]
    fn a_changed_golden_fails_the_check() {
        let tmp = Tmp::new("start-golden");
        let root = fake_repo(&tmp);
        let before = tree_sha(&root.join("golden")).unwrap();
        check_golden(&root, &before).unwrap();
        std::fs::write(root.join("golden/J-E-2416.ew/project.json"), "{ }\n").unwrap();
        let err = check_golden(&root, &before).unwrap_err();
        assert!(err.to_string().contains("golden/ changed"), "{err}");
        std::fs::write(root.join("golden/J-E-2416.ew/project.json"), "{}\n").unwrap();
        check_golden(&root, &before).unwrap();
        // A new file (a lock, a journal) is a change too.
        std::fs::write(root.join("golden/J-E-2416.ew/lock"), "").unwrap();
        assert!(check_golden(&root, &before).is_err());
    }
}
