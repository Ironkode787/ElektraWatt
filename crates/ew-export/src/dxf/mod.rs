// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The DXF writer (doc 09 §4.4, §4.9, FR-OUT-015): one AutoCAD 2010 (AC1024) file per page from the page's sheet
//! layers, in UTF-8, in millimetres (`$INSUNITS` 4), y up from the sheet's bottom edge, and the picture files its pages
//! name.
//!
//! # Object model
//!
//! | Input | DXF |
//! |---|---|
//! | a page | `<label>.dxf` (as [`crate::svg_file_name`] names SVG files); `$EXTMIN`/`$EXTMAX` and `$LIMMIN`/`$LIMMAX` the sheet, so a re-import puts the drawing back where it was |
//! | a layer of the page | a DXF layer by [`DxfLayers`]: frame, symbols, wires, texts, graphics, tables (and the underlay when printed) |
//! | `Instance` | an `INSERT` of a `BLOCK` per symbol geometry, scaled by the page-to-sheet map, turned and mirrored as the instance's transform, on the symbols layer; the block's content on layer `0`, named in words (the symbol's name and its variant, [`DxfOptions::symbol_names`]: `Coil R0`; else `EW_<symbol>_<variant letter>`), unique in the set |
//! | the texts right after an `Instance` with its entity | the symbol's own texts, in its block (the first insert's; an insert whose own texts differ keeps them as `TEXT`s) |
//! | [`DxfSheet::attributes`] | the `ATTRIB`s of the insert with that entity, named as given (`_2`, `_3` for a name again), on the texts layer; an `ATTDEF` per name in the block, where its first insert has it |
//! | `Path` | one `LWPOLYLINE` per subpath (closed with `70 = 1`); quadratic and cubic Béziers flattened to within [`FLATNESS_NM`]; the stroke width as the nearest standard lineweight; a dash pattern as an `LTYPE` |
//! | a filled `Path` | a solid `HATCH` per closed subpath; a colour other than black as a true colour (420) |
//! | `Glyphs` | a `TEXT` at the start of its glyphs on the baseline, its face's cap height ([`DxfFace::cap`]: 0.714 em for the Noto faces), angle and width factor (`text`); one `STYLE` per face, naming its font file and its family (doc 09 §4.8) |
//! | a `Link::Source` | a hyperlink (`PE_URL`) on its texts to the file of the page that draws the target |
//! | `Group` | its transform applied to its items; a clip is not written |
//! | `Image` | a PNG or JPEG as an `IMAGE` of a file written beside the pages (`picture`); one without bytes is not written, counted |
//!
//! # Determinism
//!
//! Every coordinate comes from integer nanometres, written in millimetres with up to six decimals; handles count
//! up from one start in file order; no date, user name or random GUID is written. The same pages give the same bytes
//! on every platform.

mod picture;
mod text;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;

use ew_dl::{
    Brush, Destination, DisplayList, EntityRef, FontKey, ImageData, Item, ItemKind, Layer, Link,
    MAX_INSTANCE_DEPTH, PathEl, ShapedRun, SymbolGeomId, Transform,
};
use ew_render_cpu::FontBytes;
use ew_units::{Nm, Orient2, Pt2, Rect};
use uuid::Uuid;

use picture::{PagePictures, Pictures};
use text::Placed;

/// The tolerance Béziers are flattened to: 0.01 mm.
pub const FLATNESS_NM: f64 = 10_000.0;

/// The extension of a PNG picture beside the pages.
const PNG: &str = "png";

/// The extension of a JPEG picture beside the pages.
const JPG: &str = "jpg";

/// The extensions of the files a DXF set holds: its pages' and its pictures' (`picture`).
pub const DXF_SET_EXTENSIONS: [&str; 3] = ["dxf", PNG, JPG];

/// The characters a DXF layer or block name may not hold, besides control characters.
const NAME_FORBIDDEN: &str = "<>/\\\":;?*|,='";

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
    /// The refusal in words for a class that is none of [`DxfLayers::classes`], an empty name, a name of more than 255
    /// characters or one with a character a DXF layer name may not hold (a control character or one of
    /// `< > / \ " : ; ? * | , = '`).
    pub fn set(&mut self, class: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("the layer class `{class}` needs a name"));
        }
        if name.chars().count() > 255 {
            return Err(format!(
                "the layer name for `{class}` is longer than 255 characters"
            ));
        }
        if let Some(c) = name
            .chars()
            .find(|c| c.is_control() || NAME_FORBIDDEN.contains(*c))
        {
            return Err(format!(
                "the layer name `{name}` may not hold {}; a DXF layer name cannot",
                if c.is_control() {
                    "a control character".to_owned()
                } else {
                    format!("`{c}`")
                }
            ));
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

    /// The names in the order the LAYER table lists them, each once (two classes may share a name) and without layer
    /// `0`, which the table starts with; DXF names compare without regard to case.
    fn all(&self) -> Vec<String> {
        let mut seen = BTreeSet::from(["0".to_owned()]);
        self.clone()
            .named_mut()
            .into_iter()
            .map(|(_, n)| n.clone())
            .filter(|n| seen.insert(n.to_uppercase()))
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
    /// The page's own destination: a link to the page leads to this file.
    pub destination: Destination,
    /// The layers written, in paint order, on the sheet.
    pub layers: Vec<(Layer, DisplayList)>,
    /// The symbol geometry of the instances, in the symbol frame, unscaled.
    pub geometry: BTreeMap<SymbolGeomId, Arc<DisplayList>>,
    /// The page-to-sheet scale in parts per 10⁹: the scale of every insert.
    pub scale_ppb: u32,
    /// The texts each symbol insert carries as its attributes, by the insert's entity, in print order; they are in no
    /// layer list. Those of an entity that inserts nothing are written as texts on the texts layer.
    pub attributes: BTreeMap<EntityRef, Vec<DxfAttribute>>,
}

/// One text a symbol's insert carries as an attribute (doc 09 §4.9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DxfAttribute {
    /// The attribute's name (`TAG`): capitals, digits and `_`; a second of one name on one insert is written `_2`.
    pub tag: String,
    /// The line it prints, on the sheet: a glyph run, or a group of the runs of one line (two faces); its link is the
    /// attribute's hyperlink.
    pub line: Item,
}

/// DXF export settings.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DxfOptions {
    /// The layer names.
    pub layers: DxfLayers,
    /// The text style of each face the runs name; a face without an entry is written as Noto Sans Regular.
    pub faces: BTreeMap<FontKey, DxfFace>,
    /// The symbols' names in words (`Coil`), by symbol id: a block is named by its symbol's and its variant.
    pub symbol_names: BTreeMap<Uuid, String>,
    /// The hyperlinks name the pages' `.dwg` files, for a set a converter turns into DWG; else their `.dxf` files.
    pub dwg_links: bool,
}

