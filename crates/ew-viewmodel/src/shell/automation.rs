// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's entries of the PLC, bus and fluid commands (WP-S2.13): each handler builds one declared command
//! from the row or selection it is aimed at and commits it as one undo step, saying what it did in words.
//!
//! - **Attach cards without a station** (FT-1672): a CPU row of the PLC navigator attaches the cards that wait for a CPU
//!   (`Session::plc_cards_without_cpu`) with one `assembly.attach`, after the station's last child position.
//! - **The rack and slot editor** (FR-PLC-028): a station row's Rack and slots… opens `Dialog::Rack`, which shows
//!   `Session::rack_view` each frame. Add module puts the chosen module part into a free slot with `plc.add_module`
//!   (a library part embedded in the same step); a module's slot field moves it with `fields.set plc_module.slot`.
//!   EW-PLC-011's prevent half refuses a slot the part does not allow; the refusal stays under the slots.
//! - **PLC exchange** (doc 10 §4, §6): Export… is a form of format and version, stations and document date that writes
//!   the file whole or not at all (`Session::plc_export`) and records it (`plc.record_exchange`); Import… reads an AML
//!   file or a TIA tag workbook into `Dialog::PlcImport`, the preview by tab (changed, conflicts, removed, parking)
//!   with the choices of its conflicts and parked objects, and Apply commits `plc.apply_exchange` under them.

use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use super::{Dialog, Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::strings;
use crate::strings::Key;

impl ShellModel {
    /// A record named by parameter `name`: its UUID, or a reference (`tag:-201U1`, `id:<uuid>`).
    pub(super) fn record_param(&self, p: &Map<String, Value>, name: &str) -> Result<Uuid, Outcome> {
        let text = match string_param(p, name)? {
            Some(t) => t.trim(),
            None => return Err(failed(key!("error-missing-param"), &[name])),
        };
        if let Ok(id) = text.parse::<Uuid>() {
            return Ok(id);
        }
        let project = self.project.as_ref().ok_or(Outcome::Refused {
            reason_key: key!("reason-no-project"),
        })?;
        project
            .session
            .resolve_reference(text)
            .map_err(|_| failed(key!("error-bad-param"), &[name]))
    }

    /// `plc.attach_cards {station, card?}` (FT-1672): the cards that wait for a station (with `card`, that one of
    /// them) become children of `station`, after its last child position, with one `assembly.attach`; a toast names
    /// them.
    pub(crate) fn act_plc_attach_cards(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.record_param(p, "station"));
        let only = match p.get("card") {
            Some(_) => Some(or_return!(self.record_param(p, "card"))),
            None => None,
        };
        let Some(project) = self.project.as_ref() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        // A card dropped on a station's row (the persona run m3-p4): one that cannot go there says why.
        let station_tag = project.session.device_tag(station).unwrap_or_default();
        if !project
            .session
            .plc_stations_for_cards()
            .iter()
            .any(|(s, _)| *s == station)
        {
            return failed(key!("error-station-takes-no-cards"), &[&station_tag]);
        }
        let mut cards = project.session.plc_cards_without_cpu();
        cards.retain(|(card, _)| only.is_none_or(|o| *card == o));
        // A card of another station dropped here moves to this one (`assembly.attach` moves a child); one of this
        // station already, or a device that is no card, says so.
        if let (true, Some(card)) = (cards.is_empty(), only) {
            let tag = project.session.device_tag(card).unwrap_or_default();
            let q = project.session.query();
            let device = q
                .devices()
                .iter()
                .find(|(id, _)| id.as_uuid() == card)
                .map(|(_, d)| d.clone());
            match device {
                Some(d) if d.parent.is_some_and(|p| p.as_uuid() == station) => {
                    return failed(key!("error-card-on-station"), &[&tag, &station_tag]);
                }
                // Only a module goes into a rack (F17 of the rc.1 test: a bus node whose part has no PLC data was
                // attached to a CPU and got eight channels no part gives); a bus device joins a PLC as a station.
                Some(d)
                    if d.aspects.plc_module.as_ref().is_some_and(|m| !m.integrated)
                        && !project.session.not_a_module(card) =>
                {
                    cards.push((card, tag));
                }
                _ if project.session.not_a_module(card) || project.session.bus_device(card) => {
                    return failed(key!("error-not-a-card-bus"), &[&tag]);
                }
                _ => return failed(key!("error-not-a-card"), &[&tag]),
            }
        }
        if cards.is_empty() {
            return Outcome::refused(key!("reason-no-cards-without-cpu"));
        }
        let q = project.session.query();
        let Some(id) = q
            .devices()
            .iter()
            .map(|(id, _)| id)
            .find(|id| id.as_uuid() == station)
        else {
            return failed(key!("error-bad-param"), &["station"]);
        };
        let last = q
            .idx()
            .device_children(id)
            .filter_map(|c| q.devices().get(c).and_then(|d| d.position))
            .max()
            .unwrap_or(0);
        let tag = project.session.device_tag(station).unwrap_or_default();
        let children: Vec<Value> = (cards.iter().zip(last + 1..))
            .map(|((card, _), position)| json!({"device": card.to_string(), "position": position}))
            .collect();
        let tags: Vec<String> = cards.into_iter().map(|(_, t)| t).collect();
        let cmd = ew_api::AnyCommand {
            name: "assembly.attach".into(),
            params: json!({"parent": station.to_string(), "children": children}),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            self.last_station = Some(station);
            let text = strings::format(key!("toast-cards-attached"), &[&tags.join(", "), &tag]);
            self.notify(text, true);
        }
        out
    }
}

impl ShellModel {
    /// The open project's session, for the dialogs' specs and describe lines.
    pub(crate) fn project_session(&self) -> Option<&ew_api::Session> {
        self.project.as_ref().map(|p| &p.session)
    }

    /// The rack and slot editor's view of `station` (doc 10 §6), as the window draws it: made once per revision and
    /// station (FT-1763).
    pub fn rack_view(&self, station: Uuid) -> Option<ew_api::output::RackView> {
        let key = (self.revision()?, station);
        if let Some((k, view)) = &*self.rack_cache.borrow()
            && *k == key
        {
            return view.clone();
        }
        self.count(|c| c.rack_view += 1);
        let view = self.project_session()?.rack_view(station);
        *self.rack_cache.borrow_mut() = Some((key, view.clone()));
        view
    }

    /// The module parts the rack and slot editor offers (`Session::module_parts`).
    pub fn module_parts(&self) -> Vec<ew_api::output::ModulePart> {
        self.project_session()
            .map(ew_api::Session::module_parts)
            .unwrap_or_default()
    }

    /// What kind of valve terminal `device` is, in words (rc.2, F15): `Valve terminal · PLC station on PROFIBUS DP`,
    /// `Valve terminal · PLC station with bus node`, `Valve terminal · multipin, wired`; `None` for any other device.
    /// The terminal's menu says it; Properties' header can show it.
    pub fn terminal_kind_words(&self, device: Uuid) -> Option<String> {
        use ew_api::output::TerminalKind;
        Some(match self.project_session()?.terminal_kind(device)? {
            TerminalKind::BusNode { bus: Some(bus) } => strings::format(key!("kind-terminal-bus"), &[&bus]),
            TerminalKind::BusNode { bus: None } => strings::text(key!("kind-terminal-bus-node")).to_owned(),
            TerminalKind::Multipin => strings::text(key!("kind-terminal-multipin")).to_owned(),
        })
    }

    /// The valve parts a valve terminal's empty position offers in the rack dialog (rc.2, `Session::terminal_valve_parts`).
    pub fn terminal_valve_parts(&self) -> Vec<ew_api::output::ModulePart> {
        self.project_session()
            .map(ew_api::Session::terminal_valve_parts)
            .unwrap_or_default()
    }

    /// The `command.run` parameters that set position `position` of valve module `module` to `occupancy`
    /// (`valve.set_occupancy`), with the copies a library valve part needs: the rack dialog's position rows (rc.2).
    pub fn occupancy_params(&self, module: Uuid, position: u32, occupancy: Value) -> Value {
        let mut params =
            json!({ "module": module.to_string(), "position": position, "occupancy": occupancy });
        let part = params["occupancy"]["valve"]["part"]
            .as_str()
            .and_then(|p| p.parse::<Uuid>().ok());
        if let (Some(part), Some(session)) = (part, self.project_session()) {
            let copies = session.module_part_copies(part);
            if !copies.is_empty() {
                params["copies"] = Value::Array(copies);
            }
        }
        json!({ "cmd": "valve.set_occupancy", "params": params })
    }

    /// The rack the open rack dialog shows: its station's (`rack_view`), or in the new-station state the draft's
    /// (`Session::draft_rack`).
    pub fn shown_rack(&self) -> Option<ew_api::output::RackView> {
        match self.dialog() {
            Some(Dialog::Rack { draft: Some(d), .. }) => {
                Some(self.project_session()?.draft_rack(d.head, &d.modules))
            }
            Some(Dialog::Rack { station: Some(s), .. }) => self.rack_view(*s),
            _ => None,
        }
    }

    /// The project's PLCs (configuration projects) by name, as the new-station dialog offers them.
    pub fn plc_config_projects(&self) -> Vec<(Uuid, String)> {
        self.project_session()
            .map(ew_api::Session::plc_config_projects)
            .unwrap_or_default()
    }

    /// Whether `device` is a valve terminal: its rack dialog offers Make the overview page.
    pub fn is_valve_terminal(&self, device: Uuid) -> bool {
        self.project_session()
            .is_some_and(|s| s.fluid_menu(device).terminal)
    }

    /// The station a module was last given to: the station chooser's first button.
    pub fn last_station(&self) -> Option<Uuid> {
        self.last_station
    }

    /// The new-station dialog's draft; `None` outside it.
    pub fn station_draft(&self) -> Option<&StationDraft> {
        match self.dialog() {
            Some(Dialog::Rack { draft: Some(d), .. }) => Some(d),
            _ => None,
        }
    }

