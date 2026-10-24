// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The viewer's state and actions in the shell (WP-S2.03 part 1, spec sections 6.1 to 6.7): the canvas area with its
//! tabs and groups, the page-content cache, walks, jumps and the per-tab history, cross-reference links, Go to page
//! or cell, the keyboard grid cursor, the page navigator and the focus.
//!
//! **The page-content cache** keeps an `Arc<CanvasPage>` per folio for the current (session, revision): the sheet
//! display list, its entity rectangles, the link list with its resolved targets (one `Session::link_targets` call),
//! the page's cells and grid, so every `Session` call a page needs is made once per entry. At the end of each
//! dispatch it keeps every page a tab shows plus the [`RECENT_PAGES`] most recently shown others; a new revision or
//! session empties it. Back, Forward and tab switches to a cached page make no `Session` call (NFR-PERF-20).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_api::output::ew_dl::Destination;
use ew_api::output::{GotoError, PageRow};
use serde_json::{Map, Value};

use super::canvas_actions::point_mm;
use super::{Dialog, Mode, Outcome, ShellModel, failed, flag, string_param};
use crate::area::{CanvasArea, Tab};
use crate::canvas::{
    CanvasModel, CanvasPage, HEADLESS_PANE, PageId, PageLink, Viewport, reading_key, rect_centre, sort_links,
};
use crate::history::Location;
use crate::navigator::{NavMode, NavRow, NavSort, PageNavigator, RowKind};
use crate::strings;

/// How many pages no tab shows the page-content cache keeps, most recently shown first.
pub const RECENT_PAGES: usize = 8;

/// Where the keyboard focus is (WP-S2.03 §6.6): it decides which context's shortcuts the window binds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    /// The page navigator.
    Navigator,
    /// The canvas of a group.
    Canvas(usize),
    /// The property grid (WP-S2.05).
    Properties,
    // WP-S2.05 part 2
    /// The find pane.
    Find,
}

/// How many times the shell has asked the `Session` for each of the viewer's reads (the exact-count tests, spec
/// section 8 item 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SessionCalls {
    /// Sheet display lists built (`page_content`).
    pub page_content: u64,
    /// `Session::page_rows`.
    pub page_rows: u64,
    /// `Session::page_cells`.
    pub page_cells: u64,
    /// `Session::link_targets`.
    pub link_targets: u64,
    /// `Session::resolve_goto`.
    pub resolve_goto: u64,
    /// `Session::page_cps` (WP-S2.03 part 2).
    pub page_cps: u64,
    /// `Session::page_tables` (WP-S2.04 §6.2), fetched with the page content.
    pub page_tables: u64,
    /// `Session::symbol_shape` (WP-S2.04 §6.2): once per symbol copy per revision while a tool holds a shape.
    pub symbol_shape: u64,
    /// `Session::place_catalogue` (the picker, WP-S2.04 §7.1): once per (revision, folio) while it is open.
    pub place_catalogue: u64,
    /// `Session::preview_with` (the tool host's previews, and a bulk command's preview before its commit).
    pub preview_with: u64,
    /// `Session::commit` of a tool's preview (the window).
    pub commit: u64,
    /// `Session::execute_with` of a tool's command (the window, without a fresh preview).
    pub execute_with: u64,
    /// `Session::run_script` (`command.run` and the tool host's headless commits).
    pub run_script: u64,
    /// `Session::property_sheet` (WP-S2.05 part 1).
    pub property_sheet: u64,
    /// `Session::field_choices`: a choice editor opened or its filter typed (WP-S2.05 part 1).
    pub field_choices: u64,
    // WP-S2.05 part 2
    /// `Session::find`: `find.run`, and a replace or a stale replace card that runs the search again.
    pub find: u64,
    /// `Session::junction_wires` (WP-S2.04 part 2, §8.5): once per (revision, junction) while the editor is open.
    pub junction_wires: u64,
    // WP-S2.05 part 3
    /// `Session::copy_fragment`: `edit.copy`, `edit.cut`, `edit.duplicate`.
    pub copy_fragment: u64,
    /// `parse_fragment`: `edit.paste_clip` and `edit.duplicate`.
    pub parse_fragment: u64,
    /// `Session::tag_list`: `edit.copy_tags`.
    pub tag_list: u64,
}

/// Counts one of the shell's `Session` calls (a free function, so that a call can be counted while the project is
/// borrowed).
pub(super) fn bump(calls: &std::cell::Cell<SessionCalls>, f: impl FnOnce(&mut SessionCalls)) {
    let mut c = calls.get();
    f(&mut c);
    calls.set(c);
}

/// The page content of the current (session, revision), by folio, with a show counter.
#[derive(Default)]
pub(super) struct PageCache {
    key: Option<(u64, u64)>,
    pages: BTreeMap<String, (Arc<CanvasPage>, u64)>,
    shows: u64,
}

