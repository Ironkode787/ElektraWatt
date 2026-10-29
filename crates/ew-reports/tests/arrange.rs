// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `arrange`: manual exclusion, filter, sort, grouping (doc 09 §3.3, FR-REP-027).

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{AnyId, Date, GroupSpec, OrderUnit, Project, Set, SortKey, Unit};
use ew_derive::ew_units::Dec3;
use ew_derive::{Q, Rev};
use ew_reports::arrange::{cell_key, cell_text};
use ew_reports::{
    Arrangement, Cell, CellType, ColumnDef, Filter, FilterOp, MissingReason, NotDerived, QtyUnit, Row, RowId,
    RowKey, RowKind, RowSet, RowSetStatus, arrange,
};
use proptest::prelude::*;

const NAME: usize = 0;
const LOC: usize = 1;
const QTY: usize = 2;

fn model() -> ew_model::Model {
    let project: Project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .unwrap();
    ew_model::Model::from_records(project, [], []).unwrap()
}

fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::new("name", CellType::Ident, "Name"),
        ColumnDef::new("loc", CellType::Ident, "Location"),
        ColumnDef::new("qty", CellType::Qty, "Quantity"),
    ]
}

fn sort(keys: &[(&str, bool)]) -> Vec<SortKey> {
    keys.iter()
        .map(|(k, d)| serde_json::from_value(serde_json::json!({"key": k, "descending": d})).unwrap())
        .collect()
}

fn group(key: &str) -> GroupSpec {
    serde_json::from_value(serde_json::json!({"key": key})).unwrap()
}

fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn aid(n: u128) -> AnyId {
    AnyId::from_uuid(uuid(n))
}

fn qty(n: i64) -> Cell {
    Cell::Qty(Dec3(n), QtyUnit::Order(OrderUnit::Piece))
}

/// A data row with the identity `Index(n)`.
fn row(n: u32, name: &str, loc: &str, q: i64) -> Row {
    Row::data(
        RowId(vec![RowKey::Index(n)]),
        vec![Cell::Ident(name.to_owned()), Cell::Ident(loc.to_owned()), qty(q)],
    )
}

fn heading(n: u32, text: &str) -> Row {
    Row {
        kind: RowKind::Heading,
        ..Row::data(
            RowId(vec![RowKey::Name("h".into()), RowKey::Index(n)]),
            vec![Cell::Ident(text.to_owned()), Cell::Empty, Cell::Empty],
        )
    }
}

fn set(rows: Vec<Row>) -> RowSet {
    RowSet {
        rows,
        ..RowSet::new(columns())
    }
}

/// An arrangement with nothing set.
struct Args {
    sort: Vec<SortKey>,
    group: Option<GroupSpec>,
    filter: Option<Filter>,
    include: Set<AnyId>,
    exclude: Set<AnyId>,
    header: bool,
    footer: bool,
}

impl Args {
    fn new() -> Args {
        Args {
            sort: Vec::new(),
            group: None,
            filter: None,
            include: Set::new(),
            exclude: Set::new(),
            header: false,
            footer: false,
        }
    }

    fn run(&self, set: RowSet) -> RowSet {
        let m = model();
        let q = Q::new(&m);
        let how = Arrangement {
            sort: &self.sort,
            group: self.group.as_ref(),
            filter: self.filter.as_ref(),
            include: &self.include,
            exclude: &self.exclude,
            group_header: self.header,
            group_footer: self.footer,
            group_context: None,
        };
        arrange(&q, set, &how)
    }
}

