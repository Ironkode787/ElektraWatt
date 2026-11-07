// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rail model and the resolver (WP-S4.02 D3 to D5) over Golden A's rail intervals (doc 20 §13; the spec's SN-1
//! to SN-6 numbers by hand) and over random rails (property tests: every mode's result holds D5's rules).

use ew_ids::{Id, Uuid};
use proptest::prelude::*;

use super::*;

const MM: i128 = 1_000_000;

fn pid(n: u128) -> Id<Placement> {
    Id::from_uuid(Uuid::from_u128(0x5000 + n))
}

/// A non-strip item in tenths of a millimetre.
fn dev(n: u128, s0: i128, s1: i128) -> RailItem {
    RailItem {
        s0: s0 * MM / 10,
        s1: s1 * MM / 10,
        strip: false,
        w: None,
        pad: [0, 0],
        locked: false,
        placement: Some(pid(n)),
    }
}

/// A strip in tenths of a millimetre with a bracket width in millimetres (`None`: it declares none).
fn strip(n: u128, s0: i128, s1: i128, w: Option<i128>) -> RailItem {
    RailItem {
        strip: true,
        w: w.map(|w| w * MM),
        ..dev(n, s0, s1)
    }
}

/// A new item (not placed yet) of `w` tenths of a millimetre.
fn new_item(w: i128) -> RailItem {
    RailItem {
        placement: None,
        ..dev(0, 0, w)
    }
}

// Golden A's R1 (570 mm): -102G1, -101F1, -102F1, -102F2, T55, S51.
const G1: u128 = 1;
const F1_101: u128 = 2;
const F1_102: u128 = 3;
const F2_102: u128 = 4;
const T55: u128 = 5;
const S51: u128 = 6;
const L1: i128 = 570 * MM;

fn r1() -> Vec<RailItem> {
    vec![
        dev(G1, 610, 1310),
        dev(F1_101, 1310, 1490),
        dev(F1_102, 1490, 1670),
        dev(F2_102, 1670, 2120),
        dev(T55, 2120, 2450),
        dev(S51, 2450, 2900),
    ]
}

// Golden A's R2 (570 mm): 1X1, -X1031, -X1032, each declaring a 10 mm end bracket.
const X1: u128 = 11;
const X1031: u128 = 12;
const X1032: u128 = 13;

fn r2() -> Vec<RailItem> {
    vec![
        strip(X1, 300, 486, Some(10)),
        strip(X1031, 586, 1626, Some(10)),
        strip(X1032, 1726, 1986, Some(10)),
    ]
}

fn mm10(v: i128) -> i128 {
    v * MM / 10
}

fn spans(items: &[RailItem]) -> Vec<(i128, i128, u128)> {
    brackets(items)
        .iter()
        .map(|b| {
            let n = items[b.item]
                .placement
                .map_or(0, |p| p.as_uuid().as_u128() - 0x5000);
            (b.s0 * 10 / MM, b.s1 * 10 / MM, n)
        })
        .collect()
}

