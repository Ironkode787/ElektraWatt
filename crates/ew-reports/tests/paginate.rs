// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pagination of a row set into generated pages (doc 09 §3.2, FR-PAG-011, FR-PAG-030), over hand-built
//! row sets, block specs and form layouts: no model.

use ew_derive::GeneratedPage;
use ew_derive::ew_ids::{Id, Uuid, v5};
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{FormDef, ReportBlock, Section};
use ew_derive::ew_units::Nm;
use ew_reports::paginate::Slice;
use ew_reports::{
    BlockSpec, Break, Cell, CellType, ColumnDef, Continuation, FixedLines, GroupRange, Layouts, NotDerived,
    PageLayout, Row, RowId, RowKey, RowKind, RowLines, RowSet, paginate,
};

/// The height of one row of the hand-built layouts (the house height).
const H: i64 = 6_520_000;

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
    }
}

fn columns() -> Vec<ColumnDef> {
    vec![ColumnDef::new("n", CellType::Number, "N")]
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

/// A layout of `rows` rows of height `H` with group-header rows of the same height.
fn small(form_id: Id<FormDef>, rows: i64, with_group_header: bool) -> PageLayout {
    PageLayout {
        form: form_id,
        capacity: Nm(H * rows),
        data: Nm(H),
        group_header: with_group_header.then_some(Nm(H)),
        group_footer: None,
        heading: Nm(H),
        wraps: [false; 4],
    }
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

#[test]
fn one_and_twenty_three_rows_make_one_page() {
    for n in [1, 23] {
        let pages = paginate(&data_set(n), &spec(901), &house(), &FixedLines);
        assert_eq!(pages.len(), 1, "{n} rows");
        assert_eq!(pages[0].label, "901");
        assert_eq!(pages[0].number, 901);
        assert_eq!(pages[0].rows, 0..n);
        assert_eq!(pages[0].page_count, 1);
    }
}

#[test]
fn twenty_four_rows_make_two_pages() {
    let pages = paginate(&data_set(24), &spec(901), &house(), &FixedLines);
    assert_eq!(labels(&pages), ["901", "901.1"]);
    assert_eq!(ranges(&pages), [(0, 23), (23, 24)]);
}

#[test]
fn twenty_seven_rows_with_titles() {
    let pages = paginate(&data_set(27), &spec(901), &house(), &FixedLines);
    assert_eq!(labels(&pages), ["901", "901.1"]);
    let t: Vec<&str> = pages.iter().map(title).collect();
    assert_eq!(t, ["Partlist 1", "Partlist 2"]);
    assert_eq!(ranges(&pages), [(0, 23), (23, 27)]);
    // Both pages carry the folio number of the run.
    assert_eq!(pages.iter().map(|p| p.number).collect::<Vec<_>>(), [901, 901]);
}

#[test]
fn fifty_rows_at_907() {
    let pages = paginate(&data_set(50), &spec(907), &house(), &FixedLines);
    assert_eq!(labels(&pages), ["907", "907.1", "907.2"]);
    assert_eq!(ranges(&pages), [(0, 23), (23, 46), (46, 50)]);
}

#[test]
fn fifty_six_rows_at_908() {
    let pages = paginate(&data_set(56), &spec(908), &house(), &FixedLines);
    assert_eq!(labels(&pages), ["908", "908.1", "908.2"]);
    assert_eq!(ranges(&pages), [(0, 23), (23, 46), (46, 56)]);
}

#[test]
fn seventy_two_rows_at_910() {
    let pages = paginate(&data_set(72), &spec(910), &house(), &FixedLines);
    assert_eq!(labels(&pages), ["910", "910.1", "910.2", "910.3"]);
    assert_eq!(ranges(&pages), [(0, 23), (23, 46), (46, 69), (69, 72)]);
}

#[test]
fn every_page_carries_its_identity() {
    let pages = paginate(&data_set(60), &spec(901), &house(), &FixedLines);
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
        assert!(!p.continued_group);
    }
}

#[test]
fn title_placeholders_fill_n_count_and_group() {
    let mut s = spec(901);
    s.title = MlText::neutral("{group}: page {n} of {count}");
    let pages = paginate(&data_set(30), &s, &house(), &FixedLines);
    // No grouping: `{group}` is empty.
    assert_eq!(title(&pages[0]), ": page 1 of 2");
    assert_eq!(title(&pages[1]), ": page 2 of 2");

    let mut set = grouped_set(&[3, 3], false);
    set.groups[0].text = "Alpha".into();
    set.groups[1].text = "Beta".into();
    let mut s = spec(901);
    s.title = MlText::neutral("List {group} {n}/{count}");
    s.group_break = Break::NewPage;
    let pages = paginate(&set, &s, &house(), &FixedLines);
    let t: Vec<&str> = pages.iter().map(title).collect();
    assert_eq!(t, ["List Alpha 1/2", "List Beta 2/2"]);
}

#[test]
fn a_multi_language_title_fills_each_language() {
    let en = "en-GB".parse().unwrap();
    let nl = "nl-NL".parse().unwrap();
    let mut s = spec(901);
    s.title = MlText::from_langs([(en, "Parts {n}/{count}"), (nl, "Onderdelen {n}/{count}")]);
    let pages = paginate(&data_set(30), &s, &house(), &FixedLines);
    assert_eq!(pages.len(), 2);
    let en: ew_derive::ew_mltext::LangTag = "en-GB".parse().unwrap();
    let nl: ew_derive::ew_mltext::LangTag = "nl-NL".parse().unwrap();
    assert_eq!(pages[0].title.get(&en), Some("Parts 1/2"));
    assert_eq!(pages[0].title.get(&nl), Some("Onderdelen 1/2"));
    assert_eq!(pages[1].title.get(&en), Some("Parts 2/2"));
    assert_eq!(pages[1].title.get(&nl), Some("Onderdelen 2/2"));
}

#[test]
fn next_folio_continuation_numbers_whole_folios() {
    let mut s = spec(907);
    s.continuation = Continuation::NextFolio;
    let pages = paginate(&data_set(50), &s, &house(), &FixedLines);
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
    let pages = paginate(&data_set(1), &s, &house(), &FixedLines);
    assert_eq!(pages[0].section, None);
}

// ---------------------------------------------------------------------------------------------------------
// Empty and not-derived sets

#[test]
fn an_empty_set_without_policy_has_no_pages() {
    assert!(paginate(&data_set(0), &spec(901), &house(), &FixedLines).is_empty());
}

#[test]
fn an_empty_set_with_a_text_has_one_note_page() {
    let mut s = spec(901);
    s.empty = Some(MlText::neutral("Nothing to list"));
    let pages = paginate(&data_set(0), &s, &house(), &FixedLines);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].rows, 0..0);
    assert_eq!(pages[0].note, Some(MlText::neutral("Nothing to list")));
    assert_eq!(pages[0].label, "901");
    assert_eq!(pages[0].page_count, 1);
    assert_eq!(title(&pages[0]), "Partlist 1");
}

