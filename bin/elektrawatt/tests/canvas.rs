// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The headless canvas through the built binary (WP-S2.02 part 1, acceptance checks A6, A7 and A8; the viewer of
//! WP-S2.03 part 1, V6 to V8 and V12; the tool framework of part 2, T7, T8 and T10): no window, no GPU, no display.
//! Every test works on a temporary copy of Golden A with `EW_CONFIG_DIR` in a temporary directory; nothing is
//! written into the repository.

mod support;

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

/// The centre (mm) of the cross-reference link on `folio` whose text sits in `source` cell and leads to `target`,
/// read through the session as the viewer reads it (WP-S2.03 §6.4).
fn link_centre(dir: &Path, folio: &str, source: &str, target: &str) -> (f64, f64) {
    let (s, _) = Session::open_read_only(dir, "0.0.0-test").unwrap();
    let list = s
        .export_pages(&ExportOptions {
            pages: PageSelection::Folios(vec![folio.to_owned()]),
            ..ExportOptions::default()
        })
        .unwrap()
        .pages
        .remove(0)
        .content;
    let cells = s.page_cells(folio).unwrap();
    let sources = list.link_sources();
    let dests: Vec<_> = sources.iter().map(|(d, _)| *d).collect();
    let targets = s.link_targets(&dests);
    let found = sources
        .iter()
        .zip(&targets)
        .find_map(|((_, r), t)| {
            let c = rect_centre(*r);
            let here = cells.cell_at(c).map(|cell| cells.text(&cell));
            let to = t.as_ref().and_then(|t| t.cell.clone());
            (here.as_deref() == Some(source) && to.as_deref() == Some(target)).then_some(c)
        })
        .unwrap_or_else(|| panic!("a link from {source} to {target}"));
    s.close();
    (found.0 as f64 / 1e6, found.1 as f64 / 1e6)
}

fn view(state: &Value) -> Value {
    state["canvas"]["view_nm"].clone()
}

/// The follow-and-back script of V6 on `dir`.
fn follow_script(dir: &Path) -> String {
    let (x, y) = link_centre(dir, "102", "102-H15", "103-A1");
    let (lx, ly) = link_centre(dir, "101", "101-H16", "102-A1");
    script(&[
        (
            "file.open",
            json!({ "dir": dir.to_string_lossy(), "read_only": true }),
        ),
        ("page.goto", json!({"folio": "102"})),
        ("view.zoom_to", json!({"percent": 200})),
        ("view.pan", json!({"dx": 40, "dy": -25})),
        ("nav.follow", json!({"x_mm": x, "y_mm": y})),
        ("nav.back", json!({})),
        ("nav.forward", json!({})),
        ("page.goto", json!({"folio": "101"})),
        ("nav.follow", json!({"x_mm": lx, "y_mm": ly})),
        ("nav.back", json!({})),
        ("ui.describe", json!({})),
        ("file.close", json!({})),
    ])
}

fn nm4(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_i64().unwrap())
        .collect()
}

/// V6 (FR-UI-019, AC-28's first part): follow "103-A1" at 24VDC-1 on 102, Back with the exact view, Forward; and
/// "102-A1" from 101-H16.
#[test]
fn v6_follow_back_and_forward_headless() {
    let tmp = Tmp::new();
    let dir = tmp.golden("v6");
    let out = run_script(&tmp, "v6.jsonl", &follow_script(&dir));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let steps = transcript(&out);
    let panned = &steps[4]["state"];
    let followed = &steps[5]["state"];
    assert_eq!(followed["page"], "103");
    assert_eq!(followed["canvas"]["cell"], "103-A1");
    let v = nm4(&view(followed));
    let c = nm4(&followed["canvas"]["cursor_nm"]);
    assert!(
        ((v[0] + v[2]) / 2 - c[0]).abs() <= 1 && ((v[1] + v[3]) / 2 - c[1]).abs() <= 1,
        "the target at the centre: {v:?} {c:?}"
    );
    // page.goto 102 was a jump too (from folio 1), the follow the second.
    assert_eq!(followed["tabs"]["groups"][0][0]["back"], 2);
    let back = &steps[6]["state"];
    assert_eq!((back["page"].clone(), view(back)), (json!("102"), view(panned)));
    assert_eq!(back["tabs"]["groups"][0][0]["forward"], 1);
    assert_eq!(steps[7]["state"]["page"], "103");
    assert_eq!(steps[9]["state"]["page"], "102");
    assert_eq!(steps[9]["state"]["canvas"]["cell"], "102-A1");
    assert_eq!(steps[10]["state"]["page"], "101");
}

