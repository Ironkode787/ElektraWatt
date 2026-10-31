// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The visual play-test driver (WP-S6.13 part 4, visual spec §3 to §5): `elektrawatt --headless --visual` runs the
//! window's own [`App`] over a `ShellModel` in `Mode::Gui`, feeds its frames with egui input built from mouse and key
//! lines, and paints a frame with the offscreen renderer when the script asks for one. Never a real window, never the
//! desktop, never a screen capture.
//!
//! **Script.** The `ew.uiscript/1` header, then three kinds of line:
//!
//! - an action line (`{"action":…,"params":…}`): dispatched on the shell as in the headless mode (the harness's
//!   prelude and epilogue; the agent never sends one), one transcript line as `run_step` writes it;
//! - an input line (`{"input":"click","x":412,"y":96}`, spec §4.1): the egui events of the input in one or more
//!   frames, then frames until the window settles; one transcript line `{"step","input","dispatched","state"}`, with
//!   `"closed":true` once the application has closed;
//! - a frame line (`{"frame":"t0003.png"}`, optional `"settle":false` for a look, `"zoom":[x,y,w,h]`, and
//!   `"paint":false` for a replay that settles as the run did but paints nothing): the
//!   current frame painted to `<frames>/<file>`; one transcript line `{"frame","sha256"}` without a step number (a
//!   look is not a step). Without `--frames` or with `--no-frames` nothing is painted and `sha256` is null.
//!
//! **Geometry.** By default the window is 1280 × 800 points at 1.2 pixels per point (1536 × 960 pixels); above it a
//! title strip of 28 points (34 pixels) shows the window title in the theme's chrome colours, so a frame is
//! 1536 × 994 pixels. A run may choose another window and scale (`--window <W>x<H>` in points, `--ppp <n>`; see
//! [`Geometry`]): the strip is then `round(28 × ppp)` pixels high and the PNG's resolution `round(96 × ppp)` dpi, so
//! 1920 × 1080 points at 1 make a frame of 1920 × 1108 pixels. Input coordinates are frame pixels; a point in the
//! strip reaches nothing. At the default geometry every frame and transcript is as before; at another the
//! transcript's header names it (`"window":[W,H],"ppp":n` after `"version"`).
//!
//! **Determinism.** The frame clock (`input.time` = frames / 60, [`crate::clock::use_frame_clock`]), theme light,
//! density comfortable, language `en`, the bundled fonts, `ui_scale` 1, the CPU canvas, the caret's blink off; the
//! canvas's tiles and the page view's image are waited for in the frame that asked for them, and a running job is
//! waited for by blocking on it, never on time.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, Vec2, ViewportId};
use ew_render_cpu::{RgbaImage, encode_png};
use ew_render2d::gpu::HeadlessDevice;
use ew_viewmodel::script::{self, ScriptError, ScriptParser};
use ew_viewmodel::shell::{Mode, Prompt, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::app::{App, Pick, Platform};
use crate::headless::{HeadlessArgs, ScriptSource};
use crate::offscreen::Offscreen;

/// The window's size in points at the default geometry ([`Geometry::DEFAULT`]).
pub const WINDOW_POINTS: Vec2 = Vec2::new(1280.0, 800.0);
/// The frame's pixels per point at the default geometry.
pub const PPP: f32 = 1.2;
/// The title strip's height in frame pixels at the default geometry.
pub const STRIP_PX: u32 = 34;
/// The frame's width in pixels at the default geometry.
pub const FRAME_WIDTH: u32 = 1536;
/// The frame's height in pixels (window and title strip) at the default geometry.
pub const FRAME_HEIGHT: u32 = 960 + STRIP_PX;
/// The title strip's height in points.
const STRIP_POINTS: u32 = 28;
/// The largest frame side in pixels (the 300 % snapshot's 3840 × 2400 renders on the hosts the project uses).
const MAX_FRAME_SIDE: (u32, u32) = (3840, 2400);
/// One frame of the frame clock, in seconds.
const FRAME_S: f64 = 1.0 / 60.0;
/// At most this many frames settle one input (2 s of frame time).
const MAX_SETTLE_FRAMES: u32 = 120;
/// The steps of a drag between its press and its release.
const DRAG_STEPS: u32 = 8;
/// The largest zoom region in frame pixels.
pub const ZOOM_MAX: (u32, u32) = (768, 497);

/// The window size and scale of a visual run: the window in points and the pixels per point in hundredths (an `Eq`
/// form: 120 is 1.2). [`Geometry::DEFAULT`] is 1280 × 800 points at 1.2, the frame of 1536 × 994 pixels every
/// earlier run painted.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Geometry {
    /// The window's width in points.
    pub width: u32,
    /// The window's height in points.
    pub height: u32,
    /// The pixels per point in hundredths.
    pub ppp_hundredths: u32,
}

impl Default for Geometry {
    fn default() -> Geometry {
        Geometry::DEFAULT
    }
}

/// `round(n × hundredths / 100)`, halves up.
fn scaled(n: u32, hundredths: u32) -> u32 {
    (n * hundredths + 50) / 100
}

impl Geometry {
    /// 1280 × 800 points at 1.2 pixels per point.
    pub const DEFAULT: Geometry = Geometry {
        width: 1280,
        height: 800,
        ppp_hundredths: 120,
    };

    /// The geometry of `--window <W>x<H>` and `--ppp <n>` (each `None` when not given: the default's value).
    /// Accepted: W 1024 to 3840 and H 640 to 2160 points, ppp 1 to 3 with at most two decimals, and a frame of at
    /// most 3840 × 2400 pixels.
    ///
    /// # Errors
    ///
    /// The text naming the first problem.
    pub fn parse(window: Option<&str>, ppp: Option<&str>) -> Result<Geometry, String> {
        let mut g = Geometry::DEFAULT;
        if let Some(text) = window {
            let (w, h) = text
                .split_once(['x', 'X'])
                .and_then(|(w, h)| Some((digits(w)?, digits(h)?)))
                .ok_or_else(|| "--window needs <width>x<height> in points, e.g. 1920x1080".to_owned())?;
            if !(1024..=3840).contains(&w) || !(640..=2160).contains(&h) {
                return Err("--window must be from 1024x640 to 3840x2160 points".to_owned());
            }
            (g.width, g.height) = (w, h);
        }
        if let Some(text) = ppp {
            g.ppp_hundredths = hundredths(text)
                .filter(|n| (100..=300).contains(n))
                .ok_or_else(|| "--ppp needs a number from 1 to 3".to_owned())?;
        }
        if g.frame_width() > MAX_FRAME_SIDE.0 || g.frame_height() > MAX_FRAME_SIDE.1 {
            return Err("--window and --ppp make a frame larger than 3840 × 2400 pixels".to_owned());
        }
        Ok(g)
    }

