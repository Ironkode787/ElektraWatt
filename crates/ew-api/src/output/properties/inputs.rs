// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The parse rules of the property sheet's editors (§6.5): a row's typed text read back, the PLC typed editors
//! (FT-429), a cable's length, a frozen tag, and the choices of a choice editor.
//!
//! Not here: a bus port's address (`bus_rows.rs`); the field types' own parsers (`output/fields.rs`).

use ew_derive::ew_ids::Id;
use serde_json::Value;

use super::bus_rows::bus_address_input;
use super::context::SheetCx;
use crate::Session;
use crate::output::fields::{
    FieldChoice, FieldType, InputError, ParsedInput, catalogue_kind, choices, field_catalogue, spec_of,
};
use crate::output::table_edit::{Seg, segments};

impl Session {
    /// The parse rules of §6.5 for the row `row_path` of `kind` (list indices allowed).
    ///
    /// # Errors
    ///
    /// [`InputError::Invalid`] with the reason, or [`InputError::Ambiguous`] with the two readings.
    pub fn parse_field_input(
        &self,
        kind: &str,
        row_path: &str,
        text: &str,
    ) -> Result<ParsedInput, InputError> {
        if (kind, row_path) == ("device", "tag.frozen")
            && let Some(parsed) = frozen_tag_input(text)
        {
            return parsed;
        }
        if (kind, row_path) == ("device", "cable.length")
            && let Some(parsed) = cable_length_input(text)
        {
            return parsed;
        }
        // FT-856: a bus port's address typed as its row shows it.
        if (kind, row_path) == ("function", "bus_port.address")
            && let Some(parsed) = bus_address_input(text)
        {
            return parsed;
        }
        // FT-429: the PLC grid's typed editors. A channel's address and a module's start as the
        // profile's text or a keyword (the command reads it with the configuration project's profile), a channel's
        // target as a device tag (its main function), a module's slot as a number.
        if let Some(parsed) = self.plc_input(kind, row_path, text) {
            return parsed;
        }
        let q = self.query();
        let ty = row_type(kind, row_path)
            .ok_or_else(|| InputError::Invalid(api_text!("refusal-input-not-a-field", row_path, kind)))?;
        let member = last_member(row_path);
        let cx = match offers_library_parts(kind, &member, &ty) {
            true => SheetCx::new(&q).with_library_parts(self),
            false => SheetCx::new(&q),
        };
        crate::output::fields::parse_input(&cx, &ty, &member, text)
    }

    /// The options of the choice editor of the row `row_path` of `kind` matching the case-folded `filter` (§6.5):
    /// an enum's words, the colour codes, the part copies, the structure nodes of a slot's aspect; empty otherwise.
    pub fn field_choices(&self, kind: &str, row_path: &str, filter: &str) -> Vec<FieldChoice> {
        let Some(ty) = row_type(kind, row_path) else {
            return Vec::new();
        };
        let q = self.query();
        let member = last_member(row_path);
        let cx = match offers_library_parts(kind, &member, &ty) {
            true => SheetCx::new(&q).with_library_parts(self),
            false => SheetCx::new(&q),
        };
        choices(&cx, &ty, &member, filter)
    }
}

/// Whether the row of member `member` of `kind`, of type `ty`, offers and takes the libraries' parts beside the
/// project's (choosing one embeds it): a Parts row, the Part row of one part use (rc.3: a cable's Part row refused a
/// catalogue cable's number) and a fluid line's tube part.
fn offers_library_parts(kind: &str, member: &str, ty: &FieldType) -> bool {
    ty.is_part_uses()
        || (kind == "device"
            && member == "part"
            && matches!(ty.inner(), FieldType::Link { kind } if kind == "part"))
        || (kind == "connection" && member == crate::output::fields::TUBE_PART)
}

impl Session {
    /// The PLC typed editors of [`Session::parse_field_input`] (FT-429); `None` for another field
    /// or a JSON object, the field's own form, parsed as before.
    fn plc_input(&self, kind: &str, row_path: &str, text: &str) -> Option<Result<ParsedInput, InputError>> {
        let t = text.trim();
        if t.starts_with('{') {
            return None;
        }
        // A JSON string (`"excluded"`) is read as its text.
        let decoded = match serde_json::from_str::<Value>(t) {
            Ok(Value::String(s)) => Some(s),
            _ => None,
        };
        let t = decoded.as_deref().map_or(t, str::trim);
        let plain = |value: Value| {
            Some(Ok(ParsedInput {
                value,
                notes: Vec::new(),
            }))
        };
        match (kind, row_path) {
            // `auto`, `excluded`, `tbd` or an address text (`I4.0`); `fields.set` reads it by the channel's profile and
            // refuses a text that does not parse with the profile's example.
            ("channel", "address") if !t.is_empty() => plain(Value::String(t.to_owned())),
            ("channel", "address") => Some(Err(InputError::Invalid(api_text!("refusal-input-plc-address")))),
            // `auto`, `excluded`, `tbd` or a start text (`Q10`): `plc.set_start` reads it.
            ("device", "plc_module.start.input" | "plc_module.start.output") if !t.is_empty() => {
                plain(Value::String(t.to_owned()))
            }
            ("device", "plc_module.slot") => Some(plc_slot_input(t)),
            ("channel", "target") if t.is_empty() => plain(Value::Null),
            ("channel", "target") => {
                let q = self.query();
                let id = match self.resolve_reference(t) {
                    Ok(id) => id,
                    // The script references' resolver words its reasons by key (`cmdlog`), in the window's language.
                    Err(reason) => {
                        return Some(Err(InputError::Invalid(api_text!(
                            "refusal-input-detail",
                            reason
                        ))));
                    }
                };
                if q.functions().get(Id::from_uuid(id)).is_some() {
                    return plain(Value::String(id.to_string()));
                }
                let device: Id<ew_derive::ew_schema::Device> = Id::from_uuid(id);
                match q
                    .devices()
                    .get(device)
                    .and(*q.get::<ew_derive::MainFunction>(device))
                {
                    Some(f) => plain(Value::String(f.as_uuid().to_string())),
                    None => Some(Err(InputError::Invalid(api_text!("refusal-input-plc-target", t)))),
                }
            }
            _ => None,
        }
    }
}

