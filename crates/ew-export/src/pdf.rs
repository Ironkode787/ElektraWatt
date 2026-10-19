// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PDF writer: sheet display lists to a deterministic PDF/A-2b with krilla (doc 09 §4.2, §4.3;
//! ADR-0015 decision 5; FR-OUT-001 … FR-OUT-011, FR-OUT-019, FR-OUT-020, FR-OUT-022, FR-OUT-026).
//!
//! # Object model
//!
//! | Input | PDF |
//! |---|---|
//! | [`PdfPage`] in slice order | one page each, `MediaBox` = the page's sheet (A4 and A3 mix; no rotation) |
//! | `PdfPage::label` | `/PageLabels`: one range per page with the label as literal prefix and no numbering style (FR-OUT-006) |
//! | `PdfPage::section`, `PdfPage::title` | the outline: consecutive pages with the same section are grouped under one node pointing at the section's first page; pages without a section are top-level entries (front matter); each page entry is its `title` (FR-OUT-004) |
//! | `PdfPage::destination` | a named destination at the page's top-left corner (`ew:p:<uuid>`), always |
//! | [`Link::Target`] item | a named destination ([`Destination::pdf_name`]) at the item's anchor, written only if some link of the export points at it (doc 09 §4.2); the first target in page order wins |
//! | [`Link::Source`] item | a link annotation over the item's box (a glyph run's `layout_quad`, else its control-point bounds) to the named destination; to a destination not in this export: plain text ([`PartialLinks::PlainText`], FR-OUT-020) |
//! | `Path` | fill (non-zero) then stroke, caps, joins, miter limit [`ew_dl::MITER_LIMIT`], dashes |
//! | `Glyphs` | the glyphs of the shaped run in its font, subset-embedded (CID, Identity-H) with ToUnicode from the run's text, so extraction keeps real spaces (FR-OUT-008); `.notdef` glyphs are dropped (PDF/A forbids them) |
//! | `Group` | a transform and a clip path |
//! | `Instance` | the resolved geometry under the instance transform; unresolved: skipped and counted |
//! | `Image` | skipped and counted (the IR carries placeholder bytes until assets reach the scene) |
//! | [`PdfOptions`] | Info and XMP: title, description, keywords, language, creator, producer, authors (empty = omitted, FR-OUT-025), creation date, document ID |
//!
//! # Determinism (doc 09 §4.3)
//!
//! - **No wall clock and no randomness.** The creation date is [`PdfOptions::date`] (the caller applies
//!   FR-OUT-026's order: release context, scheme date, `SOURCE_DATE_EPOCH`, the model's edited date); the
//!   document ID is krilla's stable hash of [`PdfOptions::document_id`] (the caller's hash of the export
//!   input), the instance ID its stable hash of the document; font subset tags are content hashes.
//! - **Integer coordinates until the last step.** nm → milli-pt with [`nm_to_milli_pt`] (half to even),
//!   then `milli_pt as f32 / 1000`; group rotations come from [`Transform::to_affine`] (`ew-dl`'s `libm`
//!   numerics). Link rectangles are mapped through the group transforms with the integer
//!   [`Transform::apply`].
//! - **Order.** Pages, items, fonts and destinations are processed in input order; every map is a
//!   `BTreeMap`. krilla numbers objects in first-use order and sorts the name tree.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use ew_dl::ew_units::{Nm, Pt2, Rect};
use ew_dl::num::nm_to_milli_pt;
use ew_dl::{
    Brush, Cap, Destination, DisplayList, FontKey, GeometryResolver, Item, ItemKind, Join, Link,
    MAX_INSTANCE_DEPTH, MITER_LIMIT, PathEl, ShapedRun, Stroke, Transform,
};
use ew_render_cpu::{FontResolver, monochrome};
use krilla::Document;
use krilla::SerializeSettings;
use krilla::annotation::{Annotation, LinkAnnotation, Target};
use krilla::color::rgb;
use krilla::configure::{Archival, ConfigurationBuilder};
use krilla::destination::{NamedDestination, XyzDestination};
use krilla::geom::{self as kg, PathBuilder, Point};
use krilla::metadata::{DateTime, Metadata};
use krilla::num::NormalizedF32;
use krilla::outline::{Outline, OutlineNode};
use krilla::page::{PageLabel, PageSettings};
use krilla::paint::{Fill, FillRule, LineCap, LineJoin, StrokeDash};
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph};

