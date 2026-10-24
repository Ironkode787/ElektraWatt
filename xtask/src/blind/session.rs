// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The product as a session the harness feeds: one step in, one transcript line out (WP-S6.13 §5.4, §6 item 1).
//!
//! [`Product`] is the child `elektrawatt --headless --script - --keep-going`; [`Backend`] is what the protocol sends
//! steps through: the keeper's client, the stateless replay, or a fake in a test. Nothing here waits on time: the
//! headless mode is synchronous, so a step's line is the next line the child writes.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

use super::run::{Run, step_line};

/// A step the harness sends: an action and its parameters.
#[derive(Clone, PartialEq, Debug)]
pub struct Step {
    /// The action ID.
    pub action: String,
    /// The parameters (an object).
    pub params: Value,
}

impl Step {
    /// The UI script line of the step.
    pub fn line(&self) -> String {
        step_line(&self.action, &self.params)
    }
}

/// The transcript line of one sent step.
#[derive(Clone, PartialEq, Debug)]
pub struct Sent {
    /// The line as the product wrote it (no newline).
    pub raw: String,
    /// The line parsed.
    pub value: Value,
}

impl Sent {
    /// Parses a transcript line.
    pub fn parse(raw: String) -> Result<Sent> {
        let value = serde_json::from_str(&raw).with_context(|| format!("not a transcript line: {raw}"))?;
        Ok(Sent { raw, value })
    }

    /// `done`, `refused` or `failed`.
    pub fn outcome(&self) -> &str {
        self.value["outcome"].as_str().unwrap_or("")
    }

    /// The reason key of a refusal or the message of a failure.
    pub fn reason(&self) -> Option<String> {
        match (self.value["reason"].as_str(), self.value["message"].as_str()) {
            (Some(r), _) => Some(r.to_owned()),
            (None, Some(m)) => Some(m.to_owned()),
            _ => None,
        }
    }

    /// The `describe` text of a `ui.describe` or `page.describe` step.
    pub fn describe(&self) -> Option<&str> {
        self.value["describe"].as_str()
    }
}

/// The session was lost: the keeper or the product does not answer. `resume` brings it back.
#[derive(Debug)]
pub struct Lost(pub String);

impl std::fmt::Display for Lost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session lost: {}", self.0)
    }
}

impl std::error::Error for Lost {}

/// A replay found a step whose outcome differs from the run's transcript (`end: diverged`).
#[derive(Debug)]
pub struct Diverged {
    /// The 1-based step over the prelude and the steps.
    pub step: usize,
    /// What differed.
    pub what: String,
}

impl std::fmt::Display for Diverged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "diverged at step {}: {}", self.step, self.what)
    }
}

impl std::error::Error for Diverged {}

/// What the protocol sends steps through.
pub trait Backend {
    /// Sends one step to the session and returns its transcript line.
    ///
    /// # Errors
    ///
    /// [`Lost`] when the session does not answer, [`Diverged`] when a stateless replay differs.
    fn execute(&mut self, step: &Step) -> Result<Sent>;

    /// Ends one call of the harness (`do`, `look`): a stateless backend quits its child here and returns its exit code,
    /// the keeper's client does nothing.
    ///
    /// # Errors
    ///
    /// The child's.
    fn end_call(&mut self) -> Result<Option<i32>>;

    /// Ends the session after the epilogue: waits for the product and returns its exit code.
    ///
    /// # Errors
    ///
    /// The session's.
    fn finish(&mut self) -> Result<Option<i32>>;
}

/// The product child: one line in, one line out.
pub trait Product: Send {
    /// Sends one script line and returns the next transcript line.
    ///
    /// # Errors
    ///
    /// [`Lost`] when the child ended.
    fn send(&mut self, line: &str) -> Result<String>;

    /// Closes its input, waits for it to end and returns the exit code.
    ///
    /// # Errors
    ///
    /// The wait's.
    fn finish(&mut self) -> Result<Option<i32>>;

    /// Ends it at once.
    fn kill(&mut self);
}

/// How a product child is made: the real one, or a fake of a test.
pub trait Spawner: Send + Sync {
    /// Starts a product for `run` and reads its transcript header.
    ///
    /// # Errors
    ///
    /// The spawn's.
    fn spawn(&self, run: &Run) -> Result<(Box<dyn Product>, String)>;
}

