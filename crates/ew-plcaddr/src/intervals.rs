// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Occupied unit ranges per area.

use crate::address::Area;
use std::collections::BTreeMap;

/// Occupied half-open unit ranges per area.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct AreaIntervals {
    map: BTreeMap<Area, BTreeMap<u32, u32>>,
}

impl AreaIntervals {
    /// An empty set.
    pub fn new() -> AreaIntervals {
        AreaIntervals::default()
    }

    /// Occupy `[start, start+len)`, merging adjacent and overlapping ranges.
    pub fn insert(&mut self, area: Area, start: u32, len: u32) {
        if len == 0 {
            return;
        }
        let m = self.map.entry(area).or_default();
        let mut s = start;
        let mut e = start.saturating_add(len);
        // A predecessor that touches or overlaps.
        if let Some((&ps, &pe)) = m.range(..=s).next_back() {
            if pe >= s {
                s = ps;
                e = e.max(pe);
            }
        }
        let absorbed: Vec<u32> = m.range(s..=e).map(|(&k, _)| k).collect();
        for k in absorbed {
            if let Some(ke) = m.remove(&k) {
                e = e.max(ke);
            }
        }
        m.insert(s, e);
    }

    /// Whether `[start, start+len)` intersects an occupied range.
    pub fn overlaps(&self, area: Area, start: u32, len: u32) -> bool {
        if len == 0 {
            return false;
        }
        let end = start.saturating_add(len);
        let Some(m) = self.map.get(&area) else {
            return false;
        };
        if let Some((_, &pe)) = m.range(..=start).next_back() {
            if pe > start {
                return true;
            }
        }
        m.range(start..end).next().is_some()
    }

    /// Smallest `s >= from` such that `[s, s+len)` is free.
    pub fn first_free(&self, area: Area, from: u32, len: u32) -> u32 {
        let Some(m) = self.map.get(&area) else {
            return from;
        };
        let mut s = from;
        if let Some((_, &pe)) = m.range(..=s).next_back() {
            if pe > s {
                s = pe;
            }
        }
        for (&is, &ie) in m.range(s..) {
            if is >= s.saturating_add(len) {
                break;
            }
            s = s.max(ie);
        }
        s
    }
}