#[test]
fn headings_alone_count_as_empty() {
    let set = RowSet {
        rows: vec![row(RowKind::Heading, 0), row(RowKind::Heading, 1)],
        ..RowSet::new(columns())
    };
    assert!(paginate(&set, &spec(901), &house(), &FixedLines).is_empty());
    let mut s = spec(901);
    s.empty = Some(MlText::neutral("none"));
    let pages = paginate(&set, &s, &house(), &FixedLines);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].note, Some(MlText::neutral("none")));
}

#[test]
fn a_not_derived_set_is_one_placeholder_page_with_the_reason() {
    let set = RowSet::not_derived(columns(), NotDerived::AwaitsQuery("wire_rows"));
    // Even a block that omits empty pages prints the placeholder.
    let pages = paginate(&set, &spec(910), &house(), &FixedLines);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].label, "910");
    assert_eq!(pages[0].rows, 0..0);
    let note = pages[0].note.as_ref().expect("a note");
    let text = note.as_neutral().unwrap();
    assert!(text.contains("awaits"), "{text}");
    assert!(text.contains("wire_rows"), "{text}");
    assert!(text.starts_with("Not derived"), "capitalised: {text}");
}

// ---------------------------------------------------------------------------------------------------------
// Groups

#[test]
fn new_page_break_puts_each_group_on_a_new_page() {
    let set = grouped_set(&[3, 30, 2], false);
    let mut s = spec(903);
    s.group_break = Break::NewPage;
    let pages = paginate(&set, &s, &house(), &FixedLines);
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
    let pages = paginate(&set, &s, &house(), &FixedLines);
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
    let pages = paginate(&set, &s, &house(), &FixedLines);
    assert_eq!(labels(&pages), ["903", "904", "905", "905.1"]);
    assert_eq!(ranges(&pages), [(0, 3), (3, 23), (23, 46), (46, 53)]);
    assert_eq!(pages[3].group.as_deref(), Some("G2"));
}

