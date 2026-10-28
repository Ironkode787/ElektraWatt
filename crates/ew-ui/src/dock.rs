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

    /// Paints the tree; `pane_ui` paints one pane.
    pub fn show(&mut self, ui: &mut Ui, pane_ui: &mut dyn FnMut(&mut Ui, PaneKind)) {
        let mut behavior = DockBehavior { pane_ui };
        self.tree.ui(&mut behavior, ui);
    }
}

struct DockBehavior<'a> {
    pane_ui: &'a mut dyn FnMut(&mut Ui, PaneKind),
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
}

#[cfg(test)]
mod tests {
    use ew_viewmodel::layout::{LayoutNode, PaneKind, SplitDir};

    use super::*;

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
