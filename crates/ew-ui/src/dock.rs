// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Docking (WP-S2.01 part 2, FT-309; UX/UI doc 16 section 3.4): the project view's panes in an egui_tiles tree.
//!
//! The shell model keeps the layout toolkit-neutral (`ew_viewmodel::layout::Layout`, the content of the workspace
//! files); this module builds the egui_tiles tree from it, shows the tree, and reads the user's changes (a
//! splitter, a tab dragged to another place) back into a `Layout`. Every pane has a thin title bar that is also the
//! drag handle, so a pane can be moved before it has a tab bar of its own.

use egui::{Button, RichText, Sense, Ui};
use egui_tiles::{Container, Linear, LinearDir, Tabs, Tile, TileId, Tiles, Tree, UiResponse};
use ew_viewmodel::layout::{Layout, LayoutNode, PaneKind, SplitDir};
use ew_viewmodel::strings;

/// The dock tree of the project view and the layout version it was built from.
pub struct Dock {
    tree: Tree<PaneKind>,
    version: u64,
}

fn build(node: &LayoutNode, tiles: &mut Tiles<PaneKind>) -> TileId {
    match node {
        LayoutNode::Pane(p) => tiles.insert_pane(*p),
        LayoutNode::Tabs { children, active } => {
            let ids: Vec<TileId> = children.iter().map(|c| build(c, tiles)).collect();
            let active_id = ids.get(*active).copied();
            let mut tabs = Tabs::new(ids);
            tabs.active = active_id;
            tiles.insert_container(tabs)
        }
        LayoutNode::Split { dir, children } => {
            let ids: Vec<TileId> = children.iter().map(|(_, c)| build(c, tiles)).collect();
            let dir = match dir {
                SplitDir::Horizontal => LinearDir::Horizontal,
                SplitDir::Vertical => LinearDir::Vertical,
            };
            let mut linear = Linear::new(dir, ids.clone());
            for (id, (share, _)) in ids.iter().zip(children) {
                linear.shares.set_share(*id, *share as f32);
            }
            tiles.insert_container(linear)
        }
    }
}

/// The layout of the subtree at `id`.
fn read(tiles: &Tiles<PaneKind>, id: TileId) -> Option<LayoutNode> {
    match tiles.get(id)? {
        Tile::Pane(p) => Some(LayoutNode::Pane(*p)),
        Tile::Container(Container::Tabs(tabs)) => {
            let children: Vec<LayoutNode> = tabs.children.iter().filter_map(|c| read(tiles, *c)).collect();
            if children.is_empty() {
                return None;
            }
            let active = tabs
                .active
                .and_then(|a| tabs.children.iter().position(|c| *c == a))
                .unwrap_or(0)
                .min(children.len() - 1);
            Some(LayoutNode::Tabs { children, active })
        }
        Tile::Container(Container::Linear(linear)) => {
            let kids: Vec<(f32, LayoutNode)> = linear
                .children
                .iter()
                .filter_map(|c| read(tiles, *c).map(|n| (linear.shares[*c], n)))
                .collect();
            split(linear_dir(linear.dir), kids)
        }
        // A grid is not part of the layout model; its children are shown side by side with equal shares.
        Tile::Container(Container::Grid(grid)) => {
            let kids: Vec<(f32, LayoutNode)> = grid
                .children()
                .filter_map(|c| read(tiles, *c).map(|n| (1.0, n)))
                .collect();
            split(SplitDir::Horizontal, kids)
        }
    }
}

fn linear_dir(dir: LinearDir) -> SplitDir {
    match dir {
        LinearDir::Horizontal => SplitDir::Horizontal,
        LinearDir::Vertical => SplitDir::Vertical,
    }
}

