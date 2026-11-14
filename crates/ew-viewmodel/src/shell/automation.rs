// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's entries of the PLC, bus and fluid commands (WP-S2.13): each handler builds one declared command
//! from the row or selection it is aimed at and commits it as one undo step, saying what it did in words.
//!
//! - **Attach cards without a CPU** (FT-1672): a CPU row of the PLC navigator attaches the cards that wait for a CPU
//!   (`Session::plc_cards_without_cpu`) with one `assembly.attach`, after the station's last child position.
//! - **The rack and slot editor** (FR-PLC-028): a station row's Rack and slots… opens `Dialog::Rack`, which shows
//!   `Session::rack_view` each frame. Add module puts the chosen module part into a free slot with `plc.add_module`
//!   (a library part embedded in the same step); a module's slot field moves it with `fields.set plc_module.slot`.
//!   EW-PLC-011's prevent half refuses a slot the part does not allow; the refusal stays under the slots.

use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::strings;

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

    /// `plc.attach_cards {station}` (FT-1672): the cards that wait for a CPU become children of `station`, after its
    /// last child position, with one `assembly.attach`; a toast names them.
    pub(crate) fn act_plc_attach_cards(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.record_param(p, "station"));
        let Some(project) = self.project.as_ref() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let cards = project.session.plc_cards_without_cpu();
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

    /// The rack and slot editor's view of `station` (doc 10 §6), as the window draws it.
    pub fn rack_view(&self, station: Uuid) -> Option<ew_api::output::RackView> {
        self.project_session()?.rack_view(station)
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
        let racked = self.rack_view(station).is_some_and(|v| v.racked);
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
