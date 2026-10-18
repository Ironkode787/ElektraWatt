// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Function semantics of an element definition (doc 14 §4.3 "Function semantics", steps 2–4; step 1, the
//! `symbol-map.csv`, is a later work package).
//!
//! [`resolve`] decides, in this order:
//!
//! 0. **Link type.** `next_report` and `previous_report` reference the built-in interruption-point FnDef
//!    ([`crate::IP_FNDEF_KEY`]); nothing is generated. `thumbnail` is a graphic: category
//!    `Graphic / QET / Thumbnail`, black box, no finding.
//! 1. **`kindInformations`** (by the `type`, `state` and `function` entries) for the link types that carry
//!    them:
//!
//!    | Link type | `type` | Category | Contact kind | Internal links |
//!    |---|---|---|---|---|
//!    | `master` | `coil` | `Coil / QET / Coil` | `Coil` | none |
//!    | `master` | `protection` | `ProtectionOvercurrent / QET / Protection` | `Pole` | pass-through, opens |
//!    | `master` | `commutator` | `Switch / QET / Commutator` | `Pole` | pass-through, opens |
//!    | `master` | `plc` | `PlcIo / QET / Module` | `PlcIo` | none |
//!    | `slave` | `simple`, `power`, `delayOn`, `delayOff`, `delayOnOff`, with `state` `NO` / `NC` / `SW` | `Contact / <type> / <state>` | `No` / `Nc` / `Co` | pass-through, opens |
//!    | `terminal` | `generic` (default), `ground` | `Terminal / <type> / <function or generic>` | `Terminal` | short |
//!    | `terminal` | `fuse`, `sectional` | as above | `Terminal` | pass-through, opens |
//!    | `terminal` | `diode` | as above | `Terminal` | pass-through |
//!
//!    "Pass-through" links the **two** terminals of the element (`PassThrough{opens}` between CP 0 and 1);
//!    with another terminal count no link is generated and EW-IMP-119 (warning) says so, since the pairing is
//!    unknown (for example the common terminal of a change-over contact). "Short" links consecutive terminals
//!    (0–1, 1–2, …). A terminal element's `function` sets every CP's potential role: `phase` → L,
//!    `neutral` → N, `PE` → PE; type `ground` without a function → PE.
//! 2. **Category path** ([`CATEGORY_TABLE`]): for an element with a collection path, the first row whose
//!    keyword occurs in a path segment (lower-cased, a leading `nn_` order prefix removed), segments tried
//!    from the last (the file name) to the first, rows in table order.
//! 3. **Black box** `BlackBox / QET / Generic` with EW-IMP-119 (warning, "map symbol").
//!
//! A `type` or `state` value that is not a valid identifier falls through to the next step.

use std::collections::BTreeMap;

use ew_interop_api::{Code, Severity};
use ew_mltext::Ident;
use ew_schema::{CategoryKey, ContactKind, InternalKind, InternalLinkDef, PassThroughSpec, PotentialType};

/// How the links between a function's terminals are generated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InternalRule {
    /// No internal links.
    None,
    /// `PassThrough{opens}` between the two terminals; nothing (and EW-IMP-119) for another count.
    PassThroughPair {
        /// The contact can open.
        opens: bool,
    },
    /// `Short` between consecutive terminals.
    ShortChain,
}

/// One row of the category-path table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CategoryRow {
    /// The keyword searched for in a path segment.
    pub keyword: &'static str,
    /// Category, group, definition.
    pub category: [&'static str; 3],
    /// The contact kind.
    pub contact_kind: ContactKindName,
    /// The internal links.
    pub internal: InternalRule,
}

/// The contact kinds the tables use (a `Copy` mirror of [`ContactKind`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContactKindName {
    /// Normally open.
    No,
    /// Normally closed.
    Nc,
    /// Change-over.
    Co,
    /// Coil.
    Coil,
    /// Pole.
    Pole,
    /// Terminal.
    Terminal,
    /// Black box.
    BlackBox,
    /// PLC input or output.
    PlcIo,
}

