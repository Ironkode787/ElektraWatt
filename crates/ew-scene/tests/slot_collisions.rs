// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-687: the placement rules of the slot texts. At one-column spacing a contact's back-reference does not overlap
//! the neighbour contact's tag (it drops below it), and a coil's cross-reference list of two lines grows downward
//! from its first line, clear of the coil's technical data ("24 V").

use std::sync::Arc;

use ew_dl::{EntityRef, ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::page::{PageKind, PageView, RepView, SlotValue, SymbolRef, page_display_list};
use ew_schema::{LibCopy, LibDocument, Representation, SlotKind, SymbolDef};
use ew_text::{builtin, bundled};
use ew_units::Rect;
use serde_json::json;

const COIL: &str = include_str!("../../../content/symbols/qet10/ew.iec.coil@qet10.json");
const CONTACT: &str = include_str!("../../../content/symbols/qet10/ew.iec.contact-co@qet10.json");

const PAGE: u128 = 0x100;

fn symbol(text: &str) -> SymbolRef {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Symbol(LibCopy { id, item, .. }) => SymbolRef {
            id,
            def: Arc::new(item),
        },
        _ => panic!("not a symbol"),
    }
}

fn uuid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

/// QET 10 px = 7 937 500 / 3 nm; positions on that grid.
fn g(k: i64) -> i64 {
    k * 7_937_500 / 3
}

/// One column of the QET A4 grid frame: 60 QET px (15.875 mm), six grid steps.
const COLUMN: i64 = 6;

fn rep(n: u128, symbol: Id<SymbolDef>, x: i64, y: i64) -> Arc<Representation> {
    Arc::new(
        serde_json::from_value(json!({
            "id": uuid(n), "functions": [uuid(n + 0x1000)], "page": uuid(PAGE),
            "symbol": symbol.as_uuid().hyphenated().to_string(), "pos": [g(x), g(y)], "orient": "r0",
        }))
        .unwrap(),
    )
}

/// The index of the slot of kind `kind` in `s`.
fn slot(s: &SymbolRef, kind: &SlotKind) -> u16 {
    s.def
        .text_slots
        .iter()
        .position(|t| &t.kind == kind)
        .map(|i| u16::try_from(i).unwrap())
        .expect("the slot")
}

fn value(slot: u16, text: &str) -> SlotValue {
    SlotValue {
        slot,
        text: text.into(),
        link: None,
        item_links: Vec::new(),
    }
}

fn page(reps: Vec<RepView>) -> PageView {
    PageView {
        page: Id::from_uuid(Uuid::from_u128(PAGE)),
        kind: PageKind::Schematic,
        grid: ew_units::Grid::QET_10PX,
        language: None,
        frame: None,
        reps,
        connections: Vec::new(),
        junctions: Vec::new(),
        wire_marker: None,
        table: None,
        embedded: Vec::new(),
        break_marks: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Default::default(),
        items: Vec::new(),
        structure_boxes: Vec::new(),
    }
}

/// The layout box of the lines of entity `entity` whose text is one of `lines`, on the labels layer of `v`.
fn text_box(v: &PageView, entity: u128, lines: &[&str]) -> Rect {
    let labels = page_display_list(v, Layer::Labels, bundled(), builtin());
    let mut out: Option<Rect> = None;
    for item in &labels.list.items {
        if item.entity != Some(EntityRef(Uuid::from_u128(entity))) {
            continue;
        }
        // A line in one face is a glyph run; a line in several faces (the "∞" of a list) a group of runs.
        let (text, quad): (String, Vec<ew_units::Pt2>) = match &item.kind {
            ItemKind::Glyphs(run) => (run.text.clone(), run.layout_quad().to_vec()),
            ItemKind::Group(group) => {
                let t = group.transform.unwrap_or(ew_dl::Transform::IDENTITY);
                let runs: Vec<_> = group
                    .items
                    .iter()
                    .filter_map(|i| match &i.kind {
                        ItemKind::Glyphs(run) => Some(run),
                        _ => None,
                    })
                    .collect();
                (
                    runs.iter().map(|r| r.text.as_str()).collect(),
                    runs.iter()
                        .flat_map(|r| r.layout_quad().map(|q| t.apply(q)))
                        .collect(),
                )
            }
            _ => continue,
        };
        if !lines.contains(&text.as_str()) {
            continue;
        }
        for q in quad {
            let r = Rect::from_point(q);
            out = Some(out.map_or(r, |o| o.union(r)));
        }
    }
    out.unwrap_or_else(|| panic!("no text {lines:?} of {entity:#x}"))
}

