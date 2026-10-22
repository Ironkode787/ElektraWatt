// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The headless mode (WP-S2.01 part 1, spec section 6): `elektrawatt --headless --script <file|->` runs a UI
//! script on the same `ShellModel::dispatch` as the window, with no window, no GPU and no display, and writes a
//! deterministic transcript. This module adds the page raster (`view.snapshot`) to the core runner of
//! `ew_viewmodel::script`, and the smoke run of architecture section 10.1.
//!
//! Exit codes: 0 all steps done; 1 a step was refused or failed (the run stops there); 2 a usage error (an
//! unreadable or malformed script, an unknown action ID).

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use ew_render_cpu::encode_png;
use ew_viewmodel::script::{self, RunEnd, StepExtension};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::page_view::render_page;

/// Where the script comes from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScriptSource {
    /// A script file, read whole and checked before the first step runs.
    File(PathBuf),
    /// Standard input, run line by line with one flushed transcript line per step.
    Stdin,
}

/// The arguments of `elektrawatt --headless`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HeadlessArgs {
    /// The script.
    pub script: ScriptSource,
    /// `--record <file.ewcmd>`: where to write the committed `command.run` steps as a command log.
    pub record: Option<PathBuf>,
}

/// `view.snapshot`: the current page as a PNG at the given dpi through the page view's render path.
struct Snapshot;

impl StepExtension for Snapshot {
    fn handle(&mut self, shell: &mut ShellModel, action: &str, params: &Value) -> Option<Outcome> {
        if action != "view.snapshot" {
            return None;
        }
        Some(snapshot(shell, params))
    }
}

fn snapshot(shell: &ShellModel, params: &Value) -> Outcome {
    let failed = |key: &str, args: &[&str]| Outcome::Failed {
        message: strings::format(key, args),
    };
    let Some(path) = params.get("path").and_then(Value::as_str) else {
        return failed("error-missing-param", &["path"]);
    };
    let dpi = match params.get("dpi") {
        None => 150,
        Some(v) => match v.as_u64().and_then(|d| u32::try_from(d).ok()).filter(|d| *d > 0) {
            Some(d) => d,
            None => return failed("error-bad-param", &["dpi"]),
        },
    };
    let Some(folio) = shell.current_folio() else {
        return Outcome::Refused {
            reason_key: "reason-no-page",
        };
    };
    let pages = match shell.page_content(folio) {
        Ok(p) => p,
        Err(message) => return Outcome::Failed { message },
    };
    let image = match render_page(&pages, dpi) {
        Ok(i) => i,
        Err(message) => return Outcome::Failed { message },
    };
    match std::fs::write(path, encode_png(&image, Some(dpi))) {
        Ok(()) => Outcome::Done,
        Err(_) => failed("error-write", &[]),
    }
}

/// Runs the headless mode and returns the process exit code. The transcript goes to `out` (one flushed line per
/// step), diagnostics to `err`.
pub fn run(
    config: ShellConfig,
    args: &HeadlessArgs,
    stdin: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let mut shell = ShellModel::new(Mode::Headless, config);
    let mut sink = |line: &str| -> io::Result<()> {
        out.write_all(line.as_bytes())?;
        out.write_all(b"\n")?;
        out.flush()
    };
    let ended = match &args.script {
        ScriptSource::File(path) => {
            let text = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    let _ = writeln!(err, "error: {}: {e}", path.display());
                    return 2;
                }
            };
            let steps = match script::parse(&text) {
                Ok(steps) => steps,
                Err(e) => {
                    let _ = writeln!(err, "error: {}: {e}", path.display());
                    return 2;
                }
            };
            // The origin of model commands names the script by its file name, never its path.
            if let Some(name) = path.file_name() {
                shell.set_origin_file(&name.to_string_lossy());
            }
            script::run(&mut shell, &mut Snapshot, &steps, &mut sink)
        }
        ScriptSource::Stdin => {
            shell.set_origin_file("stdin");
            let mut lines = stdin.lines();
            script::run_stream(&mut shell, &mut Snapshot, &mut lines, &mut sink)
        }
    };
    let end = match ended {
        Ok(end) => end,
        Err(e) => {
            let _ = writeln!(err, "error: {e}");
            shell.shutdown();
            return 2;
        }
    };
    if let RunEnd::Usage(e) = &end {
        let _ = writeln!(err, "error: {e}");
    }
    if let Some(path) = &args.record
        && let Err(e) = std::fs::write(path, script::record_log(&shell))
    {
        let _ = writeln!(err, "error: {}: {e}", path.display());
        shell.shutdown();
        return 2;
    }
    shell.shutdown();
    end.exit_code()
}

/// `elektrawatt --smoke <project-dir>` (architecture section 10.1): opens the project read-only, renders its first
/// page at 36 dpi through the page-view path and exits. Returns 0 or 1.
pub fn smoke(mut config: ShellConfig, dir: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    // A smoke run reads: it leaves no settings file and no recent-projects entry behind.
    config.config_dir = None;
    let mut shell = ShellModel::new(Mode::Headless, config);
    let opened = shell.dispatch(
        "file.open",
        &json!({ "dir": dir.to_string_lossy(), "read_only": true }),
    );
    let code = match opened {
        Outcome::Done => smoke_first_page(&shell, out, err),
        Outcome::Refused { reason_key } => {
            let _ = writeln!(err, "error: {}", strings::text(reason_key));
            1
        }
        Outcome::Failed { message } => {
            let _ = writeln!(err, "error: {message}");
            1
        }
    };
    shell.shutdown();
    code
}

fn smoke_first_page(shell: &ShellModel, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some(folio) = shell.current_folio() else {
        let _ = writeln!(err, "error: {}", strings::text("page-no-pages"));
        return 1;
    };
    let rendered = shell
        .page_content(folio)
        .and_then(|pages| render_page(&pages, 36));
    match rendered {
        Ok(image) => {
            let _ = writeln!(
                out,
                "smoke ok: {} pages, page {folio} drawn {} x {} px at 36 dpi",
                shell.page_count(),
                image.width,
                image.height
            );
            0
        }
        Err(message) => {
            let _ = writeln!(err, "error: {message}");
            1
        }
    }
}
