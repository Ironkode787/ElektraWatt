// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Statistics of the memo engine: counters per query family, per commit and for the store's lifetime.

use std::collections::BTreeMap;

use crate::Rev;

/// Counters of one query family over a window (one commit, or the store's lifetime).
///
/// The store keeps one [`Stats`] for the current commit (reset by `Store::advance`, which emits it first) and
/// one for its lifetime. NFR-PERF-20 asserts the per-edit counts as exact numbers, so every counter is
/// deterministic: it counts engine decisions, never time.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Counters {
    /// A read answered by a memo already verified at the current revision.
    pub hits: u64,
    /// A memo re-stamped by the durability rule, without walking its dependencies.
    pub durability_skips: u64,
    /// A memo re-stamped after a dependency walk found nothing changed.
    pub verifications: u64,
    /// The query ran (new memo, a changed dependency, or an evicted value).
    pub re_executions: u64,
    /// A re-execution produced an equivalent value; `changed_at` was backdated.
    pub cut_offs: u64,
    /// Values dropped by the LRU.
    pub evictions: u64,
    /// Cold re-computations compared by the oracle.
    pub oracle_checks: u64,
    /// Oracle checks that differed from the memo.
    pub oracle_mismatches: u64,
    /// Cycles detected.
    pub cycles: u64,
}

/// What happened, for [`Counters::record`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Event {
    /// A read answered by an up-to-date memo.
    Hit,
    /// A memo re-stamped by the durability rule.
    DurabilitySkip,
    /// A memo re-stamped after a dependency walk.
    Verified,
    /// The query ran.
    ReExecuted,
    /// A re-execution produced an equivalent value.
    CutOff,
    /// A value was dropped by the LRU.
    Evicted,
    /// The oracle compared a cold re-computation.
    OracleCheck,
    /// The oracle found a difference.
    OracleMismatch,
    /// A cycle was detected.
    Cycle,
}

impl Counters {
    /// Adds one to the counter matching `event` (saturating).
    pub(crate) fn record(&mut self, event: Event) {
        let field = match event {
            Event::Hit => &mut self.hits,
            Event::DurabilitySkip => &mut self.durability_skips,
            Event::Verified => &mut self.verifications,
            Event::ReExecuted => &mut self.re_executions,
            Event::CutOff => &mut self.cut_offs,
            Event::Evicted => &mut self.evictions,
            Event::OracleCheck => &mut self.oracle_checks,
            Event::OracleMismatch => &mut self.oracle_mismatches,
            Event::Cycle => &mut self.cycles,
        };
        *field = field.saturating_add(1);
    }

    /// Adds every counter of `other` to this one (field-wise, saturating).
    pub fn merge(&mut self, other: &Counters) {
        self.hits = self.hits.saturating_add(other.hits);
        self.durability_skips = self.durability_skips.saturating_add(other.durability_skips);
        self.verifications = self.verifications.saturating_add(other.verifications);
        self.re_executions = self.re_executions.saturating_add(other.re_executions);
        self.cut_offs = self.cut_offs.saturating_add(other.cut_offs);
        self.evictions = self.evictions.saturating_add(other.evictions);
        self.oracle_checks = self.oracle_checks.saturating_add(other.oracle_checks);
        self.oracle_mismatches = self.oracle_mismatches.saturating_add(other.oracle_mismatches);
        self.cycles = self.cycles.saturating_add(other.cycles);
    }

    /// Whether every counter is zero.
    pub fn is_zero(&self) -> bool {
        *self == Counters::default()
    }
}

/// Counters per query family (by family name), in name order.
///
/// The store keeps one `Stats` for the current commit (reset by `Store::advance`, which emits it first) and
/// one for its lifetime. NFR-PERF-20 asserts per-edit counts as exact numbers.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Stats {
    families: BTreeMap<&'static str, Counters>,
}

impl Stats {
    /// The counters of the family called `name`; zero counters if it has none.
    pub fn family(&self, name: &str) -> Counters {
        self.families.get(name).copied().unwrap_or_default()
    }