    /// The head parts the new-station dialog offers for what the station is (`Session::station_parts`).
    pub fn station_parts(&self) -> Vec<ew_api::output::ModulePart> {
        match (self.project_session(), self.station_draft()) {
            (Some(s), Some(d)) => s.station_parts(d.kind.word()),
            _ => Vec::new(),
        }
    }

    /// The devices already in the project that the new-station dialog offers as the station (rc.2 persona run): the bus
    /// devices in no PLC, those that lead a bus for a PLC (CPU), the others for every other kind; by tag.
    pub fn station_candidates(&self) -> Vec<(Uuid, String)> {
        let (Some(s), Some(d)) = (self.project_session(), self.station_draft()) else {
            return Vec::new();
        };
        let cpu = d.kind == StationKind::Cpu;
        s.free_bus_devices()
            .into_iter()
            .filter(|(_, _, leads)| *leads == cpu)
            .map(|(id, tag, _)| (id, tag))
            .collect()
    }

    /// The folio a page the rack dialog makes takes, asking nothing: New page's proposal, else the first number no page
    /// has.
    fn free_folio(&self) -> String {
        let (proposal, _) = self.new_page_proposal();
        if !proposal.is_empty() {
            return proposal;
        }
        let labels = self.page_labels();
        (1u64..)
            .map(|n| n.to_string())
            .find(|n| !labels.iter().any(|l| l == n))
            .unwrap_or_default()
    }

    /// The project's schematic pages (multi-line and single-line), which take a station's boxes: folio and title, in
    /// page order (the rack dialog's Draw on a page lists them).
    pub fn schematic_pages(&self) -> Vec<(String, String)> {
        self.page_rows()
            .iter()
            .filter(|r| matches!(r.kind.as_str(), "multi_line" | "single_line"))
            .map(|r| (r.folio.clone(), r.title.clone()))
            .collect()
    }

    /// The project's bus-topology pages: folio and title, in page order (the rack dialog's Put it on a bus page asks
    /// which when there are several).
    pub fn bus_pages(&self) -> Vec<(String, String)> {
        self.page_rows()
            .iter()
            .filter(|r| r.kind == "bus_topology")
            .map(|r| (r.folio.clone(), r.title.clone()))
            .collect()
    }

    /// The module part the open rack and slot editor adds: the one chosen, else the first offered.
    pub fn rack_part(&self) -> Option<ew_api::output::ModulePart> {
        let chosen = match self.dialog() {
            Some(Dialog::Rack { part, .. }) => *part,
            _ => None,
        };
        let parts = self.module_parts();
        chosen
            .and_then(|c| parts.iter().find(|p| p.copy == c).cloned())
            .or_else(|| parts.into_iter().next())
    }

    /// The rack dialog of `station`, or of a new station with `draft`.
    pub(super) fn rack_dialog(station: Option<Uuid>, draft: Option<StationDraft>) -> Dialog {
        Dialog::Rack {
            station,
            draft: draft.map(Box::new),
            part: None,
            count: 1,
            note: None,
            error: None,
        }
    }