impl ContactKindName {
    /// The schema value.
    pub fn to_schema(self) -> ContactKind {
        match self {
            ContactKindName::No => ContactKind::No,
            ContactKindName::Nc => ContactKind::Nc,
            ContactKindName::Co => ContactKind::Co,
            ContactKindName::Coil => ContactKind::Coil,
            ContactKindName::Pole => ContactKind::Pole,
            ContactKindName::Terminal => ContactKind::Terminal,
            ContactKindName::BlackBox => ContactKind::BlackBox,
            ContactKindName::PlcIo => ContactKind::PlcIo,
        }
    }
}

/// QET collection category paths to EW categories, seeded with the functions the golden symbols need (doc
/// 07 §9.1: switches, MCBs, thermostats, position switches, lamps, heaters, terminals, earth, plug pins,
/// coils). **Extensible:** add rows (more specific keywords before more general ones), never change one.
pub const CATEGORY_TABLE: &[CategoryRow] = &[
    row(
        "thermostat",
        ["Switch", "QET", "Thermostat"],
        ContactKindName::Pole,
        PASS_OPENS,
    ),
    row(
        "position",
        ["Switch", "QET", "Position"],
        ContactKindName::Pole,
        PASS_OPENS,
    ),
    row(
        "push_button",
        ["Switch", "QET", "PushButton"],
        ContactKindName::Pole,
        PASS_OPENS,
    ),
    row(
        "switch",
        ["Switch", "QET", "Switch"],
        ContactKindName::Pole,
        PASS_OPENS,
    ),
    row(
        "circuit_breaker",
        ["ProtectionOvercurrent", "QET", "Breaker"],
        ContactKindName::Pole,
        PASS_OPENS,
    ),
    row(
        "breaker",
        ["ProtectionOvercurrent", "QET", "Breaker"],
        ContactKindName::Pole,
        PASS_OPENS,
    ),
    row(
        "fuse",
        ["ProtectionOvercurrent", "QET", "Fuse"],
        ContactKindName::Pole,
        PASS_OPENS,
    ),
    row(
        "protection",
        ["ProtectionOvercurrent", "QET", "Protection"],
        ContactKindName::Pole,
        PASS_OPENS,
    ),
    row(
        "lamp",
        ["Lamp", "QET", "Lamp"],
        ContactKindName::BlackBox,
        InternalRule::None,
    ),
    row(
        "signaling",
        ["Lamp", "QET", "Signal"],
        ContactKindName::BlackBox,
        InternalRule::None,
    ),
    row(
        "heater",
        ["Heater", "QET", "Heater"],
        ContactKindName::BlackBox,
        InternalRule::None,
    ),
    row(
        "heating",
        ["Heater", "QET", "Heater"],
        ContactKindName::BlackBox,
        InternalRule::None,
    ),
    row(
        "terminal",
        ["Terminal", "QET", "Generic"],
        ContactKindName::Terminal,
        InternalRule::ShortChain,
    ),
    row(
        "earth",
        ["Terminal", "QET", "PE"],
        ContactKindName::Terminal,
        InternalRule::ShortChain,
    ),
    row(
        "ground",
        ["Terminal", "QET", "PE"],
        ContactKindName::Terminal,
        InternalRule::ShortChain,
    ),
    row(
        "plug",
        ["Connector", "QET", "Pin"],
        ContactKindName::Terminal,
        InternalRule::ShortChain,
    ),
    row(
        "connector",
        ["Connector", "QET", "Pin"],
        ContactKindName::Terminal,
        InternalRule::ShortChain,
    ),
    row(
        "coil",
        ["Coil", "QET", "Coil"],
        ContactKindName::Coil,
        InternalRule::None,
    ),
    row(
        "relay",
        ["Coil", "QET", "Coil"],
        ContactKindName::Coil,
        InternalRule::None,
    ),
    row(
        "contactor",
        ["Coil", "QET", "Coil"],
        ContactKindName::Coil,
        InternalRule::None,
    ),
];

const PASS_OPENS: InternalRule = InternalRule::PassThroughPair { opens: true };

