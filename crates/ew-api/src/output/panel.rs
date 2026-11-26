// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The surface view of a panel-layout page as the scene reads it
//! ([`ew_scene::panel`]): the parts of a [`PanelView`] each panel layer reads, filled from the derived layout queries
//! of `ew_derive::layout` (`placement_geometry`, `carrier_geometry`, `layout_label`), never re-derived here.
//!
//! | Part | Layer | Reads |
//! |---|---|---|
//! | [`PageSurfaceView`] | every panel layer | the page row (kind, scale), its page items and its frame's page-to-sheet map |
//! | the surface outlines | `surface` | the surface, its enclosure, the enclosure's and the plate's devices and their part copies (the enclosure and mounting-plate sections) |
//! | the carriers | `carriers` | `carriers_of_surface`, `carrier_geometry`, the carriers' `params` |
//! | the placements | `placements` | the placements on the surface's carriers and on the surface, `placement_geometry`; the end brackets of the surface's rails (`group_accessories`, `rail_occupancy` through `rail_items`) |
//! | the labels | `labels` and `carriers` | the same placements, `layout_label`, [`ShapedText`] in the label style, the surface's usable polygon |
//!
//! The carriers layer reads the labels for their knockout boxes (the carriers are cut around the labels).
//! So a `layout.move` rebuilds the `placements`, `labels` and `carriers` families of its page
//! only, never `surface`, `frame` or `tables`.

use std::sync::Arc;

use ew_derive::layout::{
    CarrierGeometry, GroupAccessories, LabelMode, LayoutLabel, PlacementGeometry, Profile, Quarter, RailAxis,
    RailExtent, RailItem, Unplaced, brackets, carrier_labels, free_length, member_order, member_slot,
    primary_part, rail_extent, rail_items, runs,
};
use ew_derive::{DeviceTag, PageRef, Policy, Q, Query, StructurePath, Tag};
use ew_ids::{Id, Uuid};
use ew_scene::forms::SheetMap;
use ew_scene::panel::{
    BracketDraw, CarrierDraw, LAYOUT_LABEL_STYLE, LabelShow, PanelLabel, PanelView, PlacementDraw,
    Projection, SurfaceOutline,
};
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, Device, Enclosure, LayoutLabels, LayoutSpace, LibCopy,
    MountKind, MountingSurface, Page, PageItem, PageItemKind, PageType, Part, Placement, StructureNode,
    SurfaceLabelMode,
};
use ew_text::{Shaped, TextStyle};
use ew_units::arith::div_round_ties_away;
use ew_units::{Nm, Pt2, Rect};

use super::layers::text_style_table;
use super::layout_edit::comma;
use super::view::PageFrameOf;

/// `shaped_text((text, style))`: a text shaped in a style by the project's fonts (ADR-0015's memoised `shaped_text`;
/// the fit test of a panel page's labels reads it). A function of its key and [`super::ProjectFontsQ`], so it
/// runs again for a text it has shaped only when the project's typeface changes.
#[derive(Debug)]
pub struct ShapedText;

impl Query for ShapedText {
    type Key = (String, TextStyle);
    type Value = Shaped;
    const FAMILY: &'static str = "shaped_text";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Shaped> {
        Arc::new(super::fonts(q).shape(&key.0, &key.1, None))
    }
}

/// The value of [`PageSurfaceView`]: a panel-layout page's surface view.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SurfaceViewOf {
    /// The view's page item.
    pub item: Id<PageItem>,
    /// The surface shown.
    pub surface: Id<MountingSurface>,
    /// How its placements are labelled.
    pub mode: LabelMode,
    /// What each placement is labelled with (the view's `labels`; a value of a newer format draws tags).
    pub show: LabelShow,
    /// The page projection at the page's scale.
    pub projection: Projection,
}

/// `page_surface_view(page)`: the surface view of a panel-layout page: its first `surface_view` item by ID (a page
/// holds at most one; a file with more draws the first), projected at the page's scale on paper: divided by the
/// frame's page-to-sheet factor as well ([`Projection::on_sheet`]). `None` for a missing page, a page of another kind
/// or a page without a view.
#[derive(Debug)]
pub struct PageSurfaceView;

impl Query for PageSurfaceView {
    type Key = Id<Page>;
    type Value = Option<SurfaceViewOf>;
    const FAMILY: &'static str = "page_surface_view";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Option<SurfaceViewOf>> {
        let Some(page) = q.pages().get(*key).cloned() else {
            return Arc::new(None);
        };
        if page.kind != PageType::PanelLayout {
            return Arc::new(None);
        }
        // The frame's page-to-sheet factor, so that the page's stated scale holds on paper (review part 1,
        // finding 5); a page without a frame, or a frame without the map, prints at 1.
        let sheet_ppb = q
            .get::<PageFrameOf>(PageRef::Drawn(*key))
            .as_ref()
            .as_ref()
            .map_or(SheetMap::IDENTITY, |f| SheetMap::of(&f.form))
            .scale_ppb;
        let items = q.page_items();
        let mut ids: Vec<Id<PageItem>> = q.idx().page_items_by_page(*key).collect();
        ids.sort();
        let view = ids.into_iter().find_map(|id| {
            let item = items.get(id)?;
            let PageItemKind::SurfaceView(v) = &item.kind else {
                return None;
            };
            Some(SurfaceViewOf {
                item: id,
                surface: v.surface,
                mode: match v.label_mode {
                    SurfaceLabelMode::Full => LabelMode::Full,
                    SurfaceLabelMode::Abbreviated | SurfaceLabelMode::Unknown(_) => LabelMode::Abbreviated,
                },
                show: match v.labels {
                    LayoutLabels::ItemNumbers => LabelShow::ItemNumbers,
                    LayoutLabels::Both => LabelShow::Both,
                    LayoutLabels::TagsOrNumbers => LabelShow::TagsOrNumbers,
                    LayoutLabels::Tags | LayoutLabels::Unknown(_) => LabelShow::Tags,
                },
                projection: Projection::on_sheet(v.datum_at, &page.scale, sheet_ppb),
            })
        });
        Arc::new(view)
    }
}

/// Which parts of a [`PanelView`] to fill.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Parts {
    pub surface: bool,
    pub carriers: bool,
    pub placements: bool,
    pub labels: bool,
    /// The plate's footprints behind a door (the highlights layer).
    pub behind: bool,
    /// The holes of the surface (the features layer).
    pub features: bool,
}

impl Parts {
    pub(crate) const NONE: Parts = Parts {
        surface: false,
        carriers: false,
        placements: false,
        labels: false,
        behind: false,
        features: false,
    };
}

/// The first part copy among a device's part uses that `f` finds something in.
fn part_section<T>(q: &Q, device: Id<Device>, f: impl Fn(&Part) -> Option<T>) -> Option<T> {
    let device = q.devices().get(device)?.clone();
    let parts = q.table::<LibCopy<Part>>();
    device
        .parts
        .iter()
        .find_map(|u| parts.get(u.part.copy).and_then(|c| f(&c.item)))
}

/// The bounds of a polygon.
fn bounds(pts: &[Pt2]) -> Option<Rect> {
    let mut it = pts.iter();
    let first = Rect::from_point(*it.next()?);
    Some(it.fold(first, |r, p| r.union(Rect::from_point(*p))))
}