/// One page of a PDF export.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PdfPage {
    /// The page's own destination ([`Destination::Page`] with its stable ID).
    pub destination: Destination,
    /// The folio label ("101", "901.1").
    pub label: String,
    /// The outline entry ("101 Power Supply 1 - Main Current").
    pub title: String,
    /// The outline section the page belongs to ("Section 1 – Wiring Definition"); `None` for front matter.
    pub section: Option<String>,
    /// The sheet width.
    pub width: Nm,
    /// The sheet height.
    pub height: Nm,
    /// The page's display list on the sheet (exported layers only, page-to-sheet map applied).
    pub content: DisplayList,
}

/// The PDF conformance level.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Conformance {
    /// PDF/A-2b, the default (FR-OUT-002).
    #[default]
    PdfA2b,
    /// Plain PDF 1.7 (tests and previews).
    None,
}

/// How links to destinations outside the export are written (FR-OUT-020).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PartialLinks {
    /// As plain text: no annotation.
    #[default]
    PlainText,
}

/// A fixed output date and time (UTC).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PdfDate {
    /// Year.
    pub year: u16,
    /// Month, 1–12.
    pub month: u8,
    /// Day, 1–31.
    pub day: u8,
    /// Hour, 0–23.
    pub hour: u8,
    /// Minute, 0–59.
    pub minute: u8,
    /// Second, 0–59.
    pub second: u8,
}

impl PdfDate {
    /// Midnight UTC of a date.
    pub const fn ymd(year: u16, month: u8, day: u8) -> PdfDate {
        PdfDate {
            year,
            month,
            day,
            hour: 0,
            minute: 0,
            second: 0,
        }
    }
}

/// Export settings and metadata. Every value comes from the model, the scheme or the caller, never from
/// the environment.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PdfOptions {
    /// Conformance level.
    pub conformance: Conformance,
    /// Draw in black and grey (FR-OUT-011).
    pub monochrome: bool,
    /// Links to pages outside the export.
    pub partial_links: PartialLinks,
    /// Document title (the project description).
    pub title: Option<String>,
    /// Document description (subject): drawing id, revision, derivation profile, font substitutions.
    pub description: Option<String>,
    /// Keywords.
    pub keywords: Vec<String>,
    /// Document language (BCP 47).
    pub language: Option<String>,
    /// Creator: the generator ("ElektraWatt 0.0.0").
    pub creator: Option<String>,
    /// Producer.
    pub producer: Option<String>,
    /// Authors; empty under personal-data omission (FR-OUT-025).
    pub authors: Vec<String>,
    /// The creation date (FR-OUT-026).
    pub date: PdfDate,
    /// The text the document ID is hashed from: a digest of the export input (doc 09 §4.3 rule 2).
    pub document_id: String,
}

impl Default for PdfOptions {
    fn default() -> PdfOptions {
        PdfOptions {
            conformance: Conformance::PdfA2b,
            monochrome: false,
            partial_links: PartialLinks::PlainText,
            title: None,
            description: None,
            keywords: Vec::new(),
            language: None,
            creator: None,
            producer: None,
            authors: Vec::new(),
            date: PdfDate::ymd(2000, 1, 1),
            document_id: String::new(),
        }
    }
}

/// What the writer skipped or decided, for the export report.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PdfReport {
    /// Fonts the resolver did not have; their runs were not drawn.
    pub missing_fonts: Vec<FontKey>,
    /// Images not drawn yet.
    pub skipped_images: usize,
    /// Symbol instances the resolver could not resolve.
    pub unresolved_instances: usize,
    /// `.notdef` glyphs dropped.
    pub dropped_notdef_glyphs: usize,
    /// Link annotations written.
    pub links: usize,
    /// Links written as plain text because their destination is not in the export.
    pub plain_text_links: usize,
    /// Named destinations written (one per page plus every linked target).
    pub destinations: usize,
}

