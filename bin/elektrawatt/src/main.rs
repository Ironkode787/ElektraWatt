// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ElektraWatt desktop application: eframe shell, file dialogs, the headless mode, the crash log and exit codes.
//! Native menus and clipboard come with a later work package.
//!
//! ```text
//! elektrawatt [--no-gpu] [<project-dir>]               the window; the project opens after the first frame
//! elektrawatt --headless --script <file|-> [--record <file.ewcmd>] [--config-dir <dir>] [--keep-going]
//!                                                      the same actions from a UI script, no window, no GPU
//! elektrawatt --smoke <project-dir>                    opens read-only, draws page 1 at 36 dpi, exits
//! ```
//!
//! A panic that ends the session in any mode is written to the local crash log ([`crash`]) and the process
//! exits with 101. Exit codes of the headless mode: 0 every step done, 1 a step was refused or failed (without `--keep-going` the run
//! stops there; with it the run goes on and the code is 1 when any step was not done), 2 a usage error. The window
//! exits with 3 when the system has no graphics adapter at all (egui's own chrome needs one; section 7.2):
//! the message names the headless mode, the command-line tool `ew` and, on Linux, Mesa's software drivers.
//! `--no-gpu` draws the canvas on the CPU and asks eframe for a software adapter (WARP, llvmpipe, lavapipe) for the
//! window.
//!
//! The read-only resources (the sample project, the starter content, the built user guide, the third-party notices)
//! are found next to the executable, in `share/elektrawatt` one level up, or in the source tree of a development
//! build; `EW_SAMPLE_DIR`, `EW_CONTENT_DIR` and `EW_DOCS_DIR` override the first three. A missing one disables only
//! the feature that needs it.

#![forbid(unsafe_code)]
// No console window before the application on Windows (FT-1710); the terminal modes attach to their parent's console
// ([`attach_console`]).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod crash;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use eframe::egui;
use ew_ui::app::{App, Pick, PickReply, Platform, PrintReply, setup, spawn_pick};
use ew_ui::ew_viewmodel::key;
use ew_ui::ew_viewmodel::shell::print::{PrintEnd, PrintRequest, PrintVia};
use ew_ui::ew_viewmodel::shell::{Mode, Prompt, Resources, ShellConfig, ShellModel};
use ew_ui::ew_viewmodel::strings;
use ew_ui::ew_viewmodel::window::{MonitorArea, WindowGeometry};
use ew_ui::gpu::{Flags, adapter_count, wgpu_configuration};
use ew_ui::headless::{self, HeadlessArgs, ScriptSource};
use ew_ui::visual::Geometry;
use ew_ui::window_memory::{Display, FirstFrameStep};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The application icon at 256 × 256 pixels, the rendering of `assets/icons/app/elektrawatt.svg` (the menu button's
/// mark; `tests/it/app_icon.rs` keeps the two equal): the window's icon. The executable's own icon is that SVG's
/// `elektrawatt.ico`, embedded by `build.rs`.
const APP_ICON_PNG: &[u8] = include_bytes!("../../../assets/icons/app/elektrawatt.png");

/// What the command line asks for.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Cli {
    /// The window, with an optional project to open, and `--no-gpu`.
    Gui(Option<PathBuf>, bool),
    /// The headless mode.
    Headless(HeadlessArgs),
    /// The smoke run.
    Smoke(PathBuf),
    /// `--version`.
    Version,
    /// `--help`.
    Help,
}

const USAGE: &str = "usage: elektrawatt [--no-gpu] [<project-dir>]\n\
       elektrawatt --headless --script <file|-> [--record <file.ewcmd>] [--config-dir <dir>] [--keep-going]\n\
                   [--visual [--frames <dir> | --no-frames] [--window <W>x<H>] [--ppp <n>]]\n\
       elektrawatt --smoke <project-dir>\n\
       elektrawatt --version | --help";

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Cli, String> {
    let mut args = args.into_iter();
    let mut headless = false;
    let mut script: Option<ScriptSource> = None;
    let mut record: Option<PathBuf> = None;
    let mut config_dir: Option<PathBuf> = None;
    let mut keep_going = false;
    let mut visual = false;
    let mut frames: Option<PathBuf> = None;
    let mut no_frames = false;
    let mut window: Option<String> = None;
    let mut ppp: Option<String> = None;
    let mut smoke: Option<PathBuf> = None;
    let mut project: Option<PathBuf> = None;
    let mut no_gpu = false;
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
            "--keep-going" => keep_going = true,
            "--visual" => visual = true,
            "--frames" => frames = Some(PathBuf::from(value("--frames")?)),
            "--no-frames" => no_frames = true,
            "--window" => window = Some(value("--window")?.to_string_lossy().into_owned()),
            "--ppp" => ppp = Some(value("--ppp")?.to_string_lossy().into_owned()),
            "--smoke" => smoke = Some(PathBuf::from(value("--smoke")?)),
            "--no-gpu" => no_gpu = true,
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => {
                if project.replace(PathBuf::from(&arg)).is_some() {
                    return Err("more than one project directory".to_owned());
                }
            }
        }
    }
    if no_gpu && (headless || smoke.is_some()) {
        return Err("--no-gpu is for the window (the headless and smoke modes use no GPU)".to_owned());
    }
    // The visual driver is a headless mode; its frame options need it.
    if visual && !headless {
        return Err("--visual needs --headless".to_owned());
    }
    if (frames.is_some() || no_frames) && !visual {
        return Err("--frames and --no-frames need --visual".to_owned());
    }
    if frames.is_some() && no_frames {
        return Err("--frames and --no-frames exclude each other".to_owned());
    }
    // A visual run's window size and scale (default 1280 × 800 points at 1.2).
    let geometry = Geometry::parse(window.as_deref(), ppp.as_deref())?;
    if (window.is_some() || ppp.is_some()) && !visual {
        return Err("--window and --ppp need --visual".to_owned());
    }
    match (headless, script, smoke) {
        (true, Some(script), None) if project.is_none() => Ok(Cli::Headless(HeadlessArgs {
            script,
            record,
            config_dir,
            keep_going,
            visual,
            frames,
            no_frames,
            geometry,
        })),
        (true, None, _) => Err("--headless needs --script <file|->".to_owned()),
        (true, _, _) => Err("--headless takes a script and no project or --smoke".to_owned()),
        (false, Some(_), _) => Err("--script needs --headless".to_owned()),
        (false, None, _) if record.is_some() => Err("--record needs --headless".to_owned()),
        (false, None, _) if config_dir.is_some() => Err("--config-dir needs --headless".to_owned()),
        (false, None, _) if keep_going => Err("--keep-going needs --headless".to_owned()),
        (false, None, Some(dir)) if project.is_none() => Ok(Cli::Smoke(dir)),
        (false, None, Some(_)) => Err("--smoke takes one directory".to_owned()),
        (false, None, None) => Ok(Cli::Gui(project, no_gpu)),
    }
}

