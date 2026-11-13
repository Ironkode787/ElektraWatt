// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The DXF writer (doc 09 §4.4, FR-OUT-015): one AutoCAD 2000 (AC1015) ASCII file per page from the page's sheet
//! layers, in millimetres (`$INSUNITS` 4), y up from the sheet's bottom edge.
//!
//! # Object model
//!
//! | Input | DXF |
//! |---|---|
//! | a page | `<label>.dxf` (as [`crate::svg_file_name`] names SVG files); `$EXTMIN`/`$EXTMAX` and `$LIMMIN`/`$LIMMAX` the sheet, so a re-import puts the drawing back where it was |
//! | a layer of the page | a DXF layer by [`DxfLayers`]: frame, symbols, wires, texts, graphics, tables (and the underlay when printed) |
//! | `Instance` | an `INSERT` of a `BLOCK` per symbol geometry (named `EW_<symbol>_<variant>[_<extra>]`), scaled by the page-to-sheet map, turned and mirrored as the instance's transform; on the symbols layer, the block's content on layer `0` |
//! | `Path` | one `LWPOLYLINE` per subpath (closed with `70 = 1`); quadratic and cubic Béziers flattened to within [`FLATNESS_NM`]; the stroke width as the nearest standard lineweight; a dash pattern as an `LTYPE` |
//! | a filled `Path` | a solid `HATCH` per closed subpath; a colour other than black as a true colour (420) |
//! | `Glyphs` | a `TEXT` at the run's baseline start, its cap height (0.714 em, the bundled face's) and angle; one `STYLE` per face; a character outside Windows-1252 as `\U+XXXX` |
//! | `Group` | its transform applied to its items; a clip is not written |
//! | `Image` | not written, counted |
//!
//! # Determinism
//!
//! Every coordinate comes from integer nanometres, written in millimetres with up to six decimals; handles count
//! up from one start in file order; no date, user name or random GUID is written. The same pages give the same bytes
//! on every platform.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

use ew_dl::ew_units::{Nm, Pt2};
use ew_dl::num::atan2_deg;
use ew_dl::{
    Brush, DisplayList, FontKey, Item, ItemKind, Layer, MAX_INSTANCE_DEPTH, PathEl, SymbolGeomId, Transform,
};

/// The tolerance Béziers are flattened to: 0.01 mm.
pub const FLATNESS_NM: f64 = 10_000.0;

/// The cap height of the bundled face per em (Noto Sans: 714 of 1000 units).
const CAP_PER_EM: i64 = 714;

/// The DXF layer names of the page layers (doc 09 §4.4's defaults; an export scheme maps them).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DxfLayers {
    /// The frame and title block.
    pub frame: String,
    /// The symbol inserts.
    pub symbols: String,
    /// The wires.
    pub wires: String,
    /// The texts outside the frame and tables.
    pub texts: String,
    /// The other graphics (page items, structure boxes, revision marks).
    pub graphics: String,
    /// The report tables.
    pub tables: String,
    /// A printed underlay.
    pub underlay: String,
}

impl Default for DxfLayers {
    fn default() -> DxfLayers {
        DxfLayers {
            frame: "EW-FRAME".to_owned(),
            symbols: "EW-SYMBOLS".to_owned(),
            wires: "EW-WIRES".to_owned(),
            texts: "EW-TEXTS".to_owned(),
            graphics: "EW-GRAPHICS".to_owned(),
            tables: "EW-TABLES".to_owned(),
            underlay: "EW-UNDERLAY".to_owned(),
        }
    }
}

impl DxfLayers {
    /// Each class (`frame`, `symbols`, `wires`, `texts`, `graphics`, `tables`, `underlay`) with its name, in the order
    /// the LAYER table lists them.
    fn named_mut(&mut self) -> [(&'static str, &mut String); 7] {
        [
            ("frame", &mut self.frame),
            ("symbols", &mut self.symbols),
            ("wires", &mut self.wires),
            ("texts", &mut self.texts),
            ("graphics", &mut self.graphics),
            ("tables", &mut self.tables),
            ("underlay", &mut self.underlay),
        ]
    }

    /// The layer classes an export scheme maps.
    pub fn classes() -> Vec<&'static str> {
        DxfLayers::default()
            .named_mut()
            .into_iter()
            .map(|(c, _)| c)
            .collect()
    }

