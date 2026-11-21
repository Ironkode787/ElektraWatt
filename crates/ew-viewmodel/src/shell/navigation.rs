// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The viewer's state and actions in the shell: the canvas area with its
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

use ew_api::output::{GotoError, PageRow};
use ew_dl::{Destination, EntityRef};
use serde_json::{Map, Value};

use super::canvas_actions::point_mm;
use super::{Dialog, Mode, Outcome, ShellModel, failed, flag, string_param};
use crate::area::{CanvasArea, Tab};
use crate::canvas::{
    CanvasModel, CanvasPage, HEADLESS_PANE, PageId, PageLink, Viewport, reading_key, rect_centre, sort_links,
};
use crate::history::Location;
use crate::key;
use crate::navigator::{NavMode, NavRow, NavSort, PageNavigator, RowKind};
use crate::strings;
use crate::strings::Key;

/// How many pages no tab shows the page-content cache keeps, most recently shown first.
pub const RECENT_PAGES: usize = 8;

/// Where a page of a generated list of more than one page stands ([`ShellModel::list_pages`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ListPages {
    /// The page's number in the list, from 1.
    pub n: usize,
    /// The list's page count.
    pub of: usize,
    /// The folio of the list's page before, `None` on its first.
    pub previous: Option<String>,
    /// The folio of the list's page after, `None` on its last.
    pub next: Option<String>,
}

/// Where the keyboard focus is: it decides which context's shortcuts the window binds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    /// A navigator: the Pages navigator, or another open one.
    Navigator(crate::navigators::NavKind),
    /// The canvas of a group.
    Canvas(usize),
    /// The property grid.
    Properties,
    /// The find pane.
    Find,
    /// The command bar's tab row (doc 16a §5.9: first in the F6 ring).
    CommandBar,
    /// The history panel (its row cursor's keys).
    History,
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
    /// `Session::page_cps`.
    pub page_cps: u64,
    /// `Session::page_tables`, fetched with the page content.
    pub page_tables: u64,
    /// `Session::panel_tables`: fetched with the page content of a panel-layout page only.
    pub panel_tables: u64,
    /// `Session::symbol_shape`: once per symbol copy per revision while a tool holds a shape.
    pub symbol_shape: u64,
    /// `Session::place_catalogue` (the picker): once per (revision, folio) while it is open.
    pub place_catalogue: u64,
    /// `Session::preview_with` (the tool host's previews, and a bulk command's preview before its commit).
    pub preview_with: u64,
    /// `Session::commit` of a tool's preview (the window).
    pub commit: u64,
    /// `Session::execute_with` of a tool's command (the window, without a fresh preview).
    pub execute_with: u64,
    /// `Session::run_script` (`command.run` and the tool host's headless commits).
    pub run_script: u64,
    /// `Session::property_sheet`.
    pub property_sheet: u64,
    /// `Session::field_choices`: a choice editor opened or its filter typed.
    pub field_choices: u64,
    /// `Session::find`: `find.run`, and a replace or a stale replace card that runs the search again.
    pub find: u64,
    /// `Session::junction_wires`: once per (revision, junction) while the editor is open.
    pub junction_wires: u64,
    /// `Session::rack_view`: once per (revision, station) while the rack and slot editor is open.
    pub rack_view: u64,
    /// A station view's derivations (its rack, a station not made yet's rack, its catalogue): each once per revision,
    /// language, catalogue, view state and page, never for a frame or a pointer move alone.
    pub station_shown: u64,
    /// `Session::copy_fragment`: `edit.copy`, `edit.cut`, `edit.duplicate`.
    pub copy_fragment: u64,
    /// `parse_fragment`: `edit.paste_clip` and `edit.duplicate`.
    pub parse_fragment: u64,
    /// `Session::tag_list`: `edit.copy_tags`.
    pub tag_list: u64,
    /// `Session::navigator_table`: once per open navigator other than Pages and revision.
    pub navigator_table: u64,
    /// `Session::highlight_members`: once per highlight and revision.
    pub highlight_members: u64,
    /// `Session::export_job`: once per started PDF export; no other `Session` call runs off this thread.
    pub export_job: u64,
    /// `Session::preview_entities`: once per card the Preview tab shows.
    pub preview_entities: u64,
    /// `Session::history_view`: once per (project session, revision) while the history panel is open.
    pub history_view: u64,
    /// `Session::layout_part_offers` for Draw rail and Draw duct: when the tool is armed, and again only when its
    /// options or the revision change.
    pub layout_part_offers: u64,
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
        None => Err(failed(key!("error-missing-param"), &[name])),
        Some(v) => v
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|&n| n < limit)
            .ok_or_else(|| failed(key!("error-bad-param"), &[name])),
    }
}