fn names(set: &RowSet) -> Vec<String> {
    set.rows
        .iter()
        .map(|r| match &r.cells[NAME] {
            Cell::Ident(s) => s.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn col(key: &str, op: FilterOp) -> Filter {
    Filter::Column {
        key: key.to_owned(),
        op,
    }
}

// ---------------------------------------------------------------------------------------------------------
// Pass-through

#[test]
fn not_derived_passes_through_unchanged() {
    let mut s = RowSet::not_derived(columns(), NotDerived::AwaitsQuery("wire_rows"));
    s.rows.push(row(1, "b", "x", 1));
    s.rows.push(row(0, "a", "x", 1));
    let mut a = Args::new();
    a.sort = sort(&[("name", false)]);
    a.group = Some(group("loc"));
    a.header = true;
    let out = a.run(s.clone());
    assert_eq!(out, s);
    assert!(matches!(out.status, RowSetStatus::NotDerived(_)));
}

#[test]
fn default_arrangement_keeps_the_kinds_order() {
    let s = set(vec![
        row(2, "c", "x", 1),
        row(0, "a", "x", 1),
        row(1, "b", "x", 1),
    ]);
    let out = Args::new().run(s.clone());
    assert_eq!(out.rows, s.rows);
    assert!(out.groups.is_empty());
    assert_eq!(out.columns, s.columns);
}

// ---------------------------------------------------------------------------------------------------------
// Manual exclusion

fn source_row(n: u32, sources: &[u128]) -> Row {
    let mut r = row(n, &format!("r{n}"), "x", 1);
    r.sources = sources.iter().map(|s| aid(*s)).collect();
    r
}

#[test]
fn exclusion_drops_a_row_only_when_all_its_sources_are_excluded() {
    let s = set(vec![
        source_row(1, &[1, 2]),
        source_row(2, &[3]),
        source_row(3, &[1]),
        source_row(4, &[]),
    ]);
    let mut a = Args::new();
    a.exclude = Set::from(vec![aid(1), aid(3)]);
    // r1 keeps source 2; r2 is excluded fully; r3 is excluded fully; r4 has no entity at all.
    assert_eq!(names(&a.run(s.clone())), ["r1", "r4"]);

    a.exclude = Set::from(vec![aid(1), aid(2)]);
    assert_eq!(names(&a.run(s)), ["r2", "r4"]);
}

#[test]
fn exclusion_reads_the_row_id_when_sources_are_empty() {
    let by_id = Row::data(
        RowId(vec![RowKey::Entity(uuid(7)), RowKey::Name("x".into())]),
        vec![Cell::Ident("by-id".into()), Cell::Empty, Cell::Empty],
    );
    let other = Row::data(
        RowId::entity(Id::<()>::from_uuid(uuid(8))),
        vec![Cell::Ident("other".into()), Cell::Empty, Cell::Empty],
    );
    let mut a = Args::new();
    a.exclude = Set::from(vec![aid(7)]);
    assert_eq!(names(&a.run(set(vec![by_id, other]))), ["other"]);
}

#[test]
fn include_overrides_exclusion() {
    let s = set(vec![source_row(1, &[1, 2]), source_row(2, &[3])]);
    let mut a = Args::new();
    a.exclude = Set::from(vec![aid(1), aid(2), aid(3)]);
    assert!(a.run(s.clone()).rows.is_empty());
    a.include = Set::from(vec![aid(2)]);
    assert_eq!(names(&a.run(s)), ["r1"]);
}

#[test]
fn a_heading_without_data_rows_is_dropped() {
    let s = set(vec![
        heading(1, "H1"),
        source_row(1, &[1]),
        heading(2, "H2"),
        source_row(2, &[2]),
        source_row(3, &[3]),
        heading(3, "H3"),
        heading(4, "H4"),
        source_row(4, &[4]),
    ]);
    let mut a = Args::new();
    a.exclude = Set::from(vec![aid(1), aid(4)]);
    // H1 loses its only row, H3 has no rows at all (next row is a heading), H4 loses its row.
    assert_eq!(names(&a.run(s.clone())), ["H2", "r2", "r3"]);

    a.exclude = Set::new();
    assert_eq!(
        a.run(s.clone()).rows.len(),
        s.rows.len() - 1,
        "only H3 (empty segment) goes"
    );
}

#[test]
fn headings_are_not_excluded_by_their_entities() {
    let mut h = heading(1, "H1");
    h.sources = vec![aid(1)];
    let mut d = source_row(1, &[2]);
    d.sources = vec![aid(2)];
    let mut a = Args::new();
    a.exclude = Set::from(vec![aid(1)]);
    assert_eq!(names(&a.run(set(vec![h, d]))), ["H1", "r1"]);
}

// ---------------------------------------------------------------------------------------------------------
// Filter

fn located(locs: &[&str]) -> RowSet {
    set(locs
        .iter()
        .zip(0u32..)
        .map(|(l, i)| row(i, &format!("r{i}"), l, 1))
        .collect())
}

fn filtered(rows: RowSet, f: Filter) -> Vec<String> {
    let mut a = Args::new();
    a.filter = Some(f);
    names(&a.run(rows))
}

/// The column filter (doc 09 §3.3): one case per operator, combinator and cell class.
#[test]
fn filter_cases() {
    let abc = located(&["+A", "+B", "+C"]);
    let a = || col("loc", FilterOp::Eq("+A".into()));
    let b = || col("loc", FilterOp::Eq("+B".into()));
    // (name, rows, filter, the names of the rows that pass)
    let mut cases: Vec<(String, RowSet, Filter, Vec<&str>)> = vec![
        // Eq and Ne compare the cell text.
        (
            "eq".into(),
            located(&["+A", "+B", "+A"]),
            col("loc", FilterOp::Eq("+A".into())),
            vec!["r0", "r2"],
        ),
        (
            "ne".into(),
            located(&["+A", "+B", "+A"]),
            col("loc", FilterOp::Ne("+A".into())),
            vec!["r1"],
        ),
        // Within matches the value and its dotted children only.
        (
            "within".into(),
            located(&[
                "+J-E-2416",
                "+J-E-2416.1",
                "+J-E-24160",
                "+J-E-2417",
                "+J-E-2416-x",
                "",
                "+J",
            ]),
            col("loc", FilterOp::Within("+J-E-2416".into())),
            vec!["r0", "r1", "r4"],
        ),
        // Combinators and unknown columns.
        (
            "all of nothing".into(),
            abc.clone(),
            Filter::All(vec![]),
            vec!["r0", "r1", "r2"],
        ),
        ("any of nothing".into(), abc.clone(), Filter::Any(vec![]), vec![]),
        (
            "any of two".into(),
            abc.clone(),
            Filter::Any(vec![a(), b()]),
            vec!["r0", "r1"],
        ),
        (
            "all of two".into(),
            abc.clone(),
            Filter::All(vec![a(), b()]),
            vec![],
        ),
        (
            "not".into(),
            abc.clone(),
            Filter::Not(Box::new(a())),
            vec!["r1", "r2"],
        ),
        (
            "unknown column present".into(),
            abc.clone(),
            col("nope", FilterOp::Present),
            vec![],
        ),
        (
            "unknown column absent".into(),
            abc.clone(),
            col("nope", FilterOp::Absent),
            vec![],
        ),
        (
            "not unknown column present".into(),
            abc,
            Filter::Not(Box::new(col("nope", FilterOp::Present))),
            vec!["r0", "r1", "r2"],
        ),
    ];
    // The filter works on the text of typed cells.
    let mut typed = row(0, "x", "y", 1500);
    typed.cells[NAME] = Cell::Number(42);
    let typed = set(vec![typed]);
    cases.push((
        "typed number".into(),
        typed.clone(),
        col("name", FilterOp::Eq("42".into())),
        vec!["Number(42)"],
    ));
    cases.push((
        "typed quantity".into(),
        typed,
        col("qty", FilterOp::Eq("1.5".into())),
        vec!["Number(42)"],
    ));
    // Present and absent judge the value.
    let mk = |c: Cell| {
        let mut r = row(0, "x", "", 1);
        r.cells[LOC] = c;
        set(vec![r])
    };
    let cells = [
        (Cell::Ident("v".into()), true),
        (Cell::Empty, false),
        (Cell::Tbd, false),
        (Cell::Missing(MissingReason::Length), false),
        (Cell::Stale(Box::new(Cell::Ident("v".into())), Rev(1)), true),
        (Cell::Stale(Box::new(Cell::Empty), Rev(1)), false),
        (Cell::Number(0), true),
        (Cell::Bool(false), true),
    ];
    for (cell, present) in cells {
        let when = |on: bool| if on { vec!["x"] } else { vec![] };
        cases.push((
            format!("present {cell:?}"),
            mk(cell.clone()),
            col("loc", FilterOp::Present),
            when(present),
        ));
        cases.push((
            format!("absent {cell:?}"),
            mk(cell),
            col("loc", FilterOp::Absent),
            when(!present),
        ));
    }
    for (name, rows, f, want) in cases {
        assert_eq!(filtered(rows, f), want, "case {name}");
    }
}

#[test]
fn an_included_row_survives_the_filter() {
    let mut r1 = source_row(1, &[1]);
    r1.cells[LOC] = Cell::Ident("+B".into());
    let mut r2 = source_row(2, &[2]);
    r2.cells[LOC] = Cell::Ident("+B".into());
    let mut a = Args::new();
    a.filter = Some(col("loc", FilterOp::Eq("+A".into())));
    a.include = Set::from(vec![aid(2)]);
    assert_eq!(names(&a.run(set(vec![r1, r2]))), ["r2"]);
}

#[test]
fn a_heading_whose_rows_are_all_filtered_out_is_dropped() {
    let mut keep = row(1, "keep", "+A", 1);
    keep.cells[LOC] = Cell::Ident("+A".into());
    let s = set(vec![
        heading(1, "H1"),
        row(2, "gone", "+B", 1),
        heading(2, "H2"),
        keep,
        row(3, "gone2", "+B", 1),
    ]);
    assert_eq!(filtered(s, col("loc", FilterOp::Eq("+A".into()))), ["H2", "keep"]);
}

// ---------------------------------------------------------------------------------------------------------
// Sort

fn sorted(rows: Vec<Row>, keys: &[(&str, bool)]) -> Vec<String> {
    let mut a = Args::new();
    a.sort = sort(keys);
    names(&a.run(set(rows)))
}

#[test]
fn sort_by_one_key_in_natural_order() {
    let rows = vec![
        row(0, "X10", "", 1),
        row(1, "X2", "", 1),
        row(2, "X1", "", 1),
        row(3, "X20", "", 1),
    ];
    assert_eq!(
        sorted(rows.clone(), &[("name", false)]),
        ["X1", "X2", "X10", "X20"]
    );
    assert_eq!(sorted(rows, &[("name", true)]), ["X20", "X10", "X2", "X1"]);
}

#[test]
fn sort_by_two_keys_and_descending_applies_per_key() {
    let rows = vec![
        row(0, "b", "L2", 1),
        row(1, "a", "L2", 1),
        row(2, "c", "L1", 1),
        row(3, "a", "L1", 1),
    ];
    assert_eq!(
        sorted(rows.clone(), &[("loc", false), ("name", false)]),
        ["a", "c", "a", "b"]
    );
    // loc ascending, name descending: L1: c, a; L2: b, a.
    assert_eq!(
        sorted(rows.clone(), &[("loc", false), ("name", true)]),
        ["c", "a", "b", "a"]
    );
    assert_eq!(
        sorted(rows, &[("loc", true), ("name", false)]),
        ["a", "b", "a", "c"]
    );
}

#[test]
fn sort_ties_break_by_row_id_ascending() {
    let rows = vec![
        row(5, "same", "", 1),
        row(1, "same", "", 1),
        row(3, "same", "", 1),
    ];
    let mut a = Args::new();
    a.sort = sort(&[("name", false)]);
    let out = a.run(set(rows.clone()));
    let ids: Vec<RowId> = out.rows.iter().map(|r| r.id.clone()).collect();
    let mut expect: Vec<RowId> = rows.iter().map(|r| r.id.clone()).collect();
    expect.sort();
    assert_eq!(ids, expect);

    // Descending reverses the key only, not the tie-break.
    a.sort = sort(&[("name", true)]);
    let out = a.run(set(rows));
    let ids: Vec<RowId> = out.rows.iter().map(|r| r.id.clone()).collect();
    assert_eq!(ids, expect);
}

#[test]
fn sort_unknown_columns() {
    let cases: [(&str, Vec<Row>, &[(&str, bool)], [&str; 2]); 2] = [
        (
            "an unknown sort column is skipped",
            vec![row(0, "b", "", 1), row(1, "a", "", 1)],
            &[("nope", false), ("name", false)],
            ["a", "b"],
        ),
        (
            "sort keys that name no column keep the kind's order",
            vec![row(1, "b", "", 1), row(0, "a", "", 1)],
            &[("nope", false)],
            ["b", "a"],
        ),
    ];
    for (name, rows, keys, want) in cases {
        assert_eq!(sorted(rows, keys), want, "case {name}");
    }
}

#[test]
fn sorting_stays_inside_heading_segments() {
    let rows = vec![
        row(9, "z-lead", "", 1),
        heading(1, "H1"),
        row(0, "d", "", 1),
        row(1, "b", "", 1),
        heading(2, "H2"),
        row(2, "c", "", 1),
        row(3, "a", "", 1),
    ];
    assert_eq!(
        sorted(rows.clone(), &[("name", false)]),
        ["z-lead", "H1", "b", "d", "H2", "a", "c"]
    );
    assert_eq!(
        sorted(rows, &[("name", true)]),
        ["z-lead", "H1", "d", "b", "H2", "c", "a"]
    );
}

#[test]
fn sort_orders_value_classes_and_variants() {
    let mk = |n: u32, c: Cell| {
        let mut r = row(n, &format!("r{n}"), "", 1);
        r.cells[LOC] = c;
        r
    };
    let rows = vec![
        mk(0, Cell::Empty),
        mk(1, Cell::Missing(MissingReason::Part)),
        mk(2, Cell::Tbd),
        mk(3, Cell::Ident("a".into())),
        mk(4, Cell::Text(MlText::neutral("a"))),
        mk(5, Cell::Number(3)),
        mk(6, Cell::Missing(MissingReason::Length)),
        mk(7, Cell::Stale(Box::new(Cell::Number(1)), Rev(0))),
    ];
    // Text < Ident < Number (2 < 3 by value: the stale 1 first), then Tbd, Missing by reason, Empty.
    assert_eq!(
        sorted(rows, &[("loc", false)]),
        ["r4", "r3", "r7", "r5", "r2", "r6", "r1", "r0"]
    );
}

// ---------------------------------------------------------------------------------------------------------
// Grouping

#[test]
fn grouping_without_header_and_footer_orders_groups_naturally() {
    let rows = vec![
        row(0, "a", "X10", 1),
        row(1, "b", "X2", 1),
        row(2, "c", "X10", 1),
        row(3, "d", "X2", 1),
    ];
    let mut a = Args::new();
    a.group = Some(group("loc"));
    let out = a.run(set(rows));
    assert_eq!(
        names(&out),
        ["b", "d", "a", "c"],
        "X2 before X10, rows keep their order"
    );
    assert_eq!(out.groups.len(), 2);
    assert_eq!(out.groups[0].text, "X2");
    assert_eq!(out.groups[0].key, Cell::Ident("X2".into()));
    assert_eq!(out.groups[0].rows, 0..2);
    assert_eq!(out.groups[1].text, "X10");
    assert_eq!(out.groups[1].rows, 2..4);
}

#[test]
fn group_rows_keep_the_sort_order() {
    let rows = vec![
        row(0, "b", "L", 1),
        row(1, "a", "L", 1),
        row(2, "d", "K", 1),
        row(3, "c", "K", 1),
    ];
    let mut a = Args::new();
    a.group = Some(group("loc"));
    a.sort = sort(&[("name", false)]);
    assert_eq!(names(&a.run(set(rows))), ["c", "d", "a", "b"]);
}

/// One grouping case with group headers and/or footers.
struct GroupRowsCase {
    name: &'static str,
    rows: Vec<Row>,
    header: bool,
    footer: bool,
    kinds: Vec<RowKind>,
    /// The checked groups: (row range, text); `None` leaves the groups unchecked.
    groups: Option<Vec<(std::ops::Range<u32>, &'static str)>>,
    /// Further checks of the arranged rows.
    extra: fn(&str, &RowSet),
}

#[test]
fn group_headers_and_footers() {
    use RowKind::{Data, GroupFooter, GroupHeader};
    let cases = [
        GroupRowsCase {
            name: "headers and footers with ranges",
            rows: vec![
                row(0, "a", "L2", 1500),
                row(1, "b", "L1", 2000),
                row(2, "c", "L2", 250),
            ],
            header: true,
            footer: true,
            kinds: vec![
                GroupHeader,
                Data,
                GroupFooter,
                GroupHeader,
                Data,
                Data,
                GroupFooter,
            ],
            groups: Some(vec![(0..3, "L1"), (3..7, "L2")]),
            extra: |c, out| {
                let header = &out.rows[0];
                assert_eq!(
                    header.id,
                    RowId(vec![
                        RowKey::Name("group-header".into()),
                        RowKey::Name("L1".into())
                    ]),
                    "case {c}"
                );
                assert_eq!(
                    header.cells,
                    vec![Cell::Empty, Cell::Ident("L1".into()), Cell::Empty],
                    "case {c}: only the key column is filled"
                );
                assert!(header.links.is_empty() && header.sources.is_empty(), "case {c}");

                let footer = &out.rows[2];
                assert_eq!(
                    footer.id,
                    RowId(vec![
                        RowKey::Name("group-footer".into()),
                        RowKey::Name("L1".into())
                    ]),
                    "case {c}"
                );
                assert_eq!(footer.cells[LOC], Cell::Ident("L1".into()), "case {c}");
                assert_eq!(footer.cells[NAME], Cell::Empty, "case {c}");
                assert_eq!(footer.cells[QTY], qty(2000), "case {c}");

                let footer2 = &out.rows[6];
                assert_eq!(footer2.cells[QTY], qty(1750), "case {c}: 1.5 + 0.25");
            },
        },
        GroupRowsCase {
            name: "only headers",
            rows: vec![row(0, "a", "L", 1000), row(1, "b", "L", 1000)],
            header: true,
            footer: false,
            kinds: vec![GroupHeader, Data, Data],
            groups: Some(vec![(0..3, "L")]),
            extra: |_, _| {},
        },
        GroupRowsCase {
            name: "only footers",
            rows: vec![row(0, "a", "L", 1000), row(1, "b", "L", 1000)],
            header: false,
            footer: true,
            kinds: vec![Data, Data, GroupFooter],
            groups: None,
            extra: |c, out| assert_eq!(out.rows[2].cells[QTY], qty(2000), "case {c}"),
        },
    ];
    for c in cases {
        let mut a = Args::new();
        a.group = Some(group("loc"));
        a.header = c.header;
        a.footer = c.footer;
        let out = a.run(set(c.rows));
        let kinds: Vec<RowKind> = out.rows.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, c.kinds, "case {}", c.name);
        if let Some(groups) = c.groups {
            let got: Vec<(std::ops::Range<u32>, &str)> = out
                .groups
                .iter()
                .map(|g| (g.rows.clone(), g.text.as_str()))
                .collect();
            assert_eq!(got, groups, "case {}", c.name);
        }
        (c.extra)(c.name, &out);
    }
}

/// The quantity sum of a group footer: (name, rows, the footers as (row index, quantity cell)).
#[test]
fn footer_sum_cases() {
    let stale = {
        let mut r1 = row(0, "a", "L", 1000);
        r1.cells[QTY] = Cell::Stale(Box::new(qty(1000)), Rev(2));
        let mut r2 = row(1, "b", "L", 1000);
        r2.cells[QTY] = Cell::Empty;
        vec![r1, r2, row(2, "c", "L", 3000)]
    };
    let mixed = {
        let mut r1 = row(0, "a", "L", 1000);
        r1.cells[QTY] = Cell::Qty(Dec3(1000), QtyUnit::Unit(Unit::Mm2));
        let mut r3 = row(2, "c", "M", 1000);
        r3.cells[QTY] = Cell::Tbd;
        vec![r1, row(1, "b", "L", 1000), r3]
    };
    let cases: [(&str, Vec<Row>, Vec<(usize, Cell)>); 3] = [
        (
            "the sum counts stale cells and skips non-quantity cells",
            stale,
            vec![(3, qty(4000))],
        ),
        // L mixes units, M has no quantity: both footers are empty.
        (
            "mixed units or no quantities give an empty footer",
            mixed,
            vec![(2, Cell::Empty), (4, Cell::Empty)],
        ),
        (
            "the sum saturates",
            vec![row(0, "a", "L", i64::MAX), row(1, "b", "L", 5)],
            vec![(2, qty(i64::MAX))],
        ),
    ];
    for (name, rows, footers) in cases {
        let mut a = Args::new();
        a.group = Some(group("loc"));
        a.footer = true;
        let out = a.run(set(rows));
        for (i, want) in footers {
            assert_eq!(out.rows[i].kind, RowKind::GroupFooter, "case {name}: row {i}");
            assert_eq!(out.rows[i].cells[QTY], want, "case {name}: row {i}");
        }
    }
}

#[test]
fn an_unknown_group_column_means_no_grouping() {
    let rows = vec![row(1, "b", "L", 1), row(0, "a", "K", 1)];
    let mut a = Args::new();
    a.group = Some(group("nope"));
    a.header = true;
    a.footer = true;
    let out = a.run(set(rows.clone()));
    assert_eq!(out.rows, rows);
    assert!(out.groups.is_empty());
}

#[test]
fn headings_join_the_group_of_their_first_data_row() {
    let rows = vec![
        heading(1, "H1"),
        row(0, "a", "L2", 1),
        row(1, "b", "L1", 1),
        heading(2, "H2"),
        row(2, "c", "L1", 1),
    ];
    let mut a = Args::new();
    a.group = Some(group("loc"));
    let out = a.run(set(rows));
    // Headings bound groups. Segment 1: H1 starts with `a` (L2), so H1 goes to L2 and L1 holds `b`; segment 2
    // is one group L1 with H2.
    assert_eq!(names(&out), ["b", "H1", "a", "H2", "c"]);
    assert_eq!(out.groups.len(), 3);
    assert_eq!(out.groups[0].rows, 0..1);
    assert_eq!(out.groups[1].rows, 1..3);
    assert_eq!(out.groups[2].rows, 3..5);
}

#[test]
fn grouping_sorts_inside_segments_first() {
    let rows = vec![
        heading(1, "H1"),
        row(0, "d", "G", 1),
        row(1, "b", "G", 1),
        heading(2, "H2"),
        row(2, "c", "G", 1),
        row(3, "a", "G", 1),
    ];
    let mut a = Args::new();
    a.sort = sort(&[("name", false)]);
    a.group = Some(group("loc"));
    let out = a.run(set(rows));
    assert_eq!(names(&out), ["H1", "b", "d", "H2", "a", "c"]);
    // The same key in two heading segments gives two groups, each in its own segment.
    assert_eq!(out.groups.len(), 2);
    assert_eq!(out.groups[0].rows, 0..3);
    assert_eq!(out.groups[1].rows, 3..6);
}

#[test]
fn group_text_and_header_ids_of_other_key_types() {
    let cols = vec![
        ColumnDef::new("n", CellType::Number, "N"),
        ColumnDef::new("d", CellType::Date, "D"),
    ];
    let mk = |i: u32, n: i64, d: &str| {
        Row::data(
            RowId(vec![RowKey::Index(i)]),
            vec![Cell::Number(n), Cell::Date(d.parse::<Date>().unwrap())],
        )
    };
    let s = RowSet {
        columns: cols,
        rows: vec![
            mk(0, 10, "2026-03-04"),
            mk(1, 9, "2026-03-04"),
            mk(2, 10, "2025-12-31"),
        ],
        ..RowSet::new(Vec::new())
    };
    let mut a = Args::new();
    a.group = Some(group("n"));
    a.header = true;
    let out = a.run(s.clone());
    assert_eq!(out.groups[0].text, "9");
    assert_eq!(out.groups[1].text, "10");
    assert_eq!(
        out.rows[0].id,
        RowId(vec![
            RowKey::Name("group-header".into()),
            RowKey::Name("9".into())
        ])
    );

    a.group = Some(group("d"));
    let out = a.run(s);
    assert_eq!(out.groups[0].text, "2025-12-31");
    assert_eq!(out.groups[1].text, "2026-03-04");
}

#[test]
fn arrange_leaves_columns_status_and_as_of_alone() {
    let mut s = set(vec![row(0, "a", "L", 1)]);
    s.as_of = Some(Rev(7));
    let out = Args::new().run(s.clone());
    assert_eq!(out.as_of, Some(Rev(7)));
    assert_eq!(out.columns, s.columns);
    assert_eq!(out.status, RowSetStatus::Derived);
}

// ---------------------------------------------------------------------------------------------------------
// cell_key and cell_text

/// The sort and group text of a colour is the printed text, a code of a newer format included.
#[test]
fn the_text_of_a_colour_is_its_printed_text() {
    use ew_derive::ew_schema::{ColourCode, ColourSpec};
    use ew_reports::render::{RenderOptions, render};
    let m = model();
    let q = Q::new(&m);
    let unknown = ColourSpec::one(ColourCode::Unknown(serde_json::json!({"future": 1})));
    for spec in [
        unknown,
        serde_json::from_str("[\"BK\"]").unwrap(),
        serde_json::from_str("[\"GN\",\"YE\"]").unwrap(),
    ] {
        let c = Cell::Colour(spec);
        assert_eq!(cell_text(&q, &c), render(&q, &c, &RenderOptions::default()));
    }
}

#[test]
fn cell_text_of_simple_cells() {
    let m = model();
    let q = Q::new(&m);
    let t = |c: Cell| cell_text(&q, &c);
    assert_eq!(t(Cell::Text(MlText::neutral("hello"))), "hello");
    assert_eq!(t(Cell::Ident("A-1".into())), "A-1");
    assert_eq!(t(Cell::Address("%I0.1".into())), "%I0.1");
    assert_eq!(t(Cell::Number(-3)), "-3");
    assert_eq!(t(qty(750)), "0.75");
    assert_eq!(t(Cell::Date("2026-01-09".parse().unwrap())), "2026-01-09");
    assert_eq!(t(Cell::Bool(true)), "true");
    assert_eq!(t(Cell::Bool(false)), "false");
    assert_eq!(t(Cell::Tbd), "TBD");
    assert_eq!(t(Cell::Missing(MissingReason::Colour)), "");
    assert_eq!(t(Cell::Empty), "");
    assert_eq!(t(Cell::Stale(Box::new(Cell::Number(5)), Rev(1))), "5");
    assert_eq!(t(Cell::Folios(Vec::new())), "");
    assert_eq!(t(Cell::Range(Vec::new())), "");
    assert_eq!(t(Cell::Colour(serde_json::from_str("[\"BK\"]").unwrap())), "BK");
    // Two codes (GNYE) print joined.
    assert_eq!(
        t(Cell::Colour(serde_json::from_str("[\"GN\",\"YE\"]").unwrap())),
        "GNYE"
    );
    assert_eq!(
        t(Cell::Colour(
            serde_json::from_str("[{\"foreign\":\"BR\"}]").unwrap()
        )),
        "BR"
    );
}

#[test]
fn cell_text_picks_the_source_language() {
    let project: Project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1",
            "languages":{"source":"nl-NL","translations":["en-GB"]}}"#,
    )
    .unwrap();
    let m = ew_model::Model::from_records(project, [], []).unwrap();
    let q = Q::new(&m);
    let langs = |pairs: &[(&str, &str)]| {
        Cell::Text(MlText::from_langs(
            pairs.iter().map(|(l, t)| (l.parse().unwrap(), (*t).to_owned())),
        ))
    };
    let both = langs(&[("en-GB", "Motor"), ("nl-NL", "Motor (nl)")]);
    assert_eq!(cell_text(&q, &both), "Motor (nl)");
    // No source text: the first language in tag order.
    let only_en = langs(&[("en-GB", "Motor")]);
    assert_eq!(cell_text(&q, &only_en), "Motor");
    assert_eq!(cell_text(&q, &Cell::Text(MlText::neutral("n"))), "n");

    // Without a project language the first language is used.
    let m0 = model();
    let q0 = Q::new(&m0);
    assert_eq!(cell_text(&q0, &both), "Motor");
}

/// `cell_key` is a total order over the value classes and orders text naturally: (name, a, b, a against b).
#[test]
fn cell_key_order() {
    use std::cmp::Ordering::{Equal, Less};
    let m = model();
    let q = Q::new(&m);
    let ident = |s: &str| Cell::Ident(s.to_owned());
    let ascending = [
        Cell::Text(MlText::neutral("zzz")),
        Cell::Ident("a".into()),
        Cell::Number(-1),
        Cell::Number(7),
        Cell::Qty(Dec3(1), QtyUnit::None),
        Cell::Colour(serde_json::from_str("[\"BK\"]").unwrap()),
        Cell::Address("a".into()),
        Cell::Date("2026-01-01".parse().unwrap()),
        Cell::Bool(false),
        Cell::Bool(true),
        Cell::Tbd,
        Cell::Missing(MissingReason::Length),
        Cell::Missing(MissingReason::Colour),
        Cell::Empty,
    ];
    let mut cases: Vec<(String, Cell, Cell, std::cmp::Ordering)> = ascending
        .windows(2)
        .map(|w| {
            (
                format!("class {:?} < {:?}", w[0], w[1]),
                w[0].clone(),
                w[1].clone(),
                Less,
            )
        })
        .collect();
    cases.extend([
        (
            "a stale cell ranks as its value".to_owned(),
            Cell::Number(1),
            Cell::Stale(Box::new(Cell::Number(1)), Rev(3)),
            Equal,
        ),
        (
            "the unit does not rank".to_owned(),
            Cell::Qty(Dec3(5), QtyUnit::None),
            qty(5),
            Equal,
        ),
        ("natural X2 < X10".to_owned(), ident("X2"), ident("X10"), Less),
        ("natural X10 < X10a".to_owned(), ident("X10"), ident("X10a"), Less),
        ("natural -F1 < -F12".to_owned(), ident("-F1"), ident("-F12"), Less),
    ]);
    for (name, a, b, want) in cases {
        assert_eq!(
            cell_key(&q, &a).partial_cmp(&cell_key(&q, &b)),
            Some(want),
            "case {name}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// Properties

fn arb_rows() -> impl Strategy<Value = Vec<Row>> {
    // Distinct row IDs (the index), cells drawn from a small alphabet so ties are common.
    proptest::collection::vec(("[A-C][0-9]{1,2}", "[LM][0-9]", 0i64..5), 0..24).prop_map(|v| {
        v.into_iter()
            .zip(0u32..)
            .map(|((name, loc, q), i)| row(i, &name, &loc, q))
            .collect()
    })
}

proptest! {
    #[test]
    fn sort_does_not_depend_on_input_order(
        rows in arb_rows(),
        seed in any::<u64>(),
        descending in any::<bool>(),
        second in any::<bool>(),
    ) {
        let mut shuffled = rows.clone();
        // A deterministic permutation from the seed (Fisher-Yates over a simple LCG).
        let mut x = seed | 1;
        for i in (1..shuffled.len()).rev() {
            x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let j = usize::try_from(x >> 33).unwrap() % (i + 1);
            shuffled.swap(i, j);
        }
        let mut a = Args::new();
        a.sort = if second {
            sort(&[("loc", descending), ("name", false)])
        } else {
            sort(&[("name", descending)])
        };
        prop_assert_eq!(a.run(set(rows.clone())), a.run(set(shuffled.clone())));

        // Grouped and with header and footer rows as well.
        a.group = Some(group("loc"));
        a.header = true;
        a.footer = true;
        prop_assert_eq!(a.run(set(rows)), a.run(set(shuffled)));
    }
}

// ---------------------------------------------------------------------------------------------------------
// Group order follows the sort direction; headings bound groups

#[test]
fn a_descending_group_column_gives_descending_groups() {
    let rows = vec![
        row(0, "a", "K2", 1),
        row(1, "b", "K10", 1),
        row(2, "c", "K1", 1),
        row(3, "d", "K2", 1),
    ];
    let mut a = Args::new();
    a.group = Some(group("loc"));
    a.sort = sort(&[("loc", true)]);
    let out = a.run(set(rows.clone()));
    let texts: Vec<&str> = out.groups.iter().map(|g| g.text.as_str()).collect();
    assert_eq!(texts, ["K10", "K2", "K1"]);
    assert_eq!(names(&out), ["b", "a", "d", "c"]);

    // A descending sort on another column leaves the groups ascending.
    a.sort = sort(&[("name", true)]);
    let out = a.run(set(rows));
    let texts: Vec<&str> = out.groups.iter().map(|g| g.text.as_str()).collect();
    assert_eq!(texts, ["K1", "K2", "K10"]);
}

#[test]
fn two_heading_segments_with_one_key_give_two_groups_in_segment_order() {
    let rows = vec![
        heading(1, "H1"),
        row(0, "a", "L", 1),
        row(1, "b", "L", 1),
        heading(2, "H2"),
        row(2, "c", "L", 1),
    ];
    let mut a = Args::new();
    a.group = Some(group("loc"));
    a.header = true;
    let out = a.run(set(rows));
    assert_eq!(out.groups.len(), 2);
    assert_eq!(out.groups[0].text, "L");
    assert_eq!(out.groups[1].text, "L");
    // Header, H1, a, b | header, H2, c.
    assert_eq!(out.groups[0].rows, 0..4);
    assert_eq!(out.groups[1].rows, 4..7);
    assert_eq!(out.rows[0].kind, RowKind::GroupHeader);
    assert_eq!(out.rows[4].kind, RowKind::GroupHeader);
}

// ---------------------------------------------------------------------------------------------------------
// Group texts of tags, folio texts of generated pages, parts_list row IDs (over small JSON models)

use ew_derive::PageRef;
use ew_derive::ew_schema::{LibDocument, Record, ReportBlock};
use ew_reports::rows_of;
use serde_json::{Value, json};

fn uid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

fn build(records: &[Value], lib: &[Value]) -> ew_model::Model {
    let project: Project =
        serde_json::from_value(json!({"id": uid(1), "derivation_profile": "2027.1"})).unwrap();
    let records: Vec<Record> = records
        .iter()
        .map(|r| serde_json::from_value(r.clone()).unwrap_or_else(|e| panic!("record {r}: {e}")))
        .collect();
    let form = json!({
        "$k": "form", "id": uid(0xF0), "library_id": uid(0x7000), "item_id": uid(0x10F0),
        "version": "1.0.0", "sha256": format!("sha256:{}", "ab".repeat(32)), "kind": "report",
        "sheet": {"width": 297_000_000, "height": 210_000_000},
    });
    let lib: Vec<LibDocument> = std::iter::once(&form)
        .chain(lib.iter())
        .map(|d| serde_json::from_value(d.clone()).unwrap_or_else(|e| panic!("lib {d}: {e}")))
        .collect();
    ew_model::Model::from_records(project, records, lib).unwrap_or_else(|e| panic!("model: {e:?}"))
}

fn lib_part(n: u128, number: &str) -> Value {
    json!({
        "$k": "part", "id": uid(n), "library_id": uid(0x7000), "item_id": uid(n + 0x1_0000),
        "version": "1.0.0", "sha256": format!("sha256:{}", "ab".repeat(32)),
        "meta": {}, "kind": "device",
        "manufacturer": {"library": uid(0x7000), "item": uid(0x9000), "version_req": "^1"},
        "part_number": number, "description": "Part", "unit": "piece",
    })
}

fn block_record(n: u128, kind: &str, first_folio: u32, extra: Value) -> Value {
    let mut r = json!({
        "$k": "report_block", "id": uid(n), "source": {"kind": kind}, "form": uid(0xF0),
        "first_folio": first_folio, "title": {"en": "Report {n}"},
    });
    for (k, v) in extra.as_object().unwrap() {
        r[k] = v.clone();
    }
    r
}

fn block_id(n: u128) -> Id<ReportBlock> {
    Id::from_uuid(uuid(n))
}

#[test]
fn group_texts_print_the_displayed_tag_for_the_section_structure() {
    let structure = json!({"function": uid(0x10), "location": uid(0x11)});
    let mut records = vec![
        json!({"$k": "structure_node", "id": uid(0x10), "aspect": "function", "code": "P01"}),
        json!({"$k": "structure_node", "id": uid(0x11), "aspect": "location", "code": "J-E-2416"}),
        json!({"$k": "section", "id": uid(0x20), "number": 1, "folio_range": [100, 199],
               "title": {"en": "S"}, "divider": false}),
        // The section's one page has the function only: the devices' location stays visible.
        json!({"$k": "page", "id": uid(0x21), "folio": {"number": 101, "section": uid(0x20)},
               "kind": "multi_line", "structure": {"function": uid(0x10)}, "title": {"en": "P"},
               "grid": {"num": 4_000_000, "den": 1}}),
    ];
    for (n, tag) in [(0x1000u128, "S52"), (0x1001, "S51")] {
        records.push(json!({
            "$k": "device", "id": uid(n), "tag": {"letter": "S", "frozen": {"text": tag}},
            "structure": structure,
            "parts": [{"part": {"copy": uid(0x601)}, "qty": "1"}],
        }));
    }
    let grouped = json!({"group": {"key": "device"}});
    let mut in_section = grouped.clone();
    in_section["section"] = json!(uid(0x20));
    records.push(block_record(0xB1, "parts_list", 901, in_section));
    records.push(block_record(0xB2, "parts_list", 905, grouped));
    let m = build(&records, &[lib_part(0x601, "P1")]);
    let q = Q::new(&m);

    let in_section = rows_of(&q, block_id(0xB1));
    let texts: Vec<&str> = in_section.groups.iter().map(|g| g.text.as_str()).collect();
    assert_eq!(
        texts,
        ["+J-E-2416-S51", "+J-E-2416-S52"],
        "displayed without the shared function"
    );

    let without = rows_of(&q, block_id(0xB2));
    let texts: Vec<&str> = without.groups.iter().map(|g| g.text.as_str()).collect();
    assert_eq!(texts, ["-S51", "-S52"], "the short tag without a section");

    // The group cell and the grouping key keep the device (full tag order).
    assert_eq!(in_section.groups[0].key, without.groups[0].key);
    assert!(matches!(in_section.groups[0].key, Cell::Tag(_)));
}

#[test]
fn group_texts_in_a_section_with_the_devices_structure_omit_all_of_it() {
    let structure = json!({"function": uid(0x10), "location": uid(0x11)});
    let records = vec![
        json!({"$k": "structure_node", "id": uid(0x10), "aspect": "function", "code": "P01"}),
        json!({"$k": "structure_node", "id": uid(0x11), "aspect": "location", "code": "J-E-2416"}),
        json!({"$k": "section", "id": uid(0x20), "number": 1, "folio_range": [100, 199],
               "title": {"en": "S"}, "divider": false}),
        json!({"$k": "page", "id": uid(0x21), "folio": {"number": 101, "section": uid(0x20)},
               "kind": "multi_line", "structure": structure, "title": {"en": "P"},
               "grid": {"num": 4_000_000, "den": 1}}),
        json!({"$k": "device", "id": uid(0x1000), "tag": {"letter": "S", "frozen": {"text": "S51"}},
               "structure": structure, "parts": [{"part": {"copy": uid(0x601)}, "qty": "1"}]}),
        block_record(
            0xB1,
            "parts_list",
            901,
            json!({"group": {"key": "device"}, "section": uid(0x20)}),
        ),
    ];
    let m = build(&records, &[lib_part(0x601, "P1")]);
    let q = Q::new(&m);
    assert_eq!(rows_of(&q, block_id(0xB1)).groups[0].text, "-S51");
}

#[test]
fn a_next_folio_block_names_its_generated_pages_by_their_folio() {
    // 30 structure nodes need two pages of the house layout.
    let mut records: Vec<Value> = (0..30u128)
        .map(|i| json!({"$k": "structure_node", "id": uid(0x100 + i), "aspect": "location", "code": format!("N{i:02}")}))
        .collect();
    records.push(block_record(
        0xB1,
        "structure_overview",
        901,
        json!({"continuation": "next_folio"}),
    ));
    records.push(block_record(0xB2, "structure_overview", 950, json!({})));
    records.push(block_record(
        0xB3,
        "toc",
        1,
        json!({"continuation": "next_folio"}),
    ));
    records.push(block_record(0xB4, "toc", 1, json!({})));
    let m = build(&records, &[]);
    let q = Q::new(&m);
    let at = |b: u128, index: u32| {
        cell_text(
            &q,
            &Cell::Folio(PageRef::Generated {
                block: block_id(b),
                index,
            }),
        )
    };

    let pages = q.get::<ew_reports::BlockPages>((block_id(0xB1), 0));
    assert!(pages.len() >= 2);
    for g in pages.iter() {
        assert_eq!(at(0xB1, g.index), g.label);
    }
    assert_eq!(at(0xB1, 1), "902", "not `901.1`");
    assert!(
        cell_key(
            &q,
            &Cell::Folio(PageRef::Generated {
                block: block_id(0xB1),
                index: 0
            })
        ) < cell_key(
            &q,
            &Cell::Folio(PageRef::Generated {
                block: block_id(0xB1),
                index: 1
            })
        )
    );

    // Sub-page continuation: unchanged.
    assert_eq!(at(0xB2, 0), "950");
    assert_eq!(at(0xB2, 1), "950.1");

    // A table of contents never reads its pages: its policy decides.
    assert_eq!(at(0xB3, 2), "3");
    assert_eq!(at(0xB4, 2), "1.2");
}

#[test]
fn a_new_folio_group_break_is_reflected_in_the_folio_text() {
    // One device and two structure nodes: group "Device" (1 row), then "Structure" (2 rows).
    let records = vec![
        json!({"$k": "structure_node", "id": uid(0x100), "aspect": "location", "code": "A"}),
        json!({"$k": "structure_node", "id": uid(0x101), "aspect": "location", "code": "B"}),
        json!({"$k": "device", "id": uid(0x1000), "tag": {"letter": "S", "frozen": {"text": "S51"}}}),
        block_record(
            0xB1,
            "structure_overview",
            901,
            json!({"group": {"key": "kind", "break": "new_folio"}}),
        ),
    ];
    let m = build(&records, &[]);
    let q = Q::new(&m);
    let pages = q.get::<ew_reports::BlockPages>((block_id(0xB1), 0));
    let labels: Vec<&str> = pages.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(labels, ["901", "902"]);
    let at = |index: u32| {
        cell_text(
            &q,
            &Cell::Folio(PageRef::Generated {
                block: block_id(0xB1),
                index,
            }),
        )
    };
    assert_eq!(at(0), "901");
    assert_eq!(at(1), "902", "not `901.1`");
}

fn parts_list_ids(parts: &[Value]) -> Vec<(String, RowId)> {
    let records = vec![
        json!({"$k": "device", "id": uid(0x1000), "tag": {"letter": "F", "frozen": {"text": "F1"}},
               "parts": parts}),
        block_record(0xB1, "parts_list", 901, json!({})),
    ];
    let lib = [lib_part(0x601, "A"), lib_part(0x602, "B"), lib_part(0x603, "C")];
    let m = build(&records, &lib);
    let q = Q::new(&m);
    let rows = rows_of(&q, block_id(0xB1));
    rows.rows
        .iter()
        .map(|r| {
            let Cell::Ident(number) = &r.cells[4] else {
                panic!("part number cell")
            };
            (number.clone(), r.id.clone())
        })
        .collect()
}

fn use_of(n: u128, variant: Option<&str>) -> Value {
    let mut part = json!({"copy": uid(n)});
    if let Some(v) = variant {
        part["variant"] = json!(v);
    }
    json!({"part": part, "qty": "1"})
}

#[test]
fn parts_list_row_ids_survive_inserting_a_part_use() {
    let before = parts_list_ids(&[use_of(0x601, None), use_of(0x602, None)]);
    let after = parts_list_ids(&[use_of(0x603, None), use_of(0x601, None), use_of(0x602, None)]);
    assert_eq!(before.len(), 2);
    assert_eq!(after.len(), 3);
    for (number, id) in &before {
        let found = after.iter().find(|(n, _)| n == number).expect("row survives");
        assert_eq!(&found.1, id, "row of part {number}");
    }
}

#[test]
fn equal_part_uses_get_ordinals_and_variants_separate_rows() {
    let rows = parts_list_ids(&[use_of(0x601, None), use_of(0x601, None), use_of(0x601, Some("v"))]);
    assert_eq!(rows.len(), 3);
    let mut ids: Vec<RowId> = rows.iter().map(|r| r.1.clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3, "all distinct");
    let ordinals: Vec<Option<&RowKey>> = rows.iter().map(|(_, id)| id.0.last()).collect();
    let mut plain: Vec<&RowKey> = rows
        .iter()
        .filter(|(_, id)| !id.0.iter().any(|k| matches!(k, RowKey::Name(_))))
        .filter_map(|(_, id)| id.0.last())
        .collect();
    plain.sort();
    assert_eq!(
        plain,
        [&RowKey::Index(0), &RowKey::Index(1)],
        "ordinals of equal uses"
    );
    assert!(ordinals.iter().all(Option::is_some));
}
