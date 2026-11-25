// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pagination of a row set into generated pages (doc 09 §3.2, FR-PAG-011, FR-PAG-030), over hand-built
//! row sets, block specs and form layouts: no model.

use ew_derive::GeneratedPage;
use ew_ids::{Id, Uuid, v5};
use ew_mltext::MlText;
use ew_reports::paginate::Slice;
use ew_reports::{
    BlockSpec, Break, Cell, CellType, ColumnDef, Continuation, GroupRange, Layouts, NotDerived, PageLayout,
    Row, RowId, RowKey, RowKind, RowSet, paginate,
};
use ew_schema::{FormDef, ReportBlock, Section};

fn form(n: u128) -> Id<FormDef> {
    Id::from_uuid(Uuid::from_u128(n))
}

fn block() -> Id<ReportBlock> {
    Id::from_uuid(Uuid::from_u128(0xB10C))
}

fn section() -> Id<Section> {
    Id::from_uuid(Uuid::from_u128(0x5EC7))
}

fn spec(first_folio: u32) -> BlockSpec {
    BlockSpec {
        block: block(),
        section: Some(section()),
        first_folio,
        continuation: Continuation::SubPage,
        title: MlText::neutral("Partlist {n}"),
        empty: None,
        group_break: Break::None,
        first_sub: 0,
    }
}

fn columns() -> Vec<ColumnDef> {
    vec![ColumnDef::new(
        "n",
        CellType::Number,
        ew_reports::text!("column-count"),
    )]
}

fn row(kind: RowKind, i: u32) -> Row {
    Row {
        kind,
        ..Row::data(RowId(vec![RowKey::Index(i)]), vec![Cell::Number(i64::from(i))])
    }
}

/// `n` data rows.
fn data_set(n: u32) -> RowSet {
    RowSet {
        rows: (0..n).map(|i| row(RowKind::Data, i)).collect(),
        ..RowSet::new(columns())
    }
}

fn house() -> Layouts {
    Layouts::single(PageLayout::house(form(1)))
}

fn labels(pages: &[GeneratedPage]) -> Vec<&str> {
    pages.iter().map(|p| p.label.as_str()).collect()
}

fn ranges(pages: &[GeneratedPage]) -> Vec<(u32, u32)> {
    pages.iter().map(|p| (p.rows.start, p.rows.end)).collect()
}

fn title(p: &GeneratedPage) -> &str {
    p.title.as_neutral().expect("a neutral title")
}

/// A layout of `rows` list-area rows.
fn small(form_id: Id<FormDef>, rows: i64) -> PageLayout {
    PageLayout::listed(form_id, rows)
}

fn group(text: &str, rows: std::ops::Range<u32>) -> GroupRange {
    GroupRange {
        text: text.to_owned(),
        key: Cell::Ident(text.to_owned()),
        rows,
    }
}

/// A grouped set: group `i` has `sizes[i]` data rows, preceded by a header row when `headers`.
fn grouped_set(sizes: &[u32], headers: bool) -> RowSet {
    let mut set = RowSet::new(columns());
    let mut next = 0u32;
    for (g, size) in sizes.iter().enumerate() {
        let start = u32::try_from(set.rows.len()).unwrap();
        if headers {
            set.rows
                .push(row(RowKind::GroupHeader, 1000 + u32::try_from(g).unwrap()));
        }
        for _ in 0..*size {
            set.rows.push(row(RowKind::Data, next));
            next += 1;
        }
        let end = u32::try_from(set.rows.len()).unwrap();
        set.groups.push(group(&format!("G{g}"), start..end));
    }
    set
}

// ---------------------------------------------------------------------------------------------------------
// House layout

/// One house-layout page-count case: `rows` data rows from folio `first`.
struct PageCount {
    name: &'static str,
    rows: u32,
    first: u32,
    labels: &'static [&'static str],
    ranges: &'static [(u32, u32)],
    /// The page titles, when the case checks them.
    titles: Option<&'static [&'static str]>,
    /// The folio number of every page, when the case checks it.
    numbers: Option<&'static [u32]>,
}