/// The file dialogs of the operating system through `rfd`.
struct RfdPlatform {
    /// The dialog every prompt starts from: on Windows it carries the window as its owner.
    base: rfd::FileDialog,
}

/// An environment variable as a path, `None` when unset or empty.
// Program boundary: the portal's session bus (`DBUS_SESSION_BUS_ADDRESS`, `XDG_RUNTIME_DIR`) and the directories of
// the application's resources (`EW_SAMPLE_DIR`, `EW_CONTENT_DIR`, `EW_DOCS_DIR`) are read once at start-up.
#[allow(
    clippy::disallowed_methods,
    reason = "the environment is read once at start-up"
)]
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

/// How a dialog of `rfd` is shown: what it asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DialogKind {
    Folder,
    File,
    Save,
}

impl RfdPlatform {
    /// The dialogs of the window `cc` was made for. On Windows the window is their owner, so it takes no input while
    /// a dialog is open, as when the dialog ran on the window's thread; elsewhere they have no parent, as
    /// before.
    fn new(cc: &eframe::CreationContext<'_>) -> RfdPlatform {
        let base = rfd::FileDialog::new();
        let base = if cfg!(target_os = "windows") {
            base.set_parent(cc)
        } else {
            base
        };
        RfdPlatform { base }
    }

    /// Whether the system has file dialogs: on Linux `rfd` needs the xdg desktop portal's session bus.
    fn available() -> bool {
        !cfg!(target_os = "linux")
            || session_bus_present(env_path("DBUS_SESSION_BUS_ADDRESS"), env_path("XDG_RUNTIME_DIR"))
    }

    /// The dialog of `prompt`, built on the window's thread (its texts are the window's).
    fn dialog(&self, prompt: Prompt) -> (rfd::FileDialog, DialogKind) {
        let base = self.base.clone();
        match prompt {
            Prompt::OpenFolder => (
                base.set_title(strings::text(key!("prompt-open-title"))),
                DialogKind::Folder,
            ),
            Prompt::NewFolder => (
                base.set_title(strings::text(key!("prompt-save-as-title"))),
                DialogKind::Folder,
            ),
            Prompt::OpenQet => (
                base.set_title(strings::text(key!("prompt-open-qet-title")))
                    .add_filter("QElectroTech", &["qet"]),
                DialogKind::File,
            ),
            Prompt::OpenPartsTable => (
                base.set_title(strings::text(key!("prompt-open-parts-title")))
                    .add_filter("CSV", &["csv"]),
                DialogKind::File,
            ),
            Prompt::OpenImage => (
                base.set_title(strings::text(key!("prompt-open-image-title")))
                    .add_filter("Images", &["svg", "png", "jpg", "jpeg"]),
                DialogKind::File,
            ),
            Prompt::SavePdf => (
                base.set_title(strings::text(key!("prompt-export-title")))
                    .set_file_name(strings::text(key!("prompt-pdf-name")))
                    .add_filter("PDF", &["pdf"]),
                DialogKind::Save,
            ),
            // The keymap editor's Import… and Export….
            Prompt::OpenKeymap => (
                base.set_title(strings::text(key!("prompt-open-keymap-title")))
                    .add_filter("TOML", &["toml"]),
                DialogKind::File,
            ),
            Prompt::SaveKeymap => (
                base.set_title(strings::text(key!("prompt-save-keymap-title")))
                    .set_file_name(strings::text(key!("prompt-keymap-name")))
                    .add_filter("TOML", &["toml"]),
                DialogKind::Save,
            ),
            // A navigator table's Export table… and Import table….
            Prompt::SaveTable => (
                base.set_title(strings::text(key!("prompt-export-table-title")))
                    .set_file_name(strings::text(key!("prompt-table-name")))
                    .add_filter("XLSX", &["xlsx"])
                    .add_filter("CSV", &["csv"]),
                DialogKind::Save,
            ),
            Prompt::OpenTable => (
                base.set_title(strings::text(key!("prompt-open-table-title")))
                    .add_filter("XLSX", &["xlsx"])
                    .add_filter("CSV", &["csv"]),
                DialogKind::File,
            ),
            // File ▸ Export .ewz… and Open .ewz….
            Prompt::SaveEwz => (
                base.set_title(strings::text(key!("prompt-export-ewz-title")))
                    .set_file_name(strings::text(key!("prompt-ewz-name")))
                    .add_filter("EWZ", &["ewz"]),
                DialogKind::Save,
            ),
            Prompt::OpenEwz => (
                base.set_title(strings::text(key!("prompt-open-ewz-title")))
                    .add_filter("EWZ", &["ewz"]),
                DialogKind::File,
            ),
            // File ▸ Import…: a QElectroTech project or a project archive, both offered at once.
            Prompt::OpenImport => (
                base.set_title(strings::text(key!("prompt-import-title")))
                    .add_filter(strings::text(key!("prompt-import-filter")), &["qet", "ewz"]),
                DialogKind::File,
            ),
            // The symbol editor's file and library folder.
            Prompt::OpenSymbol => (
                base.set_title(strings::text(key!("prompt-open-symbol-title")))
                    .add_filter(
                        strings::text(key!("prompt-symbol-filter")),
                        &ew_ui::ew_viewmodel::shell::symbol_editor::FILE_EXTENSIONS,
                    ),
                DialogKind::File,
            ),
            Prompt::SaveSymbol => (
                base.set_title(strings::text(key!("prompt-save-symbol-title")))
                    .set_file_name(strings::text(key!("prompt-symbol-name")))
                    .add_filter("JSON", &["json"]),
                DialogKind::Save,
            ),
            Prompt::LibraryFolder => (
                base.set_title(strings::text(key!("prompt-library-folder-title"))),
                DialogKind::Folder,
            ),
        }
    }

