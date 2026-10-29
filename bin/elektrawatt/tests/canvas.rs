// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The headless canvas through the built binary (WP-S2.02 part 1, acceptance checks A6 and A8; the viewer of
//! WP-S2.03 part 1, V8; the tool framework of part 2, T7 and T8): no window, no GPU, no display. A7's transcripts are
//! `headless.rs`'s and its snapshot bytes T7's; V6, V7 and T10's flows are `ew-viewmodel`'s tests and V12's two runs
//! are A7's (test clean-up, 2026-10-30).
//! Every test works on a temporary copy of Golden A with `EW_CONFIG_DIR` in a temporary directory; nothing is
//! written into the repository.

mod support;

#[path = "canvas/folio_scripts.rs"]
mod folio_cases;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::canvas::{HOVER_SLOP_PT, pick, rect_centre};
use ew_ui::ew_viewmodel::ew_api::Session;
use ew_ui::ew_viewmodel::ew_api::output::ew_dl::ew_units::Rect;
use ew_ui::ew_viewmodel::ew_api::output::{ExportOptions, PageSelection};
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-canvas-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A copy of Golden A named `J-E-2416.ew` in a subdirectory of its own.
    fn golden(&self, sub: &str) -> PathBuf {
        let target = self.0.join(sub).join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &target);
        target
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn script(steps: &[(&str, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"canvas\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

fn run_script(tmp: &Tmp, name: &str, text: &str) -> Output {
    let file = tmp.path(name);
    std::fs::write(&file, text).unwrap();
    Command::new(bin_exe())
        .args(["--headless", "--script"])
        .arg(&file)
        .env("EW_CONFIG_DIR", tmp.path("config"))
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn transcript(out: &Output) -> Vec<Value> {
    String::from_utf8(out.stdout.clone())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn decode_rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("valid png");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        other => panic!("unexpected PNG colour type {other:?}"),
    };
    (info.width, info.height, rgba)
}

/// The share of pixels (in per cent) where any channel differs by more than `threshold`.
fn differing_percent(a: &[u8], b: &[u8], threshold: u8) -> f64 {
    assert_eq!(a.len(), b.len());
    let bad = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > threshold))
        .count();
    100.0 * bad as f64 / (a.len() / 4) as f64
}

/// The canvas script: folio 103 at 156.25 % (D = 150) in a 1200 × 800 pane, snapshots with the grid on and off,
/// then fitted (the surround and the page bounds in view); `theme` set first.
fn canvas_script(dir: &Path, theme: &str, out: &Path) -> String {
    let p = |name: &str| out.join(name).to_string_lossy().into_owned();
    script(&[
        ("settings.set", json!({"key": "theme", "value": theme})),
        (
            "file.open",
            json!({ "dir": dir.to_string_lossy(), "read_only": true }),
        ),
        ("page.goto", json!({"folio": "103"})),
        (
            "view.canvas_resize",
            json!({"width": 1200, "height": 800, "ppp": 1.0}),
        ),
        ("view.zoom_to", json!({"percent": 156.25})),
        ("canvas.snapshot", json!({"path": p("grid.png")})),
        ("view.grid", json!({})),
        ("canvas.snapshot", json!({"path": p("plain.png")})),
        ("view.fit_page", json!({})),
        ("canvas.snapshot", json!({"path": p("fit.png")})),
        ("ui.describe", json!({})),
        ("file.close", json!({})),
    ])
}

/// The settled origin of the sheet on screen (whole pixels) and the resolution `round(D)` of a transcript state.
fn settled(state: &Value) -> ((i64, i64), u32) {
    let c = &state["canvas"];
    let zoom = c["zoom"].as_f64().unwrap() / 100.0;
    let v: Vec<i64> = c["view_nm"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_i64().unwrap())
        .collect();
    let (cx, cy) = ((v[0] + v[2]) as f64 / 2.0, (v[1] + v[3]) as f64 / 2.0);
    let r = (zoom * 96.0).round();
    let o = |half: f64, c: f64| (half - c * r / 25_400_000.0).round() as i64;
    ((o(600.0, cx), o(400.0, cy)), r as u32)
}

fn rgb_at(img: &[u8], width: u32, x: i64, y: i64) -> [u8; 3] {
    let i = (y as usize * width as usize + x as usize) * 4;
    [img[i], img[i + 1], img[i + 2]]
}

/// A6: the canvas snapshot's sheet matches the golden page, the grid dots sit on the QET 10 px pitch, the surround
/// and the page bounds are where the view says, and the dark theme changes the surround only.
#[test]
fn a6_the_canvas_snapshot_matches_the_golden_page_and_shows_the_view() {
    let tmp = Tmp::new();
    let dir = tmp.golden("a6");
    let light = tmp.path("light");
    let dark = tmp.path("dark");
    std::fs::create_dir_all(&light).unwrap();
    std::fs::create_dir_all(&dark).unwrap();
    let out = run_script(&tmp, "light.jsonl", &canvas_script(&dir, "light", &light));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let steps = transcript(&out);
    // Step 5 (zoom_to) is the 156.25 % view; step 9 (fit_page) the fitted one.
    let zoomed = &steps[5]["state"];
    assert_eq!(zoomed["canvas"]["zoom"], 156.3);
    let ((ox, oy), r) = settled(zoomed);
    assert_eq!(r, 150);

    // The sheet with the grid off matches the crop of the 150 dpi golden PNG within doc 20's tolerance.
    let (gw, _, golden) = decode_rgba(&std::fs::read(Path::new(EXPECTED).join("png/103.png")).unwrap());
    let (w, h, plain) = decode_rgba(&std::fs::read(light.join("plain.png")).unwrap());
    assert_eq!((w, h), (1200, 800));
    let mut crop = Vec::with_capacity(plain.len());
    for y in 0..800i64 {
        for x in 0..1200i64 {
            let (sx, sy) = (x - ox, y - oy);
            let i = (sy as usize * gw as usize + sx as usize) * 4;
            crop.extend_from_slice(&golden[i..i + 4]);
        }
    }
    let pct = differing_percent(&plain, &crop, 16);
    println!(
        "canvas snapshot at D = 150 against the golden crop: {pct:.4} % of the pixels differ by more than 16"
    );
    assert!(
        pct <= 0.5,
        "{pct:.3} % of the sheet pixels differ from the golden crop"
    );

    // The grid dots: the pixels the grid adds are grid-dot grey, at the QET 10 px pitch mapped onto the sheet.
    let (_, _, grid) = decode_rgba(&std::fs::read(light.join("grid.png")).unwrap());
    let pitch = 7_937_500.0 / 3.0 * 986_282_578.0 / 1e9;
    let px_nm = 25_400_000.0 / f64::from(r);
    let on_grid = |screen: i64, origin: i64| {
        let sheet = (screen as f64 + 0.5 - origin as f64) * px_nm;
        let k = (sheet - 9_790_000.0) / pitch;
        (k - k.round()).abs() * pitch <= px_nm
    };
    let mut dots = 0;
    for y in 0..800i64 {
        for x in 0..1200i64 {
            if rgb_at(&grid, 1200, x, y) == rgb_at(&plain, 1200, x, y) {
                continue;
            }
            // Doc 16a §4.6 (WP-S2.17 part 5): `canvas.grid_dot`.
            assert_eq!(rgb_at(&grid, 1200, x, y), [0xB8, 0xBE, 0xC7], "({x}, {y})");
            assert!(
                on_grid(x, ox) && on_grid(y, oy),
                "a dot off the grid at ({x}, {y})"
            );
            dots += 1;
        }
    }
    assert!(dots > 1000, "{dots} dots");

    // Fitted: the surround outside the sheet and the 1-pixel page bounds where the view says.
    let fitted = &steps[9]["state"];
    let ((fx, fy), fr) = settled(fitted);
    let (fw, fh, fit) = decode_rgba(&std::fs::read(light.join("fit.png")).unwrap());
    assert_eq!((fw, fh), (1200, 800));
    let sheet_w = (297_000_000i64 * i64::from(fr) + 12_700_000) / 25_400_000;
    let sheet_h = (210_000_000i64 * i64::from(fr) + 12_700_000) / 25_400_000;
    assert!(
        fx > 1 && fy > 1 && fx + sheet_w < 1199 && fy + sheet_h < 799,
        "the whole sheet is in view"
    );
    assert_eq!(rgb_at(&fit, 1200, 0, 0), [0xE3, 0xE6, 0xEA], "the light surround");
    // Doc 16a §4.6 (WP-S2.17 part 5): the page bound in `border.default`.
    let border = [0xD5, 0xD9, 0xDF];
    assert_eq!(
        rgb_at(&fit, 1200, fx - 1, fy + sheet_h / 2),
        border,
        "the left page bound"
    );
    assert_eq!(
        rgb_at(&fit, 1200, fx + sheet_w, fy + 3),
        border,
        "the right page bound"
    );
    assert_eq!(rgb_at(&fit, 1200, fx + 2, fy - 1), border, "the top page bound");
    assert_eq!(
        rgb_at(&fit, 1200, fx + 2, fy + sheet_h),
        border,
        "the bottom page bound"
    );
    assert_ne!(
        rgb_at(&fit, 1200, fx, fy + 1),
        border,
        "the sheet starts at the origin"
    );
    let describe = steps[11]["describe"].as_str().unwrap();
    assert!(describe.contains("canvas grid off\n"), "{describe}");
    assert!(describe.contains("canvas backend \"CPU\""));

    // The dark theme: the sheet's pixels are byte-identical, the surround differs (FR-UI-043's MVP part).
    let out = run_script(&tmp, "dark.jsonl", &canvas_script(&dir, "dark", &dark));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        std::fs::read(dark.join("plain.png")).unwrap(),
        std::fs::read(light.join("plain.png")).unwrap(),
        "the sheet is the same in both themes"
    );
    let (_, _, fit_dark) = decode_rgba(&std::fs::read(dark.join("fit.png")).unwrap());
    assert_eq!(
        rgb_at(&fit_dark, 1200, 0, 0),
        [0x2E, 0x32, 0x38],
        "the dark surround (doc 16a canvas.surround)"
    );
    for y in fy..fy + sheet_h {
        for x in fx..fx + sheet_w {
            assert_eq!(
                rgb_at(&fit_dark, 1200, x, y),
                rgb_at(&fit, 1200, x, y),
                "sheet pixel ({x}, {y})"
            );
        }
    }
}

