// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-project text styles (ADR-0032, FR-SYM-013): the member `ProjectSettings.text_styles`, the built-in
//! table it overrides and the effective table every renderer and EW-CORE-061 read.
//!
//! A project stores its text styles as **overrides** of the built-in table of doc 20 §17.5
//! ([`BUILTIN_TEXT_STYLES`]): a JSON object keyed by style key, each value `{"size": <em height in nm>,
//! "weight": "regular" | "bold"}` (weight optional) plus unknown members kept verbatim. An absent or empty
//! member means no overrides, so every file written before ADR-0032 renders as before; nothing writes the
//! member into a file that lacks it. A key in both takes the override's size and, when given, its weight; a key
//! only in the overrides extends the table ([`effective_text_styles`]).
//!
//! ```
//! use ew_schema::{ProjectSettings, effective_text_styles, new_project_text_styles};
//! use ew_units::Nm;
//!
//! let plain = effective_text_styles(&ProjectSettings::default());
//! let marker = plain.iter().find(|s| s.key.as_str() == "marker").unwrap();
//! assert_eq!(marker.em, Nm(1_322_917)); // 5 QET px
//!
//! let settings = ProjectSettings { text_styles: new_project_text_styles(), ..ProjectSettings::default() };
//! let fresh = effective_text_styles(&settings);
//! let marker = fresh.iter().find(|s| s.key.as_str() == "marker").unwrap();
//! assert_eq!(marker.em, Nm(1_852_083)); // 7 QET px, 1.32 mm cap height
//! assert_eq!(fresh.len(), plain.len());
//! ```

use std::collections::BTreeMap;

use ew_mltext::Ident;
use ew_units::{Grid, Nm};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::{Extra, de_extra};
use crate::project::ProjectSettings;
use crate::proxy::NmS;
use crate::track::open_enum;

/// The weight of a text style (ADR-0032 decision 1).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "TextWeight")]
pub enum TextWeight {
    /// Regular.
    Regular,
    /// Bold.
    Bold,
    /// A weight of a newer format, kept verbatim; the effective table reads it as absent.
    #[serde(skip)]
    Unknown(Value),
}

/// One entry of `ProjectSettings.text_styles`: an override of a built-in style or a new style (ADR-0032).
///
/// Family, italic and alignment are not typed (the bundled face is the only family at MVP, ADR-0028; alignment
/// belongs to the slot) and stay in `extra`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TextStyleSpec {
    /// The em height in nm, 500,000 … 50,000,000 (0.5 mm … 50 mm).
    #[schemars(with = "NmS")]
    pub size: Nm,
    /// The weight; absent keeps the built-in row's weight (regular for a new style).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<TextWeight>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl TextStyleSpec {
    /// An entry of `size` without a weight.
    pub fn of_size(size: Nm) -> TextStyleSpec {
        TextStyleSpec {
            size,
            weight: None,
            extra: Extra::new(),
        }
    }
}

/// The smallest em height a text style may have: 0.5 mm (ADR-0032 decision 1).
pub const TEXT_STYLE_MIN_SIZE: Nm = Nm(500_000);

/// The largest em height a text style may have: 50 mm (ADR-0032 decision 1).
pub const TEXT_STYLE_MAX_SIZE: Nm = Nm(50_000_000);

/// The built-in text-style table of doc 20 §17.5 as `(key, size in QET px, weight)`, in table order: the base every
/// project's overrides apply to (ADR-0032 decision 4: part of the derivation profile; a change of a row is a
/// derivation-profile change). `ew_text::builtin()` holds the same rows for callers without a project, and a test in
/// `ew-scene` keeps the two equal.
pub const BUILTIN_TEXT_STYLES: &[(&str, u16, TextWeight)] = &[
    ("cover.title", 48, TextWeight::Regular),
    ("section.id", 37, TextWeight::Bold),
    ("section.subtitle", 29, TextWeight::Regular),
    ("cover.subtitle", 27, TextWeight::Regular),
    ("box.title", 21, TextWeight::Regular),
    ("cover.table", 16, TextWeight::Regular),
    ("tag.main", 13, TextWeight::Regular),
    ("text.normal", 12, TextWeight::Regular),
    ("field.label", 11, TextWeight::Regular),
    ("data.small", 9, TextWeight::Regular),
    ("pin", 8, TextWeight::Regular),
    ("xref.device", 7, TextWeight::Regular),
    ("cp.small", 5, TextWeight::Regular),
    ("marker", 5, TextWeight::Regular),
];

