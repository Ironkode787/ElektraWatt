// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The colour and size profile shims of the wire-property chain (doc 03 §5.1, doc 08 §4.2).
//!
//! Until `ew-schema` types `ProjectSettings.colour_profile`, `.size_profile` and `.wire_default`,
//! they are read from `ProjectSettings.extra` with the JSON shapes of doc 03 §5.1 (record spellings:
//! `l_plus`, `dc`; `Dec3` and `Csa` as strings; `ColourSpec` as an array). A member that is present but does
//! not parse is treated as absent (`project.settings.set` refuses one); without a colour or size profile the
//! built-in defaults of the derivation profile apply.
//!
//! # Matching (`Select`, doc 08 §4.2)
//!
//! - An empty or absent member matches anything. `ptype` matches when the list holds the potential's type;
//!   `ac_dc` is equal; `u_min <= voltage <= u_max` (inclusive).
//! - A constrained field the potential does not know (no voltage, no AC/DC, no type) does not match.
//! - A phase (type L1, L2 or L3) is a conductor of an AC supply: without an AC/DC of its own it matches as AC, and
//!   without a known voltage its circuit class is mains (`attrs_of`, `own_class`).
//! - `discipline` and `analogue` match only when the potential has that attribute. No potential has either in
//!   this phase, so a selector that constrains one never matches.
//! - Colour rows: the row with the most constrained fields wins, ties go to the first row, so two rows of equal
//!   specificity that match one potential give the first row's colour. Class rules: the first match wins.
//! - The built-in colour depends on the circuit class: an L-family potential of an AC control circuit is RD
//!   (doc 08 §4.3, EN 60204-1 column), see `builtin_colour_for`.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ew_schema::{AcDc, CircuitClass, ColourCode, ColourSpec, Csa, IecColour, PotentialType, WireProps};
use ew_units::Dec3;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::names::PotentialAttrsOf;
use crate::potentials::PotentialId;
use crate::query::{Policy, Query};
use crate::settings::{self, Member};
use crate::{PotentialFlow, Q};

/// What a selector is matched against: the final attributes of a potential (flow values).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Attrs {
    /// The type, if known.
    pub ptype: Option<PotentialType>,
    /// The voltage (V), if known.
    pub voltage: Option<Dec3>,
    /// AC or DC, if known.
    pub ac_dc: Option<AcDc>,
}

/// Whether a potential type is a phase of an AC supply (L1, L2, L3; IEC 60445).
fn is_phase(t: Option<&PotentialType>) -> bool {
    matches!(t, Some(PotentialType::L1 | PotentialType::L2 | PotentialType::L3))
}

/// The final attributes of a potential: the flow values, else its own (`potential_attrs`; none for an unknown
/// ID). A phase (L1, L2, L3) that says neither AC nor DC is AC: a phase is a conductor of an AC supply.
pub(crate) fn attrs_of(q: &Q, id: PotentialId) -> Attrs {
    let mut attrs = if let Some(e) = q.get::<PotentialFlow>(id).as_ref() {
        Attrs {
            ptype: e.ptype.as_ref().map(|a| a.value.clone()),
            voltage: e.voltage.as_ref().map(|a| a.value),
            ac_dc: e.env.as_ref().and_then(|a| a.value.ac_dc.clone()),
        }
    } else {
        let own = q.get::<PotentialAttrsOf>(id);
        Attrs {
            ptype: own.ptype.as_ref().map(|a| a.value.clone()),
            voltage: own.voltage.as_ref().map(|a| a.value),
            ac_dc: own.env.as_ref().and_then(|a| a.value.ac_dc.clone()),
        }
    };
    if attrs.ac_dc.is_none() && is_phase(attrs.ptype.as_ref()) {
        attrs.ac_dc = Some(AcDc::Ac);
    }
    attrs
}

/// The circuit class a potential's own definition gives it, if any; a conductor of an AC supply (a phase L1, L2, L3,
/// a single phase L or the neutral N) whose voltage is not known is of the mains class (a potential given
/// the type L1 took the control circuit's 0.75 mm²; so did a 16 A circuit of a single-phase supply).
pub(crate) fn own_class(q: &Q, id: PotentialId) -> Option<CircuitClass> {
    let own = q.get::<PotentialAttrsOf>(id);
    own.circuit_class.as_ref().map(|c| c.value.clone()).or_else(|| {
        let attrs = attrs_of(q, id);
        let ac_conductor = is_phase(attrs.ptype.as_ref())
            || matches!(attrs.ptype, Some(PotentialType::L | PotentialType::N));
        (ac_conductor && attrs.voltage.is_none()).then_some(CircuitClass::Mains)
    })
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
pub struct ColourProfile(ColourProfileRaw);

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
pub struct SizeProfile(SizeProfileRaw);

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

/// The three provisional settings members, parsed: the value of [`WireProfilesQ`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WireProfiles {
    /// The colour profile, if present and well-formed.
    pub(crate) colour: Option<ColourProfile>,
    /// The size profile, if present and well-formed.
    pub(crate) size: Option<SizeProfile>,
    /// The project default (level 1); empty when absent or malformed.
    pub(crate) default: WireProps,
}

