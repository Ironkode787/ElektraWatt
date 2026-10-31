// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-reference acceptance on the golden model (WP-S1.08; doc 20 §15, AC-05, AC-20, NFR-REL-07).
//!
//! The expected texts are typed by hand from doc 20 §15. Tests:
//!
//! - `interruption_point_texts_match_section_15` (AC-05, FR-XRF-024): the 22 pairs, 44 texts, anchor cells and
//!   links.
//! - `external_points_print_the_external_target` (FR-XRF-012): the 4 external points.
//! - `main_and_auxiliary_texts_match_section_15` (FR-XRF-015, FR-XRF-016, FR-XRF-005): 9 main and 10
//!   auxiliary texts with their links.
//! - `the_cell_follows_the_anchor_inside_and_across_cells` (AC-20, D-14): an anchor off the cell centre keeps
//!   its cell, an anchor in the next cell prints the next cell; the bus port never prints 3-B0.
//! - `renumbering_folio_3_to_4_updates_the_bus_port_reference` (AC-20, D-01): the cross-reference part.
//! - `xref_texts_do_not_depend_on_record_order` (NFR-REL-07): reversed records.
//! - `page_index_counts_every_page_record_but_not_continuations` (FR-XRF-003): `%f-%l%c` prints "6-A1" for
//!   L-1 on folio 102.
//! - `no_xref_notes_on_the_golden_model`: no `Xref` and no `MalformedShim` note.

use crate::golden_common;

use std::collections::BTreeMap;
use std::str::FromStr;

use ew_api::Session;
use ew_derive::{DeriveNote, XrefRelationOf, XrefText, XrefTextOf};
use ew_ids::Id;
use ew_import_tables::ew_interop_api::ImportBatch;
use ew_schema::{Page, Record, Representation};
use ew_units::{Nm, Pt2};
use golden_common::{batch, ids_by_key, session, session_with};

type Ids = BTreeMap<String, String>;

/// The representation ID of a source key such as `L-1@101` or `-201U1#bus`.
fn rep(ids: &Ids, key: &str) -> Id<Representation> {
    let k = format!("representations|{key}");
    let id = ids.get(&k).unwrap_or_else(|| panic!("no representation {k}"));
    Id::from_str(id).unwrap()
}

fn text(s: &Session, id: Id<Representation>) -> XrefText {
    (*s.query().get::<XrefTextOf>(id)).clone()
}

/// The printed folio of every page, by page ID.
fn folios(b: &ImportBatch) -> BTreeMap<Id<Page>, String> {
    b.records
        .iter()
        .filter_map(|r| match r {
            Record::Page(p) => Some((
                p.id,
                p.folio
                    .label
                    .as_ref()
                    .map_or_else(|| p.folio.number.to_string(), ToString::to_string),
            )),
            _ => None,
        })
        .collect()
}

/// Doc 20 §15: name, source cell, target cell. Printed at the source: the target cell; at the target: the
/// source cell.
const POINTS: [(&str, &str, &str); 22] = [
    ("L-1", "101-H16", "102-A1"),
    ("N-1", "101-H16", "102-A1"),
    ("PE-1.1", "101-G15", "103-G6"),
    ("PE-1.2", "102-A15", "103-G6"),
    ("0VDC", "102-C15", "103-A1"),
    ("PE-0VDC", "102-D15", "103-G1"),
    ("24VDC-1", "102-H15", "103-A1"),
    ("24VDC-2", "102-H15", "103-C1"),
    ("24VDC-3", "102-G15", "103-D1"),
    ("24VDC-4", "102-G15", "103-F1"),
    ("24VDC-AIR1", "103-A6", "106-E1"),
    ("24VDC-AIR2", "103-B6", "106-E1"),
    ("24VDC-AIR3", "103-B6", "106-B1"),
    ("24VDC-AIR4", "103-B6", "106-B1"),
    ("0VDC-AIR1", "103-A14", "106-E1"),
    ("0VDC-AIR2", "103-B14", "106-F1"),
    ("0VDC-AIR3", "103-B14", "106-B1"),
    ("0VDC-AIR4", "103-B14", "106-C1"),
    ("PE-AIR1", "106-E1", "103-G1"),
    ("PE-AIR2", "106-B1", "103-G1"),
    ("PE-1.3", "106-D1", "103-G6"),
    ("PE-1.4", "106-A1", "103-G6"),
];

fn folio_of(cell: &str) -> &str {
    cell.split_once('-').unwrap().0
}

