// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The SVG writer: one SVG document per page from the same sheet display lists the PDF writer draws
//! (doc 09 §4.4; FR-OUT-013, FR-OUT-019; AC-14).
//!
//! # Object model
//!
//! | Input | SVG |
//! |---|---|
//! | [`PdfPage`] | one file `<label>.svg` (characters outside `[A-Za-z0-9._-]` become `_`); `width`/`height` in mm, `viewBox` in mm; the root `id` is the page's destination name in NCName form (`ew-p-<uuid>`); a `<title>` with the page's outline title |
//! | `Path` | `<path d>` with fill (non-zero) and stroke: width, caps, joins, miter limit [`ew_dl::MITER_LIMIT`], dashes, opacity; a path with neither draws nothing unless it is a link source (then it is a transparent hit region, `pointer-events="all"`) |
//! | `Glyphs` | `<text>` with the run's text **as text** (FR-OUT-013), one `x` (and `y` where a glyph is offset) per character when characters and glyphs pair one to one, else the start and the run's `textLength`; `font-family`, `font-weight`, `font-style` and `font-size` from [`SvgOptions::faces`]; spaces kept (`white-space: pre`); `.notdef` glyphs dropped as in the PDF |
//! | `Group` | `<g transform>` with a `<clipPath>` rectangle when clipped |
//! | `Instance` | the resolved geometry under the instance transform in a `<g>`; unresolved: skipped and counted |
//! | `Image` | skipped and counted, as in the PDF |
//! | [`Link::Target`] | the item's `id`: the destination name with `:` replaced by `-` (`ew-r-<uuid>`), written once per page on the first item of that target that is drawn; links point at the first page in page order that draws it (ids are written for link targets only) |
//! | [`Link::Source`] | `<a href="<file>#<id>">` around the item (a page destination links the file); a destination outside the export prints as plain text |
//!
//! # Determinism
//!
//! Numbers are written from integer nanometres: coordinates and lengths in mm with at most 3 decimals
//! (rounded half away from zero to whole micrometres, trailing zeros dropped); a transform's linear part
//! comes from [`Transform::to_affine`] (`ew-dl`'s `libm` numerics, exact for quarter turns) with at most 6
//! decimals. Element order is display-list order; clip ids count up per page. No date, no random id, no
//! absolute path. The per-page phase runs under [`Parallelism`] and is assembled by page index, so serial
//! and parallel exports are byte-identical.
//!
//! *As built (cycle 14):* the fonts are named, not embedded: `font-family` is the face's family with the
//! generic `sans-serif` after it. Doc 09 §4.4 embeds the font subsets as `@font-face` data; that needs a
//! subsetter that keeps the `cmap` (open ruling of the cycle-14 report).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};

use ew_dl::ew_units::{Nm, Pt2};
use ew_dl::{
    Brush, Cap, Destination, DisplayList, FontKey, GeometryResolver, ImageFormat, ImageItem, Item, ItemKind,
    Join, Link, MAX_INSTANCE_DEPTH, MITER_LIMIT, PathEl, ShapedRun, Stroke, Transform,
};
use ew_render_cpu::{decode_image, monochrome};

use crate::PdfPage;
use crate::parallel::{Parallelism, par_map};

/// How a font face is named in the SVG.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SvgFace {
    /// The family name ("Noto Sans").
    pub family: String,
    /// The CSS weight (400 regular, 700 bold).
    pub weight: u16,
    /// Italic.
    pub italic: bool,
}

/// SVG export settings.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SvgOptions {
    /// Draw in black and grey (FR-OUT-011), as the PDF.
    pub monochrome: bool,
    /// The faces the runs name; a run whose face is missing is not drawn and is reported.
    pub faces: BTreeMap<FontKey, SvgFace>,
}

/// One written SVG document.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SvgFile {
    /// The file name, `<label>.svg`.
    pub name: String,
    /// The document.
    pub text: String,
}

