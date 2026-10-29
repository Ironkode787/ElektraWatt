// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `--stateless` (WP-S6.13 §5.4, D6): no keeper and no socket, for agent shells that block loopback or kill detached
//! processes. Every `do` and `look` replays the prelude and the accepted steps in a fresh product, checks that the
//! outcomes repeat, sends the new steps and quits the product with `app.quit` `discard`; the project is saved only by
//! the agent's own `file.save` steps, which the replay repeats. It costs a replay per call and is otherwise equal: the
//! same files, the same transcript, the same score.

use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::json;

use super::keeper::replay_steps;
use super::run::{Run, parse_script, step_line, write_atomic};
use super::session::{Backend, Lost, Product, Sent, Spawner, Step};

/// The replaying backend of one call.
pub struct Stateless<'a> {
    run: &'a Run,
    spawner: &'a dyn Spawner,
    product: Option<Box<dyn Product>>,
    quit: bool,
}

/// Removes the folder `name` of the run (`work`, `config`): only a direct child of the run folder, never anything else.
fn remove_inside(run: &Run, name: &str) -> Result<()> {
    let path = run.path(name);
    let plain = {
        let mut parts = Path::new(name).components();
        matches!(parts.next(), Some(std::path::Component::Normal(_))) && parts.next().is_none()
    };
    if !plain || path.parent() != Some(run.dir.as_path()) {
        bail!("refusing to delete {} outside the run folder", path.display());
    }
    match std::fs::remove_dir_all(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("deleting {}", path.display())),
    }
}

impl<'a> Stateless<'a> {
    /// A backend for one call; nothing starts until the first step.
    pub fn new(run: &'a Run, spawner: &'a dyn Spawner) -> Stateless<'a> {
        Stateless {
            run,
            spawner,
            product: None,
            quit: false,
        }
    }

    /// Starts a fresh product and replays the prelude and the accepted steps.
    fn begin(&mut self) -> Result<()> {
        remove_inside(self.run, "work")?;
        remove_inside(self.run, "config")?;
        // A check run's project is a copy in `work/`: it goes back with it.
        super::start::reseed_check_project(self.run)?;
        let (mut product, header) = self.spawner.spawn(self.run)?;
        let prelude = parse_script(&std::fs::read_to_string(self.run.path("prelude.uiscript"))?)?;
        let mut all = prelude.clone();
        all.extend(self.run.steps()?);
        let expected = self.run.transcript()?;
        let lines = match replay_steps(product.as_mut(), &all, &expected, Some(&self.run.path("frames"))) {
            Ok(l) => l,
            Err(e) => {
                product.kill();
                return Err(e);
            }
        };
        if expected.is_empty() {
            // The first call: the prelude's lines are the transcript's start.
            let mut text = format!("{header}\n");
            for l in &lines {
                text.push_str(l);
                text.push('\n');
            }
            write_atomic(&self.run.path("transcript.jsonl"), &text)?;
        }
        self.product = Some(product);
        Ok(())
    }
}