    /// `plc.rack_editor {station}` (FR-PLC-028): opens the rack and slot editor of a station.
    pub(crate) fn act_plc_rack_editor(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.record_param(p, "station"));
        if self.rack_view(station).is_none() {
            return failed(key!("error-bad-param"), &["station"]);
        }
        self.last_station = Some(station);
        self.open_dialog(Self::rack_dialog(Some(station), None))
    }

    /// `plc.new_station {kind?}` (F18 of the rc.1 test): the rack dialog in the new-station state, a PLC (CPU) unless
    /// `kind` says another, in the project's first PLC, else a new one.
    pub(crate) fn act_plc_new_station(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match string_param(p, "kind") {
            Ok(Some(w)) => match StationKind::from_word(w) {
                Some(k) => k,
                None => return failed(key!("error-bad-param"), &["kind"]),
            },
            Ok(None) => StationKind::Cpu,
            Err(e) => return e,
        };
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let config = session.plc_config_projects().first().map(|(c, _)| *c);
        self.open_dialog(Self::rack_dialog(
            None,
            Some(StationDraft {
                kind,
                head: None,
                config,
                modules: Vec::new(),
            }),
        ))
    }

    /// `plc.rack_set {kind?, head?, config?, count?}`: the new-station dialog's choices (what it is, its head part, its
    /// PLC: an ID or `new`) and the count Add module fills.
    pub(crate) fn act_plc_rack_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match string_param(p, "kind") {
            Ok(Some(w)) => match StationKind::from_word(w) {
                Some(k) => Some(k),
                None => return failed(key!("error-bad-param"), &["kind"]),
            },
            Ok(None) => None,
            Err(e) => return e,
        };
        let head = match p.get("head") {
            Some(_) => {
                let offered = self
                    .project_session()
                    .map(|s| s.station_parts("any"))
                    .unwrap_or_default();
                Some(or_return!(offered_part(p, "head", &offered)).copy)
            }
            None => None,
        };
        let config = match string_param(p, "config") {
            Ok(Some("new")) => Some(None),
            Ok(Some(_)) => {
                let c = or_return!(self.record_param(p, "config"));
                let known = self
                    .project_session()
                    .is_some_and(|s| s.plc_config_projects().iter().any(|(x, _)| *x == c));
                if !known {
                    return failed(key!("error-bad-param"), &["config"]);
                }
                Some(Some(c))
            }
            Ok(None) => None,
            Err(e) => return e,
        };
        let count = match p.get("count").map(Value::as_u64) {
            Some(Some(n @ 1..=64)) => u16::try_from(n).ok(),
            Some(_) => return failed(key!("error-bad-param"), &["count"]),
            None => None,
        };
        let Some(Dialog::Rack { draft, count: c, .. }) = &mut self.dialog else {
            return Outcome::refused(key!("reason-rack-closed"));
        };
        if let Some(n) = count {
            *c = n;
        }
        if kind.is_some() || head.is_some() || config.is_some() {
            let Some(d) = draft else {
                return Outcome::refused(key!("reason-rack-made"));
            };
            if let Some(k) = kind {
                d.kind = k;
            }
            if let Some(h) = head {
                d.head = Some(h);
            }
            if let Some(c) = config {
                d.config = c;
            }
        }
        Outcome::Done
    }

    /// `plc.rack_part {part}`: the module part the open editor's Add module puts into a slot.
    pub(crate) fn act_plc_rack_part(&mut self, p: &Map<String, Value>) -> Outcome {
        let part = or_return!(self.record_param(p, "part"));
        if !self.module_parts().iter().any(|m| m.copy == part) {
            return failed(key!("error-bad-param"), &["part"]);
        }
        if let Some(Dialog::Rack { part: chosen, .. }) = &mut self.dialog {
            *chosen = Some(part);
        }
        Outcome::Done
    }

    /// The station of the open rack and slot editor (not a draft).
    fn rack_station(&self) -> Result<Uuid, Outcome> {
        match self.dialog() {
            Some(Dialog::Rack {
                station: Some(station),
                ..
            }) => Ok(*station),
            Some(Dialog::Rack { .. }) => Err(Outcome::refused(key!("reason-rack-draft"))),
            _ => Err(Outcome::refused(key!("reason-rack-closed"))),
        }
    }

    /// Writes `why` under the open rack dialog's slots and fails with it.
    fn rack_refuse(&mut self, why: String) -> Outcome {
        if let Some(Dialog::Rack { error, .. }) = &mut self.dialog {
            *error = Some(why.clone());
        }
        Outcome::Failed { message: why }
    }

    /// Commits a change of the open rack and slot editor: a refusal stays under its slots, a change clears it; a change
    /// that moved addresses says so under the slots (F18: the rack's order is the truth for slots and addresses).
    fn rack_commit(&mut self, name: &str, params: Value) -> Outcome {
        let addresses = |s: &Self| -> Vec<(Option<Uuid>, String)> {
            s.shown_rack()
                .map(|v| v.rows.into_iter().map(|r| (r.module, r.addresses)).collect())
                .unwrap_or_default()
        };
        let before = addresses(self);
        let cmd = ew_api::AnyCommand {
            name: name.to_owned(),
            params,
        };
        let out = self.run_commit(&cmd);
        let error = match &out {
            Outcome::Done => None,
            Outcome::Failed { message } => Some(message.clone()),
            Outcome::Refused { reason_key } => Some(strings::text(*reason_key).to_owned()),
        };
        let after = addresses(self);
        let moved = out == Outcome::Done
            && before
                .iter()
                .any(|(m, a)| m.is_some() && !a.is_empty() && after.iter().any(|(n, b)| n == m && b != a));
        if let Some(Dialog::Rack { error: e, note, .. }) = &mut self.dialog {
            *e = error;
            *note = moved.then(|| strings::text(key!("rack-addresses-moved")).to_owned());
        }
        out
    }

    /// The part a rack action names (`part`), else the one chosen in the editor; it must be a module part offered.
    fn rack_part_param(&self, p: &Map<String, Value>) -> Result<ew_api::output::ModulePart, Outcome> {
        if p.contains_key("part") {
            return offered_part(p, "part", &self.module_parts());
        }
        self.rack_part()
            .ok_or_else(|| Outcome::refused(key!("reason-rack-no-part")))
    }

    /// `plc.rack_add {slot, rack?, part?}`: modules of the part (the one named, else the editor's) into a free slot and,
    /// with the dialog's count above one, the slots after it. In the new-station state they join the draft; else one
    /// `plc.add_module` (a library part embedded with it): as `rack.slot` in a station whose modules have rack
    /// positions, else positional after the station's modules.
    pub(crate) fn act_plc_rack_add(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(slot) = p
            .get("slot")
            .and_then(Value::as_u64)
            .and_then(|s| u16::try_from(s).ok())
        else {
            return failed(key!("error-bad-param"), &["slot"]);
        };
        let rack = p
            .get("rack")
            .and_then(Value::as_u64)
            .and_then(|s| u16::try_from(s).ok());
        let count = match self.dialog() {
            Some(Dialog::Rack { count, .. }) => *count,
            _ => return Outcome::refused(key!("reason-rack-closed")),
        };
        // Only a free slot the editor offers takes a module: a taken one names its module, in words under the slots.
        let view = self.shown_rack();
        let racked = view.as_ref().is_some_and(|v| v.racked);
        let rows = view.as_ref().map_or(&[][..], |v| v.rows.as_slice());
        let at = |r: &&ew_api::output::RackRow| u32::from(slot) == r.slot && rack.unwrap_or(0) == r.rack;
        if !rows.iter().any(|r| r.module.is_none() && at(&r)) {
            let shown = match rack.filter(|r| *r > 0) {
                Some(r) => format!("{r}.{slot}"),
                None => slot.to_string(),
            };
            let why = match rows.iter().find(|r| r.module.is_some() && at(r)) {
                Some(taken) => strings::format(key!("rack-slot-taken"), &[&shown, &taken.tag]),
                None => strings::format(key!("rack-slot-not-free"), &[&shown]),
            };
            return self.rack_refuse(why);
        }
        let part = or_return!(self.rack_part_param(p));
        if let Some(Dialog::Rack {
            draft: Some(d),
            error,
            ..
        }) = &mut self.dialog
        {
            d.modules
                .extend(std::iter::repeat_n(part.copy, usize::from(count)));
            *error = None;
            return Outcome::Done;
        }
        let station = or_return!(self.rack_station());
        let copies = self
            .project_session()
            .map(|s| s.module_part_copies(part.copy))
            .unwrap_or_default();
        let mut params = json!({
            "station": station.to_string(),
            "part": {"part": {"copy": part.copy.to_string()}, "qty": "1"},
        });
        if racked || rack.is_some_and(|r| r > 0) {
            params["slot"] = json!(slot);
            params["rack"] = json!(rack.unwrap_or(0));
        }
        if count > 1 {
            params["count"] = json!(count);
        }
        if !copies.is_empty() {
            params["copies"] = Value::Array(copies);
        }
        self.rack_commit("plc.add_module", params)
    }

    /// `plc.rack_move {module, slot}`: a module of the open editor into the slot typed (`3`, `<rack>.<slot>`), with
    /// `fields.set plc_module.slot`; EW-PLC-011 refuses a slot its part does not allow.
    pub(crate) fn act_plc_rack_move(&mut self, p: &Map<String, Value>) -> Outcome {
        or_return!(self.rack_station());
        let module = or_return!(self.record_param(p, "module"));
        let text = match string_param(p, "slot") {
            Ok(Some(t)) => t.trim().to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["slot"]),
            Err(e) => return e,
        };
        let numbers: Vec<Option<u16>> = text.split('.').map(|n| n.trim().parse::<u16>().ok()).collect();
        let (rack, slot) = match numbers.as_slice() {
            [Some(slot)] => (0, *slot),
            [Some(rack), Some(slot)] => (*rack, *slot),
            _ => {
                return self.rack_refuse(strings::format(key!("rack-bad-slot"), &[&text]));
            }
        };
        self.rack_commit(
            "fields.set",
            json!({"kind": "device", "targets": [module.to_string()], "path": "plc_module.slot",
                "value": {"rack": {"rack": rack, "slot": slot}}}),
        )
    }

    /// `plc.rack_slot {slot, rack?, op, part?}`: a module's row: `up` and `down` swap it with the module above or below
    /// (positional modules their positions with one `assembly.attach`, racked ones their slots with one
    /// `fields.set_many`, or the slot beside it when that one is free); `empty` takes it out (a module drawn nowhere is
    /// deleted, a drawn one leaves the rack with `plc.remove_from_rack` and keeps its drawing); `replace` gives it the
    /// part named (`fields.set parts`); `draw` draws its box on the page shown, bound to its channels
    /// (`plc.draw_station`). In the new-station state the draft
    /// changes.
    pub(crate) fn act_plc_rack_slot(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(slot) = p
            .get("slot")
            .and_then(Value::as_u64)
            .and_then(|s| u32::try_from(s).ok())
        else {
            return failed(key!("error-bad-param"), &["slot"]);
        };
        let rack = p
            .get("rack")
            .and_then(Value::as_u64)
            .and_then(|s| u16::try_from(s).ok())
            .unwrap_or(0);
        let op = match string_param(p, "op") {
            Ok(Some(o)) if ["up", "down", "empty", "replace", "draw"].contains(&o) => o.to_owned(),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["op"]),
            Ok(None) => return failed(key!("error-missing-param"), &["op"]),
            Err(e) => return e,
        };
        let part = if op == "replace" {
            Some(or_return!(self.rack_part_param(p)))
        } else {
            None
        };
        if let Some(Dialog::Rack {
            draft: Some(d),
            error,
            ..
        }) = &mut self.dialog
        {
            let i = usize::try_from(slot).unwrap_or(usize::MAX).wrapping_sub(1);
            if i >= d.modules.len() {
                return failed(key!("error-bad-param"), &["slot"]);
            }
            match op.as_str() {
                "up" if i > 0 => d.modules.swap(i, i - 1),
                "down" if i + 1 < d.modules.len() => d.modules.swap(i, i + 1),
                "empty" => {
                    d.modules.remove(i);
                }
                "replace" => d.modules[i] = part.map_or(d.modules[i], |x| x.copy),
                "draw" => return Outcome::refused(key!("reason-rack-draft")),
                _ => {}
            }
            *error = None;
            return Outcome::Done;
        }
        let station = or_return!(self.rack_station());
        let Some(view) = self.rack_view(station) else {
            return failed(key!("error-bad-param"), &["station"]);
        };
        let modules: Vec<&ew_api::output::RackRow> =
            view.rows.iter().filter(|r| r.module.is_some()).collect();
        let Some(i) = modules.iter().position(|r| r.slot == slot && r.rack == rack) else {
            return failed(key!("error-bad-param"), &["slot"]);
        };
        let row = modules[i];
        let module = row.module.unwrap_or_default();
        match op.as_str() {
            "up" | "down" => {
                let target = if op == "up" {
                    slot.checked_sub(1)
                } else {
                    slot.checked_add(1)
                };
                let Some(target) = target.filter(|t| *t > 0) else {
                    return Outcome::Done;
                };
                let other = modules.iter().find(|r| r.rack == rack && r.slot == target);
                if row.positional {
                    let Some(other) = other else {
                        return Outcome::Done;
                    };
                    let q = self.project_session().map(ew_api::Session::query);
                    let position = |m: Uuid| {
                        q.as_ref().and_then(|q| {
                            q.devices()
                                .get(m.to_string().parse().ok()?)
                                .and_then(|d| d.position)
                        })
                    };
                    let (Some(a), Some(b)) = (position(module), other.module.and_then(position)) else {
                        return Outcome::Done;
                    };
                    let other = other.module.unwrap_or_default();
                    return self.rack_commit(
                        "assembly.attach",
                        json!({"parent": station.to_string(), "children": [
                            {"device": module.to_string(), "position": b},
                            {"device": other.to_string(), "position": a}]}),
                    );
                }
                let set = |m: Uuid, s: u32| {
                    json!({"kind": "device", "targets": [m.to_string()], "path": "plc_module.slot",
                        "value": {"rack": {"rack": rack, "slot": s}}})
                };
                match other.and_then(|o| o.module) {
                    Some(o) => self.rack_commit(
                        "fields.set_many",
                        json!({"edits": [set(module, target), set(o, slot)]}),
                    ),
                    None => self.rack_commit("fields.set", set(module, target)),
                }
            }
            "empty" if row.drawn => {
                self.rack_commit("plc.remove_from_rack", json!({"device": module.to_string()}))
            }
            "empty" => self.rack_commit(
                "edit.delete",
                json!({"targets": [{"kind": "device", "id": module.to_string()}]}),
            ),
            "replace" => {
                let Some(part) = part else {
                    return Outcome::refused(key!("reason-rack-no-part"));
                };
                let copies = self
                    .project_session()
                    .map(|s| s.module_part_copies(part.copy))
                    .unwrap_or_default();
                self.rack_commit(
                    "fields.set",
                    json!({"kind": "device", "targets": [module.to_string()], "path": "parts",
                        "value": [{"part": {"copy": part.copy.to_string()}, "qty": "1"}], "copies": copies}),
                )
            }
            _ => self.draw_from_rack(station, Some(module)),
        }
    }

    /// `plc.draw_station`'s parameters for `station` on `page` (`Session::station_draw_params`): each device not drawn
    /// yet a box 39.7 × 31.75 mm (wider where its pins need it), with its part's connection points laid out as the black box
    /// tool lays them (`tools::black_box::layout`), beside the one before it from the next free place, a new row under
    /// the first where the next box would pass the drawing area's right. A device whose part has no black-box entry is
    /// left out. `None` without the page shown, its frame or the catalogue.
    fn station_boxes(&mut self, station: Uuid, page: Uuid) -> Option<Value> {
        const SIZE: (i64, i64, i64) = (39_687_500, 31_750_000, 7_937_500);
        let mut params = self.project_session()?.station_draw_params(station, page)?;
        let grid = crate::tools::wire::PageGridMap::new(self.canvas()?.page().tables.grid);
        let cat = self.catalogue()?;
        let snap = |v: i64| grid.coord(grid.index(v));
        let at = |i: usize| params["at"][i].as_i64().unwrap_or(0);
        let (x0, y0, right) = (
            snap(at(0)),
            snap(at(1)),
            params["right"].as_i64().unwrap_or(i64::MAX),
        );
        let (mut x, mut y) = (x0, y0);
        let mut boxes = Vec::new();
        for d in params["devices"].as_array().cloned().unwrap_or_default() {
            let (Some(device), Some(part)) =
                (d[0].as_str(), d[1].as_str().and_then(|p| p.parse::<Uuid>().ok()))
            else {
                continue;
            };
            let Some(entry) = cat
                .parts
                .iter()
                .find(|r| r.copy == part)
                .and_then(|r| r.functions.iter().find(|f| f.black_box))
            else {
                continue;
            };
            if x > x0 && x + SIZE.0 > right {
                x = x0;
                y += SIZE.1 + 2 * SIZE.2;
            }
            let Some((pos, outline, cps)) = crate::tools::black_box::layout(
                grid,
                (x, y),
                (snap(x + SIZE.0), snap(y + SIZE.1)),
                &entry.cps,
                &[],
            ) else {
                continue;
            };
            let width = outline.iter().map(|p| p.0).max().unwrap_or(SIZE.0);
            boxes.push(json!({
                "device": device,
                "pos": [pos.0, pos.1],
                "box": {
                    "outline": outline.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
                    "cps": cps.iter().map(|(i, p, dir)| json!({"idx": i, "pos": [p.0, p.1], "dir": dir.word()}))
                        .collect::<Vec<_>>(),
                },
            }));
            x = snap(x + width + SIZE.2);
        }
        if let Some(m) = params.as_object_mut() {
            for k in ["at", "right", "devices"] {
                m.remove(k);
            }
            m.insert("boxes".into(), Value::Array(boxes));
        }
        Some(params)
    }

    /// Draws devices of `station` not drawn yet on the schematic page shown, with one `plc.draw_station`: all of them
    /// (`only` absent: Draw on this page), or the one module `only` (its row's Draw…), each bound to its channels as a
    /// box drawn by hand is (normaliser N3). The dialog closes when it drew; a refusal stays under the slots.
    fn draw_from_rack(&mut self, station: Uuid, only: Option<Uuid>) -> Outcome {
        if !matches!(
            self.page_rows().get(self.page_index()).map(|r| r.kind.as_str()),
            Some("multi_line" | "single_line")
        ) {
            return self.rack_refuse(strings::text(key!("reason-rack-draw-page")).to_owned());
        }
        let page = self.page_rows().get(self.page_index()).map(|r| r.id);
        let Some(mut params) = page.and_then(|page| self.station_boxes(station, page)) else {
            return self.rack_refuse(strings::text(key!("reason-rack-no-frame")).to_owned());
        };
        if let (Some(module), Some(boxes)) = (only, params["boxes"].as_array_mut()) {
            boxes.retain(|b| b["device"].as_str() == Some(module.to_string().as_str()));
        }
        let out = self.rack_commit("plc.draw_station", params);
        if out == Outcome::Done {
            self.dialog = None;
        }
        out
    }

    /// `plc.rack_create` (F18 of the rc.1 test): the new-station dialog's Create: the station and its modules, drawn
    /// nowhere yet, with one `plc.add_station` (library parts embedded with it), one undo step; the dialog then shows
    /// the station's rack and says what comes next.
    pub(crate) fn act_plc_rack_create(&mut self) -> Outcome {
        let Some(d) = self.station_draft().cloned() else {
            return Outcome::refused(key!("reason-rack-made"));
        };
        let Some(head) = d.head else {
            return self.rack_refuse(strings::text(key!("rack-choose-head")).to_owned());
        };
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let mut copies: Vec<Value> = Vec::new();
        for part in std::iter::once(&head).chain(&d.modules) {
            for doc in session.module_part_copies(*part) {
                if !copies.contains(&doc) {
                    copies.push(doc);
                }
            }
        }
        let stations = |s: &ew_api::Session| -> Vec<Uuid> {
            s.query()
                .devices()
                .iter()
                .filter(|(_, x)| x.aspects.plc_station.is_some())
                .map(|(id, _)| id.as_uuid())
                .collect()
        };
        let before = stations(session);
        let use_of = |c: &Uuid| json!({"part": {"copy": c.to_string()}, "qty": "1"});
        let mut params = json!({
            "part": use_of(&head),
            "modules": d.modules.iter().map(|m| json!({"part": use_of(m)})).collect::<Vec<_>>(),
            "copies": copies,
        });
        if let Some(c) = d.config {
            params["config_project"] = json!(c.to_string());
        }
        let out = self.rack_commit("plc.add_station", params);
        if out != Outcome::Done {
            return out;
        }
        let made = self
            .project_session()
            .and_then(|s| stations(s).into_iter().find(|x| !before.contains(x)));
        if let Some(Dialog::Rack {
            station, draft, note, ..
        }) = &mut self.dialog
        {
            *station = made;
            *draft = None;
            *note = Some(strings::text(key!("rack-created")).to_owned());
        }
        out
    }

    /// `plc.rack_next {step}` (F18 of the rc.1 test): what comes after the station is made, from its rack dialog:
    /// `draw` puts the station's symbol on the pointer on a schematic page (a valve terminal's box of positions on a fluid
    /// page); `fluid` draws a valve terminal on a fluid page (`fluid.draw_terminal`); `overview` makes a valve terminal's overview
    /// pages (`fluid.generate_overview`, previewed); `bus` opens the first bus-topology page and puts the station's first
    /// interface not drawn there on the pointer.
    pub(crate) fn act_plc_rack_next(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.rack_station());
        match string_param(p, "step") {
            // A valve terminal on a fluid page: drawn as its positions there (rc.2).
            Ok(Some("fluid")) => {
                self.dialog = None;
                self.dispatch("fluid.draw_terminal", &json!({ "terminal": station.to_string() }))
            }
            Ok(Some("draw"))
                if p.get("folio").is_none()
                    && self.page_rows().get(self.page_index()).is_some_and(|r| r.kind == "fluid")
                    && self.is_valve_terminal(station) =>
            {
                self.dialog = None;
                self.dispatch("fluid.draw_terminal", &json!({ "terminal": station.to_string() }))
            }
            // The station and its modules not drawn yet, in a row in slot order, one undo step (rc.2): on the page shown,
            // the schematic page `folio`, or a new one (`folio` `new`, asking nothing; a persona run's station landed
            // on page 1 without a word).
            Ok(Some("draw")) => {
                let folio = match string_param(p, "folio") {
                    Ok(Some("new")) => {
                        let folio = self.free_folio();
                        let title = strings::text(key!("rack-draw-page-title"));
                        let made = self.dispatch(
                            "page.new",
                            &json!({ "folio": folio, "type": "multi_line", "title": title }),
                        );
                        if made != Outcome::Done {
                            return made;
                        }
                        Some(folio)
                    }
                    Ok(Some(f)) if self.schematic_pages().iter().any(|(x, _)| x == f) => Some(f.to_owned()),
                    Ok(Some(_)) => return failed(key!("error-bad-param"), &["folio"]),
                    Ok(None) => None,
                    Err(e) => return e,
                };
                if let Some(folio) = folio {
                    let opened = self.dispatch("page.goto", &json!({ "folio": folio }));
                    if opened != Outcome::Done {
                        return opened;
                    }
                }
                self.draw_from_rack(station, None)
            }
            Ok(Some("overview")) => {
                self.dialog = None;
                self.act_fluid_generate_overview(&Map::from_iter([(
                    "station".to_owned(),
                    Value::String(station.to_string()),
                )]))
            }
            Ok(Some("bus")) => {
                // The bus page named, else the only one; with none one is made, asking nothing; with several the
                // window asks which (rc.2 persona run: the button ended in "make one with New page first").
                // A station whose part has no bus interface cannot stand on a bus page: said under the slots (the
                // rc.2 persona run's valve terminal closed the dialog and did nothing).
                let (tag, bus) = self.project_session().map_or((String::new(), false), |s| {
                    (
                        s.device_tag(station).unwrap_or_default(),
                        s.has_bus_interfaces(station),
                    )
                });
                if !bus {
                    return self.rack_refuse(strings::format(key!("rack-no-bus-interface"), &[&tag]));
                }
                let pages = self.bus_pages();
                let folio = match string_param(p, "folio") {
                    Ok(Some(f)) if pages.iter().any(|(x, _)| x == f) => f.to_owned(),
                    Ok(Some(_)) => return failed(key!("error-bad-param"), &["folio"]),
                    Err(e) => return e,
                    Ok(None) => match pages.as_slice() {
                        [(only, _)] => only.clone(),
                        [] => {
                            let folio = self.free_folio();
                            let title = strings::text(key!("rack-bus-page-title"));
                            let made = self.dispatch(
                                "page.new",
                                &json!({ "folio": folio, "type": "bus_topology", "title": title }),
                            );
                            if made != Outcome::Done {
                                return self
                                    .rack_refuse(strings::text(key!("reason-rack-no-bus-page")).to_owned());
                            }
                            folio
                        }
                        _ => {
                            return self
                                .rack_refuse(strings::text(key!("reason-rack-which-bus-page")).to_owned());
                        }
                    },
                };
                // Every interface not drawn there, a box each (rc.2 persona run: the CPU stood there with its DP alone).
                let interfaces = self
                    .project_session()
                    .map(|s| s.undrawn_interfaces(station, &folio))
                    .unwrap_or_default();
                if interfaces.is_empty() {
                    return self.rack_refuse(strings::format(key!("rack-bus-drawn"), &[&tag, &folio]));
                }
                self.dialog = None;
                let opened = self.dispatch("page.goto", &json!({ "folio": folio }));
                if opened != Outcome::Done {
                    return opened;
                }
                let Some(cat) = self.catalogue() else {
                    return Outcome::refused(key!("reason-no-catalogue"));
                };
                let labels: Vec<&str> = interfaces.iter().map(|i| i.label.as_str()).collect();
                let label = format!("{tag} {}", labels.join(", "));
                let target = json!({"device": {"device": station.to_string()}});
                self.arm_ports(&cat, target, label, &interfaces, None)
            }
            Ok(_) => failed(key!("error-bad-param"), &["step"]),
            Err(e) => e,
        }
    }

    /// `plc.add_to_plc {device, config?}` (F19 of the rc.1 test): a bus device that belongs to no PLC becomes a station
    /// of the PLC `config` (absent: a new one) with one `plc.add_station`, and its rack dialog opens, so its modules are
    /// added there; a network switch has no rack, and a toast says which PLC it belongs to now. A module, a station and
    /// a device without an addressed bus port are refused in words.
    pub(crate) fn act_plc_add_to_plc(&mut self, p: &Map<String, Value>) -> Outcome {
        let device = or_return!(self.record_param(p, "device"));
        let config = match p.get("config") {
            Some(_) => Some(or_return!(self.record_param(p, "config"))),
            None => None,
        };
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let tag = session.device_tag(device).unwrap_or_default();
        if !session.bus_device(device) {
            return failed(key!("error-not-a-bus-device"), &[&tag]);
        }
        let mut params = json!({ "device": device.to_string() });
        if let Some(c) = config {
            params["config_project"] = json!(c.to_string());
        }
        let out = self.run_commit(&ew_api::AnyCommand {
            name: "plc.add_station".into(),
            params,
        });
        if out != Outcome::Done {
            return out;
        }
        // A network switch belongs to the PLC with no rack of its own: a toast says so, with Undo (rc.2 persona run).
        let switch = self.project_session().is_some_and(|s| s.network_switch(device));
        if switch {
            let plc = match self.project_session().map(|s| s.plc_offer(device)) {
                Some(ew_api::output::PlcOffer::Station(name)) => name,
                _ => String::new(),
            };
            self.notify(strings::format(key!("toast-plc-joined"), &[&tag, &plc]), true);
            // Chosen in the new-station dialog, the dialog is done.
            if matches!(self.dialog, Some(Dialog::Rack { .. })) {
                self.dialog = None;
            }
            return Outcome::Done;
        }
        self.open_dialog(Self::rack_dialog(Some(device), None))
    }

    /// The describe lines of the rack dialog: a draft's choices, then the slots and findings.
    pub(crate) fn rack_lines(&self) -> Vec<String> {
        let Some(v) = self.shown_rack() else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        if let Some(d) = self.station_draft() {
            lines.push(format!("  kind {}", d.kind.word()));
            if !v.part.is_empty() {
                lines.push(format!("  head \"{}\"", v.part));
            }
            let config = d
                .config
                .and_then(|c| {
                    self.project_session()?
                        .plc_config_projects()
                        .into_iter()
                        .find(|(x, _)| *x == c)
                        .map(|(_, n)| n)
                })
                .unwrap_or_else(|| strings::text(key!("rack-new-plc")).to_owned());
            lines.push(format!("  plc \"{config}\""));
        }
        for (label, code) in &v.order_codes {
            lines.push(format!("  order_code \"{label}\" \"{code}\""));
        }
        for r in &v.rows {
            let mut line = format!("  slot {}", rack_slot_text(r));
            if r.module.is_some() {
                line.push_str(&format!(" \"{}\" \"{}\"", r.tag, r.part));
                if !r.gives.is_empty() {
                    line.push_str(&format!(" gives \"{}\"", r.gives));
                }
                if !r.addresses.is_empty() {
                    line.push_str(&format!(" at \"{}\"", r.addresses));
                }
                if !r.allowed.is_empty() {
                    let allowed: Vec<String> = r.allowed.iter().map(u16::to_string).collect();
                    line.push_str(&format!(" allowed {}", allowed.join(",")));
                }
                if !r.positions.is_empty() {
                    line.push_str(&format!(" positions {}", rack_positions_text(r)));
                }
            } else {
                line.push_str(" free");
            }
            lines.push(line);
        }
        let lang = strings::rule_language();
        for f in &v.findings {
            lines.push(format!(
                "  finding \"{}\"",
                ew_api::checks::messages::render(f, lang)
            ));
        }
        lines
    }
}

