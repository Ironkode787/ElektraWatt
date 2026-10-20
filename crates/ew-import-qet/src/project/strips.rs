// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Terminal elements → terminal strips and terminals, for projects without QET 0.100 strip data (doc 14
//! §4.11 "Older projects", §14 item 5).
//!
//! QET draws every terminal as an element of link type `terminal`. Each one joins a strip by, in this order:
//!
//! 1. **Label prefix**: a label `X1031:1+` (optional structure prefix, `+JE2416-X1031:1+`) names the strip
//!    before the `:` and the designation after it.
//! 2. **Strip header**: an element whose label is a strip designation (`X1031`, `-X1031`, `+JE2416-X1031`,
//!    `1X1`: an optional structure prefix and sign, then digits, letters and digits) heads its line; the
//!    following elements of the line join it. Lines are per folio: elements turned a quarter (orientation 1
//!    or 3) stack vertically and form a column per x, the others a row per y. The header itself is a
//!    terminal of its strip.
//! 3. **Fallback**: every other terminal element of a folio joins the strip `X?<page>` of that folio
//!    (EW-IMP-005, for review).
//!
//! A strip is identified by its own designation **including the structure prefix** (`+A-X1` and `+B-X1`
//! are two strips; source key `qet:strip:<structure>-<product>`), the fallback strip by its folio key
//! (`qet:strip:folio:<folio key>`), so re-importing an edited file keeps strip IDs (FR-IMP-040). One strip
//! drawn in several lines or folios is merged and reported (EW-IMP-109).
//!
//! Each terminal element becomes a terminal device (`DeviceKind::Terminal`, `Terminal { strip, seq,
//! designation }`) with its main function and representation; `seq` counts the strip's terminals in line
//! order (folio, line, position along the line). The designation is the label suffix after `:`, else the
//! first `UserText` dynamic text (where the reference set writes "1+", "PE"), else a label that is not a
//! strip designation, else the sequence number (EW-IMP-008: an empty designation cannot be stored, so
//! EW-TERM-001 cannot report it).
//!
//! QET 0.100+ `<terminal_strips>` data (strips, bridges) is not read yet: the importer reports the member
//! as not imported (EW-IMP-004), so bridges stored there are dropped.

use std::collections::{BTreeMap, BTreeSet};

use ew_interop_api::{Code, Severity};
use ew_mltext::{Ident, MlText};
use ew_schema::{DeviceAspects, DeviceKind, FnRole, FrozenTag, Side, Strip, TagScheme, TagSpec, Terminal};
use serde_json::Value;

use super::elements::{Elem, FnSpec, Mx, infer_tag, split_structure, user_text};

/// Whether `label` designates a strip: an optional structure prefix, an optional product sign, then digits,
/// letters and digits (`1X1`, `X1031`).
pub(crate) fn is_strip_label(label: &str) -> bool {
    let (_, product) = split_structure(label.trim());
    let p = product.strip_prefix('-').unwrap_or(product);
    let rest = p.trim_start_matches(|c: char| c.is_ascii_digit());
    let letters = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_alphabetic()).len();
    let digits = &rest[letters..];
    letters > 0 && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// The strip identity: structure prefix and product designation without sign, or the folio fallback.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum StripKey {
    /// A strip named by a header or a label prefix.
    Named { structure: String, product: String },
    /// The fallback strip `X?<page>` of a folio (diagram index).
    Fallback(usize),
}

/// The strip identity of a strip designation such as `+JE2416-X1031` or `X1031`.
fn named(designation: &str) -> StripKey {
    let (structure, product) = split_structure(designation.trim());
    StripKey::Named {
        structure: structure.unwrap_or("").to_owned(),
        product: product.strip_prefix('-').unwrap_or(product).to_owned(),
    }
}

/// One terminal assigned to a strip: the element, its line and the designation its label gives.
struct Member<'e, 'a> {
    e: &'e Elem<'a>,
    line: (usize, bool, i64),
    designation: Option<&'a str>,
}

/// One strip: its first label (for the tag) and its members in line order.
struct StripInfo<'e, 'a> {
    label: Option<&'a str>,
    members: Vec<Member<'e, 'a>>,
}