/// The overrides the built-in basic project of a new project writes (ADR-0032 decision 3): `cp.small` and
/// `marker` at `Grid::QET_1PX.at(7)` = 1,852,083 nm em height, a printed cap height of 1.32 mm.
pub const NEW_PROJECT_TEXT_STYLES: &[(&str, Nm)] = &[("cp.small", Nm(1_852_083)), ("marker", Nm(1_852_083))];

/// [`NEW_PROJECT_TEXT_STYLES`] as the member's value.
pub fn new_project_text_styles() -> BTreeMap<Ident, TextStyleSpec> {
    NEW_PROJECT_TEXT_STYLES
        .iter()
        .filter_map(|(key, size)| Some((key.parse().ok()?, TextStyleSpec::of_size(*size))))
        .collect()
}

/// The em height of `px` QET pixels: `round(px × 793 750 / 3)` nm (doc 20 §17.5).
pub fn qet_px_em(px: u16) -> Nm {
    Grid::QET_1PX.at(i64::from(px)).unwrap_or(Nm::ZERO)
}

/// One row of the effective text-style table (ADR-0032 decision 1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectiveStyle {
    /// The style key.
    pub key: Ident,
    /// The em height.
    pub em: Nm,
    /// The weight: [`TextWeight::Regular`] or [`TextWeight::Bold`], never `Unknown`.
    pub weight: TextWeight,
}

impl EffectiveStyle {
    /// Whether the weight is bold.
    pub fn is_bold(&self) -> bool {
        self.weight == TextWeight::Bold
    }
}

/// Whether `size` lies in [`TEXT_STYLE_MIN_SIZE`] … [`TEXT_STYLE_MAX_SIZE`].
pub fn text_style_size_ok(size: Nm) -> bool {
    (TEXT_STYLE_MIN_SIZE..=TEXT_STYLE_MAX_SIZE).contains(&size)
}

/// The effective text-style table of `settings` (ADR-0032 decision 1): the rows of [`BUILTIN_TEXT_STYLES`] in their
/// order, each with its override applied, then the extending keys in key order.
///
/// An override whose size lies outside [`TEXT_STYLE_MIN_SIZE`] … [`TEXT_STYLE_MAX_SIZE`] (a hand-edited file;
/// `project.settings.set` refuses it) is ignored: the built-in row stays, and an extending key is not added. A
/// weight of a newer format reads as absent.
pub fn effective_text_styles(settings: &ProjectSettings) -> Vec<EffectiveStyle> {
    let overrides = &settings.text_styles;
    let weight = |spec: &TextStyleSpec, base: &TextWeight| match &spec.weight {
        Some(w @ (TextWeight::Bold | TextWeight::Regular)) => w.clone(),
        _ => base.clone(),
    };
    let mut rows: Vec<EffectiveStyle> = BUILTIN_TEXT_STYLES
        .iter()
        .filter_map(|(key, px, w)| {
            let key: Ident = key.parse().ok()?;
            Some(
                match overrides.get(key.as_str()).filter(|s| text_style_size_ok(s.size)) {
                    Some(spec) => EffectiveStyle {
                        em: spec.size,
                        weight: weight(spec, w),
                        key,
                    },
                    None => EffectiveStyle {
                        em: qet_px_em(*px),
                        weight: w.clone(),
                        key,
                    },
                },
            )
        })
        .collect();
    for (key, spec) in overrides {
        if text_style_size_ok(spec.size) && !BUILTIN_TEXT_STYLES.iter().any(|(k, _, _)| *k == key.as_str()) {
            rows.push(EffectiveStyle {
                key: key.clone(),
                em: spec.size,
                weight: weight(spec, &TextWeight::Regular),
            });
        }
    }
    rows
}

/// Why a value for `text_styles` is refused ([`check_text_styles`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextStyleError {
    /// The JSON Pointer of the refused value below the member (`/tag.main/size`; empty for the whole value).
    pub pointer: String,
    /// What is wrong.
    pub message: String,
}