impl ShellModel {
    /// `plc.preview {cmd, params}` (FR-PLC-014, FR-PLC-015): the previewed PLC addressing command `cmd` (`plc.readdress`,
    /// `plc.convert_manual_to_auto`, `plc.freeze`) as a card in the Preview tab, whose list of changes is the command's
    /// notices (every channel whose address moves, old → new); Apply commits it as one undo step.
    pub(crate) fn act_plc_preview(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "cmd") {
            Ok(Some(n)) if ["plc.readdress", "plc.convert_manual_to_auto", "plc.freeze"].contains(&n) => {
                n.to_owned()
            }
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["cmd"]),
            Ok(None) => return failed(key!("error-missing-param"), &["cmd"]),
            Err(e) => return e,
        };
        let params = p.get("params").cloned().unwrap_or_else(|| json!({}));
        self.preview_bulk(&name, params)
    }

    /// `plc.instrument_list {config, path?}` (FR-PLC-050, doc 10 §4.7): a configuration project row's Import instrument
    /// list…: the instrument list (a CSV or the first sheet of a workbook) as `plc.apply_instrument_list`, previewed in
    /// the Preview tab; Apply commits it as one undo step. In the window a missing path asks for the table.
    pub(crate) fn act_plc_instrument_list(&mut self, p: &Map<String, Value>) -> Outcome {
        let config = or_return!(self.record_param(p, "config"));
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => {
                return self.need_path("plc.instrument_list", crate::shell::Prompt::OpenTable, "path", p);
            }
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = or_return!(read_exchange_file(&path, &file));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let config = config.to_string();
        let cmd = match session.instrument_list(Some(&config), &bytes) {
            Ok(cmd) => cmd,
            Err(e) => return failed(key!("error-plc-import-file"), &[&file, &e.to_string()]),
        };
        self.preview_bulk(&cmd.name, cmd.params)
    }

    /// `fluid.generate_overview {station}` (FR-FLU-033, doc 11 §5.5): `valve_terminal.generate_overview` of a valve
    /// terminal as a card in the Preview tab (`Session::overview_params`: the band of the page's frame, the starter
    /// overview module for the members not drawn yet), whose changes list each page's members, the pages it creates
    /// and a page it empties; the derived changes show the tags a new page break changes. Apply commits it as one
    /// undo step.
    pub(crate) fn act_fluid_generate_overview(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = if p.contains_key("station") {
            or_return!(self.record_param(p, "station"))
        } else {
            match self.selected_valve_terminal() {
                Ok(s) => s,
                Err(reason) => return Outcome::refused(reason),
            }
        };
        let Some(mut params) = self.project_session().and_then(|s| s.overview_params(station)) else {
            return Outcome::refused(key!("reason-overview-no-page"));
        };
        // A new overview page names the terminal it shows (the persona round: page 202 had no title).
        if let Some(tag) = self.project_session().and_then(|s| s.device_tag(station)) {
            params["title"] = Value::String(strings::format(key!("overview-page-title"), &[&tag]));
        }
        self.preview_bulk("valve_terminal.generate_overview", params)
    }

    /// The valve terminal the selection is (one device whose `fluid_menu` says so); else the reason in words: select
    /// one, or the selected device is none.
    pub(crate) fn selected_valve_terminal(&self) -> Result<Uuid, Key<'static>> {
        let session = self.project_session().ok_or(key!("reason-no-project"))?;
        let selected = self.selected_devices();
        let [device] = selected.as_slice() else {
            return Err(key!("reason-overview-select"));
        };
        let device: Uuid = device.parse().map_err(|_| key!("reason-overview-select"))?;
        if session.fluid_menu(device).terminal {
            Ok(device)
        } else {
            Err(key!("reason-overview-not-terminal"))
        }
    }

    /// `fluid.valve_add {module, position}` (doc 11 §7, the persona round): a valve terminal's Add valve at position
    /// n: one `valve.set_occupancy` with the part of its other valves, else the libraries' valve for its positions
    /// (embedded with it), then the new valve's symbol on the pointer when the page shown takes it, so that a click
    /// places it; a toast names the valve and its position.
    pub(crate) fn act_fluid_valve_add(&mut self, p: &Map<String, Value>) -> Outcome {
        let module = or_return!(self.record_param(p, "module"));
        let Some(position) = p.get("position").and_then(Value::as_u64) else {
            return failed(key!("error-missing-param"), &["position"]);
        };
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(part) = session.fluid_menu(module).valve_part else {
            return Outcome::refused(key!("reason-valve-no-part"));
        };
        let mut params = json!({
            "module": module.to_string(), "position": position,
            "occupancy": { "valve": { "state": "used", "part": part.to_string() } },
        });
        let copies = session.module_part_copies(part);
        if !copies.is_empty() {
            params["copies"] = Value::Array(copies);
        }
        let out = self.run_logged("valve.set_occupancy".to_owned(), params);
        if out != Outcome::Done {
            return out;
        }
        let Some(session) = self.project_session() else {
            return out;
        };
        let valve = session.valve_at(module, position);
        let tag = valve.and_then(|v| session.device_tag(v)).unwrap_or_default();
        let module_tag = session.device_tag(module).unwrap_or_default();
        self.notify(
            strings::format(
                key!("toast-valve-added"),
                &[&tag, &position.to_string(), &module_tag],
            ),
            true,
        );
        if let Some(valve) = valve.filter(|_| self.canvas().is_some()) {
            let mut pick = Map::new();
            pick.insert("function".into(), Value::String(valve.to_string()));
            let _ = self.act_place_pick(&pick);
        }
        Outcome::Done
    }

    /// `fluid.go_output {channel?}` (rc.2, F15): a terminal valve's Go to output: the PLC list shown with the channel
    /// that drives the solenoid marked; refused in words while no output drives it.
    pub(crate) fn act_fluid_go_output(&mut self, p: &Map<String, Value>) -> Outcome {
        if p.get("channel").is_none_or(Value::is_null) {
            return Outcome::refused(key!("reason-solenoid-no-output"));
        }
        let channel = or_return!(self.record_param(p, "channel"));
        let shown = self.dispatch("navigator.show", &json!({ "kind": "plc" }));
        if shown != Outcome::Done {
            return shown;
        }
        self.dispatch(
            "navigator.select",
            &json!({ "kind": "plc", "keys": [format!("channel:{channel}")] }),
        )
    }

    /// `fluid.draw_slots {rep}` (rc.2, F15): Draw as slots on a valve terminal drawn before as a free box: the preview
    /// of `valve_terminal.draw_slots`, committed from it as one undo step.
    pub(crate) fn act_fluid_draw_slots(&mut self, p: &Map<String, Value>) -> Outcome {
        let rep = or_return!(self.record_param(p, "rep"));
        self.preview_bulk("valve_terminal.draw_slots", json!({ "rep": rep.to_string() }))
    }

    /// `fluid.cell_fill {terminal, module, position, replace?, part?}` (rc.2, F15): Fill position n… or Replace… on a
    /// cell of a valve terminal drawn as its positions on the page shown: with `part`, that valve is placed in the cell
    /// ([`Self::fill_cell`]); without, the device browser opens narrowed to valves for a terminal, and its choice does
    /// it. Refused in words when the terminal is not drawn as its positions on the page shown.
    pub(crate) fn act_fluid_cell_fill(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = or_return!(self.record_param(p, "terminal"));
        let module = or_return!(self.record_param(p, "module"));
        let Some(position) = p.get("position").and_then(Value::as_u64) else {
            return failed(key!("error-missing-param"), &["position"]);
        };
        let replace = p.get("replace").and_then(Value::as_bool).unwrap_or(false);
        let at = self.canvas().and_then(|c| {
            let t = &c.page().tables;
            let grid = crate::tools::wire::PageGridMap::new(t.grid);
            let cell = t
                .reps
                .iter()
                .filter(|r| r.device == Some(terminal))
                .filter_map(|r| r.black_box.as_ref())
                .flat_map(|b| b.cells.iter())
                .find(|c| c.module == module && u64::from(c.position) == position)?;
            Some((
                grid.coord(grid.index(cell.rect[0]) + 1),
                grid.coord(grid.index(cell.rect[1]) + 2),
            ))
        });
        let Some(at) = at else {
            return Outcome::refused(key!("reason-cell-not-shown"));
        };
        let fill = crate::tools::CellFill {
            terminal,
            replace,
            at,
        };
        if p.contains_key("part") {
            let part = or_return!(self.record_param(p, "part"));
            return self.fill_cell(fill, part);
        }
        let out = self.act_open_picker(crate::tools::PickerMode::Parts);
        if out == Outcome::Done
            && let Some(picker) = &mut self.tools.picker
        {
            picker.cell = Some(fill);
            picker.refilter();
        }
        out
    }

    /// Places valve part `part` in the cell `fill` names: one `device.place` of the part's main function with its
    /// symbol at the cell's point, into the terminal (the cell is its position; `replace` removes the cell's valve or
    /// blanking plate), one undo step; the toast says where it sits.
    pub(crate) fn fill_cell(&mut self, fill: crate::tools::CellFill, part: Uuid) -> Outcome {
        self.tools.picker = None;
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let Some(row) = cat.parts.iter().find(|r| r.copy == part) else {
            return Outcome::refused(key!("reason-valve-no-part"));
        };
        let Some(f) = row.functions.iter().find(|f| f.main).or(row.functions.first()) else {
            return Outcome::refused(key!("reason-no-symbol-for-function"));
        };
        let Some(key) = f.symbol.clone() else {
            return Outcome::refused(key!("reason-no-symbol-for-function"));
        };
        let (Some(project), Some(c)) = (self.project.as_ref(), self.canvas()) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let symbol = cat
            .symbols
            .iter()
            .find(|s| s.key == key)
            .and_then(|s| s.copy)
            .or_else(|| project.session.starter_symbol_copy(&key));
        let Some(symbol) = symbol else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let mut params = json!({
            "page": c.page().tables.page.to_string(),
            "symbol": symbol.to_string(),
            "pos": [fill.at.0, fill.at.1],
            "target": {"from_part": {"part": part.to_string(), "template": f.index,
                "letter": row.letter.clone().unwrap_or_else(|| "Q".to_owned())}},
            "into": fill.terminal.to_string(),
        });
        if fill.replace {
            params["replace"] = Value::Bool(true);
        }
        match project.session.starter_embed_part(part, &key) {
            Ok(Some(embed)) => params["copies"] = embed["copies"].clone(),
            Ok(None) => {}
            Err(keys) => return failed(key!("error-part-fndef-missing"), &[&keys.join(", ")]),
        }
        self.run_logged("device.place".to_owned(), params)
    }

    /// `fluid.draw_solenoid {function}` (rc.2, F11): a valve's Draw solenoid 14 on an electrical page…: on the page
    /// shown when it is an electrical (multi-line) page, else on the first one, the solenoid's coil symbol on the
    /// pointer, so that a click places it as that solenoid of the valve; a toast says which and where. Refused in words
    /// without an electrical page.
    pub(crate) fn act_fluid_draw_solenoid(&mut self, p: &Map<String, Value>) -> Outcome {
        let function = or_return!(self.record_param(p, "function"));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some((tag, name, _)) = session.solenoid_of(function) else {
            return Outcome::refused(key!("reason-not-solenoid"));
        };
        let out = self.arm_on_page(function, "multi_line", key!("reason-no-electrical-page"));
        if out != Outcome::Done {
            return out;
        }
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.notify(
            strings::format(key!("toast-solenoid-draw"), &[&name, &tag, &folio]),
            false,
        );
        Outcome::Done
    }

    /// `fluid.draw_plug {terminal?}` (rc.2): a multipin valve terminal's Draw the plug on an electrical page…, from any
    /// page, as a valve's Draw solenoid: the plug's box on the pointer there, so that a click places it; a toast says
    /// whose and where. Without `terminal`, of the selected multipin terminal. Refused in words for another device or
    /// without an electrical page.
    pub(crate) fn act_fluid_draw_plug(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = if p.contains_key("terminal") {
            or_return!(self.record_param(p, "terminal"))
        } else {
            match self.selected_multipin_terminal() {
                Ok(t) => t,
                Err(reason) => return Outcome::refused(reason),
            }
        };
        let Some(plug) = self.project_session().and_then(|s| s.terminal_plug(terminal)) else {
            return Outcome::refused(key!("reason-not-multipin"));
        };
        let out = self.arm_on_page(plug.function, "multi_line", key!("reason-no-electrical-page-plug"));
        if out != Outcome::Done {
            return out;
        }
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.notify(strings::format(key!("toast-plug-draw"), &[&plug.tag, &folio]), false);
        Outcome::Done
    }

    /// The multipin valve terminal the selection is; else the reason in words: select one, or the selected device is
    /// none.
    pub(crate) fn selected_multipin_terminal(&self) -> Result<Uuid, Key<'static>> {
        let session = self.project_session().ok_or(key!("reason-no-project"))?;
        let selected = self.selected_devices();
        let [device] = selected.as_slice() else {
            return Err(key!("reason-plug-select"));
        };
        let device: Uuid = device.parse().map_err(|_| key!("reason-plug-select"))?;
        match session.terminal_kind(device) {
            Some(ew_api::output::TerminalKind::Multipin) => Ok(device),
            _ => Err(key!("reason-not-multipin")),
        }
    }

    /// `fluid.draw_terminal {terminal}` (rc.2): a valve terminal made as a station (its valves in its rack) drawn on a
    /// fluid page: the fluid page shown, else the nearest one, with its box of positions on the pointer, its valves in
    /// their cells, so that a click places it; a toast says which and where. Refused in words for a device that is no
    /// valve terminal or without a fluid page.
    pub(crate) fn act_fluid_draw_terminal(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = or_return!(self.record_param(p, "terminal"));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        if session.terminal_kind(terminal).is_none() {
            return Outcome::refused(key!("reason-not-valve-terminal"));
        }
        let main = session.main_function(terminal).map(|(f, _)| f);
        let tag = session.device_tag(terminal).unwrap_or_default();
        let out = match main {
            Some(main) => self.arm_on_page(main, "fluid", key!("reason-no-fluid-page-terminal")),
            // Made in the New station dialog, it has no main function yet: its box gives it one.
            None => match self.show_page_of_kind("fluid", key!("reason-no-fluid-page-terminal")) {
                Outcome::Done => self.arm_terminal_box(terminal),
                other => other,
            },
        };
        if out != Outcome::Done {
            return out;
        }
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.notify(strings::format(key!("toast-terminal-draw"), &[&tag, &folio]), false);
        Outcome::Done
    }

    /// Arms the placement of `function` on the page shown when it is of kind `kind` (`multi_line`, `fluid`), else on the
    /// nearest such page before it (a project's electrical pages usually come before its fluid pages), else after it;
    /// refused with `none` without one.
    fn arm_on_page(&mut self, function: Uuid, kind: &str, none: Key<'static>) -> Outcome {
        let out = self.show_page_of_kind(kind, none);
        if out != Outcome::Done {
            return out;
        }
        let mut pick = Map::new();
        pick.insert("function".into(), Value::String(function.to_string()));
        self.act_place_pick(&pick)
    }

    /// Shows the page shown when it is of kind `kind`, else the nearest such page before it, else after it; refused
    /// with `none` without one.
    fn show_page_of_kind(&mut self, kind: &str, none: Key<'static>) -> Outcome {
        let of_kind = |i: &usize| self.page_rows().get(*i).is_some_and(|r| r.kind == kind);
        let here = self.page_index();
        let Some(page) = (here..=here)
            .chain((0..here).rev())
            .chain(here + 1..self.page_count())
            .find(of_kind)
        else {
            return Outcome::refused(none);
        };
        if page != here {
            let out = self.walk(page);
            if out != Outcome::Done {
                return out;
            }
            // The page change ends the tool now, not after a tool is armed on the new page.
            self.sync_tool_context();
        }
        Outcome::Done
    }

    /// `fluid.set_medium {line, medium}` (FR-FLU-009): the medium the fluid line's net carries, defined at the line's
    /// end with one `fluid.medium.define`; a toast names the medium and the line.
    pub(crate) fn act_fluid_set_medium(&mut self, p: &Map<String, Value>) -> Outcome {
        let line = or_return!(self.record_param(p, "line"));
        let medium = or_return!(self.record_param(p, "medium"));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some((function, idx)) = session.line_end_cp(line) else {
            return Outcome::refused(key!("reason-line-no-port"));
        };
        let name = session
            .media()
            .into_iter()
            .find(|(m, _)| *m == medium)
            .map(|(_, n)| n)
            .unwrap_or_default();
        let cmd = ew_api::AnyCommand {
            name: "fluid.medium.define".into(),
            params: json!({"function": function.to_string(), "idx": idx, "medium": medium.to_string()}),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            self.notify(strings::format(key!("toast-line-medium"), &[&name]), true);
        }
        out
    }
}

