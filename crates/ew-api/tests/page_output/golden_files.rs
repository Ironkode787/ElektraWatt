// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The committed expected outputs of the golden project (doc 20 §3.1, §3.3): one function per artefact
//! computing its bytes from the model, and the bless switch (`EW_BLESS=1`) that rewrites the committed file
//! from the same function instead of comparing.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ew_dl::{DisplayList, FontKey, Item, ItemKind, Transform};
use ew_render_cpu::FontBytes;
use ew_scene::forms::{FrameGeometry, SheetMap};
use ew_scene::page::PageScene;
use ew_schema::{FormDef, LibCopy, LibDocument, Pen, Primitive, Stroke};
use ew_units::{Nm, Pt2, Rect};
use serde_json::{Map, Value};

use super::view::repo;

/// The directory of the expected outputs.
pub fn expected_dir(sub: &str) -> PathBuf {
    repo().join("golden/J-E-2416/expected").join(sub)
}

/// Whether the committed files are to be rewritten (`EW_BLESS=1`).
pub fn blessing() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// Compares `bytes` with the committed file `path`, or writes it when blessing.
pub fn check_or_bless(path: &Path, bytes: &[u8]) {
    if blessing() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
        return;
    }
    let committed = std::fs::read(path)
        .unwrap_or_else(|e| panic!("{}: {e} (run with EW_BLESS=1 to generate it)", path.display()));
    assert!(
        committed == bytes,
        "{} differs from the generated output (EW_BLESS=1 rewrites it)",
        path.display()
    );
}

// ---------------------------------------------------------------------------------------------------------
// Text CSV

/// nm to whole µm, ties away from zero.
fn um(nm: i64) -> i64 {
    let a = (i128::from(nm).abs() + 500) / 1000;
    let v = i64::try_from(a).unwrap();
    if nm < 0 { -v } else { v }
}

/// µm as `-?I.FFF` mm.
fn mm3(um: i64) -> String {
    let sign = if um < 0 { "-" } else { "" };
    let a = um.unsigned_abs();
    format!("{sign}{}.{:03}", a / 1000, a % 1000)
}

fn nm_mm3(nm: i64) -> String {
    mm3(um(nm))
}

/// nm to centi-pt (1 pt = 25.4 / 72 mm), ties away from zero, as `I.FF`.
fn size_pt(nm: i64) -> String {
    let num = i128::from(nm) * 7200;
    let den = 25_400_000i128;
    let q = (num.abs() * 2 + den) / (2 * den);
    let q = if num < 0 { -q } else { q };
    let sign = if q < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", q.abs() / 100, q.abs() % 100)
}