/// What the writer skipped or decided, summed over the pages in page order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SvgReport {
    /// `<text>` elements written.
    pub texts: usize,
    /// `<a>` elements written.
    pub links: usize,
    /// Links written as plain content because their destination is not in the export.
    pub plain_text_links: usize,
    /// Faces [`SvgOptions::faces`] did not name; their runs were not drawn. Sorted, unique.
    pub missing_fonts: Vec<FontKey>,
    /// `.notdef` glyphs dropped.
    pub dropped_notdef_glyphs: usize,
    /// Images written: PNG and JPEG as `<image>` elements with their bytes embedded (FT-384).
    pub images: usize,
    /// Images not drawn: without bytes, of another format (an SVG asset reaches the writer as paths), or with bytes
    /// whose magic number is not their format's.
    pub skipped_images: usize,
    /// Symbol instances the resolver could not resolve.
    pub unresolved_instances: usize,
}

impl SvgReport {
    fn add(&mut self, o: &SvgReport) {
        self.texts += o.texts;
        self.links += o.links;
        self.plain_text_links += o.plain_text_links;
        self.missing_fonts.extend(o.missing_fonts.iter().copied());
        self.missing_fonts.sort();
        self.missing_fonts.dedup();
        self.dropped_notdef_glyphs += o.dropped_notdef_glyphs;
        self.images += o.images;
        self.skipped_images += o.skipped_images;
        self.unresolved_instances += o.unresolved_instances;
    }
}

/// The SVG documents of an export and the report.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SvgOutput {
    /// One file per page, in page order.
    pub files: Vec<SvgFile>,
    /// The report.
    pub report: SvgReport,
}

/// Why no SVG was written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SvgError {
    /// The export has no pages.
    NoPages,
    /// A page size is zero or negative.
    PageSize {
        /// The page index.
        index: usize,
    },
    /// Two pages map to one file name.
    DuplicateFile {
        /// The index of the second page with that name.
        index: usize,
        /// The file name.
        name: String,
    },
}

impl fmt::Display for SvgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SvgError::NoPages => write!(f, "the export has no pages"),
            SvgError::PageSize { index } => write!(f, "page {index} has no valid size"),
            SvgError::DuplicateFile { index, name } => {
                write!(f, "page {index} repeats the file name {name} of an earlier page")
            }
        }
    }
}

impl std::error::Error for SvgError {}

/// The file name of a page: its label with every character outside `[A-Za-z0-9._-]` replaced by `_`, and
/// `.svg`.
pub fn svg_file_name(label: &str) -> String {
    let stem: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = if stem.is_empty() { "_".to_owned() } else { stem };
    format!("{stem}.svg")
}

/// The NCName form of a destination name: `ew:r:<uuid>` → `ew-r-<uuid>` (doc 09 §4.4).
pub fn svg_id(d: &Destination) -> String {
    d.pdf_name().replace(':', "-")
}

