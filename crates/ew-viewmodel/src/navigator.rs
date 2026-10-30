// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page navigator (WP-S2.03 part 1, spec section 6.1; FR-PAG-024, the viewing part): the pages of the derived
//! page order as a flat list, by section, or by structure (function, location, section), sorted by folio (page
//! order) or by page type, with a filter and a row cursor. It replaces WP-S2.01's page list in the `pages` pane.
//!
//! The model reads the rows of `Session::page_rows`, which the shell fetches once per revision; the trees are
//! derived from them on demand. **Node keys** are path strings, unique in every mode because a location or section
//! node repeats under different parents: `f:<text>`, `f:<text>/l:<text>`, `f:<text>/l:<text>/s:<n>` in structure
//! mode (the `(none)` nodes `f:-` and `l:-`, front matter `s:front`), `s:<n>` and `s:front` in section mode; a page
//! row is `<parent key>/p:<page uuid>`, or `p:<page uuid>` in flat mode. Nodes are ordered by their first page in
//! the chosen sort. With a filter, a row matches when the filter, lower-cased, is a substring of its folio or title
//! lower-cased; parents of a match stay shown and are expanded.

use std::collections::BTreeSet;

use ew_api::output::PageRow;

use crate::canvas::PageId;
use crate::strings;

/// The three views of the navigator.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NavMode {
    /// One level: the pages.
    Flat,
    /// A node per section (front matter first), then the pages.
    #[default]
    Section,
    /// Function nodes, location nodes inside them, section nodes inside those, then the pages.
    Structure,
}

impl NavMode {
    /// The three modes in menu order.
    pub const ALL: [NavMode; 3] = [NavMode::Flat, NavMode::Section, NavMode::Structure];

    /// The mode's word (`flat`, `section`, `structure`): the `pages.mode` parameter and the describe text.
    pub fn word(self) -> &'static str {
        match self {
            NavMode::Flat => "flat",
            NavMode::Section => "section",
            NavMode::Structure => "structure",
        }
    }

    /// The mode of a word.
    pub fn from_word(w: &str) -> Option<NavMode> {
        NavMode::ALL.into_iter().find(|m| m.word() == w)
    }
}

/// The two sort orders of the navigator.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NavSort {
    /// Page order.
    #[default]
    Folio,
    /// Page type (the schema's declaration order), then page order.
    Type,
}

impl NavSort {
    /// The two orders in menu order.
    pub const ALL: [NavSort; 2] = [NavSort::Folio, NavSort::Type];

    /// The order's word (`folio`, `type`): the `pages.sort` parameter and the describe text.
    pub fn word(self) -> &'static str {
        match self {
            NavSort::Folio => "folio",
            NavSort::Type => "type",
        }
    }

    /// The order of a word.
    pub fn from_word(w: &str) -> Option<NavSort> {
        NavSort::ALL.into_iter().find(|s| s.word() == w)
    }
}

/// What a node groups.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeAspect {
    /// A section (or front matter).
    Section,
    /// A function (`=`).
    Function,
    /// A location (`+`).
    Location,
}

impl NodeAspect {
    /// The aspect's accessible word.
    pub fn word(self) -> &'static str {
        match self {
            NodeAspect::Section => "section",
            NodeAspect::Function => "function",
            NodeAspect::Location => "location",
        }
    }
}

/// What a navigator row shows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RowKind {
    /// A group node.
    Node {
        /// Its label (`Front matter`, `Section 1 - Wiring Definition`, `=P01`, `(none)`).
        label: String,
        /// What it groups.
        aspect: NodeAspect,
    },
    /// A page: the index of its row in the page rows.
    Page(usize),
}

/// One visible row of the navigator, in display order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavRow {
    /// The row's key (see the module documentation).
    pub key: String,
    /// The key of its parent node, `None` at the top level.
    pub parent: Option<String>,
    /// The depth, from 1.
    pub level: usize,
    /// What it shows.
    pub kind: RowKind,
    /// Whether a node shows its children (always `false` for a page).
    pub expanded: bool,
    /// The 1-based position among its siblings.
    pub position: usize,
    /// The number of its siblings, itself included.
    pub set_size: usize,
}

/// The state of the page navigator.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PageNavigator {
    /// The view.
    pub mode: NavMode,
    /// The sort order.
    pub sort: NavSort,
    /// The filter text.
    pub filter: String,
    /// The keys of the collapsed nodes (every node starts expanded).
    collapsed: BTreeSet<String>,
    /// The row cursor's key, when the user moved it; else the current page's row.
    row: Option<String>,
    // WP-S2.06 part 1
    /// The keys of the marked rows (multi-selection, §6.4).
    pub(crate) marked: BTreeSet<String>,
    /// The key a Shift range starts from.
    pub(crate) anchor: Option<String>,
    /// The selection generation the navigator last applied or followed (the generation guard, §6.5).
    pub(crate) applied: Option<u64>,
}

