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
const VALVE: &str = include_str!("../../../content/symbols/qet10/ew.iec.valve-coil@qet10.json");
const CONTACT: &str = include_str!("../../../content/symbols/qet10/ew.iec.contact-co@qet10.json");
const CONTACT_NO: &str = include_str!("../../../content/symbols/qet10/ew.iec.contact-no@qet10.json");

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
        panel: None,
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
    contacts_with(CONTACT, spacing, "(101-B2)", false)
}

/// [`contacts`] with the contact symbol `json` and the back-reference `back`; with `designations` every CP
/// designation slot prints too ("13", "14", "12" by CP index), as a placed contact does.
fn contacts_with(json: &str, spacing: i64, back: &str, designations: bool) -> PageView {
    let c = symbol(json);
    let (tag, xref) = (slot(&c, &SlotKind::Tag), slot(&c, &SlotKind::Xref));
    let mut slots = vec![value(tag, "-101K1"), value(xref, back)];
    if designations {
        for (i, t) in c.def.text_slots.iter().enumerate() {
            if let SlotKind::CpDesignation(cp) = t.kind {
                let text = ["13", "14", "12"].get(usize::from(cp)).copied().unwrap_or("9");
                slots.push(value(u16::try_from(i).unwrap(), text));
            }
        }
    }
    let contact = |n: u128, x: i64| RepView {
        rep: rep(n, c.id, x, 20),
        symbol: Some(c.def.clone()),
        slots: slots.clone(),
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

/// Two starter valve coils `-101M1` and `-101M2` side by side, `spacing` grid steps apart, each with its tag, its
/// technical data "24 V" and its CP designations A1 and A2.
fn valves(spacing: i64) -> PageView {
    let c = symbol(VALVE);
    let (tag, data) = (slot(&c, &SlotKind::Tag), slot(&c, &SlotKind::TechData));
    let a = |i: u16| {
        c.def
            .text_slots
            .iter()
            .position(|t| t.kind == SlotKind::CpDesignation(i))
            .map(|i| u16::try_from(i).unwrap())
            .expect("the CP designation slot")
    };
    let valve = |n: u128, x: i64, name: &str| RepView {
        rep: rep(n, c.id, x, 20),
        symbol: Some(c.def.clone()),
        slots: vec![
            value(tag, name),
            value(data, "24 V"),
            value(a(0), "A1"),
            value(a(1), "A2"),
        ],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    page(vec![
        valve(0x31, 20, "-101M1"),
        valve(0x32, 20 + spacing, "-101M2"),
    ])
}

/// Two rectangles' horizontal clearance when their heights overlap (`None` when one is above the other).
fn side_gap(a: Rect, b: Rect) -> Option<i64> {
    (a.min().y < b.max().y && b.min().y < a.max().y)
        .then(|| (b.min().x.0 - a.max().x.0).max(a.min().x.0 - b.max().x.0))
}

/// Cycle 37 (window check, contacts 25 mm apart): a back-reference never runs into the neighbour contact's tag; a
/// back-reference beside it keeps at least the gap, else it drops below.
#[test]
fn a_back_reference_keeps_the_gap_from_the_neighbour_contacts_tag() {
    let gap = ew_scene::page::SLOT_TEXT_GAP.0;
    let cases = [
        (CONTACT, "(101-B2)", 9),
        (CONTACT, "(101-D6)", 9),
        (CONTACT, "(101-D6)", 10),
        (CONTACT, "(101-D6)", COLUMN),
        // The window check's case: two make contacts 9 grid steps apart, tag and back-reference on one line.
        (CONTACT_NO, "(101-D6)", 9),
        (CONTACT_NO, "(101-D6)", 10),
        (CONTACT_NO, "(101-D6)", COLUMN),
    ];
    for (json, back, spacing) in cases {
        let v = contacts_with(json, spacing, back, true);
        let b = text_box(&v, 0x11, &[back]);
        let tag = text_box(&v, 0x12, &["-101K1"]);
        assert!(!overlap(b, tag), "{back}: {b:?} overlaps {tag:?}");
        assert!(
            side_gap(b, tag).is_none_or(|g| g >= gap - 1_000),
            "{back} at {spacing}: {b:?} runs into {tag:?}"
        );
        for own in ["13", "14"] {
            let d = text_box(&v, 0x11, &[own]);
            assert!(
                !overlap(b, d),
                "{back} at {spacing}: {b:?} overlaps its {own} {d:?}"
            );
        }
    }
}

/// Cycle 37 (visual play-test): two starter valve coils side by side at one-column spacing and closer: M1's "24 V"
/// keeps at least the gap from M2's tag (it drops below it), its own A1/A2 and tag stay where the symbol puts them,
/// and at a spacing where nothing collides it stays beside its coil.
#[test]
fn a_valves_technical_data_keeps_the_gap_from_the_next_valves_tag() {
    let gap = ew_scene::page::SLOT_TEXT_GAP.0;
    let alone = text_box(&valves(20), 0x31, &["24 V"]);
    for spacing in [COLUMN, COLUMN + 1, COLUMN + 2] {
        let v = valves(spacing);
        let data = text_box(&v, 0x31, &["24 V"]);
        let tag = text_box(&v, 0x32, &["-101M2"]);
        assert!(!overlap(data, tag), "{spacing}: {data:?} overlaps {tag:?}");
        assert!(
            side_gap(data, tag).is_none_or(|g| g >= gap - 1_000),
            "{spacing}: {data:?} beside {tag:?}"
        );
        assert_eq!(data.min().x, alone.min().x, "{spacing}: the same column");
        assert!(
            (data.min().y.0 - tag.max().y.0 - gap).abs() <= 1_000,
            "{spacing}: one gap below the tag: {data:?} {tag:?}"
        );
        for own in ["A1", "A2", "-101M1"] {
            let b = text_box(&v, 0x31, &[own]);
            assert_eq!(b, text_box(&valves(20), 0x31, &[own]), "{spacing}: {own} unmoved");
            assert!(!overlap(data, b), "{spacing}: {data:?} overlaps {own} {b:?}");
        }
    }
    // Far apart, the data stays where the symbol puts it.
    assert_eq!(text_box(&valves(20), 0x31, &["24 V"]), alone);
}

/// Cycle 37: a dropped back-reference that lands on its own CP designation (a three-digit "114" reaching under it)
/// drops below that too, clear of the neighbour's tag and of its own texts.
#[test]
fn a_dropped_back_reference_also_clears_its_own_designation() {
    let c = symbol(CONTACT_NO);
    let (tag, xref) = (slot(&c, &SlotKind::Tag), slot(&c, &SlotKind::Xref));
    let end = slot(&c, &SlotKind::CpDesignation(1));
    let contact = |n: u128, x: i64| RepView {
        rep: rep(n, c.id, x, 20),
        symbol: Some(c.def.clone()),
        slots: vec![value(tag, "-101K1"), value(xref, "(101-D6)"), value(end, "114")],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    let v = page(vec![contact(0x11, 20), contact(0x12, 29)]);
    let back = text_box(&v, 0x11, &["(101-D6)"]);
    let next = text_box(&v, 0x12, &["-101K1"]);
    let own = text_box(&v, 0x11, &["114"]);
    assert!(
        !overlap(back, next) && !overlap(back, own),
        "{back:?} {next:?} {own:?}"
    );
    assert!(
        (back.min().y.0 - own.max().y.0 - ew_scene::page::SLOT_TEXT_GAP.0).abs() <= 1_000,
        "one gap below its own designation: {back:?} {own:?}"
    );
}