/// SN-1 (FR-LAY-060): R2's four brackets attributed 1X1, 1X1, -X1031, -X1032; -X1032 moved to 222.6 mm (gap 60 mm ≥
/// 20 mm) gives five; a strip without an end-bracket part use gives none; zero-width brackets join touching strips only.
#[test]
fn sn1_brackets_at_the_strip_boundaries() {
    assert_eq!(
        spans(&r2()),
        vec![
            (200, 300, X1),
            (486, 586, X1),
            (1626, 1726, X1031),
            (1986, 2086, X1032)
        ]
    );
    let mut moved = r2();
    moved[2] = strip(X1032, 2226, 2486, Some(10));
    assert_eq!(
        spans(&moved),
        vec![
            (200, 300, X1),
            (486, 586, X1),
            (1626, 1726, X1031),
            (2126, 2226, X1032),
            (2486, 2586, X1032)
        ]
    );
    // A gap of 19.9 mm (< 2 w) keeps one run; 20 mm separates the runs.
    let mut near = r2();
    near[2] = strip(X1032, 1825, 2085, Some(10));
    assert_eq!(runs(&near).len(), 1);
    near[2] = strip(X1032, 1826, 2086, Some(10));
    assert_eq!(runs(&near).len(), 2);
    // -X1031 declares no bracket part: it contributes none; its 10 mm gaps are no longer below w(a) + w(b) = 10 mm, so
    // each strip is its own run and -X1032 gets its own run-start bracket.
    let mut bare = r2();
    bare[1].w = None;
    assert_eq!(runs(&bare).len(), 3);
    assert_eq!(
        spans(&bare),
        vec![
            (200, 300, X1),
            (486, 586, X1),
            (1626, 1726, X1032),
            (1986, 2086, X1032)
        ]
    );
    // Zero-width brackets: touching strips are one run, a gap of 0.1 mm is not.
    let zero = vec![
        strip(1, 0, 100, Some(0)),
        strip(2, 100, 200, Some(0)),
        strip(3, 201, 300, Some(0)),
    ];
    assert_eq!(runs(&zero), vec![vec![0, 1], vec![2]]);
    assert_eq!(
        spans(&zero),
        vec![
            (0, 0, 1),
            (100, 100, 1),
            (200, 200, 2),
            (201, 201, 3),
            (300, 300, 3)
        ]
    );
    // A non-strip between two strips separates them.
    let split = vec![
        strip(1, 0, 100, Some(1)),
        dev(2, 100, 150),
        strip(3, 150, 200, Some(1)),
    ];
    assert_eq!(runs(&split), vec![vec![0], vec![2]]);
}

/// D12's free lengths, for part 4: R1 341 mm, R2 381.4 mm.
#[test]
fn free_lengths_of_golden_a() {
    assert_eq!(free_length(L1, &r1(), &brackets(&r1())), 341 * MM);
    assert_eq!(free_length(L1, &r2(), &brackets(&r2())), mm10(3814));
}

/// SN-2 (free): F55 (18 mm, centre clip) at clip 158 mm lands at clip 52 mm (the left gap's end is nearer); at clip
/// 320 mm it stays; a 300 mm part fits no gap.
#[test]
fn sn2_free_takes_the_nearest_gap() {
    let f55 = new_item(180);
    let r = resolve(&r1(), L1, f55, mm10(1490), SnapMode::Free).unwrap();
    assert_eq!(
        r,
        Resolution {
            s_left: mm10(430),
            moved: vec![]
        }
    );
    let r = resolve(&r1(), L1, f55, mm10(3110), SnapMode::Free).unwrap();
    assert_eq!(r.s_left, mm10(3110));
    assert_eq!(
        resolve(&r1(), L1, new_item(3000), mm10(80), SnapMode::Free),
        Err(SnapError::NoGap { width: 300 * MM })
    );
    // Equal distance to two gaps: the smaller s wins.
    let two = vec![dev(1, 100, 200)];
    let r = resolve(&two, 1000 * MM, new_item(100), mm10(100), SnapMode::Free).unwrap();
    assert_eq!(r.s_left, 0);
}

/// SN-3 (push): F55 at clip 158 mm stays at 158 mm and moves -102F1, -102F2, T55 and S51 18 mm; a locked -102F2
/// refuses; a 300 mm part pushes S51 past the rail end by 20 mm.
#[test]
fn sn3_push_moves_the_right_neighbours() {
    let r = resolve(&r1(), L1, new_item(180), mm10(1490), SnapMode::Push).unwrap();
    assert_eq!(r.s_left, mm10(1490));
    assert_eq!(
        r.moved,
        vec![
            (pid(F1_102), mm10(1670)),
            (pid(F2_102), mm10(1850)),
            (pid(T55), mm10(2300)),
            (pid(S51), mm10(2630)),
        ]
    );
    let mut locked = r1();
    locked[3].locked = true;
    assert_eq!(
        resolve(&locked, L1, new_item(180), mm10(1490), SnapMode::Push),
        Err(SnapError::Locked(Some(pid(F2_102))))
    );
    // A locked occupant to the left stays and does not refuse.
    let mut left_locked = r1();
    left_locked[1].locked = true;
    assert!(resolve(&left_locked, L1, new_item(180), mm10(1490), SnapMode::Push).is_ok());
    assert_eq!(
        resolve(&r1(), L1, new_item(3000), mm10(80), SnapMode::Push),
        Err(SnapError::PastEnd {
            neighbour: Some(pid(S51)),
            by: 20 * MM
        })
    );
    // Past the end with nothing to its right: the item itself.
    assert_eq!(
        resolve(&r1(), L1, new_item(180), mm10(5600), SnapMode::Push),
        Err(SnapError::PastEnd {
            neighbour: None,
            by: 8 * MM
        })
    );
    // A locked item is never pushed or swapped (free moves it).
    let mut item = r1()[1];
    item.locked = true;
    assert_eq!(
        resolve(&r1(), L1, item, mm10(3000), SnapMode::Push),
        Err(SnapError::MovedLocked)
    );
    assert!(resolve(&r1(), L1, item, mm10(3000), SnapMode::Free).is_ok());
}

