// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The jump history of a canvas tab (WP-S2.03 part 1, spec section 6.3; ruling D4): Back and Forward walk the
//! locations a tab jumped from, per tab and not persisted.
//!
//! A jump (following a link, Go to, the navigator's Open) pushes the location it leaves on the back list (at most
//! [`MAX_HISTORY`] entries, the oldest dropped) and clears the forward list; walking pages (Page Up, Page Down,
//! first, last) records nothing. Back moves the current location to the forward list and returns the last back
//! entry; Forward is its mirror. Closing a tab drops its history.

use crate::canvas::{PageId, Viewport};

/// The most entries the back list keeps.
pub const MAX_HISTORY: usize = 50;

/// A place in a tab: a page and its view.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Location {
    /// The page.
    pub page: PageId,
    /// The view of the page.
    pub viewport: Viewport,
}

/// The back and forward lists of a tab, most recent last.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct History {
    back: Vec<Location>,
    forward: Vec<Location>,
}

impl History {
    /// An empty history.
    pub fn new() -> History {
        History::default()
    }

    /// The number of back entries.
    pub fn back_len(&self) -> usize {
        self.back.len()
    }

    /// The number of forward entries.
    pub fn forward_len(&self) -> usize {
        self.forward.len()
    }

    /// A jump leaves `from`: it goes on the back list (the oldest dropped beyond [`MAX_HISTORY`]) and the forward
    /// list is cleared.
    pub fn record(&mut self, from: Location) {
        self.back.push(from);
        if self.back.len() > MAX_HISTORY {
            let extra = self.back.len() - MAX_HISTORY;
            self.back.drain(..extra);
        }
        self.forward.clear();
    }

    /// Back from `current`: the last back entry whose page `exists`, entries of vanished pages dropped on the way;
    /// `current` goes on the forward list. `None` (and nothing changes but the dropped entries) when none is left.
    pub fn back(&mut self, current: Location, exists: impl Fn(PageId) -> bool) -> Option<Location> {
        Self::step(&mut self.back, &mut self.forward, current, exists)
    }

    /// Forward from `current`: the mirror of [`History::back`].
    pub fn forward(&mut self, current: Location, exists: impl Fn(PageId) -> bool) -> Option<Location> {
        Self::step(&mut self.forward, &mut self.back, current, exists)
    }

    fn step(
        from: &mut Vec<Location>,
        to: &mut Vec<Location>,
        current: Location,
        exists: impl Fn(PageId) -> bool,
    ) -> Option<Location> {
        while let Some(entry) = from.pop() {
            if exists(entry.page) {
                to.push(current);
                if to.len() > MAX_HISTORY {
                    to.remove(0);
                }
                return Some(entry);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_api::output::ew_dl::Destination;

    fn page(n: u128) -> PageId {
        // A page identity made of a number (the crate names no UUID type: parse one).
        let text = format!("00000000-0000-0000-0000-{n:012x}");
        PageId(Destination::Page(std::str::FromStr::from_str(&text).unwrap()))
    }

    fn at(n: u128, x: i64) -> Location {
        Location {
            page: page(n),
            viewport: Viewport {
                centre: (x, 0),
                zoom: 1.0,
                fitted: false,
            },
        }
    }

    #[test]
    fn back_and_forward_walk_the_jumps() {
        let mut h = History::new();
        h.record(at(1, 0));
        h.record(at(2, 0));
        assert_eq!((h.back_len(), h.forward_len()), (2, 0));
        let b = h.back(at(3, 0), |_| true).unwrap();
        assert_eq!(b, at(2, 0));
        assert_eq!((h.back_len(), h.forward_len()), (1, 1));
        let f = h.forward(b, |_| true).unwrap();
        assert_eq!(f, at(3, 0));
        // A new jump clears the forward list.
        h.back(f, |_| true).unwrap();
        h.record(at(9, 0));
        assert_eq!(h.forward_len(), 0);
    }

    #[test]
    fn the_back_list_keeps_fifty_and_vanished_pages_are_skipped() {
        let mut h = History::new();
        for n in 0..60 {
            h.record(at(n, 0));
        }
        assert_eq!(h.back_len(), MAX_HISTORY);
        // The ten oldest went: back from the end reaches page 10 last.
        let mut cur = at(99, 0);
        let mut last = None;
        while let Some(l) = h.back(cur, |_| true) {
            cur = l;
            last = Some(l);
        }
        assert_eq!(last, Some(at(10, 0)));
        // Entries of pages that no longer exist are dropped and the next is tried.
        let mut h = History::new();
        h.record(at(1, 1));
        h.record(at(2, 2));
        h.record(at(3, 3));
        let gone = page(3);
        let b = h.back(at(4, 4), |p| p != gone && p != page(2)).unwrap();
        assert_eq!(b, at(1, 1));
        assert_eq!((h.back_len(), h.forward_len()), (0, 1));
        assert_eq!(h.back(b, |_| true), None);
    }
}