    /// Shows `dialog` and waits for the user, on the calling thread.
    fn run(dialog: rfd::FileDialog, kind: DialogKind) -> Pick {
        let picked = match kind {
            DialogKind::Folder => dialog.pick_folder(),
            DialogKind::File => dialog.pick_file(),
            DialogKind::Save => dialog.save_file(),
        };
        picked.map_or(Pick::Cancelled, Pick::Path)
    }
}

impl Platform for RfdPlatform {
    fn pick(&mut self, prompt: Prompt) -> Pick {
        if !Self::available() {
            return Pick::Unavailable;
        }
        let (dialog, kind) = self.dialog(prompt);
        Self::run(dialog, kind)
    }

    /// Every dialog runs on a thread of its own, so the window's event loop keeps running and the answer is painted
    /// at once.
    fn start_pick(&mut self, prompt: Prompt, ctx: &egui::Context) -> PickReply {
        if !Self::available() {
            return PickReply::Now(Pick::Unavailable);
        }
        let (dialog, kind) = self.dialog(prompt);
        match spawn_pick(ctx, move || Self::run(dialog, kind)) {
            Ok(rx) => PickReply::Later(rx),
            // Without a thread for it the dialog runs on the window's thread.
            Err(_) => PickReply::Now(self.pick(prompt)),
        }
    }

    fn open_url(&mut self, url: &str) -> bool {
        open_in_browser(url)
    }

    /// An exported file by its path, through the opener the print window's Open in PDF viewer uses
    /// (`ew_print::Program::opener`: `explorer.exe` on Windows, `xdg-open` elsewhere), no shell.
    fn open_file(&mut self, file: &Path) -> bool {
        let opener = ew_print::Program::opener();
        std::process::Command::new(&opener.path)
            .args(&opener.leading)
            .arg(file)
            .spawn()
            .is_ok()
    }

    /// The print set goes through `ew-print`'s chain on a thread of its own: Print… through the
    /// system's chain (on Linux the portal, then `lp`, then the viewer; on Windows the Windows print dialog, then the
    /// viewer), Open in PDF viewer through the viewer alone.
    /// The window keeps painting; the end comes back as a toast.
    fn print(&mut self, request: PrintRequest, ctx: &egui::Context) -> PrintReply {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("ew-print".to_owned())
            .spawn(move || {
                let end = print_end(run_print(&request));
                // The window may have closed meanwhile; then nobody waits for the answer.
                let _ = tx.send(end);
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => PrintReply::Later(rx),
            Err(e) => PrintReply::Now(PrintEnd::Failed(e.to_string())),
        }
    }
}

/// Runs `request` through the chain its way names.
fn run_print(request: &PrintRequest) -> Result<ew_print::Printed, Vec<(&'static str, ew_print::PrintError)>> {
    let temp = std::env::temp_dir();
    let chain = match request.via {
        PrintVia::Printer => ew_print::Chain::system(temp, &request.title),
        PrintVia::Viewer => ew_print::Chain::viewer(temp, &request.title),
    };
    chain.print(&ew_print::PrintJob {
        title: request.title.clone(),
        pdf: request.set.pdf.clone(),
        pages: request.set.pages.clone(),
        fonts: ew_ui::ew_viewmodel::ew_api::output::font_bytes().clone(),
        printer: None,
        monochrome: request.monochrome,
        // The Jobs pane's Cancel of the print sets it.
        cancel: ew_print::PrintCancel::of(std::sync::Arc::clone(&request.cancel)),
    })
}

/// What the window says of a chain's result: a print or an opened viewer, with D11's notice when an earlier way
/// failed; nothing when the person cancelled the system's dialog; the last cause when every way failed.
fn print_end(result: Result<ew_print::Printed, Vec<(&'static str, ew_print::PrintError)>>) -> PrintEnd {
    match result {
        Ok(printed) => match (&printed.outcome, printed.fallback.is_some()) {
            (ew_print::PrintOutcome::Opened { .. }, false) => PrintEnd::Opened,
            (ew_print::PrintOutcome::Opened { .. }, true) => PrintEnd::FellBackToViewer,
            (ew_print::PrintOutcome::Spooled { .. }, false) => PrintEnd::Printed,
            (ew_print::PrintOutcome::Spooled { .. }, true) => PrintEnd::FellBackToPrinter,
        },
        Err(failed) => match failed.last() {
            Some((_, ew_print::PrintError::Cancelled)) => PrintEnd::Cancelled,
            Some((_, e)) => PrintEnd::Failed(e.to_string()),
            None => PrintEnd::Unavailable,
        },
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
        _ => ("xdg-open", vec![url.to_owned()]),
    }
}

/// The first of `candidates` that is a directory.
fn first_dir(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| p.is_dir())
}

/// Finds the application's read-only resources. Each has an environment
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
    // `PRIVACY.md` beside the executable, else the repository's in a development
    // build.
    let privacy = exe_dir
        .as_ref()
        .map(|d| d.join("PRIVACY.md"))
        .filter(|p| p.is_file())
        .or_else(|| tree("PRIVACY.md").filter(|p| p.is_file()))
        .map(|p| p.canonicalize().unwrap_or(p));
    Resources {
        sample_dir: find_dir("EW_SAMPLE_DIR", "samples/J-E-2416.ew", "golden/J-E-2416.ew"),
        content_dir: find_dir("EW_CONTENT_DIR", "content", "content"),
        docs_dir: find_dir("EW_DOCS_DIR", "docs", "target/book"),
        notices_file: notices.filter(|p| p.is_file()),
        privacy_file: privacy,
    }
}

/// The path of this executable, `None` when the system does not say.
fn current_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

