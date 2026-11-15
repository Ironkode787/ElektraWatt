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

use super::{Outcome, ShellModel, failed, string_param};
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
                Some(d) if d.aspects.plc_module.as_ref().is_some_and(|m| !m.integrated) => {
                    cards.push((card, tag));
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

    /// The module part the open rack and slot editor adds: the one chosen, else the first offered.
    pub fn rack_part(&self) -> Option<ew_api::output::ModulePart> {
        let chosen = match self.dialog() {
            Some(crate::shell::Dialog::Rack { part, .. }) => *part,
            _ => None,
        };
        let parts = self.module_parts();
        chosen
            .and_then(|c| parts.iter().find(|p| p.copy == c).cloned())
            .or_else(|| parts.into_iter().next())
    }

    /// `plc.rack_editor {station}` (FR-PLC-028): opens the rack and slot editor of a station.
    pub(crate) fn act_plc_rack_editor(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.record_param(p, "station"));
        if self.rack_view(station).is_none() {
            return failed(key!("error-bad-param"), &["station"]);
        }
        self.open_dialog(crate::shell::Dialog::Rack {
            station,
            part: None,
            error: None,
        })
    }

    /// `plc.rack_part {part}`: the module part the open editor's Add module puts into a slot.
    pub(crate) fn act_plc_rack_part(&mut self, p: &Map<String, Value>) -> Outcome {
        let part = or_return!(self.record_param(p, "part"));
        if !self.module_parts().iter().any(|m| m.copy == part) {
            return failed(key!("error-bad-param"), &["part"]);
        }
        if let Some(crate::shell::Dialog::Rack { part: chosen, .. }) = &mut self.dialog {
            *chosen = Some(part);
        }
        Outcome::Done
    }

    /// The station of the open rack and slot editor.
    fn rack_station(&self) -> Result<Uuid, Outcome> {
        match self.dialog() {
            Some(crate::shell::Dialog::Rack { station, .. }) => Ok(*station),
            _ => Err(Outcome::refused(key!("reason-rack-closed"))),
        }
    }

    /// Commits a change of the open rack and slot editor: a refusal stays under its slots, a change clears it.
    fn rack_commit(&mut self, name: &str, params: Value) -> Outcome {
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
        if let Some(crate::shell::Dialog::Rack { error: e, .. }) = &mut self.dialog {
            *e = error;
        }
        out
    }

    /// `plc.rack_add {slot, rack?}`: a module of the editor's part into a free slot of its station, with one
    /// `plc.add_module` (a library part embedded with it): as `rack.slot` in a station whose modules have rack
    /// positions, else positional after the station's modules.
    pub(crate) fn act_plc_rack_add(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.rack_station());
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
        // Only a free slot the editor offers takes a module: a taken one names its module, in words under the slots.
        let view = self.rack_view(station);
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
            if let Some(crate::shell::Dialog::Rack { error, .. }) = &mut self.dialog {
                *error = Some(why.clone());
            }
            return Outcome::Failed { message: why };
        }
        let Some(part) = self.rack_part() else {
            return Outcome::refused(key!("reason-rack-no-part"));
        };
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
                let why = strings::format(key!("rack-bad-slot"), &[&text]);
                if let Some(crate::shell::Dialog::Rack { error, .. }) = &mut self.dialog {
                    *error = Some(why.clone());
                }
                return Outcome::Failed { message: why };
            }
        };
        self.rack_commit(
            "fields.set",
            json!({"kind": "device", "targets": [module.to_string()], "path": "plc_module.slot",
                "value": {"rack": {"rack": rack, "slot": slot}}}),
        )
    }

    /// The describe lines of the rack and slot editor's slots and findings.
    pub(crate) fn rack_lines(&self, station: Uuid) -> Vec<String> {
        let Some(v) = self.rack_view(station) else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        for (label, code) in &v.order_codes {
            lines.push(format!("  order_code \"{label}\" \"{code}\""));
        }
        for r in &v.rows {
            let mut line = format!("  slot {}", rack_slot_text(r));
            if r.module.is_some() {
                line.push_str(&format!(" \"{}\" \"{}\"", r.tag, r.part));
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

/// A valve terminal module's positions in words: `1 -201U3-Q1 · 2 blank · 3 empty`.
pub fn rack_positions_text(r: &ew_api::output::RackRow) -> String {
    r.positions
        .iter()
        .enumerate()
        .map(|(i, (valve, blank))| {
            let what = match (valve, blank) {
                (Some(tag), _) => tag.clone(),
                (None, true) => strings::text(key!("rack-blank")).to_owned(),
                (None, false) => strings::text(key!("rack-empty")).to_owned(),
            };
            format!("{} {what}", i + 1)
        })
        .collect::<Vec<_>>()
        .join(" · ")
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
