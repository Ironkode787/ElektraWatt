// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Value rendering of report cells (doc 09 §3.6) over the golden project: tag-range compression against
//! doc 20 §14.1, the Folios column, quantities, dates and the absent values.

mod common;

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

#[test]
fn decimals() {
    assert_eq!(decimal(Dec3(750), false), "0.75");
    assert_eq!(decimal(Dec3(750), true), "0,75");
    assert_eq!(decimal(Dec3(1500), false), "1.5");
    assert_eq!(decimal(Dec3(2000), false), "2");
    assert_eq!(decimal(Dec3(-5), false), "-0.005");
    assert_eq!(decimal(Dec3(0), false), "0");
    assert_eq!(decimal(Dec3(-1250), true), "-1,25");
}

#[test]
fn quantities_with_units_and_language() {
    let s = common::golden_session();
    let q = s.query();
    let mm2 = Cell::Qty(Dec3(750), QtyUnit::Unit(Unit::Mm2));
    assert_eq!(render(&q, &mm2, &RenderOptions::default()), "0.75 mm²");
    let nl = RenderOptions {
        lang: Some(lang("nl-NL")),
        ..RenderOptions::default()
    };
    assert_eq!(render(&q, &mm2, &nl), "0,75 mm²");
    let en = RenderOptions {
        lang: Some(lang("en-GB")),
        ..RenderOptions::default()
    };
    assert_eq!(render(&q, &mm2, &en), "0.75 mm²");
    let d = RenderOptions::default();
    assert_eq!(render(&q, &Cell::Qty(Dec3(2000), QtyUnit::None), &d), "2");
    assert_eq!(
        render(&q, &Cell::Qty(Dec3(24_000), QtyUnit::Unit(Unit::DegC)), &d),
        "24 °C"
    );
}

#[test]
fn absent_values_and_markers() {
    let s = common::golden_session();
    let q = s.query();
    let d = RenderOptions::default();
    assert_eq!(render(&q, &Cell::Tbd, &d), "TBD");
    let custom = RenderOptions {
        tbd: "offen".to_owned(),
        ..RenderOptions::default()
    };
    assert_eq!(render(&q, &Cell::Tbd, &custom), "offen");
    assert_eq!(
        render(&q, &Cell::Missing(MissingReason::Length), &d),
        "length missing"
    );
    assert_eq!(
        render(&q, &Cell::Missing(MissingReason::Part), &d),
        "part missing"
    );
    assert_eq!(
        render(&q, &Cell::Missing(MissingReason::CrossSection), &d),
        "cross-section missing"
    );
    assert_eq!(
        render(&q, &Cell::Missing(MissingReason::Colour), &d),
        "colour missing"
    );
    assert_eq!(render(&q, &Cell::Empty, &d), "");
    let stale = Cell::Stale(Box::new(Cell::Qty(Dec3(810_000), QtyUnit::Unit(Unit::M))), Rev(3));
    assert_eq!(render(&q, &stale, &d), "≈ 810 m");
}

#[test]
fn date_bool_number_ident() {
    let s = common::golden_session();
    let q = s.query();
    let d = RenderOptions::default();
    let date = Date::new(2025, 3, 3).unwrap();
    assert_eq!(render(&q, &Cell::Date(date), &d), "03-03-2025");
    assert_eq!(render(&q, &Cell::Bool(true), &d), "yes");
    assert_eq!(render(&q, &Cell::Bool(false), &d), "no");
    assert_eq!(render(&q, &Cell::Number(-12), &d), "-12");
    assert_eq!(render(&q, &Cell::Ident("UK 5 N".to_owned()), &d), "UK 5 N");
}

#[test]
fn text_language_fallback() {
    let s = common::golden_session();
    let q = s.query();
    let ml = MlText::from_langs([
        (lang("en"), "Heater".to_owned()),
        (lang("nl"), "Verwarming".to_owned()),
    ]);
    let nl = RenderOptions {
        lang: Some(lang("nl")),
        ..RenderOptions::default()
    };
    assert_eq!(render(&q, &Cell::Text(ml.clone()), &nl), "Verwarming");
    // A language the text lacks falls back to the first language present.
    let de = RenderOptions {
        lang: Some(lang("de")),
        ..RenderOptions::default()
    };
    assert_eq!(render(&q, &Cell::Text(ml.clone()), &de), "Heater");
    assert_eq!(render(&q, &Cell::Text(ml), &RenderOptions::default()), "Heater");
    let neutral = Cell::Text(MlText::neutral("Plate"));
    assert_eq!(render(&q, &neutral, &nl), "Plate");
    assert_eq!(render(&q, &Cell::Text(MlText::neutral("")), &nl), "");
}

#[test]
fn tag_full_and_displayed() {
    let s = common::golden_session();
    let q = s.query();
    let row = bom_row(&q, "571334");
    let dev = row.devices[0];
    let short = RenderOptions::default();
    let full = RenderOptions {
        tag_form: TagForm::Full,
        ..RenderOptions::default()
    };
    let short_text = render(&q, &Cell::Tag(dev), &short);
    let full_text = render(&q, &Cell::Tag(dev), &full);
    assert_eq!(short_text, "-201U3-Q1");
    assert!(full_text.starts_with('='), "{full_text}");
    assert!(full_text.ends_with(&short_text), "{full_text}");
    assert_ne!(full_text, short_text);
}