/// The real product: the run's copied `elektrawatt` with the run's folders as its world.
pub struct ProcessProduct {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl ProcessProduct {
    /// Starts `<run>/.harness/bin/elektrawatt --headless --script - --keep-going --record record.ewcmd --config-dir
    /// config` in `<run>/work`, with `EW_SAMPLE_DIR` at the empty folder, `EW_DOCS_DIR` at the copied guide and
    /// `EW_CONTENT_DIR` at the copied starter content.
    ///
    /// # Errors
    ///
    /// The spawn's, or an empty header.
    pub fn spawn(run: &Run, record: &str) -> Result<(ProcessProduct, String)> {
        let work = run.path("work");
        std::fs::create_dir_all(&work)?;
        std::fs::create_dir_all(run.path("config"))?;
        let stderr = std::fs::File::create(run.path(".harness/product.stderr"))
            .context("creating the product's stderr file")?;
        let mut child = Command::new(run.product_exe("elektrawatt"))
            .args(["--headless", "--script", "-", "--keep-going", "--record"])
            .arg(run.path(record))
            .arg("--config-dir")
            .arg(run.path("config"))
            .current_dir(&work)
            .env("EW_SAMPLE_DIR", run.path(".harness/empty-sample"))
            .env("EW_DOCS_DIR", run.path("book"))
            .env("EW_CONTENT_DIR", run.path(".harness/content"))
            .env_remove("EW_CONFIG_DIR")
            .env_remove("SOURCE_DATE_EPOCH")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr))
            .spawn()
            .with_context(|| format!("starting {}", run.product_exe("elektrawatt").display()))?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("the product has no stdout"))?;
        let mut product = ProcessProduct {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        };
        let header = product.read_line()?;
        if header.is_empty() {
            bail!("the product wrote no transcript header");
        }
        // The script's own header is the stream's first line; it answers with nothing.
        let stdin = product
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("the product's input is closed"))?;
        stdin
            .write_all(format!("{}\n", super::run::script_header("blind run")).as_bytes())
            .and_then(|()| stdin.flush())
            .context("sending the script header")?;
        Ok((product, header))
    }

    fn read_line(&mut self) -> Result<String> {
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line)?;
        if n == 0 {
            return Err(Lost("the product ended".to_owned()).into());
        }
        Ok(line.trim_end_matches(['\r', '\n']).to_owned())
    }
}

