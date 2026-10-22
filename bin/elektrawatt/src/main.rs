// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ElektraWatt desktop application: eframe shell, file dialogs, the headless mode and exit codes. Native menus,
//! clipboard and the crash handler come with later work packages (WP-S2.01 part 2, FT-310).
//!
//! ```text
//! elektrawatt [<project-dir>]                          the window; the project opens after the first frame
//! elektrawatt --headless --script <file|-> [--record <file.ewcmd>]
//!                                                      the same actions from a UI script, no window, no GPU
//! elektrawatt --smoke <project-dir>                    opens read-only, draws page 1 at 36 dpi, exits
//! ```
//!
//! Exit codes of the headless mode: 0 every step done, 1 a step was refused or failed, 2 a usage error.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use eframe::egui;
use ew_ui::app::{App, Platform, setup};
use ew_ui::ew_viewmodel::shell::{Mode, Prompt, ShellConfig, ShellModel};
use ew_ui::ew_viewmodel::strings;
use ew_ui::headless::{self, HeadlessArgs, ScriptSource};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the command line asks for.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Cli {
    /// The window, with an optional project to open.
    Gui(Option<PathBuf>),
    /// The headless mode.
    Headless(HeadlessArgs),
    /// The smoke run.
    Smoke(PathBuf),
    /// `--version`.
    Version,
    /// `--help`.
    Help,
}

const USAGE: &str = "usage: elektrawatt [<project-dir>]\n\
       elektrawatt --headless --script <file|-> [--record <file.ewcmd>]\n\
       elektrawatt --smoke <project-dir>\n\
       elektrawatt --version | --help";

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Cli, String> {
    let mut args = args.into_iter();
    let mut headless = false;
    let mut script: Option<ScriptSource> = None;
    let mut record: Option<PathBuf> = None;
    let mut smoke: Option<PathBuf> = None;
    let mut project: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy().into_owned();
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match text.as_str() {
            "--version" | "-V" => return Ok(Cli::Version),
            "--help" | "-h" => return Ok(Cli::Help),
            "--headless" => headless = true,
            "--script" => {
                let v = value("--script")?;
                script = Some(if v == "-" {
                    ScriptSource::Stdin
                } else {
                    ScriptSource::File(PathBuf::from(v))
                });
            }
            "--record" => record = Some(PathBuf::from(value("--record")?)),
            "--smoke" => smoke = Some(PathBuf::from(value("--smoke")?)),
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => {
                if project.replace(PathBuf::from(&arg)).is_some() {
                    return Err("more than one project directory".to_owned());
                }
            }
        }
    }
    match (headless, script, smoke) {
        (true, Some(script), None) if project.is_none() => Ok(Cli::Headless(HeadlessArgs { script, record })),
        (true, None, _) => Err("--headless needs --script <file|->".to_owned()),
        (true, _, _) => Err("--headless takes a script and no project or --smoke".to_owned()),
        (false, Some(_), _) => Err("--script needs --headless".to_owned()),
        (false, None, _) if record.is_some() => Err("--record needs --headless".to_owned()),
        (false, None, Some(dir)) if project.is_none() => Ok(Cli::Smoke(dir)),
        (false, None, Some(_)) => Err("--smoke takes one directory".to_owned()),
        (false, None, None) => Ok(Cli::Gui(project)),
    }
}

/// The file dialogs of the operating system through `rfd`.
struct RfdPlatform;

impl Platform for RfdPlatform {
    fn pick(&mut self, prompt: Prompt) -> Option<PathBuf> {
        match prompt {
            Prompt::OpenFolder => rfd::FileDialog::new()
                .set_title(strings::text("prompt-open-title"))
                .pick_folder(),
            Prompt::NewFolder => rfd::FileDialog::new()
                .set_title(strings::text("prompt-save-as-title"))
                .pick_folder(),
            Prompt::SavePdf => rfd::FileDialog::new()
                .set_title(strings::text("prompt-export-title"))
                .set_file_name(strings::text("prompt-pdf-name"))
                .add_filter("PDF", &["pdf"])
                .save_file(),
        }
    }
}

/// The eframe application: the shell frame of `ew-ui`.
struct ShellApp(App);

impl eframe::App for ShellApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }

    fn on_exit(&mut self) {
        // A clean close of the session: its journal goes and the project lock is released.
        self.0.shell_mut().shutdown();
    }
}

fn run_gui(initial: Option<PathBuf>) -> i32 {
    let shell = ShellModel::new(Mode::Gui, ShellConfig::from_env(VERSION));
    let app = App::new(shell, Box::new(RfdPlatform), initial);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(strings::text("window-title-start"))
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        renderer: eframe::Renderer::Wgpu,
        ..eframe::NativeOptions::default()
    };
    let result = eframe::run_native(
        strings::text("window-title-start"),
        options,
        Box::new(move |cc| {
            setup(&cc.egui_ctx);
            Ok(Box::new(ShellApp(app)))
        }),
    );
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: the window could not be opened: {e}");
            1
        }
    }
}

fn main() {
    let code = match parse_args(std::env::args_os().skip(1)) {
        Err(message) => {
            eprintln!("error: {message}\n{USAGE}");
            2
        }
        Ok(Cli::Help) => {
            println!("{USAGE}");
            0
        }
        Ok(Cli::Version) => {
            println!("elektrawatt {VERSION}");
            0
        }
        Ok(Cli::Gui(initial)) => run_gui(initial),
        Ok(Cli::Smoke(dir)) => {
            let code = headless::smoke(
                ShellConfig::from_env(VERSION),
                &dir,
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            );
            let _ = std::io::stdout().flush();
            code
        }
        Ok(Cli::Headless(args)) => {
            let stdin = std::io::stdin();
            headless::run(
                ShellConfig::from_env(VERSION),
                &args,
                &mut stdin.lock(),
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            )
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn no_arguments_is_the_window() {
        assert_eq!(parse(&[]), Ok(Cli::Gui(None)));
        assert_eq!(parse(&["p.ew"]), Ok(Cli::Gui(Some(PathBuf::from("p.ew")))));
    }

    #[test]
    fn the_headless_mode_needs_a_script() {
        assert_eq!(
            parse(&["--headless", "--script", "s.jsonl"]),
            Ok(Cli::Headless(HeadlessArgs {
                script: ScriptSource::File(PathBuf::from("s.jsonl")),
                record: None
            }))
        );
        assert_eq!(
            parse(&["--headless", "--script", "-", "--record", "r.ewcmd"]),
            Ok(Cli::Headless(HeadlessArgs {
                script: ScriptSource::Stdin,
                record: Some(PathBuf::from("r.ewcmd"))
            }))
        );
        assert!(parse(&["--headless"]).is_err());
        assert!(parse(&["--script", "s"]).is_err());
        assert!(parse(&["--record", "r"]).is_err());
        assert!(parse(&["--headless", "--script"]).is_err());
        assert!(parse(&["--headless", "--script", "s", "p.ew"]).is_err());
    }

    #[test]
    fn smoke_and_the_informational_flags() {
        assert_eq!(parse(&["--smoke", "p.ew"]), Ok(Cli::Smoke(PathBuf::from("p.ew"))));
        assert_eq!(parse(&["--version"]), Ok(Cli::Version));
        assert_eq!(parse(&["--help"]), Ok(Cli::Help));
        assert!(parse(&["--nonsense"]).is_err());
        assert!(parse(&["a", "b"]).is_err());
    }
}