#[test]
fn no_break_packs_groups_continuously() {
    let set = grouped_set(&[10, 10, 10], false);
    let pages = paginate(&set, &spec(903), &house(), &FixedLines);
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
    let pages = paginate(&set, &s, &house(), &FixedLines);
    assert_eq!(ranges(&pages), [(0, 3), (3, 6)]);
}

// ---------------------------------------------------------------------------------------------------------
// Group headers

#[test]
fn a_group_header_is_kept_with_its_first_row() {
    // Five rows per page: group A is header + 3 rows, so group B's header would be the last row of the
    // page; it moves to the next page with its first data row.
    let set = grouped_set(&[3, 2], true);
    let layouts = Layouts::single(small(form(1), 5, true));
    let pages = paginate(&set, &spec(901), &layouts, &FixedLines);
    assert_eq!(ranges(&pages), [(0, 4), (4, 7)]);
    assert_eq!(pages[0].group.as_deref(), Some("G0"));
    assert_eq!(pages[1].group.as_deref(), Some("G1"));
    assert!(!pages[1].continued_group, "the group begins on this page");
    assert_eq!(
        set.rows[usize::try_from(pages[1].rows.start).unwrap()].kind,
        RowKind::GroupHeader
    );
}

#[test]
fn a_header_that_fits_with_its_first_row_stays() {
    let set = grouped_set(&[2, 2], true);
    let layouts = Layouts::single(small(form(1), 6, true));
    let pages = paginate(&set, &spec(901), &layouts, &FixedLines);
    // 3 + 3 rows fill exactly one page.
    assert_eq!(ranges(&pages), [(0, 6)]);
}

#[test]
fn a_continued_group_repeats_its_header_and_reserves_its_height() {
    // One group of a header and nine rows on a five-row page.
    let set = grouped_set(&[9], true);
    let layouts = Layouts::single(small(form(1), 5, true));
    let pages = paginate(&set, &spec(901), &layouts, &FixedLines);
    // First page: header + 4 rows; continuation pages: the repeated header + 4 rows.
    assert_eq!(ranges(&pages), [(0, 5), (5, 9), (9, 10)]);
    assert_eq!(
        pages.iter().map(|p| p.continued_group).collect::<Vec<_>>(),
        [false, true, true]
    );
    assert!(pages.iter().all(|p| p.group.as_deref() == Some("G0")));
}

#[test]
fn without_a_header_template_continuation_pages_are_full() {
    let set = grouped_set(&[9], false);
    let layouts = Layouts::single(small(form(1), 5, false));
    let pages = paginate(&set, &spec(901), &layouts, &FixedLines);
    assert_eq!(ranges(&pages), [(0, 5), (5, 9)]);
    assert!(pages.iter().all(|p| !p.continued_group));
}

#[test]
fn a_heading_is_kept_with_its_next_row() {
    // Rows: 4 data, a heading, 2 data on a five-row page.
    let mut set = data_set(4);
    set.rows.push(row(RowKind::Heading, 50));
    set.rows.push(row(RowKind::Data, 4));
    set.rows.push(row(RowKind::Data, 5));
    let layouts = Layouts::single(small(form(1), 5, false));
    let pages = paginate(&set, &spec(901), &layouts, &FixedLines);
    assert_eq!(ranges(&pages), [(0, 4), (4, 7)]);
}

// ---------------------------------------------------------------------------------------------------------
// Continuation chain

#[test]
fn the_form_chain_gives_each_page_of_a_run_its_form() {
    let layouts = Layouts {
        chain: vec![small(form(11), 10, false), small(form(12), 20, false)],
    };
    let pages = paginate(&data_set(45), &spec(901), &layouts, &FixedLines);
    assert_eq!(ranges(&pages), [(0, 10), (10, 30), (30, 45)]);
    assert_eq!(
        pages.iter().map(|p| p.form).collect::<Vec<_>>(),
        [form(11), form(12), form(12)]
    );
}

