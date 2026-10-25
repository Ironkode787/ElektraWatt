// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Image items resolved for drawing (FT-384, decision log ID-576): the display lists place an image by its asset's
//! hash with **empty bytes** (the forms layer, symbol images); the export job, which is not a memoised query, resolves
//! them here from the bytes the project holds before any renderer reads the list ([`resolve_images`]).
//!
//! - **SVG** becomes vector paths ([`svg_items`]): the bytes pass the store's rejecter first
//!   ([`ew_schema::svg_hazard`], FT-383), then usvg parses and normalises them (default features off: no text, no
//!   system fonts; image `href`s resolve to nothing, so no file or URL is read) and every visible path becomes an
//!   `ew-dl` path in the image's rectangle, in a group clipped to it. Every renderer draws paths already, so the logo
//!   prints as vectors in the CPU renderer, the PDF writer and the SVG writer (resvg is not used).
//! - **PNG and JPEG** keep their image item and get the bytes, for the renderers that embed or decode them.
//!
//! The SVG subset drawn: paths with solid fills and strokes (width, caps, joins, dashes), opacity multiplied into the
//! colour's alpha, group and path transforms. Approximated: a gradient fill or stroke paints the mean colour of its
//! stops, and the even-odd fill rule is drawn non-zero (the display list has the non-zero rule only). Not drawn:
//! text (an SVG logo's lettering must be converted to outlines), images inside the SVG, patterns, clip paths, masks
//! and filters; [`SvgItems::skipped`] counts them. Ruled by decision log ID-576; the subset is a design-pass item.
//!
//! The parser is guarded twice: the rejecter refuses, besides scripts and external references, what the cargo-vet
//! audit of usvg 0.48.1 and its dependencies found it cannot bear (deep nesting, reference cycles, expansion bombs,
//! huge numbers, the constructs that panic it; [`ew_schema::svg_hazard`]), and the parse runs inside `catch_unwind`.
//!
//! Asset presence is read here, outside the memoised queries (FT-382): the caller passes the bytes it holds.

use std::sync::Arc;

use ew_dl::{
    Brush, Cap, Color, Dash, DisplayList, ImageData, ImageFormat, Item, ItemKind, Join, Path, Stroke,
};
use ew_units::{Nm, Pt2, Rect};
use usvg::tiny_skia_path::PathSegment;

/// Why an SVG asset is not drawn.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SvgError {
    /// The store's rejecter refuses it (FT-383).
    Hazard(ew_schema::SvgHazard),
    /// usvg cannot parse it.
    Parse(String),
}

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SvgError::Hazard(h) => {
                write!(f, "the SVG is refused: {}; {}", h.describe(), h.advice())
            }
            SvgError::Parse(e) => write!(f, "the SVG cannot be parsed: {e}"),
        }
    }
}

impl std::error::Error for SvgError {}

/// The paths of an SVG asset in a rectangle.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SvgItems {
    /// The path items, in paint order, in the rectangle's coordinates.
    pub items: Vec<Item>,
    /// The SVG nodes that are not drawn (images, pattern paints) and the effects that are dropped (clip paths,
    /// masks, filters). Text never reaches the tree: usvg's `text` feature is off, so the parser drops it.
    pub skipped: usize,
}

/// The usvg options of an asset: nothing outside the bytes is ever read.
fn options() -> usvg::Options<'static> {
    usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    }
}

