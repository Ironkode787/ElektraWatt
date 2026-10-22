// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ElektraWatt desktop application: eframe shell, file dialogs, the headless mode and exit codes. Native menus,
//! clipboard and the crash handler come with later work packages (WP-S2.01 part 2, FT-310).
//!
//! ```text
//! elektrawatt [<project-dir>]                          the window; the project opens after the first frame
//! elektrawatt --headless --script <file|-> [--record <file.ewcmd>] [--config-dir <dir>]
//!                                                      the same actions from a UI script, no window, no GPU
//! elektrawatt --smoke <project-dir>                    opens read-only, draws page 1 at 36 dpi, exits
//! ```
//!
//! Exit codes of the headless mode: 0 every step done, 1 a step was refused or failed, 2 a usage error.
//!
//! The read-only resources (the sample project, the starter content, the built user guide, the third-party notices)
//! are found next to the executable, in `share/elektrawatt` one level up, or in the source tree of a development
//! build; `EW_SAMPLE_DIR`, `EW_CONTENT_DIR` and `EW_DOCS_DIR` override the first three. A missing one disables only
//! the feature that needs it.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use eframe::egui;
use ew_ui::app::{App, Pick, Platform, setup};
use ew_ui::ew_viewmodel::shell::{Mode, Prompt, Resources, ShellConfig, ShellModel};
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
       elektrawatt --headless --script <file|-> [--record <file.ewcmd>] [--config-dir <dir>]\n\
       elektrawatt --smoke <project-dir>\n\
       elektrawatt --version | --help";

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Cli, String> {
    let mut args = args.into_iter();
    let mut headless = false;
    let mut script: Option<ScriptSource> = None;
    let mut record: Option<PathBuf> = None;
    let mut config_dir: Option<PathBuf> = None;
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
            "--config-dir" => config_dir = Some(PathBuf::from(value("--config-dir")?)),
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
        (true, Some(script), None) if project.is_none() => Ok(Cli::Headless(HeadlessArgs {
            script,
            record,
            config_dir,
        })),
        (true, None, _) => Err("--headless needs --script <file|->".to_owned()),
        (true, _, _) => Err("--headless takes a script and no project or --smoke".to_owned()),
        (false, Some(_), _) => Err("--script needs --headless".to_owned()),
        (false, None, _) if record.is_some() => Err("--record needs --headless".to_owned()),
        (false, None, _) if config_dir.is_some() => Err("--config-dir needs --headless".to_owned()),
        (false, None, Some(dir)) if project.is_none() => Ok(Cli::Smoke(dir)),
        (false, None, Some(_)) => Err("--smoke takes one directory".to_owned()),
        (false, None, None) => Ok(Cli::Gui(project)),
    }
}

/// The file dialogs of the operating system through `rfd`.
struct RfdPlatform;

/// An environment variable as a path, `None` when unset or empty.
// Program boundary: the portal's session bus (`DBUS_SESSION_BUS_ADDRESS`, `XDG_RUNTIME_DIR`) and the directories of
// the application's resources (`EW_SAMPLE_DIR`, `EW_CONTENT_DIR`, `EW_DOCS_DIR`) are read once at start-up.
#[allow(clippy::disallowed_methods)]
fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Whether a session bus is there for the xdg desktop portal, which `rfd` talks to on Linux: the address in the
/// environment, or the bus socket in the runtime directory. Without one `rfd` returns nothing, silently.
fn session_bus_present(address: Option<PathBuf>, runtime_dir: Option<PathBuf>) -> bool {
    address.is_some() || runtime_dir.is_some_and(|d| d.join("bus").exists())
}

impl Platform for RfdPlatform {
    fn pick(&mut self, prompt: Prompt) -> Pick {
        if cfg!(target_os = "linux")
            && !session_bus_present(env_path("DBUS_SESSION_BUS_ADDRESS"), env_path("XDG_RUNTIME_DIR"))
        {
            return Pick::Unavailable;
        }
        let picked = match prompt {
            Prompt::OpenFolder => rfd::FileDialog::new()
                .set_title(strings::text("prompt-open-title"))
                .pick_folder(),
            Prompt::NewFolder => rfd::FileDialog::new()
                .set_title(strings::text("prompt-save-as-title"))
                .pick_folder(),
            Prompt::OpenQet => rfd::FileDialog::new()
                .set_title(strings::text("prompt-open-qet-title"))
                .add_filter("QElectroTech", &["qet"])
                .pick_file(),
            Prompt::SavePdf => rfd::FileDialog::new()
                .set_title(strings::text("prompt-export-title"))
                .set_file_name(strings::text("prompt-pdf-name"))
                .add_filter("PDF", &["pdf"])
                .save_file(),
        };
        picked.map_or(Pick::Cancelled, Pick::Path)
    }

    fn open_url(&mut self, url: &str) -> bool {
        open_in_browser(url)
    }
}

/// Opens `url` (a `file://` URL of the installed user guide) with the system's opener: a user action, no network
/// traffic. The opener is the one of the platform; its failure is reported to the caller.
fn open_in_browser(url: &str) -> bool {
    let (program, args) = opener(std::env::consts::OS, url);
    std::process::Command::new(program).args(args).spawn().is_ok()
}

