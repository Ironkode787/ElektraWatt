// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frame cells (doc 06 §5.1, doc 20 §6.1 and §15, FR-PAG-020): `cell_of` snaps first and uses exact
//! rationals, `rep_anchor_cell` reads the symbol's anchor policy.

mod common;

use common::{Fx, id, uid};
use ew_derive::{CellOf, Q, RepAnchorCell};
use ew_model::Model;
use ew_schema::{Page, Representation};
use ew_units::{Grid, Nm, Pt2};
use proptest::prelude::*;
use serde_json::{Value, json};

const PAGE: u128 = 0x101;
const FNDEF: u128 = 0x500;
const SYM: u128 = 0x510;
const DEVICE: u128 = 0x1000;
const FUNCTION: u128 = 0x1001;
const FRAME_COPY: &str = "c80af135-34f8-574c-8f52-756bd24c7bd0";

/// A fixture with the golden grid frame and its title block loaded.
fn golden() -> Fx {
    let mut fx = Fx::new();
    fx.lib(
        serde_json::from_str(include_str!(
            "../../../content/forms/ew.frame.qet-a4-landscape-grid.json"
        ))
        .unwrap(),
    );
    fx.lib(serde_json::from_str(include_str!("../../../content/forms/ew.titleblock.qet-a4.json")).unwrap());
    fx
}

/// Adds the page with the golden frame on the QET 10 px grid.
fn qet_page(fx: &mut Fx) {
    fx.page(PAGE, 101, None, "multi_line", &[], "Power");
    fx.record(PAGE)["form"] = json!(FRAME_COPY);
    fx.record(PAGE)["grid"] = json!({"num": 7_937_500, "den": 3});
}

fn pt(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(x), Nm(y))
}

/// The point at grid index `(kx, ky)` of the QET 10 px grid.
fn qet_at(kx: i64, ky: i64) -> Pt2 {
    pt(
        Grid::QET_10PX.at(kx).expect("fits").0,
        Grid::QET_10PX.at(ky).expect("fits").0,
    )
}

fn cell_str(m: &Model, page: u128, p: Pt2) -> Option<String> {
    let q = Q::new(m);
    let cell = q.get::<CellOf>((id::<Page>(page), p));
    (*cell).as_ref().map(ToString::to_string)
}

#[test]
fn doc06_5_1_l1_anchor_is_h16() {
    let mut fx = golden();
    qet_page(&mut fx);
    let m = fx.build();
    // The L-1 anchor at (1010 px, 620 px): grid index (101, 62), stored as Grid::at(101) / Grid::at(62) nm.
    let p = qet_at(101, 62);
    let q = Q::new(&m);
    let cell = q.get::<CellOf>((id(PAGE), p));
    let cell = (*cell).as_ref().expect("inside the drawing area");
    assert_eq!(cell.to_string(), "H16");
    assert_eq!((cell.column, cell.row), (16, 7));
    assert_eq!((cell.column_label.as_str(), cell.row_label.as_str()), ("16", "H"));
    // Doc 03 §6 on the QET frame: column = floor((kx - 2) / 6), row = "ABCDEFGH"[floor((ky - 2) / 8)].
    for kx in 2..=103 {
        for ky in [2, 9, 10, 17, 18, 34, 58, 65] {
            let want_col = (kx - 2) / 6;
            let want_row = ["A", "B", "C", "D", "E", "F", "G", "H"][((ky - 2) / 8) as usize];
            assert_eq!(
                cell_str(&m, PAGE, qet_at(kx, ky)),
                Some(format!("{want_row}{want_col}")),
                "k = ({kx}, {ky})"
            );
        }
    }
}