/// Reads one member: `None` when absent or malformed (`project.settings.set` refuses a malformed one, through
/// [`check_member`]).
fn read<T: DeserializeOwned>(value: Option<&Value>) -> Option<T> {
    serde_json::from_value::<T>(value?.clone()).ok()
}

/// Checks one provisional member (`colour_profile`, `size_profile`, else `wire_default`) with the parser
/// [`load_members`] uses.
pub(crate) fn check_member(member: crate::Member, value: &Value) -> Result<(), crate::SettingsIssue> {
    use crate::SettingsIssue as I;
    fn parse<T: DeserializeOwned>(value: &Value) -> Result<T, crate::SettingsIssue> {
        serde_json::from_value::<T>(value.clone()).map_err(I::Serde)
    }
    match member {
        crate::Member::ColourProfile => {
            parse::<ColourProfileRaw>(value)?;
            check_colour_codes(value)
        }
        crate::Member::SizeProfile => {
            let profile: SizeProfileRaw = parse(value)?;
            for (i, row) in profile.rows.iter().enumerate() {
                let positive = match &row.csa {
                    ew_schema::Csa::Mm2(d) | ew_schema::Csa::Kcmil(d) => d.0 > 0,
                    ew_schema::Csa::Awg(_) => true,
                    ew_schema::Csa::Unknown(_) => false,
                };
                if !positive {
                    return Err(I::NotPositiveSize {
                        row: i,
                        csa: serde_json::to_string(&row.csa).unwrap_or_default(),
                    });
                }
            }
            Ok(())
        }
        _ => parse::<WireProps>(value).map(drop),
    }
}

