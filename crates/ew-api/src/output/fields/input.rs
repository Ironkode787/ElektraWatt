// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A typed value read (see the module documentation of `fields`): a row's text parsed by its type, the refusal of a
//! text that does not read, a part found by its words, and the choices a row offers.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A scale typed in the words [`scale_text`] shows: `1:n` (spaces allowed around the colon) is
/// `{"one_to": n}`, `not to scale` is `not_to_scale`; another ratio of numbers (`1:4.5`, `2:9`, `1:0`) is refused in
/// words, as it is no scale the page can draw; `None` for any other text (a
/// JSON value, parsed as before).
fn scale_input(text: &str) -> Option<Result<Value, InputError>> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("not to scale") {
        return Some(Ok(Value::String("not_to_scale".to_owned())));
    }
    let (one, n) = t.split_once(':')?;
    let (one, n) = (one.trim(), n.trim());
    let number = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',');
    if !number(one) || !number(n) {
        return None;
    }
    Some(match n.parse::<u32>() {
        Ok(k) if k > 0 && one == "1" => Ok(serde_json::json!({ "one_to": k })),
        _ => Err(invalid(text!("refusal-input-scale", t))),
    })
}

/// A rectangle typed in the words [`rect_text`] shows: its place, a comma and a space, its other coordinate, then `·`
/// and its width `×` (or `x`) its height, each in millimetres or with its unit; `None` for any other text (a JSON
/// value, parsed as before).
fn rect_input(text: &str) -> Option<Result<Value, InputError>> {
    let t = text.trim();
    let (place, size) = t.split_once('·')?;
    let place = place.trim();
    let place = place.strip_suffix("mm").unwrap_or(place);
    let (x, y) = place.split_once(", ")?;
    let (w, h) = size.split_once(['×', 'x'])?;
    let bad = || invalid(text!("refusal-input-rect", t));
    let nm = |s: &str| parse_nm(s).ok().and_then(|v| v.as_i64());
    Some(match (nm(x), nm(y), nm(w), nm(h)) {
        (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 => {
            match (x.checked_add(w), y.checked_add(h)) {
                (Some(x2), Some(y2)) => Ok(serde_json::json!([[x, y], [x2, y2]])),
                _ => Err(bad()),
            }
        }
        _ => Err(bad()),
    })
}

/// A parsed editor input.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParsedInput {
    /// The value in the field's JSON encoding.
    pub value: Value,
    /// Notes that do not block the commit: confusable code points, an alias that resolved a node.
    pub notes: Vec<String>,
}

/// Why an editor input does not parse.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum InputError {
    /// Not a value of the field's type; the text says why, by key: the window shows it in its language.
    Invalid(crate::Text),
    /// No part copy of the project has the typed part number (the text as typed, trimmed); the shell says so in a
    /// sentence of its strings table.
    UnknownPart(String),
    /// Two readings; the user picks one: (the choice's text, its value).
    Ambiguous {
        /// The choices.
        choices: Vec<(String, Value)>,
    },
    /// Not one of an enumerated field's words: the text as typed, trimmed, and the stored words, which the
    /// shell names in its words ([`InputError::text`] gives the English sentence).
    NotOneOf {
        /// The text as typed, trimmed.
        typed: String,
        /// The stored words, in schema order.
        options: Vec<String>,
    },
}

impl InputError {
    /// The English sentence of an [`InputError::Invalid`] or [`InputError::NotOneOf`], for a front-end without the
    /// shell's strings (the table round trip, the part editor); `None` for the others.
    pub fn text(&self) -> Option<String> {
        match self {
            InputError::Invalid(why) => Some(why.to_string()),
            InputError::NotOneOf { typed, options } => {
                Some(format!("`{typed}` is not one of {}", options.join(", ")))
            }
            InputError::UnknownPart(_) | InputError::Ambiguous { .. } => None,
        }
    }
}

/// One option of a choice editor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldChoice {
    /// The text shown.
    pub text: String,
    /// The value it writes.
    pub value: Value,
    /// A note (a colour's name, a node's description), if any.
    pub note: Option<String>,
}

/// One part copy as the part resolver reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct PartCopyText {
    /// The copy ID (hyphenated UUID).
    pub id: String,
    /// The part number.
    pub number: String,
    /// The manufacturer's name.
    pub manufacturer: String,
    /// The part kind's word (`terminal`, `end_bracket`, `accessory`).
    pub kind: String,
    /// The description's neutral text (else its first language), `""` without one.
    pub description: String,
}