/// A PDF and its report.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PdfOutput {
    /// The file.
    pub bytes: Vec<u8>,
    /// The report.
    pub report: PdfReport,
}

/// Why no PDF was written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PdfError {
    /// The export has no pages.
    NoPages,
    /// A page size is zero or not representable.
    PageSize {
        /// The page index.
        index: usize,
    },
    /// The date is not a valid date and time.
    Date,
    /// krilla refused the document (for example a PDF/A validation error).
    Writer(String),
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdfError::NoPages => write!(f, "the export has no pages"),
            PdfError::PageSize { index } => write!(f, "page {index} has no valid size"),
            PdfError::Date => write!(f, "the output date is not a valid date"),
            PdfError::Writer(e) => write!(f, "the PDF writer failed: {e}"),
        }
    }
}

impl std::error::Error for PdfError {}

/// The PDF bytes of `pages` (see the module documentation).
///
/// # Errors
///
/// [`PdfError`] when there are no pages, a page has no size, the date is invalid or krilla refuses the
/// document.
pub fn pdf_bytes(
    pages: &[PdfPage],
    options: &PdfOptions,
    fonts: &dyn FontResolver,
) -> Result<Vec<u8>, PdfError> {
    pdf(pages, options, fonts, None).map(|o| o.bytes)
}

/// The PDF of `pages` with its report; instances are drawn through `geometry` (sheet lists from
/// `ew-scene` have them inlined already).
///
/// # Errors
///
/// As [`pdf_bytes`].
pub fn pdf(
    pages: &[PdfPage],
    options: &PdfOptions,
    fonts: &dyn FontResolver,
    geometry: Option<&dyn GeometryResolver>,
) -> Result<PdfOutput, PdfError> {
    if pages.is_empty() {
        return Err(PdfError::NoPages);
    }
    let date = date_time(options.date).ok_or(PdfError::Date)?;
    let mono: Vec<DisplayList>;
    let contents: Vec<&DisplayList> = if options.monochrome {
        mono = pages.iter().map(|p| monochrome(&p.content)).collect();
        mono.iter().collect()
    } else {
        pages.iter().map(|p| &p.content).collect()
    };

    let mut report = PdfReport::default();
    let page_index: BTreeMap<Destination, usize> = pages
        .iter()
        .enumerate()
        .map(|(i, p)| (p.destination, i))
        .rev()
        .collect();
    // Pass 1: where every link target is, and which destinations links ask for.
    let mut targets: BTreeMap<Destination, (usize, Point)> = BTreeMap::new();
    let mut wanted: BTreeSet<Destination> = BTreeSet::new();
    for (index, list) in contents.iter().enumerate() {
        let mut scan = Scan {
            page: index,
            geometry,
            targets: &mut targets,
            wanted: &mut wanted,
        };
        scan.items(&list.items, &[], 0);
    }
    let resolve = |d: &Destination| -> Option<(usize, Point)> {
        match d {
            Destination::Page(_) => page_index.get(d).map(|&i| (i, Point::from_xy(0.0, 0.0))),
            _ => targets.get(d).copied(),
        }
    };

    let mut document = Document::new_with(settings(options.conformance)?);
    let mut font_cache: BTreeMap<FontKey, Option<Font>> = BTreeMap::new();
    let mut missing: BTreeSet<FontKey> = BTreeSet::new();
    for (index, (page, list)) in pages.iter().zip(&contents).enumerate() {
        let (w, h) = (pt(page.width), pt(page.height));
        let settings = PageSettings::from_wh(w, h)
            .filter(|_| w > 0.0 && h > 0.0)
            .ok_or(PdfError::PageSize { index })?
            .with_page_label(PageLabel::new(None, Some(page.label.clone()), None));
        let mut kpage = document.start_page_with(settings);
        let mut links = Vec::new();
        {
            let mut surface = kpage.surface();
            let mut w = Writer {
                fonts,
                geometry,
                font_cache: &mut font_cache,
                missing: &mut missing,
                report: &mut report,
                links: &mut links,
            };
            w.items(&mut surface, &list.items, &[], 0);
            surface.finish();
        }
        for (dest, quad) in links {
            match resolve(&dest) {
                Some((target_page, point)) => {
                    let Some(rect) = rect_of(&quad) else { continue };
                    let named =
                        NamedDestination::new(dest.pdf_name(), XyzDestination::new(target_page, point));
                    kpage.add_annotation(Annotation::new_link(
                        LinkAnnotation::new(rect, Target::Destination(named.into())),
                        None,
                    ));
                    report.links += 1;
                }
                None => report.plain_text_links += 1,
            }
        }
        kpage.finish();
    }
    // Named destinations: one per page, then every linked target (krilla registers the linked ones on
    // use as well; registering them here fixes their order).
    let mut names = BTreeSet::new();
    for (i, p) in pages.iter().enumerate() {
        let name = p.destination.pdf_name();
        if names.insert(name.clone()) {
            let nd = NamedDestination::new(name, XyzDestination::new(i, Point::from_xy(0.0, 0.0)));
            let _ = document.register_named_destination(nd);
        }
    }
    for d in &wanted {
        if let Some((i, point)) = resolve(d) {
            let name = d.pdf_name();
            if names.insert(name.clone()) {
                let _ = document
                    .register_named_destination(NamedDestination::new(name, XyzDestination::new(i, point)));
            }
        }
    }
    report.destinations = names.len();
    document.set_outline(outline(pages));
    document.set_metadata(metadata(options, date));
    report.missing_fonts = missing.into_iter().collect();
    let bytes = document
        .finish()
        .map_err(|e| PdfError::Writer(format!("{e:?}")))?;
    Ok(PdfOutput { bytes, report })
}