/// The SVG documents of `pages` (see the module documentation). Instances are drawn through `geometry`
/// (sheet lists from `ew-scene` have them inlined already).
///
/// # Errors
///
/// [`SvgError`] when there are no pages, a page has no size or two pages map to one file name.
pub fn svg(
    pages: &[PdfPage],
    options: &SvgOptions,
    geometry: Option<&(dyn GeometryResolver + Sync)>,
    parallelism: Parallelism,
) -> Result<SvgOutput, SvgError> {
    if pages.is_empty() {
        return Err(SvgError::NoPages);
    }
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    for (index, p) in pages.iter().enumerate() {
        if p.width <= Nm::ZERO || p.height <= Nm::ZERO {
            return Err(SvgError::PageSize { index });
        }
        let name = svg_file_name(&p.label);
        if names.insert(name.clone(), index).is_some() {
            return Err(SvgError::DuplicateFile { index, name });
        }
    }
    let mono: Vec<DisplayList>;
    let contents: Vec<&DisplayList> = if options.monochrome {
        mono = par_map(pages, parallelism, |_, p| monochrome(&p.content));
        mono.iter().collect()
    } else {
        pages.iter().map(|p| &p.content).collect()
    };
    let files: Vec<String> = pages.iter().map(|p| svg_file_name(&p.label)).collect();
    // The PDF writer's rule: a PNG or JPEG is embedded only if its bytes decode with the CPU renderer's decoder within
    // its pixel bound, decided once per asset (the export job has withdrawn the bytes of those it found undecodable).
    let mut rasters = BTreeMap::new();
    for list in &contents {
        drawable_rasters(&list.items, &mut rasters);
    }
    // Pass 1: which link targets each page actually draws (an id is written only on a drawn element, so a
    // target whose item draws nothing, such as an empty path, an image or an unresolved instance, is not
    // one). Ids do not depend on links, so the pass writes with no destinations and keeps only the ids.
    let none = BTreeMap::new();
    let probe = Ctx {
        targets: &none,
        files: &files,
        faces: &options.faces,
        geometry,
        rasters: &rasters,
    };
    let drawn = par_map(pages, parallelism, |i, page| probe.page(i, page, contents[i]).2);
    // Where every destination is: pages by their own destination, link targets on the first page in page
    // order that draws them.
    let mut targets: BTreeMap<Destination, (usize, bool)> = BTreeMap::new();
    for (i, p) in pages.iter().enumerate() {
        targets.insert(p.destination, (i, true));
    }
    for (i, ids) in drawn.iter().enumerate() {
        for d in ids {
            targets.entry(*d).or_insert((i, false));
        }
    }
    // Pass 2: the documents, with the links resolved.
    let ctx = Ctx {
        targets: &targets,
        files: &files,
        faces: &options.faces,
        geometry,
        rasters: &rasters,
    };
    let written = par_map(pages, parallelism, |i, page| ctx.page(i, page, contents[i]));
    let mut report = SvgReport::default();
    let mut out = Vec::with_capacity(pages.len());
    for ((text, r, _), name) in written.into_iter().zip(files) {
        report.add(&r);
        out.push(SvgFile { name, text });
    }
    Ok(SvgOutput { files: out, report })
}

struct Ctx<'a> {
    targets: &'a BTreeMap<Destination, (usize, bool)>,
    files: &'a [String],
    faces: &'a BTreeMap<FontKey, SvgFace>,
    geometry: Option<&'a (dyn GeometryResolver + Sync)>,
    /// Whether the PNG or JPEG bytes of an asset decode ([`drawable_rasters`]).
    rasters: &'a BTreeMap<[u8; 32], bool>,
}

/// Records for every PNG or JPEG image item of `items` (groups included) with bytes whether they decode with
/// [`decode_image`], once per asset hash.
fn drawable_rasters(items: &[Item], out: &mut BTreeMap<[u8; 32], bool>) {
    for item in items {
        match &item.kind {
            ItemKind::Group(g) => drawable_rasters(&g.items, out),
            ItemKind::Image(img) if !img.image.bytes.is_empty() && img.image.format != ImageFormat::Svg => {
                out.entry(img.image.sha256)
                    .or_insert_with(|| decode_image(img.image.format, &img.image.bytes).is_some());
            }
            _ => {}
        }
    }
}

impl Ctx<'_> {
    /// One document, its report and the link targets it draws (each with its `id`, once per page).
    fn page(
        &self,
        index: usize,
        page: &PdfPage,
        list: &DisplayList,
    ) -> (String, SvgReport, BTreeSet<Destination>) {
        let mut w = PageWriter {
            ctx: self,
            index,
            out: String::new(),
            report: SvgReport::default(),
            clips: 0,
            ids: BTreeSet::new(),
        };
        let (wd, ht) = (mm(page.width), mm(page.height));
        let _ = write!(
            w.out,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" id=\"{}\" width=\"{wd}mm\" height=\"{ht}mm\" viewBox=\"0 0 {wd} {ht}\">\n<title>{}</title>\n<style>text{{white-space:pre}}</style>\n",
            svg_id(&page.destination),
            escape(&page.title)
        );
        w.items(&list.items, 0);
        w.out.push_str("</svg>\n");
        (w.out, w.report, w.ids)
    }
}