/// How a tab arrives on a page.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Arrival {
    /// The tab's remembered view of the page, else the fit.
    Remembered,
    /// This view exactly (Back, Forward).
    Exact(Viewport),
    /// The view of a jump to a point: centred at `max(zoom, 2 × fit)`.
    Jump {
        /// The sheet point.
        at: (i64, i64),
        /// The tab's zoom before the jump.
        zoom: f64,
    },
}

/// Where a jump puts the grid cursor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CursorAt {
    /// Nowhere (an arrival clears it).
    None,
    /// Exactly at the point (a link target).
    Exact((i64, i64)),
    /// At the grid point nearest to the point (a cell's centre).
    Nearest((i64, i64)),
}

/// A `usize` parameter that must be below `limit`.
fn index_param(p: &Map<String, Value>, name: &str, limit: usize) -> Result<usize, Outcome> {
    match p.get(name) {
        None => Err(failed("error-missing-param", &[name])),
        Some(v) => v
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|&n| n < limit)
            .ok_or_else(|| failed("error-bad-param", &[name])),
    }
}

/// An outcome of an area operation's refusal key.
fn refused(key: &'static str, param: &str) -> Outcome {
    if key.starts_with("error-") {
        failed(key, &[param])
    } else {
        Outcome::Refused { reason_key: key }
    }
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The canvas area while a project with pages is open.
    pub fn area(&self) -> Option<&CanvasArea> {
        self.area.as_ref()
    }

    /// Where the keyboard focus is; `None` without a project.
    pub fn focus(&self) -> Option<Focus> {
        self.focus
    }

    /// The page navigator.
    pub fn navigator(&self) -> &PageNavigator {
        &self.navigator
    }

    /// The rows of the derived page order (`Session::page_rows`, fetched once per revision).
    pub fn page_rows(&self) -> &[PageRow] {
        self.project.as_ref().map_or(&[], |p| p.rows.as_slice())
    }

    /// The navigator's rows on show.
    pub fn navigator_rows(&self) -> Vec<NavRow> {
        self.navigator.visible(self.page_rows())
    }

    /// The key of the navigator's row cursor.
    pub fn navigator_row_key(&self) -> Option<String> {
        let rows = self.navigator_rows();
        self.navigator
            .row_key(&rows, self.page_rows(), self.current_page())
    }

    /// The page of the focused group's active tab.
    pub fn current_page(&self) -> Option<PageId> {
        self.area.as_ref().map(|a| a.current().page)
    }

    /// The position in the page order of `page`.
    pub fn row_index(&self, page: PageId) -> Option<usize> {
        self.page_rows().iter().position(|r| PageId::of(r) == page)
    }

    /// The row of a tab's page.
    pub fn tab_row(&self, tab: &Tab) -> Option<&PageRow> {
        self.row_index(tab.page).map(|i| &self.page_rows()[i])
    }

    /// A tab's title: `<folio> <title>`.
    pub fn tab_title(&self, tab: &Tab) -> String {
        match self.tab_row(tab) {
            Some(r) => strings::format("tab-title", &[&r.folio, &r.title]),
            None => String::new(),
        }
    }

    /// The canvas of the active tab of `group`.
    pub fn canvas_of(&self, group: usize) -> Option<&CanvasModel> {
        self.area
            .as_ref()?
            .groups
            .get(group)?
            .active_tab()
            .canvas
            .as_ref()
    }

    /// The pane of `group` in logical points and its pixels per point (the shell's default without it).
    pub fn group_pane(&self, group: usize) -> ((f64, f64), f64) {
        match self.area.as_ref().and_then(|a| a.groups.get(group)) {
            Some(g) => (g.pane, g.ppp),
            None => (self.canvas_pane, self.canvas_ppp),
        }
    }

    /// How many times the shell has made each of the viewer's `Session` reads.
    pub fn session_calls(&self) -> SessionCalls {
        SessionCalls {
            page_content: self.page_content_calls.get(),
            ..self.calls.get()
        }
    }

    /// The folios the page-content cache holds (tests).
    pub fn cached_pages(&self) -> Vec<String> {
        self.cache.pages.keys().cloned().collect()
    }

    /// The point the status bar's cursor field shows: the pointer while it is over the focused group's canvas,
    /// else the grid cursor.
    pub fn shown_point(&self) -> Option<(i64, i64)> {
        let c = self.canvas()?;
        c.pointer().or(c.cursor())
    }

    pub(super) fn count(&self, f: impl FnOnce(&mut SessionCalls)) {
        bump(&self.calls, f);
    }

    // -----------------------------------------------------------------------------------------------------------
    // The view state of an opened project

    /// Sets up the view state of the project just installed: its page rows, one tab on the first page, the canvas
    /// focused, the navigator reset, the cache emptied. The ID aliases stay: they live for the whole script run, so an
    /// earlier `@n` never names another record (ID-630 (b), FT-422).
    pub(super) fn install_view(&mut self) {
        self.navigator = PageNavigator::new();
        self.cache = PageCache::default();
        self.focus = None;
        self.area = None;
        self.reset_tools();
        let rows = match &self.project {
            Some(p) => p.session.page_rows(),
            None => return,
        };
        self.count(|c| c.page_rows += 1);
        if let Some(p) = &mut self.project {
            p.labels = rows.iter().map(|r| r.folio.clone()).collect();
            p.rows = rows;
        }
        self.focus = Some(Focus::Canvas(0));
        if let Some(first) = self.page_rows().first() {
            self.area = Some(CanvasArea::new(
                PageId::of(first),
                self.canvas_pane,
                self.canvas_ppp,
            ));
        }
    }

    /// Takes a new revision's page rows: a tab whose page no longer exists moves to the page at the same position in
    /// page order (clamped).
    pub(super) fn refresh_rows(&mut self) {
        let Some(p) = &mut self.project else { return };
        let rows = p.session.page_rows();
        let old = std::mem::replace(&mut p.rows, rows);
        p.labels = p.rows.iter().map(|r| r.folio.clone()).collect();
        self.count(|c| c.page_rows += 1);
        let rows = self.page_rows().to_vec();
        if rows.is_empty() {
            self.area = None;
            return;
        }
        if self.area.is_none() {
            self.area = Some(CanvasArea::new(
                PageId::of(&rows[0]),
                self.canvas_pane,
                self.canvas_ppp,
            ));
            return;
        }
        let Some(area) = &mut self.area else { return };
        for g in &mut area.groups {
            for tab in &mut g.tabs {
                if rows.iter().any(|r| PageId::of(r) == tab.page) {
                    continue;
                }
                let at = old
                    .iter()
                    .position(|r| PageId::of(r) == tab.page)
                    .unwrap_or(0)
                    .min(rows.len() - 1);
                tab.page = PageId::of(&rows[at]);
                tab.canvas = None;
            }
        }
    }

    /// The page content of `folio` at the current revision, through the cache (built once per entry).
    pub(super) fn page_for(&mut self, folio: &str) -> Result<Arc<CanvasPage>, String> {
        let (Some(epoch), Some(rev)) = (self.project_epoch(), self.revision()) else {
            return Err(strings::text("reason-no-project").to_owned());
        };
        if self.cache.key != Some((epoch, rev)) {
            self.cache.pages.clear();
            self.cache.key = Some((epoch, rev));
        }
        self.cache.shows += 1;
        let shows = self.cache.shows;
        if let Some((page, n)) = self.cache.pages.get_mut(folio) {
            *n = shows;
            return Ok(Arc::clone(page));
        }
        if let Some((e, r, f, message)) = &self.canvas_error
            && (*e, *r) == (epoch, rev)
            && f == folio
        {
            return Err(message.clone());
        }
        match self.build_page(folio, epoch, rev) {
            Ok(page) => {
                let page = Arc::new(page);
                self.cache
                    .pages
                    .insert(folio.to_owned(), (Arc::clone(&page), shows));
                Ok(page)
            }
            Err(message) => {
                self.canvas_error = Some((epoch, rev, folio.to_owned(), message.clone()));
                Err(message)
            }
        }
    }

    /// Builds the page content of `folio`: the sheet display list, the grid, the cells, the connection points (and
    /// with them the snapper) and the links with their targets in reading order.
    fn build_page(&self, folio: &str, epoch: u64, rev: u64) -> Result<CanvasPage, String> {
        let pages = self.page_content(folio)?;
        let Some(first) = pages.pages.into_iter().next() else {
            return Err(strings::text("page-no-pages").to_owned());
        };
        let Some(project) = &self.project else {
            return Err(strings::text("reason-no-project").to_owned());
        };
        let grid = project.session.page_grid(folio);
        let title = first
            .title
            .strip_prefix(&format!("{folio} "))
            .unwrap_or(&first.title)
            .to_owned();
        let mut page = CanvasPage::new(
            rev,
            folio,
            &title,
            (first.width.0, first.height.0),
            Arc::new(first.content),
            grid,
        );
        page.session = epoch;
        page.cells = project.session.page_cells(folio);
        self.count(|c| c.page_cells += 1);
        page.set_cps(project.session.page_cps(folio));
        self.count(|c| c.page_cps += 1);
        if let Some(tables) = project.session.page_tables(folio) {
            page.tables = Arc::new(tables);
        }
        self.count(|c| c.page_tables += 1);
        let sources = page.list.link_sources();
        if !sources.is_empty() {
            let dests: Vec<Destination> = sources
                .iter()
                .map(|(d, _)| *d)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let targets = project.session.link_targets(&dests);
            self.count(|c| c.link_targets += 1);
            let by_dest: BTreeMap<Destination, _> = dests.into_iter().zip(targets).collect();
            let mut links: Vec<PageLink> = sources
                .into_iter()
                .enumerate()
                .map(|(paint, (dest, rect))| PageLink {
                    dest,
                    rect,
                    target: by_dest.get(&dest).cloned().flatten(),
                    paint,
                })
                .collect();
            sort_links(&mut links, page.cells.as_ref());
            page.links = links;
        }
        Ok(page)
    }

    /// Keeps the cache's pages of every tab plus the most recently shown others.
    fn trim_cache(&mut self) {
        let mut shown: BTreeSet<String> = match &self.area {
            Some(a) => a
                .tabs()
                .filter_map(|(_, _, t)| self.tab_row(t).map(|r| r.folio.clone()))
                .collect(),
            None => BTreeSet::new(),
        };
        // The link preview's page is on show too (WP-S2.03 §7.8).
        if let Some(lp) = self.link_preview() {
            shown.insert(lp.target.folio.clone());
        }
        let mut others: Vec<(u64, String)> = self
            .cache
            .pages
            .iter()
            .filter(|(f, _)| !shown.contains(*f))
            .map(|(f, (_, n))| (*n, f.clone()))
            .collect();
        others.sort_unstable_by(|a, b| b.cmp(a));
        let keep: BTreeSet<String> = others.into_iter().take(RECENT_PAGES).map(|(_, f)| f).collect();
        self.cache
            .pages
            .retain(|f, _| shown.contains(f) || keep.contains(f));
    }

    /// Shows `page` in the active tab of group `g`, arriving by `arrival`.
    fn show(&mut self, g: usize, page: PageId, arrival: Arrival) {
        let Some(i) = self.row_index(page) else { return };
        let folio = self.page_rows()[i].folio.clone();
        let built = self.page_for(&folio);
        let ui_scale = self.canvas_ui_scale();
        let Some(area) = &mut self.area else { return };
        let Some(group) = area.groups.get_mut(g) else {
            return;
        };
        let (pane, ppp) = (group.pane, group.ppp);
        let tab = group.active_tab_mut();
        tab.page = page;
        tab.canvas = built.ok().map(|content| {
            let mut c = CanvasModel::new(content, pane, ppp);
            c.set_ui_scale(ui_scale);
            match arrival {
                Arrival::Remembered => {
                    if let Some(v) = tab.recall(page) {
                        c.set_viewport(&v);
                    }
                }
                Arrival::Exact(v) => c.set_viewport(&v),
                Arrival::Jump { at, zoom } => c.jump_view(at, zoom),
            }
            c
        });
    }

    /// Stores the view of group `g`'s active tab in its memory.
    fn remember_view(&mut self, g: usize) {
        if let Some(tab) = self
            .area
            .as_mut()
            .and_then(|a| a.groups.get_mut(g))
            .map(|g| g.active_tab_mut())
            && let Some(v) = tab.canvas.as_ref().map(CanvasModel::viewport)
        {
            let page = tab.page;
            tab.remember(page, v);
        }
    }

    /// The location of group `g`'s active tab.
    fn location(&self, g: usize) -> Option<Location> {
        let tab = self.area.as_ref()?.groups.get(g)?.active_tab();
        let viewport = tab.canvas.as_ref().map_or(
            Viewport {
                centre: (0, 0),
                zoom: 1.0,
                fitted: true,
            },
            CanvasModel::viewport,
        );
        Some(Location {
            page: tab.page,
            viewport,
        })
    }

    /// A walk (Page Up, Page Down, first, last): the focused tab moves to page `index` without a history entry.
    pub(super) fn walk(&mut self, index: usize) -> Outcome {
        let Some(row) = self
            .page_rows()
            .get(index.min(self.page_count().saturating_sub(1)))
        else {
            return Outcome::Refused {
                reason_key: "reason-no-pages",
            };
        };
        let page = PageId::of(row);
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Done;
        };
        if self.current_page() == Some(page) {
            return Outcome::Done;
        }
        self.remember_view(g);
        self.show(g, page, Arrival::Remembered);
        Outcome::Done
    }

    /// A jump of group `g`'s active tab to `page`, to the point `at` by the jump view, or arriving: the location it
    /// leaves goes on the tab's back list unless it is the location it reaches.
    fn jump(&mut self, g: usize, page: PageId, at: Option<(i64, i64)>, cursor: CursorAt) {
        let Some(from) = self.location(g) else { return };
        let zoom = self.canvas_of(g).map_or(1.0, CanvasModel::zoom);
        self.remember_view(g);
        let arrival = match at {
            Some(at) => Arrival::Jump { at, zoom },
            None => Arrival::Remembered,
        };
        self.show(g, page, arrival);
        if let Some(c) = self.canvas_mut(g) {
            match cursor {
                CursorAt::None => {}
                CursorAt::Exact(p) => c.put_cursor(Some(p)),
                CursorAt::Nearest(p) => c.place_cursor(p),
            }
        }
        let Some(to) = self.location(g) else { return };
        if from != to
            && let Some(a) = &mut self.area
        {
            a.groups[g].active_tab_mut().history.record(from);
        }
    }

    /// The canvas of group `g`'s active tab, to change.
    pub(super) fn canvas_mut(&mut self, g: usize) -> Option<&mut CanvasModel> {
        self.area
            .as_mut()?
            .groups
            .get_mut(g)?
            .active_tab_mut()
            .canvas
            .as_mut()
    }

    /// The optional `group` parameter (default: the focused group).
    pub(super) fn group_param(&self, p: &Map<String, Value>) -> Result<usize, Outcome> {
        let groups = self.area.as_ref().map_or(1, |a| a.groups.len());
        match p.get("group") {
            None => Ok(self.area.as_ref().map_or(0, |a| a.focused)),
            Some(_) => index_param(p, "group", groups),
        }
    }

    /// The end of every dispatch: every visible canvas shows its page at the current revision (a new revision of
    /// the same page keeps the view), the focus follows the focused group, and the cache is trimmed.
    pub(super) fn sync_views(&mut self) {
        if self.project.is_none() {
            self.area = None;
            self.focus = None;
            return;
        }
        let (Some(epoch), Some(rev)) = (self.project_epoch(), self.revision()) else {
            return;
        };
        let groups = self.area.as_ref().map_or(0, |a| a.groups.len());
        for g in 0..groups {
            let Some(tab) = self.area.as_ref().map(|a| a.groups[g].active_tab()) else {
                continue;
            };
            let page = tab.page;
            let current = tab.canvas.as_ref().map(|c| (c.page().session, c.page().rev));
            let Some(i) = self.row_index(page) else { continue };
            match current {
                Some((s, r)) if s == epoch && r == rev => {}
                Some((s, _)) if s == epoch => {
                    let folio = self.page_rows()[i].folio.clone();
                    let built = self.page_for(&folio);
                    if let Some(tab) = self.area.as_mut().map(|a| a.groups[g].active_tab_mut()) {
                        match built {
                            Ok(content) => {
                                if let Some(c) = &mut tab.canvas {
                                    c.replace_page(content);
                                }
                            }
                            Err(_) => tab.canvas = None,
                        }
                    }
                }
                _ => self.show(g, page, Arrival::Remembered),
            }
        }
        // A tab moved across the groups (`tab.move`, side by side off) takes its new group's pane.
        if let Some(a) = &mut self.area {
            for group in &mut a.groups {
                let (pane, ppp) = (group.pane, group.ppp);
                if let Some(c) = &mut group.active_tab_mut().canvas
                    && (c.pane() != pane || c.ppp() != ppp)
                {
                    c.resize(pane, ppp);
                }
            }
        }
        if let (Some(Focus::Canvas(_)), Some(a)) = (self.focus, &self.area) {
            self.focus = Some(Focus::Canvas(a.focused));
        }
        self.trim_cache();
    }

    // -----------------------------------------------------------------------------------------------------------
    // Tabs and groups (§6.2)

    fn area_op(
        &mut self,
        f: impl FnOnce(&mut CanvasArea) -> Result<(), &'static str>,
        param: &str,
    ) -> Outcome {
        let Some(area) = &mut self.area else {
            return Outcome::Refused {
                reason_key: "reason-no-pages",
            };
        };
        match f(area) {
            Ok(()) => Outcome::Done,
            Err(key) => refused(key, param),
        }
    }

    pub(super) fn act_tab_new(&mut self) -> Outcome {
        self.area_op(CanvasArea::new_tab, "")
    }

    pub(super) fn act_tab_close(&mut self) -> Outcome {
        self.area_op(CanvasArea::close_tab, "")
    }

    pub(super) fn act_tab_cycle(&mut self, step: isize) -> Outcome {
        self.area_op(|a| a.cycle(step), "")
    }

    pub(super) fn act_focus_other(&mut self) -> Outcome {
        let out = self.area_op(CanvasArea::focus_other, "");
        if out == Outcome::Done {
            self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
        }
        out
    }

    pub(super) fn act_tab_activate(&mut self, p: &Map<String, Value>) -> Outcome {
        let groups = self.area.as_ref().map_or(0, |a| a.groups.len());
        let g = match index_param(p, "group", groups) {
            Ok(g) => g,
            Err(e) => return e,
        };
        let tabs = self.area.as_ref().map_or(0, |a| a.groups[g].tabs.len());
        let i = match index_param(p, "index", tabs) {
            Ok(i) => i,
            Err(e) => return e,
        };
        let out = self.area_op(|a| a.activate(g, i), "index");
        if out == Outcome::Done && matches!(self.focus, Some(Focus::Canvas(_))) {
            self.focus = Some(Focus::Canvas(g));
        }
        out
    }

    pub(super) fn act_tab_move(&mut self, p: &Map<String, Value>) -> Outcome {
        let groups = self.area.as_ref().map_or(0, |a| a.groups.len());
        let (g, to_g) = match (
            index_param(p, "group", groups),
            index_param(p, "to_group", groups),
        ) {
            (Ok(g), Ok(t)) => (g, t),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let tabs = self.area.as_ref().map_or(0, |a| a.groups[g].tabs.len());
        let i = match index_param(p, "index", tabs) {
            Ok(i) => i,
            Err(e) => return e,
        };
        let to_i = match p.get("to_index").and_then(Value::as_u64) {
            Some(n) => usize::try_from(n).unwrap_or(usize::MAX),
            None => return failed("error-missing-param", &["to_index"]),
        };
        self.area_op(|a| a.move_tab(g, i, to_g, to_i), "index")
    }

    pub(super) fn act_tab_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let folio = match string_param(p, "folio") {
            Err(e) => return e,
            Ok(Some(f)) => f.to_owned(),
            Ok(None) => return failed("error-missing-param", &["folio"]),
        };
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        let Some(row) = self.page_rows().iter().find(|r| r.folio == folio) else {
            return failed("error-no-folio", &[&folio]);
        };
        let page = PageId::of(row);
        let out = self.area_op(|a| a.open(page, g), "group");
        if out == Outcome::Done {
            self.show(g, page, Arrival::Remembered);
        }
        out
    }

    pub(super) fn act_side_by_side(&mut self) -> Outcome {
        let Some(area) = &self.area else {
            return Outcome::Refused {
                reason_key: "reason-no-pages",
            };
        };
        if area.side_by_side() {
            if let Some(a) = &mut self.area {
                a.join();
            }
            if matches!(self.focus, Some(Focus::Canvas(_))) {
                self.focus = Some(Focus::Canvas(0));
            }
            return Outcome::Done;
        }
        let index = (self.page_index() + 1).min(self.page_count().saturating_sub(1));
        let page = PageId::of(&self.page_rows()[index]);
        let out = self.area_op(|a| a.split(page, HEADLESS_PANE, 1.0), "");
        if out == Outcome::Done {
            self.show(1, page, Arrival::Remembered);
            if self.focus.is_some() {
                self.focus = Some(Focus::Canvas(1));
            }
        }
        out
    }

    // -----------------------------------------------------------------------------------------------------------
    // Jumps, history, links and Go to (§6.3 to §6.5)

    pub(super) fn act_history(&mut self, back: bool) -> Outcome {
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Refused {
                reason_key: "reason-history-empty",
            };
        };
        let Some(current) = self.location(g) else {
            return Outcome::Done;
        };
        let entry = {
            let rows: Vec<PageId> = self.page_rows().iter().map(PageId::of).collect();
            let exists = |p: PageId| rows.contains(&p);
            let Some(area) = &mut self.area else {
                return Outcome::Done;
            };
            let h = &mut area.groups[g].active_tab_mut().history;
            if back {
                h.back(current, exists)
            } else {
                h.forward(current, exists)
            }
        };
        let Some(entry) = entry else {
            return Outcome::Refused {
                reason_key: "reason-history-empty",
            };
        };
        self.remember_view(g);
        self.show(g, entry.page, Arrival::Exact(entry.viewport));
        Outcome::Done
    }

    pub(super) fn act_follow(&mut self, p: &Map<String, Value>) -> Outcome {
        let point = match point_mm(p) {
            Ok(point) => point,
            Err(e) => return e,
        };
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        let Some(c) = self.canvas_of(g) else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let index = match (point, c.cursor()) {
            (Some(at), _) | (None, Some(at)) => c.link_at(at),
            (None, None) => c.pointer().and_then(|at| c.link_at(at)),
        };
        match index {
            Some(i) => self.follow_link(g, i),
            None => Outcome::Refused {
                reason_key: "reason-no-link",
            },
        }
    }

    /// Follows link `index` of group `g`'s page: a jump to its target, the cursor at the target point.
    pub(super) fn follow_link(&mut self, g: usize, index: usize) -> Outcome {
        let Some(link) = self.canvas_of(g).and_then(|c| c.page().links.get(index)) else {
            return Outcome::Refused {
                reason_key: "reason-no-link",
            };
        };
        let Some(target) = link.target.clone() else {
            return Outcome::Refused {
                reason_key: "reason-link-external",
            };
        };
        let page = PageId::of_target(&target);
        match target.at_nm {
            Some(at) => self.jump(g, page, Some(at), CursorAt::Exact(at)),
            None => self.jump(g, page, None, CursorAt::None),
        }
        Outcome::Done
    }

    pub(super) fn act_link_step(&mut self, p: &Map<String, Value>, forward: bool) -> Outcome {
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        let Some(c) = self.canvas_mut(g) else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let page = Arc::clone(c.page());
        if page.links.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-link",
            };
        }
        let from = c.cursor().unwrap_or(c.centre());
        let here = reading_key(page.cells.as_ref(), from);
        let keys: Vec<_> = page
            .links
            .iter()
            .map(|l| reading_key(page.cells.as_ref(), rect_centre(l.rect)))
            .collect();
        let at = if forward {
            keys.iter().position(|k| *k > here).unwrap_or(0)
        } else {
            keys.iter().rposition(|k| *k < here).unwrap_or(keys.len() - 1)
        };
        c.put_cursor(Some(rect_centre(page.links[at].rect)));
        self.cursor_moved(g);
        Outcome::Done
    }

    pub(super) fn act_nav_goto(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match string_param(p, "text") {
            Err(e) => return e,
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => {
                return match self.mode {
                    Mode::Headless => failed("error-missing-param", &["text"]),
                    Mode::Gui => self.open_dialog(Dialog::GotoPage { text: String::new() }),
                };
            }
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let resolved = project.session.resolve_goto(&text);
        self.count(|c| c.resolve_goto += 1);
        let target = match resolved {
            Ok(t) => t,
            Err(GotoError::Empty) => return failed("error-goto-empty", &[]),
            Err(GotoError::NotFound) => return failed("error-goto-not-found", &[text.trim()]),
            Err(GotoError::NoSuchCell { folio, cell }) => {
                return failed("error-goto-no-cell", &[&folio, &cell]);
            }
        };
        self.dialog = None;
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Done;
        };
        let page = PageId(Destination::Page(target.page));
        match target.rect_nm {
            Some([x0, y0, x1, y1]) => {
                let centre = (x0 + (x1 - x0) / 2, y0 + (y1 - y0) / 2);
                self.jump(g, page, Some(centre), CursorAt::Nearest(centre));
            }
            None => self.jump(g, page, None, CursorAt::None),
        }
        Outcome::Done
    }

    /// Shows the page whose ID text is `id` in the focused group's active tab, as a jump from where it stands: the page
    /// a creation action just made (FT-408). Nothing happens when no page has the ID.
    pub(super) fn show_page_by_id(&mut self, id: &str) {
        let Some(page) = self
            .page_rows()
            .iter()
            .find(|r| r.id.to_string() == id)
            .map(PageId::of)
        else {
            return;
        };
        if let Some(g) = self.area.as_ref().map(|a| a.focused) {
            self.jump(g, page, None, CursorAt::None);
        }
    }

    pub(super) fn act_page_goto(&mut self, p: &Map<String, Value>) -> Outcome {
        let folio = match string_param(p, "folio") {
            Err(e) => return e,
            Ok(Some(folio)) => folio.to_owned(),
            Ok(None) => {
                return match self.mode {
                    Mode::Headless => failed("error-missing-param", &["folio"]),
                    Mode::Gui => self.open_dialog(Dialog::GotoPage { text: String::new() }),
                };
            }
        };
        let Some(page) = self.page_rows().iter().find(|r| r.folio == folio).map(PageId::of) else {
            return failed("error-no-folio", &[&folio]);
        };
        self.dialog = None;
        if let Some(g) = self.area.as_ref().map(|a| a.focused) {
            self.jump(g, page, None, CursorAt::None);
        }
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The grid cursor (§6.6)

    pub(super) fn act_cursor(&mut self, p: &Map<String, Value>, dx: i64, dy: i64, cell: bool) -> Outcome {
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        match self.canvas_mut(g) {
            Some(c) => {
                if cell {
                    c.cursor_cell_step(dx, dy);
                } else {
                    c.cursor_step(dx, dy);
                }
                self.cursor_moved(g);
                Outcome::Done
            }
            None => Outcome::Refused {
                reason_key: "reason-no-page",
            },
        }
    }

    pub(super) fn act_cursor_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let point = match point_mm(p) {
            Ok(point) => point,
            Err(e) => return e,
        };
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        match self.canvas_mut(g) {
            Some(c) => {
                match point {
                    Some(at) => c.place_cursor(at),
                    None => c.put_cursor(None),
                }
                self.cursor_moved(g);
                Outcome::Done
            }
            None => Outcome::Refused {
                reason_key: "reason-no-page",
            },
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The page navigator (§6.1)

    pub(super) fn act_pages_mode(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "mode") {
            Err(e) => e,
            Ok(None) => failed("error-missing-param", &["mode"]),
            Ok(Some(w)) => match NavMode::from_word(w) {
                Some(m) => {
                    self.navigator.mode = m;
                    self.navigator.reset_row();
                    Outcome::Done
                }
                None => failed("error-bad-param", &["mode"]),
            },
        }
    }

    pub(super) fn act_pages_sort(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "by") {
            Err(e) => e,
            Ok(None) => failed("error-missing-param", &["by"]),
            Ok(Some(w)) => match NavSort::from_word(w) {
                Some(s) => {
                    self.navigator.sort = s;
                    Outcome::Done
                }
                None => failed("error-bad-param", &["by"]),
            },
        }
    }

    pub(super) fn act_pages_filter(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "text") {
            Err(e) => e,
            Ok(None) => failed("error-missing-param", &["text"]),
            Ok(Some(t)) => {
                self.navigator.filter = t.to_owned();
                Outcome::Done
            }
        }
    }

    /// A row key parameter that names a row on show.
    fn row_param(&self, p: &Map<String, Value>) -> Result<NavRow, Outcome> {
        let key = match string_param(p, "key") {
            Err(e) => return Err(e),
            Ok(None) => return Err(failed("error-missing-param", &["key"])),
            Ok(Some(k)) => k,
        };
        self.navigator_rows()
            .into_iter()
            .find(|r| r.key == key)
            .ok_or_else(|| failed("error-bad-param", &["key"]))
    }

    pub(super) fn act_pages_toggle(&mut self, p: &Map<String, Value>) -> Outcome {
        match self.row_param(p) {
            Err(e) => e,
            Ok(row) if matches!(row.kind, RowKind::Node { .. }) => {
                self.navigator.toggle(&row.key);
                self.navigator.set_row(&row.key);
                Outcome::Done
            }
            Ok(_) => failed("error-bad-param", &["key"]),
        }
    }

    pub(super) fn act_pages_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let rows = self.navigator_rows();
        if rows.is_empty() {
            return Outcome::Done;
        }
        let key = if p.contains_key("key") {
            match self.row_param(p) {
                Ok(r) => r.key,
                Err(e) => return e,
            }
        } else if let Some(to) = p.get("to") {
            match to.as_str() {
                Some("first") => rows[0].key.clone(),
                Some("last") => rows[rows.len() - 1].key.clone(),
                _ => return failed("error-bad-param", &["to"]),
            }
        } else if let Some(step) = p.get("step") {
            let Some(step) = step.as_i64().filter(|s| *s == 1 || *s == -1) else {
                return failed("error-bad-param", &["step"]);
            };
            let here = self
                .navigator_row_key()
                .and_then(|k| rows.iter().position(|r| r.key == k))
                .unwrap_or(0);
            let at = (here as i64 + step).clamp(0, rows.len() as i64 - 1) as usize;
            rows[at].key.clone()
        } else {
            return failed("error-missing-param", &["key"]);
        };
        self.navigator.set_row(&key);
        Outcome::Done
    }

    pub(super) fn act_pages_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let index = if p.contains_key("key") {
            match self.row_param(p) {
                Err(e) => return e,
                Ok(row) => match row.kind {
                    RowKind::Page(i) => {
                        self.navigator.set_row(&row.key);
                        i
                    }
                    RowKind::Node { .. } => return failed("error-bad-param", &["key"]),
                },
            }
        } else {
            match string_param(p, "folio") {
                Err(e) => return e,
                Ok(None) => return failed("error-missing-param", &["key"]),
                Ok(Some(f)) => match self.page_rows().iter().position(|r| r.folio == f) {
                    Some(i) => i,
                    None => return failed("error-no-folio", &[f]),
                },
            }
        };
        let page = PageId::of(&self.page_rows()[index]);
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Done;
        };
        if flag(p, "new_tab") {
            let out = self.area_op(|a| a.open(page, g), "");
            if out == Outcome::Done {
                self.show(g, page, Arrival::Remembered);
            }
            return out;
        }
        self.jump(g, page, None, CursorAt::None);
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The focus (§6.6)

    pub(super) fn act_focus(&mut self, p: &Map<String, Value>) -> Outcome {
        let area = match string_param(p, "area") {
            Err(e) => return e,
            Ok(None) => return failed("error-missing-param", &["area"]),
            Ok(Some(a)) => a.to_owned(),
        };
        match area.as_str() {
            "navigator" => {
                self.focus = Some(Focus::Navigator);
                Outcome::Done
            }
            "properties" => {
                self.focus = Some(Focus::Properties);
                Outcome::Done
            }
            // WP-S2.05 part 2
            "find" => {
                self.focus = Some(Focus::Find);
                Outcome::Done
            }
            "canvas" => {
                let g = match self.group_param(p) {
                    Ok(g) => g,
                    Err(e) => return e,
                };
                if let Some(a) = &mut self.area {
                    a.focused = g;
                }
                self.focus = Some(Focus::Canvas(g));
                Outcome::Done
            }
            _ => failed("error-bad-param", &["area"]),
        }
    }

    /// F6 (`step` 1) and Shift+F6 (`step` −1): navigator, canvas group 1, canvas group 2, then the property grid and the
    /// find pane while their panes are in the layout, cyclic.
    pub(super) fn act_focus_cycle(&mut self, step: isize) -> Outcome {
        let groups = self.area.as_ref().map_or(0, |a| a.groups.len());
        let ring: Vec<Focus> = std::iter::once(Focus::Navigator)
            .chain((0..groups).map(Focus::Canvas))
            .chain(self.properties_shown().then_some(Focus::Properties))
            // WP-S2.05 part 2
            .chain(self.find_shown().then_some(Focus::Find))
            .collect();
        let here = self
            .focus
            .and_then(|f| ring.iter().position(|r| *r == f))
            .unwrap_or(0);
        let next = ring[(here as isize + step).rem_euclid(ring.len() as isize) as usize];
        if let (Focus::Canvas(g), Some(a)) = (next, &mut self.area) {
            a.focused = g;
        }
        self.focus = Some(next);
        Outcome::Done
    }
}