#[test]
fn boundary_belongs_to_the_right_or_lower_cell() {
    let mut fx = golden();
    qet_page(&mut fx);
    let m = fx.build();
    // Column 1 starts at k = 8 (60 px), row B at k_y = 10 (80 px below the 20 px band).
    assert_eq!(cell_str(&m, PAGE, qet_at(7, 5)).as_deref(), Some("A0"));
    assert_eq!(cell_str(&m, PAGE, qet_at(8, 5)).as_deref(), Some("A1"));
    assert_eq!(cell_str(&m, PAGE, qet_at(8, 9)).as_deref(), Some("A1"));
    assert_eq!(cell_str(&m, PAGE, qet_at(8, 10)).as_deref(), Some("B1"));
    // The stored boundary coordinate is rounded to the nm and may lie left of the exact boundary: the
    // point 1 nm left of the stored boundary snaps to the same grid index and stays in the right cell.
    let exact = qet_at(8, 10);
    assert_eq!(
        cell_str(&m, PAGE, pt(exact.x.0 - 1, exact.y.0 - 1)).as_deref(),
        Some("B1")
    );
}

#[test]
fn rounding_left_of_the_boundary_stays_in_the_right_cell() {
    // Step 5/2 nm on both the page grid and the frame: boundary 1 is at 2.5 nm, whose stored nm value
    // rounds (ties toward zero) to 2 nm, 0.5 nm left of the boundary. A plain floor on the stored value
    // would answer column 0.
    let mut fx = Fx::new();
    fx.lib_item(
        "form",
        0x600,
        json!({
            "kind": "frame",
            "sheet": {"width": 297_000_000, "height": 210_000_000},
            "grid": {
                "columns": {"count": 4, "width": {"num": 5, "den": 2}},
                "rows": {"count": 3, "height": {"num": 5, "den": 2}},
            },
        }),
    );
    fx.page(PAGE, 101, None, "multi_line", &[], "Power");
    fx.record(PAGE)["form"] = json!(uid(0x600));
    fx.record(PAGE)["grid"] = json!({"num": 5, "den": 2});
    let m = fx.build();
    let g = Grid::new(5, 2).unwrap();
    assert_eq!(g.at(1), Some(Nm(2)));
    assert_eq!(
        g.cell(Nm(2), Nm(0)),
        0,
        "the plain floor on the stored value is the pitfall"
    );
    assert_eq!(cell_str(&m, PAGE, pt(2, 2)).as_deref(), Some("B1"));
    assert_eq!(cell_str(&m, PAGE, pt(2, 0)).as_deref(), Some("A1"));
    assert_eq!(cell_str(&m, PAGE, pt(0, 0)).as_deref(), Some("A0"));
    // Right of the last column (k = 4 starts column 4) and below the last row.
    assert_eq!(cell_str(&m, PAGE, pt(10, 0)), None);
    assert_eq!(cell_str(&m, PAGE, pt(0, 8)), None);
    assert_eq!(cell_str(&m, PAGE, pt(8, 5)).as_deref(), Some("C3"));
}

#[test]
fn outside_the_drawing_area_and_without_a_frame() {
    let mut fx = golden();
    qet_page(&mut fx);
    fx.page(0x102, 102, None, "multi_line", &[], "No frame");
    let m = fx.build();
    // Left of column 0 (k_x = 1 is inside the 20 px header band), above row A.
    assert_eq!(cell_str(&m, PAGE, qet_at(1, 10)), None);
    assert_eq!(cell_str(&m, PAGE, qet_at(10, 1)), None);
    assert_eq!(cell_str(&m, PAGE, qet_at(-5, 10)), None);
    // Row H ends at k_y = 66: k_y = 65 is still H, 66 is below it. Column 16 ends at k_x = 104.
    assert_eq!(cell_str(&m, PAGE, qet_at(10, 65)).as_deref(), Some("H1"));
    assert_eq!(cell_str(&m, PAGE, qet_at(10, 66)), None);
    assert_eq!(cell_str(&m, PAGE, qet_at(103, 10)).as_deref(), Some("B16"));
    assert_eq!(cell_str(&m, PAGE, qet_at(104, 10)), None);
    // Extreme coordinates neither panic nor answer.
    assert_eq!(cell_str(&m, PAGE, pt(i64::MIN, i64::MAX)), None);
    assert_eq!(cell_str(&m, PAGE, pt(i64::MAX, i64::MIN)), None);
    // A page without a frame, and a page that does not exist.
    assert_eq!(cell_str(&m, 0x102, qet_at(10, 10)), None);
    assert_eq!(cell_str(&m, 0x999, qet_at(10, 10)), None);
}

