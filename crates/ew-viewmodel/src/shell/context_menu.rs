// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The first entries of the canvas context menu (FT-1693, doc 16a §6.13): what fits the thing under the pointer, by
//! what it is. The right-click selected the thing first, so the entries read the selection; each calls an action
//! that exists, with the parameters that aim it at the thing, and an entry that cannot run now carries its reason.
//!
//! - **One wire:** Edit route, Wire marker, Define potential…, Cable… (its cable where it is a core), Print wire
//!   numbers. **Several wires:** Make cable from the selection….
//! - **A junction:** Define potential….
//! - **An interruption point:** Supply comes in here… (Remove the supply mark on a marked point; several points
//!   selected offer it alone, for them all), Go to its partner, Direction: Source and Direction: Target, Continue on page….
//! - **A terminal:** Add terminal (its strip's next free number, of its own kind, floating at the pointer), Edit strip,
//!   Define potential….
//! - **A symbol of a device:** on a coil Add contact per free contact of its device (each floats its symbol), on a
//!   contact Go to coil; Join device…; Choose part…; Edit symbol and Copy and edit; Define potential… on a symbol of
//!   one connection point.
//!
//! [`crate::command_bar::canvas_menu`] puts them first, then the bar's selection commands and the general ones.

use ew_api::output::TableRep;
use serde_json::{Map, Value, json};

use super::ShellModel;
use crate::actions::{self, ActionItem};
use crate::key;
use crate::strings;
use crate::tools::{EntityKind, selection_kinds};

/// How far beyond a symbol's body a cross-reference of its own prints (nm): a contact's reference to its coil and an
/// interruption point's to its partner stand beside it.
const LINK_REACH: i64 = 6_000_000;

impl ShellModel {
    /// The entries that fit the selection on the page shown (the module documentation); empty for anything else.
    pub(crate) fn menu_fits(&self) -> Vec<ActionItem> {
        let Some(c) = self.canvas() else {
            return Vec::new();
        };
        let tables = &c.page().tables;
        let kinds = selection_kinds(&self.tools.selection, tables);
        let mut out = Vec::new();
        if !kinds.is_empty() && kinds.iter().all(|(_, k)| *k == EntityKind::Connection) {
            if kinds.len() == 1 {
                for id in [
                    "tool.route_edit",
                    "tool.marker",
                    "potential.draw",
                    "cable.from_wires",
                    "project.wire_numbers",
                ] {
                    out.extend(self.menu_item(id));
                }
            } else {
                out.extend(self.menu_item("cable.from_wires").map(|mut i| {
                    i.label = strings::text(key!("canvas-make-cable")).to_owned();
                    i
                }));
            }
            return out;
        }
        let [(e, kind)] = kinds.as_slice() else {
            // Several interruption points: Supply comes in here… for them all (FT-1699).
            out.extend(self.supply_in_item());
            return out;
        };
        match kind {
            EntityKind::Junction => out.extend(self.menu_item("potential.draw")),
            EntityKind::Rep => {
                if let Some(rep) = tables.reps.iter().find(|r| r.id == e.0) {
                    if rep.ip.is_some() {
                        self.ip_fits(rep, &mut out);
                    } else if rep.terminal {
                        self.terminal_fits(rep, &mut out);
                    } else if rep.device.is_some() {
                        self.device_fits(rep, &mut out);
                    }
                }
            }
            _ => {}
        }
        out
    }

    /// The item of action `id` in the present state, with no parameters.
    pub(crate) fn menu_item(&self, id: &str) -> Option<ActionItem> {
        actions::find(id).map(|m| actions::item_of(m, self))
    }

    /// The item of action `id` aimed by `params`, labelled `label`.
    fn aimed(&self, id: &str, params: Value, label: String) -> Option<ActionItem> {
        self.menu_item(id).map(|mut i| {
            i.params = params;
            i.label = label;
            i
        })
    }

    /// Supply comes in here…, Go to its partner, the two directions, Continue on page….
    fn ip_fits(&self, rep: &TableRep, out: &mut Vec<ActionItem>) {
        out.extend(self.supply_in_item());
        let external = rep.ip.as_ref().is_some_and(|ip| ip.external);
        match self.rep_link(rep).filter(|_| !external) {
            Some((params, cell)) => out.extend(self.aimed(
                "nav.follow",
                params,
                strings::format(key!("canvas-go-partner"), &[&cell]),
            )),
            None => out.extend(self.menu_item("nav.follow").map(|mut i| {
                i.label = strings::text(key!("canvas-go-partner-none")).to_owned();
                i.enabled = false;
                // An external point (the supply coming in, FT-1699) continues in another drawing set.
                let reason = if external {
                    key!("reason-ip-external")
                } else {
                    key!("reason-ip-no-partner")
                };
                i.reason = Some(strings::text(reason).to_owned());
                i
            })),
        }
        let direction = rep.ip.as_ref().map(|ip| ip.direction.as_str());
        for (word, choice) in [
            ("source", key!("choice-source")),
            ("target", key!("choice-target")),
        ] {
            let label = strings::format(key!("canvas-ip-direction"), &[strings::text(choice)]);
            let params = json!({"row": "function/kind.ip.direction", "value": word});
            out.extend(self.aimed("props.set", params, label).map(|mut i| {
                i.checked = direction == Some(word);
                i
            }));
        }
        out.extend(self.menu_item("ip.continue_on"));
    }