/// A8: the canvas member and the describe lines through the binary; the pointer at an entity's centre hovers it
/// (or a smaller entity there).
#[test]
fn a8_the_pointer_at_an_entity_hovers_it_in_the_transcript_and_the_describe_text() {
    let tmp = Tmp::new();
    let dir = tmp.golden("a8");
    // An entity of folio 103 and its rectangle, from the project itself.
    let (session, _) = Session::open_read_only(&dir, "test").unwrap();
    let pages = session
        .export_pages(&ExportOptions {
            pages: PageSelection::Folios(vec!["103".into()]),
            ..ExportOptions::default()
        })
        .unwrap();
    let entities = pages.pages[0].content.entity_bounds();
    drop(session);
    let (entity, rect) = entities[entities.len() / 2];
    let (x, y) = rect_centre(rect);
    let (xm, ym) = (x as f64 / 1e6, y as f64 / 1e6);
    let out = run_script(
        &tmp,
        "a8.jsonl",
        &script(&[
            ("ui.describe", json!({})),
            (
                "file.open",
                json!({ "dir": dir.to_string_lossy(), "read_only": true }),
            ),
            ("page.goto", json!({"folio": "103"})),
            ("view.zoom_to", json!({"percent": 400, "x_mm": xm, "y_mm": ym})),
            ("canvas.pointer", json!({"x_mm": xm, "y_mm": ym})),
            ("view.frame_stats", json!({})),
            ("ui.describe", json!({})),
            ("canvas.pointer", json!({})),
            ("file.close", json!({})),
        ]),
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let steps = transcript(&out);
    assert_eq!(
        steps[1]["state"]["canvas"],
        Value::Null,
        "no page before the open"
    );
    let pointed = &steps[5]["state"]["canvas"];
    assert_eq!(pointed["pointer_nm"], json!([x, y]));
    let hover = pointed["hover"]
        .as_str()
        .expect("an entity is hovered")
        .to_owned();
    // The entity as its alias `@n`, never its UUID (FT-422, ID-630); the describe line names it with its box, which
    // identifies the entity of the page.
    assert!(ew_ui::ew_viewmodel::alias::is_alias(&hover), "{hover}");
    let describe = steps[7]["describe"].as_str().unwrap();
    let line = describe
        .lines()
        .find(|l| l.starts_with(&format!("canvas hover {hover} box ")))
        .unwrap_or_else(|| panic!("{describe}"));
    let mm3 = ew_ui::ew_viewmodel::canvas::mm3;
    let hovered = entities
        .iter()
        .find(|(_, r)| {
            line.ends_with(&format!(
                "box {} {} {} {} mm",
                mm3(r.min().x.0),
                mm3(r.min().y.0),
                mm3(r.max().x.0),
                mm3(r.max().y.0)
            ))
        })
        .expect("the hover is an entity of the page");
    let area = |r: &Rect| i128::from(r.max().x.0 - r.min().x.0) * i128::from(r.max().y.0 - r.min().y.0);
    assert!(area(&hovered.1) <= area(&rect), "{hover} vs {}", entity.0);
    assert!(describe.contains("canvas statistics shown"), "{describe}");
    assert!(describe.contains("canvas zoom 400.0 % view "), "{describe}");
    // The member's order in the step's own bytes.
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    let line = stdout.lines().nth(5).unwrap();
    let canvas = &line[line.find("\"canvas\":{").unwrap()..];
    let keys = [
        "folio",
        "zoom",
        "view_nm",
        "grid",
        "pointer_nm",
        "hover",
        "backend",
        "statistics",
        "renderer",
    ];
    let at: Vec<usize> = keys
        .iter()
        .map(|k| canvas.find(&format!("\"{k}\"")).unwrap())
        .collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{canvas}");
    assert_eq!(steps[8]["state"]["canvas"]["hover"], Value::Null);
}

fn view(state: &Value) -> Value {
    state["canvas"]["view_nm"].clone()
}

/// The side-by-side script of V8 on `dir`, snapshots into `out`.
fn side_by_side_script(dir: &Path, out: &Path) -> String {
    let p = |name: &str| out.join(name).to_string_lossy().into_owned();
    script(&[
        (
            "file.open",
            json!({ "dir": dir.to_string_lossy(), "read_only": true }),
        ),
        ("page.goto", json!({"folio": "102"})),
        ("view.side_by_side", json!({})),
        ("view.zoom_to", json!({"percent": 150, "group": 0})),
        ("view.pan", json!({"dx": 30, "dy": 10, "group": 0})),
        ("view.zoom_to", json!({"percent": 300, "group": 1})),
        ("canvas.pointer", json!({"x_mm": 120, "y_mm": 90, "group": 0})),
        ("canvas.pointer", json!({"x_mm": 60, "y_mm": 40, "group": 1})),
        ("tab.focus_other", json!({})),
        ("tab.new", json!({})),
        ("page.goto", json!({"folio": "106"})),
        ("tab.previous", json!({})),
        ("nav.goto", json!({"text": "102-C15"})),
        ("nav.back", json!({})),
        ("canvas.snapshot", json!({"path": p("left.png"), "group": 0})),
        ("canvas.snapshot", json!({"path": p("right.png"), "group": 1})),
        ("tab.focus_other", json!({})),
        ("ui.describe", json!({})),
        ("file.close", json!({})),
    ])
}

/// V8 (FR-UI-035): 102 left and 103 right with their own zooms, pans and pointers; tab switches and Back keep each
/// tab's view; a snapshot per group.
#[test]
fn v8_side_by_side_headless() {
    let tmp = Tmp::new();
    let dir = tmp.golden("v8");
    let out_dir = tmp.path("v8out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = run_script(&tmp, "v8.jsonl", &side_by_side_script(&dir, &out_dir));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let steps = transcript(&out);
    let right = &steps[8]["state"];
    assert_eq!(right["page"], "103");
    assert_eq!(right["canvas"]["zoom"], 300.0);
    assert_eq!(right["canvas"]["pointer_nm"], json!([60_000_000, 40_000_000]));
    let left = &steps[9]["state"];
    assert_eq!(left["page"], "102");
    assert_eq!(left["canvas"]["zoom"], 150.0);
    assert_eq!(
        left["canvas"]["pointer_nm"],
        Value::Null,
        "a pointer on one group clears the other's"
    );
    let left_view = view(left);
    // A new tab, another page, back to the first tab: its view is kept; Go to and Back restore it.
    assert_eq!(view(&steps[12]["state"]), left_view);
    assert_eq!(view(&steps[14]["state"]), left_view);
    assert_eq!(steps[14]["state"]["tabs"]["groups"].as_array().unwrap().len(), 2);
    // The right group kept its own view.
    assert_eq!(steps[17]["state"]["canvas"]["zoom"], 300.0);
    let describe = steps[18]["describe"].as_str().unwrap();
    assert!(describe.contains("tabs group 1 of 2\n"), "{describe}");
    assert!(describe.contains("tabs group 2 of 2 focused\n"), "{describe}");
    let (lw, lh, l) = decode_rgba(&std::fs::read(out_dir.join("left.png")).unwrap());
    let (rw, rh, r) = decode_rgba(&std::fs::read(out_dir.join("right.png")).unwrap());
    assert_eq!((lw, lh, rw, rh), (1200, 800, 1200, 800));
    assert_ne!(l, r);
}

/// The path of the `elektrawatt` binary under test. nextest sets `NEXTEST_BIN_EXE_elektrawatt` at run time (the binaries of a
/// `--archive-file` run are unpacked elsewhere than the compile-time path, FT-353); a plain `cargo test` falls back
/// to the compile-time path.
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> std::path::PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

// -----------------------------------------------------------------------------------------------------------------
// The tool framework (WP-S2.03 part 2)

/// How many pixels of `rgba` have exactly the colour `c`.
fn count(rgba: &[u8], c: [u8; 3]) -> usize {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] == c[0] && p[1] == c[1] && p[2] == c[2])
        .count()
}

