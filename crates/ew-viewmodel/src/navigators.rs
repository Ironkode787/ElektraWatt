// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigator framework (WP-S2.06 part 1, §6.2 to §6.5; D5, D6, D7): the state of an open navigator over its
//! [`NavTable`], the visible rows (a depth-first walk over the expanded nodes, or the flat list), the filter over the
//! precomputed tokens and facets, the sort, and the marked rows.
//!
//! The Pages navigator keeps its as-built tree (`crate::navigator`, WP-S2.03 §6.1) with its keys, actions and
//! describe lines (D5); it shares this module's filter ([`Query`]), the marks and the selection synchronisation. Every
//! other navigator is a [`Navigator`] over the table that `Session::navigator_table` builds, fetched once per
//! (kind, revision) while the navigator is open.
//!
//! **Visible rows** are rebuilt only when the table, the view, the sort, the filter or an expansion changes, and are
//! cached per those inputs. Siblings sort by the natural key of the label (or of the sort column's cell), ties by node
//! order; descending reverses the key order, ties still by node order. In the flat view the rows are the table rows
//! (for Devices the device nodes), one level.
//!
//! **Filter** (§6.3). The query is split at whitespace: a part `<facet>:<value>` with a facet of the closed list
//! (`page`, `potential`, `state`, `kind`, `part`) matches a node with that facet and a case-folded equal value;
//! `unplaced` and `frozen` alone match the flag (in the PLC navigator also `unassigned`, `spare`, `tbd` and `duplicate`,
//! and the facet `config:<name>`, WP-S2.07 §8); every other part is plain text, split into tokens like the node tokens
//! (`ew_api::output::nav_tokens`). A node matches when every facet and flag matches, and either every plain token is a
//! prefix of one of its tokens or the plain text, case-folded, is a substring of its case-folded label. Matches keep
//! their ancestors, shown and expanded.

// WP-S2.09 part 2
pub mod messages;
pub mod paste;
pub mod pattern;
pub mod table;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

pub use ew_api::output::{Drawable, NaturalKey, NavKind, NavNode, NavRole, NavTable};

/// The facets of the closed list that take a value (§6.1 item 5).
pub const FACETS: [&str; 7] = ["page", "potential", "state", "kind", "part", "strip", "config"];

/// The flags that a query word alone matches.
pub const FLAGS: [&str; 2] = ["unplaced", "frozen"];

/// The flags that a query word alone matches in the PLC navigator besides [`FLAGS`] (WP-S2.07 §8, FR-PLC-021): a
/// channel without an address, a spare channel, an open address, and a duplicate symbol or overlapping address. In
/// the other navigators these words stay plain text.
pub const PLC_FLAGS: [&str; 4] = ["unassigned", "spare", "tbd", "duplicate"];

/// The two views of a navigator other than Pages.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NavView {
    /// The tree.
    #[default]
    Tree,
    /// The table rows, one level.
    Flat,
}

impl NavView {
    /// The view's word (`tree`, `flat`).
    pub fn word(self) -> &'static str {
        match self {
            NavView::Tree => "tree",
            NavView::Flat => "flat",
        }
    }

    /// The view of a word.
    pub fn from_word(w: &str) -> Option<NavView> {
        [NavView::Tree, NavView::Flat].into_iter().find(|v| v.word() == w)
    }
}

/// What a navigator's siblings sort by.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum SortBy {
    /// The label.
    #[default]
    Label,
    /// A column, by its key.
    Column(String),
    /// The provider's node order (WP-S2.07 D2: the Terminal strips navigator's mounting order, its default).
    Order,
}

impl SortBy {
    /// The default sort of a navigator kind: `Order` for Terminal strips (WP-S2.07 D2), else the label.
    pub fn default_for(kind: NavKind) -> SortBy {
        if kind == NavKind::Strips {
            SortBy::Order
        } else {
            SortBy::Label
        }
    }
}

/// A parsed filter query (§6.3).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Query {
    /// `(facet, value)` conditions, the value case-folded.
    pub facets: Vec<(String, String)>,
    /// Flags that must be set.
    pub flags: Vec<String>,
    /// The plain tokens.
    pub tokens: Vec<String>,
    /// The sub-tokens of each typed plain word that splits into more than one (`24VDC-4` gives `24`, `vdc`, `4`):
    /// each such group must match within one cell of a row (FT-663, §6.3).
    pub words: Vec<Vec<String>>,
    /// The plain text, case-folded and trimmed (the substring alternative).
    pub text: String,
}

