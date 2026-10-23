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
//! The panes of the Schematic workspace that exist today are the page list, the page view and the property grid
//! (WP-S2.05); the navigators and the message panels join with their work packages (WP-S2.06 to WP-S2.09). A saved
//! workspace without the property grid loads as saved; View ▸ Properties adds it to the right.

use serde_json::{Map, Value, json};

use crate::strings;

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
}

impl PaneKind {
    /// Every pane kind.
    pub const ALL: [PaneKind; 4] = [
        PaneKind::Pages,
        PaneKind::PageView,
        PaneKind::Properties,
        PaneKind::Find,
    ];

    /// The name in a workspace file.
    pub fn key(self) -> &'static str {
        match self {
            PaneKind::Pages => "pages",
            PaneKind::PageView => "page-view",
            PaneKind::Properties => "properties",
            PaneKind::Find => "find",
        }
    }

    /// The pane of a name in a workspace file.
    pub fn from_key(key: &str) -> Option<PaneKind> {
        PaneKind::ALL.into_iter().find(|p| p.key() == key)
    }

    /// The key of the pane's title in [`crate::strings`].
    pub fn title_key(self) -> &'static str {
        match self {
            PaneKind::Pages => "pane-pages",
            PaneKind::PageView => "pane-page-view",
            PaneKind::Properties => "pane-properties",
            PaneKind::Find => "pane-find",
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
    /// The Schematic workspace: the page list on the left, the page view in the centre, the property grid on the right
    /// (shares 200 : 800 : 250, written in thousandths as the dock reads them back; doc 16 section 2.6; its other docks
    /// come with their panels).
    pub fn schematic() -> Layout {
        Layout {
            root: LayoutNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    (160, LayoutNode::Pane(PaneKind::Pages)),
                    (640, LayoutNode::Pane(PaneKind::PageView)),
                    (200, LayoutNode::Pane(PaneKind::Properties)),
                ],
            },
        }
    }

    /// The layout with the property grid on the right (View ▸ Properties): appended to a horizontal root split,
    /// else the root and the grid side by side (800 : 250). Unchanged when the grid is in it.
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
                children.push((200, LayoutNode::Pane(PaneKind::Properties)));
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

    // WP-S2.05 part 2
    /// The layout with the find pane below the page view (Ctrl+F, Ctrl+H; the bottom dock of doc 16 §3.4): the page
    /// view's place becomes a vertical split of the page view and the find pane (750 : 250). Unchanged when the find
    /// pane is in it.
    pub fn with_find(&self) -> Layout {
        fn put(n: &LayoutNode) -> LayoutNode {
            match n {
                LayoutNode::Pane(PaneKind::PageView) => LayoutNode::Split {
                    dir: SplitDir::Vertical,
                    children: vec![
                        (750, LayoutNode::Pane(PaneKind::PageView)),
                        (250, LayoutNode::Pane(PaneKind::Find)),
                    ],
                },
                LayoutNode::Pane(p) => LayoutNode::Pane(*p),
                LayoutNode::Tabs { children, active } => LayoutNode::Tabs {
                    children: children.iter().map(put).collect(),
                    active: *active,
                },
                LayoutNode::Split { dir, children } => LayoutNode::Split {
                    dir: *dir,
                    children: children.iter().map(|(s, c)| (*s, put(c))).collect(),
                },
            }
        }
        if self.panes().contains(&PaneKind::Find) {
            return self.clone();
        }
        Layout {
            root: put(&self.root),
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
            "split horizontal [\"Pages\" 16%, \"Page view\" 64%, \"Properties\" 20%]"
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
            "split horizontal [\"Pages\" 16%, split vertical [\"Page view\" 75%, \"Find and replace\" 25%] 64%, \"Properties\" 20%]"
        );
        assert_eq!(full.with_find(), full, "added once");
        assert_eq!(Layout::parse(&full.to_json("Find")).unwrap().1, full);
        assert_eq!(full.without(PaneKind::Find), Layout::schematic());
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
            // The message panel is not a pane yet (WP-S2.09); `find` is one since WP-S2.05 part 2.
            (r#"{"pane":"messages"}"#, "unknown pane"),
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