impl PartCopyText {
    /// The display form `<part number> (<manufacturer>)`.
    pub fn short(&self) -> String {
        format!("{} ({})", self.number, self.manufacturer)
    }

    /// The display form with the description, `<part number> (<manufacturer>) · <description>`; the short form
    /// when the description is empty.
    pub fn long(&self) -> String {
        if self.description.is_empty() {
            self.short()
        } else {
            format!("{} \u{b7} {}", self.short(), self.description)
        }
    }
}

fn invalid(text: crate::Text) -> InputError {
    InputError::Invalid(text)
}

/// A decimal input: `1,500` and `1.500` are ambiguous (doc 16 §6.3).
fn parse_dec3(text: &str, wrap: &dyn Fn(String) -> Value) -> Result<Value, InputError> {
    let t = text.trim();
    let b = t.as_bytes();
    // `^[0-9]{1,3}[.,][0-9]{3}$`
    let ambiguous = (5..=7).contains(&b.len())
        && matches!(b[b.len() - 4], b'.' | b',')
        && b[..b.len() - 4].iter().all(u8::is_ascii_digit)
        && b[b.len() - 3..].iter().all(u8::is_ascii_digit);
    if ambiguous {
        let int: String = t[..t.len() - 4].to_owned();
        let frac: String = t[t.len() - 3..].to_owned();
        let decimal = Dec3::parse_lenient(&format!("{int}.{frac}"))
            .map_err(|e| invalid(crate::Text::bare(e.to_string())))?;
        let thousands = Dec3::parse_lenient(&format!("{int}{frac}"))
            .map_err(|e| invalid(crate::Text::bare(e.to_string())))?;
        return Err(InputError::Ambiguous {
            choices: vec![
                (
                    format!("{} (decimal)", dec3_text(decimal)),
                    wrap(dec3_text(decimal)),
                ),
                (
                    format!("{} (thousands)", dec3_text(thousands)),
                    wrap(dec3_text(thousands)),
                ),
            ],
        });
    }
    Dec3::parse_lenient(t)
        .map(|d| wrap(dec3_text(d)))
        .map_err(|_| invalid(text!("refusal-input-decimal", t)))
}

/// A cross-section as Properties shows it, or with a space before the gauge: `0.75`, `0.75 mm²`, `AWG18`, `AWG 18`,
/// `18 AWG`, `AWG2/0`, `250 kcmil`.
fn parse_csa(text: &str) -> Result<Value, InputError> {
    let t = text.trim();
    let awg = |n: &str| -> Result<Value, InputError> {
        let n = n.trim();
        let value = if let Some(zeros) = n.strip_suffix("/0") {
            let k: i64 = zeros
                .parse()
                .map_err(|_| invalid(text!("refusal-input-awg", n)))?;
            if !(1..=4).contains(&k) {
                return Err(invalid(text!("refusal-input-awg", n)));
            }
            1 - k
        } else {
            let k: i64 = n.parse().map_err(|_| invalid(text!("refusal-input-awg", n)))?;
            if !(1..=40).contains(&k) {
                return Err(invalid(text!("refusal-input-awg", n)));
            }
            k
        };
        Ok(serde_json::json!({ "awg": value }))
    };
    let lower = t.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("awg") {
        return awg(rest);
    }
    if let Some(rest) = lower.strip_suffix("awg") {
        return awg(rest);
    }
    if let Some(rest) = lower.strip_suffix("kcmil") {
        return parse_dec3(rest, &|d| serde_json::json!({ "kcmil": d }));
    }
    let number = t
        .strip_suffix("mm²")
        .or_else(|| t.strip_suffix("mm2"))
        .unwrap_or(t);
    parse_dec3(number, &Value::String)
}

/// A length typed as [`parse_mm`](crate::output::parse_mm) reads it, as whole nanometres.
fn parse_nm(text: &str) -> Result<Value, InputError> {
    crate::output::parse_mm(text)
        .and_then(|nm| i64::try_from(nm).ok())
        .map(Value::from)
        .ok_or_else(|| invalid(text!("refusal-input-mm", text.trim())))
}