/// A split of `kids` with shares normalised to thousandths (at least 1 each).
fn split(dir: SplitDir, kids: Vec<(f32, LayoutNode)>) -> Option<LayoutNode> {
    if kids.is_empty() {
        return None;
    }
    let total: f32 = kids
        .iter()
        .map(|(s, _)| s.max(0.0))
        .sum::<f32>()
        .max(f32::EPSILON);
    let children = kids
        .into_iter()
        .map(|(share, node)| {
            let permille = (share.max(0.0) / total * 1000.0).round().max(1.0);
            (permille as u32, node)
        })
        .collect();
    Some(LayoutNode::Split { dir, children })
}

impl Dock {
    /// A dock tree for `layout`, tagged with the shell's layout `version`.
    pub fn new(layout: &Layout, version: u64) -> Dock {
        let mut tiles = Tiles::default();
        let root = build(&layout.root, &mut tiles);
        Dock {
            tree: Tree::new(egui::Id::unique("project-dock"), root, tiles),
            version,
        }
    }

    /// The layout version the tree was built from.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The tree as a layout; `None` for an empty tree.
    pub fn layout(&self) -> Option<Layout> {
        let root = self.tree.root()?;
        read(&self.tree.tiles, root).map(|root| Layout { root })
    }

    /// The number of splitter handles the tree shows: one between each two neighbours of a split.
    pub fn splitters(&self) -> usize {
        fn count(n: &LayoutNode) -> usize {
            match n {
                LayoutNode::Pane(_) => 0,
                LayoutNode::Tabs { children, .. } => children.iter().map(count).sum(),
                LayoutNode::Split { children, .. } => {
                    children.len().saturating_sub(1) + children.iter().map(|(_, c)| count(c)).sum::<usize>()
                }
            }
        }
        self.layout().map_or(0, |l| count(&l.root))
    }

    /// The panes of the tree (tests).
    pub fn panes(&self) -> Vec<PaneKind> {
        self.layout().map(|l| l.panes()).unwrap_or_default()
    }

    /// Moves `pane` into the tab group of `target` (tests drive what a drag does).
    pub fn tab_together(&mut self, pane: PaneKind, target: PaneKind) {
        let (Some(a), Some(b)) = (
            self.tree.tiles.find_pane(&pane),
            self.tree.tiles.find_pane(&target),
        ) else {
            return;
        };
        // A tab group takes the target's place, then the moved pane joins it, as egui_tiles does for a drop.
        let parent = self.tree.tiles.parent_of(b);
        let container = self.tree.tiles.insert_container(Tabs::new(vec![b]));
        match parent {
            Some(parent) => {
                if let Some(Tile::Container(c)) = self.tree.tiles.get_mut(parent) {
                    let _ = c.replace_child(b, container);
                }
            }
            None => {
                self.tree = Tree::new(self.tree.id(), container, std::mem::take(&mut self.tree.tiles));
            }
        }
        self.tree.move_tile_to_container(a, container, 1, false);
        let options = egui_tiles::SimplificationOptions::default();
        self.tree.simplify(&options);
    }

    /// Paints the tree; `pane_ui` paints one pane. A tab picked from a tab group's "All tabs" menu (FT-509) becomes
    /// that group's active tab.
    pub fn show(&mut self, ui: &mut Ui, pane_ui: &mut dyn FnMut(&mut Ui, PaneKind)) {
        let tokens = crate::theme::current(ui.ctx());
        let height = ui.available_height();
        let (clamps, guessed) = self.keep_bottom_min(height, tokens.size_dock_bottom_min);
        if guessed {
            // R3-4: a split without a last shown height was clamped against the guess; the next frame uses its
            // real height.
            ui.ctx().request_repaint();
        }
        let mut behavior = DockBehavior {
            pane_ui,
            chosen: None,
            tokens,
        };
        self.tree.ui(&mut behavior, ui);
        self.restore(&clamps);
        if let Some((group, child)) = behavior.chosen {
            self.activate(group, child);
            ui.ctx().request_repaint();
        }
    }

