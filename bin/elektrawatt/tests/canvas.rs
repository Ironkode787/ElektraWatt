// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The headless canvas through the built binary (WP-S2.02 part 1, acceptance checks A6, A7 and A8): no window, no
//! GPU, no display. Every test works on a temporary copy of Golden A with `EW_CONFIG_DIR` in a temporary
//! directory; nothing is written into the repository.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::canvas::rect_centre;
use ew_ui::ew_viewmodel::ew_api::Session;
use ew_ui::ew_viewmodel::ew_api::output::ew_dl::ew_units::Rect;
use ew_ui::ew_viewmodel::ew_api::output::{ExportOptions, PageSelection};
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
            assert_eq!(rgb_at(&grid, 1200, x, y), [0x9E, 0x9E, 0x9E], "({x}, {y})");
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
    assert_eq!(rgb_at(&fit, 1200, 0, 0), [0xD4, 0xD4, 0xD4], "the light surround");
    let border = [0x8C, 0x8C, 0x8C];
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
        [0x2B, 0x2B, 0x2B],
        "the dark surround"
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

/// A7: the canvas script twice gives byte-identical transcripts and snapshots.
#[test]
fn a7_the_canvas_script_is_deterministic() {
    let run = |tmp: &Tmp| {
        let dir = tmp.golden("g");
        let out_dir = tmp.path("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        let out = run_script(tmp, "c.jsonl", &canvas_script(&dir, "system", &out_dir));
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let pngs: Vec<Vec<u8>> = ["grid.png", "plain.png", "fit.png"]
            .iter()
            .map(|n| std::fs::read(out_dir.join(n)).unwrap())
            .collect();
        (out.stdout, pngs)
    };
    let (tmp_a, tmp_b) = (Tmp::new(), Tmp::new());
    let (a, pa) = run(&tmp_a);
    let (b, pb) = run(&tmp_b);
    assert_eq!(a, b, "the transcripts differ");
    assert_eq!(pa, pb, "the snapshots differ");
    let text = String::from_utf8(a).unwrap();
    assert!(
        !text.contains(&*tmp_a.0.to_string_lossy()),
        "a path in the transcript"
    );
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
    let hovered = entities
        .iter()
        .find(|(e, _)| e.0.to_string() == hover)
        .expect("the hover is an entity of the page");
    let area = |r: &Rect| i128::from(r.max().x.0 - r.min().x.0) * i128::from(r.max().y.0 - r.min().y.0);
    assert!(area(&hovered.1) <= area(&rect), "{hover} vs {}", entity.0);
    let describe = steps[7]["describe"].as_str().unwrap();
    assert!(
        describe.contains(&format!("canvas hover \"{hover}\" box ")),
        "{describe}"
    );
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
