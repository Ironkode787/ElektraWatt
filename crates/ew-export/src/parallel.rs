// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The parallel page phase of an export (doc 09 §4.2, §4.3 rule 3; FR-OUT-019, NFR-REL-07, AC-14).
//!
//! An export builds its pages independently (display lists, SVG documents) and assembles them in page
//! order. [`par_map`] runs that per-page phase on scoped threads of the standard library and collects every
//! result **by page index**, so the output does not depend on the thread count, on which thread finished
//! first or on the scheduler: serial and parallel exports are the same bytes.
//!
//! *As built (cycle 14):* scoped threads of `std` with a strided split (worker `w` of `n` takes the indices
//! `w, w + n, w + 2n, …`), not rayon (doc 09 §4.2 names rayon): no new dependency for a phase that has no
//! nested parallelism yet. Swapping the pool keeps the contract, results by index.

use std::num::NonZeroUsize;

/// How many threads the per-page phase of an export uses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Parallelism {
    /// The calling thread only.
    #[default]
    Serial,
    /// At most this many worker threads (never more than there are items).
    Threads(NonZeroUsize),
}

impl Parallelism {
    /// `n` threads; `0` and `1` are [`Parallelism::Serial`].
    pub fn threads(n: usize) -> Parallelism {
        match NonZeroUsize::new(n) {
            Some(n) if n.get() > 1 => Parallelism::Threads(n),
            _ => Parallelism::Serial,
        }
    }

    /// The number of threads for `items` items: at least 1, at most `items`.
    pub fn workers(self, items: usize) -> usize {
        match self {
            Parallelism::Serial => 1,
            Parallelism::Threads(n) => n.get().min(items).max(1),
        }
    }
}

/// `f` over `items`, the results in item order, whatever the parallelism. A panic in `f` propagates to the
/// caller (after the other workers stop).
pub fn par_map<T, U, F>(items: &[T], parallelism: Parallelism, f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    F: Fn(usize, &T) -> U + Sync,
{
    let workers = parallelism.workers(items.len());
    if workers <= 1 {
        return items.iter().enumerate().map(|(i, t)| f(i, t)).collect();
    }
    let f = &f;
    let parts: Vec<Vec<(usize, U)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                scope.spawn(move || {
                    (w..items.len())
                        .step_by(workers)
                        .map(|i| (i, f(i, &items[i])))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect()
    });
    let mut slots: Vec<Option<U>> = std::iter::repeat_with(|| None).take(items.len()).collect();
    for (i, u) in parts.into_iter().flatten() {
        slots[i] = Some(u);
    }
    slots
        .into_iter()
        .map(|u| u.expect("every index is mapped exactly once"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_come_back_in_item_order_for_every_thread_count() {
        let items: Vec<u64> = (0..37).collect();
        let serial = par_map(&items, Parallelism::Serial, |i, x| (i, x * x));
        for n in [1, 2, 3, 8, 64] {
            assert_eq!(
                par_map(&items, Parallelism::threads(n), |i, x| (i, x * x)),
                serial
            );
        }
        assert!(par_map(&[] as &[u8], Parallelism::threads(4), |_, x| *x).is_empty());
    }

    #[test]
    fn workers_are_bounded_by_the_items() {
        assert_eq!(Parallelism::threads(0), Parallelism::Serial);
        assert_eq!(Parallelism::threads(1), Parallelism::Serial);
        assert_eq!(Parallelism::threads(8).workers(3), 3);
        assert_eq!(Parallelism::threads(8).workers(0), 1);
        assert_eq!(Parallelism::Serial.workers(10), 1);
    }

    #[test]
    #[should_panic(expected = "boom")]
    fn a_panic_in_a_worker_reaches_the_caller() {
        let items: Vec<u8> = (0..8).collect();
        par_map(&items, Parallelism::threads(4), |_, x| {
            assert!(*x != 5, "boom");
            *x
        });
    }
}
