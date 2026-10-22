// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canvas area of the project view (WP-S2.03 part 1, spec section 6.2; decision log ID-533): the workspace's
//! `page-view` pane holds a nested `egui_tiles` tree built from the shell's toolkit-neutral `CanvasArea`, one `Tabs`
//! container per tab group in a horizontal `Linear` container with equal shares.
//!
//! **The model is the one source of truth.** egui_tiles 0.18.0 cannot refuse a drop, so the tree is rebuilt from the
//! model whenever the model's version changes and after any structural change the tree reports (ruling 4): a tab
//! dragged within or between the groups becomes `tab.move`, anything else (a third group, a vertical split) is
//! discarded by the rebuild. A tab click is `tab.activate`, a close button `tab.activate` then `tab.close`. A splitter
//! drag between the groups stays in the tree (its share is host state) and reaches the model only as each group's
//! `view.canvas_resize`. The registry owns Ctrl+Tab: `cycle_tabs_with_keyboard` is false.
//!
//! **Accessibility.** Each tab is a node with the role `Tab`, its title as the name and the `selected` state on the
//! active one; its close button is named `Close <folio>`. egui_tiles 0.18.0 exposes no node for the tab bar, so the
//! `TabList` role is a follow-up joined with FT-338 (the splitter names); the splitter between the groups has no
//! accessible name and is counted by [`CanvasAreaHost::splitter_count`].

use egui::{Id, Response, Sense, TextStyle, Ui, Vec2, WidgetText, vec2};
use egui_tiles::{
    Behavior, Container, Linear, LinearDir, SimplificationOptions, TabState, Tabs, Tile, TileId, Tiles, Tree,
    UiResponse,
};
use ew_viewmodel::area::CanvasArea;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::canvas::CanvasHost;

/// A pane of the tree: a tab by its ID.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TabPane(pub u32);

/// The nested tree of the canvas area and what it was built from.
pub struct CanvasAreaHost {
    tree: Option<Tree<TabPane>>,
    /// The project session and the area's version the tree was built from.
    built: Option<(u64, u64)>,
}

impl Default for CanvasAreaHost {
    fn default() -> CanvasAreaHost {
        CanvasAreaHost::new()
    }
}

/// The tab IDs per group of the model.
fn model_groups(area: &CanvasArea) -> Vec<Vec<u32>> {
    area.groups
        .iter()
        .map(|g| g.tabs.iter().map(|t| t.id).collect())
        .collect()
}

/// The tab IDs per `Tabs` container of the tree, in depth-first order (the groups as the tree shows them).
fn tree_groups(tree: &Tree<TabPane>) -> Vec<Vec<u32>> {
    fn walk(tiles: &Tiles<TabPane>, id: TileId, out: &mut Vec<Vec<u32>>) {
        match tiles.get(id) {
            Some(Tile::Pane(p)) => out.push(vec![p.0]),
            Some(Tile::Container(Container::Tabs(t))) => {
                let mut panes = Vec::new();
                for c in &t.children {
                    match tiles.get(*c) {
                        Some(Tile::Pane(p)) => panes.push(p.0),
                        Some(Tile::Container(_)) => walk(tiles, *c, out),
                        None => {}
                    }
                }
                if !panes.is_empty() {
                    out.push(panes);
                }
            }
            Some(Tile::Container(c)) => {
                for child in c.children() {
                    walk(tiles, *child, out);
                }
            }
            None => {}
        }
    }
    let mut out = Vec::new();
    if let Some(root) = tree.root() {
        walk(&tree.tiles, root, &mut out);
    }
    out
}

/// The one tab whose move turns `from` into `to`, with its position before and after: `(group, index, to_group,
/// to_index)`; `None` when the change is not a single move between at most two groups.
pub fn single_move(from: &[Vec<u32>], to: &[Vec<u32>]) -> Option<(usize, usize, usize, usize)> {
    if to.len() > 2 || from == to {
        return None;
    }
    let pos = |groups: &[Vec<u32>], id: u32| {
        groups
            .iter()
            .enumerate()
            .find_map(|(g, tabs)| tabs.iter().position(|t| *t == id).map(|i| (g, i)))
    };
    let without = |groups: &[Vec<u32>], id: u32| -> Vec<Vec<u32>> {
        groups
            .iter()
            .map(|g| g.iter().copied().filter(|t| *t != id).collect::<Vec<_>>())
            .filter(|g| !g.is_empty())
            .collect()
    };
    for tabs in from {
        for &id in tabs {
            if without(from, id) == without(to, id) {
                let (g, i) = pos(from, id)?;
                let (tg, ti) = pos(to, id)?;
                if (g, i) == (tg, ti) && from.len() == to.len() {
                    continue;
                }
                // A tab dropped into a group the model does not have is no move: the rebuild discards it.
                if tg >= from.len() {
                    return None;
                }
                return Some((g, i, tg, ti));
            }
        }
    }
    None
}