/// The surface's outlines (doc 12 §5.2). The plate's page: the enclosure's outer rectangle (its part's enclosure
/// section, placed by the plate frame), the plate (its device's mounting-plate section, else the usable polygon's
/// bounds) and the usable polygon. Any other surface's page: its sheet, its edge margin, a door's hinge. Both: the
/// datum mark at the corner the holes are measured from and the caption, in the project's language.
fn surface_outline(q: &Q, s: &MountingSurface) -> SurfaceOutline {
    let lang = ew_derive::print_language(q);
    let words = |t: &crate::Text| super::print::in_print_language(t, lang.as_ref());
    let face = ew_derive::layout::face_of(q, s.id);
    let caption = face
        .as_ref()
        .map(|f| words(&ew_derive::text::surface_caption(&f.kind, f.door, f.doors)));
    let corner_of = |r: Rect| {
        use ew_schema::Corner as C;
        let (a, b) = (r.min(), r.max());
        let (at, dirs) = match ew_derive::layout::surface_datum(q, s) {
            C::BottomLeft | C::Unknown(_) => (a, (1, 1)),
            C::BottomRight => (Pt2::new(b.x, a.y), (-1, 1)),
            C::TopLeft => (Pt2::new(a.x, b.y), (1, -1)),
            C::TopRight => (b, (-1, -1)),
        };
        (at, dirs, "0".to_owned())
    };
    if s.kind != ew_schema::SurfaceKind::Plate {
        let sheet = bounds(&s.usable);
        let margin = s.edge_margin.filter(|m| m.0 > 0).zip(sheet).and_then(|(m, r)| {
            let a = Pt2::new(r.min().x.checked_add(m)?, r.min().y.checked_add(m)?);
            let b = Pt2::new(r.max().x.checked_sub(m)?, r.max().y.checked_sub(m)?);
            (a.x < b.x && a.y < b.y).then(|| Rect::from_points(a, b))
        });
        let hinge = (s.kind == ew_schema::SurfaceKind::Door).then(|| {
            (
                s.hinge == Some(ew_schema::Hinge::Right),
                words(&ew_derive::text::hinge_word()),
            )
        });
        return SurfaceOutline {
            surface: s.id,
            enclosure: None,
            plate: None,
            usable: Vec::new(),
            sheet,
            margin,
            hinge,
            datum: sheet.map(corner_of),
            caption,
        };
    }
    let enclosure = q.table::<Enclosure>().get(s.enclosure).cloned().and_then(|e| {
        let (w, h) = part_section(q, e.device, |p| {
            let sec = p.sections.enclosure.as_ref()?;
            Some((sec.width?, sec.height?))
        })?;
        // MVP frames carry no turn: the enclosure's corner lies at minus the frame's offset.
        let [tx, ty, _] = s.frame.t;
        let a = Pt2::new(Nm(0i64.checked_sub(tx.0)?), Nm(0i64.checked_sub(ty.0)?));
        let b = Pt2::new(Nm(w.0.checked_sub(tx.0)?), Nm(h.0.checked_sub(ty.0)?));
        Some((e.id, Rect::from_points(a, b)))
    });
    let plate = s
        .device
        .and_then(|d| {
            part_section(q, d, |p| {
                let sec = p.sections.mounting_plate.as_ref()?;
                Some(Rect::from_points(Pt2::ORIGIN, Pt2::new(sec.width?, sec.height?)))
            })
        })
        .or_else(|| bounds(&s.usable));
    SurfaceOutline {
        surface: s.id,
        enclosure,
        plate,
        usable: s.usable.clone(),
        sheet: None,
        margin: None,
        hinge: None,
        datum: plate.map(corner_of),
        caption,
    }
}

/// On a door's page, the footprints of its enclosure's plate in the door's frame (doc 12 §5.2): a plate point and a
/// door point of the same `x`, `y` in the enclosure's front view lie one in front of the other (`to_enclosure`).
fn behind_door(q: &Q, door: &MountingSurface) -> Vec<Rect> {
    if door.kind != ew_schema::SurfaceKind::Door {
        return Vec::new();
    }
    let Some(plate) = q
        .table::<MountingSurface>()
        .iter()
        .find(|(_, s)| s.enclosure == door.enclosure && s.kind == ew_schema::SurfaceKind::Plate)
        .map(|(id, _)| id)
    else {
        return Vec::new();
    };
    let shift = |p: Pt2| {
        let e = ew_derive::layout::to_enclosure(q, plate, p)?;
        Some(Pt2::new(
            e.x.checked_sub(door.frame.t[0])?,
            e.y.checked_sub(door.frame.t[1])?,
        ))
    };
    placements(q, &placements_on(q, plate))
        .into_iter()
        .filter_map(|d| {
            Some(Rect::from_points(
                shift(d.footprint.min())?,
                shift(d.footprint.max())?,
            ))
        })
        .collect()
}

/// The carriers of a surface with their geometry, by ID.
fn carriers(q: &Q, surface: Id<MountingSurface>) -> Vec<CarrierDraw> {
    let mut ids: Vec<_> = q.idx().carriers_of_surface(surface).collect();
    ids.sort();
    let table = q.table::<ew_schema::Carrier>();
    ids.into_iter()
        .filter_map(|id| {
            let geom = q.get::<CarrierGeometry>(id);
            let geom = geom.as_ref().as_ref()?;
            let c = table.get(id)?;
            let duct = match (&c.kind, &c.params) {
                (CarrierKind::Duct, CarrierParams::Duct(d)) => Some((d.width, d.height)),
                _ => None,
            };
            Some(CarrierDraw {
                carrier: id,
                axis: geom
                    .axis
                    .as_ref()
                    .and(c.path.first().copied().zip(c.path.get(1).copied())),
                outline: geom.outline.clone(),
                duct,
            })
        })
        .collect()
}

/// The placements on a surface (on its carriers and on the surface itself), by ID.
pub(crate) fn placements_on(q: &Q, surface: Id<MountingSurface>) -> Vec<Id<Placement>> {
    let mut ids: Vec<Id<Placement>> = q
        .idx()
        .carriers_of_surface(surface)
        .flat_map(|c| q.idx().placements_by_carrier(c).collect::<Vec<_>>())
        .collect();
    ids.extend(q.idx().placements_on_surface(surface));
    ids.sort();
    ids.dedup();
    ids
}

/// The placements of a surface with their geometry (`placement_geometry`; one without geometry draws nothing).
fn placements(q: &Q, ids: &[Id<Placement>]) -> Vec<PlacementDraw> {
    ids.iter()
        .filter_map(|id| {
            let g = q.get::<PlacementGeometry>(*id);
            let g = g.as_ref().as_ref()?;
            Some(PlacementDraw {
                placement: *id,
                footprint: g.footprint,
                unknown: g.unknown,
                outline: g.outline.clone(),
                round: g.round,
            })
        })
        .collect()
}

