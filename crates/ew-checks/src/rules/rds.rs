// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-RDS rules, pack EW-IEC81346 (doc 15 §4.2; numbering in doc 07; the parser and the identifier sets in
//! `ew-rds`, ADR-0026). WP-S1.16 part 4 adds EW-RDS-001, -002, -003, -007, -009 and -010.
//!
//! # Determinism (WP-S1.16 §10)
//!
//! Texts name devices by their derived tags and structure nodes by their sign and code; lists inside a message are in
//! natural order of their printed text. IDs enter only targets and fingerprints.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{Ident, natural_cmp};
use ew_derive::ew_rds::{
    Aspect, IdentifierSet, KnownCodes, LetterCheck, LetterCode, RdsSyntax, ReservedLetter,
    code_has_separators, parse_strict, spelling_key,
};
use ew_derive::ew_schema::{Device, Kind, Open, StructureNode};
use ew_derive::{DeviceTag, MainFunction, Q};

use crate::engine::touched_ids;
use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};
use crate::rules::term::{case_setting, full_tag};

/// EW-RDS-004 "Duplicate designation" (doc 15 §4.3): conflict, o r, error, project (the catalogue's p is
/// not declared at MVP).
pub const DUPLICATE_DESIGNATION: RuleMeta = RuleMeta {
    id: "EW-RDS-004",
    code: "017004",
    pack: "EW-IEC81346",
    title: "Duplicate designation",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-RDS-004: two devices print the same full designation (FR-NUM-016). Terminals are excluded: their
/// designation is unique per strip (EW-TERM-002). Designations compare under the project's case setting;
/// an unnumbered designation (it still contains `?`, EW-IMP-122) is skipped.
///
/// One finding per duplicated designation, targets every device of the group, no discriminator.
///
/// Deviations: the catalogue lists the scope as device, but the grouping is project-wide, so the rule is
/// evaluated once in the project scope. Prevent mode is not declared at MVP (owner ruling; ADR-0009
/// decision 3 is amended by the design pass): tag uniqueness on commit would need every derived tag on
/// each commit.
#[derive(Clone, Copy, Debug)]
pub struct DuplicateDesignation;

impl Rule for DuplicateDesignation {
    fn meta(&self) -> &'static RuleMeta {
        &DUPLICATE_DESIGNATION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let case = case_setting(q);
        let mut groups: BTreeMap<String, (String, Vec<Id<Device>>)> = BTreeMap::new();
        for (id, device) in q.devices().iter() {
            if device.aspects.terminal.is_some() {
                continue;
            }
            let tag = q.get::<DeviceTag>(id);
            let Some(tag) = tag.as_ref().as_ref() else {
                continue;
            };
            if tag.full.contains('?') {
                continue;
            }
            let key = match tag.full.parse::<Ident>() {
                Ok(ident) => ident.comparison_key(case).into_owned(),
                Err(_) => tag.full.clone(),
            };
            groups
                .entry(key)
                .or_insert_with(|| (tag.full.clone(), Vec::new()))
                .1
                .push(id);
        }
        groups
            .into_values()
            .filter(|(_, members)| members.len() > 1)
            .map(|(full, members)| {
                let message = format!("`{full}` is printed by {} devices", members.len());
                Finding::new(
                    &DUPLICATE_DESIGNATION,
                    members.iter().map(|m| Target::new(Kind::Device, *m)),
                    "",
                    full,
                    message,
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// Shared helpers

/// The known structure codes of the project: every node's code and aliases, per aspect (FR-PRJ-017, FR-PRJ-028). A
/// node of an unknown aspect, or a code the registry refuses (a conflict the importer reports), is left out.
fn known_codes(q: &Q) -> KnownCodes {
    let mut known = KnownCodes::new();
    let mut nodes: Vec<(Id<StructureNode>, &StructureNode)> = q
        .structure_nodes()
        .iter()
        .map(|(id, n)| (id, n.as_ref()))
        .collect();
    nodes.sort_by_key(|(id, _)| *id);
    for (_, n) in &nodes {
        if let Open::Known(aspect) = n.aspect {
            let _ = known.add_code(aspect, n.code.as_str());
        }
    }
    for (_, n) in &nodes {
        if let Open::Known(aspect) = n.aspect {
            for alias in &n.aliases {
                let _ = known.add_alias(aspect, alias.as_str(), n.code.as_str());
            }
        }
    }
    known
}

/// A structure node as printed: its aspect's sign and its code (`+J-E-2416`).
fn node_text(n: &StructureNode) -> String {
    match n.aspect {
        Open::Known(aspect) => format!("{}{}", RdsSyntax::default().sign(aspect), n.code.as_str()),
        Open::Unknown(_) => n.code.as_str().to_owned(),
    }
}

/// The project's identifier set (ADR-0026 decision 5): the project's setting, else the pinned profile's; `None` for a
/// key no shipped set has.
fn identifier_set(q: &Q) -> Option<&'static IdentifierSet> {
    let key = q
        .project()
        .settings
        .identifier_set
        .as_ref()
        .map_or_else(|| q.profile().identifier_set.clone(), |k| k.as_str().to_owned());
    IdentifierSet::by_key(&key)
}

/// A device whose tag is its own letter code and counter: not a terminal (its tag is its strip's) and with a tag.
fn own_tag_device(q: &Q, device: Id<Device>) -> Option<std::sync::Arc<Device>> {
    let d = q.devices().get(device)?;
    (d.aspects.terminal.is_none()).then(|| d.clone())
}

// ---------------------------------------------------------------------------------------------------------
// EW-RDS-001

/// EW-RDS-001 "Designation does not parse" (doc 15 §4.3): conflict, o r, error, device.
pub const DESIGNATION_DOES_NOT_PARSE: RuleMeta = RuleMeta {
    id: "EW-RDS-001",
    code: "017001",
    pack: "EW-IEC81346",
    title: "Designation does not parse",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-RDS-001: a device whose derived full designation does not parse under the strict IEC 81346 grammar
/// (`ew_rds::parse_strict`, with the project's structure codes as atomic tokens) (FR-NUM-001; D-03). Left out: a
/// terminal (its designation is its strip's with a connection point, EW-TERM-001 and -002 own the rest) and an
/// unnumbered tag (it contains `?`, EW-IMP-122 and the numbering commands own it). A frozen tag that deviates from the
/// scheme is EW-NUM-001's, which supersedes this rule for the device. One finding per device: target the device,
/// discriminator `parse`.
#[derive(Clone, Copy, Debug)]
pub struct DesignationDoesNotParse;

impl Rule for DesignationDoesNotParse {
    fn meta(&self) -> &'static RuleMeta {
        &DESIGNATION_DOES_NOT_PARSE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        if own_tag_device(q, device).is_none() {
            return Vec::new();
        }
        let tag = q.get::<DeviceTag>(device);
        let Some(tag) = tag.as_ref().as_ref() else {
            return Vec::new();
        };
        if tag.full.contains('?') {
            return Vec::new();
        }
        let Err(e) = parse_strict(&tag.full, &RdsSyntax::default(), &known_codes(q)) else {
            return Vec::new();
        };
        vec![Finding::new(
            &DESIGNATION_DOES_NOT_PARSE,
            [Target::new(Kind::Device, device)],
            "parse",
            tag.full.clone(),
            format!(
                "{} does not parse as an IEC 81346 designation: {}",
                tag.full, e.kind
            ),
        )]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-RDS-002

/// EW-RDS-002 "Separator in a structure code" (doc 15 §4.3): hygiene, o, info, project.
pub const SEPARATOR_IN_CODE: RuleMeta = RuleMeta {
    id: "EW-RDS-002",
    code: "017002",
    pack: "EW-IEC81346",
    title: "Separator in a structure code",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("o"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-RDS-002: a structure node code that contains a prefix or separator character (`ew_rds::code_has_separators`),
/// so exported full designations are ambiguous to third-party parsers: `+J-E-2416` reads as location `J`, then
/// products `-E` and `-2416` (FR-CHK-027, STD RD-02; D-03). EW keeps codes atomic and never re-parses them; the
/// finding warns about exports. Online only, so it runs in `drafting` and `full`; `pre-delivery` switches it off. One
/// finding per node: target the node, discriminator `separator`.
#[derive(Clone, Copy, Debug)]
pub struct SeparatorInCode;

impl Rule for SeparatorInCode {
    fn meta(&self) -> &'static RuleMeta {
        &SEPARATOR_IN_CODE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let syntax = RdsSyntax::default();
        let mut rows: Vec<(String, Id<StructureNode>)> = q
            .structure_nodes()
            .iter()
            .filter(|(_, n)| code_has_separators(n.code.as_str(), &syntax))
            .map(|(id, n)| (node_text(n), id))
            .collect();
        rows.sort_by(|a, b| {
            natural_cmp(&a.0, &b.0)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.cmp(&b.1))
        });
        rows.into_iter()
            .map(|(text, id)| {
                Finding::new(
                    &SEPARATOR_IN_CODE,
                    [Target::new(Kind::StructureNode, id)],
                    "separator",
                    text.clone(),
                    format!(
                        "the structure code {text} contains separator characters; tools that read exported \
                         designations may split it into several levels"
                    ),
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-RDS-003

/// EW-RDS-003 "Letter code against function" (doc 15 §4.3): hygiene, o r, warning, device.
pub const LETTER_AGAINST_FUNCTION: RuleMeta = RuleMeta {
    id: "EW-RDS-003",
    code: "017003",
    pack: "EW-IEC81346",
    title: "Letter code against function",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-RDS-003's parameter `frozen_severity` (doc 15 §4.3, ADR-0026 decision 6): the severity for a frozen tag. The
/// shipped value is the rule's own (warning for frozen and new tags alike).
pub const FROZEN_SEVERITY: Severity = Severity::Warning;

/// EW-RDS-003: a device whose letter code (`TagSpec.letter`) is neither the letter nor an accepted alternative of its
/// main function's category in the project's identifier set (`IdentifierSet::check_letter`), with the set's letter as
/// the suggestion (FR-CHK-026, STD RD-03; D-03, EF-18). A frozen tag reports at [`FROZEN_SEVERITY`]. Left out: a
/// terminal, a device without a main function, a main function of category `Assembly` (the generic black-box and
/// overview definitions name no function; Golden A's -102F2, -102G1 and the CPX modules) and a category the set does
/// not map. The shipped sets are not
/// verified against the standards (ADR-0026), and the message says so. One finding per device: target the device,
/// discriminator `letter`. On Golden A exactly T55 (T for a temperature switch; suggestion B, BT under the 2019 set).
#[derive(Clone, Copy, Debug)]
pub struct LetterAgainstFunction;

impl Rule for LetterAgainstFunction {
    fn meta(&self) -> &'static RuleMeta {
        &LETTER_AGAINST_FUNCTION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(d) = own_tag_device(q, device) else {
            return Vec::new();
        };
        let Some(set) = identifier_set(q) else {
            return Vec::new();
        };
        let Some(f) = *q.get::<MainFunction>(device) else {
            return Vec::new();
        };
        let Some(def) = q.functions().get(f).and_then(|f| q.fndefs().get(f.def)) else {
            return Vec::new();
        };
        let (category, group, _) = &def.item.category;
        // The generic `Assembly/BlackBox` and `Assembly/Overview` definitions say nothing about what the device does.
        if category.as_str() == "Assembly" {
            return Vec::new();
        }
        let letter = d.tag.letter.as_str();
        let LetterCheck::Mismatch(row) = set.check_letter(category.as_str(), Some(group.as_str()), letter)
        else {
            return Vec::new();
        };
        let tag = full_tag(q, device, "device");
        let unverified = if row.verified() {
            ""
        } else {
            " (the set is not verified against the standard)"
        };
        let severity = if d.tag.frozen.is_some() {
            FROZEN_SEVERITY
        } else {
            LETTER_AGAINST_FUNCTION.severity
        };
        vec![
            Finding::new(
                &LETTER_AGAINST_FUNCTION,
                [Target::new(Kind::Device, device)],
                "letter",
                tag.clone(),
                format!(
                    "{tag}: the letter code {letter} does not match its function category {}; the identifier set {} \
                     suggests {}{unverified}",
                    category.as_str(),
                    set.key(),
                    row.letter().as_str()
                ),
            )
            .arg("device", tag)
            .arg("letter", letter.to_string())
            .arg("category", category.as_str())
            .arg("set", set.key().to_string())
            .arg("suggested", row.letter().as_str())
            .arg("verified", if row.verified() { "yes" } else { "no" })
            .with_severity(severity),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-RDS-007

/// EW-RDS-007 "Reserved letters" (doc 15 §4.3): hygiene, o r, warning, device.
pub const RESERVED_LETTERS: RuleMeta = RuleMeta {
    id: "EW-RDS-007",
    code: "017007",
    pack: "EW-IEC81346",
    title: "Reserved letters",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-RDS-007: a device's letter code with `I` or `O` in it, or whose class letter the project's identifier set
/// reserves (`IdentifierSet::reserved_letter_findings`) (FR-CHK-060, STD RD-07). A letter code that is not 1–3
/// upper-case letters is not a letter code of a set and is left out, as is a terminal. One finding per device with
/// every reason: target the device, discriminator `reserved`.
#[derive(Clone, Copy, Debug)]
pub struct ReservedLetters;

impl Rule for ReservedLetters {
    fn meta(&self) -> &'static RuleMeta {
        &RESERVED_LETTERS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(d) = own_tag_device(q, device) else {
            return Vec::new();
        };
        let Some(set) = identifier_set(q) else {
            return Vec::new();
        };
        let Ok(letter) = d.tag.letter.as_str().parse::<LetterCode>() else {
            return Vec::new();
        };
        let reasons: Vec<String> = set
            .reserved_letter_findings(letter)
            .into_iter()
            .map(|r| match r {
                ReservedLetter::IOrO { letter: c, .. } => format!("{c} is not used in letter codes"),
                ReservedLetter::ReservedClass(c) => {
                    format!("the identifier set {} reserves the class letter {c}", set.key())
                }
                _ => "the identifier set reserves it".to_owned(),
            })
            .collect();
        if reasons.is_empty() {
            return Vec::new();
        }
        let tag = full_tag(q, device, "device");
        vec![Finding::new(
            &RESERVED_LETTERS,
            [Target::new(Kind::Device, device)],
            "reserved",
            tag.clone(),
            format!(
                "{tag}: the letter code {} uses reserved letters: {}",
                letter.as_str(),
                reasons.join("; ")
            ),
        )]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-RDS-009

/// EW-RDS-009 "Confusable character" (doc 15 §4.3): conflict, o p r, error, device.
pub const CONFUSABLE_CHARACTER: RuleMeta = RuleMeta {
    id: "EW-RDS-009",
    code: "017009",
    pack: "EW-IEC81346",
    title: "Confusable character",
    kind: RuleKind::Conflict,
    modes: Modes::letters("opr"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// The Latin letter a non-Latin character is drawn like, for the confusable skeleton of EW-RDS-009: the Cyrillic and
/// Greek capitals and small letters whose glyph is a Latin letter's in the bundled faces, and the full-width Latin
/// letters and digits. A character outside the table has no Latin look-alike.
fn latin_lookalike(c: char) -> Option<char> {
    let fullwidth = |c: char, from: u32, base: u8| char::from_u32(u32::from(base) + (c as u32 - from));
    Some(match c {
        // Cyrillic capitals.
        'А' => 'A',
        'В' => 'B',
        'С' => 'C',
        'Е' => 'E',
        'Н' => 'H',
        'І' => 'I',
        'Ј' => 'J',
        'К' => 'K',
        'М' => 'M',
        'О' => 'O',
        'Р' => 'P',
        'Ѕ' => 'S',
        'Т' => 'T',
        'Х' => 'X',
        'У' => 'Y',
        // Cyrillic small letters.
        'а' => 'a',
        'с' => 'c',
        'е' => 'e',
        'і' => 'i',
        'ј' => 'j',
        'о' => 'o',
        'р' => 'p',
        'ѕ' => 's',
        'х' => 'x',
        'у' => 'y',
        // Greek capitals.
        'Α' => 'A',
        'Β' => 'B',
        'Ε' => 'E',
        'Ζ' => 'Z',
        'Η' => 'H',
        'Ι' => 'I',
        'Κ' => 'K',
        'Μ' => 'M',
        'Ν' => 'N',
        'Ο' => 'O',
        'Ρ' => 'P',
        'Τ' => 'T',
        'Υ' => 'Y',
        'Χ' => 'X',
        // Greek small omicron.
        'ο' => 'o',
        // Full-width Latin letters and digits.
        'Ａ'..='Ｚ' => fullwidth(c, 0xFF21, b'A')?,
        'ａ'..='ｚ' => fullwidth(c, 0xFF41, b'a')?,
        '０'..='９' => fullwidth(c, 0xFF10, b'0')?,
        _ => return None,
    })
}

/// The confusable characters of `text`: `None` unless the text has a non-ASCII character and every one of them has a
/// Latin look-alike (its skeleton is pure ASCII: a tag typed on a Cyrillic keyboard); then the look-alikes as
/// `U+XXXX (looks like Y)`, in text order. A genuinely non-Latin identifier gives `None`.
fn confusables(text: &str) -> Option<Vec<String>> {
    let foreign: Vec<char> = text.chars().filter(|c| !c.is_ascii()).collect();
    if foreign.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for c in foreign {
        let latin = latin_lookalike(c)?;
        out.push(format!("U+{:04X} (looks like {latin})", c as u32));
    }
    Some(out)
}

/// The identifiers of a device that EW-RDS-009 reads, as `(what, text)`: its letter code, its frozen tag and, for a
/// terminal, its designation.
fn identifiers(d: &Device) -> Vec<(&'static str, String)> {
    let mut out = vec![("letter code", d.tag.letter.as_str().to_owned())];
    if let Some(f) = &d.tag.frozen {
        out.push(("frozen tag", f.text.as_str().to_owned()));
    }
    if let Some(t) = &d.aspects.terminal {
        out.push(("terminal designation", t.designation.as_str().to_owned()));
    }
    out
}

/// The EW-RDS-009 findings of one device.
fn confusable_findings(q: &Q, device: Id<Device>, d: &Device) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut tag: Option<String> = None;
    for (what, text) in identifiers(d) {
        let Some(chars) = confusables(&text) else {
            continue;
        };
        let tag = tag.get_or_insert_with(|| full_tag(q, device, "device")).clone();
        out.push(Finding::new(
            &CONFUSABLE_CHARACTER,
            [Target::new(Kind::Device, device)],
            what,
            tag.clone(),
            format!(
                "the {what} {text} of {tag} contains non-Latin characters that look like Latin ones: {}; search and \
                 cross-references cannot find it; retype it in Latin letters",
                chars.join(", ")
            ),
        ));
    }
    out
}

/// EW-RDS-009: a device identifier (the letter code, the frozen tag, a terminal's designation) that contains non-ASCII
/// characters every one of which looks like a Latin letter or digit, such as a Cyrillic "А" in a tag: its confusable
/// skeleton is pure ASCII, so it is unreachable by search and breaks cross-references (FR-I18N-011). A genuinely
/// non-Latin identifier (one with a character that has no Latin look-alike) is no finding. One finding per identifier:
/// target the device, discriminator the identifier's kind.
///
/// Prevent: a touched device whose identifier the batch created or changed and that is now confusable; a confusable
/// identifier the base already held does not refuse an edit.
#[derive(Clone, Copy, Debug)]
pub struct ConfusableCharacter;

impl Rule for ConfusableCharacter {
    fn meta(&self) -> &'static RuleMeta {
        &CONFUSABLE_CHARACTER
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(d) = q.devices().get(device) else {
            return Vec::new();
        };
        confusable_findings(q, device, d)
    }

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let mut out = Vec::new();
        for id in touched_ids::<Device>(touched, Kind::Device) {
            let Some(d) = after.devices().get(id) else {
                continue;
            };
            let held: BTreeSet<(&'static str, String)> = before
                .devices()
                .get(id)
                .map(|b| identifiers(b).into_iter().collect())
                .unwrap_or_default();
            let fresh: Vec<Finding> = confusable_findings(after, id, d)
                .into_iter()
                .filter(|f| {
                    identifiers(d)
                        .into_iter()
                        .find(|(w, _)| *w == f.discriminator)
                        .is_none_or(|t| !held.contains(&t))
                })
                .collect();
            out.extend(fresh);
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-RDS-010

/// EW-RDS-010 "Structure-code variants" (doc 15 §4.3): conflict, o r, warning, project.
pub const STRUCTURE_CODE_VARIANTS: RuleMeta = RuleMeta {
    id: "EW-RDS-010",
    code: "017010",
    pack: "EW-IEC81346",
    title: "Structure-code variants",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-RDS-010: two or more structure nodes of one aspect whose codes differ only in separators or case
/// (`ew_rds::spelling_key`: `+JE2416` against `+J-E-2416`) (FR-PRJ-018; D-03, MF-02). One finding per group: targets
/// every node of the group, discriminator `spelling`. The catalogue's merge quick fix waits for a structure-merge
/// command (none exists; WP-S1.16 §11.2 item 6, §13.1).
#[derive(Clone, Copy, Debug)]
pub struct StructureCodeVariants;

impl Rule for StructureCodeVariants {
    fn meta(&self) -> &'static RuleMeta {
        &STRUCTURE_CODE_VARIANTS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        type Members = Vec<(String, Id<StructureNode>)>;
        let mut groups: BTreeMap<(String, String), Members> = BTreeMap::new();
        for (id, n) in q.structure_nodes().iter() {
            let Open::Known(aspect) = n.aspect else {
                continue;
            };
            groups
                .entry((aspect_key(aspect), spelling_key(n.code.as_str())))
                .or_default()
                .push((node_text(n), id));
        }
        let mut rows: Vec<(String, Members)> = groups
            .into_values()
            .filter(|members| members.len() > 1)
            .map(|mut members| {
                members.sort_by(|a, b| {
                    natural_cmp(&a.0, &b.0)
                        .then_with(|| a.0.cmp(&b.0))
                        .then_with(|| a.1.cmp(&b.1))
                });
                (members[0].0.clone(), members)
            })
            .collect();
        rows.sort_by(|a, b| natural_cmp(&a.0, &b.0).then_with(|| a.0.cmp(&b.0)));
        rows.into_iter()
            .map(|(first, members)| {
                let texts: Vec<&str> = members.iter().map(|(t, _)| t.as_str()).collect();
                Finding::new(
                    &STRUCTURE_CODE_VARIANTS,
                    members.iter().map(|(_, id)| Target::new(Kind::StructureNode, *id)),
                    "spelling",
                    first,
                    format!(
                        "the structure codes {} differ only in separators or case; merge them into one node and keep \
                         the other spellings as aliases",
                        texts.join(", ")
                    ),
                )
            })
            .collect()
    }
}

/// The aspect as a sort key.
fn aspect_key(aspect: Aspect) -> String {
    aspect.to_string()
}
