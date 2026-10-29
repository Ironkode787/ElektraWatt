// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `start` (WP-S6.13 §5.1, §5.4): a run folder outside the repository with the copies the run needs, the composed
//! prompt, the prelude and, unless `--stateless`, a keeper.
//!
//! The agent never needs the repository: the harness, the product binaries, the user guide, the starter content and
//! the reference files are copied into the folder. The sample project is not: `EW_SAMPLE_DIR` points at an empty
//! folder, and `golden/` is hashed here and again at `stop` (D16).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::compose::{ComposeIn, REFERENCE_FILES, compose, run_files};
use super::run::{
    Counts, GOLDEN_PROJECT_DIR, GOLDEN_SCOPE_PROJECT, HARNESS_VERSION, Kind, RUN_SCHEMA, Run, RunJson,
    Senses, VISUAL_PPP, VISUAL_WINDOW, copy_dir, golden_digest, script_header, turns_header, valid_run_id,
};

/// The arguments of `start`.
#[derive(Clone, Debug)]
pub struct StartArgs {
    /// `free`, `gate`, `check` or `brief`.
    pub kind: Kind,
    /// `text` (the base specification) or `visual` (WP-S6.13 part 4).
    pub senses: Senses,
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

/// Compares the golden digest recorded in `rj` with the fixture now, by the run's `golden_scope` (D16): the golden
/// project fixture for a run recorded since FT-484, all of `golden/` for an older run (legacy scheme).
///
/// # Errors
///
/// The hash cannot be taken, or the fixture changed.
pub fn check_golden(root: &Path, rj: &RunJson) -> Result<()> {
    let now = golden_digest(root, rj.golden_scope.as_deref())?;
    if now != rj.golden {
        let what = if rj.golden_scope.is_some() {
            GOLDEN_PROJECT_DIR
        } else {
            "golden/"
        };
        bail!(
            "golden/ changed during the run ({what} digest {now}, at the start {}): the fixture is read through copies only",
            rj.golden
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
    if a.kind == Kind::Brief && !a.task.starts_with("brief-") {
        bail!("a brief run's task is a brief (`brief-<name>`)");
    }
    if a.kind != Kind::Brief && a.task.starts_with("brief-") && a.kind != Kind::Check {
        bail!("the task `{}` is a brief: start it with `--kind brief`", a.task);
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
    let golden = golden_digest(root, Some(GOLDEN_SCOPE_PROJECT))?;
    // Every option is validated here, before anything is created: the composition checks the persona, the task and the
    // kind (FT-484), and the sources are looked up now.
    let blind = run.blind_command();
    let frames = run.path("frames").display().to_string();
    let inp = compose_input(root, a, &blind, &frames, &folios);
    let composed = compose(&inp)?;
    let files = run_files(&inp, &composed)?;
    let bin = a.bin.clone().unwrap_or_else(|| root.join("target/debug"));
    let own = std::env::current_exe().ok();
    // The copied harness is the build's `xtask` next to the product binaries, else the one that is running.
    let xtask_from = [bin.join(exe_name("xtask")), own.clone().unwrap_or_default()]
        .into_iter()
        .find(|p| p.is_file())
        .context("no xtask binary to copy")?;
    // A refusal part-way removes only the folder this call created, never one that was there before.
    let existed = run.dir.exists();
    let built = populate(
        root,
        a,
        version,
        &run,
        &Inputs {
            bin,
            xtask_from,
            drafter,
            composed,
            files,
            folios,
            golden,
        },
    );
    if built.is_err() && !existed {
        let _ = std::fs::remove_dir_all(&run.dir);
    }
    built?;
    Ok(run)
}

/// What `prepare` has validated and computed before it creates the run folder.
struct Inputs {
    bin: PathBuf,
    xtask_from: PathBuf,
    drafter: Option<Run>,
    composed: super::compose::Composed,
    files: BTreeMap<String, String>,
    folios: Vec<String>,
    golden: String,
}

/// Creates the run folder's content and `run.json` (the part of `prepare` that writes).
fn populate(
    root: &Path,
    a: &StartArgs,
    version: &dyn Fn(&Path) -> Result<String>,
    run: &Run,
    inputs: &Inputs,
) -> Result<()> {
    let Inputs {
        bin,
        xtask_from,
        drafter,
        composed,
        files,
        folios,
        golden,
    } = inputs;
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
    copy_file(xtask_from, &run.harness_exe())?;
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
    let visual = a.senses == Senses::Visual;
    if visual {
        copy_file(
            &root.join("tools/blind/HELP-VISUAL.md"),
            &run.path(".harness/HELP-VISUAL.md"),
        )?;
        std::fs::create_dir_all(run.path("frames"))?;
    }

    std::fs::write(run.path("prompt.md"), &composed.text)?;

    // The prelude: the task's own, or the harness's for a check run (the drafter's project, opened).
    let prelude = if let Some(drafter) = drafter {
        seed_from(drafter, run)?;
        copy_reports(drafter, run)?;
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
    // A visual run's steps begin with its start frame (visual spec §3.1 item 4), which the keeper paints after the
    // prelude.
    let start_frame = if visual {
        format!("{}\n", super::visual::start_frame_line())
    } else {
        String::new()
    };
    std::fs::write(
        run.path("steps.uiscript"),
        format!("{}\n{start_frame}", script_header("steps")),
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
        folios: folios.clone(),
        caps: a.kind.caps_for(a.senses),
        files: files.clone(),
        golden: golden.clone(),
        golden_scope: Some(GOLDEN_SCOPE_PROJECT.to_owned()),
        app_version,
        harness_version: HARNESS_VERSION.to_owned(),
        stateless: a.stateless,
        senses: visual.then(|| a.senses.word().to_owned()),
        ppp: visual.then_some(VISUAL_PPP),
        window: visual.then_some(VISUAL_WINDOW),
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
    Ok(())
}

fn compose_input<'a>(
    root: &'a Path,
    a: &'a StartArgs,
    blind: &'a str,
    frames: &'a str,
    folios: &'a [String],
) -> ComposeIn<'a> {
    ComposeIn {
        root,
        kind: a.kind,
        task: &a.task,
        persona: a.persona.as_deref(),
        run_id: &a.run_id,
        blind,
        caps: a.kind.caps_for(a.senses),
        senses: a.senses,
        frames,
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
    for name in [
        "check.txt",
        "bom.csv",
        "score.json",
        "report.md",
        "connections.txt",
        "pages.txt",
    ] {
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

/// Whether the copied product can paint a frame here: it runs the visual driver over an empty script with a frames
/// folder, which ends at once with exit 0, or with exit 2 when the host has no offscreen adapter.
fn has_offscreen_adapter(run: &Run) -> bool {
    let script = run.path(".harness/probe.uiscript");
    if std::fs::write(&script, format!("{}\n", script_header("probe"))).is_err() {
        return false;
    }
    let probe = run.path(".harness/probe");
    let ok = std::process::Command::new(run.product_exe("elektrawatt"))
        .args(["--headless", "--visual", "--script"])
        .arg(&script)
        .arg("--frames")
        .arg(&probe)
        .arg("--config-dir")
        .arg(&probe)
        .current_dir(run.path(".harness"))
        .env("EW_CONTENT_DIR", run.path(".harness/content"))
        .stdin(std::process::Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success());
    let _ = std::fs::remove_file(&script);
    let _ = std::fs::remove_dir_all(&probe);
    ok
}

/// `start`: for a brief, the proof that the current build can complete it (`check-brief`); then the folder, and, unless
/// `--stateless`, the keeper. A visual run needs an offscreen adapter.
///
/// # Errors
///
/// See [`prepare`]; a brief that is not runnable; no offscreen adapter; the keeper's failure.
pub fn start(root: &Path, a: &StartArgs) -> Result<Run> {
    if a.kind == Kind::Brief {
        let bin = a.bin.clone().unwrap_or_else(|| root.join("target/debug"));
        let c = super::brief::check_brief(root, &a.task, &bin)?;
        if !c.runnable() {
            bail!(
                "harness: the brief `{}` is not runnable on this build:\n{}",
                a.task,
                c.problems.join("\n")
            );
        }
    }
    let run = prepare(root, a, &read_version)?;
    if a.senses == Senses::Visual && !has_offscreen_adapter(&run) {
        let _ = std::fs::remove_dir_all(&run.dir);
        bail!("{}", super::visual::NO_ADAPTER);
    }
    if !a.stateless {
        super::keeper::spawn_keeper(&run, false, super::keeper::DEFAULT_IDLE_MINUTES)?;
    }
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::run::tree_sha;
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
            senses: crate::blind::run::Senses::Text,
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
        assert_eq!(rj.golden, tree_sha(&root.join("golden/J-E-2416.ew")).unwrap());
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
                .starts_with("harness: report check, bom, score, connections, pages or pdf")
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

    /// BD10 (FT-484): the golden project fixture is hashed at the start and compared at the stop; a changed byte or a
    /// new file under it fails the run, a change next to it (`expected/`, the README) does not.
    #[test]
    fn a_changed_golden_fails_the_check() {
        let tmp = Tmp::new("start-golden");
        let root = fake_repo(&tmp);
        let mut rj = crate::blind::testkit::new_run_json("g", Kind::Free);
        rj.golden = golden_digest(&root, Some(GOLDEN_SCOPE_PROJECT)).unwrap();
        rj.golden_scope = Some(GOLDEN_SCOPE_PROJECT.to_owned());
        check_golden(&root, &rj).unwrap();
        // Expected outputs and documents beside the project may change during a run.
        std::fs::create_dir_all(root.join("golden/J-E-2416/expected")).unwrap();
        std::fs::write(
            root.join("golden/J-E-2416/expected/x.csv"),
            "a
",
        )
        .unwrap();
        std::fs::write(
            root.join("golden/J-E-2416/README.md"),
            "r
",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.0.join("runs/d1/work/project")).unwrap();
        check_golden(&root, &rj).unwrap();
        std::fs::write(
            root.join("golden/J-E-2416.ew/project.json"),
            "{ }
",
        )
        .unwrap();
        let err = check_golden(&root, &rj).unwrap_err();
        assert!(err.to_string().contains("golden/ changed"), "{err}");
        std::fs::write(
            root.join("golden/J-E-2416.ew/project.json"),
            "{}
",
        )
        .unwrap();
        check_golden(&root, &rj).unwrap();
        // A new file (a lock, a journal) is a change too.
        std::fs::write(root.join("golden/J-E-2416.ew/lock"), "").unwrap();
        assert!(check_golden(&root, &rj).is_err());
    }

    /// A run folder recorded before FT-484 has no `golden_scope`: its digest covers all of `golden/` (legacy scheme).
    #[test]
    fn an_old_scheme_run_is_compared_with_the_whole_golden_digest() {
        let tmp = Tmp::new("start-golden-legacy");
        let root = fake_repo(&tmp);
        let mut rj = crate::blind::testkit::new_run_json("g", Kind::Free);
        assert_eq!(rj.golden_scope, None);
        rj.golden = tree_sha(&root.join("golden")).unwrap();
        check_golden(&root, &rj).unwrap();
        // The field is absent from an old run.json and defaults to the legacy scheme.
        let text = serde_json::to_string(&rj).unwrap();
        assert!(!text.contains("golden_scope"), "{text}");
        let back: RunJson = serde_json::from_str(&text).unwrap();
        assert_eq!(back.golden_scope, None);
        // Under the legacy scheme a change beside the project voids the run.
        std::fs::create_dir_all(root.join("golden/J-E-2416/expected")).unwrap();
        std::fs::write(
            root.join("golden/J-E-2416/expected/x.csv"),
            "a
",
        )
        .unwrap();
        assert!(check_golden(&root, &rj).is_err());
        // A new run records the project scope.
        let run = prepare(&root, &args(&tmp, "new"), &|_| Ok("0".to_owned())).unwrap();
        assert_eq!(
            run.load().unwrap().golden_scope.as_deref(),
            Some(GOLDEN_SCOPE_PROJECT)
        );
    }

    /// FT-484: a refused `start` leaves no run folder behind; the next correct `start` of the same name succeeds.
    #[test]
    fn a_refused_start_leaves_no_run_folder() {
        let tmp = Tmp::new("start-refused-clean");
        let root = fake_repo(&tmp);
        // A task whose prompt takes a persona (the selftest's takes none).
        for rel in [
            "tools/blind/CHECKER.md",
            "tools/blind/BLIND_DRAFTER.md",
            "tools/blind/tasks/t1/task.md",
        ] {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x\n").unwrap();
        }
        // A stopped drafter run to check.
        prepare(&root, &args(&tmp, "d1"), &|_| Ok("0".to_owned())).unwrap();
        std::fs::write(
            tmp.0.join("runs/d1/out/report.md"),
            "r
",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.0.join("runs/d1/work/project")).unwrap();
        let mut check = args(&tmp, "r1");
        check.kind = Kind::Check;
        check.task = "t1".to_owned();
        check.of = Some("d1".to_owned());
        check.persona = Some("x".to_owned());
        let err = prepare(&root, &check, &|_| Ok("0".to_owned())).unwrap_err();
        assert!(err.to_string().contains("a check run has no persona"), "{err}");
        assert!(!tmp.0.join("runs/r1").exists());
        // A drafter run refused for its persona leaves nothing either.
        let mut a = args(&tmp, "r2");
        a.task = "t1".to_owned();
        a.persona = Some("no-such-persona".to_owned());
        assert!(prepare(&root, &a, &|_| Ok("0".to_owned())).is_err());
        assert!(!tmp.0.join("runs/r2").exists());
        // A failure part-way (the version cannot be read) removes the folder this call created.
        let a = args(&tmp, "r3");
        assert!(prepare(&root, &a, &|_| anyhow::bail!("no version")).is_err());
        assert!(!tmp.0.join("runs/r3").exists());
        // The same names then start (r1 as the check run it was refused as, without the persona).
        check.persona = None;
        prepare(&root, &check, &|_| Ok("0".to_owned())).unwrap();
        prepare(&root, &a, &|_| Ok("0".to_owned())).unwrap();
        // A folder that existed before (empty) survives a refusal.
        std::fs::create_dir_all(tmp.0.join("runs/r4")).unwrap();
        let mut a = args(&tmp, "r4");
        a.kind = Kind::Check;
        a.task = "t1".to_owned();
        a.of = Some("d1".to_owned());
        a.persona = Some("x".to_owned());
        assert!(prepare(&root, &a, &|_| Ok("0".to_owned())).is_err());
        assert!(tmp.0.join("runs/r4").is_dir());
    }
}