/// SN-4 (swap): -101F1 dropped onto -102F2 gives -102F2 153.5, -102F1 185, -101F1 203 mm (clip points) and moves
/// nothing else; two equal neighbours exchange places; another rail and an empty spot refuse.
#[test]
fn sn4_swap_exchanges_within_the_segment() {
    let f1 = r1()[1];
    // Its centre over -102F2's centre (189.5 mm): left edge 180.5 mm.
    let r = resolve(&r1(), L1, f1, mm10(1805), SnapMode::Swap).unwrap();
    assert_eq!(r.s_left, mm10(1940), "-101F1's clip 203 mm");
    assert_eq!(
        r.moved,
        vec![(pid(F2_102), mm10(1310)), (pid(F1_102), mm10(1760))],
        "-102F2's clip 153.5 mm, -102F1's 185 mm"
    );
    // R3's -201U3 and -201U4 (40 mm each, abutting).
    let r3 = vec![dev(1, 880, 1280), dev(2, 1280, 1680), dev(3, 1680, 2080)];
    let r = resolve(&r3, L1, r3[0], mm10(1300), SnapMode::Swap).unwrap();
    assert_eq!(r.s_left, mm10(1280));
    assert_eq!(r.moved, vec![(pid(2), mm10(880))]);
    let elsewhere = dev(99, 0, 180);
    assert_eq!(
        resolve(&r1(), L1, elsewhere, mm10(1805), SnapMode::Swap),
        Err(SnapError::SwapAcrossRails)
    );
    assert_eq!(
        resolve(&r1(), L1, new_item(180), mm10(1805), SnapMode::Swap),
        Err(SnapError::SwapAcrossRails)
    );
    assert_eq!(
        resolve(&r1(), L1, f1, mm10(4000), SnapMode::Swap),
        Err(SnapError::SwapNoTarget)
    );
    // A locked target, or a locked placement in the segment, refuses.
    let mut locked = r1();
    locked[3].locked = true;
    assert_eq!(
        resolve(&locked, L1, f1, mm10(1805), SnapMode::Swap),
        Err(SnapError::Locked(Some(pid(F2_102))))
    );
    let mut inside = r1();
    inside[2].locked = true;
    assert_eq!(
        resolve(&inside, L1, f1, mm10(1805), SnapMode::Swap),
        Err(SnapError::Locked(Some(pid(F1_102))))
    );
    // A wider item swapped forward grows the segment: the items after it cascade.
    let rail = vec![dev(1, 0, 100), dev(2, 100, 300), dev(3, 300, 400)];
    let wide = RailItem {
        s1: mm10(200),
        ..rail[0]
    };
    let r = resolve(&rail, 100 * MM, wide, mm10(100), SnapMode::Swap).unwrap();
    assert_eq!(r.s_left, mm10(200));
    assert_eq!(r.moved, vec![(pid(2), 0), (pid(3), mm10(400))]);
}

