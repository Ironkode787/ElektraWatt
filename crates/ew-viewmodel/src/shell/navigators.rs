// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigators in the shell (WP-S2.06 part 1, §6.2 to §6.9): the open navigators with their tables, the
//! `navigator.*` actions (which also take `kind = pages` and then do what the six `pages.*` actions do), multi-selection,
//! selection synchronisation both ways with the chip line "Not on this page" (§6.5), the reorder of pages through
//! `page.reorder` and its card (§6.6), and the New page pre-fill (FT-461).
//!
//! **Tables.** At the end of every dispatch each open navigator other than Pages whose table is not of the current
//! revision fetches it with one `Session::navigator_table` call (counted); a closed navigator makes none. Filtering,
//! sorting, expanding, moving the cursor, marking and type-ahead make no call. The Pages navigator reads the page rows
//! the shell already holds (fetched once per revision).
//!
//! **Synchronisation.** Marking rows in navigator N selects their drawables on the shown page, or else their records;
//! the page is never switched, and the drawables elsewhere become the chips. N then holds the selection's generation.
//! At the end of every dispatch each open navigator whose generation differs from the selection's marks the rows of
//! the selected records (the deepest node per record), expands their ancestors and puts its cursor on the first of
//! them; the focus stays. The chips go when the selection or the page changes; a page switch puts the Pages cursor
//! back on the current page (FT-371's navigator item).

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_api::output::ew_dl::{Destination, EntityRef};
use ew_api::output::{Drawable, NavKind, PageRow, Uuid};
use ew_api::{AnyCommand, Origin, Submit};
use serde_json::{Map, Value, json};

use super::navigation::{Focus, bump};
use super::{Mode, Outcome, ShellModel, failed, flag, string_param};
use crate::canvas::PageId;
use crate::layout::PaneKind;
use crate::navigator::RowKind;
use crate::navigators::{NavView, Navigator, SortBy, starts_with_folded};
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, PendingEdit};
use crate::strings;
use crate::tools::{PreviewState, SelectionChange};

/// The most chips the line "Not on this page" shows (§6.5).
pub const MAX_CHIPS: usize = 12;

/// The command `pages.reorder` builds.
const REORDER: &str = "page.reorder";

/// A chip of the line "Not on this page": a drawable of the marked rows on another page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Chip {
    /// The page's folio.
    pub folio: String,
    /// The anchor cell, empty without a frame grid.
    pub cell: String,
    /// The page (its page UUID).
    pub page: Uuid,
    /// The representation or connection.
    pub record: Uuid,
}

impl Chip {
    /// `<folio>-<cell>`, the folio alone without a cell.
    pub fn text(&self) -> String {
        if self.cell.is_empty() {
            self.folio.clone()
        } else {
            format!("{}-{}", self.folio, self.cell)
        }
    }
}

/// What a row on show carries for dragging (§6.6, §6.7).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DragSource {
    /// A page record row of the Pages navigator: its folio.
    Page(String),
    /// An unplaced function row of the Devices navigator: the function's ID text.
    Function(String),
}

/// One row on show of any navigator, as the widget paints it and the describe text prints it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ShownRow {
    /// The row's key.
    pub key: String,
    /// Its parent's key.
    pub parent: Option<String>,
    /// The depth, from 1.
    pub level: usize,
    /// The role word (`page`, `section`, `device`, `representation`).
    pub role: &'static str,
    /// The painted text.
    pub text: String,
    /// The accessible name.
    pub name: String,
    /// `Some(expanded)` for a node with children on show.
    pub expanded: Option<bool>,
    /// The 1-based position among its siblings, and their number.
    pub position: usize,
    /// The number of its siblings on show, itself included.
    pub set_size: usize,
    /// Whether the row is marked.
    pub marked: bool,
    /// Whether the row is the current page (Pages).
    pub current: bool,
    /// Whether Enter opens it (otherwise Enter toggles it).
    pub openable: bool,
    /// What a drag of the row carries.
    pub drag: Option<DragSource>,
}