#[test]
fn frame_defaults_from_the_project_settings() {
    let mut fx = golden();
    // A frame form without a `grid` member (a cover, say): its pages have no cells.
    fx.lib_item(
        "form",
        0x600,
        json!({"kind": "frame", "sheet": {"width": 297_000_000, "height": 210_000_000}}),
    );
    let nogrid_id = uid(0x600);
    fx.project_set(
        "settings",
        json!({"frames": {
            "default": FRAME_COPY,
            "per_page_type": [{"page_type": "fluid", "form": nogrid_id}],
        }}),
    );
    fx.page(0x101, 101, None, "multi_line", &[], "default frame");
    fx.page(0x102, 102, None, "fluid", &[], "per page type");
    fx.page(0x103, 103, None, "multi_line", &[], "page override");
    fx.record(0x103)["form"] = json!(nogrid_id);
    for n in [0x101, 0x102, 0x103] {
        fx.record(n)["grid"] = json!({"num": 7_937_500, "den": 3});
    }
    let m = fx.build();
    let p = qet_at(101, 62);
    assert_eq!(cell_str(&m, 0x101, p).as_deref(), Some("H16"));
    assert_eq!(
        cell_str(&m, 0x102, p),
        None,
        "the fluid page uses the frame form without a grid"
    );
    assert_eq!(
        cell_str(&m, 0x103, p),
        None,
        "Page.form wins over the project settings"
    );
}

#[test]
fn malformed_frame_settings_give_no_frame() {
    for frames in [
        json!("x"),
        json!([1]),
        json!({"default": 5}),
        json!({"default": "not-a-uuid"}),
        json!({"per_page_type": 3, "default": FRAME_COPY}),
        json!({"per_page_type": [{"page_type": "multi_line", "form": 7}], "default": FRAME_COPY}),
    ] {
        let mut fx = golden();
        fx.project_set("settings", json!({"frames": frames}));
        fx.page(PAGE, 101, None, "multi_line", &[], "P");
        fx.record(PAGE)["grid"] = json!({"num": 7_937_500, "den": 3});
        let m = fx.build();
        assert_eq!(cell_str(&m, PAGE, qet_at(101, 62)), None, "{frames}");
    }
    // An entry of another page type or a malformed entry of no interest does not hide the default.
    let mut fx = golden();
    fx.project_set(
        "settings",
        json!({"frames": {"default": FRAME_COPY, "per_page_type": [7, {"page_type": "fluid", "form": 1}]}}),
    );
    fx.page(PAGE, 101, None, "multi_line", &[], "P");
    fx.record(PAGE)["grid"] = json!({"num": 7_937_500, "den": 3});
    let m = fx.build();
    assert_eq!(cell_str(&m, PAGE, qet_at(101, 62)).as_deref(), Some("H16"));
}

