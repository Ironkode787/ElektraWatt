// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The typed readers of the selection (architecture §9.1): what an enable check and its handler read of what is
//! selected, each reader in the one form its callers read, so that the gate and the handler cannot disagree.

use std::collections::BTreeSet;

use ew_api::output::{PageTables, TableRep, Uuid};
use ew_dl::EntityRef;

use super::ShellModel;
use crate::tools::{EntityKind, selection_kinds};

/// The interruption points of a page that a selection holds: their table rows, in selection order.
pub(super) fn selected_ips<'t>(model: &ShellModel, tables: &'t PageTables) -> Vec<&'t TableRep> {
    selection_kinds(model.selection(), tables)
        .into_iter()
        .filter_map(|(e, _)| tables.reps.iter().find(|r| r.id == e.0 && r.ip.is_some()))
        .collect()
}

impl ShellModel {
    /// How many interruption points of the focused page are selected (`ip.continue_on` wants one, `ip.arrange_selected`
    /// two or more).
    pub fn selected_ip_count(&self) -> usize {
        self.canvas()
            .map_or(0, |c| selected_ips(self, &c.page().tables).len())
    }

    /// Exactly one black box of the focused page is selected (`box.grip`, `box.add_cp`).
    pub fn one_black_box_selected(&self) -> bool {
        self.selected_box().is_some()
    }

    /// The one selected black box of the page shown.
    pub(super) fn selected_box(&self) -> Option<TableRep> {
        let t = &self.canvas()?.page().tables;
        match self.selection().items.as_slice() {
            [one] => t
                .reps
                .iter()
                .find(|r| r.id == one.0 && r.black_box.is_some())
                .cloned(),
            _ => None,
        }
    }

    /// Exactly one junction is selected on the focused page (`junction.open`'s condition).
    pub fn one_junction_selected(&self) -> bool {
        self.canvas().is_some_and(|c| {
            let kinds = selection_kinds(&self.tools.selection, &c.page().tables);
            matches!(kinds.as_slice(), [(_, EntityKind::Junction)])
        })
    }

    /// Exactly one connection is selected on the focused page (`tool.route_edit`'s condition).
    pub fn one_connection_selected(&self) -> bool {
        self.canvas().is_some_and(|c| {
            let kinds = selection_kinds(&self.tools.selection, &c.page().tables);
            matches!(kinds.as_slice(), [(_, EntityKind::Connection)])
        })
    }

    /// The terminals of the window selection (D9): (terminal function, terminal device) for each item — a
    /// representation with one function, a function, or a device (its function) — each terminal once, in selection
    /// order; `None` when an item is none of these or not a terminal.
    pub(super) fn selected_terminals(&self) -> Option<Vec<(Uuid, Uuid)>> {
        let project = self.project.as_ref()?;
        let q = project.session.query();
        let mut out: Vec<(Uuid, Uuid)> = Vec::new();
        for item in &self.tools.selection.items {
            let text = item.0.to_string();
            let function = if let Some(rep) = text.parse().ok().and_then(|id| q.reps().get(id)) {
                let [function] = rep.functions.as_slice() else {
                    return None;
                };
                *function
            } else if let Some(id) = text.parse().ok().filter(|id| q.functions().get(*id).is_some()) {
                id
            } else {
                // A device: its function (a terminal has one; the lowest ID when it has several).
                let device = text.parse().ok().filter(|id| q.devices().get(*id).is_some())?;
                q.functions()
                    .iter()
                    .filter(|(_, f)| f.device == Some(device))
                    .map(|(id, _)| id)
                    .min()?
            };
            let f = q.functions().get(function)?;
            let device = f.device?;
            q.devices().get(device)?.aspects.terminal.as_ref()?;
            let pair = (function.as_uuid(), device.as_uuid());
            if !out.iter().any(|(x, _)| *x == pair.0) {
                out.push(pair);
            }
        }
        Some(out)
    }

    /// The devices of the window selection (D9): a device itself, a function's device, a representation's functions'
    /// devices; each once, in selection order.
    pub(super) fn selected_record_devices(&self) -> Vec<Uuid> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let mut out: Vec<Uuid> = Vec::new();
        let push = |u: Uuid, out: &mut Vec<Uuid>| {
            if !out.contains(&u) {
                out.push(u);
            }
        };
        for item in &self.tools.selection.items {
            let text = item.0.to_string();
            if let Ok(id) = text.parse()
                && q.devices().get(id).is_some()
            {
                push(item.0, &mut out);
                continue;
            }
            if let Ok(id) = text.parse()
                && let Some(f) = q.functions().get(id)
            {
                if let Some(d) = f.device {
                    push(d.as_uuid(), &mut out);
                }
                continue;
            }
            if let Ok(id) = text.parse()
                && let Some(rep) = q.reps().get(id)
            {
                for f in &rep.functions {
                    if let Some(d) = q.functions().get(*f).and_then(|f| f.device) {
                        push(d.as_uuid(), &mut out);
                    }
                }
            }
        }
        out
    }

    /// The devices of the selection: a selected device, the device of a selected function or of a selected
    /// representation's function; sorted, once each.
    pub(super) fn selected_devices(&self) -> Vec<String> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let mut out: BTreeSet<Uuid> = BTreeSet::new();
        for e in &self.tools.selection.items {
            // The typed row IDs come from the canonical text, fixed by inference (the shell names no schema type).
            let text = e.0.to_string();
            let device = text
                .parse()
                .ok()
                .filter(|d| q.devices().get(*d).is_some())
                .or_else(|| {
                    text.parse()
                        .ok()
                        .and_then(|f| q.functions().get(f))
                        .and_then(|f| f.device)
                })
                .or_else(|| {
                    text.parse()
                        .ok()
                        .and_then(|r| q.reps().get(r))
                        .and_then(|r| r.functions.first().copied())
                        .and_then(|f| q.functions().get(f))
                        .and_then(|f| f.device)
                });
            if let Some(d) = device {
                out.insert(d.as_uuid());
            }
        }
        out.into_iter().map(|u| u.to_string()).collect()
    }

    /// The selected terminals of `tables` with their points, when they stand in one row (one `y`); else none.
    pub(super) fn selected_terminal_row(&self, tables: &PageTables) -> Vec<(EntityRef, (i64, i64))> {
        let row: Vec<(EntityRef, (i64, i64))> = tables
            .reps
            .iter()
            .filter(|r| r.terminal && self.tools.selection.contains(EntityRef(r.id)))
            .map(|r| (EntityRef(r.id), r.pos))
            .collect();
        if row.iter().any(|(_, at)| at.1 != row[0].1.1) {
            return Vec::new();
        }
        row
    }

    /// Whether two or more terminals of one row are selected on the page shown (Spacing…'s condition).
    pub fn terminal_row_selected(&self) -> bool {
        self.canvas()
            .is_some_and(|c| self.selected_terminal_row(&c.page().tables).len() >= 2)
    }
}