    /// Names the layer of `class` `name`.
    ///
    /// # Errors
    ///
    /// The refusal in words for a class that is none of [`DxfLayers::classes`] or an empty name.
    pub fn set(&mut self, class: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("the layer class `{class}` needs a name"));
        }
        let slot = self.named_mut().into_iter().find(|(c, _)| *c == class);
        match slot {
            Some((_, s)) => {
                *s = name.to_owned();
                Ok(())
            }
            None => Err(format!(
                "`{class}` is no layer class; give one of {}",
                DxfLayers::classes().join(", ")
            )),
        }
    }

    /// The names in the order the LAYER table lists them.
    fn all(&self) -> Vec<String> {
        self.clone()
            .named_mut()
            .into_iter()
            .map(|(_, n)| n.clone())
            .collect()
    }
}

/// One page as the writer reads it: its layers on the sheet with their symbol instances kept (the page-to-sheet map
/// applied without a resolver, `ew_scene::forms::SheetMap::apply_display_list`), and their geometry.
#[derive(Clone, Debug)]
pub struct DxfSheet {
    /// The folio label ("101").
    pub label: String,
    /// The sheet width.
    pub width: Nm,
    /// The sheet height.
    pub height: Nm,
    /// The layers written, in paint order, on the sheet.
    pub layers: Vec<(Layer, DisplayList)>,
    /// The symbol geometry of the instances, in the symbol frame, unscaled.
    pub geometry: BTreeMap<SymbolGeomId, Arc<DisplayList>>,
    /// The page-to-sheet scale in parts per 10⁹: the scale of every insert.
    pub scale_ppb: u32,
}

/// DXF export settings.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DxfOptions {
    /// The layer names.
    pub layers: DxfLayers,
    /// The family and boldness of each face the runs name (its STYLE); a face without an entry is written as
    /// `Noto Sans`.
    pub faces: BTreeMap<FontKey, (String, bool)>,
}

/// One written DXF file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DxfFile {
    /// The file name, `<label>.dxf`.
    pub name: String,
    /// The document.
    pub text: String,
    /// Images not written.
    pub skipped_images: usize,
}

/// The file name of a page: its label as [`crate::svg_file_name`] cleans it, and `.dxf`.
pub fn dxf_file_name(label: &str) -> String {
    let svg = crate::svg_file_name(label);
    format!("{}.dxf", svg.trim_end_matches(".svg"))
}

/// A length in millimetres with up to six decimals.
fn mm(n: i64) -> String {
    let neg = n < 0;
    let a = n.unsigned_abs();
    let (int, frac) = (a / 1_000_000, a % 1_000_000);
    let mut s = if neg && a != 0 {
        format!("-{int}")
    } else {
        int.to_string()
    };
    if frac != 0 {
        let f = format!("{frac:06}");
        s.push('.');
        s.push_str(f.trim_end_matches('0'));
    }
    s
}

/// A real number with up to nine decimals.
fn real(v: f64) -> String {
    let s = format!("{v:.9}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".to_owned() } else { s.to_owned() }
}

/// The text of a DXF string: a character outside Windows-1252 as `\U+XXXX`, a line break as a space, `%%` doubled
/// to `%%%`.
fn dxf_text(s: &str) -> String {
    const CP1252_HIGH: &str = "€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ";
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let ok = (c as u32) < 0x80 || ((c as u32) >= 0xa0 && (c as u32) <= 0xff) || CP1252_HIGH.contains(c);
        match c {
            '\n' | '\r' => out.push(' '),
            _ if ok => out.push(c),
            _ => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\U+{unit:04X}");
                }
            }
        }
    }
    out.replace("%%", "%%%")
}

/// The standard lineweights in 1/100 mm.
const LINEWEIGHTS: [i64; 24] = [
    0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200, 211,
];

/// The standard lineweight nearest to a width.
fn lineweight(width: Nm) -> i64 {
    let w = width.0 / 10_000;
    LINEWEIGHTS
        .iter()
        .copied()
        .min_by_key(|l| (l - w).abs())
        .unwrap_or(25)
}

/// The points of a flattened Bézier from `p0` through the controls to `p3` (`p0` excluded), within [`FLATNESS_NM`].
fn bezier(p0: Pt2, c1: Pt2, c2: Pt2, p3: Pt2) -> Vec<Pt2> {
    let f = |p: Pt2| (p.x.0 as f64, p.y.0 as f64);
    let (a, b, c, d) = (f(p0), f(c1), f(c2), f(p3));
    // The control polygon's deviation bounds the curve's: n segments keep it within the tolerance.
    let dev = |u: (f64, f64), v: (f64, f64), w: (f64, f64)| {
        let x = u.0 - 2.0 * v.0 + w.0;
        let y = u.1 - 2.0 * v.1 + w.1;
        (x * x + y * y).sqrt()
    };
    let m = dev(a, b, c).max(dev(b, c, d));
    let n = ((0.75 * m / FLATNESS_NM).sqrt().ceil() as usize).clamp(1, 1024);
    (1..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let s = 1.0 - t;
            let x = s * s * s * a.0 + 3.0 * s * s * t * b.0 + 3.0 * s * t * t * c.0 + t * t * t * d.0;
            let y = s * s * s * a.1 + 3.0 * s * s * t * b.1 + 3.0 * s * t * t * c.1 + t * t * t * d.1;
            if i == n {
                p3
            } else {
                Pt2::new(Nm(x.round_ties_even() as i64), Nm(y.round_ties_even() as i64))
            }
        })
        .collect()
}

