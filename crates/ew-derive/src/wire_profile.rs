// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The colour and size profile shims of the wire-property chain (doc 03 §5.1, doc 08 §4.2).
//!
//! Until `ew-schema` types `ProjectSettings.colour_profile`, `.size_profile` and `.wire_default` (WP-S3.03),
//! they are read from `ProjectSettings.extra` with the JSON shapes of doc 03 §5.1 (record spellings:
//! `l_plus`, `dc`; `Dec3` and `Csa` as strings; `ColourSpec` as an array). A member that is present but does
//! not parse is noted ([`DeriveNote::MalformedShim`]) and treated as absent; an absent colour or size profile
//! is noted ([`DeriveNote::DefaultWireProfile`]) and the built-in defaults of the derivation profile apply.
//!
//! # Matching (`Select`, doc 08 §4.2)
//!
//! - An empty or absent member matches anything. `ptype` matches when the list holds the potential's type;
//!   `ac_dc` is equal; `u_min <= voltage <= u_max` (inclusive).
//! - A constrained field the potential does not know (no voltage, no AC/DC, no type) does not match.
//! - `discipline` and `analogue` match only when the potential has that attribute. No potential has either in
//!   this phase, so a selector that constrains one never matches.
//! - Colour rows: the row with the most constrained fields wins, ties go to the first row. Class rules: the
//!   first match wins.

use ew_schema::{
    AcDc, CircuitClass, ColourCode, ColourSpec, Csa, IecColour, PotentialType, Project, WireProps,
};
use ew_units::Dec3;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::note::DeriveNote;

/// What a selector is matched against: the final attributes of a potential (flow values).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct Attrs {
    /// The type, if known.
    pub ptype: Option<PotentialType>,
    /// The voltage (V), if known.
    pub voltage: Option<Dec3>,
    /// AC or DC, if known.
    pub ac_dc: Option<AcDc>,
}

/// `ProfileSelect` (doc 03 §5.1).
#[derive(Clone, PartialEq, Eq, Debug, Default, Deserialize)]
struct Select {
    #[serde(default)]
    ptype: Vec<PotentialType>,
    #[serde(default)]
    ac_dc: Option<AcDc>,
    #[serde(default)]
    u_min: Option<Dec3>,
    #[serde(default)]
    u_max: Option<Dec3>,
    /// Read as a value: no potential has a discipline in this phase.
    #[serde(default)]
    discipline: Option<Value>,
    #[serde(default)]
    analogue: Option<bool>,
}

impl Select {
    /// The number of constrained fields (the specificity of a colour row).
    fn specificity(&self) -> usize {
        usize::from(!self.ptype.is_empty())
            + usize::from(self.ac_dc.is_some())
            + usize::from(self.u_min.is_some())
            + usize::from(self.u_max.is_some())
            + usize::from(self.discipline.is_some())
            + usize::from(self.analogue.is_some())
    }