/// The derived end brackets of a surface's rails, carrier by ID then in `group_accessories`' order: each
/// a rectangle of the bracket's width along the rail by the largest footprint extent across the rail of the strips of
/// its run (the terminals' height), centred on the rail axis. A rail without a valid axis draws none.
fn bracket_draws(q: &Q, surface: Id<MountingSurface>) -> Vec<BracketDraw> {
    let table = q.table::<Carrier>();
    let mut ids: Vec<Id<Carrier>> = q.idx().carriers_of_surface(surface).collect();
    ids.sort();
    let mut out = Vec::new();
    for c in ids {
        let Some(axis) = table
            .get(c)
            .filter(|rec| rec.kind == CarrierKind::Rail)
            .and_then(|rec| RailAxis::of(&rec.path))
        else {
            continue;
        };
        let accessories = q.get::<GroupAccessories>(c);
        if accessories.is_empty() {
            continue;
        }
        let (du, dv) = axis.dir.rotate((1, 0));
        let across = |r: Rect| {
            if du == 0 {
                i128::from(r.width().0)
            } else {
                i128::from(r.height().0)
            }
        };
        // The height of each strip's run: the largest footprint extent across the rail among its strips.
        let items = rail_items(q, c);
        let mut height: std::collections::BTreeMap<Id<Placement>, i128> = std::collections::BTreeMap::new();
        for run in runs(&items) {
            let placed: Vec<Id<Placement>> = run.iter().filter_map(|k| items.get(*k)?.placement).collect();
            let h = placed
                .iter()
                .filter_map(|p| {
                    q.get::<PlacementGeometry>(*p)
                        .as_ref()
                        .as_ref()
                        .map(|g| across(g.footprint))
                })
                .max()
                .unwrap_or(0);
            height.extend(placed.into_iter().map(|p| (p, h)));
        }
        let (su, sv) = (i128::from(axis.start.x.0), i128::from(axis.start.y.0));
        let at = |s: i128, off: i128| -> Option<Pt2> {
            let u = su + s * du - off * dv;
            let v = sv + s * dv + off * du;
            Some(Pt2::new(Nm(i64::try_from(u).ok()?), Nm(i64::try_from(v).ok()?)))
        };
        for b in accessories.iter() {
            let h = height.get(&b.placement).copied().unwrap_or(0);
            let below = h / 2;
            if let (Some(a), Some(z)) = (at(b.s0, -below), at(b.s1, h - below)) {
                out.push(BracketDraw {
                    placement: b.placement,
                    rect: Rect::from_points(a, z),
                });
            }
        }
    }
    out
}

/// The label style of the project's text-style table ([`LAYOUT_LABEL_STYLE`]).
pub(crate) fn label_style(q: &Q) -> TextStyle {
    text_style_table(q).resolve_or_default(LAYOUT_LABEL_STYLE)
}

/// The labels of a surface's placements on `page` (`layout_label`), each shaped by [`ShapedText`], with its item number
/// (its position in the surface's `layout_legend`, read only when the view shows item numbers, so balloons and the
/// legend cannot disagree) and where a person moved it (`label_at`).
fn labels(q: &Q, page: Id<Page>, view: &SurfaceViewOf, ids: &[Id<Placement>]) -> Vec<PanelLabel> {
    let style = label_style(q);
    let items: std::collections::BTreeMap<Id<Placement>, u32> = if view.show == LabelShow::Tags {
        std::collections::BTreeMap::new()
    } else {
        ew_derive::layout::layout_legend(q, view.surface)
            .into_iter()
            .filter_map(|row| match row.item {
                ew_derive::layout::LegendItem::Placement(p) => Some((p, row.pos)),
                _ => None,
            })
            .collect()
    };
    let placements = q.table::<Placement>();
    ids.iter()
        .filter_map(|id| {
            let g = q.get::<PlacementGeometry>(*id);
            let g = g.as_ref().as_ref()?;
            let text = q.get::<LayoutLabel>((*id, page, view.mode)).as_ref().clone();
            let shaped = (!text.is_empty()).then(|| q.get::<ShapedText>((text.clone(), style)));
            Some(PanelLabel {
                placement: *id,
                text,
                footprint: g.footprint,
                shaped,
                item: items.get(id).copied(),
                moved: placements.get(*id).and_then(|p| p.label_at),
            })
        })
        .collect()
}

/// The keys of [`ShapedText`] the labels of `page` read (the sweep of `ew verify-derived`).
pub(crate) fn label_keys(q: &Q, page: Id<Page>) -> Vec<(String, TextStyle)> {
    let Some(view) = q.get::<PageSurfaceView>(page).as_ref().clone() else {
        return Vec::new();
    };
    let style = label_style(q);
    placements_on(q, view.surface)
        .into_iter()
        .map(|id| {
            (
                q.get::<LayoutLabel>((id, page, view.mode)).as_ref().clone(),
                style,
            )
        })
        .filter(|(t, _)| !t.is_empty())
        .collect()
}

/// The surface view of `page` with the `parts` filled; `None` for a page without one (or whose surface is gone).
pub(crate) fn panel_view(q: &Q, page: Id<Page>, parts: Parts) -> Option<PanelView> {
    let view = q.get::<PageSurfaceView>(page).as_ref().clone()?;
    let surface = q.table::<MountingSurface>().get(view.surface).cloned()?;
    let ids = if parts.placements || parts.labels {
        placements_on(q, surface.id)
    } else {
        Vec::new()
    };
    Some(PanelView {
        item: view.item,
        projection: view.projection,
        surface: parts.surface.then(|| surface_outline(q, &surface)),
        bounds: if parts.labels {
            bounds(&surface.usable)
        } else {
            None
        },
        carriers: if parts.carriers {
            carriers(q, surface.id)
        } else {
            Vec::new()
        },
        placements: if parts.placements {
            placements(q, &ids)
        } else {
            Vec::new()
        },
        brackets: if parts.placements {
            bracket_draws(q, surface.id)
        } else {
            Vec::new()
        },
        labels: if parts.labels {
            labels(q, page, &view, &ids)
        } else {
            Vec::new()
        },
        show: view.show,
        linked: parts.placements && !legend_blocks(q, surface.id).is_empty(),
        behind: if parts.behind {
            behind_door(q, &surface)
        } else {
            Vec::new()
        },
        features: if parts.features {
            feature_draws(q, &surface)
        } else {
            Vec::new()
        },
    })
}

/// The holes of `surface` as its features layer draws them (`ew_derive::layout::SurfaceFeatures`): at their made size,
/// dashed where a footprint covers the centre; a hole by hand selects its record, a derived one its first owner.
fn feature_draws(q: &Q, surface: &MountingSurface) -> Vec<ew_scene::panel::FeatureDraw> {
    use ew_derive::layout::{Made, Owner, PlacementGeometry, SurfaceFeatures};
    let footprints: Vec<Rect> = placements_on(q, surface.id)
        .into_iter()
        .chain(
            q.idx()
                .carriers_of_surface(surface.id)
                .flat_map(|c| q.idx().placements_by_carrier(c).collect::<Vec<_>>()),
        )
        .filter_map(|p| {
            q.get::<PlacementGeometry>(p)
                .as_ref()
                .as_ref()
                .map(|g| g.footprint)
        })
        .collect();
    q.get::<SurfaceFeatures>(surface.id)
        .iter()
        .filter_map(|f| {
            let hidden = footprints.iter().any(|r| r.contains(f.at));
            let entity = match f.owners.first()? {
                Owner::ByHand { feature } => Some(feature.as_uuid()),
                _ if hidden => None,
                Owner::Placement { p, .. } => Some(p.as_uuid()),
                Owner::Carrier { c, .. } => Some(c.as_uuid()),
            };
            Some(ew_scene::panel::FeatureDraw {
                at: f.at,
                size: f.made.extent(),
                round: !matches!(f.made, Made::Cut { .. }),
                hidden,
                entity,
            })
        })
        .collect()
}