#[test]
fn a_new_folio_restarts_the_form_chain() {
    let layouts = Layouts {
        chain: vec![small(form(11), 10, false), small(form(12), 20, false)],
    };
    let set = grouped_set(&[25, 5], false);
    let mut s = spec(901);
    s.group_break = Break::NewFolio;
    let pages = paginate(&set, &s, &layouts, &FixedLines);
    assert_eq!(labels(&pages), ["901", "901.1", "902"]);
    assert_eq!(ranges(&pages), [(0, 10), (10, 25), (25, 30)]);
    assert_eq!(
        pages.iter().map(|p| p.form).collect::<Vec<_>>(),
        [form(11), form(12), form(11)]
    );
}

// ---------------------------------------------------------------------------------------------------------
// Wrapping

/// Every row takes two lines.
struct TwoLines;

impl RowLines for TwoLines {
    fn lines(&self, _row: &Row, _layout: &PageLayout) -> u32 {
        2
    }
}

/// Must never be asked.
struct Never;

impl RowLines for Never {
    fn lines(&self, _row: &Row, _layout: &PageLayout) -> u32 {
        panic!("a row of a kind that does not wrap was measured");
    }
}

#[test]
fn wrapping_rows_take_their_measured_lines() {
    let mut layout = PageLayout::house(form(1));
    layout.wraps[0] = true;
    let pages = paginate(&data_set(25), &spec(901), &Layouts::single(layout), &TwoLines);
    // 23 lines per page: eleven two-line rows.
    assert_eq!(ranges(&pages), [(0, 11), (11, 22), (22, 25)]);
}

#[test]
fn rows_that_do_not_wrap_are_never_measured() {
    let pages = paginate(&data_set(30), &spec(901), &house(), &Never);
    assert_eq!(ranges(&pages), [(0, 23), (23, 30)]);

    // Only the kinds whose template wraps are measured: here none of the data rows.
    let mut layout = PageLayout::house(form(1));
    layout.wraps[3] = true;
    let mut set = data_set(2);
    set.rows.insert(0, row(RowKind::Heading, 9));
    let result = std::panic::catch_unwind(|| paginate(&set, &spec(901), &Layouts::single(layout), &Never));
    assert!(result.is_err(), "the wrapping heading is measured");
}

/// The row with number 1 takes 30 lines; the others one.
struct TallSecond;

impl RowLines for TallSecond {
    fn lines(&self, row: &Row, _layout: &PageLayout) -> u32 {
        match row.cells[0] {
            Cell::Number(1) => 30,
            _ => 1,
        }
    }
}

#[test]
fn a_row_taller_than_the_page_is_placed_alone() {
    let mut layout = PageLayout::house(form(1));
    layout.wraps[0] = true;
    let layouts = Layouts::single(layout);
    // The tall row is the second of four.
    let pages = paginate(&data_set(4), &spec(901), &layouts, &TallSecond);
    assert_eq!(ranges(&pages), [(0, 1), (1, 2), (2, 4)]);

    // The tall row is the first row: it is placed on an empty page, then the next row starts a new one.
    struct TallFirst;
    impl RowLines for TallFirst {
        fn lines(&self, row: &Row, _layout: &PageLayout) -> u32 {
            match row.cells[0] {
                Cell::Number(0) => 40,
                _ => 1,
            }
        }
    }
    let pages = paginate(&data_set(3), &spec(901), &layouts, &TallFirst);
    assert_eq!(ranges(&pages), [(0, 1), (1, 3)]);
}

// ---------------------------------------------------------------------------------------------------------
// Table of contents, determinism

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

#[test]
fn paginating_twice_gives_equal_pages() {
    let set = grouped_set(&[7, 30, 3], true);
    let layouts = Layouts {
        chain: vec![small(form(11), 9, true), small(form(12), 12, true)],
    };
    let mut s = spec(901);
    s.group_break = Break::NewPage;
    let a = paginate(&set, &s, &layouts, &FixedLines);
    let b = paginate(&set, &s, &layouts, &FixedLines);
    assert!(!a.is_empty());
    assert_eq!(a, b);
}