impl Query {
    /// The query of a filter text in navigator `kind`: [`Query::parse`], with the PLC navigator's own flags
    /// ([`PLC_FLAGS`]).
    pub fn parse_for(kind: NavKind, filter: &str) -> Query {
        let extra: &[&str] = if kind == NavKind::Plc { &PLC_FLAGS } else { &[] };
        Query::parse_with(filter, extra)
    }

    /// The query of a filter text.
    pub fn parse(filter: &str) -> Query {
        Query::parse_with(filter, &[])
    }

    /// The query of a filter text with `extra` flags beside [`FLAGS`].
    fn parse_with(filter: &str, extra: &[&str]) -> Query {
        let mut q = Query::default();
        let mut plain: Vec<&str> = Vec::new();
        for part in filter.split_whitespace() {
            let lower = part.to_lowercase();
            if let Some((facet, value)) = lower.split_once(':')
                && FACETS.contains(&facet)
                && !value.is_empty()
            {
                q.facets.push((facet.to_owned(), value.to_owned()));
                continue;
            }
            if FLAGS.contains(&lower.as_str()) || extra.contains(&lower.as_str()) {
                q.flags.push(lower);
                continue;
            }
            plain.push(part);
        }
        let text = plain.join(" ");
        q.tokens = ew_api::output::nav_tokens(&text);
        q.words = plain
            .iter()
            .map(|w| ew_api::output::nav_tokens(w))
            .filter(|w| w.len() > 1)
            .collect();
        q.text = text.to_lowercase().trim().to_owned();
        q
    }

    /// Whether the query is empty (everything matches).
    pub fn is_empty(&self) -> bool {
        self.facets.is_empty() && self.flags.is_empty() && self.tokens.is_empty() && self.text.is_empty()
    }

    /// Whether a node with these tokens (sorted), facets, label and cells matches (§6.3): every facet and flag
    /// holds, and either every query token is a prefix of one of the node's tokens, the sub-tokens of one typed word
    /// all matching within one cell (the label or one column's value, FT-663), or the plain text is a substring of
    /// the label.
    pub fn matches(
        &self,
        tokens: &[String],
        facets: &[(String, String)],
        label: &str,
        cells: &[String],
    ) -> bool {
        let facet_ok = self
            .facets
            .iter()
            .all(|(f, v)| facets.iter().any(|(nf, nv)| nf == f && nv.to_lowercase() == *v));
        let flags_ok = self.flags.iter().all(|f| facets.iter().any(|(nf, _)| nf == f));
        if !(facet_ok && flags_ok) {
            return false;
        }
        if self.tokens.is_empty() && self.text.is_empty() {
            return true;
        }
        let prefixes = !self.tokens.is_empty()
            && self.tokens.iter().all(|t| has_prefix(tokens, t))
            && (self.words.is_empty() || self.words_in_one_cell(label, cells));
        prefixes || (!self.text.is_empty() && label.to_lowercase().contains(&self.text))
    }

    /// Whether every multi-token typed word has all its sub-tokens within one cell: the label or one cell.
    fn words_in_one_cell(&self, label: &str, cells: &[String]) -> bool {
        let cell_tokens: Vec<Vec<String>> = std::iter::once(label)
            .chain(cells.iter().map(String::as_str))
            .map(|c| {
                let mut t = ew_api::output::nav_tokens(c);
                t.sort();
                t.dedup();
                t
            })
            .collect();
        self.words
            .iter()
            .all(|w| cell_tokens.iter().any(|c| w.iter().all(|t| has_prefix(c, t))))
    }
}

/// Whether one of the sorted `tokens` starts with `t`.
fn has_prefix(tokens: &[String], t: &str) -> bool {
    // The tokens are sorted: the first token not below `t` is the only candidate prefix run.
    let at = tokens.partition_point(|n| n.as_str() < t);
    tokens.get(at).is_some_and(|n| n.starts_with(t))
}

/// One visible row of a navigator other than Pages, in display order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VisRow {
    /// The node's index in the table.
    pub node: u32,
    /// The depth, from 1.
    pub level: usize,
    /// `Some(expanded)` for a node with children on show, `None` for a leaf.
    pub expanded: Option<bool>,
    /// The 1-based position among its siblings on show.
    pub position: usize,
    /// The number of its siblings on show, itself included.
    pub set_size: usize,
}