#[test]
fn interruption_point_texts_match_section_15() {
    let b = batch();
    let ids = ids_by_key(&b);
    let folios = folios(&b);
    let s = session(&b);
    let mut checked = 0;
    for (name, src, dst) in POINTS {
        let src_rep = rep(&ids, &format!("{name}@{}", folio_of(src)));
        let dst_rep = rep(&ids, &format!("{name}@{}", folio_of(dst)));
        // (own representation, partner representation, partner cell = the text printed here)
        for (own, partner, shown) in [(src_rep, dst_rep, dst), (dst_rep, src_rep, src)] {
            let t = text(&s, own);
            assert_eq!(t.text, shown, "text of {name} printing {shown}");
            checked += 1;
            assert_eq!(t.links.len(), 1, "{name}: one link");
            assert_eq!(t.links[0].rep, partner, "{name}: link target");
            assert_eq!(t.links[0].range, 0..t.text.len(), "{name}: link covers the text");
            let rel = s.query().get::<XrefRelationOf>(own);
            assert_eq!(rel.targets.len(), 1, "{name}: one target");
            let target = &rel.targets[0];
            assert_eq!(target.rep, partner);
            let cell = target
                .cell
                .as_ref()
                .unwrap_or_else(|| panic!("{name}: target cell"));
            assert_eq!(
                format!("{}-{cell}", folios[&target.page]),
                shown,
                "{name}: anchor"
            );
        }
    }
    assert_eq!(checked, 44);
}

#[test]
fn external_points_print_the_external_target() {
    let b = batch();
    let ids = ids_by_key(&b);
    let s = session(&b);
    for key in ["J-E-206-L@101", "J-E-206-N@101", "J-E-206-PE@101", "DP@3"] {
        let t = text(&s, rep(&ids, key));
        assert_eq!(t.text, "+J-E-206 / JE-N1000-03", "{key}");
        assert!(t.links.is_empty(), "{key}: no links");
    }
}

/// Device, main cell, auxiliary representation keys with their positions as printed on the main.
type MainRow = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

const MAINS: [MainRow; 9] = [
    (
        "-201U1",
        "201-E1",
        &[("-201U1#bus", "3-B2"), ("-201U1#power", "106-G3")],
    ),
    ("-202U1", "202-E1", &[("-202U1#power", "106-G9")]),
    ("-201U3", "201-E4", &[("-201U3#aux", "203-B4")]),
    ("-201U4", "201-E6", &[("-201U4#aux", "203-B12")]),
    ("-201U5", "201-E8", &[("-201U5#aux", "204-B4")]),
    ("-201U6", "201-E10", &[("-201U6#aux", "204-B12")]),
    ("-201U7", "201-E12", &[("-201U7#aux", "205-B4")]),
    ("-201U8", "201-E14", &[("-201U8#aux", "205-B12")]),
    ("-202U2", "202-E3", &[("-202U2#aux", "206-B4")]),
];

#[test]
fn main_and_auxiliary_texts_match_section_15() {
    // Doc 20 §15 counts 20 texts for this block because the -201U1 list has two items (two link targets):
    // 9 main texts + 10 auxiliary texts = 19 texts, and 44 + 20 = 64 link targets.
    let b = batch();
    let ids = ids_by_key(&b);
    let s = session(&b);
    let mut texts = 0;
    let mut targets = 0;
    for (device, main_cell, auxes) in MAINS {
        let main = rep(&ids, device);
        let list: Vec<&str> = auxes.iter().map(|(_, c)| *c).collect();
        let t = text(&s, main);
        assert_eq!(t.text, format!("∞ {}", list.join(", ")), "main {device}");
        texts += 1;
        assert_eq!(t.links.len(), auxes.len(), "{device}: links");
        for (link, (aux_key, cell)) in t.links.iter().zip(auxes) {
            assert_eq!(link.rep, rep(&ids, aux_key), "{device}: link to {aux_key}");
            assert_eq!(&t.text[link.range.clone()], *cell, "{device}: link range");
            targets += 1;
            let a = text(&s, rep(&ids, aux_key));
            assert_eq!(a.text, format!("({main_cell})"), "aux {aux_key}");
            texts += 1;
            assert_eq!(a.links.len(), 1, "{aux_key}: one link");
            assert_eq!(a.links[0].rep, main, "{aux_key}: link to main");
            targets += 1;
        }
    }
    assert_eq!(texts, 19);
    // 10 auxiliary targets (one per auxiliary) plus 10 main-to-auxiliary targets.
    assert_eq!(targets, 20);
}

fn all_texts(ids: &Ids, s: &Session) -> BTreeMap<String, XrefText> {
    ids.iter()
        .filter_map(|(k, id)| {
            let key = k.strip_prefix("representations|")?;
            Some((key.to_owned(), text(s, Id::from_str(id).unwrap())))
        })
        .collect()
}

/// A session in which the representation `key` is moved by `(dx, dy)` nm.
fn shifted(b: &ImportBatch, key: &str, dx: i64, dy: i64) -> Session {
    session_with(b, |_, records| {
        let mut found = 0;
        for r in records.iter_mut() {
            if let Record::Representation(rep) = r
                && rep
                    .source_ref
                    .as_ref()
                    .is_some_and(|s| s.key == format!("representations|{key}"))
            {
                rep.pos = Pt2::new(Nm(rep.pos.x.0 + dx), Nm(rep.pos.y.0 + dy));
                found += 1;
            }
        }
        assert_eq!(found, 1, "{key}");
    })
}

