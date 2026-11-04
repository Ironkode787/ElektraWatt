// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The local crash log (WP-S6.05 D6, D7; NFR-REL-13's 0.1.0 part). [`install`] runs first in `main`, for every mode:
//! its panic hook appends one entry per panic to the process's log, `<state>/crashes/<yyyymmddThhmmssZ>-<pid>.log`
//! (the process start in UTC; the file is created, user-only, at the first panic), and then calls the hook it replaced,
//! so stderr keeps Rust's message. An entry holds the lines L1 to L7 of WP-S6.05 §5 and the frames of a forced
//! backtrace, and ends with an empty line; a panic that reaches `main`'s outer catch gets the end line [`ENDED_LINE`]
//! after its entry ([`ended`]), which tells a crash from a panic caught on its way (a job, a rule run, a decoder). The
//! lines stay English: support reads them. Nothing is ever sent.
//!
//! The hook takes no lock of the session, the store or the journal (every journal entry is synced as it is appended,
//! so the journal needs nothing from it); a panic inside the hook is not logged again; a write error is ignored.
//! Without a state directory nothing is written.

use std::cell::Cell;
use std::io::Write as _;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use ew_ui::ew_viewmodel::settings::{user_only_dir, user_only_file};
use ew_ui::ew_viewmodel::shell::startup::ENDED_LINE;

/// The start of the process, read once by [`install`].
static START: OnceLock<SystemTime> = OnceLock::new();

/// The process's crash log, once the mode named its state directory ([`set_dir`]).
static LOG: OnceLock<PathBuf> = OnceLock::new();

thread_local! {
    /// Whether this thread is inside the hook: a panic there is not logged again.
    static IN_HOOK: Cell<bool> = const { Cell::new(false) };
}

/// The wall clock: the process start (the log's name) and the time of a panic (L3), from the clock the lock and the
/// journal use (`ew_store::SystemClock`); the crash log, like them, is no project data.
fn now() -> SystemTime {
    use ew_ui::ew_viewmodel::ew_api::Clock as _;
    ew_ui::ew_viewmodel::ew_api::SystemClock.now()
}

/// `t` in UTC as `yyyy-mm-ddThh:mm:ssZ` (`compact`: `yyyymmddThhmmssZ`, for a file name).
fn utc(t: SystemTime, compact: bool) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let Some(d) = i64::try_from(secs)
        .ok()
        .and_then(ew_ui::ew_viewmodel::ew_api::output::date_from_epoch)
    else {
        return String::from("unknown");
    };
    if compact {
        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
            d.year, d.month, d.day, d.hour, d.minute, d.second
        )
    } else {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            d.year, d.month, d.day, d.hour, d.minute, d.second
        )
    }
}

/// Installs the panic hook of this module before the one that was there, which it then calls.
pub(crate) fn install(version: &'static str) {
    let _ = START.set(now());
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record(version, info);
        previous(info);
    }));
}

/// The crash directory of this run (`<state>/crashes`); `None`: no state directory, no log.
pub(crate) fn set_dir(dir: Option<PathBuf>) {
    let (Some(dir), Some(start)) = (dir, START.get()) else {
        return;
    };
    let name = format!("{}-{}.log", utc(*start, true), std::process::id());
    let _ = LOG.set(dir.join(name));
}

/// The text of one entry (L1 to L7, then the frames, then an empty line).
pub(crate) fn entry(version: &str, time: &str, thread: &str, location: &str, message: &str, frames: &str) -> String {
    format!(
        "ElektraWatt {version} crash log\nOS: {} {}\nTime: {time}\nThread: {thread}\nLocation: {location}\n\
         Message: {message}\nBacktrace:\n{}\n\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        frames.trim_end()
    )
}

/// The panic's message: its payload's text, else "(no message)".
fn message(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".to_owned())
}

/// The hook's work: one entry in the log, unless this thread is in the hook already.
fn record(version: &str, info: &PanicHookInfo<'_>) {
    if IN_HOOK.with(|f| f.replace(true)) {
        return;
    }
    if let Some(path) = LOG.get() {
        let thread = std::thread::current();
        let location = info
            .location()
            .map_or_else(|| "unknown".to_owned(), |l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let frames = std::backtrace::Backtrace::force_capture().to_string();
        let text = entry(
            version,
            &utc(now(), false),
            thread.name().unwrap_or("unnamed"),
            &location,
            &message(info),
            &frames,
        );
        append(path, &text);
    }
    IN_HOOK.with(|f| f.set(false));
}

/// Appends `text` to the log at `path`, creating it and its directory user-only (WP-S6.05 D10); errors are ignored.
fn append(path: &Path, text: &str) {
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    user_only_dir(dir);
    if let Some(state) = dir.parent() {
        user_only_dir(state);
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(path) {
        user_only_file(path);
        let _ = file.write_all(text.as_bytes());
        let _ = file.sync_all();
    }
}

/// The panic that `main`'s outer catch caught ended the session: its entry gets the end line (L8).
pub(crate) fn ended() {
    if let Some(path) = LOG.get().filter(|p| p.is_file()) {
        append(path, &format!("{ENDED_LINE}\n"));
    }
}
