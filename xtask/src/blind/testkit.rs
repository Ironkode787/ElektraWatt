// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers of the harness's unit tests: temporary folders and a run folder with a fake world.

use std::path::PathBuf;

use serde_json::json;

use super::run::{
    Counts, HARNESS_VERSION, Kind, RUN_SCHEMA, Run, RunJson, script_header, step_line, turns_header,
};

/// A fresh temporary folder, removed on drop.
pub struct Tmp(pub PathBuf);

impl Tmp {
    /// A new folder tagged `tag`.
    pub fn new(tag: &str) -> Tmp {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-blind-test-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The `run.json` of a test run of `kind`.
pub fn new_run_json(id: &str, kind: Kind) -> RunJson {
    RunJson {
        schema: RUN_SCHEMA.to_owned(),
        run: id.to_owned(),
        kind: kind.word().to_owned(),
        task: "selftest".to_owned(),
        persona: None,
        of: None,
        folios: vec!["101".to_owned()],
        caps: kind.caps(),
        files: Default::default(),
        golden: "0".to_owned(),
        golden_scope: None,
        app_version: "0.0.0".to_owned(),
        harness_version: HARNESS_VERSION.to_owned(),
        stateless: false,
        counts: Counts::default(),
        end: None,
        handed_back: false,
        diverged_at: None,
        ci: "none".to_owned(),
        stale: None,
        clean_before_epilogue: None,
        child_exit: None,
        golden_changed: false,
    }
}

/// A run folder with a one-step prelude (`file.new`), an empty script and its `run.json`.
pub fn test_run(tmp: &Tmp) -> Run {
    let run = Run {
        dir: tmp.0.join("run"),
    };
    std::fs::create_dir_all(run.path(".harness")).unwrap();
    std::fs::write(
        run.path("prelude.uiscript"),
        format!(
            "{}\n{}\n",
            script_header("prelude"),
            step_line("file.new", &json!({"dir": "project"}))
        ),
    )
    .unwrap();
    std::fs::write(
        run.path("steps.uiscript"),
        format!("{}\n", script_header("steps")),
    )
    .unwrap();
    std::fs::write(run.path("turns.jsonl"), format!("{}\n", turns_header())).unwrap();
    run.save(&new_run_json("r1", Kind::Free)).unwrap();
    run
}

/// A stopped drafter run holding a project (`work/project/marker.txt`) and a check run of it, side by side in one
/// folder as `start --kind check --of drafter` makes them; the check run's work folder is empty.
pub fn check_pair(tmp: &Tmp) -> (Run, Run) {
    let runs = tmp.0.join("runs");
    let drafter = Run {
        dir: runs.join("drafter"),
    };
    std::fs::create_dir_all(drafter.path("work/project")).unwrap();
    std::fs::write(drafter.path("work/project/marker.txt"), "the drafter's project").unwrap();
    drafter.save(&new_run_json("drafter", Kind::Gate)).unwrap();
    let check = Run {
        dir: runs.join("check"),
    };
    std::fs::create_dir_all(check.path(".harness")).unwrap();
    std::fs::write(
        check.path("prelude.uiscript"),
        format!(
            "{}\n{}\n",
            script_header("check prelude"),
            step_line("file.open", &json!({"dir": "project"}))
        ),
    )
    .unwrap();
    std::fs::write(
        check.path("steps.uiscript"),
        format!("{}\n", script_header("steps")),
    )
    .unwrap();
    std::fs::write(check.path("turns.jsonl"), format!("{}\n", turns_header())).unwrap();
    let mut rj = new_run_json("check", Kind::Check);
    rj.of = Some("drafter".to_owned());
    check.save(&rj).unwrap();
    (drafter, check)
}