/// One or two colour codes separated by space, `/` or nothing (`GNYE`).
fn parse_colour(text: &str, company: &[(String, String)]) -> Result<Value, InputError> {
    let t = text.trim();
    let known = |c: &str| c.parse::<IecColour>().is_ok() || company.iter().any(|(k, _)| k == c);
    let accepted = || {
        let mut all: Vec<String> = IecColour::ALL.iter().map(|c| c.as_str().to_owned()).collect();
        all.extend(company.iter().map(|(c, _)| c.clone()));
        all.join(", ")
    };
    let unknown = |code: &str| {
        let alias = ew_import_tables::COLOUR_ALIASES
            .iter()
            .find(|(a, _)| a.eq_ignore_ascii_case(code))
            .map(|(_, iec)| iec.as_str());
        match alias {
            Some(iec) => invalid(text!(
                "refusal-input-colour-alias",
                code,
                iec,
                iec_name(iec),
                accepted()
            )),
            None => invalid(text!("refusal-input-colour", code, accepted())),
        }
    };
    let mut tokens: Vec<String> = t
        .split(|c: char| c.is_whitespace() || c == '/')
        .filter(|s| !s.is_empty())
        .map(str::to_uppercase)
        .collect();
    if tokens.is_empty() {
        return Err(invalid(text!("refusal-input-colour-none")));
    }
    if tokens.len() == 1 && !known(&tokens[0]) {
        // Without a separator: split into two known codes (`GNYE`, `DBUWH`).
        let one = tokens[0].clone();
        let split = (1..one.len())
            .filter(|&k| one.is_char_boundary(k))
            .find(|&k| known(&one[..k]) && known(&one[k..]));
        match split {
            Some(k) => tokens = vec![one[..k].to_owned(), one[k..].to_owned()],
            None => {
                let first = if one.len() >= 2 && one.is_char_boundary(2) && known(&one[..2]) {
                    one[2..].to_owned()
                } else {
                    one.clone()
                };
                return Err(unknown(if first.is_empty() { &one } else { &first }));
            }
        }
    }
    if tokens.len() > 2 {
        return Err(invalid(text!("refusal-input-colour-count")));
    }
    for code in &tokens {
        if !known(code) {
            return Err(unknown(code));
        }
    }
    Ok(Value::Array(tokens.into_iter().map(Value::String).collect()))
}

/// A parsed value without notes.
fn plain(value: Value) -> Result<ParsedInput, InputError> {
    Ok(ParsedInput {
        value,
        notes: Vec::new(),
    })
}

/// Parses `text` as a value of type `ty`. `member` is the field's last path member (a slot's aspect).
pub(crate) fn parse_input(
    cx: &dyn ParseCx,
    ty: &FieldType,
    member: &str,
    text: &str,
) -> Result<ParsedInput, InputError> {
    // A scale is typed as it shows ("1:5").
    if matches!(ty, FieldType::Json)
        && let Some(scale) = scale_input(text)
    {
        return scale.and_then(plain);
    }
    // A rectangle is typed as it shows ("20, 15 mm · 60 × 40 mm").
    if matches!(ty, FieldType::Json)
        && let Some(rect) = rect_input(text)
    {
        return rect.and_then(plain);
    }
    match ty {
        FieldType::Optional(inner) => {
            if text.trim().is_empty() {
                return plain(Value::Null);
            }
            parse_input(cx, inner, member, text)
        }
        FieldType::Text => {
            if text.chars().any(char::is_control) {
                return Err(invalid(text!("refusal-input-text-control")));
            }
            plain(Value::String(text.to_owned()))
        }
        FieldType::Ident => {
            let ident = Ident::from_input(text).map_err(|e| invalid(crate::ident_error_text(&e)))?;
            let notes = ident_notes(ident.as_str());
            Ok(ParsedInput {
                value: Value::String(ident.as_str().to_owned()),
                notes,
            })
        }
        FieldType::MlText => {
            if text.chars().any(char::is_control) {
                return Err(invalid(text!("refusal-input-text-control")));
            }
            if text.is_empty() {
                plain(Value::Null)
            } else {
                plain(Value::String(text.to_owned()))
            }
        }
        FieldType::Bool => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => plain(Value::Bool(true)),
            "false" | "no" | "0" => plain(Value::Bool(false)),
            _ => Err(invalid(text!("refusal-input-yes-no", text.trim()))),
        },
        FieldType::Int { min, max } => parse_int(text, *min, *max),
        FieldType::Dec3 => parse_dec3(text, &Value::String).map(|value| ParsedInput {
            value,
            notes: Vec::new(),
        }),
        FieldType::Quantity => parse_quantity(text),
        FieldType::Csa => parse_csa(text).map(|value| ParsedInput {
            value,
            notes: Vec::new(),
        }),
        FieldType::Nm => parse_nm(text).map(|value| ParsedInput {
            value,
            notes: Vec::new(),
        }),
        FieldType::Date => parse_date(cx, text),
        FieldType::Colour => parse_colour(text, &cx.company_codes()).map(|value| ParsedInput {
            value,
            notes: Vec::new(),
        }),
        FieldType::Enum { options } => parse_enum(options, text),
        FieldType::Link { kind } if kind == "part" => resolve_part(cx, text, None).map(|copy| ParsedInput {
            value: serde_json::json!({ "copy": copy }),
            notes: Vec::new(),
        }),
        FieldType::Link { kind } if kind == "form" => parse_form(cx, text),
        FieldType::Slot => parse_slot(cx, member, text),
        FieldType::Uuid | FieldType::Link { .. } => Err(invalid(text!("refusal-input-shown"))),
        FieldType::List(_) if ty.is_part_uses() && !text.trim_start().starts_with('[') => {
            parse_part_uses(cx, text)
        }
        FieldType::List(_) | FieldType::Group(_) | FieldType::Json => ew_schema::parse_value(text)
            .map(|value| ParsedInput {
                value,
                notes: Vec::new(),
            })
            .map_err(|e| invalid(text!("refusal-input-json", crate::schema_error_text(&e)))),
    }
}