#[test]
fn house_layout_page_counts() {
    #[rustfmt::skip]
    let cases = [
        PageCount { name: "1 row makes one page", rows: 1, first: 901, labels: &["901"], ranges: &[(0, 1)],
            titles: None, numbers: Some(&[901]) },
        PageCount { name: "23 rows make one page", rows: 23, first: 901, labels: &["901"], ranges: &[(0, 23)],
            titles: None, numbers: Some(&[901]) },
        // The last page takes rows from the page before, so it holds three.
        PageCount { name: "24 rows make two pages", rows: 24, first: 901, labels: &["901", "901.1"],
            ranges: &[(0, 21), (21, 24)], titles: None, numbers: None },
        // Both pages carry the folio number of the run.
        PageCount { name: "27 rows with titles", rows: 27, first: 901, labels: &["901", "901.1"],
            ranges: &[(0, 23), (23, 27)], titles: Some(&["Partlist 1", "Partlist 2"]), numbers: Some(&[901, 901]) },
        PageCount { name: "50 rows at 907", rows: 50, first: 907, labels: &["907", "907.1", "907.2"],
            ranges: &[(0, 23), (23, 46), (46, 50)], titles: None, numbers: None },
        PageCount { name: "56 rows at 908", rows: 56, first: 908, labels: &["908", "908.1", "908.2"],
            ranges: &[(0, 23), (23, 46), (46, 56)], titles: None, numbers: None },
        PageCount { name: "72 rows at 910", rows: 72, first: 910, labels: &["910", "910.1", "910.2", "910.3"],
            ranges: &[(0, 23), (23, 46), (46, 69), (69, 72)], titles: None, numbers: None },
    ];
    for c in cases {
        let pages = paginate(&data_set(c.rows), &spec(c.first), &house());
        assert_eq!(labels(&pages), c.labels, "case {}", c.name);
        assert_eq!(ranges(&pages), c.ranges, "case {}", c.name);
        let count = u32::try_from(c.labels.len()).unwrap();
        assert!(
            pages.iter().all(|p| p.page_count == count),
            "case {}: page_count",
            c.name
        );
        if let Some(titles) = c.titles {
            let t: Vec<&str> = pages.iter().map(title).collect();
            assert_eq!(t, titles, "case {}", c.name);
        }
        if let Some(numbers) = c.numbers {
            assert_eq!(
                pages.iter().map(|p| p.number).collect::<Vec<_>>(),
                numbers,
                "case {}",
                c.name
            );
        }
    }
}

#[test]
fn every_page_carries_its_identity() {
    let pages = paginate(&data_set(60), &spec(901), &house());
    assert_eq!(pages.len(), 3);
    for (k, p) in pages.iter().enumerate() {
        let k = u32::try_from(k).unwrap();
        let id: Id<GeneratedPage> = v5::report_page_id(block(), k);
        assert_eq!(p.id, id);
        assert_eq!(p.index, k);
        assert_eq!(p.page_count, 3);
        assert_eq!(p.block, block());
        assert_eq!(p.section, Some(section()));
        assert_eq!(p.form, form(1));
        assert_eq!(p.note, None);
        assert_eq!(p.group, None);
    }
}

#[test]
fn title_placeholders_fill_n_count_and_group() {
    let mut s = spec(901);
    s.title = MlText::neutral("{group}: page {n} of {count}");
    let pages = paginate(&data_set(30), &s, &house());
    // No grouping: `{group}` is empty.
    assert_eq!(title(&pages[0]), ": page 1 of 2");
    assert_eq!(title(&pages[1]), ": page 2 of 2");

    let mut set = grouped_set(&[3, 3], false);
    set.groups[0].text = "Alpha".into();
    set.groups[1].text = "Beta".into();
    let mut s = spec(901);
    s.title = MlText::neutral("List {group} {n}/{count}");
    s.group_break = Break::NewPage;
    let pages = paginate(&set, &s, &house());
    let t: Vec<&str> = pages.iter().map(title).collect();
    assert_eq!(t, ["List Alpha 1/2", "List Beta 2/2"]);
}