    /// Whether this is [`Geometry::DEFAULT`].
    pub fn is_default(&self) -> bool {
        *self == Geometry::DEFAULT
    }

    /// The pixels per point.
    pub fn ppp(&self) -> f32 {
        self.ppp_hundredths as f32 / 100.0
    }

    /// The pixels per point as the transcript writes it (`1.2`, `1`, `1.25`).
    pub fn ppp_text(&self) -> String {
        let (int, frac) = (self.ppp_hundredths / 100, self.ppp_hundredths % 100);
        match frac {
            0 => format!("{int}"),
            f if f % 10 == 0 => format!("{int}.{}", f / 10),
            f => format!("{int}.{f:02}"),
        }
    }

    /// The window's size in points.
    pub fn window_points(&self) -> Vec2 {
        Vec2::new(self.width as f32, self.height as f32)
    }

    /// The window's size in pixels.
    pub fn window_pixels(&self) -> [u32; 2] {
        [
            scaled(self.width, self.ppp_hundredths),
            scaled(self.height, self.ppp_hundredths),
        ]
    }

    /// The title strip's height in pixels: `round(28 × ppp)`.
    pub fn strip_px(&self) -> u32 {
        scaled(STRIP_POINTS, self.ppp_hundredths)
    }

    /// The frame's width in pixels.
    pub fn frame_width(&self) -> u32 {
        self.window_pixels()[0]
    }

    /// The frame's height in pixels (window and title strip).
    pub fn frame_height(&self) -> u32 {
        self.window_pixels()[1] + self.strip_px()
    }

    /// The PNG's resolution: `round(96 × ppp)` dpi.
    pub fn dpi(&self) -> u32 {
        scaled(96, self.ppp_hundredths)
    }

    /// The window's point of frame pixel `(x, y)`, `None` in the title strip.
    pub fn to_points(&self, at: (i64, i64)) -> Option<Pos2> {
        let ppp = self.ppp();
        let y = at.1 - i64::from(self.strip_px());
        (y >= 0).then(|| Pos2::new((at.0 as f32 + 0.5) / ppp, (y as f32 + 0.5) / ppp))
    }
}

/// A whole number of 1 to 4 ASCII digits.
fn digits(text: &str) -> Option<u32> {
    if text.is_empty() || text.len() > 4 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// A decimal number with at most two decimals in hundredths (`1.25` is 125).
fn hundredths(text: &str) -> Option<u32> {
    let (int, frac) = text.split_once('.').unwrap_or((text, ""));
    if text.ends_with('.') || frac.len() > 2 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let int = digits(int)?;
    let frac = match frac.len() {
        0 => 0,
        1 => frac.parse::<u32>().ok()? * 10,
        _ => frac.parse::<u32>().ok()?,
    };
    Some(int * 100 + frac)
}

/// The file dialogs answered without a dialog (spec §4.3): bare names in the working directory (the run's `work/`;
/// a test names its own folder): a project `project`, a PDF `project.pdf`; an import or an image is cancelled, and
/// the system browser opens nothing (the trait's default).
struct VisualPlatform {
    base: PathBuf,
}

impl Platform for VisualPlatform {
    fn pick(&mut self, prompt: Prompt) -> Pick {
        match prompt {
            Prompt::OpenFolder | Prompt::NewFolder => Pick::Path(self.base.join("project")),
            Prompt::SavePdf => Pick::Path(self.base.join("project.pdf")),
            Prompt::OpenQet | Prompt::OpenPartsTable | Prompt::OpenImage => Pick::Cancelled,
        }
    }
}

/// One input of the vocabulary (spec §4.1), coordinates in frame pixels.
#[derive(Clone, PartialEq, Debug)]
pub enum Input {
    /// A click (press, then release in the next frame).
    Click {
        /// Where.
        at: (i64, i64),
        /// Which button.
        button: PointerButton,
        /// The modifiers held.
        mods: Modifiers,
    },
    /// Two clicks in consecutive frames.
    DoubleClick {
        /// Where.
        at: (i64, i64),
        /// The modifiers held.
        mods: Modifiers,
    },
    /// A press, eight moves on the straight line and a release.
    Drag {
        /// Where the press is.
        from: (i64, i64),
        /// Where the release is.
        to: (i64, i64),
        /// Which button.
        button: PointerButton,
        /// The modifiers held.
        mods: Modifiers,
    },
    /// The wheel, in lines (positive `dy` scrolls down, positive `dx` right).
    Scroll {
        /// Where the pointer is.
        at: (i64, i64),
        /// Lines to the right.
        dx: i64,
        /// Lines down.
        dy: i64,
        /// The modifiers held.
        mods: Modifiers,
    },
    /// Text, one character per frame.
    Type {
        /// The text.
        text: String,
    },
    /// A key with its modifiers, pressed and released `repeat` times.
    Key {
        /// The key; `None` for a chord of modifiers alone (`alt`).
        key: Option<Key>,
        /// The modifiers held.
        mods: Modifiers,
        /// How many times.
        repeat: u32,
    },
    /// The pointer moves there and stays.
    Hover {
        /// Where.
        at: (i64, i64),
    },
    /// Frames of the frame clock until the time has passed.
    Wait {
        /// Milliseconds.
        ms: u32,
    },
}

fn bad(text: impl Into<String>) -> String {
    text.into()
}

/// Reads the modifiers of `mods` (`ctrl`, `shift`, `alt` joined by `+`).
fn parse_mods(text: &str) -> Result<Modifiers, String> {
    let mut mods = Modifiers::NONE;
    for part in text.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" => {
                mods.ctrl = true;
                mods.command = true;
            }
            "shift" => mods.shift = true,
            "alt" => mods.alt = true,
            other => return Err(bad(format!("unknown modifier `{other}`"))),
        }
    }
    Ok(mods)
}

/// The key named `name`: egui's names and aliases, any case.
fn key_of(name: &str) -> Option<Key> {
    Key::from_name(name).or_else(|| {
        Key::ALL
            .iter()
            .copied()
            .find(|k| k.name().eq_ignore_ascii_case(name) || k.symbol_or_name().eq_ignore_ascii_case(name))
            .or(match name.to_ascii_lowercase().as_str() {
                "esc" => Some(Key::Escape),
                "return" => Some(Key::Enter),
                "del" => Some(Key::Delete),
                "up" | "arrowup" => Some(Key::ArrowUp),
                "down" | "arrowdown" => Some(Key::ArrowDown),
                "left" | "arrowleft" => Some(Key::ArrowLeft),
                "right" | "arrowright" => Some(Key::ArrowRight),
                "pgup" => Some(Key::PageUp),
                "pgdn" => Some(Key::PageDown),
                _ => None,
            })
    })
}

