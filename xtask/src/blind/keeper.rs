// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The session keeper (WP-S6.13 §5.4, D6): the run's copied harness holds the product as a child and serves steps over
//! a loopback socket with a per-run token. **`xtask` listens, never the product**: `elektrawatt` only reads its
//! standard input and writes its standard output, so `PRIVACY.md` and the network sandbox of doc 18 §12.2 are
//! unchanged.
//!
//! A request is one line `{"token":…,"steps":[{"action":…,"params":…}]}` (or `{"token":…,"end":true}`, or
//! `{"token":…,"shutdown":true}`); the answer is one line `{"ok":true,"lines":[…]}`. A connection from anywhere but the
//! loopback, or whose first line lacks the token, is closed without an answer. The only timer is the idle period behind
//! the [`Clock`] trait, which the tests drive by hand.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use super::run::{FRAME_LINE, Run, parse_script, step_line, write_atomic};
use super::session::{Backend, Diverged, Lost, Product, Sent, Spawner, Step};

/// The default idle period of a keeper, in minutes.
pub const DEFAULT_IDLE_MINUTES: u64 = 30;

/// The most a request line may hold.
const MAX_REQUEST: u64 = 8 * 1024 * 1024;

/// What waiting for the next connection ended with.
pub enum Recv {
    /// A connection.
    Conn(TcpStream),
    /// The idle period passed.
    Idle,
    /// The acceptor ended.
    Closed,
}

/// The keeper's clock: it waits for the next connection for at most the idle period.
pub trait Clock: Send + Sync {
    /// Waits for a connection from `rx`.
    fn recv(&self, rx: &Receiver<TcpStream>, idle: Duration) -> Recv;
}

/// The system's clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn recv(&self, rx: &Receiver<TcpStream>, idle: Duration) -> Recv {
        match rx.recv_timeout(idle) {
            Ok(c) => Recv::Conn(c),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Recv::Idle,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Recv::Closed,
        }
    }
}

/// How a keeper starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// A new run: the prelude runs and writes the transcript.
    Fresh,
    /// A lost run: the prelude and every accepted step are replayed and compared with the transcript.
    Resume,
}

/// The first line of a reply the product's transcript gave for a step, compared by what decides a replay: the action,
/// the outcome and the reason (or message); for a visual input or frame line (visual spec §5.3, R1v) the input, the
/// dispatched actions, the frame's name and whether the application has closed. A frame's SHA-256 is not compared.
pub fn same_outcome(expected: &str, got: &str) -> Result<(), String> {
    let (Ok(a), Ok(b)) = (
        serde_json::from_str::<Value>(expected),
        serde_json::from_str::<Value>(got),
    ) else {
        return Err("a transcript line is not JSON".to_owned());
    };
    for key in [
        "action",
        "outcome",
        "reason",
        "message",
        "input",
        "dispatched",
        "frame",
        "closed",
    ] {
        if a[key] != b[key] {
            return Err(format!("{key} was {} and is now {}", a[key], b[key]));
        }
    }
    Ok(())
}

/// Replays the prelude and the steps of `run` into `product` and compares each line with `expected`. A visual frame line
/// whose file already exists in `frames` (or every frame line, without `frames`) is sent with `"paint":false`: the
/// replay settles as the run did and paints only a frame it does not have yet (visual spec §5.4).
///
/// # Errors
///
/// [`Diverged`] at the first step whose line differs; [`Lost`] when the product ends.
pub fn replay_steps(
    product: &mut dyn Product,
    steps: &[(String, Value)],
    expected: &[String],
    frames: Option<&Path>,
) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for (i, (action, params)) in steps.iter().enumerate() {
        let mut params = params.clone();
        if action == FRAME_LINE {
            let have = match (frames, params["frame"].as_str()) {
                (Some(dir), Some(file)) => dir.join(file).is_file(),
                _ => true,
            };
            if have {
                params["paint"] = Value::Bool(false);
            }
        }
        let line = product.send(&step_line(action, &params))?;
        if let Some(want) = expected.get(i)
            && let Err(what) = same_outcome(want, &line)
        {
            return Err(Diverged { step: i + 1, what }.into());
        }
        lines.push(line);
    }
    Ok(lines)
}

