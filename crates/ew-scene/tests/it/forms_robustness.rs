// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Property tests of the exact integer arithmetic of the forms engine and a never-panics test of the
//! placeholder template parser.

use ew_scene::forms::{BoundValue, PPB_ONE, PageBindings, SheetMap, eval_visible_if, fill};
use ew_text::units_to_nm;
use ew_units::{Nm, Pt2};
use proptest::prelude::*;

/// `n / d` (d > 0) rounded to the nearest integer, exact ties toward zero, by a different route than the
/// production code: round half down on the magnitude, sign applied last.
fn round_ties_toward_zero(n: i128, d: i128) -> i128 {
    let m = n.abs();
    // Round half down on the magnitude: floor((2m + d − 1) / 2d).
    let q = (2 * m + d - 1) / (2 * d);
    if n < 0 { -q } else { q }
}

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config())]

    /// `units_to_nm` of a value `m / 2^k` (exactly representable in `f32`) equals the exact rational
    /// `m × size / (upem × 2^k)` rounded to the nearest nm, ties toward zero.
    #[test]
    fn units_to_nm_is_the_exact_rational(
        m in -(1i32 << 24)..=(1i32 << 24),
        k in 0u32..=12,
        size in 0i64..=2_000_000_000,
        upem in 1u16..=u16::MAX,
    ) {
        let v = m as f32 / (1u64 << k) as f32; // exact: a power-of-two quotient of a 24-bit integer
        let want = round_ties_toward_zero(
            i128::from(m) * i128::from(size),
            i128::from(upem) << k,
        );
        prop_assert_eq!(i128::from(units_to_nm(v, size, upem)), want);
    }

    /// The result is odd-symmetric: `units_to_nm(-v) == -units_to_nm(v)`.
    #[test]
    fn units_to_nm_is_odd_symmetric(
        m in 0i32..=(1 << 24),
        k in 0u32..=12,
        size in 0i64..=2_000_000_000,
        upem in 1u16..=u16::MAX,
    ) {
        let v = m as f32 / (1u64 << k) as f32;
        prop_assert_eq!(units_to_nm(-v, size, upem), -units_to_nm(v, size, upem));
    }

    /// `SheetMap::scale_len(n) = round(n × scale / 10⁹)`, ties toward zero, exactly.
    #[test]
    fn scale_len_is_the_exact_rational(
        n in -1_000_000_000_000_000i64..=1_000_000_000_000_000,
        scale in 0u32..=2_000_000_000,
    ) {
        let map = SheetMap { scale_ppb: scale, offset: Pt2::ORIGIN };
        let want = round_ties_toward_zero(i128::from(n) * i128::from(scale), i128::from(PPB_ONE));
        prop_assert_eq!(i128::from(map.scale_len(Nm(n)).0), want);
        prop_assert_eq!(map.scale_len(Nm(-n)).0, -map.scale_len(Nm(n)).0);
    }

    /// `apply` is `offset + scale_len` per coordinate.
    #[test]
    fn apply_is_offset_plus_scaled(
        x in -1_000_000_000_000i64..=1_000_000_000_000,
        y in -1_000_000_000_000i64..=1_000_000_000_000,
        ox in -1_000_000_000i64..=1_000_000_000,
        oy in -1_000_000_000i64..=1_000_000_000,
        scale in 0u32..=2_000_000_000,
    ) {
        let map = SheetMap { scale_ppb: scale, offset: Pt2::new(Nm(ox), Nm(oy)) };
        let p = map.apply(Pt2::new(Nm(x), Nm(y)));
        prop_assert_eq!(p.x.0, ox + map.scale_len(Nm(x)).0);
        prop_assert_eq!(p.y.0, oy + map.scale_len(Nm(y)).0);
    }

    /// Filling a template and evaluating a `visible_if` expression never panic, whatever the text.
    #[test]
    fn template_parser_never_panics_on_arbitrary_text(s in template_text()) {
        let mut b = PageBindings::new();
        b.text("page.title", "T").unwrap();
        b.text("project.name", "").unwrap();
        b.set("page.n", BoundValue::Text("7".to_owned())).unwrap();
        let _ = fill(&s, &b);
        let _ = fill(&s, &PageBindings::new());
        let _ = eval_visible_if(&s, &b);
    }
}

/// Strings built from the template grammar's delimiters, path pieces and arbitrary characters.
fn template_text() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        3 => proptest::sample::select(vec![
            "{", "}", "{{", "}}", "|", "(", ")", "\"", ",", ".", "=", "!", " ", "page", "project", "ew",
            "title", "date", "upper", "default", "tag", "full", "x_y-z", "{page.title}", "{page.title | date}",
            "{ew.version | default(\"?\")}", "{=1+2}", "{unknown.scope}", "\\", "'", "\n", "é", "日",
        ]).prop_map(str::to_owned),
        1 => any::<char>().prop_map(String::from),
    ];
    proptest::collection::vec(piece, 0..40).prop_map(|v| v.concat())
}
