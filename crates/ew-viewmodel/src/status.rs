// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The status-bar model (WP-S2.01 part 1, spec section 5.3; WP-S2.02 part 1, spec section 6.5; UX/UI doc 16
//! section 3.6).
//!
//! Left to right: the hint (the active tool's status hint, or the hovered link's target while the tool shows the
//! hover; WP-S2.03 §7.2), the page as `<folio> (<n> of <total>)`,
//! the cursor `102-H15 · x 255.0 y 172.5 mm` (the cell where one exists, then sheet coordinates: the pointer over
//! the focused canvas, else the grid cursor, else left out; WP-S2.03 §6.5, FT-345), the zoom `156 %`, the grid (`QET 10 px`, a pitch
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

/// The accessible texts of a canvas (WP-S2.03 §6.9, doc 16 §10.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CanvasNodeText {
    /// The node's name: `Page <folio>, <title>`.
    pub name: String,
    /// Its description: `<hint>, zoom <zoom>`, then `, cursor <cell> x <x> y <y> mm` while the grid cursor shows.
    pub description: String,
    /// The polite live region's text: `<cell>, x <x> y <y> mm`, then `, link to <label>` when a link is at the
    /// cursor; empty while no cursor shows (the region stays present, so a screen reader keeps it).
    pub live: String,
}

/// The accessible texts of group `group`'s canvas; `None` without a canvas there.
pub fn canvas_node(model: &ShellModel, group: usize) -> Option<CanvasNodeText> {
    let c = model.canvas_of(group)?;
    let page = c.page();
    let mut description = strings::format(
        "canvas-description",
        &[
            &hint_of(model, c),
            &strings::format("status-zoom", &[&zoom_percent_text(c.zoom())]),
        ],
    );
    let live = c
        .cursor()
        .map(|(x, y)| {
            let cell = page.cell_text((x, y));
            description.push_str(&match &cell {
                Some(cell) => strings::format("canvas-cursor-description", &[cell, &mm1(x), &mm1(y)]),
                None => strings::format("canvas-cursor-description-no-cell", &[&mm1(x), &mm1(y)]),
            });
            let mut live = match &cell {
                Some(cell) => strings::format("canvas-cursor-live", &[cell, &mm1(x), &mm1(y)]),
                None => strings::format("status-cursor", &[&mm1(x), &mm1(y)]),
            };
            if let Some(label) = c.cursor_link().and_then(crate::canvas::PageLink::label) {
                live.push_str(&strings::format("canvas-cursor-live-link", &[&label]));
            }
            if let Some(cp) = c.cursor_cp() {
                live.push_str(&strings::format("canvas-cursor-live-cp", &[&cp.label]));
            }
            live
        })
        .unwrap_or_default();
    // A tool's state change or a selection change is said until the cursor moves again (WP-S2.03 §7.2, §7.6); it is
    // the focused group's.
    let focused = model.area().map(|a| a.focused) == Some(group);
    let live = match model.tool_announcement() {
        Some(text) if focused => text.to_owned(),
        _ => live,
    };
    Some(CanvasNodeText {
        name: strings::format("canvas-name", &[&page.folio, &page.title]),
        description,
        live,
    })
}

/// The hint of one canvas: the hovered link's target while the active tool shows the hover, else the tool's status
/// hint (WP-S2.03 §6.4, §7.2).
fn hint_of(model: &ShellModel, c: &crate::canvas::CanvasModel) -> String {
    match c.hovered_link().filter(|_| model.tool().wants_hover()) {
        Some(link) => match link.label() {
            Some(label) => strings::format("status-hint-link", &[&label]),
            None => strings::text("status-hint-link-external").to_owned(),
        },
        None => model.tool_hint().to_owned(),
    }
}

/// The hint field: the active tool's status hint (`Click to select; drag for a rubber band`), or the hovered link's
/// `Click or F: go to <label>` (WP-S2.03 §6.4, §7.2).
pub fn hint(model: &ShellModel) -> String {
    match model.canvas() {
        Some(c) => hint_of(model, c),
        None => model.tool_hint().to_owned(),
    }
}

/// The status-bar fields of `model`; `exporting` adds the label of a running export.
pub fn fields(model: &ShellModel, exporting: bool) -> Vec<String> {
    let mut out = Vec::new();
    match model.project_name() {
        None => out.push(strings::text("status-hint-start").to_owned()),
        Some(_) => {
            out.push(hint(model));
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
                if let Some((x, y)) = model.shown_point() {
                    out.push(match canvas.page().cell_text((x, y)) {
                        Some(cell) => strings::format("status-cursor-cell", &[&cell, &mm1(x), &mm1(y)]),
                        None => strings::format("status-cursor", &[&mm1(x), &mm1(y)]),
                    });
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