fn settings(conformance: Conformance) -> Result<SerializeSettings, PdfError> {
    let builder = ConfigurationBuilder::new();
    let builder = match conformance {
        Conformance::PdfA2b => builder.with_archival_validator(Archival::A2_B),
        Conformance::None => builder,
    };
    let configuration = builder.finish().map_err(|e| PdfError::Writer(format!("{e:?}")))?;
    Ok(SerializeSettings {
        configuration,
        // Every page is built by this writer; nothing needs tagging until tagged PDF (WP-S3.14).
        enable_tagging: false,
        ..SerializeSettings::default()
    })
}

fn date_time(d: PdfDate) -> Option<DateTime> {
    let valid = (1..=12).contains(&d.month)
        && (1..=31).contains(&d.day)
        && d.hour < 24
        && d.minute < 60
        && d.second < 60
        && d.year <= 9999;
    valid.then(|| {
        DateTime::new(d.year)
            .month(d.month)
            .day(d.day)
            .hour(d.hour)
            .minute(d.minute)
            .second(d.second)
            .utc_offset_hour(0)
            .utc_offset_minute(0)
    })
}

fn metadata(o: &PdfOptions, date: DateTime) -> Metadata {
    let mut m = Metadata::new().creation_date(date);
    if let Some(t) = &o.title {
        m = m.title(t.clone());
    }
    if let Some(d) = &o.description {
        m = m.description(d.clone());
    }
    if !o.keywords.is_empty() {
        m = m.keywords(o.keywords.clone());
    }
    if let Some(l) = &o.language {
        m = m.language(l.clone());
    }
    if let Some(c) = &o.creator {
        m = m.creator(c.clone());
    }
    if let Some(p) = &o.producer {
        m = m.producer(p.clone());
    }
    if !o.authors.is_empty() {
        m = m.authors(o.authors.clone());
    }
    if !o.document_id.is_empty() {
        m = m.document_id(o.document_id.clone());
    }
    m
}