    fn matches(&self, a: &Attrs) -> bool {
        if !self.ptype.is_empty() && !a.ptype.as_ref().is_some_and(|t| self.ptype.contains(t)) {
            return false;
        }
        if let Some(want) = &self.ac_dc
            && a.ac_dc.as_ref() != Some(want)
        {
            return false;
        }
        if let Some(lo) = self.u_min
            && !a.voltage.is_some_and(|v| lo <= v)
        {
            return false;
        }
        if let Some(hi) = self.u_max
            && !a.voltage.is_some_and(|v| v <= hi)
        {
            return false;
        }
        // Neither attribute exists on a potential in this phase.
        self.discipline.is_none() && self.analogue.is_none()
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct ColourRowRaw {
    #[serde(default)]
    select: Select,
    colour: ColourSpec,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct ColourProfileRaw {
    #[serde(default)]
    key: String,
    #[serde(default)]
    rows: Vec<ColourRowRaw>,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct ClassRuleRaw {
    #[serde(default)]
    select: Select,
    class: CircuitClass,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct SizeRowRaw {
    class: CircuitClass,
    csa: Csa,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct SizeProfileRaw {
    #[serde(default)]
    key: String,
    #[serde(default)]
    class_rules: Vec<ClassRuleRaw>,
    #[serde(default)]
    rows: Vec<SizeRowRaw>,
}

/// The project's colour profile (`ProjectSettings.colour_profile`): its key and rows in cover-table order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct ColourProfile(ColourProfileRaw);

impl ColourProfile {
    /// The profile key.
    pub(crate) fn key(&self) -> &str {
        &self.0.key
    }

    /// The row for these attributes: the most constrained matching row, the first on a tie. Returns the
    /// row index (cover-table order) and its colour.
    pub(crate) fn select(&self, a: &Attrs) -> Option<(usize, &ColourSpec)> {
        let mut best: Option<(usize, usize)> = None;
        for (i, row) in self.0.rows.iter().enumerate() {
            if !row.select.matches(a) {
                continue;
            }
            let n = row.select.specificity();
            if best.is_none_or(|(_, bn)| n > bn) {
                best = Some((i, n));
            }
        }
        best.map(|(i, _)| (i, &self.0.rows[i].colour))
    }
}

/// The project's size profile (`ProjectSettings.size_profile`): class rules and the size row per class.
/// `pe_class` and `bond_rule` have a single value each in this format version and are not read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct SizeProfile(SizeProfileRaw);

impl SizeProfile {
    /// The profile key.
    pub(crate) fn key(&self) -> &str {
        &self.0.key
    }

    /// The circuit class of a potential by the profile's class rules: the first matching rule, else
    /// `Control` (the closing rule of doc 08 §4.2).
    pub(crate) fn class_of(&self, a: &Attrs) -> CircuitClass {
        self.0
            .class_rules
            .iter()
            .find(|r| r.select.matches(a))
            .map_or(CircuitClass::Control, |r| r.class.clone())
    }

    /// The cross-section of a class: the first row of that class.
    pub(crate) fn csa_of(&self, class: &CircuitClass) -> Option<&Csa> {
        self.0.rows.iter().find(|r| &r.class == class).map(|r| &r.csa)
    }
}

/// The three provisional settings members, parsed.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct WireProfiles {
    /// The colour profile, if present and well-formed.
    pub colour: Option<ColourProfile>,
    /// The size profile, if present and well-formed.
    pub size: Option<SizeProfile>,
    /// The project default (level 1); empty when absent or malformed.
    pub default: WireProps,
    /// `MalformedShim` and `DefaultWireProfile` notes.
    pub notes: Vec<DeriveNote>,
}

/// Reads one member: `None` when absent or malformed (the latter noted).
fn read<T: DeserializeOwned>(project: &Project, name: &str, notes: &mut Vec<DeriveNote>) -> Option<T> {
    let value = project.settings.extra.get(name)?;
    match serde_json::from_value::<T>(value.clone()) {
        Ok(v) => Some(v),
        Err(e) => {
            notes.push(DeriveNote::MalformedShim {
                member: format!("settings.{name}"),
                detail: e.to_string(),
            });
            None
        }
    }
}

/// Reads `colour_profile`, `size_profile` and `wire_default` from `ProjectSettings.extra`.
///
/// An absent member gives `DefaultWireProfile` (colour and size only); a malformed one gives
/// `MalformedShim` and is then treated as absent, without the `DefaultWireProfile` note.
pub(crate) fn load(project: &Project) -> WireProfiles {
    let mut notes = Vec::new();
    let colour = read::<ColourProfileRaw>(project, "colour_profile", &mut notes).map(ColourProfile);
    let size = read::<SizeProfileRaw>(project, "size_profile", &mut notes).map(SizeProfile);
    let default = read::<WireProps>(project, "wire_default", &mut notes).unwrap_or_default();
    if colour.is_none() && !project.settings.extra.contains_key("colour_profile") {
        notes.push(DeriveNote::DefaultWireProfile {
            member: "colour_profile",
        });
    }
    if size.is_none() && !project.settings.extra.contains_key("size_profile") {
        notes.push(DeriveNote::DefaultWireProfile {
            member: "size_profile",
        });
    }
    WireProfiles {
        colour,
        size,
        default,
        notes,
    }
}

// -------------------------------------------------------------------------------------------------------------
// Built-in defaults (level 0)

/// The circuit class by the company rules of doc 08 §4.2, used when the project has no size profile:
/// PE, PEN and FE are `Pe`; AC from 50 V is `Mains`; the rest is `Control`. (The `bus` and `analogue`
/// rules need attributes no potential has yet.)
pub(crate) fn builtin_class(a: &Attrs) -> CircuitClass {
    if matches!(
        a.ptype,
        Some(PotentialType::Pe | PotentialType::Pen | PotentialType::Fe)
    ) {
        CircuitClass::Pe
    } else if a.ac_dc == Some(AcDc::Ac) && a.voltage.is_some_and(|v| v >= Dec3(50_000)) {
        CircuitClass::Mains
    } else {
        CircuitClass::Control
    }
}

/// The built-in colour by potential type (the EN 60204-1 13.2 column of doc 08 §4.3).
pub(crate) fn builtin_colour(a: &Attrs) -> ColourSpec {
    let iec = |c: IecColour| ColourSpec::one(ColourCode::Iec(c));
    match &a.ptype {
        Some(PotentialType::N | PotentialType::LPlus | PotentialType::LMinus | PotentialType::M) => {
            iec(IecColour::Bu)
        }
        Some(PotentialType::Pe | PotentialType::Pen) => {
            ColourSpec::two(ColourCode::Iec(IecColour::Gn), ColourCode::Iec(IecColour::Ye))
        }
        Some(PotentialType::Fe) => iec(IecColour::Pk),
        Some(PotentialType::Signal) => {
            if a.ac_dc == Some(AcDc::Dc) {
                iec(IecColour::Bu)
            } else {
                iec(IecColour::Rd)
            }
        }
        // L, L1, L2, L3, Undefined, unknown and no type.
        _ => iec(IecColour::Bk),
    }
}

/// The built-in cross-section by class: Mains 1.5, Control, Analogue and Bus 0.75; any other class
/// (a `Pe` class the PE rule did not resolve, an unknown class) 1.5.
pub(crate) fn builtin_csa(class: &CircuitClass) -> Csa {
    match class {
        CircuitClass::Control | CircuitClass::Analogue | CircuitClass::Bus => Csa::Mm2(Dec3(750)),
        _ => Csa::Mm2(Dec3(1_500)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn attrs(ptype: PotentialType, voltage: Option<i64>, ac_dc: Option<AcDc>) -> Attrs {
        Attrs {
            ptype: Some(ptype),
            voltage: voltage.map(|v| Dec3(v * 1000)),
            ac_dc,
        }
    }

    fn colour_profile(rows: Value) -> ColourProfile {
        ColourProfile(serde_json::from_value(json!({"key": "p", "rows": rows})).unwrap())
    }

    fn code(spec: &ColourSpec) -> String {
        serde_json::to_string(spec).unwrap()
    }

    #[test]
    fn empty_select_matches_anything_and_ranges_are_inclusive() {
        let s: Select = serde_json::from_value(json!({})).unwrap();
        assert!(s.matches(&Attrs::default()));
        assert_eq!(s.specificity(), 0);
        let s: Select = serde_json::from_value(json!({"u_min": "50", "u_max": "250"})).unwrap();
        for (v, want) in [(49, false), (50, true), (230, true), (250, true), (251, false)] {
            assert_eq!(s.matches(&attrs(PotentialType::L, Some(v), None)), want, "{v} V");
        }
        // A constrained field the potential does not know does not match.
        assert!(!s.matches(&attrs(PotentialType::L, None, None)));
        let s: Select = serde_json::from_value(json!({"ac_dc": "dc"})).unwrap();
        assert!(!s.matches(&attrs(PotentialType::L, Some(24), None)));
        assert!(s.matches(&attrs(PotentialType::L, None, Some(AcDc::Dc))));
        let s: Select = serde_json::from_value(json!({"ptype": ["l_plus", "m"]})).unwrap();
        assert!(s.matches(&attrs(PotentialType::M, None, None)));
        assert!(!s.matches(&attrs(PotentialType::N, None, None)));
        assert!(!s.matches(&Attrs::default()));
    }

    #[test]
    fn discipline_and_analogue_never_match_yet() {
        for select in [
            json!({"analogue": true}),
            json!({"discipline": "bus"}),
            json!({"analogue": false}),
        ] {
            let s: Select = serde_json::from_value(select).unwrap();
            assert_eq!(s.specificity(), 1);
            assert!(!s.matches(&attrs(PotentialType::Signal, Some(24), Some(AcDc::Dc))));
        }
    }

    #[test]
    fn the_most_constrained_colour_row_wins_and_ties_go_to_the_first() {
        let p = colour_profile(json!([
            {"select": {"ptype": ["l_plus"]}, "colour": ["RD"]},
            {"select": {"ptype": ["l_plus"], "ac_dc": "dc"}, "colour": ["DBU"]},
            {"select": {"ptype": ["l_plus"], "ac_dc": "dc"}, "colour": ["GY"]},
            {"colour": ["WH"]},
        ]));
        let (i, c) = p
            .select(&attrs(PotentialType::LPlus, Some(24), Some(AcDc::Dc)))
            .unwrap();
        assert_eq!((i, code(c).as_str()), (1, r#"["DBU"]"#));
        let (i, c) = p.select(&attrs(PotentialType::LPlus, None, None)).unwrap();
        assert_eq!((i, code(c).as_str()), (0, r#"["RD"]"#));
        let (i, _) = p.select(&attrs(PotentialType::N, None, None)).unwrap();
        assert_eq!(i, 3);
        assert_eq!(p.key(), "p");
    }

    #[test]
    fn class_rules_first_match_wins_and_default_is_control() {
        let p: SizeProfileRaw = serde_json::from_value(json!({
            "key": "s",
            "class_rules": [
                {"select": {"ptype": ["pe", "pen", "fe"]}, "class": "pe"},
                {"select": {"ac_dc": "ac", "u_min": "50"}, "class": "mains"},
            ],
            "rows": [{"class": "control", "csa": "0.75"}, {"class": "mains", "csa": "1.5"}],
        }))
        .unwrap();
        let p = SizeProfile(p);
        assert_eq!(
            p.class_of(&attrs(PotentialType::Fe, None, None)),
            CircuitClass::Pe
        );
        assert_eq!(
            p.class_of(&attrs(PotentialType::L, Some(230), Some(AcDc::Ac))),
            CircuitClass::Mains
        );
        assert_eq!(
            p.class_of(&attrs(PotentialType::LPlus, Some(24), Some(AcDc::Dc))),
            CircuitClass::Control
        );
        assert_eq!(p.csa_of(&CircuitClass::Mains), Some(&Csa::Mm2(Dec3(1_500))));
        assert_eq!(p.csa_of(&CircuitClass::Bus), None);
        assert_eq!(p.key(), "s");
    }

    #[test]
    fn builtin_defaults() {
        let a = |t, ac_dc| Attrs {
            ptype: t,
            voltage: None,
            ac_dc,
        };
        let colour = |t, ac_dc| code(&builtin_colour(&a(t, ac_dc)));
        assert_eq!(colour(Some(PotentialType::L2), None), r#"["BK"]"#);
        assert_eq!(colour(Some(PotentialType::N), None), r#"["BU"]"#);
        assert_eq!(colour(Some(PotentialType::Pen), None), r#"["GN","YE"]"#);
        assert_eq!(colour(Some(PotentialType::Fe), None), r#"["PK"]"#);
        assert_eq!(colour(Some(PotentialType::LMinus), None), r#"["BU"]"#);
        assert_eq!(colour(Some(PotentialType::Signal), Some(AcDc::Dc)), r#"["BU"]"#);
        assert_eq!(colour(Some(PotentialType::Signal), Some(AcDc::Ac)), r#"["RD"]"#);
        assert_eq!(colour(Some(PotentialType::Undefined), None), r#"["BK"]"#);
        assert_eq!(colour(None, None), r#"["BK"]"#);
        assert_eq!(builtin_csa(&CircuitClass::Mains), Csa::Mm2(Dec3(1_500)));
        assert_eq!(builtin_csa(&CircuitClass::Bus), Csa::Mm2(Dec3(750)));
        assert_eq!(builtin_csa(&CircuitClass::Pe), Csa::Mm2(Dec3(1_500)));
        assert_eq!(
            builtin_class(&attrs(PotentialType::L, Some(230), Some(AcDc::Ac))),
            CircuitClass::Mains
        );
        assert_eq!(
            builtin_class(&attrs(PotentialType::L, Some(24), Some(AcDc::Ac))),
            CircuitClass::Control
        );
        assert_eq!(
            builtin_class(&attrs(PotentialType::Pen, None, None)),
            CircuitClass::Pe
        );
    }

    #[test]
    fn a_well_formed_profile_parses_in_the_record_spellings() {
        let p: ColourProfileRaw = serde_json::from_value(json!({
            "key": "company.dbu-wh-24v",
            "name": {"en": "DBU/WH 24 V"},
            "basis": "company_convention",
            "verified": true,
            "codes": [{"code": "DBU", "base": "BU", "shade": "dark"}],
            "rows": [{"group": {"en": "24 VDC"}, "function": {"en": "Positive"},
                      "select": {"ptype": ["l_plus"], "ac_dc": "dc", "u_max": "120"}, "colour": ["DBU"]}],
        }))
        .unwrap();
        assert_eq!(p.rows.len(), 1);
        assert_eq!(p.rows[0].select.u_max, Some(Dec3(120_000)));
    }

    #[test]
    fn a_malformed_colour_is_an_error() {
        assert!(
            serde_json::from_value::<ColourProfileRaw>(json!({"rows": [{"colour": ["BK", "BU", "RD"]}]}))
                .is_err()
        );
        assert!(serde_json::from_value::<SizeProfileRaw>(json!({"rows": [{"class": "control"}]})).is_err());
    }
}