/// A whole number within the schema's bounds.
fn parse_int(text: &str, min: Option<i64>, max: Option<i64>) -> Result<ParsedInput, InputError> {
    let t = text.trim();
    let n: i64 = t.parse().map_err(|_| invalid(text!("refusal-input-whole", t)))?;
    if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) {
        return Err(invalid(text!(
            "refusal-input-range",
            n.to_string(),
            min.map_or_else(String::new, |m| m.to_string()),
            max.map_or_else(String::new, |m| m.to_string())
        )));
    }
    plain(Value::from(n))
}

/// A number with its unit.
fn parse_quantity(text: &str) -> Result<ParsedInput, InputError> {
    let t = text.trim();
    // The unit is the longest suffix that names one (its persisted spelling or its symbol); the number before it.
    let names: Vec<(String, String)> = ew_schema::Unit::ALL
        .iter()
        .filter_map(|u| u.spelling().map(|p| (p, u.symbol())))
        .flat_map(|(p, s)| [(p.clone(), p.clone()), (p, s.to_owned())])
        .collect();
    let found = names.iter().fold(None::<(&str, &str)>, |best, (p, s)| {
        if t.ends_with(s.as_str()) && best.is_none_or(|(_, b)| s.len() > b.len()) {
            Some((p, s))
        } else {
            best
        }
    });
    let Some((unit, spelling)) = found else {
        // "24 V DC", as a panel builder writes a control voltage: DC or AC is no part of the value.
        let current = ["DC", "AC"].iter().any(|w| {
            t.len() > w.len()
                && t.get(t.len() - w.len()..)
                    .is_some_and(|end| end.eq_ignore_ascii_case(w))
        });
        if current {
            return Err(invalid(text!("refusal-input-ac-dc", t)));
        }
        // The units by their symbols.
        let units: Vec<&str> = ew_schema::Unit::ALL.iter().map(ew_schema::Unit::symbol).collect();
        return Err(invalid(text!("refusal-input-no-unit", t, units.join(", "))));
    };
    let number = t[..t.len() - spelling.len()].trim();
    parse_dec3(number, &|d| serde_json::json!({ "value": d, "unit": unit })).map(|value| ParsedInput {
        value,
        notes: Vec::new(),
    })
}

/// A date, typed as the project shows it, stored ISO.
fn parse_date(cx: &dyn ParseCx, text: &str) -> Result<ParsedInput, InputError> {
    let format = cx.date_format();
    match format.parse(text) {
        Ok(d) => plain(Value::String(d.to_string())),
        Err(_) => Err(invalid(text!(
            "refusal-input-date",
            text.trim(),
            format.example()
        ))),
    }
}

