// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-REL, the release gate's rules (doc 15 §4.8; WP-S1.18 D9): EW-REL-001 "Four eyes", EW-REL-002 "Stale committed
//! computation" and EW-REL-004 "Revision label order". All three are integrity rules (never waivable, never pending)
//! of mode `g`: only the release gate evaluates them ([`gate`](crate::gate)), never `ew check` or the message panel
//! (doc 15 §3.2), so "checker not set" does not turn every open revision's check run red.
//!
//! EW-REL-001 compares the sign-off with the content digest of the saved generation, which the snapshot does not
//! know: the gate hands it in ([`GateInput`]). Evaluated through [`Rule::run`] (no saved generation in hand) the rule
//! reports the role and sign-off conditions only.

use ew_derive::Q;
use ew_derive::ew_schema::{Kind, LabelFormat, PartyRole, Revision, Sha256};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};

/// What the gate knows besides the snapshot (WP-S1.18 D8, D10).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GateInput {
    /// The content digest of the saved generation the gate runs on (`ew_format::content_digest`, doc 15 §5.4); `None`
    /// when no saved generation is in hand, so EW-REL-001 does not compare the sign-off.
    pub content_digest: Option<Sha256>,
}

/// EW-REL-001 "Four eyes" (doc 15 §4.8): integrity, g, error, project.
pub const FOUR_EYES: RuleMeta = RuleMeta {
    id: "EW-REL-001",
    code: "190001",
    pack: "EW-REL",
    title: "Four eyes",
    kind: RuleKind::Integrity,
    modes: Modes::letters("g"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-REL-002 "Stale committed computation" (doc 15 §4.8): integrity, g, error, project.
pub const STALE_COMMITTED: RuleMeta = RuleMeta {
    id: "EW-REL-002",
    code: "190002",
    pack: "EW-REL",
    title: "Stale committed computation",
    kind: RuleKind::Integrity,
    modes: Modes::letters("g"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-REL-004 "Revision label order" (doc 15 §4.8): integrity, p g, error, project.
pub const LABEL_ORDER: RuleMeta = RuleMeta {
    id: "EW-REL-004",
    code: "190004",
    pack: "EW-REL",
    title: "Revision label order",
    kind: RuleKind::Integrity,
    modes: Modes::letters("pg"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// The role's name as the file writes it (`checker`).
fn role(r: &PartyRole) -> String {
    serde_json::to_value(r)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "a role of a newer format".to_owned())
}

/// The open revision (at most one, EW-INV-016; the first in key order otherwise).
fn open_revision(q: &Q) -> Option<std::sync::Arc<Revision>> {
    q.table::<Revision>()
        .iter()
        .find(|(_, r)| r.is_open())
        .map(|(_, r)| std::sync::Arc::clone(r))
}

/// EW-REL-001's findings on the open revision (doc 15 §5.4; FR-REV-005, FR-REV-021): author not set, checker not set,
/// checker equal to the author, sign-off missing or incomplete, and, with a digest in `input`, a sign-off whose content
/// digest is not the saved generation's (content edited after approval). A project without an open revision has
/// nothing to release here. One finding per condition (discriminators `author`, `checker`, `four-eyes`, `sign-off`,
/// `stale`); the sign-off is judged only once the roles are right, since the checker is written with it.
pub fn four_eyes(q: &Q, input: &GateInput) -> Vec<Finding> {
    let Some(r) = open_revision(q) else {
        return Vec::new();
    };
    let label = r.label.as_str();
    let subject = format!("revision {label}");
    let target = [Target::new(Kind::Revision, r.id)];
    let finding =
        |disc: &str, message: String| Finding::new(&FOUR_EYES, target, disc, subject.clone(), message);
    let mut out = Vec::new();
    if r.author.is_none() {
        out.push(finding(
            "author",
            format!("revision {label}: author not set; set it with Revision ▸ Set author"),
        ));
    }
    match (&r.author, &r.checker) {
        (_, None) => out.push(finding(
            "checker",
            format!("revision {label}: checker not set; the checker approves the revision"),
        )),
        (Some(a), Some(c)) if a == c => out.push(finding(
            "four-eyes",
            format!(
                "revision {label}: the checker is the author ({}); four eyes need another party",
                role(c)
            ),
        )),
        _ => {}
    }
    if !out.is_empty() {
        return out;
    }
    let digest = r.sign_off.as_ref().and_then(|s| s.content_digest);
    match (digest, input.content_digest) {
        (None, _) => out.push(finding(
            "sign-off",
            format!("revision {label}: sign-off missing; the checker approves the saved revision"),
        )),
        (Some(signed), Some(saved)) if signed != saved => out.push(finding(
            "stale",
            format!(
                "revision {label}: the sign-off does not match the saved content (edited after approval); approve it \
                 again"
            ),
        )),
        _ => {}
    }
    out
}

/// EW-REL-004's gate half (doc 15 §4.8, §5.2; FR-REV-002): every revision label in the project's format, and the open
/// revision's label after every other one in the format's order (the prevent half is `revision.create`'s and `fields.set
/// label`'s refusal). Withdrawal records repeat the label they withdraw and are passed over; a format of a newer build
/// is not judged. One finding per revision: discriminator `format` or `order`.
pub fn label_order(q: &Q) -> Vec<Finding> {
    let format = q.project().settings.revision_label_format();
    if matches!(format, LabelFormat::Unknown(_)) {
        return Vec::new();
    }
    let table = q.table::<Revision>();
    let current: Vec<&std::sync::Arc<Revision>> = table
        .iter()
        .map(|(_, r)| r)
        .filter(|r| !r.is_withdrawal())
        .collect();
    let mut out = Vec::new();
    for r in &current {
        if format.order_key(r.label.as_str()).is_none() {
            out.push(Finding::new(
                &LABEL_ORDER,
                [Target::new(Kind::Revision, r.id)],
                "format",
                format!("revision {}", r.label),
                format!(
                    "revision {}: the label is not in the project's revision label format {}",
                    r.label,
                    format.pattern()
                ),
            ));
        }
    }
    let open = current.iter().find(|r| r.is_open());
    if let Some((open, key)) = open.and_then(|o| format.order_key(o.label.as_str()).map(|k| (o, k))) {
        let later = current
            .iter()
            .filter(|r| r.id != open.id)
            .filter_map(|r| format.order_key(r.label.as_str()).map(|k| (k, r)))
            .filter(|(k, _)| *k >= key)
            .max_by_key(|(k, r)| (*k, r.id));
        if let Some((_, r)) = later {
            out.push(Finding::new(
                &LABEL_ORDER,
                [Target::new(Kind::Revision, open.id)],
                "order",
                format!("revision {}", open.label),
                format!(
                    "revision {}: the open revision's label does not follow {} of revision {}; revision labels \
                     increase, the open revision last",
                    open.label, r.label, r.id
                ),
            ));
        }
    }
    out
}

/// EW-REL-001 (doc 15 §4.8, §5.4): [`four_eyes`] without a saved generation's digest. The gate evaluates it with one.
#[derive(Clone, Copy, Debug)]
pub struct FourEyes;

impl Rule for FourEyes {
    fn meta(&self) -> &'static RuleMeta {
        &FOUR_EYES
    }

    fn run(&self, q: &Q, _scope: &Scope) -> Vec<Finding> {
        four_eyes(q, &GateInput::default())
    }
}

/// EW-REL-002 (doc 15 §4.8): a committed computation that a release output uses is stale. Registered so that the gate
/// lists it; it reports nothing until committed routes, thermal results and frozen report snapshots exist (WP-S4.x):
/// at MVP no committed computation has an input hash to compare (WP-S1.18 D9).
#[derive(Clone, Copy, Debug)]
pub struct StaleCommitted;

impl Rule for StaleCommitted {
    fn meta(&self) -> &'static RuleMeta {
        &STALE_COMMITTED
    }

    fn run(&self, _q: &Q, _scope: &Scope) -> Vec<Finding> {
        Vec::new()
    }
}

/// EW-REL-004 (doc 15 §4.8, §5.2): [`label_order`]. Its prevent half is the commands' refusal (`CmdError::Prevented`,
/// WP-S1.18 part 1), so [`Rule::prevent`] reports nothing here.
#[derive(Clone, Copy, Debug)]
pub struct LabelOrder;

impl Rule for LabelOrder {
    fn meta(&self) -> &'static RuleMeta {
        &LABEL_ORDER
    }

    fn run(&self, q: &Q, _scope: &Scope) -> Vec<Finding> {
        label_order(q)
    }
}