impl CanvasAreaHost {
    /// A host without a tree.
    pub fn new() -> CanvasAreaHost {
        CanvasAreaHost {
            tree: None,
            built: None,
        }
    }

    /// The number of splitter handles the tree shows: one between two groups.
    pub fn splitter_count(&self, shell: &ShellModel) -> usize {
        shell.area().map_or(0, |a| a.groups.len().saturating_sub(1))
    }

    /// The tree of `area`; `shares` (one per group, left to right) carry the splitter's position across a rebuild
    /// when the number of groups is unchanged (§6.2: the share is host state), else the groups share equally.
    fn build(area: &CanvasArea, session: u64, shares: Option<&[f32]>) -> Tree<TabPane> {
        let mut tiles = Tiles::default();
        let mut groups = Vec::new();
        for g in &area.groups {
            let ids: Vec<TileId> = g.tabs.iter().map(|t| tiles.insert_pane(TabPane(t.id))).collect();
            let active = ids.get(g.active).copied();
            let mut tabs = Tabs::new(ids);
            tabs.active = active;
            groups.push(tiles.insert_container(tabs));
        }
        let mut linear = Linear::new(LinearDir::Horizontal, groups.clone());
        if let Some(shares) = shares.filter(|s| s.len() == groups.len()) {
            for (id, share) in groups.iter().zip(shares) {
                linear.shares.set_share(*id, *share);
            }
        }
        let root = tiles.insert_container(linear);
        Tree::new(Id::unique(("canvas-area", session)), root, tiles)
    }

    /// The shares of the groups of the shown tree, left to right (the splitter's position); `None` with one group.
    pub fn shares(&self) -> Option<Vec<f32>> {
        let tree = self.tree.as_ref()?;
        match tree.tiles.get(tree.root()?)? {
            Tile::Container(Container::Linear(l)) if l.children.len() > 1 => {
                Some(l.children.iter().map(|c| l.shares[*c]).collect())
            }
            _ => None,
        }
    }

    /// Paints the canvas area of `shell`: the tree, each group's active canvas through `canvas`; the user's input
    /// goes to `events`.
    pub(crate) fn show(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        canvas: &mut CanvasHost,
        events: &mut Vec<Event>,
    ) {
        let Some(area) = shell.area() else {
            self.tree = None;
            self.built = None;
            canvas.show_group(ui, shell, 0, events);
            return;
        };
        let key = (shell.project_epoch().unwrap_or(0), area.version());
        if self.built != Some(key) || self.tree.is_none() {
            let shares = self.shares();
            self.tree = Some(Self::build(area, key.0, shares.as_deref()));
            self.built = Some(key);
        }
        let Some(tree) = &mut self.tree else { return };
        let mut behavior = AreaBehavior {
            shell,
            area,
            canvas,
            events,
        };
        tree.ui(&mut behavior, ui);
        // A structural change of the tree: a single move becomes `tab.move`; the tree is rebuilt from the model.
        let shown = tree_groups(tree);
        let model = model_groups(area);
        if shown != model {
            if let Some((g, i, tg, ti)) = single_move(&model, &shown) {
                events.push(Event::Action(
                    "tab.move",
                    json!({"group": g, "index": i, "to_group": tg, "to_index": ti}),
                ));
            }
            self.built = None;
        }
    }
}

struct AreaBehavior<'a> {
    shell: &'a ShellModel,
    area: &'a CanvasArea,
    canvas: &'a mut CanvasHost,
    events: &'a mut Vec<Event>,
}

impl AreaBehavior<'_> {
    /// The group and index of a tab.
    fn place(&self, id: u32) -> Option<(usize, usize)> {
        self.area
            .tabs()
            .find(|(_, _, t)| t.id == id)
            .map(|(g, i, _)| (g, i))
    }

    /// A tab's title (`<folio> <title>` with its badges) and its folio.
    fn title(&self, id: u32) -> (String, String) {
        let Some(tab) = self.area.tabs().find(|(_, _, t)| t.id == id).map(|(_, _, t)| t) else {
            return (String::new(), String::new());
        };
        let Some(row) = self.shell.tab_row(tab) else {
            return (String::new(), String::new());
        };
        let mut title = self.shell.tab_title(tab);
        if row.qet_grid {
            title.push_str(&format!(" [{}]", strings::text("badge-qet-grid")));
        }
        if row.generated {
            title.push_str(&format!(" [{}]", strings::text("badge-generated")));
        }
        (title, row.folio.clone())
    }
}