/// The program and arguments that open `url` on the operating system `os` (`std::env::consts::OS`). No shell is
/// involved on any system, so no character of the URL has a meaning to a command line; the URL is percent-encoded
/// besides (`ew_viewmodel`'s `file_url`).
fn opener(os: &str, url: &str) -> (&'static str, Vec<String>) {
    match os {
        "windows" => (
            "rundll32",
            vec!["url.dll,FileProtocolHandler".to_owned(), url.to_owned()],
        ),
        "macos" => ("open", vec![url.to_owned()]),
        _ => ("xdg-open", vec![url.to_owned()]),
    }
}

/// The first of `candidates` that is a directory.
fn first_dir(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| p.is_dir())
}

/// Finds the application's read-only resources (decision log: part 2 of WP-S2.01). Each has an environment
/// override (`EW_SAMPLE_DIR`, `EW_CONTENT_DIR`, `EW_DOCS_DIR`) and then the places an installation or a development
/// build has it, relative to the executable: next to it, in `share/elektrawatt` one level up, and, in a build from
/// the source tree (`target/<profile>/elektrawatt`), the repository's own directories. A resource that is not
/// found disables only the feature that needs it.
fn locate_resources(exe: Option<&Path>) -> Resources {
    let exe_dir = exe.and_then(Path::parent).map(Path::to_path_buf);
    let near = |rel: &str| -> Vec<PathBuf> {
        let Some(dir) = &exe_dir else {
            return Vec::new();
        };
        vec![
            dir.join(rel),
            dir.join("..").join("share").join("elektrawatt").join(rel),
        ]
    };
    let tree = |rel: &str| -> Option<PathBuf> { exe_dir.as_ref().map(|d| d.join("../..").join(rel)) };
    let find_dir = |var: &str, rel: &str, tree_rel: &str| -> Option<PathBuf> {
        env_path(var)
            .or_else(|| first_dir(near(rel)))
            .or_else(|| first_dir(tree(tree_rel)))
            .map(|p| p.canonicalize().unwrap_or(p))
    };
    let notices = exe_dir.as_ref().map(|d| d.join("THIRD-PARTY-NOTICES.txt"));
    Resources {
        sample_dir: find_dir("EW_SAMPLE_DIR", "samples/J-E-2416.ew", "golden/J-E-2416.ew"),
        content_dir: find_dir("EW_CONTENT_DIR", "content", "content"),
        docs_dir: find_dir("EW_DOCS_DIR", "docs", "target/book"),
        notices_file: notices.filter(|p| p.is_file()),
    }
}

/// The path of this executable, `None` when the system does not say.
fn current_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

/// The configuration of the environment with the resources of this installation.
fn shell_config(isolated: bool) -> ShellConfig {
    let config = if isolated {
        ShellConfig::isolated_from_env(VERSION)
    } else {
        ShellConfig::from_env(VERSION)
    };
    config.with_resources(locate_resources(current_exe().as_deref()))
}

/// The eframe application: the shell frame of `ew-ui`.
struct ShellApp(App);

impl eframe::App for ShellApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }

    fn on_exit(&mut self) {
        // Shutdown at exit: the project lock is released; the journal is kept while the project is dirty
        // (crash-recovery semantics, doc 05 section 8) and removed only after a clean close.
        self.0.shell_mut().shutdown();
    }
}

fn run_gui(initial: Option<PathBuf>) -> i32 {
    let mut shell = ShellModel::new(Mode::Gui, shell_config(false));
    // The first start (and only that) shows the first-run flow, unless the user named a project to open.
    if initial.is_none() {
        shell.start_first_run_if_needed();
    }
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
                shell_config(false),
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
                shell_config(true),
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
                record: None,
                config_dir: None
            }))
        );
        assert_eq!(
            parse(&["--headless", "--script", "-", "--record", "r.ewcmd"]),
            Ok(Cli::Headless(HeadlessArgs {
                script: ScriptSource::Stdin,
                record: Some(PathBuf::from("r.ewcmd")),
                config_dir: None
            }))
        );
        assert_eq!(
            parse(&["--headless", "--script", "s", "--config-dir", "c"]),
            Ok(Cli::Headless(HeadlessArgs {
                script: ScriptSource::File(PathBuf::from("s")),
                record: None,
                config_dir: Some(PathBuf::from("c"))
            }))
        );
        assert!(parse(&["--config-dir", "c"]).is_err());
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

    #[test]
    fn the_guide_is_opened_without_a_shell() {
        let url = "file:///C:/docs/a%26b/x.html#y";
        assert_eq!(
            opener("windows", url),
            (
                "rundll32",
                vec!["url.dll,FileProtocolHandler".to_owned(), url.to_owned()]
            )
        );
        assert_eq!(opener("macos", url), ("open", vec![url.to_owned()]));
        assert_eq!(opener("linux", url), ("xdg-open", vec![url.to_owned()]));
        for os in ["windows", "macos", "linux", "freebsd"] {
            assert!(
                !["cmd", "sh", "bash", "powershell"].contains(&opener(os, url).0),
                "{os}"
            );
        }
    }

    #[test]
    fn the_portal_needs_a_session_bus() {
        assert!(session_bus_present(Some(PathBuf::from("unix:path=/x")), None));
        assert!(!session_bus_present(None, None));
        let dir = std::env::temp_dir().join(format!("ew-bus-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!session_bus_present(None, Some(dir.clone())));
        std::fs::write(dir.join("bus"), b"").unwrap();
        assert!(session_bus_present(None, Some(dir.clone())));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