/// Whether a page row is a page record (not a generated page, not a section's divider).
fn is_record(r: &PageRow) -> bool {
    !r.generated && r.kind != "section_divider"
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The navigators other than Pages that are open (their panes are in the layout), in menu order.
    pub fn open_navigators(&self) -> Vec<NavKind> {
        self.navs.keys().copied().collect()
    }

    /// Whether navigator `kind` is open (Pages: its pane is in the layout).
    pub fn navigator_open(&self, kind: NavKind) -> bool {
        match kind {
            NavKind::Pages => self.layout().panes().contains(&PaneKind::Pages),
            k => self.navs.contains_key(&k),
        }
    }

    /// The state of an open navigator other than Pages.
    pub fn nav(&self, kind: NavKind) -> Option<&Navigator> {
        self.navs.get(&kind)
    }

    /// The chips of the line "Not on this page" (§6.5).
    pub fn chips(&self) -> &[Chip] {
        &self.chips
    }

    /// Counted up by every dispatch: the widget rebuilds its rows when it changed.
    pub fn nav_version(&self) -> u64 {
        self.nav_version
    }

    /// The navigator the focus is in.
    pub fn focused_navigator(&self) -> Option<NavKind> {
        match self.focus {
            Some(Focus::Navigator(k)) => Some(k),
            _ => None,
        }
    }

    /// The FnDef key of a function, from an open navigator's function row (its label, §6.1 item 2).
    pub(super) fn function_fndef_key(&self, function: &str) -> Option<String> {
        self.navs.values().find_map(|n| {
            let t = n.table()?;
            t.nodes
                .iter()
                .find(|x| {
                    x.role == ew_api::output::NavRole::Function
                        && x.records.iter().any(|r| r.to_string() == function)
                })
                .map(|x| x.label.clone())
        })
    }

    /// The keys of navigator `kind`'s rows on show, in display order.
    fn vis_keys(&self, kind: NavKind) -> Vec<String> {
        match kind {
            NavKind::Pages => self.navigator_rows().into_iter().map(|r| r.key).collect(),
            k => match self.navs.get(&k) {
                Some(n) => {
                    let Some(t) = n.table() else { return Vec::new() };
                    n.visible()
                        .iter()
                        .map(|r| t.nodes[r.node as usize].key.clone())
                        .collect()
                }
                None => Vec::new(),
            },
        }
    }

    /// The row cursor's key of navigator `kind`.
    pub fn nav_cursor(&self, kind: NavKind) -> Option<String> {
        match kind {
            NavKind::Pages => self.navigator_row_key(),
            k => self.navs.get(&k).and_then(Navigator::cursor_key),
        }
    }

    /// The rows on show of navigator `kind`.
    pub fn shown_rows(&self, kind: NavKind) -> Vec<ShownRow> {
        match kind {
            NavKind::Pages => {
                let rows = self.page_rows();
                let current = self.current_page();
                let nav = &self.navigator;
                let draggable = nav.mode != crate::navigator::NavMode::Structure;
                self.navigator_rows()
                    .into_iter()
                    .map(|r| {
                        let name = crate::navigator::PageNavigator::accessible_name(&r, rows, current);
                        let (text, role, current, openable, drag) = match &r.kind {
                            RowKind::Node { label, aspect } => {
                                (label.clone(), aspect.word(), false, false, None)
                            }
                            RowKind::Page(i) => {
                                let p = &rows[*i];
                                let mut t = format!("{} {}", p.folio, p.title);
                                if p.generated {
                                    t.push_str(&format!(" [{}]", strings::text("badge-generated")));
                                }
                                if p.qet_grid {
                                    t.push_str(&format!(" [{}]", strings::text("badge-qet-grid")));
                                }
                                let drag =
                                    (draggable && is_record(p)).then(|| DragSource::Page(p.folio.clone()));
                                (t, "page", Some(PageId::of(p)) == current, true, drag)
                            }
                        };
                        ShownRow {
                            marked: nav.is_marked(&r.key),
                            key: r.key,
                            parent: r.parent,
                            level: r.level,
                            role,
                            text,
                            name,
                            expanded: matches!(r.kind, RowKind::Node { .. }).then_some(r.expanded),
                            position: r.position,
                            set_size: r.set_size,
                            current,
                            openable,
                            drag,
                        }
                    })
                    .collect()
            }
            k => {
                let Some(n) = self.navs.get(&k) else {
                    return Vec::new();
                };
                let Some(t) = n.table() else { return Vec::new() };
                n.visible()
                    .iter()
                    .map(|v| {
                        let node = &t.nodes[v.node as usize];
                        let mut name = format!("{}, {}", node.label, node.role.word());
                        for b in &node.badges {
                            name.push_str(&format!(", {b}"));
                        }
                        let mut text = node.label.clone();
                        for b in &node.badges {
                            text.push_str(&format!(" [{b}]"));
                        }
                        let drag = (node.role == ew_api::output::NavRole::Function
                            && node.badges.iter().any(|b| b == "unplaced"))
                        .then(|| node.records.first().map(|r| DragSource::Function(r.to_string())))
                        .flatten();
                        ShownRow {
                            key: node.key.clone(),
                            parent: node.parent.map(|p| t.nodes[p as usize].key.clone()),
                            level: v.level,
                            role: node.role.word(),
                            text,
                            name,
                            expanded: v.expanded,
                            position: v.position,
                            set_size: v.set_size,
                            marked: n.marked().contains(&node.key),
                            current: false,
                            openable: node.open.is_some(),
                            drag,
                        }
                    })
                    .collect()
            }
        }
    }

    /// The New page pre-fill (FT-461): the folio proposal and the section number text. The folio: when the current page
    /// is a page record with number k, the smallest integer n > k whose text is not a folio of the page labels; else
    /// empty. The section: of the marked or cursor section node of the Pages navigator, else the current page's.
    pub fn new_page_proposal(&self) -> (String, String) {
        let rows = self.page_rows();
        let current = self
            .current_page()
            .and_then(|c| rows.iter().find(|r| PageId::of(r) == c));
        let folio = current
            .filter(|r| is_record(r))
            .and_then(|r| r.folio.split('.').next()?.parse::<u64>().ok())
            .map(|k| {
                let labels = self.page_labels();
                (k + 1..)
                    .find(|n| !labels.iter().any(|l| *l == n.to_string()))
                    .map_or_else(String::new, |n| n.to_string())
            })
            .unwrap_or_default();
        let section_key = |key: &str| -> Option<String> {
            let last = key.rsplit('/').next()?;
            last.strip_prefix("s:")
                .filter(|s| *s != "front")
                .map(str::to_owned)
        };
        let nav = &self.navigator;
        let from_nav = nav
            .marked()
            .iter()
            .find_map(|k| section_key(k))
            .or_else(|| self.navigator_row_key().and_then(|k| section_key(&k)));
        let section = from_nav
            .or_else(|| current.and_then(|r| r.section.as_ref().map(|(n, _)| n.to_string())))
            .unwrap_or_default();
        (folio, section)
    }

    // -----------------------------------------------------------------------------------------------------------
    // The end of every dispatch

    /// Opens and closes navigator states with the layout, fetches the tables of the current revision, clears the chips
    /// when the selection or the page changed, follows a page switch with the Pages cursor, and marks the selected
    /// records' rows in every navigator that has not seen the selection's generation (§6.5).
    pub(super) fn sync_navigators(&mut self) {
        self.nav_version += 1;
        if !self.has_project() {
            self.navs.clear();
            self.chips.clear();
            self.last_page = None;
            return;
        }
        let panes = self.layout().panes();
        let wanted: BTreeSet<NavKind> = panes
            .iter()
            .filter_map(|p| match p {
                PaneKind::Navigator(k) => Some(*k),
                _ => None,
            })
            .collect();
        self.navs.retain(|k, _| wanted.contains(k));
        for k in &wanted {
            self.navs.entry(*k).or_insert_with(|| Navigator::new(*k));
        }
        if let Some(Focus::Navigator(k)) = self.focus
            && k != NavKind::Pages
            && !self.navs.contains_key(&k)
        {
            self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
        }
        // One table per open navigator and revision.
        let rev = self.revision();
        let kinds: Vec<NavKind> = self.navs.keys().copied().collect();
        for k in kinds {
            let stale = self
                .navs
                .get(&k)
                .is_none_or(|n| n.table().is_none_or(|t| Some(t.rev) != rev));
            if !stale {
                continue;
            }
            let Some(project) = &self.project else { return };
            let table = project.session.navigator_table(k);
            bump(&self.calls, |c| c.navigator_table += 1);
            if let Some(n) = self.navs.get_mut(&k) {
                n.table = Some(Arc::new(table));
            }
        }
        // The chips belong to the selection and the page they were made for.
        let generation = self.tools.selection.generation;
        let page = self.current_page();
        if self.chips_for != Some((generation, page)) {
            self.chips.clear();
        }
        if page != self.last_page {
            self.last_page = page;
            self.navigator.reset_row();
        }
        // Canvas (or any other origin) → navigators.
        let items: Vec<_> = self.tools.selection.items.iter().map(|e| e.0).collect();
        if self.navigator.applied != Some(generation) {
            self.navigator.applied = Some(generation);
            let rows = self.page_rows().to_vec();
            let marked: BTreeSet<String> = rows
                .iter()
                .enumerate()
                .filter(|(_, r)| is_record(r) && items.contains(&r.id))
                .map(|(i, _)| self.navigator.key_of(&rows, i))
                .collect();
            if !marked.is_empty() {
                let vis = self.navigator_rows();
                if let Some(first) = vis.iter().find(|r| marked.contains(&r.key)) {
                    let key = first.key.clone();
                    self.navigator.set_row(&key);
                }
            }
            self.navigator.marked = marked;
        }
        for n in self.navs.values_mut() {
            if n.applied == Some(generation) {
                continue;
            }
            n.applied = Some(generation);
            let Some(table) = n.table().cloned() else { continue };
            let mut nodes: Vec<u32> = items
                .iter()
                .filter_map(|id| table.index.get(id).and_then(|v| v.last().copied()))
                .collect();
            nodes.sort_unstable();
            nodes.dedup();
            n.marked = nodes
                .iter()
                .map(|i| table.nodes[*i as usize].key.clone())
                .collect();
            for i in &nodes {
                n.reveal(*i);
            }
            if !nodes.is_empty() {
                let vis = n.visible();
                if let Some(first) = vis.iter().find(|r| nodes.contains(&r.node)) {
                    n.cursor = Some(table.nodes[first.node as usize].key.clone());
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Parameters

    /// The `kind` parameter: a navigator word, else (absent) the focused navigator.
    fn kind_param(&self, p: &Map<String, Value>) -> Result<NavKind, Outcome> {
        match string_param(p, "kind")? {
            Some(w) => NavKind::from_word(w).ok_or_else(|| {
                let words: Vec<&str> = NavKind::ALL.iter().map(|k| k.word()).collect();
                failed("error-nav-kind", &[w, &words.join(", ")])
            }),
            None => self
                .focused_navigator()
                .ok_or_else(|| failed("error-missing-param", &["kind"])),
        }
    }

    /// An open navigator's kind from the parameters.
    fn open_kind(&self, p: &Map<String, Value>) -> Result<NavKind, Outcome> {
        let k = self.kind_param(p)?;
        if self.navigator_open(k) || k == NavKind::Pages {
            Ok(k)
        } else {
            Err(failed("error-nav-closed", &[k.word()]))
        }
    }

    /// A row key on show of navigator `kind`, from parameter `name`.
    fn nav_key_param(
        &self,
        kind: NavKind,
        p: &Map<String, Value>,
        name: &str,
    ) -> Result<Option<String>, Outcome> {
        match string_param(p, name)? {
            None => Ok(None),
            Some(k) if self.vis_keys(kind).iter().any(|v| v == k) => Ok(Some(k.to_owned())),
            Some(k) => Err(failed("error-nav-key", &[k, kind.word()])),
        }
    }

    fn set_cursor(&mut self, kind: NavKind, key: &str) {
        match kind {
            NavKind::Pages => self.navigator.set_row(key),
            k => {
                if let Some(n) = self.navs.get_mut(&k) {
                    n.cursor = Some(key.to_owned());
                }
            }
        }
    }

    fn marks(&self, kind: NavKind) -> BTreeSet<String> {
        match kind {
            NavKind::Pages => self.navigator.marked.clone(),
            k => self.navs.get(&k).map(|n| n.marked.clone()).unwrap_or_default(),
        }
    }

    fn set_marks(&mut self, kind: NavKind, marks: BTreeSet<String>, anchor: Option<String>) {
        match kind {
            NavKind::Pages => {
                self.navigator.marked = marks;
                if anchor.is_some() {
                    self.navigator.anchor = anchor;
                }
            }
            k => {
                if let Some(n) = self.navs.get_mut(&k) {
                    n.marked = marks;
                    if anchor.is_some() {
                        n.anchor = anchor;
                    }
                }
            }
        }
    }

    fn anchor(&self, kind: NavKind) -> Option<String> {
        match kind {
            NavKind::Pages => self.navigator.anchor.clone(),
            k => self.navs.get(&k).and_then(|n| n.anchor.clone()),
        }
    }

    /// The records and drawables of a row.
    fn row_payload(&self, kind: NavKind, key: &str) -> (Vec<Uuid>, Vec<Drawable>) {
        match kind {
            NavKind::Pages => {
                let rows = self.page_rows();
                match crate::navigator::PageNavigator::page_of(key, rows) {
                    Some(i) if is_record(&rows[i]) => (vec![rows[i].id], Vec::new()),
                    _ => (Vec::new(), Vec::new()),
                }
            }
            k => match self.navs.get(&k).and_then(|n| n.node_of(key)) {
                Some((_, node)) => (node.records.clone(), node.drawables.clone()),
                None => (Vec::new(), Vec::new()),
            },
        }
    }

    /// The label type-ahead compares with.
    fn row_label(&self, kind: NavKind, key: &str) -> String {
        match kind {
            NavKind::Pages => {
                let rows = self.page_rows();
                match crate::navigator::PageNavigator::page_of(key, rows) {
                    Some(i) => rows[i].folio.clone(),
                    None => self
                        .navigator_rows()
                        .into_iter()
                        .find(|r| r.key == key)
                        .and_then(|r| match r.kind {
                            RowKind::Node { label, .. } => Some(label),
                            RowKind::Page(_) => None,
                        })
                        .unwrap_or_default(),
                }
            }
            k => self
                .navs
                .get(&k)
                .and_then(|n| n.node_of(key))
                .map(|(_, n)| n.label.clone())
                .unwrap_or_default(),
        }
    }

    /// Navigator → canvas (§6.5): the window selection becomes the marked rows' drawables on the shown page, else their
    /// records; the page is never switched; the drawables elsewhere become the chips.
    fn apply_marks(&mut self, kind: NavKind) {
        let marks = self.marks(kind);
        let mut keys: Vec<String> = self
            .vis_keys(kind)
            .into_iter()
            .filter(|k| marks.contains(k))
            .collect();
        for k in &marks {
            if !keys.contains(k) {
                keys.push(k.clone());
            }
        }
        let shown = match self.current_page().map(|p| p.0) {
            Some(Destination::Page(u)) => Some(u),
            _ => None,
        };
        let mut recs = Vec::new();
        let mut here = Vec::new();
        let mut elsewhere: Vec<Drawable> = Vec::new();
        for k in &keys {
            let (r, d) = self.row_payload(kind, k);
            for x in r {
                if !recs.contains(&x) {
                    recs.push(x);
                }
            }
            for x in d {
                if Some(x.page) == shown {
                    if !here.contains(&x.record) {
                        here.push(x.record);
                    }
                } else if !elsewhere.iter().any(|e| e.record == x.record) {
                    elsewhere.push(x);
                }
            }
        }
        let items: Vec<EntityRef> = if here.is_empty() { &recs } else { &here }
            .iter()
            .map(|u| EntityRef(*u))
            .collect();
        if self.tools.selection.apply(&SelectionChange::Replace(items)) {
            self.tools.announcement = Some(strings::format(
                "announce-selected",
                &[&self.tools.selection.items.len().to_string()],
            ));
        }
        let generation = self.tools.selection.generation;
        self.chips = if here.is_empty() && !recs.is_empty() {
            elsewhere.sort_by_key(|d| !d.main);
            elsewhere
                .into_iter()
                .take(MAX_CHIPS)
                .map(|d| Chip {
                    folio: d.folio,
                    cell: d.cell,
                    page: d.page,
                    record: d.record,
                })
                .collect()
        } else {
            Vec::new()
        };
        self.chips_for = Some((generation, self.current_page()));
        match kind {
            NavKind::Pages => self.navigator.applied = Some(generation),
            k => {
                if let Some(n) = self.navs.get_mut(&k) {
                    n.applied = Some(generation);
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The actions

    /// `navigator.show {kind, on?}`: shows a navigator as a tab beside the Pages pane and focuses it, or (`on: false`)
    /// closes it.
    pub(super) fn act_navigator_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.kind_param(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let on = p.get("on").and_then(Value::as_bool).unwrap_or(true);
        let pane = match kind {
            NavKind::Pages => PaneKind::Pages,
            k => PaneKind::Navigator(k),
        };
        if on {
            let layout = self.layout().with_navigator(pane);
            self.replace_layout(layout);
            if kind != NavKind::Pages {
                self.navs.entry(kind).or_insert_with(|| Navigator::new(kind));
            }
            self.focus = Some(Focus::Navigator(kind));
        } else {
            let layout = self.layout().without(pane);
            self.replace_layout(layout);
            self.navs.remove(&kind);
            if self.focus == Some(Focus::Navigator(kind)) {
                self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
            }
        }
        Outcome::Done
    }

    /// `navigator.view {kind, view}`: Pages takes its modes (`pages.mode`), the others `tree` or `flat`.
    pub(super) fn act_navigator_view(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let view = match string_param(p, "view") {
            Err(e) => return e,
            Ok(None) => return failed("error-missing-param", &["view"]),
            Ok(Some(v)) => v.to_owned(),
        };
        if kind == NavKind::Pages {
            return self.act_pages_mode(&json_map(json!({ "mode": view })));
        }
        match NavView::from_word(&view) {
            Some(v) => {
                if let Some(n) = self.navs.get_mut(&kind) {
                    n.view = v;
                }
                Outcome::Done
            }
            None => failed("error-nav-value", &["view", &view, "tree, flat"]),
        }
    }

    /// `navigator.sort {kind, by, descending?}`: Pages takes `folio` or `type`, the others `label` or a column key.
    pub(super) fn act_navigator_sort(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let by = match string_param(p, "by") {
            Err(e) => return e,
            Ok(None) => return failed("error-missing-param", &["by"]),
            Ok(Some(v)) => v.to_owned(),
        };
        let descending = flag(p, "descending");
        if kind == NavKind::Pages {
            if descending {
                return failed("error-bad-param", &["descending"]);
            }
            return self.act_pages_sort(&json_map(json!({ "by": by })));
        }
        let Some(n) = self.navs.get_mut(&kind) else {
            return failed("error-nav-closed", &[kind.word()]);
        };
        let columns: Vec<String> = n
            .table()
            .map(|t| t.columns.iter().map(|c| c.key.clone()).collect())
            .unwrap_or_default();
        let sort = if by == "label" {
            SortBy::Label
        } else if columns.contains(&by) {
            SortBy::Column(by)
        } else {
            let mut valid = vec!["label".to_owned()];
            valid.extend(columns);
            return failed("error-nav-value", &["by", &by, &valid.join(", ")]);
        };
        n.sort = sort;
        n.descending = descending;
        Outcome::Done
    }

    /// `navigator.filter {kind, text}`.
    pub(super) fn act_navigator_filter(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        if kind == NavKind::Pages {
            return self.act_pages_filter(p);
        }
        match string_param(p, "text") {
            Err(e) => e,
            Ok(None) => failed("error-missing-param", &["text"]),
            Ok(Some(t)) => {
                if let Some(n) = self.navs.get_mut(&kind) {
                    n.filter = t.to_owned();
                }
                Outcome::Done
            }
        }
    }

    /// `navigator.toggle {kind, key}`: expands or collapses a node with children.
    pub(super) fn act_navigator_toggle(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        if kind == NavKind::Pages {
            return self.act_pages_toggle(p);
        }
        let key = match self.nav_key_param(kind, p, "key") {
            Err(e) => return e,
            Ok(None) => return failed("error-missing-param", &["key"]),
            Ok(Some(k)) => k,
        };
        let has_children = self
            .shown_rows(kind)
            .iter()
            .any(|r| r.key == key && r.expanded.is_some());
        if !has_children {
            return failed("error-nav-key", &[&key, kind.word()]);
        }
        if let Some(n) = self.navs.get_mut(&kind) {
            if !n.collapsed.remove(&key) {
                n.collapsed.insert(key.clone());
            }
            n.cursor = Some(key);
        }
        Outcome::Done
    }

    /// `navigator.row {kind, key?, step?, to?, extend?}`: moves the cursor; `extend` marks the range from the anchor to
    /// the new cursor in visible order (Shift+Up, Shift+Down, Shift+click).
    pub(super) fn act_navigator_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let before = self.nav_cursor(kind);
        let mut q = p.clone();
        q.remove("kind");
        q.remove("extend");
        let out = if kind == NavKind::Pages {
            self.act_pages_row(&q)
        } else {
            let keys = self.vis_keys(kind);
            if keys.is_empty() {
                return Outcome::Done;
            }
            let key = if q.contains_key("key") {
                match self.nav_key_param(kind, &q, "key") {
                    Ok(Some(k)) => k,
                    Ok(None) => return failed("error-missing-param", &["key"]),
                    Err(e) => return e,
                }
            } else if let Some(to) = q.get("to") {
                match to.as_str() {
                    Some("first") => keys[0].clone(),
                    Some("last") => keys[keys.len() - 1].clone(),
                    _ => return failed("error-bad-param", &["to"]),
                }
            } else if let Some(step) = q.get("step") {
                let Some(step) = step.as_i64() else {
                    return failed("error-bad-param", &["step"]);
                };
                let here = before
                    .as_ref()
                    .and_then(|k| keys.iter().position(|x| x == k))
                    .unwrap_or(0);
                let at = (here as i64 + step).clamp(0, keys.len() as i64 - 1) as usize;
                keys[at].clone()
            } else {
                return failed("error-missing-param", &["key"]);
            };
            self.set_cursor(kind, &key);
            Outcome::Done
        };
        if out != Outcome::Done || !flag(p, "extend") {
            return out;
        }
        let keys = self.vis_keys(kind);
        let anchor = self.anchor(kind).or(before);
        let (Some(anchor), Some(now)) = (anchor, self.nav_cursor(kind)) else {
            return out;
        };
        let (Some(a), Some(b)) = (
            keys.iter().position(|k| *k == anchor),
            keys.iter().position(|k| *k == now),
        ) else {
            return out;
        };
        let (lo, hi) = (a.min(b), a.max(b));
        let marks: BTreeSet<String> = keys[lo..=hi].iter().cloned().collect();
        self.set_marks(kind, marks, Some(anchor));
        self.apply_marks(kind);
        out
    }

    /// `navigator.open {kind, key?, new_tab?}`: opens a row's target in the focused tab as a jump (a page row its page;
    /// a device row the page of its first drawable with its drawables there selected); a node without a target toggles.
    pub(super) fn act_navigator_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        if kind == NavKind::Pages {
            let mut q = p.clone();
            q.remove("kind");
            if !q.contains_key("key")
                && let Some(k) = self.nav_cursor(kind)
            {
                q.insert("key".into(), Value::String(k));
            }
            let row = q
                .get("key")
                .and_then(Value::as_str)
                .and_then(|k| self.navigator_rows().into_iter().find(|r| r.key == k));
            if let Some(r) = row
                && matches!(r.kind, RowKind::Node { .. })
            {
                return self.act_pages_toggle(&q);
            }
            return self.act_pages_open(&q);
        }
        let key = match self.nav_key_param(kind, p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k,
            Ok(None) => match self.nav_cursor(kind) {
                Some(k) => k,
                None => {
                    return Outcome::Refused {
                        reason_key: "reason-no-nav-row",
                    };
                }
            },
        };
        let Some((_, node)) = self.navs.get(&kind).and_then(|n| n.node_of(&key)) else {
            return failed("error-nav-key", &[&key, kind.word()]);
        };
        let Some(open) = node.open.clone() else {
            let mut q = Map::new();
            q.insert("kind".into(), Value::from(kind.word()));
            q.insert("key".into(), Value::String(key));
            return self.act_navigator_toggle(&q);
        };
        self.set_cursor(kind, &key);
        let page = PageId(Destination::Page(open.page));
        if self.row_index(page).is_none() {
            return failed("error-no-folio", &[&open.page.to_string()]);
        }
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Done;
        };
        if flag(p, "new_tab") {
            let out = self.open_in_new_tab(g, page);
            if out != Outcome::Done {
                return out;
            }
        } else {
            self.jump_to(g, page);
        }
        let items: Vec<EntityRef> = open.select.iter().map(|u| EntityRef(*u)).collect();
        if self.tools.selection.apply(&SelectionChange::Replace(items)) {
            self.tools.announcement = Some(strings::format(
                "announce-selected",
                &[&self.tools.selection.items.len().to_string()],
            ));
        }
        let generation = self.tools.selection.generation;
        if let Some(n) = self.navs.get_mut(&kind) {
            n.applied = Some(generation);
        }
        self.chips.clear();
        Outcome::Done
    }

    /// `navigator.select {kind, keys, add?}`: marks the rows (`add`: to the marked ones) and synchronises the selection.
    pub(super) fn act_navigator_select(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let Some(Value::Array(list)) = p.get("keys") else {
            return failed("error-missing-param", &["keys"]);
        };
        let vis = self.vis_keys(kind);
        let mut keys = Vec::new();
        for v in list {
            let Some(k) = v.as_str() else {
                return failed("error-bad-param", &["keys"]);
            };
            if !vis.iter().any(|x| x == k) {
                return failed("error-nav-key", &[k, kind.word()]);
            }
            keys.push(k.to_owned());
        }
        let mut marks = if flag(p, "add") {
            self.marks(kind)
        } else {
            BTreeSet::new()
        };
        marks.extend(keys.iter().cloned());
        self.set_marks(kind, marks, keys.first().cloned());
        if let Some(last) = keys.last() {
            self.set_cursor(kind, last);
        }
        self.apply_marks(kind);
        Outcome::Done
    }

    /// `navigator.mark {kind?, key?}` (Space): toggles the mark of a row (default the cursor row).
    pub(super) fn act_navigator_mark(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let key = match self.nav_key_param(kind, p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k,
            Ok(None) => match self.nav_cursor(kind) {
                Some(k) => k,
                None => {
                    return Outcome::Refused {
                        reason_key: "reason-no-nav-row",
                    };
                }
            },
        };
        let mut marks = self.marks(kind);
        if !marks.remove(&key) {
            marks.insert(key.clone());
        }
        self.set_marks(kind, marks, Some(key.clone()));
        self.set_cursor(kind, &key);
        self.apply_marks(kind);
        Outcome::Done
    }

    /// `navigator.jump {kind, prefix}`: moves the cursor to the next visible row (after the cursor, wrapping) whose label
    /// starts with `prefix`, case-folded (type-ahead without a clock).
    pub(super) fn act_navigator_jump(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let prefix = match string_param(p, "prefix") {
            Err(e) => return e,
            Ok(None) => return failed("error-missing-param", &["prefix"]),
            Ok(Some(t)) => t.to_owned(),
        };
        let keys = self.vis_keys(kind);
        if keys.is_empty() || prefix.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        }
        let here = self
            .nav_cursor(kind)
            .and_then(|k| keys.iter().position(|x| *x == k))
            .unwrap_or(0);
        // A one-character buffer moves on from the cursor; a longer one may stay on it.
        let start = if prefix.chars().count() == 1 {
            here + 1
        } else {
            here
        };
        let found = (0..keys.len())
            .map(|i| (start + i) % keys.len())
            .find(|&i| starts_with_folded(&self.row_label(kind, &keys[i]), &prefix));
        match found {
            Some(i) => {
                let key = keys[i].clone();
                self.set_cursor(kind, &key);
                Outcome::Done
            }
            None => Outcome::Refused {
                reason_key: "reason-no-nav-row",
            },
        }
    }

    /// `navigator.apply` (Ctrl+Enter with a card in the pane): applies the reorder card.
    pub(super) fn act_navigator_apply(&mut self) -> Outcome {
        if !self
            .props
            .card()
            .is_some_and(|c| matches!(c.origin, CardOrigin::Reorder { .. }))
        {
            return Outcome::Refused {
                reason_key: "reason-no-card",
            };
        }
        self.act_preview_apply()
    }

    // -----------------------------------------------------------------------------------------------------------
    // Reorder (§6.6)

    /// The page row of a printed folio that is a page record.
    fn record_row(&self, folio: &str) -> Result<PageRow, Outcome> {
        match self.page_rows().iter().find(|r| r.folio == folio) {
            None => Err(failed("error-no-folio", &[folio])),
            Some(r) if !is_record(r) => Err(failed("error-generated-page-order", &[])),
            Some(r) => Ok(r.clone()),
        }
    }

    /// The marked page records of the Pages navigator, in page order (the cursor row's page when none is marked).
    fn marked_pages(&self) -> Result<Vec<PageRow>, Outcome> {
        let rows = self.page_rows();
        let marks = &self.navigator.marked;
        let mut keys: Vec<&String> = marks.iter().collect();
        let cursor = self.navigator_row_key();
        if keys.is_empty()
            && let Some(c) = &cursor
        {
            keys.push(c);
        }
        let mut out: Vec<usize> = Vec::new();
        for k in keys {
            if let Some(i) = crate::navigator::PageNavigator::page_of(k, rows) {
                if !is_record(&rows[i]) {
                    return Err(failed("error-generated-page-order", &[]));
                }
                out.push(i);
            }
        }
        out.sort_unstable();
        out.dedup();
        if out.is_empty() {
            return Err(Outcome::Refused {
                reason_key: "reason-no-pages-marked",
            });
        }
        Ok(out.into_iter().map(|i| rows[i].clone()).collect())
    }

    /// `pages.reorder {pages?, before?, end?}`: builds `page.reorder` from folios (default the marked page records) and
    /// shows its card in the Pages pane.
    pub(super) fn act_pages_reorder(&mut self, p: &Map<String, Value>) -> Outcome {
        let pages: Vec<PageRow> = match p.get("pages") {
            None => match self.marked_pages() {
                Ok(v) => v,
                Err(e) => return e,
            },
            Some(Value::Array(list)) => {
                let mut v = Vec::new();
                for x in list {
                    let Some(f) = x.as_str() else {
                        return failed("error-bad-param", &["pages"]);
                    };
                    match self.record_row(f) {
                        Ok(r) => v.push(r),
                        Err(e) => return e,
                    }
                }
                v
            }
            Some(_) => return failed("error-bad-param", &["pages"]),
        };
        let before = match string_param(p, "before") {
            Err(e) => return e,
            Ok(None) => None,
            Ok(Some(f)) => match self.record_row(f) {
                Ok(r) => Some(r),
                Err(e) => return e,
            },
        };
        if before.is_none() && !flag(p, "end") {
            return failed("error-missing-param", &["before"]);
        }
        self.reorder(&pages, before.as_ref())
    }

    /// Previews `page.reorder` of `pages` before `before` (else to the end of their section) and shows the card.
    fn reorder(&mut self, pages: &[PageRow], before: Option<&PageRow>) -> Outcome {
        let mut params = json!({ "pages": pages.iter().map(|r| r.id.to_string()).collect::<Vec<_>>() });
        match before {
            Some(b) => params["before"] = Value::String(b.id.to_string()),
            None => {
                if let Some((n, _)) = pages.first().and_then(|r| r.section.clone()) {
                    let Some(project) = &self.project else {
                        return Outcome::Refused {
                            reason_key: "reason-no-project",
                        };
                    };
                    match project.session.resolve_reference(&format!("section:{n}")) {
                        Ok(id) => params["section"] = Value::String(id.to_string()),
                        Err(message) => return Outcome::Failed { message },
                    }
                }
            }
        }
        self.preview_reorder(params, pages.len())
    }

    /// Previews `page.reorder` with `params` (a Bulk command: always the card) and shows the card.
    fn preview_reorder(&mut self, params: Value, moved: usize) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let cmd = AnyCommand {
            name: REORDER.to_owned(),
            params: params.clone(),
        };
        let submit = match self.mode {
            Mode::Gui => Submit::from(Origin::Gui),
            Mode::Headless => Submit::from(Origin::CmdLog {
                file: self.origin_file.clone(),
            }),
        };
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        bump(calls, |c| c.preview_with += 1);
        let result = project.session.preview_with(&cmd, submit);
        let (derived, notices) = match &result {
            Ok(p) => (project.session.derived_diff(p), p.outcome().notices.clone()),
            Err(_) => (Default::default(), Vec::new()),
        };
        if let Err(e) = &result {
            // A refused reorder opens no card: its reason is the outcome.
            return Outcome::Failed {
                message: e.to_string(),
            };
        }
        let result = result.map_err(|e| e.to_string());
        let edit = PendingEdit {
            row: String::new(),
            kind: "page".to_owned(),
            route: EditRoute::ReadOnly(FieldReadOnly::NoCommand),
            path: String::new(),
            targets: Vec::new(),
            leaf: None,
            base: None,
            name: REORDER.to_owned(),
            params: params.clone(),
            typed: String::new(),
            label: String::new(),
        };
        let changes = notices.len();
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: params,
                name: REORDER.to_owned(),
                result,
                derived,
            },
            edit,
            targets: moved,
            changes,
            reason: "reason-bulk",
            origin: CardOrigin::Reorder { notices },
        });
        Outcome::Done
    }

    /// Apply on a reorder card: a fresh card commits one transaction; a stale one previews the same parameters again
    /// and shows a fresh card (a second Apply).
    pub(super) fn apply_reorder_card(&mut self, card: Card) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        if let Err(message) = &card.preview.result {
            return Outcome::Failed {
                message: message.clone(),
            };
        }
        if card.preview.base_rev == rev {
            let Card { preview, edit, .. } = card;
            let out = self.commit_kept(preview, &edit.name, &edit.params, None);
            if out == Outcome::Done {
                self.navigator.marked.clear();
            }
            return out;
        }
        self.preview_reorder(card.edit.params, card.targets)
    }

    /// `pages.move_up` (Alt+Up) and `pages.move_down` (Alt+Down): the marked page records (else the cursor row's page)
    /// one position up or down within their section.
    pub(super) fn act_pages_move(&mut self, up: bool) -> Outcome {
        let moved = match self.marked_pages() {
            Ok(v) => v,
            Err(e) => return e,
        };
        let section = moved[0].section.clone();
        if moved.iter().any(|r| r.section != section) {
            return failed("error-pages-sections", &[]);
        }
        let ids: Vec<_> = moved.iter().map(|r| r.id).collect();
        let s: Vec<PageRow> = self
            .page_rows()
            .iter()
            .filter(|r| is_record(r) && r.section == section)
            .cloned()
            .collect();
        let pos = |id| s.iter().position(|r| r.id == id);
        let first = ids.iter().filter_map(|i| pos(*i)).min().unwrap_or(0);
        let last = ids.iter().filter_map(|i| pos(*i)).max().unwrap_or(0);
        if up {
            // Before the page above the first moved one.
            match s[..first].iter().rev().find(|r| !ids.contains(&r.id)) {
                Some(b) => {
                    let b = b.clone();
                    self.reorder(&moved, Some(&b))
                }
                None => Outcome::Refused {
                    reason_key: "reason-page-at-start",
                },
            }
        } else {
            // After the page below the last moved one: before the page after it, else at the end.
            let below = s[last + 1..].iter().position(|r| !ids.contains(&r.id));
            match below {
                None => Outcome::Refused {
                    reason_key: "reason-page-at-end",
                },
                Some(k) => {
                    let after = s[last + 1 + k + 1..]
                        .iter()
                        .find(|r| !ids.contains(&r.id))
                        .cloned();
                    self.reorder(&moved, after.as_ref())
                }
            }
        }
    }
}

/// A JSON object value as a parameter map.
fn json_map(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}