/// Adds member `m` to strip `key` (created on first use, in order of first appearance).
fn add<'e, 'a>(
    strips: &mut BTreeMap<StripKey, StripInfo<'e, 'a>>,
    order: &mut Vec<StripKey>,
    key: StripKey,
    label: Option<&'a str>,
    m: Member<'e, 'a>,
) {
    let info = strips.entry(key.clone()).or_insert_with(|| {
        order.push(key);
        StripInfo {
            label: None,
            members: Vec::new(),
        }
    });
    if info.label.is_none() {
        info.label = label;
    }
    info.members.push(m);
}

/// Maps the terminal elements `terminals` (in document order).
pub(crate) fn map(mx: &mut Mx<'_, '_>, terminals: &[&Elem<'_>]) {
    // Lines: (diagram, vertical, cross coordinate) → elements in order along the line.
    let mut lines: BTreeMap<(usize, bool, i64), Vec<&Elem<'_>>> = BTreeMap::new();
    for e in terminals {
        let vertical = e.el.orientation % 2 == 1;
        let cross = if vertical { e.pos.x.0 } else { e.pos.y.0 };
        lines.entry((e.diagram, vertical, cross)).or_default().push(e);
    }
    let mut order: Vec<StripKey> = Vec::new();
    let mut strips: BTreeMap<StripKey, StripInfo<'_, '_>> = BTreeMap::new();
    for (line, mut members) in lines {
        let vertical = line.1;
        members.sort_by_key(|e| {
            let along = if vertical { e.pos.y.0 } else { e.pos.x.0 };
            (along, e.key.clone())
        });
        let mut header: Option<StripKey> = None;
        for e in members {
            let label = e.el.info_nonempty("label");
            match label {
                Some(l) if l.contains(':') => {
                    let (strip, desig) = l.split_once(':').unwrap_or((l, ""));
                    let desig = desig.trim();
                    add(
                        &mut strips,
                        &mut order,
                        named(strip),
                        Some(strip),
                        Member {
                            e,
                            line,
                            designation: (!desig.is_empty()).then_some(desig),
                        },
                    );
                }
                Some(l) if is_strip_label(l) => {
                    let key = named(l);
                    header = Some(key.clone());
                    add(
                        &mut strips,
                        &mut order,
                        key,
                        Some(l),
                        Member {
                            e,
                            line,
                            designation: None,
                        },
                    );
                }
                _ => {
                    let key = header.clone().unwrap_or(StripKey::Fallback(e.diagram));
                    add(
                        &mut strips,
                        &mut order,
                        key,
                        None,
                        Member {
                            e,
                            line,
                            designation: None,
                        },
                    );
                }
            }
        }
    }

    for key in order {
        let Some(info) = strips.remove(&key) else { continue };
        let first = info.members[0].e;
        let folio = mx.folio(first.diagram).to_owned();
        let (skey, tag, structure) = match &key {
            StripKey::Named { structure, product } => {
                let label = info.label.unwrap_or(product);
                let (tag, _, _) = infer_tag(label, &folio, "X");
                (
                    format!("qet:strip:{structure}-{product}"),
                    tag,
                    (!structure.is_empty()).then(|| structure.clone()),
                )
            }
            StripKey::Fallback(d) => {
                mx.out.inferred += 1;
                mx.at(
                    first,
                    Code::imp(5),
                    Severity::Warning,
                    format!(
                        "{} terminal elements without strip header or label prefix inferred as strip X?{folio}",
                        info.members.len()
                    ),
                );
                let page_key = mx.inp.pages.pages[*d].key.clone();
                (format!("qet:strip:folio:{page_key}"), fallback_tag(&folio), None)
            }
        };
        let mut extra = BTreeMap::new();
        if let Some(s) = structure {
            extra.insert("qet.structure".to_owned(), Value::String(s));
        }
        let strip = mx.device(
            &skey,
            tag,
            MlText::default(),
            None,
            DeviceAspects {
                strip: Some(Strip {
                    label_mode: Default::default(),
                    designation_scheme: None,
                    accessories: Vec::new(),
                    tag_print: Default::default(),
                    groups: Vec::new(),
                    field_side: Side::Side2,
                    extra: BTreeMap::new(),
                }),
                ..DeviceAspects::default()
            },
            extra,
        );
        let segments: BTreeSet<(usize, bool, i64)> = info.members.iter().map(|m| m.line).collect();
        if matches!(key, StripKey::Named { .. }) && segments.len() > 1 {
            mx.at(
                first,
                Code::imp(109),
                Severity::Info,
                format!(
                    "strip drawn in {} segments with one designation; merged into one strip",
                    segments.len()
                ),
            );
        }
        let mut seq: u32 = 0;
        let mut numbered = 0u32;
        for m in &info.members {
            let e = m.e;
            seq += 1;
            let mut textra = BTreeMap::new();
            let label = e.el.info_nonempty("label");
            let found = m
                .designation
                .or_else(|| user_text(e.el))
                .or(label.filter(|l| !is_strip_label(l) && !l.contains(':')));
            let designation = match found.map(|d| (d, Ident::from_input(d))) {
                Some((_, Ok(d))) => d,
                Some((raw, Err(_))) => {
                    textra.insert("qet.designation".to_owned(), Value::String(raw.to_owned()));
                    numbered += 1;
                    seq_ident(seq)
                }
                None => {
                    textra.insert("qet.designation".to_owned(), Value::String(String::new()));
                    numbered += 1;
                    seq_ident(seq)
                }
            };
            let Some(def) = e.import.fndef.as_ref().map(|f| f.id) else {
                mx.drop_element(
                    e,
                    "the terminal definition references a built-in function definition",
                );
                continue;
            };
            let dkey = format!("qet:el:{}", e.ek);
            let device = mx.device(
                &dkey,
                TagSpec::new(x_letter(), None),
                MlText::default(),
                Some(DeviceKind::Terminal),
                DeviceAspects {
                    terminal: Some(Terminal {
                        strip,
                        seq,
                        designation,
                        levels: Vec::new(),
                        rail_bonded: false,
                        extra: textra,
                    }),
                    ..DeviceAspects::default()
                },
                BTreeMap::new(),
            );
            mx.function_and_rep(e, FnSpec::normal(e, def, device, FnRole::Main));
        }
        if numbered > 0 {
            mx.at(
                first,
                Code::FOREIGN_VALUE,
                Severity::Warning,
                format!(
                    "{numbered} terminals of this strip without a usable designation: numbered by sequence"
                ),
            );
        }
    }
}

/// The tag of the fallback strip of folio `folio`: the frozen designation `X?<folio>` (doc 14 §14 item 5),
/// else the letter X without counter.
fn fallback_tag(folio: &str) -> TagSpec {
    match Ident::from_input(&format!("X?{folio}")) {
        Ok(text) => TagSpec {
            letter: x_letter(),
            counter: None,
            scheme: TagScheme::Default,
            frozen: Some(FrozenTag {
                text,
                sign: Default::default(),
                extra: BTreeMap::new(),
            }),
            extra: BTreeMap::new(),
        },
        Err(_) => TagSpec::new(x_letter(), None),
    }
}

fn x_letter() -> Ident {
    Ident::from_input("X").unwrap_or_else(|_| unreachable!("a letter is an identifier"))
}

fn seq_ident(seq: u32) -> Ident {
    Ident::from_input(&seq.to_string()).unwrap_or_else(|_| unreachable!("a decimal is an identifier"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_labels() {
        assert!(is_strip_label("X1031"));
        assert!(is_strip_label("-X1031"));
        assert!(is_strip_label("+JE2416-X1031"));
        assert!(is_strip_label("1X1"));
        assert!(!is_strip_label("+"));
        assert!(!is_strip_label("1+"));
        assert!(!is_strip_label("PE"));
    }

    #[test]
    fn strip_identity_keeps_the_structure_prefix() {
        assert_ne!(named("+A-X1"), named("+B-X1"));
        assert_eq!(named("-X1"), named("X1"));
        assert_eq!(
            named("+JE2416-X1031"),
            StripKey::Named {
                structure: "+JE2416".into(),
                product: "X1031".into()
            }
        );
    }

    #[test]
    fn the_fallback_strip_is_named_by_its_folio() {
        let t = fallback_tag("103");
        assert_eq!(
            t.frozen.map(|f| f.text.as_str().to_owned()).as_deref(),
            Some("X?103")
        );
    }
}