/// Reads a chord: modifiers and at most one key, joined by `+` (`ctrl+s`, `shift+tab`, `ctrl++`, `alt`).
pub fn parse_chord(chord: &str) -> Result<(Option<Key>, Modifiers), String> {
    if chord.is_empty() || chord.len() > 40 {
        return Err(bad("`chord` is 1 to 40 characters"));
    }
    // `ctrl++` and `+`: the last `+` is the key.
    let (head, last) = match chord.strip_suffix("++") {
        Some(head) => (head, "+"),
        None if chord == "+" => ("", "+"),
        None => match chord.rsplit_once('+') {
            Some((head, last)) => (head, last),
            None => ("", chord),
        },
    };
    let mut mods = if head.is_empty() {
        Modifiers::NONE
    } else {
        parse_mods(head)?
    };
    match last.to_ascii_lowercase().as_str() {
        "ctrl" | "shift" | "alt" => {
            let more = parse_mods(last)?;
            mods |= more;
            if more.ctrl {
                mods.command = true;
            }
            Ok((None, mods))
        }
        _ => match key_of(last) {
            Some(key) => Ok((Some(key), mods)),
            None => Err(bad(format!("unknown key `{last}`"))),
        },
    }
}

fn coordinate(o: &Map<String, Value>, name: &str, limit: u32) -> Result<i64, String> {
    match o.get(name).and_then(Value::as_i64) {
        Some(v) if (0..i64::from(limit)).contains(&v) => Ok(v),
        Some(_) => Err(bad(format!("`{name}` is outside the frame (0 to {})", limit - 1))),
        None => Err(bad(format!("`{name}` is an integer frame pixel"))),
    }
}

fn point(o: &Map<String, Value>, x: &str, y: &str, g: Geometry) -> Result<(i64, i64), String> {
    Ok((
        coordinate(o, x, g.frame_width())?,
        coordinate(o, y, g.frame_height())?,
    ))
}

fn mods_member(o: &Map<String, Value>) -> Result<Modifiers, String> {
    match o.get("mods") {
        None => Ok(Modifiers::NONE),
        Some(Value::String(s)) => parse_mods(s),
        Some(_) => Err(bad("`mods` is text like \"ctrl+shift\"")),
    }
}

fn button_member(o: &Map<String, Value>) -> Result<PointerButton, String> {
    match o.get("button").map(Value::as_str) {
        None | Some(Some("left")) => Ok(PointerButton::Primary),
        Some(Some("right")) => Ok(PointerButton::Secondary),
        Some(Some("middle")) => Ok(PointerButton::Middle),
        Some(_) => Err(bad("`button` is left, right or middle")),
    }
}

fn lines_member(o: &Map<String, Value>, name: &str) -> Result<i64, String> {
    match o.get(name) {
        None => Ok(0),
        Some(v) => match v.as_i64() {
            Some(n) if (-20..=20).contains(&n) => Ok(n),
            _ => Err(bad(format!("`{name}` is a whole number of lines from -20 to 20"))),
        },
    }
}

/// Reads an input line's object (spec §4.1); the error names what is wrong.
///
/// # Errors
///
/// A text naming the first problem: an unknown input or member, a value out of range.
pub fn parse_input(o: &Map<String, Value>) -> Result<Input, String> {
    parse_input_in(o, Geometry::DEFAULT)
}

/// [`parse_input`] with coordinates checked against the frame of `g`.
///
/// # Errors
///
/// As [`parse_input`].
pub fn parse_input_in(o: &Map<String, Value>, g: Geometry) -> Result<Input, String> {
    let kind = o
        .get("input")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("`input` names the input"))?;
    let members: &[&str] = match kind {
        "click" => &["x", "y", "button", "mods"],
        "double_click" | "right_click" => &["x", "y", "mods"],
        "drag" => &["x", "y", "to_x", "to_y", "button", "mods"],
        "scroll" => &["x", "y", "dx", "dy", "mods"],
        "type" => &["text"],
        "key" => &["chord", "repeat"],
        "hover" => &["x", "y"],
        "wait" => &["ms"],
        other => return Err(bad(format!("unknown input `{other}`"))),
    };
    if let Some(extra) = o.keys().find(|k| *k != "input" && !members.contains(&k.as_str())) {
        return Err(bad(format!("unknown member `{extra}` of `{kind}`")));
    }
    Ok(match kind {
        "click" => Input::Click {
            at: point(o, "x", "y", g)?,
            button: button_member(o)?,
            mods: mods_member(o)?,
        },
        "right_click" => Input::Click {
            at: point(o, "x", "y", g)?,
            button: PointerButton::Secondary,
            mods: mods_member(o)?,
        },
        "double_click" => Input::DoubleClick {
            at: point(o, "x", "y", g)?,
            mods: mods_member(o)?,
        },
        "drag" => Input::Drag {
            from: point(o, "x", "y", g)?,
            to: point(o, "to_x", "to_y", g)?,
            button: button_member(o)?,
            mods: mods_member(o)?,
        },
        "scroll" => {
            let (dx, dy) = (lines_member(o, "dx")?, lines_member(o, "dy")?);
            if dx == 0 && dy == 0 {
                return Err(bad("a scroll needs `dx` or `dy`"));
            }
            Input::Scroll {
                at: point(o, "x", "y", g)?,
                dx,
                dy,
                mods: mods_member(o)?,
            }
        }
        "type" => {
            let text = o
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("`text` is the text to type"))?;
            let n = text.chars().count();
            if !(1..=200).contains(&n) || text.chars().any(char::is_control) {
                return Err(bad("`text` is 1 to 200 characters without control characters"));
            }
            Input::Type {
                text: text.to_owned(),
            }
        }
        "key" => {
            let chord = o
                .get("chord")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("`chord` names the key, like \"ctrl+s\""))?;
            let (key, mods) = parse_chord(chord)?;
            let repeat = match o.get("repeat") {
                None => 1,
                Some(v) => match v.as_u64() {
                    Some(n @ 1..=20) => n as u32,
                    _ => return Err(bad("`repeat` is 1 to 20")),
                },
            };
            Input::Key { key, mods, repeat }
        }
        "hover" => Input::Hover {
            at: point(o, "x", "y", g)?,
        },
        _ => match o.get("ms").and_then(Value::as_u64) {
            Some(ms @ 100..=5000) => Input::Wait { ms: ms as u32 },
            _ => return Err(bad("`ms` is 100 to 5000")),
        },
    })
}