    // W31-1, R2-2 of cycle 31
    /// Keeps the bottom dock (D14) at least `min` points tall on show: in the vertical split that holds the page
    /// view, the child that holds a bottom pane is shown with at least `min` of the split's height (the split's last
    /// shown height, `fallback` before the first frame), and never more than two thirds of it, so the page view keeps
    /// its place. Display only: returns the shares it changed, which [`Dock::restore`] puts back after the frame, so
    /// the user's shares stay the layout (no rewrite of the workspace file, no share that only ever grows). Other
    /// vertical splits (a column where Messages was docked) are left alone.
    /// The flag is true when a split had no last shown height, so `fallback` was used.
    fn keep_bottom_min(&mut self, fallback: f32, min: f32) -> (Vec<Clamp>, bool) {
        fn holds(tiles: &Tiles<PaneKind>, id: TileId, pred: &dyn Fn(&PaneKind) -> bool) -> bool {
            match tiles.get(id) {
                Some(Tile::Pane(p)) => pred(p),
                Some(Tile::Container(c)) => c.children().any(|c| holds(tiles, *c, pred)),
                None => false,
            }
        }
        let is_bottom = |p: &PaneKind| PaneKind::BOTTOM.contains(p);
        let is_page = |p: &PaneKind| *p == PaneKind::PageView;
        let splits: Vec<TileId> = self
            .tree
            .tiles
            .iter()
            .filter_map(|(id, t)| match t {
                Tile::Container(Container::Linear(l)) if l.dir == LinearDir::Vertical => Some(*id),
                _ => None,
            })
            .collect();
        let mut clamps = Vec::new();
        let mut guessed = false;
        for id in splits {
            let height = match self.tree.tiles.rect(id) {
                Some(r) => r.height(),
                None => {
                    guessed = true;
                    fallback
                }
            };
            let Some(Tile::Container(Container::Linear(linear))) = self.tree.tiles.get(id) else {
                continue;
            };
            let children = linear.children.clone();
            let tiles = &self.tree.tiles;
            let page = children.iter().copied().find(|c| holds(tiles, *c, &is_page));
            let Some(bottom) = children
                .iter()
                .copied()
                .find(|c| holds(tiles, *c, &is_bottom) && Some(*c) != page)
            else {
                continue;
            };
            if page.is_none() || height <= 0.0 {
                continue;
            }
            let want = (min / height).min(2.0 / 3.0);
            let Some(Tile::Container(Container::Linear(linear))) = self.tree.tiles.get_mut(id) else {
                continue;
            };
            let total: f32 = children.iter().map(|c| linear.shares[*c].max(0.0)).sum();
            let own = linear.shares[bottom].max(0.0);
            if total <= 0.0 || own / total >= want {
                continue;
            }
            let user: Vec<(TileId, f32)> = children.iter().map(|c| (*c, linear.shares[*c])).collect();
            // The others keep their ratios; the bottom child's share grows to `want` of the total.
            let others = total - own;
            linear.shares.set_share(bottom, want / (1.0 - want) * others);
            let shown: Vec<(TileId, f32)> = children.iter().map(|c| (*c, linear.shares[*c])).collect();
            clamps.push(Clamp {
                split: id,
                user,
                shown,
            });
        }
        (clamps, guessed)
    }

    /// Puts back the user's shares of every split [`Dock::keep_bottom_min`] changed for the frame, unless the user
    /// moved one of its splitters during the frame (then the shares on show are the user's new ones).
    fn restore(&mut self, clamps: &[Clamp]) {
        for clamp in clamps {
            let Some(Tile::Container(Container::Linear(linear))) = self.tree.tiles.get_mut(clamp.split)
            else {
                continue;
            };
            let untouched = clamp
                .shown
                .iter()
                .all(|(c, share)| linear.children.contains(c) && linear.shares[*c] == *share);
            if untouched {
                for (c, share) in &clamp.user {
                    linear.shares.set_share(*c, *share);
                }
            }
        }
    }