fn outline(pages: &[PdfPage]) -> Outline {
    let top = |i: usize| XyzDestination::new(i, Point::from_xy(0.0, 0.0));
    let mut outline = Outline::new();
    let mut i = 0;
    while i < pages.len() {
        match &pages[i].section {
            None => {
                outline.push_child(OutlineNode::new(pages[i].title.clone(), top(i)));
                i += 1;
            }
            Some(section) => {
                let mut node = OutlineNode::new(section.clone(), top(i)).with_open(true);
                while i < pages.len() && pages[i].section.as_ref() == Some(section) {
                    node.push_child(OutlineNode::new(pages[i].title.clone(), top(i)));
                    i += 1;
                }
                outline.push_child(node);
            }
        }
    }
    outline
}

/// nm → pt through integer milli-pt (doc 09 §4.3 rule 4).
fn pt(n: Nm) -> f32 {
    nm_to_milli_pt(n) as f32 / 1000.0
}

fn point(p: Pt2) -> Point {
    Point::from_xy(pt(p.x), pt(p.y))
}

/// Maps a point of the innermost coordinate system to the sheet: through every transform, innermost first.
fn to_sheet(stack: &[Transform], p: Pt2) -> Pt2 {
    stack.iter().rev().fold(p, |q, t| t.apply(q))
}

fn rect_of(quad: &[Pt2]) -> Option<kg::Rect> {
    let r = quad.iter().map(|&p| Rect::from_point(p)).reduce(Rect::union)?;
    kg::Rect::from_ltrb(pt(r.min().x), pt(r.min().y), pt(r.max().x), pt(r.max().y))
}

/// The anchor of a link target: an instance's insertion point, a run's origin, else the first point.
fn anchor(item: &Item) -> Option<Pt2> {
    match &item.kind {
        ItemKind::Instance(i) => Some(i.transform.translate),
        ItemKind::Glyphs(run) => Some(run.origin),
        ItemKind::Path(p) => p.path.points().next(),
        ItemKind::Image(img) => Some(img.rect.min()),
        ItemKind::Group(g) => Some(g.transform.unwrap_or(Transform::IDENTITY).translate),
    }
}

/// The box of a link source in its own coordinates: a run's layout quad, else its control points.
fn source_quad(item: &Item, geometry: Option<&dyn GeometryResolver>) -> Vec<Pt2> {
    match &item.kind {
        ItemKind::Glyphs(run) => run.layout_quad().to_vec(),
        _ => {
            let list = DisplayList {
                items: vec![item.clone()],
            };
            let bounds = match geometry {
                Some(g) => list.bounds_with(g),
                None => list.bounds(),
            };
            bounds.map_or_else(Vec::new, |r| vec![r.min(), r.max()])
        }
    }
}

struct Scan<'a> {
    page: usize,
    geometry: Option<&'a dyn GeometryResolver>,
    targets: &'a mut BTreeMap<Destination, (usize, Point)>,
    wanted: &'a mut BTreeSet<Destination>,
}

impl Scan<'_> {
    fn items(&mut self, items: &[Item], stack: &[Transform], depth: usize) {
        for item in items {
            match item.link {
                Some(Link::Target(d)) => {
                    if let Some(a) = anchor(item) {
                        let p = point(to_sheet(stack, a));
                        self.targets.entry(d).or_insert((self.page, p));
                    }
                }
                Some(Link::Source(d)) => {
                    self.wanted.insert(d);
                }
                None => {}
            }
            match &item.kind {
                ItemKind::Group(g) => {
                    let mut s = stack.to_vec();
                    s.push(g.transform.unwrap_or(Transform::IDENTITY));
                    self.items(&g.items, &s, depth);
                }
                ItemKind::Instance(inst) if depth < MAX_INSTANCE_DEPTH => {
                    if let Some(geom) = self.geometry.and_then(|g| g.geometry(inst.symbol)) {
                        let mut s = stack.to_vec();
                        s.push(inst.transform);
                        self.items(&geom.items, &s, depth + 1);
                    }
                }
                _ => {}
            }
        }
    }
}

struct Writer<'a> {
    fonts: &'a dyn FontResolver,
    geometry: Option<&'a dyn GeometryResolver>,
    font_cache: &'a mut BTreeMap<FontKey, Option<Font>>,
    missing: &'a mut BTreeSet<FontKey>,
    report: &'a mut PdfReport,
    /// Link sources of this page: destination and box corners on the sheet.
    links: &'a mut Vec<(Destination, Vec<Pt2>)>,
}