/// A slot of the rack and slot editor as the window and the describe text print it: `3`, or `<rack>.<slot>`.
pub fn rack_slot_text(r: &ew_api::output::RackRow) -> String {
    if r.rack == 0 {
        r.slot.to_string()
    } else {
        format!("{}.{}", r.rack, r.slot)
    }
}

/// What one valve position holds in words: the valve's tag, `blanking plate` or `empty`.
pub fn rack_position_text(p: &ew_api::output::RackPosition) -> String {
    match (&p.valve, p.blank) {
        (Some(tag), _) => tag.clone(),
        (None, true) => strings::text(key!("rack-blank")).to_owned(),
        (None, false) => strings::text(key!("rack-empty")).to_owned(),
    }
}

/// A valve terminal module's positions in words: `1 -201U3-Q1 · 2 blank · 3 empty`.
pub fn rack_positions_text(r: &ew_api::output::RackRow) -> String {
    r.positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let what = rack_position_text(p);
            format!("{} {what}", i + 1)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The part of `offered` that parameter `name` names: its copy ID, or its part number (a script's way).
fn offered_part(
    p: &Map<String, Value>,
    name: &str,
    offered: &[ew_api::output::ModulePart],
) -> Result<ew_api::output::ModulePart, Outcome> {
    let text = match string_param(p, name)? {
        Some(t) => t.trim(),
        None => return Err(failed(key!("error-missing-param"), &[name])),
    };
    let id = text.parse::<Uuid>().ok();
    offered
        .iter()
        .find(|m| Some(m.copy) == id || m.text.split(" · ").next() == Some(text))
        .cloned()
        .ok_or_else(|| failed(key!("error-bad-param"), &[name]))
}

/// What a new station is (F18 of the rc.1 test), as the new-station dialog asks it; it narrows the head parts offered
/// (`Session::station_parts`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StationKind {
    /// A PLC: a CPU.
    Cpu,
    /// A remote I/O station: an interface module on a bus.
    Remote,
    /// A valve terminal with a bus node.
    ValveTerminal,
    /// A compact device with channels of its own.
    Compact,
}

impl StationKind {
    /// Every kind, in the dialog's order.
    pub const ALL: [StationKind; 4] = [
        StationKind::Cpu,
        StationKind::Remote,
        StationKind::ValveTerminal,
        StationKind::Compact,
    ];

    /// The word of `plc.new_station`'s and `plc.rack_set`'s `kind`.
    pub fn word(self) -> &'static str {
        match self {
            StationKind::Cpu => "cpu",
            StationKind::Remote => "remote",
            StationKind::ValveTerminal => "valve_terminal",
            StationKind::Compact => "compact",
        }
    }

    /// The kind of a word.
    pub fn from_word(word: &str) -> Option<StationKind> {
        StationKind::ALL.into_iter().find(|k| k.word() == word)
    }

    /// The kind in words, as the dialog offers it.
    pub fn text_key(self) -> Key<'static> {
        match self {
            StationKind::Cpu => key!("rack-kind-cpu"),
            StationKind::Remote => key!("rack-kind-remote"),
            StationKind::ValveTerminal => key!("rack-kind-valve-terminal"),
            StationKind::Compact => key!("rack-kind-compact"),
        }
    }

    /// The kind in full words, the hover text of its short choice.
    pub fn tip_key(self) -> Key<'static> {
        match self {
            StationKind::Cpu => key!("rack-kind-cpu-tip"),
            StationKind::Remote => key!("rack-kind-remote-tip"),
            StationKind::ValveTerminal => key!("rack-kind-valve-terminal-tip"),
            StationKind::Compact => key!("rack-kind-compact-tip"),
        }
    }
}

