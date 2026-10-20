// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Terminal elements → terminal strips and terminals, for projects without QET 0.100 strip data (doc 14
//! §4.11 "Older projects", spike form).
//!
//! QET draws every terminal as an element of link type `terminal`. On each folio the terminal elements are
//! grouped into **lines**: elements turned a quarter (orientation 1 or 3) stack vertically and form a
//! column per x; the others form a row per y. Along a line, an element whose label is a strip designation
//! (`X1031`, `-X1031`, `+JE2416-X1031`, `1X1`: letters and digits after an optional structure prefix)
//! starts a **run** of that strip; the elements up to the next such label belong to it. Elements before the
//! first strip label of a line form an inferred strip (EW-IMP-005). Runs with one strip designation become
//! one strip with several segments (EW-IMP-109).
//!
//! Each terminal element becomes a terminal device (`DeviceKind::Terminal`, `Terminal { strip, seq,
//! designation }`) with its main function and representation. `seq` counts the strip's terminals in run
//! order; the designation is the element's first `UserText` dynamic text (where the reference set writes
//! "1+", "PE"), else its label when that is not a strip designation, else the sequence number (EW-IMP-008:
//! an empty designation cannot be stored, so EW-TERM-001 cannot report it).

use std::collections::BTreeMap;

use ew_interop_api::{Code, Severity};
use ew_mltext::{Ident, MlText};
use ew_schema::{DeviceAspects, DeviceKind, FnRole, Side, Strip, TagSpec, Terminal};
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

/// The strip identity of a run.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum StripKey {
    /// By its product designation without sign (`X1031`).
    Named(String),
    /// Inferred for the unlabelled start of line `line` of diagram `diagram`.
    Inferred { diagram: usize, line: usize },
}

/// One run: its strip, the label that started it (if any) and its elements in line order.
struct Run<'e, 'a> {
    strip: StripKey,
    label: Option<&'a str>,
    diagram: usize,
    line: usize,
    members: Vec<&'e Elem<'a>>,
}

/// Maps the terminal elements `terminals` (in document order).
pub(crate) fn map(mx: &mut Mx<'_, '_>, terminals: &[&Elem<'_>]) {
    // Lines: (diagram, vertical, cross coordinate) → elements.
    let mut lines: BTreeMap<(usize, bool, i64), Vec<&Elem<'_>>> = BTreeMap::new();
    for e in terminals {
        let vertical = e.el.orientation % 2 == 1;
        let cross = if vertical { e.pos.x.0 } else { e.pos.y.0 };
        lines.entry((e.diagram, vertical, cross)).or_default().push(e);
    }
    let mut runs: Vec<Run<'_, '_>> = Vec::new();
    for (line_no, ((diagram, vertical, _), mut members)) in lines.into_iter().enumerate() {
        members.sort_by_key(|e| {
            let along = if vertical { e.pos.y.0 } else { e.pos.x.0 };
            (along, e.key.clone())
        });
        for e in members {
            let label = e.el.info_nonempty("label").filter(|l| is_strip_label(l));
            match label {
                Some(l) => {
                    let (_, product) = split_structure(l.trim());
                    let name = product.strip_prefix('-').unwrap_or(product).to_owned();
                    runs.push(Run {
                        strip: StripKey::Named(name),
                        label: Some(l),
                        diagram,
                        line: line_no,
                        members: vec![e],
                    });
                }
                None => match runs.last_mut() {
                    Some(r) if r.line == line_no => r.members.push(e),
                    _ => runs.push(Run {
                        strip: StripKey::Inferred {
                            diagram,
                            line: line_no,
                        },
                        label: None,
                        diagram,
                        line: line_no,
                        members: vec![e],
                    }),
                },
            }
        }
    }

    // Strips in order of first appearance.
    let mut order: Vec<StripKey> = Vec::new();
    let mut by_strip: BTreeMap<StripKey, Vec<usize>> = BTreeMap::new();
    for (i, r) in runs.iter().enumerate() {
        let list = by_strip.entry(r.strip.clone()).or_default();
        if list.is_empty() {
            order.push(r.strip.clone());
        }
        list.push(i);
    }
    for key in order {
        let run_ids = &by_strip[&key];
        let first = &runs[run_ids[0]];
        let folio = mx.folio(first.diagram).to_owned();
        let (skey, tag, structure) = match &key {
            StripKey::Named(name) => {
                let label = first.label.unwrap_or(name);
                let (tag, structure, _) = infer_tag(label, &folio, "X");
                (format!("qet:strip:{name}"), tag, structure.map(str::to_owned))
            }
            StripKey::Inferred { diagram, line } => {
                mx.out.inferred += 1;
                let e = first.members[0];
                mx.at(
                    e,
                    Code::imp(5),
                    Severity::Warning,
                    format!(
                        "{} terminal elements without strip label inferred as one strip",
                        first.members.len()
                    ),
                );
                (
                    format!("qet:strip:d{diagram}:{line}"),
                    TagSpec::new(x_letter(), None),
                    None,
                )
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
        if run_ids.len() > 1 {
            let e = runs[run_ids[1]].members[0];
            mx.at(
                e,
                Code::imp(109),
                Severity::Info,
                format!(
                    "strip drawn in {} segments with one designation; merged into one strip",
                    run_ids.len()
                ),
            );
        }
        let mut seq: u32 = 0;
        let mut numbered = 0u32;
        for &ri in run_ids {
            for e in &runs[ri].members {
                seq += 1;
                let mut textra = BTreeMap::new();
                let label = e.el.info_nonempty("label");
                let found = user_text(e.el).or(label.filter(|l| !is_strip_label(l)));
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
        }
        if numbered > 0 {
            let e = first.members[0];
            mx.at(
                e,
                Code::FOREIGN_VALUE,
                Severity::Warning,
                format!(
                    "{numbered} terminals of this strip without a usable designation: numbered by sequence"
                ),
            );
        }
    }
}

fn x_letter() -> Ident {
    Ident::from_input("X").unwrap_or_else(|_| unreachable!("a letter is an identifier"))
}

fn seq_ident(seq: u32) -> Ident {
    Ident::from_input(&seq.to_string()).unwrap_or_else(|_| unreachable!("a decimal is an identifier"))
}