/// The inputs the visible rows depend on.
#[derive(Clone, PartialEq, Eq, Debug)]
struct VisKey {
    table: usize,
    rev: u64,
    view: NavView,
    sort: SortBy,
    descending: bool,
    filter: String,
    collapsed: BTreeSet<String>,
    table_mode: bool,
}

/// The cache of the visible rows (not part of the state's equality).
#[derive(Default)]
struct VisCache(RefCell<Option<(VisKey, Arc<Vec<VisRow>>)>>);

impl Clone for VisCache {
    fn clone(&self) -> VisCache {
        VisCache::default()
    }
}

impl std::fmt::Debug for VisCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VisCache")
    }
}

/// What a navigator's rows on show depend on: a widget rebuilds its rows only when it changed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavStamp {
    table: Option<(usize, u64)>,
    view: NavView,
    sort: SortBy,
    descending: bool,
    filter: String,
    collapsed: BTreeSet<String>,
    marked: BTreeSet<String>,
    cursor: Option<String>,
    table_mode: Option<table::TableMode>,
}

/// The state of an open navigator other than Pages (§6.2).
#[derive(Clone, Debug)]
pub struct Navigator {
    /// Which navigator.
    pub kind: NavKind,
    /// The view.
    pub view: NavView,
    /// What siblings sort by.
    pub sort: SortBy,
    /// Whether the order is descending.
    pub descending: bool,
    /// The filter text.
    pub filter: String,
    /// The keys of the collapsed nodes (every node starts expanded).
    pub(crate) collapsed: BTreeSet<String>,
    /// The keys of the marked rows.
    pub(crate) marked: BTreeSet<String>,
    /// The row cursor's key.
    pub(crate) cursor: Option<String>,
    /// The key a Shift range starts from.
    pub(crate) anchor: Option<String>,
    /// The table of the current revision.
    pub(crate) table: Option<Arc<NavTable>>,
    /// The selection generation this navigator last applied or followed (the generation guard, §6.5).
    pub(crate) applied: Option<u64>,
    /// Table mode (WP-S2.06 part 3, §8): Some while the navigator shows its table.
    pub(crate) table_mode: Option<table::TableMode>,
    cache: VisCache,
    /// How many times the visible rows were built (tests).
    pub(crate) rebuilds: std::cell::Cell<u64>,
}

impl Navigator {
    /// A navigator of `kind` in its default state: tree view, by label ascending, no filter, every node expanded.
    pub fn new(kind: NavKind) -> Navigator {
        Navigator {
            kind,
            view: NavView::Tree,
            sort: SortBy::default_for(kind),
            descending: false,
            filter: String::new(),
            collapsed: BTreeSet::new(),
            marked: BTreeSet::new(),
            cursor: None,
            anchor: None,
            table: None,
            applied: None,
            table_mode: None,
            cache: VisCache::default(),
            rebuilds: std::cell::Cell::new(0),
        }
    }

    /// The table on show (`None` before the first fetch).
    pub fn table(&self) -> Option<&Arc<NavTable>> {
        self.table.as_ref()
    }

    /// The marked keys.
    pub fn marked(&self) -> &BTreeSet<String> {
        &self.marked
    }

    /// Table mode's state, while the navigator shows its table (§8).
    pub fn table_mode(&self) -> Option<&table::TableMode> {
        self.table_mode.as_ref()
    }

    /// The cursor cell on show in table mode: (row key, column index).
    pub fn cursor_cell(&self) -> Option<(String, usize)> {
        let mode = self.table_mode.as_ref()?;
        let table = self.table.as_ref()?;
        Some((self.cursor_key()?, mode.cursor_column(table)?))
    }

    /// What the rows on show depend on.
    pub fn stamp(&self) -> NavStamp {
        NavStamp {
            table: self.table.as_ref().map(|t| (Arc::as_ptr(t) as usize, t.rev)),
            view: self.view,
            sort: self.sort.clone(),
            descending: self.descending,
            filter: self.filter.clone(),
            collapsed: self.collapsed.clone(),
            marked: self.marked.clone(),
            cursor: self.cursor.clone(),
            table_mode: self.table_mode.clone(),
        }
    }

    /// How many times the visible rows were built (tests: scrolling and repainting build none).
    pub fn rebuilds(&self) -> u64 {
        self.rebuilds.get()
    }