const fn row(
    keyword: &'static str,
    category: [&'static str; 3],
    contact_kind: ContactKindName,
    internal: InternalRule,
) -> CategoryRow {
    CategoryRow {
        keyword,
        category,
        contact_kind,
        internal,
    }
}

/// The resolved function semantics of a definition.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Resolution {
    /// Reference the built-in FnDef with this key; generate nothing.
    Builtin(&'static str),
    /// Generate a FnDef.
    Generated(Generated),
}

/// The parts of a generated FnDef that resolution decides.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Generated {
    /// Category.
    pub category: CategoryKey,
    /// Contact kind.
    pub contact_kind: ContactKind,
    /// Internal links.
    pub internal: Vec<InternalLinkDef>,
    /// The potential role of every CP, if any.
    pub potential_role: Option<PotentialType>,
    /// A finding to report: code, severity, message.
    pub finding: Option<(Code, Severity, String)>,
}

/// Resolves the function semantics of a definition with `link_type`, its `kindInformations` entries
/// `kind` (name → trimmed text), its collection `path` and `terminals` terminals, by the steps of the
/// module documentation.
pub fn resolve(
    link_type: &str,
    kind: &BTreeMap<String, String>,
    path: Option<&str>,
    terminals: usize,
) -> Resolution {
    if matches!(link_type, "next_report" | "previous_report") {
        return Resolution::Builtin(crate::IP_FNDEF_KEY);
    }
    if link_type == "thumbnail" {
        return Resolution::Generated(generated(
            ["Graphic", "QET", "Thumbnail"],
            ContactKindName::BlackBox,
            InternalRule::None,
            None,
            terminals,
        ));
    }
    if let Some(r) = from_kind_informations(link_type, kind, terminals) {
        return Resolution::Generated(r);
    }
    if let Some(row) = path.and_then(category_row) {
        return Resolution::Generated(generated(
            row.category,
            row.contact_kind,
            row.internal,
            None,
            terminals,
        ));
    }
    let mut g = generated(
        ["BlackBox", "QET", "Generic"],
        ContactKindName::BlackBox,
        InternalRule::None,
        None,
        terminals,
    );
    g.finding = Some((
        Code::BLACK_BOX,
        Severity::Warning,
        "element without function semantics: generic black box".to_owned(),
    ));
    Resolution::Generated(g)
}

/// Step 1: `kindInformations`.
fn from_kind_informations(
    link_type: &str,
    kind: &BTreeMap<String, String>,
    terminals: usize,
) -> Option<Generated> {
    let ty = kind.get("type").map(String::as_str);
    match link_type {
        "master" => {
            let (cat, ck, rule) = match ty? {
                "coil" => (["Coil", "QET", "Coil"], ContactKindName::Coil, InternalRule::None),
                "protection" => (
                    ["ProtectionOvercurrent", "QET", "Protection"],
                    ContactKindName::Pole,
                    PASS_OPENS,
                ),
                "commutator" => (["Switch", "QET", "Commutator"], ContactKindName::Pole, PASS_OPENS),
                "plc" => (
                    ["PlcIo", "QET", "Module"],
                    ContactKindName::PlcIo,
                    InternalRule::None,
                ),
                _ => return None,
            };
            Some(generated(cat, ck, rule, None, terminals))
        }
        "slave" => {
            let ty = ty?;
            if !matches!(ty, "simple" | "power" | "delayOn" | "delayOff" | "delayOnOff") {
                return None;
            }
            let state = kind.get("state").map(String::as_str)?;
            let ck = match state {
                "NO" => ContactKindName::No,
                "NC" => ContactKindName::Nc,
                "SW" => ContactKindName::Co,
                _ => return None,
            };
            let category = category_key(["Contact", ty, state])?;
            Some(generated_key(category, ck, PASS_OPENS, None, terminals))
        }
        "terminal" => {
            let ty = ty.unwrap_or("generic");
            let rule = match ty {
                "generic" | "ground" => InternalRule::ShortChain,
                "fuse" | "sectional" => PASS_OPENS,
                "diode" => InternalRule::PassThroughPair { opens: false },
                _ => return None,
            };
            let function = kind.get("function").map(String::as_str);
            let role = match function {
                Some("phase") => Some(PotentialType::L),
                Some("neutral") => Some(PotentialType::N),
                Some("PE") => Some(PotentialType::Pe),
                _ if ty == "ground" => Some(PotentialType::Pe),
                _ => None,
            };
            let category = category_key(["Terminal", ty, function.unwrap_or("generic")])?;
            Some(generated_key(
                category,
                ContactKindName::Terminal,
                rule,
                role,
                terminals,
            ))
        }
        _ => None,
    }
}

