// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`Rule`] trait and the rule metadata of doc 15 §2.3: ID, code, pack, kind, modes, severities, scope.

use ew_derive::{PotentialId, Q};
use ew_ids::Id;
use ew_schema::{AnyId, Device, Kind, LogicalNet, Page, PhysicalNet, PlcConfigProject};

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

impl RuleKind {
    /// The catalogue's text (`rules.json`, the rule reference): `safety`, `conflict`, `completeness`, `hygiene`,
    /// `integrity`. Stable: a sign-off's pack digest binds it ([`pack_definition`](crate::pack_definition)).
    pub fn name(self) -> &'static str {
        match self {
            RuleKind::Safety => "safety",
            RuleKind::Conflict => "conflict",
            RuleKind::Completeness => "completeness",
            RuleKind::Hygiene => "hygiene",
            RuleKind::Integrity => "integrity",
        }
    }
}

/// The modes a rule runs in (doc 15 §3.1). The integrity-run, import-run and library-linter modes of the catalogue
/// (n, i, l) are not modelled yet: no implemented rule has them.
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
    /// g: the release gate only (doc 15 §3.2, §4.8; the EW-REL rules): never in an online run or a plain release-mode
    /// run, so `ew check` does not report them; [`gate`](crate::gate) evaluates them.
    pub gate: bool,
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
            gate: false,
        };
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'o' => m.online = true,
                b'f' => m.offline = true,
                b'p' => m.prevent = true,
                b'r' => m.release = true,
                b'x' => m.export = true,
                b'g' => m.gate = true,
                _ => {}
            }
            i += 1;
        }
        m
    }
}

/// Declares the scope kinds once: [`ScopeKind`] with its catalogue name, and [`Scope`], one scope of a kind with the
/// identifier of its record.
macro_rules! scope_kinds {
    ($($(#[$doc:meta])* $kind:ident $(($id:ty))? = $name:literal;)*) => {
        /// The scope kind of a rule: the memo key of `checks(scope)`, not the finding granularity (doc 15 §2.3).
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub enum ScopeKind {
            $($(#[$doc])* $kind,)*
        }

        impl ScopeKind {
            /// The catalogue's text (`rules.json`, the rule reference): `project`, `page`, `strip`, `device`, `cable`,
            /// `potential`, `module`, `cfg`, `lnet`, `pnet`, `station`. Stable: a sign-off's pack digest binds it
            /// ([`pack_definition`](crate::pack_definition)).
            pub fn name(self) -> &'static str {
                match self {
                    $(ScopeKind::$kind => $name,)*
                }
            }
        }

        /// One evaluation scope: the key of the memo family [`CheckOf`](crate::CheckOf) together with a rule ID.
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub enum Scope {
            $($(#[$doc])* $kind$(($id))?,)*
        }

        impl Scope {
            /// The kind of the scope.
            pub fn kind(&self) -> ScopeKind {
                match self {
                    $(Scope::$kind { .. } => ScopeKind::$kind,)*
                }
            }
        }
    };
}

scope_kinds! {
    /// The whole project.
    Project = "project";
    /// One drawn page.
    Page(Id<Page>) = "page";
    /// One terminal strip (a device with the strip aspect).
    Strip(Id<Device>) = "strip";
    /// One device.
    Device(Id<Device>) = "device";
    /// One cable (a device with the cable aspect).
    Cable(Id<Device>) = "cable";
    /// One potential of the potential partition.
    Potential(PotentialId) = "potential";
    /// One PLC module (a device with the `plc_module` aspect; doc 15 §2.3 `module`).
    Module(Id<Device>) = "module";
    /// One PLC configuration project (doc 15 §2.3 `cfg`, requested by doc 10 §3.10).
    Cfg(Id<PlcConfigProject>) = "cfg";
    /// One logical bus net (doc 15 §2.3 `lnet`).
    Lnet(Id<LogicalNet>) = "lnet";
    /// One physical bus net (doc 15 §2.3 `pnet`).
    Pnet(Id<PhysicalNet>) = "pnet";
    /// One PLC station (a device with the `plc_station` aspect; doc 15 §2.3 `station`).
    Station(Id<Device>) = "station";
}

/// The static description of a shipped rule (doc 15 §2.3, §4; ADR-0019 decision 1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RuleMeta {
    /// The rule ID, `<OWNER>-<FAMILY>-<NNN>` (doc 15 §2.2), for example `EW-TERM-002`.
    pub id: &'static str,
    /// The key of the rule's message in the rule messages, `rule-<id in lower case>` (ADR-0019 amendment 4): its
    /// pattern words a finding, its `.title` and `.hint` the rule.
    pub message: &'static str,
    /// The six-digit code `CCCNNN` (doc 15 §2.2), for example `001002`.
    pub code: &'static str,
    /// The English title, the catalogue's; the en-GB `.title` attribute of the rule's Fluent message
    /// ([`crate::messages`]) is the same text, which a test holds.
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

/// The families of doc 15 §2.2 with shipped rules: the family of a rule ID, its pack and the offset of its codes in
/// the pack's class (EW-SAF-001 is code `110101`).
pub(crate) const FAMILIES: &[(&str, &str, u32)] = &[
    ("TERM", "EW-TERM", 0),
    ("POT", "EW-POT", 0),
    ("PLC", "EW-PLC", 0),
    ("SAF", "EW-PLC", 100),
    ("PRC", "EW-PLC", 200),
    ("BUS", "EW-BUS", 0),
    ("IOL", "EW-BUS", 100),
    ("RDS", "EW-IEC81346", 0),
    ("NUM", "EW-IEC81346", 100),
    ("CORE", "EW-CORE", 0),
    ("REL", "EW-REL", 0),
    ("LAY", "EW-LAYOUT", 0),
    ("FL", "EW-FL", 0),
    ("IMP", "EW-IMP", 0),
];

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
    /// The pack, for example `EW-TERM` or `EW-IEC81346`: the pack of the ID's family ([`FAMILIES`]); empty for a
    /// family without a pack.
    pub fn pack(&self) -> &'static str {
        let family = self.id.split('-').nth(1).unwrap_or("");
        FAMILIES
            .iter()
            .find(|(f, _, _)| *f == family)
            .map_or("", |(_, pack, _)| pack)
    }

    /// The rule's page of the user guide (FR-CHK-015): `reference/rules/<id in lower case>`, a path below the book's
    /// source without its extension, which `cargo xtask docs-gen` writes from the catalogue.
    pub fn docs(&self) -> String {
        format!("reference/rules/{}", self.id.to_lowercase())
    }

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