// Doc 16a §4.6 (WP-S2.17 part 5): the measure line in slate-900; the snap markers, the grid cursor and the rubber
// band in the one accent.
const MEASURE: [u8; 3] = [0x1A, 0x1D, 0x21];
const ACCENT: [u8; 3] = [0x0B, 0x63, 0xCE];

/// Points of `folio` read through an in-process shell as the viewer reads them: a point with no entity and no link,
/// and the centre of a link to another page.
fn points(dir: &Path, folio: &str) -> ((f64, f64), (f64, f64)) {
    let config = std::env::temp_dir().join(format!(
        "ew-tools-points-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut shell = ShellModel::new(Mode::Headless, ShellConfig::in_dir(&config, "0.0.0-test"));
    let open = json!({"dir": dir.to_string_lossy(), "read_only": true});
    assert_eq!(shell.dispatch("file.open", &open), Outcome::Done);
    assert_eq!(
        shell.dispatch("page.goto", &json!({ "folio": folio })),
        Outcome::Done
    );
    let c = shell.canvas().unwrap();
    let slop = HOVER_SLOP_PT / c.points_per_nm();
    let (w, h) = c.sheet();
    let mut empty = None;
    'scan: for y in (5_000_000..h).step_by(3_000_000) {
        for x in (5_000_000..w).step_by(3_000_000) {
            if pick(&c.page().entities, Some((x, y)), slop).is_none() && c.link_at((x, y)).is_none() {
                empty = Some((x, y));
                break 'scan;
            }
        }
    }
    let link = c
        .page()
        .links
        .iter()
        .find(|l| {
            l.target
                .as_ref()
                .is_some_and(|t| t.folio != folio && t.at_nm.is_some())
        })
        .map(|l| rect_centre(l.rect))
        .expect("a link to another page");
    let mm = |p: (i64, i64)| (p.0 as f64 / 1e6, p.1 as f64 / 1e6);
    let out = (mm(empty.expect("an empty point")), mm(link));
    shell.shutdown();
    let _ = std::fs::remove_dir_all(&config);
    out
}

/// T7: the snapshot composites the Measure line, its snap markers, the grid cursor and a rubber band; twice the
/// same bytes; labels never.
#[test]
fn t7_the_snapshot_composites_the_tool_overlay() {
    let tmp = Tmp::new();
    let dir = tmp.golden("t7");
    let (empty, _) = points(&dir, "102");
    let out = tmp.path("t7out");
    std::fs::create_dir_all(&out).unwrap();
    let p = |name: &str| out.join(name).to_string_lossy().into_owned();
    let text = script(&[
        (
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": true}),
        ),
        ("page.goto", json!({"folio": "102"})),
        ("view.zoom_to", json!({"percent": 200})),
        ("canvas.snapshot", json!({"path": p("plain.png")})),
        ("tool.measure", json!({})),
        ("cp.next", json!({})),
        ("canvas.activate", json!({})),
        ("cp.next", json!({})),
        ("cp.next", json!({})),
        ("canvas.activate", json!({})),
        ("canvas.snapshot", json!({"path": p("measure.png")})),
        // R1-8: the snap markers and the cursor share the accent; the same overlay with the cursor hidden tells them
        // apart.
        ("cursor.set", json!({})),
        ("canvas.snapshot", json!({"path": p("markers.png")})),
        ("ui.describe", json!({})),
        ("ui.dismiss", json!({})),
        ("cursor.set", json!({})),
        ("view.fit_page", json!({})),
        ("canvas.press", json!({"x_mm": empty.0, "y_mm": empty.1})),
        (
            "canvas.pointer",
            json!({"x_mm": empty.0 + 25.0, "y_mm": empty.1 + 15.0}),
        ),
        ("canvas.snapshot", json!({"path": p("band.png")})),
        ("ui.dismiss", json!({})),
        ("file.close", json!({})),
    ]);
    let first = run_script(&tmp, "t7.jsonl", &text);
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    let steps = transcript(&first);
    let describe = steps
        .iter()
        .find_map(|s| s["describe"].as_str())
        .unwrap()
        .to_owned();
    assert!(describe.contains("canvas overlay 4 label \"dx "), "{describe}");
    let read = |name: &str| std::fs::read(out.join(name)).unwrap();
    let (plain, measure, band) = (read("plain.png"), read("measure.png"), read("band.png"));
    let markers = read("markers.png");
    let again = run_script(&tmp, "t7.jsonl", &text);
    assert_eq!(again.status.code(), Some(0));
    assert_eq!(read("measure.png"), measure, "byte-identical");
    assert_eq!(read("band.png"), band, "byte-identical");
    let (_, _, plain) = decode_rgba(&plain);
    let (_, _, measure) = decode_rgba(&measure);
    let (_, _, band) = decode_rgba(&band);
    let (_, _, markers) = decode_rgba(&markers);
    assert_eq!(count(&plain, MEASURE), 0);
    assert_eq!(count(&plain, ACCENT), 0);
    assert!(count(&measure, MEASURE) > 20, "the measure line");
    // The two snap markers alone (the cursor hidden; the measure line stays, with the pixels the cursor covered),
    // then the cursor's own pixels.
    assert!(
        count(&markers, MEASURE) >= count(&measure, MEASURE),
        "the measure line stays"
    );
    assert!(count(&markers, ACCENT) >= 32, "the snap markers");
    assert!(
        count(&measure, ACCENT) >= count(&markers, ACCENT) + 20,
        "the grid cursor"
    );
    assert!(
        count(&band, ACCENT) > count(&plain, ACCENT) + 100,
        "the rubber band"
    );
    assert_eq!(
        count(&band, MEASURE),
        0,
        "the band's snapshot has no measure line"
    );
}

/// T8: `canvas.preview_snapshot` writes the link preview (320 × 200 points), byte-identical twice; refused without
/// a preview.
#[test]
fn t8_the_preview_snapshot_is_deterministic() {
    let tmp = Tmp::new();
    let dir = tmp.golden("t8");
    let (_, link) = points(&dir, "101");
    let out = tmp.path("t8out");
    std::fs::create_dir_all(&out).unwrap();
    let path = out.join("preview.png").to_string_lossy().into_owned();
    let text = script(&[
        (
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": true}),
        ),
        ("page.goto", json!({"folio": "101"})),
        ("canvas.pointer", json!({"x_mm": link.0, "y_mm": link.1})),
        ("canvas.dwell", json!({"ms": 300})),
        ("canvas.preview_snapshot", json!({ "path": path })),
        ("ui.describe", json!({})),
        ("file.close", json!({})),
    ]);
    let first = run_script(&tmp, "t8.jsonl", &text);
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    let steps = transcript(&first);
    // (Line 0 of a transcript is its header: step n is line n.)
    let folio = steps[5]["state"]["canvas"]["link_preview"]
        .as_str()
        .expect("the preview's folio")
        .to_owned();
    assert!(
        steps[6]["describe"]
            .as_str()
            .unwrap()
            .contains(&format!("canvas link preview \"{folio}\""))
    );
    let bytes = std::fs::read(&path).unwrap();
    let (w, h, rgba) = decode_rgba(&bytes);
    assert_eq!((w, h), (320, 200));
    assert!(count(&rgba, ACCENT) >= 20, "the cursor crosshair at the target");
    let again = run_script(&tmp, "t8.jsonl", &text);
    assert_eq!(again.status.code(), Some(0));
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "byte-identical");
    // Without a preview the step is refused.
    let refused = run_script(
        &tmp,
        "t8b.jsonl",
        &script(&[
            (
                "file.open",
                json!({"dir": dir.to_string_lossy(), "read_only": true}),
            ),
            ("canvas.preview_snapshot", json!({ "path": path })),
        ]),
    );
    assert_eq!(refused.status.code(), Some(1));
    let steps = transcript(&refused);
    assert_eq!(steps[2]["reason"], "reason-no-preview");
}