#[test]
fn explicit_labels_and_start_index() {
    let mut fx = Fx::new();
    fx.lib_item(
        "form",
        0x600,
        json!({
            "kind": "frame",
            "sheet": {"width": 297_000_000, "height": 210_000_000},
            "grid": {
                "header_band": {"num": 10_000_000, "den": 1},
                "columns": {"count": 3, "start_index": 1, "width": {"num": 20_000_000, "den": 1}, "labels": ["I", "II"]},
                "rows": {"count": 4, "height": {"num": 20_000_000, "den": 1}, "skip_io": true},
            },
        }),
    );
    fx.page(PAGE, 101, None, "multi_line", &[], "P");
    fx.record(PAGE)["form"] = json!(uid(0x600));
    fx.record(PAGE)["grid"] = json!({"num": 1_000_000, "den": 1});
    // Rows without labels: the letters of A..Z skipping I and O.
    fx.lib_item(
        "form",
        0x601,
        json!({
            "kind": "frame",
            "sheet": {"width": 297_000_000, "height": 210_000_000},
            "grid": {
                "columns": {"count": 2, "start_index": 5, "width": {"num": 1_000_000, "den": 1}},
                "rows": {"count": 26, "height": {"num": 1_000_000, "den": 1}, "skip_io": true},
            },
        }),
    );
    fx.page(0x102, 102, None, "multi_line", &[], "Q");
    fx.record(0x102)["form"] = json!(uid(0x601));
    fx.record(0x102)["grid"] = json!({"num": 1_000_000, "den": 1});
    fx.lib_item(
        "form",
        0x602,
        json!({
            "kind": "frame",
            "sheet": {"width": 297_000_000, "height": 210_000_000},
            "grid": {
                "columns": {"count": 1, "width": {"num": 1_000_000, "den": 1}},
                "rows": {"count": 26, "height": {"num": 1_000_000, "den": 1}},
            },
        }),
    );
    fx.page(0x103, 103, None, "multi_line", &[], "R");
    fx.record(0x103)["form"] = json!(uid(0x602));
    fx.record(0x103)["grid"] = json!({"num": 1_000_000, "den": 1});
    let m = fx.build();
    let mm = |x: i64, y: i64| pt(x * 1_000_000, y * 1_000_000);
    // Page 101: band 10 mm, 20 mm cells; two labels for three columns, so column index 2 has none.
    assert_eq!(cell_str(&m, PAGE, mm(11, 11)).as_deref(), Some("AI"));
    assert_eq!(cell_str(&m, PAGE, mm(31, 31)).as_deref(), Some("BII"));
    assert_eq!(cell_str(&m, PAGE, mm(51, 11)), None, "missing label");
    assert_eq!(cell_str(&m, PAGE, mm(11, 71)).as_deref(), Some("DI"));
    assert_eq!(cell_str(&m, PAGE, mm(11, 91)), None, "below the last row");
    // Page 102: start index 5; 24 letters with I and O skipped, so row 24 is beyond Z.
    assert_eq!(cell_str(&m, 0x102, mm(1, 0)).as_deref(), Some("A6"));
    assert_eq!(
        cell_str(&m, 0x102, mm(0, 8)).as_deref(),
        Some("J5"),
        "A..H, then J (I skipped)"
    );
    assert_eq!(
        cell_str(&m, 0x102, mm(0, 13)).as_deref(),
        Some("P5"),
        "N, then P (O skipped)"
    );
    assert_eq!(cell_str(&m, 0x102, mm(0, 23)).as_deref(), Some("Z5"));
    assert_eq!(cell_str(&m, 0x102, mm(0, 24)), None);
    // Page 103: all 26 letters.
    assert_eq!(cell_str(&m, 0x103, mm(0, 8)).as_deref(), Some("I0"));
    assert_eq!(cell_str(&m, 0x103, mm(0, 25)).as_deref(), Some("Z0"));
    assert_eq!(cell_str(&m, 0x103, mm(0, 26)), None);
}

// ---------------------------------------------------------------------------------------------------------
// Property test against an independent exact reference

/// Parameters of one random frame and page grid.
#[derive(Clone, Debug)]
struct Params {
    page_grid: (u64, u64),
    header: Option<(u64, u64)>,
    columns: (u16, u16, (u64, u64)),
    rows: (u16, bool, (u64, u64)),
}

fn step() -> impl Strategy<Value = (u64, u64)> {
    (1u64..=1000, 0u64..=1_000_000).prop_map(|(den, extra)| (den + extra, den))
}

fn params() -> impl Strategy<Value = Params> {
    (
        step(),
        proptest::option::of(step()),
        (1u16..=30, 0u16..=5, step()),
        (1u16..=30, any::<bool>(), step()),
    )
        .prop_map(|(page_grid, header, columns, rows)| Params {
            page_grid,
            header,
            columns,
            rows: (rows.0, rows.1, rows.2),
        })
}

