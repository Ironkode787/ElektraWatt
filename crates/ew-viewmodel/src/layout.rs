// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dock layout of the project view (WP-S2.01 part 2, FT-309; UX/UI doc 16 section 2.6 and 3.4): a tree of
//! panes in splits and tab groups, toolkit-neutral. `ew-ui` converts it to and from its egui_tiles tree; the
//! workspace files (`workspaces/<name>.json`) hold it as JSON.
//!
//! A file is `{"$schema":"ew.workspace/1","name":"Schematic","root":<node>}`; a node is `{"pane":"pages"}`,
//! `{"tabs":[<node>,...],"active":0}` or `{"split":"horizontal","children":[{"share":200,"node":<node>},...]}`
//! (a `horizontal` split puts its children side by side). The shares are positive integers in any scale; only
//! their ratios count. The text is deterministic: members sorted, no floating point, so a workspace file saved
//! twice is the same bytes. This is not the serialised egui_tiles tree that doc 16 section 2.5 names: that tree
//! keeps its tiles in a hash map, whose order would reach the file.
//!
//! The Schematic workspace (WP-S2.17 part 3, D10) is the left column (the navigator group over the property grid) and
//! the page view; the other navigators join the navigator group from the left rail, the bottom panes open below the
//! page view. A workspace file written before WP-S2.17 loads as saved, its panes in their places; View ▸ Properties
//! adds the grid under the Pages pane's column.

use serde_json::{Map, Value, json};

use crate::navigators::NavKind;
use crate::strings;

/// The key of a navigator pane's title in [`crate::strings`] (WP-S2.06 §6.8).
pub fn nav_title_key(kind: NavKind) -> &'static str {
    match kind {
        NavKind::Pages => "pane-pages",
        NavKind::Devices => "nav-title-devices",
        NavKind::Connections => "nav-title-connections",
        NavKind::Strips => "nav-title-strips",
        NavKind::Cables => "nav-title-cables",
        NavKind::Plc => "nav-title-plc",
        NavKind::Potentials => "nav-title-potentials",
        NavKind::Ips => "nav-title-ips",
        NavKind::Structure => "nav-title-structure",
        NavKind::Parts => "nav-title-parts",
        NavKind::Blocks => "nav-title-blocks",
        NavKind::Macros => "nav-title-macros",
        NavKind::Libraries => "nav-title-libraries",
        NavKind::Messages => "nav-title-messages",
    }
}

/// The `$schema` of a workspace file.
pub const SCHEMA: &str = "ew.workspace/1";

/// What a pane shows.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PaneKind {
    /// The page list.
    Pages,
    /// The page view (the canvas).
    PageView,
    /// The property grid (WP-S2.05).
    Properties,
    /// The find pane (WP-S2.05 part 2): opened by Ctrl+F or Ctrl+H below the page view.
    Find,
    // WP-S2.06 part 1
    /// A navigator other than Pages (§6.8): docked as a tab beside the Pages pane; its workspace key is the kind word.
    Navigator(NavKind),
    // WP-S2.09 part 1
    /// The Jobs pane (§6.4): the PDF exports with their progress and results, in the bottom dock (D14).
    Jobs,
    // WP-S2.09 part 3
    /// The Preview tab (§8.1): the open card's preview, in the bottom dock (D11, D14).
    Preview,
    /// The history panel (§8.4): the undo steps and the marker lines, in the bottom dock (D13, D14).
    History,
}

impl PaneKind {
    /// Every pane kind but the navigators.
    pub const ALL: [PaneKind; 7] = [
        PaneKind::Pages,
        PaneKind::PageView,
        PaneKind::Properties,
        PaneKind::Find,
        PaneKind::Jobs,
        PaneKind::Preview,
        PaneKind::History,
    ];

    /// The panes of the bottom dock (D14): they share one tab group below the page view (WP-S2.09 part 2: the
    /// Messages navigator is one of them; part 3: the Preview tab and the history panel).
    pub const BOTTOM: [PaneKind; 5] = [
        PaneKind::Find,
        PaneKind::Jobs,
        PaneKind::Navigator(NavKind::Messages),
        PaneKind::Preview,
        PaneKind::History,
    ];

