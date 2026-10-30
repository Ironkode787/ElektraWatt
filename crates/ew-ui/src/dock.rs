// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Docking (WP-S2.01 part 2, FT-309; UX/UI doc 16 section 3.4): the project view's panes in an egui_tiles tree.
//!
//! The shell model keeps the layout toolkit-neutral (`ew_viewmodel::layout::Layout`, the content of the workspace
//! files); this module builds the egui_tiles tree from it, shows the tree, and reads the user's changes (a
//! splitter, a tab dragged to another place) back into a `Layout`.
//!
//! WP-S2.17 part 3 (doc 16a §5.4, §5.7): the tab group that holds the Pages pane (the navigator group) draws no tab
//! bar, the left rail chooses its pane; a pane outside a tab bar has the 32-px panel header ("PROJECT",
//! "PROPERTIES"), which is also its drag handle; the page view and the panes of a tab bar have no heading. The Project
//! entry's panes (Pages and Devices) carry the segmented control under their header. The splitters are named
//! (FT-338, `crate::splitter`), and the left column collapses to its rail.

use std::collections::BTreeMap;

use egui::{Rect, Sense, Ui, pos2, vec2};
use egui_tiles::{Container, Linear, LinearDir, Tabs, Tile, TileId, Tiles, Tree, UiResponse};
use ew_viewmodel::layout::{Layout, LayoutNode, PaneKind, SplitDir};
use ew_viewmodel::rail::{LEFT, PROJECT_PANES};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

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

    /// The number of splitter handles the tree shows: one between each two neighbours of a shown split.
    pub fn splitters(&self) -> usize {
        crate::splitter::count(&self.tree)
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
    /// that group's active tab. `collapsed` hides the left column (the root's child that holds the navigator group;
    /// doc 16a §5.4). `counts` are the count chips of the dock tabs (doc 16a §5.7: Messages' open findings); a pane
    /// absent or at zero has none. Returns what the panel headers' and the tab bars' controls asked for (the segmented
    /// control's `rail.show`, the bottom dock's close button).
    pub(crate) fn show(
        &mut self,
        ui: &mut Ui,
        collapsed: bool,
        counts: &[(PaneKind, usize)],
        pane_ui: &mut dyn FnMut(&mut Ui, PaneKind),
    ) -> Vec<Event> {
        let tokens = crate::theme::current(ui.ctx());
        let height = ui.available_height();
        self.collapse_left(collapsed);
        // FX38d: the page view (the canvas and its tab bar) keeps at least half the window's client height (the
        // default 1536 × 994 included); ID-743's 300-point least height gives way to it in a short window.
        let floor = ui.ctx().content_rect().height() / 2.0;
        let (clamps, guessed) = self.keep_bottom_min(height, tokens.size_dock_bottom_min, floor);
        if guessed {
            // R3-4: a split without a last shown height was clamped against the guess; the next frame uses its
            // real height.
            ui.ctx().request_repaint();
        }
        let chrome = self.chrome();
        let mut behavior = DockBehavior {
            pane_ui,
            chosen: None,
            tokens: tokens.clone(),
            chrome,
            events: Vec::new(),
            counts,
        };
        self.tree.ui(&mut behavior, ui);
        let events = std::mem::take(&mut behavior.events);
        let chosen = behavior.chosen.take();
        drop(behavior);
        crate::splitter::show(ui, &mut self.tree, &tokens, "dock", &|tiles, id| {
            title_of(tiles, id)
        });
        self.restore(&clamps);
        if let Some((group, child)) = chosen {
            self.activate(group, child);
            ui.ctx().request_repaint();
        }
        events
    }

    // WP-S2.17 part 3
    /// Hides (or shows again) the left column: the child of a horizontal root split that holds the navigator group,
    /// unless it also holds the page view.
    fn collapse_left(&mut self, collapsed: bool) {
        let Some(root) = self.tree.root() else { return };
        let Some(Tile::Container(Container::Linear(linear))) = self.tree.tiles.get(root) else {
            return;
        };
        if linear.dir != LinearDir::Horizontal {
            return;
        }
        let children = linear.children.clone();
        for child in children {
            let tiles = &self.tree.tiles;
            if holds(tiles, child, &is_rail_pane) && !holds(tiles, child, &|p| *p == PaneKind::PageView) {
                self.tree.tiles.set_visible(child, !collapsed);
            }
        }
    }

    /// The navigator group's tab containers and the header of every pane (WP-S2.17 part 3).
    fn chrome(&self) -> Chrome {
        let tiles = &self.tree.tiles;
        let mut chrome = Chrome::default();
        for (id, tile) in tiles.iter() {
            let Tile::Container(Container::Tabs(tabs)) = tile else {
                continue;
            };
            if tabs.children.iter().any(|c| holds(tiles, *c, &is_rail_pane)) {
                chrome.rail_groups.push(*id);
            }
        }
        for (id, tile) in tiles.iter() {
            let Tile::Pane(pane) = tile else { continue };
            let parent = tiles.parent_of(*id);
            let in_bar = parent.is_some_and(|p| {
                matches!(tiles.get(p), Some(Tile::Container(Container::Tabs(_))))
                    && !chrome.rail_groups.contains(&p)
            });
            if in_bar || *pane == PaneKind::PageView {
                continue;
            }
            let shown = parent
                .filter(|p| chrome.rail_groups.contains(p))
                .and_then(|p| match tiles.get(p) {
                    Some(Tile::Container(Container::Tabs(tabs))) => tabs.active,
                    _ => None,
                });
            // A navigator alone in its place (no tab group) is the navigator group too.
            let alone =
                !parent.is_some_and(|p| matches!(tiles.get(p), Some(Tile::Container(Container::Tabs(_)))));
            let in_rail =
                parent.is_some_and(|p| chrome.rail_groups.contains(&p)) || (alone && is_rail_pane(pane));
            let project = in_rail && PROJECT_PANES.contains(pane);
            let segmented = project.then(|| {
                // Devices is in the group: the control shows it chosen when it is the active tab.
                match shown.and_then(|a| tiles.get(a)) {
                    Some(Tile::Pane(p)) => *p,
                    _ => *pane,
                }
            });
            chrome.headers.insert(
                id.0,
                Header {
                    title: header_title(*pane, in_rail),
                    segmented,
                    // FX38d (doc 16a §5.7): a bottom pane alone, without the tab row, carries the dock's close button
                    // in its header.
                    close: PaneKind::BOTTOM.contains(pane),
                },
            );
        }
        chrome
    }

    // W31-1, R2-2 of cycle 31
    /// Keeps the bottom dock (D14) at least `min` points tall on show: in the vertical split that holds the page
    /// view, the child that holds a bottom pane is shown with at least `min` of the split's height (the split's last
    /// shown height, `fallback` before the first frame), and never more than two thirds of it nor so much that the page
    /// view's child falls under `floor` (half the window, FX38d), so the page view keeps its place. Display only: returns the shares it changed, which [`Dock::restore`] puts back after the frame, so
    /// the user's shares stay the layout (no rewrite of the workspace file, no share that only ever grows). Other
    /// vertical splits (a column where Messages was docked) are left alone.
    /// The flag is true when a split had no last shown height, so `fallback` was used.
    fn keep_bottom_min(&mut self, fallback: f32, min: f32, floor: f32) -> (Vec<Clamp>, bool) {
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
            let room = (height - floor).max(0.0);
            let want = (min.min(room) / height).min(2.0 / 3.0);
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

/// Whether `pane` belongs to the navigator group: the Pages pane, the Devices navigator or a left-rail navigator.
fn is_rail_pane(pane: &PaneKind) -> bool {
    PROJECT_PANES.contains(pane) || LEFT.iter().any(|(.., p)| p == pane)
}

/// Whether the subtree at `id` holds a pane `pred` accepts.
fn holds(tiles: &Tiles<PaneKind>, id: TileId, pred: &dyn Fn(&PaneKind) -> bool) -> bool {
    match tiles.get(id) {
        Some(Tile::Pane(p)) => pred(p),
        Some(Tile::Container(c)) => c.children().any(|c| holds(tiles, *c, pred)),
        None => false,
    }
}

/// The title of the pane a tile shows (a splitter's name): a pane's own, a tab group's active pane's, a split's first
/// child's.
fn title_of(tiles: &Tiles<PaneKind>, id: TileId) -> String {
    match tiles.get(id) {
        Some(Tile::Pane(p)) => strings::text(p.title_key()).to_owned(),
        Some(Tile::Container(Container::Tabs(tabs))) => tabs
            .active
            .or_else(|| tabs.children.first().copied())
            .map(|c| title_of(tiles, c))
            .unwrap_or_default(),
        Some(Tile::Container(c)) => c
            .children()
            .next()
            .map(|c| title_of(tiles, *c))
            .unwrap_or_default(),
        None => String::new(),
    }
}

/// The header title of `pane`: in the navigator group its rail entry's name (Project for Pages and Devices), else
/// the pane's title.
fn header_title(pane: PaneKind, in_rail: bool) -> String {
    if in_rail && PROJECT_PANES.contains(&pane) {
        return strings::text("rail-project").to_owned();
    }
    strings::text(pane.title_key()).to_owned()
}

/// A pane's panel header (doc 16a §5.4).
struct Header {
    /// The title, drawn in `type.label` (upper case); the accessible name keeps its case.
    title: String,
    /// The Project entry's segmented control with the pane it shows chosen.
    segmented: Option<PaneKind>,
    /// The bottom dock's close button at the header's right end (a bottom pane alone; FX38d).
    close: bool,
}

/// A dock tab's horizontal padding and its underline's inset (doc 16a §5.7: 10 px).
const DOCK_TAB_PADDING: f32 = 10.0;

/// How a dock tab draws its title (doc 16a §6.4).
struct TabLook {
    font: egui::FontId,
    colour: crate::theme::Rgba,
    /// The 2-px accent underline.
    underline: bool,
}

/// A dock tab's look: `type.small`; active SemiBold in `color.text.primary` with the underline, else
/// `color.text.secondary`.
fn tab_look(t: &crate::theme::Tokens, active: bool) -> TabLook {
    if active {
        TabLook {
            font: crate::theme::font(&crate::theme::TypeToken {
                semibold: true,
                ..t.type_small
            }),
            colour: t.text_primary,
            underline: true,
        }
    } else {
        TabLook {
            font: crate::theme::font(&t.type_small),
            colour: t.text_secondary,
            underline: false,
        }
    }
}

/// What the dock draws around the panes (WP-S2.17 part 3).
#[derive(Default)]
struct Chrome {
    /// The tab groups whose tab bar is not drawn (the rail drives them).
    rail_groups: Vec<TileId>,
    /// The panel header of each pane that has one, by tile.
    headers: BTreeMap<u64, Header>,
}

struct DockBehavior<'a> {
    pane_ui: &'a mut dyn FnMut(&mut Ui, PaneKind),
    /// FT-509: the (tab group, tab) picked from the "All tabs" menu this frame.
    chosen: Option<(TileId, TileId)>,
    /// The tokens of the theme on show (doc 16a §4.14's egui_tiles row).
    tokens: std::sync::Arc<crate::theme::Tokens>,
    /// The navigator group and the panel headers.
    chrome: Chrome,
    /// What the headers' controls asked for this frame.
    events: Vec<Event>,
    /// The count chips of the dock tabs.
    counts: &'a [(PaneKind, usize)],
}

impl DockBehavior<'_> {
    /// The panel header of a pane (doc 16a §5.4): `size.panel_header` high, the title in `type.label` at `space.4`;
    /// a button without a frame for AccessKit, sensing drags (the pane's drag handle).
    fn header(&mut self, ui: &mut Ui, header: &Header) -> egui::Response {
        let t = self.tokens.clone();
        let (rect, response) = ui.allocate_exact_size(
            vec2(ui.available_width(), t.size_panel_header),
            Sense::click_and_drag(),
        );
        let name = header.title.clone();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
        let text = if t.type_label.upper {
            header.title.to_uppercase()
        } else {
            header.title.clone()
        };
        ui.painter().text(
            pos2(rect.left() + t.space_4, rect.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            crate::theme::font(&t.type_label),
            t.text_secondary.color32(),
        );
        if header.close {
            // Doc 16a §5.7: one close button (Ctrl+J) at the right end closes the bottom dock.
            let size = t.size_control_inline;
            let at = Rect::from_center_size(
                pos2(rect.right() - t.space_2 - size / 2.0, rect.center().y),
                vec2(size, size),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(at));
            let name = strings::text("dock-close");
            if crate::components::icon_button(&mut child, crate::icons::IconId::x, name).clicked() {
                self.events
                    .push(Event::Action("view.bottom_dock", json!({ "on": false })));
            }
        }
        if let Some(shown) = header.segmented {
            self.segmented(ui, shown);
        }
        response
    }

    /// The Project entry's segmented control "Pages | Devices" (doc 16a §6.20): a `TabList` of two `Tab` nodes; a click
    /// shows the other navigator in the group.
    fn segmented(&mut self, ui: &mut Ui, shown: PaneKind) {
        let t = self.tokens.clone();
        let width = ui.available_width();
        let (outer, _) = ui.allocate_exact_size(
            vec2(width, t.size_control_inline + 2.0 * t.space_1 + t.space_3),
            Sense::hover(),
        );
        let track = Rect::from_min_size(
            pos2(outer.left() + t.space_4, outer.top()),
            vec2(
                (width - 2.0 * t.space_4).max(0.0),
                t.size_control_inline + 2.0 * t.space_1,
            ),
        );
        let list_id = ui.scope_id().with("ew-segmented");
        let list = ui.interact(track, list_id, Sense::hover());
        let list_name = strings::text("rail-views").to_owned();
        list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TabList, true, &list_name));
        // The two tabs are the list's children.
        let tabs = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(track)
                .accessibility_parent(list_id),
        );
        let painter = ui.painter();
        painter.rect_filled(
            track,
            crate::theme::radius(t.radius_md),
            t.surface_sunken.color32(),
        );
        let inner = track.shrink(t.space_1);
        let half = inner.width() / 2.0;
        for (i, (pane, key, word)) in [
            (PROJECT_PANES[0], "rail-pages", "pages"),
            (PROJECT_PANES[1], "rail-devices", "devices"),
        ]
        .into_iter()
        .enumerate()
        {
            let rect = Rect::from_min_size(
                pos2(inner.left() + i as f32 * half, inner.top()),
                vec2(half, inner.height()),
            );
            let chosen = shown == pane;
            let id = list_id.with(word);
            let response = tabs.interact(rect, id, Sense::click());
            let label = strings::text(key).to_owned();
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, &label));
            if chosen {
                ui.painter()
                    .add(egui::Shape::Rect(egui::epaint::RectShape::filled(
                        rect,
                        crate::theme::radius(t.radius_sm),
                        t.surface_panel.color32(),
                    )));
            } else if response.hovered() {
                ui.painter()
                    .rect_filled(rect, crate::theme::radius(t.radius_sm), t.state_hover.color32());
            }
            let (font, colour) = if chosen {
                (
                    crate::theme::font(&crate::theme::TypeToken {
                        semibold: true,
                        ..t.type_small
                    }),
                    t.text_primary,
                )
            } else {
                (crate::theme::font(&t.type_small), t.text_secondary)
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &label,
                font,
                colour.color32(),
            );
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect,
                    crate::theme::radius(t.radius_sm),
                    crate::theme::stroke(t.focus_width, t.focus),
                    egui::StrokeKind::Inside,
                );
            }
            if response.clicked() && !chosen {
                self.events.push(Event::Action(
                    "rail.show",
                    json!({ "side": "left", "pane": word }),
                ));
            }
        }
    }
}

