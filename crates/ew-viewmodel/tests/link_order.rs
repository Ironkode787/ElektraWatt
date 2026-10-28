// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-483 (WP-S2.03 §6.4, decision log ID-677): the page's link list is in reading order, and links at the same
//! reading key go by what is printed, the label of their target in natural order, before the destination and the
//! paint order, which follow random IDs.

use ew_viewmodel::canvas::{PageLink, sort_links};
use ew_viewmodel::ew_api::output::LinkTarget;
use ew_viewmodel::ew_api::output::ew_dl::Destination;
use ew_viewmodel::ew_api::output::ew_dl::ew_units::{Nm, Pt2, Rect};

/// The links on one spot (so one reading key), the target folios given in the order of the (random) IDs: the first
/// gets the smallest destination and the first paint index, or the largest with `reversed`.
fn listed(folios: &[(&str, Option<&str>)], reversed: bool) -> Vec<String> {
    let uuid = |n: u128| format!("{n:032x}").parse().expect("a UUID");
    let rect = Rect::from_points(
        Pt2::new(Nm(1_000_000), Nm(2_000_000)),
        Pt2::new(Nm(3_000_000), Nm(4_000_000)),
    );
    let count = folios.len();
    let mut links: Vec<PageLink> = folios
        .iter()
        .enumerate()
        .map(|(i, (folio, cell))| {
            let n = if reversed { count - 1 - i } else { i };
            PageLink {
                dest: Destination::Representation(uuid(n as u128 + 1)),
                rect,
                target: Some(LinkTarget {
                    page: uuid(0x100),
                    folio: (*folio).to_owned(),
                    at_nm: None,
                    cell: cell.map(str::to_owned),
                }),
                paint: n,
            }
        })
        .collect();
    sort_links(&mut links, None);
    links.iter().map(|l| l.label().expect("a target")).collect()
}

/// Two links tied on the reading key give the same list whichever of them has the smaller destination and the
/// earlier paint index; the order is the natural order of the target labels (9 before 10, 103-A2 before 103-A10).
#[test]
fn links_tied_on_reading_order_go_by_the_target_label_not_the_ids() {
    let two = [("10", None), ("9", None)];
    assert_eq!(listed(&two, false), ["9", "10"]);
    assert_eq!(listed(&two, true), ["9", "10"]);
    // A cell label is the printed text of a target with a cell: 103-A2 before 103-A10 in natural order.
    let cells = [
        ("103", Some("103-A10")),
        ("103", Some("103-A2")),
        ("103", Some("102-C1")),
    ];
    assert_eq!(listed(&cells, false), ["102-C1", "103-A2", "103-A10"]);
    assert_eq!(listed(&cells, true), ["102-C1", "103-A2", "103-A10"]);
}

/// A link without a target comes after the links with one at the same reading key, and equal labels keep the
/// destination order (the IDs decide only when nothing printed does).
#[test]
fn a_link_without_a_target_is_last_and_equal_labels_fall_to_the_ids() {
    let uuid = |n: u128| format!("{n:032x}").parse().expect("a UUID");
    let rect = Rect::from_points(Pt2::new(Nm(0), Nm(0)), Pt2::new(Nm(2_000_000), Nm(2_000_000)));
    let make = |n: u128, folio: Option<&str>| PageLink {
        dest: Destination::Page(uuid(n)),
        rect,
        target: folio.map(|f| LinkTarget {
            page: uuid(0x100),
            folio: f.to_owned(),
            at_nm: None,
            cell: None,
        }),
        paint: n as usize,
    };
    let mut links = vec![make(1, None), make(3, Some("101")), make(2, Some("101"))];
    sort_links(&mut links, None);
    let dests: Vec<_> = links.iter().map(|l| l.dest).collect();
    assert_eq!(
        dests,
        [
            Destination::Page(uuid(2)),
            Destination::Page(uuid(3)),
            Destination::Page(uuid(1))
        ]
    );
}