struct PageWriter<'a> {
    ctx: &'a Ctx<'a>,
    index: usize,
    out: String,
    report: SvgReport,
    clips: usize,
    /// The link targets whose id this page wrote (an id is written once per page, on a drawn element).
    ids: BTreeSet<Destination>,
}

impl PageWriter<'_> {
    fn items(&mut self, items: &[Item], depth: usize) {
        for item in items {
            self.item(item, depth);
        }
    }

    /// The link target of `item` whose id this page has not written yet, and its `id` attribute (empty
    /// otherwise). The id is claimed only when the item is drawn ([`PageWriter::item`]).
    fn target_id(&self, item: &Item) -> (Option<Destination>, String) {
        match item.link {
            Some(Link::Target(d)) if !self.ids.contains(&d) => (Some(d), format!(" id=\"{}\"", svg_id(&d))),
            _ => (None, String::new()),
        }
    }

    /// The `href` of a link source, or `None` (plain content) when its destination is not in the export.
    fn href(&self, d: Destination) -> Option<String> {
        let &(page, is_page) = self.ctx.targets.get(&d)?;
        let file = &self.ctx.files[page];
        Some(if is_page {
            file.clone()
        } else {
            format!("{file}#{}", svg_id(&d))
        })
    }

    fn item(&mut self, item: &Item, depth: usize) {
        let link = match item.link {
            Some(Link::Source(d)) => match self.href(d) {
                Some(h) => Some(h),
                None => {
                    self.report.plain_text_links += 1;
                    None
                }
            },
            _ => None,
        };
        if let Some(h) = &link {
            let _ = writeln!(self.out, "<a href=\"{}\">", escape(h));
            self.report.links += 1;
        }
        let (target, id) = self.target_id(item);
        let drawn = match &item.kind {
            ItemKind::Path(p) => self.path(&p.path.els, p.stroke.as_ref(), p.fill, &id, link.is_some()),
            ItemKind::Glyphs(run) => self.glyphs(run, &id),
            ItemKind::Image(img) => self.image(img, &id),
            ItemKind::Instance(inst) => {
                let geom = (depth < MAX_INSTANCE_DEPTH)
                    .then(|| self.ctx.geometry.and_then(|g| g.geometry(inst.symbol)))
                    .flatten();
                match geom {
                    Some(geom) => {
                        let _ = writeln!(self.out, "<g{id}{}>", transform_attr(inst.transform));
                        self.items(&geom.items, depth + 1);
                        self.out.push_str("</g>\n");
                        true
                    }
                    None => {
                        self.report.unresolved_instances += 1;
                        false
                    }
                }
            }
            ItemKind::Group(g) => {
                let t = g.transform.unwrap_or(Transform::IDENTITY);
                let _ = write!(self.out, "<g{id}{}", transform_attr(t));
                match g.clip {
                    Some(c) => {
                        self.clips += 1;
                        let _ = writeln!(self.out, " clip-path=\"url(#c{}-{})\">", self.index, self.clips);
                        let _ = writeln!(
                            self.out,
                            "<clipPath id=\"c{}-{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
                            self.index,
                            self.clips,
                            mm(c.min().x),
                            mm(c.min().y),
                            mm(c.max().x - c.min().x),
                            mm(c.max().y - c.min().y)
                        );
                    }
                    None => self.out.push_str(">\n"),
                }
                self.items(&g.items, depth);
                self.out.push_str("</g>\n");
                true
            }
        };
        if let Some(d) = target.filter(|_| drawn) {
            self.ids.insert(d);
        }
        if link.is_some() {
            self.out.push_str("</a>\n");
        }
    }

    /// Writes a path; `false` when it draws nothing and nothing is written.
    fn path(
        &mut self,
        els: &[PathEl],
        stroke: Option<&Stroke>,
        fill: Option<Brush>,
        id: &str,
        hit: bool,
    ) -> bool {
        let stroke = stroke.filter(|s| s.width > Nm::ZERO);
        if els.is_empty() || (stroke.is_none() && fill.is_none() && !hit) {
            return false;
        }
        let mut d = String::new();
        for el in els {
            if !d.is_empty() {
                d.push(' ');
            }
            let _ = match *el {
                PathEl::MoveTo(p) => write!(d, "M{}", pt(p)),
                PathEl::LineTo(p) => write!(d, "L{}", pt(p)),
                PathEl::QuadTo(c, p) => write!(d, "Q{} {}", pt(c), pt(p)),
                PathEl::CubicTo(c1, c2, p) => write!(d, "C{} {} {}", pt(c1), pt(c2), pt(p)),
                PathEl::ClosePath => write!(d, "Z"),
            };
        }
        let _ = write!(self.out, "<path{id} d=\"{d}\"");
        match fill {
            Some(b) => {
                let (c, a) = colour(b);
                let _ = write!(self.out, " fill=\"{c}\"");
                if let Some(a) = a {
                    let _ = write!(self.out, " fill-opacity=\"{a}\"");
                }
            }
            None => self.out.push_str(" fill=\"none\""),
        }
        if let Some(s) = stroke {
            let (c, a) = colour(s.brush);
            let _ = write!(self.out, " stroke=\"{c}\" stroke-width=\"{}\"", mm(s.width));
            if let Some(a) = a {
                let _ = write!(self.out, " stroke-opacity=\"{a}\"");
            }
            let cap = match s.cap {
                Cap::Butt => None,
                Cap::Round => Some("round"),
                Cap::Square => Some("square"),
            };
            if let Some(cap) = cap {
                let _ = write!(self.out, " stroke-linecap=\"{cap}\"");
            }
            match s.join {
                Join::Miter => {
                    let _ = write!(self.out, " stroke-miterlimit=\"{}\"", decimal(MITER_LIMIT, 6));
                }
                Join::Round => self.out.push_str(" stroke-linejoin=\"round\""),
                Join::Bevel => self.out.push_str(" stroke-linejoin=\"bevel\""),
            }
            if let Some(dash) = s
                .dash
                .as_ref()
                .filter(|d| !d.segments.is_empty() && d.segments.iter().any(|x| *x > Nm::ZERO))
            {
                let array: Vec<String> = dash.segments.iter().map(|x| mm(*x)).collect();
                let _ = write!(self.out, " stroke-dasharray=\"{}\"", array.join(" "));
                if dash.offset != Nm::ZERO {
                    let _ = write!(self.out, " stroke-dashoffset=\"{}\"", mm(dash.offset));
                }
            }
        }
        if hit && stroke.is_none() && fill.is_none() {
            self.out.push_str(" pointer-events=\"all\"");
        }
        self.out.push_str("/>\n");
        true
    }

    /// Writes a glyph run as `<text>`; `false` when nothing is written.
    fn glyphs(&mut self, run: &ShapedRun, id: &str) -> bool {
        if run.glyphs.is_empty() || run.text.is_empty() || run.size <= Nm::ZERO {
            return false;
        }
        let Some(face) = self.ctx.faces.get(&run.font) else {
            if let Err(i) = self.report.missing_fonts.binary_search(&run.font) {
                self.report.missing_fonts.insert(i, run.font);
            }
            return false;
        };
        let chars: Vec<char> = run.text.chars().collect();
        let one_to_one = chars.len() == run.glyphs.len();
        let kept: Vec<usize> = (0..run.glyphs.len()).filter(|&i| run.glyphs[i].id != 0).collect();
        self.report.dropped_notdef_glyphs += run.glyphs.len() - kept.len();
        let Some(&first) = kept.first() else {
            return false;
        };
        let place = Transform {
            translate: run.origin + run.transform.translate,
            ..run.transform
        };
        let (text, xs, ys): (String, Vec<String>, Vec<String>) = if one_to_one {
            let text = kept.iter().map(|&i| chars[i]).collect();
            let xs = kept.iter().map(|&i| mm(run.glyphs[i].x)).collect();
            let ys: Vec<String> = kept.iter().map(|&i| mm(run.glyphs[i].y)).collect();
            let offset = kept.iter().any(|&i| run.glyphs[i].y != Nm::ZERO);
            (text, xs, if offset { ys } else { vec!["0".to_owned()] })
        } else {
            (
                run.text.clone(),
                vec![mm(run.glyphs[first].x)],
                vec!["0".to_owned()],
            )
        };
        let (c, a) = colour(run.brush);
        let _ = write!(
            self.out,
            "<text{id}{} x=\"{}\" y=\"{}\" font-family=\"{}, sans-serif\" font-size=\"{}\"",
            transform_attr(place),
            xs.join(" "),
            ys.join(" "),
            escape(&quote_family(&face.family)),
            mm(run.size)
        );
        if face.weight != 400 {
            let _ = write!(self.out, " font-weight=\"{}\"", face.weight);
        }
        if face.italic {
            self.out.push_str(" font-style=\"italic\"");
        }
        if !one_to_one {
            let width = run.layout_box.max().x - run.glyphs[first].x;
            if width > Nm::ZERO {
                let _ = write!(
                    self.out,
                    " textLength=\"{}\" lengthAdjust=\"spacingAndGlyphs\"",
                    mm(width)
                );
            }
        }
        let _ = write!(self.out, " fill=\"{c}\"");
        if let Some(a) = a {
            let _ = write!(self.out, " fill-opacity=\"{a}\"");
        }
        let _ = writeln!(self.out, ">{}</text>", escape(&text));
        self.report.texts += 1;
        true
    }
}