/// A frame line's request.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FrameRequest {
    /// The file name in the frames folder.
    pub file: String,
    /// Whether the window settles first (a look does not run a frame).
    pub settle: bool,
    /// A region of the frame to enlarge twice, in frame pixels.
    pub zoom: Option<[u32; 4]>,
    /// Whether the frame is painted (`"paint":false`: a replay settles as the run did and paints nothing, the
    /// harness's replay of a frame it already has).
    pub paint: bool,
}

/// Reads a frame line's object.
///
/// # Errors
///
/// A text naming the first problem.
pub fn parse_frame(o: &Map<String, Value>) -> Result<FrameRequest, String> {
    parse_frame_in(o, Geometry::DEFAULT)
}

/// [`parse_frame`] with a zoom checked against the frame of `g`.
///
/// # Errors
///
/// As [`parse_frame`].
pub fn parse_frame_in(o: &Map<String, Value>, g: Geometry) -> Result<FrameRequest, String> {
    if let Some(extra) = o
        .keys()
        .find(|k| !["frame", "settle", "zoom", "paint"].contains(&k.as_str()))
    {
        return Err(bad(format!("unknown member `{extra}` of a frame line")));
    }
    let file = o
        .get("frame")
        .and_then(Value::as_str)
        .filter(|f| {
            f.ends_with(".png")
                && f.len() <= 40
                && f.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
                && !f.starts_with('.')
        })
        .ok_or_else(|| bad("`frame` is a plain file name ending in .png"))?;
    let settle = match o.get("settle") {
        None => true,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(bad("`settle` is true or false")),
    };
    let paint = match o.get("paint") {
        None => true,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(bad("`paint` is true or false")),
    };
    let zoom = match o.get("zoom") {
        None => None,
        Some(Value::Array(a)) if a.len() == 4 => {
            let v: Vec<u32> = a
                .iter()
                .filter_map(|n| n.as_u64().and_then(|n| u32::try_from(n).ok()))
                .collect();
            let [x, y, w, h] = v[..] else {
                return Err(bad("`zoom` is [x, y, w, h] in frame pixels"));
            };
            if w == 0
                || h == 0
                || w > ZOOM_MAX.0
                || h > ZOOM_MAX.1
                || x.checked_add(w).is_none_or(|end| end > g.frame_width())
                || y.checked_add(h).is_none_or(|end| end > g.frame_height())
            {
                return Err(bad(format!(
                    "`zoom` is at most {} × {} pixels inside the frame",
                    ZOOM_MAX.0, ZOOM_MAX.1
                )));
            }
            Some([x, y, w, h])
        }
        Some(_) => return Err(bad("`zoom` is [x, y, w, h] in frame pixels")),
    };
    Ok(FrameRequest {
        file: file.to_owned(),
        settle,
        zoom,
        paint,
    })
}

/// The window's point of frame pixel `(x, y)` at the default geometry, `None` in the title strip.
pub fn to_points(at: (i64, i64)) -> Option<Pos2> {
    Geometry::DEFAULT.to_points(at)
}

/// The offscreen painters of a run that paints frames.
struct Painter {
    dev: HeadlessDevice,
    window: Offscreen,
    strip: Offscreen,
    strip_ctx: egui::Context,
    /// The frees of the last frame: applied before the next frame's sets (its shapes may still use them).
    frees: Vec<egui::TextureId>,
}

/// The driver: the window's frame, its input state and the painters.
pub struct Driver {
    app: Option<App>,
    /// The shell action lines go to once the application has closed (the epilogue; a fresh headless shell).
    after: Option<ShellModel>,
    config: ShellConfig,
    ctx: egui::Context,
    frames: u64,
    pointer: Option<Pos2>,
    clipboard: String,
    paste_next: bool,
    last_delay: Option<std::time::Duration>,
    shapes: Vec<egui::epaint::ClippedShape>,
    painter: Option<Painter>,
    frames_dir: Option<PathBuf>,
    record: Option<String>,
    /// The dispatch log of the input that closed the application.
    closing_log: Option<Value>,
    closed: bool,
    /// The accessibility tree of the frame, when a test asked for it ([`Driver::enable_accesskit`]).
    access: Option<Access>,
    /// The window size and scale.
    geometry: Geometry,
}

/// The accessibility tree as the frames reported it: its root and every node by ID.
#[derive(Default)]
struct Access {
    root: Option<egui::accesskit::NodeId>,
    nodes: std::collections::BTreeMap<egui::accesskit::NodeId, egui::accesskit::Node>,
}

impl Driver {
    /// A driver over a new shell with `config`; `frames_dir` is where frames go, `None` to paint none.
    ///
    /// # Errors
    ///
    /// A text when frames are wanted and the machine has no offscreen adapter.
    pub fn new(config: ShellConfig, frames_dir: Option<PathBuf>) -> Result<Driver, String> {
        Driver::in_folder(config, frames_dir, PathBuf::new())
    }

    /// [`Driver::new`] whose file dialogs answer with names in `base` (tests; the product's is the working
    /// directory).
    ///
    /// # Errors
    ///
    /// As [`Driver::new`].
    pub fn in_folder(
        config: ShellConfig,
        frames_dir: Option<PathBuf>,
        base: PathBuf,
    ) -> Result<Driver, String> {
        Driver::with_geometry(config, frames_dir, base, Geometry::DEFAULT)
    }

