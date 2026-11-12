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
//! - **A terminal:** Add terminal (its strip's next free number, of its own kind and part, floating at the pointer),
//!   Choose part…, Edit strip, Define potential…. **Several terminals:** Choose part… for them all.
//! - **A symbol of a device:** on a coil Add contact per free contact of its device (each floats its symbol), on a
//!   contact Go to coil; Join device… where another device can take it; Choose part…; Edit symbol and Copy and edit;
//!   Define potential… on a symbol of one connection point.
//!
//! [`crate::command_bar::canvas_menu`] puts them first, then the bar's selection commands and the general ones. A
//! navigator's row offers what the same thing offers on the page from the same functions
//! ([`ShellModel::row_fits`]).

use ew_api::output::{Drawable, NavKind, NavNode, NavRole, TableRep, Uuid};
use serde_json::{Map, Value, json};

use super::ShellModel;
use crate::actions::{self, ActionItem};
use crate::key;
use crate::strings;
use crate::tools::{EntityKind, selection_kinds};

/// How far beyond a symbol's body a cross-reference of its own prints (nm): a contact's reference to its coil and an
/// interruption point's to its partner stand beside it.
const LINK_REACH: i64 = 6_000_000;

/// A device as the menu meets it: on the page, the symbol under the pointer; on a navigator's row, the device with its
/// first placement.
struct DeviceThing<'a> {
    device: Uuid,
    /// The function the symbol draws: a coil offers its device's free contacts, a contact its coil.
    function: Option<Uuid>,
    /// The symbol copy Edit symbol opens; none for a device not placed.
    symbol: Option<Uuid>,
    /// The symbol on the page shown, in the page's menu: Go to coil, Join device… and Define potential… read it.
    here: Option<&'a TableRep>,
}

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
        // Several terminals: Choose part… for them all.
        let terminal =
            |e: &ew_api::output::ew_dl::EntityRef| tables.reps.iter().any(|r| r.id == e.0 && r.terminal);
        if kinds.len() > 1 && kinds.iter().all(|(e, k)| *k == EntityKind::Rep && terminal(e)) {
            out.extend(self.menu_item("part.choose"));
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

    /// The item of action `id` aimed by `params`, labelled `label`, enabled as the action is with them.
    fn aimed(&self, id: &str, params: Value, label: String) -> Option<ActionItem> {
        let meta = actions::find(id)?;
        let mut i = actions::item_of(meta, self);
        if let Some(p) = params.as_object().filter(|p| !p.is_empty()) {
            let state = actions::enabled_with(meta, self, p);
            i.enabled = state.is_ok();
            i.reason = state.err().map(|k| strings::text(k).to_owned());
        }
        i.params = params;
        i.label = label;
        Some(i)
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

    /// Add terminal to its strip (of its kind and its part), Choose part…, Edit strip, Define potential….
    fn terminal_fits(&self, rep: &TableRep, out: &mut Vec<ActionItem>) {
        if let Some(strip) = rep.device.and_then(|d| self.strip_of_device(d)) {
            let mut params = json!({ "strip": strip.to_string() });
            if !rep.symbol_key.is_empty() {
                params["kind"] = Value::String(rep.symbol_key.clone());
            }
            // Its part; without one, Terminal…'s proposal.
            let part = self.project.as_ref().zip(rep.device).and_then(|(p, d)| {
                let q = p.session.query();
                let device = q.devices().get(d.to_string().parse().ok()?)?;
                device.parts.first().map(|u| u.part.copy.as_uuid())
            });
            if let Some(part) = part {
                params["part"] = Value::String(part.to_string());
            }
            let label = strings::text(key!("canvas-add-terminal")).to_owned();
            out.extend(self.aimed("tool.place_terminal", params, label));
        }
        out.extend(self.menu_item("part.choose"));
        let label = strings::text(key!("canvas-edit-strip")).to_owned();
        out.extend(self.aimed("strip.edit", json!({}), label));
        if self.potential_point().is_ok() {
            out.extend(self.menu_item("potential.draw"));
        }
    }

    /// The symbol of a device on the page: [`Self::device_entries`] of its device, function and symbol.
    fn device_fits(&self, rep: &TableRep, out: &mut Vec<ActionItem>) {
        let thing = DeviceThing {
            device: rep.device.unwrap_or_default(),
            function: rep.functions.first().copied(),
            symbol: Some(rep.symbol),
            here: Some(rep),
        };
        self.device_entries(&thing, out);
    }

    /// What a device offers, on the page and on a navigator's row alike (FT-1693): on a coil Add contact per free
    /// contact; on the page's contact Go to coil; on the page's symbol Join device… where another device can take it;
    /// Choose part…; Edit symbol and Copy and edit of its symbol; on the page's symbol of one connection point Define
    /// potential….
    fn device_entries(&self, thing: &DeviceThing<'_>, out: &mut Vec<ActionItem>) {
        let session = self.project.as_ref().map(|p| &p.session);
        let role = session
            .zip(thing.function)
            .and_then(|(s, f)| s.coil_or_contact(f));
        match role {
            Some(true) => {
                let device = thing.device;
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
                if let Some((params, cell)) = thing.here.and_then(|rep| self.rep_link(rep)) {
                    let label = strings::format(key!("canvas-go-coil"), &[&cell]);
                    out.extend(self.aimed("nav.follow", params, label));
                }
            }
            None => {}
        }
        let joinable = thing.here.is_some()
            && self
                .join_rep()
                .is_some_and(|(rep, own, fndef)| !self.rep_join_rows(rep, own, fndef).is_empty());
        if joinable {
            out.extend(self.menu_item("place.join"));
        }
        out.extend(self.menu_item("part.choose"));
        if let Some(symbol) = thing.symbol {
            let symbol = Map::from_iter([("symbol".to_owned(), Value::String(symbol.to_string()))]);
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
                Value::Object(symbol),
                strings::text(key!("action-symbol-copy-edit")).to_owned(),
            ));
        }
        if thing.here.is_some() && self.potential_point().is_ok() {
            out.extend(self.menu_item("potential.draw"));
        }
    }

    /// A navigator row's entries (FT-1693): what the same thing offers on the page, from the same functions; empty for
    /// a row that offers nothing. The right-click selected the row first (a page row: opened its page), so Choose
    /// part…, Properties of selection and the page's entries act on it.
    ///
    /// - **A device** (a Devices row, a PLC station or module): Go to its first placement, [`Self::device_entries`] of
    ///   its first placement's function and symbol, Properties of selection.
    /// - **A PLC channel:** Go to its place, Properties of selection.
    /// - **A cable:** Cable… on its wires, Go to its first wire.
    /// - **A page** (the Pages table's row; the tree's in `command_bar::pages_tree_menu`): [`Self::page_row_fits`].
    pub(crate) fn row_fits(&self, kind: NavKind, node: &NavNode) -> Vec<ActionItem> {
        let mut out = Vec::new();
        match (kind, node.role, node.records.first().copied()) {
            (NavKind::Devices, NavRole::Device, Some(device))
            | (NavKind::Plc, NavRole::Station | NavRole::Module, Some(device)) => {
                self.go_to(kind, Some(node), &mut out);
                self.device_entries(&self.row_device(device, node), &mut out);
                out.extend(self.menu_item("props.focus"));
            }
            (NavKind::Plc, NavRole::Channel, _) => {
                self.go_to(kind, Some(node), &mut out);
                out.extend(self.menu_item("props.focus"));
            }
            (NavKind::Cables, NavRole::Cable, Some(cable)) => {
                let wired = self
                    .project
                    .as_ref()
                    .is_some_and(|p| !p.session.cable_wires(cable).is_empty());
                let label = strings::text(key!("action-cable-from-wires")).to_owned();
                let params = json!({ "cable": cable.to_string() });
                out.extend(self.aimed("cable.from_wires", params, label).map(|mut i| {
                    if i.enabled && !wired {
                        i.enabled = false;
                        i.reason = Some(strings::text(key!("reason-cable-no-wire")).to_owned());
                    }
                    i
                }));
                // Its first wire: the first core row under it that draws one.
                let table = self.nav(kind).and_then(|n| n.table());
                let wire = table.and_then(|t| {
                    let at = t.nodes.iter().position(|n| n.key == node.key)?;
                    t.nodes
                        .iter()
                        .find(|n| n.parent == u32::try_from(at).ok() && n.open.is_some())
                });
                self.go_to(kind, wire, &mut out);
            }
            (NavKind::Pages, NavRole::Page, _) => self.page_row_fits(&node.key, &mut out),
            _ => {}
        }
        out
    }

    /// A page row's entries (FT-1693), as the Page tab offers them: Open page in a new tab, Title block fields…, Page
    /// properties, New page…, Delete page.
    pub(crate) fn page_row_fits(&self, key: &str, out: &mut Vec<ActionItem>) {
        let open = json!({ "kind": NavKind::Pages.word(), "key": key, "new_tab": true });
        out.extend(self.aimed(
            "navigator.open",
            open,
            strings::text(key!("action-tab-open")).to_owned(),
        ));
        out.extend(self.menu_item("page.title_block"));
        out.extend(self.aimed(
            "props.focus",
            json!({}),
            strings::text(key!("canvas-page-properties")).to_owned(),
        ));
        out.extend(self.menu_item("page.new"));
        out.extend(self.aimed(
            "navigator.delete",
            json!({ "kind": NavKind::Pages.word() }),
            strings::text(key!("command-page-delete")).to_owned(),
        ));
    }

    /// Go to `<place>`: `navigator.open` on the row `target` of `kind`, labelled with where its first drawable sits (or
    /// the page it opens); disabled with the reason where the row draws nothing.
    fn go_to(&self, kind: NavKind, target: Option<&NavNode>, out: &mut Vec<ActionItem>) {
        let place = target.and_then(|n| {
            let open = n.open.as_ref()?;
            Some(n.drawables.first().map_or_else(
                || {
                    self.page_rows()
                        .iter()
                        .find(|r| r.id == open.page)
                        .map(|r| r.folio.clone())
                        .unwrap_or_default()
                },
                Drawable::place,
            ))
        });
        let item = match (target, place) {
            (Some(n), Some(place)) => self.aimed(
                "navigator.open",
                json!({ "kind": kind.word(), "key": n.key }),
                strings::format(key!("canvas-go-to"), &[&place]),
            ),
            _ => self.menu_item("navigator.open").map(|mut i| {
                i.label = strings::text(key!("canvas-go-to-none")).to_owned();
                i.enabled = false;
                i.reason = Some(strings::text(key!("reason-nav-not-placed")).to_owned());
                i
            }),
        };
        out.extend(item);
    }

    /// The device of a navigator row: its first placement's function and symbol (the main function's first), else its
    /// main function.
    fn row_device<'a>(&self, device: Uuid, node: &NavNode) -> DeviceThing<'a> {
        let session = self.project.as_ref().map(|p| &p.session);
        let first = session.zip(node.drawables.first()).and_then(|(s, d)| {
            let q = s.query();
            let rep = q.reps().get(d.record.to_string().parse().ok()?)?;
            Some((rep.functions.first().map(|f| f.as_uuid()), rep.symbol.as_uuid()))
        });
        let (function, symbol) = match first {
            Some((function, symbol)) => (function, Some(symbol)),
            None => (
                session.and_then(|s| s.main_function(device)).map(|(f, _)| f),
                None,
            ),
        };
        DeviceThing {
            device,
            function,
            symbol,
            here: None,
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