    /// The name in a workspace file.
    pub fn key(self) -> &'static str {
        match self {
            PaneKind::Pages => "pages",
            PaneKind::PageView => "page-view",
            PaneKind::Properties => "properties",
            PaneKind::Find => "find",
            PaneKind::Navigator(k) => k.word(),
            PaneKind::Jobs => "jobs",
            PaneKind::Preview => "preview",
            PaneKind::History => "history",
        }
    }

    /// The pane of a name in a workspace file.
    pub fn from_key(key: &str) -> Option<PaneKind> {
        PaneKind::ALL.into_iter().find(|p| p.key() == key).or_else(|| {
            NavKind::from_word(key)
                .filter(|k| *k != NavKind::Pages)
                .map(PaneKind::Navigator)
        })
    }

    /// The key of the pane's title in [`crate::strings`].
    pub fn title_key(self) -> &'static str {
        match self {
            PaneKind::Pages => "pane-pages",
            PaneKind::PageView => "pane-page-view",
            PaneKind::Properties => "pane-properties",
            PaneKind::Find => "pane-find",
            PaneKind::Navigator(k) => nav_title_key(k),
            PaneKind::Jobs => "pane-jobs",
            PaneKind::Preview => "pane-preview",
            PaneKind::History => "pane-history",
        }
    }
}

/// The direction of a split.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitDir {
    /// Children side by side.
    Horizontal,
    /// Children one above the other.
    Vertical,
}

impl SplitDir {
    fn key(self) -> &'static str {
        match self {
            SplitDir::Horizontal => "horizontal",
            SplitDir::Vertical => "vertical",
        }
    }
}

/// One node of the layout tree.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LayoutNode {
    /// A pane.
    Pane(PaneKind),
    /// Tabs: the children share one area, `active` is shown.
    Tabs {
        /// The tabs.
        children: Vec<LayoutNode>,
        /// The index of the shown tab.
        active: usize,
    },
    /// A split into areas with relative shares.
    Split {
        /// The direction.
        dir: SplitDir,
        /// The areas: the share and what is in it.
        children: Vec<(u32, LayoutNode)>,
    },
}

impl LayoutNode {
    /// Whether `pane` is in this subtree.
    pub fn holds(&self, pane: PaneKind) -> bool {
        match self {
            LayoutNode::Pane(p) => *p == pane,
            LayoutNode::Tabs { children, .. } => children.iter().any(|c| c.holds(pane)),
            LayoutNode::Split { children, .. } => children.iter().any(|(_, c)| c.holds(pane)),
        }
    }
}

/// Why a workspace text is not a layout.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LayoutError {
    /// Not JSON, or not the shape of a workspace file; the text says where.
    Malformed(String),
    /// A pane appears twice, the page view is missing, or the tree is too deep or too large.
    Invalid(String),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::Malformed(m) | LayoutError::Invalid(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for LayoutError {}

/// The deepest nesting a layout file may have.
const MAX_DEPTH: usize = 8;

/// The most nodes a layout file may have.
const MAX_NODES: usize = 32;

/// A workspace layout.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Layout {
    /// The root of the tree.
    pub root: LayoutNode,
}