/// A family name as a CSS value: quoted when it is not a plain identifier sequence.
fn quote_family(family: &str) -> String {
    if family
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-')
    {
        family.to_owned()
    } else {
        format!("'{}'", family.replace('\'', "\\'"))
    }
}

/// `#rrggbb` and the opacity when it is not 1.
fn colour(brush: Brush) -> (String, Option<String>) {
    let Brush::Solid(c) = brush;
    let hex = format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    let a = (c.a != 255).then(|| decimal(f64::from(c.a) / 255.0, 3));
    (hex, a)
}

/// The `transform` attribute of `t` in mm, empty for the identity.
fn transform_attr(t: Transform) -> String {
    if t.is_identity() {
        return String::new();
    }
    if t.angle.normalized().0 == 0 && matches!(t.orient, ew_dl::ew_units::Orient2::R0) {
        return format!(
            " transform=\"translate({} {})\"",
            mm(t.translate.x),
            mm(t.translate.y)
        );
    }
    let c = t.to_affine().as_coeffs();
    format!(
        " transform=\"matrix({} {} {} {} {} {})\"",
        decimal(c[0], 6),
        decimal(c[1], 6),
        decimal(c[2], 6),
        decimal(c[3], 6),
        mm(t.translate.x),
        mm(t.translate.y)
    )
}