    /// The visible rows, cached per (table, view, sort, filter, expansion).
    pub fn visible(&self) -> Arc<Vec<VisRow>> {
        let Some(table) = &self.table else {
            return Arc::new(Vec::new());
        };
        let key = VisKey {
            table: Arc::as_ptr(table) as usize,
            rev: table.rev,
            view: self.view,
            sort: self.sort.clone(),
            descending: self.descending,
            filter: self.filter.clone(),
            collapsed: self.collapsed.clone(),
            table_mode: self.table_mode.is_some(),
        };
        if let Some((k, rows)) = &*self.cache.0.borrow()
            && *k == key
        {
            return Arc::clone(rows);
        }
        let query = Query::parse_for(self.kind, &self.filter);
        let rows = Arc::new(if self.table_mode.is_some() {
            table::table_rows(table, &self.sort, self.descending, &query)
        } else {
            visible_rows(
                table,
                self.view,
                &self.sort,
                self.descending,
                &query,
                &self.collapsed,
            )
        });
        self.rebuilds.set(self.rebuilds.get() + 1);
        *self.cache.0.borrow_mut() = Some((key, Arc::clone(&rows)));
        rows
    }

    /// The key of the row cursor among the visible rows: the row it was put on while it is on show, else the first.
    pub fn cursor_key(&self) -> Option<String> {
        let table = self.table.as_ref()?;
        let rows = self.visible();
        if let Some(k) = &self.cursor
            && rows.iter().any(|r| table.nodes[r.node as usize].key == *k)
        {
            return Some(k.clone());
        }
        rows.first().map(|r| table.nodes[r.node as usize].key.clone())
    }

    /// The node of a key.
    pub fn node_of(&self, key: &str) -> Option<(u32, &NavNode)> {
        let table = self.table.as_ref()?;
        table
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.key == key)
            .map(|(i, n)| (u32::try_from(i).unwrap_or(u32::MAX), n))
    }

    /// The label the `sort "<label>"` of the describe text names: the column's label, else `Tag` for Devices and
    /// `Label` for the others.
    pub fn sort_label(&self) -> String {
        match &self.sort {
            SortBy::Column(c) => self
                .table
                .as_ref()
                .and_then(|t| t.columns.iter().find(|x| x.key == *c))
                .map_or_else(|| c.clone(), |x| x.label.clone()),
            SortBy::Label if self.kind == NavKind::Devices => "Tag".to_owned(),
            SortBy::Label => "Label".to_owned(),
            SortBy::Order => "Order".to_owned(),
        }
    }

    /// Expands every ancestor of node `i`.
    pub(crate) fn reveal(&mut self, i: u32) {
        let Some(table) = self.table.clone() else { return };
        let mut p = table.nodes.get(i as usize).and_then(|n| n.parent);
        while let Some(at) = p {
            self.collapsed.remove(&table.nodes[at as usize].key);
            p = table.nodes[at as usize].parent;
        }
    }
}