/// The library tiers of the shell's sessions: the system libraries of the installation's content
/// (`content/libraries`), the user's data and cache directories. An isolated run (a script) reads libraries only when
/// `EW_DATA_DIR` names its data directory, never the user's own, and keeps its search index in `EW_CACHE_DIR` when
/// that is set, else in `<EW_DATA_DIR>/cache`, never in the user's own cache.
fn library_tiers(
    isolated: bool,
    content: Option<&Path>,
) -> Option<ew_ui::ew_viewmodel::ew_api::library::LibraryTiers> {
    library_tiers_from(
        isolated,
        content,
        env_path("EW_DATA_DIR"),
        env_path("EW_CACHE_DIR"),
    )
}

/// [`library_tiers`] with `EW_DATA_DIR` and `EW_CACHE_DIR` given (`None` when unset).
fn library_tiers_from(
    isolated: bool,
    content: Option<&Path>,
    data_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
) -> Option<ew_ui::ew_viewmodel::ew_api::library::LibraryTiers> {
    use ew_ui::ew_viewmodel::ew_api::library::LibraryTiers;
    let system: Vec<PathBuf> = content
        .map(|c| c.join("libraries"))
        .filter(|p| p.is_dir())
        .into_iter()
        .collect();
    if isolated {
        let data = data_dir?;
        let cache = cache_dir.unwrap_or_else(|| data.join("cache"));
        return Some(LibraryTiers::under(&data, &cache, system, Vec::new()));
    }
    LibraryTiers::for_user(system, Vec::new())
}

/// The configuration of the environment with the resources and the library tiers of this installation.
fn shell_config(isolated: bool) -> ShellConfig {
    let config = if isolated {
        ShellConfig::isolated_from_env(VERSION)
    } else {
        ShellConfig::from_env(VERSION)
    };
    let resources = locate_resources(current_exe().as_deref());
    let libraries = library_tiers(isolated, resources.content_dir.as_deref());
    config.with_resources(resources).with_libraries(libraries)
}

/// The crash directory of a run with `config`: `crashes/` in its state directory, which is the store's
/// default when the configuration names none.
fn crash_dir(config: &ShellConfig) -> Option<PathBuf> {
    config
        .state_dir
        .clone()
        .or_else(|| ew_ui::ew_viewmodel::ew_api::StoreOptions::new(VERSION).state_dir)
        .map(|state| state.join(ew_ui::ew_viewmodel::shell::startup::CRASH_DIR))
}

/// The eframe application: the shell frame of `ew-ui`, and whether the window has been placed on a monitor.
struct ShellApp(App, bool);

impl eframe::App for ShellApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if !self.1 {
            let saved = self.0.shell().settings().window;
            place_on_screen(ui.ctx(), frame, saved);
            self.1 = true;
        }
        self.0.ui(ui);
    }

    fn on_exit(&mut self) {
        // Shutdown at exit: the project lock is released; the journal is kept while the project is dirty
        // (crash-recovery semantics, doc 05 section 8) and removed only after a clean close.
        self.0.shell_mut().shutdown();
    }
}

/// On the first frame the window goes to the saved outer position of `saved` (in the
/// platform's desktop space: physical pixels), one whose position is on no monitor (its
/// monitor gone) moves onto the primary monitor, and a window saved maximised is maximised after the move (the builder
/// never maximises it). This reads winit's physical values; the conversion, the clamp and the order of the steps are
/// the pure `ew_ui::window_memory::first_frame`. A minimised or full-screen window and a frame without a winit window
/// are left alone; a window without a known position (Wayland) is only maximised when it was.
fn place_on_screen(ctx: &egui::Context, frame: &eframe::Frame, saved: Option<WindowGeometry>) {
    let Some(window) = frame.winit_window() else {
        return;
    };
    if window.is_minimized() == Some(true) || window.fullscreen().is_some() {
        return;
    }
    let outer = window.outer_position().ok().map(|p| (p.x as f32, p.y as f32));
    let primary = window.primary_monitor();
    let displays: Vec<Display> = window
        .available_monitors()
        .map(|m| {
            let (at, size) = (m.position(), m.size());
            Display {
                area_px: MonitorArea {
                    x: at.x as f32,
                    y: at.y as f32,
                    width: size.width as f32,
                    height: size.height as f32,
                },
                primary: primary.as_ref() == Some(&m),
            }
        })
        .collect();
    let inner = window.inner_size();
    let steps = ew_ui::window_memory::first_frame(
        saved,
        outer,
        (inner.width as f32, inner.height as f32),
        &displays,
        window.scale_factor() as f32,
        ctx.zoom_factor(),
    );
    // egui-winit applies a frame's commands in the order they were sent: the move, then the maximise.
    for step in steps {
        ctx.send_viewport_cmd(match step {
            FirstFrameStep::MoveTo(to) => egui::ViewportCommand::OuterPosition(to),
            FirstFrameStep::Maximize => egui::ViewportCommand::Maximized(true),
        });
    }
}

/// The exit code 3 of a host without any graphics adapter (section 7.2).
const EXIT_NO_ADAPTER: i32 = 3;

/// Whether eframe failed because no graphics adapter exists, as opposed to any other failure of the window. Any
/// wgpu error counts when `adapters` (a fresh enumeration, called only then) finds none: without any working
/// backend egui-wgpu fails already at `create_surface` (wgpu-core's "failed to create surface for any backend"),
/// before it asks for an adapter. An adapter request or the `--no-gpu` selector failing is the missing adapter
/// itself.
fn is_no_adapter(e: &eframe::Error, adapters: impl FnOnce() -> usize) -> bool {
    match e {
        eframe::Error::Wgpu(
            eframe::egui_wgpu::WgpuError::RequestAdapterError(_)
            | eframe::egui_wgpu::WgpuError::CustomNativeAdapterSelectionError(_),
        ) => true,
        eframe::Error::Wgpu(_) => adapters() == 0,
        _ => false,
    }
}

/// The one message of exit code 3 on the operating system `os` (`std::env::consts::OS`).
fn no_adapter_message(os: &str, detail: &str) -> String {
    let mut m = format!(
        "error: no graphics adapter was found, so the window cannot open ({detail}).\n\
         The headless mode needs none: elektrawatt --headless --script <file>; the command-line tool `ew` opens, \
         checks and exports projects without a window."
    );
    if os == "linux" {
        m.push_str(
            "\nOn Linux without a GPU, Mesa's software drivers provide an adapter: llvmpipe (OpenGL) or lavapipe \
             (Vulkan), in the distribution's Mesa packages.",
        );
    }
    m
}

