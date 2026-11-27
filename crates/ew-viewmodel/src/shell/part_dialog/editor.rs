// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part editor of the part dialog: the copy it edits, Edit part, New part, a bus port's row, a field, the own-part removal and Save.

use super::{
    BusPart, PLC_PRESETS, PartEditor, card_fields, draft_refusal, matches_text, plc_preset, plug_description,
    preset_bus,
};
use crate::key;
use crate::properties::{CardOrigin, PartCard};
use crate::shell::{Dialog, Outcome, Prompt, ShellModel, failed, flag, string_param};
use crate::strings;
use ew_api::output::{NavKind, NavRole, Uuid};
use ew_api::{OfferView, PartDraftError, PartOfferQuery, PartPick};
use serde_json::{Map, Value};

impl ShellModel {
    /// The copy `part.edit` edits (see the module documentation).
    pub(in crate::shell) fn part_edit_copy(&self, p: &Map<String, Value>) -> Result<Option<Uuid>, Outcome> {
        let Some(project) = &self.project else {
            return Ok(None);
        };
        if let Some(text) = string_param(p, "part")? {
            if let Some(id) = text.trim().strip_prefix("id:") {
                return Ok(id.parse().ok());
            }
            let held = project.session.part_offers(&PartOfferQuery {
                view: OfferView::Project,
                include_inactive: true,
                lang: Some(strings::rule_language().to_owned()),
                ..PartOfferQuery::default()
            });
            let hits: Vec<Uuid> = held
                .iter()
                .filter(|o| matches_text(text, &o.part_number, &o.manufacturer, &o.description))
                .filter_map(|o| match o.pick {
                    PartPick::Held(c) => Some(c),
                    PartPick::Library { .. } => None,
                })
                .collect();
            return match hits.as_slice() {
                [one] => Ok(Some(*one)),
                [] => Err(failed(key!("error-part-unknown"), &[text])),
                _ => Err(failed(key!("error-accessory-part-ambiguous"), &[text])),
            };
        }
        let lib_row = || {
            self.nav_cursor(NavKind::Libraries).and_then(|k| {
                let nav = self.generic(NavKind::Libraries)?;
                let (_, n) = nav.node_of(&k)?;
                (n.role == NavRole::LibCopy)
                    .then(|| n.records.first().copied())
                    .flatten()
            })
        };
        if self.focused_navigator() == Some(NavKind::Libraries)
            && let Some(c) = lib_row()
        {
            return Ok(Some(c));
        }
        let grid = self.props.sheet().and_then(|sheet| {
            let section = sheet.sections.iter().find(|s| s.kind == "device")?;
            match &section.rows.iter().find(|r| r.ty.is_part_uses())?.value {
                ew_api::output::RowValue::One { json, .. } => json
                    .get(0)?
                    .get("part")?
                    .get("copy")?
                    .as_str()?
                    .parse::<Uuid>()
                    .ok(),
                _ => None,
            }
        });
        Ok(grid.or_else(lib_row))
    }

    /// `part.edit {part?}`: opens the part editor with the draft of a held part copy.
    pub(crate) fn act_part_edit(&mut self, p: &Map<String, Value>) -> Outcome {
        let copy = match self.part_edit_copy(p) {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Outcome::refused(key!("reason-part-edit-target"));
            }
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Ok(draft) = project.session.part_draft(copy) else {
            return Outcome::refused(key!("reason-part-edit-target"));
        };
        let name = format!("{} ({})", draft.text("part_number"), draft.text("manufacturer"));
        let manufacturers = project.session.part_manufacturers();
        let out = self.open_dialog(Dialog::PartEdit(Box::new(PartEditor {
            section: "identity",
            draft,
            name,
            manufacturers,
            error: None,
            back: None,
            cable_type: false,
            cable_form: None,
            bus_part: None,
        })));
        // At a section of its own (a quick fix's Edit part… at Front view and holes).
        match p.get("section") {
            Some(_) if out == Outcome::Done => self.act_part_section(p),
            _ => out,
        }
    }