/// V7 (FR-UI-039): Go to a cell centres it, a folio arrives, the errors, and "/103.A1" after the EPLAN preset.
#[test]
fn v7_go_to_page_or_cell_headless() {
    let tmp = Tmp::new();
    let dir = tmp.golden("v7");
    let preset = json!({"cmd": "project.settings.set", "params": {"member": "xref_format", "value": {
        "key": "eplan-cell", "ip": "/{page}.{row}{column}", "aux_to_main": "(/{page}.{row}{column})",
        "main_to_aux": "∞ {list}", "structure": "when_different", "same_page": "full",
        "matrix": {"per_row": 3, "max": 9, "overflow": "…"}}}});
    let out = run_script(
        &tmp,
        "v7.jsonl",
        &script(&[
            ("file.open", json!({ "dir": dir.to_string_lossy() })),
            ("nav.goto", json!({"text": "103-A1"})),
            ("nav.goto", json!({"text": "101"})),
            ("view.fit_page", json!({})),
            ("command.run", preset),
            ("nav.goto", json!({"text": "/103.A1"})),
            ("app.quit", json!({"discard": true})),
        ]),
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let steps = transcript(&out);
    let a1 = &steps[2]["state"];
    assert_eq!(
        (a1["page"].clone(), a1["canvas"]["cell"].clone()),
        (json!("103"), json!("103-A1"))
    );
    let v = nm4(&view(a1));
    let c = nm4(&a1["canvas"]["cursor_nm"]);
    assert!(
        ((v[0] + v[2]) / 2 - c[0]).abs() < 3_000_000,
        "A1's centre at the view centre"
    );
    // A folio arrives: 101 is seen for the first time in the tab, so it is fitted, without a cursor.
    let p101 = &steps[3]["state"];
    assert_eq!(
        (p101["page"].clone(), p101["canvas"]["cursor_nm"].clone()),
        (json!("101"), Value::Null)
    );
    // Fitted: Home changes nothing.
    assert_eq!(view(p101), view(&steps[4]["state"]));
    assert_eq!(p101["canvas"]["zoom"], steps[4]["state"]["canvas"]["zoom"]);
    let eplan = &steps[6]["state"];
    assert_eq!(eplan["page"], "103");
    assert_eq!(eplan["canvas"]["cell"], "/103.A1");
    for (text, message) in [
        ("", "Type a folio or a cell."),
        ("999-A1", "No page or cell matches \"999-A1\"."),
        ("103-Z9", "Page 103 has no cell Z9."),
    ] {
        let out = run_script(
            &tmp,
            "v7e.jsonl",
            &script(&[
                (
                    "file.open",
                    json!({ "dir": dir.to_string_lossy(), "read_only": true }),
                ),
                ("nav.goto", json!({ "text": text })),
            ]),
        );
        assert_eq!(out.status.code(), Some(1));
        let steps = transcript(&out);
        assert_eq!(steps[2]["outcome"], "failed");
        assert_eq!(steps[2]["message"], message);
    }
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

/// V12: the scripts of V6 and V8 give byte-identical transcripts (and snapshots) on two runs.
#[test]
fn v12_the_viewer_transcripts_are_deterministic() {
    let tmp = Tmp::new();
    let dir = tmp.golden("v12");
    let run_twice = |name: &str, text: &str| {
        let a = run_script(&tmp, name, text);
        let b = run_script(&tmp, name, text);
        assert_eq!(a.status.code(), Some(0), "{}", stderr(&a));
        assert_eq!(a.stdout, b.stdout, "{name}");
    };
    run_twice("v6.jsonl", &follow_script(&dir));
    let out = tmp.path("v12out");
    std::fs::create_dir_all(&out).unwrap();
    run_twice("v8.jsonl", &side_by_side_script(&dir, &out));
    let first = std::fs::read(out.join("left.png")).unwrap();
    let again = run_script(&tmp, "v8.jsonl", &side_by_side_script(&dir, &out));
    assert_eq!(again.status.code(), Some(0));
    assert_eq!(
        std::fs::read(out.join("left.png")).unwrap(),
        first,
        "snapshots are byte-identical"
    );
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

/// T10: a Select-and-Measure script twice gives byte-identical transcripts with the tool members.
#[test]
fn t10_a_select_and_measure_script_is_deterministic() {
    let tmp = Tmp::new();
    let dir = tmp.golden("t10");
    let (empty, link) = points(&dir, "102");
    let text = script(&[
        (
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": true}),
        ),
        ("page.goto", json!({"folio": "102"})),
        ("canvas.press", json!({"x_mm": empty.0, "y_mm": empty.1})),
        (
            "canvas.pointer",
            json!({"x_mm": empty.0 + 40.0, "y_mm": empty.1 + 30.0}),
        ),
        (
            "canvas.release",
            json!({"x_mm": empty.0 + 40.0, "y_mm": empty.1 + 30.0, "shift": true}),
        ),
        ("ui.describe", json!({})),
        ("canvas.pointer", json!({"x_mm": link.0, "y_mm": link.1})),
        ("canvas.dwell", json!({"ms": 80})),
        ("canvas.dwell", json!({"ms": 300})),
        ("ui.describe", json!({})),
        ("tool.measure", json!({})),
        ("cp.next_unconnected", json!({})),
        ("canvas.activate", json!({})),
        ("cp.next", json!({})),
        (
            "canvas.pointer",
            json!({"x_mm": empty.0, "y_mm": empty.1, "alt": true}),
        ),
        ("canvas.activate", json!({})),
        ("ui.describe", json!({})),
        ("tool.back", json!({})),
        ("ui.dismiss", json!({})),
        ("ui.dismiss", json!({})),
        ("ui.describe", json!({})),
        ("file.close", json!({})),
    ]);
    let a = run_script(&tmp, "t10.jsonl", &text);
    let b = run_script(&tmp, "t10.jsonl", &text);
    assert_eq!(a.status.code(), Some(0), "{}", stderr(&a));
    assert_eq!(a.stdout, b.stdout, "byte-identical transcripts");
    let steps = transcript(&a);
    // (Line 0 of a transcript is its header: step n is line n.)
    let canvas = |n: usize| steps[n]["state"]["canvas"].clone();
    assert_eq!(canvas(6)["tool"], json!({"name": "select", "state": "idle"}));
    assert!(canvas(6)["selection"].as_array().is_some_and(|s| !s.is_empty()));
    assert!(canvas(9)["link_preview"].is_string());
    assert_eq!(canvas(14)["tool"]["state"], "measuring");
    assert_eq!(canvas(14)["snap"]["kind"], "cp");
    assert_eq!(canvas(15)["snap"]["kind"], "grid", "Alt snaps to the grid");
    assert_eq!(canvas(17)["tool"]["state"], "armed");
    assert_eq!(canvas(17)["overlay"], 4);
    assert_eq!(canvas(21)["tool"]["name"], "select");
    assert_eq!(canvas(21)["selection"], json!([]), "Escape cleared the selection");
    let text = steps[17]["describe"].as_str().unwrap();
    assert!(text.contains("canvas tool \"Measure\" state armed"), "{text}");
    assert!(text.contains("canvas overlay 4 label \"dx "), "{text}");
}

/// WP-S2.04 part 1 through the built binary (acceptance check PL7 and part 1's share of §13.6): folio 101's
/// representations placed by a UI script generated from Golden A's records on a new project made by the
/// company-standard wizard, with the oracle of §13.6 (17 representations, each the golden symbol at the golden
/// insertion point and orientation), two runs giving byte-identical transcripts, and the `--record` log replayed
/// by the session's command-log replay (what `ew apply` runs) giving equal records.
mod folio_scripts {
    use std::collections::BTreeMap;
    use std::path::Path;

    use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
    use serde_json::{Value, json};

    use super::bin_exe;
    use super::support::{
        SheetMap, Tmp, all_done, copy_dir, golden_reps, path, placement_steps, records_of, reps_of, run,
        start, steps,
    };

    #[test]
    fn folio_101_is_placed_by_a_generated_script_twice_identically_and_replays() {
        let tmp = Tmp::new();
        let job = start(&bin_exe(), &tmp);
        // The page's sheet map and the strip's ID, read from the start project.
        let (session, _) = Session::open_read_only(&job, "test").unwrap();
        let grid = session.page_grid("101").expect("page 101");
        let map = SheetMap {
            scale_ppb: i128::from(grid.sheet_scale_ppb),
            offset: (
                i128::from(grid.sheet_offset_nm.0),
                i128::from(grid.sheet_offset_nm.1),
            ),
        };
        let q = session.query();
        let strip = q
            .devices()
            .iter()
            .find(|(_, d)| d.aspects.strip.is_some())
            .map(|(id, _)| id.to_string())
            .expect("the start project's strip");
        drop(q);
        drop(session);
        let (placements, runs) = placement_steps(101, map, &strip, &BTreeMap::new());
        // Two copies of the start: two runs of the same script give byte-identical transcripts.
        let mut transcripts = Vec::new();
        let mut records = Vec::new();
        for n in ["a", "b"] {
            let dir = tmp.path(n).join("folio.ew");
            copy_dir(&job, &dir);
            let mut script = steps(&[
                ("file.open", json!({ "dir": path(&dir) })),
                ("page.goto", json!({"folio": "101"})),
            ]);
            script.extend(placements.iter().cloned());
            script.extend(steps(&[
                ("ui.describe", json!({})),
                ("file.save", json!({})),
                ("file.close", json!({})),
            ]));
            let record = tmp.path(&format!("{n}.ewcmd"));
            let out = run(&bin_exe(), &tmp, &format!("{n}.jsonl"), &script, Some(&record));
            all_done(&out);
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(
                !text.contains(&*tmp.dir().to_string_lossy()),
                "a path in the transcript"
            );
            transcripts.push(text);
            records.push(record);
        }
        if let Some((a, b)) = transcripts[0]
            .lines()
            .zip(transcripts[1].lines())
            .find(|(a, b)| a != b)
        {
            let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "two runs, two transcripts: first difference at {at}: {} | {}",
                &a[at.saturating_sub(120)..(at + 80).min(a.len())],
                &b[at.saturating_sub(120)..(at + 80).min(b.len())]
            );
        }
        assert_eq!(transcripts[0], transcripts[1], "two runs, one transcript");
        // FT-422: no UUID in any transcript member or describe text.
        super::support::assert_no_uuid(&transcripts[0]);
        // The oracle of §13.6: 17 representations, each the golden symbol at the golden point and orientation.
        let golden: Vec<(String, (i64, i64), u8)> = {
            let mut v: Vec<_> = golden_reps(101)
                .into_iter()
                .map(|r| (r.symbol, r.pos, r.orient))
                .collect();
            v.sort();
            v
        };
        assert_eq!(golden.len(), 17);
        let placed = reps_of(&tmp.path("a").join("folio.ew"));
        assert_eq!(placed.len(), 17, "representations on 101");
        assert_eq!(placed, golden);
        // No step left to a later part: the six interruption points are the IP tool's (part 3).
        assert_eq!(runs, 0);
        // IB7: the IP tool placed Golden A's six interruption points of 101 with their names, directions, symbols
        // and points, the three external connection points with their external references.
        let ip_rows = |t: &ew_ui::ew_viewmodel::ew_api::output::PageTables| {
            let mut v: Vec<_> = t
                .reps
                .iter()
                .filter_map(|r| {
                    r.ip.as_ref()
                        .map(|i| (i.name.clone(), i.direction.clone(), r.symbol_key.clone(), r.pos))
                })
                .collect();
            v.sort();
            v
        };
        let (golden_session, _) =
            Session::open_read_only(Path::new(super::support::GOLDEN_A), "test").unwrap();
        let (scripted_ips, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
        assert_eq!(
            ip_rows(&scripted_ips.page_tables("101").unwrap()),
            ip_rows(&golden_session.page_tables("101").unwrap())
        );
        let externals = scripted_ips
            .query()
            .functions()
            .iter()
            .filter(|(_, f)| serde_json::to_value(&***f).unwrap()["kind"]["ip"]["external"].is_string())
            .count();
        assert_eq!(
            externals, 3,
            "the external connection points name their references"
        );
        // `--record` replayed on a third copy of the start (the session's command-log replay, `ew apply`'s path) gives
        // the same representations.
        let replay = tmp.path("c").join("folio.ew");
        copy_dir(&job, &replay);
        let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
        let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names.iter().filter(|n| **n == "device.place").count(),
            11,
            "{names:?}"
        );
        assert_eq!(names.iter().filter(|n| **n == "ip.place").count(), 6);
        let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
        store.state_dir = Some(tmp.path("state"));
        let (mut s, _) = Session::open(
            &replay,
            store,
            ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
            ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
        )
        .unwrap();
        s.run_script(&log, "a.ewcmd").unwrap();
        let mut replayed: Vec<_> = s
            .page_tables("101")
            .unwrap()
            .reps
            .iter()
            .map(|r| (r.symbol_key.clone(), r.pos, r.orient))
            .collect();
        replayed.sort();
        assert_eq!(
            replayed, placed,
            "the replay's records equal the scripted session's"
        );
        // The devices (tags, parts, terminal aspects) and functions too, IDs aside (fix round, item 7).
        let (scripted, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
        let (devices, functions) = records_of(&scripted);
        assert_eq!(records_of(&s), (devices.clone(), functions.clone()));
        assert!(
            devices.iter().filter(|d| d.contains("\"terminal\":{")).count() >= 3,
            "the terminals"
        );
        assert!(functions.len() >= 17);
        s.close();
    }

    /// WP-S2.04 part 2's share of §13.6 (WR9): folio 101 placed, its golden tags typed (`fields.set` through `command.run`
    /// on `tag:` references), its 21 connections and 2 junctions drawn with the Wire tool from the keyboard; two runs give
    /// byte-identical transcripts; the oracle: 17 representations, 21 drawn connections and 2 junctions, every
    /// connection's end labels and every junction's sequence (by far-end labels) as on Golden A; the `--record` log
    /// replayed on the start gives the same wiring and the same devices and functions, IDs aside.
    #[test]
    fn folio_101_is_wired_by_a_generated_script_twice_identically_and_replays() {
        use super::support::{golden_wiring, typed_tag_steps, wiring_oracle, wiring_steps};
        let tmp = Tmp::new();
        let job = start(&bin_exe(), &tmp);
        let (session, _) = Session::open_read_only(&job, "test").unwrap();
        let grid = session.page_grid("101").expect("page 101");
        let map = SheetMap {
            scale_ppb: i128::from(grid.sheet_scale_ppb),
            offset: (
                i128::from(grid.sheet_offset_nm.0),
                i128::from(grid.sheet_offset_nm.1),
            ),
        };
        let q = session.query();
        let strip = q
            .devices()
            .iter()
            .find(|(_, d)| d.aspects.strip.is_some())
            .map(|(id, _)| id.to_string())
            .expect("the start project's strip");
        drop(q);
        drop(session);
        let (placements, runs) = placement_steps(101, map, &strip, &BTreeMap::new());
        // Phase A: the placements alone, to learn the tags they print.
        let placed = tmp.path("p").join("folio.ew");
        copy_dir(&job, &placed);
        let mut script = steps(&[
            ("file.open", json!({ "dir": path(&placed) })),
            ("page.goto", json!({"folio": "101"})),
        ]);
        script.extend(placements.iter().cloned());
        script.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
        all_done(&run(
            &bin_exe(),
            &tmp,
            "p.jsonl",
            &script,
            Some(&tmp.path("p.ewcmd")),
        ));
        let tags = typed_tag_steps(101, &placed);
        let (wiring, commits, _points) = wiring_steps(101, map);
        assert_eq!(commits, 19, "15 plain wires and 2 per junction");
        // The reference to the first device the tag steps type (a tag the placements print, no ID).
        let first_tag = tags[0].1["params"]["targets"][0]["ref"].clone();
        assert!(
            first_tag.as_str().is_some_and(|t| t.starts_with("tag:")),
            "{first_tag}"
        );
        // The full script, twice.
        let mut transcripts = Vec::new();
        let mut records = Vec::new();
        for n in ["a", "b"] {
            let dir = tmp.path(n).join("folio.ew");
            copy_dir(&job, &dir);
            let mut script = steps(&[
                ("file.open", json!({ "dir": path(&dir) })),
                ("page.goto", json!({"folio": "101"})),
            ]);
            script.extend(placements.iter().cloned());
            // FT-422: a device the script placed (its ID minted, so different in each run) selected and cleared: the
            // transcript names it by its alias, the same in both runs.
            script.extend(steps(&[
                ("select.set", json!({"refs": [first_tag.clone()]})),
                ("select.set", json!({"ids": []})),
            ]));
            script.extend(tags.iter().cloned());
            script.extend(wiring.iter().cloned());
            script.extend(steps(&[
                ("ui.describe", json!({})),
                ("file.save", json!({})),
                ("file.close", json!({})),
            ]));
            let record = tmp.path(&format!("{n}.ewcmd"));
            let out = run(&bin_exe(), &tmp, &format!("{n}.jsonl"), &script, Some(&record));
            all_done(&out);
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(
                !text.contains(&*tmp.dir().to_string_lossy()),
                "a path in the transcript"
            );
            transcripts.push(text);
            records.push(record);
        }
        if let Some((a, b)) = transcripts[0]
            .lines()
            .zip(transcripts[1].lines())
            .find(|(a, b)| a != b)
        {
            let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "two runs, two transcripts: first difference at {at}: {} | {}",
                &a[at.saturating_sub(120)..(at + 80).min(a.len())],
                &b[at.saturating_sub(120)..(at + 80).min(b.len())]
            );
        }
        assert_eq!(transcripts[0], transcripts[1], "two runs, one transcript");
        // FT-422: no UUID in any transcript member or describe text, and the minted device's alias in both runs.
        super::support::assert_no_uuid(&transcripts[0]);
        let selected: Vec<Value> = transcripts[0]
            .lines()
            .skip(1)
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .filter(|s| s["action"] == "select.set")
            .map(|s| s["state"]["canvas"]["selection"].clone())
            .collect();
        assert_eq!(selected.len(), 2, "{selected:?}");
        let alias = selected[0][0].as_str().unwrap_or_default();
        assert!(
            ew_ui::ew_viewmodel::alias::is_alias(alias),
            "the selection prints an alias: {selected:?}"
        );
        assert_eq!(selected[1], json!([]));
        // The oracle of §13.6.
        let golden = golden_wiring(101);
        assert_eq!(golden.counts, (21, 2));
        let (scripted, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
        let tables = scripted.page_tables("101").unwrap();
        assert_eq!(tables.reps.len(), 17, "representations on 101");
        let wired = wiring_oracle(&tables);
        assert_eq!(wired.counts, (21, 2), "drawn connections and junctions on 101");
        assert_eq!(wired.sequences, golden.sequences, "every junction sequence");
        assert_eq!(wired.pairs, golden.pairs, "every connection end labels");
        assert_eq!(wired.sequences, golden.sequences, "every junction's sequence");
        // Every drawn connection carries the route the Wire tool showed.
        assert!(tables.connections.iter().all(|c| c.stored_route));
        // The interruption points are the IP tool's (part 3); the typed tags stay `command.run` steps.
        assert_eq!(runs, 0);
        assert!(!tags.is_empty());
        // `--record` replayed on a third copy of the start.
        let replay = tmp.path("c").join("folio.ew");
        copy_dir(&job, &replay);
        let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
        let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names.iter().filter(|n| **n == "connection.draw").count(),
            19,
            "{names:?}"
        );
        let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
        store.state_dir = Some(tmp.path("state"));
        let (mut s, _) = Session::open(
            &replay,
            store,
            ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
            ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
        )
        .unwrap();
        // The log names the functions and connections the session minted (a new project's IDs are random, FT-416):
        // each `connection.draw` end is mapped to the replay's record at the same place before it runs (a CP by its
        // point and index, a wire by its connection point end), every other line replays as recorded.
        let a_tables = scripted.page_tables("101").unwrap();
        for cmd in log.commands() {
            let mut cmd = cmd.clone();
            if cmd.name == "connection.draw" {
                let now = s.page_tables("101").unwrap();
                let map_cp = |f: &str, idx: u64| -> String {
                    let at = a_tables
                        .cps
                        .iter()
                        .find(|c| c.function.to_string() == f && u64::from(c.idx) == idx)
                        .expect("a CP of the run")
                        .at;
                    now.cps
                        .iter()
                        .find(|c| c.at == at && u64::from(c.idx) == idx)
                        .expect("the CP of the replay")
                        .function
                        .to_string()
                };
                for end in ["a", "b"] {
                    let e = &mut cmd.params[end];
                    if let Some(cp) = e.get_mut("cp") {
                        let f = map_cp(cp["function"].as_str().unwrap(), cp["idx"].as_u64().unwrap());
                        cp["function"] = Value::from(f);
                    } else if let Some(on) = e.get_mut("on_connection") {
                        let id = on["connection"].as_str().unwrap().to_owned();
                        let w = a_tables
                            .connections
                            .iter()
                            .find(|c| c.id.to_string() == id)
                            .expect("the wire");
                        let ew_ui::ew_viewmodel::ew_api::output::TableEnd::Cp { function, idx } = w.a else {
                            panic!("the split wire starts at a CP");
                        };
                        let f = map_cp(&function.to_string(), u64::from(idx));
                        let mapped = now
                            .connections
                            .iter()
                            .find(|c| {
                                matches!(c.a, ew_ui::ew_viewmodel::ew_api::output::TableEnd::Cp { function, idx: i }
                                    if function.to_string() == f && i == idx)
                            })
                            .expect("the wire of the replay")
                            .id;
                        on["connection"] = Value::from(mapped.to_string());
                    }
                }
            }
            // One line through `run_script`, the replay's own path (references resolved there).
            let line = json!({"cmd": cmd.name, "params": cmd.params});
            let one = CmdLog::parse(&format!("{{\"$schema\":\"ew.cmdlog/1\"}}\n{line}\n")).unwrap();
            s.run_script(&one, "a.ewcmd")
                .unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
        }
        let replayed = s.page_tables("101").unwrap();
        assert_eq!(wiring_oracle(&replayed), wired, "the replay's wiring");
        let routes = |t: &ew_ui::ew_viewmodel::ew_api::output::PageTables| {
            let mut r: Vec<Vec<(i64, i64)>> = t.connections.iter().map(|c| c.polyline.clone()).collect();
            r.sort();
            r
        };
        assert_eq!(
            routes(&replayed),
            routes(&tables),
            "the replay's routes: the router never runs on replay"
        );
        assert_eq!(records_of(&s), records_of(&scripted));
        s.close();
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
        use ew_ui::ew_viewmodel::ew_api::output::PageTables;
        let tmp = Tmp::new();
        let job = start(&bin_exe(), &tmp);
        let (session, _) = Session::open_read_only(&job, "test").unwrap();
        let grid = session.page_grid("102").expect("page 102");
        let map = SheetMap {
            scale_ppb: i128::from(grid.sheet_scale_ppb),
            offset: (
                i128::from(grid.sheet_offset_nm.0),
                i128::from(grid.sheet_offset_nm.1),
            ),
        };
        drop(session);
        let (placements, runs) = placement_steps(102, map, "", &BTreeMap::new());
        assert_eq!(runs, 0, "no command.run step");
        assert!(
            placements.iter().filter(|(a, _)| a == "tool.black_box").count() == 2,
            "two black boxes"
        );
        let mut transcripts = Vec::new();
        let mut records = Vec::new();
        for n in ["a", "b"] {
            let dir = tmp.path(n).join("folio.ew");
            copy_dir(&job, &dir);
            let mut script = steps(&[
                ("file.open", json!({ "dir": path(&dir) })),
                ("page.goto", json!({"folio": "102"})),
            ]);
            script.extend(placements.iter().cloned());
            script.extend(steps(&[
                ("ui.describe", json!({})),
                ("file.save", json!({})),
                ("file.close", json!({})),
            ]));
            let record = tmp.path(&format!("{n}.ewcmd"));
            let out = run(&bin_exe(), &tmp, &format!("{n}.jsonl"), &script, Some(&record));
            all_done(&out);
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(
                !text.contains(&*tmp.dir().to_string_lossy()),
                "a path in the transcript"
            );
            transcripts.push(text);
            records.push(record);
        }
        if let Some((a, b)) = transcripts[0]
            .lines()
            .zip(transcripts[1].lines())
            .find(|(a, b)| a != b)
        {
            let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "two runs, two transcripts: first difference at {at}: {} | {}",
                &a[at.saturating_sub(120)..(at + 80).min(a.len())],
                &b[at.saturating_sub(120)..(at + 80).min(b.len())]
            );
        }
        assert_eq!(transcripts[0], transcripts[1], "two runs, one transcript");
        super::support::assert_no_uuid(&transcripts[0]);
        // The oracle.
        type Boxes = Vec<((i64, i64), Vec<(i64, i64)>, Vec<(u16, (i64, i64), String)>)>;
        let boxes = |t: &PageTables| -> Boxes {
            let mut v: Boxes = t
                .reps
                .iter()
                .filter_map(|r| {
                    let b = r.black_box.as_ref()?;
                    let mut cps: Vec<_> = b
                        .cps
                        .iter()
                        .map(|(i, p, d)| (*i, *p, d.word().to_owned()))
                        .collect();
                    cps.sort();
                    Some((r.pos, b.outline.clone(), cps))
                })
                .collect();
            v.sort();
            v
        };
        let ip_rows = |t: &PageTables| {
            let mut v: Vec<_> = t
                .reps
                .iter()
                .filter_map(|r| {
                    r.ip.as_ref()
                        .map(|i| (i.name.clone(), i.direction.clone(), r.symbol_key.clone(), r.pos))
                })
                .collect();
            v.sort();
            v
        };
        let (golden_session, _) =
            Session::open_read_only(Path::new(super::support::GOLDEN_A), "test").unwrap();
        let golden_tables = golden_session.page_tables("102").unwrap();
        let (scripted, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
        let tables = scripted.page_tables("102").unwrap();
        let reps = |t: &PageTables| {
            let mut v: Vec<_> = t
                .reps
                .iter()
                .map(|r| (r.symbol_key.clone(), r.pos, r.orient))
                .collect();
            v.sort();
            v
        };
        assert_eq!(golden_tables.reps.len(), 12);
        assert_eq!(reps(&tables), reps(&golden_tables), "the representations of 102");
        assert_eq!(
            ip_rows(&tables),
            ip_rows(&golden_tables),
            "the interruption points"
        );
        assert_eq!(boxes(&golden_tables).len(), 2);
        assert_eq!(boxes(&tables), boxes(&golden_tables), "the black boxes");
        // `--record` replayed on a third copy of the start: each `box.reshape` names the representation the run minted,
        // mapped to the replay's black box at the same point.
        let replay = tmp.path("c").join("folio.ew");
        copy_dir(&job, &replay);
        let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
        let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names.iter().filter(|n| **n == "box.black").count(),
            2,
            "{names:?}"
        );
        assert_eq!(names.iter().filter(|n| **n == "ip.place").count(), 9, "{names:?}");
        assert!(names.contains(&"box.reshape"));
        let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
        store.state_dir = Some(tmp.path("state"));
        let (mut s, _) = Session::open(
            &replay,
            store,
            ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
            ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
        )
        .unwrap();
        for cmd in log.commands() {
            let mut cmd = cmd.clone();
            if cmd.name == "box.reshape" {
                let id = cmd.params["rep"].as_str().unwrap().to_owned();
                let at = tables
                    .reps
                    .iter()
                    .find(|r| r.id.to_string() == id)
                    .expect("the run's box")
                    .pos;
                let now = s.page_tables("102").unwrap();
                let mapped = now
                    .reps
                    .iter()
                    .find(|r| r.pos == at && r.black_box.is_some())
                    .expect("the replay's box")
                    .id;
                cmd.params["rep"] = Value::from(mapped.to_string());
            }
            let line = json!({"cmd": cmd.name, "params": cmd.params});
            let one = CmdLog::parse(&format!("{{\"$schema\":\"ew.cmdlog/1\"}}\n{line}\n")).unwrap();
            s.run_script(&one, "a.ewcmd")
                .unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
        }
        let replayed = s.page_tables("102").unwrap();
        assert_eq!(reps(&replayed), reps(&tables));
        assert_eq!(boxes(&replayed), boxes(&tables));
        assert_eq!(records_of(&s), records_of(&scripted));
        s.close();
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
        use super::support::{
            external_connection_anchors, golden_wiring, marker_steps, potential_steps, structure_box_steps,
            typed_tag_steps, wiring_oracle, wiring_steps,
        };
        use ew_ui::ew_viewmodel::ew_api::output::{PageTables, TableConnection, TableEnd};
        let tmp = Tmp::new();
        let job = start(&bin_exe(), &tmp);
        let (session, _) = Session::open_read_only(&job, "test").unwrap();
        let grid = session.page_grid("101").expect("page 101");
        let map = SheetMap::of_grid(&grid);
        let q = session.query();
        let strip = q
            .devices()
            .iter()
            .find(|(_, d)| d.aspects.strip.is_some())
            .map(|(id, _)| id.to_string())
            .expect("the start project's strip");
        drop(q);
        drop(session);
        let (placements, runs) = placement_steps(101, map, &strip, &BTreeMap::new());
        // Phase A: the placements alone, to learn the tags they print.
        let placed = tmp.path("p").join("folio.ew");
        copy_dir(&job, &placed);
        let mut script = steps(&[
            ("file.open", json!({ "dir": path(&placed) })),
            ("page.goto", json!({"folio": "101"})),
        ]);
        script.extend(placements.iter().cloned());
        script.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
        all_done(&run(
            &bin_exe(),
            &tmp,
            "p.jsonl",
            &script,
            Some(&tmp.path("p.ewcmd")),
        ));
        let tags = typed_tag_steps(101, &placed);
        assert!(!tags.is_empty(), "the typed tags of 101");
        let (wiring, commits, _points) = wiring_steps(101, map);
        assert_eq!(commits, 19, "15 plain wires and 2 per junction");
        // The tags and the wiring of the phase-A copy, to learn the rehearsal's page tables: the marker search runs
        // on the drawn routes, not on Golden A's.
        let mut probe = steps(&[
            ("file.open", json!({ "dir": path(&placed) })),
            ("page.goto", json!({"folio": "101"})),
        ]);
        probe.extend(tags.iter().cloned());
        probe.extend(wiring.iter().cloned());
        probe.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
        all_done(&run(
            &bin_exe(),
            &tmp,
            "probe.jsonl",
            &probe,
            Some(&tmp.path("probe.ewcmd")),
        ));
        let (probe_session, _) = Session::open_read_only(&placed, "test").unwrap();
        let tables = probe_session.page_tables("101").unwrap();
        drop(probe_session);
        let (box_steps, marker) = (
            structure_box_steps(map, (grid.num_nm, grid.den), &external_connection_anchors(101)),
            marker_steps(101, &tables, map),
        );
        let (marker_steps_, fallbacks) = marker;
        assert!(
            fallbacks.is_empty(),
            "no marker fallback on 101: the Marker tool places both groups"
        );
        // The full script, twice.
        let first_tag = tags[0].1["params"]["targets"][0]["ref"].clone();
        let mut transcripts = Vec::new();
        let mut records = Vec::new();
        for n in ["a", "b"] {
            let dir = tmp.path(n).join("folio.ew");
            copy_dir(&job, &dir);
            let mut script = steps(&[
                ("file.open", json!({ "dir": path(&dir) })),
                ("page.goto", json!({"folio": "101"})),
            ]);
            script.extend(placements.iter().cloned());
            // A device the script placed, selected and cleared: the transcript names it by its alias (FT-422).
            script.extend(steps(&[
                ("select.set", json!({"refs": [first_tag.clone()]})),
                ("select.set", json!({"ids": []})),
            ]));
            script.extend(tags.iter().cloned());
            script.extend(wiring.iter().cloned());
            script.extend(box_steps.iter().cloned());
            script.extend(marker_steps_.iter().cloned());
            script.extend(steps(&[
                ("ui.describe", json!({})),
                ("file.save", json!({})),
                ("file.close", json!({})),
            ]));
            let record = tmp.path(&format!("{n}.ewcmd"));
            let out = run(&bin_exe(), &tmp, &format!("{n}.jsonl"), &script, Some(&record));
            all_done(&out);
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(
                !text.contains(&*tmp.dir().to_string_lossy()),
                "a path in the transcript"
            );
            transcripts.push(text);
            records.push(record);
        }
        if let Some((a, b)) = transcripts[0]
            .lines()
            .zip(transcripts[1].lines())
            .find(|(a, b)| a != b)
        {
            let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "two runs, two transcripts: first difference at {at}: {} | {}",
                &a[at.saturating_sub(120)..(at + 80).min(a.len())],
                &b[at.saturating_sub(120)..(at + 80).min(b.len())]
            );
        }
        assert_eq!(transcripts[0], transcripts[1], "two runs, one transcript");
        super::support::assert_no_uuid(&transcripts[0]);
        // §13.6's golden potential definitions of folio 101 through a Session (the script names no minted function);
        // they colour the marker groups.
        let apply_potentials = |dir: &Path, state: &str| {
            let potentials = potential_steps(&[101], dir);
            assert!(!potentials.is_empty(), "the golden potentials of folio 101");
            let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
            store.state_dir = Some(tmp.path(state));
            let (mut p, _) = Session::open(
                dir,
                store,
                ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
                ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
            )
            .unwrap();
            for (label, cmd) in &potentials {
                p.execute(cmd)
                    .unwrap_or_else(|e| panic!("potential {label}: {e:?}"));
            }
            p.save().unwrap();
            p.close();
        };
        for n in ["a", "b"] {
            apply_potentials(&tmp.path(n).join("folio.ew"), &format!("state-pot-{n}"));
        }
        // The oracle.
        let (scripted, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
        let tables = scripted.page_tables("101").unwrap();
        assert_eq!(tables.reps.len(), 17, "representations on 101");
        assert_eq!(wiring_oracle(&tables), golden_wiring(101), "the wiring of 101");
        assert_eq!(tables.structure_boxes.len(), 1, "one structure box on 101");
        assert_eq!(
            tables.structure_boxes[0].caption,
            "External Connection (+J-E-206 / JE-N1000-03)"
        );
        let mut markers: Vec<String> = tables
            .items
            .iter()
            .filter(|i| i.kind == "wire_marker")
            .filter_map(|i| i.text.clone())
            .collect();
        markers.sort();
        assert_eq!(markers, ["2.5 BK BU", "2.5 BK BU"]);
        assert_eq!(runs, 0, "the placements leave no command.run step");
        // `--record` replayed on a third copy of the start: each `connection.draw` end (by its CP's point and index)
        // and each `marker.place` connection (by its two end labels) mapped to the replay's.
        let replay = tmp.path("c").join("folio.ew");
        copy_dir(&job, &replay);
        let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
        let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names.iter().filter(|n| **n == "connection.draw").count(),
            19,
            "{names:?}"
        );
        assert_eq!(
            names.iter().filter(|n| **n == "marker.place").count(),
            2,
            "{names:?}"
        );
        assert_eq!(
            names.iter().filter(|n| **n == "box.structure").count(),
            1,
            "{names:?}"
        );
        let ends = |t: &PageTables, c: &TableConnection| -> (Option<String>, Option<String>) {
            let label = |e: TableEnd| match e {
                TableEnd::Cp { function, idx } => t
                    .cps
                    .iter()
                    .find(|x| x.function == function && x.idx == idx)
                    .map(|x| x.label.clone()),
                TableEnd::Junction { .. } => None,
            };
            (label(c.a), label(c.b))
        };
        let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
        store.state_dir = Some(tmp.path("state"));
        let (mut s, _) = Session::open(
            &replay,
            store,
            ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
            ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
        )
        .unwrap();
        for cmd in log.commands() {
            let mut cmd = cmd.clone();
            if cmd.name == "connection.draw" {
                let now = s.page_tables("101").unwrap();
                let map_cp = |f: &str, idx: u64| -> String {
                    let at = tables
                        .cps
                        .iter()
                        .find(|c| c.function.to_string() == f && u64::from(c.idx) == idx)
                        .expect("a CP of the run")
                        .at;
                    now.cps
                        .iter()
                        .find(|c| c.at == at && u64::from(c.idx) == idx)
                        .expect("the CP of the replay")
                        .function
                        .to_string()
                };
                for end in ["a", "b"] {
                    let e = &mut cmd.params[end];
                    if let Some(cp) = e.get_mut("cp") {
                        let f = map_cp(cp["function"].as_str().unwrap(), cp["idx"].as_u64().unwrap());
                        cp["function"] = Value::from(f);
                    } else if let Some(on) = e.get_mut("on_connection") {
                        let id = on["connection"].as_str().unwrap().to_owned();
                        let w = tables
                            .connections
                            .iter()
                            .find(|c| c.id.to_string() == id)
                            .expect("the wire");
                        let TableEnd::Cp { function, idx } = w.a else {
                            panic!("the split wire starts at a CP");
                        };
                        let f = map_cp(&function.to_string(), u64::from(idx));
                        let mapped = now
                            .connections
                            .iter()
                            .find(|c| {
                                matches!(c.a, TableEnd::Cp { function, idx: i }
                                    if function.to_string() == f && i == idx)
                            })
                            .expect("the wire of the replay")
                            .id;
                        on["connection"] = Value::from(mapped.to_string());
                    }
                }
            }
            if cmd.name == "marker.place" {
                let now = s.page_tables("101").unwrap();
                let ids: Vec<String> = cmd.params["connections"]
                    .as_array()
                    .expect("the marker's connections")
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect();
                let mut mapped = Vec::new();
                for id in ids {
                    let connection = tables
                        .connections
                        .iter()
                        .find(|c| c.id.to_string() == id)
                        .expect("the marker's connection");
                    let want = ends(&tables, connection);
                    let found = now
                        .connections
                        .iter()
                        .find(|c| {
                            let have = ends(&now, c);
                            have == want || (have.0 == want.1 && have.1 == want.0)
                        })
                        .expect("the marker's connection in the replay");
                    mapped.push(found.id.to_string());
                }
                cmd.params["connections"] = json!(mapped);
            }
            let line = json!({"cmd": cmd.name, "params": cmd.params});
            let one = CmdLog::parse(&format!("{{\"$schema\":\"ew.cmdlog/1\"}}\n{line}\n")).unwrap();
            s.run_script(&one, "a.ewcmd")
                .unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
        }
        s.save().unwrap();
        s.close();
        // The same golden potentials on the replay (its functions are its own, so they map again).
        apply_potentials(&replay, "state-pot-replay");
        let (replayed_s, _) = Session::open_read_only(&replay, "test").unwrap();
        let replayed = replayed_s.page_tables("101").unwrap();
        assert_eq!(
            replayed.structure_boxes.len(),
            1,
            "one structure box in the replay"
        );
        assert_eq!(
            replayed.structure_boxes[0].outline, tables.structure_boxes[0].outline,
            "the box outline"
        );
        assert_eq!(
            replayed.structure_boxes[0].caption,
            "External Connection (+J-E-206 / JE-N1000-03)"
        );
        let mut replayed_markers: Vec<String> = replayed
            .items
            .iter()
            .filter(|i| i.kind == "wire_marker")
            .filter_map(|i| i.text.clone())
            .collect();
        replayed_markers.sort();
        assert_eq!(replayed_markers, markers, "the marker texts of the replay");
        assert_eq!(records_of(&replayed_s), records_of(&scripted));
        replayed_s.close();
    }
}
