// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `render::folios_spans` (WP-S3.06 D10, ruling 1): the printed Folios text and the byte span of each page in it,
//! the one function the printer and the link boxes read.

use ew_reports::render::folios_spans;

fn pages(items: &[(&str, Option<usize>)]) -> Vec<(String, Option<usize>)> {
    items.iter().map(|(l, p)| ((*l).to_owned(), *p)).collect()
}

/// Golden A's BOM row 21: four consecutive pages print "203–206"; the two ends span their labels (the en dash is 3
/// bytes, so "206" starts at byte 6), and 204 and 205 inside the run have no span.
#[test]
fn a_run_spans_its_two_ends_only() {
    let (text, spans) = folios_spans(&pages(&[
        ("203", Some(13)),
        ("204", Some(14)),
        ("205", Some(15)),
        ("206", Some(16)),
    ]));
    assert_eq!(text, "203–206");
    assert_eq!(spans, [Some(0..3), None, None, Some(6..9)]);
    assert_eq!(&text[6..9], "206");
}

/// Golden A's BOM rows 17, 19 and 20: two pages print as a list, each spanning its own label.
#[test]
fn a_short_list_spans_every_page() {
    let (text, spans) = folios_spans(&pages(&[("201", Some(10)), ("202", Some(11))]));
    assert_eq!(text, "201, 202");
    assert_eq!(spans, [Some(0..3), Some(5..8)]);
}

/// A run and a list mixed, and a page outside the page order, which ends a run and prints on its own.
#[test]
fn runs_and_lists_mix_in_page_order() {
    let (text, spans) = folios_spans(&pages(&[
        ("3", Some(2)),
        ("101", Some(5)),
        ("102", Some(6)),
        ("103", Some(7)),
        ("x", None),
        ("901.1", Some(20)),
    ]));
    assert_eq!(text, "3, 101–103, x, 901.1");
    assert_eq!(
        spans,
        [
            Some(0..1),
            Some(3..6),
            None,
            Some(9..12),
            Some(14..15),
            Some(17..22)
        ]
    );
}

/// One page spans the whole text; no page prints nothing.
#[test]
fn one_page_and_none() {
    assert_eq!(
        folios_spans(&pages(&[("101", Some(5))])),
        ("101".to_owned(), vec![Some(0..3)])
    );
    assert_eq!(folios_spans(&[]), (String::new(), Vec::new()));
}
