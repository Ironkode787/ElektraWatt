// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A field's value in words (see the module documentation of `fields`): the text a row shows for its type.

#[allow(clippy::wildcard_imports)]
use super::*;

/// The English names of the IEC 60757 codes, in IEC 60757 order.
pub(super) const IEC_NAMES: [(&str, &str); 14] = [
    ("BK", "black"),
    ("BN", "brown"),
    ("RD", "red"),
    ("OG", "orange"),
    ("YE", "yellow"),
    ("GN", "green"),
    ("BU", "blue"),
    ("VT", "violet"),
    ("GY", "grey"),
    ("WH", "white"),
    ("PK", "pink"),
    ("GD", "gold"),
    ("TQ", "turquoise"),
    ("SR", "silver"),
];

/// The English name of an IEC 60757 code.
pub(crate) fn iec_name(code: &str) -> &'static str {
    IEC_NAMES.iter().find(|(c, _)| *c == code).map_or("", |(_, n)| n)
}

/// No context.
struct NoCx;

impl DisplayCx for NoCx {}

/// The display text of `value` as a value of type `ty`: total over every type and value.
pub fn format_field_value(ty: &FieldType, value: &Value) -> String {
    format_with(&NoCx, ty, value)
}

/// The text of a multi-language value: the neutral text, else the source language's (the first language without a
/// source), with ` [+n]` for the other languages.
fn mltext_text(cx: &dyn DisplayCx, value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(m) if !m.is_empty() => {
            let source = cx.source_language();
            let (_, text) = source
                .as_deref()
                .and_then(|s| m.get_key_value(s))
                .or_else(|| m.iter().next())
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
                .unwrap_or_default();
            if m.len() > 1 {
                format!("{text} [+{}]", m.len() - 1)
            } else {
                text
            }
        }
        other => compact_json(other),
    }
}

/// Compact canonical JSON, cut at 120 characters with `…`.
pub(crate) fn compact_json(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() > 120 {
        let cut: String = text.chars().take(119).collect();
        format!("{cut}…")
    } else {
        text
    }
}

/// A cross-section as the lists print it (`ew_derive::csa_with_unit`: `0.75 mm²`, `AWG18`, `AWG2/0`, `250 kcmil`); a
/// value of a newer format as compact JSON.
fn csa_text(value: &Value) -> String {
    serde_json::from_value::<ew_schema::Csa>(value.clone())
        .ok()
        .and_then(|c| ew_derive::csa_with_unit(&c))
        .unwrap_or_else(|| compact_json(value))
}

/// The typographic symbol of a persisted unit spelling; a spelling of a newer format as it is.
fn unit_text(unit: &str) -> &str {
    ew_schema::Unit::named(unit).map_or(unit, |u| u.symbol())
}

