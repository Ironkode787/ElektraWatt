// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Findings, their fingerprints and quick fixes (doc 15 §2.5, §2.6).

use std::fmt;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::natural_cmp;
use ew_derive::ew_schema::{AnyId, Kind, Page};
use sha2::{Digest, Sha256};

use crate::rule::RuleMeta;

/// The severity of a finding (doc 15 §2.3), ordered `Info < Warning < Error`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Severity {
    /// Information.
    Info,
    /// A warning.
    Warning,
    /// An error: blocks a release when unwaived.
    Error,
}

impl Severity {
    /// The lower-case name: `info`, `warning`, `error`.
    pub fn name(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The state of a finding (doc 15 §2.1). Only the states the MVP engine produces are modelled.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FindingState {
    /// Open: counts towards the totals.
    Open,
    /// Covered by a waiver: keeps its severity but leaves the counts (doc 15 §3.2).
    Waived {
        /// The waiver's record ID, as stored.
        waiver: String,
    },
}

/// The 128-bit fingerprint of a finding (FR-CHK-044):
/// `first 128 bits of SHA-256("ew-fp/1" ‖ rule_id ‖ sorted target UUIDs ‖ discriminator)`, the UUIDs as
/// their 16 bytes. It holds only stable data, so a renumber or a move keeps it, and with it every waiver.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Fingerprint(pub u128);

impl Fingerprint {
    /// The fingerprint of `rule` over `targets` (sorted and deduplicated here) and `discriminator`.
    pub fn of(rule: &str, targets: &[AnyId], discriminator: &str) -> Fingerprint {
        let mut ids: Vec<AnyId> = targets.to_vec();
        ids.sort();
        ids.dedup();
        let mut h = Sha256::new();
        h.update(b"ew-fp/1");
        h.update(rule.as_bytes());
        for id in &ids {
            h.update(id.as_uuid().as_bytes());
        }
        h.update(discriminator.as_bytes());
        let digest = h.finalize();
        let mut first = [0u8; 16];
        first.copy_from_slice(&digest[..16]);
        Fingerprint(u128::from_be_bytes(first))
    }

    /// Parses the 32 lower- or upper-case hex digits of [`Display`](fmt::Display).
    pub fn parse(s: &str) -> Option<Fingerprint> {
        if s.len() != 32 {
            return None;
        }
        u128::from_str_radix(s, 16).ok().map(Fingerprint)
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

/// A record a finding is about: its kind and ID.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Target {
    /// The record kind.
    pub kind: Kind,
    /// The record ID.
    pub id: AnyId,
}

impl Target {
    /// The target `id` of kind `kind`.
    pub fn new<T>(kind: Kind, id: Id<T>) -> Target {
        Target { kind, id: id.cast() }
    }
}

/// A quick fix: a command proposal that runs as a preview and commits as one undoable transaction
/// (doc 15 §2.6, ADR-0008).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QuickFix {
    /// The label, in English (Fluent keys arrive with the message panel).
    pub label: String,
    /// The command name from the catalogue, for example `terminal.designate`.
    pub command: String,
    /// The command's parameters.
    pub params: serde_json::Value,
}

/// One finding (doc 15 §2.1, §2.5): what a rule reports about a set of records.
///
/// Findings sort by code, then their printed subject and message, then the fingerprint (doc 15 §3.9, FT-594) and
/// hold no wall-clock data, so two platforms, and two fresh copies of one project, give identical finding lists.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Finding {
    /// The rule ID.
    pub rule: &'static str,
    /// The six-digit code.
    pub code: &'static str,
    /// The severity: the rule's (or a finding-specific one) as the rule reports it, the effective severity
    /// of the scheme once the engine has resolved it (doc 15 §3.2).
    pub severity: Severity,
    /// Open or waived.
    pub state: FindingState,
    /// The records concerned, sorted and distinct; the first is the primary target (the subject).
    pub targets: Vec<Target>,
    /// Stable data that tells findings of one rule over the same targets apart (a CP index, a field path);
    /// never a tag, folio, position or count.
    pub discriminator: String,
    /// What the message is about, as printed: a derived tag or a name (rendered when the rule ran).
    pub subject: String,
    /// The page the finding is shown on, if any.
    pub page: Option<Id<Page>>,
    /// The message, in English.
    pub message: String,
    /// A quick fix, where the rule offers one.
    pub fix: Option<QuickFix>,
    /// The fingerprint.
    pub fingerprint: Fingerprint,
}

impl Finding {
    /// A finding of `rule` over `targets` with the rule's default severity, state open, no page and no fix.
    /// The targets are sorted and deduplicated; the fingerprint is computed from them and `discriminator`.
    pub fn new(
        rule: &'static RuleMeta,
        targets: impl IntoIterator<Item = Target>,
        discriminator: impl Into<String>,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Finding {
        let mut targets: Vec<Target> = targets.into_iter().collect();
        targets.sort();
        targets.dedup();
        let discriminator = discriminator.into();
        let ids: Vec<AnyId> = targets.iter().map(|t| t.id).collect();
        Finding {
            rule: rule.id,
            code: rule.code,
            severity: rule.severity,
            state: FindingState::Open,
            fingerprint: Fingerprint::of(rule.id, &ids, &discriminator),
            targets,
            discriminator,
            subject: subject.into(),
            page: None,
            message: message.into(),
            fix: None,
        }
    }

    /// The same finding with another severity.
    #[must_use]
    pub fn with_severity(mut self, severity: Severity) -> Finding {
        self.severity = severity;
        self
    }

    /// The same finding shown on `page`.
    #[must_use]
    pub fn on_page(mut self, page: Id<Page>) -> Finding {
        self.page = Some(page);
        self
    }

    /// The same finding with its fingerprint computed from `key` (and the discriminator) instead of every target:
    /// the display targets stay, the identity a waiver matches does not follow them. A potential's findings key on
    /// the potential's identity (WP-S1.16 D8, FT-588): a waiver survives a member added to or removed from it.
    #[must_use]
    pub fn keyed_on(mut self, key: &[Target]) -> Finding {
        let ids: Vec<AnyId> = key.iter().map(|t| t.id).collect();
        self.fingerprint = Fingerprint::of(self.rule, &ids, &self.discriminator);
        self
    }

    /// The same finding with a quick fix.
    #[must_use]
    pub fn with_fix(mut self, fix: QuickFix) -> Finding {
        self.fix = Some(fix);
        self
    }

    /// Whether the finding is open (not waived).
    pub fn is_open(&self) -> bool {
        self.state == FindingState::Open
    }

    /// The canonical order (doc 15 §3.9, FT-594): the code, then the printed subject in natural order (then by
    /// bytes), then the message, and only then the fingerprint and the rule. Findings of one code thus print in an
    /// order that follows the project's content: the fingerprint follows record IDs, which are random in a new
    /// project (ID-677), so it decides only between two findings that print alike.
    pub fn order_cmp(&self, other: &Finding) -> std::cmp::Ordering {
        self.code
            .cmp(other.code)
            .then_with(|| natural_cmp(&self.subject, &other.subject))
            .then_with(|| self.subject.cmp(&other.subject))
            .then_with(|| self.message.cmp(&other.message))
            .then_with(|| self.fingerprint.cmp(&other.fingerprint))
            .then_with(|| self.rule.cmp(other.rule))
    }
}

/// Sorts findings in the canonical order ([`Finding::order_cmp`]) and drops exact duplicates.
pub fn sort_findings(findings: &mut Vec<Finding>) {
    findings.sort_by(Finding::order_cmp);
    findings.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> AnyId {
        Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(n))
    }