/// WP-S2.04 part 1 through the built binary (acceptance check PL7 and part 1's share of §13.6): folio 101's
/// representations placed by a UI script generated from Golden A's records on a new project made by the
/// company-standard wizard, with the oracle of §13.6 (17 representations, each the golden symbol at the golden
/// insertion point and orientation), and the `--record` log replayed by the session's command-log replay (what
/// `ew apply` runs) giving equal records. Each test runs its script once per gate; the second run and the
/// byte-identical comparison of the two transcripts the docs below name are the deep nightly's
/// (`headless::nightly_every_script_transcript_is_byte_identical_twice`). The bodies are `canvas/folio_scripts.rs`.
mod folio_scripts {

    #[test]
    fn folio_101_is_placed_by_a_generated_script_twice_identically_and_replays() {
        super::folio_cases::folio_101_is_placed_by_a_generated_script_twice_identically_and_replays_case(
            false,
        );
    }

    /// WP-S2.04 part 2's share of §13.6 (WR9): folio 101 placed, its golden tags typed (`fields.set` through `command.run`
    /// on `tag:` references), its 21 connections and 2 junctions drawn with the Wire tool from the keyboard; two runs give
    /// byte-identical transcripts; the oracle: 17 representations, 21 drawn connections and 2 junctions, every
    /// connection's end labels and every junction's sequence (by far-end labels) as on Golden A; the `--record` log
    /// replayed on the start gives the same wiring and the same devices and functions, IDs aside.
    #[test]
    fn folio_101_is_wired_by_a_generated_script_twice_identically_and_replays() {
        super::folio_cases::folio_101_is_wired_by_a_generated_script_twice_identically_and_replays_case(
            false,
        );
    }

