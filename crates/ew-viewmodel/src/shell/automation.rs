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
//! - **PLC exchange** (doc 10 §4, §6): Export… is a form of format and version, stations and document date that writes
//!   the file whole or not at all (`Session::plc_export`) and records it (`plc.record_exchange`); Import… reads an AML
//!   file or a TIA tag workbook into `Dialog::PlcImport`, the preview by tab (changed, conflicts, removed, parking)
//!   with the choices of its conflicts and parked objects, and Apply commits `plc.apply_exchange` under them.

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

/// The bytes of a file a dialog holds, printed by their length only.
#[derive(Clone, PartialEq, Eq)]
pub struct FileBytes(pub std::sync::Arc<Vec<u8>>);

impl std::fmt::Debug for FileBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} bytes", self.0.len())
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
    /// Its bytes (read once; Apply merges them again with the choices).
    pub bytes: FileBytes,
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
        let too_big =
            std::fs::metadata(&path).is_ok_and(|m| m.len() > ew_api::plc_exchange::max_file_bytes());
        let bytes = match std::fs::read(&path) {
            Ok(_) if too_big => return failed(key!("error-plc-import-file"), &[&file, "too large"]),
            Ok(b) => b,
            Err(e) => return failed(key!("error-plc-import-file"), &[&file, &e.kind().to_string()]),
        };
        let complete = p.get("complete").and_then(Value::as_bool).unwrap_or(false);
        let card = PlcImportCard {
            file,
            bytes: FileBytes(std::sync::Arc::new(bytes)),
            tab: ImportTab::Changed,
            changes: Vec::new(),
            conflicts: Vec::new(),
            removed: Vec::new(),
            parking: Vec::new(),
            choices: std::collections::BTreeMap::new(),
        };
        match self.plc_import_plan(card, complete) {
            Ok(mut card) => {
                if card.changes.is_empty() && !card.conflicts.is_empty() {
                    card.tab = ImportTab::Conflicts;
                }
                self.open_dialog(crate::shell::Dialog::PlcImport(Box::new(card)))
            }
            Err(out) => out,
        }
    }

    /// The card with the plan of its file under its choices, in words.
    fn plc_import_plan(&self, mut card: PlcImportCard, complete: bool) -> Result<PlcImportCard, Outcome> {
        let Some(session) = self.project_session() else {
            return Err(Outcome::refused(key!("reason-no-project")));
        };
        let options = ew_api::plc_exchange::PlcOptions {
            complete,
            resolutions: card.choices.clone(),
            ..Default::default()
        };
        let import = session
            .plc_import(&card.bytes.0, &options)
            .map_err(|e| Outcome::Failed {
                message: strings::format(key!("error-plc-import"), &[&e.to_string()]),
            })?;
        let words = |v: &Option<String>| v.clone().unwrap_or_default();
        let (conflicts, changes): (Vec<_>, Vec<_>) = import
            .plan
            .changes
            .iter()
            .map(|c| ImportChange {
                id: c.id.clone(),
                text: format!(
                    "{} · {}",
                    c.subject,
                    ew_api::plc_exchange::field_words(&c.object, &c.field)
                ),
                ours: words(&c.ours),
                theirs: words(&c.theirs),
                state: c.state.word(),
            })
            .partition(|c| c.state == "conflict");
        card.changes = changes;
        card.conflicts = conflicts;
        card.removed = import
            .plan
            .removed
            .iter()
            .map(|(o, _, name)| format!("{} {name}", ew_api::plc_exchange::object_words(o)))
            .collect();
        card.parking = import
            .plan
            .parking
            .iter()
            .map(|p| ImportParked {
                id: p.id.clone(),
                text: format!("{} {}", ew_api::plc_exchange::object_words(&p.object), p.name),
                ambiguous: p.ambiguous,
                creatable: p.creatable,
            })
            .collect();
        Ok(card)
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
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let options = ew_api::plc_exchange::PlcOptions {
            resolutions: card.choices.clone(),
            ..Default::default()
        };
        let import = match session.plc_import(&card.bytes.0, &options) {
            Ok(i) => i,
            Err(e) => return failed(key!("error-plc-import"), &[&e.to_string()]),
        };
        let out = self.run_commit(&import.apply);
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
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => super::jobs::absolute_target(std::path::PathBuf::from(path)),
            Ok(None) => {
                let mut rest = p.clone();
                rest.insert("replace".to_owned(), Value::Bool(true));
                self.dialog = None;
                return self.need_path("plc.export", crate::shell::Prompt::SavePlcExchange, "path", &rest);
            }
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
        let out = self.run_commit(&export.record);
        if out == Outcome::Done {
            let [stations, modules, channels, tags] = export.counts.map(|n| n.to_string());
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