fn csv_field(s: &str) -> String {
    let quote = s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ');
    if quote {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

struct Row {
    layer: &'static str,
    entity: String,
    text: String,
    origin: Pt2,
    size: i64,
    angle: i32,
    bbox: [i64; 4],
}

fn through(stack: &[Transform], mut p: Pt2) -> Pt2 {
    for t in stack.iter().rev() {
        p = t.apply(p);
    }
    p
}

fn walk(
    layer: &'static str,
    items: &[Item],
    stack: &mut Vec<Transform>,
    entity: Option<String>,
    out: &mut Vec<Row>,
) {
    for item in items {
        let entity = item
            .entity
            .map(|e| e.0.hyphenated().to_string())
            .or_else(|| entity.clone());
        match &item.kind {
            ItemKind::Glyphs(run) => {
                let origin = through(stack, run.to_parent(Pt2::ORIGIN));
                let quad = run.layout_quad().map(|p| through(stack, p));
                let xs = quad.iter().map(|p| p.x.0);
                let ys = quad.iter().map(|p| p.y.0);
                out.push(Row {
                    layer,
                    entity: entity.unwrap_or_default(),
                    text: run.text.clone(),
                    origin,
                    size: run.size.0,
                    angle: run.transform.angle.0.rem_euclid(360_000),
                    bbox: [
                        xs.clone().min().unwrap(),
                        ys.clone().min().unwrap(),
                        xs.max().unwrap(),
                        ys.max().unwrap(),
                    ],
                });
            }
            ItemKind::Group(g) => {
                if let Some(t) = g.transform {
                    stack.push(t);
                }
                walk(layer, &g.items, stack, entity, out);
                if g.transform.is_some() {
                    stack.pop();
                }
            }
            _ => {}
        }
    }
}

/// The exported glyph runs of a page on the sheet (see the README of the golden project), as CSV.
pub fn text_csv(scene: &PageScene) -> String {
    let mut rows = Vec::new();
    for (layer, list) in &scene.layers {
        if !layer.is_exported() {
            continue;
        }
        let sheet: DisplayList = scene.map.apply_display_list(list, Some(&scene.geometry));
        walk(layer.name(), &sheet.items, &mut Vec::new(), None, &mut rows);
    }
    let mut out = String::from(
        "seq,layer,entity,text,x_mm,y_mm,size_pt,angle_mdeg,box_x0_mm,box_y0_mm,box_x1_mm,box_y1_mm\n",
    );
    for (i, r) in rows.iter().enumerate() {
        writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            i + 1,
            r.layer,
            r.entity,
            csv_field(&r.text),
            nm_mm3(r.origin.x.0),
            nm_mm3(r.origin.y.0),
            size_pt(r.size),
            r.angle,
            nm_mm3(r.bbox[0]),
            nm_mm3(r.bbox[1]),
            nm_mm3(r.bbox[2]),
            nm_mm3(r.bbox[3]),
        )
        .unwrap();
    }
    out
}

/// The number of data rows of a text CSV.
pub fn csv_rows(csv: &str) -> usize {
    csv.lines().count().saturating_sub(1)
}

// ---------------------------------------------------------------------------------------------------------
// Frame geometry

/// The form with `key` from `content/forms`.
pub fn content_form(key: &str) -> FormDef {
    let dir = repo().join("content/forms");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    for p in paths {
        if let LibDocument::Form(LibCopy { item, .. }) =
            LibDocument::from_json(&std::fs::read_to_string(&p).unwrap()).unwrap()
            && item.meta.key.as_ref().is_some_and(|k| k.as_str() == key)
        {
            return item;
        }
    }
    panic!("no form {key} in content/forms");
}

fn mm(nm: Nm) -> Value {
    Value::from(um(nm.0) as f64 / 1000.0)
}

fn obj(mut members: Vec<(&str, Value)>) -> Value {
    members.sort_by(|a, b| a.0.cmp(b.0));
    let mut m = Map::new();
    for (k, v) in members {
        m.insert(k.to_owned(), v);
    }
    Value::Object(m)
}

fn rect(x0: Nm, y0: Nm, x1: Nm, y1: Nm) -> Value {
    obj(vec![
        ("x0", mm(x0)),
        ("y0", mm(y0)),
        ("x1", mm(x1)),
        ("y1", mm(y1)),
    ])
}

/// The figures of doc 20 §17.1 that the printed frame must show, as the mismatches (each within 0.01 mm
/// of the document's value); empty when they all agree.
pub fn frame_figure_errors() -> Vec<String> {
    let tb = content_form("ew.titleblock.qet-a4");
    let f = frame_figures(&content_form("ew.frame.qet-a4-landscape-grid"), &tb);
    let n = |v: &Value| v.as_f64().unwrap();
    let mut errors = Vec::new();
    let mut near = |a: f64, b: f64, what: &str| {
        if (a - b).abs() > 0.01 {
            errors.push(format!("{what}: {a} vs {b}"));
        }
    };
    near(n(&f["outer"]["x0"]), 9.79, "outer x0");
    near(n(&f["outer"]["x1"]), 281.18, "outer x1");
    near(n(&f["outer"]["y0"]), 9.79, "outer y0");
    near(n(&f["outer"]["y1"]), 199.76, "outer y1");
    near(n(&f["header_band"]), 15.01 - 9.79, "header band thickness");
    let xs = f["column_edges"].as_array().unwrap();
    let ys = f["row_edges"].as_array().unwrap();
    near(n(&xs[1]) - n(&xs[0]), 15.66, "column width");
    near(n(&ys[1]) - n(&ys[0]), 20.88, "row height");
    near(
        n(&xs[0]),
        15.01,
        "first column edge (the left band ends at 15.01)",
    );
    near(n(&f["drawing_area"]["y1"]), 182.02, "drawing area bottom");
    errors
}

/// The printed frame figures of `form` with `title_block` (doc 20 §17.1), in mm on the sheet.
pub fn frame_figures(form: &FormDef, title_block: &FormDef) -> Value {
    let g = FrameGeometry::of(form, Some(title_block)).expect("a frame form with a grid");
    let map = SheetMap::of(form);
    let p = |pt: Pt2| map.apply(pt);
    let area = |r: Rect| {
        let (a, b) = (p(r.min()), p(r.max()));
        rect(a.x, a.y, b.x, b.y)
    };
    let xs = g.column_edges.iter().map(|&x| p(Pt2::new(x, Nm::ZERO)).x);
    let ys = g.row_edges.iter().map(|&y| p(Pt2::new(Nm::ZERO, y)).y);
    let tb = g.title_block_origin.map(|o| {
        let (o, e) = (p(o), p(g.outer.max()));
        rect(o.x, o.y, e.x, e.y)
    });
    let strings = |v: &[String]| Value::Array(v.iter().cloned().map(Value::from).collect());
    // The stroked lines of the form's own graphics (the cover's rules and boxes), as sheet segments with their
    // printed width; a stroke with a named pen or no stroke is not listed.
    let width = |s: &Option<Stroke>| {
        s.as_ref().and_then(|s| match &s.pen {
            Pen::Width(n) => Some(map.scale_len(*n)),
            _ => None,
        })
    };
    let segment = |a: Pt2, b: Pt2, w: Nm| {
        let (a, b) = (p(a), p(b));
        obj(vec![
            ("x0", mm(a.x)),
            ("y0", mm(a.y)),
            ("x1", mm(b.x)),
            ("y1", mm(b.y)),
            ("width", mm(w)),
        ])
    };
    let mut graphics = Vec::new();
    for prim in &form.graphics {
        match prim {
            Primitive::Line(l) => {
                if let Some(w) = width(&l.stroke) {
                    graphics.push(segment(l.from, l.to, w));
                }
            }
            Primitive::Rect(r) => {
                if let Some(w) = width(&r.stroke) {
                    let (a, b) = (r.rect.0.min(), r.rect.0.max());
                    let (tr, bl) = (Pt2::new(b.x, a.y), Pt2::new(a.x, b.y));
                    for (from, to) in [(a, tr), (tr, b), (b, bl), (bl, a)] {
                        graphics.push(segment(from, to, w));
                    }
                }
            }
            _ => {}
        }
    }
    let mut members = vec![
        (
            "sheet",
            obj(vec![
                ("width", mm(form.sheet.width)),
                ("height", mm(form.sheet.height)),
            ]),
        ),
        ("outer", area(g.outer)),
        ("drawing_area", area(g.drawing_area)),
        ("header_band", mm(map.scale_len(g.header_band))),
        ("column_edges", Value::Array(xs.map(mm).collect())),
        ("row_edges", Value::Array(ys.map(mm).collect())),
        ("column_labels", strings(&g.column_labels)),
        ("row_labels", strings(&g.row_labels)),
        ("title_block", tb.unwrap_or(Value::Null)),
        ("line_width", mm(map.scale_len(g.line_width))),
    ];
    if !graphics.is_empty() {
        members.push(("graphics", Value::Array(graphics)));
    }
    obj(members)
}

/// `frame.json`: the figures of the frame forms with the QET title block, keyed by form key.
pub fn frame_json() -> String {
    let tb = content_form("ew.titleblock.qet-a4");
    let members: Vec<(&str, Value)> = [
        "ew.frame.qet-a4-landscape-grid",
        "ew.frame.qet-a4-landscape-nogrid",
        // The cover frame is the no-grid frame with the cover graphics (D-30).
        "ew.frame.qet-a4-landscape-cover",
    ]
    .into_iter()
    .map(|key| (key, frame_figures(&content_form(key), &tb)))
    .collect();
    let mut s = serde_json::to_string_pretty(&obj(members)).unwrap();
    s.push('\n');
    s
}

// ---------------------------------------------------------------------------------------------------------
// PNG

/// The 150 dpi PNG of a page.
pub fn page_png(scene: &PageScene, fonts: &BTreeMap<FontKey, FontBytes>) -> Vec<u8> {
    let sheet = ew_render_cpu::SheetSize {
        width: scene.sheet_width,
        height: scene.sheet_height,
    };
    let options = ew_render_cpu::RenderOptions {
        dpi: 150,
        ..Default::default()
    };
    let (img, report) =
        ew_render_cpu::render(&scene.sheet_list(), sheet, &options, fonts, None).expect("the page renders");
    assert!(report.missing_fonts.is_empty(), "{report:?}");
    ew_render_cpu::encode_png(&img, Some(150))
}

/// A decoded PNG as RGBA8 and its size.
pub fn decode_rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
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
pub fn differing_percent(a: &[u8], b: &[u8], threshold: u8) -> f64 {
    assert_eq!(a.len(), b.len());
    let n = a.len() / 4;
    let bad = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > threshold))
        .count();
    bad as f64 * 100.0 / n as f64
}
