// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ElektraWatt desktop application: eframe shell, file dialogs, the headless mode and exit codes. Native menus,
//! clipboard and the crash handler come with later work packages (WP-S2.01 part 2, FT-310).
//!
//! ```text
//! elektrawatt [--no-gpu] [<project-dir>]               the window; the project opens after the first frame
//! elektrawatt --headless --script <file|-> [--record <file.ewcmd>] [--config-dir <dir>] [--keep-going]
//!                                                      the same actions from a UI script, no window, no GPU
//! elektrawatt --smoke <project-dir>                    opens read-only, draws page 1 at 36 dpi, exits
//! ```
//!
//! Exit codes of the headless mode: 0 every step done, 1 a step was refused or failed (without `--keep-going` the run
//! stops there; with it the run goes on and the code is 1 when any step was not done), 2 a usage error. The window
//! exits with 3 when the system has no graphics adapter at all (egui's own chrome needs one; WP-S2.02 section 7.2):
//! the message names the headless mode, the command-line tool `ew` and, on Linux, Mesa's software drivers.
//! `--no-gpu` draws the canvas on the CPU and asks eframe for a software adapter (WARP, llvmpipe, lavapipe) for the
//! window.
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
use ew_ui::app::{App, Pick, PickReply, Platform, setup, spawn_pick};
use ew_ui::ew_viewmodel::shell::{Mode, Prompt, Resources, ShellConfig, ShellModel};
use ew_ui::ew_viewmodel::strings;
use ew_ui::ew_viewmodel::window::{MonitorArea, WindowGeometry};
use ew_ui::gpu::{Flags, adapter_count, wgpu_configuration};
use ew_ui::headless::{self, HeadlessArgs, ScriptSource};
use ew_ui::visual::Geometry;
use ew_ui::window_memory::{CoordSpace, Display, FirstFrameStep};

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
    // WP-S6.13 part 4 (BV1): the visual driver is a headless mode; its frame options need it.
    if visual && !headless {
        return Err("--visual needs --headless".to_owned());
    }
    if (frames.is_some() || no_frames) && !visual {
        return Err("--frames and --no-frames need --visual".to_owned());
    }
    if frames.is_some() && no_frames {
        return Err("--frames and --no-frames exclude each other".to_owned());
    }
    // Cycle 39: a visual run's window size and scale (default 1280 × 800 points at 1.2).
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

/// How a dialog of `rfd` is shown: what it asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DialogKind {
    Folder,
    File,
    Save,
}

