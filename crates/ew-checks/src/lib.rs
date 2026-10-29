// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rule engine, rule packs, the `RuleImpl` trait and registry, online checks as queries, offline check jobs,
//! waivers and quick fixes (ADR-0019, doc 15 §2–§4).
//!
//! # Shape (MVP engine)
//!
//! - **Rules.** A shipped rule is a [`Rule`]: static [`RuleMeta`] (ID, six-digit code, pack, kind, modes,
//!   severity and release severity, scope kind, supersessions, copied from the catalogue of doc 15 §4) and
//!   `run(&Q, &Scope) -> Vec<Finding>`, a pure function over the tracked-read context of `ew-derive`. A rule
//!   whose modes include prevent also implements [`Rule::prevent`], which compares the base and the
//!   candidate snapshot of a batch. Rules read through `Q` only: this crate does not enable `ew-model`'s
//!   `raw-read` (ARCH §3 rule 1; `cargo xtask check-raw-read`).
//! - **Packs** are the `pack` field of the metadata; the [`rules()`] registry lists every shipped Rust rule in
//!   ID order (ADR-0019 amendment 7). TOML packs, rhai and SQL rules, company and project packs follow.
//! - **Schemes** ([`Scheme`]) select rules; the [`CheckMode`] decides which rules can run (online, or
//!   release, which also reports prevent rules) and which severity column applies (doc 15 §3.2). Shipped:
//!   `drafting`, `pre-delivery`, `migration`, `full`.
//! - **Memoisation.** [`CheckOf`] is the query family `check_findings(rule, scope)` with policy Memo: in a
//!   session's memo store a rule re-runs in a scope only when something it read changed (doc 15 §3.3). The
//!   scope kinds are project, page, strip, device, cable and potential.
//! - **Runs.** [`run`] evaluates a scheme over a snapshot and applies effective severities, supersession,
//!   waivers (the reserved `waiver` records, matched by [`Fingerprint`]) and EW-CORE-081; the
//!   [`CheckReport`] is in canonical order (code, printed subject and message, fingerprint) and carries the totals and `ew check`'s exit code.
//!   [`prevent`] is the commit validator's entry (ADR-0009): `ew-api` installs it as the model's
//!   `PreventHook`.
//!
//! # Example
//!
//! ```
//! use ew_checks::{CheckMode, PRE_DELIVERY, rules};
//!
//! assert!(rules().iter().any(|r| r.meta().id == "EW-TERM-002"));
//! let term002 = ew_checks::rule("EW-TERM-002").unwrap().meta();
//! assert!(PRE_DELIVERY.enabled(term002, CheckMode::Release));
//! assert_eq!(term002.code, "001002");
//! ```

#![forbid(unsafe_code)]

mod engine;
mod finding;
mod registry;
pub mod report;
mod rule;
pub mod rules;
mod scheme;

pub use engine::{CheckOf, CheckReport, Counts, IntegrityItem, prevent, run, scopes};
pub use finding::{Finding, FindingState, Fingerprint, QuickFix, Severity, Target, sort_findings};
pub use registry::{rule, rules};
pub use rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};
pub use scheme::{CheckMode, DRAFTING, FULL, MIGRATION, PRE_DELIVERY, SCHEMES, Scheme, scheme};