    /// Add terminal to its strip, Edit strip, Define potential….
    fn terminal_fits(&self, rep: &TableRep, out: &mut Vec<ActionItem>) {
        if let Some(strip) = rep.device.and_then(|d| self.strip_of_device(d)) {
            let mut params = json!({ "strip": strip.to_string() });
            if !rep.symbol_key.is_empty() {
                params["kind"] = Value::String(rep.symbol_key.clone());
            }
            let label = strings::text(key!("canvas-add-terminal")).to_owned();
            out.extend(self.aimed("tool.place_terminal", params, label));
        }
        let label = strings::text(key!("canvas-edit-strip")).to_owned();
        out.extend(self.aimed("strip.edit", json!({}), label));
        if self.potential_point().is_ok() {
            out.extend(self.menu_item("potential.draw"));
        }
    }

    /// On a coil Add contact per free contact, on a contact Go to coil; Join device…, Choose part…, Edit symbol, Copy
    /// and edit, Define potential….
    fn device_fits(&self, rep: &TableRep, out: &mut Vec<ActionItem>) {
        let session = self.project.as_ref().map(|p| &p.session);
        let role = session
            .zip(rep.functions.first())
            .and_then(|(s, f)| s.coil_or_contact(*f));
        match role {
            Some(true) => {
                let device = rep.device.unwrap_or_default();
                let free = session.map(|s| s.unplaced_contacts(device)).unwrap_or_default();
                for (function, slot) in &free {
                    let label = strings::format(key!("canvas-add-contact"), &[slot]);
                    out.extend(self.aimed("place.pick", json!({ "function": function.to_string() }), label));
                }
                if free.is_empty() {
                    out.extend(self.menu_item("place.pick").map(|mut i| {
                        let tag = session.and_then(|s| s.device_tag(device)).unwrap_or_default();
                        i.label = strings::text(key!("canvas-add-contact-none")).to_owned();
                        i.enabled = false;
                        i.reason = Some(strings::format(key!("reason-no-free-contact"), &[&tag]));
                        i
                    }));
                }
            }
            Some(false) => {
                if let Some((params, cell)) = self.rep_link(rep) {
                    let label = strings::format(key!("canvas-go-coil"), &[&cell]);
                    out.extend(self.aimed("nav.follow", params, label));
                }
            }
            None => {}
        }
        if self.join_rep().is_some() {
            out.extend(self.menu_item("place.join"));
        }
        out.extend(self.menu_item("part.choose"));
        let symbol = Map::from_iter([("symbol".to_owned(), Value::String(rep.symbol.to_string()))]);
        let refusal = self.symbol_edit_reason(&symbol);
        out.extend(self.menu_item("symbol.edit").map(|mut i| {
            i.params = Value::Object(symbol.clone());
            if let Some(why) = refusal {
                i.enabled = false;
                i.reason = Some(why);
            }
            i
        }));
        out.extend(self.aimed(
            "symbol.copy_edit",
            Value::Object(symbol.clone()),
            strings::text(key!("action-symbol-copy-edit")).to_owned(),
        ));
        if self.potential_point().is_ok() {
            out.extend(self.menu_item("potential.draw"));
        }
    }

    /// The cross-reference printed beside `rep` (its own, nearest to its body's middle within [`LINK_REACH`]) as
    /// `nav.follow`'s parameters at its middle, and the cell or folio it leads to; `None` when none prints there.
    fn rep_link(&self, rep: &TableRep) -> Option<(Value, String)> {
        let page = self.canvas()?.page();
        let a = page.snapper.page_to_sheet((rep.body[0], rep.body[1]));
        let b = page.snapper.page_to_sheet((rep.body[2], rep.body[3]));
        let (x0, x1) = (a.0.min(b.0) - LINK_REACH, a.0.max(b.0) + LINK_REACH);
        let (y0, y1) = (a.1.min(b.1) - LINK_REACH, a.1.max(b.1) + LINK_REACH);
        let mid = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
        let (at, label) = page
            .links
            .iter()
            .filter_map(|l| Some((crate::canvas::rect_centre(l.rect), l.label()?)))
            .filter(|((x, y), _)| (x0..=x1).contains(x) && (y0..=y1).contains(y))
            .min_by_key(|(p, _)| crate::tools::dist2(*p, mid))?;
        let mm = |v: i64| v as f64 / 1e6;
        Some((json!({"x_mm": mm(at.0), "y_mm": mm(at.1)}), label))
    }
}