/// Step 2: the first table row matching a segment of `path`.
pub fn category_row(path: &str) -> Option<&'static CategoryRow> {
    let path = path.strip_prefix("embed://").unwrap_or(path);
    let segments: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let s = s.to_lowercase();
            let digits = s.bytes().take_while(u8::is_ascii_digit).count();
            match s[digits..].strip_prefix('_') {
                Some(rest) if digits > 0 => rest.to_owned(),
                _ => s,
            }
        })
        .collect();
    segments
        .iter()
        .rev()
        .find_map(|seg| CATEGORY_TABLE.iter().find(|row| seg.contains(row.keyword)))
}

fn category_key(parts: [&str; 3]) -> Option<CategoryKey> {
    Some((
        Ident::from_input(parts[0]).ok()?,
        Ident::from_input(parts[1]).ok()?,
        Ident::from_input(parts[2]).ok()?,
    ))
}

fn generated(
    category: [&'static str; 3],
    ck: ContactKindName,
    rule: InternalRule,
    role: Option<PotentialType>,
    terminals: usize,
) -> Generated {
    let key = category_key(category).unwrap_or_else(|| unreachable!("table categories are identifiers"));
    generated_key(key, ck, rule, role, terminals)
}

fn generated_key(
    category: CategoryKey,
    ck: ContactKindName,
    rule: InternalRule,
    role: Option<PotentialType>,
    terminals: usize,
) -> Generated {
    let link = |a: usize, b: usize, kind: InternalKind| InternalLinkDef {
        a: a as u16,
        b: b as u16,
        kind,
        extra: Default::default(),
    };
    let mut finding = None;
    let internal = match rule {
        InternalRule::None => Vec::new(),
        InternalRule::PassThroughPair { opens } if terminals == 2 => {
            vec![link(
                0,
                1,
                InternalKind::PassThrough(PassThroughSpec {
                    opens,
                    extra: Default::default(),
                }),
            )]
        }
        InternalRule::PassThroughPair { .. } => {
            finding = Some((
                Code::BLACK_BOX,
                Severity::Warning,
                format!(
                    "{} terminals where a pass-through pair is expected: internal links not generated",
                    terminals
                ),
            ));
            Vec::new()
        }
        InternalRule::ShortChain => (1..terminals.min(usize::from(u16::MAX)))
            .map(|i| link(i - 1, i, InternalKind::Short))
            .collect(),
    };
    Generated {
        category,
        contact_kind: ck.to_schema(),
        internal,
        potential_role: role,
        finding,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_schema::PassThroughSpec;

    fn kind(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn generated_of(r: Resolution) -> Generated {
        match r {
            Resolution::Generated(g) => g,
            Resolution::Builtin(k) => panic!("builtin {k}"),
        }
    }

    fn cat(g: &Generated) -> [&str; 3] {
        [
            g.category.0.as_str(),
            g.category.1.as_str(),
            g.category.2.as_str(),
        ]
    }

    #[test]
    fn interruption_points_are_builtin() {
        let none = BTreeMap::new();
        assert_eq!(
            resolve("next_report", &none, None, 1),
            Resolution::Builtin("ew.gen.ip")
        );
        assert_eq!(
            resolve("previous_report", &none, None, 1),
            Resolution::Builtin("ew.gen.ip")
        );
    }

    #[test]
    fn slave_contacts() {
        let g = generated_of(resolve(
            "slave",
            &kind(&[("type", "simple"), ("state", "NO")]),
            None,
            2,
        ));
        assert_eq!(cat(&g), ["Contact", "simple", "NO"]);
        assert_eq!(g.contact_kind, ContactKind::No);
        assert_eq!(g.internal.len(), 1);
        assert_eq!((g.internal[0].a, g.internal[0].b), (0, 1));
        assert_eq!(
            g.internal[0].kind,
            InternalKind::PassThrough(PassThroughSpec {
                opens: true,
                extra: Default::default()
            })
        );
        assert_eq!(g.finding, None);

        let g = generated_of(resolve(
            "slave",
            &kind(&[("type", "delayOn"), ("state", "NC")]),
            None,
            2,
        ));
        assert_eq!(g.contact_kind, ContactKind::Nc);

        // A change-over contact with three terminals: the pairing is unknown.
        let g = generated_of(resolve(
            "slave",
            &kind(&[("type", "power"), ("state", "SW")]),
            None,
            3,
        ));
        assert_eq!(g.contact_kind, ContactKind::Co);
        assert!(g.internal.is_empty());
        assert_eq!(g.finding.as_ref().map(|f| f.0), Some(Code::BLACK_BOX));

        // Unknown state falls through to the black box.
        let g = generated_of(resolve(
            "slave",
            &kind(&[("type", "simple"), ("state", "XX")]),
            None,
            2,
        ));
        assert_eq!(cat(&g), ["BlackBox", "QET", "Generic"]);
    }

    #[test]
    fn masters() {
        let g = generated_of(resolve("master", &kind(&[("type", "coil")]), None, 2));
        assert_eq!(
            (cat(&g), g.contact_kind.clone()),
            (["Coil", "QET", "Coil"], ContactKind::Coil)
        );
        assert!(g.internal.is_empty());
        let g = generated_of(resolve("master", &kind(&[("type", "protection")]), None, 2));
        assert_eq!(
            g.internal[0].kind,
            InternalKind::PassThrough(PassThroughSpec {
                opens: true,
                extra: Default::default()
            })
        );
        let g = generated_of(resolve("master", &kind(&[("type", "plc")]), None, 8));
        assert_eq!(g.contact_kind, ContactKind::PlcIo);
    }

    #[test]
    fn terminals() {
        let g = generated_of(resolve(
            "terminal",
            &kind(&[("type", "generic"), ("function", "PE")]),
            None,
            3,
        ));
        assert_eq!(cat(&g), ["Terminal", "generic", "PE"]);
        assert_eq!(g.potential_role, Some(PotentialType::Pe));
        assert_eq!(g.internal.len(), 2);
        assert!(g.internal.iter().all(|l| l.kind == InternalKind::Short));
        let g = generated_of(resolve("terminal", &BTreeMap::new(), None, 2));
        assert_eq!(cat(&g), ["Terminal", "generic", "generic"]);
        assert_eq!(g.potential_role, None);
        let g = generated_of(resolve("terminal", &kind(&[("type", "diode")]), None, 2));
        assert_eq!(
            g.internal[0].kind,
            InternalKind::PassThrough(PassThroughSpec {
                opens: false,
                extra: Default::default()
            })
        );
        let g = generated_of(resolve("terminal", &kind(&[("type", "ground")]), None, 1));
        assert_eq!(g.potential_role, Some(PotentialType::Pe));
    }

    #[test]
    fn category_paths() {
        let r = category_row("embed://import/10_electric/20_switches/30_thermostat_nc.elmt").unwrap();
        assert_eq!(r.category, ["Switch", "QET", "Thermostat"]);
        // The deepest segment wins over a more general parent.
        let r = category_row("10_electric/40_signaling/lamp_red.elmt").unwrap();
        assert_eq!(r.category, ["Lamp", "QET", "Lamp"]);
        assert!(category_row("embed://import/misc/thing.elmt").is_none());
        let g = generated_of(resolve("simple", &BTreeMap::new(), Some("x/heater.elmt"), 3));
        assert_eq!(cat(&g), ["Heater", "QET", "Heater"]);
        assert_eq!(g.finding, None);
    }

    #[test]
    fn black_box_and_thumbnail() {
        let g = generated_of(resolve("simple", &BTreeMap::new(), None, 4));
        assert_eq!(cat(&g), ["BlackBox", "QET", "Generic"]);
        assert_eq!(g.contact_kind, ContactKind::BlackBox);
        assert_eq!(
            g.finding.as_ref().map(|f| (f.0, f.1)),
            Some((Code::BLACK_BOX, Severity::Warning))
        );
        let g = generated_of(resolve("thumbnail", &BTreeMap::new(), None, 0));
        assert_eq!(cat(&g), ["Graphic", "QET", "Thumbnail"]);
        assert_eq!(g.finding, None);
    }

    #[test]
    fn table_rows_are_identifiers_and_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for row in CATEGORY_TABLE {
            assert!(category_key(row.category).is_some(), "{row:?}");
            assert!(seen.insert(row.keyword), "duplicate {}", row.keyword);
        }
    }
}
