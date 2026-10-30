// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(missing_docs)]

use ew_plcaddr::*;
use proptest::prelude::*;

fn p() -> &'static AddressProfile {
    &SIEMENS_TIA_INTL
}

#[test]
fn render() {
    let q = PlcAddress::bit(Area::Q, 0, 1);
    assert_eq!(p().render(&q, Purpose::Drawing), "Q0.1");
    assert_eq!(p().render(&q, Purpose::Exchange), "%Q0.1");
    let iw = PlcAddress::unit(Area::I, Size::Word, 64);
    assert_eq!(p().render(&iw, Purpose::Drawing), "IW64");
    let qb = PlcAddress::unit(Area::Q, Size::Byte, 6);
    assert_eq!(p().render(&qb, Purpose::Drawing), "QB6");
    let pi = PlcAddress::bit(Area::Pi, 3, 2);
    assert_eq!(p().render(&pi, Purpose::Drawing), "PI3.2");
    let pq = PlcAddress::unit(Area::Pq, Size::LWord, 8);
    assert_eq!(p().render(&pq, Purpose::Exchange), "%PQL8");
}

#[test]
fn bits_and_units() {
    assert_eq!(p().bit_at(Area::Q, 6, 7), Some(PlcAddress::bit(Area::Q, 6, 7)));
    assert_eq!(p().bit_at(Area::Q, 0, 9), Some(PlcAddress::bit(Area::Q, 1, 1)));
    // Hostile input: a start byte at the end of the range and a large offset overflow instead of wrapping.
    assert_eq!(
        p().bit_at(Area::Q, u32::MAX, 7),
        Some(PlcAddress::bit(Area::Q, u32::MAX, 7))
    );
    assert_eq!(p().bit_at(Area::Q, u32::MAX, 8), None);
    assert_eq!(p().bit_at(Area::Q, u32::MAX - 1, 16), None);
    assert_eq!((p().units(8), p().units(9), p().units(0)), (1, 2, 0));
    assert_eq!(Size::LWord.bits(), 64);
}

#[test]
fn builtins_lookup() {
    let b = builtin("siemens-tia-intl@2027.1").unwrap();
    assert_eq!(b.reference(), "siemens-tia-intl@2027.1");
    assert!(builtin("siemens-tia-intl").is_none());
    assert_eq!(builtins().len(), 1);
}

#[test]
fn intervals() {
    let mut i = AreaIntervals::new();
    i.insert(Area::Q, 0, 2);
    assert_eq!(i.first_free(Area::Q, 0, 1), 2);
    assert_eq!(i.first_free(Area::I, 0, 1), 0);
    i.insert(Area::Q, 4, 1);
    assert_eq!(i.first_free(Area::Q, 2, 3), 5);
    assert_eq!(i.first_free(Area::Q, 2, 2), 2);
    assert!(i.overlaps(Area::Q, 1, 1));
    assert!(i.overlaps(Area::Q, 3, 2));
    assert!(!i.overlaps(Area::Q, 2, 2));
    assert!(!i.overlaps(Area::I, 0, 10));
    i.insert(Area::Q, 2, 2);
    assert_eq!(i.first_free(Area::Q, 0, 1), 5);
    i.insert(Area::Q, 9, 0);
    assert!(!i.overlaps(Area::Q, 9, 1));
    i.insert(Area::M, u32::MAX - 2, 5);
    assert!(i.overlaps(Area::M, u32::MAX - 1, 1));
}

#[test]
fn map_parse_eval() {
    assert_eq!(MapExpr::parse("n").unwrap().eval(5), Some(5));
    assert_eq!(MapExpr::parse("16n").unwrap().eval(2), Some(32));
    assert_eq!(MapExpr::parse("2p").unwrap().eval(3), Some(6));
    assert_eq!(MapExpr::parse("2p+1").unwrap().eval(3), Some(7));
    assert_eq!(MapExpr::parse("2n").unwrap().eval(u32::MAX), None);
    for bad in [
        "", "n+", "2q", "+1", "p+1+1", "02n", "0n", "n+01", "2 n", "n +1", "-n", "np",
    ] {
        assert!(MapExpr::parse(bad).is_none(), "{bad}");
    }
    assert_eq!(MapExpr::parse("2p+1").unwrap().to_string(), "2p+1");
    assert_eq!(MapExpr::parse("n+0").unwrap().to_string(), "n");
}

proptest! {
    #[test]
    fn map_round_trip(p in any::<bool>(), k in 1u32.., c in any::<u32>()) {
        let e = MapExpr { var: if p { MapVar::P } else { MapVar::N }, k, c };
        prop_assert_eq!(MapExpr::parse(&e.to_string()), Some(e));
    }
}