/// One node of the full tree.
struct Node {
    key: String,
    label: String,
    aspect: NodeAspect,
    nodes: Vec<Node>,
    pages: Vec<usize>,
}

impl Node {
    fn child(&mut self, key: String, label: String, aspect: NodeAspect) -> &mut Node {
        let at = match self.nodes.iter().position(|n| n.key == key) {
            Some(i) => i,
            None => {
                self.nodes.push(Node {
                    key,
                    label,
                    aspect,
                    nodes: Vec::new(),
                    pages: Vec::new(),
                });
                self.nodes.len() - 1
            }
        };
        &mut self.nodes[at]
    }
}

/// The key of a page row under `parent`.
fn page_key(parent: &str, row: &PageRow) -> String {
    let id = PageId::of(row).uuid_text();
    if parent.is_empty() {
        format!("p:{id}")
    } else {
        format!("{parent}/p:{id}")
    }
}

impl PageNavigator {
    /// A navigator in its default state: section view, sorted by folio, no filter, every node expanded.
    pub fn new() -> PageNavigator {
        PageNavigator::default()
    }

    /// Whether the node `key` shows its children (a filter expands every node it shows).
    pub fn is_expanded(&self, key: &str) -> bool {
        !self.filter.is_empty() || !self.collapsed.contains(key)
    }

    /// Collapses an expanded node or expands a collapsed one.
    pub fn toggle(&mut self, key: &str) {
        if !self.collapsed.remove(key) {
            self.collapsed.insert(key.to_owned());
        }
    }

    /// Puts the row cursor on `key`.
    pub fn set_row(&mut self, key: &str) {
        self.row = Some(key.to_owned());
    }

    /// Puts the row cursor back on the current page.
    pub fn reset_row(&mut self) {
        self.row = None;
    }

    /// The page order positions of `rows` in the chosen sort.
    fn sorted(&self, rows: &[PageRow]) -> Vec<usize> {
        let mut order: Vec<usize> = (0..rows.len()).collect();
        if self.sort == NavSort::Type {
            order.sort_by_key(|&i| (rows[i].kind_rank, i));
        }
        order
    }