fn coordinate() -> impl Strategy<Value = i64> {
    prop_oneof![
        any::<i64>(),
        -(1i64 << 27)..(1i64 << 27),
        -(1i64 << 12)..(1i64 << 12)
    ]
}

fn model_of(p: &Params) -> Model {
    let grid = |(n, d): (u64, u64)| {
        let g = Grid::new(n, d).expect("valid step");
        json!({"num": g.num(), "den": g.den()})
    };
    let mut frame = json!({
        "columns": {"count": p.columns.0, "width": grid(p.columns.2)},
        "rows": {"count": p.rows.0, "height": grid(p.rows.2)},
    });
    if p.columns.1 != 0 {
        frame["columns"]["start_index"] = json!(p.columns.1);
    }
    if p.rows.1 {
        frame["rows"]["skip_io"] = json!(true);
    }
    if let Some(h) = p.header {
        frame["header_band"] = grid(h);
    }
    let mut fx = Fx::new();
    fx.lib_item(
        "form",
        0x600,
        json!({"kind": "frame", "sheet": {"width": 297_000_000, "height": 210_000_000}, "grid": frame}),
    );
    fx.page(PAGE, 101, None, "multi_line", &[], "P");
    fx.record(PAGE)["form"] = json!(uid(0x600));
    fx.record(PAGE)["grid"] = grid(p.page_grid);
    fx.build()
}

/// `round(x · den / num)` with ties toward zero, by remainder inspection.
fn ref_snap(x: i64, num: i128, den: i128) -> i128 {
    let t = i128::from(x) * den;
    let (q, r) = (t.div_euclid(num), t.rem_euclid(num));
    match (2 * r).cmp(&num) {
        std::cmp::Ordering::Less => q,
        std::cmp::Ordering::Greater => q + 1,
        std::cmp::Ordering::Equal => {
            if t > 0 {
                q
            } else {
                q + 1
            }
        }
    }
}

/// The cell index of the exact position `k·gn/gd` along one axis, found by scanning the cells
/// `[a/b + c·sn/sd, a/b + (c+1)·sn/sd)` with cross-multiplied comparisons.
fn ref_axis(
    k: i128,
    (gn, gd): (i128, i128),
    (a, b): (i128, i128),
    (sn, sd): (i128, i128),
    count: u16,
) -> Option<u32> {
    // x < y  <=>  k·gn / gd < (a·sd + c·sn·b) / (b·sd)
    let below = |c: i128| k * gn * (b * sd) < (a * sd + c * sn * b) * gd;
    (0..i128::from(count))
        .find(|&c| !below(c) && below(c + 1))
        .map(|c| c as u32)
}

fn ref_cell(p: &Params, x: i64, y: i64) -> Option<String> {
    let g = (
        i128::from(Grid::new(p.page_grid.0, p.page_grid.1).unwrap().num()),
        { i128::from(Grid::new(p.page_grid.0, p.page_grid.1).unwrap().den()) },
    );
    let rat = |(n, d): (u64, u64)| {
        let g = Grid::new(n, d).unwrap();
        (i128::from(g.num()), i128::from(g.den()))
    };
    let (kx, ky) = (ref_snap(x, g.0, g.1), ref_snap(y, g.0, g.1));
    // The snapped index is the nearest one.
    assert!(2 * (i128::from(x) * g.1 - kx * g.0).abs() <= g.0);
    let origin = p.header.map_or((0, 1), rat);
    let c = ref_axis(kx, g, origin, rat(p.columns.2), p.columns.0)?;
    let r = ref_axis(ky, g, origin, rat(p.rows.2), p.rows.0)?;
    let letters: Vec<char> = ('A'..='Z')
        .filter(|c| !(p.rows.1 && matches!(c, 'I' | 'O')))
        .collect();
    let row = letters.get(r as usize)?;
    Some(format!("{row}{}", u32::from(p.columns.1) + c))
}