/// The legend lists of `surface` (doc 12 §5.4): the page blocks of kind `enclosure_legend` whose layout page shows it,
/// by ID.
pub(crate) fn legend_blocks(q: &Q, surface: Id<MountingSurface>) -> Vec<Id<ew_schema::ReportBlock>> {
    q.report_blocks()
        .iter()
        .filter(|(_, b)| {
            b.embedded.is_none()
                && matches!(
                    b.source,
                    ew_schema::ReportSource::Kind(ew_schema::ReportKind::EnclosureLegend)
                )
                && b.layout_page
                    .is_some_and(|p| ew_derive::layout::page_surface(q, p) == Some(surface))
        })
        .map(|(id, _)| id)
        .collect()
}

/// The panel page a placement's legend row links back to: the first panel page, in page order, that shows the surface
/// the placement sits on.
pub(crate) fn placement_page(q: &Q, placement: Id<Placement>) -> Option<Id<Page>> {
    let on = q.table::<Placement>().get(placement)?.carrier.clone();
    super::layout_edit::showing_page(q, &on)
}

/// The pages of the legend lists of the surface `placement` sits on ([`legend_blocks`]), in page order: where its
/// label links to.
pub(crate) fn legend_pages(q: &Q, placement: Id<Placement>) -> Vec<PageRef> {
    let Some(surface) = q
        .table::<Placement>()
        .get(placement)
        .and_then(|p| super::layout_edit::surface_of(q, &p.carrier))
    else {
        return Vec::new();
    };
    let blocks = legend_blocks(q, surface);
    q.get::<ew_derive::PageOrder>(())
        .entries
        .iter()
        .map(|e| e.page)
        .filter(|p| matches!(p, PageRef::Generated { block, .. } if blocks.contains(block)))
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// The panel tables: the panel data a layout tool reads, `page_tables`' pattern for a
// panel-layout page, so that a tool never calls the `Session`. A plain function over `Q` that reads the existing
// layout queries (`page_surface_view`, `carrier_geometry`, `rail_occupancy` through `rail_items`,
// `group_accessories`, `placement_geometry`, `layout_label`, `unplaced`): no memo family of its own. Everything is
// in surface nm; sheet <-> surface goes through the page (the frame's page-to-sheet map, then the inverse
// projection [`surface_of_page`]).

/// A rail's axis in the surface frame: the clip point at `s` along it lies at `start + dir · s`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PanelAxis {
    /// `path[0]`, surface nm.
    pub start: (i128, i128),
    /// The unit direction from `path[0]` to `path[1]`: one of `(1, 0)`, `(0, 1)`, `(−1, 0)`, `(0, −1)`.
    pub dir: (i128, i128),
}

impl PanelAxis {
    /// The surface point at `s` along the axis.
    pub fn point_at(&self, s: i128) -> (i128, i128) {
        (
            self.start.0.saturating_add(self.dir.0.saturating_mul(s)),
            self.start.1.saturating_add(self.dir.1.saturating_mul(s)),
        )
    }

    /// The distance along the axis of the foot of the surface point `p`, and its distance from the axis line.
    pub fn project(&self, p: (i128, i128)) -> (i128, i128) {
        let (du, dv) = (p.0.saturating_sub(self.start.0), p.1.saturating_sub(self.start.1));
        let s = du
            .saturating_mul(self.dir.0)
            .saturating_add(dv.saturating_mul(self.dir.1));
        let off = du
            .saturating_mul(self.dir.1)
            .saturating_sub(dv.saturating_mul(self.dir.0))
            .saturating_abs();
        (s, off)
    }
}

/// One carrier of a panel page's surface, in `carrier_labels` order (rails, then ducts).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PanelCarrier {
    /// The carrier's ID.
    pub id: Uuid,
    /// The carrier record.
    pub carrier: Id<Carrier>,
    /// `rail` or `duct`.
    pub kind: &'static str,
    /// Its label: "Rail 1", "Duct 2" (`ew_derive::text::carrier_label`), which the window renders in its language.
    pub label: crate::Text,
    /// Its label in a sentence: "rail 1".
    pub sentence: crate::Text,
    /// The path, surface nm.
    pub path: Vec<(i64, i64)>,
    /// A rail's axis; `None` for a duct and for a rail whose path is not valid.
    pub axis: Option<PanelAxis>,
    /// The path length, nm; `None` for an invalid path.
    pub length: Option<i128>,
    /// A rail's profile.
    pub profile: Option<MountKind>,
    /// The part copy the carrier realises.
    pub part: Option<Uuid>,
    /// The width across the path: a rail's profile width, a duct's width.
    pub width: Option<i128>,
    /// A duct's height.
    pub height: Option<i128>,
    /// The outline rectangles (`carrier_geometry`), surface nm.
    pub outline: Vec<Rect>,
    /// A rail's items (`rail_items`: the list the commands resolve over).
    pub items: Vec<RailItem>,
    /// A rail's derived end brackets (`group_accessories`): `[s0, s1]` along the axis.
    pub brackets: Vec<(i128, i128)>,
    /// A rail's free length (`free_length`): `L − Σ items − Σ brackets`.
    pub free_length: Option<i128>,
}

/// One placement on a panel page's surface, by ID.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PanelPlacement {
    /// The placement's ID (the entity a click on its footprint or label selects).
    pub id: Uuid,
    /// The placement record.
    pub placement: Id<Placement>,
    /// The device's ID.
    pub device: Uuid,
    /// The layout label on this page (`layout_label`).
    pub label: String,
    /// The box of its label (or balloon) where the page prints it, page nm: what a drag of the label grabs; `None`
    /// when the page prints none.
    pub label_box: Option<Rect>,
    /// Whether a person moved its label (`label_at`): its menu offers to put it back.
    pub label_moved: bool,
    /// The device's tag as a sentence names it (`-102F1`, `T55`).
    pub tag: String,
    /// The rail it sits on; `None` on the surface.
    pub carrier: Option<Uuid>,
    /// The pose: `t.x`, `t.y` (a rail's clip point `s` and 0, or a surface's `u`, `v`) and its quarter turns.
    pub pose: (i64, i64, u8),
    /// The footprint, surface nm.
    pub footprint: Rect,
    /// Whether it is locked.
    pub locked: bool,
    /// Whether its footprint is unknown.
    pub unknown: bool,
    /// Whether it places a strip as a whole.
    pub strip: bool,
    /// A strip's declared end-bracket width.
    pub bracket_width: Option<i128>,
    /// The clearance pads along a rail, `[before, after]`, for its current turn.
    pub pads: [i128; 2],
    /// The realised part's mount kind.
    pub mount_kind: Option<MountKind>,
    /// The mount kind in words (`TS 35×7.5`, `screw-mounted`; `ew_derive::text::mount_kind`); empty without one.
    pub mount_label: crate::Text,
    /// The clip point's distance from the footprint's left edge along a rail, for its current turn.
    pub clip: i128,
    /// What it takes along a rail turned 0°.
    pub extent: RailExtent,
    /// What it takes along a rail turned 180°.
    pub extent_flipped: RailExtent,
}