/// Whether two rectangles share interior points.
fn overlap(a: Rect, b: Rect) -> bool {
    a.min().x < b.max().x && b.min().x < a.max().x && a.min().y < b.max().y && b.min().y < a.max().y
}

/// Two change-over contacts of `-101K1` one column apart, each with its tag and its back-reference.
fn contacts(spacing: i64) -> PageView {
    let c = symbol(CONTACT);
    let (tag, xref) = (slot(&c, &SlotKind::Tag), slot(&c, &SlotKind::Xref));
    let contact = |n: u128, x: i64| RepView {
        rep: rep(n, c.id, x, 20),
        symbol: Some(c.def.clone()),
        slots: vec![value(tag, "-101K1"), value(xref, "(101-B2)")],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    page(vec![contact(0x11, 20), contact(0x12, 20 + spacing)])
}

#[test]
fn a_back_reference_drops_below_the_neighbour_contacts_tag_at_one_column_spacing() {
    let v = contacts(COLUMN);
    let back = text_box(&v, 0x11, &["(101-B2)"]);
    let tag = text_box(&v, 0x12, &["-101K1"]);
    assert!(!overlap(back, tag), "{back:?} overlaps {tag:?}");
    // It keeps its column and sits below the tag, one gap apart.
    let gap = back.min().y - tag.max().y;
    assert!(
        (gap.0 - ew_scene::page::SLOT_TEXT_GAP.0).abs() <= 1_000,
        "one gap below the tag: {gap:?} {back:?} {tag:?} {:?}",
        text_box(&v, 0x11, &["-101K1"])
    );
    // Its own contact's tag and the neighbour's back-reference are where the symbol puts them.
    let own_tag = text_box(&v, 0x11, &["-101K1"]);
    assert!(!overlap(back, own_tag));
    let wide = contacts(2 * COLUMN);
    let far = text_box(&wide, 0x11, &["(101-B2)"]);
    assert_eq!(far.min().x, back.min().x, "the same column");
    assert!(
        far.min().y < back.min().y,
        "at two columns the back-reference stays beside its contact"
    );
    assert!(!overlap(far, text_box(&wide, 0x12, &["-101K1"])));
}

/// A coil with its tag, technical data and a two-line cross-reference list.
fn coil(list: &str) -> PageView {
    let c = symbol(COIL);
    let (tag, xref, data) = (
        slot(&c, &SlotKind::Tag),
        slot(&c, &SlotKind::Xref),
        slot(&c, &SlotKind::TechData),
    );
    page(vec![RepView {
        rep: rep(0x21, c.id, 20, 20),
        symbol: Some(c.def.clone()),
        slots: vec![value(tag, "-101K1"), value(xref, list), value(data, "24 V")],
        terminals: Vec::new(),
        pins: Vec::new(),
    }])
}

#[test]
fn a_coils_cross_reference_list_grows_down_from_its_first_line_clear_of_its_data() {
    let two = coil("∞ 101-B6, 101-B8, 101-D6\n101-D8");
    let data = text_box(&two, 0x21, &["24 V"]);
    let list = text_box(&two, 0x21, &["∞ 101-B6, 101-B8, 101-D6", "101-D8"]);
    assert!(!overlap(data, list), "{data:?} overlaps {list:?}");
    // The first line sits where a one-line list sits.
    let one = coil("∞ 101-B6, 101-B8, 101-D6");
    let first = text_box(&two, 0x21, &["∞ 101-B6, 101-B8, 101-D6"]);
    assert_eq!(first, text_box(&one, 0x21, &["∞ 101-B6, 101-B8, 101-D6"]));
    assert!(!overlap(data, first));
    let second = text_box(&two, 0x21, &["101-D8"]);
    assert!(
        second.min().y > first.min().y,
        "the second line is below the first"
    );
}