impl Layout {
    /// The Schematic workspace (WP-S2.17 part 3, D10; doc 16a §5.1): the left column holds the navigator group (the
    /// Pages pane, driven by the left rail) over the property grid (500 : 500), the page view takes the rest (233 : 767
    /// of the dock's width, so 280 of the 1200 px a 1280-px window leaves between the rails); the bottom dock and the
    /// right column are closed. The shares are written in thousandths, as the dock reads them back.
    pub fn schematic() -> Layout {
        Layout {
            root: LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    (
                        233,
                        LayoutNode::Split {
                            dir: SplitDir::Vertical,
                            children: vec![
                                (500, LayoutNode::Pane(PaneKind::Pages)),
                                (500, LayoutNode::Pane(PaneKind::Properties)),
                            ],
                        },
                    ),
                    (767, LayoutNode::Pane(PaneKind::PageView)),
                ],
            },
        }
    }

    /// The layout with the property grid (View ▸ Properties): under the left column that holds the Pages pane
    /// (500 : 500; WP-S2.17 part 3, D10), else appended to a horizontal root split, else the root and the grid side by
    /// side (800 : 200). Unchanged when the grid is in it.
    pub fn with_properties(&self) -> Layout {
        if self.panes().contains(&PaneKind::Properties) {
            return self.clone();
        }
        let root = match &self.root {
            LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children,
            } => {
                let mut children = children.clone();
                match children.iter().position(|(_, c)| c.holds(PaneKind::Pages)) {
                    Some(i) => {
                        let column = children[i].1.clone();
                        children[i].1 = LayoutNode::Split {
                            dir: SplitDir::Vertical,
                            children: vec![(500, column), (500, LayoutNode::Pane(PaneKind::Properties))],
                        };
                    }
                    None => children.push((200, LayoutNode::Pane(PaneKind::Properties))),
                }
                LayoutNode::Split {
                    dir: SplitDir::Horizontal,
                    children,
                }
            }
            other => LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    (800, other.clone()),
                    (200, LayoutNode::Pane(PaneKind::Properties)),
                ],
            },
        };
        Layout { root }
    }

    // WP-S2.17 part 3
    /// The pane shown in the place of `pane`: the active tab of the tab group that holds it, else `pane` itself;
    /// `None` when `pane` is not in the layout (the rails' active entry, doc 16a §5.4).
    pub fn shown_in_place_of(&self, pane: PaneKind) -> Option<PaneKind> {
        fn walk(n: &LayoutNode, pane: PaneKind) -> Option<PaneKind> {
            match n {
                LayoutNode::Pane(p) => (*p == pane).then_some(pane),
                LayoutNode::Tabs { children, active } => {
                    if children.contains(&LayoutNode::Pane(pane)) {
                        return match children.get(*active) {
                            Some(LayoutNode::Pane(p)) => Some(*p),
                            _ => Some(pane),
                        };
                    }
                    children.iter().find_map(|c| walk(c, pane))
                }
                LayoutNode::Split { children, .. } => children.iter().find_map(|(_, c)| walk(c, pane)),
            }
        }
        walk(&self.root, pane)
    }

    // WP-S2.05 part 2
    /// The layout with the find pane below the page view (Ctrl+F, Ctrl+H; the bottom dock of doc 16 §3.4): see
    /// [`Layout::with_bottom`].
    pub fn with_find(&self) -> Layout {
        self.with_bottom(PaneKind::Find)
    }

    // WP-S2.09 part 1
    /// The layout with `pane` in the bottom dock (D14; Find, Jobs): beside the bottom pane that is in the layout
    /// already, as a new shown tab of its tab group (its place becomes tabs, or its tabs get one more); with none, the
    /// page view's place becomes a vertical split of the page view and `pane` (650 : 350; W31-1), which the first of them
    /// creates. Unchanged when `pane` is in it.
    pub fn with_bottom(&self, pane: PaneKind) -> Layout {
        fn below(n: &LayoutNode, pane: PaneKind) -> LayoutNode {
            match n {
                LayoutNode::Pane(PaneKind::PageView) => LayoutNode::Split {
                    dir: SplitDir::Vertical,
                    children: vec![
                        (650, LayoutNode::Pane(PaneKind::PageView)),
                        (350, LayoutNode::Pane(pane)),
                    ],
                },
                LayoutNode::Pane(p) => LayoutNode::Pane(*p),
                LayoutNode::Tabs { children, active } => LayoutNode::Tabs {
                    children: children.iter().map(|c| below(c, pane)).collect(),
                    active: *active,
                },
                LayoutNode::Split { dir, children } => LayoutNode::Split {
                    dir: *dir,
                    children: children.iter().map(|(s, c)| (*s, below(c, pane))).collect(),
                },
            }
        }
        /// `pane` as a tab beside the first bottom pane met; `None` when the subtree has none.
        fn beside(n: &LayoutNode, pane: PaneKind) -> Option<LayoutNode> {
            match n {
                LayoutNode::Pane(p) if PaneKind::BOTTOM.contains(p) => Some(LayoutNode::Tabs {
                    children: vec![LayoutNode::Pane(*p), LayoutNode::Pane(pane)],
                    active: 1,
                }),
                LayoutNode::Pane(_) => None,
                LayoutNode::Tabs { children, .. }
                    if children
                        .iter()
                        .any(|c| matches!(c, LayoutNode::Pane(p) if PaneKind::BOTTOM.contains(p))) =>
                {
                    let mut children = children.clone();
                    children.push(LayoutNode::Pane(pane));
                    let active = children.len() - 1;
                    Some(LayoutNode::Tabs { children, active })
                }
                LayoutNode::Tabs { children, active } => {
                    let mut out = children.clone();
                    for (i, c) in children.iter().enumerate() {
                        if let Some(new) = beside(c, pane) {
                            out[i] = new;
                            return Some(LayoutNode::Tabs {
                                children: out,
                                active: *active,
                            });
                        }
                    }
                    None
                }
                LayoutNode::Split { dir, children } => {
                    let mut out = children.clone();
                    for (i, (share, c)) in children.iter().enumerate() {
                        if let Some(new) = beside(c, pane) {
                            out[i] = (*share, new);
                            return Some(LayoutNode::Split {
                                dir: *dir,
                                children: out,
                            });
                        }
                    }
                    None
                }
            }
        }
        if self.panes().contains(&pane) {
            return self.clone();
        }
        if let Some(root) = beside(&self.root, pane) {
            return Layout { root };
        }
        Layout {
            root: below(&self.root, pane),
        }
    }

    // WP-S2.06 part 1
    /// The layout with a navigator pane (View ▸ Navigators, §6.8): the Pages pane at the left of a horizontal root
    /// split (160 of the shares), another navigator as a new tab beside the Pages pane (the Pages pane's place becomes
    /// tabs, or the tabs it is in get one more), shown; without a Pages pane, at the left like Pages. Unchanged when
    /// the pane is in it.
    pub fn with_navigator(&self, pane: PaneKind) -> Layout {
        fn put(n: &LayoutNode, pane: PaneKind) -> Option<LayoutNode> {
            match n {
                LayoutNode::Pane(PaneKind::Pages) => Some(LayoutNode::Tabs {
                    children: vec![LayoutNode::Pane(PaneKind::Pages), LayoutNode::Pane(pane)],
                    active: 1,
                }),
                LayoutNode::Pane(_) => None,
                LayoutNode::Tabs { children, .. }
                    if children.contains(&LayoutNode::Pane(PaneKind::Pages)) =>
                {
                    let mut children = children.clone();
                    children.push(LayoutNode::Pane(pane));
                    let active = children.len() - 1;
                    Some(LayoutNode::Tabs { children, active })
                }
                LayoutNode::Tabs { children, active } => {
                    let mut out = children.clone();
                    for (i, c) in children.iter().enumerate() {
                        if let Some(new) = put(c, pane) {
                            out[i] = new;
                            return Some(LayoutNode::Tabs {
                                children: out,
                                active: *active,
                            });
                        }
                    }
                    None
                }
                LayoutNode::Split { dir, children } => {
                    let mut out = children.clone();
                    for (i, (share, c)) in children.iter().enumerate() {
                        if let Some(new) = put(c, pane) {
                            out[i] = (*share, new);
                            return Some(LayoutNode::Split {
                                dir: *dir,
                                children: out,
                            });
                        }
                    }
                    None
                }
            }
        }
        if self.panes().contains(&pane) {
            return self.clone();
        }
        if pane != PaneKind::Pages
            && let Some(root) = put(&self.root, pane)
        {
            return Layout { root };
        }
        let root = match &self.root {
            LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children,
            } => {
                let mut children = children.clone();
                children.insert(0, (160, LayoutNode::Pane(pane)));
                LayoutNode::Split {
                    dir: SplitDir::Horizontal,
                    children,
                }
            }
            other => LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![(160, LayoutNode::Pane(pane)), (840, other.clone())],
            },
        };
        Layout { root }
    }

    // WP-S2.09 part 3
    /// The layout with `pane` shown: the tab group that holds it shows it (D11: opening a card shows the Preview tab).
    /// Unchanged when `pane` is not in it or is already shown.
    pub fn showing(&self, pane: PaneKind) -> Layout {
        fn show(n: &LayoutNode, pane: PaneKind) -> LayoutNode {
            match n {
                LayoutNode::Pane(p) => LayoutNode::Pane(*p),
                LayoutNode::Tabs { children, active } => {
                    let at = children
                        .iter()
                        .position(|c| matches!(c, LayoutNode::Pane(p) if *p == pane));
                    LayoutNode::Tabs {
                        children: children.iter().map(|c| show(c, pane)).collect(),
                        active: at.unwrap_or(*active),
                    }
                }
                LayoutNode::Split { dir, children } => LayoutNode::Split {
                    dir: *dir,
                    children: children.iter().map(|(s, c)| (*s, show(c, pane))).collect(),
                },
            }
        }
        Layout {
            root: show(&self.root, pane),
        }
    }

    /// The layout without `pane`: a container left with one child is replaced by it, an empty one goes. The page view
    /// is never removed.
    pub fn without(&self, pane: PaneKind) -> Layout {
        fn strip(n: &LayoutNode, pane: PaneKind) -> Option<LayoutNode> {
            match n {
                LayoutNode::Pane(p) if *p == pane => None,
                LayoutNode::Pane(p) => Some(LayoutNode::Pane(*p)),
                LayoutNode::Tabs { children, active } => {
                    let kept: Vec<LayoutNode> = children.iter().filter_map(|c| strip(c, pane)).collect();
                    match kept.len() {
                        0 => None,
                        1 => kept.into_iter().next(),
                        n => Some(LayoutNode::Tabs {
                            children: kept,
                            active: (*active).min(n - 1),
                        }),
                    }
                }
                LayoutNode::Split { dir, children } => {
                    let kept: Vec<(u32, LayoutNode)> = children
                        .iter()
                        .filter_map(|(s, c)| strip(c, pane).map(|c| (*s, c)))
                        .collect();
                    match kept.len() {
                        0 => None,
                        1 => kept.into_iter().next().map(|(_, c)| c),
                        _ => Some(LayoutNode::Split {
                            dir: *dir,
                            children: kept,
                        }),
                    }
                }
            }
        }
        if pane == PaneKind::PageView {
            return self.clone();
        }
        Layout {
            root: strip(&self.root, pane).unwrap_or(LayoutNode::Pane(PaneKind::PageView)),
        }
    }

    /// The panes of the tree in layout order.
    pub fn panes(&self) -> Vec<PaneKind> {
        fn walk(n: &LayoutNode, out: &mut Vec<PaneKind>) {
            match n {
                LayoutNode::Pane(p) => out.push(*p),
                LayoutNode::Tabs { children, .. } => children.iter().for_each(|c| walk(c, out)),
                LayoutNode::Split { children, .. } => children.iter().for_each(|(_, c)| walk(c, out)),
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }

    fn check(&self) -> Result<(), LayoutError> {
        let panes = self.panes();
        for (i, p) in panes.iter().enumerate() {
            if panes[i + 1..].contains(p) {
                return Err(LayoutError::Invalid(strings::format(
                    "layout-pane-twice",
                    &[p.key()],
                )));
            }
        }
        if !panes.contains(&PaneKind::PageView) {
            return Err(LayoutError::Invalid(
                strings::text("layout-no-page-view").to_owned(),
            ));
        }
        Ok(())
    }

    /// The JSON text of the workspace `name`: one line, members sorted, with a final newline.
    pub fn to_json(&self, name: &str) -> String {
        fn node(n: &LayoutNode) -> Value {
            match n {
                LayoutNode::Pane(p) => json!({ "pane": p.key() }),
                LayoutNode::Tabs { children, active } => {
                    json!({ "tabs": children.iter().map(node).collect::<Vec<_>>(), "active": active })
                }
                LayoutNode::Split { dir, children } => json!({
                    "split": dir.key(),
                    "children": children
                        .iter()
                        .map(|(share, c)| json!({ "share": share, "node": node(c) }))
                        .collect::<Vec<_>>(),
                }),
            }
        }
        let value = json!({ "$schema": SCHEMA, "name": name, "root": node(&self.root) });
        format!("{value}\n")
    }

    /// Reads a workspace file: the layout and its name.
    ///
    /// # Errors
    ///
    /// [`LayoutError`] for text that is not a workspace file or a layout that is not usable.
    pub fn parse(text: &str) -> Result<(String, Layout), LayoutError> {
        let bad = |key: &str| LayoutError::Malformed(strings::text(key).to_owned());
        let value: Value = serde_json::from_str(text)
            .map_err(|e| LayoutError::Malformed(strings::format("layout-not-json", &[&e.to_string()])))?;
        let Value::Object(mut file) = value else {
            return Err(bad("layout-not-object"));
        };
        if file.get("$schema").and_then(Value::as_str) != Some(SCHEMA) {
            return Err(bad("layout-schema"));
        }
        let name = file
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("layout-name-missing"))?
            .to_owned();
        let root = file.remove("root").ok_or_else(|| bad("layout-root-missing"))?;
        let mut count = 0;
        let root = read_node(&root, 1, &mut count)?;
        let layout = Layout { root };
        layout.check()?;
        Ok((name, layout))
    }

    /// The layout as one line of text for `ui.describe`: `split horizontal [pages 20%, page view 80%]`. The
    /// structural words are fixed English; the pane titles are user text.
    pub fn describe(&self) -> String {
        fn node(n: &LayoutNode) -> String {
            match n {
                LayoutNode::Pane(p) => format!("\"{}\"", strings::text(p.title_key())),
                LayoutNode::Tabs { children, active } => {
                    let items: Vec<String> = children
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            if i == *active {
                                format!("{} (shown)", node(c))
                            } else {
                                node(c)
                            }
                        })
                        .collect();
                    format!("tabs [{}]", items.join(", "))
                }
                LayoutNode::Split { dir, children } => {
                    let total: u64 = children.iter().map(|(s, _)| u64::from(*s)).sum::<u64>().max(1);
                    let items: Vec<String> = children
                        .iter()
                        .map(|(s, c)| format!("{} {}%", node(c), (u64::from(*s) * 100 + total / 2) / total))
                        .collect();
                    format!("split {} [{}]", dir.key(), items.join(", "))
                }
            }
        }
        node(&self.root)
    }
}

