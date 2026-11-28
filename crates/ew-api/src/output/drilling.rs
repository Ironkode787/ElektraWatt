// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The drawings for drilling (doc 13 §9.1): per surface with holes a template to print at full size
//! (`<location> <surface> template.pdf`, its sheets from `ew_derive::layout::template_sheets` drawn by
//! `ew_scene::template`) and a DXF for a machine (`<location> <surface>.dxf`, its entities per layer from
//! `surface_features`, written by `ew_export::drilling_dxf`), all from the one query the hole table reads, in the
//! project's language. The files are made in memory; [`Session::write_drilling_files`] writes each whole.

use std::io;
use std::path::Path;

use ew_derive::layout::{
    Made, Paper, SurfaceFeatures, TEMPLATE_OVERLAP, carrier_labels, drawable, face_of, surface_datum,
    surface_page, template_sheets,
};
use ew_derive::{Q, print_language};
use ew_export::{DrillEntity, DrillSheet, PdfPage};
use ew_ids::{Id, Uuid};
use ew_l10n::{List, Text, TextArg};
use ew_schema::{Corner, Enclosure, Hinge, LayoutSpace, MountingSurface, SurfaceKind};
use ew_units::{Nm, Pt2, Rect};

use crate::Session;

/// A surface the Drilling… dialog offers: its words, its page, how many holes and how many printed sheets its
/// template takes on the paper asked for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DrillingSurface {
    /// The surface.
    pub surface: Uuid,
    /// Its words ("Door"), in the window's language by the caller.
    pub words: crate::Text,
    /// The folio of its page; `None` without one.
    pub folio: Option<String>,
    /// Its holes.
    pub holes: usize,
    /// The printed sheets of its template.
    pub sheets: usize,
}

/// One file of the drawings for drilling.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DrillingFile {
    /// Its name, its extension included.
    pub name: String,
    /// Its bytes.
    pub bytes: Vec<u8>,
}

/// The name of the folder the drawings for drilling stand in ("Drilling", Dutch "Boortekeningen"): beside the project,
/// where Drilling… writes them (in the window's language), and in the package (in the project's).
pub fn drilling_folder() -> Text {
    text!("drilling-folder")
}

/// What the drawings for drilling are made of.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DrillingAsk {
    /// The surfaces, in the order their files are made.
    pub surfaces: Vec<Uuid>,
    /// The paper of the templates.
    pub paper: Paper,
    /// Make the templates.
    pub template: bool,
    /// Make the DXFs.
    pub dxf: bool,
    /// The templates' creation date; `None`: the model's (the package's date, doc 13-package §8.8).
    pub date: Option<super::PdfDate>,
}

/// The layer of a way of making a hole (doc 13 §9.1): `HOLE_4.5`, `TAP_M5`, `CUT_22.3`, `CUT_RECT`.
fn layer_of(made: &Made) -> String {
    let d = |n: Nm| ew_derive::mm_tenth_short(i128::from(n.0));
    match made {
        Made::Drill { d: n } => format!("HOLE_{}", d(*n)),
        Made::Tap { screw, drill } => match screw.mm() {
            Some(m) => format!("TAP_M{m}"),
            None => format!("TAP_{}", d(*drill)),
        },
        Made::Punch { d: n } => format!("CUT_{}", d(*n)),
        Made::Cut { .. } => "CUT_RECT".to_owned(),
    }
}

/// The rank of a layer in doc 13 §9.1's order.
fn layer_rank(layer: &str) -> u8 {
    match layer.split('_').next() {
        Some("HOLE") => 0,
        Some("TAP") => 1,
        Some("CUT") if layer != "CUT_RECT" => 2,
        _ => 3,
    }
}

/// The sheet of `s` in its frame.
fn sheet_of(s: &MountingSurface) -> Option<Rect> {
    let xs = s.usable.iter().map(|p| p.x.0);
    let ys = s.usable.iter().map(|p| p.y.0);
    Some(Rect::from_points(
        Pt2::new(Nm(xs.clone().min()?), Nm(ys.clone().min()?)),
        Pt2::new(Nm(xs.max()?), Nm(ys.max()?)),
    ))
}

/// The datum corner of `sheet` for `corner`, in the surface frame.
fn corner_point(sheet: Rect, corner: &Corner) -> Pt2 {
    let (lo, hi) = (sheet.min(), sheet.max());
    match corner {
        Corner::BottomLeft | Corner::Unknown(_) => lo,
        Corner::BottomRight => Pt2::new(hi.x, lo.y),
        Corner::TopLeft => Pt2::new(lo.x, hi.y),
        Corner::TopRight => hi,
    }
}