impl Product for ProcessProduct {
    fn send(&mut self, line: &str) -> Result<String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| Lost("the product's input is closed".to_owned()))?;
        stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush())
            .map_err(|e| Lost(format!("the product does not read: {e}")))?;
        self.read_line()
    }

    fn finish(&mut self) -> Result<Option<i32>> {
        drop(self.stdin.take());
        // Anything the product still writes (nothing, after `app.quit`) is read to the end so that it can exit.
        let mut rest = String::new();
        let _ = std::io::Read::read_to_string(&mut self.stdout, &mut rest);
        let status = self.child.wait()?;
        Ok(status.code())
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ProcessProduct {
    fn drop(&mut self) {
        // A product still running when the harness lets go of it is ended: no stray process outlives its session.
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// The spawner of the real product.
pub struct RealSpawner;

impl Spawner for RealSpawner {
    fn spawn(&self, run: &Run) -> Result<(Box<dyn Product>, String)> {
        let (p, header) = ProcessProduct::spawn(run, "record.ewcmd")?;
        Ok((Box::new(p), header))
    }
}

/// Runs `exe` with `args` in `dir` and returns its exit code and standard output (the copied `ew` and the help
/// printers); the environment is the run's.
///
/// # Errors
///
/// The spawn's.
pub fn run_tool(run: &Run, exe: &Path, args: &[&str], dir: &Path) -> Result<(Option<i32>, String, String)> {
    let out = Command::new(exe)
        .args(args)
        .current_dir(dir)
        .env("EW_SAMPLE_DIR", run.path(".harness/empty-sample"))
        .env("EW_DOCS_DIR", run.path("book"))
        .env("EW_CONTENT_DIR", run.path(".harness/content"))
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("running {}", exe.display()))?;
    Ok((
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    ))
}

/// A fake product for the tests: the transcript line of each step is made from the action by simple rules, so that
/// the protocol, the keeper and the replay are tested without a build of the application.
#[cfg(test)]
pub mod fake {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// What the fake does with an action: `edit.redo` and `refuse.me` are refused, `file.open` and `fail.me` fail,
    /// `change.me` is refused from the life `change_from` on (a changed product), `edit.undo` and `dirty.me` make the
    /// project dirty, `file.save` cleans it, `app.quit` ends the fake, everything else is done.
    #[derive(Clone, Default)]
    pub struct FakeSpawner {
        /// Every line the fakes of this spawner were sent, for the tests to read.
        pub log: Arc<Mutex<Vec<String>>>,
        /// The number of fakes made so far.
        pub spawned: Arc<Mutex<u32>>,
        /// From this life on `change.me` is refused instead of done.
        pub change_from: Option<u32>,
    }

    impl Spawner for FakeSpawner {
        fn spawn(&self, _: &Run) -> Result<(Box<dyn Product>, String)> {
            let life = {
                let mut n = self.spawned.lock().unwrap();
                *n += 1;
                *n
            };
            let changed = self.change_from.is_some_and(|c| life >= c);
            Ok((
                Box::new(Fake {
                    log: Arc::clone(&self.log),
                    step: 0,
                    dirty: false,
                    changed,
                    quit: false,
                }),
                "{\"$schema\":\"ew.uitranscript/1\",\"version\":\"0.0.0\"}".to_owned(),
            ))
        }
    }

    struct Fake {
        log: Arc<Mutex<Vec<String>>>,
        step: u32,
        dirty: bool,
        changed: bool,
        quit: bool,
    }

    impl Product for Fake {
        fn send(&mut self, line: &str) -> Result<String> {
            if self.quit {
                return Err(Lost("the fake product ended".to_owned()).into());
            }
            self.log.lock().unwrap().push(line.to_owned());
            self.step += 1;
            let v: Value = serde_json::from_str(line)?;
            let action = v["action"].as_str().unwrap_or("").to_owned();
            let (outcome, reason, message) = match action.as_str() {
                "refuse.me" | "edit.redo" => ("refused", Some("reason-nothing-to-redo"), None),
                "change.me" if self.changed => ("refused", Some("reason-changed"), None),
                "fail.me" | "file.open" => ("failed", None, Some("It failed.")),
                _ => ("done", None, None),
            };
            if action == "dirty.me" || action == "edit.undo" {
                self.dirty = true;
            }
            if action == "file.save" && outcome == "done" {
                self.dirty = false;
            }
            if action == "app.quit" {
                self.quit = true;
            }
            let describe = match action.as_str() {
                "ui.describe" => Some("title: Fake\nmenu File\n  item \"New\" enabled\npage \"101\"\n  label \"x\"\nstatus bar: ready".to_owned()),
                "page.describe" => Some(format!(
                    "page \"{}\" \"Title\" reps 1 connections 0 junctions 0\n  rep \"S1\" cell \"101-B2\"",
                    v["params"]["folio"].as_str().unwrap_or("101")
                )),
                _ => None,
            };
            let mut out = format!(
                "{{\"step\":{},\"action\":{},\"outcome\":\"{outcome}\",\"reason\":{},\"message\":{},\"state\":{{\"title\":\"Fake\",\"dirty\":{},\"page\":\"101\",\"canvas\":{{\"zoom\":{}}}}}",
                self.step,
                Value::from(action.as_str()),
                reason.map_or("null".to_owned(), |r| Value::from(r).to_string()),
                message.map_or("null".to_owned(), |m| Value::from(m).to_string()),
                self.dirty,
                100 + self.step,
            );
            if let Some(d) = describe {
                out.push_str(&format!(",\"describe\":{}", Value::from(d)));
            }
            out.push('}');
            Ok(out)
        }

        fn finish(&mut self) -> Result<Option<i32>> {
            self.quit = true;
            Ok(Some(i32::from(self.dirty)))
        }

        fn kill(&mut self) {
            self.quit = true;
        }
    }
}