impl egui_tiles::Behavior<PaneKind> for DockBehavior<'_> {
    fn pane_ui(&mut self, ui: &mut Ui, tile_id: TileId, pane: &mut PaneKind) -> UiResponse {
        // WP-S2.17 part 3: the panel header is the drag handle; a pane in a tab bar is dragged by its tab.
        let header = self.chrome.headers.remove(&tile_id.0);
        let dragged = match &header {
            Some(h) => self.header(ui, h).drag_started(),
            None => false,
        };
        (self.pane_ui)(ui, *pane);
        if dragged {
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

    // WP-S2.17 part 3: the navigator group's tab bar is the rail.
    fn is_tab_bar_visible(&self, tile_id: TileId, _tabs: &Tabs) -> bool {
        !self.chrome.rail_groups.contains(&tile_id)
    }

    /// FT-338: the dock draws its own named splitter handles (`crate::splitter`); egui_tiles paints the idle line.
    fn is_container_resizable(&self, _tiles: &Tiles<PaneKind>, _tile_id: TileId) -> bool {
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

    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
        crate::theme::stroke(self.tokens.border_width, self.tokens.border_subtle)
    }

    /// FT-540: a dock tab (doc 16a §6.4, §5.7): no box; `type.small` with 10-px padding, the inactive one in
    /// `color.text.secondary` with the hover overlay, the active one SemiBold in `color.text.primary` with a 2-px
    /// accent underline inset 10 px; the focus ring. A button node named by the title, selected while active, as
    /// egui_tiles names it.
    fn tab_ui(
        &mut self,
        tiles: &mut Tiles<PaneKind>,
        ui: &mut Ui,
        id: egui::Id,
        tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Response {
        let t = self.tokens.clone();
        let title = self.tab_title_for_tile(tiles, tile_id).text().to_owned();
        let look = tab_look(&t, state.active);
        let galley = ui
            .painter()
            .layout_no_wrap(title.clone(), look.font, look.colour.color32());
        // Doc 16a §5.7: a tab whose pane has items carries a count chip (§6.16's count badge) after its title.
        let count = match tiles.get(tile_id) {
            Some(Tile::Pane(p)) => self.counts.iter().find(|(k, _)| k == p).map_or(0, |(_, n)| *n),
            _ => 0,
        };
        let chip = (count > 0).then(|| crate::components::count_badge_layout(ui, &t, count));
        let chip_w = chip.as_ref().map_or(0.0, |(_, size)| t.space_2 + size.x);
        let (_, rect) = ui.allocate_space(vec2(
            galley.size().x + chip_w + 2.0 * DOCK_TAB_PADDING,
            ui.available_height(),
        ));
        let draggable = self.is_tile_draggable(tiles, tile_id);
        let sense = if draggable {
            Sense::click_and_drag()
        } else {
            Sense::click()
        };
        let response = ui.interact(rect, id, sense);
        let response = if draggable {
            response.on_hover_cursor(self.tab_hover_cursor_icon())
        } else {
            response
        };
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), state.active, &title)
        });
        if count > 0 {
            ui.ctx()
                .accesskit_node_builder(response.id, |n| n.set_value(count.to_string()));
        }
        if ui.is_rect_visible(rect) && !state.is_being_dragged {
            if !state.active {
                crate::command_bar::state_overlay(ui, &t, &response, true, t.radius_sm);
            }
            let at = pos2(
                rect.left() + DOCK_TAB_PADDING,
                rect.center().y - galley.size().y / 2.0,
            );
            let text_right = at.x + galley.size().x;
            ui.painter().galley(at, galley, look.colour.color32());
            if let Some((chip, size)) = chip {
                let r =
                    Rect::from_min_size(pos2(text_right + t.space_2, rect.center().y - size.y / 2.0), size);
                crate::components::paint_count_badge_on_chrome(ui.painter(), &t, r, chip);
            }
            if look.underline {
                let line = Rect::from_min_max(
                    pos2(rect.left() + DOCK_TAB_PADDING, rect.bottom() - t.space_1),
                    pos2(rect.right() - DOCK_TAB_PADDING, rect.bottom()),
                );
                ui.painter()
                    .rect_filled(line, crate::theme::radius(t.radius_mark), t.accent.color32());
            }
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect,
                    crate::theme::radius(t.radius_sm),
                    crate::theme::stroke(t.focus_width, t.focus),
                    egui::StrokeKind::Inside,
                );
            }
        }
        response
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
        // Doc 16a §5.7: one close button at the right end of the bottom dock's tab row closes the dock.
        if panes.iter().any(|(_, p)| PaneKind::BOTTOM.contains(p)) {
            ui.add_space(self.tokens.space_2);
            let name = strings::text("dock-close");
            if crate::components::icon_button(ui, crate::icons::IconId::x, name).clicked() {
                self.events
                    .push(Event::Action("view.bottom_dock", json!({ "on": false })));
            }
        }
        if panes.len() < 2 {
            return;
        }
        // FT-562: "All tabs" is a Subtle 24-px icon button without a border (doc 16a §6.3), its list a menu (§6.13).
        let name = strings::text("dock-all-tabs");
        let response = crate::components::icon_button(ui, crate::icons::IconId::dots, name);
        egui::Popup::menu(&response)
            .style(crate::menu::menu_style(&self.tokens))
            .show(|ui| {
                for (child, pane) in &panes {
                    let title = strings::text(pane.title_key());
                    if ui.selectable_label(tabs.is_active(*child), title).clicked() {
                        self.chosen = Some((tile_id, *child));
                        ui.close();
                    }
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use ew_viewmodel::layout::{LayoutNode, PaneKind, SplitDir};
    use ew_viewmodel::navigators::NavKind;

    use super::*;

    /// FT-540: the dock tabs follow doc 16a §6.4: `type.small`, the active one SemiBold in the primary text colour
    /// with the accent underline, the others in the secondary colour without it.
    #[test]
    fn the_dock_tabs_follow_the_tab_family() {
        let t = crate::theme::tokens(crate::theme::Theme::Light, crate::theme::Density::Comfortable);
        let active = tab_look(&t, true);
        let other = tab_look(&t, false);
        assert_eq!(active.font.size, t.type_small.size);
        assert_eq!(other.font, crate::theme::font(&t.type_small));
        assert_ne!(active.font, other.font, "the active tab is SemiBold");
        assert!(active.underline && !other.underline);
        assert_eq!(active.colour, t.text_primary);
        assert_eq!(other.colour, t.text_secondary);
    }

    /// One frame of `dock` in a `w` × `h` window.
    fn frame(ctx: &egui::Context, dock: &mut Dock, w: f32, h: f32) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| dock.show(ui, false, &[], &mut |_, _| {}));
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
    /// has its own share (350 of 1000) again, not the raised share of the short frame (FX38d: which leaves the page
    /// view half the window).
    #[test]
    fn a_short_then_tall_window_does_not_ratchet_the_share() {
        let layout = page_over_messages();
        let mut dock = Dock::new(&layout, 0);
        let ctx = egui::Context::default();
        frame(&ctx, &mut dock, 1280.0, 400.0);
        frame(&ctx, &mut dock, 1280.0, 400.0);
        let short = shown_height(&dock, PaneKind::Navigator(NavKind::Messages));
        let page = shown_height(&dock, PaneKind::PageView);
        let share = short / (short + page);
        assert!(share > 0.4, "short: Messages {short}, page view {page}: raised");
        assert!(
            page >= 400.0 / 2.0 - 1.0,
            "short: the page view {page} keeps half the window"
        );
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
        let (clamps, guessed) = dock.keep_bottom_min(800.0, 300.0, 0.0);
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
            [PaneKind::Pages, PaneKind::Properties, PaneKind::PageView]
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
        // WP-S2.17 part 3: Properties under Pages; dragged onto the page view, Pages leaves the left column.
        let LayoutNode::Split { children, .. } = &layout.root else {
            panic!("the property grid stays beside the tab group: {layout:?}");
        };
        assert!(
            children.iter().any(|(_, c)| matches!(
                c,
                LayoutNode::Tabs { children, .. }
                    if children.contains(&LayoutNode::Pane(PaneKind::Pages))
                        && children.contains(&LayoutNode::Pane(PaneKind::PageView))
            )),
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

    /// FX38d: at a window of 1280 × 828 points the bottom dock's display clamp leaves the page view at least half the
    /// window's height; the user's shares stay the layout.
    #[test]
    fn the_bottom_clamp_leaves_the_page_view_half_the_window() {
        let layout =
            ew_viewmodel::layout::Layout::schematic().with_bottom(PaneKind::Navigator(NavKind::Messages));
        let mut dock = Dock::new(&layout, 0);
        let ctx = egui::Context::default();
        for _ in 0..3 {
            frame(&ctx, &mut dock, 1280.0, 828.0);
        }
        let page = shown_height(&dock, PaneKind::PageView);
        let messages = shown_height(&dock, PaneKind::Navigator(NavKind::Messages));
        assert!(page >= 828.0 / 2.0 - 1.0, "page view {page}, Messages {messages}");
        assert!(
            messages > 0.35 * (page + messages) - 1.0,
            "Messages {messages} keeps its own share"
        );
        assert_eq!(dock.layout().as_ref(), Some(&layout));
    }
}
