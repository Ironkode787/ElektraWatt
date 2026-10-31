// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Value rendering of report cells (doc 09 §3.6) over the golden project: tag-range compression against
//! doc 20 §14.1, the Folios column, quantities, dates and the absent values.

use crate::common;

use ew_derive::ew_mltext::{LangTag, MlText};
use ew_derive::ew_units::Dec3;
use ew_derive::{BomMode, BomRow, BomRows, PageOrder, PageRef, Q, Rev};
use ew_reports::render::{RenderOptions, TagForm, compress_range, decimal, render};
use ew_reports::{Cell, MissingReason, QtyUnit};
use ew_schema::{Date, Unit};

fn bom_row(q: &Q, part_number: &str) -> BomRow {
    q.get::<BomRows>(BomMode::Component)
        .iter()
        .find(|r| r.part_number == part_number)
        .unwrap_or_else(|| panic!("no BOM row {part_number}"))
        .clone()
}

fn lang(s: &str) -> LangTag {
    s.parse().unwrap()
}

#[test]
fn range_of_the_valve_row_is_one_run() {
    let s = common::golden_session();
    let q = s.query();
    let row = bom_row(&q, "571334");
    assert_eq!(compress_range(&q, &row.devices), "-201U3-Q1 … -202U2-Q4");
}

#[test]
fn range_of_the_electronic_modules_is_interrupted_by_the_supply_plates() {
    let s = common::golden_session();
    let q = s.query();
    let row = bom_row(&q, "533361");
    assert_eq!(compress_range(&q, &row.devices), "-201U3 … -201U8, -202U2");
}

#[test]
fn range_of_the_feed_through_terminals_is_a_list() {
    let s = common::golden_session();
    let q = s.query();
    let row = bom_row(&q, "UK 5 N");
    assert_eq!(compress_range(&q, &row.devices), "1X1:1, 1X1:2");
}

#[test]
fn range_of_a_terminal_run_drops_the_strip_prefix() {
    let s = common::golden_session();
    let q = s.query();
    let row = bom_row(&q, "PT 2.5-TWIN");
    assert_eq!(compress_range(&q, &row.devices), "-X1031:1 … 20");
    assert_eq!(compress_range(&q, &[]), "");
}

#[test]
fn folios_of_the_valve_row_print_as_a_range() {
    let s = common::golden_session();
    let q = s.query();
    let row = bom_row(&q, "571334");
    let opts = RenderOptions::default();
    assert_eq!(render(&q, &Cell::Folios(row.pages.clone()), &opts), "203–206");
    let two = bom_row(&q, "533361");
    assert_eq!(render(&q, &Cell::Folios(two.pages.clone()), &opts), "201, 202");
}

#[test]
fn folio_and_cell_ref_print_the_folio_label() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let first = order
        .entries
        .iter()
        .find(|e| matches!(e.page, PageRef::Drawn(_)) && e.folio == "101")
        .expect("folio 101");
    let opts = RenderOptions::default();
    assert_eq!(render(&q, &Cell::Folio(first.page), &opts), "101");
    let cell = ew_reports::GridCell {
        column: 16,
        row: 7,
        column_label: "16".to_owned(),
        row_label: "H".to_owned(),
    };
    assert_eq!(render(&q, &Cell::CellRef(first.page, cell), &opts), "101-H16");
}