impl PageWriter<'_> {
    /// A PNG or JPEG image as an `<image>` stretched over its rectangle, its bytes embedded as a `data:` URL in base64
    /// (the file stays self-contained, as the pages' fonts are); anything else, and bytes that do not decode (the PDF
    /// writer's rule, [`drawable_rasters`]), is counted as skipped.
    fn image(&mut self, img: &ImageItem, id: &str) -> bool {
        let bytes = &img.image.bytes;
        if self.ctx.rasters.get(&img.image.sha256) != Some(&true) {
            self.report.skipped_images += 1;
            return false;
        }
        let mime = match img.image.format {
            ImageFormat::Png if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => "image/png",
            ImageFormat::Jpeg if bytes.starts_with(&[0xff, 0xd8, 0xff]) => "image/jpeg",
            _ => {
                self.report.skipped_images += 1;
                return false;
            }
        };
        let r = img.rect;
        let _ = writeln!(
            self.out,
            "<image{id} x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"data:{mime};base64,{}\"/>",
            mm(r.min().x),
            mm(r.min().y),
            mm(r.max().x - r.min().x),
            mm(r.max().y - r.min().y),
            base64(bytes)
        );
        self.report.images += 1;
        true
    }
}

/// Base64 of `bytes` (RFC 4648 section 4: the standard alphabet, padded, no line breaks).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A point as `x y` in mm.
fn pt(p: Pt2) -> String {
    format!("{} {}", mm(p.x), mm(p.y))
}