    /// WP-S2.04 part 3's share of §13.6 (IB7): folio 102 placed by a generated script: its nine interruption points
    /// with the IP tool, its two black boxes (-102G1, -102F2) with the black box tool from their parts and their CPs
    /// dragged along the outline to the golden points (Select's CP drag, `box.reshape`), its contact with the Place
    /// tool. Two runs give byte-identical transcripts without a UUID; the oracle: 12 representations, each the golden
    /// symbol at the golden point and orientation, the interruption points with their names and directions, and both
    /// boxes' outlines and CPs (points and directions) as on Golden A; the `--record` log replayed on the start (the
    /// minted representation of a `box.reshape` mapped by its point) gives the same records.
    #[test]
    fn folio_102_ips_and_black_boxes_are_placed_by_a_generated_script_twice_identically_and_replays() {
        super::folio_cases::folio_102_ips_and_black_boxes_are_placed_by_a_generated_script_twice_identically_and_replays_case(false);
    }

    /// WP-S2.04 part 4b's share of §13.6 (PI9): folio 101's structure box around the three external connection
    /// points and its two marker groups (W4/W5 and W7/W9, 2.5 BK BU) appended to the wiring script of
    /// `folio_101_is_wired_by_a_generated_script_twice_identically_and_replays`; two runs give byte-identical
    /// transcripts without a UUID; the oracle: one structure box with the caption "External Connection
    /// (+J-E-206 / JE-N1000-03)", two wire markers "2.5 BK BU", no marker fallback, and the wiring oracle unchanged;
    /// the `--record` log replayed on a third copy (each `connection.draw` and each `marker.place` mapped) gives the
    /// same box outline, caption and marker texts and the same records.
    #[test]
    fn folio_101_structure_box_and_markers_are_drawn_by_a_generated_script_twice_identically_and_replay() {
        super::folio_cases::folio_101_structure_box_and_markers_are_drawn_by_a_generated_script_twice_identically_and_replay_case(false);
    }
}