    /// [`Driver::in_folder`] with the window size and scale of `geometry`.
    ///
    /// # Errors
    ///
    /// As [`Driver::new`].
    pub fn with_geometry(
        config: ShellConfig,
        frames_dir: Option<PathBuf>,
        base: PathBuf,
        geometry: Geometry,
    ) -> Result<Driver, String> {
        crate::clock::use_frame_clock();
        let painter = match &frames_dir {
            None => None,
            Some(_) => {
                let dev = crate::offscreen::device()
                    .ok_or_else(|| "no offscreen adapter for a visual run".to_owned())?;
                let strip_ctx = egui::Context::default();
                crate::app::setup(&strip_ctx);
                Some(Painter {
                    window: Offscreen::new(&dev),
                    strip: Offscreen::new(&dev),
                    dev,
                    strip_ctx,
                    frees: Vec::new(),
                })
            }
        };
        let mut shell = ShellModel::new(Mode::Gui, config.clone());
        // The fixed rendering inputs (spec §3.1), before the log is on: they are the driver's, not an input's.
        for (key, value) in [
            ("language", "en"),
            ("theme", "light"),
            ("density", "comfortable"),
            ("canvas_backend", "cpu"),
        ] {
            let _ = shell.dispatch("settings.set", &json!({ "key": key, "value": value }));
        }
        let _ = shell.dispatch("view.ui_scale", &json!({ "scale": 1.0 }));
        shell.set_dispatch_log(true);
        let ctx = egui::Context::default();
        crate::app::setup(&ctx);
        Ok(Driver {
            app: Some(App::new(shell, Box::new(VisualPlatform { base }), None)),
            after: None,
            config,
            ctx,
            frames: 0,
            pointer: None,
            clipboard: String::new(),
            paste_next: false,
            last_delay: None,
            shapes: Vec::new(),
            painter,
            frames_dir,
            record: None,
            closing_log: None,
            closed: false,
            access: None,
            geometry,
        })
    }

    /// The shell action lines go to.
    pub fn shell_mut(&mut self) -> &mut ShellModel {
        match self.app {
            Some(ref mut app) => app.shell_mut(),
            None => {
                let config = &self.config;
                self.after
                    .get_or_insert_with(|| ShellModel::new(Mode::Headless, config.clone()))
            }
        }
    }

    /// Whether the application has closed.
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// The window size and scale.
    pub fn geometry(&self) -> Geometry {
        self.geometry
    }

    /// The frames run so far.
    pub fn frame_count(&self) -> u64 {
        self.frames
    }

    /// The window's frame (tests).
    pub fn app(&self) -> Option<&App> {
        self.app.as_ref()
    }

    /// Keeps the accessibility tree of every frame from now on, for [`Driver::find`] (tests).
    pub fn enable_accesskit(&mut self) {
        self.ctx.enable_accesskit();
        self.access = Some(Access::default());
    }

    /// The centre, in frame pixels, of the first widget in the window whose accessible name is `label` (depth first,
    /// in the tree's order); `None` without one or without [`Driver::enable_accesskit`].
    pub fn find(&self, label: &str) -> Option<(i64, i64)> {
        let access = self.access.as_ref()?;
        let mut stack = vec![access.root?];
        while let Some(id) = stack.pop() {
            let Some(node) = access.nodes.get(&id) else {
                continue;
            };
            if node.label() == Some(label)
                && let Some(b) = node.bounds()
            {
                let ppp = f64::from(self.geometry.ppp());
                let x = (b.x0 + b.x1) / 2.0 * ppp;
                let y = (b.y0 + b.y1) / 2.0 * ppp + f64::from(self.geometry.strip_px());
                return Some((x.floor() as i64, y.floor() as i64));
            }
            stack.extend(node.children().iter().rev().copied());
        }
        None
    }

    /// Every labelled widget of the window with its centre in frame pixels, in the tree's order (tests).
    pub fn labels(&self) -> Vec<(String, (i64, i64))> {
        let mut out = Vec::new();
        let Some(access) = self.access.as_ref() else {
            return out;
        };
        let mut stack: Vec<_> = access.root.into_iter().collect();
        while let Some(id) = stack.pop() {
            let Some(node) = access.nodes.get(&id) else {
                continue;
            };
            if let (Some(label), Some(b)) = (node.label(), node.bounds()) {
                let ppp = f64::from(self.geometry.ppp());
                let x = (b.x0 + b.x1) / 2.0 * ppp;
                let y = (b.y0 + b.y1) / 2.0 * ppp + f64::from(self.geometry.strip_px());
                out.push((label.to_owned(), (x.floor() as i64, y.floor() as i64)));
            }
            stack.extend(node.children().iter().rev().copied());
        }
        out
    }