/// One unplaced part use of the surface's layout space, in `unplaced` order (natural full tag, then the part use's
/// order): a device with a door coupling handle beside its switch is two rows until both are placed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnplacedDevice {
    /// The device's ID.
    pub device: Uuid,
    /// The part copy to place; `None` for a strip and a device without parts.
    pub part: Option<Uuid>,
    /// Whether it is a part use beside the primary one (a door coupling handle).
    pub second: bool,
    /// Its tag as a sentence names it (`F55`, `-201U9`).
    pub label: String,
    /// The primary part's number; `None` without a part.
    pub part_number: Option<String>,
    /// The primary part's `W × H`, nm; `None` when unknown.
    pub size: Option<(i128, i128)>,
    /// The part's mount kind.
    pub mount_kind: Option<MountKind>,
    /// The mount kind in words (`TS 35×7.5`, `screw-mounted`); empty without one.
    pub mount_label: crate::Text,
    /// Whether it is a strip placed as a whole.
    pub strip: bool,
    /// A strip's declared end-bracket width.
    pub bracket_width: Option<i128>,
    /// Whether it belongs to an assembly (it has children, or a parent).
    pub assembly: bool,
    /// What it takes along a rail turned 0°.
    pub extent: RailExtent,
    /// What it takes along a rail turned 180°.
    pub extent_flipped: RailExtent,
    /// Per rail of the surface that holds a member of its assembly: the left edge it is pushed in at
    /// (`member_slot`), in carrier order.
    pub slots: Vec<(Uuid, i128)>,
}

/// The panel data of a panel-layout page for the layout tools, all in surface nm.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PanelTables {
    /// The printed folio.
    pub folio: String,
    /// The page's ID.
    pub page: Uuid,
    /// The surface shown.
    pub surface: Uuid,
    /// The surface record.
    pub surface_id: Id<MountingSurface>,
    /// Whether the surface is the mounting plate (else a door, a wall, the top or the bottom).
    pub plate: bool,
    /// The projection the placements layer draws with (`page_surface_view`, the frame's factor included).
    pub projection: Projection,
    /// The surface's usable polygon.
    pub usable: Vec<(i64, i64)>,
    /// The location of the surface's layout space as a designation prints it (`+J-E-2416`).
    pub location: String,
    /// The carriers, in `carrier_labels` order.
    pub carriers: Vec<PanelCarrier>,
    /// The placements on the surface and on its carriers, by ID.
    pub placements: Vec<PanelPlacement>,
    /// The unplaced devices of the surface's layout space, in `unplaced` order.
    pub unplaced: Vec<UnplacedDevice>,
    /// Whether the view labels its placements with item numbers (alone, with their tags, or where a tag does not fit).
    pub numbered: bool,
    /// What the view labels its placements with.
    pub show: LabelShow,
    /// Whether the project writes a decimal comma (`settings.formatting.decimal_separator`): the tools' texts
    /// print millimetres as the project's other millimetre texts do.
    pub decimal_comma: bool,
    /// The report blocks embedded on the page (its legend), by ID: what Select picks, drags and resizes there.
    pub blocks: Vec<PanelBlock>,
}

/// A report block embedded on a panel page (a legend), page nm.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PanelBlock {
    /// The block's ID.
    pub id: Uuid,
    /// Its rectangle as stored (`ReportBlock.embedded.rect`): the room it may take.
    pub rect: Rect,
    /// The box it draws (`ew_scene::embedded::boxed_box`: a lookup table ends under its last row).
    pub drawn: Rect,
}

impl PanelTables {
    /// The embedded block with the ID `id`.
    pub fn block(&self, id: Uuid) -> Option<&PanelBlock> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// The placement row with the ID `id`.
    pub fn placement(&self, id: Uuid) -> Option<&PanelPlacement> {
        self.placements.iter().find(|p| p.id == id)
    }

    /// The carrier row with the ID `id`.
    pub fn carrier(&self, id: Uuid) -> Option<&PanelCarrier> {
        self.carriers.iter().find(|c| c.id == id)
    }

    /// The first unplaced row of the device `id` (its primary part's, while that is unplaced).
    pub fn unplaced_device(&self, id: Uuid) -> Option<&UnplacedDevice> {
        self.unplaced.iter().find(|u| u.device == id)
    }

    /// The unplaced row of the device `id` with the part `part`, else its first.
    pub fn unplaced_part(&self, id: Uuid, part: Option<Uuid>) -> Option<&UnplacedDevice> {
        part.and_then(|p| self.unplaced.iter().find(|u| u.device == id && u.part == Some(p)))
            .or_else(|| self.unplaced_device(id))
    }

    /// The surface point of the page point `p` ([`surface_of_page`]).
    pub fn to_surface(&self, p: (i64, i64)) -> (i128, i128) {
        surface_of_page(&self.projection, p)
    }

    /// The page point of the surface point `p` ([`page_of_surface`]).
    pub fn to_page(&self, p: (i128, i128)) -> (i64, i64) {
        page_of_surface(&self.projection, p)
    }
}

/// The surface point of the page point `p` by the inverse projection: `u = (x − datum.x)·den/num`,
/// `v = (datum.y − y)·den/num`, in i128 with one rounding to the nearest nm, halves away from zero.
pub fn surface_of_page(projection: &Projection, p: (i64, i64)) -> (i128, i128) {
    let num = projection.num.max(1);
    let den = projection.den;
    let (dx, dy) = (i128::from(projection.datum.x.0), i128::from(projection.datum.y.0));
    (
        div_round_ties_away((i128::from(p.0) - dx).saturating_mul(den), num),
        div_round_ties_away((dy - i128::from(p.1)).saturating_mul(den), num),
    )
}

/// The page point of the surface point `(u, v)` by the projection (`Projection::point` in i128, saturating at the
/// `i64` range).
pub fn page_of_surface(projection: &Projection, (u, v): (i128, i128)) -> (i64, i64) {
    let len = |l: i128| div_round_ties_away(l.saturating_mul(projection.num), projection.den.max(1));
    let x = i128::from(projection.datum.x.0).saturating_add(len(u));
    let y = i128::from(projection.datum.y.0).saturating_sub(len(v));
    let sat = |n: i128| i64::try_from(n).unwrap_or(if n < 0 { i64::MIN } else { i64::MAX });
    (sat(x), sat(y))
}

/// A location as a designation prints it: `+` and its path's codes (`+J-E-2416`); empty for a missing node.
pub(super) fn location_text(q: &Q, node: Id<StructureNode>) -> String {
    let path = q.get::<StructurePath>(node);
    if path.codes.is_empty() {
        return String::new();
    }
    format!("+{}", path.codes.join("+"))
}

