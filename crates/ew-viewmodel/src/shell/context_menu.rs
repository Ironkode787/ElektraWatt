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
//!   Define potential… on a symbol of one electrical connection point; then its device's fluid actions (a source's
//!   Medium: entries and New medium…, the medium another fluid device's ports carry, Add valve, Generate overview…, Plug
//!   port).
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
use crate::strings::Key;
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
            // A fluid line: its medium, one entry per medium of the project (doc 11 §7), then New medium….
            let fluid = |e: &ew_api::output::ew_dl::EntityRef| {
                tables
                    .connections
                    .iter()
                    .any(|c| c.id == e.0 && c.kind == "fluid")
            };
            if let [(e, _)] = kinds.as_slice()
                && fluid(e)
            {
                let line_kind = tables
                    .connections
                    .iter()
                    .find(|c| c.id == e.0)
                    .and_then(|c| c.line_kind.clone());
                self.line_kind_entries(line_kind.as_deref(), &mut out);
                self.medium_entries(e.0, &mut out);
                return out;
            }
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

    /// A fluid line's line kinds (FT-1733: the line-kind picker), each as Line: `<kind>`, checked for its own, through
    /// the Properties row `connection/line_kind`.
    fn line_kind_entries(&self, current: Option<&str>, out: &mut Vec<ActionItem>) {
        for &word in strings::LINE_KINDS {
            let kind = strings::lookup(&format!("choice-{word}")).unwrap_or(word);
            let label = strings::format(key!("canvas-line-kind"), &[kind]);
            let params = json!({"row": "connection/line_kind", "value": word});
            out.extend(self.aimed("props.set", params, label).map(|mut i| {
                i.checked = current == Some(word);
                i
            }));
        }
    }

    /// A fluid line's medium entries: Medium `<name>` for each medium of the project, checked for the medium its net
    /// carries (`fluid.set_medium` on the line), then New medium….
    fn medium_entries(&self, line: Uuid, out: &mut Vec<ActionItem>) {
        let Some(session) = self.project_session() else {
            return;
        };
        let carried = session.line_medium(line);
        for (medium, name) in session.media() {
            let label = strings::format(key!("canvas-line-medium"), &[&name]);
            let params = json!({ "line": line.to_string(), "medium": medium.to_string() });
            out.extend(self.aimed("fluid.set_medium", params, label).map(|mut i| {
                i.checked = carried == Some(medium);
                i
            }));
        }
        out.extend(self.menu_item("fluid.medium_new"));
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
        self.plc_entries(thing.device, out);
        // The persona round: a valve terminal's box offered no Add valve nor Generate overview….
        let coil = match rep.functions.as_slice() {
            [f] => Some(*f),
            _ => None,
        };
        self.fluid_entries(thing.device, coil, out);
    }

    /// A device's PLC entries (F19 of the rc.1 test), on its symbol on any page and on its rows in the Devices and Bus
    /// panels: a station offers Rack… and names its PLC; a bus device that belongs to no PLC offers Add to `<PLC>` for
    /// each PLC of the project and Add to a new PLC; a device whose bus ports are all passive (a switch, a plug) says in
    /// a disabled row why it takes no part.
    fn plc_entries(&self, device: Uuid, out: &mut Vec<ActionItem>) {
        use ew_api::output::PlcOffer;
        let Some(session) = self.project_session() else {
            return;
        };
        match session.plc_offer(device) {
            PlcOffer::Station(name) => {
                let label = strings::text(key!("action-plc-rack-editor")).to_owned();
                out.extend(self.aimed("plc.rack_editor", json!({ "station": device.to_string() }), label));
                out.extend(self.menu_item("plc.add_to_plc").map(|mut i| {
                    i.label = strings::format(key!("canvas-plc-of"), &[&name]);
                    i.enabled = false;
                    i.reason = Some(strings::format(key!("reason-plc-station-of"), &[&name]));
                    i
                }));
            }
            PlcOffer::BusDevice => {
                for (cfg, name) in session.plc_config_projects() {
                    let label = strings::format(key!("canvas-plc-add-to"), &[&name]);
                    let params = json!({ "device": device.to_string(), "config": cfg.to_string() });
                    out.extend(self.aimed("plc.add_to_plc", params, label));
                }
                let label = strings::text(key!("canvas-plc-add-new")).to_owned();
                out.extend(self.aimed("plc.add_to_plc", json!({ "device": device.to_string() }), label));
            }
            PlcOffer::Passive => out.extend(self.menu_item("plc.add_to_plc").map(|mut i| {
                i.label = strings::text(key!("canvas-plc-add")).to_owned();
                i.enabled = false;
                i.reason = Some(strings::text(key!("reason-plc-passive")).to_owned());
                i
            })),
            PlcOffer::None => {}
        }
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
            | (NavKind::Plc | NavKind::PlcGrid, NavRole::Station | NavRole::Module, Some(device)) => {
                self.go_to(kind, Some(node), &mut out);
                self.device_entries(&self.row_device(device, node), &mut out);
                out.extend(self.menu_item("props.focus"));
                if kind == NavKind::Devices {
                    self.plc_entries(device, &mut out);
                }
                match node.role {
                    NavRole::Station => self.station_entries(device, &mut out),
                    NavRole::Module => {
                        self.attach_entries(device, &mut out);
                        self.addressing_entries(json!({ "modules": [device.to_string()] }), &mut out);
                        let label = strings::text(key!("action-plc-module-safety")).to_owned();
                        out.extend(self.aimed(
                            "plc.module_safety",
                            json!({ "module": device.to_string() }),
                            label,
                        ));
                    }
                    _ => {}
                }
                self.fluid_entries(device, None, &mut out);
            }
            // A fluid navigator's device: its fluid actions (doc 11 §7).
            (NavKind::Fluid, NavRole::Device, Some(device)) => {
                self.go_to(kind, Some(node), &mut out);
                self.device_entries(&self.row_device(device, node), &mut out);
                out.extend(self.menu_item("props.focus"));
                self.fluid_entries(device, None, &mut out);
            }
            // A configuration project's addressing commands (FR-PLC-014, FR-PLC-015), each previewed.
            (NavKind::Plc | NavKind::PlcGrid, NavRole::ConfigProject, Some(cfg)) => {
                self.addressing_entries(json!({ "config_project": cfg.to_string() }), &mut out);
                out.extend(self.menu_item("props.focus"));
                self.config_entries(cfg, &mut out);
            }
            (NavKind::Plc | NavKind::PlcGrid, NavRole::Channel, channel) => {
                self.go_to(kind, Some(node), &mut out);
                out.extend(self.menu_item("props.focus"));
                if let Some(channel) = channel {
                    let label = strings::text(key!("action-plc-channel-settings")).to_owned();
                    out.extend(self.aimed(
                        "plc.channel_settings",
                        json!({ "channel": channel.to_string() }),
                        label,
                    ));
                }
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
            // A finding's row: its quick fix, where it has one, and Waive… or Unwaive, as the detail under the list
            // offers them (the persona round of M3: a right-click on a message did nothing).
            (NavKind::Messages, NavRole::Finding, _) => {
                let key = json!({ "key": node.key });
                let has = |badge: &str| node.badges.iter().any(|b| b == badge);
                if has("fix") {
                    let label = strings::text(key!("messages-fix-button")).to_owned();
                    out.extend(self.aimed("messages.fix", key.clone(), label));
                }
                let (action, label) = if has("waived") {
                    ("messages.unwaive", key!("messages-unwaive-button"))
                } else {
                    ("messages.waive", key!("messages-waive-button"))
                };
                out.extend(self.aimed(action, key, strings::text(label).to_owned()));
            }
            // A port's row in the Bus navigator: its device's PLC entries (F19 of the rc.1 test).
            (NavKind::Bus, NavRole::BusPort, Some(function)) => {
                let device = self.project_session().and_then(|s| {
                    let q = s.query();
                    let f = q.functions().get(function.to_string().parse().ok()?)?;
                    f.device.map(|d| d.as_uuid())
                });
                if let Some(device) = device {
                    self.plc_entries(device, &mut out);
                }
            }
            // A port not drawn yet (the Bus navigator): Place port, its box on a bus-topology page; its device's PLC
            // entries.
            (NavKind::Bus, NavRole::Device, Some(device)) => {
                self.plc_entries(device, &mut out);
                if let Some((device, port)) = super::navigators::undrawn_port(&node.key) {
                    let label = strings::format(key!("canvas-place-port"), &[port]);
                    let bus_page = self.page_rows().get(self.page_index()).map(|r| r.kind.as_str())
                        == Some("bus_topology");
                    let params = json!({ "device": device, "port": port });
                    out.extend(self.aimed("place.pick", params, label).map(|mut i| {
                        if i.enabled && !bus_page {
                            i.enabled = false;
                            i.reason = Some(strings::text(key!("reason-port-bus-page")).to_owned());
                        }
                        i
                    }));
                }
            }
            _ => {}
        }
        out
    }

    /// The PLC panel's and the PLC grid's Addresses menu (the persona round: Freeze addresses was found only through
    /// the guide): the addressing commands of each configuration project, as its row's menu offers them, each label
    /// led by the configuration project's name when the project has more than one. Empty without one.
    pub fn plc_addressing_items(&self) -> Vec<ActionItem> {
        let cfgs = self
            .project_session()
            .map(ew_api::Session::plc_config_projects)
            .unwrap_or_default();
        let mut out = Vec::new();
        for (id, name) in &cfgs {
            let mut items = Vec::new();
            self.addressing_entries(json!({ "config_project": id.to_string() }), &mut items);
            if cfgs.len() > 1 {
                for i in &mut items {
                    i.label = format!("{name}: {}", i.label);
                }
            }
            out.extend(items);
        }
        out
    }

    /// The previewed addressing commands of a configuration project or a module (`scope` their parameters): Re-address…
    /// (`plc.readdress`, the preview lists every old and new address), Make manual addresses automatic…
    /// (`plc.convert_manual_to_auto`) and, for a configuration project, Freeze addresses… (`plc.freeze`).
    fn addressing_entries(&self, scope: Value, out: &mut Vec<ActionItem>) {
        let mut commands = vec!["plc.readdress", "plc.convert_manual_to_auto"];
        if scope.get("config_project").is_some() {
            commands.push("plc.freeze");
        }
        for name in commands {
            let label =
                strings::text(Key::run_time(&format!("menu-{}", name.replace(['.', '_'], "-")))).to_owned();
            let params = json!({ "cmd": name, "params": scope });
            out.extend(self.aimed("plc.preview", params, label));
        }
    }

    /// A waiting card's own entries: Attach to `<station>` for each station that takes cards, so that a card listed
    /// under Modules without a station says on its own row where it can go (the persona round of M3).
    fn attach_entries(&self, card: Uuid, out: &mut Vec<ActionItem>) {
        let Some(session) = self.project_session() else {
            return;
        };
        if !session.plc_cards_without_cpu().iter().any(|(c, _)| *c == card) {
            return;
        }
        for (station, tag) in session.plc_stations_for_cards() {
            let label = strings::format(key!("action-plc-attach-to"), &[&tag]);
            out.extend(self.aimed(
                "plc.attach_cards",
                json!({ "station": station.to_string(), "card": card.to_string() }),
                label,
            ));
        }
    }

    /// A station row's PLC entries: Place port `<interface>` for each port of its part not drawn yet (FT-831: the Place
    /// tool armed with the bus port symbol, joined to the station), Rack and slots… (FR-PLC-028), Attach cards without
    /// a CPU (FT-1672), disabled with its reason while no card waits for one.
    fn station_entries(&self, station: Uuid, out: &mut Vec<ActionItem>) {
        if let Some(session) = self.project_session() {
            // On a bus-topology page each interface not drawn there yet is a box with its sockets (doc 10 §2.7);
            // elsewhere each port no function carries yet is its bus's port symbol.
            let folio = self.current_folio().unwrap_or_default();
            if self.page_rows().get(self.page_index()).map(|r| r.kind.as_str()) == Some("bus_topology") {
                for i in session.undrawn_interfaces(station, folio) {
                    let label = strings::format(key!("canvas-place-port"), &[&i.label]);
                    let params = json!({ "device": station.to_string(), "port": i.label });
                    out.extend(self.aimed("place.pick", params, label));
                }
            } else {
                let tag = session.device_tag(station).unwrap_or_default();
                for (port, symbol) in session.uncarried_bus_ports(station) {
                    let label = strings::format(key!("canvas-place-port"), &[&port]);
                    out.extend(self.aimed("place.pick", json!({ "symbol": symbol, "device": tag }), label));
                }
            }
        }
        let label = strings::text(key!("action-plc-rack-editor")).to_owned();
        out.extend(self.aimed(
            "plc.rack_editor",
            json!({ "station": station.to_string() }),
            label,
        ));
        let waiting = self
            .project
            .as_ref()
            .map_or(0, |p| p.session.plc_cards_without_cpu().len());
        let label = strings::text(key!("action-plc-attach-cards")).to_owned();
        out.extend(
            self.aimed(
                "plc.attach_cards",
                json!({ "station": station.to_string() }),
                label,
            )
            .map(|mut i| {
                if i.enabled && waiting == 0 {
                    i.enabled = false;
                    i.reason = Some(strings::text(key!("reason-no-cards-without-cpu")).to_owned());
                }
                i
            }),
        );
    }

    /// A configuration project row's own entries: Symbol policy… (FR-PLC-047), Unlock `<group>` for each field group a
    /// PLC import protects (`plc.unlock`, doc 10 §3.9) and Import instrument list… (FR-PLC-050).
    fn config_entries(&self, cfg: Uuid, out: &mut Vec<ActionItem>) {
        let label = strings::text(key!("action-plc-symbol-policy")).to_owned();
        out.extend(self.aimed("plc.symbol_policy", json!({ "config": cfg.to_string() }), label));
        let groups = self
            .project_session()
            .map(|s| s.plc_protected_groups(cfg))
            .unwrap_or_default();
        for group in groups {
            let label = strings::text(match group {
                "module_starts" => key!("canvas-plc-unlock-module-starts"),
                "symbols" => key!("canvas-plc-unlock-symbols"),
                _ => key!("canvas-plc-unlock-plc-data"),
            })
            .to_owned();
            let params = json!({ "cmd": "plc.unlock", "params": {
                "config_project": cfg.to_string(), "group": group } });
            out.extend(self.aimed("command.run", params, label));
        }
        let label = strings::text(key!("action-plc-instrument-list")).to_owned();
        out.extend(self.aimed("plc.instrument_list", json!({ "config": cfg.to_string() }), label));
    }

    /// A device's fluid actions, on its row and on its symbol alike (doc 11 §7, FT-1733; `Session::fluid_menu`): on a
    /// valve terminal Generate overview… (FR-FLU-033); on a valve module Add valve at each blank or empty position (the
    /// part of its other valves, else the libraries' valve; `fluid.valve_add` puts its symbol on the pointer) and Blank
    /// position at each empty one; on a valve Set used or Set spare and Blank position, on a blanking plate Add valve;
    /// Plug port for each open working
    /// port (EW-FL-001's). Each is one `valve.set_occupancy` or `fluid.port.close`, one undo step. A stand-alone
    /// valve's solenoids (rc.2, F11): Go to solenoid 14 where its coil is drawn, else Draw solenoid 14 on an electrical
    /// page… (`fluid.draw_solenoid`); on the symbol of the coil itself (`symbol_of`, the function it draws) Solenoid 14
    /// of -201Q1, which goes to the valve.
    fn fluid_entries(&self, device: Uuid, symbol_of: Option<Uuid>, out: &mut Vec<ActionItem>) {
        let Some(session) = self.project_session() else {
            return;
        };
        match symbol_of.and_then(|f| session.solenoid_of(f)) {
            Some((tag, name, valve_at)) => {
                let label = strings::format(key!("props-solenoid-of"), &[&name, &tag]);
                out.extend(
                    self.aimed("nav.goto", json!({ "text": valve_at }), label)
                        .map(|mut i| {
                            if valve_at.is_none() {
                                i.enabled = false;
                                i.reason = Some(strings::text(key!("reason-nav-not-placed")).to_owned());
                            }
                            i
                        }),
                );
            }
            None => {
                for s in session.valve_solenoids(device) {
                    out.extend(match &s.drawn {
                        Some(place) => self.aimed(
                            "nav.goto",
                            json!({ "text": place }),
                            strings::format(key!("canvas-solenoid-go"), &[&s.name, place]),
                        ),
                        None => self.aimed(
                            "fluid.draw_solenoid",
                            json!({ "function": s.function.to_string() }),
                            strings::format(key!("canvas-solenoid-draw"), &[&s.name]),
                        ),
                    });
                }
            }
        }
        self.device_medium_entries(session, device, out);
        let menu = session.fluid_menu(device);
        if menu.terminal {
            let label = strings::text(key!("action-fluid-generate-overview")).to_owned();
            out.extend(self.aimed(
                "fluid.generate_overview",
                json!({ "station": device.to_string() }),
                label,
            ));
        }
        let occupancy = |module: Uuid, position: u32, occupancy: &Value| {
            json!({ "cmd": "valve.set_occupancy", "params": {
                "module": module.to_string(), "position": position, "occupancy": occupancy } })
        };
        // Add valve: the valve child, then its symbol on the pointer (`fluid.valve_add`).
        let add = |out: &mut Vec<ActionItem>, module: Uuid, position: u32| {
            let label = strings::format(key!("canvas-valve-add"), &[&position.to_string()]);
            let params = json!({ "module": module.to_string(), "position": position });
            out.extend(self.aimed("fluid.valve_add", params, label).map(|mut i| {
                if i.enabled && menu.valve_part.is_none() {
                    i.enabled = false;
                    i.reason = Some(strings::text(key!("reason-valve-no-part")).to_owned());
                }
                i
            }));
        };
        // An empty position is blanked from the module too (the persona round added a valve to blank it).
        let blank = |out: &mut Vec<ActionItem>, module: Uuid, position: u32| {
            let label = strings::format(key!("canvas-valve-blank"), &[&position.to_string()]);
            out.extend(self.aimed(
                "command.run",
                occupancy(module, position, &json!({ "blank": {} })),
                label,
            ));
        };
        for (position, holds) in &menu.positions {
            if *holds != "valve" {
                add(out, device, *position);
            }
            if *holds == "empty" {
                blank(out, device, *position);
            }
        }
        if let Some((module, position, state)) = &menu.own {
            if state == "blank" {
                add(out, *module, *position);
            } else {
                let spare = json!({ "valve": { "state": "spare" } });
                let used = json!({ "valve": { "state": "used" } });
                let (word, change) = if state == "spare" {
                    (key!("canvas-valve-used"), used)
                } else {
                    (key!("canvas-valve-spare"), spare)
                };
                out.extend(self.aimed(
                    "command.run",
                    occupancy(*module, *position, &change),
                    strings::text(word).to_owned(),
                ));
                let label = strings::format(key!("canvas-valve-blank"), &[&position.to_string()]);
                out.extend(self.aimed(
                    "command.run",
                    occupancy(*module, *position, &json!({ "blank": {} })),
                    label,
                ));
            }
        }
        for (function, idx, designation) in &menu.open_ports {
            let label = strings::format(key!("canvas-port-plug"), &[designation]);
            let params = json!({ "cmd": "fluid.port.close", "params": {
                "function": function.to_string(), "idx": idx, "closure": { "kind": "plug" } } });
            out.extend(self.aimed("command.run", params, label));
        }
    }

    /// A fluid device's medium (rc.2, F9): a source chooses it, Medium: `<medium>` for each medium of the project (its
    /// own checked, through its Properties row `device/derived.medium`), then New medium…; any other fluid device names
    /// each medium its ports carry, or that one carries none, each opening its Properties, where the port rows say from
    /// where and what to do.
    fn device_medium_entries(&self, session: &ew_api::Session, device: Uuid, out: &mut Vec<ActionItem>) {
        let Some(media) = session.device_media(device) else {
            return;
        };
        if media.source {
            let current = media.carried.first().cloned().flatten().map(|(id, _)| id);
            for (medium, _) in session.media() {
                let words = session.medium_text(medium).unwrap_or_default();
                let label = strings::format(key!("canvas-line-medium"), &[&words]);
                let params = json!({ "row": "device/derived.medium", "value": medium.to_string() });
                out.extend(self.aimed("props.set", params, label).map(|mut i| {
                    i.checked = current == Some(medium);
                    i
                }));
            }
            out.extend(self.menu_item("fluid.medium_new"));
            return;
        }
        for carried in &media.carried {
            let label = match carried {
                Some((_, words)) => strings::format(key!("canvas-line-medium"), &[words]),
                None => strings::text(key!("canvas-medium-none")).to_owned(),
            };
            out.extend(self.aimed("props.focus", json!({}), label));
        }
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
