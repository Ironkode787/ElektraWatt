// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `stop` (WP-S6.13 §5.8): the epilogue, the product's exit, the exports of the copied `ew`, the second hash of `golden/`
//! and the report.
//!
//! The epilogue is part of `steps.uiscript`: `page.describe` for every folio of the run, `file.save` only when the
//! project is dirty (recorded, so that the scoring can see that the agent did not save), and `app.quit`. Then `stop`
//! scores the run (`score.json`, part 2, [`super::score`]) and writes `out/report.md` with the score, the protocol's
//! counts and observations.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Result, bail};
use serde_json::{Value, json};

use super::run::{Run, Turn, append, sha256_hex, step_line};
use super::score::{ExportFacts, score_markdown, score_run};
use super::session::{Backend, Sent, Step};
use super::start::check_golden;

/// The headings of the checker's audit (§5.6).
pub const CHECKER_HEADINGS: [&str; 5] = [
    "1. Verdict per folio",
    "2. Misses against the task sheet",
    "3. Findings worth fixing",
    "4. What a reviewer would mark up",
    "5. Confidence",
];

/// The epilogue's steps for a drafter run: describe every folio, save when dirty, quit.
pub fn epilogue_steps(folios: &[String], dirty: bool, check: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    if !check {
        for f in folios {
            steps.push(Step {
                action: "page.describe".to_owned(),
                params: json!({ "folio": f }),
            });
        }
        if dirty {
            steps.push(Step {
                action: "file.save".to_owned(),
                params: json!({}),
            });
        }
    }
    steps.push(Step {
        action: "app.quit".to_owned(),
        params: json!({"discard": true}),
    });
    steps
}

/// Whether the project was dirty after the last transcript line (`None` without one). In a visual run the last line
/// with a state counts (a frame line has none); an application that has closed has no project open (`None`).
fn last_dirty(run: &Run, visual: bool) -> Option<bool> {
    let lines = run.transcript().ok()?;
    if !visual {
        let line = lines.into_iter().rev().find(|l| !l.is_empty())?;
        let v: Value = serde_json::from_str(&line).ok()?;
        return v["state"]["dirty"].as_bool();
    }
    for line in lines.iter().rev() {
        let v: Value = serde_json::from_str(line).ok()?;
        if v["closed"].as_bool() == Some(true) {
            return None;
        }
        if let Some(d) = v["state"]["dirty"].as_bool() {
            return Some(d);
        }
    }
    None
}

/// The wires and the potentials of a project for the checker (`report connections`, visual spec §6.2): a fixed `ew sql`
/// over the wire and potential views, as two CSV sections.
const WIRES_SQL: &str = "SELECT seq, number, kind, from_tag, from_cp, to_tag, to_cp, potential_name, csa, csa_unit, colour, pages FROM api_v1_wire ORDER BY seq";
const POTENTIALS_SQL: &str = "SELECT name, aliases, ptype, voltage, ac_dc, circuit_class, protected_by_tag, member_count, pages FROM api_v1_potential ORDER BY name";
/// The authored pages of a project in page order.
const FOLIOS_SQL: &str = "SELECT folio FROM api_v1_page WHERE generated = 0 ORDER BY physical_index";

/// Runs one `ew sql` over `work/project` and returns its CSV, `None` when it fails.
fn sql(run: &Run, query: &str) -> Option<String> {
    let ew = run.product_exe("ew");
    let (code, out, _) = super::session::run_tool(
        run,
        &ew,
        &["sql", "work/project", query, "--format", "csv"],
        &run.dir,
    )
    .ok()?;
    (code == Some(0)).then_some(out)
}