/// A station not made yet: the new-station dialog's choices (F18 of the rc.1 test).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StationDraft {
    /// What it is.
    pub kind: StationKind,
    /// Its head part (a project or library part copy); `None` before one is chosen.
    pub head: Option<Uuid>,
    /// The PLC (configuration project) it belongs to; `None`: a new one.
    pub config: Option<Uuid>,
    /// Its module parts in slot order.
    pub modules: Vec<Uuid>,
}

/// The command a preview was made of, printed by its name only (its parameters hold the whole file read).
#[derive(Clone, PartialEq, Eq)]
pub struct PreviewedCommand(pub std::sync::Arc<ew_api::AnyCommand>);

impl std::fmt::Debug for PreviewedCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.name)
    }
}

/// The tabs of the PLC import's preview (doc 10 §6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportTab {
    /// The differing fields the merge applies, keeps, rejects or leaves pending.
    Changed,
    /// The fields both sides changed: each takes the file's value or keeps the project's.
    Conflicts,
    /// The objects of the last exchange the file no longer holds.
    Removed,
    /// The file's objects without a counterpart: a creatable one can be created unplaced.
    Parking,
}

impl ImportTab {
    /// The tabs in order.
    pub const ALL: [ImportTab; 4] = [
        ImportTab::Changed,
        ImportTab::Conflicts,
        ImportTab::Removed,
        ImportTab::Parking,
    ];