/// Every colour code of a colour profile is an IEC 60757 code or a company code that the profile's `codes`
/// list declares on an IEC 60757 base (doc 08 §4.5: DBU, base BU, shade dark).
fn check_colour_codes(value: &Value) -> Result<(), crate::SettingsIssue> {
    use crate::SettingsIssue as I;
    use ew_schema::{ColourCode, IecColour};
    let mut declared: Vec<&str> = Vec::new();
    if let Some(codes) = value.get("codes") {
        let Some(codes) = codes.as_array() else {
            return Err(I::CodesNotList);
        };
        for (index, c) in codes.iter().enumerate() {
            let Some(code) = c.get("code").and_then(Value::as_str) else {
                return Err(I::CodeWithoutText { index });
            };
            if code.parse::<IecColour>().is_ok() {
                return Err(I::CodeIsIec {
                    index,
                    code: code.to_owned(),
                });
            }
            match c.get("base").and_then(Value::as_str) {
                Some(base) if base.parse::<IecColour>().is_ok() => declared.push(code),
                _ => {
                    return Err(I::CodeWithoutBase {
                        index,
                        code: code.to_owned(),
                    });
                }
            }
        }
    }
    let profile: ColourProfileRaw = serde_json::from_value(value.clone()).map_err(I::Serde)?;
    for (row, spec) in profile.rows.iter().enumerate() {
        for code in spec.colour.codes() {
            match code {
                ColourCode::Iec(_) => {}
                ColourCode::Company(c) if declared.contains(&c.as_str()) => {}
                ColourCode::Company(c) => {
                    return Err(I::UndeclaredCode {
                        row,
                        code: c.to_string(),
                    });
                }
                ColourCode::Foreign(c) => {
                    return Err(I::ForeignCode {
                        row,
                        code: c.to_string(),
                    });
                }
                ColourCode::Unknown(v) => {
                    return Err(I::NotColourCode {
                        row,
                        value: v.to_string(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Parses `colour_profile`, `size_profile` and `wire_default` (the members of `ProjectSettings.extra`; `None`
/// when absent or malformed, and the built-in defaults of the derivation profile apply).
pub(crate) fn load_members(
    colour: Option<&Value>,
    size: Option<&Value>,
    default: Option<&Value>,
) -> WireProfiles {
    WireProfiles {
        colour: read::<ColourProfileRaw>(colour).map(ColourProfile),
        size: read::<SizeProfileRaw>(size).map(SizeProfile),
        default: read::<WireProps>(default).unwrap_or_default(),
    }
}

// -------------------------------------------------------------------------------------------------------------
// The queries over the profiles (ARCH §6.4, ADR-0002 amendment 4 "Query shapes")

/// `wire_profiles()`: the three provisional settings members, parsed. Reads the members through
/// `settings::member` only (never the project row). Policy Memo.
///
/// Readers that need one potential's rows read [`ProfileRowFor`] and the project default [`WireDefaultProps`],
/// so that an edit of one colour row reaches the potentials that select it and no others.
#[derive(Debug)]
pub struct WireProfilesQ;

impl Query for WireProfilesQ {
    type Key = ();
    type Value = WireProfiles;
    const FAMILY: &'static str = "wire_profiles";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<WireProfiles> {
        let colour = settings::member(q, Member::ColourProfile);
        let size = settings::member(q, Member::SizeProfile);
        let default = settings::member(q, Member::WireDefault);
        Arc::new(load_members(
            (*colour).as_ref(),
            (*size).as_ref(),
            (*default).as_ref(),
        ))
    }
}

/// `wire_default_props()`: the project default of the wire-property chain (level 1,
/// `ProjectSettings.extra["wire_default"]`). Policy Memo.
#[derive(Debug)]
pub struct WireDefaultProps;

impl Query for WireDefaultProps {
    type Key = ();
    type Value = WireProps;
    const FAMILY: &'static str = "wire_default_props";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<WireProps> {
        Arc::new(q.get::<WireProfilesQ>(()).default.clone())
    }
}

/// The ordered form of an enum that has an `Unknown(Value)` variant: the variant index for a known variant,
/// `(u8::MAX, the value's text)` for an unknown one.
type Tag = (u8, String);

fn known(index: u8) -> Tag {
    (index, String::new())
}

fn ptype_tag(t: &PotentialType) -> Tag {
    known(match t {
        PotentialType::L1 => 0,
        PotentialType::L2 => 1,
        PotentialType::L3 => 2,
        PotentialType::L => 3,
        PotentialType::N => 4,
        PotentialType::Pe => 5,
        PotentialType::Pen => 6,
        PotentialType::Fe => 7,
        PotentialType::LPlus => 8,
        PotentialType::LMinus => 9,
        PotentialType::M => 10,
        PotentialType::Signal => 11,
        PotentialType::Undefined => 12,
        PotentialType::Unknown(v) => return (u8::MAX, v.to_string()),
    })
}

fn ac_dc_tag(a: &AcDc) -> Tag {
    known(match a {
        AcDc::Ac => 0,
        AcDc::Dc => 1,
        AcDc::Unknown(v) => return (u8::MAX, v.to_string()),
    })
}

fn class_tag(c: &CircuitClass) -> Tag {
    known(match c {
        CircuitClass::Mains => 0,
        CircuitClass::Control => 1,
        CircuitClass::Analogue => 2,
        CircuitClass::Bus => 3,
        CircuitClass::Pe => 4,
        CircuitClass::Unknown(v) => return (u8::MAX, v.to_string()),
    })
}

/// The ordered form of a [`ProfileKey`]: type, voltage, AC/DC, class.
type Canon = (Option<Tag>, Option<Dec3>, Option<Tag>, Option<Tag>);

/// The key of [`ProfileRowFor`]: the attributes of a potential and its own circuit class, if it has one.
///
/// `PotentialType`, `AcDc` and `CircuitClass` carry an `Unknown(Value)` variant and so are neither `Ord` nor
/// `Hash`. The key keeps the originals and orders and hashes by a canonical tuple only (a known variant by its
/// index, an unknown one by the text of its value); equality, order and hash all go through that tuple, so
/// they agree.
#[derive(Clone, Debug)]
pub struct ProfileKey {
    attrs: Attrs,
    class: Option<CircuitClass>,
    canon: Canon,
}

impl ProfileKey {
    /// The key of attributes `attrs` and the potential's own circuit class `class`.
    pub fn new(attrs: &Attrs, class: Option<CircuitClass>) -> ProfileKey {
        let canon = (
            attrs.ptype.as_ref().map(ptype_tag),
            attrs.voltage,
            attrs.ac_dc.as_ref().map(ac_dc_tag),
            class.as_ref().map(class_tag),
        );
        ProfileKey {
            attrs: attrs.clone(),
            class,
            canon,
        }
    }

    /// The key the wire properties of potential `p` use: its final attributes (the flow values, else its own)
    /// and its defined circuit class.
    pub fn of_potential(q: &Q, p: PotentialId) -> ProfileKey {
        ProfileKey::new(&attrs_of(q, p), own_class(q, p))
    }

    /// The attributes.
    pub fn attrs(&self) -> Attrs {
        self.attrs.clone()
    }

    /// The potential's own circuit class, if it has one.
    pub fn class(&self) -> Option<&CircuitClass> {
        self.class.as_ref()
    }
}

impl PartialEq for ProfileKey {
    fn eq(&self, other: &ProfileKey) -> bool {
        self.canon == other.canon
    }
}

impl Eq for ProfileKey {}

impl PartialOrd for ProfileKey {
    fn partial_cmp(&self, other: &ProfileKey) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProfileKey {
    fn cmp(&self, other: &ProfileKey) -> std::cmp::Ordering {
        self.canon.cmp(&other.canon)
    }
}

impl Hash for ProfileKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.canon.hash(state);
    }
}

/// The value of [`ProfileRowFor`]: what the profiles say about one attribute set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProfileRow {
    /// The circuit class: the key's own, else the size profile's class rules on the attributes, else (no size
    /// profile) the built-in rules.
    pub class: CircuitClass,
    /// The colour row the attributes select: the colour profile's key, the row index (cover-table order) and
    /// the colour; `None` without a colour profile or a matching row.
    pub colour: Option<(String, usize, ColourSpec)>,
    /// The size row of `class`: the size profile's key and the cross-section; `None` without a size profile
    /// or a row of that class.
    pub csa: Option<(String, Csa)>,
}

/// `profile_row_for(key)`: the class, the colour row and the size row of one attribute set. Reads
/// [`WireProfilesQ`] whole and nothing else; readers take it as a cut-off point (`Q::get_value_part`), so a
/// profile edit reaches only the potentials whose row changed. Policy Memo.
#[derive(Debug)]
pub struct ProfileRowFor;

impl Query for ProfileRowFor {
    type Key = ProfileKey;
    type Value = ProfileRow;
    const FAMILY: &'static str = "profile_row_for";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &ProfileKey) -> Arc<ProfileRow> {
        let profiles = q.get::<WireProfilesQ>(());
        let attrs = key.attrs();
        let class = match (key.class(), &profiles.size) {
            (Some(c), _) => c.clone(),
            (None, Some(size)) => size.class_of(&attrs),
            (None, None) => builtin_class(&attrs),
        };
        let colour = profiles.colour.as_ref().and_then(|p| {
            p.select(&attrs)
                .map(|(row, c)| (p.key().to_owned(), row, c.clone()))
        });
        let csa = profiles
            .size
            .as_ref()
            .and_then(|s| s.csa_of(&class).map(|csa| (s.key().to_owned(), csa.clone())));
        Arc::new(ProfileRow { class, colour, csa })
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

/// One row of the current table: a cross-section in mm² and the current it carries in A.
#[derive(Deserialize)]
struct CurrentRow {
    csa: Dec3,
    current: Dec3,
}

/// The current table a wire's cross-section is proposed from by the rating of the device that protects it (doc 08
/// §4.2): `content/profiles/current/ew.current.iec60204-1-b2.toml`, its rows rising.
#[derive(Deserialize)]
struct CurrentTable {
    id: String,
    basis: String,
    rows: Vec<CurrentRow>,
}

/// What the current table cites (its standard, table and installation method), for the provenance a person reads.
pub fn current_table_basis() -> &'static str {
    &current_table().basis
}

/// The compiled-in current table, parsed once.
fn current_table() -> &'static CurrentTable {
    static TABLE: std::sync::OnceLock<CurrentTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        toml::from_str(include_str!(
            "../../../content/profiles/current/ew.current.iec60204-1-b2.toml"
        ))
        .unwrap_or_else(|e| unreachable!("the shipped current table reads: {e}"))
    })
}

/// The cross-section a wire of `size` mm² takes for `rating` amperes, with the table's key: where the rating exceeds
/// what the size's own row carries (the largest row at or below the size), the smallest row that carries it. `None`
/// where the size carries it, for a size below the table's first row (the table does not say what it carries), and
/// for a rating above its last row.
pub(crate) fn csa_raised(size: Dec3, rating: Dec3) -> Option<(&'static str, Csa)> {
    let table = current_table();
    let carried = table.rows.iter().rev().find(|r| r.csa <= size)?.current;
    if rating <= carried {
        return None;
    }
    table
        .rows
        .iter()
        .find(|r| r.current >= rating)
        .map(|r| (table.id.as_str(), Csa::Mm2(r.csa)))
}

/// The cross-section a wire of `size` mm² set by hand falls short of for `rating` amperes (EW-POT-022): the current
/// table's least row that carries the rating where the size's own row (the largest at or below it) carries less. A
/// size below the table's first row carries less than that row, so it falls short of a rating above the first row's
/// current; `None` where the size carries the rating or may, and for a rating above the table's last row.
pub fn csa_short_of(size: Dec3, rating: Dec3) -> Option<Csa> {
    let table = current_table();
    let first = table.rows.first()?;
    let carried = table
        .rows
        .iter()
        .rev()
        .find(|r| r.csa <= size)
        .map_or(first.current, |r| r.current);
    if rating <= carried {
        return None;
    }
    table
        .rows
        .iter()
        .find(|r| r.current >= rating)
        .map(|r| Csa::Mm2(r.csa))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A size is raised only for a rating above what its own row carries, to the first row that carries it: a rating
    /// the size carries exactly keeps it, one just above goes to the next row, a size between two rows counts as the
    /// lower, a size below the first row and a rating above the last row propose nothing.
    #[test]
    fn a_size_is_raised_to_the_least_cross_section_that_carries_a_rating_above_its_own() {
        let table = current_table();
        assert!(
            table
                .rows
                .windows(2)
                .all(|w| w[0].csa < w[1].csa && w[0].current < w[1].current)
        );
        let (first, second, last) = (&table.rows[0], &table.rows[1], &table.rows[table.rows.len() - 1]);
        let raised = |size: Dec3, rating: Dec3| csa_raised(size, rating).map(|(_, c)| c);
        assert_eq!(raised(first.csa, first.current), None);
        assert_eq!(
            raised(first.csa, Dec3(first.current.0 + 1)),
            Some(Csa::Mm2(second.csa))
        );
        assert_eq!(
            raised(Dec3(first.csa.0 + 1), Dec3(first.current.0 + 1)),
            Some(Csa::Mm2(second.csa))
        );
        assert_eq!(raised(Dec3(first.csa.0 - 1), Dec3(1_000)), None);
        assert_eq!(raised(first.csa, Dec3(last.current.0 + 1)), None);
    }

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

    fn hash_of(k: &ProfileKey) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        k.hash(&mut h);
        h.finish()
    }

    #[test]
    fn profile_keys_order_hash_and_compare_by_the_canonical_tuple() {
        let key = |ptype: Option<PotentialType>, class: Option<CircuitClass>| {
            ProfileKey::new(
                &Attrs {
                    ptype,
                    voltage: Some(Dec3(24_000)),
                    ac_dc: Some(AcDc::Dc),
                },
                class,
            )
        };
        let a = key(Some(PotentialType::L), None);
        let b = key(Some(PotentialType::L), None);
        assert_eq!(a, b);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
        assert_eq!(hash_of(&a), hash_of(&b));
        // Known variants order by their index; an unknown value sorts after every known variant and is
        // told apart by its value.
        let n = key(Some(PotentialType::N), None);
        let u1 = key(Some(PotentialType::Unknown(json!("x"))), None);
        let u2 = key(Some(PotentialType::Unknown(json!("y"))), None);
        assert!(a < n && n < u1 && u1 < u2);
        assert_ne!(u1, u2);
        assert_eq!(u1, key(Some(PotentialType::Unknown(json!("x"))), None));
        // The class takes part; no class is not a class.
        let c = key(Some(PotentialType::L), Some(CircuitClass::Control));
        assert_ne!(a, c);
        assert!(a < c);
        assert!(c < key(Some(PotentialType::L), Some(CircuitClass::Unknown(json!(1)))));
        // The originals stay readable.
        assert_eq!(c.class(), Some(&CircuitClass::Control));
        assert_eq!(c.attrs().ac_dc, Some(AcDc::Dc));
    }

    fn load_colour(rows: Value) -> WireProfiles {
        load_members(Some(&json!({"key": "p", "rows": rows})), None, None)
    }

    fn winning_row(rows: Value, ptype: PotentialType) -> Option<usize> {
        let profiles = load_colour(rows);
        let attrs = Attrs {
            ptype: Some(ptype),
            ..Attrs::default()
        };
        profiles.colour.and_then(|c| c.select(&attrs).map(|(i, _)| i))
    }

    #[test]
    fn of_two_rows_of_equal_specificity_the_first_wins() {
        let attrs_rows = json!([
            {"select": {"ptype": ["l"]}, "colour": ["RD"]},
            {"select": {"ptype": ["l", "n"]}, "colour": ["BK"]},
        ]);
        assert_eq!(winning_row(attrs_rows, PotentialType::L), Some(0));
    }
}