/// The location's designation of the cabinet of `s` ("+CAB"), empty without one.
fn location_of(q: &Q, s: &MountingSurface) -> String {
    let Some(space) = q.table::<Enclosure>().get(s.enclosure).map(|e| e.space) else {
        return String::new();
    };
    let Some(location) = q.table::<LayoutSpace>().get(space).map(|l| l.location) else {
        return String::new();
    };
    let path = q.get::<ew_derive::StructurePath>(location);
    if path.codes.is_empty() {
        String::new()
    } else {
        format!("+{}", path.codes.join("+"))
    }
}

/// A rectangle's outline as a closed polyline with its corner radius `r` as bulges, from its bottom-left.
fn rect_outline(c: Pt2, w: Nm, h: Nm, r: Option<Nm>) -> DrillEntity {
    let (hw, hh) = (w.0 / 2, h.0 / 2);
    let r = r.map_or(0, |r| r.0.min(hw).min(hh)).max(0);
    let p = |x: i64, y: i64| Pt2::new(Nm(c.x.0 + x), Nm(c.y.0 + y));
    if r == 0 {
        return DrillEntity::Polyline {
            pts: vec![p(-hw, -hh), p(hw, -hh), p(hw, hh), p(-hw, hh)],
            closed: true,
            bulges: Vec::new(),
        };
    }
    // A quarter arc's bulge: tan(90° / 4).
    const QUARTER: i64 = 414_213_562;
    DrillEntity::Polyline {
        pts: vec![
            p(-hw + r, -hh),
            p(hw - r, -hh),
            p(hw, -hh + r),
            p(hw, hh - r),
            p(hw - r, hh),
            p(-hw + r, hh),
            p(-hw, hh - r),
            p(-hw, -hh + r),
        ],
        closed: true,
        bulges: vec![0, QUARTER, 0, QUARTER, 0, QUARTER, 0, QUARTER],
    }
}

impl Session {
    /// Whether a surface of the project has holes its drawings can be made of: what Drilling… needs.
    pub fn has_holes(&self) -> bool {
        let q = self.query();
        q.table::<MountingSurface>()
            .iter()
            .any(|(id, _)| !q.get::<SurfaceFeatures>(id).is_empty() && drawable(&q, id))
    }

    /// The surfaces the drawings for drilling can be made of: every surface with holes whose coordinates a drawing
    /// takes (`ew_derive::layout::drawable`), in the page order of their pages (a surface without a page last), with its
    /// template's printed sheets on `paper`.
    pub fn drilling_surfaces(&self, paper: Paper) -> Vec<DrillingSurface> {
        let q = self.query();
        let order: Vec<_> = q
            .get::<ew_derive::PageOrder>(())
            .entries
            .iter()
            .filter_map(|e| match e.page {
                ew_derive::PageRef::Drawn(p) => Some((p, e.folio.clone())),
                _ => None,
            })
            .collect();
        let mut out: Vec<(usize, DrillingSurface)> = q
            .table::<MountingSurface>()
            .iter()
            .filter_map(|(id, _)| {
                let holes = q.get::<SurfaceFeatures>(id).len();
                if holes == 0 || !drawable(&q, id) {
                    return None;
                }
                let face = face_of(&q, id)?;
                let page = surface_page(&q, id);
                let at = page.and_then(|p| order.iter().position(|(o, _)| *o == p));
                let sheets = template_sheets(&q, id, paper).map_or(0, |t| t.printed());
                Some((
                    at.unwrap_or(usize::MAX),
                    DrillingSurface {
                        surface: id.as_uuid(),
                        words: ew_derive::text::surface_word(&face.kind, face.door, face.doors),
                        folio: at.map(|i| order[i].1.clone()),
                        holes,
                        sheets,
                    },
                ))
            })
            .collect();
        out.sort_by_key(|(at, d)| (*at, d.surface));
        out.into_iter().map(|(_, d)| d).collect()
    }