/// One of the closed list's words, matched without regard to case.
fn parse_enum(options: &[String], text: &str) -> Result<ParsedInput, InputError> {
    let t = text.trim();
    options
        .iter()
        .find(|o| o.eq_ignore_ascii_case(t))
        .map(|o| ParsedInput {
            value: Value::String(o.clone()),
            notes: Vec::new(),
        })
        .ok_or_else(|| InputError::NotOneOf {
            typed: t.to_owned(),
            options: options.to_vec(),
        })
}

/// A page's or a report block's form by its name, as the row's choices word it, or by its copy ID.
fn parse_form(cx: &dyn ParseCx, text: &str) -> Result<ParsedInput, InputError> {
    let t = text.trim();
    let forms = cx.frame_forms();
    match forms
        .iter()
        .find(|(id, name)| name.to_string().eq_ignore_ascii_case(t) || id == t)
    {
        Some((id, _)) => Ok(ParsedInput {
            value: Value::String(id.clone()),
            notes: Vec::new(),
        }),
        None => {
            // The forms' names joined by "; ", each in the reader's language.
            let names = forms.into_iter().rev().fold(None, |tail, (_, name)| {
                Some(match tail {
                    None => crate::TextArg::from(name),
                    Some(tail) => ew_commands::text!("refusal-words-list", name, tail).into(),
                })
            });
            Err(invalid(text!(
                "refusal-input-form",
                t,
                names.unwrap_or_else(|| "".into())
            )))
        }
    }
}

/// A structure slot: empty (inherit), `none`, or a node by its path, its ID or an alias.
fn parse_slot(cx: &dyn ParseCx, member: &str, text: &str) -> Result<ParsedInput, InputError> {
    let t = text.trim();
    if t.is_empty() {
        return plain(Value::Null);
    }
    if t.eq_ignore_ascii_case("none") {
        return plain(Value::String("none".into()));
    }
    let nodes = cx.slot_nodes(member);
    if let Some((id, _, _)) = nodes.iter().find(|(_, path, _)| path == t) {
        return plain(Value::String(id.clone()));
    }
    if let Some((id, _, _)) = nodes.iter().find(|(id, _, _)| id == t) {
        return plain(Value::String(id.clone()));
    }
    match nodes
        .iter()
        .find(|(_, _, aliases)| aliases.iter().any(|a| a == t))
    {
        Some((id, _, _)) => Ok(ParsedInput {
            value: Value::String(id.clone()),
            notes: vec![format!("resolved through alias '{t}'")],
        }),
        None => Err(invalid(text!("refusal-input-no-node", t))),
    }
}

/// The copy ID of the part typed as `text`: `id:<uuid>`, a part number, the display form `<part number>
/// (<manufacturer>)`, or the long display form `<part number> (<manufacturer>) · <description>`,
/// matched without regard to case, among the copies of the `kinds` given (every kind for `None`). Two or more matches
/// are [`InputError::Ambiguous`] with every match as a choice (its long form, the value `{"copy": <id>}`).
pub(crate) fn resolve_part(
    cx: &dyn ParseCx,
    text: &str,
    kinds: Option<&[&str]>,
) -> Result<String, InputError> {
    match find_part(cx, text, kinds) {
        Ok(id) => Ok(id),
        Err(PartMiss::Unknown(t)) => Err(InputError::UnknownPart(t)),
        Err(PartMiss::Invalid(m)) => Err(invalid(m)),
        Err(PartMiss::Ambiguous(hits)) => Err(InputError::Ambiguous {
            choices: hits
                .into_iter()
                .map(|c| (c.long(), serde_json::json!({ "copy": c.id })))
                .collect(),
        }),
    }
}

/// Why a part text did not resolve to one copy.
pub(crate) enum PartMiss {
    /// No copy matches (the text as typed, trimmed).
    Unknown(String),
    /// A malformed `id:` text.
    Invalid(crate::Text),
    /// Two or more copies match, in the order of [`part_order`].
    Ambiguous(Vec<PartCopyText>),
}

/// The order of part choices: natural part number, then manufacturer, description, ID.
pub(crate) fn part_order(a: &PartCopyText, b: &PartCopyText) -> std::cmp::Ordering {
    ew_mltext::natural_cmp(&a.number, &b.number)
        .then_with(|| ew_mltext::natural_cmp(&a.manufacturer, &b.manufacturer))
        .then_with(|| ew_mltext::natural_cmp(&a.description, &b.description))
        .then_with(|| a.id.cmp(&b.id))
}

