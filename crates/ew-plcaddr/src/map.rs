// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The part-data map language (doc 10 section 2.6): `<k>n[+<c>]` or `<k>p[+<c>]`.

use core::fmt;

/// The variable of a map expression.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MapVar {
    /// `n`.
    N,
    /// `p`.
    P,
}

/// A map expression `k*x + c`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MapExpr {
    /// The variable.
    pub var: MapVar,
    /// The factor (at least 1).
    pub k: u32,
    /// The constant.
    pub c: u32,
}

/// Parse a decimal without leading zeros (except "0" itself).
fn number(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if s.len() > 1 && s.starts_with('0') {
        return None;
    }
    s.parse().ok()
}

impl MapExpr {
    /// Parse; `None` for anything outside the grammar.
    pub fn parse(s: &str) -> Option<MapExpr> {
        let vpos = s.find(['n', 'p'])?;
        let var = if s.as_bytes()[vpos] == b'n' {
            MapVar::N
        } else {
            MapVar::P
        };
        let (kt, rest) = (&s[..vpos], &s[vpos + 1..]);
        let k = if kt.is_empty() { 1 } else { number(kt)? };
        if k == 0 {
            return None;
        }
        let c = if rest.is_empty() {
            0
        } else {
            number(rest.strip_prefix('+')?)?
        };
        Some(MapExpr { var, k, c })
    }

    /// Evaluate `k*x + c`; `None` on overflow.
    pub fn eval(&self, x: u32) -> Option<u32> {
        self.k.checked_mul(x)?.checked_add(self.c)
    }
}

impl fmt::Display for MapExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.k != 1 {
            write!(f, "{}", self.k)?;
        }
        f.write_str(match self.var {
            MapVar::N => "n",
            MapVar::P => "p",
        })?;
        if self.c != 0 {
            write!(f, "+{}", self.c)?;
        }
        Ok(())
    }
}