/// SN-5 (clearance): an item with 5 mm pads keeps a 10 mm gap to a neighbour with the same pads; flipped pads swap;
/// strips ignore pads.
#[test]
fn sn5_pads_space_the_items() {
    let padded = |n, s0, s1, pad: [i128; 2]| RailItem {
        pad: pad.map(|p| p * MM),
        ..dev(n, s0, s1)
    };
    let rail = vec![padded(1, 1000, 1180, [5, 5])];
    let item = RailItem {
        pad: [5 * MM, 5 * MM],
        ..new_item(180)
    };
    let r = resolve(&rail, L1, item, mm10(1200), SnapMode::Free).unwrap();
    assert_eq!(r.s_left, mm10(1280), "10 mm after the neighbour's end");
    // Pads [0, 5] (a flipped [5, 0] part): only the after-pad meets the next item.
    let flipped = vec![padded(1, 1000, 1180, [5, 0])];
    let r = resolve(&flipped, L1, new_item(180), mm10(1180), SnapMode::Free).unwrap();
    assert_eq!(r.s_left, mm10(1180));
    let after = vec![padded(1, 1000, 1180, [0, 5])];
    let r = resolve(&after, L1, new_item(180), mm10(1180), SnapMode::Free).unwrap();
    assert_eq!(r.s_left, mm10(1230));
    assert_eq!(
        min_gap(&strip(1, 0, 10, Some(2)), &padded(2, 20, 30, [3, 3])),
        5 * MM
    );
    assert_eq!(
        min_gap(&padded(1, 0, 10, [3, 4]), &strip(2, 20, 30, Some(2))),
        6 * MM
    );
}

/// SN-6 (strips): on R2 an 18 mm non-strip dropped at 200 mm lands at 208.6 mm (after -X1032's bracket); a 26 mm strip
/// with a 10 mm bracket at 205 mm lands at 208.6 mm, joins the run and gives five brackets.
#[test]
fn sn6_strips_keep_their_brackets() {
    let r = resolve(&r2(), L1, new_item(180), mm10(2000), SnapMode::Free).unwrap();
    assert_eq!(r.s_left, mm10(2086));
    let s = RailItem {
        strip: true,
        w: Some(10 * MM),
        ..new_item(260)
    };
    let r = resolve(&r2(), L1, s, mm10(2050), SnapMode::Free).unwrap();
    assert_eq!(r.s_left, mm10(2086));
    let mut after = r2();
    after.push(s.at(r.s_left));
    assert_eq!(runs(&after).len(), 1);
    assert_eq!(brackets(&after).len(), 5);
    // A strip at the rail start keeps room for its bracket.
    let r = resolve(&[], L1, s, 0, SnapMode::Free).unwrap();
    assert_eq!(r.s_left, 10 * MM);
}

// ---------------------------------------------------------------------------------------------------------
// Property tests: random rails of at most 8 items (widths 1 to 80 mm, pads 0 to 6 mm, brackets 0 to 12 mm) on rails of
// 100 to 800 mm, laid out without overlap; 256 cases each.

/// A random non-overlapping rail: items laid from the start with random gaps.
fn rail() -> impl Strategy<Value = (i128, Vec<RailItem>)> {
    rail_of(0..=8)
}

/// [`rail`] with a number of items in `count` (ID-1493: the swap test draws a rail with an item rather than reject
/// one without, which aborted at the deep nightly's case count).
fn rail_of(count: std::ops::RangeInclusive<usize>) -> impl Strategy<Value = (i128, Vec<RailItem>)> {
    (
        100i128..=800,
        prop::collection::vec(
            (
                1i128..=80,
                0i128..=30,
                any::<bool>(),
                0i128..=12,
                0i128..=6,
                0i128..=6,
                any::<bool>(),
            ),
            count,
        ),
    )
        .prop_map(|(len, raw)| {
            let mut s = 0i128;
            let mut items = Vec::new();
            for (n, (w, gap, is_strip, bw, pb, pa, locked)) in raw.into_iter().enumerate() {
                s += gap * MM;
                let item = RailItem {
                    s0: s,
                    s1: s + w * MM,
                    strip: is_strip,
                    w: is_strip.then_some(bw * MM),
                    pad: [pb * MM, pa * MM],
                    locked: locked && n % 3 == 0,
                    placement: Some(pid(n as u128 + 1)),
                };
                s = item.s1;
                items.push(item);
            }
            (len.max(s / MM) * MM, items)
        })
}

fn sorted(mut v: Vec<RailItem>) -> Vec<RailItem> {
    v.sort_by_key(|i| (i.s0, i.s1, i.placement));
    v
}