    /// Whether a page row passes the filter: the framework's filter (WP-S2.06 §6.3) over the row's label
    /// `<folio> <title>`, its tokens and its facets (`page:<folio>`, `kind:page`). A substring of the folio or of the
    /// title is a substring of the label, so the as-built matches are kept (D6).
    fn matches(&self, row: &PageRow, query: &crate::navigators::Query) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let label = format!("{} {}", row.folio, row.title);
        let mut tokens = ew_api::output::nav_tokens(&label);
        tokens.sort();
        tokens.dedup();
        let facets = [
            ("page".to_owned(), row.folio.to_lowercase()),
            ("kind".to_owned(), "page".to_owned()),
        ];
        query.matches(&tokens, &facets, &label, &[])
    }

    /// The full tree of the rows the filter keeps.
    fn tree(&self, rows: &[PageRow]) -> Node {
        let mut root = Node {
            key: String::new(),
            label: String::new(),
            aspect: NodeAspect::Section,
            nodes: Vec::new(),
            pages: Vec::new(),
        };
        let section = |parent: &str, row: &PageRow| -> (String, String) {
            let (key, label) = match &row.section {
                None => ("s:front".to_owned(), strings::text("nav-front-matter").to_owned()),
                Some((n, title)) => (
                    format!("s:{n}"),
                    strings::format("nav-section", &[&n.to_string(), title]),
                ),
            };
            let key = if parent.is_empty() {
                key
            } else {
                format!("{parent}/{key}")
            };
            (key, label)
        };
        let none = || strings::text("nav-none").to_owned();
        let query = crate::navigators::Query::parse(&self.filter);
        for i in self.sorted(rows) {
            let row = &rows[i];
            if !self.matches(row, &query) {
                continue;
            }
            match self.mode {
                NavMode::Flat => root.pages.push(i),
                NavMode::Section => {
                    let (key, label) = section("", row);
                    root.child(key, label, NodeAspect::Section).pages.push(i);
                }
                NavMode::Structure => {
                    let (fk, fl) = match &row.function {
                        Some(f) => (format!("f:{f}"), f.clone()),
                        None => ("f:-".to_owned(), none()),
                    };
                    let (lk, ll) = match &row.location {
                        Some(l) => (format!("{fk}/l:{l}"), l.clone()),
                        None => (format!("{fk}/l:-"), none()),
                    };
                    let (sk, sl) = section(&lk, row);
                    root.child(fk, fl, NodeAspect::Function)
                        .child(lk, ll, NodeAspect::Location)
                        .child(sk, sl, NodeAspect::Section)
                        .pages
                        .push(i);
                }
            }
        }
        root
    }

    /// The rows on show, in display order: the nodes and pages the filter keeps, children of collapsed nodes left
    /// out.
    pub fn visible(&self, rows: &[PageRow]) -> Vec<NavRow> {
        let mut out = Vec::new();
        self.flatten(&self.tree(rows), rows, 1, None, &mut out);
        out
    }

    fn flatten(
        &self,
        node: &Node,
        rows: &[PageRow],
        level: usize,
        parent: Option<&str>,
        out: &mut Vec<NavRow>,
    ) {
        let size = node.nodes.len() + node.pages.len();
        for (k, child) in node.nodes.iter().enumerate() {
            let expanded = self.is_expanded(&child.key);
            out.push(NavRow {
                key: child.key.clone(),
                parent: parent.map(str::to_owned),
                level,
                kind: RowKind::Node {
                    label: child.label.clone(),
                    aspect: child.aspect,
                },
                expanded,
                position: k + 1,
                set_size: size,
            });
            if expanded {
                self.flatten(child, rows, level + 1, Some(&child.key), out);
            }
        }
        for (k, &i) in node.pages.iter().enumerate() {
            out.push(NavRow {
                key: page_key(parent.unwrap_or(""), &rows[i]),
                parent: parent.map(str::to_owned),
                level,
                kind: RowKind::Page(i),
                expanded: false,
                position: node.nodes.len() + k + 1,
                set_size: size,
            });
        }
    }

    /// The key of the row cursor among `visible`: the row the user moved it to while it is on show, else the row of
    /// `current` (the focused tab's page), else the first row.
    pub fn row_key(&self, visible: &[NavRow], rows: &[PageRow], current: Option<PageId>) -> Option<String> {
        if let Some(k) = &self.row
            && visible.iter().any(|r| &r.key == k)
        {
            return Some(k.clone());
        }
        current
            .and_then(|c| {
                visible
                    .iter()
                    .find(|r| matches!(r.kind, RowKind::Page(i) if PageId::of(&rows[i]) == c))
            })
            .or_else(|| visible.first())
            .map(|r| r.key.clone())
    }

    /// The accessible name of a row (WP-S2.03 §6.1): a page `"<folio> <title>, page, <badges>"` (the badges
    /// `generated`, `QET grid` and `current`, comma-separated), a node `"<label>, <section|function|location>"`. The
    /// level, the position in its set, the set size and the expanded state are AccessKit properties, not words.
    pub fn accessible_name(row: &NavRow, rows: &[PageRow], current: Option<PageId>) -> String {
        match &row.kind {
            RowKind::Node { label, aspect } => strings::format("nav-node-name", &[label, aspect.word()]),
            RowKind::Page(i) => {
                let r = &rows[*i];
                let mut badges = String::new();
                if r.generated {
                    badges.push_str(&format!(", {}", strings::text("badge-generated")));
                }
                if r.qet_grid {
                    badges.push_str(&format!(", {}", strings::text("badge-qet-grid")));
                }
                if current == Some(PageId::of(r)) {
                    badges.push_str(&format!(", {}", strings::text("badge-current")));
                }
                strings::format("nav-page-name", &[&r.folio, &r.title, &badges])
            }
        }
    }

    /// The key the user moved the row cursor to, while that row is on show; `None` while the cursor follows the
    /// current page (the transcript's `navigator.row`, so that a fresh project's page IDs reach no transcript until a
    /// script names a row itself).
    pub fn moved_row(&self, visible: &[NavRow]) -> Option<String> {
        self.row.clone().filter(|k| visible.iter().any(|r| &r.key == k))
    }

    // WP-S2.06 part 1
    /// The key of page row `i` in the current view (as [`PageNavigator::visible`] would key it without a filter).
    pub fn key_of(&self, rows: &[PageRow], i: usize) -> String {
        let row = &rows[i];
        let section = match &row.section {
            None => "s:front".to_owned(),
            Some((n, _)) => format!("s:{n}"),
        };
        match self.mode {
            NavMode::Flat => page_key("", row),
            NavMode::Section => page_key(&section, row),
            NavMode::Structure => {
                let f = row
                    .function
                    .as_deref()
                    .map_or_else(|| "f:-".to_owned(), |f| format!("f:{f}"));
                let l = row
                    .location
                    .as_deref()
                    .map_or_else(|| format!("{f}/l:-"), |l| format!("{f}/l:{l}"));
                page_key(&format!("{l}/{section}"), row)
            }
        }
    }

    /// The marked keys.
    pub fn marked(&self) -> &BTreeSet<String> {
        &self.marked
    }

    /// Whether a row is marked.
    pub fn is_marked(&self, key: &str) -> bool {
        self.marked.contains(key)
    }

    /// The row index of the page a row key names (`…/p:<uuid>`).
    pub fn page_of(key: &str, rows: &[PageRow]) -> Option<usize> {
        let id = key.rsplit('/').next()?.strip_prefix("p:")?;
        rows.iter().position(|r| PageId::of(r).uuid_text() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        n: u128,
        folio: &str,
        title: &str,
        kind_rank: u16,
        section: Option<u32>,
        f: Option<&str>,
    ) -> PageRow {
        PageRow {
            id: std::str::FromStr::from_str(&format!("00000000-0000-0000-0000-{n:012x}")).unwrap(),
            folio: folio.into(),
            title: title.into(),
            kind: "multi_line".into(),
            kind_rank,
            generated: false,
            section: section.map(|s| (s, format!("S{s}"))),
            function: f.map(str::to_owned),
            location: None,
            qet_grid: true,
        }
    }

    fn rows() -> Vec<PageRow> {
        vec![
            row(1, "1", "Title", 0, None, None),
            row(2, "101", "Power", 5, Some(1), Some("=P01")),
            row(3, "102", "Control", 5, Some(1), Some("=P02")),
            row(4, "201", "Overview", 6, Some(2), Some("=P01")),
            row(5, "901", "Wire list", 13, Some(9), None),
        ]
    }

    fn keys(v: &[NavRow]) -> Vec<&str> {
        v.iter().map(|r| r.key.as_str()).collect()
    }

    #[test]
    fn the_three_views_and_their_keys() {
        let rows = rows();
        let mut n = PageNavigator::new();
        let v = n.visible(&rows);
        assert_eq!(v.len(), 4 + 5, "front matter and three sections, five pages");
        assert_eq!(v[0].key, "s:front");
        assert_eq!(v[1].key, "s:front/p:00000000-0000-0000-0000-000000000001");
        assert_eq!((v[1].level, v[1].position, v[1].set_size), (2, 1, 1));
        assert_eq!((v[0].position, v[0].set_size), (1, 4));
        n.mode = NavMode::Flat;
        let v = n.visible(&rows);
        assert_eq!(v.len(), 5);
        assert!(v.iter().all(|r| r.level == 1 && r.key.starts_with("p:")));
        n.mode = NavMode::Structure;
        let v = n.visible(&rows);
        // =P01 holds 101 and 201 (sections 1 and 2); (none) holds 1 and 901.
        assert_eq!(
            v[0].key, "f:-",
            "ordered by the first page: page 1 has no function"
        );
        assert!(keys(&v).contains(&"f:=P01/l:-/s:2"));
        assert!(keys(&v).contains(&"f:-/l:-/s:front"));
        assert!(keys(&v).contains(&"f:=P02/l:-/s:1/p:00000000-0000-0000-0000-000000000003"));
        let unique: BTreeSet<&str> = keys(&v).into_iter().collect();
        assert_eq!(unique.len(), v.len(), "keys are unique");
    }

    #[test]
    fn sort_filter_collapse_and_the_row_cursor() {
        let rows = rows();
        let mut n = PageNavigator::new();
        n.mode = NavMode::Flat;
        n.sort = NavSort::Type;
        let folios: Vec<&str> = n
            .visible(&rows)
            .iter()
            .map(|r| match r.kind {
                RowKind::Page(i) => rows[i].folio.as_str(),
                RowKind::Node { .. } => "",
            })
            .collect();
        assert_eq!(folios, ["1", "101", "102", "201", "901"]);
        n.mode = NavMode::Section;
        n.toggle("s:1");
        assert!(!keys(&n.visible(&rows)).iter().any(|k| k.starts_with("s:1/")));
        // A filter keeps the ancestors of a match and expands them.
        n.filter = "CONTR".into();
        let v = n.visible(&rows);
        assert_eq!(keys(&v), ["s:1", "s:1/p:00000000-0000-0000-0000-000000000003"]);
        n.filter.clear();
        n.toggle("s:1");
        // The cursor follows the current page until moved.
        let v = n.visible(&rows);
        let cur = PageId::of(&rows[2]);
        assert_eq!(
            n.row_key(&v, &rows, Some(cur)).as_deref(),
            Some("s:1/p:00000000-0000-0000-0000-000000000003")
        );
        n.set_row("s:2");
        assert_eq!(n.row_key(&v, &rows, Some(cur)).as_deref(), Some("s:2"));
        assert_eq!(
            PageNavigator::page_of("s:1/p:00000000-0000-0000-0000-000000000003", &rows),
            Some(2)
        );
        assert_eq!(PageNavigator::page_of("s:1", &rows), None);
    }
}