/// The exit code and the message of a window that could not run; `adapters` counts the system's adapters.
fn gui_failure(e: &eframe::Error, os: &str, adapters: impl FnOnce() -> usize) -> (i32, String) {
    if is_no_adapter(e, adapters) {
        (EXIT_NO_ADAPTER, no_adapter_message(os, &e.to_string()))
    } else {
        (1, format!("error: the window could not be opened: {e}"))
    }
}

fn run_gui(initial: Option<PathBuf>, no_gpu: bool) -> i32 {
    let config = shell_config(false);
    crash::set_dir(crash_dir(&config));
    let mut shell = ShellModel::new(Mode::Gui, config);
    // The start-up offer of an earlier session's unsaved work and crash log.
    shell.start_up();
    // The first start (and only that) shows the first-run flow, unless the user named a project to open.
    if initial.is_none() {
        shell.start_first_run_if_needed();
    }
    let flags = Flags { no_gpu };
    // The last size, position and maximised state, restored; the window's icon is the menu button's mark, the
    // application icon (FT-1710: without one eframe showed egui's).
    let mut builder = egui::ViewportBuilder::default()
        .with_title(strings::text(key!("window-title-start")))
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 400.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(APP_ICON_PNG) {
        builder = builder.with_icon(icon);
    }
    let viewport = ew_ui::window_memory::restore(builder, shell.settings().window);
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: wgpu_configuration(flags),
        ..eframe::NativeOptions::default()
    };
    let result = eframe::run_native(
        strings::text(key!("window-title-start")),
        options,
        Box::new(move |cc| {
            setup(&cc.egui_ctx);
            // The file dialogs need the window (their owner on Windows), so the frame is made here.
            let mut app = App::new(shell, Box::new(RfdPlatform::new(cc)), initial);
            // The canvas draws on eframe's device (one wgpu device, ADR-0014); without a render state it is the CPU
            // canvas.
            app.attach_gpu(cc.wgpu_render_state.clone(), flags);
            Ok(Box::new(ShellApp(app, false)))
        }),
    );
    match result {
        Ok(()) => 0,
        Err(e) => {
            let (code, message) = gui_failure(&e, std::env::consts::OS, adapter_count);
            attach_console();
            eprintln!("{message}");
            code
        }
    }
}

/// On Windows the program has no console of its own (`windows_subsystem = "windows"`: no console window opens
/// before the application, FT-1710), so a mode that writes to a terminal attaches to the console of the process that
/// started it, unless its output goes to a pipe or a file already. Elsewhere nothing to do.
fn attach_console() {
    #[cfg(windows)]
    ew_print::attach_parent_console();
}

fn main() {
    // The crash log's hook first, for every mode; a panic that reaches this frame ended the session.
    crash::install(VERSION);
    let cli = parse_args(std::env::args_os().skip(1));
    if !matches!(cli, Ok(Cli::Gui(..))) {
        attach_console();
    }
    let code = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(cli))) {
        Ok(code) => code,
        Err(_) => {
            crash::ended();
            101
        }
    };
    std::process::exit(code);
}

