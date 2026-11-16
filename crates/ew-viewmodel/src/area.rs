// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canvas area: the open pages as tabs in
//! one or two tab groups side by side, toolkit-neutral. `ew-ui` rebuilds its nested egui_tiles tree from this model
//! after every structural change ([`CanvasArea::version`]), so the model is the one source of truth.
//!
//! **Invariants** while a project is open: at least one group and one tab, at most two groups, at most
//! [`MAX_TABS`] tabs over both groups, every group's `active` a valid index, `focused` a valid group. Tab IDs count
//! from 1 per opened project and are never reused in it. Two groups are two canvases over one `Session`: every tab reads the same revision and page content.
//!
//! The structural operations here are pure; the shell fetches page content and builds the tabs' canvases.

use std::collections::BTreeMap;

use crate::canvas::{CanvasModel, PageId, Viewport};
use crate::history::History;
use crate::key;
use crate::strings::Key;

/// The most tabs over both groups.
pub const MAX_TABS: usize = 32;

/// The most pages whose viewport a tab remembers; the least recently visited goes first.
pub const MAX_VIEWPORTS: usize = 64;

/// One open page.
#[derive(Clone, Debug)]
pub struct Tab {
    /// The tab's ID, unique in the opened project.
    pub id: u32,
    /// The page it shows.
    pub page: PageId,
    /// Its canvas; `None` until the shell built it, or when the page's content could not be built.
    pub canvas: Option<CanvasModel>,
    /// Its jump history (not persisted).
    pub history: History,
    /// The station whose view the tab shows instead of its page (doc 10 §6); `None` for a tab that shows its page.
    /// The tab keeps the page it was opened from: Draw on this page draws there, and a jump to a page leaves the
    /// tab for a page tab.
    pub station: Option<StationTab>,
    /// The remembered view of every page visited, with the visit counter of the last visit.
    viewports: BTreeMap<PageId, (Viewport, u64)>,
    visits: u64,
}

/// A station's view in a tab (doc 10 §6): a PLC, a remote I/O station or a valve terminal as its rack, or a station
/// not made yet.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct StationTab {
    /// The station device; `None` while the station is not made yet.
    pub station: Option<ew_api::output::Uuid>,
    /// The station not made yet: what it is, its head part, its PLC and its cards. Create makes it.
    pub draft: Option<crate::shell::automation::StationDraft>,
    /// The place chosen in the rack: a card selects its device as a click on a page does; a free slot takes the
    /// catalogue's next part.
    pub cursor: Option<StationCursor>,
    /// The catalogue's filter text.
    pub filter: String,
    /// A line under the rack after a change: the addresses that moved, or what comes next after Create.
    pub note: Option<String>,
    /// The last refusal of a change, with the place it was refused at; `None` after a change went through.
    pub error: Option<(Option<StationCursor>, String)>,
}

/// A place in a station's rack.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StationCursor {
    /// The station itself: its head (a CPU, an interface module, a valve terminal's node).
    Head,
    /// A slot, free or taken.
    Slot {
        /// The rack (0 for a positional slot).
        rack: u16,
        /// The slot.
        slot: u32,
    },
    /// A valve position of a valve terminal.
    Position {
        /// The module the position belongs to (the terminal or one of its modules).
        module: ew_api::output::Uuid,
        /// The position on that module, 1-based.
        position: u32,
    },
}

impl Tab {
    /// A tab on `page` without a canvas yet.
    pub fn new(id: u32, page: PageId) -> Tab {
        Tab {
            id,
            page,
            canvas: None,
            history: History::new(),
            station: None,
            viewports: BTreeMap::new(),
            visits: 0,
        }
    }

    /// Remembers the view of `page` (at most [`MAX_VIEWPORTS`] pages; the least recently visited is forgotten).
    pub fn remember(&mut self, page: PageId, viewport: Viewport) {
        self.visits += 1;
        self.viewports.insert(page, (viewport, self.visits));
        while self.viewports.len() > MAX_VIEWPORTS {
            let oldest = self
                .viewports
                .iter()
                .min_by_key(|(_, (_, n))| *n)
                .map(|(p, _)| *p);
            match oldest {
                Some(p) => {
                    self.viewports.remove(&p);
                }
                None => break,
            }
        }
    }