/// The subpaths of a path's elements: their points and whether each is closed.
fn subpaths(els: &[PathEl], t: Transform) -> Vec<(Vec<Pt2>, bool)> {
    let mut out: Vec<(Vec<Pt2>, bool)> = Vec::new();
    let mut cur: Vec<Pt2> = Vec::new();
    let finish = |cur: &mut Vec<Pt2>, out: &mut Vec<(Vec<Pt2>, bool)>, closed: bool| {
        if cur.len() >= 2 {
            out.push((std::mem::take(cur), closed));
        }
        cur.clear();
    };
    for el in els {
        let el = el.map(|p| t.apply(p));
        match el {
            PathEl::MoveTo(p) => {
                finish(&mut cur, &mut out, false);
                cur.push(p);
            }
            PathEl::LineTo(p) => cur.push(p),
            PathEl::QuadTo(c, p) => {
                let Some(&p0) = cur.last() else { continue };
                // The cubic of a quadratic: controls at two thirds towards the control point.
                let c1 = Pt2::new(
                    Nm(p0.x.0 + (c.x.0 - p0.x.0) * 2 / 3),
                    Nm(p0.y.0 + (c.y.0 - p0.y.0) * 2 / 3),
                );
                let c2 = Pt2::new(
                    Nm(p.x.0 + (c.x.0 - p.x.0) * 2 / 3),
                    Nm(p.y.0 + (c.y.0 - p.y.0) * 2 / 3),
                );
                cur.extend(bezier(p0, c1, c2, p));
            }
            PathEl::CubicTo(c1, c2, p) => {
                let Some(&p0) = cur.last() else { continue };
                cur.extend(bezier(p0, c1, c2, p));
            }
            PathEl::ClosePath => finish(&mut cur, &mut out, true),
        }
    }
    finish(&mut cur, &mut out, false);
    for (pts, closed) in &mut out {
        pts.dedup();
        if *closed && pts.len() > 1 && pts.first() == pts.last() {
            pts.pop();
        }
    }
    out.retain(|(pts, _)| pts.len() >= 2);
    out
}

/// The block name of a symbol geometry.
fn block_name(id: &SymbolGeomId) -> String {
    let mut s = format!("EW_{}_{}", id.symbol.simple(), id.variant.letter());
    if let Some(e) = id.extra {
        let _ = write!(s, "_{e}");
    }
    s
}

/// The writer of one file: its handles, its line types and styles, and the sections' text.
struct Writer<'a> {
    opts: &'a DxfOptions,
    height: Nm,
    next: u64,
    /// Dash patterns (on/off lengths in nm) by first use: `EW_DASH_<n>`.
    dashes: Vec<Vec<i64>>,
    /// Faces by first use.
    faces: Vec<FontKey>,
    skipped_images: usize,
}

/// One entity's text, before its handle and owner are known.
struct Ent {
    kind: &'static str,
    layer: String,
    body: String,
}