/// [`resolve_part`]'s matching, with the matches of an ambiguous text.
pub(crate) fn find_part(cx: &dyn ParseCx, text: &str, kinds: Option<&[&str]>) -> Result<String, PartMiss> {
    let t = text.trim();
    let copies: Vec<PartCopyText> = cx
        .part_copies()
        .into_iter()
        .chain(cx.library_parts())
        .filter(|c| kinds.is_none_or(|k| k.contains(&c.kind.as_str())))
        .collect();
    if let Some(id) = t.strip_prefix("id:") {
        return match copies.iter().find(|c| c.id == id) {
            Some(c) => Ok(c.id.clone()),
            None => Err(PartMiss::Invalid(text!("refusal-input-no-part-copy", id))),
        };
    }
    let mut hits: Vec<&PartCopyText> = copies
        .iter()
        .filter(|c| c.number.eq_ignore_ascii_case(t))
        .collect();
    // The long display form `<part number> (<manufacturer>) · <description>` parses back.
    if hits.is_empty()
        && let Some((short, description)) = t.split_once(" \u{b7} ")
    {
        let description = description.trim();
        hits = copies
            .iter()
            .filter(|c| {
                c.short().eq_ignore_ascii_case(short.trim())
                    && c.description.eq_ignore_ascii_case(description)
            })
            .collect();
    }
    // The display form `<part number> (<manufacturer>)` parses back.
    if hits.is_empty()
        && let Some((pn, rest)) = t.rsplit_once(" (")
        && let Some(mfr) = rest.strip_suffix(')')
    {
        hits = copies
            .iter()
            .filter(|c| c.number.eq_ignore_ascii_case(pn) && c.manufacturer.eq_ignore_ascii_case(mfr))
            .collect();
    }
    match hits.as_slice() {
        [one] => Ok(one.id.clone()),
        [] => Err(PartMiss::Unknown(t.to_owned())),
        _ => {
            let mut all: Vec<PartCopyText> = hits.into_iter().cloned().collect();
            all.sort_by(part_order);
            Err(PartMiss::Ambiguous(all))
        }
    }
}

/// A list of part uses typed as text (the minimal part chooser): part numbers separated by commas, each optionally
/// followed by `× <quantity>` (the row's display form, `533356 × 1`), quantity 1 when left out; an empty text is the
/// empty list. A text starting with `[` is the list's strict JSON instead (`parse_input`). An ambiguous item
/// makes the whole list ambiguous: one choice per match of the first ambiguous item, each the whole list.
fn parse_part_uses(cx: &dyn ParseCx, text: &str) -> Result<ParsedInput, InputError> {
    let mut uses = Vec::new();
    let mut ambiguous: Option<(usize, Vec<PartCopyText>)> = None;
    for item in text.split(',').map(str::trim).filter(|i| !i.is_empty()) {
        let (part, qty) = match item.rsplit_once('\u{d7}') {
            Some((p, q)) => {
                let q = q.trim();
                let qty = Dec3::parse_lenient(q)
                    .ok()
                    .filter(|d| d.0 > 0)
                    .ok_or_else(|| invalid(text!("refusal-input-quantity", q)))?;
                (p.trim(), dec3_text(qty))
            }
            None => (item, "1".to_owned()),
        };
        let copy = match find_part(cx, part, None) {
            Ok(copy) => copy,
            Err(PartMiss::Unknown(t)) => return Err(InputError::UnknownPart(t)),
            Err(PartMiss::Invalid(m)) => return Err(invalid(m)),
            Err(PartMiss::Ambiguous(hits)) => {
                if ambiguous.is_none() {
                    ambiguous = Some((uses.len(), hits.clone()));
                }
                hits[0].id.clone()
            }
        };
        uses.push(serde_json::json!({ "part": { "copy": copy }, "qty": qty }));
    }
    if let Some((at, hits)) = ambiguous {
        let choices = hits
            .into_iter()
            .map(|c| {
                let mut list = uses.clone();
                list[at]["part"]["copy"] = Value::String(c.id.clone());
                (c.long(), Value::Array(list))
            })
            .collect();
        return Err(InputError::Ambiguous { choices });
    }
    Ok(ParsedInput {
        value: Value::Array(uses),
        notes: Vec::new(),
    })
}

