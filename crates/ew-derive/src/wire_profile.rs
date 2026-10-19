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
//! - Two colour rows of equal specificity whose selectors can match a common potential (`overlaps`: the
//!   `ptype` lists intersect or either is empty, `ac_dc` equal or either absent, the voltage intervals
//!   intersect, `discipline` and `analogue` equal or either absent) and that give different colours are
//!   ambiguous: `load` notes a [`DeriveNote::ProfileOverlap`] for `settings.colour_profile` naming the two
//!   row indexes. The profile stays loaded and `select` keeps the first-row tie rule, so the output is
//!   deterministic but no longer silent.
//! - The built-in colour depends on the circuit class: an L-family potential of an AC control circuit is RD
//!   (doc 08 §4.3, EN 60204-1 column), see `builtin_colour_for`.

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

    /// Whether some potential can match both selectors (conservative: true when unsure).
    fn overlaps(&self, other: &Select) -> bool {
        let ptype = self.ptype.is_empty()
            || other.ptype.is_empty()
            || self.ptype.iter().any(|t| other.ptype.contains(t));
        let ac_dc = self.ac_dc.is_none() || other.ac_dc.is_none() || self.ac_dc == other.ac_dc;
        // Inclusive intervals, `None` unbounded: they intersect when max(lo) <= min(hi).
        let lo = match (self.u_min, other.u_min) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        let hi = match (self.u_max, other.u_max) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let voltage = match (lo, hi) {
            (Some(lo), Some(hi)) => lo <= hi,
            _ => true,
        };
        let discipline =
            self.discipline.is_none() || other.discipline.is_none() || self.discipline == other.discipline;
        let analogue = self.analogue.is_none() || other.analogue.is_none() || self.analogue == other.analogue;
        ptype && ac_dc && voltage && discipline && analogue
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
    if let Some(profile) = &colour {
        let rows = &profile.0.rows;
        for (i, ri) in rows.iter().enumerate() {
            for (j, rj) in rows.iter().enumerate().skip(i + 1) {
                if ri.select.specificity() == rj.select.specificity()
                    && ri.colour != rj.colour
                    && ri.select.overlaps(&rj.select)
                {
                    notes.push(DeriveNote::ProfileOverlap {
                        shim: "settings.colour_profile".into(),
                        rows: (i, j),
                        detail: "have the same specificity, overlap and give different colours".into(),
                    });
                }
            }
        }
    }
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

/// The built-in colour by potential type with the class of the built-in rules (`builtin_class`); see
/// [`builtin_colour_for`], which this wraps.
#[cfg(test)]
pub(crate) fn builtin_colour(a: &Attrs) -> ColourSpec {
    builtin_colour_for(a, &builtin_class(a))
}

/// The built-in colour by potential type and circuit class (the EN 60204-1 13.2 column of doc 08 §4.3):
/// N, L+, L- and M are BU, PE and PEN are GN/YE, FE is PK, a signal is BU on DC and RD otherwise, an
/// L-family potential (`L`, `L1`..`L3`) of class `Control` on AC is RD ("AC control, L -> RD") and any
/// other L is BK.
pub(crate) fn builtin_colour_for(a: &Attrs, class: &CircuitClass) -> ColourSpec {
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
        Some(PotentialType::L | PotentialType::L1 | PotentialType::L2 | PotentialType::L3)
            if *class == CircuitClass::Control && a.ac_dc == Some(AcDc::Ac) =>
        {
            iec(IecColour::Rd)
        }
        // Other L, L1, L2, L3, Undefined, unknown and no type.
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

    #[test]
    fn builtin_colour_honours_the_class() {
        let a = |t, ac_dc| Attrs {
            ptype: Some(t),
            voltage: Some(Dec3(230_000)),
            ac_dc,
        };
        let l = a(PotentialType::L, Some(AcDc::Ac));
        assert_eq!(code(&builtin_colour_for(&l, &CircuitClass::Control)), r#"["RD"]"#);
        assert_eq!(code(&builtin_colour_for(&l, &CircuitClass::Mains)), r#"["BK"]"#);
        let lp = a(PotentialType::LPlus, Some(AcDc::Dc));
        assert_eq!(
            code(&builtin_colour_for(&lp, &CircuitClass::Control)),
            r#"["BU"]"#
        );
        let n = a(PotentialType::N, Some(AcDc::Ac));
        assert_eq!(code(&builtin_colour_for(&n, &CircuitClass::Control)), r#"["BU"]"#);
        let pe = a(PotentialType::Pe, None);
        assert_eq!(
            code(&builtin_colour_for(&pe, &CircuitClass::Pe)),
            r#"["GN","YE"]"#
        );
    }

    fn load_notes(rows: Value) -> Vec<DeriveNote> {
        let mut project: Project = serde_json::from_value(
            json!({"id": "00000000-0000-4000-8000-000000000001", "derivation_profile": "2027.1"}),
        )
        .unwrap();
        project
            .settings
            .extra
            .insert("colour_profile".into(), json!({"key": "p", "rows": rows}));
        load(&project)
            .notes
            .into_iter()
            .filter(|n| {
                matches!(
                    n,
                    DeriveNote::MalformedShim { .. } | DeriveNote::ProfileOverlap { .. }
                )
            })
            .collect()
    }

    fn winning_row(rows: Value, ptype: PotentialType) -> Option<usize> {
        let mut project: Project = serde_json::from_value(
            json!({"id": "00000000-0000-4000-8000-000000000001", "derivation_profile": "2027.1"}),
        )
        .unwrap();
        project
            .settings
            .extra
            .insert("colour_profile".into(), json!({"key": "p", "rows": rows}));
        let profiles = load(&project);
        let attrs = Attrs {
            ptype: Some(ptype),
            ..Attrs::default()
        };
        profiles.colour.and_then(|c| c.select(&attrs).map(|(i, _)| i))
    }

    #[test]
    fn overlap_gives_one_note_and_the_first_row_wins() {
        let attrs_rows = json!([
            {"select": {"ptype": ["l"]}, "colour": ["RD"]},
            {"select": {"ptype": ["l", "n"]}, "colour": ["BK"]},
        ]);
        let notes = load_notes(attrs_rows.clone());
        assert_eq!(notes.len(), 1);
        assert!(matches!(
            &notes[0],
            DeriveNote::ProfileOverlap { rows: (0, 1), .. }
        ));
        assert_eq!(winning_row(attrs_rows, PotentialType::L), Some(0));
    }

    #[test]
    fn overlapping_equal_specificity_rows_with_different_colours_are_noted() {
        let notes = load_notes(json!([
            {"select": {"ptype": ["l"], "u_min": "50"}, "colour": ["RD"]},
            {"select": {"ptype": ["l"], "u_max": "250"}, "colour": ["BK"]},
        ]));
        assert_eq!(
            notes,
            vec![DeriveNote::ProfileOverlap {
                shim: "settings.colour_profile".into(),
                rows: (0, 1),
                detail: "have the same specificity, overlap and give different colours".into(),
            }]
        );
        assert_eq!(
            notes[0].to_string(),
            "settings.colour_profile: rows 0 and 1 have the same specificity, overlap and give different \
             colours; the first row wins"
        );
        // Same colours: no note.
        assert!(
            load_notes(json!([
                {"select": {"ptype": ["l"], "u_min": "50"}, "colour": ["RD"]},
                {"select": {"ptype": ["l"], "u_max": "250"}, "colour": ["RD"]},
            ]))
            .is_empty()
        );
        // Disjoint voltage ranges: no note.
        assert!(
            load_notes(json!([
                {"select": {"u_max": "49"}, "colour": ["RD"]},
                {"select": {"u_min": "50"}, "colour": ["BK"]},
            ]))
            .is_empty()
        );
        // Different specificity: no note.
        assert!(
            load_notes(json!([
                {"select": {"ptype": ["l"]}, "colour": ["RD"]},
                {"select": {"ptype": ["l"], "ac_dc": "ac"}, "colour": ["BK"]},
            ]))
            .is_empty()
        );
    }
}