#[test]
fn a_multi_language_title_fills_each_language() {
    let en = "en-GB".parse().unwrap();
    let nl = "nl-NL".parse().unwrap();
    let mut s = spec(901);
    s.title = MlText::from_langs([(en, "Parts {n}/{count}"), (nl, "Onderdelen {n}/{count}")]);
    let pages = paginate(&data_set(30), &s, &house());
    assert_eq!(pages.len(), 2);
    let en: ew_mltext::LangTag = "en-GB".parse().unwrap();
    let nl: ew_mltext::LangTag = "nl-NL".parse().unwrap();
    assert_eq!(pages[0].title.get(&en), Some("Parts 1/2"));
    assert_eq!(pages[0].title.get(&nl), Some("Onderdelen 1/2"));
    assert_eq!(pages[1].title.get(&en), Some("Parts 2/2"));
    assert_eq!(pages[1].title.get(&nl), Some("Onderdelen 2/2"));
}

#[test]
fn next_folio_continuation_numbers_whole_folios() {
    let mut s = spec(907);
    s.continuation = Continuation::NextFolio;
    let pages = paginate(&data_set(50), &s, &house());
    assert_eq!(labels(&pages), ["907", "908", "909"]);
    assert_eq!(
        pages.iter().map(|p| p.number).collect::<Vec<_>>(),
        [907, 908, 909]
    );
}

#[test]
fn an_absent_section_stays_absent() {
    let mut s = spec(901);
    s.section = None;
    let pages = paginate(&data_set(1), &s, &house());
    assert_eq!(pages[0].section, None);
}

// ---------------------------------------------------------------------------------------------------------
// Empty and not-derived sets

#[test]
fn empty_set_cases() {
    let headings = RowSet {
        rows: vec![row(RowKind::Heading, 0), row(RowKind::Heading, 1)],
        ..RowSet::new(columns())
    };
    // (name, set, the empty-set text of the block)
    let cases = [
        ("no rows without a policy", data_set(0), None),
        ("no rows with a text", data_set(0), Some("Nothing to list")),
        ("headings alone without a policy", headings.clone(), None),
        ("headings alone with a text", headings, Some("none")),
    ];
    for (name, set, empty) in cases {
        let mut s = spec(901);
        s.empty = empty.map(MlText::neutral);
        let pages = paginate(&set, &s, &house());
        let Some(text) = empty else {
            assert!(pages.is_empty(), "case {name}: {pages:?}");
            continue;
        };
        // One note page.
        assert_eq!(pages.len(), 1, "case {name}");
        assert_eq!(pages[0].rows, 0..0, "case {name}");
        assert_eq!(pages[0].note, Some(MlText::neutral(text)), "case {name}");
        assert_eq!(pages[0].label, "901", "case {name}");
        assert_eq!(pages[0].page_count, 1, "case {name}");
        assert_eq!(title(&pages[0]), "Partlist 1", "case {name}");
    }
}

#[test]
fn a_not_derived_set_is_one_placeholder_page_with_the_reason() {
    let set = RowSet::not_derived(columns(), NotDerived::Unsupported("wire_rows".into()));
    // Even a block that omits empty pages prints the placeholder.
    let pages = paginate(&set, &spec(910), &house());
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].label, "910");
    assert_eq!(pages[0].rows, 0..0);
    let note = pages[0].note.as_ref().expect("a note");
    // The reason in words, in English and Dutch, without the family's name.
    let text = |tag: &str| note.get(&tag.parse().unwrap()).unwrap_or_default().to_owned();
    let (en, nl) = (text("en-GB"), text("nl-NL"));
    assert!(
        en.starts_with("Not derived") && nl.starts_with("Niet afgeleid"),
        "{en} / {nl}"
    );
    assert!(!en.contains("wire_rows") && !en.contains('`'), "{en}");
}

// ---------------------------------------------------------------------------------------------------------
// Groups

#[test]
fn new_page_break_puts_each_group_on_a_new_page() {
    let set = grouped_set(&[3, 30, 2], false);
    let mut s = spec(903);
    s.group_break = Break::NewPage;
    let pages = paginate(&set, &s, &house());
    assert_eq!(labels(&pages), ["903", "903.1", "903.2", "903.3"]);
    assert_eq!(ranges(&pages), [(0, 3), (3, 26), (26, 33), (33, 35)]);
    let groups: Vec<Option<&str>> = pages.iter().map(|p| p.group.as_deref()).collect();
    assert_eq!(groups, [Some("G0"), Some("G1"), Some("G1"), Some("G2")]);
}

