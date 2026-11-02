// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fitting scale of a panel-layout page and a mounting surface (FT-1016, ID-1148): the first 1:n of
//! [`FIT_SCALES`] at which the bounding box of the surface's usable outline, drawn at 1:n on paper, fits the drawing
//! area of the page's frame; 1:100 when none fits; none when the surface records no usable outline or the page has
//! no frame. The list is doc 12's (FR-LAY-010: 1:2 to 1:20) with 1:1 and ISO 5455's 1:50 and 1:100.
//!
//! The window sends it with a surface view set on a page still drawn not to scale (`page.surface_view.set`'s
//! `scale`); a scale already chosen is kept.

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::{PageRef, Q};
use ew_scene::forms::{FrameGeometry, SheetMap};
use ew_schema::{Extra, FormDef, FormKind, FrameGrid, ItemMeta, MountingSurface, Page, SheetSpec};

use super::view::PageFrameOf;
use crate::Session;

/// The scales a panel page is fitted at, 1:n by n, smallest reduction first (ID-1148).
pub const FIT_SCALES: [u32; 7] = [1, 2, 5, 10, 20, 50, 100];

/// The first n of [`FIT_SCALES`] at which the bounding box of `usable` (surface nm) fits a drawing area of `area`
/// (width, height on the sheet, nm) at 1:n on paper; the last, 1:100, when none fits; `None` for an empty outline.
pub fn fitting_scale(area: (i64, i64), usable: &[Pt2]) -> Option<u32> {
    let xs = usable.iter().map(|p| i128::from(p.x.0));
    let ys = usable.iter().map(|p| i128::from(p.y.0));
    let w = xs.clone().max()? - xs.min()?;
    let h = ys.clone().max()? - ys.min()?;
    let (aw, ah) = (i128::from(area.0), i128::from(area.1));
    let last = FIT_SCALES[FIT_SCALES.len() - 1];
    Some(
        FIT_SCALES
            .into_iter()
            .find(|n| w <= aw * i128::from(*n) && h <= ah * i128::from(*n))
            .unwrap_or(last),
    )
}

/// The drawing area of `page` on the sheet and the map of its frame from the page to the sheet: the frame form's
/// grid; for a page whose frame form has no grid, or which has no frame form (a new project's pages), the frame grid
/// its cells read (the built-in default grid, `ew_derive::frame_grid`), as the canvas does. `None` without a grid.
pub(crate) fn drawing_area(q: &Q, page: Id<Page>) -> Option<(Rect, SheetMap)> {
    let frame = q.get::<PageFrameOf>(PageRef::Drawn(page)).as_ref().clone();
    let map = frame
        .as_ref()
        .map_or(SheetMap::IDENTITY, |f| SheetMap::of(&f.form));
    let area = match frame
        .as_ref()
        .and_then(|f| FrameGeometry::of(&f.form, f.title_block.as_deref()))
    {
        Some(g) => g.drawing_area,
        None => FrameGeometry::of(&grid_form(ew_derive::frame_grid(q, page)?), None)?.drawing_area,
    };
    Some((area, map))
}

/// A frame form that holds only `grid`, to read its geometry.
fn grid_form(grid: FrameGrid) -> FormDef {
    FormDef {
        kind: FormKind::Frame,
        sheet: SheetSpec {
            width: Nm::ZERO,
            height: Nm::ZERO,
            extra: Extra::new(),
        },
        page_to_sheet: None,
        grid: Some(grid),
        title_block: None,
        graphics: Vec::new(),
        cells: Vec::new(),
        data: None,
        continuation: None,
        meta: ItemMeta::default(),
        extra: Extra::new(),
    }
}

/// [`fitting_scale`] of `surface` on `page`'s drawing area; `None` for an unknown page or surface, a page without
/// a frame grid or a surface without a usable outline.
pub(crate) fn page_fitting_scale(q: &Q, page: Id<Page>, surface: Id<MountingSurface>) -> Option<u32> {
    let s = q.table::<MountingSurface>().get(surface)?;
    let (area, _) = drawing_area(q, page)?;
    fitting_scale((area.width().0, area.height().0), &s.usable)
}

impl Session {
    /// The fitting scale's n (1:n) of the mounting surface `surface` on the panel page `page` (FT-1016, ID-1148):
    /// see [`fitting_scale`]. `None` for an unknown page or surface, a page without a frame grid or a surface that
    /// records no usable outline.
    pub fn fitting_scale(&self, page: Uuid, surface: Uuid) -> Option<u32> {
        page_fitting_scale(&self.query(), Id::from_uuid(page), Id::from_uuid(surface))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MM: i64 = 1_000_000;

    fn rect(w: i64, h: i64) -> Vec<Pt2> {
        [(0, 0), (w, 0), (w, h), (0, h)]
            .map(|(x, y)| Pt2::new(Nm(x * MM), Nm(y * MM)))
            .to_vec()
    }

    #[test]
    fn the_first_scale_of_the_list_at_which_the_outline_fits() {
        let area = (380 * MM, 250 * MM);
        assert_eq!(fitting_scale(area, &rect(380, 250)), Some(1), "exactly the area");
        assert_eq!(fitting_scale(area, &rect(381, 10)), Some(2));
        assert_eq!(
            fitting_scale(area, &rect(10, 1250)),
            Some(5),
            "the height decides"
        );
        assert_eq!(fitting_scale(area, &rect(650, 1251)), Some(10));
        assert_eq!(fitting_scale(area, &rect(7600, 100)), Some(20));
        assert_eq!(fitting_scale(area, &rect(19_000, 100)), Some(50));
        assert_eq!(fitting_scale(area, &rect(38_000, 100)), Some(100));
        assert_eq!(
            fitting_scale(area, &rect(38_001, 100)),
            Some(100),
            "none fits: 1:100"
        );
    }

    #[test]
    fn an_outline_off_the_origin_counts_by_its_bounding_box() {
        let area = (100 * MM, 100 * MM);
        let off: Vec<Pt2> = rect(150, 90)
            .into_iter()
            .map(|p| Pt2::new(Nm(p.x.0 - 500 * MM), Nm(p.y.0 + 900 * MM)))
            .collect();
        assert_eq!(fitting_scale(area, &off), Some(2));
    }

    #[test]
    fn no_outline_has_no_fitting_scale() {
        assert_eq!(fitting_scale((100 * MM, 100 * MM), &[]), None);
    }
}