proptest! {
    #[test]
    fn cell_of_matches_the_exact_reference(
        p in params(),
        pts in proptest::collection::vec((coordinate(), coordinate()), 1..12),
    ) {
        let m = model_of(&p);
        let q = Q::new(&m);
        for (x, y) in pts {
            let got = q.get::<CellOf>((id(PAGE), pt(x, y)));
            let got = (*got).as_ref().map(ToString::to_string);
            prop_assert_eq!(got, ref_cell(&p, x, y), "point ({}, {})", x, y);
        }
    }

    #[test]
    fn cell_of_never_panics_on_extreme_grids(
        page_grid in (1u64..=(1 << 53) - 1, 1u64..=(1 << 53) - 1),
        header in proptest::option::of((1u64..=(1 << 53) - 1, 1u64..=(1 << 53) - 1)),
        col in (1u64..=(1 << 53) - 1, 1u64..=(1 << 53) - 1),
        row in (1u64..=(1 << 53) - 1, 1u64..=(1 << 53) - 1),
        x in any::<i64>(),
        y in any::<i64>(),
    ) {
        // `Grid::new` rejects steps below 1 nm: order each pair as num >= den.
        let order = |(a, b): (u64, u64)| (a.max(b), a.min(b));
        let p = Params {
            page_grid: order(page_grid),
            header: header.map(order),
            columns: (17, 0, order(col)),
            rows: (8, false, order(row)),
        };
        let m = model_of(&p);
        let q = Q::new(&m);
        let _ = q.get::<CellOf>((id(PAGE), pt(x, y)));
        let _ = q.get::<CellOf>((id(PAGE), pt(0, 0)));
    }
}

// ---------------------------------------------------------------------------------------------------------
// Anchor cells (doc 20 §6.1 "Main rep." and §15)

/// What a symbol's anchor policy and geometry are, in nm and symbol coordinates.
#[derive(Clone, Debug)]
struct Sym {
    anchor: Value,
    graphics: Vec<Value>,
    slots: Vec<Value>,
}

impl Sym {
    fn origin() -> Sym {
        Sym::with(json!("origin"))
    }

    fn with(anchor: Value) -> Sym {
        Sym {
            anchor,
            graphics: Vec::new(),
            slots: Vec::new(),
        }
    }
}

const MM: i64 = 1_000_000;

