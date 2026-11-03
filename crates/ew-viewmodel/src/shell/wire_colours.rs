// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 6 (D18, FR-UI-047): the wire-colour view's host duties. `view.wire_colours {on?}` (View ▸ Wire
//! colours) switches the view, a session's view state like View ▸ Grid, off at start; [`ShellModel::wire_colour_overlay`]
//! gives the strokes of a canvas group's shown page for the window's painter ([`crate::wire_colours`]).
//!
//! **The colours** are the effective colours of the Connections navigator's table (the wire list's Colour column):
//! the table is fetched once per (project session, revision) while the view is on, and a drawn connection takes the
//! colour of the row whose wire it belongs to (`NavTable::index`).

use std::sync::Arc;

use ew_api::output::{NavKind, NavRole, NavTable};
use serde_json::{Map, Value};

use super::{Outcome, ShellModel};
use crate::wire_colours::WireStroke;

/// The Connections table of the wire-colour view and the (session epoch, revision) it was fetched at.
pub(super) type Cached = ((u64, u64), Arc<NavTable>);

impl ShellModel {
    /// Whether the wire-colour view is on.
    pub fn wire_colours_on(&self) -> bool {
        self.wire_colours
    }

    /// `view.wire_colours {on?}`: the view on or off (a toggle without `on`).
    pub(crate) fn act_wire_colours(&mut self, p: &Map<String, Value>) -> Outcome {
        match super::jobs::bool_param(p, "on", !self.wire_colours) {
            Ok(on) => {
                self.wire_colours = on;
                Outcome::Done
            }
            Err(e) => e,
        }
    }

    /// The Connections table of the open project's revision, fetched once per (session, revision).
    fn wire_colour_table(&self) -> Option<Arc<NavTable>> {
        let project = self.project.as_ref()?;
        let key = (project.epoch, project.session.world().rev.0);
        let mut cache = self.wire_colour_cache.borrow_mut();
        if let Some((k, table)) = cache.as_ref()
            && *k == key
        {
            return Some(Arc::clone(table));
        }
        let table = Arc::new(project.session.navigator_table(NavKind::Connections));
        *cache = Some((key, Arc::clone(&table)));
        Some(table)
    }

    /// The strokes of the wire-colour view on canvas group `group`'s page: each drawn connection whose effective colour
    /// is known, in the page tables' order. Empty while the view is off.
    pub fn wire_colour_overlay(&self, group: usize) -> Vec<WireStroke> {
        if !self.wire_colours {
            return Vec::new();
        }
        let (Some(c), Some(table)) = (self.canvas_of(group), self.wire_colour_table()) else {
            return Vec::new();
        };
        let Some(col) = table.columns.iter().position(|c| c.key == "colour") else {
            return Vec::new();
        };
        let page = c.page();
        page.tables
            .connections
            .iter()
            .filter_map(|t| {
                let code = table
                    .index
                    .get(&t.id)?
                    .iter()
                    .map(|i| &table.nodes[*i as usize])
                    .find(|n| n.role == NavRole::Connection)?
                    .cells
                    .get(col)
                    .filter(|code| !code.is_empty())?;
                let polyline = t
                    .polyline
                    .iter()
                    .map(|p| page.snapper.page_to_sheet(*p))
                    .collect();
                Some(WireStroke::new(t.id, polyline, code))
            })
            .collect()
    }
}