/// How a DXF names one face (doc 09 §4.8): a DXF cannot carry a font, so its `STYLE` names the face and the font file a
/// reader draws it with, and its `TEXT`s are as high as the face's capitals.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DxfFace {
    /// The family (`Noto Sans`): the style's name, `EW-NOTO-SANS`, and the family its extended data names.
    pub family: String,
    /// Whether it is the bold face: the style's name ends `-BOLD`.
    pub bold: bool,
    /// The font file the style names (group 3: `NotoSans-Regular.ttf`).
    pub file: String,
    /// The face's cap height and its units per em: a `TEXT`'s height is the run's em height times their ratio.
    pub cap: (i32, u16),
    /// The face's file, whose advance widths give a text's width factor; without it a text has none.
    pub font: Option<FontBytes>,
}

impl DxfFace {
    /// The face a run without an entry is written as: Noto Sans Regular, 714 of its 1000 units high.
    fn noto() -> DxfFace {
        DxfFace {
            family: "Noto Sans".to_owned(),
            bold: false,
            file: "NotoSans-Regular.ttf".to_owned(),
            cap: (714, 1000),
            font: None,
        }
    }
}

/// One written DXF file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DxfFile {
    /// The file name, `<label>.dxf`.
    pub name: String,
    /// The document.
    pub text: String,
    /// Images not written: without bytes, or bytes the renderers' decoder does not take.
    pub skipped_images: usize,
}

/// A picture file the pages name, written beside them (doc 09 §4.9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DxfPicture {
    /// The file name, `picture-<the first 12 hex digits of its SHA-256>.png` (or `.jpg`).
    pub name: String,
    /// The file, as the project holds it.
    pub bytes: Arc<[u8]>,
}

/// A written DXF set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DxfSet {
    /// One file per page, in page order.
    pub files: Vec<DxfFile>,
    /// The picture files the pages name, each once, in the order of their names.
    pub pictures: Vec<DxfPicture>,
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

/// The name of a symbol geometry whose symbol has no name in words.
fn block_name(id: &SymbolGeomId) -> String {
    let mut s = format!("EW_{}_{}", id.symbol.simple(), id.variant.letter());
    if let Some(e) = id.extra {
        let _ = write!(s, "_{e}");
    }
    s
}

/// A variant as the program names it (doc 07 §2.3).
fn variant_name(o: Orient2) -> &'static str {
    match o {
        Orient2::R0 => "R0",
        Orient2::R90 => "R90",
        Orient2::R180 => "R180",
        Orient2::R270 => "R270",
        Orient2::M0 => "M0",
        Orient2::M90 => "M90",
        Orient2::M180 => "M180",
        Orient2::M270 => "M270",
    }
}