impl Writer<'_> {
    fn items(&mut self, surface: &mut Surface<'_>, items: &[Item], stack: &[Transform], depth: usize) {
        for item in items {
            if let Some(Link::Source(d)) = item.link {
                let quad: Vec<Pt2> = source_quad(item, self.geometry)
                    .into_iter()
                    .map(|p| to_sheet(stack, p))
                    .collect();
                if !quad.is_empty() {
                    self.links.push((d, quad));
                }
            }
            match &item.kind {
                ItemKind::Path(p) => self.path(surface, &p.path.els, p.stroke.as_ref(), p.fill),
                ItemKind::Glyphs(run) => self.glyphs(surface, run),
                ItemKind::Image(_) => self.report.skipped_images += 1,
                ItemKind::Instance(inst) => {
                    let geom = (depth < MAX_INSTANCE_DEPTH)
                        .then(|| self.geometry.and_then(|g| g.geometry(inst.symbol)))
                        .flatten();
                    match geom {
                        Some(geom) => {
                            surface.push_transform(&transform(inst.transform));
                            let mut s = stack.to_vec();
                            s.push(inst.transform);
                            self.items(surface, &geom.items, &s, depth + 1);
                            surface.pop();
                        }
                        None => self.report.unresolved_instances += 1,
                    }
                }
                ItemKind::Group(g) => {
                    let t = g.transform.unwrap_or(Transform::IDENTITY);
                    surface.push_transform(&transform(t));
                    let clip = g.clip.and_then(|c| {
                        let mut pb = PathBuilder::new();
                        let r =
                            kg::Rect::from_ltrb(pt(c.min().x), pt(c.min().y), pt(c.max().x), pt(c.max().y))?;
                        pb.push_rect(r);
                        pb.finish()
                    });
                    if let Some(c) = &clip {
                        surface.push_clip_path(c, &FillRule::NonZero);
                    }
                    let mut s = stack.to_vec();
                    s.push(t);
                    self.items(surface, &g.items, &s, depth);
                    if clip.is_some() {
                        surface.pop();
                    }
                    surface.pop();
                }
            }
        }
    }

    fn path(
        &mut self,
        surface: &mut Surface<'_>,
        els: &[PathEl],
        stroke: Option<&Stroke>,
        fill: Option<Brush>,
    ) {
        let mut pb = PathBuilder::new();
        for el in els {
            match *el {
                PathEl::MoveTo(p) => pb.move_to(pt(p.x), pt(p.y)),
                PathEl::LineTo(p) => pb.line_to(pt(p.x), pt(p.y)),
                PathEl::QuadTo(c, p) => pb.quad_to(pt(c.x), pt(c.y), pt(p.x), pt(p.y)),
                PathEl::CubicTo(c1, c2, p) => {
                    pb.cubic_to(pt(c1.x), pt(c1.y), pt(c2.x), pt(c2.y), pt(p.x), pt(p.y));
                }
                PathEl::ClosePath => pb.close(),
            }
        }
        let Some(path) = pb.finish() else { return };
        if let Some(brush) = fill {
            surface.set_stroke(None);
            surface.set_fill(Some(fill_of(brush)));
            surface.draw_path(&path);
        }
        if let Some(s) = stroke.filter(|s| s.width > Nm::ZERO) {
            surface.set_fill(None);
            surface.set_stroke(Some(stroke_of(s)));
            surface.draw_path(&path);
        }
    }

    fn font(&mut self, key: FontKey) -> Option<Font> {
        let fonts = self.fonts;
        self.font_cache
            .entry(key)
            .or_insert_with(|| {
                let bytes = fonts.font(key)?;
                let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes.data);
                Font::new(data.into(), bytes.index)
            })
            .clone()
    }

    fn glyphs(&mut self, surface: &mut Surface<'_>, run: &ShapedRun) {
        if run.glyphs.is_empty() || run.text.is_empty() || run.size <= Nm::ZERO {
            return;
        }
        let Some(font) = self.font(run.font) else {
            self.missing.insert(run.font);
            return;
        };
        let size = run.size.0 as f64;
        // Clusters: one char per glyph when the counts agree, else the whole text as one cluster.
        let chars: Vec<(usize, char)> = run.text.char_indices().collect();
        let one_to_one = chars.len() == run.glyphs.len();
        let range = |i: usize| {
            if one_to_one {
                let start = chars[i].0;
                start..start + chars[i].1.len_utf8()
            } else {
                0..run.text.len()
            }
        };
        let kept: Vec<usize> = (0..run.glyphs.len()).filter(|&i| run.glyphs[i].id != 0).collect();
        self.report.dropped_notdef_glyphs += run.glyphs.len() - kept.len();
        let Some(&first) = kept.first() else { return };
        let end_x = run.layout_box.max().x.0;
        let glyphs: Vec<KrillaGlyph> = kept
            .iter()
            .enumerate()
            .map(|(k, &i)| {
                let g = run.glyphs[i];
                let next_x = kept.get(k + 1).map_or(end_x.max(g.x.0), |&j| run.glyphs[j].x.0);
                KrillaGlyph::new(
                    GlyphId::new(g.id),
                    ((next_x - g.x.0) as f64 / size) as f32,
                    0.0,
                    (-(g.y.0 as f64) / size) as f32,
                    0.0,
                    range(i),
                    None,
                )
            })
            .collect();
        let start = run.glyphs[first].x;
        let place = Transform {
            translate: run.origin + run.transform.translate,
            ..run.transform
        };
        surface.push_transform(&transform(place));
        surface.set_stroke(None);
        surface.set_fill(Some(fill_of(run.brush)));
        surface.draw_glyphs(
            Point::from_xy(pt(start), 0.0),
            &glyphs,
            font,
            &run.text,
            pt(run.size),
            false,
        );
        surface.pop();
    }
}