impl RfdPlatform {
    /// The dialogs of the window `cc` was made for. On Windows the window is their owner, so it takes no input while
    /// a dialog is open, as when the dialog ran on the window's thread (FT-467); elsewhere they have no parent, as
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
                base.set_title(strings::text("prompt-open-title")),
                DialogKind::Folder,
            ),
            Prompt::NewFolder => (
                base.set_title(strings::text("prompt-save-as-title")),
                DialogKind::Folder,
            ),
            Prompt::OpenQet => (
                base.set_title(strings::text("prompt-open-qet-title"))
                    .add_filter("QElectroTech", &["qet"]),
                DialogKind::File,
            ),
            Prompt::OpenPartsTable => (
                base.set_title(strings::text("prompt-open-parts-title"))
                    .add_filter("CSV", &["csv"]),
                DialogKind::File,
            ),
            Prompt::OpenImage => (
                base.set_title(strings::text("prompt-open-image-title"))
                    .add_filter("Images", &["svg", "png", "jpg", "jpeg"]),
                DialogKind::File,
            ),
            Prompt::SavePdf => (
                base.set_title(strings::text("prompt-export-title"))
                    .set_file_name(strings::text("prompt-pdf-name"))
                    .add_filter("PDF", &["pdf"]),
                DialogKind::Save,
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
    /// at once (FT-467).
    fn start_pick(&mut self, prompt: Prompt, ctx: &egui::Context) -> PickReply {
        if !Self::available() {
            return PickReply::Now(Pick::Unavailable);
        }
        let (dialog, kind) = self.dialog(prompt);
        match spawn_pick(ctx, move || Self::run(dialog, kind)) {
            Ok(rx) => PickReply::Later(rx),
            // Without a thread for it the dialog runs on the window's thread, as before FT-467.
            Err(_) => PickReply::Now(self.pick(prompt)),
        }
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

/// The library tiers of the shell's sessions (WP-S1.19 §11): the system libraries of the installation's content
/// (`content/libraries`), the user's data and cache directories. An isolated run (a script) reads libraries only when
/// `EW_DATA_DIR` names its data directory, never the user's own, and keeps its search index in `EW_CACHE_DIR` when
/// that is set, else in `<EW_DATA_DIR>/cache`, never in the user's own cache (FX38b j1).
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

/// FT-526 (R1-4, R2-1, R3-1, R3-2): on the first frame the window goes to the saved outer position of `saved` (in the
/// platform's desktop space: physical pixels, logical points on macOS), one whose position is on no monitor (its
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
                scale: m.scale_factor() as f32,
                primary: primary.as_ref() == Some(&m),
            }
        })
        .collect();
    let inner = window.inner_size();
    let steps = ew_ui::window_memory::first_frame(
        CoordSpace::of_this_platform(),
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

/// The exit code 3 of a host without any graphics adapter (WP-S2.02 section 7.2, decision log ID-514).
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
    let mut shell = ShellModel::new(Mode::Gui, shell_config(false));
    // The first start (and only that) shows the first-run flow, unless the user named a project to open.
    if initial.is_none() {
        shell.start_first_run_if_needed();
    }
    let flags = Flags { no_gpu };
    // WP-S2.17 part 3 (DK6, FT-348): the last size, position and maximised state, restored.
    let viewport = ew_ui::window_memory::restore(
        egui::ViewportBuilder::default()
            .with_title(strings::text("window-title-start"))
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        shell.settings().window,
    );
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: wgpu_configuration(flags),
        ..eframe::NativeOptions::default()
    };
    let result = eframe::run_native(
        strings::text("window-title-start"),
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
            eprintln!("{message}");
            code
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
        Ok(Cli::Gui(initial, no_gpu)) => run_gui(initial, no_gpu),
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

    /// FX38b j1: an isolated run with only `EW_DATA_DIR` keeps its search index under that directory, never in the
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
    fn no_arguments_is_the_window() {
        assert_eq!(parse(&[]), Ok(Cli::Gui(None, false)));
        assert_eq!(parse(&["p.ew"]), Ok(Cli::Gui(Some(PathBuf::from("p.ew")), false)));
    }

    #[test]
    fn b6_no_gpu_is_a_flag_of_the_window_only() {
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

    #[test]
    fn the_headless_mode_needs_a_script() {
        assert_eq!(
            parse(&["--headless", "--script", "s.jsonl"]),
            Ok(Cli::Headless(HeadlessArgs {
                script: ScriptSource::File(PathBuf::from("s.jsonl")),
                record: None,
                config_dir: None,
                keep_going: false,
                visual: false,
                frames: None,
                no_frames: false,
                geometry: Geometry::DEFAULT
            }))
        );
        assert_eq!(
            parse(&["--headless", "--script", "-", "--record", "r.ewcmd"]),
            Ok(Cli::Headless(HeadlessArgs {
                script: ScriptSource::Stdin,
                record: Some(PathBuf::from("r.ewcmd")),
                config_dir: None,
                keep_going: false,
                visual: false,
                frames: None,
                no_frames: false,
                geometry: Geometry::DEFAULT
            }))
        );
        assert_eq!(
            parse(&["--headless", "--script", "-", "--keep-going"]),
            Ok(Cli::Headless(HeadlessArgs {
                script: ScriptSource::Stdin,
                record: None,
                config_dir: None,
                keep_going: true,
                visual: false,
                frames: None,
                no_frames: false,
                geometry: Geometry::DEFAULT
            }))
        );
        assert_eq!(
            parse(&["--headless", "--script", "s", "--config-dir", "c"]),
            Ok(Cli::Headless(HeadlessArgs {
                script: ScriptSource::File(PathBuf::from("s")),
                record: None,
                config_dir: Some(PathBuf::from("c")),
                keep_going: false,
                visual: false,
                frames: None,
                no_frames: false,
                geometry: Geometry::DEFAULT
            }))
        );
        assert!(parse(&["--config-dir", "c"]).is_err());
        // WP-S6.13 BD1: the flag belongs to the headless mode.
        assert!(parse(&["--keep-going"]).is_err());
        assert!(parse(&["--smoke", "p.ew", "--keep-going"]).is_err());
        assert!(USAGE.contains("--keep-going"));
        assert!(parse(&["--headless"]).is_err());
        assert!(parse(&["--script", "s"]).is_err());
        assert!(parse(&["--record", "r"]).is_err());
        assert!(parse(&["--headless", "--script"]).is_err());
        assert!(parse(&["--headless", "--script", "s", "p.ew"]).is_err());
    }

    /// WP-S6.13 part 4 BV1: `--visual` only with `--headless` (else a usage error, exit 2); `--frames` and
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
                script: ScriptSource::Stdin,
                record: None,
                config_dir: None,
                keep_going: true,
                visual: true,
                frames: Some(PathBuf::from("f")),
                no_frames: false,
                geometry: Geometry::DEFAULT
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

    /// Cycle 39: `--window <W>x<H>` and `--ppp <n>` choose a visual run's window size and scale; each error names
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