/// The visible rows of `table` (§6.2): a pure function, so that a test can run it on a synthetic table of any size.
pub fn visible_rows(
    table: &NavTable,
    view: NavView,
    sort: &SortBy,
    descending: bool,
    query: &Query,
    collapsed: &BTreeSet<String>,
) -> Vec<VisRow> {
    let n = table.nodes.len();
    let col = match sort {
        SortBy::Label | SortBy::Order => None,
        SortBy::Column(c) => table.columns.iter().position(|x| x.key == *c),
    };
    let by_order = *sort == SortBy::Order;
    let sort_key = |i: usize| -> Option<&NaturalKey> {
        let node = &table.nodes[i];
        match col {
            Some(c) => node.natural.get(c + 1),
            None => node.natural.first(),
        }
    };
    let order = |v: &mut Vec<usize>| {
        v.sort_by(|&a, &b| {
            // `Order`: the node order alone (ascending or descending).
            let k = if by_order {
                std::cmp::Ordering::Equal
            } else {
                sort_key(a).cmp(&sort_key(b))
            };
            let k = if by_order && descending {
                a.cmp(&b).reverse()
            } else {
                k
            };
            let k = if descending { k.reverse() } else { k };
            k.then(a.cmp(&b))
        });
    };
    let filtering = !query.is_empty();
    let hit: Vec<bool> = if filtering {
        table
            .nodes
            .iter()
            .map(|x| query.matches(&x.tokens, &x.facets, &x.label, &x.cells))
            .collect()
    } else {
        Vec::new()
    };
    if view == NavView::Flat {
        let mut rows: Vec<usize> = (0..n)
            .filter(|&i| {
                let node = &table.nodes[i];
                let flat = if table.kind == NavKind::Devices {
                    node.role == NavRole::Device
                } else {
                    node.table_row
                };
                flat && (!filtering || hit[i])
            })
            .collect();
        order(&mut rows);
        let size = rows.len();
        return rows
            .into_iter()
            .enumerate()
            .map(|(k, i)| VisRow {
                node: u32::try_from(i).unwrap_or(u32::MAX),
                level: 1,
                expanded: None,
                position: k + 1,
                set_size: size,
            })
            .collect();
    }
    // Which nodes are on show: every node without a filter; with one, the matches and their ancestors.
    let mut shown = vec![!filtering; n];
    if filtering {
        for (i, _) in hit.iter().enumerate().filter(|(_, h)| **h) {
            {
                let mut at = Some(i);
                while let Some(j) = at {
                    if shown[j] {
                        break;
                    }
                    shown[j] = true;
                    at = table.nodes[j].parent.map(|p| p as usize);
                }
            }
        }
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut roots: Vec<usize> = Vec::new();
    for (i, _) in shown.iter().enumerate().filter(|(_, s)| **s) {
        match table.nodes[i].parent {
            Some(p) => children[p as usize].push(i),
            None => roots.push(i),
        }
    }
    let mut out = Vec::with_capacity(n);
    // An explicit stack instead of recursion: a deep synthetic tree must not overflow.
    order(&mut roots);
    let mut stack: Vec<(Vec<usize>, usize, usize)> = vec![(roots, 0, 1)];
    while let Some((list, at, level)) = stack.pop() {
        if at >= list.len() {
            continue;
        }
        let i = list[at];
        let size = list.len();
        let has = !children[i].is_empty();
        let expanded = filtering || !collapsed.contains(&table.nodes[i].key);
        out.push(VisRow {
            node: u32::try_from(i).unwrap_or(u32::MAX),
            level,
            expanded: has.then_some(expanded),
            position: at + 1,
            set_size: size,
        });
        stack.push((list, at + 1, level));
        if has && expanded {
            let mut kids = std::mem::take(&mut children[i]);
            order(&mut kids);
            stack.push((kids, 0, level + 1));
        }
    }
    out
}

/// Whether a label starts with a type-ahead prefix (case-folded).
pub fn starts_with_folded(label: &str, prefix: &str) -> bool {
    label.to_lowercase().starts_with(&prefix.to_lowercase())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use ew_api::output::{NavColumn, nav_tokens};

    fn node(key: &str, parent: Option<u32>, role: NavRole, label: &str, facets: &[(&str, &str)]) -> NavNode {
        let mut tokens = nav_tokens(label);
        tokens.sort();
        tokens.dedup();
        NavNode {
            key: key.to_owned(),
            parent,
            role,
            label: label.to_owned(),
            badges: Vec::new(),
            records: Vec::new(),
            kinds: Vec::new(),
            define_at: None,
            drawables: Vec::new(),
            open: None,
            cells: Vec::new(),
            table_row: role == NavRole::Device,
            tokens,
            facets: facets
                .iter()
                .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
                .collect(),
            natural: vec![ew_api::output::nav_natural_key(label)],
        }
    }

    fn table(nodes: Vec<NavNode>) -> NavTable {
        NavTable {
            kind: NavKind::Devices,
            rev: 1,
            columns: Vec::<NavColumn>::new(),
            nodes,
            index: BTreeMap::new(),
        }
    }

    fn fixture() -> NavTable {
        table(vec![
            node("function:=P01", None, NavRole::Function, "=P01", &[]),
            node(
                "function:=P01/device:b",
                Some(0),
                NavRole::Device,
                "-102K10",
                &[("state", "spare")],
            ),
            node(
                "function:=P01/device:a",
                Some(0),
                NavRole::Device,
                "-102K2",
                &[("page", "102")],
            ),
            node(
                "function:=P01/device:c",
                Some(0),
                NavRole::Device,
                "-201U3",
                &[("unplaced", "")],
            ),
            node(
                "function:=P01/device:c/function:f",
                Some(3),
                NavRole::Function,
                "coil",
                &[],
            ),
        ])
    }

    fn labels(t: &NavTable, rows: &[VisRow]) -> Vec<String> {
        rows.iter()
            .map(|r| t.nodes[r.node as usize].label.clone())
            .collect()
    }

    fn show(t: &NavTable, filter: &str) -> Vec<String> {
        labels(
            t,
            &visible_rows(
                t,
                NavView::Tree,
                &SortBy::Label,
                false,
                &Query::parse(filter),
                &BTreeSet::new(),
            ),
        )
    }

    #[test]
    fn natural_order_collapse_and_flat() {
        let t = fixture();
        assert_eq!(show(&t, ""), ["=P01", "-102K2", "-102K10", "-201U3", "coil"]);
        let mut collapsed = BTreeSet::new();
        collapsed.insert("function:=P01/device:c".to_owned());
        let rows = visible_rows(
            &t,
            NavView::Tree,
            &SortBy::Label,
            false,
            &Query::default(),
            &collapsed,
        );
        assert_eq!(labels(&t, &rows), ["=P01", "-102K2", "-102K10", "-201U3"]);
        assert_eq!(rows[3].expanded, Some(false));
        assert_eq!((rows[1].level, rows[1].position, rows[1].set_size), (2, 1, 3));
        let flat = visible_rows(
            &t,
            NavView::Flat,
            &SortBy::Label,
            true,
            &Query::default(),
            &collapsed,
        );
        assert_eq!(labels(&t, &flat), ["-201U3", "-102K10", "-102K2"], "descending");
    }

    #[test]
    fn the_filter_prefix_substring_facets_and_flags() {
        let t = fixture();
        // Every token a prefix: `102k` splits into `102` and `k`.
        assert_eq!(show(&t, "102k"), ["=P01", "-102K2", "-102K10"]);
        // The substring of the label as the alternative: `02` prefixes no token of -102K10, but `02k1` is in its label.
        assert_eq!(show(&t, "02k1"), ["=P01", "-102K10"]);
        assert_eq!(show(&t, "page:102"), ["=P01", "-102K2"]);
        assert_eq!(show(&t, "state:spare"), ["=P01", "-102K10"]);
        assert_eq!(show(&t, "unplaced"), ["=P01", "-201U3"]);
        // An unknown facet is a plain token: `colour:red` splits into `colour` and `red`.
        assert_eq!(show(&t, "colour:red"), Vec::<String>::new());
        // A match deep in the tree shows and expands its ancestors.
        let mut collapsed = BTreeSet::new();
        collapsed.insert("function:=P01/device:c".to_owned());
        let rows = visible_rows(
            &t,
            NavView::Tree,
            &SortBy::Label,
            false,
            &Query::parse("coil"),
            &collapsed,
        );
        assert_eq!(labels(&t, &rows), ["=P01", "-201U3", "coil"]);
        assert_eq!(rows[1].expanded, Some(true));
    }

    /// A table row with cells, its tokens the label's and the cells' (as the provider builds them).
    fn row(label: &str, cells: &[&str]) -> (Vec<String>, Vec<String>) {
        let cells: Vec<String> = cells.iter().map(|c| (*c).to_owned()).collect();
        let mut tokens: Vec<String> = std::iter::once(label)
            .chain(cells.iter().map(String::as_str))
            .flat_map(nav_tokens)
            .collect();
        tokens.sort();
        tokens.dedup();
        (tokens, cells)
    }

    /// FT-663 (§6.3): the sub-tokens of one typed word match within one cell; separate words may match different
    /// cells.
    #[test]
    fn one_typed_word_matches_within_one_cell() {
        let (t1, c1) = row("24VDC-1", &["24VDC-1", "4"]);
        let (t2, c2) = row("24VDC-4", &["24VDC-4", "1"]);
        let q = Query::parse("24VDC-4");
        assert!(
            !q.matches(&t1, &[], "24VDC-1", &c1),
            "24vdc and 4 in different cells"
        );
        assert!(q.matches(&t2, &[], "24VDC-4", &c2));
        // Two typed words may match different cells.
        assert!(Query::parse("24VDC 4").matches(&t1, &[], "24VDC-1", &c1));
        // A word with its sub-tokens in the label alone (no cells) still matches.
        assert!(q.matches(&t2, &[], "24VDC-4", &[]));
        // The label substring alternative is unchanged.
        assert!(Query::parse("vdc-4").matches(&t2, &[], "24VDC-4", &c2));
    }
}