    /// Runs one frame with `events`.
    fn frame(&mut self, mut events: Vec<Event>) {
        let Some(app) = self.app.as_mut() else { return };
        if std::mem::take(&mut self.paste_next) && !self.clipboard.is_empty() {
            events.insert(0, Event::Paste(self.clipboard.clone()));
        }
        let rect = Rect::from_min_size(Pos2::ZERO, self.geometry.window_points());
        let mut raw = egui::RawInput {
            screen_rect: Some(rect),
            time: Some(self.frames as f64 * FRAME_S),
            predicted_dt: FRAME_S as f32,
            events,
            focused: true,
            system_theme: Some(egui::Theme::Light),
            ..Default::default()
        };
        if let Some(viewport) = raw.viewports.get_mut(&ViewportId::ROOT) {
            viewport.native_pixels_per_point = Some(self.geometry.ppp());
            viewport.focused = Some(true);
            viewport.inner_rect = Some(rect);
        }
        let mut output = self.ctx.run_ui(raw, |ui| app.ui(ui));
        self.frames += 1;
        if let (Some(access), Some(update)) =
            (&mut self.access, output.platform_output.accesskit_update.take())
        {
            if let Some(tree) = update.tree {
                access.root = Some(tree.root);
            }
            access.nodes.extend(update.nodes);
        }
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                self.clipboard.clone_from(text);
            }
        }
        let viewport = output.viewport_output.remove(&ViewportId::ROOT);
        self.last_delay = viewport.as_ref().map(|v| v.repaint_delay);
        if viewport
            .iter()
            .flat_map(|v| &v.commands)
            .any(|c| matches!(c, egui::ViewportCommand::RequestPaste))
        {
            self.paste_next = true;
        }
        if let Some(p) = &mut self.painter {
            let frees = std::mem::take(&mut p.frees);
            for id in frees {
                p.window.free(id);
            }
            p.window.set_textures(&p.dev, &output.textures_delta);
            p.frees = output.textures_delta.free.iter().copied().collect();
            // A stable order for the frees (a hash set).
            p.frees.sort_by_key(|id| format!("{id:?}"));
        }
        // Handled (or, without painters, not needed): egui checks that a frame's texture changes were taken.
        output.textures_delta.clear();
        self.shapes = output.shapes;
        // A running job is waited for by blocking on it (spec §5.1), and the frame after it shows its end.
        if app.shell().jobs().entries().any(|e| e.is_running()) {
            let _ = app.shell_mut().wait_jobs();
            self.last_delay = Some(std::time::Duration::ZERO);
        }
        if app.shell().should_quit() {
            self.close();
        }
    }

    /// The application has closed: its shell is shut down, its command log kept for `--record`.
    fn close(&mut self) {
        if let Some(mut app) = self.app.take() {
            self.record = Some(script::record_log(app.shell()));
            self.closing_log = Some(dispatched_json(app.shell_mut()));
            app.shell_mut().shutdown();
        }
        self.closed = true;
    }

    /// Runs frames until the window settles: no repaint wanted within a frame and no job running, at most 120.
    pub fn settle(&mut self) {
        for _ in 0..MAX_SETTLE_FRAMES {
            if self.app.is_none() {
                return;
            }
            let wants = self.last_delay.is_none_or(|d| d.as_secs_f64() <= FRAME_S);
            if !wants {
                return;
            }
            self.frame(Vec::new());
        }
    }

    fn mods_event(mods: Modifiers) -> Event {
        Event::ModifiersChanged(mods)
    }

    /// The pointer at `at`: moved there, or gone when `at` is in the title strip.
    fn pointer_event(&mut self, at: (i64, i64)) -> Event {
        self.pointer = self.geometry.to_points(at);
        match self.pointer {
            Some(pos) => Event::PointerMoved(pos),
            None => Event::PointerGone,
        }
    }

    fn click_frames(&mut self, at: (i64, i64), button: PointerButton, mods: Modifiers) {
        let moved = self.pointer_event(at);
        let Some(pos) = self.pointer else {
            self.frame(vec![moved]);
            return;
        };
        let press = Event::PointerButton {
            pos,
            button,
            pressed: true,
            modifiers: mods,
        };
        self.frame(vec![Self::mods_event(mods), moved, press]);
        self.frame(vec![
            Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: mods,
            },
            Self::mods_event(Modifiers::NONE),
        ]);
    }

    /// Sends one input and settles (spec §4.1, §5.1).
    pub fn input(&mut self, input: &Input) {
        if self.app.is_none() {
            return;
        }
        // An input acts on what the last frame laid out: before the first input the window has been on screen.
        if self.frames == 0 {
            self.settle();
        }
        match input {
            Input::Click { at, button, mods } => self.click_frames(*at, *button, *mods),
            Input::DoubleClick { at, mods } => {
                self.click_frames(*at, PointerButton::Primary, *mods);
                self.click_frames(*at, PointerButton::Primary, *mods);
            }
            Input::Drag {
                from,
                to,
                button,
                mods,
            } => {
                let moved = self.pointer_event(*from);
                match self.pointer {
                    None => self.frame(vec![moved]),
                    Some(start) => {
                        self.frame(vec![
                            Self::mods_event(*mods),
                            moved,
                            Event::PointerButton {
                                pos: start,
                                button: *button,
                                pressed: true,
                                modifiers: *mods,
                            },
                        ]);
                        let (fx, fy) = (from.0 as f64, from.1 as f64);
                        let (tx, ty) = (to.0 as f64, to.1 as f64);
                        for k in 1..=DRAG_STEPS {
                            let t = f64::from(k) / f64::from(DRAG_STEPS);
                            let at = (
                                (fx + (tx - fx) * t).round() as i64,
                                (fy + (ty - fy) * t).round() as i64,
                            );
                            // The pointer stays in the window while the button is down (the strip clamps to its edge).
                            let at = (at.0, at.1.max(i64::from(self.geometry.strip_px())));
                            let ev = self.pointer_event(at);
                            self.frame(vec![ev]);
                        }
                        let end = self.pointer.unwrap_or(start);
                        self.frame(vec![
                            Event::PointerButton {
                                pos: end,
                                button: *button,
                                pressed: false,
                                modifiers: *mods,
                            },
                            Self::mods_event(Modifiers::NONE),
                        ]);
                    }
                }
            }
            Input::Scroll { at, dx, dy, mods } => {
                let moved = self.pointer_event(*at);
                let mut events = vec![Self::mods_event(*mods), moved];
                if self.pointer.is_some() {
                    events.push(Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: Vec2::new(-(*dx as f32), -(*dy as f32)),
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        modifiers: *mods,
                    });
                }
                self.frame(events);
                self.frame(vec![Self::mods_event(Modifiers::NONE)]);
            }
            Input::Type { text } => {
                for c in text.chars() {
                    self.frame(vec![Event::Text(c.to_string())]);
                }
            }
            Input::Key { key, mods, repeat } => {
                for _ in 0..*repeat {
                    self.key_frames(*key, *mods);
                }
            }
            Input::Hover { at } => {
                let ev = self.pointer_event(*at);
                self.frame(vec![ev]);
            }
            Input::Wait { ms } => {
                let n = (f64::from(*ms) / 1000.0 / FRAME_S).ceil() as u32;
                for _ in 0..n {
                    self.frame(Vec::new());
                }
            }
        }
        self.settle();
    }

    /// One key press and release, as `egui-winit` reports it: Ctrl+C, Ctrl+X and Ctrl+V become the clipboard's
    /// events, a printable key without Ctrl or Alt also types its character.
    fn key_frames(&mut self, key: Option<Key>, mods: Modifiers) {
        let Some(key) = key else {
            self.frame(vec![Self::mods_event(mods)]);
            self.frame(vec![Self::mods_event(Modifiers::NONE)]);
            return;
        };
        let command = mods.command_only();
        let mut down = vec![Self::mods_event(mods)];
        match key {
            Key::C if command => down.push(Event::Copy),
            Key::X if command => down.push(Event::Cut),
            Key::V if command => {
                if !self.clipboard.is_empty() {
                    down.push(Event::Paste(self.clipboard.clone()));
                }
            }
            _ => {
                down.push(Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers: mods,
                });
                if !mods.ctrl
                    && !mods.alt
                    && let Some(text) = key_text(key, mods.shift)
                {
                    down.push(Event::Text(text));
                }
            }
        }
        self.frame(down);
        self.frame(vec![
            Event::Key {
                key,
                physical_key: Some(key),
                pressed: false,
                repeat: false,
                modifiers: mods,
            },
            Self::mods_event(Modifiers::NONE),
        ]);
    }

    /// Paints the current frame: the title strip over the window (1536 × 994 pixels at the default geometry). `None`
    /// without painters.
    pub fn paint(&mut self) -> Option<RgbaImage> {
        let title = match &self.app {
            Some(app) => app.shell().title(),
            None => String::new(),
        };
        let tokens = crate::theme::current(&self.ctx);
        let p = self.painter.as_mut()?;
        let window = p.window.paint(
            &p.dev,
            &self.ctx,
            self.shapes.clone(),
            self.geometry.window_pixels(),
            self.geometry.ppp(),
        );
        let strip = paint_strip(p, &tokens, &title, self.frames, self.geometry);
        let mut pixels = strip.pixels;
        pixels.extend_from_slice(&window.pixels);
        Some(RgbaImage {
            width: window.width,
            height: strip.height + window.height,
            pixels,
        })
    }

    /// Answers a frame line: settles unless it is a look, paints, writes the PNG and returns its SHA-256.
    ///
    /// # Errors
    ///
    /// A text when the PNG could not be written.
    pub fn frame_line(&mut self, request: &FrameRequest) -> Result<Option<String>, String> {
        if request.settle || self.frames == 0 {
            self.settle();
        }
        if !request.paint {
            return Ok(None);
        }
        let Some(dir) = self.frames_dir.clone() else {
            return Ok(None);
        };
        if self.app.is_none() {
            return Ok(None);
        }
        let Some(mut image) = self.paint() else {
            return Ok(None);
        };
        if let Some([x, y, w, h]) = request.zoom {
            image = enlarge(&image, x, y, w, h);
        }
        let bytes = encode_png(&image, Some(self.geometry.dpi()));
        let path = dir.join(&request.file);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Some(format!("{:x}", Sha256::digest(&bytes))))
    }
}