/// The arrangement a resolution gives: every item at its new place, the dropped one at `s_left`.
fn arranged(items: &[RailItem], moved: &RailItem, r: &Resolution) -> Vec<RailItem> {
    let mut out: Vec<RailItem> = items
        .iter()
        .filter(|i| moved.placement.is_none() || i.placement != moved.placement)
        .map(|i| match r.moved.iter().find(|(p, _)| Some(*p) == i.placement) {
            Some((_, s0)) => i.at(*s0),
            None => *i,
        })
        .collect();
    out.push(moved.at(r.s_left));
    sorted(out)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Free: the item lands inside the rail with its minimum gaps to its consecutive neighbours held, nothing else
    /// moves, and a position that already fits is kept.
    #[test]
    fn free_holds_the_gaps((len, items) in rail(), w in 1i128..=60, at in 0i128..=800, pb in 0i128..=6, pa in 0i128..=6) {
        let m = RailItem { pad: [pb * MM, pa * MM], ..new_item(w * 10) };
        if let Ok(r) = resolve(&items, len, m, at * MM, SnapMode::Free) {
            prop_assert!(r.moved.is_empty());
            let all = arranged(&items, &m, &r);
            let k = all.iter().position(|i| i.placement.is_none()).unwrap();
            let me = all[k];
            prop_assert!(me.s0 >= start_gap(&me) && me.s1 + end_gap(&me) <= len);
            if k > 0 { prop_assert!(me.s0 - all[k - 1].s1 >= min_gap(&all[k - 1], &me)); }
            if k + 1 < all.len() { prop_assert!(all[k + 1].s0 - me.s1 >= min_gap(&me, &all[k + 1])); }
            // Determinism: the ghost and the commit call the same function on the same input.
            prop_assert_eq!(resolve(&items, len, m, at * MM, SnapMode::Free), Ok(r));
        }
    }

    /// Push: the occupants left of the item's centre stay, no locked placement moves, nothing moves left, and the
    /// arrangement from the item to the rail end holds every minimum gap.
    #[test]
    fn push_never_moves_left_or_a_lock((len, items) in rail(), w in 1i128..=60, at in 0i128..=800) {
        let m = new_item(w * 10);
        if let Ok(r) = resolve(&items, len, m, at * MM, SnapMode::Push) {
            for (p, s0) in &r.moved {
                let before = items.iter().find(|i| i.placement == Some(*p)).unwrap();
                prop_assert!(!before.locked);
                prop_assert!(*s0 > before.s0);
                prop_assert!(before.s0 + before.s1 >= 2 * at * MM + m.width());
            }
            let all = arranged(&items, &m, &r);
            // Every moved item (the dropped one included) keeps its minimum gap to the next item and the rail end.
            let in_m = |i: &RailItem| i.placement.is_none() || r.moved.iter().any(|(p, _)| Some(*p) == i.placement);
            for pair in all.windows(2) {
                if in_m(&pair[0]) {
                    prop_assert!(pair[1].s0 - pair[0].s1 >= min_gap(&pair[0], &pair[1]));
                }
            }
            let last = all[all.len() - 1];
            if in_m(&last) {
                prop_assert!(last.s1 + end_gap(&last) <= len);
            }
        }
    }

    /// Swap: a placement of the rail dropped anywhere either refuses or keeps every other item's order, moves no
    /// locked placement and leaves the arrangement within the rail without new overlaps.
    #[test]
    fn swap_keeps_the_rail_well_formed((len, items) in rail_of(1..=8), pick in 0usize..8, at in 0i128..=800) {
        let m = items[pick % items.len()];
        if let Ok(r) = resolve(&items, len, m, at * MM, SnapMode::Swap) {
            for (p, _) in &r.moved {
                prop_assert!(!items.iter().find(|i| i.placement == Some(*p)).unwrap().locked);
            }
            let all = arranged(&items, &m, &r);
            for pair in all.windows(2) {
                prop_assert!(pair[1].s0 >= pair[0].s1);
            }
            let last = all[all.len() - 1];
            let in_m = last.placement == m.placement || r.moved.iter().any(|(p, _)| Some(*p) == last.placement);
            if in_m {
                prop_assert!(last.s1 + end_gap(&last) <= len);
            }
        }
    }

    /// No input panics: arbitrary (overlapping, reversed, out-of-range) items and extreme positions.
    #[test]
    fn no_input_panics(
        raw in prop::collection::vec((any::<i64>(), any::<i64>(), any::<bool>(), any::<i64>(), any::<i64>(), any::<bool>()), 0..=6),
        len in any::<i64>(), at in any::<i64>(), mode in 0u8..3,
    ) {
        let items: Vec<RailItem> = raw.iter().enumerate().map(|(n, (a, b, s, w, p, l))| RailItem {
            s0: i128::from(*a), s1: i128::from(*b), strip: *s, w: Some(i128::from(*w)), pad: [i128::from(*p); 2],
            locked: *l, placement: Some(pid(n as u128)),
        }).collect();
        let mode = [SnapMode::Free, SnapMode::Push, SnapMode::Swap][usize::from(mode)];
        let m = items.first().copied().unwrap_or_else(|| new_item(10));
        let _ = resolve(&items, i128::from(len), m, i128::from(at), mode);
        let _ = brackets(&items);
        let _ = free_length(i128::from(len), &items, &brackets(&items));
        let _ = next_fit(&items, i128::from(len), &m, i128::from(at));
    }
}