    /// `part.new {symbol?}` (Edit ▸ New part…, and New part… in the part dialog and the device browser): opens the part
    /// editor on a new part's draft; with `symbol` (the symbol editor's saved symbol, its library address:
    /// Make a part for this symbol…), a part drawn with that symbol. From the part dialog, Save returns to it on the new
    /// part. With `plc` (the rack dialog's New part…): the draft starts as what the dialog was asked for, `cpu`,
    /// `remote`, `valve_terminal` or `compact` a station part with that kind's defaults ([`plc_preset`]), `module` a
    /// module part; Save returns to the rack dialog with the new part in its catalogue, its head when the empty head
    /// was the place chosen. A station's preset bus row goes when the role is made Module ([`ShellModel::act_part_field`]).
    pub(crate) fn act_part_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let symbol = or_return!(string_param(p, "symbol")).map(str::to_owned);
        let plc = match or_return!(string_param(p, "plc")) {
            Some(w) if PLC_PRESETS.contains(&w) => Some(w.to_owned()),
            Some(_) => return failed(key!("error-bad-param"), &["plc"]),
            None => None,
        };
        let from = match or_return!(string_param(p, "from")).map(str::parse::<Uuid>) {
            None => None,
            Some(Ok(c)) => Some(c),
            Some(Err(_)) => return failed(key!("error-bad-param"), &["from"]),
        };
        let project = or_return!(crate::shell::gated(&self.project));
        let made = match (&symbol, from) {
            (Some(address), _) => project.session.new_part_draft_for_symbol(address),
            (None, Some(copy)) => project.session.new_part_draft_from(copy),
            (None, None) => project.session.new_part_draft(),
        };
        let mut draft = match made {
            Ok(d) => d,
            Err(e) => return draft_refusal(e),
        };
        if let Some(word) = &plc {
            plc_preset(&project.session, &mut draft, word);
        }
        // New cable type…: a cable part, sold by the metre, in the small form.
        let cable_type = flag(p, "cable");
        if cable_type {
            for (field, value) in [("kind", "cable"), ("unit", "m")] {
                let _ = project.session.set_draft_field(&mut draft, field, value);
            }
        }
        // New bus cable… and New plug… of a bus line's Cable form: a cable made for the bus, its cores as the bus
        // has them, or a plug with a passive interface of the bus; Save returns to the form's field `field`.
        let bus_part = match or_return!(string_param(p, "bus")) {
            None => None,
            Some(bus) => {
                let plug = flag(p, "plug");
                let field = match or_return!(string_param(p, "field")) {
                    Some("plug_a") => "plug_a",
                    Some("plug_b") => "plug_b",
                    _ if plug => "plug_a",
                    _ => "part",
                };
                let set = |draft: &mut ew_api::PartDraft, f: &str, v: &str| {
                    let _ = project.session.set_draft_field(draft, f, v);
                };
                if plug {
                    set(&mut draft, "kind", "accessory");
                    draft.add_bus_row();
                    for (f, v) in [
                        ("bus.0.bus", bus),
                        ("bus.0.role", "passive"),
                        ("bus.0.termination", ""),
                    ] {
                        set(&mut draft, f, v);
                    }
                } else {
                    for (f, v) in [("kind", "cable"), ("unit", "m"), ("cable.bus", bus)] {
                        set(&mut draft, f, v);
                    }
                }
                let made = BusPart {
                    plug,
                    field,
                    angled: false,
                    socket: false,
                };
                if plug {
                    let words = plug_description(&draft, made);
                    set(&mut draft, "description", &words);
                }
                Some(made)
            }
        };
        let cable_type = cable_type || bus_part.is_some_and(|b| !b.plug);
        let manufacturers = project.session.part_manufacturers();
        // The device browser gives way to the editor; it lists the new part when it opens again. From the symbol
        // editor, its view is left with its draft kept (as Close editor does): the part is the project's.
        self.tools.picker = None;
        if symbol.is_some()
            && let Some(editor) = &mut self.symbol_editor
        {
            editor.shown = false;
        }
        // From the Cable form (its New cable type…), Save returns to it with the new type chosen.
        let (back, cable_form) = match self.dialog.take() {
            Some(Dialog::PartChoose(c)) => (Some(c), None),
            Some(Dialog::Form(f))
                if (cable_type && crate::shell::cables::chooses_cable_type(&f))
                    || (bus_part.is_some() && f.action == crate::shell::cables::BUS_CABLE) =>
            {
                (None, Some(Box::new(f)))
            }
            other => {
                self.dialog = other;
                (None, None)
            }
        };
        let out = self.open_dialog(Dialog::PartEdit(Box::new(PartEditor {
            section: "identity",
            draft,
            name: String::new(),
            manufacturers,
            error: None,
            back: None,
            cable_type,
            cable_form: None,
            bus_part,
        })));
        if let Some(Dialog::PartEdit(e)) = &mut self.dialog {
            e.back = back;
            e.cable_form = cable_form;
        }
        out
    }

    /// `part.bus_row {remove?}`: adds a bus interface row to the new part, or removes row `remove` (1-based).
    pub(crate) fn act_part_bus_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let remove = match p.get("remove") {
            None => None,
            Some(v) => match v
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .filter(|i| *i >= 1)
            {
                Some(i) => Some(i - 1),
                None => return failed(key!("error-bad-param"), &["remove"]),
            },
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        if !editor.draft.new && !editor.draft.own {
            return failed(key!("error-part-field"), &["bus"]);
        }
        editor.error = None;
        match remove {
            None => editor.draft.add_bus_row(),
            Some(i) => {
                if !editor.draft.remove_bus_row(i) {
                    return failed(key!("error-bad-param"), &["remove"]);
                }
            }
        }
        Outcome::Done
    }

    /// `part.delete` (Delete part… of the part editor): deletes the edited part, a part of the project's own that no
    /// device uses (`part.delete`, one undo step); the editor closes and a toast names the part. Refused in words for a
    /// library's part and for a part a device uses.
    pub(crate) fn act_part_delete(&mut self) -> Outcome {
        let Some(Dialog::PartEdit(editor)) = &self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        let (copy, number) = (editor.draft.copy, editor.draft.text("part_number").to_owned());
        let cmd = ew_api::AnyCommand {
            name: "part.delete".into(),
            params: serde_json::json!({ "copy": copy.to_string() }),
        };
        let out = self.run_commit(&cmd);
        if matches!(out, Outcome::Done) {
            self.dialog = None;
            self.notify(strings::format(key!("part-deleted"), &[&number]), false);
        }
        out
    }

    /// `part.section {section}`: shows the editor's section `section` (one of those the draft lists).
    pub(crate) fn act_part_section(&mut self, p: &Map<String, Value>) -> Outcome {
        let word = match string_param(p, "section") {
            Ok(Some(w)) => w.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["section"]),
            Err(e) => return e,
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        match editor.draft.sections().into_iter().find(|s| s.word == word) {
            Some(s) => {
                editor.section = s.word;
                Outcome::Done
            }
            None => failed(key!("error-bad-param"), &["section"]),
        }
    }

    /// `part.table_row {table, remove?}`: adds a row to the editor's table `table` (`accessories`, `variants`), or
    /// removes row `remove` (1-based).
    pub(crate) fn act_part_table_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let table = match string_param(p, "table") {
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["table"]),
            Err(e) => return e,
        };
        let remove = match p.get("remove") {
            None => None,
            Some(v) => match v
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .filter(|i| *i >= 1)
            {
                Some(i) => Some(i - 1),
                None => return failed(key!("error-bad-param"), &["remove"]),
            },
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        let editable = ew_api::PART_SECTIONS
            .iter()
            .find(|s| s.table.is_some_and(|t| t.path == table))
            .is_some_and(|s| editor.draft.section_editable(s.word));
        if !editable {
            return failed(key!("error-part-field"), &[&table]);
        }
        editor.error = None;
        let done = match remove {
            None => editor.draft.add_table_row(&table),
            Some(i) => editor.draft.remove_table_row(&table, i),
        };
        if done {
            Outcome::Done
        } else {
            failed(
                key!("error-bad-param"),
                &[if remove.is_some() { "remove" } else { "table" }],
            )
        }
    }

    /// `part.outline_import {path?}` (Import outline… of Front view and holes, doc 07 §5): the front outline of the DXF
    /// drawing at `path` into the editor's draft, drawn instead of the front's shape; without `path` the file dialog
    /// asks. Refused in words, under the editor, where the section is not the person's to change, the file cannot be
    /// read or holds no line; Save writes it.
    pub(crate) fn act_part_outline_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => return self.need_path("part.outline_import", Prompt::OpenOutline, "path", p),
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        match project.session.draft_outline_import(&mut editor.draft, &path) {
            Ok(_) => {
                editor.error = None;
                Outcome::Done
            }
            Err(message) => {
                editor.error = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    /// `part.outline_remove` (Remove of Front view and holes): the front's imported outline goes from the draft, and the
    /// front is drawn by its shape again.
    pub(crate) fn act_part_outline_remove(&mut self) -> Outcome {
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        if editor.draft.remove_outline() {
            editor.error = None;
            Outcome::Done
        } else {
            failed(key!("error-part-field"), &["outline"])
        }
    }

    /// `part.field {field, value}`: sets one field of the draft; a message shows under the field.
    pub(crate) fn act_part_field(&mut self, p: &Map<String, Value>) -> Outcome {
        let (field, value) = match (string_param(p, "field"), string_param(p, "value")) {
            (Ok(Some(f)), Ok(Some(v))) => (f.to_owned(), v.to_owned()),
            (Ok(None), _) => return failed(key!("error-missing-param"), &["field"]),
            (_, Ok(None)) => return failed(key!("error-missing-param"), &["value"]),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        editor.error = None;
        if field == "to_library" && editor.draft.new {
            editor.draft.to_library = value == "true";
            return Outcome::Done;
        }
        // A new plug's cable outlet and programming socket are words of its description, which follows them, its
        // bus and its terminating resistor.
        if let Some(mut plug) = editor.bus_part.filter(|b| b.plug) {
            match field.as_str() {
                "plug.angled" => plug.angled = value == "true",
                "plug.socket" => plug.socket = value == "true",
                _ => {}
            }
            editor.bus_part = Some(plug);
            let out = if field.starts_with("plug.") {
                Outcome::Done
            } else {
                match project.session.set_draft_field(&mut editor.draft, &field, &value) {
                    Err(PartDraftError::Refused(_)) => return failed(key!("error-part-field"), &[&field]),
                    Ok(()) | Err(PartDraftError::Field(_)) => Outcome::Done,
                }
            };
            if field.starts_with("plug.") || field.starts_with("bus.") {
                let words = plug_description(&editor.draft, plug);
                let _ = project
                    .session
                    .set_draft_field(&mut editor.draft, "description", &words);
            }
            return out;
        }
        // A module starts without a bus interface: the row a station's preset put in (New part… of a new station's
        // catalogue starts as its head) goes when the role is made Module, while it is as the preset made it.
        if field == "plc.role" && value == "module" && editor.draft.new && preset_bus(&editor.draft.bus) {
            editor.draft.remove_bus_row(0);
        }
        match project.session.set_draft_field(&mut editor.draft, &field, &value) {
            Err(PartDraftError::Refused(_)) => failed(key!("error-part-field"), &[&field]),
            Ok(()) | Err(PartDraftError::Field(_)) => Outcome::Done,
        }
    }

    /// `part.remove_own {part?, confirm?}` (Remove from my parts library… in the part editor of a part of My parts):
    /// the part (the `part` text as `part.edit` reads it, else the part editor's) taken out of My parts, so
    /// the next projects no longer offer it. The project keeps its copy, and the devices that use it keep it, which the
    /// toast names: every project embeds the parts it uses. Undo does not bring it back, so without `confirm` the
    /// window asks first (Cancel returns to the part editor) and a script is refused in words, as the journal's
    /// discard is. Refused in words for a part that is not in My parts.
    pub(crate) fn act_part_remove_own(&mut self, p: &Map<String, Value>) -> Outcome {
        // The part named, else the part editor's, else the part Edit part… would open: the selected device's, the
        // Libraries navigator's row.
        let copy = match (self.part_edit_copy(p), &self.dialog) {
            (Ok(Some(c)), _) if p.contains_key("part") => c,
            (_, Some(Dialog::PartEdit(e))) if !p.contains_key("part") => e.draft.copy,
            (Ok(Some(c)), _) => c,
            (Err(e), _) => return e,
            _ => return Outcome::refused(key!("reason-part-edit-target")),
        };
        let project = or_return!(crate::shell::gated_mut(&mut self.project));
        let held = {
            let q = project.session.query();
            copy.to_string().parse().ok().and_then(|id| {
                q.parts()
                    .get(id)
                    .map(|c| (c.library_id.as_uuid(), c.item_id.as_uuid()))
            })
        };
        // Only a part that My parts holds is taken out of it: anything else is refused before the question.
        let Some(((library, item), number)) = held.and_then(|(library, item)| {
            let number = project.session.own_part_number(library, item)?;
            Some(((library, item), number))
        }) else {
            return Outcome::refused(key!("reason-part-not-own"));
        };
        if !flag(p, "confirm") {
            return match self.mode {
                crate::shell::Mode::Headless => failed(key!("error-part-remove-confirm"), &[]),
                crate::shell::Mode::Gui => {
                    if matches!(self.dialog, Some(Dialog::PartEdit(_))) {
                        self.recovery_stash = self.dialog.take();
                    }
                    self.open_dialog(Dialog::ConfirmRemovePart {
                        part: format!("id:{copy}"),
                        number,
                    })
                }
            };
        }
        let project = or_return!(crate::shell::gated_mut(&mut self.project));
        let removed = match project.session.remove_own_part(library, item) {
            Ok(r) => r,
            Err(e) => return draft_refusal(e),
        };
        // The confirmation and the part editor it was asked from give way.
        if matches!(&self.dialog, Some(Dialog::ConfirmRemovePart { .. }))
            || matches!(&self.dialog, Some(Dialog::PartEdit(e)) if e.draft.copy == copy)
        {
            self.dialog = None;
        }
        self.recovery_stash = None;
        self.libraries_changed();
        let text = if removed.devices.is_empty() {
            strings::format(key!("part-removed-own"), &[&removed.number])
        } else {
            strings::format(
                key!("part-removed-own-kept"),
                &[&removed.number, &removed.devices.join(", ")],
            )
        };
        self.notify(text, false);
        Outcome::Done
    }

    /// `part.save {another?}` (Enter): opens the `part` card of `part.edit_copy`; a field that does not parse
    /// refuses. A new part is saved at once; with `another` (a new cable type's Save and add another size) the form
    /// stays open on the next size of the series.
    pub(crate) fn act_part_save(&mut self, p: &Map<String, Value>) -> Outcome {
        if matches!(&self.dialog, Some(Dialog::PartEdit(e)) if e.draft.new) {
            return self.part_save_new(flag(p, "another"));
        }
        if matches!(&self.dialog, Some(Dialog::PartEdit(e)) if e.draft.own && e.draft.fork) {
            return self.part_save_own_library();
        }
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Some(Dialog::PartEdit(editor)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        let plan = match project.session.part_edit_plan(&editor.draft) {
            Ok(plan) => plan,
            Err(e) => {
                let out = draft_refusal(e);
                editor.error = Some(match &out {
                    Outcome::Refused { reason: reason_key } => strings::render(reason_key),
                    Outcome::Failed { message } => message.clone(),
                    Outcome::Done => String::new(),
                });
                return out;
            }
        };
        let card = PartCard {
            title: editor.title(),
            fields: card_fields(&plan.fields),
            devices: Vec::new(),
            removed: plan.bom_removed.clone(),
            added: plan.bom_added.clone(),
            changed: plan.bom_changed.clone(),
            lost: plan.lost.clone(),
            boxes: self.box_words(&plan.boxes),
            fork: plan.fork.is_some(),
        };
        let devices: Vec<String> = plan.devices.iter().map(|d| self.tag_of(d.as_uuid())).collect();
        self.dialog = None;
        self.preview_host_card_as(
            &plan.command.name,
            plan.command.params,
            key!("reason-part-edit"),
            CardOrigin::Part(Box::new(PartCard { devices, ..card })),
        )
    }
}