fn read_node(v: &Value, depth: usize, count: &mut usize) -> Result<LayoutNode, LayoutError> {
    let bad = |key: &str| LayoutError::Malformed(strings::text(key).to_owned());
    *count += 1;
    if depth > MAX_DEPTH {
        return Err(LayoutError::Invalid(strings::text("layout-too-deep").to_owned()));
    }
    if *count > MAX_NODES {
        return Err(LayoutError::Invalid(strings::text("layout-too-many").to_owned()));
    }
    let Value::Object(obj) = v else {
        return Err(bad("layout-node-object"));
    };
    let known = |obj: &Map<String, Value>, allowed: &[&str]| -> Result<(), LayoutError> {
        match obj.keys().find(|k| !allowed.contains(&k.as_str())) {
            Some(k) => Err(LayoutError::Malformed(strings::format(
                "layout-unknown-member",
                &[k],
            ))),
            None => Ok(()),
        }
    };
    if let Some(p) = obj.get("pane") {
        known(obj, &["pane"])?;
        let key = p.as_str().ok_or_else(|| bad("layout-pane-name"))?;
        return PaneKind::from_key(key)
            .map(LayoutNode::Pane)
            .ok_or_else(|| LayoutError::Malformed(strings::format("layout-unknown-pane", &[key])));
    }
    if let Some(t) = obj.get("tabs") {
        known(obj, &["tabs", "active"])?;
        let items = t.as_array().ok_or_else(|| bad("layout-tabs-list"))?;
        if items.is_empty() {
            return Err(bad("layout-tabs-empty"));
        }
        let children = items
            .iter()
            .map(|c| read_node(c, depth + 1, count))
            .collect::<Result<Vec<_>, _>>()?;
        let active = obj
            .get("active")
            .and_then(Value::as_u64)
            .and_then(|a| usize::try_from(a).ok())
            .unwrap_or(0)
            .min(children.len() - 1);
        return Ok(LayoutNode::Tabs { children, active });
    }
    if let Some(s) = obj.get("split") {
        known(obj, &["split", "children"])?;
        let dir = match s.as_str() {
            Some("horizontal") => SplitDir::Horizontal,
            Some("vertical") => SplitDir::Vertical,
            _ => return Err(bad("layout-split-dir")),
        };
        let items = obj
            .get("children")
            .and_then(Value::as_array)
            .ok_or_else(|| bad("layout-children-list"))?;
        if items.is_empty() {
            return Err(bad("layout-children-empty"));
        }
        let mut children = Vec::new();
        for item in items {
            let Value::Object(item) = item else {
                return Err(bad("layout-child-object"));
            };
            known(item, &["share", "node"])?;
            let share = item
                .get("share")
                .and_then(Value::as_u64)
                .and_then(|s| u32::try_from(s).ok())
                .filter(|s| *s > 0)
                .ok_or_else(|| bad("layout-share"))?;
            let node = item.get("node").ok_or_else(|| bad("layout-node-missing"))?;
            children.push((share, read_node(node, depth + 1, count)?));
        }
        return Ok(LayoutNode::Split { dir, children });
    }
    Err(bad("layout-node-kind"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schematic_workspace_round_trips_and_describes_itself() {
        let layout = Layout::schematic();
        let text = layout.to_json("Schematic");
        assert!(text.ends_with('\n') && text.matches('\n').count() == 1);
        let (name, back) = Layout::parse(&text).unwrap();
        assert_eq!(name, "Schematic");
        assert_eq!(back, layout);
        assert_eq!(back.to_json("Schematic"), text, "the same bytes twice");
        assert_eq!(
            layout.describe(),
            "split horizontal [split vertical [\"Pages\" 50%, \"Properties\" 50%] 23%, \"Page view\" 77%]"
        );
    }

    #[test]
    fn the_property_grid_is_added_and_removed() {
        let full = Layout::schematic();
        let bare = full.without(PaneKind::Properties);
        assert_eq!(bare.panes(), [PaneKind::Pages, PaneKind::PageView]);
        assert_eq!(bare.with_properties(), full);
        assert_eq!(full.with_properties(), full);
        assert_eq!(full.without(PaneKind::PageView), full);
        let tabbed = Layout {
            root: LayoutNode::Tabs {
                children: vec![
                    LayoutNode::Pane(PaneKind::PageView),
                    LayoutNode::Pane(PaneKind::Properties),
                ],
                active: 1,
            },
        };
        assert_eq!(
            tabbed.without(PaneKind::Properties).root,
            LayoutNode::Pane(PaneKind::PageView)
        );
        assert_eq!(
            Layout {
                root: LayoutNode::Pane(PaneKind::PageView)
            }
            .with_properties()
            .panes(),
            [PaneKind::PageView, PaneKind::Properties]
        );
    }

    #[test]
    fn tabs_and_nested_splits_round_trip() {
        let layout = Layout {
            root: LayoutNode::Split {
                dir: SplitDir::Vertical,
                children: vec![
                    (
                        3,
                        LayoutNode::Tabs {
                            children: vec![
                                LayoutNode::Pane(PaneKind::PageView),
                                LayoutNode::Pane(PaneKind::Pages),
                            ],
                            active: 1,
                        },
                    ),
                    (
                        1,
                        LayoutNode::Split {
                            dir: SplitDir::Horizontal,
                            children: vec![],
                        },
                    ),
                ],
            },
        };
        // An empty split is not a layout; the writer does not check, the reader does.
        assert!(Layout::parse(&layout.to_json("x")).is_err());
        let layout = Layout {
            root: LayoutNode::Tabs {
                children: vec![
                    LayoutNode::Pane(PaneKind::PageView),
                    LayoutNode::Pane(PaneKind::Pages),
                ],
                active: 1,
            },
        };
        let (_, back) = Layout::parse(&layout.to_json("Tabbed")).unwrap();
        assert_eq!(back, layout);
        assert_eq!(back.describe(), "tabs [\"Page view\", \"Pages\" (shown)]");
    }

    // WP-S2.05 part 2
    #[test]
    fn the_find_pane_goes_below_the_page_view_and_round_trips() {
        let full = Layout::schematic().with_find();
        assert_eq!(
            full.describe(),
            "split horizontal [split vertical [\"Pages\" 50%, \"Properties\" 50%] 23%, split vertical [\"Page view\" 65%, \"Find and replace\" 35%] 77%]"
        );
        assert_eq!(full.with_find(), full, "added once");
        assert_eq!(Layout::parse(&full.to_json("Find")).unwrap().1, full);
        assert_eq!(full.without(PaneKind::Find), Layout::schematic());
    }

    // WP-S2.09 part 1
    #[test]
    fn the_bottom_panes_share_one_tab_group_below_the_page_view() {
        let jobs = Layout::schematic().with_bottom(PaneKind::Jobs);
        assert_eq!(
            jobs.describe(),
            "split horizontal [split vertical [\"Pages\" 50%, \"Properties\" 50%] 23%, split vertical [\"Page view\" 65%, \"Jobs\" 35%] 77%]"
        );
        let both = jobs.with_find();
        assert_eq!(
            both.describe(),
            "split horizontal [split vertical [\"Pages\" 50%, \"Properties\" 50%] 23%, split vertical [\"Page view\" 65%, tabs [\"Jobs\", \"Find and replace\" (shown)] 35%] 77%]"
        );
        assert_eq!(both.with_bottom(PaneKind::Jobs), both, "added once");
        assert_eq!(Layout::parse(&both.to_json("Bottom")).unwrap().1, both);
        assert_eq!(both.without(PaneKind::Jobs), Layout::schematic().with_find());
        assert_eq!(PaneKind::from_key("jobs"), Some(PaneKind::Jobs));
    }

    #[test]
    fn bad_files_are_refused_with_a_reason() {
        let page_view = r#"{"pane":"page-view"}"#;
        let file = |root: &str| format!(r#"{{"$schema":"ew.workspace/1","name":"x","root":{root}}}"#);
        assert!(Layout::parse("not json").is_err());
        assert!(Layout::parse(r#"{"name":"x","root":{}}"#).is_err());
        assert!(Layout::parse(&file(page_view)).is_ok());
        for (root, why) in [
            (r#"{"pane":"pages"}"#, "the page view is missing"),
            (
                r#"{"tabs":[{"pane":"page-view"},{"pane":"page-view"}]}"#,
                "appears twice",
            ),
            // `find` is a pane since WP-S2.05 part 2, `messages` since WP-S2.09 part 2.
            (r#"{"pane":"no-such-pane"}"#, "unknown pane"),
            (r#"{"pane":"pages","extra":1}"#, "unknown member"),
            (r#"{"tabs":[]}"#, "empty"),
            (
                r#"{"split":"horizontal","children":[{"share":0,"node":{"pane":"page-view"}}]}"#,
                "positive",
            ),
            (r#"{"split":"diagonal","children":[]}"#, "horizontal or vertical"),
            (r#"{}"#, "`pane`, `tabs` or `split`"),
        ] {
            let err = Layout::parse(&file(root)).unwrap_err().to_string();
            assert!(err.contains(why), "{root}: {err}");
        }
        // Too deep.
        let mut deep = page_view.to_owned();
        for _ in 0..9 {
            deep = format!(r#"{{"tabs":[{deep}]}}"#);
        }
        assert!(
            Layout::parse(&file(&deep))
                .unwrap_err()
                .to_string()
                .contains("deeply")
        );
    }

    #[test]
    fn the_active_tab_is_clamped() {
        let text =
            r#"{"$schema":"ew.workspace/1","name":"x","root":{"tabs":[{"pane":"page-view"}],"active":7}}"#;
        let (_, layout) = Layout::parse(text).unwrap();
        assert!(matches!(layout.root, LayoutNode::Tabs { active: 0, .. }));
    }
}