/// Runs the mode the command line asked for and returns the process exit code.
fn run(cli: Result<Cli, String>) -> i32 {
    match cli {
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
        Ok(Cli::Gui(initial, no_gpu)) => run_gui(initial, no_gpu),
        Ok(Cli::Smoke(dir)) => {
            let config = shell_config(false);
            crash::set_dir(crash_dir(&config));
            let code = headless::smoke(config, &dir, &mut std::io::stdout(), &mut std::io::stderr());
            let _ = std::io::stdout().flush();
            code
        }
        Ok(Cli::Headless(args)) => {
            let config = shell_config(true);
            // `--config-dir` puts the state directory in it, as `headless::run` does.
            let state = match &args.config_dir {
                Some(dir) => ShellConfig::in_dir(dir, VERSION),
                None => config.clone(),
            };
            crash::set_dir(crash_dir(&state));
            let stdin = std::io::stdin();
            // A debug build ends a headless run with a real panic on the main thread when asked, after the
            // script's last step and before the shutdown, the test of the crash log; a release build
            // compiles none of it.
            let forced_panic = || {
                #[cfg(debug_assertions)]
                if env_path("EW_DEBUG_PANIC").is_some_and(|v| v.as_os_str() == "1") {
                    panic!("forced panic (EW_DEBUG_PANIC)");
                }
            };
            headless::run_ending(
                config,
                &args,
                &mut stdin.lock(),
                &mut std::io::stdout(),
                &mut std::io::stderr(),
                &forced_panic,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One entry, whole, with the frames trimmed and the empty line that ends it.
    #[test]
    fn a_crash_log_entry_has_its_seven_lines() {
        assert_eq!(
            crash::entry(
                "0.1.0",
                "2026-11-04T10:15:07Z",
                "main",
                "bin/elektrawatt/src/main.rs:1:2",
                "boom",
                "   0: main\n\n"
            ),
            format!(
                "ElektraWatt 0.1.0 crash log\nOS: {} {}\nTime: 2026-11-04T10:15:07Z\nThread: main\n\
                 Location: bin/elektrawatt/src/main.rs:1:2\nMessage: boom\nBacktrace:\n   0: main\n\n",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        );
    }

    /// PV-5: with the hook installed, a panic caught on its way (as a rule run's EW-CORE-083 path catches one) appends
    /// an entry without the end line, the run goes on, and the next start offers no such log. Nextest runs each test
    /// in a process of its own, so the hook stays in this one.
    #[test]
    fn a_caught_panic_is_logged_without_the_end_line_and_never_offered() {
        let dir = ew_testdir::fresh_dir("ew-crash-caught");
        let config = ShellConfig::in_dir(&dir, VERSION);
        crash::install(VERSION);
        crash::set_dir(crash_dir(&config));
        let caught = std::panic::catch_unwind(|| panic!("a rule run panicked"));
        assert!(caught.is_err());
        let crashes = dir.join("state").join("crashes");
        let logs: Vec<PathBuf> = std::fs::read_dir(&crashes)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(logs.len(), 1);
        let text = std::fs::read_to_string(&logs[0]).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], format!("ElektraWatt {VERSION} crash log"));
        assert_eq!(lines[5], "Message: a rule run panicked");
        assert!(text.ends_with("\n\n"), "{text}");
        assert!(!text.contains(ew_ui::ew_viewmodel::shell::startup::ENDED_LINE));
        let mut shell = ShellModel::new(Mode::Headless, config);
        shell.start_up();
        assert_eq!(shell.startup_offer(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn parse(args: &[&str]) -> Result<Cli, String> {
        parse_args(args.iter().map(OsString::from))
    }

    /// An isolated run with only `EW_DATA_DIR` keeps its search index under that directory, never in the
    /// user's default cache; `EW_CACHE_DIR` still wins; without `EW_DATA_DIR` it reads no libraries.
    #[test]
    fn an_isolated_run_keeps_its_cache_under_its_data_dir() {
        let data = PathBuf::from("isolated-data");
        let tiers = library_tiers_from(true, None, Some(data.clone()), None).expect("tiers");
        assert_eq!(tiers.cache, data.join("cache"));
        assert_eq!(tiers.user, data.join("libraries"));
        if let Some(user_cache) = ew_ui::ew_viewmodel::ew_api::library::default_cache_dir() {
            assert!(!tiers.cache.starts_with(&user_cache), "{}", tiers.cache.display());
        }
        let cache = PathBuf::from("isolated-cache");
        let tiers = library_tiers_from(true, None, Some(data), Some(cache.clone())).expect("tiers");
        assert_eq!(tiers.cache, cache);
        assert!(library_tiers_from(true, None, None, Some(PathBuf::from("c"))).is_none());
    }

    #[test]
    fn no_arguments_is_the_window_and_no_gpu_is_a_flag_of_the_window_only() {
        assert_eq!(parse(&[]), Ok(Cli::Gui(None, false)));
        assert_eq!(parse(&["p.ew"]), Ok(Cli::Gui(Some(PathBuf::from("p.ew")), false)));
        assert_eq!(parse(&["--no-gpu"]), Ok(Cli::Gui(None, true)));
        assert_eq!(
            parse(&["--no-gpu", "p.ew"]),
            Ok(Cli::Gui(Some(PathBuf::from("p.ew")), true))
        );
        assert!(parse(&["--no-gpu", "--headless", "--script", "s"]).is_err());
        assert!(parse(&["--no-gpu", "--smoke", "p.ew"]).is_err());
    }

    #[test]
    fn b6_a_missing_adapter_exits_with_code_3_and_names_the_ways_without_one() {
        use eframe::egui_wgpu::WgpuError;
        // The --no-gpu selector found no adapter at all (the injected failure of an empty adapter list).
        let selector = eframe::Error::Wgpu(WgpuError::CustomNativeAdapterSelectionError(
            "no graphics adapter was found".into(),
        ));
        let none = || 0;
        let one = || 1;
        let (code, message) = gui_failure(&selector, "windows", one);
        assert_eq!(code, 3);
        assert!(message.contains("elektrawatt --headless"), "{message}");
        assert!(message.contains("`ew`"), "{message}");
        assert!(
            !message.contains("Mesa"),
            "Mesa is named on Linux only: {message}"
        );
        assert_eq!(message.lines().count(), 2);
        let (code, linux) = gui_failure(&selector, "linux", one);
        assert_eq!(code, 3);
        assert!(
            linux.contains("Mesa") && linux.contains("llvmpipe") && linux.contains("lavapipe"),
            "{linux}"
        );
        // The adapter request itself failing.
        let request = eframe::Error::Wgpu(WgpuError::RequestAdapterError(
            eframe::wgpu::RequestAdapterError::NotFound {
                active_backends: eframe::wgpu::Backends::empty(),
                requested_backends: eframe::wgpu::Backends::all(),
                supported_backends: eframe::wgpu::Backends::all(),
                no_fallback_backends: eframe::wgpu::Backends::empty(),
                no_adapter_backends: eframe::wgpu::Backends::all(),
                incompatible_surface_backends: eframe::wgpu::Backends::empty(),
            },
        ));
        assert_eq!(gui_failure(&request, "linux", one).0, 3);
        // A wgpu error before any adapter was asked for (no backend could make a surface: Linux without the Vulkan
        // loader and EGL) is the missing adapter when a fresh enumeration finds none, and exit 1 otherwise.
        let surface = eframe::Error::Wgpu(WgpuError::NoSurfaceFormatsAvailable);
        assert_eq!(gui_failure(&surface, "linux", none).0, 3);
        let (code, message) = gui_failure(&surface, "linux", one);
        assert_eq!(code, 1);
        assert!(message.starts_with("error: the window could not be opened"));
        // A failure that is no wgpu error never counts the adapters.
        let not_wgpu = eframe::Error::AppCreation("app".into());
        assert_eq!(
            gui_failure(&not_wgpu, "linux", || unreachable!(
                "no enumeration for other errors"
            ))
            .0,
            1
        );
    }

    /// The arguments of a headless run of `script` with every other option at its default.
    fn headless(script: ScriptSource) -> HeadlessArgs {
        HeadlessArgs {
            script,
            record: None,
            config_dir: None,
            keep_going: false,
            visual: false,
            frames: None,
            no_frames: false,
            geometry: Geometry::DEFAULT,
        }
    }

    #[test]
    fn the_headless_mode_needs_a_script() {
        assert_eq!(
            parse(&["--headless", "--script", "s.jsonl"]),
            Ok(Cli::Headless(headless(ScriptSource::File(PathBuf::from(
                "s.jsonl"
            )))))
        );
        assert_eq!(
            parse(&["--headless", "--script", "-", "--record", "r.ewcmd"]),
            Ok(Cli::Headless(HeadlessArgs {
                record: Some(PathBuf::from("r.ewcmd")),
                ..headless(ScriptSource::Stdin)
            }))
        );
        assert_eq!(
            parse(&["--headless", "--script", "-", "--keep-going"]),
            Ok(Cli::Headless(HeadlessArgs {
                keep_going: true,
                ..headless(ScriptSource::Stdin)
            }))
        );
        assert_eq!(
            parse(&["--headless", "--script", "s", "--config-dir", "c"]),
            Ok(Cli::Headless(HeadlessArgs {
                config_dir: Some(PathBuf::from("c")),
                ..headless(ScriptSource::File(PathBuf::from("s")))
            }))
        );
        assert!(parse(&["--config-dir", "c"]).is_err());
        // The flag belongs to the headless mode.
        assert!(parse(&["--keep-going"]).is_err());
        assert!(parse(&["--smoke", "p.ew", "--keep-going"]).is_err());
        assert!(USAGE.contains("--keep-going"));
        assert!(parse(&["--headless"]).is_err());
        assert!(parse(&["--script", "s"]).is_err());
        assert!(parse(&["--record", "r"]).is_err());
        assert!(parse(&["--headless", "--script"]).is_err());
        assert!(parse(&["--headless", "--script", "s", "p.ew"]).is_err());
    }

    /// `--visual` only with `--headless` (else a usage error, exit 2); `--frames` and
    /// `--no-frames` only with `--visual`, and not together.
    #[test]
    fn the_visual_flags_belong_to_the_headless_mode() {
        assert_eq!(
            parse(&[
                "--headless",
                "--visual",
                "--script",
                "-",
                "--frames",
                "f",
                "--keep-going"
            ]),
            Ok(Cli::Headless(HeadlessArgs {
                keep_going: true,
                visual: true,
                frames: Some(PathBuf::from("f")),
                ..headless(ScriptSource::Stdin)
            }))
        );
        assert!(matches!(
            parse(&["--headless", "--visual", "--script", "s", "--no-frames"]),
            Ok(Cli::Headless(HeadlessArgs {
                visual: true,
                no_frames: true,
                frames: None,
                ..
            }))
        ));
        assert_eq!(parse(&["--visual"]), Err("--visual needs --headless".to_owned()));
        assert_eq!(
            parse(&["--visual", "p.ew"]),
            Err("--visual needs --headless".to_owned())
        );
        assert!(parse(&["--headless", "--script", "s", "--frames", "f"]).is_err());
        assert!(parse(&["--headless", "--script", "s", "--no-frames"]).is_err());
        assert!(
            parse(&[
                "--headless",
                "--visual",
                "--script",
                "s",
                "--frames",
                "f",
                "--no-frames"
            ])
            .is_err()
        );
        assert!(USAGE.contains("--visual"));
    }

    /// `--window <W>x<H>` and `--ppp <n>` choose a visual run's window size and scale; each error names
    /// what is wrong.
    #[test]
    fn the_window_and_ppp_flags_choose_the_geometry() {
        let v = ["--headless", "--visual", "--script", "s"];
        let with = |more: &[&str]| parse(&[&v[..], more].concat());
        assert!(matches!(
            with(&["--window", "1920x1080", "--ppp", "1"]),
            Ok(Cli::Headless(HeadlessArgs {
                geometry: Geometry {
                    width: 1920,
                    height: 1080,
                    ppp_hundredths: 100
                },
                ..
            }))
        ));
        assert!(matches!(
            with(&["--ppp", "1.25"]),
            Ok(Cli::Headless(HeadlessArgs {
                geometry: Geometry {
                    width: 1280,
                    height: 800,
                    ppp_hundredths: 125
                },
                ..
            }))
        ));
        assert!(matches!(
            with(&[]),
            Ok(Cli::Headless(HeadlessArgs {
                geometry: Geometry::DEFAULT,
                ..
            }))
        ));
        assert_eq!(
            with(&["--window", "1920"]),
            Err("--window needs <width>x<height> in points, e.g. 1920x1080".to_owned())
        );
        assert_eq!(
            with(&["--window", "800x600"]),
            Err("--window must be from 1024x640 to 3840x2160 points".to_owned())
        );
        assert_eq!(
            with(&["--ppp", "0.5"]),
            Err("--ppp needs a number from 1 to 3".to_owned())
        );
        assert_eq!(
            with(&["--ppp", "1.234"]),
            Err("--ppp needs a number from 1 to 3".to_owned())
        );
        assert_eq!(
            with(&["--window", "3840x2160", "--ppp", "2"]),
            Err("--window and --ppp make a frame larger than 3840 × 2400 pixels".to_owned())
        );
        assert_eq!(
            parse(&["--headless", "--script", "s", "--window", "1920x1080"]),
            Err("--window and --ppp need --visual".to_owned())
        );
        assert_eq!(
            parse(&["--headless", "--script", "s", "--ppp", "2"]),
            Err("--window and --ppp need --visual".to_owned())
        );
        assert!(USAGE.contains("--window <W>x<H>") && USAGE.contains("--ppp <n>"));
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
        assert_eq!(opener("linux", url), ("xdg-open", vec![url.to_owned()]));
        for os in ["windows", "linux", "freebsd"] {
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
        let dir = ew_testdir::fresh_dir("ew-bus-test");
        assert!(!session_bus_present(None, Some(dir.clone())));
        std::fs::write(dir.join("bus"), b"").unwrap();
        assert!(session_bus_present(None, Some(dir.clone())));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The chain's result reads as the window's end of a print: the notice when an earlier
    /// way failed, nothing on a cancel, the last cause when every way failed.
    #[test]
    fn the_chain_result_reads_as_the_print_end() {
        let spooled = ew_print::PrintOutcome::Spooled {
            printer: None,
            message: String::new(),
        };
        let opened = ew_print::PrintOutcome::Opened {
            file: PathBuf::from("set.pdf"),
        };
        let printed = |adapter, outcome: &ew_print::PrintOutcome, fallback: bool| ew_print::Printed {
            adapter,
            outcome: outcome.clone(),
            fallback: fallback.then(|| ew_print::Fallback {
                failed: vec![("portal", ew_print::PrintError::LpMissing)],
                used: adapter,
            }),
        };
        assert_eq!(
            print_end(Ok(printed("portal", &spooled, false))),
            PrintEnd::Printed
        );
        assert_eq!(
            print_end(Ok(printed("lp", &spooled, true))),
            PrintEnd::FellBackToPrinter
        );
        assert_eq!(print_end(Ok(printed("viewer", &opened, false))), PrintEnd::Opened);
        assert_eq!(
            print_end(Ok(printed("viewer", &opened, true))),
            PrintEnd::FellBackToViewer
        );
        assert_eq!(
            print_end(Err(vec![("portal", ew_print::PrintError::Cancelled)])),
            PrintEnd::Cancelled
        );
        assert_eq!(
            print_end(Err(vec![
                ("portal", ew_print::PrintError::Cancelled),
                ("lp", ew_print::PrintError::LpMissing),
            ])),
            PrintEnd::Failed(
                "printing needs the CUPS command `lp`; install cups-client or print from the window"
                    .to_owned()
            )
        );
        assert_eq!(print_end(Err(Vec::new())), PrintEnd::Unavailable);
    }

    /// The fixture of IN-2 under the system's temporary directory, removed on drop: the flat tree
    /// of D3 beside an executable at `x/flat`, the `share/elektrawatt` tree for an executable at `inst/bin`, an
    /// executable directory with nothing near it at `x/empty`, and the directories the environment names at `env/`.
    /// Each executable sits two levels below the root, so the source-tree fallback (`<exe dir>/../..`) lands on the
    /// root, which holds none of the resources.
    struct Layouts(PathBuf);

    impl Layouts {
        fn new() -> Layouts {
            let root = std::env::temp_dir().join(format!("ew-locate-resources-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            for base in ["x/flat", "inst/share/elektrawatt"] {
                for dir in ["samples/J-E-2416.ew", "content", "docs"] {
                    std::fs::create_dir_all(root.join(base).join(dir)).unwrap();
                }
                std::fs::write(root.join(base).join("THIRD-PARTY-NOTICES.txt"), "notices").unwrap();
                std::fs::write(root.join(base).join("PRIVACY.md"), "privacy").unwrap();
            }
            for dir in ["inst/bin", "x/empty", "env/sample", "env/content", "env/docs"] {
                std::fs::create_dir_all(root.join(dir)).unwrap();
            }
            Layouts(root)
        }
    }

    impl Drop for Layouts {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// IN-2's child: run by [`locate_resources_finds_the_installed_layouts`] with `-- <case> <root>` and the
    /// environment of the case set on the parent's side; run by the harness (no `--`), it does nothing.
    #[test]
    fn locate_resources_child() {
        let args: Vec<String> = std::env::args().collect();
        let Some(at) = args.iter().position(|a| a == "--") else {
            return;
        };
        let root = PathBuf::from(&args[at + 2]);
        let found = |dir: &str| locate_resources(Some(&root.join(dir).join("elektrawatt")));
        let dir = |rel: &str| Some(root.join(rel).canonicalize().unwrap());
        let notices = Some(root.join("x/flat/THIRD-PARTY-NOTICES.txt"));
        let privacy = Some(root.join("x/flat/PRIVACY.md").canonicalize().unwrap());
        match args[at + 1].as_str() {
            "layouts" => {
                // The flat tree: all four beside the executable.
                assert_eq!(
                    found("x/flat"),
                    Resources {
                        sample_dir: dir("x/flat/samples/J-E-2416.ew"),
                        content_dir: dir("x/flat/content"),
                        docs_dir: dir("x/flat/docs"),
                        notices_file: notices,
                        privacy_file: privacy,
                    }
                );
                // `share/elektrawatt` one level up: the three directories; the notices are looked for beside the
                // executable only, so the ones in that tree are not found.
                assert_eq!(
                    found("inst/bin"),
                    Resources {
                        sample_dir: dir("inst/share/elektrawatt/samples/J-E-2416.ew"),
                        content_dir: dir("inst/share/elektrawatt/content"),
                        docs_dir: dir("inst/share/elektrawatt/docs"),
                        notices_file: None,
                        privacy_file: None,
                    }
                );
                // Nothing near the executable, and no executable path at all: every resource is absent.
                assert_eq!(found("x/empty"), Resources::default());
                assert_eq!(locate_resources(None), Resources::default());
            }
            "environment" => {
                // The variables win over the flat tree; the notices have none and stay the ones beside the executable.
                assert_eq!(
                    found("x/flat"),
                    Resources {
                        sample_dir: dir("env/sample"),
                        content_dir: dir("env/content"),
                        docs_dir: dir("env/docs"),
                        notices_file: notices,
                        privacy_file: privacy,
                    }
                );
                // The share layout's directories lose to the variables as well.
                assert_eq!(found("inst/bin").content_dir, dir("env/content"));
            }
            other => panic!("unknown case {other}"),
        }
    }

    /// IN-2: `locate_resources` over the flat layout of D3 and the `share/elektrawatt` layout, an
    /// environment variable winning over both, and an absent resource `None`. Each case runs in a child process (this
    /// test binary, `--exact tests::locate_resources_child`) with the variables set or removed by `Command::env`, so
    /// that no variable of this process changes (`set_var` is `unsafe` in edition 2024, and this binary forbids
    /// `unsafe`) and none of the caller's reaches the layout case.
    #[test]
    fn locate_resources_finds_the_installed_layouts() {
        let fixture = Layouts::new();
        let vars = [
            ("EW_SAMPLE_DIR", "env/sample"),
            ("EW_CONTENT_DIR", "env/content"),
            ("EW_DOCS_DIR", "env/docs"),
        ];
        for case in ["layouts", "environment"] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child
                .args([
                    "--exact",
                    "tests::locate_resources_child",
                    "--nocapture",
                    "--",
                    case,
                ])
                .arg(&fixture.0);
            for (var, rel) in vars {
                if case == "environment" {
                    child.env(var, fixture.0.join(rel));
                } else {
                    child.env_remove(var);
                }
            }
            let output = child.output().unwrap();
            let said = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success(), "case {case}: {said}");
            // The child ran the one test, not an empty filter.
            assert!(said.contains("test result: ok. 1 passed"), "case {case}: {said}");
        }
    }
}