impl Writer<'_> {
    fn handle(&mut self) -> String {
        let h = format!("{:X}", self.next);
        self.next += 1;
        h
    }

    /// A point on the sheet as DXF x and y (y up from the bottom edge).
    fn xy(&self, p: Pt2) -> (String, String) {
        (mm(p.x.0), mm(self.height.0 - p.y.0))
    }

    /// A point of a block's content (symbol frame, y down) as DXF x and y.
    fn block_xy(p: Pt2) -> (String, String) {
        (mm(p.x.0), mm(-p.y.0))
    }

    fn dash(&mut self, segments: &[i64]) -> String {
        let n = match self.dashes.iter().position(|d| d == segments) {
            Some(i) => i,
            None => {
                self.dashes.push(segments.to_vec());
                self.dashes.len() - 1
            }
        };
        format!("EW_DASH_{}", n + 1)
    }

    fn style(&mut self, font: FontKey) -> String {
        if !self.faces.contains(&font) {
            self.faces.push(font);
        }
        style_name(self.opts, font)
    }

    /// The entities of `items` drawn under `t`, on `layer` (`None`: each item's class layer within `dl`); `block`
    /// writes block content (symbol frame, y down).
    #[allow(clippy::too_many_arguments)]
    fn items(
        &mut self,
        items: &[Item],
        t: Transform,
        dl: Option<Layer>,
        block: bool,
        geometry: &BTreeMap<SymbolGeomId, Arc<DisplayList>>,
        scale_ppb: u32,
        depth: usize,
        out: &mut Vec<Ent>,
    ) {
        for item in items {
            let layer = |w: &Self, kind: &str| -> String {
                if block {
                    return "0".to_owned();
                }
                let l = &w.opts.layers;
                match dl {
                    Some(Layer::Frame) => l.frame.clone(),
                    Some(Layer::Tables) => l.tables.clone(),
                    Some(Layer::Underlay) => l.underlay.clone(),
                    Some(Layer::Wires) if kind == "path" => l.wires.clone(),
                    _ if kind == "text" => l.texts.clone(),
                    _ if kind == "insert" => l.symbols.clone(),
                    _ => l.graphics.clone(),
                }
            };
            match &item.kind {
                ItemKind::Path(p) => {
                    if p.stroke.is_none() && p.fill.is_none() {
                        continue;
                    }
                    let lay = layer(self, "path");
                    for (pts, closed) in subpaths(&p.path.els, t) {
                        if let Some(fill) = &p.fill
                            && pts.len() >= 3
                        {
                            out.push(self.hatch(&pts, fill, &lay, block));
                        }
                        if let Some(stroke) = &p.stroke {
                            let mut body = String::new();
                            if let Some(d) = &stroke.dash {
                                let segs: Vec<i64> = d.segments.iter().map(|n| n.0).collect();
                                if !segs.is_empty() {
                                    let name = self.dash(&segs);
                                    let _ = write!(body, "  6\n{name}\n");
                                }
                            }
                            let _ = write!(body, "370\n{}\n", lineweight(stroke.width));
                            let Brush::Solid(c) = stroke.brush;
                            if c != ew_dl::Color::BLACK {
                                let _ = write!(body, "420\n{}\n", rgb(c));
                            }
                            let _ = write!(
                                body,
                                "100\nAcDbPolyline\n 90\n{}\n 70\n{}\n 43\n0\n",
                                pts.len(),
                                u8::from(closed)
                            );
                            for p in &pts {
                                let (x, y) = if block { Self::block_xy(*p) } else { self.xy(*p) };
                                let _ = write!(body, " 10\n{x}\n 20\n{y}\n");
                            }
                            out.push(Ent {
                                kind: "LWPOLYLINE",
                                layer: lay.clone(),
                                body,
                            });
                        }
                    }
                }
                ItemKind::Glyphs(run) => {
                    let lay = layer(self, "text");
                    let rt = run
                        .transform
                        .then(Transform::from_translation(run.origin))
                        .then(t);
                    let at = rt.apply(Pt2::ORIGIN);
                    let ahead = rt.apply(Pt2::new(Nm(1_000_000), Nm(0)));
                    // The angle on the sheet (y down) as DXF's (y up): the visual angle is the same.
                    let angle = atan2_deg(-((ahead.y.0 - at.y.0) as f64), (ahead.x.0 - at.x.0) as f64);
                    let cap = run.size.0 * CAP_PER_EM / 1000;
                    let style = self.style(run.font);
                    let (x, y) = if block { Self::block_xy(at) } else { self.xy(at) };
                    let mut body = String::new();
                    let Brush::Solid(c) = run.brush;
                    if c != ew_dl::Color::BLACK {
                        let _ = write!(body, "420\n{}\n", rgb(c));
                    }
                    let _ = write!(
                        body,
                        "100\nAcDbText\n 10\n{x}\n 20\n{y}\n 30\n0\n 40\n{}\n  1\n{}\n 50\n{}\n  7\n{style}\n100\nAcDbText\n",
                        mm(cap),
                        dxf_text(&run.text),
                        real(angle.rem_euclid(360.0)),
                    );
                    out.push(Ent {
                        kind: "TEXT",
                        layer: lay,
                        body,
                    });
                }
                ItemKind::Image(_) => self.skipped_images += 1,
                ItemKind::Group(g) => {
                    let inner = g.transform.map_or(t, |gt| gt.then(t));
                    self.items(&g.items, inner, dl, block, geometry, scale_ppb, depth, out);
                }
                ItemKind::Instance(inst) => {
                    if block {
                        // An instance inside symbol geometry is drawn in place.
                        if depth < MAX_INSTANCE_DEPTH
                            && let Some(g) = geometry.get(&inst.symbol)
                        {
                            let inner = inst.transform.then(t);
                            self.items(&g.items, inner, dl, block, geometry, scale_ppb, depth + 1, out);
                        }
                        continue;
                    }
                    let lay = layer(self, "insert");
                    let full = inst.transform.then(t);
                    let at = full.apply(Pt2::ORIGIN);
                    let (x, y) = self.xy(at);
                    // The orientation's matrix with the turn, in the sheet's y-down frame, seen in DXF's y-up frame:
                    // a quarter turn clockwise on the sheet is −90°, a mirror a negative x scale.
                    let turns = i64::from(full.orient.quarter_turns());
                    let angle = (f64::from(full.angle.0) / 1000.0 - 90.0 * turns as f64).rem_euclid(360.0);
                    let s = f64::from(scale_ppb) / 1e9;
                    let sx = if full.orient.is_mirrored() { -s } else { s };
                    let body = format!(
                        "100\nAcDbBlockReference\n  2\n{}\n 10\n{x}\n 20\n{y}\n 30\n0\n 41\n{}\n 42\n{}\n 43\n{}\n 50\n{}\n",
                        block_name(&inst.symbol),
                        real(sx),
                        real(s),
                        real(s),
                        real(angle),
                    );
                    out.push(Ent {
                        kind: "INSERT",
                        layer: lay,
                        body,
                    });
                }
            }
        }
    }

    fn hatch(&mut self, pts: &[Pt2], fill: &Brush, layer: &str, block: bool) -> Ent {
        let mut body = String::new();
        let Brush::Solid(c) = fill;
        if *c != ew_dl::Color::BLACK {
            let _ = write!(body, "420\n{}\n", rgb(*c));
        }
        let _ = write!(
            body,
            "100\nAcDbHatch\n 10\n0\n 20\n0\n 30\n0\n210\n0\n220\n0\n230\n1\n  2\nSOLID\n 70\n1\n 71\n0\n 91\n1\n 92\n3\n 72\n0\n 73\n1\n 93\n{}\n",
            pts.len()
        );
        for p in pts {
            let (x, y) = if block { Self::block_xy(*p) } else { self.xy(*p) };
            let _ = write!(body, " 10\n{x}\n 20\n{y}\n");
        }
        body.push_str(" 97\n0\n 75\n0\n 76\n1\n 98\n0\n");
        Ent {
            kind: "HATCH",
            layer: layer.to_owned(),
            body,
        }
    }
}