fn ident_notes(text: &str) -> Vec<String> {
    use ew_mltext::IdentFinding;
    ident_findings(text)
        .into_iter()
        .map(|f| match f {
            IdentFinding::MixedScript => "mixed scripts".to_owned(),
            IdentFinding::Confusable { ch, prototype, .. } => format!(
                "{} {ch} (U+{:04X}) looks like {prototype}",
                script_name(ch),
                u32::from(ch)
            ),
            other => format!("{other:?}"),
        })
        .collect()
}

/// The script of a confusable character, for the note text.
fn script_name(ch: char) -> &'static str {
    match u32::from(ch) {
        0x0370..=0x03FF => "Greek",
        0x0400..=0x04FF => "Cyrillic",
        0x0530..=0x058F => "Armenian",
        _ => "character",
    }
}

/// The options of a choice editor matching the case-folded `filter`: empty for other types.
pub(crate) fn choices(cx: &dyn ParseCx, ty: &FieldType, member: &str, filter: &str) -> Vec<FieldChoice> {
    let filter = filter.to_lowercase();
    let keep = |c: &FieldChoice| filter.is_empty() || c.text.to_lowercase().contains(&filter);
    let all: Vec<FieldChoice> = match ty.inner() {
        FieldType::Enum { options } => options
            .iter()
            .map(|o| FieldChoice {
                text: o.clone(),
                value: Value::String(o.clone()),
                note: None,
            })
            .collect(),
        FieldType::Colour => {
            let mut out: Vec<FieldChoice> = IEC_NAMES
                .iter()
                .map(|(code, name)| FieldChoice {
                    text: (*code).to_owned(),
                    value: Value::Array(vec![Value::String((*code).to_owned())]),
                    note: Some((*name).to_owned()),
                })
                .collect();
            out.extend(cx.company_codes().into_iter().map(|(code, base)| FieldChoice {
                text: code.clone(),
                value: Value::Array(vec![Value::String(code)]),
                note: Some(format!("company code on {base}")),
            }));
            out
        }
        // A fluid line's tube part is a part that is a tube (not a valve).
        FieldType::Link { kind } if kind == "part" => {
            let mut all = part_choices(cx, |id| serde_json::json!({ "copy": id }));
            if member == TUBE_PART {
                let tubes: BTreeSet<String> = cx
                    .part_copies()
                    .into_iter()
                    .chain(cx.library_parts())
                    .filter(|c| c.kind == "tube")
                    .map(|c| c.id)
                    .collect();
                all.retain(|c| c.value["copy"].as_str().is_some_and(|id| tubes.contains(id)));
            }
            all
        }
        // A page's or a report block's form: the project's forms a page prints on, by name.
        FieldType::Link { kind } if kind == "form" => cx
            .frame_forms()
            .into_iter()
            .map(|(id, name)| FieldChoice {
                text: name.to_string(),
                value: Value::String(id),
                note: None,
            })
            .collect(),
        // The minimal part chooser: one part use of quantity 1 replaces the list.
        FieldType::List(_) if ty.is_part_uses() => part_choices(
            cx,
            |id| serde_json::json!([{ "part": { "copy": id }, "qty": "1" }]),
        ),
        FieldType::Slot => {
            let mut nodes = cx.slot_nodes(member);
            nodes.sort_by(|a, b| ew_mltext::natural_cmp(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
            nodes
                .into_iter()
                .map(|(id, path, _)| FieldChoice {
                    text: path,
                    value: Value::String(id),
                    note: None,
                })
                .collect()
        }
        _ => Vec::new(),
    };
    all.into_iter().filter(keep).collect()
}

/// The project's part copies and the libraries' parts the project does not hold as options,
/// `<part number> (<manufacturer>)`, each writing `value(id)`; in the place chooser's one order, (maker, part number,
/// copy), embedded and library parts alike, so that embedding a part does not move it (item 4).
fn part_choices(cx: &dyn ParseCx, value: impl Fn(&str) -> Value) -> Vec<FieldChoice> {
    let mut copies = cx.part_copies();
    copies.extend(cx.library_parts());
    copies.sort_by(|a, b| (&a.manufacturer, &a.number, &a.id).cmp(&(&b.manufacturer, &b.number, &b.id)));
    copies
        .into_iter()
        .map(|c| FieldChoice {
            text: c.short(),
            value: value(&c.id),
            note: None,
        })
        .collect()
}