    /// The files of the drawings for drilling of `ask` (the module documentation): per surface its template's PDF and
    /// its DXF, as asked; a surface without holes makes none.
    ///
    /// # Errors
    ///
    /// The PDF writer's refusal, in words.
    pub fn drilling_files(&self, ask: &DrillingAsk) -> Result<Vec<DrillingFile>, String> {
        let q = self.query();
        let lang = print_language(&q);
        // Every word of the drawings in the project's language, as a page prints it.
        let words = |t: &Text| super::print::in_print_language(t, lang.as_ref());
        let mut out = Vec::new();
        for id in &ask.surfaces {
            let id: Id<MountingSurface> = Id::from_uuid(*id);
            let Some(s) = q.table::<MountingSurface>().get(id).cloned() else {
                continue;
            };
            let features = q.get::<SurfaceFeatures>(id);
            let (Some(face), Some(sheet)) = (face_of(&q, id), sheet_of(&s)) else {
                continue;
            };
            if features.is_empty() || !drawable(&q, id) {
                continue;
            }
            let location = location_of(&q, &s);
            let surface = ew_derive::text::surface_word(&face.kind, face.door, face.doors);
            // The files are named in the project's language too (doc 13-package §8.2).
            let surface_words = words(&surface);
            let lower = surface_words.to_lowercase();
            let seen = ew_derive::text::surface_seen(&face.kind);
            let corner = surface_datum(&q, &s);
            let origin = corner_point(sheet, &corner);
            let labels: std::collections::BTreeMap<_, _> = carrier_labels(&q, id).into_iter().collect();
            let owners = |f: &ew_derive::layout::Feature| -> String {
                let list: Vec<TextArg> = f
                    .owners
                    .iter()
                    .filter_map(|o| ew_reports::hole_owner_words(&q, o, &labels, &f.hole))
                    .collect();
                let between = || Text::bare(TextArg::Plain(", ".to_owned()));
                words(&Text::bare(List::new(list, between(), between())))
            };
            if ask.dxf {
                let rel = |p: Pt2| Pt2::new(Nm(p.x.0 - origin.x.0), Nm(p.y.0 - origin.y.0));
                let mut holes: Vec<(String, u32, DrillEntity, DrillEntity)> = features
                    .iter()
                    .map(|f| {
                        let at = rel(f.at);
                        let (w, h) = f.made.extent();
                        let shape = match &f.made {
                            Made::Cut { w, h, r } => rect_outline(at, *w, *h, *r),
                            _ => DrillEntity::Circle { c: at, d: w },
                        };
                        let number = DrillEntity::Text {
                            at: Pt2::new(Nm(at.x.0 + w.0 / 2 + 1_000_000), Nm(at.y.0 + h.0 / 2 + 1_000_000)),
                            h: Nm(3_000_000),
                            s: format!("H{}", f.no),
                        };
                        (layer_of(&f.made), f.no, shape, number)
                    })
                    .collect();
                holes.sort_by(|a, b| (layer_rank(&a.0), &a.0, a.1).cmp(&(layer_rank(&b.0), &b.0, b.1)));
                let (lo, hi) = (rel(sheet.min()), rel(sheet.max()));
                let mut layers: Vec<(String, Vec<DrillEntity>)> = vec![(
                    "OUTLINE".to_owned(),
                    vec![DrillEntity::Polyline {
                        pts: vec![lo, Pt2::new(hi.x, lo.y), hi, Pt2::new(lo.x, hi.y)],
                        closed: true,
                        bulges: Vec::new(),
                    }],
                )];
                if let Some(m) = s.edge_margin.filter(|m| m.0 > 0) {
                    let (a, b) = (Pt2::new(lo.x + m, lo.y + m), Pt2::new(hi.x - m, hi.y - m));
                    layers.push((
                        "MARGIN".to_owned(),
                        vec![DrillEntity::Polyline {
                            pts: vec![a, Pt2::new(b.x, a.y), b, Pt2::new(a.x, b.y)],
                            closed: true,
                            bulges: Vec::new(),
                        }],
                    ));
                }
                let mut numbers = Vec::new();
                for (layer, _, shape, number) in holes {
                    match layers.last_mut() {
                        Some((l, ents)) if *l == layer => ents.push(shape),
                        _ => layers.push((layer, vec![shape])),
                    }
                    numbers.push(number);
                }
                layers.push(("NUMBERS".to_owned(), numbers));
                let arm = 5_000_000;
                let line = |a: Pt2, b: Pt2| DrillEntity::Polyline {
                    pts: vec![a, b],
                    closed: false,
                    bulges: Vec::new(),
                };
                let datum = words(&ew_derive::text::drill_datum(&corner, &lower, seen.clone()));
                layers.push((
                    "DATUM".to_owned(),
                    vec![
                        line(Pt2::new(Nm(-arm), Nm(0)), Pt2::new(Nm(arm), Nm(0))),
                        line(Pt2::new(Nm(0), Nm(-arm)), Pt2::new(Nm(0), Nm(arm))),
                        DrillEntity::Text {
                            at: Pt2::new(Nm(2_000_000), Nm(-8_000_000)),
                            h: Nm(3_000_000),
                            s: datum,
                        },
                    ],
                ));
                let file = ew_export::drilling_dxf(&DrillSheet {
                    name: format!("{} {surface_words}.dxf", location).trim().to_owned(),
                    extent: (lo, hi),
                    layers,
                });
                out.push(DrillingFile {
                    name: file.name,
                    bytes: file.text.into_bytes(),
                });
            }
            if ask.template
                && let Some(t) = template_sheets(&q, id, ask.paper)
            {
                // The project as its title block names it: its reference, else its description.
                let master = &q.project().master;
                let project = master
                    .project_ref
                    .as_ref()
                    .map_or_else(|| super::view::neutral(&master.description), ToString::to_string);
                let revision = ew_derive::current_revision(&q)
                    .map(|r| r.label.to_string())
                    .unwrap_or_default();
                let caption = words(&ew_derive::text::surface_caption(
                    &face.kind, face.door, face.doors,
                ));
                let what = words(&ew_derive::text::template_band_what(
                    &project, &location, &caption, &revision,
                ));
                let (left, top) = (t.left(), t.top());
                // The two edges at the corner, each along the sheet's side, with its words.
                let (lo, hi) = (t.sheet.min(), t.sheet.max());
                let ex = if left { lo.x } else { hi.x };
                let ey = if top { hi.y } else { lo.y };
                let edges = vec![
                    (
                        Pt2::new(ex, lo.y),
                        Pt2::new(ex, hi.y),
                        words(&ew_derive::text::template_edge(
                            if left { "left" } else { "right" },
                            &lower,
                        )),
                    ),
                    (
                        Pt2::new(lo.x, ey),
                        Pt2::new(hi.x, ey),
                        words(&ew_derive::text::template_edge(
                            if top { "top" } else { "bottom" },
                            &lower,
                        )),
                    ),
                ];
                let margin = s.edge_margin.filter(|m| m.0 > 0).and_then(|m| {
                    let (a, b) = (Pt2::new(lo.x + m, lo.y + m), Pt2::new(hi.x - m, hi.y - m));
                    (a.x < b.x && a.y < b.y).then(|| Rect::from_points(a, b))
                });
                let hinge = (s.kind == SurfaceKind::Door).then(|| {
                    let x = if s.hinge == Some(Hinge::Right) { hi.x } else { lo.x };
                    (Pt2::new(x, lo.y), Pt2::new(x, hi.y))
                });
                let holes: Vec<ew_scene::template::TemplateHole> = features
                    .iter()
                    .map(|f| ew_scene::template::TemplateHole {
                        at: f.at,
                        size: f.made.extent(),
                        round: !matches!(f.made, Made::Cut { .. }),
                        words: format!(
                            "H{} \u{b7} {} \u{b7} {}",
                            f.no,
                            words(&ew_derive::text::made(&f.made)),
                            owners(f)
                        ),
                    })
                    .collect();
                // Where four sheets meet: the middle of each shared corner square.
                let joins: Vec<Pt2> = t
                    .tiles
                    .iter()
                    .filter(|a| a.row + 1 < t.rows && a.col + 1 < t.cols)
                    .map(|a| {
                        let x = if left {
                            a.area.max().x.0 - TEMPLATE_OVERLAP.0 / 2
                        } else {
                            a.area.min().x.0 + TEMPLATE_OVERLAP.0 / 2
                        };
                        let y = if top {
                            a.area.min().y.0 + TEMPLATE_OVERLAP.0 / 2
                        } else {
                            a.area.max().y.0 - TEMPLATE_OVERLAP.0 / 2
                        };
                        Pt2::new(Nm(x), Nm(y))
                    })
                    .collect();
                let printed = t.printed();
                // The map as the paper shows it: its rows top down, its columns left to right.
                let map_at = |row: u32, col: u32| {
                    let r = if top { row } else { t.rows - 1 - row };
                    let c = if left { col } else { t.cols - 1 - col };
                    (r, c)
                };
                let on_map: Vec<(u32, u32)> = t
                    .tiles
                    .iter()
                    .filter(|a| a.number.is_some())
                    .map(|a| map_at(a.row, a.col))
                    .collect();
                let fonts = super::fonts(&q);
                let styles = super::layers::text_style_table(&q);
                let mut pages = Vec::new();
                for tile in &t.tiles {
                    let Some(n) = tile.number else { continue };
                    let (mr, mc) = map_at(tile.row, tile.col);
                    let mm = |n: i64| ew_derive::mm_tenth_short(i128::from(n));
                    let (x0, x1) = if left {
                        (tile.area.min().x.0 - lo.x.0, tile.area.max().x.0 - lo.x.0)
                    } else {
                        (hi.x.0 - tile.area.max().x.0, hi.x.0 - tile.area.min().x.0)
                    };
                    let (y0, y1) = if top {
                        (hi.y.0 - tile.area.max().y.0, hi.y.0 - tile.area.min().y.0)
                    } else {
                        (tile.area.min().y.0 - lo.y.0, tile.area.max().y.0 - lo.y.0)
                    };
                    let place = ew_derive::text::template_band_sheet(
                        n,
                        printed,
                        tile.row + 1,
                        tile.col + 1,
                        (&mm(x0.max(0)), &mm(x1.min(t.sheet.width().0))),
                        if left { "left" } else { "right" },
                        (&mm(y0.max(0)), &mm(y1.min(t.sheet.height().0))),
                        if top { "top" } else { "bottom" },
                    );
                    let right_n = t
                        .tiles
                        .iter()
                        .any(|o| o.row == tile.row && map_at(o.row, o.col).1 == mc + 1);
                    let up_n = mr > 0;
                    let left_n = mc > 0;
                    let down_n = mr + 1 < t.rows;
                    let view = ew_scene::template::TemplateView {
                        paper: t.paper,
                        area: tile.area,
                        holes: holes.clone(),
                        edges: edges.clone(),
                        margin,
                        hinge,
                        datum: Some(origin),
                        tint: (right_n, up_n),
                        cut: (left_n, down_n),
                        joins: joins.clone(),
                        band: vec![
                            what.clone(),
                            words(&place),
                            words(&ew_derive::text::template_band_datum(&corner)),
                        ],
                        map: ew_scene::template::TemplateMap {
                            rows: t.rows,
                            cols: t.cols,
                            this: (mr, mc),
                            printed: on_map.clone(),
                        },
                        bar: words(&ew_derive::text::template_bar()),
                    };
                    let content = ew_scene::template::template_sheet(&view, fonts, &styles);
                    pages.push(PdfPage {
                        destination: ew_dl::Destination::Page(Uuid::from_u128(u128::from(n))),
                        label: n.to_string(),
                        title: words(&place),
                        section: None,
                        width: t.paper.0,
                        height: t.paper.1,
                        content,
                        placement: None,
                    });
                }
                let stem = words(&ew_derive::text::template_file(&location, &surface_words));
                let mut options =
                    ew_export::PdfOptions::new(ask.date.unwrap_or_else(|| super::model_date(&q)));
                options.title = Some(stem.clone());
                options.creator = Some(format!("ElektraWatt {}", env!("CARGO_PKG_VERSION")));
                options.document_id =
                    format!("ew-drilling/1 {} {}", id.as_uuid().hyphenated(), ask.paper.word());
                let pdf = ew_export::pdf(&pages, &options, &super::DrawingFonts, None)
                    .map_err(|e| e.to_string())?;
                out.push(DrillingFile {
                    name: format!("{}.pdf", stem.trim()),
                    bytes: pdf.bytes,
                });
            }
        }
        Ok(out)
    }

    /// Writes `files` into the folder `dir`, made where it is missing, each file whole or not at all and never outside
    /// `dir` ([`ew_store::file::write_in`]).
    ///
    /// # Errors
    ///
    /// The I/O error of the folder or of the first file that could not be written (a name that is no plain file name:
    /// `InvalidInput`); the files before it are written.
    pub fn write_drilling_files(dir: &Path, files: &[DrillingFile]) -> io::Result<()> {
        std::fs::create_dir_all(dir)?;
        for f in files {
            ew_store::file::write_in(dir, &f.name, &f.bytes)?;
        }
        Ok(())
    }
}