/// The character a key types (letters, digits, space and ASCII punctuation), `None` for the others.
fn key_text(key: Key, shift: bool) -> Option<String> {
    if key == Key::Space {
        return Some(" ".to_owned());
    }
    let symbol = key.symbol_or_name();
    let mut chars = symbol.chars();
    let c = chars.next()?;
    if chars.next().is_some() || !c.is_ascii_graphic() {
        return None;
    }
    Some(if c.is_ascii_alphabetic() && !shift {
        c.to_ascii_lowercase().to_string()
    } else {
        c.to_string()
    })
}

/// The title strip: the theme's chrome surface, the title at the left, a subtle line below.
fn paint_strip(
    p: &mut Painter,
    tokens: &crate::theme::Tokens,
    title: &str,
    frames: u64,
    g: Geometry,
) -> RgbaImage {
    let ppp = g.ppp();
    let height = g.strip_px() as f32 / ppp;
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(g.window_points().x, height));
    let mut raw = egui::RawInput {
        screen_rect: Some(rect),
        time: Some(frames as f64 * FRAME_S),
        predicted_dt: FRAME_S as f32,
        ..Default::default()
    };
    if let Some(viewport) = raw.viewports.get_mut(&ViewportId::ROOT) {
        viewport.native_pixels_per_point = Some(ppp);
    }
    let fill = tokens.surface_chrome.color32();
    let line = crate::theme::stroke(1.0 / ppp, tokens.border_subtle);
    let inset = tokens.space_2 * 2.0;
    let text = tokens.text_primary.color32();
    let size = tokens.type_label.size;
    let mut output = p.strip_ctx.run_ui(raw, |ui| {
        let painter = ui.painter();
        painter.rect_filled(rect, crate::theme::SQUARE, fill);
        painter.hline(rect.x_range(), rect.bottom() - 0.5 / ppp, line);
        painter.text(
            Pos2::new(inset, rect.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(size),
            text,
        );
    });
    p.strip.set_textures(&p.dev, &output.textures_delta);
    let image = p.strip.paint(
        &p.dev,
        &p.strip_ctx,
        std::mem::take(&mut output.shapes),
        [g.frame_width(), g.strip_px()],
        ppp,
    );
    p.strip.free_textures(&output.textures_delta);
    output.textures_delta.clear();
    image
}

/// The region `x, y, w, h` of `image`, each pixel twice as wide and high (a zoom: a picture only).
fn enlarge(image: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
    let mut pixels = Vec::with_capacity((w * h * 16) as usize);
    for row in y..y + h {
        let start = ((row * image.width + x) * 4) as usize;
        let line = &image.pixels[start..start + (w * 4) as usize];
        let mut wide = Vec::with_capacity(line.len() * 2);
        for px in line.as_chunks::<4>().0 {
            wide.extend_from_slice(px);
            wide.extend_from_slice(px);
        }
        pixels.extend_from_slice(&wide);
        pixels.extend_from_slice(&wide);
    }
    RgbaImage {
        width: w * 2,
        height: h * 2,
        pixels,
    }
}

/// The `dispatched` member: the dispatch log of an input as `[{action, outcome, reason}]`.
fn dispatched_json(shell: &mut ShellModel) -> Value {
    Value::Array(
        shell
            .take_dispatch_log()
            .into_iter()
            .map(|e| json!({"action": e.action, "outcome": e.outcome, "reason": e.reason}))
            .collect(),
    )
}

/// What one script line is.
enum Line {
    Blank,
    Action(script::Step),
    Input(Value, Input),
    Frame(FrameRequest),
}

/// Reads one line: input and frame lines here, the header and action lines by the script parser (whose line count
/// covers every line).
fn read_line(parser: &mut ScriptParser, raw: &str, number: usize, g: Geometry) -> Result<Line, ScriptError> {
    let malformed = |message: String| ScriptError::Malformed {
        line: number,
        message,
    };
    if parser.header_seen() && !raw.trim().is_empty() {
        let value: Value = serde_json::from_str(raw).map_err(|e| malformed(format!("not JSON: {e}")))?;
        if let Value::Object(o) = &value {
            if o.contains_key("input") {
                parser.feed("")?;
                let input = parse_input_in(o, g).map_err(malformed)?;
                return Ok(Line::Input(value, input));
            }
            if o.contains_key("frame") {
                parser.feed("")?;
                return parse_frame_in(o, g).map(Line::Frame).map_err(malformed);
            }
        }
    }
    Ok(match parser.feed(raw)? {
        None => Line::Blank,
        Some(step) => Line::Action(step),
    })
}

/// Runs the visual mode (`--headless --visual`) and returns the process exit code; the transcript goes to `out`. The
/// file dialogs answer with names in the working directory (the run's `work/`).
pub fn run(
    config: ShellConfig,
    args: &HeadlessArgs,
    stdin: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    run_in_folder(config, args, PathBuf::new(), stdin, out, err)
}

