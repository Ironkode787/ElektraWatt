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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ew_ui::ew_viewmodel::settings::{user_only_dir, user_only_file};
use ew_ui::ew_viewmodel::shell::startup::ENDED_LINE;

/// The start of the process, read once by [`install`].
static START: OnceLock<SystemTime> = OnceLock::new();

/// The process's crash log, once the mode named its state directory ([`set_dir`]).
static LOG: OnceLock<PathBuf> = OnceLock::new();

/// The most entries one process writes to its log (C50-p3#1): a panic that a job, a rule run or the tessellator
/// catches on every check or redraw would otherwise grow the log without end.
pub(crate) const MAX_ENTRIES: usize = 50;

/// The most bytes of one process's log before it stops taking entries (C50-p3#1).
pub(crate) const MAX_BYTES: u64 = 1024 * 1024;

/// The line after the last entry of a log that reached its cap.
pub(crate) const CAPPED_LINE: &str = "further panics not logged";

/// The entries this process wrote, and whether it wrote [`CAPPED_LINE`].
static ENTRIES: AtomicUsize = AtomicUsize::new(0);
static CAPPED: AtomicBool = AtomicBool::new(false);

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
pub(crate) fn entry(
    version: &str,
    time: &str,
    thread: &str,
    location: &str,
    message: &str,
    frames: &str,
) -> String {
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

/// The hook's work: one entry in the log, unless this thread is in the hook already; past [`MAX_ENTRIES`] or
/// [`MAX_BYTES`] only [`CAPPED_LINE`], once.
fn record(version: &str, info: &PanicHookInfo<'_>) {
    if IN_HOOK.with(|f| f.replace(true)) {
        return;
    }
    let full = |path: &Path| {
        ENTRIES.load(Ordering::Relaxed) >= MAX_ENTRIES
            || std::fs::metadata(path).is_ok_and(|m| m.len() >= MAX_BYTES)
    };
    if let Some(path) = LOG.get().filter(|p| full(p)) {
        if !CAPPED.swap(true, Ordering::Relaxed) {
            append(path, &format!("{CAPPED_LINE}\n"));
        }
    } else if let Some(path) = LOG.get() {
        ENTRIES.fetch_add(1, Ordering::Relaxed);
        let thread = std::thread::current();
        let location = info.location().map_or_else(
            || "unknown".to_owned(),
            |l| format!("{}:{}:{}", l.file(), l.line(), l.column()),
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(stem: &str) -> PathBuf {
        let dir = ew_testdir::fresh_dir(stem);
        install("0.0.0-test");
        set_dir(Some(dir.join("crashes")));
        dir
    }

    fn log_text() -> String {
        std::fs::read_to_string(LOG.get().unwrap()).unwrap()
    }

    /// C50-p3#1: a panic caught over and over writes at most `MAX_ENTRIES` entries, then the cap line once; the
    /// session's end line still follows. Nextest runs each test in a process of its own, so the hook and the count
    /// are this test's.
    #[test]
    fn a_log_stops_taking_entries_at_its_cap() {
        let dir = fresh("ew-crash-cap");
        for n in 0..MAX_ENTRIES + 5 {
            assert!(std::panic::catch_unwind(move || panic!("caught {n}")).is_err());
        }
        let text = log_text();
        assert_eq!(text.matches(" crash log\n").count(), MAX_ENTRIES);
        assert!(text.ends_with(&format!("\n\n{CAPPED_LINE}\n")));
        ended();
        assert!(log_text().ends_with(&format!("{CAPPED_LINE}\n{ENDED_LINE}\n")));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// C50-p3#12: a panic on a thread already in the hook writes nothing (std aborts on a panic inside the hook, so
    /// the guard is reached here by setting it).
    #[test]
    fn a_panic_inside_the_hook_is_not_logged_again() {
        let dir = fresh("ew-crash-guard");
        IN_HOOK.with(|f| f.set(true));
        assert!(std::panic::catch_unwind(|| panic!("inside")).is_err());
        IN_HOOK.with(|f| f.set(false));
        assert!(!LOG.get().unwrap().exists());
        assert!(std::panic::catch_unwind(|| panic!("outside")).is_err());
        assert!(log_text().contains("\nMessage: outside\n"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