#[test]
fn new_folio_break_starts_the_next_folio() {
    let set = grouped_set(&[3, 20, 5], false);
    let mut s = spec(903);
    s.group_break = Break::NewFolio;
    let pages = paginate(&set, &s, &house());
    assert_eq!(labels(&pages), ["903", "904", "905"]);
    assert_eq!(
        pages.iter().map(|p| p.number).collect::<Vec<_>>(),
        [903, 904, 905]
    );
    assert_eq!(ranges(&pages), [(0, 3), (3, 23), (23, 28)]);
}

#[test]
fn a_long_group_after_a_folio_break_continues_as_sub_pages() {
    let set = grouped_set(&[3, 20, 30], false);
    let mut s = spec(903);
    s.group_break = Break::NewFolio;
    let pages = paginate(&set, &s, &house());
    assert_eq!(labels(&pages), ["903", "904", "905", "905.1"]);
    assert_eq!(ranges(&pages), [(0, 3), (3, 23), (23, 46), (46, 53)]);
    assert_eq!(pages[3].group.as_deref(), Some("G2"));
}

#[test]
fn no_break_packs_groups_continuously() {
    let set = grouped_set(&[10, 10, 10], false);
    let pages = paginate(&set, &spec(903), &house());
    assert_eq!(labels(&pages), ["903", "903.1"]);
    assert_eq!(ranges(&pages), [(0, 23), (23, 30)]);
    // The page's group is the first group on it.
    assert_eq!(pages[0].group.as_deref(), Some("G0"));
    assert_eq!(pages[1].group.as_deref(), Some("G2"));
}

#[test]
fn an_empty_group_range_is_skipped() {
    let mut set = grouped_set(&[3, 3], false);
    set.groups.insert(1, group("Empty", 3..3));
    let mut s = spec(901);
    s.group_break = Break::NewPage;
    let pages = paginate(&set, &s, &house());
    assert_eq!(ranges(&pages), [(0, 3), (3, 6)]);
}

// ---------------------------------------------------------------------------------------------------------
// Group headers

#[test]
fn the_form_chain_gives_each_page_of_a_run_its_form() {
    let layouts = Layouts {
        chain: vec![small(form(11), 10), small(form(12), 20)],
    };
    let pages = paginate(&data_set(45), &spec(901), &layouts);
    assert_eq!(ranges(&pages), [(0, 10), (10, 30), (30, 45)]);
    assert_eq!(
        pages.iter().map(|p| p.form).collect::<Vec<_>>(),
        [form(11), form(12), form(12)]
    );
}

#[test]
fn a_new_folio_restarts_the_form_chain() {
    let layouts = Layouts {
        chain: vec![small(form(11), 10), small(form(12), 20)],
    };
    let set = grouped_set(&[25, 5], false);
    let mut s = spec(901);
    s.group_break = Break::NewFolio;
    let pages = paginate(&set, &s, &layouts);
    assert_eq!(labels(&pages), ["901", "901.1", "902"]);
    assert_eq!(ranges(&pages), [(0, 10), (10, 25), (25, 30)]);
    assert_eq!(
        pages.iter().map(|p| p.form).collect::<Vec<_>>(),
        [form(11), form(12), form(11)]
    );
}

// ---------------------------------------------------------------------------------------------------------
// Wrapping

#[test]
fn toc_slices_number_like_other_blocks() {
    let slices: Vec<Slice> = paginate::toc_slices(2, 23);
    let mut s = spec(2);
    s.title = MlText::neutral("Table of Contents {n}");
    let pages = paginate::number(&slices, &RowSet::new(columns()), &s, &house());
    assert_eq!(labels(&pages), ["2", "2.1"]);
    let t: Vec<&str> = pages.iter().map(title).collect();
    assert_eq!(t, ["Table of Contents 1", "Table of Contents 2"]);
    assert_eq!(ranges(&pages), [(0, 23), (23, 46)]);
    assert_eq!(pages[0].page_count, 2);
}

#[test]
fn a_toc_has_at_least_one_page() {
    assert_eq!(paginate::toc_slices(0, 23).len(), 1);
}

// ---------------------------------------------------------------------------------------------------------
// Keep-with-next and the "(cont.)" reservation (doc 09 §3.2)