    // FT-509
    /// Makes `child` the active tab of the tab group `group`.
    fn activate(&mut self, group: TileId, child: TileId) {
        if let Some(Tile::Container(Container::Tabs(tabs))) = self.tree.tiles.get_mut(group)
            && tabs.children.contains(&child)
        {
            tabs.set_active(child);
        }
    }

    /// The active pane of the tab group that holds `pane` (tests).
    pub fn active_beside(&self, pane: PaneKind) -> Option<PaneKind> {
        let id = self.tree.tiles.find_pane(&pane)?;
        let group = self.tree.tiles.parent_of(id)?;
        match self.tree.tiles.get(group)? {
            Tile::Container(Container::Tabs(tabs)) => match self.tree.tiles.get(tabs.active?)? {
                Tile::Pane(p) => Some(*p),
                Tile::Container(_) => None,
            },
            _ => None,
        }
    }
}

/// The shares of one split before and after [`Dock::keep_bottom_min`] raised its bottom child for a frame.
struct Clamp {
    split: TileId,
    /// The user's shares (the layout).
    user: Vec<(TileId, f32)>,
    /// The shares on show this frame.
    shown: Vec<(TileId, f32)>,
}

/// The panes of a tab group, each with its tile, in tab order (FT-509's menu).
fn tab_panes(tiles: &Tiles<PaneKind>, tabs: &Tabs) -> Vec<(TileId, PaneKind)> {
    tabs.children
        .iter()
        .filter_map(|c| match tiles.get(*c)? {
            Tile::Pane(p) => Some((*c, *p)),
            Tile::Container(_) => None,
        })
        .collect()
}

struct DockBehavior<'a> {
    pane_ui: &'a mut dyn FnMut(&mut Ui, PaneKind),
    /// FT-509: the (tab group, tab) picked from the "All tabs" menu this frame.
    chosen: Option<(TileId, TileId)>,
    /// The tokens of the theme on show (doc 16a §4.14's egui_tiles row).
    tokens: std::sync::Arc<crate::theme::Tokens>,
}

