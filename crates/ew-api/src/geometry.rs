// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The orientation arithmetic the canvas tools need without a `Session` (WP-S2.04 §4.3): pure functions equal to
//! `ew_units::Orient2`'s mapping, on the plain integers a tool holds.
//!
//! An orientation is `o` in `0..=7`, the variant letters A to H of doc 06 §2.5: R0, R90, R180, R270 (quarter turns
//! clockwise on the sheet, y down), then M0, M90, M180, M270 (mirrored about the vertical axis first, then turned).
//! A value above 7 is taken modulo 8. [`orient_then`] composes as `rep.transform` does: first the current
//! orientation, then `rotate` quarter turns clockwise, then, with `mirror`, the mirror about the vertical axis.

use ew_derive::ew_units::{Nm, Orient2, Pt2};

/// A direction on the page (y down): the side a wire leaves a connection point to.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Dir {
    /// Towards smaller y.
    Up,
    /// Towards larger y.
    Down,
    /// Towards smaller x.
    Left,
    /// Towards larger x.
    Right,
}

impl Dir {
    /// The unit vector of the direction.
    pub const fn vector(self) -> (i64, i64) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }

    /// The direction of a unit vector along an axis; `None` for any other vector.
    pub const fn of_vector(v: (i64, i64)) -> Option<Dir> {
        match v {
            (0, -1) => Some(Dir::Up),
            (0, 1) => Some(Dir::Down),
            (-1, 0) => Some(Dir::Left),
            (1, 0) => Some(Dir::Right),
            _ => None,
        }
    }

    /// The word of the direction: `up`, `down`, `left`, `right`.
    pub const fn word(self) -> &'static str {
        match self {
            Dir::Up => "up",
            Dir::Down => "down",
            Dir::Left => "left",
            Dir::Right => "right",
        }
    }
}

/// The orientation `o` (taken modulo 8) as `Orient2`.
pub fn orient2(o: u8) -> Orient2 {
    Orient2::ALL[usize::from(o % 8)]
}

/// The index `0..=7` of an `Orient2` (its letter's position).
pub fn orient_index(o: Orient2) -> u8 {
    // `ALL` lists the eight variants in letter order.
    Orient2::ALL.iter().position(|x| *x == o).map_or(0, |i| i as u8)
}

/// `p` under orientation `o`: `Orient2::apply` (a coordinate of `i64::MIN` saturates instead of overflowing).
pub fn orient_point(o: u8, p: (i64, i64)) -> (i64, i64) {
    let clamp = |v: i64| v.max(-i64::MAX);
    let q = orient2(o).apply(Pt2::new(Nm(clamp(p.0)), Nm(clamp(p.1))));
    (q.x.0, q.y.0)
}

/// Direction `d` under orientation `o`.
pub fn orient_dir(o: u8, d: Dir) -> Dir {
    Dir::of_vector(orient_point(o, d.vector())).unwrap_or(d)
}

/// The orientation `rep.transform` gives a representation of orientation `o` for `rotate` quarter turns clockwise
/// (taken modulo 4) and `mirror`: `o.then(R(rotate))`, then `.then(M0)` with `mirror`.
pub fn orient_then(o: u8, rotate: u8, mirror: bool) -> u8 {
    let mut r = orient2(o).then(Orient2::from_parts(false, rotate % 4));
    if mirror {
        r = r.then(Orient2::M0);
    }
    orient_index(r)
}

/// The variant letter of orientation `o`: `A` to `H`.
pub fn variant_letter(o: u8) -> char {
    orient2(o).letter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_functions_equal_orient2_for_every_combination() {
        let p = (3_000_001_i64, -1_250_000_i64);
        for (i, o) in Orient2::ALL.iter().enumerate() {
            let i = i as u8;
            assert_eq!(orient_index(*o), i);
            assert_eq!(variant_letter(i), o.letter());
            let q = o.apply(Pt2::new(Nm(p.0), Nm(p.1)));
            assert_eq!(orient_point(i, p), (q.x.0, q.y.0));
            for d in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
                let v = o.apply(Pt2::new(Nm(d.vector().0), Nm(d.vector().1)));
                assert_eq!(orient_dir(i, d).vector(), (v.x.0, v.y.0));
            }
            for rotate in 0..4u8 {
                for mirror in [false, true] {
                    let mut expected = o.then(Orient2::from_parts(false, rotate));
                    if mirror {
                        expected = expected.then(Orient2::M0);
                    }
                    assert_eq!(orient_then(i, rotate, mirror), orient_index(expected));
                }
            }
        }
        assert_eq!(variant_letter(8), 'A');
        assert_eq!(orient_then(0, 1, false), 1);
        assert_eq!(orient_then(0, 2, true), orient_index(Orient2::M180));
    }
}
