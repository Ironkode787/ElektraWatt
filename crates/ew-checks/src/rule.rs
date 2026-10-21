// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`Rule`] trait and the rule metadata of doc 15 §2.3: ID, code, pack, kind, modes, severities, scope.

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{AnyId, Device, Kind, Page};
use ew_derive::{PotentialId, Q};

use crate::finding::{Finding, Severity};

/// The kind of a rule (doc 15 §2.3): drives pending display and waiver limits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum RuleKind {
    /// A defect that endangers people or equipment; fingerprint waivers only.
    Safety,
    /// Two pieces of data contradict each other.
    Conflict,
    /// Data is missing.
    Completeness,
    /// Style and tidiness.
    Hygiene,
    /// About the stored project, the check run or the release workflow; never waivable, never pending.
    Integrity,
}

/// The modes a rule runs in (doc 15 §3.1). The gate-only, integrity-run, import-run and library-linter
/// modes of the catalogue (g, n, i, l) are not modelled yet: no implemented rule has them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Modes {
    /// o: memoised `checks(scope)` on every commit.
    pub online: bool,
    /// f: a check job on a fork.
    pub offline: bool,
    /// p: the commit validator on the candidate snapshot (ADR-0009).
    pub prevent: bool,
    /// r: the release gate and `ew check`.
    pub release: bool,
    /// x: before an exporter writes.
    pub export: bool,
}

impl Modes {
    /// The modes from the catalogue letters (`"por"` is prevent, online, release). Other characters are
    /// ignored, so the catalogue's spaced form `"p o r"` works too.
    pub const fn letters(s: &str) -> Modes {
        let b = s.as_bytes();
        let mut m = Modes {
            online: false,
            offline: false,
            prevent: false,
            release: false,
            export: false,
        };
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'o' => m.online = true,
                b'f' => m.offline = true,
                b'p' => m.prevent = true,
                b'r' => m.release = true,
                b'x' => m.export = true,
                _ => {}
            }
            i += 1;
        }
        m
    }
}

/// The scope kind of a rule: the memo key of `checks(scope)`, not the finding granularity (doc 15 §2.3).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ScopeKind {
    /// The whole project.
    Project,
    /// One drawn page.
    Page,
    /// One terminal strip (a device with the strip aspect).
    Strip,
    /// One device.
    Device,
    /// One cable (a device with the cable aspect).
    Cable,
    /// One potential of the potential partition.
    Potential,
}

/// One evaluation scope: the key of the memo family [`CheckOf`](crate::CheckOf) together with a rule ID.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Scope {
    /// The whole project.
    Project,
    /// A drawn page.
    Page(Id<Page>),
    /// A strip device.
    Strip(Id<Device>),
    /// A device.
    Device(Id<Device>),
    /// A cable device.
    Cable(Id<Device>),
    /// A potential.
    Potential(PotentialId),
}

impl Scope {
    /// The kind of the scope.
    pub fn kind(&self) -> ScopeKind {
        match self {
            Scope::Project => ScopeKind::Project,
            Scope::Page(_) => ScopeKind::Page,
            Scope::Strip(_) => ScopeKind::Strip,
            Scope::Device(_) => ScopeKind::Device,
            Scope::Cable(_) => ScopeKind::Cable,
            Scope::Potential(_) => ScopeKind::Potential,
        }
    }
}

/// The static description of a shipped rule (doc 15 §2.3, §4; ADR-0019 decision 1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RuleMeta {
    /// The rule ID, `<OWNER>-<FAMILY>-<NNN>` (doc 15 §2.2), for example `EW-TERM-002`.
    pub id: &'static str,
    /// The six-digit code `CCCNNN` (doc 15 §2.2), for example `001002`.
    pub code: &'static str,
    /// The pack, for example `EW-TERM` or `EW-IEC81346`.
    pub pack: &'static str,
    /// The English title (the `.title` attribute of the rule's Fluent message, until the messages ship).
    pub title: &'static str,
    /// The kind.
    pub kind: RuleKind,
    /// The modes.
    pub modes: Modes,
    /// The default severity (online and offline).
    pub severity: Severity,
    /// The release severity where it differs (`a/b` in the catalogue): release mode and `ew check`.
    pub release_severity: Option<Severity>,
    /// The scope kind.
    pub scope: ScopeKind,
    /// The rules this rule supersedes for overlapping targets (doc 15 §3.6).
    pub supersedes: &'static [&'static str],
    /// The release whose pack ships the rule (the catalogue's release column, doc 15 §4.1): a 1.0 rule of a
    /// pack that is complete at 1.0, such as EW-CORE-065, belongs to the pack's 1.0 content.
    pub release: Release,
}

/// The release column of the rule catalogue (doc 15 §4, §4.1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Release {
    /// The MVP content of the pack.
    Mvp,
    /// The pack's 1.0 content (the pack is complete at 1.0).
    V1_0,
}

impl Release {
    /// The catalogue's text: `MVP`, `1.0`.
    pub fn name(self) -> &'static str {
        match self {
            Release::Mvp => "MVP",
            Release::V1_0 => "1.0",
        }
    }
}

impl RuleMeta {
    /// Whether a waiver may cover the rule's findings (`false` for every rule of kind `integrity`).
    pub fn waivable(&self) -> bool {
        self.kind != RuleKind::Integrity
    }
}

/// The keys a batch put, as `(kind, id)` pairs sorted by kind then ID: the `touched` argument of a prevent
/// evaluation (the model's `AnyKey`, which this crate does not name).
pub type Touched = [(Kind, AnyId)];

/// A rule implementation registered as `rust:<name>@<version>` (ADR-0019 decision 2).
///
/// `run` is a pure function of the tracked-read context: it reads through `q` only (no raw model read, no
/// clock, no environment), uses no floating point and no hash-map order, and returns the same findings for
/// the same snapshot. The engine memoises it per `(rule, scope)` through [`CheckOf`](crate::CheckOf), so it
/// re-runs only when what it read changed (doc 15 §3.3).
pub trait Rule: Send + Sync + 'static {
    /// The rule's metadata.
    fn meta(&self) -> &'static RuleMeta;

    /// The findings of the rule in one scope of its [`ScopeKind`], each with the rule's default severity
    /// or a finding-specific one (for example info for a frozen tag). The engine sorts them.
    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding>;

    /// The prevent-mode evaluation (ADR-0009 decision 3): the violations that the batch from `before` to
    /// `after` creates among the `touched` keys. A violation the model already held is not reported here
    /// (a lenient import may have committed it; online and release mode report it). Only rules whose
    /// modes include `prevent` are asked; the default reports nothing.
    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let _ = (before, after, touched);
        Vec::new()
    }
}