/// A device's tag as a sentence names it (the derived short tag, `-102F1`), empty without one.
fn device_tag(q: &Q, d: Id<Device>) -> String {
    q.try_get::<DeviceTag>(d)
        .ok()
        .and_then(|t| t.as_ref().as_ref().map(Tag::short))
        .unwrap_or_default()
}

/// The root of a device's assembly (as `layout.place_assembly` finds it): the device when it has
/// children, else its parent.
fn assembly_root(q: &Q, device: &Device) -> Option<Id<Device>> {
    if q.idx().device_children(device.id).any(|c| c != device.id) {
        Some(device.id)
    } else {
        device.parent
    }
}

/// The `i64` pair of a point.
fn xy(p: Pt2) -> (i64, i64) {
    (p.x.0, p.y.0)
}

/// The carriers of a surface.
fn carriers_of(q: &Q, surface: Id<MountingSurface>) -> Vec<PanelCarrier> {
    let table = q.table::<Carrier>();
    let mut out = Vec::new();
    for (id, label) in carrier_labels(q, surface) {
        let Some(c) = table.get(id) else {
            continue;
        };
        let geom = q.get::<CarrierGeometry>(id);
        let geom = geom.as_ref().as_ref();
        let rail = c.kind == CarrierKind::Rail;
        let (profile, width, height) = match &c.params {
            CarrierParams::Rail(r) => (
                Some(r.profile.clone()),
                Profile::of(&r.profile).map(Profile::width),
                None,
            ),
            CarrierParams::Duct(d) => (None, Some(i128::from(d.width.0)), Some(i128::from(d.height.0))),
            _ => (None, None, None),
        };
        let axis = rail.then(|| RailAxis::of(&c.path)).flatten().map(|a| PanelAxis {
            start: (i128::from(a.start.x.0), i128::from(a.start.y.0)),
            dir: a.dir.rotate((1, 0)),
        });
        let length = geom.and_then(|g| g.length);
        let (items, spans, free) = if axis.is_some() {
            let items = rail_items(q, id);
            let spans = q
                .get::<GroupAccessories>(id)
                .iter()
                .map(|b| (b.s0, b.s1))
                .collect();
            let free = length.map(|l| free_length(l, &items, &brackets(&items)));
            (items, spans, free)
        } else {
            (Vec::new(), Vec::new(), None)
        };
        out.push(PanelCarrier {
            id: id.as_uuid(),
            carrier: id,
            kind: if rail { "rail" } else { "duct" },
            label: ew_derive::text::carrier_label(label),
            sentence: ew_derive::text::carrier_in_sentence(label),
            path: c.path.iter().copied().map(xy).collect(),
            axis,
            length,
            profile,
            part: c.part.as_ref().map(|p| p.copy.as_uuid()),
            width,
            height,
            outline: geom.map(|g| g.outline.clone()).unwrap_or_default(),
            items,
            brackets: spans,
            free_length: free,
        });
    }
    out
}

/// A mount kind in words; an empty text without one.
fn mount_words(kind: Option<&MountKind>) -> crate::Text {
    kind.map_or_else(
        || crate::Text::bare(crate::TextArg::Plain(String::new())),
        ew_derive::text::mount_kind,
    )
}

/// The mount kind of a part use's part.
fn mount_of(q: &Q, part: Option<&ew_schema::PartRef>) -> Option<MountKind> {
    let p = part?;
    q.table::<LibCopy<Part>>()
        .get(p.copy)
        .and_then(|c| c.item.mech.as_ref().and_then(|m| m.mount_kind.clone()))
}

/// The placements of a surface with their rail data.
fn placements_of(q: &Q, page: Id<Page>, surface: Id<MountingSurface>) -> Vec<PanelPlacement> {
    let Some(view) = q.get::<PageSurfaceView>(page).as_ref().clone() else {
        return Vec::new();
    };
    let table = q.table::<Placement>();
    let devices = q.devices();
    let labels = super::layers::page_layer(q, PageRef::Drawn(page), ew_dl::Layer::Labels);
    let mut out = Vec::new();
    for pid in placements_on(q, surface) {
        let (Some(pl), Some(geom)) = (table.get(pid), q.get::<PlacementGeometry>(pid).as_ref().clone())
        else {
            continue;
        };
        let Some(dev) = devices.get(pl.device) else {
            continue;
        };
        let turn = Quarter::of(pl.pose.q);
        let extent = rail_extent(q, dev, pl.part.as_ref(), false);
        let extent_flipped = rail_extent(q, dev, pl.part.as_ref(), true);
        let current = if turn == Some(Quarter::HALF) {
            extent_flipped
        } else {
            extent
        };
        let realised = pl.part.clone().or_else(|| primary_part(q, dev));
        out.push(PanelPlacement {
            id: pid.as_uuid(),
            placement: pid,
            device: pl.device.as_uuid(),
            label: q.get::<LayoutLabel>((pid, page, view.mode)).as_ref().clone(),
            label_box: labels.panel_labels.get(&pid.as_uuid()).copied(),
            label_moved: pl.label_at.is_some(),
            tag: device_tag(q, pl.device),
            carrier: match pl.carrier {
                CarrierRef::Carrier(c) => Some(c.as_uuid()),
                _ => None,
            },
            pose: (pl.pose.t[0].0, pl.pose.t[1].0, turn.map_or(0, Quarter::k)),
            footprint: geom.footprint,
            locked: pl.locked,
            unknown: geom.unknown,
            strip: current.strip,
            bracket_width: if current.strip {
                ew_derive::strip_bracket(q, dev).map(|b| b.width)
            } else {
                None
            },
            pads: current.pad,
            mount_kind: mount_of(q, realised.as_ref()),
            mount_label: mount_words(mount_of(q, realised.as_ref()).as_ref()),
            clip: current.clip,
            extent,
            extent_flipped,
        });
    }
    out
}