/// [`run`] whose file dialogs answer with names in `base` (tests: never the crate's own folder).
pub fn run_in_folder(
    mut config: ShellConfig,
    args: &HeadlessArgs,
    base: PathBuf,
    stdin: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if let Some(dir) = &args.config_dir {
        // The library tiers stay (WP-S1.19 §11: `EW_DATA_DIR` names them for a script).
        config = ShellConfig::in_dir(dir, &config.app_version).with_libraries(config.libraries);
    }
    let frames_dir = if args.no_frames { None } else { args.frames.clone() };
    let mut driver = match Driver::with_geometry(config.clone(), frames_dir, base, args.geometry) {
        Ok(d) => d,
        Err(message) => {
            let _ = writeln!(err, "error: {message}");
            return 2;
        }
    };
    let mut sink = |line: &str| -> io::Result<()> {
        out.write_all(line.as_bytes())?;
        out.write_all(b"\n")?;
        out.flush()
    };
    let lines: Box<dyn Iterator<Item = io::Result<String>>> = match &args.script {
        ScriptSource::File(path) => match std::fs::read_to_string(path) {
            Ok(text) => {
                if let Some(name) = path.file_name() {
                    driver.shell_mut().set_origin_file(&name.to_string_lossy());
                }
                Box::new(
                    text.lines()
                        .map(|l| Ok(l.to_owned()))
                        .collect::<Vec<_>>()
                        .into_iter(),
                )
            }
            Err(e) => {
                let _ = writeln!(err, "error: {}: {e}", path.display());
                return 2;
            }
        },
        ScriptSource::Stdin => {
            driver.shell_mut().set_origin_file("stdin");
            Box::new(stdin.lines())
        }
    };
    let ended = drive(&mut driver, lines, args.keep_going, &mut sink);
    let code = match ended {
        Err(e) => {
            let _ = writeln!(err, "error: {e}");
            2
        }
        Ok(Err(e)) => {
            let _ = writeln!(err, "error: {e}");
            2
        }
        Ok(Ok(code)) => code,
    };
    let shell = driver.shell_mut();
    let unseen = shell.wait_jobs();
    for text in &unseen {
        let _ = writeln!(err, "error: {text}");
    }
    let mut code = code;
    if code == 0 && !driver.closed() && driver.shell_mut().is_dirty() {
        let _ = writeln!(err, "error: {}", strings::text("error-unsaved-at-end"));
        code = 1;
    }
    if !unseen.is_empty() && code == 0 {
        code = 1;
    }
    if let Some(path) = &args.record {
        let text = match driver.record.clone() {
            Some(t) => t,
            None => script::record_log(driver.shell_mut()),
        };
        if let Err(e) = std::fs::write(path, text) {
            let _ = writeln!(err, "error: {}: {e}", path.display());
            driver.shell_mut().shutdown();
            return 2;
        }
    }
    driver.shell_mut().shutdown();
    code
}

/// The transcript's header: the shell's, and at a geometry other than the default its window and scale after the
/// version (`{"$schema":…,"version":…,"window":[1920,1080],"ppp":1}`).
pub fn transcript_header(version: &str, g: Geometry) -> String {
    let header = script::transcript_header(version);
    if g.is_default() {
        return header;
    }
    let open = header.strip_suffix('}').unwrap_or(&header);
    format!(
        "{open},\"window\":[{},{}],\"ppp\":{}}}",
        g.width,
        g.height,
        g.ppp_text()
    )
}

/// The script loop: the transcript's lines to `sink`; the exit code, or the usage error that ended the run.
fn drive(
    driver: &mut Driver,
    lines: Box<dyn Iterator<Item = io::Result<String>> + '_>,
    keep_going: bool,
    sink: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<Result<i32, ScriptError>> {
    let version = driver.config.app_version.clone();
    driver.shell_mut().reset_aliases();
    sink(&transcript_header(&version, driver.geometry))?;
    let mut parser = ScriptParser::new();
    let mut ext = crate::headless::Snapshot;
    let mut number = 0usize;
    let mut not_done = 0usize;
    for (i, raw) in lines.enumerate() {
        let raw = raw?;
        let line = match read_line(&mut parser, &raw, i + 1, driver.geometry) {
            Ok(l) => l,
            Err(e) => return Ok(Err(e)),
        };
        match line {
            Line::Blank => {}
            Line::Action(step) => {
                number += 1;
                let shell = driver.shell_mut();
                let result = script::run_step(shell, &mut ext, number, &step);
                // An action line's own dispatches are its outcome, not an input's.
                let _ = shell.take_dispatch_log();
                // The model changed under the window: the next frame line settles it (the last frame's repaint delay
                // was the window's answer to its own input, not to this line).
                driver.last_delay = None;
                sink(&result.line)?;
                if !result.ok {
                    if !keep_going {
                        return Ok(Ok(1));
                    }
                    not_done += 1;
                }
                if driver.shell_mut().should_quit() {
                    driver.close();
                    return Ok(Ok(i32::from(not_done > 0)));
                }
            }
            Line::Input(value, input) => {
                number += 1;
                let was_closed = driver.closed();
                if was_closed {
                    sink(&format!(
                        "{{\"step\":{number},\"input\":{value},\"dispatched\":[],\"closed\":true}}"
                    ))?;
                    continue;
                }
                driver.input(&input);
                let line = match driver.app.as_mut() {
                    Some(app) => {
                        let shell = app.shell_mut();
                        let dispatched = dispatched_json(shell);
                        let state = script::state_json(shell);
                        shell.alias_text(&format!(
                            "{{\"step\":{number},\"input\":{value},\"dispatched\":{dispatched},\"state\":{state}}}"
                        ))
                    }
                    None => {
                        // The input closed the application: what it dispatched is in the closed shell's log, which
                        // `close` kept.
                        let dispatched = driver.closing_log.take().unwrap_or(Value::Array(Vec::new()));
                        format!(
                            "{{\"step\":{number},\"input\":{value},\"dispatched\":{dispatched},\"closed\":true}}"
                        )
                    }
                };
                sink(&line)?;
            }
            Line::Frame(request) => {
                let sha = match driver.frame_line(&request) {
                    Ok(sha) => sha,
                    Err(message) => {
                        return Ok(Err(ScriptError::Malformed { line: i + 1, message }));
                    }
                };
                let sha = sha.map_or(Value::Null, Value::from);
                // What settling before the frame dispatched (the start frame's layout) is the frame line's.
                let tail = match driver.app.as_mut() {
                    Some(app) => format!(",\"dispatched\":{}", dispatched_json(app.shell_mut())),
                    None => ",\"closed\":true".to_owned(),
                };
                sink(&format!(
                    "{{\"frame\":{},\"sha256\":{sha}{tail}}}",
                    Value::from(request.file.as_str())
                ))?;
            }
        }
    }
    Ok(match parser.finish() {
        Ok(()) => Ok(i32::from(not_done > 0)),
        Err(e) => Err(e),
    })
}