    /// The tab's word (`plc.import_tab`'s `tab`).
    pub fn word(self) -> &'static str {
        match self {
            ImportTab::Changed => "changed",
            ImportTab::Conflicts => "conflicts",
            ImportTab::Removed => "removed",
            ImportTab::Parking => "parking",
        }
    }
}

/// One differing field of the import's preview.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportChange {
    /// Its key in the resolutions.
    pub id: String,
    /// The object and the field in words (`channel Q0.1 · symbol`).
    pub text: String,
    /// The project's value, and the file's.
    pub ours: String,
    /// The file's value.
    pub theirs: String,
    /// What the merge makes of it: `apply`, `rejected`, `kept`, `pending` or `conflict`.
    pub state: &'static str,
    /// The side Apply takes without a choice: `ours` or `theirs`.
    pub default: &'static str,
    /// Why the file's value cannot be taken, in words; `None` when it can.
    pub refused: Option<String>,
}

/// One parked object of the import's preview.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportParked {
    /// Its ID in the file: its key in the resolutions.
    pub id: String,
    /// Its kind and name in words.
    pub text: String,
    /// Whether several objects of the project match it (else none does).
    pub ambiguous: bool,
    /// Whether "create unplaced" applies.
    pub creatable: bool,
}

/// The PLC import's preview (`Dialog::PlcImport`): the file, its tab on show, the plan in words, and the choices.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlcImportCard {
    /// The file's name.
    pub file: String,
    /// The file's path, which the preview is made of again when "the file is complete" changes.
    pub path: String,
    /// Whether the file is complete (doc 10 §4.4): an object of the last exchange it lacks is removed.
    pub complete: bool,
    /// Why Apply would be refused (its prevent rules, tried on the preview), in words; `None` when it commits.
    pub refusal: Option<String>,
    /// The `plc.apply_exchange` the preview was made of, with every option it was made with (`complete`); Apply
    /// commits it under the choices, and the file is neither read nor merged again here.
    pub apply: PreviewedCommand,
    /// The tab on show.
    pub tab: ImportTab,
    /// The differing fields (not conflicts).
    pub changes: Vec<ImportChange>,
    /// The conflicts.
    pub conflicts: Vec<ImportChange>,
    /// The removed objects in words.
    pub removed: Vec<String>,
    /// The parked objects.
    pub parking: Vec<ImportParked>,
    /// The choices: a change's key or a parked object's ID to `ours`, `theirs` or `create`.
    pub choices: std::collections::BTreeMap<String, String>,
}

impl PlcImportCard {
    /// The rows of tab `tab`.
    pub fn count(&self, tab: ImportTab) -> usize {
        match tab {
            ImportTab::Changed => self.changes.len(),
            ImportTab::Conflicts => self.conflicts.len(),
            ImportTab::Removed => self.removed.len(),
            ImportTab::Parking => self.parking.len(),
        }
    }
}

/// The bytes of the exchange file or instrument list at `path` (named `file`), read through the importers' input limit:
/// a file larger than the limit, or one that cannot be read, is refused in words.
fn read_exchange_file(path: &std::path::Path, file: &str) -> Result<Vec<u8>, Outcome> {
    match super::drawing_import::read_limited(path) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => {
            let mib = (ew_api::plc_exchange::max_file_bytes() >> 20).to_string();
            Err(failed(key!("error-plc-import-too-large"), &[file, &mib]))
        }
        Err(e) => Err(failed(
            key!("error-plc-import-file"),
            &[file, &crate::jobs::io_cause(&e)],
        )),
    }
}

/// The PLC import's card of the preview `import` of the file `file`: its rows in the window's words.
fn plc_import_card(
    file: String,
    import: ew_api::plc_exchange::PlcImport,
    refusal: Option<&(ew_api::CmdError, Vec<String>)>,
) -> PlcImportCard {
    let words = |v: &Option<String>| v.clone().unwrap_or_default();
    // Apply's refusal in the window's words; the rows of the changes it concerns say so.
    let why = refusal.map(|(e, _)| match super::tools::command_failed(e, &Value::Null) {
        Outcome::Failed { message } => message,
        _ => e.to_string(),
    });
    let named: &[String] = refusal.map_or(&[], |(_, changes)| changes.as_slice());
    let (conflicts, changes): (Vec<_>, Vec<_>) = import
        .plan
        .changes
        .iter()
        .map(|c| {
            let field = plc_field_text(&c.object, &c.field);
            let refused = (!c.acceptable).then(|| {
                // A field of a group the file cannot take is a comment of the other form (the command's reason).
                let key = if c.group.is_some() {
                    key!("refusal-plc-exchange-language")
                } else {
                    key!("refusal-plc-exchange-not-acceptable")
                };
                strings::sentence_start(&strings::format(key, &[&field, &c.subject])).into_owned()
            });
            let refused = refused.or_else(|| why.clone().filter(|_| named.contains(&c.id)));
            ImportChange {
                id: c.id.clone(),
                text: strings::format(key!("plc-import-row"), &[&c.subject, &field]),
                ours: words(&c.ours),
                theirs: words(&c.theirs),
                state: c.state.word(),
                default: match c.default {
                    ew_api::plc_exchange::Choice::Ours => "ours",
                    ew_api::plc_exchange::Choice::Theirs => "theirs",
                },
                refused,
            }
        })
        .partition(|c| c.state == "conflict");
    let object = |o: &ew_api::plc_exchange::ExchangeObject, name: &str| {
        strings::format(key!("plc-import-object"), &[&plc_object_text(o), name])
    };
    PlcImportCard {
        file,
        path: String::new(),
        complete: false,
        refusal: why,
        apply: PreviewedCommand(std::sync::Arc::new(import.apply)),
        tab: ImportTab::Changed,
        changes,
        conflicts,
        removed: import
            .plan
            .removed
            .iter()
            .map(|(o, _, name)| object(o, name))
            .collect(),
        parking: import
            .plan
            .parking
            .iter()
            .map(|p| ImportParked {
                id: p.id.clone(),
                text: object(&p.object, &p.name),
                ambiguous: p.ambiguous,
                creatable: p.creatable,
            })
            .collect(),
        choices: std::collections::BTreeMap::new(),
    }
}