/// The unplaced devices of a layout space with their slots on the surface's rails.
fn unplaced_of(q: &Q, space: Id<LayoutSpace>, carriers: &[PanelCarrier]) -> Vec<UnplacedDevice> {
    let devices = q.devices();
    let parts = q.table::<LibCopy<Part>>();
    let placements = q.table::<Placement>();
    let mut out = Vec::new();
    for (d, use_part) in q.get::<Unplaced>(space).iter() {
        let d = *d;
        let Some(dev) = devices.get(d) else {
            continue;
        };
        let primary = primary_part(q, dev);
        let part = use_part.clone().or_else(|| primary.clone());
        let second = use_part
            .as_ref()
            .is_some_and(|u| primary.as_ref().is_none_or(|p| p.copy != u.copy));
        let mech = part
            .as_ref()
            .and_then(|p| parts.get(p.copy))
            .and_then(|c| c.item.mech.clone());
        let extent = rail_extent(q, dev, part.as_ref(), false);
        let extent_flipped = rail_extent(q, dev, part.as_ref(), true);
        let size = mech
            .as_ref()
            .and_then(|m| Some((i128::from(m.width?.0), i128::from(m.height?.0))));
        let mount_kind = mech.as_ref().and_then(|m| m.mount_kind.clone());
        let bracket_width = if extent.strip {
            ew_derive::strip_bracket(q, dev).map(|b| b.width)
        } else {
            None
        };
        let root = assembly_root(q, dev);
        let mut slots = Vec::new();
        if let Some(root) = root {
            let order = member_order(q, root);
            let at = order.iter().position(|x| *x == d).unwrap_or(0);
            let item = RailItem {
                s0: 0,
                s1: extent.width,
                strip: extent.strip,
                w: bracket_width,
                pad: extent.pad,
                locked: false,
                placement: None,
            };
            for c in carriers.iter().filter(|c| c.axis.is_some()) {
                let items_of = |member: Id<Device>| -> Vec<RailItem> {
                    c.items
                        .iter()
                        .filter(|i| {
                            i.placement
                                .and_then(|p| placements.get(p))
                                .is_some_and(|p| p.device == member)
                        })
                        .copied()
                        .collect()
                };
                // As `layout.place_assembly` inserts a member: after its nearest earlier member on the
                // rail, else before its first later one.
                let pred = order[..at]
                    .iter()
                    .rev()
                    .find_map(|m| items_of(*m).into_iter().max_by_key(|i| (i.s1, i.s0)));
                let succ = match pred {
                    Some(_) => None,
                    None => order
                        .get(at + 1..)
                        .unwrap_or_default()
                        .iter()
                        .find_map(|m| items_of(*m).into_iter().min_by_key(|i| (i.s0, i.s1))),
                };
                if pred.is_some() || succ.is_some() {
                    slots.push((c.id, member_slot(pred.as_ref(), succ.as_ref(), &item)));
                }
            }
        }
        out.push(UnplacedDevice {
            device: d.as_uuid(),
            part: use_part.as_ref().map(|p| p.copy.as_uuid()),
            second,
            label: device_tag(q, d),
            part_number: part
                .as_ref()
                .and_then(|p| ew_derive::printed_part(q, p))
                .map(|c| c.item.part_number.to_string()),
            size,
            mount_label: mount_words(mount_kind.as_ref()),
            mount_kind,
            strip: extent.strip,
            bracket_width,
            assembly: root.is_some(),
            extent,
            extent_flipped,
            slots,
        });
    }
    out
}

/// The panel tables of the panel-layout page `page`; `None` without a surface view or a surface.
pub(crate) fn panel_tables_of(q: &Q, folio: &str, page: Id<Page>) -> Option<PanelTables> {
    let view = q.get::<PageSurfaceView>(page).as_ref().clone()?;
    let surface = q.table::<MountingSurface>().get(view.surface).cloned()?;
    let carriers = carriers_of(q, surface.id);
    let placements = placements_of(q, page, surface.id);
    let space = q.table::<Enclosure>().get(surface.enclosure).map(|e| e.space);
    let location = space
        .and_then(|s| q.table::<LayoutSpace>().get(s).map(|s| s.location))
        .map(|l| location_text(q, l))
        .unwrap_or_default();
    let unplaced = space.map(|s| unplaced_of(q, s, &carriers)).unwrap_or_default();
    Some(PanelTables {
        folio: folio.to_owned(),
        page: page.as_uuid(),
        surface: surface.id.as_uuid(),
        surface_id: surface.id,
        plate: surface.kind == ew_schema::SurfaceKind::Plate,
        projection: view.projection,
        usable: surface.usable.iter().copied().map(xy).collect(),
        location,
        carriers,
        placements,
        unplaced,
        numbered: view.show != LabelShow::Tags,
        show: view.show,
        decimal_comma: comma(q),
        blocks: blocks_of(q, page),
    })
}

/// The report blocks embedded on `page`, by ID, with the boxes they draw.
fn blocks_of(q: &Q, page: Id<Page>) -> Vec<PanelBlock> {
    let sheet = q
        .get::<PageFrameOf>(PageRef::Drawn(page))
        .as_ref()
        .as_ref()
        .map_or(SheetMap::IDENTITY, |f| SheetMap::of(&f.form));
    let styles = text_style_table(q);
    let mut out: Vec<PanelBlock> = q
        .table::<ew_schema::ReportBlock>()
        .iter()
        .filter_map(|(id, b)| {
            let e = b.embedded.as_ref().filter(|e| e.page == page)?;
            let rect = e.rect.0;
            let table = super::view::boxed_table(q, b, page);
            let drawn = ew_scene::embedded::boxed_box(rect, &table, sheet, super::fonts(q), &styles);
            Some(PanelBlock {
                id: id.as_uuid(),
                rect,
                drawn,
            })
        })
        .collect();
    out.sort_by_key(|b| b.id);
    out
}

/// Where Add legend would put a panel page's legend ([`Session::legend_offer`](crate::Session::legend_offer)).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LegendOffer {
    /// The page is no panel-layout page, or it shows no surface yet.
    NoLayout,
    /// The page holds a legend already: this embedded `enclosure_legend` block (the first by ID).
    Exists(Uuid),
    /// The surface the page shows has its legend page (a legend list, doc 12 §5.4).
    Paged,
    /// No side of the layout leaves the room in which the legend prints every row and every text whole: Add legend
    /// makes its legend page.
    NoRoom,
    /// The rectangle beside the layout on the page (page nm) where the legend goes and prints whole.
    At(Rect),
}

/// The printed columns of a legend a person makes, beside the layout or as its page (doc 12 §5.4): `(key, weight)`,
/// the position, the tag, the quantity, the part number and the description.
pub const LEGEND_COLUMNS: [(&str, u32); 5] = [
    ("pos", 8),
    ("tag", 18),
    ("qty", 10),
    ("part_number", 24),
    ("description", 40),
];

/// `legend_offer(page)`: [`LegendOffer`] of a panel page, memoised, since the window asks it every frame for the menus
/// and the hint while the fit of the legend shapes its texts.
#[derive(Debug)]
pub struct LegendOfferOf;

impl Query for LegendOfferOf {
    type Key = Id<Page>;
    type Value = LegendOffer;
    const FAMILY: &'static str = "legend_offer";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Page>) -> Arc<LegendOffer> {
        Arc::new(legend_offer_of(q, *key))
    }
}