/// A module's slot typed as the PLC grid prints it: empty, `auto` or `positional` for the positional
/// slot, a number for that slot in rack 0, `<rack>.<slot>` and `<rack>.<slot>.<subslot>`.
pub(crate) fn plc_slot_input(t: &str) -> Result<ParsedInput, InputError> {
    let lower = t.to_ascii_lowercase();
    if lower.is_empty() || lower == "auto" || lower == "positional" {
        return Ok(ParsedInput {
            value: Value::String("positional".to_owned()),
            notes: Vec::new(),
        });
    }
    let parts: Result<Vec<u16>, _> = lower.split('.').map(str::parse::<u16>).collect();
    let rack = match parts.as_deref() {
        Ok([slot]) => serde_json::json!({"rack": 0, "slot": slot}),
        Ok([rack, slot]) => serde_json::json!({"rack": rack, "slot": slot}),
        Ok([rack, slot, sub]) => serde_json::json!({"rack": rack, "slot": slot, "subslot": sub}),
        _ => {
            return Err(InputError::Invalid(api_text!("refusal-input-plc-slot", t)));
        }
    };
    Ok(ParsedInput {
        value: serde_json::json!({ "rack": rack }),
        notes: Vec::new(),
    })
}

/// A cable's length source typed as its words (FT-429): `<number> m` (or the number alone, in metres)
/// is a manual length, `missing` (or `length missing`, the cable overview's text) none, `from part` the part's.
/// `None` for anything else (a JSON value, the field's own form, parsed as before).
fn cable_length_input(text: &str) -> Option<Result<ParsedInput, InputError>> {
    use ew_derive::ew_schema::LengthSource;
    use ew_derive::ew_units::Nm;
    let t = text.trim();
    let plain = |source: LengthSource| {
        Some(
            serde_json::to_value(&source)
                .map(|value| ParsedInput {
                    value,
                    notes: Vec::new(),
                })
                .map_err(|e| InputError::Invalid(crate::serde_error_text(&e))),
        )
    };
    match t.to_lowercase().as_str() {
        "missing" | "length missing" => return plain(LengthSource::Missing),
        "from part" | "from_part" => return plain(LengthSource::FromPart),
        _ => {}
    }
    let number = t.strip_suffix('m').map_or(t, str::trim_end);
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let (whole, frac) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty() || frac.contains('.') || frac.len() > 9 {
        return Some(Err(InputError::Invalid(api_text!("refusal-input-metres", t))));
    }
    let nm = whole
        .parse::<i64>()
        .ok()
        .and_then(|w| w.checked_mul(1_000_000_000))
        .and_then(|w| {
            let f: i64 = format!("{frac:0<9}").parse().ok()?;
            w.checked_add(f)
        });
    match nm {
        Some(nm) => plain(LengthSource::Manual(Nm(nm))),
        None => Some(Err(InputError::Invalid(api_text!("refusal-input-too-long", t)))),
    }
}

/// The frozen tag typed as the page should print it (item 2): `S51` freezes the text `S51` with the sign hidden,
/// `-S51` the text `S51` with the sign shown (the default sign, left out of the value); a JSON string (`"S51"`) is read
/// as its text. An empty text unfreezes (`null`). `None` for a JSON object, which is the field's own form and parsed
/// as before.
fn frozen_tag_input(text: &str) -> Option<Result<ParsedInput, InputError>> {
    let t = text.trim();
    if t.starts_with('{') {
        return None;
    }
    let t = match serde_json::from_str::<Value>(t) {
        Ok(Value::String(s)) => s,
        _ => t.to_owned(),
    };
    let t = t.trim();
    if t.is_empty() {
        return Some(Ok(ParsedInput {
            value: Value::Null,
            notes: Vec::new(),
        }));
    }
    let (shown, body) = match t.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, t),
    };
    Some(match ew_mltext::Ident::from_input(body) {
        Err(e) => Err(InputError::Invalid(crate::ident_error_text(&e))),
        Ok(ident) => {
            let mut value = serde_json::json!({ "text": ident.as_str() });
            if !shown {
                value["sign"] = Value::from("hidden");
            }
            Ok(ParsedInput {
                value,
                notes: Vec::new(),
            })
        }
    })
}

/// The last member of a row path (`structure.location` → `location`).
fn last_member(row_path: &str) -> String {
    segments(row_path)
        .into_iter()
        .rev()
        .find_map(|s| match s {
            Seg::Member(m) => Some(m),
            Seg::Index(_) => None,
        })
        .unwrap_or_default()
}

/// The type of a row: the catalogue's, a derived row's partner's, or `Json` for an unlisted member.
fn row_type(kind: &str, row_path: &str) -> Option<FieldType> {
    catalogue_kind(kind)?;
    let catalogue = field_catalogue(kind)?;
    if let Some(spec) = spec_of(&catalogue, row_path) {
        return Some(spec.ty.clone());
    }
    // An unlisted member (a derived row's partner such as `overrides.flags.shielded` is a catalogue node).
    Some(FieldType::Json)
}