    /// The remembered view of `page`.
    pub fn recall(&self, page: PageId) -> Option<Viewport> {
        self.viewports.get(&page).map(|(v, _)| *v)
    }

    /// The number of remembered views.
    pub fn remembered(&self) -> usize {
        self.viewports.len()
    }
}

/// The tabs of one side.
#[derive(Clone, Debug)]
pub struct TabGroup {
    /// The tabs, at least one.
    pub tabs: Vec<Tab>,
    /// The active tab.
    pub active: usize,
    /// The group's pane in logical points.
    pub pane: (f64, f64),
    /// Its physical pixels per point.
    pub ppp: f64,
    /// Whether the host has sized the pane (`view.canvas_resize`): until then a resize fits a fitted page to the real
    /// pane; after it a resize (a dock opened, closed or dragged) refits a fitted, untouched view and keeps a zoomed
    /// or panned view's zoom and centre.
    pub sized: bool,
}

impl TabGroup {
    fn new(tab: Tab, pane: (f64, f64), ppp: f64) -> TabGroup {
        TabGroup {
            tabs: vec![tab],
            active: 0,
            pane,
            ppp,
            sized: false,
        }
    }

    /// The active tab.
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// The active tab, to change.
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }
}

/// The open pages of the project view.
#[derive(Clone, Debug)]
pub struct CanvasArea {
    /// One or two groups, left to right.
    pub groups: Vec<TabGroup>,
    /// The focused group.
    pub focused: usize,
    /// The ID of the next tab.
    pub next_tab: u32,
    version: u64,
}

impl CanvasArea {
    /// One group with one tab (ID 1) on `page`, in a pane of `pane` points at `ppp`.
    pub fn new(page: PageId, pane: (f64, f64), ppp: f64) -> CanvasArea {
        CanvasArea {
            groups: vec![TabGroup::new(Tab::new(1, page), pane, ppp)],
            focused: 0,
            next_tab: 2,
            version: 0,
        }
    }

    /// A number bumped by every structural change: the host rebuilds its tree when it changes.
    pub fn version(&self) -> u64 {
        self.version
    }

    fn changed(&mut self) {
        self.version += 1;
    }

    /// The number of tabs over both groups.
    pub fn tab_count(&self) -> usize {
        self.groups.iter().map(|g| g.tabs.len()).sum()
    }

    /// The focused group.
    pub fn focused_group(&self) -> &TabGroup {
        &self.groups[self.focused]
    }

    /// The focused group's active tab.
    pub fn current(&self) -> &Tab {
        self.focused_group().active_tab()
    }

    fn take_id(&mut self) -> u32 {
        let id = self.next_tab;
        self.next_tab += 1;
        id
    }