/// The value cells (doc 09 §3.6): decimals with either separator, quantities with units and the language's
/// separator, the absent values and markers, dates, booleans, numbers and identifiers, a text's language fallback,
/// and the full and displayed tag forms. One case per value; the case name is in every failure message.
#[test]
fn values_render_by_kind_language_and_form() {
    let s = common::golden_session();
    let q = s.query();
    let d = RenderOptions::default();
    let in_lang = |l: &str| RenderOptions {
        lang: Some(lang(l)),
        ..RenderOptions::default()
    };
    let offen = RenderOptions {
        tbd: "offen".to_owned(),
        ..RenderOptions::default()
    };
    let mm2 = Cell::Qty(Dec3(750), QtyUnit::Unit(Unit::Mm2));
    let stale = Cell::Stale(Box::new(Cell::Qty(Dec3(810_000), QtyUnit::Unit(Unit::M))), Rev(3));
    let ml = Cell::Text(MlText::from_langs([
        (lang("en"), "Heater".to_owned()),
        (lang("nl"), "Verwarming".to_owned()),
    ]));
    let date = Date::new(2025, 3, 3).unwrap();
    let cases: Vec<(&str, String, &str)> = vec![
        // decimal()
        ("decimal 0.75", decimal(Dec3(750), false), "0.75"),
        ("decimal 0,75", decimal(Dec3(750), true), "0,75"),
        ("decimal 1.5", decimal(Dec3(1500), false), "1.5"),
        ("decimal 2", decimal(Dec3(2000), false), "2"),
        ("decimal -0.005", decimal(Dec3(-5), false), "-0.005"),
        ("decimal 0", decimal(Dec3(0), false), "0"),
        ("decimal -1,25", decimal(Dec3(-1250), true), "-1,25"),
        // Quantities with units and language.
        ("mm² default", render(&q, &mm2, &d), "0.75 mm²"),
        ("mm² nl-NL", render(&q, &mm2, &in_lang("nl-NL")), "0,75 mm²"),
        ("mm² en-GB", render(&q, &mm2, &in_lang("en-GB")), "0.75 mm²"),
        (
            "quantity without unit",
            render(&q, &Cell::Qty(Dec3(2000), QtyUnit::None), &d),
            "2",
        ),
        (
            "°C",
            render(&q, &Cell::Qty(Dec3(24_000), QtyUnit::Unit(Unit::DegC)), &d),
            "24 °C",
        ),
        // Absent values and markers.
        ("TBD", render(&q, &Cell::Tbd, &d), "TBD"),
        ("TBD text of the options", render(&q, &Cell::Tbd, &offen), "offen"),
        (
            "length missing",
            render(&q, &Cell::Missing(MissingReason::Length), &d),
            "length missing",
        ),
        (
            "part missing",
            render(&q, &Cell::Missing(MissingReason::Part), &d),
            "part missing",
        ),
        (
            "cross-section missing",
            render(&q, &Cell::Missing(MissingReason::CrossSection), &d),
            "cross-section missing",
        ),
        (
            "colour missing",
            render(&q, &Cell::Missing(MissingReason::Colour), &d),
            "colour missing",
        ),
        ("empty", render(&q, &Cell::Empty, &d), ""),
        ("stale", render(&q, &stale, &d), "≈ 810 m"),
        // Dates, booleans, numbers, identifiers.
        ("date", render(&q, &Cell::Date(date), &d), "03-03-2025"),
        ("true", render(&q, &Cell::Bool(true), &d), "yes"),
        ("false", render(&q, &Cell::Bool(false), &d), "no"),
        ("number", render(&q, &Cell::Number(-12), &d), "-12"),
        (
            "identifier",
            render(&q, &Cell::Ident("UK 5 N".to_owned()), &d),
            "UK 5 N",
        ),
        // A text's language; a language the text lacks falls back to the first language present.
        ("text nl", render(&q, &ml, &in_lang("nl")), "Verwarming"),
        ("text de falls back", render(&q, &ml, &in_lang("de")), "Heater"),
        ("text without a language", render(&q, &ml, &d), "Heater"),
        (
            "neutral text",
            render(&q, &Cell::Text(MlText::neutral("Plate")), &in_lang("nl")),
            "Plate",
        ),
        (
            "empty neutral text",
            render(&q, &Cell::Text(MlText::neutral("")), &in_lang("nl")),
            "",
        ),
    ];
    for (name, got, want) in &cases {
        assert_eq!(got, want, "case {name}");
    }
    // The tag forms: the displayed tag, and the full tag that ends with it.
    let dev = bom_row(&q, "571334").devices[0];
    let full = RenderOptions {
        tag_form: TagForm::Full,
        ..RenderOptions::default()
    };
    let short_text = render(&q, &Cell::Tag(dev), &d);
    let full_text = render(&q, &Cell::Tag(dev), &full);
    assert_eq!(short_text, "-201U3-Q1", "case tag displayed");
    assert!(full_text.starts_with('='), "case tag full: {full_text}");
    assert!(full_text.ends_with(&short_text), "case tag full: {full_text}");
    assert_ne!(full_text, short_text, "case tag full");
}