/// Whether the legend Add legend would make in `rect` on `page` prints every row and every text whole there: as its
/// box draws it, no row left out and no text cut (a description may wrap, an identifier shrink).
fn prints_whole(q: &Q, page: Id<Page>, rect: Rect) -> bool {
    let kind = ew_schema::ReportKind::EnclosureLegend;
    let Some(def) = ew_reports::Registry::builtin().get(&kind) else {
        return false;
    };
    let nil = Uuid::nil();
    let block = ew_schema::ReportBlock {
        id: Id::from_uuid(nil),
        v: 1,
        source: ew_schema::ReportSource::Kind(kind),
        manual: Default::default(),
        sort: Vec::new(),
        group: None,
        form: Id::from_uuid(nil),
        section: None,
        first_folio: None,
        continuation: Default::default(),
        title: Default::default(),
        empty: Default::default(),
        embedded: Some(ew_schema::EmbedSpec {
            page,
            rect: ew_schema::Bounds(rect),
            extra: Default::default(),
        }),
        source_ref: None,
        extra: Default::default(),
        columns: LEGEND_COLUMNS
            .iter()
            .map(|(key, weight)| ew_schema::ReportColumn {
                key: (*key).to_owned(),
                weight: *weight,
                heading: Default::default(),
                extra: Default::default(),
            })
            .collect(),
        caption: Default::default(),
        legend: Default::default(),
        added_rows: Vec::new(),
        layout_page: None,
    };
    let set = ew_reports::arranged(q, &block, def.rows(q, &block));
    let table = super::view::boxed_table_of(q, &block, &set, page);
    let sheet = q
        .get::<PageFrameOf>(PageRef::Drawn(page))
        .as_ref()
        .as_ref()
        .map_or(SheetMap::IDENTITY, |f| SheetMap::of(&f.form));
    let view = ew_scene::embedded::EmbeddedView {
        rect,
        content: ew_scene::embedded::EmbeddedContent::Table(table),
    };
    let lowered = ew_scene::embedded::lower_embedded(&view, sheet, super::fonts(q), &text_style_table(q));
    !lowered.findings.iter().any(|f| {
        matches!(
            f,
            ew_scene::table::TableFinding::TooManyRows { .. }
                | ew_scene::table::TableFinding::Overflow { .. }
        )
    })
}

/// The least room a legend takes beside the layout: 60 mm wide, 40 mm high (page nm).
const LEGEND_MIN: (i64, i64) = (60_000_000, 40_000_000);

/// The gap between the layout and its legend, and between the legend and the drawing area's edge: 10 mm and 5 mm.
const LEGEND_GAP: (i64, i64) = (10_000_000, 5_000_000);

/// [`LegendOffer`] of the panel page `page`: the layout's extent on the page is its enclosure's outline (else its
/// plate's, else its usable polygon's bounds) as the page projects it; the legend takes the largest free part of the
/// frame's drawing area right of it, left of it, below it or above it (in that order on a tie), [`LEGEND_GAP`] off
/// the layout and the area's edge, when that part is at least [`LEGEND_MIN`] and the legend prints whole there
/// ([`prints_whole`]). A surface with a legend page is offered no second legend.
fn legend_offer_of(q: &Q, page: Id<Page>) -> LegendOffer {
    let Some(view) = panel_view(
        q,
        page,
        Parts {
            surface: true,
            ..Parts::NONE
        },
    ) else {
        return LegendOffer::NoLayout;
    };
    let held = q.table::<ew_schema::ReportBlock>().iter().find_map(|(id, b)| {
        (b.embedded.as_ref().is_some_and(|e| e.page == page)
            && matches!(
                b.source,
                ew_schema::ReportSource::Kind(ew_schema::ReportKind::EnclosureLegend)
            ))
        .then_some(id)
    });
    if let Some(block) = held {
        return LegendOffer::Exists(block.as_uuid());
    }
    if view
        .surface
        .as_ref()
        .is_some_and(|s| !legend_blocks(q, s.surface).is_empty())
    {
        return LegendOffer::Paged;
    }
    let p = view.projection;
    let Some(layout) = view.surface.as_ref().and_then(|s| {
        s.enclosure
            .map(|(_, r)| r)
            .or(s.sheet)
            .or(s.plate)
            .or_else(|| bounds(&s.usable))
            .and_then(|r| p.rect(r))
    }) else {
        return LegendOffer::NoLayout;
    };
    // The drawing area is in page coordinates already (the frame's geometry before its page-to-sheet map).
    let Some((area, _)) = super::scale_fit::drawing_area(q, page) else {
        return LegendOffer::NoRoom;
    };
    let (gap, edge) = LEGEND_GAP;
    let (x0, y0, x1, y1) = (
        area.min().x.0 + edge,
        area.min().y.0 + edge,
        area.max().x.0 - edge,
        area.max().y.0 - edge,
    );
    let (lx0, ly0, lx1, ly1) = (
        layout.min().x.0 - gap,
        layout.min().y.0 - gap,
        layout.max().x.0 + gap,
        layout.max().y.0 + gap,
    );
    let sides = [
        (lx1, y0, x1, y1),
        (x0, y0, lx0, y1),
        (x0, ly1, x1, y1),
        (x0, y0, x1, ly0),
    ];
    let mut best: Option<(i128, Rect)> = None;
    for (a, b, c, d) in sides {
        let (w, h) = (c.saturating_sub(a), d.saturating_sub(b));
        if w < LEGEND_MIN.0 || h < LEGEND_MIN.1 {
            continue;
        }
        let size = i128::from(w) * i128::from(h);
        if best.is_none_or(|(s, _)| size > s) {
            best = Some((
                size,
                Rect::from_points(Pt2::new(Nm(a), Nm(b)), Pt2::new(Nm(c), Nm(d))),
            ));
        }
    }
    match best {
        Some((_, r)) if prints_whole(q, page, r) => LegendOffer::At(r),
        _ => LegendOffer::NoRoom,
    }
}

impl crate::Session {
    /// The slot texts of the drawn page labelled `folio` where its labels layer prints them, in print order: what a
    /// drag of a symbol's text grabs (doc 06 §2.5); empty for an unknown folio or a generated page.
    pub fn page_slot_texts(&self, folio: &str) -> Vec<ew_scene::page::SlotTextBox> {
        let q = self.query();
        let order = q.get::<ew_derive::PageOrder>(());
        let Some(entry) = order.by_folio(folio) else {
            return Vec::new();
        };
        super::layers::page_layer(&q, entry.page, ew_dl::Layer::Labels)
            .slot_texts
            .clone()
    }

    /// Where Add legend would put the legend of the panel page `page` ([`LegendOffer`]); `NoLayout` for a page the
    /// project does not hold.
    pub fn legend_offer(&self, page: Uuid) -> LegendOffer {
        *self.query().get::<LegendOfferOf>(Id::from_uuid(page))
    }

    /// The printed folio of the first page, in page order, of the legend list of the plate the panel page `page` shows
    /// (doc 12 §5.4: a legend belongs to the plate, whichever of its layout pages it was made from); `None` without one.
    pub fn legend_page_folio(&self, page: Uuid) -> Option<String> {
        let q = self.query();
        let blocks = legend_blocks(&q, ew_derive::layout::page_surface(&q, Id::from_uuid(page))?);
        let order = q.get::<ew_derive::PageOrder>(());
        order
            .entries
            .iter()
            .find(|e| matches!(e.page, PageRef::Generated { block, .. } if blocks.contains(&block)))
            .map(|e| e.folio.clone())
    }

    /// `panel_tables(folio)`: the panel data of the panel-layout page labelled `folio` for the layout
    /// tools, keyed by folio like [`Session::page_tables`](crate::Session::page_tables); `None` for an unknown folio,
    /// a page that is not a panel-layout page or one without a surface view. A plain function over `Q`.
    pub fn panel_tables(&self, folio: &str) -> Option<PanelTables> {
        let q = self.query();
        let order = q.get::<ew_derive::PageOrder>(());
        let entry = order.by_folio(folio)?;
        let ew_derive::PageRef::Drawn(page) = entry.page else {
            return None;
        };
        panel_tables_of(&q, folio, page)
    }
}
