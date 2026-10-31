// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 4a (D20, D21, FT-394, FR-PRT-022): the part editor's draft. [`Session::part_draft`] reads a held part
//! copy into texts; [`Session::set_draft_field`] checks each field as it is typed; [`Session::part_edit_plan`] builds
//! `part.edit_copy` (in place for a project-local copy, a fork of a copy from a library) with what the card shows: the
//! field diff, the devices that use the copy and the component BOM rows that change, read from a preview on a fork of
//! the current snapshot.

use std::collections::BTreeSet;

use ew_commands::commands::{EditPartCopy, FieldChange, classify_update};
use ew_derive::Q;
use ew_derive::ew_ids::{Id, Uuid};
use ew_mltext::Ident;
use ew_schema::{Device, Kind, LibCopy, LibDocument, LibRef, OrderUnit, Part, PartKind, TechVal, TechValue};
use serde_json::Value;

use crate::library::ItemQuery;
use crate::output::{FieldType, InputError};
use crate::part_offers::{PartPick, kind_word, lib_ref, text_of};
use crate::{AnyCommand, BomMode, Export, Session};

/// The fields of a draft in the editor's order (`part.field`'s `field`; `tech:<key>` names a technical value).
pub const DRAFT_FIELDS: [&str; 9] = [
    "manufacturer",
    "part_number",
    "type_designation",
    "order_number",
    "gtin",
    "description",
    "kind",
    "category",
    "unit",
];

/// The part kinds in doc 07 §2.5's order, as the `kind` field spells them (the part editor's Kind offers them; a test
/// ties them to the schema's part kinds, both ways).
pub const PART_KINDS: [&str; 15] = [
    "device",
    "terminal",
    "cable",
    "wire",
    "ferrule",
    "marker",
    "duct",
    "rail",
    "busbar",
    "end_bracket",
    "enclosure",
    "mounting_plate",
    "tube",
    "accessory",
    "configured",
];

/// One field's message: the strings key and its arguments (the shell words it, D21).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DraftMessage {
    /// The field (`manufacturer`, `tech:rated_current`).
    pub field: String,
    /// The strings key (`reason-part-no-manufacturer`, `error-part-tech-value`).
    pub key: &'static str,
    /// The key's arguments, in its placeholders' order.
    pub args: Vec<String>,
}

/// Why a draft field or the draft cannot be saved (D21).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PartDraftError {
    /// The whole draft: a reason key (`reason-part-unchanged`).
    Refused(&'static str),
    /// One field's message.
    Field(DraftMessage),
}

/// The part editor's draft (D20, D21): the editable members as texts, their messages, and the read-only members the
/// editor shows.
#[derive(Clone, PartialEq, Debug)]
pub struct PartDraft {
    /// The held copy.
    pub copy: Uuid,
    /// The texts of [`DRAFT_FIELDS`], in that order.
    pub fields: Vec<(&'static str, String)>,
    /// The technical data: (key, value text), in key order.
    pub tech: Vec<(String, String)>,
    /// The messages of the fields that do not parse, in the order they were found.
    pub messages: Vec<DraftMessage>,
    /// Whether saving forks the copy (a copy from a library, D20).
    pub fork: bool,
    /// The source library's name of a copy from a library (its ID when it is not installed); `None` for a
    /// project-local copy.
    pub library: Option<String>,
    /// The copy's version.
    pub version: String,
    /// The template's function names (read-only).
    pub functions: Vec<String>,
    /// The CP pattern's designations (read-only).
    pub cps: Vec<String>,
    original: Part,
    manufacturer: (LibRef, Vec<Value>),
}

impl PartDraft {
    /// The text of a field of [`DRAFT_FIELDS`] or `tech:<key>`; `""` for another.
    pub fn text(&self, field: &str) -> &str {
        if let Some(key) = field.strip_prefix("tech:") {
            return self
                .tech
                .iter()
                .find(|(k, _)| k == key)
                .map_or("", |(_, v)| v.as_str());
        }
        self.fields
            .iter()
            .find(|(f, _)| *f == field)
            .map_or("", |(_, v)| v.as_str())
    }