impl egui_tiles::Behavior<PaneKind> for DockBehavior<'_> {
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: TileId, pane: &mut PaneKind) -> UiResponse {
        // The title bar is the drag handle: a pane without a tab bar can still be moved.
        // A button without a frame, so that the title has an accessible name (a label with a drag sense does not).
        let title = ui.add(
            Button::new(RichText::new(strings::text(pane.title_key())).strong())
                .frame(false)
                .sense(Sense::click_and_drag()),
        );
        ui.separator();
        (self.pane_ui)(ui, *pane);
        if title.drag_started() {
            UiResponse::DragStarted
        } else {
            UiResponse::None
        }
    }

    fn tab_title_for_pane(&mut self, pane: &PaneKind) -> egui::WidgetText {
        strings::text(pane.title_key()).into()
    }

    fn is_tab_closable(&self, _tiles: &Tiles<PaneKind>, _tile_id: TileId) -> bool {
        false
    }

    // WP-S2.17 part 1: the dock's look from the tokens (doc 16a §4.14).
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        self.tokens.size_dock_tab_bar
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        self.tokens.border_width
    }

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        self.tokens.surface_chrome.color32()
    }

    fn tab_bg_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<PaneKind>,
        _tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            self.tokens.surface_panel.color32()
        } else {
            crate::theme::NONE
        }
    }

    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<PaneKind>,
        _tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Stroke {
        if state.active {
            crate::theme::stroke(self.tokens.border_width, self.tokens.border_default)
        } else {
            egui::Stroke::NONE
        }
    }

    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
        crate::theme::stroke(self.tokens.border_width, self.tokens.border_default)
    }

    fn tab_text_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<PaneKind>,
        _tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            self.tokens.text_primary.color32()
        } else {
            self.tokens.text_secondary.color32()
        }
    }

    fn resize_stroke(&self, _style: &egui::Style, resize_state: egui_tiles::ResizeState) -> egui::Stroke {
        let t = &self.tokens;
        match resize_state {
            egui_tiles::ResizeState::Idle => crate::theme::stroke(t.border_width, t.border_default),
            egui_tiles::ResizeState::Hovering | egui_tiles::ResizeState::Dragging => {
                crate::theme::stroke(t.border_width, t.accent)
            }
        }
    }

    /// The registry owns Ctrl+Tab (`tab.next`, WP-S2.03 §6.2): egui_tiles' own cycling of the hovered container's tabs
    /// would act outside `dispatch`.
    fn cycle_tabs_with_keyboard(&self) -> bool {
        false
    }

    // WP-S2.06 part 1
    /// Panes are dragged (by their tab or title), containers are not: a tab group (the Pages pane with the navigators
    /// beside it, §6.8) gets no unnamed drag area of its own, as in the canvas area.
    fn is_tile_draggable(&self, tiles: &Tiles<PaneKind>, tile_id: TileId) -> bool {
        matches!(tiles.get(tile_id), Some(egui_tiles::Tile::Pane(_)))
    }

    // FT-509
    /// A tab group of two or more tabs gets an "All tabs" menu at the right of its tab strip, listing every tab by its
    /// title, so that a tab hidden behind the strip's overflow arrows stays reachable and named.
    fn top_bar_right_ui(
        &mut self,
        tiles: &Tiles<PaneKind>,
        ui: &mut Ui,
        tile_id: TileId,
        tabs: &Tabs,
        _scroll_offset: &mut f32,
    ) {
        let panes = tab_panes(tiles, tabs);
        if panes.len() < 2 {
            return;
        }
        let name = strings::text("dock-all-tabs");
        let response = ui
            .menu_button("≡", |ui| {
                for (child, pane) in &panes {
                    let title = strings::text(pane.title_key());
                    if ui.selectable_label(tabs.is_active(*child), title).clicked() {
                        self.chosen = Some((tile_id, *child));
                        ui.close();
                    }
                }
            })
            .response;
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
        response.on_hover_text(name);
    }
}

#[cfg(test)]
mod tests {
    use ew_viewmodel::layout::{LayoutNode, PaneKind, SplitDir};
    use ew_viewmodel::navigators::NavKind;

    use super::*;