/// The paths of the SVG `bytes` stretched over `rect` (the SVG's size, after its `viewBox`, mapped onto the
/// rectangle; the forms layer already fits the rectangle to the asset's aspect ratio). See the module documentation
/// for the subset drawn.
///
/// # Errors
///
/// [`SvgError::Hazard`] when the rejecter refuses the bytes, [`SvgError::Parse`] when usvg does.
pub fn svg_items(bytes: &[u8], rect: Rect) -> Result<SvgItems, SvgError> {
    if let Some(h) = ew_schema::svg_hazard(bytes) {
        return Err(SvgError::Hazard(h));
    }
    let text = std::str::from_utf8(bytes).map_err(|e| SvgError::Parse(e.to_string()))?;
    // The rejecter keeps out what the parser's audit found it cannot bear; a panic it still meets is contained.
    let parsed = std::panic::catch_unwind(|| usvg::Tree::from_str(text, &options()))
        .map_err(|_| SvgError::Parse("the parser failed on this SVG".to_owned()))?;
    let tree = parsed.map_err(|e| SvgError::Parse(e.to_string()))?;
    let size = tree.size();
    let (w, h) = (f64::from(size.width()), f64::from(size.height()));
    if !(w > 0.0 && h > 0.0) {
        return Ok(SvgItems::default());
    }
    let map = Map {
        x0: rect.min().x.0 as f64,
        y0: rect.min().y.0 as f64,
        sx: rect.width().0 as f64 / w,
        sy: rect.height().0 as f64 / h,
    };
    let mut out = SvgItems::default();
    group(tree.root(), 1.0, &map, &mut out);
    Ok(out)
}

/// SVG size units to sheet nanometres: `p ↦ (x0 + sx·x, y0 + sy·y)`.
struct Map {
    x0: f64,
    y0: f64,
    sx: f64,
    sy: f64,
}

impl Map {
    fn point(&self, t: &usvg::Transform, x: f32, y: f32) -> Pt2 {
        let (x, y) = (f64::from(x), f64::from(y));
        let ax = f64::from(t.sx) * x + f64::from(t.kx) * y + f64::from(t.tx);
        let ay = f64::from(t.ky) * x + f64::from(t.sy) * y + f64::from(t.ty);
        Pt2::new(nm(self.x0 + self.sx * ax), nm(self.y0 + self.sy * ay))
    }

    /// The factor a length is scaled by under `t` and this map (the square root of the area scale).
    fn scale(&self, t: &usvg::Transform) -> f64 {
        let det = f64::from(t.sx) * f64::from(t.sy) - f64::from(t.kx) * f64::from(t.ky);
        (det.abs() * (self.sx * self.sy).abs()).sqrt()
    }
}

/// A coordinate in nanometres, rounded; non-finite values become 0.
fn nm(v: f64) -> Nm {
    if v.is_finite() {
        Nm(v.round().clamp(-1.0e15, 1.0e15) as i64)
    } else {
        Nm(0)
    }
}

/// The colour of a paint with `opacity` (0 to 1): solid colours as they are, gradients as the mean of their stops,
/// patterns `None`.
fn colour(paint: &usvg::Paint, opacity: f64) -> Option<Color> {
    let alpha = |a: f64| (a.clamp(0.0, 1.0) * 255.0).round() as u8;
    match paint {
        usvg::Paint::Color(c) => Some(Color::from_rgba8(c.red, c.green, c.blue, alpha(opacity))),
        usvg::Paint::LinearGradient(g) => mean_stop(g.stops(), opacity),
        usvg::Paint::RadialGradient(g) => mean_stop(g.stops(), opacity),
        usvg::Paint::Pattern(_) => None,
    }
}

fn mean_stop(stops: &[usvg::Stop], opacity: f64) -> Option<Color> {
    if stops.is_empty() {
        return None;
    }
    let n = stops.len() as f64;
    let mean = |f: &dyn Fn(&usvg::Stop) -> f64| stops.iter().map(f).sum::<f64>() / n;
    let channel = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    Some(Color::from_rgba8(
        channel(mean(&|s| f64::from(s.color().red))),
        channel(mean(&|s| f64::from(s.color().green))),
        channel(mean(&|s| f64::from(s.color().blue))),
        channel(mean(&|s| f64::from(s.opacity().get())) * opacity.clamp(0.0, 1.0) * 255.0),
    ))
}

fn group(g: &usvg::Group, opacity: f64, map: &Map, out: &mut SvgItems) {
    let opacity = opacity * f64::from(g.opacity().get());
    if g.clip_path().is_some() || g.mask().is_some() || !g.filters().is_empty() {
        out.skipped += 1;
    }
    for node in g.children() {
        match node {
            usvg::Node::Group(child) => group(child, opacity, map, out),
            usvg::Node::Path(p) => path(p, opacity, map, out),
            usvg::Node::Image(_) | usvg::Node::Text(_) => out.skipped += 1,
        }
    }
}

