// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 2 below the shell (acceptance check WR8, §8.6): crossings drawn as hops. `project.drawing.set
//! {crossings: hops}` changes folio 103's display list only by the hop arcs of the rule (`ew_scene::hops`), the
//! derived diff is empty and no connection changes; absent and `plain` draw Golden A's bytes.

use crate::common;

use common::golden_session;
use ew_api::output::page_layer;
use ew_api::{AnyCommand, Session};
use ew_derive::ew_units::Pt2;
use ew_derive::{PageOrder, PageRef};
use ew_dl::{Item, ItemKind, Layer, PathEl};
use ew_scene::hops::{hop_path, hops};
use serde_json::json;

const LAYERS: [Layer; 5] = [
    Layer::Frame,
    Layer::Symbols,
    Layer::Wires,
    Layer::Labels,
    Layer::Tables,
];

fn page_of(s: &Session, folio: &str) -> PageRef {
    let q = s.query();
    let order = q.get::<PageOrder>(());
    order
        .entries
        .iter()
        .find(|e| e.folio == folio)
        .map(|e| e.page)
        .expect("the folio")
}

/// Every layer's items of `page`.
fn layers(s: &Session, page: PageRef) -> Vec<Vec<Item>> {
    let q = s.query();
    LAYERS
        .iter()
        .map(|l| page_layer(&q, page, *l).list.items.clone())
        .collect()
}

/// The connection records as JSON lines.
fn connections(s: &Session) -> Vec<String> {
    let q = s.query();
    q.connections()
        .iter()
        .map(|(_, c)| serde_json::to_string(&**c).unwrap())
        .collect()
}

fn drawing(crossings: &str) -> AnyCommand {
    AnyCommand {
        name: "project.drawing.set".into(),
        params: json!({ "crossings": crossings }),
    }
}

/// The polyline of a straight-segment path (`MoveTo` then `LineTo`s); `None` for any other path.
fn polyline(item: &Item) -> Option<Vec<Pt2>> {
    let ItemKind::Path(p) = &item.kind else {
        return None;
    };
    p.path
        .els
        .iter()
        .enumerate()
        .map(|(i, e)| match (i, e) {
            (0, PathEl::MoveTo(a)) => Some(*a),
            (i, PathEl::LineTo(a)) if i > 0 => Some(*a),
            _ => None,
        })
        .collect()
}

#[test]
fn wr8_crossings_as_hops_change_only_the_wires_by_the_hop_arcs() {
    let mut s = golden_session();
    let page = page_of(&s, "103");
    let before = layers(&s, page);
    let records = connections(&s);
    // `plain` draws Golden A's bytes.
    let preview = s.preview(&drawing("plain")).unwrap();
    let diff = s.derived_diff(&preview);
    assert!(diff.tags.is_empty() && diff.xrefs.is_empty(), "{diff:?}");
    s.commit(preview).unwrap();
    assert_eq!(layers(&s, page), before, "plain draws the golden bytes");
    // `hops`: the derived diff is empty, no connection changes.
    let preview = s.preview(&drawing("hops")).unwrap();
    let diff = s.derived_diff(&preview);
    assert!(diff.tags.is_empty() && diff.xrefs.is_empty(), "{diff:?}");
    s.commit(preview).unwrap();
    assert_eq!(connections(&s), records, "no connection changes");
    let after = layers(&s, page);
    for (k, layer) in LAYERS.iter().enumerate() {
        if *layer != Layer::Wires {
            assert_eq!(after[k], before[k], "{layer:?} unchanged");
        }
    }
    // The wires: the connections' polylines (in paint order, which is connection-ID order) with the hops of the rule,
    // every other item as it was.
    let w = LAYERS.iter().position(|l| *l == Layer::Wires).unwrap();
    let (old, new) = (&before[w], &after[w]);
    assert_eq!(old.len(), new.len());
    let lines: Vec<(usize, Vec<Pt2>)> = old
        .iter()
        .enumerate()
        .filter_map(|(i, item)| polyline(item).map(|p| (i, p)))
        .collect();
    let expected = hops(&lines.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>());
    let mut hopped = 0;
    for (k, (i, line)) in lines.iter().enumerate() {
        let (ItemKind::Path(a), ItemKind::Path(b)) = (&old[*i].kind, &new[*i].kind) else {
            panic!("a path");
        };
        assert_eq!(b.path, hop_path(line, &expected[k]), "item {i}");
        assert_eq!((&a.stroke, &a.fill), (&b.stroke, &b.fill));
        assert_eq!(old[*i].entity, new[*i].entity);
        hopped += expected[k].iter().map(|(_, xs)| xs.len()).sum::<usize>();
    }
    for (i, item) in old.iter().enumerate() {
        if !lines.iter().any(|(j, _)| *j == i) {
            assert_eq!(&new[i], item, "a junction or other item is unchanged");
        }
    }
    assert!(hopped > 0, "folio 103 has crossings of the rule");
    // Back to absent (`null`): the golden bytes again.
    s.execute(&AnyCommand {
        name: "project.drawing.set".into(),
        params: json!({ "crossings": null }),
    })
    .unwrap();
    assert_eq!(layers(&s, page), before);
}

/// Fix round (FT-421 follow-up): the next designation keeps the width of a zero-padded one (`05` is followed by
/// `06`, not `6`).
#[test]
fn the_next_designation_keeps_the_width_of_a_padded_one() {
    let mut s = golden_session();
    let terminal = s.resolve_reference("tag:-X1032:5").expect("-X1032:5");
    s.execute(&AnyCommand {
        name: "terminal.designate".into(),
        params: json!({"assignments": [{"terminal": terminal.to_string(), "designation": "05"}]}),
    })
    .unwrap();
    let cat = s.place_catalogue("103").unwrap();
    let x1032 = cat.strips.iter().find(|r| r.tag == "-X1032").unwrap();
    assert_eq!(x1032.next_designation, "06");
}