/// A row of the anchor table: device, symbol, orientation, rep position as a grid index, expected cell.
type AnchorRow = (&'static str, Sym, &'static str, (i64, i64), &'static str);

/// A model with one function drawn by one representation of a symbol with `sym`, rotated or mirrored by
/// `orient` at grid index `(kx, ky)` of the QET grid on the golden frame page.
fn anchor_model(sym: &Sym, orient: &str, k: (i64, i64), overrides: Value) -> Model {
    let mut fx = golden();
    qet_page(&mut fx);
    fx.fndef(FNDEF, ["ProtectionOvercurrent", "MCB", "1P"], &[(0, "1")]);
    fx.symbol(SYM, FNDEF, sym.anchor.clone(), &[(0, 0, 0, 0)]);
    let doc = fx.lib.last_mut().expect("the symbol");
    if !sym.graphics.is_empty() {
        doc["graphics"] = json!(sym.graphics);
    }
    if !sym.slots.is_empty() {
        doc["text_slots"] = json!(sym.slots);
    }
    fx.device(DEVICE, "F", Some(1))
        .function(FUNCTION, Some(DEVICE), FNDEF, true);
    let at = qet_at(k.0, k.1);
    fx.rep(0x2000, PAGE, SYM, &[FUNCTION], at.x.0, at.y.0);
    fx.record(0x2000)["orient"] = json!(orient);
    if !overrides.is_null() {
        fx.record(0x2000)["text_slots"] = overrides;
    }
    fx.build()
}

fn anchor_cell(m: &Model) -> Option<String> {
    let q = Q::new(m);
    let cell = q.get::<RepAnchorCell>(id::<Representation>(0x2000));
    (*cell).as_ref().map(ToString::to_string)
}

fn slot(kind: &str, x: i64, y: i64) -> Value {
    json!({"kind": kind, "pos": [x, y], "align": "centre"})
}

fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Value {
    json!({"rect": {"rect": [[x0, y0], [x1, y1]]}})
}

#[test]
fn doc20_anchor_cells() {
    // (device of doc 20, symbol, orientation, rep position as QET grid index, expected cell).
    // Cell (column c, row r) of the QET frame spans k_x in 2+6c ..= 7+6c and k_y in 2+8r ..= 9+8r
    // (centre 5+6c, 6+8r); 1 grid step is 2.6458 mm.
    let named = |x: i64, y: i64| Sym::with(json!({"named": [x, y]}));
    let mut slot_xref = Sym::with(json!({"slot": "xref"}));
    slot_xref.slots = vec![slot("tag", 0, 0), slot("xref", 8 * MM, -8 * MM)];
    let mut slot_tag = Sym::with(json!({"slot": "tag"}));
    slot_tag.slots = vec![slot("tag", 0, 0)];
    let mut centre_box = Sym::with(json!("graphic_centre"));
    centre_box.graphics = vec![rect(0, 0, 16 * MM, 32 * MM)];
    let mut centre_ellipse = Sym::with(json!("graphic_centre"));
    centre_ellipse.graphics = vec![
        json!({"circle": {"centre": [0, 0], "r": 8 * MM}}),
        json!({"ellipse": {"rect": [[0, -8 * MM], [32 * MM, 8 * MM]]}}),
    ];
    let centre_none = Sym::with(json!("graphic_centre"));

    let rows: Vec<AnchorRow> = vec![
        // Origin anchors.
        ("101-B8 -101F1", Sym::origin(), "r0", (53, 14), "B8"),
        ("101-H16 L-1 (mirrored)", Sym::origin(), "m0", (101, 62), "H16"),
        ("106-F5 -106W1 (rotated)", Sym::origin(), "r270", (35, 46), "F5"),
        ("3-B2 -3X1", Sym::origin(), "r0", (17, 14), "B2"),
        ("101-D5 -XPE (rotated)", Sym::origin(), "r90", (35, 30), "D5"),
        // Named point (0, 16 mm), rotated a quarter turn clockwise: (-16 mm, 0), about 6 steps left.
        ("101-G15 PE-1.1", named(0, 16 * MM), "r90", (101, 54), "G15"),
        // Named point (8 mm, 0), mirrored and turned: (0, 8 mm), about 3 steps down.
        ("103-A3", named(8 * MM, 0), "m270", (23, 5), "A3"),
        // Named point (16 mm, 8 mm) under M90 -> (-8 mm, -16 mm).
        ("201-E15 -201U9", named(16 * MM, 8 * MM), "m90", (97, 44), "E15"),
        // Text-slot anchor: the xref slot at (8 mm, -8 mm).
        ("102-A1", slot_xref.clone(), "r0", (5, 6), "A1"),
        // Graphic centre of a 16 x 32 mm rectangle: (8 mm, 16 mm), turned half a turn.
        ("103-F3 -X1032", centre_box, "r180", (25, 50), "F3"),
        // Graphic centre of a circle and an ellipse: x in -8..32 mm, y in -8..8 mm: (12 mm, 0), M180.
        ("201-E4 -201U3", centre_ellipse, "m180", (23, 38), "E4"),
        // A slot anchor without a matching slot and a graphic centre without graphics: the origin.
        ("201-E1 -201U1", centre_none, "r0", (11, 38), "E1"),
        ("201-E2 -201U2", slot_tag.clone(), "r0", (17, 38), "E2"),
    ];
    assert!(rows.len() >= 10);
    for (device, sym, orient, k, want) in rows {
        let m = anchor_model(&sym, orient, k, Value::Null);
        assert_eq!(anchor_cell(&m).as_deref(), Some(want), "{device}");
    }

    // A slot anchor naming a kind the symbol has no slot of falls back to the origin.
    let mut missing = Sym::with(json!({"slot": "plc_address"}));
    missing.slots = vec![slot("tag", 16 * MM, 16 * MM)];
    assert_eq!(
        anchor_cell(&anchor_model(&missing, "r0", (11, 38), Value::Null)).as_deref(),
        Some("E1")
    );
}

#[test]
fn doc20_106_g3_slot_override_moves_the_anchor() {
    // -201U1 power rep at 106-G3: the symbol's tag slot is at the origin; the rep drags it 16 mm to the
    // right (symbol coordinates, before the orientation), which carries the anchor one column on.
    let mut sym = Sym::with(json!({"slot": "tag"}));
    sym.slots = vec![slot("tag", 0, 0)];
    let plain = anchor_model(&sym, "r0", (17, 54), Value::Null);
    assert_eq!(anchor_cell(&plain).as_deref(), Some("G2"));
    let moved = anchor_model(&sym, "r0", (17, 54), json!([{"slot": 0, "pos": [16 * MM, 0]}]));
    assert_eq!(anchor_cell(&moved).as_deref(), Some("G3"));
    // The override is a symbol-frame point: under R90 the 16 mm to the right turn into 16 mm downwards.
    let turned = anchor_model(&sym, "r90", (23, 54), json!([{"slot": 0, "pos": [16 * MM, 0]}]));
    assert_eq!(anchor_cell(&turned).as_deref(), Some("H3"));
    // An override of another slot or without a position changes nothing.
    let other = anchor_model(&sym, "r0", (17, 54), json!([{"slot": 1, "pos": [16 * MM, 0]}]));
    assert_eq!(anchor_cell(&other).as_deref(), Some("G2"));
    let hidden = anchor_model(&sym, "r0", (17, 54), json!([{"slot": 0, "visible": false}]));
    assert_eq!(anchor_cell(&hidden).as_deref(), Some("G2"));
}

#[test]
fn anchor_outside_the_drawing_area_has_no_cell() {
    // The origin anchor on the frame's header band and a named anchor pushed below the last row.
    let m = anchor_model(&Sym::origin(), "r0", (1, 1), Value::Null);
    assert_eq!(anchor_cell(&m), None);
    let far = Sym::with(json!({"named": [0, 400 * MM]}));
    assert_eq!(
        anchor_cell(&anchor_model(&far, "r0", (11, 38), Value::Null)),
        None
    );
    // The largest anchor a record can hold is far outside the frame: no cell, no panic.
    let huge = Sym::with(json!({"named": [9_007_199_254_740_991_i64, -9_007_199_254_740_991_i64]}));
    assert_eq!(
        anchor_cell(&anchor_model(&huge, "m90", (11, 38), Value::Null)),
        None
    );
}

#[test]
fn missing_rep_or_symbol_copy_has_no_anchor_cell() {
    let m = anchor_model(&Sym::origin(), "r0", (11, 38), Value::Null);
    let q = Q::new(&m);
    assert_eq!(*q.get::<RepAnchorCell>(id::<Representation>(0x9999)), None);
    assert_eq!(anchor_cell(&m).as_deref(), Some("E1"));

    // A representation whose symbol copy is missing cannot be built: the model refuses the dangling
    // `Representation.symbol` reference, so that branch of `anchor_point` is a defensive `None` only.
    let mut fx = golden();
    qet_page(&mut fx);
    fx.fndef(FNDEF, ["ProtectionOvercurrent", "MCB", "1P"], &[(0, "1")]);
    fx.device(DEVICE, "F", Some(1))
        .function(FUNCTION, Some(DEVICE), FNDEF, true);
    fx.rep(0x2000, PAGE, 0x777, &[FUNCTION], 40 * MM, 40 * MM);
    assert!(std::panic::catch_unwind(|| fx.build()).is_err());
}