/// D-14 ("3-B0") is guarded by `rep_anchor_cell`: the cell comes from the anchor point of the symbol, never
/// from rendered geometry and never from a cell centre. The golden bus port sits on a grid point; moving it
/// by a fraction of a cell keeps its cell however far off any cell centre the anchor lands, and moving it by
/// one column width prints the next column.
#[test]
fn the_cell_follows_the_anchor_inside_and_across_cells() {
    const MM: i64 = 1_000_000;
    // One column of the default frame: 15.875 mm.
    const COLUMN: i64 = 15_875_000;
    let b = batch();
    let ids = ids_by_key(&b);
    let all = all_texts(&ids, &session(&b));
    assert!(all.len() > 100, "every representation is evaluated");
    for (key, t) in &all {
        assert!(!t.text.contains("3-B0"), "{key} prints {:?}", t.text);
    }
    assert_eq!(all["-201U1"].text, "∞ 3-B2, 106-G3");
    let main = rep(&ids, "-201U1");
    for (dx, dy) in [(MM / 2, MM / 2), (MM, MM), (-MM / 4, MM / 2)] {
        let moved = shifted(&b, "-201U1#bus", dx, dy);
        assert_eq!(
            text(&moved, main).text,
            "∞ 3-B2, 106-G3",
            "moved by ({dx}, {dy}) nm"
        );
    }
    let next = shifted(&b, "-201U1#bus", COLUMN, 0);
    assert_eq!(text(&next, main).text, "∞ 3-B3, 106-G3");
}

#[test]
fn renumbering_folio_3_to_4_updates_the_bus_port_reference() {
    let b = batch();
    let ids = ids_by_key(&b);
    assert!(
        !folios(&b).values().any(|f| f == "4"),
        "folio 4 must not exist before the renumbering"
    );
    let original = session(&b);
    let renumbered = session_with(&b, |_, records| {
        let mut found = 0;
        for r in records.iter_mut() {
            if let Record::Page(p) = r
                && p.source_ref.as_ref().is_some_and(|s| s.key == "pages|3")
            {
                assert_eq!(p.folio.number, 3);
                assert!(p.folio.label.is_none());
                p.folio.number = 4;
                found += 1;
            }
        }
        assert_eq!(found, 1);
    });
    let main = rep(&ids, "-201U1");
    let bus = rep(&ids, "-201U1#bus");
    assert_eq!(text(&original, main).text, "∞ 3-B2, 106-G3");
    assert_eq!(text(&renumbered, main).text, "∞ 4-B2, 106-G3");
    assert_eq!(text(&renumbered, bus).text, "(201-E1)");
    assert_eq!(text(&original, bus).text, "(201-E1)");
}

#[test]
fn xref_texts_do_not_depend_on_record_order() {
    let b = batch();
    let ids = ids_by_key(&b);
    let forward = all_texts(&ids, &session(&b));
    let reversed = all_texts(&ids, &session_with(&b, |_, r| r.reverse()));
    assert_eq!(forward, reversed);
}

/// FR-XRF-003: `%f` is the position in `page_order` counting drawn pages, dividers and the base page of each
/// report block (the TOC), not the continuation page 2.1: front 1, TOC 2, [2.1], 3, divider 100, 101, 102
/// is the sixth.
#[test]
fn page_index_counts_every_page_record_but_not_continuations() {
    let b = batch();
    let ids = ids_by_key(&b);
    let s = session_with(&b, |p, _| {
        p.settings.extra.insert(
            "xref_format".into(),
            serde_json::json!({
                "key": "qet", "ip": "%f-%l%c", "aux_to_main": "({page}-{row}{column})",
                "main_to_aux": "∞ {list}", "structure": "when_different", "same_page": "full",
                "matrix": {"per_row": 3, "max": 9, "overflow": "…"},
            }),
        );
    });
    assert_eq!(text(&s, rep(&ids, "L-1@101")).text, "6-A1");
}

#[test]
fn no_xref_notes_on_the_golden_model() {
    let b = batch();
    let ids = ids_by_key(&b);
    let s = session(&b);
    let mut q = s.query();
    for (k, id) in &ids {
        if k.starts_with("representations|") {
            q.get::<XrefTextOf>(Id::from_str(id).unwrap());
        }
    }
    let notes = q.diagnostics().notes();
    let bad: Vec<String> = notes
        .iter()
        .filter(|n| matches!(n, DeriveNote::Xref { .. } | DeriveNote::MalformedShim { .. }))
        .map(ToString::to_string)
        .collect();
    assert!(bad.is_empty(), "unexpected notes: {bad:?}");
}
