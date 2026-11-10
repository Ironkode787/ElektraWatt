// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 6 (D18, FR-UI-047): the wire-colour view's host duties. `view.wire_colours {on?}` (View ▸ Wire
//! colours) switches the view, a session's view state like View ▸ Grid, off at start; [`ShellModel::wire_colour_overlay`]
//! gives the strokes of a canvas group's shown page for the window's painter ([`crate::wire_colours`]).
//!
//! **The colours** are the effective colours `Session::wire_colours` gives for the page's connections, as lists of
//! codes, with the colour profile's company codes (their declared base and shade): read once per (project session,
//! revision, page) while the view is on.

use std::sync::Arc;

use ew_api::output::{PageTables, Uuid, WireColours};
use serde_json::{Map, Value};

use super::{Outcome, ShellModel};
use crate::wire_colours::WireStroke;

/// The wire-colour view's facts and the (session epoch, revision, page) they were read at.
pub(super) type Cached = ((u64, u64, Uuid), Arc<WireColours>);

/// The wire-colour view's switch and its fetched Connections table.
#[derive(Default)]
pub(super) struct WireColourState {
    /// Whether the wire-colour view is on (View ▸ Wire colours, D18): view state of the session, off at start.
    pub(super) on: bool,
    /// The wire-colour view's Connections table and the (session epoch, revision) it was fetched at.
    pub(super) cache: std::cell::RefCell<Option<Cached>>,
}

impl ShellModel {
    /// Whether the wire-colour view is on.
    pub fn wire_colours_on(&self) -> bool {
        self.wire_colours.on
    }

    /// `view.wire_colours {on?}`: the view on or off (a toggle without `on`).
    pub(crate) fn act_wire_colours(&mut self, p: &Map<String, Value>) -> Outcome {
        match super::jobs::bool_param(p, "on", !self.wire_colours.on) {
            Ok(on) => {
                self.wire_colours.on = on;
                Outcome::Done
            }
            Err(e) => e,
        }
    }

    /// The effective colours of the connections drawn on `tables`' page and the profile's company codes, read once per
    /// (session, revision, page).
    fn wire_colour_facts(&self, tables: &PageTables) -> Option<Arc<WireColours>> {
        let project = self.project.as_ref()?;
        let key = (project.epoch, project.session.world().rev.0, tables.page);
        let mut cache = self.wire_colours.cache.borrow_mut();
        if let Some((k, facts)) = cache.as_ref()
            && *k == key
        {
            return Some(Arc::clone(facts));
        }
        let ids: Vec<Uuid> = tables.connections.iter().map(|c| c.id).collect();
        let facts = Arc::new(project.session.wire_colours(&ids));
        *cache = Some((key, Arc::clone(&facts)));
        Some(facts)
    }

    /// The strokes of the wire-colour view on canvas group `group`'s page: each drawn connection whose effective colour
    /// is known, in the page tables' order. Empty while the view is off.
    pub fn wire_colour_overlay(&self, group: usize) -> Vec<WireStroke> {
        if !self.wire_colours.on {
            return Vec::new();
        }
        let Some(c) = self.canvas_of(group) else {
            return Vec::new();
        };
        let page = c.page();
        let Some(facts) = self.wire_colour_facts(&page.tables) else {
            return Vec::new();
        };
        let mut strokes: Vec<WireStroke> = page
            .tables
            .connections
            .iter()
            .filter_map(|t| {
                let codes = facts.connections.get(&t.id)?;
                let polyline = t
                    .polyline
                    .iter()
                    .map(|p| page.snapper.page_to_sheet(*p))
                    .collect();
                Some(WireStroke::new(t.id, polyline, page.sheet, codes, &facts.company))
            })
            .collect();
        // W49.2 F9: wires drawn on one path stand side by side.
        crate::wire_colours::assign_lanes(&mut strokes);
        strokes
    }
}