fn path(p: &usvg::Path, opacity: f64, map: &Map, out: &mut SvgItems) {
    if !p.is_visible() {
        return;
    }
    let t = p.abs_transform();
    let mut b = Path::builder();
    for seg in p.data().segments() {
        b = match seg {
            PathSegment::MoveTo(a) => b.move_to(map.point(&t, a.x, a.y)),
            PathSegment::LineTo(a) => b.line_to(map.point(&t, a.x, a.y)),
            PathSegment::QuadTo(c, a) => b.quad_to(map.point(&t, c.x, c.y), map.point(&t, a.x, a.y)),
            PathSegment::CubicTo(c1, c2, a) => b.cubic_to(
                map.point(&t, c1.x, c1.y),
                map.point(&t, c2.x, c2.y),
                map.point(&t, a.x, a.y),
            ),
            PathSegment::Close => b.close(),
        };
    }
    let geometry = b.build();
    if geometry.is_empty() {
        return;
    }
    let fill = p.fill().and_then(|f| {
        let c = colour(f.paint(), opacity * f64::from(f.opacity().get()));
        if c.is_none() {
            out.skipped += 1;
        }
        c.map(Brush::Solid)
    });
    let stroke = p.stroke().and_then(|s| {
        let Some(c) = colour(s.paint(), opacity * f64::from(s.opacity().get())) else {
            out.skipped += 1;
            return None;
        };
        let scale = map.scale(&t);
        let width = nm(f64::from(s.width().get()) * scale);
        if width.0 <= 0 {
            return None;
        }
        Some(Stroke {
            width,
            cap: match s.linecap() {
                usvg::LineCap::Butt => Cap::Butt,
                usvg::LineCap::Round => Cap::Round,
                usvg::LineCap::Square => Cap::Square,
            },
            join: match s.linejoin() {
                usvg::LineJoin::Round => Join::Round,
                usvg::LineJoin::Bevel => Join::Bevel,
                usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => Join::Miter,
            },
            dash: s.dasharray().map(|d| Dash {
                segments: d.iter().map(|v| nm(f64::from(*v) * scale)).collect(),
                offset: nm(f64::from(s.dashoffset()) * scale),
            }),
            brush: Brush::Solid(c),
        })
    });
    if fill.is_some() || stroke.is_some() {
        out.items.push(Item::path(geometry, stroke, fill));
    }
}

/// The bytes of an asset by its SHA-256, `None` when the caller does not hold it.
pub type AssetBytes<'a> = dyn Fn(&[u8; 32]) -> Option<Arc<[u8]>> + 'a;

/// Why an image item of a list is not drawn as it should be.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ImageProblem {
    /// The caller does not hold the asset's bytes: the item is left, and renderers skip it.
    Missing,
    /// The SVG is refused by the rejecter or by usvg: the item is dropped.
    Refused(SvgError),
    /// The PNG or JPEG bytes are held but do not decode within the renderers' pixel bound (a corrupt or truncated
    /// file, a declared size above the bound, an unsupported variant): the item is left, and renderers skip it. The
    /// export job adds this after resolving, with the renderers' decoder (FT-406; the sniffer reads headers only).
    Undecodable(ImageFormat),
}

/// An image item that is not drawn, by its asset's hash.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImageIssue {
    /// The SHA-256 of the asset.
    pub sha256: [u8; 32],
    /// What is wrong.
    pub problem: ImageProblem,
}

impl std::fmt::Display for ImageIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex: String = self.sha256.iter().map(|b| format!("{b:02x}")).collect();
        match &self.problem {
            ImageProblem::Missing => write!(
                f,
                "the image sha256:{hex} is not drawn: the project does not hold its bytes"
            ),
            ImageProblem::Refused(e) => write!(f, "the image sha256:{hex} is not drawn: {e}"),
            ImageProblem::Undecodable(format) => {
                let format = match format {
                    ImageFormat::Png => "PNG",
                    ImageFormat::Jpeg => "JPEG",
                    ImageFormat::Svg => "SVG",
                };
                write!(
                    f,
                    "the image sha256:{hex} is not drawn: its {format} bytes do not decode (corrupt, truncated, \
                     unsupported or larger than 16 Mi pixels); export it again as a plain PNG or baseline JPEG"
                )
            }
        }
    }
}