    /// The message of a field, if it does not parse.
    pub fn message(&self, field: &str) -> Option<&DraftMessage> {
        self.messages.iter().find(|m| m.field == field)
    }

    fn set_text(&mut self, field: &str, text: &str) {
        if let Some(slot) = self.fields.iter_mut().find(|(f, _)| *f == field) {
            slot.1 = text.to_owned();
        }
    }

    fn set_message(&mut self, field: &str, m: Option<DraftMessage>) {
        self.messages.retain(|x| x.field != field);
        if let Some(m) = m {
            self.messages.push(m);
        }
    }
}

/// What saving a draft does (D20's card): the command, the fork item, the field diff, the devices that use the copy
/// and the component BOM rows that change.
#[derive(Clone, Debug)]
pub struct PartEditPlan {
    /// The `part.edit_copy` command (Bulk: preview and commit it).
    pub command: AnyCommand,
    /// The fork's item ID; `None` for an edit in place.
    pub fork: Option<Uuid>,
    /// The members that differ.
    pub fields: Vec<FieldChange>,
    /// The devices that use the copy (in ID order).
    pub devices: Vec<Id<Device>>,
    /// The component BOM's CSV rows that go (in BOM order).
    pub bom_removed: Vec<String>,
    /// The component BOM's CSV rows that come (in BOM order).
    pub bom_added: Vec<String>,
}

fn message(field: &str, key: &'static str, args: &[&str]) -> DraftMessage {
    DraftMessage {
        field: field.to_owned(),
        key,
        args: args.iter().map(|a| (*a).to_owned()).collect(),
    }
}

fn unit_word(u: &OrderUnit) -> String {
    serde_json::to_value(u)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn opt_text(i: Option<&Ident>) -> String {
    i.map_or_else(String::new, |i| i.as_str().to_owned())
}

/// The text of a technical value: a quantity `4 A`, an identifier verbatim.
fn tech_text(v: &TechVal) -> String {
    match v {
        TechVal::Quantity(_) | TechVal::Unknown(_) => {
            let value = serde_json::to_value(v).unwrap_or(Value::Null);
            crate::output::format_field_value(&FieldType::Quantity, &value)
        }
        TechVal::Ident(i) => i.as_str().to_owned(),
    }
}

impl Session {
    /// The manufacturer copies the project holds: (name, reference), in name order.
    fn held_manufacturers(&self, q: &Q) -> Vec<(String, LibRef)> {
        let mut out: Vec<(String, LibRef)> = q
            .manufacturers()
            .iter()
            .map(|(_, c)| {
                let r = lib_ref(c.library_id.as_uuid(), c.item_id.as_uuid());
                (ew_derive::manufacturer_name(q, &r), r)
            })
            .collect();
        out.sort_by(|a, b| ew_mltext::natural_cmp(&a.0, &b.0));
        out
    }

    /// The manufacturers the editor offers (D21): the project's manufacturer copies, then the catalogue's that the
    /// project does not hold, by name in natural order, each name once.
    pub fn part_manufacturers(&self) -> Vec<String> {
        let q = self.query();
        let mut names: Vec<String> = self.held_manufacturers(&q).into_iter().map(|(n, _)| n).collect();
        for h in self.libraries().search_items(&ItemQuery {
            kinds: vec![Kind::Manufacturer],
            ..ItemQuery::default()
        }) {
            let name = if h.key.is_empty() { h.name } else { h.key };
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                names.push(name);
            }
        }
        names.sort_by(|a, b| ew_mltext::natural_cmp(a, b));
        names
    }

    /// The draft of a held part copy (D20): its editable members as texts and the read-only ones.
    ///
    /// # Errors
    ///
    /// [`crate::ApiError::NotHeld`] when the project holds no part copy `copy`.
    pub fn part_draft(&self, copy: Uuid) -> Result<PartDraft, crate::ApiError> {
        let q = self.query();
        let c: LibCopy<Part> = (**q
            .parts()
            .get(Id::from_uuid(copy))
            .ok_or(crate::ApiError::NotHeld(copy))?)
        .clone();
        let project = q.project().id.as_uuid();
        let lib = c.library_id.as_uuid();
        let p = &c.item;
        let library = (lib != project).then(|| {
            self.libraries()
                .libraries()
                .into_iter()
                .find(|l| l.id == lib)
                .map_or_else(|| lib.to_string(), |l| l.name)
        });
        let fields = vec![
            ("manufacturer", ew_derive::manufacturer_name(&q, &p.manufacturer)),
            ("part_number", p.part_number.as_str().to_owned()),
            ("type_designation", opt_text(p.type_designation.as_ref())),
            ("order_number", opt_text(p.order_number.as_ref())),
            ("gtin", opt_text(p.gtin.as_ref())),
            ("description", text_of(&p.description)),
            ("kind", kind_word(&p.kind)),
            (
                "category",
                p.category.as_ref().map_or_else(String::new, |(a, b, c)| {
                    format!("{}/{}/{}", a.as_str(), b.as_str(), c.as_str())
                }),
            ),
            ("unit", unit_word(&p.unit)),
        ];
        let tech = p
            .tech_data
            .iter()
            .map(|t| (t.key.as_str().to_owned(), tech_text(&t.value)))
            .collect();
        Ok(PartDraft {
            copy,
            fields,
            tech,
            messages: Vec::new(),
            fork: lib != project,
            library,
            version: c.version.clone(),
            functions: self.part_offer_functions(&PartPick::Held(copy)),
            cps: p
                .cp_pattern
                .iter()
                .map(|cp| cp.designation.as_str().to_owned())
                .collect(),
            manufacturer: (p.manufacturer.clone(), Vec::new()),
            original: c.item,
        })
    }

    /// The manufacturer named `name`: a held copy (no documents), else a catalogue manufacturer with the documents
    /// the project lacks.
    fn find_manufacturer(&self, q: &Q, name: &str) -> Option<(LibRef, Vec<Value>)> {
        if let Some((_, r)) = self
            .held_manufacturers(q)
            .into_iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
        {
            return Some((r, Vec::new()));
        }
        let hit = self
            .libraries()
            .search_items(&ItemQuery {
                text: name.to_owned(),
                kinds: vec![Kind::Manufacturer],
                ..ItemQuery::default()
            })
            .into_iter()
            .find(|h| h.key.eq_ignore_ascii_case(name) || h.name.eq_ignore_ascii_case(name))?;
        let r = lib_ref(hit.library, hit.item);
        let docs = self.closure_docs(std::slice::from_ref(&r)).ok()?;
        Some((
            r,
            docs.iter()
                .map(|d| serde_json::to_value(d).expect("a library document has a JSON form"))
                .collect(),
        ))
    }

    /// Sets a field of the draft from its text and checks it (D21): the text is kept either way, a field that does not
    /// parse gets its message ([`PartDraft::message`]) and the error. `tech:<key>` sets a technical value (an empty
    /// text removes it): a text that starts with a digit parses by the property grid's quantity rule, any other as an
    /// identifier; a key given twice (up to case) is refused.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Field`] with the field's message; [`PartDraftError::Refused`] (`error-part-field`) for a
    /// field the editor does not have.
    pub fn set_draft_field(
        &self,
        draft: &mut PartDraft,
        field: &str,
        text: &str,
    ) -> Result<(), PartDraftError> {
        let t = text.trim();
        let result = if let Some(key) = field.strip_prefix("tech:") {
            self.set_tech(draft, key, t)
        } else {
            if !DRAFT_FIELDS.contains(&field) {
                return Err(PartDraftError::Refused("error-part-field"));
            }
            draft.set_text(field, text);
            self.check_field(draft, field, t)
        };
        match result {
            Ok(()) => {
                draft.set_message(field, None);
                Ok(())
            }
            Err(m) => {
                draft.set_message(field, Some(m.clone()));
                Err(PartDraftError::Field(m))
            }
        }
    }

    fn check_field(&self, draft: &mut PartDraft, field: &str, t: &str) -> Result<(), DraftMessage> {
        let ident =
            |t: &str| Ident::from_input(t).map_err(|e| message(field, "error-part-value", &[&e.to_string()]));
        match field {
            "manufacturer" => {
                if t.is_empty() {
                    return Err(message(field, "reason-part-no-manufacturer", &[]));
                }
                let q = self.query();
                let current = ew_derive::manufacturer_name(&q, &draft.original.manufacturer);
                draft.manufacturer = if current.eq_ignore_ascii_case(t) {
                    (draft.original.manufacturer.clone(), Vec::new())
                } else {
                    self.find_manufacturer(&q, t)
                        .ok_or_else(|| message(field, "error-part-manufacturer", &[t]))?
                };
                Ok(())
            }
            "part_number" => {
                if t.is_empty() {
                    return Err(message(field, "reason-part-no-number", &[]));
                }
                ident(t).map(|_| ())
            }
            "type_designation" | "order_number" | "gtin" | "description" => {
                if t.is_empty() || field == "description" {
                    Ok(())
                } else {
                    ident(t).map(|_| ())
                }
            }
            "kind" => match serde_json::from_value::<PartKind>(Value::String(t.to_owned())) {
                Ok(PartKind::Unknown(_)) | Err(_) => Err(message(field, "error-part-kind", &[t])),
                Ok(_) => Ok(()),
            },
            "category" => {
                if t.is_empty() {
                    return Ok(());
                }
                let levels: Vec<&str> = t.split('/').map(str::trim).collect();
                if levels.len() != 3
                    || levels
                        .iter()
                        .any(|l| Ident::from_input(l).is_err() || l.is_empty())
                {
                    return Err(message(field, "error-part-category", &[]));
                }
                Ok(())
            }
            "unit" => match t {
                "piece" | "m" | "set" => Ok(()),
                _ => Err(message(field, "reason-part-unit", &[])),
            },
            _ => Ok(()),
        }
    }

    fn set_tech(&self, draft: &mut PartDraft, key: &str, t: &str) -> Result<(), DraftMessage> {
        let field = format!("tech:{key}");
        if Ident::from_input(key).is_err() || key.trim().is_empty() || key != key.trim() {
            return Err(message(&field, "error-part-tech-value", &[key, "not a key"]));
        }
        if t.is_empty() {
            draft.tech.retain(|(k, _)| k != key);
            return Ok(());
        }
        if draft
            .tech
            .iter()
            .any(|(k, _)| k != key && k.eq_ignore_ascii_case(key))
        {
            return Err(message(&field, "error-part-tech-twice", &[key]));
        }
        match draft.tech.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = t.to_owned(),
            None => {
                draft.tech.push((key.to_owned(), t.to_owned()));
                draft.tech.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
        self.tech_value(key, t).map(|_| ())
    }

    /// A technical value's text as its JSON form (D21).
    fn tech_value(&self, key: &str, t: &str) -> Result<TechVal, DraftMessage> {
        let field = format!("tech:{key}");
        if t.starts_with(|c: char| c.is_ascii_digit()) {
            let parsed = self.parse_quantity_text(t).map_err(|e| {
                let why = match e {
                    InputError::Invalid(m) | InputError::UnknownPart(m) => m,
                    InputError::Ambiguous { choices } => choices
                        .iter()
                        .map(|(t, _)| t.as_str())
                        .collect::<Vec<_>>()
                        .join(" or "),
                };
                message(&field, "error-part-tech-value", &[key, &why])
            })?;
            serde_json::from_value(parsed)
                .map_err(|e| message(&field, "error-part-tech-value", &[key, &e.to_string()]))
        } else {
            Ident::from_input(t)
                .map(TechVal::Ident)
                .map_err(|e| message(&field, "error-part-tech-value", &[key, &e.to_string()]))
        }
    }

    /// The draft's part item and the manufacturer documents it needs; the first message of a field that does not
    /// parse is the refusal.
    fn draft_part(&self, draft: &PartDraft) -> Result<(Part, Vec<Value>), PartDraftError> {
        if let Some(m) = draft.messages.first() {
            return Err(PartDraftError::Field(m.clone()));
        }
        // the checks again, so that a draft built by hand cannot bypass them
        let mut check = draft.clone();
        for f in DRAFT_FIELDS {
            let text = draft.text(f).trim().to_owned();
            self.check_field(&mut check, f, &text)
                .map_err(PartDraftError::Field)?;
        }
        let mut p = draft.original.clone();
        let text = |f: &str| draft.text(f).trim().to_owned();
        let opt = |f: &str| {
            let t = text(f);
            (!t.is_empty()).then(|| Ident::from_input(&t).expect("checked"))
        };
        p.manufacturer = check.manufacturer.0.clone();
        p.part_number = Ident::from_input(&text("part_number")).expect("checked");
        p.type_designation = opt("type_designation");
        p.order_number = opt("order_number");
        p.gtin = opt("gtin");
        let description = draft.text("description").trim().to_owned();
        if description != text_of(&p.description) {
            if p.description.is_neutral() {
                p.description.set_neutral(&description);
            } else {
                let tag = p
                    .description
                    .languages()
                    .next()
                    .cloned()
                    .expect("a language text");
                p.description.set_lang(tag, &description);
            }
        }
        p.kind = serde_json::from_value(Value::String(text("kind"))).expect("checked");
        p.category = {
            let t = text("category");
            (!t.is_empty()).then(|| {
                let l: Vec<Ident> = t
                    .split('/')
                    .map(|s| Ident::from_input(s.trim()).expect("checked"))
                    .collect();
                (l[0].clone(), l[1].clone(), l[2].clone())
            })
        };
        p.unit = serde_json::from_value(Value::String(text("unit"))).expect("checked");
        let mut tech: Vec<TechValue> = Vec::new();
        for (k, v) in &draft.tech {
            let value = self.tech_value(k, v).map_err(PartDraftError::Field)?;
            let key = Ident::from_input(k).expect("checked");
            // keep a value's unknown members when its text is unchanged
            match draft.original.tech_data.iter().find(|t| t.key == key) {
                Some(old) if old.value == value => tech.push(old.clone()),
                _ => tech.push(TechValue {
                    key,
                    value,
                    extra: ew_schema::Extra::new(),
                }),
            }
        }
        p.tech_data = ew_schema::KeyedList::from_vec(tech)
            .map_err(|e| PartDraftError::Field(message("tech", "error-part-value", &[&e.to_string()])))?;
        Ok((p, check.manufacturer.1))
    }

    /// `part.edit_copy` of a draft (D20, D21): in place for a project-local copy, else a fork whose item ID
    /// [`Session::next_fork_item`] mints.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Field`] for the first field that does not parse; [`PartDraftError::Refused`]
    /// (`reason-part-unchanged`) when the part equals the copy's.
    pub fn part_edit_command(&self, draft: &PartDraft) -> Result<AnyCommand, PartDraftError> {
        let (part, copies) = self.draft_part(draft)?;
        if part == draft.original {
            return Err(PartDraftError::Refused("reason-part-unchanged"));
        }
        let cmd = EditPartCopy {
            copy: Id::from_uuid(draft.copy),
            part: serde_json::to_value(&part).expect("a part has a JSON form"),
            fork: draft.fork.then(|| Id::from_uuid(self.next_fork_item(draft.copy))),
            copies,
        };
        AnyCommand::of(&cmd)
            .map_err(|e| PartDraftError::Field(message("part", "error-part-value", &[&e.to_string()])))
    }

    /// [`Session::part_edit_command`] with what the card shows (D20): the field diff, the devices that use the copy
    /// and the component BOM rows that change, from a preview of the command on a fork of the current snapshot.
    ///
    /// # Errors
    ///
    /// As [`Session::part_edit_command`]; a refused preview is [`PartDraftError::Field`] on `part` with the
    /// command's message.
    pub fn part_edit_plan(&self, draft: &PartDraft) -> Result<PartEditPlan, PartDraftError> {
        let command = self.part_edit_command(draft)?;
        let q = self.query();
        let held = q
            .parts()
            .get(Id::from_uuid(draft.copy))
            .map(|c| (**c).clone())
            .ok_or(PartDraftError::Refused("reason-part-edit-target"))?;
        let mut edited = held.clone();
        edited.item = serde_json::from_value(command.params["part"].clone()).expect("the draft's part");
        let fields = classify_update(&LibDocument::Part(held.clone()), &LibDocument::Part(edited)).fields;
        let devices: BTreeSet<Id<Device>> = q
            .devices()
            .iter()
            .filter(|(_, d)| d.parts.iter().any(|u| u.part.copy == held.id))
            .map(|(id, _)| id)
            .collect();
        let model = self.world().model.clone();
        let mut engine = ew_commands::Engine::new(
            model,
            ew_commands::EngineOptions {
                prevent: Some(Box::new(crate::ShippedPrevent)),
                ..ew_commands::EngineOptions::default()
            },
        );
        let preview = engine
            .preview(&command)
            .map_err(|e| PartDraftError::Field(message("part", "error-part-value", &[&e.to_string()])))?;
        let after = self.query_over(preview.candidate());
        let bom = |q: &Q| -> Vec<String> {
            ew_reports::export::export_csv(q, Export::Bom(BomMode::Component))
                .map(|t| t.lines().skip(1).map(str::to_owned).collect())
                .unwrap_or_default()
        };
        let (before_rows, after_rows) = (bom(&q), bom(&after));
        Ok(PartEditPlan {
            fork: command
                .params
                .get("fork")
                .and_then(Value::as_str)
                .and_then(|s| s.parse().ok()),
            command,
            fields,
            devices: devices.into_iter().collect(),
            bom_removed: before_rows
                .iter()
                .filter(|r| !after_rows.contains(r))
                .cloned()
                .collect(),
            bom_added: after_rows
                .iter()
                .filter(|r| !before_rows.contains(r))
                .cloned()
                .collect(),
        })
    }
}

#[cfg(test)]
mod part_kinds_tests {
    use ew_schema::PartKind;
    use serde_json::Value;

    use super::{PART_KINDS, kind_word};

    /// M39: every kind of [`PART_KINDS`] reads as a schema part kind and writes back the same word, and every schema
    /// part kind (but the verbatim unknown one) is in the list, so the editor offers exactly the schema's kinds.
    #[test]
    fn the_part_kinds_are_the_schemas() {
        let mut seen = Vec::new();
        for k in PART_KINDS {
            let kind: PartKind = serde_json::from_value(Value::String(k.to_owned())).expect(k);
            assert!(!matches!(kind, PartKind::Unknown(_)), "{k}");
            assert_eq!(kind_word(&kind), k);
            seen.push(kind);
        }
        // Exhaustive: a new variant fails to compile here, a reminder to list it above.
        for kind in &seen {
            match kind {
                PartKind::Device
                | PartKind::Terminal
                | PartKind::Cable
                | PartKind::Wire
                | PartKind::Ferrule
                | PartKind::Marker
                | PartKind::Duct
                | PartKind::Rail
                | PartKind::Busbar
                | PartKind::EndBracket
                | PartKind::Enclosure
                | PartKind::MountingPlate
                | PartKind::Tube
                | PartKind::Accessory
                | PartKind::Configured => {}
                PartKind::Unknown(_) => unreachable!(),
            }
        }
    }
}