fn rgb(c: ew_dl::Color) -> u32 {
    (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b)
}

/// The STYLE name of a face: its family in capitals with `-` for spaces, `-BOLD` for a bold face.
fn style_name(opts: &DxfOptions, font: FontKey) -> String {
    let (family, bold) = opts
        .faces
        .get(&font)
        .cloned()
        .unwrap_or_else(|| ("Noto Sans".to_owned(), false));
    let mut s = format!("EW-{}", family.to_ascii_uppercase().replace(' ', "-"));
    if bold {
        s.push_str("-BOLD");
    }
    s
}

/// The font file of a face: its family without spaces, `-Bold` or `-Regular`, `.ttf`.
fn font_file(opts: &DxfOptions, font: FontKey) -> String {
    let (family, bold) = opts
        .faces
        .get(&font)
        .cloned()
        .unwrap_or_else(|| ("Noto Sans".to_owned(), false));
    format!(
        "{}-{}.ttf",
        family.replace(' ', ""),
        if bold { "Bold" } else { "Regular" }
    )
}

/// The DXF file of each page (see the module documentation).
pub fn dxf(pages: &[DxfSheet], options: &DxfOptions) -> Vec<DxfFile> {
    pages.iter().map(|p| page(p, options)).collect()
}

/// One page's file.
fn page(sheet: &DxfSheet, opts: &DxfOptions) -> DxfFile {
    let mut w = Writer {
        opts,
        height: sheet.height,
        next: 0x20,
        dashes: Vec::new(),
        faces: Vec::new(),
        skipped_images: 0,
    };
    // The entities of the model space and the symbol blocks it inserts, collected first: their line types and
    // styles make the tables.
    let mut model = Vec::new();
    for (layer, list) in &sheet.layers {
        w.items(
            &list.items,
            Transform::IDENTITY,
            Some(*layer),
            false,
            &sheet.geometry,
            sheet.scale_ppb,
            0,
            &mut model,
        );
    }
    let mut used = std::collections::BTreeSet::new();
    for (_, list) in &sheet.layers {
        collect_instances(&list.items, &mut used);
    }
    let mut blocks: Vec<(String, Vec<Ent>)> = Vec::new();
    for id in &used {
        let mut ents = Vec::new();
        if let Some(g) = sheet.geometry.get(id) {
            w.items(
                &g.items,
                Transform::IDENTITY,
                None,
                true,
                &sheet.geometry,
                sheet.scale_ppb,
                0,
                &mut ents,
            );
        }
        blocks.push((block_name(id), ents));
    }
    let mut s = String::new();
    let p = |s: &mut String, code: i32, v: &str| {
        let _ = write!(s, "{code:>3}\n{v}\n");
    };
    // Handles: the tables, the block records, the layouts and dictionaries first, so that the header's seed is
    // known when the file is assembled.
    let (wd, ht) = (mm(sheet.width.0), mm(sheet.height.0));
    let mut tables = String::new();
    let table = |w: &mut Writer<'_>, tables: &mut String, name: &str, entries: Vec<String>| {
        let h = w.handle();
        let _ = write!(
            tables,
            "  0\nTABLE\n  2\n{name}\n  5\n{h}\n330\n0\n100\nAcDbSymbolTable\n 70\n{}\n",
            entries.len()
        );
        if name == "DIMSTYLE" {
            tables.push_str("100\nAcDbDimStyleTable\n");
        }
        for e in entries {
            let eh = w.handle();
            let code = if name == "DIMSTYLE" { 105 } else { 5 };
            let _ = write!(
                tables,
                "  0\n{name}\n{code:>3}\n{eh}\n330\n{h}\n100\nAcDbSymbolTableRecord\n{e}"
            );
        }
        tables.push_str("  0\nENDTAB\n");
    };
    let half = |n: Nm| mm(n.0 / 2);
    table(
        &mut w,
        &mut tables,
        "VPORT",
        vec![format!(
            "100\nAcDbViewportTableRecord\n  2\n*Active\n 70\n0\n 10\n0\n 20\n0\n 11\n1\n 21\n1\n 12\n{}\n 22\n{}\n 13\n0\n 23\n0\n 14\n10\n 24\n10\n 15\n10\n 25\n10\n 16\n0\n 26\n0\n 36\n1\n 17\n0\n 27\n0\n 37\n0\n 40\n{ht}\n 41\n1.414\n 42\n50\n 43\n0\n 44\n0\n 50\n0\n 51\n0\n 71\n0\n 72\n100\n 73\n1\n 74\n3\n 75\n0\n 76\n0\n 77\n0\n 78\n0\n",
            half(sheet.width),
            half(sheet.height)
        )],
    );
    let mut ltypes = vec![
        "100\nAcDbLinetypeTableRecord\n  2\nByBlock\n 70\n0\n  3\n\n 72\n65\n 73\n0\n 40\n0\n".to_owned(),
        "100\nAcDbLinetypeTableRecord\n  2\nByLayer\n 70\n0\n  3\n\n 72\n65\n 73\n0\n 40\n0\n".to_owned(),
        "100\nAcDbLinetypeTableRecord\n  2\nContinuous\n 70\n0\n  3\nSolid line\n 72\n65\n 73\n0\n 40\n0\n"
            .to_owned(),
    ];
    for (i, d) in w.dashes.clone().iter().enumerate() {
        // An odd list repeats (PDF and SVG semantics): written twice to make on/off pairs.
        let full: Vec<i64> = if d.len() % 2 == 1 {
            d.iter().chain(d.iter()).copied().collect()
        } else {
            d.clone()
        };
        let total: i64 = full.iter().sum();
        let mut e = format!(
            "100\nAcDbLinetypeTableRecord\n  2\nEW_DASH_{}\n 70\n0\n  3\nElektraWatt dash {}\n 72\n65\n 73\n{}\n 40\n{}\n",
            i + 1,
            i + 1,
            full.len(),
            mm(total)
        );
        for (k, len) in full.iter().enumerate() {
            let v = if k % 2 == 0 { *len } else { -*len };
            let _ = write!(e, " 49\n{}\n 74\n0\n", mm(v));
        }
        ltypes.push(e);
    }
    table(&mut w, &mut tables, "LTYPE", ltypes);
    let mut layers =
        vec!["100\nAcDbLayerTableRecord\n  2\n0\n 70\n0\n 62\n7\n  6\nContinuous\n370\n-3\n".to_owned()];
    for name in opts.layers.all() {
        layers.push(format!(
            "100\nAcDbLayerTableRecord\n  2\n{}\n 70\n0\n 62\n7\n  6\nContinuous\n370\n-3\n",
            dxf_text(&name)
        ));
    }
    table(&mut w, &mut tables, "LAYER", layers);
    let mut styles = vec![
        "100\nAcDbTextStyleTableRecord\n  2\nStandard\n 70\n0\n 40\n0\n 41\n1\n 50\n0\n 71\n0\n 42\n2.5\n  3\nNotoSans-Regular.ttf\n  4\n\n"
            .to_owned(),
    ];
    for f in w.faces.clone() {
        styles.push(format!(
            "100\nAcDbTextStyleTableRecord\n  2\n{}\n 70\n0\n 40\n0\n 41\n1\n 50\n0\n 71\n0\n 42\n2.5\n  3\n{}\n  4\n\n",
            style_name(opts, f),
            font_file(opts, f)
        ));
    }
    table(&mut w, &mut tables, "STYLE", styles);
    table(&mut w, &mut tables, "VIEW", Vec::new());
    table(&mut w, &mut tables, "UCS", Vec::new());
    table(
        &mut w,
        &mut tables,
        "APPID",
        vec!["100\nAcDbRegAppTableRecord\n  2\nACAD\n 70\n0\n".to_owned()],
    );
    table(
        &mut w,
        &mut tables,
        "DIMSTYLE",
        vec!["100\nAcDbDimStyleTableRecord\n  2\nStandard\n 70\n0\n".to_owned()],
    );
    // The block records: the model and paper spaces, then one per symbol block. Their handles are fixed before the
    // table is written, since the layouts and the blocks point at them.
    let record_table = w.handle();
    let model_record = w.handle();
    let paper_record = w.handle();
    let block_records: Vec<String> = blocks.iter().map(|_| w.handle()).collect();
    let dict_root = w.handle();
    let dict_group = w.handle();
    let dict_layout = w.handle();
    let layout_model = w.handle();
    let layout_paper = w.handle();
    let _ = write!(
        tables,
        "  0\nTABLE\n  2\nBLOCK_RECORD\n  5\n{record_table}\n330\n0\n100\nAcDbSymbolTable\n 70\n{}\n",
        2 + blocks.len()
    );
    let record = |tables: &mut String, h: &str, name: &str, layout: &str| {
        let _ = write!(
            tables,
            "  0\nBLOCK_RECORD\n  5\n{h}\n330\n{record_table}\n100\nAcDbSymbolTableRecord\n100\nAcDbBlockTableRecord\n  2\n{name}\n340\n{layout}\n 70\n0\n280\n1\n281\n0\n"
        );
    };
    record(&mut tables, &model_record, "*Model_Space", &layout_model);
    record(&mut tables, &paper_record, "*Paper_Space", &layout_paper);
    for ((name, _), h) in blocks.iter().zip(&block_records) {
        record(&mut tables, h, name, "0");
    }
    tables.push_str("  0\nENDTAB\n");
    // The blocks section.
    let mut blocks_text = String::new();
    let block = |w: &mut Writer<'_>,
                 out: &mut String,
                 record: &str,
                 name: &str,
                 paper: bool,
                 ents: Vec<Ent>| {
        let h = w.handle();
        let space = if paper { " 67\n1\n" } else { "" };
        let _ = write!(
            out,
            "  0\nBLOCK\n  5\n{h}\n330\n{record}\n100\nAcDbEntity\n{space}  8\n0\n100\nAcDbBlockBegin\n  2\n{name}\n 70\n0\n 10\n0\n 20\n0\n 30\n0\n  3\n{name}\n  1\n\n"
        );
        for e in ents {
            let eh = w.handle();
            let _ = write!(
                out,
                "  0\n{}\n  5\n{eh}\n330\n{record}\n100\nAcDbEntity\n  8\n{}\n{}",
                e.kind,
                dxf_text(&e.layer),
                e.body
            );
        }
        let eh = w.handle();
        let _ = write!(
            out,
            "  0\nENDBLK\n  5\n{eh}\n330\n{record}\n100\nAcDbEntity\n{space}  8\n0\n100\nAcDbBlockEnd\n"
        );
    };
    block(
        &mut w,
        &mut blocks_text,
        &model_record,
        "*Model_Space",
        false,
        Vec::new(),
    );
    block(
        &mut w,
        &mut blocks_text,
        &paper_record,
        "*Paper_Space",
        true,
        Vec::new(),
    );
    for ((name, ents), h) in blocks.into_iter().zip(&block_records) {
        block(&mut w, &mut blocks_text, h, &name, false, ents);
    }
    // The entities.
    let mut entities = String::new();
    for e in model {
        let eh = w.handle();
        let _ = write!(
            entities,
            "  0\n{}\n  5\n{eh}\n330\n{model_record}\n100\nAcDbEntity\n  8\n{}\n{}",
            e.kind,
            dxf_text(&e.layer),
            e.body
        );
    }
    // The objects: the root dictionary, the group and layout dictionaries, and the two layouts.
    let mut objects = String::new();
    let _ = write!(
        objects,
        "  0\nDICTIONARY\n  5\n{dict_root}\n330\n0\n100\nAcDbDictionary\n281\n1\n  3\nACAD_GROUP\n350\n{dict_group}\n  3\nACAD_LAYOUT\n350\n{dict_layout}\n"
    );
    let _ = write!(
        objects,
        "  0\nDICTIONARY\n  5\n{dict_group}\n330\n{dict_root}\n100\nAcDbDictionary\n281\n1\n"
    );
    let _ = write!(
        objects,
        "  0\nDICTIONARY\n  5\n{dict_layout}\n330\n{dict_root}\n100\nAcDbDictionary\n281\n1\n  3\nLayout1\n350\n{layout_paper}\n  3\nModel\n350\n{layout_model}\n"
    );
    for (h, name, record, tab, flags) in [
        (&layout_model, "Model", &model_record, 0, 1),
        (&layout_paper, "Layout1", &paper_record, 1, 0),
    ] {
        let _ = write!(
            objects,
            "  0\nLAYOUT\n  5\n{h}\n330\n{dict_layout}\n100\nAcDbPlotSettings\n  1\n\n  2\nnone_device\n  4\n\n  6\n\n 40\n0\n 41\n0\n 42\n0\n 43\n0\n 44\n0\n 45\n0\n 46\n0\n 47\n0\n 48\n0\n 49\n0\n140\n0\n141\n0\n142\n1\n143\n1\n 70\n1712\n 72\n1\n 73\n0\n 74\n5\n  7\n\n 75\n16\n147\n1\n148\n0\n149\n0\n100\nAcDbLayout\n  1\n{name}\n 70\n{flags}\n 71\n{tab}\n 10\n0\n 20\n0\n 11\n{wd}\n 21\n{ht}\n 12\n0\n 22\n0\n 32\n0\n 14\n0\n 24\n0\n 34\n0\n 15\n{wd}\n 25\n{ht}\n 35\n0\n146\n0\n 13\n0\n 23\n0\n 33\n0\n 16\n1\n 26\n0\n 36\n0\n 17\n0\n 27\n1\n 37\n0\n 76\n0\n330\n{record}\n"
        );
    }
    let seed = format!("{:X}", w.next);
    // The file.
    p(&mut s, 0, "SECTION");
    p(&mut s, 2, "HEADER");
    for (var, pairs) in [
        ("$ACADVER", vec![(1, "AC1015".to_owned())]),
        ("$ACADMAINTVER", vec![(70, "6".to_owned())]),
        ("$DWGCODEPAGE", vec![(3, "ANSI_1252".to_owned())]),
        (
            "$INSBASE",
            vec![(10, "0".to_owned()), (20, "0".to_owned()), (30, "0".to_owned())],
        ),
        (
            "$EXTMIN",
            vec![(10, "0".to_owned()), (20, "0".to_owned()), (30, "0".to_owned())],
        ),
        (
            "$EXTMAX",
            vec![(10, wd.clone()), (20, ht.clone()), (30, "0".to_owned())],
        ),
        ("$LIMMIN", vec![(10, "0".to_owned()), (20, "0".to_owned())]),
        ("$LIMMAX", vec![(10, wd.clone()), (20, ht.clone())]),
        ("$LTSCALE", vec![(40, "1".to_owned())]),
        ("$INSUNITS", vec![(70, "4".to_owned())]),
        ("$MEASUREMENT", vec![(70, "1".to_owned())]),
        ("$HANDSEED", vec![(5, seed)]),
    ] {
        p(&mut s, 9, var);
        for (c, v) in pairs {
            p(&mut s, c, &v);
        }
    }
    p(&mut s, 0, "ENDSEC");
    for (name, body) in [
        ("CLASSES", String::new()),
        ("TABLES", tables),
        ("BLOCKS", blocks_text),
        ("ENTITIES", entities),
        ("OBJECTS", objects),
    ] {
        p(&mut s, 0, "SECTION");
        p(&mut s, 2, name);
        s.push_str(&body);
        p(&mut s, 0, "ENDSEC");
    }
    p(&mut s, 0, "EOF");
    DxfFile {
        name: dxf_file_name(&sheet.label),
        text: s,
        skipped_images: w.skipped_images,
    }
}

/// The symbol geometries `items` insert, groups included.
fn collect_instances(items: &[Item], out: &mut std::collections::BTreeSet<SymbolGeomId>) {
    for i in items {
        match &i.kind {
            ItemKind::Instance(inst) => {
                out.insert(inst.symbol);
            }
            ItemKind::Group(g) => collect_instances(&g.items, out),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_are_millimetres_with_up_to_six_decimals() {
        assert_eq!(mm(0), "0");
        assert_eq!(mm(1_500_000), "1.5");
        assert_eq!(mm(-250), "-0.00025");
        assert_eq!(mm(297_000_000), "297");
    }

    #[test]
    fn text_outside_windows_1252_is_escaped() {
        assert_eq!(dxf_text("∞ 2,5 mm²"), "\\U+221E 2,5 mm²");
        assert_eq!(dxf_text("50%%"), "50%%%");
    }
}