/// The folios of the saved project, for a brief's epilogue (a brief has no folios of its own).
fn saved_folios(run: &Run) -> Vec<String> {
    sql(run, FOLIOS_SQL)
        .map(|csv| {
            csv.lines()
                .skip(1)
                .map(|l| l.trim().trim_matches('"').to_owned())
                .filter(|f| !f.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// What the copied `ew` produced, for the report.
#[derive(Default)]
struct Exports {
    notes: Vec<String>,
    pdf_same: Option<bool>,
    /// The exit code of `ew check` (the JSON run).
    check_exit: Option<i32>,
}

fn run_exports(run: &Run) -> Result<Exports> {
    let mut ex = Exports::default();
    let ew = run.product_exe("ew");
    let project = run.path("work/project");
    if !ew.is_file() {
        ex.notes
            .push("the copied `ew` is not there: no export was made".to_owned());
        return Ok(ex);
    }
    if !project.is_dir() {
        ex.notes
            .push("the run has no project (`work/project`): no export was made".to_owned());
        return Ok(ex);
    }
    let dir = run.dir.clone();
    let mut go = |label: &str, args: &[&str]| -> Result<Option<i32>> {
        let (code, _out, err) = super::session::run_tool(run, &ew, args, &dir)?;
        let mut note = format!("{label}: exit {}", code.map_or("?".to_owned(), |c| c.to_string()));
        if let Some(first) = err.lines().next().filter(|_| code != Some(0)) {
            let _ = write!(note, " ({})", first.trim());
        }
        ex.notes.push(note);
        Ok(code)
    };
    go(
        "ew export pdf --threads 1",
        &[
            "export",
            "pdf",
            "work/project",
            "--out",
            "out/project-t1.pdf",
            "--threads",
            "1",
        ],
    )?;
    go(
        "ew export pdf --threads 4",
        &[
            "export",
            "pdf",
            "work/project",
            "--out",
            "out/project-t4.pdf",
            "--threads",
            "4",
        ],
    )?;
    go(
        "ew export bom --mode parts",
        &[
            "export",
            "bom",
            "work/project",
            "--mode",
            "parts",
            "--format",
            "csv",
            "--out",
            "out/bom.csv",
        ],
    )?;
    go(
        "ew check --format text",
        &[
            "check",
            "work/project",
            "--format",
            "text",
            "--out",
            "out/check.txt",
        ],
    )?;
    ex.check_exit = go(
        "ew check --format json",
        &[
            "check",
            "work/project",
            "--format",
            "json",
            "--out",
            "out/check.json",
        ],
    )?;
    let (a, b) = (run.path("out/project-t1.pdf"), run.path("out/project-t4.pdf"));
    ex.pdf_same = match (std::fs::read(&a), std::fs::read(&b)) {
        (Ok(x), Ok(y)) => Some(x == y),
        _ => None,
    };
    Ok(ex)
}

/// The exports `stop` makes (the two PDFs, the parts list, the check reports) for a run whose project is in
/// `work/project`, as the facts the score reads (used by `rescore`).
///
/// # Errors
///
/// A product that does not start.
pub fn exports_of(run: &Run) -> Result<ExportFacts> {
    let ex = run_exports(run)?;
    Ok(ExportFacts {
        pdfs_identical: ex.pdf_same,
        ew_check_exit: ex.check_exit,
    })
}

/// The checker's text exports of a stopped run (visual spec §6.2): `out/connections.txt` (the wires and the potentials
/// of the saved project) and `out/pages.txt` (the epilogue's page texts). They are not in a recording.
fn write_checker_exports(run: &Run, range: [usize; 2]) -> Result<()> {
    let mut text = String::from("== wires ==\n");
    text.push_str(&sql(run, WIRES_SQL).unwrap_or_else(|| "(no project)\n".to_owned()));
    text.push_str("\n== potentials ==\n");
    text.push_str(&sql(run, POTENTIALS_SQL).unwrap_or_else(|| "(no project)\n".to_owned()));
    std::fs::create_dir_all(run.path("out"))?;
    super::run::write_atomic(&run.path("out/connections.txt"), &text)?;
    let prelude = super::run::read_script(&run.path("prelude.uiscript"))?;
    let pages = super::replay::epilogue_text(
        &run.steps()?,
        &run.transcript()?,
        prelude.len(),
        [
            u32::try_from(range[0]).unwrap_or(u32::MAX),
            u32::try_from(range[1]).unwrap_or(u32::MAX),
        ],
    );
    super::run::write_atomic(&run.path("out/pages.txt"), &pages)
}

/// `stop <run> [--reason <r>]`.
///
/// # Errors
///
/// A missing hand-back (unless the reason is `orchestrator`), a product that fails, a `golden/` that changed.
pub fn stop(root: &Path, run: &Run, backend: &mut dyn Backend, reason: &str) -> Result<String> {
    let rj = run.load()?;
    if reason != "orchestrator" && !rj.handed_back {
        bail!(
            "stop needs the hand-back (`handback <run> -`); `--reason orchestrator` ends a run without one"
        );
    }
    if run.path("out/report.md").exists() {
        bail!("run `{}` is already stopped", rj.run);
    }
    let check = rj.kind == "check";
    // A visual run whose application has closed: the epilogue opens the saved project in the driver's fresh shell.
    let closed = rj.end.as_deref() == Some("app-closed");
    let dirty = last_dirty(run, rj.visual());
    let first = run.steps()?.len() + 1;
    let mut texts = 0usize;
    let mut send = |step: &Step| -> Result<()> {
        let sent: Sent = backend.execute(step)?;
        append(
            &run.path("steps.uiscript"),
            &format!("{}\n", step_line(&step.action, &step.params)),
        )?;
        append(&run.path("transcript.jsonl"), &format!("{}\n", sent.raw))?;
        if step.action == "page.describe" && sent.describe().is_some() {
            texts += 1;
        }
        Ok(())
    };
    if closed && run.path("work/project").is_dir() {
        send(&Step {
            action: "file.open".to_owned(),
            params: json!({"dir": "project"}),
        })?;
    }
    if rj.kind == "brief" {
        // A brief names no folios: the project is saved first (when dirty) and every authored page is described.
        let steps = epilogue_steps(&[], dirty == Some(true), false);
        let (quit, save) = steps.split_last().map_or((None, &[][..]), |(q, s)| (Some(q), s));
        for step in save {
            send(step)?;
        }
        for folio in saved_folios(run) {
            send(&Step {
                action: "page.describe".to_owned(),
                params: json!({ "folio": folio }),
            })?;
        }
        if let Some(q) = quit {
            send(q)?;
        }
    } else {
        for step in &epilogue_steps(&rj.folios, dirty == Some(true), check) {
            send(step)?;
        }
    }
    let last = run.steps()?.len();
    run.push_turn(Turn {
        kind: "epilogue".to_owned(),
        steps: Some([
            u32::try_from(first).unwrap_or(u32::MAX),
            u32::try_from(last).unwrap_or(u32::MAX),
        ]),
        ..Turn::default()
    })?;
    let code = if rj.stateless {
        backend.end_call()?
    } else {
        backend.finish()?
    };
    let ex = if check {
        Exports::default()
    } else {
        run_exports(run)?
    };
    if !check {
        write_checker_exports(run, [first, last])?;
    }
    let golden = check_golden(root, &rj);
    let rj = run.update(|j| {
        if reason == "orchestrator" && j.end.is_none() {
            j.end = Some("orchestrator".to_owned());
        }
        j.clean_before_epilogue = dirty.map(|d| !d);
        j.child_exit = code;
        j.golden_changed = golden.is_err();
    })?;
    // The score (§5.9): a gate run against its oracle, any other run only its protocol; a check run has none.
    let scored = if check {
        None
    } else {
        let facts = ExportFacts {
            pdfs_identical: ex.pdf_same,
            ew_check_exit: ex.check_exit,
        };
        Some(score_run(root, run, facts)?)
    };
    let mut md = String::new();
    let _ = writeln!(md, "# Run {}\n", rj.run);
    let _ = writeln!(
        md,
        "kind {}, task {}, end {}\n",
        rj.kind,
        rj.task,
        rj.end.as_deref().unwrap_or("none")
    );
    if let Some(s) = &scored {
        md.push_str(&score_markdown(s));
    }
    let c = rj.counts;
    let _ = writeln!(md, "## Protocol\n");
    let _ = writeln!(
        md,
        "turns {}, steps {}, looks {}, harness refusals {}, refused {}, failed {}, broken {}\n",
        c.turns, c.steps, c.looks, c.refusals, c.refused, c.failed, c.broken
    );
    let _ = writeln!(md, "## Observations\n");
    let _ = writeln!(
        md,
        "- the project was clean after the agent's last step: {}",
        match dirty {
            Some(false) => "yes",
            Some(true) => "no (the epilogue saved it)",
            None => "not known",
        }
    );
    let _ = writeln!(
        md,
        "- the product's exit code: {}",
        code.map_or("not known".to_owned(), |c| c.to_string())
    );
    let _ = writeln!(md, "- page texts of the epilogue: {texts}");
    let _ = writeln!(
        md,
        "- the PDFs of 1 and 4 threads are byte-identical: {}",
        match ex.pdf_same {
            Some(true) => "yes",
            Some(false) => "no",
            None => "not exported",
        }
    );
    for n in &ex.notes {
        let _ = writeln!(md, "- {n}");
    }
    let _ = writeln!(
        md,
        "- golden/ unchanged: {}",
        if golden.is_ok() { "yes" } else { "NO" }
    );
    let _ = writeln!(md, "\n## Prelude\n");
    let prelude = super::run::read_script(&run.path("prelude.uiscript"))?;
    let _ = writeln!(md, "{} lines\n", prelude.len());
    let _ = writeln!(md, "## Broken steps\n");
    let drafter = std::fs::read_to_string(run.path("drafter.md")).unwrap_or_default();
    let broken: Vec<&str> = drafter.lines().filter(|l| l.starts_with("broken: ")).collect();
    if broken.is_empty() {
        let _ = writeln!(md, "none");
    }
    for b in broken {
        let _ = writeln!(md, "- {b}");
    }
    let _ = writeln!(md, "\n## Harness refusals\n");
    let refusals: Vec<String> = run.turns()?.into_iter().filter_map(|t| t.refusal).collect();
    if refusals.is_empty() {
        let _ = writeln!(md, "none");
    }
    for r in &refusals {
        let _ = writeln!(md, "- {r}");
    }
    std::fs::create_dir_all(run.path("out"))?;
    super::run::write_atomic(&run.path("out/report.md"), &md)?;
    // The PDF's digest is what a recording keeps (the file itself is not committed).
    if let Ok(pdf) = std::fs::read(run.path("out/project-t1.pdf")) {
        super::run::write_atomic(
            &run.path("out/project-t1.pdf.sha256"),
            &format!("{}\n", sha256_hex(&pdf)),
        )?;
    }
    // A checker's audit goes to the drafter's folder.
    if check
        && let Some(of) = &rj.of
        && let Ok(drafter_run) = super::run::locate(of, run.dir.parent(), None)
        && let Ok(text) = std::fs::read_to_string(run.path("checker.md"))
    {
        super::run::write_atomic(&drafter_run.path("checker.md"), &text)?;
    }
    golden?;
    Ok(format!(
        "run `{}` stopped; the report is {}\n",
        rj.run,
        run.path("out/report.md").display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::protocol::tests::{fixture, s};
    use crate::blind::protocol::{HANDBACK_HEADINGS, do_steps, handback, note};
    use crate::blind::run::{Kind, tree_sha};

    fn repo_with_golden(f: &crate::blind::protocol::tests::Fixture) -> std::path::PathBuf {
        let root = f.tmp.0.join("repo");
        std::fs::create_dir_all(root.join("golden")).unwrap();
        std::fs::write(root.join("golden/x.txt"), "1").unwrap();
        let sha = tree_sha(&root.join("golden")).unwrap();
        f.run.update(|j| j.golden = sha).unwrap();
        root
    }

    fn hand_back(f: &crate::blind::protocol::tests::Fixture) {
        let text: Vec<String> = HANDBACK_HEADINGS.iter().map(|h| format!("## {h}\nx")).collect();
        handback(&f.run, &text.join("\n")).unwrap();
    }

    /// The epilogue describes every folio, saves only when the project is dirty and quits.
    #[test]
    fn the_epilogue_describes_the_folios_saves_only_a_dirty_project_and_quits() {
        let folios = vec!["101".to_owned(), "102".to_owned()];
        let actions = |dirty: bool, check: bool| {
            epilogue_steps(&folios, dirty, check)
                .iter()
                .map(|s| s.action.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            actions(false, false),
            ["page.describe", "page.describe", "app.quit"]
        );
        assert_eq!(
            actions(true, false),
            ["page.describe", "page.describe", "file.save", "app.quit"]
        );
        assert_eq!(actions(true, true), ["app.quit"], "a check run only quits");
        assert_eq!(
            epilogue_steps(&folios, false, false)[0].params,
            json!({"folio": "101"})
        );
        assert_eq!(
            epilogue_steps(&folios, false, false).last().unwrap().params,
            json!({"discard": true})
        );
    }

    #[test]
    fn stop_needs_the_hand_back_unless_the_reason_is_orchestrator() {
        let mut f = fixture(Kind::Free);
        let root = repo_with_golden(&f);
        let err = stop(&root, &f.run, &mut f.backend, "handback").unwrap_err();
        assert!(err.to_string().contains("needs the hand-back"), "{err}");
        let out = stop(&root, &f.run, &mut f.backend, "orchestrator").unwrap();
        assert!(out.contains("stopped"), "{out}");
        assert_eq!(f.run.load().unwrap().end.as_deref(), Some("orchestrator"));
        // A run is stopped once.
        assert!(stop(&root, &f.run, &mut f.backend, "orchestrator").is_err());
    }

    #[test]
    fn stop_runs_the_epilogue_records_it_and_reports() {
        let mut f = fixture(Kind::Free);
        let root = repo_with_golden(&f);
        // A dirty project: the fake becomes dirty on `edit.undo`.
        f.run.update(|j| j.folios = vec!["101".to_owned()]).unwrap();
        std::fs::write(
            f.run.path("transcript.jsonl"),
            "{\"$schema\":\"ew.uitranscript/1\",\"version\":\"0\"}\n",
        )
        .unwrap();
        do_steps(&f.run, &f.reference, &mut f.backend, &[s("edit.undo", json!({}))]).unwrap();
        note(&f.run, "patience 9 flow 9 x").unwrap();
        hand_back(&f);
        let out = stop(&root, &f.run, &mut f.backend, "handback").unwrap();
        assert!(out.contains("stopped"), "{out}");
        let steps: Vec<String> = f.run.steps().unwrap().into_iter().map(|(a, _)| a).collect();
        assert_eq!(steps, ["edit.undo", "page.describe", "file.save", "app.quit"]);
        let turns = f.run.turns().unwrap();
        let last = turns.last().unwrap();
        assert_eq!(last.kind, "epilogue");
        assert_eq!(last.steps, Some([2, 4]));
        let rj = f.run.load().unwrap();
        assert_eq!(rj.clean_before_epilogue, Some(false), "the agent did not save");
        let report = std::fs::read_to_string(f.run.path("out/report.md")).unwrap();
        assert!(report.contains("turns 1, steps 1, looks 0"), "{report}");
        assert!(report.contains("no (the epilogue saved it)"), "{report}");
        assert!(report.contains("golden/ unchanged: yes"), "{report}");
        assert!(report.contains("not exported"), "{report}");
        // §5.9: a run without an oracle holds its protocol in `score.json`, and the report has a score section.
        assert!(
            report.contains("## Score") && report.contains("holds no oracle"),
            "{report}"
        );
        let score: crate::blind::score::Score =
            serde_json::from_str(&std::fs::read_to_string(f.run.path("out/score.json")).unwrap()).unwrap();
        assert_eq!(score.run, "r1");
        assert_eq!(score.outcome, None);
        assert_eq!(score.protocol.counts.steps, 1);
        assert_eq!(score.protocol.observations.clean_before_epilogue, Some(false));
        assert!(
            !report.contains(&f.tmp.0.to_string_lossy().to_string()),
            "no absolute path in the report"
        );
        // The transcript holds the epilogue's page text.
        let t = f.run.transcript().unwrap();
        assert!(
            t.iter().any(|l| l.contains("\"action\":\"page.describe\"")
                && l.contains("\"describe\":\"page \\\"101\\\""))
        );
    }

    /// FT-484: a run of the project scheme stops cleanly when only `expected/` or a document changed, and fails when the
    /// golden project changed.
    #[test]
    fn only_the_golden_project_voids_a_run_of_the_project_scheme() {
        let mut f = fixture(Kind::Free);
        let root = f.tmp.0.join("repo");
        std::fs::create_dir_all(root.join("golden/J-E-2416.ew")).unwrap();
        std::fs::create_dir_all(root.join("golden/J-E-2416/expected")).unwrap();
        std::fs::write(root.join("golden/J-E-2416.ew/p.json"), "1").unwrap();
        std::fs::write(root.join("golden/J-E-2416/README.md"), "a").unwrap();
        let sha = crate::blind::run::golden_digest(&root, Some("project")).unwrap();
        f.run
            .update(|j| {
                j.golden = sha;
                j.golden_scope = Some("project".to_owned());
            })
            .unwrap();
        std::fs::write(root.join("golden/J-E-2416/README.md"), "b").unwrap();
        std::fs::write(root.join("golden/J-E-2416/expected/x.csv"), "c").unwrap();
        stop(&root, &f.run, &mut f.backend, "orchestrator").unwrap();
        assert!(!f.run.load().unwrap().golden_changed);

        let mut g = fixture(Kind::Free);
        let root = g.tmp.0.join("repo");
        std::fs::create_dir_all(root.join("golden/J-E-2416.ew")).unwrap();
        std::fs::write(root.join("golden/J-E-2416.ew/p.json"), "1").unwrap();
        let sha = crate::blind::run::golden_digest(&root, Some("project")).unwrap();
        g.run
            .update(|j| {
                j.golden = sha;
                j.golden_scope = Some("project".to_owned());
            })
            .unwrap();
        std::fs::write(root.join("golden/J-E-2416.ew/p.json"), "2").unwrap();
        assert!(stop(&root, &g.run, &mut g.backend, "orchestrator").is_err());
        assert!(g.run.load().unwrap().golden_changed);
    }

    /// BD10: a changed golden fails the stop, after the report is written.
    #[test]
    fn a_golden_that_changed_during_the_run_fails_the_stop() {
        let mut f = fixture(Kind::Free);
        let root = repo_with_golden(&f);
        std::fs::write(root.join("golden/x.txt"), "2").unwrap();
        let err = stop(&root, &f.run, &mut f.backend, "orchestrator").unwrap_err();
        assert!(err.to_string().contains("golden/ changed"), "{err}");
        let report = std::fs::read_to_string(f.run.path("out/report.md")).unwrap();
        assert!(report.contains("golden/ unchanged: NO"), "{report}");
        // The run is marked, and `bless` refuses it.
        assert!(f.run.load().unwrap().golden_changed);
        let err = crate::blind::replay::bless(&root, &f.run, "x", "none").unwrap_err();
        assert!(err.to_string().contains("golden/ changed"), "{err}");
        assert!(!root.join("tools/blind/recordings/x").exists());
    }
}
