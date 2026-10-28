// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Named splitter handles for egui_tiles trees (FT-338; WP-S2.17 part 3 §7 item 8).
//!
//! egui_tiles 0.18 publishes its resize handles without a role or a name. The dock and the canvas area turn its own
//! handles off (`Behavior::is_container_resizable` is false, so it only paints the idle line) and call [`show`] after
//! `Tree::ui`: one `Splitter` node per pair of neighbours of every shown linear container, named "Resize <a> and <b>"
//! by the panes on either side, which a drag resizes as egui_tiles' handle did (the two neighbours trade share, each
//! kept at least egui_tiles' minimum size) and a double click centres.

use egui::{CursorIcon, Id, Rect, Sense, Ui, pos2};
use egui_tiles::{Container, LinearDir, Tile, TileId, Tiles, Tree};

use crate::theme::{self, Tokens};

/// The narrowest a tile becomes by a drag (egui_tiles' `Behavior::min_size` default).
const MIN_SIZE: f32 = 32.0;

/// One handle: the container, the two neighbours, the direction and the handle's line.
struct Handle {
    container: TileId,
    a: TileId,
    b: TileId,
    dir: LinearDir,
    at: f32,
    span: (f32, f32),
}

/// The handles of the shown linear containers of `tiles`, in tile order.
fn handles<P>(tiles: &Tiles<P>) -> Vec<Handle> {
    let mut ids: Vec<TileId> = tiles.tile_ids().collect();
    ids.sort_by_key(|t| t.0);
    let mut out = Vec::new();
    for id in ids {
        if !tiles.is_visible(id) {
            continue;
        }
        let (Some(Tile::Container(Container::Linear(linear))), Some(rect)) = (tiles.get(id), tiles.rect(id))
        else {
            continue;
        };
        let shown: Vec<TileId> = linear
            .children
            .iter()
            .copied()
            .filter(|c| tiles.is_visible(*c) && tiles.rect(*c).is_some())
            .collect();
        for pair in shown.windows(2) {
            let (Some(ra), Some(rb)) = (tiles.rect(pair[0]), tiles.rect(pair[1])) else {
                continue;
            };
            let (at, span) = match linear.dir {
                LinearDir::Horizontal => ((ra.right() + rb.left()) / 2.0, (rect.top(), rect.bottom())),
                LinearDir::Vertical => ((ra.bottom() + rb.top()) / 2.0, (rect.left(), rect.right())),
            };
            out.push(Handle {
                container: id,
                a: pair[0],
                b: pair[1],
                dir: linear.dir,
                at,
                span,
            });
        }
    }
    out
}

/// The number of handles [`show`] draws for `tree` (tests).
pub(crate) fn count<P>(tree: &Tree<P>) -> usize {
    handles(&tree.tiles).len()
}

/// Draws the named handles of `tree` into `ui` after `Tree::ui`; `title` names a tile by the pane it shows. `salt`
/// keeps the ids of two trees apart.
pub(crate) fn show<P>(
    ui: &Ui,
    tree: &mut Tree<P>,
    t: &Tokens,
    salt: &str,
    title: &dyn Fn(&Tiles<P>, TileId) -> String,
) {
    let grab = ui.style().interaction.resize_grab_radius_side;
    for (i, h) in handles(&tree.tiles).into_iter().enumerate() {
        let rect = match h.dir {
            LinearDir::Horizontal => {
                Rect::from_min_max(pos2(h.at - grab, h.span.0), pos2(h.at + grab, h.span.1))
            }
            LinearDir::Vertical => {
                Rect::from_min_max(pos2(h.span.0, h.at - grab), pos2(h.span.1, h.at + grab))
            }
        };
        let id = Id::unique(("ew-splitter", salt, h.container, i));
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let name = ew_viewmodel::strings::format(
            "splitter-name",
            &[&title(&tree.tiles, h.a), &title(&tree.tiles, h.b)],
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Splitter, true, &name));
        let cursor = match h.dir {
            LinearDir::Horizontal => CursorIcon::ResizeHorizontal,
            LinearDir::Vertical => CursorIcon::ResizeVertical,
        };
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(cursor);
            let stroke = theme::stroke(t.border_width, t.accent);
            match h.dir {
                LinearDir::Horizontal => ui.painter().vline(h.at, h.span.0..=h.span.1, stroke),
                LinearDir::Vertical => ui.painter().hline(h.span.0..=h.span.1, h.at, stroke),
            };
        }
        let (Some(ra), Some(rb)) = (tree.tiles.rect(h.a), tree.tiles.rect(h.b)) else {
            continue;
        };
        let size = |r: Rect| match h.dir {
            LinearDir::Horizontal => r.width(),
            LinearDir::Vertical => r.height(),
        };
        let delta = if response.double_clicked() {
            Some((size(rb) - size(ra)) / 2.0)
        } else if response.dragged() {
            let d = response.drag_delta();
            Some(match h.dir {
                LinearDir::Horizontal => d.x,
                LinearDir::Vertical => d.y,
            })
        } else {
            None
        };
        let Some(delta) = delta else { continue };
        let (wa, wb) = (size(ra), size(rb));
        let delta = delta.clamp(MIN_SIZE - wa, wb - MIN_SIZE);
        if delta == 0.0 || wa + wb <= 0.0 {
            continue;
        }
        if let Some(Tile::Container(Container::Linear(linear))) = tree.tiles.get_mut(h.container) {
            let (sa, sb) = (linear.shares[h.a], linear.shares[h.b]);
            let per_point = (sa + sb) / (wa + wb);
            linear.shares.set_share(h.a, sa + delta * per_point);
            linear.shares.set_share(h.b, sb - delta * per_point);
            ui.ctx().request_repaint();
        }
    }
}