/// An outcome of an area operation's refusal key.
fn refused(key: Key<'static>, param: &str) -> Outcome {
    if key.as_str().starts_with("error-") {
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
        let sections = self.project.as_ref().map_or(&[][..], |p| p.sections.as_slice());
        self.navigator.visible_with(self.page_rows(), sections)
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

    /// A tab's title: `<folio> <title>`; a station's view `<tag> · rack` (a station drawn nowhere by its part
    /// number, one not made yet "New station").
    pub fn tab_title(&self, tab: &Tab) -> String {
        if let Some(s) = &tab.station {
            return match s.station.and_then(|id| self.rack_view(id)) {
                Some(v) if v.tag.contains('?') && !v.part.is_empty() => strings::format(
                    key!("station-tab-title"),
                    &[v.part.split(" · ").next().unwrap_or_default()],
                ),
                Some(v) => strings::format(key!("station-tab-title"), &[&v.tag]),
                None => strings::text(key!("rack-new-title")).to_owned(),
            };
        }
        let Some(r) = self.tab_row(tab) else {
            return String::new();
        };
        let title = strings::format(key!("tab-title"), &[&r.folio, &r.title]);
        // A list of more than one page says which page the tab shows.
        match self.row_index(tab.page).and_then(|i| self.list_pages(i)) {
            Some(l) => strings::format(
                key!("tab-title-list-page"),
                &[&title, &l.n.to_string(), &l.of.to_string()],
            ),
            None => title,
        }
    }

    /// Where the generated list of the page at position `i` of the page order stands among its pages (the
    /// plugs on a parts list's second page went unseen): the run of generated list pages of one folio and its
    /// sub-pages (`901`, `901.1`, ...). `None` for a list of one page and for any other page.
    pub fn list_pages(&self, i: usize) -> Option<ListPages> {
        let rows = self.page_rows();
        let here = rows.get(i).filter(|r| r.kind == "report" && r.generated)?;
        let base = here.folio.split('.').next()?;
        let of_list = |r: &PageRow| {
            r.kind == "report"
                && r.generated
                && r.folio
                    .strip_prefix(base)
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
        };
        let start = (0..=i).rev().take_while(|&j| of_list(&rows[j])).last()?;
        let end = (i..rows.len()).take_while(|&j| of_list(&rows[j])).last()?;
        (start < end).then(|| ListPages {
            n: i - start + 1,
            of: end - start + 1,
            previous: (i > start).then(|| rows[i - 1].folio.clone()),
            next: (i < end).then(|| rows[i + 1].folio.clone()),
        })
    }

    /// The row under the last row of the list on show that runs on: its words, the folio it goes to and its
    /// sheet rectangle, nm `[left, top, right, bottom]`; `None` on any other page.
    pub fn list_foot(&self) -> Option<(String, String, [i64; 4])> {
        let next = self.list_pages(self.page_index())?.next?;
        let folio = &self.page_rows().get(self.page_index())?.folio;
        let table = self.project.as_ref()?.session.page_list_table(folio)?;
        let text = strings::format(key!("list-continues-row"), &[&next]);
        Some((text, next, table.foot))
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
            None => (self.view.pane, self.view.ppp),
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
    /// earlier `@n` never names another record.
    pub(super) fn install_view(&mut self) {
        self.navigator = PageNavigator::new();
        // A project's navigator states and chips go with it (the panes stay in the layout).
        for n in self.navs.values_mut() {
            *n = crate::navigators::Navigator::new(n.kind);
        }
        self.chips.clear();
        self.chips_for = None;
        self.last_page = None;
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
            p.sections = ew_api::output::print::sections(&p.session.query());
        }
        self.focus = Some(Focus::Canvas(0));
        if let Some(first) = self.page_rows().first() {
            let mut area = CanvasArea::new(PageId::of(first), self.view.pane, self.view.ppp);
            area.groups[0].sized = self.view.sized;
            self.area = Some(area);
        }
    }

    /// Takes a new revision's page rows: a tab whose page no longer exists moves to the page at the same position in
    /// page order (clamped).
    pub(super) fn refresh_rows(&mut self) {
        let Some(p) = &mut self.project else { return };
        let rows = p.session.page_rows();
        let old = std::mem::replace(&mut p.rows, rows);
        p.labels = p.rows.iter().map(|r| r.folio.clone()).collect();
        p.sections = ew_api::output::print::sections(&p.session.query());
        self.count(|c| c.page_rows += 1);
        let rows = self.page_rows().to_vec();
        if rows.is_empty() {
            self.area = None;
            return;
        }
        if self.area.is_none() {
            let mut area = CanvasArea::new(PageId::of(&rows[0]), self.view.pane, self.view.ppp);
            area.groups[0].sized = self.view.sized;
            self.area = Some(area);
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
            return Err(strings::text(key!("reason-no-project")).to_owned());
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
        if let Some((e, r, f, message)) = &self.view.error
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
                self.view.error = Some((epoch, rev, folio.to_owned(), message.clone()));
                Err(message)
            }
        }
    }

    /// Builds the page content of `folio`: the sheet display list, the grid, the cells, the connection points (and
    /// with them the snapper) and the links with their targets in reading order.
    fn build_page(&self, folio: &str, epoch: u64, rev: u64) -> Result<CanvasPage, String> {
        let pages = self.page_content(folio)?;
        let Some(first) = pages.pages.into_iter().next() else {
            return Err(strings::text(key!("page-no-pages")).to_owned());
        };
        let Some(project) = &self.project else {
            return Err(strings::text(key!("reason-no-project")).to_owned());
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
        if let Some(draft) = project.session.page_draft(folio) {
            page.draft = Arc::new(draft);
        }
        page.texts = Arc::new(project.session.page_slot_texts(folio));
        // A panel-layout page's panel data, once per (session, revision, folio); none for another page.
        if self
            .page_rows()
            .iter()
            .any(|r| r.folio == folio && r.kind == "panel_layout" && !r.generated)
        {
            page.panel = project.session.panel_tables(folio).map(Arc::new);
            self.count(|c| c.panel_tables += 1);
            // A legend on the page is picked by the box it draws, as a placement by its footprint.
            if let Some(panel) = page.panel.clone() {
                for b in &panel.blocks {
                    let a = page.snapper.page_to_sheet((b.drawn.min().x.0, b.drawn.min().y.0));
                    let z = page.snapper.page_to_sheet((b.drawn.max().x.0, b.drawn.max().y.0));
                    page.entities.push((EntityRef(b.id), crate::tools::rect_of(a, z)));
                }
            }
        }
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
        // The link preview's page is on show too.
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
                Arrival::Jump { at, zoom } => {
                    c.jump_view(at, zoom);
                    // The arrival, centred, is marked for a moment.
                    c.mark_arrival(at);
                }
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
                first: true,
            },
            CanvasModel::viewport,
        );
        Some(Location {
            page: tab.page,
            viewport,
        })
    }

    /// A walk (Page Up, Page Down, first, last): the focused tab moves to page `index` without a history entry.
    pub(crate) fn walk(&mut self, index: usize) -> Outcome {
        let Some(row) = self
            .page_rows()
            .get(index.min(self.page_count().saturating_sub(1)))
        else {
            return Outcome::refused(key!("reason-no-pages"));
        };
        let page = PageId::of(row);
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Done;
        };
        self.leave_station(g);
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
        self.leave_station(g);
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

    /// The context restore of an undo or redo: group `g`'s active tab shows `page` at `viewport` (a fitted view
    /// fits again); the location it leaves goes on the tab's back list unless it is the one it reaches.
    pub(super) fn restore_location(&mut self, g: usize, page: PageId, viewport: Viewport) {
        let Some(from) = self.location(g) else { return };
        if from.page == page {
            // The same page keeps its canvas (and the keyboard cursor): only the view moves.
            if let Some(c) = self.canvas_mut(g) {
                c.set_viewport(&viewport);
            }
        } else {
            // A step undone on another page than the one a station's view was opened from is shown on its page.
            self.leave_station(g);
            self.remember_view(g);
            self.show(g, page, Arrival::Exact(viewport));
        }
        let Some(to) = self.location(g) else { return };
        if from != to
            && let Some(a) = &mut self.area
        {
            a.groups[g].active_tab_mut().history.record(from);
        }
    }

    /// A jump or a walk from a station's view shows the page in a page tab (`CanvasArea::leave_station`): the
    /// station's tab stays in the strip to come back to. The page tab is built before it is used.
    pub(super) fn leave_station(&mut self, g: usize) {
        let shows_station = |s: &Self| {
            s.area
                .as_ref()
                .and_then(|a| a.groups.get(g))
                .is_some_and(|x| x.active_tab().station.is_some())
        };
        if !shows_station(self) {
            return;
        }
        if let Some(a) = &mut self.area {
            a.leave_station(g);
        }
        let tab = self.area.as_ref().map(|a| a.groups[g].active_tab());
        if let Some((page, None)) = tab.map(|t| (t.page, t.canvas.as_ref())) {
            self.show(g, page, Arrival::Remembered);
        }
    }

    /// A jump of group `g`'s active tab to `page` (a navigator row's target): one back-history entry.
    pub(super) fn jump_to(&mut self, g: usize, page: PageId) {
        self.jump(g, page, None, CursorAt::None);
    }

    /// Opens `page` in a new tab of group `g`.
    pub(super) fn open_in_new_tab(&mut self, g: usize, page: PageId) -> Outcome {
        let out = self.area_op(|a| a.open(page, g), "");
        if out == Outcome::Done {
            self.show(g, page, Arrival::Remembered);
        }
        out
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
        self.drop_gone_stations();
        let groups = self.area.as_ref().map_or(0, |a| a.groups.len());
        for g in 0..groups {
            let Some(tab) = self.area.as_ref().map(|a| a.groups[g].active_tab()) else {
                continue;
            };
            // A station's view shows no page: its tab's page is built when the tab shows it again.
            if tab.station.is_some() {
                continue;
            }
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
        // A station's view lists the place catalogue's parts: read once per revision while the view is on show.
        if self.station_tab().is_some() {
            let _ = self.catalogue();
        }
        self.trim_cache();
    }

    // -----------------------------------------------------------------------------------------------------------
    // Tabs and groups

    fn area_op(
        &mut self,
        f: impl FnOnce(&mut CanvasArea) -> Result<(), Key<'static>>,
        param: &str,
    ) -> Outcome {
        let Some(area) = &mut self.area else {
            return Outcome::refused(key!("reason-no-pages"));
        };
        match f(area) {
            Ok(()) => Outcome::Done,
            Err(key) => refused(key, param),
        }
    }

    pub(crate) fn act_tab_new(&mut self) -> Outcome {
        self.area_op(CanvasArea::new_tab, "")
    }

    /// `tab.close {confirm?}`: closes the focused group's active tab. The view of a station not made yet asks first
    /// (nothing of it is written before Create); `confirm` closes it without asking.
    pub(crate) fn act_tab_close(&mut self, p: &Map<String, Value>) -> Outcome {
        let unmade = self
            .station_draft()
            .is_some_and(|d| d.head.is_some() || !d.modules.is_empty());
        if unmade && !flag(p, "confirm") && self.mode == super::Mode::Gui {
            return self.open_dialog(super::Dialog::DiscardStation);
        }
        if matches!(self.dialog, Some(super::Dialog::DiscardStation)) {
            self.dialog = None;
        }
        self.area_op(CanvasArea::close_tab, "")
    }

    pub(crate) fn act_tab_cycle(&mut self, step: isize) -> Outcome {
        self.area_op(|a| a.cycle(step), "")
    }

    pub(crate) fn act_focus_other(&mut self) -> Outcome {
        let out = self.area_op(CanvasArea::focus_other, "");
        if out == Outcome::Done {
            self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
        }
        out
    }

    pub(crate) fn act_tab_activate(&mut self, p: &Map<String, Value>) -> Outcome {
        let groups = self.area.as_ref().map_or(0, |a| a.groups.len());
        let g = or_return!(index_param(p, "group", groups));
        let tabs = self.area.as_ref().map_or(0, |a| a.groups[g].tabs.len());
        let i = or_return!(index_param(p, "index", tabs));
        let out = self.area_op(|a| a.activate(g, i), "index");
        if out == Outcome::Done && matches!(self.focus, Some(Focus::Canvas(_))) {
            self.focus = Some(Focus::Canvas(g));
        }
        out
    }

    pub(crate) fn act_tab_move(&mut self, p: &Map<String, Value>) -> Outcome {
        let groups = self.area.as_ref().map_or(0, |a| a.groups.len());
        let (g, to_g) = match (
            index_param(p, "group", groups),
            index_param(p, "to_group", groups),
        ) {
            (Ok(g), Ok(t)) => (g, t),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let tabs = self.area.as_ref().map_or(0, |a| a.groups[g].tabs.len());
        let i = or_return!(index_param(p, "index", tabs));
        let to_i = match p.get("to_index").and_then(Value::as_u64) {
            Some(n) => usize::try_from(n).unwrap_or(usize::MAX),
            None => return failed(key!("error-missing-param"), &["to_index"]),
        };
        self.area_op(|a| a.move_tab(g, i, to_g, to_i), "index")
    }

    pub(crate) fn act_tab_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let folio = match string_param(p, "folio") {
            Err(e) => return e,
            Ok(Some(f)) => f.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["folio"]),
        };
        let g = or_return!(self.group_param(p));
        let Some(row) = PageRow::by_folio(self.page_rows(), &folio) else {
            return failed(key!("error-no-folio"), &[&folio]);
        };
        let page = PageId::of(row);
        let out = self.area_op(|a| a.open(page, g), "group");
        if out == Outcome::Done {
            self.show(g, page, Arrival::Remembered);
        }
        out
    }

    pub(crate) fn act_side_by_side(&mut self) -> Outcome {
        let Some(area) = &self.area else {
            return Outcome::refused(key!("reason-no-pages"));
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
    // Jumps, history, links and Go to

    pub(crate) fn act_history(&mut self, back: bool) -> Outcome {
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::refused(key!("reason-history-empty"));
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
            return Outcome::refused(key!("reason-history-empty"));
        };
        self.remember_view(g);
        self.show(g, entry.page, Arrival::Exact(entry.viewport));
        Outcome::Done
    }

    pub(crate) fn act_follow(&mut self, p: &Map<String, Value>) -> Outcome {
        let point = or_return!(point_mm(p));
        let g = or_return!(self.group_param(p));
        let Some(c) = self.canvas_of(g) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let index = match (point, c.cursor()) {
            (Some(at), _) | (None, Some(at)) => c.link_at(at),
            (None, None) => c.pointer().and_then(|at| c.link_at(at)),
        };
        match index {
            Some(i) => self.follow_link(g, i),
            // The selected coil of a valve's solenoid goes to its valve, as the coil's menu does (a re-test:
            // Follow reference did nothing visible).
            None => match self.selected_coil_valve() {
                Some(params) => self.dispatch("nav.show", &params),
                None => Outcome::refused(key!("reason-no-link")),
            },
        }
    }

    /// The `nav.show` parameters that go from the one selected symbol, the coil of a valve's solenoid, to its valve;
    /// `None` for any other selection.
    fn selected_coil_valve(&self) -> Option<Value> {
        let page = self.canvas()?.page();
        let [e] = crate::tools::transform::selected_reps(self.selection(), page)[..] else {
            return None;
        };
        let rep = page.tables.reps.iter().find(|r| r.id == e.0)?;
        let [function] = rep.functions[..] else {
            return None;
        };
        let (tag, name, _) = self.project_session()?.solenoid_of(function)?;
        let words = strings::format(key!("nav-show-solenoid"), &[&tag, &name]);
        Some(serde_json::json!({ "record": rep.device?.to_string(), "words": words }))
    }

    /// Follows link `index` of group `g`'s page: a jump to its target, the cursor at the target point.
    pub(super) fn follow_link(&mut self, g: usize, index: usize) -> Outcome {
        let Some(link) = self.canvas_of(g).and_then(|c| c.page().links.get(index)) else {
            return Outcome::refused(key!("reason-no-link"));
        };
        let Some(target) = link.target.clone() else {
            return Outcome::refused(key!("reason-link-external"));
        };
        let page = PageId::of_target(&target);
        match target.at_nm {
            Some(at) => self.jump(g, page, Some(at), CursorAt::Exact(at)),
            None => self.jump(g, page, None, CursorAt::None),
        }
        Outcome::Done
    }

    /// `props.follow {row?}` (F or a click on a link row of the grid): follows the device link of
    /// `row` (else the focused row) as the Devices navigator opens the device: its main page shows in the focused
    /// group and its drawings there are selected. A valve's solenoid row goes to where its coil is drawn, or draws it
    /// while it is not (`fluid.draw_solenoid`); a coil's valve row goes to the valve; a multipin terminal's
    /// plug row goes to the plug, or draws it while it is not (`fluid.draw_plug`). A draw goes to the electrical page
    /// `page` names, else as its action decides (the page in front, or the window asks).
    pub(crate) fn act_props_follow(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = or_return!(self.props_row_param(p));
        // A wire's value its cable core gives: the cable, where it is changed.
        if self.cable_row(&key).is_some() {
            return self.props_go_to_source(&key);
        }
        let solenoid = self
            .props_find(&key)
            .and_then(|(_, row)| match (row.ty.inner(), &row.value) {
                (
                    crate::properties::FieldType::Link { kind },
                    crate::properties::RowValue::One { json, .. },
                ) if kind == "solenoid" => Some(json.clone()),
                _ => None,
            });
        // A multipin terminal's plug row: where it is drawn, or draws it while it is not.
        let plug = self
            .props_find(&key)
            .and_then(|(_, row)| match (row.ty.inner(), &row.value) {
                (
                    crate::properties::FieldType::Link { kind },
                    crate::properties::RowValue::One { json, .. },
                ) if kind == "plug" => Some(json.clone()),
                _ => None,
            });
        // The electrical page a draw goes to (`page`, as the window's form asks it; a script names it).
        let page = p.get("page").cloned().unwrap_or(Value::Null);
        if let Some(json) = plug {
            return match json.get("drawn").and_then(Value::as_str) {
                Some(place) => self.dispatch("nav.goto", &serde_json::json!({ "text": place })),
                None => {
                    let t = json.get("terminal").cloned().unwrap_or(Value::Null);
                    self.dispatch(
                        "fluid.draw_plug",
                        &serde_json::json!({ "terminal": t, "page": page }),
                    )
                }
            };
        }
        if let Some(json) = solenoid {
            let mut params = Map::new();
            // A terminal valve's solenoid: its output in the PLC list, or its plug pin on the page.
            let seat = &json["seat"];
            match seat.get("kind").and_then(Value::as_str) {
                Some("terminal") => {
                    let channel = seat.get("channel").cloned().unwrap_or(Value::Null);
                    return self.dispatch("fluid.go_output", &serde_json::json!({ "channel": channel }));
                }
                Some(_) => {
                    return match seat.get("plug").and_then(Value::as_str) {
                        Some(place) => self.dispatch("nav.goto", &serde_json::json!({ "text": place })),
                        None => {
                            let t = seat.get("terminal_device").cloned().unwrap_or(Value::Null);
                            self.dispatch(
                                "fluid.draw_plug",
                                &serde_json::json!({ "terminal": t, "page": page }),
                            )
                        }
                    };
                }
                None => {}
            }
            return match json.get("drawn").and_then(Value::as_str) {
                Some(_) => {
                    let name = json.get("name").and_then(Value::as_str).unwrap_or_default();
                    let tag = self
                        .props
                        .sheet()
                        .map(|s| s.header.title.clone())
                        .unwrap_or_default();
                    params.insert(
                        "record".into(),
                        json.get("function").cloned().unwrap_or(Value::Null),
                    );
                    let words = strings::format(key!("nav-show-solenoid"), &[&tag, name]);
                    params.insert("words".into(), Value::String(words));
                    self.act_nav_show(&params)
                }
                None => {
                    if let Some(f) = json.get("function") {
                        params.insert("function".into(), f.clone());
                    }
                    params.insert("page".into(), page);
                    self.act_fluid_draw_solenoid(&params)
                }
            };
        }
        let device = self
            .props_find(&key)
            .and_then(|(_, row)| match (row.ty.inner(), &row.value) {
                (
                    crate::properties::FieldType::Link { kind },
                    crate::properties::RowValue::One { json, text },
                ) if kind == "device" => json
                    .as_str()
                    .and_then(|id| id.parse::<ew_api::output::Uuid>().ok())
                    .map(|id| (id, text.clone())),
                (
                    crate::properties::FieldType::Link { kind },
                    crate::properties::RowValue::One { json, .. },
                ) if kind == "valve" => json
                    .get("device")
                    .and_then(Value::as_str)
                    .and_then(|id| id.parse::<ew_api::output::Uuid>().ok())
                    .map(|id| {
                        (
                            id,
                            json.get("tag")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                        )
                    }),
                _ => None,
            });
        let Some((device, tag)) = device else {
            return Outcome::refused(key!("reason-row-no-link"));
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        // The device's row of the Devices navigator (a terminal's of the Strips navigator) knows its main page and
        // its drawings there (main function first, then page order).
        let open = [ew_api::output::NavKind::Devices, ew_api::output::NavKind::Strips]
            .into_iter()
            .find_map(|kind| {
                project
                    .session
                    .navigator_table(kind)
                    .nodes
                    .into_iter()
                    .find(|n| n.records.first() == Some(&device) && n.open.is_some())
                    .and_then(|n| n.open)
            });
        let Some(open) = open else {
            return failed(key!("error-device-not-drawn"), &[&tag]);
        };
        let page = PageId(Destination::Page(open.page));
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Done;
        };
        if self.row_index(page).is_none() {
            return Outcome::refused(key!("reason-no-page"));
        }
        self.jump_to(g, page);
        let items: Vec<ew_dl::EntityRef> = open.select.iter().map(|u| ew_dl::EntityRef(*u)).collect();
        if self
            .tools
            .selection
            .apply(&crate::tools::SelectionChange::Replace(items))
        {
            self.announce_selection();
        }
        Outcome::Done
    }

    pub(crate) fn act_link_step(&mut self, p: &Map<String, Value>, forward: bool) -> Outcome {
        let g = or_return!(self.group_param(p));
        let Some(c) = self.canvas_mut(g) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let page = Arc::clone(c.page());
        if page.links.is_empty() {
            return Outcome::refused(key!("reason-no-link"));
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

    pub(crate) fn act_nav_goto(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match string_param(p, "text") {
            Err(e) => return e,
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => {
                return match self.mode {
                    Mode::Headless => failed(key!("error-missing-param"), &["text"]),
                    Mode::Gui => self.open_dialog(Dialog::GotoPage { text: String::new() }),
                };
            }
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let resolved = project.session.resolve_goto(&text);
        self.count(|c| c.resolve_goto += 1);
        let target = match resolved {
            Ok(t) => t,
            Err(GotoError::Empty) => return failed(key!("error-goto-empty"), &[]),
            Err(GotoError::NotFound) => return failed(key!("error-goto-not-found"), &[text.trim()]),
            Err(GotoError::NoSuchCell { folio, cell }) => {
                return failed(key!("error-goto-no-cell"), &[&folio, &cell]);
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

    /// `nav.show {record, words?}`: Go to a thing by its drawing, not by its cell: the page of the first drawing of
    /// function `record` (of device `record`'s main function) shows with the symbol in the middle, selected (its whole
    /// outline marked), and `words` stand beside it until the pointer moves (so that a ring at the cell's
    /// middle is not read as nothing). Refused in words for a thing drawn nowhere.
    pub(crate) fn act_nav_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let record = match p
            .get("record")
            .and_then(Value::as_str)
            .map(str::parse::<ew_api::output::Uuid>)
        {
            Some(Ok(id)) => id,
            _ => return failed(key!("error-missing-param"), &["record"]),
        };
        let words = match string_param(p, "words") {
            Ok(w) => w.unwrap_or_default().to_owned(),
            Err(e) => return e,
        };
        let Some((page, rep)) = self
            .project
            .as_ref()
            .and_then(|x| x.session.first_drawing(record))
        else {
            return Outcome::refused(key!("reason-nav-not-placed"));
        };
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Done;
        };
        let page = PageId(Destination::Page(page));
        if self.row_index(page).is_none() {
            return Outcome::refused(key!("reason-no-page"));
        }
        self.jump_to(g, page);
        let Some(body) = self
            .canvas_of(g)
            .and_then(|c| c.page().tables.reps.iter().find(|r| r.id == rep).map(|r| r.body))
        else {
            return Outcome::Done;
        };
        let centre = (
            body[0] + (body[2] - body[0]) / 2,
            body[1] + (body[3] - body[1]) / 2,
        );
        self.jump(g, page, Some(centre), CursorAt::None);
        // The page change resets the tool now, not after the mark is set.
        self.sync_tool_context();
        if self
            .tools
            .selection
            .apply(&crate::tools::SelectionChange::Replace(vec![ew_dl::EntityRef(
                rep,
            )]))
        {
            self.announce_selection();
        }
        if !words.is_empty()
            && let Some(at) = self
                .canvas_of(g)
                .map(|c| c.page().snapper.page_to_sheet((body[2], body[1])))
        {
            self.tools.overlay = crate::tools::Overlay::new(vec![crate::tools::OverlayItem::Label {
                at,
                text: words,
                anchor: crate::tools::Anchor::Right,
            }]);
        }
        Outcome::Done
    }

    /// Shows the page whose ID text is `id` in the focused group's active tab, as a jump from where it stands: the page
    /// a creation action just made. Nothing happens when no page has the ID.
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

    /// The folio of the first page of the project's parts list (FT-717): the first page in page order of a
    /// `parts_summary` report block (the BOM), else of a `parts_list` block; `None` when the project has neither
    /// with a page.
    pub fn parts_list_folio(&self) -> Option<String> {
        let project = self.project.as_ref()?;
        let q = project.session.query();
        let rows = self.page_rows();
        ["parts_summary", "parts_list"].into_iter().find_map(|kind| {
            q.report_blocks()
                .iter()
                .filter(|(_, b)| b.embedded.is_none() && ew_api::output::block_kind(b) == kind)
                .filter_map(|(id, _)| {
                    let folios = ew_api::output::report_folios(&q, &id.to_string()).ok()?;
                    let first = folios.into_iter().next()?;
                    let at = rows.iter().position(|r| r.folio == first)?;
                    Some((at, first))
                })
                .min()
                .map(|(_, folio)| folio)
        })
    }

    /// `lists.parts` (FT-717): opens the first page of the parts list in the focused group, as `page.goto` does.
    /// The command ends with that page fitted in the canvas and its row selected in the page tree,
    /// and the earlier export's toast dismissed, so its effect is in view.
    pub(crate) fn act_lists_parts(&mut self) -> Outcome {
        match self.parts_list_folio() {
            Some(folio) => {
                let mut p = Map::new();
                p.insert("folio".to_owned(), Value::String(folio.clone()));
                let out = self.act_page_goto(&p);
                if out != Outcome::Done {
                    return out;
                }
                let _ = self.act_fit_page(&Map::new());
                let rows = self.page_rows().to_vec();
                if let Some(i) = rows.iter().position(|r| r.folio == folio) {
                    let key = self.navigator.key_of(&rows, i);
                    let mut select = Map::new();
                    select.insert("kind".to_owned(), Value::from("pages"));
                    select.insert("keys".to_owned(), Value::from(vec![key]));
                    let _ = self.act_navigator_select(&select);
                }
                self.dismiss_export_toasts();
                self.notify_unlisted();
                Outcome::Done
            }
            None => Outcome::refused(key!("reason-no-parts-list")),
        }
    }

    /// Takes the visible toasts of an export off the screen (the log keeps them).
    fn dismiss_export_toasts(&mut self) {
        let export: Vec<u64> = self
            .notes
            .visible()
            .iter()
            .filter(|n| n.key == "export")
            .map(|n| n.serial)
            .collect();
        for serial in export {
            self.notes.dismiss(serial);
        }
    }

    pub(crate) fn act_page_goto(&mut self, p: &Map<String, Value>) -> Outcome {
        let folio = match string_param(p, "folio") {
            Err(e) => return e,
            Ok(Some(folio)) => folio.to_owned(),
            Ok(None) => {
                return match self.mode {
                    Mode::Headless => failed(key!("error-missing-param"), &["folio"]),
                    Mode::Gui => self.open_dialog(Dialog::GotoPage { text: String::new() }),
                };
            }
        };
        let Some(page) = PageRow::by_folio(self.page_rows(), &folio).map(PageId::of) else {
            return failed(key!("error-no-folio"), &[&folio]);
        };
        self.dialog = None;
        if let Some(g) = self.area.as_ref().map(|a| a.focused) {
            self.jump(g, page, None, CursorAt::None);
        }
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The grid cursor

    pub(crate) fn act_cursor(&mut self, p: &Map<String, Value>, dx: i64, dy: i64, cell: bool) -> Outcome {
        // In a station's view the arrows go from slot to slot, or from cell to cell of its channel table.
        if self.station_tab().is_some() {
            if self.step_channel_cursor(dx, dy) {
                return Outcome::Done;
            }
            let step = Map::from_iter([("step".to_owned(), Value::from(dx + dy))]);
            return self.act_plc_rack_select(&step);
        }
        let g = or_return!(self.group_param(p));
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
            None => Outcome::refused(key!("reason-no-page")),
        }
    }

    pub(crate) fn act_cursor_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let point = or_return!(point_mm(p));
        let g = or_return!(self.group_param(p));
        match self.canvas_mut(g) {
            Some(c) => {
                match point {
                    Some(at) => c.place_cursor(at),
                    None => c.put_cursor(None),
                }
                self.cursor_moved(g);
                Outcome::Done
            }
            None => Outcome::refused(key!("reason-no-page")),
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The page navigator

    pub(crate) fn act_pages_mode(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "mode") {
            Err(e) => e,
            Ok(None) => failed(key!("error-missing-param"), &["mode"]),
            Ok(Some(w)) => match NavMode::from_word(w) {
                Some(m) => {
                    if self.navigator.mode != m {
                        // The keys change with the view, so the marks go.
                        self.navigator.marked.clear();
                        self.navigator.anchor = None;
                    }
                    self.navigator.mode = m;
                    self.navigator.reset_row();
                    Outcome::Done
                }
                None => failed(key!("error-bad-param"), &["mode"]),
            },
        }
    }

    pub(crate) fn act_pages_sort(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "by") {
            Err(e) => e,
            Ok(None) => failed(key!("error-missing-param"), &["by"]),
            Ok(Some(w)) => match NavSort::from_word(w) {
                Some(s) => {
                    self.navigator.sort = s;
                    Outcome::Done
                }
                None => failed(key!("error-bad-param"), &["by"]),
            },
        }
    }

    pub(crate) fn act_pages_filter(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "text") {
            Err(e) => e,
            Ok(None) => failed(key!("error-missing-param"), &["text"]),
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
            Ok(None) => return Err(failed(key!("error-missing-param"), &["key"])),
            Ok(Some(k)) => k,
        };
        self.navigator_rows()
            .into_iter()
            .find(|r| r.key == key)
            .ok_or_else(|| failed(key!("error-bad-param"), &["key"]))
    }

    pub(crate) fn act_pages_toggle(&mut self, p: &Map<String, Value>) -> Outcome {
        match self.row_param(p) {
            Err(e) => e,
            Ok(row) if matches!(row.kind, RowKind::Node { .. }) => {
                self.navigator.toggle(&row.key);
                self.navigator.set_row(&row.key);
                Outcome::Done
            }
            Ok(_) => failed(key!("error-bad-param"), &["key"]),
        }
    }

    pub(crate) fn act_pages_row(&mut self, p: &Map<String, Value>) -> Outcome {
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
                _ => return failed(key!("error-bad-param"), &["to"]),
            }
        } else if let Some(step) = p.get("step") {
            let Some(step) = step.as_i64().filter(|s| *s == 1 || *s == -1) else {
                return failed(key!("error-bad-param"), &["step"]);
            };
            let here = self
                .navigator_row_key()
                .and_then(|k| rows.iter().position(|r| r.key == k))
                .unwrap_or(0);
            let at = (here as i64).saturating_add(step).clamp(0, rows.len() as i64 - 1) as usize;
            rows[at].key.clone()
        } else {
            return failed(key!("error-missing-param"), &["key"]);
        };
        self.navigator.set_row(&key);
        Outcome::Done
    }

    pub(crate) fn act_pages_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let index = if p.contains_key("key") {
            match self.row_param(p) {
                Err(e) => return e,
                Ok(row) => match row.kind {
                    RowKind::Page(i) => {
                        self.navigator.set_row(&row.key);
                        i
                    }
                    RowKind::Node { .. } => return failed(key!("error-bad-param"), &["key"]),
                },
            }
        } else {
            match string_param(p, "folio") {
                Err(e) => return e,
                Ok(None) => return failed(key!("error-missing-param"), &["key"]),
                Ok(Some(f)) => match self.page_rows().iter().position(|r| r.folio == f) {
                    Some(i) => i,
                    None => return failed(key!("error-no-folio"), &[f]),
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
    // The focus

    pub(crate) fn act_focus(&mut self, p: &Map<String, Value>) -> Outcome {
        let area = match string_param(p, "area") {
            Err(e) => return e,
            Ok(None) => return failed(key!("error-missing-param"), &["area"]),
            Ok(Some(a)) => a.to_owned(),
        };
        match area.as_str() {
            "navigator" => {
                // `kind` names an open navigator (default Pages).
                let kind = match string_param(p, "kind") {
                    Err(e) => return e,
                    Ok(None) => crate::navigators::NavKind::Pages,
                    Ok(Some(w)) => match crate::navigators::NavKind::from_word(w) {
                        Some(k) if k == crate::navigators::NavKind::Pages || self.navs.contains_key(&k) => k,
                        _ => return failed(key!("error-bad-param"), &["kind"]),
                    },
                };
                self.focus = Some(Focus::Navigator(kind));
                self.bring_navigator_to_front(kind);
                Outcome::Done
            }
            "properties" => {
                self.focus = Some(Focus::Properties);
                Outcome::Done
            }
            "find" => {
                self.focus = Some(Focus::Find);
                Outcome::Done
            }
            "commandbar" => {
                self.focus = Some(Focus::CommandBar);
                Outcome::Done
            }
            "history" if self.history_shown() => {
                self.focus = Some(Focus::History);
                Outcome::Done
            }
            "canvas" => {
                let g = or_return!(self.group_param(p));
                if let Some(a) = &mut self.area {
                    a.focused = g;
                }
                self.focus = Some(Focus::Canvas(g));
                Outcome::Done
            }
            _ => failed(key!("error-bad-param"), &["area"]),
        }
    }

    /// F6 (`step` 1) and Shift+F6 (`step` −1): the command bar, navigator, canvas group 1, canvas
    /// group 2, then the property grid and the find pane while their panes are in the layout, cyclic. Without a focus
    /// the cycle starts from the navigator, as before the command bar.
    pub(crate) fn act_focus_cycle(&mut self, step: isize) -> Outcome {
        let groups = self.area.as_ref().map_or(0, |a| a.groups.len());
        // The command bar first (doc 16a §5.9).
        let ring: Vec<Focus> = std::iter::once(Focus::CommandBar)
            .chain(std::iter::once(Focus::Navigator(
                crate::navigators::NavKind::Pages,
            )))
            // The open navigators after Pages.
            .chain(self.navs.keys().map(|k| Focus::Navigator(*k)))
            .chain((0..groups).map(Focus::Canvas))
            .chain(self.properties_shown().then_some(Focus::Properties))
            .chain(self.find_shown().then_some(Focus::Find))
            .chain(self.history_shown().then_some(Focus::History))
            .collect();
        let here = self
            .focus
            .and_then(|f| ring.iter().position(|r| *r == f))
            .unwrap_or(1);
        let next = ring[(here as isize + step).rem_euclid(ring.len() as isize) as usize];
        if let (Focus::Canvas(g), Some(a)) = (next, &mut self.area) {
            a.focused = g;
        }
        self.focus = Some(next);
        if let Focus::Navigator(kind) = next {
            self.bring_navigator_to_front(kind);
        }
        Outcome::Done
    }

    /// A navigator given the focus is the shown one: behind another tab of its pane, its tab comes
    /// to the front, as the Messages pane does.
    pub(super) fn bring_navigator_to_front(&mut self, kind: crate::navigators::NavKind) {
        let pane = match kind {
            crate::navigators::NavKind::Pages => crate::layout::PaneKind::Pages,
            k => crate::layout::PaneKind::Navigator(k),
        };
        let layout = self.layout().showing(pane);
        if layout != *self.layout() {
            self.replace_layout(layout);
        }
    }
}