/// The path of `session.json`: the keeper's port and token.
pub fn session_path(run: &Run) -> PathBuf {
    run.path("session.json")
}

/// A fresh token in the shape of a UUID v4 (version and variant bits set), 122 random bits from the operating
/// system's random source: the standard library seeds every [`std::hash::RandomState`] from it, so no new package and
/// no `uuid` feature is needed (`uuid/v4` is banned outside dev edges, CI-PR-09, doc 18 §3.5). The token is a secret of
/// one run on the loopback and is written only to `session.json`.
// Allowed: the keeper's token is the one place that needs a secret from the operating system, and it never reaches a
// persisted result a test compares (it lives in `session.json` of one run and dies with the keeper).
#[allow(clippy::disallowed_types)]
pub fn new_token() -> String {
    use std::hash::{BuildHasher, RandomState};
    let a = RandomState::new().hash_one(1u8);
    let b = RandomState::new().hash_one(2u8);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_be_bytes());
    bytes[8..].copy_from_slice(&b.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h = super::run::hex(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// Runs a keeper for `run` until the run ends, the idle period passes or a client ends it. `ready` is called with the
/// port once the keeper listens.
///
/// # Errors
///
/// A prelude step that was not done, a replay that differs ([`Diverged`]), or the product's failure.
pub fn serve(
    run: &Run,
    spawner: &dyn Spawner,
    clock: &dyn Clock,
    mode: Mode,
    idle: Duration,
    ready: &mut dyn FnMut(u16),
) -> Result<()> {
    let (mut product, header) = spawner.spawn(run)?;
    let prelude = parse_script(
        &std::fs::read_to_string(run.path("prelude.uiscript")).context("reading prelude.uiscript")?,
    )?;
    match mode {
        Mode::Fresh => {
            let mut lines = replay_steps(product.as_mut(), &prelude, &[], None)?;
            for (i, line) in lines.iter().enumerate() {
                if !line.contains("\"outcome\":\"done\"") {
                    product.kill();
                    bail!("prelude step {} ({}) was not done: {line}", i + 1, prelude[i].0);
                }
            }
            // A visual run's start frame is the first line of its steps (`start` writes it).
            let first = run.steps()?;
            lines.extend(replay_steps(
                product.as_mut(),
                &first,
                &[],
                Some(&run.path("frames")),
            )?);
            let mut text = format!("{header}\n");
            for l in &lines {
                text.push_str(l);
                text.push('\n');
            }
            write_atomic(&run.path("transcript.jsonl"), &text)?;
        }
        Mode::Resume => {
            let mut all = prelude;
            all.extend(run.steps()?);
            let expected = run.transcript()?;
            if let Err(e) = replay_steps(product.as_mut(), &all, &expected, Some(&run.path("frames"))) {
                product.kill();
                return Err(e);
            }
        }
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).context("binding the loopback socket")?;
    let port = listener.local_addr()?.port();
    let token = new_token();
    write_atomic(
        &session_path(run),
        &format!("{}\n", json!({"port": port, "token": token})),
    )?;
    let (tx, rx) = channel();
    let acceptor = std::thread::spawn(move || {
        for c in listener.incoming() {
            match c {
                Ok(s) => {
                    if tx.send(s).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    ready(port);
    let result = loop {
        match clock.recv(&rx, idle) {
            Recv::Idle => {
                product.kill();
                let _ = run.update(|j| {
                    if j.end.is_none() {
                        j.end = Some("lost".to_owned());
                    }
                });
                break Ok(());
            }
            Recv::Closed => break Ok(()),
            Recv::Conn(stream) => match handle(product.as_mut(), stream, &token) {
                Handled::Continue => {}
                Handled::End => break Ok(()),
                Handled::Lost(why) => {
                    let _ = run.update(|j| {
                        if j.end.is_none() {
                            j.end = Some("lost".to_owned());
                        }
                    });
                    break Err(anyhow!(Lost(why)));
                }
            },
        }
    };
    // Unblock the acceptor: it ends when its channel is closed.
    drop(rx);
    let _ = TcpStream::connect(("127.0.0.1", port));
    let _ = acceptor.join();
    let _ = std::fs::remove_file(session_path(run));
    result
}

enum Handled {
    Continue,
    End,
    Lost(String),
}

fn reply(stream: &mut TcpStream, v: &Value) {
    let _ = stream.write_all(format!("{v}\n").as_bytes());
    let _ = stream.flush();
}

fn handle(product: &mut dyn Product, mut stream: TcpStream, token: &str) -> Handled {
    // Loopback only: the listener is bound to 127.0.0.1, and a peer that is anything else is dropped as well.
    if !stream.peer_addr().is_ok_and(|a| a.ip().is_loopback()) {
        return Handled::Continue;
    }
    // A client that sends nothing must not hold the keeper for ever.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let mut line = String::new();
    let Ok(clone) = stream.try_clone() else {
        return Handled::Continue;
    };
    if BufReader::new(clone.take(MAX_REQUEST))
        .read_line(&mut line)
        .is_err()
    {
        return Handled::Continue;
    }
    let Ok(req) = serde_json::from_str::<Value>(&line) else {
        return Handled::Continue;
    };
    // A first line without the token closes the connection and says nothing.
    if req["token"].as_str() != Some(token) {
        return Handled::Continue;
    }
    if req["shutdown"].as_bool() == Some(true) {
        product.kill();
        reply(&mut stream, &json!({"ok": true}));
        return Handled::End;
    }
    if req["end"].as_bool() == Some(true) {
        let code = product.finish().ok().flatten();
        reply(&mut stream, &json!({"ok": true, "exit_code": code}));
        return Handled::End;
    }
    let Some(steps) = req["steps"].as_array() else {
        reply(&mut stream, &json!({"ok": false, "error": "no steps"}));
        return Handled::Continue;
    };
    let mut lines = Vec::new();
    for s in steps {
        let (Some(action), params) = (
            s["action"].as_str(),
            s.get("params").cloned().unwrap_or_else(|| json!({})),
        ) else {
            reply(
                &mut stream,
                &json!({"ok": false, "error": "a step has no action"}),
            );
            return Handled::Continue;
        };
        match product.send(&step_line(action, &params)) {
            Ok(l) => lines.push(l),
            Err(e) => {
                reply(&mut stream, &json!({"ok": false, "lost": e.to_string()}));
                return Handled::Lost(e.to_string());
            }
        }
    }
    reply(&mut stream, &json!({"ok": true, "lines": lines}));
    Handled::Continue
}

/// The client of a keeper: what `do` and `look` send through.
pub struct KeeperClient {
    port: u16,
    token: String,
}

impl KeeperClient {
    /// Reads `session.json` of `run`.
    ///
    /// # Errors
    ///
    /// [`Lost`] when there is no session file.
    pub fn open(run: &Run) -> Result<KeeperClient> {
        let text = std::fs::read_to_string(session_path(run)).map_err(|_| Lost("no session".to_owned()))?;
        let v: Value =
            serde_json::from_str(&text).map_err(|_| Lost("session.json is not JSON".to_owned()))?;
        let port = v["port"]
            .as_u64()
            .and_then(|p| u16::try_from(p).ok())
            .ok_or_else(|| Lost("session.json has no port".to_owned()))?;
        let token = v["token"]
            .as_str()
            .ok_or_else(|| Lost("session.json has no token".to_owned()))?
            .to_owned();
        Ok(KeeperClient { port, token })
    }

    /// Sends a request with the token and reads the answer.
    ///
    /// # Errors
    ///
    /// [`Lost`] when the keeper does not answer.
    pub fn request(&self, req: &Value) -> Result<Value> {
        let mut req = req.clone();
        req["token"] = Value::from(self.token.as_str());
        let lost = |e: std::io::Error| Lost(format!("the keeper does not answer: {e}"));
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).map_err(lost)?;
        stream.write_all(format!("{req}\n").as_bytes()).map_err(lost)?;
        stream.flush().map_err(lost)?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).map_err(lost)?;
        if line.is_empty() {
            return Err(Lost("the keeper closed the connection".to_owned()).into());
        }
        let v: Value =
            serde_json::from_str(&line).map_err(|_| Lost("the keeper's answer is not JSON".to_owned()))?;
        Ok(v)
    }

    /// Ends the keeper and its product; the answer holds the product's exit code.
    ///
    /// # Errors
    ///
    /// [`Lost`] when the keeper does not answer.
    pub fn end(&self) -> Result<Option<i32>> {
        let v = self.request(&json!({"end": true}))?;
        Ok(v["exit_code"].as_i64().and_then(|c| i32::try_from(c).ok()))
    }

    /// Ends the keeper at once.
    ///
    /// # Errors
    ///
    /// [`Lost`] when the keeper does not answer.
    pub fn shutdown(&self) -> Result<()> {
        self.request(&json!({"shutdown": true})).map(|_| ())
    }
}

impl Backend for KeeperClient {
    fn execute(&mut self, step: &Step) -> Result<Sent> {
        let v = self.request(&json!({"steps": [{"action": step.action, "params": step.params}]}))?;
        if v["ok"].as_bool() != Some(true) {
            if let Some(why) = v["lost"].as_str() {
                return Err(Lost(why.to_owned()).into());
            }
            bail!(
                "the keeper refused the step: {}",
                v["error"].as_str().unwrap_or("?")
            );
        }
        let line = v["lines"][0]
            .as_str()
            .ok_or_else(|| anyhow!("the keeper's answer holds no transcript line"))?;
        Sent::parse(line.to_owned())
    }

    fn end_call(&mut self) -> Result<Option<i32>> {
        Ok(None)
    }

    fn finish(&mut self) -> Result<Option<i32>> {
        self.end()
    }
}

/// The keeper's program: the run's copy of the harness, never the build's own (a running `xtask` in `target/` would lock
/// the next build on Windows).
pub fn keeper_program(run: &Run) -> PathBuf {
    run.harness_exe()
}

/// Starts a detached keeper for `run` from the run's copied harness and waits for its ready line.
///
/// # Errors
///
/// The spawn's, or the keeper's own error line (a prelude that was not done, a replay that differs).
pub fn spawn_keeper(run: &Run, resume: bool, idle_minutes: u64) -> Result<u16> {
    let stderr = std::fs::File::create(run.path(".harness/keeper.stderr"))
        .context("creating the keeper's stderr file")?;
    let mut cmd = Command::new(keeper_program(run));
    cmd.arg("blind").arg("serve").arg(&run.dir);
    if resume {
        cmd.arg("--resume");
    }
    cmd.arg("--idle-minutes").arg(idle_minutes.to_string());
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
        cmd.creation_flags(0x0000_0008 | 0x0000_0200 | 0x0800_0000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .with_context(|| format!("starting {}", keeper_program(run).display()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("the keeper has no stdout"))?;
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line)?;
    // The keeper outlives this call: the handle is let go without waiting.
    drop(child);
    let line = line.trim();
    match line.strip_prefix("ready ") {
        Some(port) => port.parse().context("the keeper's port"),
        None => {
            if let Some(d) = line.strip_prefix("diverged ") {
                let step: usize = d.split(' ').next().and_then(|s| s.parse().ok()).unwrap_or(0);
                return Err(Diverged {
                    step,
                    what: d.split_once(' ').map_or(String::new(), |x| x.1.to_owned()),
                }
                .into());
            }
            bail!(
                "the keeper did not start: {}",
                if line.is_empty() { "no answer" } else { line }
            )
        }
    }
}

/// `serve <run-dir> [--resume] [--idle-minutes n]`: the keeper's own process.
pub fn serve_command(args: &[String]) -> Result<bool> {
    let dir = args
        .first()
        .ok_or_else(|| anyhow!("serve needs the run folder"))?;
    let run = Run {
        dir: PathBuf::from(dir),
    };
    let mut mode = Mode::Fresh;
    let mut minutes = DEFAULT_IDLE_MINUTES;
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--resume" => mode = Mode::Resume,
            "--idle-minutes" => {
                minutes = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| anyhow!("--idle-minutes needs a number"))?;
            }
            other => bail!("serve: unknown argument `{other}`"),
        }
    }
    let result = serve(
        &run,
        &super::session::RealSpawner,
        &SystemClock,
        mode,
        Duration::from_secs(minutes.saturating_mul(60)),
        &mut |port| {
            let mut out = std::io::stdout();
            let _ = writeln!(out, "ready {port}");
            let _ = out.flush();
        },
    );
    match result {
        Ok(()) => Ok(true),
        Err(e) => {
            let mut out = std::io::stdout();
            match e.downcast_ref::<Diverged>() {
                Some(d) => {
                    let _ = run.update(|j| {
                        j.end = Some("diverged".to_owned());
                        j.diverged_at = Some(d.step);
                    });
                    let _ = writeln!(out, "diverged {} {}", d.step, d.what);
                }
                None => {
                    let _ = writeln!(out, "error {e:#}");
                }
            }
            let _ = out.flush();
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::TryRecvError;

    use super::*;
    use crate::blind::run::script_header;
    use crate::blind::session::fake::FakeSpawner;
    use crate::blind::testkit::{Tmp, test_run};

    /// A clock that never waits: the test says when the idle period has passed.
    struct Manual(Arc<AtomicBool>);

    impl Clock for Manual {
        fn recv(&self, rx: &Receiver<TcpStream>, _: Duration) -> Recv {
            loop {
                match rx.try_recv() {
                    Ok(c) => return Recv::Conn(c),
                    Err(TryRecvError::Disconnected) => return Recv::Closed,
                    Err(TryRecvError::Empty) => {}
                }
                if self.0.load(Ordering::SeqCst) {
                    return Recv::Idle;
                }
                std::thread::yield_now();
            }
        }
    }

    /// Runs a keeper in a thread; returns the run, a client and the thread.
    fn start(
        run: &Run,
        spawner: FakeSpawner,
        mode: Mode,
        idle: Arc<AtomicBool>,
    ) -> (KeeperClient, std::thread::JoinHandle<Result<()>>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let r = run.clone();
        let handle = std::thread::spawn(move || {
            serve(
                &r,
                &spawner,
                &Manual(idle),
                mode,
                Duration::from_secs(1),
                &mut |p| {
                    let _ = tx.send(p);
                },
            )
        });
        match rx.recv() {
            Ok(_) => {}
            Err(_) => {
                // The keeper ended before it listened: its error is the test's.
                handle.join().unwrap().unwrap();
                panic!("the keeper ended without listening");
            }
        }
        (KeeperClient::open(run).unwrap(), handle)
    }

    fn step(action: &str) -> Step {
        Step {
            action: action.to_owned(),
            params: json!({}),
        }
    }

    /// BD8: the token is a UUID v4, the socket is the loopback, and a request without the token gets no answer.
    #[test]
    fn the_keeper_wants_its_token_and_listens_on_the_loopback_only() {
        assert_ne!(new_token(), new_token(), "every token is fresh");
        let tmp = Tmp::new("keeper-token");
        let run = test_run(&tmp);
        let (mut client, handle) = start(&run, FakeSpawner::default(), Mode::Fresh, Arc::default());
        let session: Value =
            serde_json::from_str(&std::fs::read_to_string(session_path(&run)).unwrap()).unwrap();
        let token = session["token"].as_str().unwrap();
        assert_eq!(token.len(), 36);
        assert_eq!(token.as_bytes()[14], b'4', "a UUID v4: {token}");
        assert!(
            matches!(token.as_bytes()[19], b'8' | b'9' | b'a' | b'b'),
            "variant bits: {token}"
        );
        let port = u16::try_from(session["port"].as_u64().unwrap()).unwrap();
        // No token, a wrong token: the connection is closed with nothing said.
        for bad in [json!({"steps": []}), json!({"token": "nope", "steps": []})] {
            let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
            assert!(s.peer_addr().unwrap().ip().is_loopback());
            s.write_all(format!("{bad}\n").as_bytes()).unwrap();
            let mut answer = String::new();
            BufReader::new(s).read_line(&mut answer).unwrap();
            assert_eq!(answer, "", "no answer to {bad}");
        }
        // The right token works; the answer holds the transcript line.
        let sent = client.execute(&step("help.about")).unwrap();
        assert_eq!(sent.outcome(), "done");
        assert_eq!(client.finish().unwrap(), Some(0));
        handle.join().unwrap().unwrap();
        // The session file is gone with the keeper.
        assert!(!session_path(&run).exists());
    }

    #[test]
    fn the_prelude_writes_the_transcript_and_a_failed_prelude_step_ends_the_keeper() {
        let tmp = Tmp::new("keeper-prelude");
        let run = test_run(&tmp);
        let (mut client, handle) = start(&run, FakeSpawner::default(), Mode::Fresh, Arc::default());
        let lines = run.transcript().unwrap();
        assert_eq!(lines.len(), 1, "the prelude's one step");
        assert!(lines[0].contains("\"action\":\"file.new\""));
        let header = std::fs::read_to_string(run.path("transcript.jsonl")).unwrap();
        assert!(header.starts_with("{\"$schema\":\"ew.uitranscript/1\""));
        client.finish().unwrap();
        handle.join().unwrap().unwrap();
        // A prelude whose step is refused ends the keeper before it listens.
        std::fs::write(
            run.path("prelude.uiscript"),
            format!("{}\n{}\n", script_header("p"), step_line("refuse.me", &json!({}))),
        )
        .unwrap();
        let err = serve(
            &run,
            &FakeSpawner::default(),
            &Manual(Arc::default()),
            Mode::Fresh,
            Duration::from_secs(1),
            &mut |_| panic!("listening"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("was not done"), "{err}");
    }

    /// BD8: `do` against a stopped keeper is a lost session.
    #[test]
    fn a_stopped_keeper_is_a_lost_session() {
        let tmp = Tmp::new("keeper-lost");
        let run = test_run(&tmp);
        let (mut client, handle) = start(&run, FakeSpawner::default(), Mode::Fresh, Arc::default());
        client.shutdown().unwrap();
        handle.join().unwrap().unwrap();
        // The client's port is closed now.
        let err = client.execute(&step("help.about")).unwrap_err();
        assert!(err.downcast_ref::<Lost>().is_some(), "{err}");
        // And without a session file there is no client at all.
        let err = KeeperClient::open(&run).err().unwrap();
        assert!(err.downcast_ref::<Lost>().is_some());
    }

    /// BD8: the idle end, with a manual clock: the product is ended and the run says `lost`.
    #[test]
    fn the_keeper_ends_after_its_idle_period_and_leaves_end_lost() {
        let tmp = Tmp::new("keeper-idle");
        let run = test_run(&tmp);
        let idle = Arc::new(AtomicBool::new(false));
        let spawner = FakeSpawner::default();
        let (_client, handle) = start(&run, spawner.clone(), Mode::Fresh, Arc::clone(&idle));
        assert_eq!(run.load().unwrap().end, None);
        idle.store(true, Ordering::SeqCst);
        handle.join().unwrap().unwrap();
        assert_eq!(run.load().unwrap().end.as_deref(), Some("lost"));
    }

    /// BD8: `resume` replays the prelude and the accepted steps, and the run goes on when the outcomes repeat; a
    /// product that changes one outcome ends the run `diverged` at that step.
    #[test]
    fn a_resume_replays_and_notices_a_changed_outcome() {
        let tmp = Tmp::new("keeper-resume");
        let run = test_run(&tmp);
        let spawner = FakeSpawner {
            change_from: Some(2),
            ..FakeSpawner::default()
        };
        // The first life: prelude, then three steps; their lines go to the transcript and the steps to the script.
        let (mut client, handle) = start(&run, spawner.clone(), Mode::Fresh, Arc::default());
        let mut transcript = std::fs::read_to_string(run.path("transcript.jsonl")).unwrap();
        for action in ["help.about", "change.me", "ui.describe"] {
            let sent = client.execute(&step(action)).unwrap();
            assert_eq!(sent.outcome(), "done");
            transcript.push_str(&sent.raw);
            transcript.push('\n');
            crate::blind::run::append(
                &run.path("steps.uiscript"),
                &format!("{}\n", step_line(action, &json!({}))),
            )
            .unwrap();
        }
        std::fs::write(run.path("transcript.jsonl"), &transcript).unwrap();
        client.shutdown().unwrap();
        handle.join().unwrap().unwrap();
        // The second life changes one outcome (`change.me` is refused from the second fake on): diverged at step 3
        // (the prelude is step 1, `help.about` step 2).
        let err = serve(
            &run,
            &spawner,
            &Manual(Arc::default()),
            Mode::Resume,
            Duration::from_secs(1),
            &mut |_| panic!("a diverged replay does not listen"),
        )
        .unwrap_err();
        let d = err.downcast_ref::<Diverged>().expect("diverged");
        assert_eq!(d.step, 3);
        assert!(d.what.contains("outcome"), "{}", d.what);
        // A product that repeats its outcomes resumes and goes on.
        let same = FakeSpawner::default();
        let (mut client, handle) = start(&run, same, Mode::Resume, Arc::default());
        let sent = client.execute(&step("help.about")).unwrap();
        assert_eq!(sent.outcome(), "done");
        client.shutdown().unwrap();
        handle.join().unwrap().unwrap();
    }

    #[test]
    fn the_spawned_keeper_is_the_runs_copy_never_the_build() {
        let tmp = Tmp::new("keeper-copy");
        let run = Run { dir: tmp.0.join("r") };
        let program = keeper_program(&run);
        assert!(program.starts_with(&run.dir), "{}", program.display());
        assert!(program.ends_with(format!("bin/xtask{}", std::env::consts::EXE_SUFFIX)));
        assert!(!program.components().any(|c| c.as_os_str() == "target"));
    }

    /// BV5, visual spec §5.4: a replay paints only the frames it does not have; inputs and frame lines compare on
    /// `dispatched`, and a different dispatch is a divergence.
    #[test]
    fn a_visual_replay_paints_only_missing_frames_and_compares_dispatched() {
        let tmp = Tmp::new("keeper-visual");
        let frames = tmp.0.join("frames");
        std::fs::create_dir_all(&frames).unwrap();
        std::fs::write(frames.join("t0000.png"), b"png").unwrap();
        let spawner = FakeSpawner::default();
        let run = test_run(&tmp);
        let (mut product, _) = spawner.spawn(&run).unwrap();
        let steps = vec![
            (FRAME_LINE.to_owned(), json!({"frame": "t0000.png"})),
            (
                crate::blind::run::INPUT_LINE.to_owned(),
                json!({"input": "key", "chord": "ctrl+z"}),
            ),
            (FRAME_LINE.to_owned(), json!({"frame": "t0001.png"})),
        ];
        let lines = replay_steps(product.as_mut(), &steps, &[], Some(&frames)).unwrap();
        let log = spawner.log.lock().unwrap().clone();
        assert!(log[0].contains("\"paint\":false"), "{}", log[0]);
        assert!(!log[1].contains("paint"), "{}", log[1]);
        assert!(
            !log[2].contains("paint"),
            "a missing frame is painted: {}",
            log[2]
        );
        assert!(lines[0].contains("\"sha256\":null"), "{}", lines[0]);
        assert!(lines[2].contains("\"sha256\":\"abab"), "{}", lines[2]);
        // Without a frames folder nothing is painted, and the outcomes are the same.
        let (mut product, _) = spawner.spawn(&run).unwrap();
        let again = replay_steps(product.as_mut(), &steps, &lines, None).unwrap();
        assert!(spawner.log.lock().unwrap()[5].contains("\"paint\":false"));
        assert_eq!(again.len(), 3);
        // A different dispatch diverges.
        let mut other = lines.clone();
        other[1] = other[1].replace("edit.undo", "edit.redo");
        let (mut product, _) = spawner.spawn(&run).unwrap();
        let err = replay_steps(product.as_mut(), &steps, &other, None).unwrap_err();
        let d = err.downcast_ref::<Diverged>().unwrap();
        assert_eq!(d.step, 2);
        assert!(d.what.starts_with("dispatched was"), "{}", d.what);
    }
}