    /// One frame of `dock` in a `w` × `h` window.
    fn frame(ctx: &egui::Context, dock: &mut Dock, w: f32, h: f32) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| dock.show(ui, &mut |_, _| {}));
        });
        // Nothing paints the frame; its texture changes are dropped on purpose.
        out.textures_delta.clear();
    }

    /// The height `pane` was shown with on the last frame.
    fn shown_height(dock: &Dock, pane: PaneKind) -> f32 {
        let id = dock.tree.tiles.find_pane(&pane).unwrap();
        dock.tree.tiles.rect(id).unwrap().height()
    }

    /// The page view above the Messages pane (650 : 350), beside the page list.
    fn page_over_messages() -> Layout {
        Layout {
            root: LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    (200, LayoutNode::Pane(PaneKind::Pages)),
                    (
                        800,
                        LayoutNode::Split {
                            dir: SplitDir::Vertical,
                            children: vec![
                                (650, LayoutNode::Pane(PaneKind::PageView)),
                                (350, LayoutNode::Pane(PaneKind::Navigator(NavKind::Messages))),
                            ],
                        },
                    ),
                ],
            },
        }
    }

    /// R2-2 of cycle 31: at 1280 × 800 the Messages pane is shown at least 300 pt tall, but the clamp is display
    /// only: the layout after the frames equals the layout before them (no rewrite of the workspace file).
    #[test]
    fn the_bottom_clamp_is_shown_but_not_persisted() {
        let layout = page_over_messages();
        let mut dock = Dock::new(&layout, 0);
        let ctx = egui::Context::default();
        for _ in 0..3 {
            frame(&ctx, &mut dock, 1280.0, 800.0);
            assert_eq!(dock.layout().as_ref(), Some(&layout), "the layout is the user's");
        }
        let h = shown_height(&dock, PaneKind::Navigator(NavKind::Messages));
        assert!(h >= 299.0, "the Messages pane is shown {h} pt tall");
    }

    /// R2-2 of cycle 31: a short window then a tall one does not raise the share: on the tall frame the Messages pane
    /// has its own share (350 of 1000) again, not the two thirds of the short frame.
    #[test]
    fn a_short_then_tall_window_does_not_ratchet_the_share() {
        let layout = page_over_messages();
        let mut dock = Dock::new(&layout, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut dock, 1280.0, 400.0);
        frame(&ctx, &mut dock, 1280.0, 400.0);
        let short = shown_height(&dock, PaneKind::Navigator(NavKind::Messages));
        let page = shown_height(&dock, PaneKind::PageView);
        assert!(short > page, "short: Messages {short}, page view {page}");
        assert_eq!(dock.layout().as_ref(), Some(&layout));
        frame(&ctx, &mut dock, 1280.0, 1400.0);
        frame(&ctx, &mut dock, 1280.0, 1400.0);
        assert_eq!(dock.layout().as_ref(), Some(&layout));
        let tall = shown_height(&dock, PaneKind::Navigator(NavKind::Messages));
        let page = shown_height(&dock, PaneKind::PageView);
        let ratio = tall / (tall + page);
        assert!(
            (0.33..0.37).contains(&ratio),
            "tall: Messages {tall}, page view {page}"
        );
    }

    /// R2-2 of cycle 31: a left column where Messages was docked under the page list is not the page view's split,
    /// so it is not clamped.
    #[test]
    fn a_left_column_holding_messages_is_not_clamped() {
        let layout = Layout {
            root: LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    (
                        300,
                        LayoutNode::Split {
                            dir: SplitDir::Vertical,
                            children: vec![
                                (500, LayoutNode::Pane(PaneKind::Pages)),
                                (500, LayoutNode::Pane(PaneKind::Navigator(NavKind::Messages))),
                            ],
                        },
                    ),
                    (700, LayoutNode::Pane(PaneKind::PageView)),
                ],
            },
        };
        let mut dock = Dock::new(&layout, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut dock, 1280.0, 500.0);
        frame(&ctx, &mut dock, 1280.0, 500.0);
        assert_eq!(dock.layout().as_ref(), Some(&layout));
        let messages = shown_height(&dock, PaneKind::Navigator(NavKind::Messages));
        let pages = shown_height(&dock, PaneKind::Pages);
        assert!(messages < 300.0, "Messages {messages}");
        assert!(
            (messages - pages).abs() < 1.0,
            "an even split: Messages {messages}, Pages {pages}"
        );
    }

    /// R3-3 of cycle 31: when the user drags the splitter during a clamped frame, `restore` keeps the dragged
    /// shares, so the layout is the user's new one.
    #[test]
    fn a_drag_during_a_clamped_frame_keeps_the_dragged_shares() {
        let layout = page_over_messages();
        let mut dock = Dock::new(&layout, 0);
        let (clamps, guessed) = dock.keep_bottom_min(800.0, 300.0);
        assert!(guessed, "no frame yet, so the fallback height was used");
        assert_eq!(
            clamps.len(),
            1,
            "350/1000 of 800 is under 300, so the split is clamped"
        );
        let clamp = &clamps[0];
        let (page, messages) = (clamp.shown[0].0, clamp.shown[1].0);
        let Some(Tile::Container(Container::Linear(linear))) = dock.tree.tiles.get_mut(clamp.split) else {
            panic!("the clamped split is a linear container");
        };
        // A drag moves share from one neighbour to the other.
        let (p, m) = (linear.shares[page], linear.shares[messages]);
        let delta = 0.1 * (p + m);
        linear.shares.set_share(page, p - delta);
        linear.shares.set_share(messages, m + delta);
        let permille = |x: f32| (x / (p + m) * 1000.0).round() as u32;
        let mut dragged = layout.clone();
        let LayoutNode::Split { children, .. } = &mut dragged.root else {
            unreachable!()
        };
        let LayoutNode::Split { children, .. } = &mut children[1].1 else {
            unreachable!()
        };
        children[0].0 = permille(p - delta);
        children[1].0 = permille(m + delta);
        dock.restore(&clamps);
        assert_eq!(dock.layout(), Some(dragged));
        assert_ne!(dock.layout().as_ref(), Some(&layout));
    }

    #[test]
    fn the_schematic_workspace_round_trips_through_the_tree() {
        let layout = Layout::schematic();
        let dock = Dock::new(&layout, 0);
        assert_eq!(dock.layout().as_ref(), Some(&layout));
        assert_eq!(
            dock.panes(),
            [PaneKind::Pages, PaneKind::PageView, PaneKind::Properties]
        );
    }

    #[test]
    fn tabs_and_nested_splits_round_trip() {
        let layout = Layout {
            root: LayoutNode::Split {
                dir: SplitDir::Vertical,
                children: vec![
                    (
                        700,
                        LayoutNode::Tabs {
                            children: vec![
                                LayoutNode::Pane(PaneKind::PageView),
                                LayoutNode::Pane(PaneKind::Pages),
                            ],
                            active: 1,
                        },
                    ),
                    (
                        300,
                        LayoutNode::Split {
                            dir: SplitDir::Horizontal,
                            children: vec![],
                        },
                    ),
                ],
            },
        };
        // An empty split reads back as nothing: the tree keeps no empty container.
        let dock = Dock::new(&layout, 3);
        assert_eq!(dock.version(), 3);
        let back = dock.layout().unwrap();
        let LayoutNode::Split { children, .. } = &back.root else {
            panic!("{back:?}");
        };
        assert_eq!(children.len(), 1, "the empty split is gone");
        assert!(matches!(children[0].1, LayoutNode::Tabs { active: 1, .. }));
    }

    #[test]
    fn shares_are_normalised_to_thousandths() {
        let layout = Layout {
            root: LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    (1, LayoutNode::Pane(PaneKind::Pages)),
                    (3, LayoutNode::Pane(PaneKind::PageView)),
                ],
            },
        };
        let back = Dock::new(&layout, 0).layout().unwrap();
        assert_eq!(
            back.root,
            LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    (250, LayoutNode::Pane(PaneKind::Pages)),
                    (750, LayoutNode::Pane(PaneKind::PageView)),
                ],
            }
        );
    }

    #[test]
    fn dragging_a_pane_onto_another_makes_a_tab_group() {
        let mut dock = Dock::new(&Layout::schematic(), 0);
        dock.tab_together(PaneKind::Pages, PaneKind::PageView);
        let layout = dock.layout().unwrap();
        assert_eq!(layout.panes().len(), 3, "no pane is lost or doubled: {layout:?}");
        let LayoutNode::Split { children, .. } = &layout.root else {
            panic!("the property grid stays beside the tab group: {layout:?}");
        };
        assert!(
            matches!(children[0].1, LayoutNode::Tabs { .. }),
            "both panes share one tab group: {layout:?}"
        );
        // And the result is a layout the workspace file can hold.
        let text = layout.to_json("Tabbed");
        assert_eq!(Layout::parse(&text).unwrap().1, layout);
    }

    #[test]
    fn container_kind_names_are_used() {
        // The tree has the same container kinds as the model: linear and tabs.
        let dock = Dock::new(&Layout::schematic(), 0);
        let root = dock.tree.root().unwrap();
        assert_eq!(
            dock.tree.tiles.get(root).and_then(|t| t.container_kind()),
            Some(egui_tiles::ContainerKind::Horizontal)
        );
    }
}