    /// `tab.new`: a tab after the active one of the focused group, on the same page with the same view, active.
    ///
    /// # Errors
    ///
    /// `reason-tabs-full` with [`MAX_TABS`] tabs.
    pub fn new_tab(&mut self) -> Result<(), Key<'static>> {
        if self.tab_count() >= MAX_TABS {
            return Err(key!("reason-tabs-full"));
        }
        let id = self.take_id();
        let g = &mut self.groups[self.focused];
        let source = g.active_tab();
        let mut tab = Tab::new(id, source.page);
        tab.canvas = source.canvas.clone();
        if let Some(c) = &mut tab.canvas {
            c.put_cursor(None);
        }
        g.tabs.insert(g.active + 1, tab);
        g.active += 1;
        self.changed();
        Ok(())
    }

    /// Opens a station's view: a tab after the active one of the focused group, on the same page, active.
    ///
    /// # Errors
    ///
    /// `reason-tabs-full` with [`MAX_TABS`] tabs.
    pub fn open_station(&mut self, station: StationTab) -> Result<(), Key<'static>> {
        if self.tab_count() >= MAX_TABS {
            return Err(key!("reason-tabs-full"));
        }
        let id = self.take_id();
        let g = &mut self.groups[self.focused];
        let mut tab = Tab::new(id, g.active_tab().page);
        tab.station = Some(station);
        g.tabs.insert(g.active + 1, tab);
        g.active += 1;
        self.changed();
        Ok(())
    }

    /// Leaves a station's view for a page: when the active tab of `group` shows a station, the nearest tab of the
    /// group that shows a page becomes active (the one before it first), else a page tab is opened before it.
    pub fn leave_station(&mut self, group: usize) {
        let Some(g) = self.groups.get(group) else { return };
        if g.active_tab().station.is_none() {
            return;
        }
        let page = |i: &usize| g.tabs[*i].station.is_none();
        let at = (0..g.active).rev().find(page).or_else(|| (g.active..g.tabs.len()).find(page));
        match at {
            Some(i) => self.groups[group].active = i,
            None => {
                let id = self.take_id();
                let g = &mut self.groups[group];
                let tab = Tab::new(id, g.active_tab().page);
                g.tabs.insert(g.active, tab);
            }
        }
        self.changed();
    }

    /// Closes the station tabs `gone` names (a station deleted, its making undone); a group's last tab shows its page
    /// again instead.
    pub fn drop_stations(&mut self, gone: impl Fn(&StationTab) -> bool) {
        let mut changed = false;
        for g in &mut self.groups {
            let mut i = 0;
            while i < g.tabs.len() {
                if !g.tabs[i].station.as_ref().is_some_and(&gone) {
                    i += 1;
                    continue;
                }
                changed = true;
                if g.tabs.len() == 1 {
                    g.tabs[i].station = None;
                    break;
                }
                g.tabs.remove(i);
                if g.active > i || g.active >= g.tabs.len() {
                    g.active = g.active.saturating_sub(1);
                }
            }
        }
        if changed {
            self.changed();
        }
    }

    /// `tab.close`: closes the focused group's active tab; the tab to its right (else its left) becomes active; a
    /// group left empty goes and the other group is focused. The project's last tab, when it shows a station, shows
    /// its page again.
    ///
    /// # Errors
    ///
    /// `reason-last-tab` for the project's last tab.
    pub fn close_tab(&mut self) -> Result<(), Key<'static>> {
        if self.tab_count() <= 1 {
            let last = self.groups[self.focused].active_tab_mut();
            if last.station.take().is_some() {
                self.changed();
                return Ok(());
            }
            return Err(key!("reason-last-tab"));
        }
        let g = &mut self.groups[self.focused];
        g.tabs.remove(g.active);
        if g.tabs.is_empty() {
            self.groups.remove(self.focused);
            self.focused = 0;
        } else {
            g.active = g.active.min(g.tabs.len() - 1);
        }
        self.changed();
        Ok(())
    }

    /// `tab.next` (`step` 1) and `tab.previous` (`step` −1) in the focused group, wrapping.
    ///
    /// # Errors
    ///
    /// `reason-one-tab` when the group has one tab.
    pub fn cycle(&mut self, step: isize) -> Result<(), Key<'static>> {
        let g = &mut self.groups[self.focused];
        let n = g.tabs.len();
        if n <= 1 {
            return Err(key!("reason-one-tab"));
        }
        g.active = (g.active as isize + step).rem_euclid(n as isize) as usize;
        self.changed();
        Ok(())
    }

    /// Whether two groups are shown side by side.
    pub fn side_by_side(&self) -> bool {
        self.groups.len() > 1
    }

    /// `view.side_by_side` on: a second group to the right with one tab on `page`, focused, in a pane of `pane`
    /// points at `ppp`.
    ///
    /// # Errors
    ///
    /// `reason-tabs-full` with [`MAX_TABS`] tabs.
    pub fn split(&mut self, page: PageId, pane: (f64, f64), ppp: f64) -> Result<(), Key<'static>> {
        if self.tab_count() >= MAX_TABS {
            return Err(key!("reason-tabs-full"));
        }
        if self.side_by_side() {
            return Ok(());
        }
        let id = self.take_id();
        self.groups.push(TabGroup::new(Tab::new(id, page), pane, ppp));
        self.focused = 1;
        self.changed();
        Ok(())
    }

    /// `view.side_by_side` off: the right group's tabs move to the end of the left group, which is focused with its
    /// active tab unchanged.
    pub fn join(&mut self) {
        if self.groups.len() < 2 {
            return;
        }
        let right = self.groups.remove(1);
        self.groups[0].tabs.extend(right.tabs);
        self.focused = 0;
        self.changed();
    }

    /// `tab.focus_other`.
    ///
    /// # Errors
    ///
    /// `reason-one-group` without side by side.
    pub fn focus_other(&mut self) -> Result<(), Key<'static>> {
        if !self.side_by_side() {
            return Err(key!("reason-one-group"));
        }
        self.focused = 1 - self.focused;
        self.changed();
        Ok(())
    }

    /// `tab.activate`: activates tab `index` of `group` and focuses the group.
    ///
    /// # Errors
    ///
    /// `error-bad-param` out of range.
    pub fn activate(&mut self, group: usize, index: usize) -> Result<(), Key<'static>> {
        let g = self.groups.get_mut(group).ok_or(key!("error-bad-param"))?;
        if index >= g.tabs.len() {
            return Err(key!("error-bad-param"));
        }
        g.active = index;
        self.focused = group;
        self.changed();
        Ok(())
    }

    /// `tab.move`: moves tab `index` of `group` to position `to_index` (clamped) of `to_group`, active and focused
    /// there; a group left empty goes.
    ///
    /// # Errors
    ///
    /// `error-bad-param` out of range.
    pub fn move_tab(
        &mut self,
        group: usize,
        index: usize,
        to_group: usize,
        to_index: usize,
    ) -> Result<(), Key<'static>> {
        if group >= self.groups.len()
            || to_group >= self.groups.len()
            || index >= self.groups[group].tabs.len()
        {
            return Err(key!("error-bad-param"));
        }
        let src = &mut self.groups[group];
        let tab = src.tabs.remove(index);
        if src.active > index || src.active >= src.tabs.len() {
            src.active = src.active.saturating_sub(1);
        }
        let mut to_group = to_group;
        if self.groups[group].tabs.is_empty() {
            self.groups.remove(group);
            if to_group > group {
                to_group -= 1;
            }
        }
        let dst = &mut self.groups[to_group];
        let at = to_index.min(dst.tabs.len());
        dst.tabs.insert(at, tab);
        dst.active = at;
        self.focused = to_group;
        self.changed();
        Ok(())
    }

    /// `tab.open`: a tab on `page` at the end of `group`, active and focused.
    ///
    /// # Errors
    ///
    /// `reason-tabs-full` with [`MAX_TABS`] tabs; `error-bad-param` for a group out of range.
    pub fn open(&mut self, page: PageId, group: usize) -> Result<(), Key<'static>> {
        if group >= self.groups.len() {
            return Err(key!("error-bad-param"));
        }
        if self.tab_count() >= MAX_TABS {
            return Err(key!("reason-tabs-full"));
        }
        let id = self.take_id();
        let g = &mut self.groups[group];
        g.tabs.push(Tab::new(id, page));
        g.active = g.tabs.len() - 1;
        self.focused = group;
        self.changed();
        Ok(())
    }

    /// Every tab with its group and index, in group order.
    pub fn tabs(&self) -> impl Iterator<Item = (usize, usize, &Tab)> {
        self.groups
            .iter()
            .enumerate()
            .flat_map(|(g, group)| group.tabs.iter().enumerate().map(move |(i, t)| (g, i, t)))
    }

    /// Checks the invariants (tests and debug assertions).
    pub fn check(&self) -> Result<(), String> {
        if self.groups.is_empty() || self.groups.len() > 2 {
            return Err(format!("{} groups", self.groups.len()));
        }
        if self.focused >= self.groups.len() {
            return Err("focused out of range".into());
        }
        if self.tab_count() > MAX_TABS {
            return Err(format!("{} tabs", self.tab_count()));
        }
        for g in &self.groups {
            if g.tabs.is_empty() || g.active >= g.tabs.len() {
                return Err("a group without tabs or a bad active tab".into());
            }
        }
        let mut ids: Vec<u32> = self.tabs().map(|(_, _, t)| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != self.tab_count() || ids.iter().any(|&id| id >= self.next_tab) {
            return Err("tab IDs repeat or run ahead".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_api::output::ew_dl::Destination;

    fn page(n: u128) -> PageId {
        let text = format!("00000000-0000-0000-0000-{n:012x}");
        PageId(Destination::Page(std::str::FromStr::from_str(&text).unwrap()))
    }

    fn area() -> CanvasArea {
        CanvasArea::new(page(1), (1200.0, 800.0), 1.0)
    }

    #[test]
    fn every_operation_keeps_the_invariants() {
        let mut a = area();
        a.check().unwrap();
        assert_eq!(a.close_tab(), Err(key!("reason-last-tab")));
        assert_eq!(a.cycle(1), Err(key!("reason-one-tab")));
        assert_eq!(a.focus_other(), Err(key!("reason-one-group")));
        a.new_tab().unwrap();
        assert_eq!((a.groups[0].active, a.current().id), (1, 2));
        a.cycle(1).unwrap();
        assert_eq!(a.groups[0].active, 0, "wraps");
        a.cycle(-1).unwrap();
        assert_eq!(a.groups[0].active, 1);
        a.split(page(2), (600.0, 800.0), 1.0).unwrap();
        assert_eq!((a.focused, a.current().id, a.current().page), (1, 3, page(2)));
        a.focus_other().unwrap();
        assert_eq!(a.focused, 0);
        a.open(page(5), 1).unwrap();
        assert_eq!((a.focused, a.groups[1].active, a.current().id), (1, 1, 4));
        assert_eq!(a.activate(1, 9), Err(key!("error-bad-param")));
        a.activate(0, 0).unwrap();
        // Move the right group's tabs one by one to the left: the right group goes when empty.
        a.move_tab(1, 0, 0, 0).unwrap();
        a.move_tab(1, 0, 0, 99).unwrap();
        assert_eq!(a.groups.len(), 1);
        assert_eq!(a.tab_count(), 4);
        a.check().unwrap();
        a.split(page(3), (600.0, 800.0), 1.0).unwrap();
        a.join();
        assert_eq!((a.groups.len(), a.focused), (1, 0));
        // Closing every tab but the last; IDs are never reused.
        while a.close_tab().is_ok() {
            a.check().unwrap();
        }
        assert_eq!(a.tab_count(), 1);
        a.new_tab().unwrap();
        assert_eq!(a.current().id, 6);
        a.check().unwrap();
    }

    #[test]
    fn closing_the_last_tab_of_a_group_leaves_side_by_side_and_the_cap_holds() {
        let mut a = area();
        a.split(page(2), (600.0, 800.0), 1.0).unwrap();
        a.close_tab().unwrap();
        assert_eq!((a.groups.len(), a.focused, a.current().page), (1, 0, page(1)));
        while a.tab_count() < MAX_TABS {
            a.new_tab().unwrap();
        }
        assert_eq!(a.new_tab(), Err(key!("reason-tabs-full")));
        assert_eq!(a.split(page(2), (1.0, 1.0), 1.0), Err(key!("reason-tabs-full")));
        assert_eq!(a.open(page(2), 0), Err(key!("reason-tabs-full")));
        a.check().unwrap();
    }

    #[test]
    fn a_tab_remembers_sixty_four_views() {
        let mut t = Tab::new(1, page(1));
        let v = |x| Viewport {
            centre: (x, 0),
            zoom: 1.0,
            fitted: false,
            first: false,
        };
        for n in 0..70u128 {
            t.remember(page(n), v(n as i64));
        }
        assert_eq!(t.remembered(), MAX_VIEWPORTS);
        assert_eq!(t.recall(page(5)), None, "the least recently visited went");
        assert_eq!(t.recall(page(69)), Some(v(69)));
        // A revisit refreshes the counter.
        t.remember(page(6), v(6));
        t.remember(page(100), v(100));
        assert_eq!(t.recall(page(6)), Some(v(6)));
        assert_eq!(t.recall(page(7)), None);
    }
}