    /// The families with counters, in name order.
    pub fn families(&self) -> impl Iterator<Item = (&'static str, &Counters)> + '_ {
        self.families.iter().map(|(name, counters)| (*name, counters))
    }

    /// The counters merged over all families.
    pub fn total(&self) -> Counters {
        let mut total = Counters::default();
        for counters in self.families.values() {
            total.merge(counters);
        }
        total
    }

    /// Whether no family has recorded anything.
    pub fn is_empty(&self) -> bool {
        self.families.is_empty()
    }

    /// Records `event` for `family`.
    pub(crate) fn record(&mut self, family: &'static str, event: Event) {
        self.families.entry(family).or_default().record(event);
    }

    /// Adds every family of `other` to this one.
    pub(crate) fn merge(&mut self, other: &Stats) {
        for (name, counters) in &other.families {
            self.families.entry(name).or_default().merge(counters);
        }
    }

    /// Forgets every counter.
    pub(crate) fn clear(&mut self) {
        self.families.clear();
    }
}

/// Emits one `tracing` event per family with non-zero counters, in name order, at DEBUG level with target
/// `ew_memo::stats` and the fields `window` (the label: `"commit"` or `"total"`), `rev`, `family` and every
/// counter by its field name.
pub(crate) fn emit(rev: Rev, window: &'static str, stats: &Stats) {
    for (family, c) in stats.families() {
        if c.is_zero() {
            continue;
        }
        tracing::event!(
            target: "ew_memo::stats",
            tracing::Level::DEBUG,
            window,
            rev = rev.0,
            family,
            hits = c.hits,
            durability_skips = c.durability_skips,
            verifications = c.verifications,
            re_executions = c.re_executions,
            cut_offs = c.cut_offs,
            evictions = c.evictions,
            oracle_checks = c.oracle_checks,
            oracle_mismatches = c.oracle_mismatches,
            cycles = c.cycles,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use tracing::field::{Field, Visit};
    use tracing::span;
    use tracing::{Event as TracingEvent, Metadata, Subscriber};

    use super::*;

    const ALL_EVENTS: [Event; 9] = [
        Event::Hit,
        Event::DurabilitySkip,
        Event::Verified,
        Event::ReExecuted,
        Event::CutOff,
        Event::Evicted,
        Event::OracleCheck,
        Event::OracleMismatch,
        Event::Cycle,
    ];

    #[test]
    fn record_increments_the_matching_field() {
        let mut c = Counters::default();
        assert!(c.is_zero());
        for event in ALL_EVENTS {
            c.record(event);
        }
        assert_eq!(
            c,
            Counters {
                hits: 1,
                durability_skips: 1,
                verifications: 1,
                re_executions: 1,
                cut_offs: 1,
                evictions: 1,
                oracle_checks: 1,
                oracle_mismatches: 1,
                cycles: 1,
            }
        );
        assert!(!c.is_zero());
        c.record(Event::Hit);
        assert_eq!(c.hits, 2);
        assert_eq!(c.cycles, 1);
    }

    #[test]
    fn record_and_merge_saturate() {
        let mut c = Counters {
            hits: u64::MAX,
            ..Counters::default()
        };
        c.record(Event::Hit);
        assert_eq!(c.hits, u64::MAX);
        let other = Counters {
            hits: 5,
            cycles: 2,
            ..Counters::default()
        };
        c.merge(&other);
        assert_eq!(c.hits, u64::MAX);
        assert_eq!(c.cycles, 2);
    }

    #[test]
    fn merge_adds_field_wise() {
        let mut a = Counters {
            hits: 1,
            re_executions: 2,
            evictions: 3,
            ..Counters::default()
        };
        let b = Counters {
            hits: 10,
            cut_offs: 4,
            evictions: 5,
            ..Counters::default()
        };
        a.merge(&b);
        assert_eq!(a.hits, 11);
        assert_eq!(a.re_executions, 2);
        assert_eq!(a.cut_offs, 4);
        assert_eq!(a.evictions, 8);
    }

    #[test]
    fn stats_record_total_and_absent_family() {
        let mut s = Stats::default();
        assert!(s.is_empty());
        assert!(s.family("nope").is_zero());
        s.record("b", Event::Hit);
        s.record("a", Event::Hit);
        s.record("a", Event::ReExecuted);
        assert!(!s.is_empty());
        assert_eq!(s.family("a").hits, 1);
        assert_eq!(s.family("a").re_executions, 1);
        assert_eq!(s.family("b").hits, 1);
        assert!(s.family("c").is_zero());
        let total = s.total();
        assert_eq!(total.hits, 2);
        assert_eq!(total.re_executions, 1);
    }

    #[test]
    fn families_iterate_in_name_order() {
        let mut s = Stats::default();
        for name in ["zeta", "alpha", "mid"] {
            s.record(name, Event::Verified);
        }
        let names: Vec<_> = s.families().map(|(n, _)| n).collect();
        assert_eq!(names, ["alpha", "mid", "zeta"]);
    }

    #[test]
    fn stats_merge_and_clear() {
        let mut a = Stats::default();
        a.record("x", Event::Hit);
        let mut b = Stats::default();
        b.record("x", Event::Hit);
        b.record("y", Event::CutOff);
        a.merge(&b);
        assert_eq!(a.family("x").hits, 2);
        assert_eq!(a.family("y").cut_offs, 1);
        a.clear();
        assert!(a.is_empty());
        assert_eq!(a, Stats::default());
    }

    /// One captured event: field name to its rendered value.
    type Captured = BTreeMap<String, String>;

    #[derive(Default)]
    struct Collector(Captured);

    impl Visit for Collector {
        fn record_u64(&mut self, field: &Field, value: u64) {
            self.0.insert(field.name().to_string(), value.to_string());
        }
        fn record_str(&mut self, field: &Field, value: &str) {
            self.0.insert(field.name().to_string(), value.to_string());
        }
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.0.insert(field.name().to_string(), format!("{value:?}"));
        }
    }

    /// (target, level, fields) per captured event.
    type Log = Vec<(String, tracing::Level, Captured)>;

    struct Capture {
        events: Arc<Mutex<Log>>,
    }

    impl Subscriber for Capture {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
            span::Id::from_u64(1)
        }
        fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
        fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
        fn event(&self, event: &TracingEvent<'_>) {
            let mut collector = Collector::default();
            event.record(&mut collector);
            let meta = event.metadata();
            self.events
                .lock()
                .unwrap()
                .push((meta.target().to_string(), *meta.level(), collector.0));
        }
        fn enter(&self, _: &span::Id) {}
        fn exit(&self, _: &span::Id) {}
    }

    #[test]
    fn emit_one_event_per_nonzero_family_in_name_order() {
        let mut s = Stats::default();
        s.record("beta", Event::Hit);
        s.record("beta", Event::Hit);
        s.record("alpha", Event::ReExecuted);
        // A family whose counters are all zero emits nothing.
        s.families.insert("gamma", Counters::default());

        let events = Arc::new(Mutex::new(Vec::new()));
        let subscriber = Capture {
            events: Arc::clone(&events),
        };
        tracing::subscriber::with_default(subscriber, || emit(Rev(7), "commit", &s));

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        for (target, level, _) in events.iter() {
            assert_eq!(target, "ew_memo::stats");
            assert_eq!(*level, tracing::Level::DEBUG);
        }
        let (_, _, first) = &events[0];
        assert_eq!(first["family"], "alpha");
        assert_eq!(first["window"], "commit");
        assert_eq!(first["rev"], "7");
        assert_eq!(first["hits"], "0");
        assert_eq!(first["re_executions"], "1");
        let (_, _, second) = &events[1];
        assert_eq!(second["family"], "beta");
        assert_eq!(second["hits"], "2");
        assert_eq!(second["re_executions"], "0");
        for field in [
            "durability_skips",
            "verifications",
            "cut_offs",
            "evictions",
            "oracle_checks",
            "oracle_mismatches",
            "cycles",
        ] {
            assert!(second.contains_key(field), "missing field {field}");
        }
    }

    #[test]
    fn emit_of_empty_stats_emits_nothing() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let subscriber = Capture {
            events: Arc::clone(&events),
        };
        tracing::subscriber::with_default(subscriber, || emit(Rev(1), "total", &Stats::default()));
        assert!(events.lock().unwrap().is_empty());
    }
}