    #[test]
    fn fingerprint_ignores_target_order_and_duplicates() {
        let a = Fingerprint::of("EW-TERM-002", &[id(1), id(2)], "");
        let b = Fingerprint::of("EW-TERM-002", &[id(2), id(1), id(2)], "");
        assert_eq!(a, b);
        assert_ne!(a, Fingerprint::of("EW-TERM-001", &[id(1), id(2)], ""));
        assert_ne!(a, Fingerprint::of("EW-TERM-002", &[id(1), id(2)], "x"));
        assert_ne!(a, Fingerprint::of("EW-TERM-002", &[id(1)], ""));
    }

    #[test]
    fn fingerprint_is_the_first_128_bits_of_sha256() {
        // SHA-256("ew-fp/1" ‖ "EW-X-001") with no targets and an empty discriminator.
        let fp = Fingerprint::of("EW-X-001", &[], "");
        let digest = Sha256::digest(b"ew-fp/1EW-X-001");
        let mut first = [0u8; 16];
        first.copy_from_slice(&digest[..16]);
        assert_eq!(fp.0, u128::from_be_bytes(first));
        assert_eq!(Fingerprint::parse(&fp.to_string()), Some(fp));
        assert_eq!(fp.to_string().len(), 32);
        assert_eq!(Fingerprint::parse("xyz"), None);
    }

    #[test]
    fn severities_order() {
        assert!(Severity::Info < Severity::Warning && Severity::Warning < Severity::Error);
        assert_eq!(Severity::Error.to_string(), "error");
    }
}