/// [`format_field_value`] with a display context.
pub(crate) fn format_with(cx: &dyn DisplayCx, ty: &FieldType, value: &Value) -> String {
    if value.is_null() {
        return String::new();
    }
    match ty {
        FieldType::Optional(inner) => format_with(cx, inner, value),
        FieldType::Text | FieldType::Ident => match value {
            Value::String(s) => s.clone(),
            other => compact_json(other),
        },
        FieldType::MlText => mltext_text(cx, value),
        FieldType::Bool => match value {
            Value::Bool(true) => "yes".into(),
            Value::Bool(false) => "no".into(),
            other => compact_json(other),
        },
        FieldType::Int { .. } => match value {
            Value::Number(n) => n.to_string(),
            other => compact_json(other),
        },
        FieldType::Dec3 => match value {
            Value::String(s) => s.clone(),
            other => compact_json(other),
        },
        FieldType::Quantity => match (
            value.get("value").and_then(Value::as_str),
            value.get("unit").and_then(Value::as_str),
        ) {
            (Some(v), Some(u)) => format!("{v} {}", unit_text(u)),
            _ => compact_json(value),
        },
        FieldType::Csa => csa_text(value),
        FieldType::Nm => value
            .as_i64()
            .map_or_else(|| compact_json(value), ew_derive::mm_padded),
        FieldType::Date => match value.as_str().map(str::parse::<ew_schema::Date>) {
            Some(Ok(d)) => cx.date_format().format(d),
            _ => value.as_str().map_or_else(|| compact_json(value), str::to_owned),
        },
        FieldType::Colour => serde_json::from_value::<ew_schema::ColourSpec>(value.clone())
            .map_or_else(|_| compact_json(value), |c| ew_derive::colour_text(&c)),
        FieldType::Enum { .. } => match value {
            Value::String(s) => s.clone(),
            other => compact_json(other),
        },
        FieldType::Link { kind } if kind == "part" => {
            let copy = value.get("copy").and_then(Value::as_str);
            match copy {
                Some(id) => cx.part_text(id).unwrap_or_else(|| format!("id:{id}")),
                None => compact_json(value),
            }
        }
        FieldType::Link { .. } | FieldType::Uuid => match value {
            Value::String(id) => cx.record_label(id).unwrap_or_else(|| id.clone()),
            other => compact_json(other),
        },
        FieldType::Slot => match value {
            Value::String(s) if s == "none" => "none".into(),
            Value::String(id) => cx.node_text(id).unwrap_or_else(|| id.clone()),
            other => compact_json(other),
        },
        FieldType::List(item) => match value {
            Value::Array(items) => {
                let part_use = matches!(&**item, FieldType::Group(c) if c.iter().any(|n| n.name == "part")
                    && c.iter().any(|n| n.name == "qty"));
                if part_use && !items.is_empty() {
                    items
                        .iter()
                        .map(|i| {
                            let copy = i.get("part").and_then(|p| p.get("copy")).and_then(Value::as_str);
                            let number = copy
                                .and_then(|c| cx.part_number(c))
                                .or_else(|| copy.map(|c| format!("id:{c}")))
                                .unwrap_or_default();
                            let qty = i.get("qty").and_then(Value::as_str).unwrap_or("");
                            format!("{number} × {qty}")
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                } else if items.len() == 1 {
                    "1 item".into()
                } else {
                    format!("{} items", items.len())
                }
            }
            other => compact_json(other),
        },
        // A unit variant of a tagged enum (a function's `black_box`) is its word.
        FieldType::Group(_) => value.as_str().map(str::to_owned).unwrap_or_default(),
        FieldType::Json => length_source_text(cx, value)
            .or_else(|| scale_text(value))
            .or_else(|| rect_text(value))
            // A connection that is a cable's core by its cable and core (`-1W1 BN`), as the strip editor
            // names it.
            .or_else(|| cx.record_label(value.get("core")?.as_str()?))
            .unwrap_or_else(|| compact_json(value)),
    }
}

/// A page's drawing scale in the words its title block prints and its editor reads back:
/// `{"one_to": n}` as `1:n`, `not_to_scale` as `not to scale`; `None` for any other JSON value.
fn scale_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if s == "not_to_scale" => Some("not to scale".to_owned()),
        Value::Object(m) if m.len() == 1 => Some(format!("1:{}", m.get("one_to")?.as_u64()?)),
        _ => None,
    }
}

/// A cable's length source in the words its typed editor reads back: as the project's lists print it
/// ([`DisplayCx::cable_length`]); without a project `{"manual": <nm>}` as `<metres> m`, `from_part` as `from part`,
/// `missing` as is; `None` for any other JSON value.
fn length_source_text(cx: &dyn DisplayCx, value: &Value) -> Option<String> {
    if let Some(text) = serde_json::from_value::<ew_schema::LengthSource>(value.clone())
        .ok()
        .filter(|s| !matches!(s, ew_schema::LengthSource::Unknown(_)))
        .and_then(|s| cx.cable_length(&s))
    {
        return Some(text);
    }
    match value {
        Value::String(s) if s == "from_part" => Some("from part".to_owned()),
        Value::String(s) if s == "missing" => Some("missing".to_owned()),
        Value::Object(m) if m.len() == 1 => {
            let nm = m.get("manual")?.as_i64()?;
            let sign = if nm < 0 { "-" } else { "" };
            let abs = nm.unsigned_abs();
            let mut frac = format!("{:09}", abs % 1_000_000_000);
            while frac.ends_with('0') {
                frac.pop();
            }
            let whole = abs / 1_000_000_000;
            Some(if frac.is_empty() {
                format!("{sign}{whole} m")
            } else {
                format!("{sign}{whole}.{frac} m")
            })
        }
        _ => None,
    }
}

/// A rectangle (`[[x1, y1], [x2, y2]]` in nanometres, a legend's area) as its place and size in millimetres, as its
/// editor reads it back: `20.000, 15.000 mm · 60.000 × 40.000 mm`; `None` for any other JSON value.
fn rect_text(value: &Value) -> Option<String> {
    let corner = |i: usize| -> Option<(i64, i64)> {
        let p = value.get(i)?.as_array().filter(|p| p.len() == 2)?;
        Some((p[0].as_i64()?, p[1].as_i64()?))
    };
    let ((x1, y1), (x2, y2)) = (corner(0)?, corner(1)?);
    if value.as_array()?.len() != 2 || x2 < x1 || y2 < y1 {
        return None;
    }
    Some(format!(
        "{}, {} mm · {} × {} mm",
        ew_derive::mm_padded(x1),
        ew_derive::mm_padded(y1),
        ew_derive::mm_padded(x2.checked_sub(x1)?),
        ew_derive::mm_padded(y2.checked_sub(y1)?)
    ))
}

/// The canonical text of a lenient decimal.
pub(super) fn dec3_text(d: Dec3) -> String {
    d.to_string()
}