#[test]
fn title_braces_are_literal_when_doubled() {
    use ew_reports::paginate::fill_title;
    assert_eq!(fill_title("{x {{n}} }", "1", "2", "g"), "{x {n} }");
    assert_eq!(fill_title("Partlist {n}", "1", "2", "g"), "Partlist 1");
    assert_eq!(fill_title("{{{n}}}", "1", "2", "g"), "{1}");
    assert_eq!(fill_title("{group} {n}", "1", "2", "{n}"), "{n} 1");
    assert_eq!(fill_title("{foo} {count} }{", "1", "2", "g"), "{foo} 2 }{");
}

#[test]
fn a_group_text_with_a_placeholder_is_printed_literally_in_the_title() {
    let mut set = grouped_set(&[3], false);
    set.groups[0].text = "{n}".into();
    let mut s = spec(901);
    s.title = MlText::neutral("{group} {n}");
    let pages = paginate(&set, &s, &house());
    assert_eq!(title(&pages[0]), "{n} 1");
}

// ---------------------------------------------------------------------------------------------------------
// Properties

mod properties {
    use super::*;
    use ew_mltext::natural_key;
    use proptest::prelude::*;

    fn arb_layout(form_id: Id<FormDef>) -> impl Strategy<Value = PageLayout> {
        (1i64..=30).prop_map(move |rows| PageLayout { form: form_id, rows })
    }

    /// Groups of items; an item is `true` for a heading row, `false` for a data row.
    fn arb_groups() -> impl Strategy<Value = Vec<Vec<bool>>> {
        prop::collection::vec(prop::collection::vec(prop::bool::weighted(0.15), 0..40), 0..6)
    }

    fn build(groups: &[Vec<bool>], grouped: bool, headers: bool) -> RowSet {
        let mut set = RowSet::new(columns());
        for (g, items) in groups.iter().enumerate() {
            let start = u32::try_from(set.rows.len()).unwrap();
            if grouped && headers {
                set.rows.push(row(RowKind::GroupHeader, start));
            }
            for is_heading in items {
                let id = u32::try_from(set.rows.len()).unwrap();
                let kind = if *is_heading {
                    RowKind::Heading
                } else {
                    RowKind::Data
                };
                set.rows.push(row(kind, id));
            }
            let end = u32::try_from(set.rows.len()).unwrap();
            if grouped {
                set.groups.push(group(&format!("G{g}"), start..end));
            }
        }
        set
    }

    proptest! {
        #[test]
        fn pagination_partitions_rows_within_capacity_and_numbers_in_order(
            groups in arb_groups(),
            grouped in any::<bool>(),
            headers in any::<bool>(),
            first in arb_layout(form(1)),
            second in arb_layout(form(2)),
            brk in 0usize..3,
            next_folio in any::<bool>(),
        ) {
            let set = build(&groups, grouped, headers);
            let layouts = Layouts { chain: vec![first, second] };
            let mut s = spec(901);
            s.group_break = [Break::None, Break::NewPage, Break::NewFolio][brk];
            if next_folio {
                s.continuation = Continuation::NextFolio;
            }
            let pages = paginate(&set, &s, &layouts);
            let len = u32::try_from(set.rows.len()).unwrap();

            if set.data_rows() == 0 {
                prop_assert!(pages.is_empty());
                return Ok(());
            }
            // (a) the ranges partition 0..len in order.
            let mut next = 0u32;
            for p in &pages {
                prop_assert!(!p.rows.is_empty());
                prop_assert_eq!(p.rows.start, next);
                next = p.rows.end;
            }
            prop_assert_eq!(next, len);

            // (b) the rows used are within the page's rows unless the page holds a single row.
            for p in &pages {
                let layout = layouts.chain.iter().find(|l| l.form == p.form).unwrap();
                let used: i64 = set.rows[p.rows.start as usize..p.rows.end as usize]
                    .iter()
                    .map(|r| layout.takes(r.kind))
                    .sum();
                prop_assert!(
                    used <= layout.rows || p.rows.len() == 1,
                    "page {} uses {used} of {}", p.label, layout.rows
                );
            }

            // (c) labels strictly increase in natural order, folio numbers never decrease.
            for w in pages.windows(2) {
                prop_assert!(
                    natural_key(&w[0].label) < natural_key(&w[1].label),
                    "{} then {}", w[0].label, w[1].label
                );
                prop_assert!(w[0].number <= w[1].number);
            }
        }
    }
}