/// nm as mm with at most 3 decimals: rounded half away from zero to whole micrometres, trailing zeros
/// dropped; integer arithmetic only.
fn mm(n: Nm) -> String {
    let v = i128::from(n.0);
    let um = if v >= 0 {
        (v + 500) / 1000
    } else {
        (v - 500) / 1000
    };
    let sign = if um < 0 { "-" } else { "" };
    let um = um.unsigned_abs();
    let (int, frac) = (um / 1000, um % 1000);
    if frac == 0 {
        return format!("{sign}{int}");
    }
    let mut f = format!("{frac:03}");
    while f.ends_with('0') {
        f.pop();
    }
    format!("{sign}{int}.{f}")
}

/// A float with at most `places` decimals, trailing zeros dropped, `-0` written as `0`.
fn decimal(v: f64, places: usize) -> String {
    let mut s = format!("{v:.places$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_owned() } else { s }
}

/// XML text and attribute escaping.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Characters XML 1.0 does not allow: replaced, never written raw.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => out.push('\u{FFFD}'),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millimetres_have_at_most_three_decimals_from_integers() {
        assert_eq!(mm(Nm(0)), "0");
        assert_eq!(mm(Nm(297_000_000)), "297");
        assert_eq!(mm(Nm(9_790_000)), "9.79");
        assert_eq!(mm(Nm(15_009_259)), "15.009");
        assert_eq!(mm(Nm(15_009_500)), "15.01");
        assert_eq!(mm(Nm(-1_500)), "-0.002");
        assert_eq!(mm(Nm(-499)), "0");
        assert_eq!(mm(Nm(264_583)), "0.265");
    }

    #[test]
    fn decimals_drop_trailing_zeros_and_negative_zero() {
        assert_eq!(decimal(1.0, 6), "1");
        assert_eq!(decimal(-0.0, 6), "0");
        assert_eq!(decimal(std::f64::consts::FRAC_1_SQRT_2, 6), "0.707107");
        assert_eq!(decimal(-1e-9, 6), "0");
    }

    #[test]
    fn file_names_and_ids_are_safe() {
        assert_eq!(svg_file_name("901.1"), "901.1.svg");
        assert_eq!(svg_file_name("A/B C"), "A_B_C.svg");
        assert_eq!(svg_file_name(""), "_.svg");
        let d = Destination::Representation(uuid::Uuid::from_u128(1));
        assert_eq!(svg_id(&d), "ew-r-00000000-0000-0000-0000-000000000001");
    }

    #[test]
    fn escaping_covers_markup_and_control_characters() {
        assert_eq!(escape("a<b>&\"c'"), "a&lt;b&gt;&amp;&quot;c&apos;");
        assert_eq!(escape("x\u{1}y"), "x\u{FFFD}y");
        assert_eq!(quote_family("Noto Sans"), "Noto Sans");
        assert_eq!(quote_family("A'B"), "'A\\'B'");
    }
}