// ---------------------------------------------------------------------------------------------------------
// WP-S4.02 part 2: next-fit (D6, `layout.auto_fill`)

/// D6: next-fit takes the first place at or after the floor where the item fits with its gaps (the rail start's on an
/// empty rail; occupied stretches skipped), never a gap before the floor, and `None` when nothing fits.
#[test]
fn next_fit_takes_the_first_fit_after_the_floor() {
    let m = new_item(180);
    assert_eq!(next_fit(&[], L1, &m, i128::MIN), Some(0));
    // Golden A's R1: the first gap is before -102G1 (61 mm), then after S51 (290 mm).
    assert_eq!(next_fit(&r1(), L1, &m, i128::MIN), Some(0));
    assert_eq!(next_fit(&r1(), L1, &m, mm10(500)), Some(mm10(2900)));
    // A floor inside a gap starts there.
    assert_eq!(next_fit(&r1(), L1, &m, mm10(3000)), Some(mm10(3000)));
    // A 300 mm item fits neither the 61 mm gap nor the 280 mm one.
    assert_eq!(next_fit(&r1(), L1, &new_item(3000), i128::MIN), None);
    // On R2, a non-strip after -X1032 keeps its run-end bracket: 208.6 mm.
    assert_eq!(next_fit(&r2(), L1, &m, mm10(1000)), Some(mm10(2086)));
    // Pads count: an item with a 5 mm pad before starts 5 mm after S51.
    let padded = RailItem {
        pad: [5 * MM, 0],
        ..m
    };
    assert_eq!(next_fit(&r1(), L1, &padded, mm10(500)), Some(mm10(2950)));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Next-fit: a found place is at or after the floor, inside the rail with its rail-end gaps, holds the minimum gap
    /// to its consecutive neighbours, and is the first such place (a tenth of a millimetre before it does not fit).
    #[test]
    fn next_fit_holds_the_gaps((len, items) in rail(), w in 1i128..=60, floor in 0i128..=800, pb in 0i128..=6, pa in 0i128..=6) {
        let m = RailItem { pad: [pb * MM, pa * MM], ..new_item(w * 10) };
        let fits = |s: i128| {
            let me = m.at(s);
            let mut all = items.clone();
            all.push(me);
            let all = sorted(all);
            let k = all.iter().position(|i| i.placement.is_none()).unwrap();
            me.s0 >= start_gap(&me)
                && me.s1 + end_gap(&me) <= len
                && (k == 0 || me.s0 - all[k - 1].s1 >= min_gap(&all[k - 1], &me))
                && (k + 1 == all.len() || all[k + 1].s0 - me.s1 >= min_gap(&me, &all[k + 1]))
        };
        if let Some(s) = next_fit(&items, len, &m, floor * MM) {
            prop_assert!(s >= floor * MM);
            prop_assert!(fits(s));
            if s > floor * MM && s > start_gap(&m) {
                prop_assert!(!fits(s - MM / 10));
            }
        }
    }
}
