// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The surface view of a panel-layout page as the scene reads it (WP-S4.01 part 3, D9 and D10;
//! [`ew_scene::panel`]): the parts of a [`PanelView`] each panel layer reads, filled from the derived layout queries
//! of `ew_derive::layout` (`placement_geometry`, `carrier_geometry`, `layout_label`), never re-derived here.
//!
//! | Part | Layer | Reads |
//! |---|---|---|
//! | [`PageSurfaceView`] | every panel layer | the page row (kind, scale) and its page items |
//! | the surface outlines | `surface` | the surface, its enclosure, the enclosure's and the plate's devices and their part copies (the enclosure and mounting-plate sections) |
//! | the carriers | `carriers` | `carriers_of_surface`, `carrier_geometry`, the carriers' `params` |
//! | the placements | `placements` | the placements on the surface's carriers and on the surface, `placement_geometry` |
//! | the labels | `labels` | the same placements, `layout_label`, [`ShapedText`] in the label style, the surface's usable polygon |
//!
//! So a `layout.move` rebuilds the `placements` and `labels` families of its page only, never `surface`,
//! `carriers`, `frame` or `tables` (LP-5).

use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::layout::{CarrierGeometry, LabelMode, LayoutLabel, PlacementGeometry};
use ew_derive::{Policy, Q, Query};
use ew_scene::panel::{
    CarrierDraw, LAYOUT_LABEL_STYLE, PanelLabel, PanelView, PlacementDraw, Projection, SurfaceOutline,
};
use ew_schema::{
    CarrierKind, CarrierParams, CarrierRef, Device, Enclosure, LibCopy, MountingSurface, Page, PageItem,
    PageItemKind, PageType, Part, Placement, SurfaceLabelMode,
};
use ew_text::{Shaped, TextStyle};

use super::layers::text_style_table;

/// `shaped_text((text, style))`: a text shaped in a style by the bundled fonts (ADR-0015's memoised `shaped_text`; the
/// fit test of a panel page's labels reads it, D10). A pure function of its key and the process's font set, so it
/// never re-runs for a text it has shaped.
#[derive(Debug)]
pub struct ShapedText;

impl Query for ShapedText {
    type Key = (String, TextStyle);
    type Value = Shaped;
    const FAMILY: &'static str = "shaped_text";
    const POLICY: Policy = Policy::Memo;

    fn run(_q: &Q, key: &Self::Key) -> Arc<Shaped> {
        Arc::new(ew_text::bundled().shape(&key.0, &key.1, None))
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
    /// The page projection at the page's scale.
    pub projection: Projection,
}

/// `page_surface_view(page)`: the surface view of a panel-layout page (D9): its first `surface_view` item by ID (a page
/// holds at most one; a file with more draws the first), projected at the page's scale. `None` for a missing page, a
/// page of another kind or a page without a view.
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
                projection: Projection::of(v.datum_at, &page.scale),
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
}

impl Parts {
    pub(crate) const ALL: Parts = Parts {
        surface: true,
        carriers: true,
        placements: true,
        labels: true,
    };
    pub(crate) const NONE: Parts = Parts {
        surface: false,
        carriers: false,
        placements: false,
        labels: false,
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

/// The surface's outlines: the enclosure's outer rectangle (its part's enclosure section, placed by the plate frame),
/// the plate (its device's mounting-plate section, else the usable polygon's bounds) and the usable polygon.
fn surface_outline(q: &Q, s: &MountingSurface) -> SurfaceOutline {
    let enclosure = q.table::<Enclosure>().get(s.enclosure).cloned().and_then(|e| {
        let (w, h) = part_section(q, e.device, |p| {
            let sec = p.sections.enclosure.as_ref()?;
            Some((sec.width?, sec.height?))
        })?;
        // MVP frames carry no turn (D5): the enclosure's corner lies at minus the frame's offset.
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
    }
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
fn placements_on(q: &Q, surface: Id<MountingSurface>) -> Vec<Id<Placement>> {
    let mut ids: Vec<Id<Placement>> = q
        .idx()
        .carriers_of_surface(surface)
        .flat_map(|c| q.idx().placements_by_carrier(c).collect::<Vec<_>>())
        .collect();
    ids.extend(
        q.table::<Placement>()
            .iter()
            .filter(|(_, p)| p.carrier == CarrierRef::Surface(surface))
            .map(|(id, _)| id),
    );
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
            })
        })
        .collect()
}

/// The label style of the project's text-style table ([`LAYOUT_LABEL_STYLE`]).
fn label_style(q: &Q) -> TextStyle {
    let table = text_style_table(q);
    table
        .resolve(LAYOUT_LABEL_STYLE)
        .or_else(|| table.resolve(ew_text::DEFAULT_STYLE_KEY))
        .unwrap_or(TextStyle::regular(Nm(3_175_000)))
}

/// The labels of a surface's placements on `page` (`layout_label`), each shaped by [`ShapedText`].
fn labels(q: &Q, page: Id<Page>, view: &SurfaceViewOf, ids: &[Id<Placement>]) -> Vec<PanelLabel> {
    let style = label_style(q);
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
        labels: if parts.labels {
            labels(q, page, &view, &ids)
        } else {
            Vec::new()
        },
    })
}