impl Backend for Stateless<'_> {
    fn execute(&mut self, step: &Step) -> Result<Sent> {
        if self.product.is_none() {
            self.begin()?;
        }
        let product = self
            .product
            .as_mut()
            .ok_or_else(|| Lost("no product".to_owned()))?;
        let line = product.send(&step.line())?;
        if step.action == "app.quit" {
            self.quit = true;
        }
        Sent::parse(line)
    }

    fn end_call(&mut self) -> Result<Option<i32>> {
        match self.product.take() {
            Some(mut product) => {
                if !self.quit {
                    // The project is saved by the agent's own steps only: the end of a call discards what is left.
                    let _ = product.send(&step_line("app.quit", &json!({"discard": true})));
                }
                product.finish()
            }
            None => Ok(None),
        }
    }

    fn finish(&mut self) -> Result<Option<i32>> {
        match self.product.take() {
            Some(mut product) => product.finish(),
            None => Err(anyhow!("a stateless run has no product to finish")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::session::fake::FakeSpawner;
    use crate::blind::testkit::{Tmp, test_run};

    fn step(action: &str) -> Step {
        Step {
            action: action.to_owned(),
            params: json!({}),
        }
    }

    /// BD8: a stateless call replays the accepted steps in a fresh product, and a changed outcome is a divergence.
    #[test]
    fn a_call_replays_the_accepted_steps_and_quits_its_product() {
        let tmp = Tmp::new("stateless");
        let run = test_run(&tmp);
        let spawner = FakeSpawner::default();
        // The first call writes the prelude's transcript and sends one step.
        let mut backend = Stateless::new(&run, &spawner);
        let sent = backend.execute(&step("help.about")).unwrap();
        assert_eq!(sent.outcome(), "done");
        backend.end_call().unwrap();
        assert_eq!(run.transcript().unwrap().len(), 1, "the prelude's line");
        let mut transcript = std::fs::read_to_string(run.path("transcript.jsonl")).unwrap();
        transcript.push_str(&format!("{}\n", sent.raw));
        std::fs::write(run.path("transcript.jsonl"), transcript).unwrap();
        crate::blind::run::append(
            &run.path("steps.uiscript"),
            &format!("{}\n", step("help.about").line()),
        )
        .unwrap();
        // The second call replays prelude and `help.about`, then sends the new step.
        let before = spawner.log.lock().unwrap().len();
        let mut backend = Stateless::new(&run, &spawner);
        backend.execute(&step("ui.describe")).unwrap();
        backend.end_call().unwrap();
        let log = spawner.log.lock().unwrap()[before..].to_vec();
        let actions: Vec<String> = log
            .iter()
            .map(|l| {
                serde_json::from_str::<serde_json::Value>(l).unwrap()["action"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            actions,
            ["file.new", "help.about", "ui.describe", "app.quit"],
            "{log:?}"
        );
        assert_eq!(
            *spawner.spawned.lock().unwrap(),
            2,
            "a fresh product for every call"
        );
        // A product whose outcome changed is a divergence at the step.
        let changed = FakeSpawner {
            change_from: Some(1),
            ..FakeSpawner::default()
        };
        crate::blind::run::append(
            &run.path("steps.uiscript"),
            &format!("{}\n", step("change.me").line()),
        )
        .unwrap();
        let mut t = std::fs::read_to_string(run.path("transcript.jsonl")).unwrap();
        t.push_str(
            "{\"step\":3,\"action\":\"change.me\",\"outcome\":\"done\",\"reason\":null,\"message\":null}\n",
        );
        std::fs::write(run.path("transcript.jsonl"), t).unwrap();
        let mut backend = Stateless::new(&run, &changed);
        let err = backend.execute(&step("help.about")).unwrap_err();
        assert!(
            err.downcast_ref::<crate::blind::session::Diverged>().is_some(),
            "{err}"
        );
    }

    /// A check run's project is a copy in `work/`: a stateless call deletes `work/` and puts the copy back, so the
    /// check prelude's `file.open dir=project` finds it (fix round).
    #[test]
    fn a_check_run_keeps_its_project_across_the_replays() {
        let tmp = Tmp::new("stateless-check");
        let (_drafter, check) = crate::blind::testkit::check_pair(&tmp);
        std::fs::create_dir_all(check.path("work")).unwrap();
        std::fs::write(check.path("work/leftover.txt"), "x").unwrap();
        let spawner = FakeSpawner::default();
        let mut backend = Stateless::new(&check, &spawner);
        backend.execute(&step("help.about")).unwrap();
        assert!(check.path("work/project/marker.txt").is_file());
        assert!(!check.path("work/leftover.txt").exists(), "work/ was reset");
        backend.end_call().unwrap();
    }

    #[test]
    fn only_a_direct_child_of_the_run_folder_is_deleted() {
        let tmp = Tmp::new("stateless-rm");
        let run = Run {
            dir: tmp.0.join("run"),
        };
        std::fs::create_dir_all(run.path("work/project")).unwrap();
        remove_inside(&run, "work").unwrap();
        assert!(!run.path("work").exists());
        // Nothing to delete is not an error; a path that climbs out or goes deeper is refused.
        remove_inside(&run, "config").unwrap();
        assert!(remove_inside(&run, "..").is_err());
        assert!(remove_inside(&run, "work/project").is_err());
    }
}