/// A name in words as a DXF name may hold it: a character it may not hold as `_`, no leading `*` (an anonymous block's
/// mark), at most 200 characters (room for a number within the 255 of a DWG).
fn dxf_name(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| {
            if c.is_control() || c == '`' || NAME_FORBIDDEN.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    s.trim()
        .trim_start_matches('*')
        .trim()
        .chars()
        .take(200)
        .collect()
}

/// The block name of every symbol geometry the set inserts: its symbol's name and its variant (`Coil R0`), else
/// [`block_name`]; a name taken already (DXF names compare without regard to case) gets ` 2`, ` 3` in the order of the
/// keys, so a geometry has one name in every file of the set.
fn block_names(pages: &[DxfSheet], names: &BTreeMap<Uuid, String>) -> BTreeMap<SymbolGeomId, String> {
    let mut used = BTreeSet::new();
    for p in pages {
        for (_, list) in &p.layers {
            collect_instances(&list.items, &mut used);
        }
    }
    let mut taken = BTreeSet::new();
    used.into_iter()
        .map(|id| {
            let base = names
                .get(&id.symbol)
                .map(|n| dxf_name(n))
                .filter(|n| !n.is_empty())
                .map_or_else(
                    || block_name(&id),
                    |n| format!("{n} {}", variant_name(id.variant)),
                );
            let mut name = base.clone();
            let mut n = 1;
            while !taken.insert(name.to_uppercase()) {
                n += 1;
                name = format!("{base} {n}");
            }
            (id, name)
        })
        .collect()
}

/// The file each destination of the set lies in: the first page (in order) that draws it, or that it names.
fn link_files(pages: &[DxfSheet], dwg: bool) -> BTreeMap<Destination, String> {
    fn targets(items: &[Item], file: &str, out: &mut BTreeMap<Destination, String>) {
        for i in items {
            if let Some(Link::Target(d)) = i.link {
                out.entry(d).or_insert_with(|| file.to_owned());
            }
            if let ItemKind::Group(g) = &i.kind {
                targets(&g.items, file, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    for p in pages {
        let dxf = dxf_file_name(&p.label);
        let file = match dxf.strip_suffix(".dxf") {
            Some(stem) if dwg => format!("{stem}.dwg"),
            _ => dxf,
        };
        out.entry(p.destination).or_insert_with(|| file.clone());
        for (_, list) in &p.layers {
            targets(&list.items, &file, &mut out);
        }
    }
    out
}

/// Whether an item is one line of text: a glyph run, or a group of the runs of one line. The DXF set's glue of
/// `ew-api` finds the attributes of a symbol's insert among such lines.
pub fn is_line(item: &Item) -> bool {
    match &item.kind {
        ItemKind::Glyphs(_) => true,
        ItemKind::Group(g) => {
            !g.items.is_empty() && g.items.iter().all(|i| matches!(i.kind, ItemKind::Glyphs(_)))
        }
        _ => false,
    }
}

/// `tag`, or `tag_2`, `tag_3` … when an insert has it already.
fn unique(tag: &str, taken: &mut BTreeSet<String>) -> String {
    let mut name = tag.to_owned();
    let mut n = 1;
    while !taken.insert(name.clone()) {
        n += 1;
        name = format!("{tag}_{n}");
    }
    name
}

/// What the whole set decides for each of its files.
struct Set {
    /// The file of each destination ([`link_files`]).
    links: BTreeMap<Destination, String>,
    /// The name of each block ([`block_names`]).
    names: BTreeMap<SymbolGeomId, String>,
}

/// The writer of one file: its handles, its line types and styles, and the sections' text.
struct Writer<'a> {
    opts: &'a DxfOptions,
    set: &'a Set,
    sheet: &'a DxfSheet,
    pictures: &'a mut Pictures,
    height: Nm,
    next: u64,
    /// Dash patterns (on/off lengths in nm) by first use: `EW_DASH_<n>`.
    dashes: Vec<Vec<i64>>,
    /// Faces by first use.
    faces: Vec<FontKey>,
    skipped_images: usize,
    /// The own texts of each block, in its frame: its first insert's.
    block_texts: BTreeMap<SymbolGeomId, Vec<Placed>>,
    /// The attribute definitions of each block, in its frame: each name where its first insert has it.
    attdefs: BTreeMap<SymbolGeomId, Vec<(String, Placed)>>,
    /// The entities whose attributes an insert has written.
    inserted: BTreeSet<EntityRef>,
    /// The pictures of this page.
    page_pictures: PagePictures,
}

/// One entity's text, before its handle and owner are known.
struct Ent {
    kind: &'static str,
    layer: String,
    body: String,
    /// The entities it owns, written after it and closed by a `SEQEND`: an insert's attributes.
    children: Vec<Ent>,
    /// The picture of an `IMAGE` (its index on the page), whose definition and reactor end its body.
    image: Option<usize>,
}

impl Ent {
    fn new(kind: &'static str, layer: String, body: String) -> Ent {
        Ent {
            kind,
            layer,
            body,
            children: Vec::new(),
            image: None,
        }
    }
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

    /// A point in nanometres in the frame of the space it is written in (DXF's, y up).
    fn frame(&self, p: Pt2, block: bool) -> (i64, i64) {
        if block {
            (p.x.0, -p.y.0)
        } else {
            (p.x.0, self.height.0 - p.y.0)
        }
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

    fn face(&self, font: FontKey) -> DxfFace {
        self.opts.faces.get(&font).cloned().unwrap_or_else(DxfFace::noto)
    }

    /// The file a link source leads to, when its target is among the set's pages.
    fn link_file(&self, link: Option<Link>) -> Option<String> {
        match link? {
            Link::Source(d) => self.set.links.get(&d).cloned(),
            Link::Target(_) => None,
        }
    }

    /// The layer of an item of `kind` (`path`, `text`, `insert`) on the page layer `dl` (`None`: block content, `0`).
    fn layer(&self, dl: Option<Layer>, kind: &str) -> String {
        let l = &self.opts.layers;
        match dl {
            None => "0".to_owned(),
            Some(Layer::Frame) => l.frame.clone(),
            Some(Layer::Tables) => l.tables.clone(),
            Some(Layer::Underlay) => l.underlay.clone(),
            Some(Layer::Wires) if kind == "path" => l.wires.clone(),
            _ if kind == "text" => l.texts.clone(),
            _ if kind == "insert" => l.symbols.clone(),
            _ => l.graphics.clone(),
        }
    }

    /// A run under `t` as a text in the frame of its space.
    fn placed_run(&mut self, run: &ShapedRun, t: Transform, block: bool, link: Option<Link>) -> Placed {
        let (at, angle) = text::start(run, t);
        let face = self.face(run.font);
        let (x, y) = self.frame(at, block);
        Placed {
            x,
            y,
            height: text::cap_height(run.size, &face),
            angle,
            factor: text::width_factor(&[(run, &face)]),
            style: self.style(run.font),
            text: run.text.clone(),
            colour: text::colour(run.brush),
            link: self.link_file(link),
        }
    }

    /// One line of text on the sheet as one text: a run as itself; the runs of a group (two faces) from the first's
    /// start, in the face that draws most of its characters, with the width factor of them all.
    fn placed_line(&mut self, line: &Item) -> Option<Placed> {
        match &line.kind {
            ItemKind::Glyphs(run) => Some(self.placed_run(run, Transform::IDENTITY, false, line.link)),
            ItemKind::Group(g) if is_line(line) => {
                let t = g.transform.unwrap_or(Transform::IDENTITY);
                let runs: Vec<&ShapedRun> = g
                    .items
                    .iter()
                    .filter_map(|i| match &i.kind {
                        ItemKind::Glyphs(r) => Some(&**r),
                        _ => None,
                    })
                    .collect();
                let main = runs
                    .iter()
                    .copied()
                    .rev()
                    .max_by_key(|r| r.text.chars().count())?;
                let first = runs.first()?;
                let (at, angle) = text::start(first, t);
                let faces: Vec<DxfFace> = runs.iter().map(|r| self.face(r.font)).collect();
                let parts: Vec<(&ShapedRun, &DxfFace)> = runs.iter().copied().zip(&faces).collect();
                let face = self.face(main.font);
                let (x, y) = self.frame(at, false);
                Some(Placed {
                    x,
                    y,
                    height: text::cap_height(main.size, &face),
                    angle,
                    factor: text::width_factor(&parts),
                    style: self.style(main.font),
                    text: runs.iter().map(|r| r.text.as_str()).collect(),
                    colour: text::colour(main.brush),
                    link: self.link_file(line.link),
                })
            }
            _ => None,
        }
    }

    /// The entities of `items` drawn under `t`, on `dl` (`None`: block content, symbol frame, y down); `link` the link
    /// of the group they are in.
    fn items(
        &mut self,
        items: &[Item],
        t: Transform,
        dl: Option<Layer>,
        link: Option<Link>,
        depth: usize,
        out: &mut Vec<Ent>,
    ) {
        let block = dl.is_none();
        let mut i = 0;
        while i < items.len() {
            let item = &items[i];
            i += 1;
            let link = item.link.or(link);
            match &item.kind {
                ItemKind::Path(p) => {
                    if p.stroke.is_none() && p.fill.is_none() {
                        continue;
                    }
                    let lay = self.layer(dl, "path");
                    for (pts, closed) in subpaths(&p.path.els, t) {
                        if let Some(fill) = &p.fill
                            && pts.len() >= 3
                        {
                            out.push(self.hatch(&pts, fill, &lay, block));
                        }
                        if let Some(stroke) = &p.stroke {
                            let mut body = String::new();
                            if let Some(d) = stroke.drawn_dash() {
                                let segs: Vec<i64> = d.segments.iter().map(|n| n.0).collect();
                                let name = self.dash(&segs);
                                let _ = write!(body, "  6\n{name}\n");
                            }
                            let _ = write!(body, "370\n{}\n", lineweight(stroke.width));
                            if let Some(c) = text::colour(stroke.brush) {
                                let _ = write!(body, "420\n{c}\n");
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
                            out.push(Ent::new("LWPOLYLINE", lay.clone(), body));
                        }
                    }
                }
                ItemKind::Glyphs(run) => {
                    let lay = self.layer(dl, "text");
                    let placed = self.placed_run(run, t, block, link);
                    out.push(Ent::new("TEXT", lay, text::text_body(&placed)));
                }
                ItemKind::Image(img) => {
                    let lay = self.layer(dl, "image");
                    match self.image(&img.image, img.rect, t, block) {
                        Some((body, k)) => out.push(Ent {
                            image: Some(k),
                            ..Ent::new("IMAGE", lay, body)
                        }),
                        None => self.skipped_images += 1,
                    }
                }
                ItemKind::Group(g) => {
                    let inner = g.transform.map_or(t, |gt| gt.then(t));
                    self.items(&g.items, inner, dl, link, depth, out);
                }
                ItemKind::Instance(inst) => {
                    if block {
                        // An instance inside symbol geometry is drawn in place.
                        if depth < MAX_INSTANCE_DEPTH
                            && let Some(g) = self.sheet.geometry.get(&inst.symbol)
                        {
                            let inner = inst.transform.then(t);
                            self.items(&g.items, inner, dl, link, depth + 1, out);
                        }
                        continue;
                    }
                    // The symbol's own texts: the lines right after it with its entity.
                    let own = match item.entity {
                        Some(e) => items[i..]
                            .iter()
                            .take_while(|x| x.entity == Some(e) && is_line(x))
                            .count(),
                        None => 0,
                    };
                    let full = inst.transform.then(t);
                    if self.own_texts(inst.symbol, full, &items[i..i + own]) {
                        i += own;
                    }
                    let (ent, beside) = insert(self, item, inst.symbol, full, dl);
                    out.push(ent);
                    out.extend(beside);
                }
            }
        }
    }

    /// Takes the symbol's own texts `own` of an insert of `symbol` under `full` into its block when the insert is
    /// unturned: the first insert's become the block's, a later insert's are left out when they are the block's within a
    /// micrometre. Whether they were taken (else they are written as texts of their own).
    fn own_texts(&mut self, symbol: SymbolGeomId, full: Transform, own: &[Item]) -> bool {
        if own.is_empty() || !unturned(full) {
            return false;
        }
        let (ax, ay) = self.frame(full.apply(Pt2::ORIGIN), false);
        let mut placed = Vec::new();
        for line in own {
            let (t, runs): (Transform, Vec<&ShapedRun>) = match &line.kind {
                ItemKind::Glyphs(r) => (Transform::IDENTITY, vec![&**r]),
                ItemKind::Group(g) => (
                    g.transform.unwrap_or(Transform::IDENTITY),
                    g.items
                        .iter()
                        .filter_map(|i| match &i.kind {
                            ItemKind::Glyphs(r) => Some(&**r),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => return false,
            };
            for run in runs {
                let p = self.placed_run(run, t, false, None);
                placed.push(p.in_block(ax, ay, self.sheet.scale_ppb));
            }
        }
        match self.block_texts.get(&symbol) {
            None => {
                self.block_texts.insert(symbol, placed);
                true
            }
            Some(b) => b.len() == placed.len() && b.iter().zip(&placed).all(|(a, p)| a.near(p)),
        }
    }

    /// The picture of an image item stretched over `rect` under `t`: the groups of its `IMAGE` up to its definition, and
    /// its index on the page; `None` for an image without bytes the decoder takes.
    fn image(&mut self, image: &ImageData, rect: Rect, t: Transform, block: bool) -> Option<(String, usize)> {
        let picture = self.pictures.get(image)?;
        let (w, h) = (f64::from(picture.pixels.0), f64::from(picture.pixels.1));
        let corner = |x: Nm, y: Nm| self.frame(t.apply(Pt2::new(x, y)), block);
        // The picture's lower left corner, and its bottom and left edges, in the frame (y up).
        let ll = corner(rect.min().x, rect.max().y);
        let lr = corner(rect.max().x, rect.max().y);
        let ul = corner(rect.min().x, rect.min().y);
        let per = |a: (i64, i64), n: f64| ((a.0 - ll.0) as f64 / n / 1e6, (a.1 - ll.1) as f64 / n / 1e6);
        let (u, v) = (per(lr, w), per(ul, h));
        let size = |p: (f64, f64)| (p.0 * p.0 + p.1 * p.1).sqrt();
        let k = self.page_pictures.index(picture, (size(u), size(v)));
        let body = format!(
            "100\nAcDbRasterImage\n 90\n0\n 10\n{}\n 20\n{}\n 30\n0\n 11\n{}\n 21\n{}\n 31\n0\n 12\n{}\n 22\n{}\n 32\n0\n 13\n{}\n 23\n{}\n",
            mm(ll.0),
            mm(ll.1),
            real(u.0),
            real(u.1),
            real(v.0),
            real(v.1),
            real(w),
            real(h),
        );
        Some((body, k))
    }

    fn hatch(&mut self, pts: &[Pt2], fill: &Brush, layer: &str, block: bool) -> Ent {
        let mut body = String::new();
        if let Some(c) = text::colour(*fill) {
            let _ = write!(body, "420\n{c}\n");
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
        Ent::new("HATCH", layer.to_owned(), body)
    }

    /// Writes `e` owned by `owner` with its handle, then the entities it owns and their `SEQEND`.
    fn write(&mut self, out: &mut String, e: Ent, owner: &str) {
        let h = self.handle();
        let _ = write!(
            out,
            "  0\n{}\n  5\n{h}\n330\n{owner}\n100\nAcDbEntity\n  8\n{}\n{}",
            e.kind,
            text::dxf_text(&e.layer),
            e.body
        );
        if let Some(k) = e.image {
            let reactor = self.handle();
            out.push_str(&self.page_pictures.image_tail(k, &reactor));
            self.page_pictures.images.push((k, h.clone(), reactor));
        }
        if !e.children.is_empty() {
            let layer = e.layer;
            for c in e.children {
                self.write(out, c, &h);
            }
            let sh = self.handle();
            let _ = write!(
                out,
                "  0\nSEQEND\n  5\n{sh}\n330\n{h}\n100\nAcDbEntity\n  8\n{}\n",
                text::dxf_text(&layer)
            );
        }
    }
}

/// Whether an insert's transform is a translation, so that its block's frame is the sheet's moved and scaled.
fn unturned(t: Transform) -> bool {
    t.orient == Orient2::R0 && t.angle.normalized().0 == 0
}

/// The `INSERT` of the instance `item` of `symbol` under `full`, with the attributes the sheet gives its entity as its
/// `ATTRIB`s (and, when it is unturned, its block's `ATTDEF`s); and an attribute that is no line of text drawn as it is
/// beside it, never lost.
fn insert(
    w: &mut Writer<'_>,
    item: &Item,
    symbol: SymbolGeomId,
    full: Transform,
    dl: Option<Layer>,
) -> (Ent, Vec<Ent>) {
    let lay = w.layer(dl, "insert");
    let at = full.apply(Pt2::ORIGIN);
    let (x, y) = w.xy(at);
    // The orientation's matrix with the turn, in the sheet's y-down frame, seen in DXF's y-up frame: a quarter turn
    // clockwise on the sheet is −90°, a mirror a negative x scale.
    let turns = i64::from(full.orient.quarter_turns());
    let angle = (f64::from(full.angle.0) / 1000.0 - 90.0 * turns as f64).rem_euclid(360.0);
    let s = f64::from(w.sheet.scale_ppb) / 1e9;
    let sx = if full.orient.is_mirrored() { -s } else { s };
    let mut children = Vec::new();
    let mut beside = Vec::new();
    if let Some(e) = item.entity
        && let Some(attrs) = w.sheet.attributes.get(&e)
    {
        w.inserted.insert(e);
        let (ax, ay) = w.frame(at, false);
        let texts = w.layer(Some(Layer::Labels), "text");
        let mut tags = BTreeSet::new();
        for a in attrs {
            match w.placed_line(&a.line) {
                Some(p) => {
                    let tag = unique(&a.tag, &mut tags);
                    if unturned(full) {
                        let defs = w.attdefs.entry(symbol).or_default();
                        if !defs.iter().any(|(t, _)| *t == tag) {
                            defs.push((tag.clone(), p.in_block(ax, ay, w.sheet.scale_ppb)));
                        }
                    }
                    children.push(Ent::new("ATTRIB", texts.clone(), text::attrib_body(&p, &tag)));
                }
                None => w.items(
                    std::slice::from_ref(&a.line),
                    Transform::IDENTITY,
                    Some(Layer::Labels),
                    None,
                    0,
                    &mut beside,
                ),
            }
        }
    }
    let follows = if children.is_empty() { "" } else { " 66\n1\n" };
    let body = format!(
        "100\nAcDbBlockReference\n{follows}  2\n{}\n 10\n{x}\n 20\n{y}\n 30\n0\n 41\n{}\n 42\n{}\n 43\n{}\n 50\n{}\n",
        text::dxf_text(
            &w.set
                .names
                .get(&symbol)
                .cloned()
                .unwrap_or_else(|| block_name(&symbol))
        ),
        real(sx),
        real(s),
        real(s),
        real(angle),
    );
    (
        Ent {
            children,
            ..Ent::new("INSERT", lay, body)
        },
        beside,
    )
}

/// The STYLE name of a face: its family in capitals with `-` for spaces, `-BOLD` for a bold face.
fn style_name(opts: &DxfOptions, font: FontKey) -> String {
    let face = opts.faces.get(&font).cloned().unwrap_or_else(DxfFace::noto);
    let mut s = format!("EW-{}", face.family.to_ascii_uppercase().replace(' ', "-"));
    if face.bold {
        s.push_str("-BOLD");
    }
    text::dxf_text(&dxf_name(&s))
}

/// The DXF file of each page and the picture files they name (see the module documentation).
pub fn dxf(pages: &[DxfSheet], options: &DxfOptions) -> DxfSet {
    let set = Set {
        links: link_files(pages, options.dwg_links),
        names: block_names(pages, &options.symbol_names),
    };
    let mut pictures = Pictures::default();
    let files = pages
        .iter()
        .map(|p| page(p, options, &set, &mut pictures))
        .collect();
    DxfSet {
        files,
        pictures: pictures.files(),
    }
}

/// One page's file.
fn page(sheet: &DxfSheet, opts: &DxfOptions, set: &Set, pictures: &mut Pictures) -> DxfFile {
    let mut w = Writer {
        opts,
        set,
        sheet,
        pictures,
        height: sheet.height,
        next: 0x20,
        dashes: Vec::new(),
        faces: Vec::new(),
        skipped_images: 0,
        block_texts: BTreeMap::new(),
        attdefs: BTreeMap::new(),
        inserted: BTreeSet::new(),
        page_pictures: PagePictures::default(),
    };
    // The entities of the model space and the symbol blocks it inserts, collected first: their line types, styles and
    // pictures make the tables and objects.
    let mut model = Vec::new();
    for (layer, list) in &sheet.layers {
        w.items(
            &list.items,
            Transform::IDENTITY,
            Some(*layer),
            None,
            0,
            &mut model,
        );
    }
    // The attributes of an entity that inserts nothing, as texts of their own.
    for (e, attrs) in &sheet.attributes {
        if !w.inserted.contains(e) {
            for a in attrs {
                w.items(
                    std::slice::from_ref(&a.line),
                    Transform::IDENTITY,
                    Some(Layer::Labels),
                    None,
                    0,
                    &mut model,
                );
            }
        }
    }
    let mut used = BTreeSet::new();
    for (_, list) in &sheet.layers {
        collect_instances(&list.items, &mut used);
    }
    let mut blocks: Vec<(String, Vec<Ent>, bool)> = Vec::new();
    for id in &used {
        let mut ents = Vec::new();
        if let Some(g) = sheet.geometry.get(id) {
            w.items(&g.items, Transform::IDENTITY, None, None, 0, &mut ents);
        }
        for p in w.block_texts.get(id).into_iter().flatten() {
            ents.push(Ent::new("TEXT", "0".to_owned(), text::text_body(p)));
        }
        let defs = w.attdefs.get(id).cloned().unwrap_or_default();
        for (tag, p) in &defs {
            ents.push(Ent::new("ATTDEF", "0".to_owned(), text::attdef_body(p, tag)));
        }
        let name = set.names.get(id).cloned().unwrap_or_else(|| block_name(id));
        blocks.push((name, ents, !defs.is_empty()));
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
            text::dxf_text(&name)
        ));
    }
    table(&mut w, &mut tables, "LAYER", layers);
    let mut styles = vec![
        "100\nAcDbTextStyleTableRecord\n  2\nStandard\n 70\n0\n 40\n0\n 41\n1\n 50\n0\n 71\n0\n 42\n2.5\n  3\nNotoSans-Regular.ttf\n  4\n\n"
            .to_owned(),
    ];
    for f in w.faces.clone() {
        let face = w.face(f);
        styles.push(format!(
            "100\nAcDbTextStyleTableRecord\n  2\n{}\n 70\n0\n 40\n0\n 41\n1\n 50\n0\n 71\n0\n 42\n2.5\n  3\n{}\n  4\n\n{}",
            style_name(opts, f),
            text::dxf_text(&face.file),
            text::style_xdata(&face),
        ));
    }
    table(&mut w, &mut tables, "STYLE", styles);
    table(&mut w, &mut tables, "VIEW", Vec::new());
    table(&mut w, &mut tables, "UCS", Vec::new());
    table(
        &mut w,
        &mut tables,
        "APPID",
        vec![
            "100\nAcDbRegAppTableRecord\n  2\nACAD\n 70\n0\n".to_owned(),
            "100\nAcDbRegAppTableRecord\n  2\nPE_URL\n 70\n0\n".to_owned(),
        ],
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
    // The pictures' objects, which the `IMAGE`s point at.
    let dict_images = w.handle();
    let raster_vars = w.handle();
    w.page_pictures.defs = (0..w.page_pictures.used.len()).map(|_| w.handle()).collect();
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
    for ((name, _, _), h) in blocks.iter().zip(&block_records) {
        record(&mut tables, h, &text::dxf_text(name), "0");
    }
    tables.push_str("  0\nENDTAB\n");
    // The blocks section.
    let mut blocks_text = String::new();
    let block = |w: &mut Writer<'_>,
                 out: &mut String,
                 record: &str,
                 name: &str,
                 paper: bool,
                 attributes: bool,
                 ents: Vec<Ent>| {
        let h = w.handle();
        let space = if paper { " 67\n1\n" } else { "" };
        let name = text::dxf_text(name);
        // Flag 2: the block has attribute definitions.
        let flags = if attributes { 2 } else { 0 };
        let _ = write!(
            out,
            "  0\nBLOCK\n  5\n{h}\n330\n{record}\n100\nAcDbEntity\n{space}  8\n0\n100\nAcDbBlockBegin\n  2\n{name}\n 70\n{flags}\n 10\n0\n 20\n0\n 30\n0\n  3\n{name}\n  1\n\n"
        );
        for e in ents {
            w.write(out, e, record);
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
        false,
        Vec::new(),
    );
    block(
        &mut w,
        &mut blocks_text,
        &paper_record,
        "*Paper_Space",
        true,
        false,
        Vec::new(),
    );
    for ((name, ents, attributes), h) in blocks.into_iter().zip(&block_records) {
        block(&mut w, &mut blocks_text, h, &name, false, attributes, ents);
    }
    // The entities.
    let mut entities = String::new();
    for e in model {
        w.write(&mut entities, e, &model_record);
    }
    // The objects: the root dictionary, the group and layout dictionaries, the two layouts and the pictures' objects.
    let mut objects = String::new();
    let _ = write!(
        objects,
        "  0\nDICTIONARY\n  5\n{dict_root}\n330\n0\n100\nAcDbDictionary\n281\n1\n  3\nACAD_GROUP\n350\n{dict_group}\n{}  3\nACAD_LAYOUT\n350\n{dict_layout}\n",
        w.page_pictures.root_entries(&dict_images, &raster_vars),
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
    objects.push_str(&w.page_pictures.objects(&dict_root, &dict_images, &raster_vars));
    let seed = format!("{:X}", w.next);
    // The file.
    p(&mut s, 0, "SECTION");
    p(&mut s, 2, "HEADER");
    for (var, pairs) in [
        ("$ACADVER", vec![(1, "AC1024".to_owned())]),
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
        ("CLASSES", w.page_pictures.classes()),
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
fn collect_instances(items: &[Item], out: &mut BTreeSet<SymbolGeomId>) {
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

    /// The entities of a DXF file read back as their group-code pairs: each code 0 opens one.
    fn entities(text: &str) -> Vec<Vec<(i32, String)>> {
        let lines: Vec<&str> = text.lines().collect();
        let mut out: Vec<Vec<(i32, String)>> = Vec::new();
        for pair in lines.chunks(2) {
            let code: i32 = pair[0].trim().parse().expect("a group code");
            if code == 0 {
                out.push(Vec::new());
            }
            if let Some(e) = out.last_mut() {
                e.push((code, pair.get(1).copied().unwrap_or("").to_owned()));
            }
        }
        out
    }

    /// The first value of `code` in an entity.
    fn get(e: &[(i32, String)], code: i32) -> Option<&str> {
        e.iter().find(|(c, _)| *c == code).map(|(_, v)| v.as_str())
    }

    fn key(b: u8) -> FontKey {
        FontKey {
            sha256: [b; 32],
            face_index: 0,
        }
    }

    fn run_of(font: FontKey, text: &str, at: Pt2) -> Arc<ShapedRun> {
        Arc::new(ShapedRun {
            font,
            size: Nm(1_024_000),
            origin: at,
            transform: Transform::IDENTITY,
            glyphs: Vec::new(),
            text: text.to_owned(),
            brush: Brush::BLACK,
            layout_box: Rect::from_point(Pt2::ORIGIN),
        })
    }

    fn sheet(label: &str, layers: Vec<(Layer, DisplayList)>) -> DxfSheet {
        DxfSheet {
            label: label.to_owned(),
            width: Nm(420_000_000),
            height: Nm(297_000_000),
            destination: Destination::Page(Uuid::from_u128(label.parse().unwrap_or(0))),
            layers,
            geometry: BTreeMap::new(),
            scale_ppb: 1_000_000_000,
            attributes: BTreeMap::new(),
        }
    }

    fn mm_pt(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x * 1_000_000), Nm(y * 1_000_000))
    }

    #[test]
    fn lengths_are_millimetres_with_up_to_six_decimals() {
        assert_eq!(mm(0), "0");
        assert_eq!(mm(1_500_000), "1.5");
        assert_eq!(mm(-250), "-0.00025");
        assert_eq!(mm(297_000_000), "297");
    }

    /// Each face's style names its font file and its family, and a text is as high as that face's capitals; a face
    /// without an entry is Noto Sans Regular, 714 of 1000 units high (doc 09 §4.8, §4.9).
    #[test]
    fn a_text_is_as_high_as_its_faces_capitals_in_a_style_that_names_its_file_and_family() {
        let page = sheet(
            "1",
            vec![(
                Layer::Labels,
                DisplayList {
                    items: vec![
                        Item::glyphs(run_of(key(1), "K1", mm_pt(10, 50))),
                        Item::glyphs(run_of(key(2), "K2", mm_pt(20, 50))),
                    ],
                },
            )],
        );
        let options = DxfOptions {
            faces: BTreeMap::from([(
                key(1),
                DxfFace {
                    family: "Routed Gothic".to_owned(),
                    bold: false,
                    file: "routed-gothic.ttf".to_owned(),
                    cap: (736, 1024),
                    font: None,
                },
            )]),
            ..DxfOptions::default()
        };
        let ents = entities(&dxf(&[page], &options).files[0].text);
        let style = ents
            .iter()
            .find(|e| e[0].1 == "STYLE" && get(e, 2) == Some("EW-ROUTED-GOTHIC"))
            .expect("the face's style");
        assert_eq!(get(style, 3), Some("routed-gothic.ttf"));
        assert_eq!(get(style, 1001), Some("ACAD"));
        assert_eq!(get(style, 1000), Some("Routed Gothic"));
        let text = |t: &str| {
            ents.iter()
                .find(|e| e[0].1 == "TEXT" && get(e, 1) == Some(t))
                .unwrap()
        };
        assert_eq!(get(text("K1"), 40), Some("0.736"));
        assert_eq!(get(text("K1"), 7), Some("EW-ROUTED-GOTHIC"));
        assert_eq!(get(text("K2"), 40), Some("0.731136"));
        assert_eq!(get(text("K2"), 7), Some("EW-NOTO-SANS"));
    }

    /// The PDF kerns ("AV" stands tighter than its glyphs' advances): its text gets the width factor that brings the
    /// face's own advances to the PDF's width; a text without kerning ("HH") gets none.
    #[test]
    fn a_kerned_text_gets_the_width_factor_of_its_width_in_the_pdf() {
        let fonts = ew_text::bundled();
        let style = ew_text::TextStyle::regular(Nm(2_500_000));
        let face = fonts.face_for(&style);
        let dxf_face = DxfFace {
            family: face.family_name.clone(),
            bold: false,
            file: "NotoSans-Regular.ttf".to_owned(),
            cap: (face.metrics.cap_height, face.metrics.units_per_em),
            font: fonts.face_data(face.key).map(|d| FontBytes {
                data: Arc::from(d),
                index: 0,
            }),
        };
        let kerned = fonts.shape("AV", &style, None);
        let factor = text::width_factor(&[(&kerned.run, &dxf_face)]).expect("a factor");
        assert!(factor < 1.0, "{factor}");
        let plain = fonts.shape("HH", &style, None);
        assert_eq!(text::width_factor(&[(&plain.run, &dxf_face)]), None);
        // Without the face's file there is nothing to measure.
        let named = DxfFace {
            font: None,
            ..dxf_face
        };
        assert_eq!(text::width_factor(&[(&kerned.run, &named)]), None);
    }

    /// A symbol's block is named by its symbol's name and its variant, a character a DXF name cannot hold as `_`, a
    /// second geometry of the same name with ` 2`; a symbol without a name keeps its id.
    #[test]
    fn a_block_is_named_in_words_and_once_in_the_set() {
        let geom = |s: u128, variant, extra| SymbolGeomId {
            symbol: Uuid::from_u128(s),
            variant,
            extra,
        };
        let (coil, contact, other, nameless) = (
            geom(1, Orient2::R0, None),
            geom(2, Orient2::M90, None),
            geom(3, Orient2::R0, None),
            geom(4, Orient2::R0, None),
        );
        let boxed = geom(1, Orient2::R0, Some(0));
        let layer = |ids: &[SymbolGeomId]| {
            (
                Layer::Symbols,
                DisplayList {
                    items: ids
                        .iter()
                        .map(|id| Item::instance(*id, Transform::IDENTITY))
                        .collect(),
                },
            )
        };
        let pages = [
            sheet("1", vec![layer(&[coil, contact, nameless])]),
            sheet("2", vec![layer(&[boxed, other])]),
        ];
        let names = BTreeMap::from([
            (Uuid::from_u128(1), "Coil".to_owned()),
            (Uuid::from_u128(2), "NO contact: 1/2".to_owned()),
            (Uuid::from_u128(3), "coil".to_owned()),
        ]);
        let got = block_names(&pages, &names);
        assert_eq!(got[&coil], "Coil R0");
        assert_eq!(got[&boxed], "Coil R0 2");
        assert_eq!(got[&contact], "NO contact_ 1_2 M90");
        assert_eq!(got[&other], "coil R0 3", "names compare without regard to case");
        assert_eq!(got[&nameless], block_name(&nameless));
    }

    /// An insert carries the texts the sheet gives its entity as its attributes (a second line, a second of one name
    /// numbered), its block an attribute definition per name and the symbol's own text; the second insert's own text is
    /// the block's and is not written again; a text that links names the file of its target's page, `.dwg` on request.
    #[test]
    fn a_device_s_texts_are_attributes_of_its_insert_and_a_link_names_its_target_s_file() {
        let symbol = SymbolGeomId {
            symbol: Uuid::from_u128(9),
            variant: Orient2::R0,
            extra: None,
        };
        let (k1, k2) = (EntityRef(Uuid::from_u128(11)), EntityRef(Uuid::from_u128(12)));
        let target = Destination::Representation(k2.0);
        let placed = |e: EntityRef, at: Pt2| {
            vec![
                Item::instance(symbol, Transform::from_translation(at))
                    .with_entity(e)
                    .with_link(Link::Target(Destination::Representation(e.0))),
                Item::glyphs(run_of(key(0), "M", at + mm_pt(1, 1))).with_entity(e),
            ]
        };
        let mut first = sheet(
            "101",
            vec![(
                Layer::Symbols,
                DisplayList {
                    items: placed(k1, mm_pt(50, 50)),
                },
            )],
        );
        let attribute = |tag: &str, text: &str, at: Pt2| DxfAttribute {
            tag: tag.to_owned(),
            line: Item::glyphs(run_of(key(0), text, at)),
        };
        first.attributes.insert(
            k1,
            vec![
                attribute("TAG", "-K1", mm_pt(60, 40)),
                DxfAttribute {
                    line: attribute("", "102-B2", mm_pt(60, 44))
                        .line
                        .with_link(Link::Source(target)),
                    tag: "CROSS_REFERENCE".to_owned(),
                },
                attribute("CROSS_REFERENCE", "102-B3", mm_pt(60, 48)),
            ],
        );
        first.layers[0]
            .1
            .items
            .extend(placed(EntityRef(Uuid::from_u128(13)), mm_pt(150, 50)));
        let second = sheet(
            "102",
            vec![(
                Layer::Symbols,
                DisplayList {
                    items: placed(k2, mm_pt(50, 50)),
                },
            )],
        );
        let geometry = Arc::new(DisplayList::new());
        let pages: Vec<DxfSheet> = [first, second]
            .into_iter()
            .map(|mut p| {
                p.geometry.insert(symbol, Arc::clone(&geometry));
                p
            })
            .collect();
        let options = DxfOptions {
            symbol_names: BTreeMap::from([(symbol.symbol, "Coil".to_owned())]),
            ..DxfOptions::default()
        };
        let set = dxf(&pages, &options);
        let ents = entities(&set.files[0].text);
        assert_eq!(get(&ents[ents.len() - 1], 0), Some("EOF"));
        // The first insert, its three attributes in order, the closing SEQEND.
        let at = ents
            .iter()
            .position(|e| get(e, 0) == Some("INSERT"))
            .expect("an insert");
        assert_eq!(get(&ents[at], 2), Some("Coil R0"));
        assert_eq!(get(&ents[at], 66), Some("1"));
        let handle = get(&ents[at], 5).unwrap().to_owned();
        let attribs: Vec<(&str, &str)> = ents[at + 1..at + 4]
            .iter()
            .map(|e| {
                assert_eq!(get(e, 0), Some("ATTRIB"));
                assert_eq!(get(e, 330), Some(handle.as_str()), "owned by its insert");
                assert_eq!(get(e, 8), Some("EW-TEXTS"));
                (get(e, 2).unwrap(), get(e, 1).unwrap())
            })
            .collect();
        assert_eq!(
            attribs,
            [
                ("TAG", "-K1"),
                ("CROSS_REFERENCE", "102-B2"),
                ("CROSS_REFERENCE_2", "102-B3")
            ]
        );
        assert_eq!(get(&ents[at + 2], 1001), Some("PE_URL"));
        assert_eq!(get(&ents[at + 2], 1000), Some("102.dxf"));
        assert_eq!(get(&ents[at + 4], 0), Some("SEQEND"));
        // The block: the symbol's own text and a definition per name, the flag that it has them.
        let begin = ents
            .iter()
            .position(|e| get(e, 0) == Some("BLOCK") && get(e, 2) == Some("Coil R0"))
            .unwrap();
        assert_eq!(get(&ents[begin], 70), Some("2"));
        let end = begin
            + ents[begin..]
                .iter()
                .position(|e| get(e, 0) == Some("ENDBLK"))
                .unwrap();
        let content = &ents[begin + 1..end];
        let defs: Vec<&str> = content
            .iter()
            .filter(|e| get(e, 0) == Some("ATTDEF"))
            .map(|e| get(e, 2).unwrap())
            .collect();
        assert_eq!(defs, ["TAG", "CROSS_REFERENCE", "CROSS_REFERENCE_2"]);
        let own: Vec<_> = content.iter().filter(|e| get(e, 0) == Some("TEXT")).collect();
        assert_eq!(own.len(), 1);
        assert_eq!(
            (get(own[0], 1), get(own[0], 10), get(own[0], 20)),
            (Some("M"), Some("1"), Some("-1"))
        );
        assert!(
            !ents.iter().any(|e| get(e, 0) == Some("TEXT")
                && get(e, 1) == Some("M")
                && get(e, 330) != get(own[0], 330)),
            "neither insert writes the block's own text again"
        );
        // A link names the DWG files on request.
        let dwg = dxf(
            &pages,
            &DxfOptions {
                dwg_links: true,
                ..options
            },
        );
        assert!(
            entities(&dwg.files[0].text)
                .iter()
                .any(|e| get(e, 1000) == Some("102.dwg"))
        );
    }

    /// A PNG picture is an `IMAGE` stretched over its rectangle that names the picture file written once for the set,
    /// with its definition in the image dictionary and the classes of the picture objects.
    #[test]
    fn a_png_picture_is_an_image_of_a_file_beside_the_pages() {
        let mut bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut bytes, 4, 2);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            let mut w = e.write_header().unwrap();
            w.write_image_data(&[255u8; 4 * 2 * 4]).unwrap();
        }
        let image = Arc::new(ImageData {
            sha256: [0xab; 32],
            format: ew_dl::ImageFormat::Png,
            bytes: Arc::from(bytes),
        });
        let frame = |label: &str| {
            sheet(
                label,
                vec![(
                    Layer::Frame,
                    DisplayList {
                        items: vec![Item::image(
                            Arc::clone(&image),
                            Rect::from_points(mm_pt(10, 20), mm_pt(50, 40)),
                        )],
                    },
                )],
            )
        };
        let set = dxf(&[frame("1"), frame("2")], &DxfOptions::default());
        assert_eq!(set.pictures.len(), 1, "one file for the set");
        assert_eq!(set.pictures[0].name, "picture-abababababab.png");
        let ents = entities(&set.files[1].text);
        let img = ents
            .iter()
            .find(|e| get(e, 0) == Some("IMAGE"))
            .expect("an image");
        assert_eq!(get(img, 8), Some("EW-FRAME"));
        // The lower left corner on the y-up sheet, 10 mm a pixel along, 10 mm a pixel up.
        assert_eq!((get(img, 10), get(img, 20)), (Some("10"), Some("257")));
        assert_eq!((get(img, 11), get(img, 22)), (Some("10"), Some("10")));
        assert_eq!((get(img, 13), get(img, 23)), (Some("4"), Some("2")));
        let def = ents.iter().find(|e| get(e, 0) == Some("IMAGEDEF")).unwrap();
        assert_eq!(get(img, 340), get(def, 5));
        assert_eq!(get(def, 1), Some("picture-abababababab.png"));
        let reactor = ents
            .iter()
            .find(|e| get(e, 0) == Some("IMAGEDEF_REACTOR"))
            .unwrap();
        assert_eq!(get(img, 360), get(reactor, 5));
        assert_eq!(get(reactor, 330), get(img, 5));
        let classes: Vec<&str> = ents
            .iter()
            .filter(|e| get(e, 0) == Some("CLASS"))
            .map(|e| get(e, 1).unwrap())
            .collect();
        assert_eq!(
            classes,
            ["IMAGE", "IMAGEDEF", "IMAGEDEF_REACTOR", "RASTERVARIABLES"]
        );
        // Bytes that do not decode are not written, and counted.
        let broken = Arc::new(ImageData {
            sha256: [1; 32],
            format: ew_dl::ImageFormat::Png,
            bytes: Arc::from(&b"no png"[..]),
        });
        let page = sheet(
            "3",
            vec![(
                Layer::Frame,
                DisplayList {
                    items: vec![Item::image(broken, Rect::from_point(Pt2::ORIGIN))],
                },
            )],
        );
        let set = dxf(&[page], &DxfOptions::default());
        assert_eq!(set.files[0].skipped_images, 1);
        assert!(set.pictures.is_empty());
    }

    #[test]
    fn a_layer_name_a_dxf_file_cannot_hold_is_refused_in_words() {
        let mut layers = DxfLayers::default();
        layers.set("wires", "EW LEITUNGEN").unwrap();
        for bad in ["A/B", "A:B", "A\nB", "A\"B", "A,B", "A<B"] {
            let e = layers.set("wires", bad).unwrap_err();
            assert!(e.contains("may not hold"), "{bad:?}: {e}");
        }
        assert!(layers.set("wires", &"A".repeat(256)).unwrap_err().contains("255"));
        assert_eq!(layers.wires, "EW LEITUNGEN", "a refused name changes nothing");
    }

    #[test]
    fn the_layer_table_lists_each_name_once_and_never_layer_zero() {
        let mut layers = DxfLayers::default();
        layers.set("wires", "EW-FRAME").unwrap();
        layers.set("texts", "ew-frame").unwrap();
        layers.set("graphics", "0").unwrap();
        let all = layers.all();
        let mut upper: Vec<String> = all.iter().map(|n| n.to_uppercase()).collect();
        upper.sort();
        upper.dedup();
        assert_eq!(upper.len(), all.len(), "{all:?}");
        assert!(!all.iter().any(|n| n == "0"));
    }
}