/// The kind of an exchanged object in the window's words: its `plc-object-<kind>` text, `plc-object-other` for a kind
/// of a newer format.
pub fn plc_object_text(object: &ew_api::plc_exchange::ExchangeObject) -> String {
    strings::lookup(&format!("plc-object-{}", object.name()))
        .filter(|_| !object.name().is_empty())
        .unwrap_or_else(|| strings::text(key!("plc-object-other")))
        .to_owned()
}

/// A field of an exchanged object in the window's words: the object's own `plc-field-<kind>-<field>` text (a tag's
/// `name` is its symbol), else `plc-field-<field>`, a comment in a language with the language; a field this version
/// does not know as it is written.
pub fn plc_field_text(object: &ew_api::plc_exchange::ExchangeObject, field: &str) -> String {
    if let Some(lang) = field.strip_prefix("comment.") {
        return strings::format(key!("plc-field-comment-in"), &[lang]);
    }
    let key = field.replace(['.', '_'], "-");
    strings::lookup(&format!("plc-field-{}-{key}", object.name()))
        .or_else(|| strings::lookup(&format!("plc-field-{key}")))
        .map_or_else(|| field.to_owned(), str::to_owned)
}

/// The PLC export formats PLC exchange… offers: (word, the format, its name); product names, the same in every
/// language.
pub const PLC_FORMATS: [(&str, &str); 4] = [
    ("aml-1.4.0", "AutomationML AR APC 1.4.0"),
    ("aml-1.2.0", "AutomationML AR APC 1.2.0"),
    ("tia-xlsx", "TIA Portal PLC tags (XLSX)"),
    ("plcopen", "PLCopen XML (CODESYS)"),
];

/// The exchange format of a word of [`PLC_FORMATS`].
pub(super) fn plc_format(word: &str) -> Option<ew_api::plc_exchange::PlcFormat> {
    use ew_api::plc_exchange::{ApcVersion, PlcFormat};
    match word {
        "aml-1.4.0" => Some(PlcFormat::Aml(ApcVersion::V1_4_0)),
        "aml-1.2.0" => Some(PlcFormat::Aml(ApcVersion::V1_2_0)),
        "tia-xlsx" => Some(PlcFormat::TiaXlsx),
        "plcopen" => Some(PlcFormat::Plcopen),
        _ => None,
    }
}

impl ShellModel {
    /// `plc.import {path?, complete?}` (doc 10 §4.4): reads an AML file or a TIA tag workbook and opens its preview
    /// against the project's configuration project; nothing changes until Apply. In the window a missing path asks for
    /// the file.
    pub(crate) fn act_plc_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => {
                return self.need_path("plc.import", crate::shell::Prompt::OpenPlcExchange, "path", p);
            }
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = or_return!(read_exchange_file(&path, &file));
        let complete = p.get("complete").and_then(Value::as_bool).unwrap_or(false);
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let options = ew_api::plc_exchange::PlcOptions {
            complete,
            ..Default::default()
        };
        let import = match session.plc_import(&bytes, &options) {
            Ok(i) => i,
            Err(e) => return failed(key!("error-plc-import"), &[&e.to_string()]),
        };
        // Apply tried on the preview (its prevent rules run), so a start the file gives that overlaps another is
        // refused here already, its row marked, rather than at Apply.
        let refusal = self
            .project
            .as_mut()
            .and_then(|project| project.session.plc_import_refusal(&import));
        let mut card = plc_import_card(file, import, refusal.as_ref());
        card.path = path.to_string_lossy().into_owned();
        card.complete = complete;
        if card.changes.is_empty() && !card.conflicts.is_empty() {
            card.tab = ImportTab::Conflicts;
        }
        self.open_dialog(crate::shell::Dialog::PlcImport(Box::new(card)))
    }

    /// `plc.import_tab {tab}`: the tab the import's preview shows.
    pub(crate) fn act_plc_import_tab(&mut self, p: &Map<String, Value>) -> Outcome {
        let tab = match string_param(p, "tab") {
            Ok(Some(w)) => match ImportTab::ALL.into_iter().find(|t| t.word() == w) {
                Some(t) => t,
                None => return failed(key!("error-bad-param"), &["tab"]),
            },
            Ok(None) => return failed(key!("error-missing-param"), &["tab"]),
            Err(e) => return e,
        };
        match &mut self.dialog {
            Some(crate::shell::Dialog::PlcImport(card)) => {
                card.tab = tab;
                Outcome::Done
            }
            _ => Outcome::refused(key!("reason-plc-import-closed")),
        }
    }

    /// `plc.import_choose {id, choice?}`: a conflict's or a parked object's choice (`ours`, `theirs`, `create`; absent:
    /// the default again).
    pub(crate) fn act_plc_import_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        let id = match string_param(p, "id") {
            Ok(Some(i)) => i.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["id"]),
            Err(e) => return e,
        };
        let choice = match string_param(p, "choice") {
            Ok(Some(c)) if ["ours", "theirs", "create"].contains(&c) => Some(c.to_owned()),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["choice"]),
            Ok(None) => None,
            Err(e) => return e,
        };
        match &mut self.dialog {
            Some(crate::shell::Dialog::PlcImport(card)) => {
                match choice {
                    Some(c) => card.choices.insert(id, c),
                    None => card.choices.remove(&id),
                };
                Outcome::Done
            }
            _ => Outcome::refused(key!("reason-plc-import-closed")),
        }
    }

    /// `plc.import_apply`: the import's `plc.apply_exchange` under its choices, one undo step; the preview closes and a
    /// toast says what changed.
    pub(crate) fn act_plc_import_apply(&mut self) -> Outcome {
        let Some(crate::shell::Dialog::PlcImport(card)) = self.dialog.clone() else {
            return Outcome::refused(key!("reason-plc-import-closed"));
        };
        let mut apply = (*card.apply.0).clone();
        apply.params["resolutions"] = json!(card.choices);
        let out = self.run_commit(&apply);
        if out == Outcome::Done {
            self.dialog = None;
            self.notify(strings::format(key!("toast-plc-import"), &[&card.file]), true);
        }
        out
    }

    /// `plc.export {format?, stations?, date?, path?, replace?}` (doc 10 §4.3, §4.5, §4.6): the configuration project
    /// exported in the format (`aml-1.4.0`, `aml-1.2.0`, `tia-xlsx`, `plcopen`) for the stations (tags, comma
    /// separated; empty: every station) at the document date (`YYYY-MM-DD`; empty: the model's), written whole or
    /// not at all, then recorded with `plc.record_exchange`. In the window a missing format opens the form and a missing
    /// path asks for the file.
    pub(crate) fn act_plc_export_file(&mut self, p: &Map<String, Value>, format: &str) -> Outcome {
        let Some(fmt) = plc_format(format) else {
            return failed(key!("error-bad-param"), &["format"]);
        };
        let stations: Vec<String> = string_param(p, "stations")
            .ok()
            .flatten()
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .map(|s| if s.contains(':') { s } else { format!("tag:{s}") })
            .collect();
        let date = match string_param(p, "date")
            .ok()
            .flatten()
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            None => None,
            Some(d) => match d.parse::<ew_api::plc_exchange::Date>() {
                Ok(d) => Some(d),
                Err(_) => return failed(key!("error-plc-export-date"), &[d]),
            },
        };
        // The form's values are checked before the file is asked for, so a value refused keeps the form.
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => super::jobs::absolute_target(std::path::PathBuf::from(path)),
            Ok(None) => {
                let mut rest = p.clone();
                rest.insert("replace".to_owned(), Value::Bool(true));
                self.dialog = None;
                let prompt = crate::shell::Prompt::SavePlcExchange(fmt.extension());
                return self.need_path("plc.export", prompt, "path", &rest);
            }
        };
        // An existing file is replaced only on request (the save dialog asks itself, and says so).
        if path.exists() && !or_return!(super::jobs::bool_param(p, "replace", false)) {
            return Outcome::refused(key!("reason-file-exists"));
        }
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let options = ew_api::plc_exchange::PlcOptions {
            stations,
            date,
            ..Default::default()
        };
        let export = match session.plc_export(fmt, &options) {
            Ok(e) => e,
            Err(e) => return failed(key!("error-plc-export"), &[&e.to_string()]),
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Err(e) = super::external::write_replacing(&path, &export.bytes) {
            return failed(key!("error-plc-export"), &[&e.to_string()]);
        }
        let out = export
            .record
            .as_ref()
            .map_or(Outcome::Done, |record| self.run_commit(record));
        if out == Outcome::Done {
            let [stations, modules, channels, tags] = export.counts;
            let [stations, modules, channels, tags] = [
                strings::format_count(crate::count_key!("plc-count-stations"), stations),
                strings::format_count(crate::count_key!("plc-count-modules"), modules),
                strings::format_count(crate::count_key!("plc-count-channels"), channels),
                strings::format_count(crate::count_key!("plc-count-symbols"), tags),
            ];
            let mut text = strings::format(
                key!("toast-plc-export"),
                &[&name, &stations, &modules, &channels, &tags],
            );
            for w in &export.warnings {
                text = format!("{} {w}.", strings::without_stop(&text));
            }
            self.notify(text, false);
        }
        out
    }
}