/// A JSON Pointer reference token (RFC 6901: `~` as `~0`, `/` as `~1`).
fn token(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Checks a whole value for `text_styles` as an editing command receives it (ADR-0032 decision 6) and reads it:
/// an object whose keys are identifiers and whose entries have an integer `size` in [`TEXT_STYLE_MIN_SIZE`] …
/// [`TEXT_STYLE_MAX_SIZE`] and, when present, a `weight` of `regular` or `bold`. Unknown members of an entry are
/// kept.
///
/// # Errors
///
/// The first refused value in key order, with its pointer below the member.
///
/// ```
/// use serde_json::json;
///
/// assert!(ew_schema::check_text_styles(&json!({"tag.main": {"size": 4_000_000}})).is_ok());
/// let e = ew_schema::check_text_styles(&json!({"tag.main": {"size": 100}})).unwrap_err();
/// assert_eq!(e.pointer, "/tag.main/size");
/// ```
pub fn check_text_styles(value: &Value) -> Result<BTreeMap<Ident, TextStyleSpec>, TextStyleError> {
    let err = |pointer: String, message: String| TextStyleError { pointer, message };
    let Value::Object(map) = value else {
        return Err(err(
            String::new(),
            "text styles are an object of style key to {\"size\": <em height in nm>, \"weight\": …}"
                .to_owned(),
        ));
    };
    let mut out = BTreeMap::new();
    for (key, entry) in map {
        let at = format!("/{}", token(key));
        let ident: Ident = key.parse().map_err(|e| {
            err(
                at.clone(),
                format!("the style key {key:?} is not an identifier: {e}"),
            )
        })?;
        let Value::Object(members) = entry else {
            return Err(err(
                at,
                format!("the style {key} is not an object {{\"size\": …, \"weight\": …}}"),
            ));
        };
        let size = members.get("size").and_then(Value::as_i64);
        let Some(size) = size.map(Nm) else {
            return Err(err(
                format!("{at}/size"),
                format!("the style {key} needs an integer size: the em height in nm"),
            ));
        };
        if !text_style_size_ok(size) {
            return Err(err(
                format!("{at}/size"),
                format!(
                    "the size {} nm of the style {key} lies outside {} … {} nm (0.5 mm … 50 mm em height)",
                    size.0, TEXT_STYLE_MIN_SIZE.0, TEXT_STYLE_MAX_SIZE.0
                ),
            ));
        }
        if let Some(w) = members.get("weight")
            && !matches!(w.as_str(), Some("regular" | "bold"))
        {
            return Err(err(
                format!("{at}/weight"),
                format!("the weight {w} of the style {key} is unknown; weights are \"regular\" and \"bold\""),
            ));
        }
        let spec: TextStyleSpec =
            serde_json::from_value(entry.clone()).map_err(|e| err(at.clone(), e.to_string()))?;
        out.insert(ident, spec);
    }
    Ok(out)
}

open_enum!(TextWeight);

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn builtin_rows_are_unique_and_the_new_project_rows_are_7_px() {
        let mut keys: Vec<&str> = BUILTIN_TEXT_STYLES.iter().map(|(k, _, _)| *k).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), BUILTIN_TEXT_STYLES.len());
        for (key, size) in NEW_PROJECT_TEXT_STYLES {
            assert_eq!(*size, qet_px_em(7), "{key}");
            assert!(BUILTIN_TEXT_STYLES.iter().any(|(k, _, _)| k == key), "{key}");
        }
        assert_eq!(new_project_text_styles().len(), NEW_PROJECT_TEXT_STYLES.len());
    }

    #[test]
    fn overrides_replace_and_extend() {
        let mut settings = ProjectSettings::default();
        let base = effective_text_styles(&settings);
        assert_eq!(base.len(), BUILTIN_TEXT_STYLES.len());
        settings.text_styles = check_text_styles(&json!({
            "tag.main": {"size": 4_000_000, "weight": "bold"},
            "note.big": {"size": 6_000_000},
            "a.first": {"size": 1_000_000, "weight": "bold"},
            "section.id": {"size": 9_000_000}
        }))
        .unwrap();
        let rows = effective_text_styles(&settings);
        let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
        let mut want: Vec<&str> = BUILTIN_TEXT_STYLES.iter().map(|(k, _, _)| *k).collect();
        want.extend(["a.first", "note.big"]);
        assert_eq!(keys, want, "base order, then extending keys in key order");
        let get = |k: &str| rows.iter().find(|r| r.key.as_str() == k).unwrap();
        assert_eq!(
            (get("tag.main").em, get("tag.main").is_bold()),
            (Nm(4_000_000), true)
        );
        // An override without a weight keeps the base row's weight.
        assert_eq!(
            (get("section.id").em, get("section.id").is_bold()),
            (Nm(9_000_000), true)
        );
        assert_eq!(
            (get("note.big").em, get("note.big").is_bold()),
            (Nm(6_000_000), false)
        );
        assert!(get("a.first").is_bold());
        assert_eq!(get("text.normal").em, Nm(3_175_000));
    }

    #[test]
    fn out_of_range_sizes_and_unknown_weights_in_a_file_are_ignored() {
        let mut settings = ProjectSettings::default();
        let mut tiny = TextStyleSpec::of_size(Nm(100));
        tiny.weight = Some(TextWeight::Bold);
        settings.text_styles.insert("marker".parse().unwrap(), tiny);
        settings
            .text_styles
            .insert("note.tiny".parse().unwrap(), TextStyleSpec::of_size(Nm(-5)));
        let mut odd = TextStyleSpec::of_size(Nm(2_000_000));
        odd.weight = Some(TextWeight::Unknown(json!("black")));
        settings.text_styles.insert("section.id".parse().unwrap(), odd);
        let rows = effective_text_styles(&settings);
        assert_eq!(rows.len(), BUILTIN_TEXT_STYLES.len());
        let get = |k: &str| rows.iter().find(|r| r.key.as_str() == k).unwrap();
        assert_eq!((get("marker").em, get("marker").is_bold()), (qet_px_em(5), false));
        assert_eq!(
            (get("section.id").em, get("section.id").weight.clone()),
            (Nm(2_000_000), TextWeight::Bold)
        );
    }

    #[test]
    fn check_refuses_at_the_member_path() {
        let e = |v: Value| check_text_styles(&v).unwrap_err();
        assert_eq!(e(json!([])).pointer, "");
        assert_eq!(e(json!({" tag": {"size": 2_000_000}})).pointer, "/ tag");
        assert_eq!(e(json!({"a/b": {"size": 1}})).pointer, "/a~1b/size");
        assert_eq!(e(json!({"tag.main": 5})).pointer, "/tag.main");
        assert_eq!(e(json!({"tag.main": {}})).pointer, "/tag.main/size");
        assert_eq!(e(json!({"tag.main": {"size": "4"}})).pointer, "/tag.main/size");
        assert_eq!(
            e(json!({"tag.main": {"size": 499_999}})).pointer,
            "/tag.main/size"
        );
        assert_eq!(
            e(json!({"tag.main": {"size": 50_000_001}})).pointer,
            "/tag.main/size"
        );
        let w = e(json!({"tag.main": {"size": 4_000_000, "weight": "black"}}));
        assert_eq!(w.pointer, "/tag.main/weight");
        assert!(w.message.contains("\"black\""), "{}", w.message);
        let ok = check_text_styles(&json!({
            "tag.main": {"size": 500_000, "family": "x"},
            "marker": {"size": 50_000_000, "weight": "regular"}
        }))
        .unwrap();
        assert_eq!(ok["tag.main"].extra["family"], json!("x"));
        assert_eq!(ok["marker"].weight, Some(TextWeight::Regular));
    }

    #[test]
    fn entries_round_trip_with_unknown_members_and_weights() {
        for line in [
            r#"{"size":1852083}"#,
            r#"{"size":1852083,"weight":"bold"}"#,
            r#"{"size":1852083,"weight":"black"}"#,
            r#"{"size":1852083,"weight":{"x":1},"family":"mono"}"#,
        ] {
            let spec: TextStyleSpec = serde_json::from_str(line).unwrap();
            assert_eq!(serde_json::to_string(&spec).unwrap(), line);
        }
    }
}