impl Behavior<TabPane> for AreaBehavior<'_> {
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: TileId, pane: &mut TabPane) -> UiResponse {
        if let Some((g, i)) = self.place(pane.0)
            && self.area.groups[g].active == i
        {
            self.canvas.show_group(ui, self.shell, g, self.events);
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &TabPane) -> WidgetText {
        self.title(pane.0).0.into()
    }

    fn is_tab_closable(&self, _tiles: &Tiles<TabPane>, _tile_id: TileId) -> bool {
        true
    }

    fn on_tab_close(&mut self, _tiles: &mut Tiles<TabPane>, _tile_id: TileId) -> bool {
        false
    }

    fn cycle_tabs_with_keyboard(&self) -> bool {
        false
    }

    /// Tabs are dragged, groups are not: a group's tab bar gets no unnamed drag area of its own.
    fn is_tile_draggable(&self, tiles: &Tiles<TabPane>, tile_id: TileId) -> bool {
        matches!(tiles.get(tile_id), Some(Tile::Pane(_)))
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..SimplificationOptions::default()
        }
    }

    fn tab_ui(
        &mut self,
        tiles: &mut Tiles<TabPane>,
        ui: &mut Ui,
        id: Id,
        tile_id: TileId,
        state: &TabState,
    ) -> Response {
        let pane = match tiles.get(tile_id) {
            Some(Tile::Pane(p)) => Some(*p),
            _ => None,
        };
        let (title, folio) = pane.map_or((String::new(), String::new()), |p| self.title(p.0));
        let font = TextStyle::Button.resolve(ui.style());
        let galley = WidgetText::from(title.clone()).into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            font,
        );
        let margin = 8.0;
        let close = Vec2::splat(self.close_button_outer_size());
        let width = galley.size().x + 2.0 * margin + 4.0 + close.x;
        let (_, rect) = ui.allocate_space(vec2(width, ui.available_height()));
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let selected = state.active;
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, &title));
        let visuals = ui.visuals();
        let fill = if selected {
            visuals.panel_fill
        } else {
            visuals.extreme_bg_color
        };
        if !state.is_being_dragged {
            ui.painter().rect_filled(rect.shrink(0.5), 0.0, fill);
            let text_at = egui::Align2::LEFT_CENTER
                .align_size_within_rect(galley.size(), rect.shrink(margin))
                .min;
            ui.painter().galley(text_at, galley, visuals.text_color());
        }
        let close_rect = egui::Align2::RIGHT_CENTER.align_size_within_rect(close, rect.shrink(margin));
        let close_id = id.with("close");
        let close_response = ui.interact(close_rect, close_id, Sense::click());
        let close_name = strings::format("tab-close-name", &[&folio]);
        close_response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &close_name));
        let stroke = ui.style().interact(&close_response).fg_stroke;
        let x = close_rect.shrink(self.close_button_inner_margin());
        ui.painter()
            .line_segment([x.left_top(), x.right_bottom()], stroke);
        ui.painter()
            .line_segment([x.right_top(), x.left_bottom()], stroke);
        if let Some((g, i)) = pane.and_then(|p| self.place(p.0)) {
            if close_response.clicked() || response.clicked_by(egui::PointerButton::Middle) {
                self.events
                    .push(Event::Action("tab.activate", json!({"group": g, "index": i})));
                self.events.push(Event::Action("tab.close", json!({})));
            } else if response.clicked() {
                self.events
                    .push(Event::Action("tab.activate", json!({"group": g, "index": i})));
            }
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_is_one_move() {
        let from = vec![vec![1, 2, 3], vec![4]];
        assert_eq!(single_move(&from, &[vec![2, 1, 3], vec![4]]), Some((0, 0, 0, 1)));
        assert_eq!(single_move(&from, &[vec![1, 3], vec![2, 4]]), Some((0, 1, 1, 0)));
        assert_eq!(single_move(&from, &[vec![1, 2, 3, 4]]), Some((1, 0, 0, 3)));
        assert_eq!(single_move(&from, &from), None);
        assert_eq!(
            single_move(&from, &[vec![1], vec![2], vec![3, 4]]),
            None,
            "a third group"
        );
        assert_eq!(single_move(&from, &[vec![3, 1, 2], vec![]]), None, "not one move");
    }
}