/// What [`resolve_images`] did.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ImageReport {
    /// SVG images replaced by their paths.
    pub vector: usize,
    /// PNG and JPEG images given their bytes.
    pub raster: usize,
    /// Images whose bytes the caller does not hold (left as they are; renderers skip them).
    pub missing: usize,
    /// SVG images refused by the rejecter or by usvg (dropped from the list).
    pub refused: usize,
    /// The missing and refused images, in list order (the export job reports them as page findings).
    pub issues: Vec<ImageIssue>,
}

/// `list` with its image items resolved from `assets` (the bytes of an asset by its SHA-256, `None` when the project
/// does not hold it): an SVG image becomes a group clipped to its rectangle holding the SVG's paths ([`svg_items`];
/// the image item's entity and link stay on the group), a PNG or JPEG image gets its bytes. Groups are resolved
/// recursively; every other item is kept as it is, so a list without images comes back unchanged.
pub fn resolve_images(list: &DisplayList, assets: &AssetBytes<'_>) -> (DisplayList, ImageReport) {
    let mut report = ImageReport::default();
    let items = resolve_items(&list.items, assets, &mut report);
    (DisplayList { items }, report)
}

fn has_images(items: &[Item]) -> bool {
    items.iter().any(|i| match &i.kind {
        ItemKind::Image(_) => true,
        ItemKind::Group(g) => has_images(&g.items),
        _ => false,
    })
}

