// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The status-bar model (WP-S2.01 part 1, spec section 5.3; WP-S2.02 part 1, spec section 6.5; UX/UI doc 16
//! section 3.6).
//!
//! Left to right: the hint, the page as `<folio> (<n> of <total>)`, the cursor `x 255.0 y 172.5 mm` (sheet
//! coordinates, left out while the pointer is outside the canvas), the zoom `156 %`, the grid (`QET 10 px`, a pitch
//! such as `4 mm`, or `Grid off`), the save state (`*` unsaved, `saved`, or `read-only: <reason>`), the canvas
//! backend (`CPU`, `WARP` or `GPU <backend>`), the UI language, and while an export runs the export label. Without
//! a project the page and save fields are left out; without a canvas the cursor, zoom and grid.

use crate::canvas::{GridLabel, grid_label, mm1, zoom_percent_text};
use crate::shell::{AccessMode, ShellModel};
use crate::strings;

/// The grid field of `model`'s canvas: the label, or "Grid off" when it is off or the page has none.
pub fn grid_text(model: &ShellModel) -> String {
    let grid = model
        .canvas()
        .and_then(|c| c.page().grid)
        .filter(|_| model.grid_on());
    match grid.map(|g| grid_label(&g)) {
        None => strings::text("status-grid-off").to_owned(),
        Some(GridLabel::Qet10) => strings::text("status-grid-qet").to_owned(),
        Some(GridLabel::Mm(n)) => strings::format("status-grid-mm", &[&n]),
    }
}

/// The status-bar fields of `model`; `exporting` adds the label of a running export.
pub fn fields(model: &ShellModel, exporting: bool) -> Vec<String> {
    let mut out = Vec::new();
    match model.project_name() {
        None => out.push(strings::text("status-hint-start").to_owned()),
        Some(_) => {
            out.push(strings::text("status-hint-view").to_owned());
            if let Some(folio) = model.current_folio() {
                out.push(strings::format(
                    "status-page",
                    &[
                        folio,
                        &(model.page_index() + 1).to_string(),
                        &model.page_count().to_string(),
                    ],
                ));
            }
            if let Some(canvas) = model.canvas() {
                if let Some((x, y)) = canvas.pointer() {
                    out.push(strings::format("status-cursor", &[&mm1(x), &mm1(y)]));
                }
                out.push(strings::format(
                    "status-zoom",
                    &[&zoom_percent_text(canvas.zoom())],
                ));
                out.push(grid_text(model));
            }
            out.push(match model.access() {
                Some(AccessMode::ReadOnly { reason_key }) => {
                    strings::format("status-read-only", &[strings::text(reason_key)])
                }
                _ if model.is_dirty() => strings::text("status-unsaved").to_owned(),
                _ => strings::text("status-saved").to_owned(),
            });
        }
    }
    out.push(model.canvas_backend().label());
    out.push(strings::text("status-language").to_owned());
    if exporting {
        out.push(strings::text("status-exporting").to_owned());
    }
    out
}