/// One operation on the intervals: insert `(area, start, len)`.
fn op() -> impl Strategy<Value = (bool, u32, u32)> {
    (any::<bool>(), 0u32..48, 0u32..10)
}

proptest! {
    /// `AreaIntervals` against a brute-force bitset per area: overlaps and first_free agree after any inserts.
    #[test]
    fn intervals_agree_with_a_bitset(
        ops in proptest::collection::vec(op(), 0..12),
        queries in proptest::collection::vec((any::<bool>(), 0u32..60, 1u32..10), 1..12),
    ) {
        const N: usize = 256;
        let mut iv = AreaIntervals::new();
        let mut bits = [[false; N]; 2];
        let area = |q: bool| if q { Area::Q } else { Area::I };
        for (q, start, len) in ops {
            iv.insert(area(q), start, len);
            for b in start..start + len {
                bits[usize::from(q)][b as usize] = true;
            }
        }
        for (q, from, len) in queries {
            let set = &bits[usize::from(q)];
            let busy = |s: u32| (s..s + len).any(|b| set[b as usize]);
            prop_assert_eq!(iv.overlaps(area(q), from, len), busy(from));
            let brute = (from..).find(|s| !busy(*s)).unwrap();
            prop_assert_eq!(iv.first_free(area(q), from, len), brute);
        }
    }
}

/// WP-S1.21 D6: an address text is parsed by the profile, the inverse of `render`; the example names output 0.1.
#[test]
fn parse_and_example() {
    let p = p();
    assert_eq!(p.example(), "Q0.1");
    assert_eq!(p.parse("I4.0"), Some(PlcAddress::bit(Area::I, 4, 0)));
    assert_eq!(p.parse(" q0.7 "), Some(PlcAddress::bit(Area::Q, 0, 7)));
    assert_eq!(p.parse("%Q6.7"), Some(PlcAddress::bit(Area::Q, 6, 7)));
    assert_eq!(p.parse("PI3.2"), Some(PlcAddress::bit(Area::Pi, 3, 2)));
    assert_eq!(p.parse("IW64"), Some(PlcAddress::unit(Area::I, Size::Word, 64)));
    assert_eq!(p.parse("%PQL8"), Some(PlcAddress::unit(Area::Pq, Size::LWord, 8)));
    assert_eq!(p.parse("MB10"), Some(PlcAddress::unit(Area::M, Size::Byte, 10)));
    for bad in [
        "",
        "I4.9",
        "I4.10",
        "I4",
        "4.0",
        "I.0",
        "I4.",
        "X4.0",
        "IW4.0",
        "I4.0x",
        "%%I4.0",
        "I4294967296.0",
        "I-1.0",
        "auto",
    ] {
        assert_eq!(p.parse(bad), None, "{bad:?}");
    }
}

/// FX37a: a very long digit string is refused (the digit runs are capped at the width of a `u32`; no timing is
/// asserted, owner's rule), and the longest byte numbers still parse.
#[test]
fn a_ten_thousand_digit_address_is_refused() {
    let p = p();
    let digits = "1".repeat(10_000);
    for text in [
        format!("I{digits}.0"),
        format!("I0.{digits}"),
        format!("IW{digits}"),
        format!("I{digits}"),
    ] {
        assert_eq!(p.parse(&text), None, "{} digits", text.len());
    }
    assert_eq!(
        p.parse("I4294967295.7"),
        Some(PlcAddress::bit(Area::I, u32::MAX, 7))
    );
    assert_eq!(
        p.parse("MW4294967295"),
        Some(PlcAddress::unit(Area::M, Size::Word, u32::MAX))
    );
    assert_eq!(p.parse("I04.0"), Some(PlcAddress::bit(Area::I, 4, 0)));
}

proptest! {
    /// Every bit address and every unit address of a size above a bit parses back from either rendering.
    #[test]
    fn parse_inverts_render(
        area in 0usize..5,
        size in 0usize..5,
        byte in any::<u32>(),
        bit in 0u8..8,
        ex in any::<bool>(),
    ) {
        let areas = [Area::I, Area::Q, Area::M, Area::Pi, Area::Pq];
        let sizes = [Size::Bit, Size::Byte, Size::Word, Size::DWord, Size::LWord];
        let a = if size == 0 {
            PlcAddress::bit(areas[area], byte, bit)
        } else {
            PlcAddress::unit(areas[area], sizes[size], byte)
        };
        let purpose = if ex { Purpose::Exchange } else { Purpose::Drawing };
        prop_assert_eq!(p().parse(&p().render(&a, purpose)), Some(a));
    }
}