fn resolve_items(items: &[Item], assets: &AssetBytes<'_>, report: &mut ImageReport) -> Vec<Item> {
    if !has_images(items) {
        return items.to_vec();
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match &item.kind {
            ItemKind::Group(g) => {
                let mut g = g.clone();
                g.items = resolve_items(&g.items, assets, report);
                out.push(Item {
                    kind: ItemKind::Group(g),
                    ..item.clone()
                });
            }
            ItemKind::Image(img) => {
                let bytes = if img.image.bytes.is_empty() {
                    assets(&img.image.sha256)
                } else {
                    Some(Arc::clone(&img.image.bytes))
                };
                let Some(bytes) = bytes else {
                    report.missing += 1;
                    report.issues.push(ImageIssue {
                        sha256: img.image.sha256,
                        problem: ImageProblem::Missing,
                    });
                    out.push(item.clone());
                    continue;
                };
                match img.image.format {
                    ImageFormat::Svg => match svg_items(&bytes, img.rect) {
                        Ok(svg) => {
                            report.vector += 1;
                            let mut group = Item::group(None, Some(img.rect), svg.items);
                            group.entity = item.entity;
                            group.link = item.link;
                            out.push(group);
                        }
                        Err(e) => {
                            report.refused += 1;
                            report.issues.push(ImageIssue {
                                sha256: img.image.sha256,
                                problem: ImageProblem::Refused(e),
                            });
                        }
                    },
                    ImageFormat::Png | ImageFormat::Jpeg => {
                        report.raster += 1;
                        let mut resolved = img.clone();
                        resolved.image = Arc::new(ImageData {
                            sha256: img.image.sha256,
                            format: img.image.format,
                            bytes,
                        });
                        out.push(Item {
                            kind: ItemKind::Image(resolved),
                            ..item.clone()
                        });
                    }
                }
            }
            _ => out.push(item.clone()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
        Rect::from_points(Pt2::new(Nm(x0), Nm(y0)), Pt2::new(Nm(x1), Nm(y1)))
    }

    #[test]
    fn a_filled_rectangle_fills_the_image_rectangle() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10"><rect width="40" height="10" fill="#c00000"/></svg>"##;
        let r = rect(1_000_000, 2_000_000, 41_000_000, 12_000_000);
        let out = svg_items(svg, r).unwrap();
        assert_eq!(out.items.len(), 1);
        assert_eq!(out.skipped, 0);
        let ItemKind::Path(p) = &out.items[0].kind else {
            panic!("a path")
        };
        assert_eq!(p.fill, Some(Brush::Solid(Color::from_rgb8(0xc0, 0, 0))));
        assert_eq!(p.stroke, None);
        assert_eq!(p.path.bounds(), Some(r));
    }

    #[test]
    fn strokes_transforms_opacity_and_gradients_map_into_the_rectangle() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 10 10">
            <defs><linearGradient id="g"><stop offset="0" stop-color="#000000"/><stop offset="1" stop-color="#ffffff"/></linearGradient></defs>
            <g transform="translate(5 0)" opacity="0.5">
              <path d="M0 0 L5 10" stroke="#0000ff" stroke-width="1" stroke-linecap="round" stroke-dasharray="1 2" fill="none"/>
            </g>
            <circle cx="2" cy="2" r="1" fill="url(#g)"/>
            <text x="1" y="9">x</text>
        </svg>"##;
        let r = rect(0, 0, 10_000_000, 10_000_000);
        let out = svg_items(svg, r).unwrap();
        // The text is dropped by the parser (usvg's `text` feature is off).
        assert_eq!(out.items.len(), 2, "{out:?}");
        let ItemKind::Path(line) = &out.items[0].kind else {
            panic!("a path")
        };
        let s = line.stroke.as_ref().expect("a stroke");
        assert_eq!(s.width, Nm(1_000_000), "1 unit of a 10-unit view box over 10 mm");
        assert_eq!(s.cap, Cap::Round);
        assert_eq!(s.brush, Brush::Solid(Color::from_rgba8(0, 0, 255, 128)));
        assert_eq!(
            s.dash.as_ref().map(|d| d.segments.clone()),
            Some(vec![Nm(1_000_000), Nm(2_000_000)])
        );
        assert_eq!(
            line.path.bounds(),
            Some(rect(5_000_000, 0, 10_000_000, 10_000_000)),
            "the group's translation applies"
        );
        let ItemKind::Path(dot) = &out.items[1].kind else {
            panic!("a path")
        };
        assert_eq!(
            dot.fill,
            Some(Brush::Solid(Color::from_rgba8(128, 128, 128, 255)))
        );
    }

    #[test]
    fn hostile_and_broken_svgs_are_not_drawn() {
        let r = rect(0, 0, 10, 10);
        assert_eq!(
            svg_items(br#"<svg xmlns="http://www.w3.org/2000/svg" onload="x()"/>"#, r),
            Err(SvgError::Hazard(ew_schema::SvgHazard::EventAttribute))
        );
        assert!(matches!(
            svg_items(b"<svg", r),
            Err(SvgError::Hazard(_) | SvgError::Parse(_))
        ));
        assert!(matches!(svg_items(b"<html/>", r), Err(SvgError::Parse(_))));
    }

    #[test]
    fn a_refused_svg_says_how_to_export_again() {
        for h in [
            ew_schema::SvgHazard::ForeignObject,
            ew_schema::SvgHazard::TooComplex,
        ] {
            assert_eq!(
                SvgError::Hazard(h).to_string(),
                format!("the SVG is refused: {}; {}", h.describe(), h.advice())
            );
        }
    }

    #[test]
    fn resolving_replaces_svg_images_and_fills_raster_bytes() {
        let svg: Arc<[u8]> = Arc::from(
            &br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><rect width="4" height="4"/></svg>"#[..],
        );
        let png: Arc<[u8]> = Arc::from(&b"\x89PNG"[..]);
        let image = |sha: u8, format| {
            Arc::new(ImageData {
                sha256: [sha; 32],
                format,
                bytes: Arc::from(Vec::new()),
            })
        };
        let r = rect(0, 0, 4_000, 4_000);
        let list = DisplayList {
            items: vec![
                Item::image(image(1, ImageFormat::Svg), r),
                Item::group(None, None, vec![Item::image(image(2, ImageFormat::Png), r)]),
                Item::image(image(3, ImageFormat::Jpeg), r),
            ],
        };
        let assets = |sha: &[u8; 32]| match sha[0] {
            1 => Some(Arc::clone(&svg)),
            2 => Some(Arc::clone(&png)),
            _ => None,
        };
        let (out, report) = resolve_images(&list, &assets);
        assert_eq!(
            report,
            ImageReport {
                vector: 1,
                raster: 1,
                missing: 1,
                refused: 0,
                issues: vec![ImageIssue {
                    sha256: [3; 32],
                    problem: ImageProblem::Missing
                }],
            }
        );
        let ItemKind::Group(g) = &out.items[0].kind else {
            panic!("the SVG became a group")
        };
        assert_eq!(g.clip, Some(r));
        assert!(matches!(g.items[0].kind, ItemKind::Path(_)));
        let ItemKind::Group(g) = &out.items[1].kind else {
            panic!("a group")
        };
        let ItemKind::Image(img) = &g.items[0].kind else {
            panic!("an image")
        };
        assert_eq!(&*img.image.bytes, &*png);
        assert!(
            matches!(out.items[2].kind, ItemKind::Image(_)),
            "missing bytes keep the item"
        );
        // A list without images comes back unchanged.
        let plain = DisplayList {
            items: vec![Item::path(Path::rect(r), None, Some(Brush::BLACK))],
        };
        assert_eq!(resolve_images(&plain, &assets).0, plain);
    }

    #[test]
    fn a_drawing_program_logo_draws_its_paths() {
        let svg = br##"<?xml version="1.0" encoding="utf-8"?>
<svg version="1.1" id="Layer_1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 120 30">
<style type="text/css">.st0{clip-path:url(#SVGID_2_);fill:url(#SVGID_3_);} .st1{fill:#1E9E3F;}</style>
<defs><rect id="SVGID_1_" width="120" height="30"/></defs>
<clipPath id="SVGID_2_"><use xlink:href="#SVGID_1_" style="overflow:visible;"/></clipPath>
<linearGradient id="SVGID_3_" gradientUnits="userSpaceOnUse" x1="0" y1="15" x2="120" y2="15"><stop offset="0" style="stop-color:#000000"/><stop offset="1" style="stop-color:#FFFFFF"/></linearGradient>
<rect class="st0" width="120" height="30"/>
<path class="st1" d="M30 5 h10 v20 z"/>
</svg>"##;
        let out = svg_items(svg, rect(0, 0, 120_000, 30_000)).unwrap();
        assert_eq!(out.items.len(), 2, "{out:?}");
        assert!(out.skipped >= 1, "the clip path is dropped and counted");
        let fills: Vec<Option<Brush>> = out
            .items
            .iter()
            .map(|i| match &i.kind {
                ItemKind::Path(p) => p.fill,
                _ => None,
            })
            .collect();
        assert_eq!(
            fills,
            [
                Some(Brush::Solid(Color::from_rgb8(128, 128, 128))),
                Some(Brush::Solid(Color::from_rgb8(0x1e, 0x9e, 0x3f)))
            ]
        );
    }

    /// Review fix 5: a cycle in a gradient `href` chain that returns to neither end (`g1 -> g2 -> g3 -> g2`, no
    /// gradient with stops) makes usvg's chain walk run forever, which `catch_unwind` cannot stop. Proved on a worker
    /// thread given 3 seconds (it is left spinning; nextest runs each test in its own process); the rejecter refuses
    /// the document before usvg sees it.
    #[test]
    fn a_gradient_href_cycle_hangs_usvg_and_is_refused_before_it() {
        const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><linearGradient id="g1" href="#g2"/><linearGradient id="g2" href="#g3"/><linearGradient id="g3" href="#g2"/><rect width="4" height="4" fill="url(#g1)"/></svg>"##;
        assert_eq!(
            svg_items(SVG.as_bytes(), rect(0, 0, 4, 4)),
            Err(SvgError::Hazard(ew_schema::SvgHazard::TooComplex))
        );
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let parsed = usvg::Tree::from_str(SVG, &options()).is_ok();
            let _ = tx.send(parsed);
        });
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(3)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout),
            "usvg finished on the gradient cycle"
        );
    }
}