/// An `ew-dl` transform as a krilla transform in pt: the linear part from [`Transform::to_affine`]
/// (exact for quarter turns), the translation through integer milli-pt.
fn transform(t: Transform) -> kg::Transform {
    let c = t.to_affine().as_coeffs();
    kg::Transform::from_row(
        c[0] as f32,
        c[1] as f32,
        c[2] as f32,
        c[3] as f32,
        pt(t.translate.x),
        pt(t.translate.y),
    )
}

fn colour(brush: Brush) -> (rgb::Color, NormalizedF32) {
    let Brush::Solid(c) = brush;
    let opacity = if c.a == 255 {
        NormalizedF32::ONE
    } else {
        NormalizedF32::new(f32::from(c.a) / 255.0).unwrap_or(NormalizedF32::ONE)
    };
    (rgb::Color::new(c.r, c.g, c.b), opacity)
}

fn fill_of(brush: Brush) -> Fill {
    let (c, opacity) = colour(brush);
    Fill {
        paint: c.into(),
        opacity,
        rule: FillRule::NonZero,
    }
}

fn stroke_of(s: &Stroke) -> krilla::paint::Stroke {
    let (c, opacity) = colour(s.brush);
    krilla::paint::Stroke {
        paint: c.into(),
        width: pt(s.width),
        miter_limit: MITER_LIMIT as f32,
        line_cap: match s.cap {
            Cap::Butt => LineCap::Butt,
            Cap::Round => LineCap::Round,
            Cap::Square => LineCap::Square,
        },
        line_join: match s.join {
            Join::Miter => LineJoin::Miter,
            Join::Round => LineJoin::Round,
            Join::Bevel => LineJoin::Bevel,
        },
        opacity,
        dash: s
            .dash
            .as_ref()
            .filter(|d| !d.segments.is_empty() && d.segments.iter().any(|x| *x > Nm::ZERO))
            .map(|d| StrokeDash {
                array: d.segments.iter().map(|x| pt(*x)).collect(),
                offset: pt(d.offset),
            }),
    }
}
