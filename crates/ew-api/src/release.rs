// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The release gate and the sign-off (doc 15 §5.4, §5.5; WP-S1.18 part 3, D8 to D10): [`Session::release_gate`] runs
//! the gate on the saved generation, [`Session::approve`] records the checker's sign-off bound to that generation's
//! content digest.
//!
//! **Saved state only** (D8). Both need a project directory, no unsaved edits, no incomplete save and a manifest whose
//! hashes are authoritative: the content digest ([`ew_store::ew_format::content_digest`]) is then a pure function of
//! one generation, and the snapshot the gate checks is that generation. The gate evaluates every rule cold (a fresh
//! `Q` over the snapshot, not the session's memo store), so no memoised value can be stale there (doc 15 §4.8).
//!
//! **One path for the sign-off.** `revision.approve` carries the gate's digest, scheme and pack versions as parameters,
//! so the session submits it only through [`Session::approve`]: [`Session::execute`] and [`Session::preview`] refuse
//! it. Its exposure is `HumanOnly`: a command log, a script, MCP and a plugin cannot reach it (ADR-0008 amendment 3).
//! The release itself (`revision.release`, the as-built archive) is WP-S1.18 part 4.

use std::fmt;

use ew_commands::Command as _;
use ew_commands::commands::{ApproveRevision, ApprovedPack};
use ew_derive::ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{Date, PartyRole, Revision, Waiver};
use ew_store::ew_format::{ContentDigestError, content_digest};

use crate::{CmdError, Receipt, Session, Sha256, checks};

/// The rule ID of the four-eyes rule, the one gate error an approval may leave open (doc 15 §5.4).
pub const FOUR_EYES_RULE: &str = "EW-REL-001";

/// The result of the release gate on the saved generation (doc 15 §5.5; WP-S1.18 D10).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Gate {
    /// The open revision the gate judged.
    pub revision: Id<Revision>,
    /// Its label.
    pub label: String,
    /// Its author's party role, when set.
    pub author: Option<PartyRole>,
    /// The saved generation.
    pub generation: u64,
    /// The content digest of that generation (doc 15 §5.4).
    pub content_digest: Sha256,
    /// The gate's report: the release scheme in release mode, EW-REL and the integrity findings of opening.
    pub report: checks::CheckReport,
    /// The packs the gate evaluated (doc 15 §3.9).
    pub packs: Vec<checks::PackVersion>,
    /// The waivers of safety-kind rules in force, which a checker acknowledges (doc 15 §5.4).
    pub safety_waivers: Vec<Id<Waiver>>,
}

impl Gate {
    /// The release's refusal list: every open error of the gate (doc 15 §4.8).
    pub fn blockers(&self) -> Vec<&checks::Finding> {
        checks::blockers(&self.report)
    }

    /// Whether the gate lets the release through: no blocker.
    pub fn passes(&self) -> bool {
        self.blockers().is_empty()
    }

    /// The blockers an approval may not leave open: every one except EW-REL-001's (doc 15 §5.4).
    pub fn approval_blockers(&self) -> Vec<&checks::Finding> {
        self.blockers()
            .into_iter()
            .filter(|f| f.rule != FOUR_EYES_RULE)
            .collect()
    }
}

/// Why the gate could not run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GateError {
    /// The session has no project directory: there is no saved generation.
    NoProject,
    /// The session has edits that are not saved: save first (the digest is the saved generation's).
    Unsaved,
    /// A save is incomplete (its roll-forward is pending).
    IncompleteSave,
    /// The project has no usable manifest yet: save it first.
    NoManifest,
    /// The manifest's hashes are not authoritative (a merged project).
    Digest(ContentDigestError),
    /// The project has no open revision.
    NoOpenRevision {
        /// The label asked for.
        label: String,
    },
    /// The label is not the open revision's.
    NotTheOpenRevision {
        /// The label asked for.
        label: String,
        /// The open revision's label.
        open: String,
    },
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateError::NoProject => f.write_str("the project is not saved in a project directory"),
            GateError::Unsaved => f.write_str("the project has unsaved edits; save it first"),
            GateError::IncompleteSave => f.write_str("a save is incomplete; wait for it to complete"),
            GateError::NoManifest => f.write_str("the project has no manifest yet; save it first"),
            GateError::Digest(e) => e.fmt(f),
            GateError::NoOpenRevision { label } => {
                write!(
                    f,
                    "revision {label} is not open: the project has no open revision"
                )
            }
            GateError::NotTheOpenRevision { label, open } => {
                write!(
                    f,
                    "revision {label} is not the open revision; the open revision is {open}"
                )
            }
        }
    }
}

impl std::error::Error for GateError {}

/// What a checker approves (doc 15 §5.4): the open revision's label (typed to confirm), the checker's party role, the
/// date, and the safety-kind waivers acknowledged.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Approval {
    /// The open revision's label.
    pub label: String,
    /// The checker's party role; never the author's.
    pub checker: PartyRole,
    /// The date of the approval (the front-end reads the clock).
    pub date: Date,
    /// The waivers of safety-kind rules the checker acknowledges ([`Gate::safety_waivers`] lists them).
    pub waivers_ack: Vec<Id<Waiver>>,
}

/// Why an approval was refused (doc 15 §5.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ApproveError {
    /// The gate could not run.
    Gate(GateError),
    /// The session is read-only: approval needs the write lock.
    ReadOnly,
    /// The open revision has no author: four eyes compare two parties.
    NoAuthor {
        /// The revision's label.
        label: String,
    },
    /// The checker is the author's party (D-34, FR-REV-005).
    FourEyes {
        /// The role.
        role: PartyRole,
    },
    /// The gate has open errors besides EW-REL-001's, listed.
    Blocked(Vec<checks::Finding>),
    /// Safety-kind waivers in force that the approval does not acknowledge, listed.
    Unacknowledged(Vec<Id<Waiver>>),
    /// The command was refused (its exposure, a changed snapshot).
    Command(CmdError),
}

impl fmt::Display for ApproveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApproveError::Gate(e) => e.fmt(f),
            ApproveError::ReadOnly => {
                f.write_str("the project is open read-only; approval needs the write lock")
            }
            ApproveError::NoAuthor { label } => write!(
                f,
                "revision {label} has no author; four eyes compare two parties, so set the author first (Revision ▸ \
                 Set author)"
            ),
            ApproveError::FourEyes { role } => write!(
                f,
                "the checker is the author's party ({}); four eyes need another party",
                role_name(role)
            ),
            ApproveError::Blocked(findings) => write!(
                f,
                "the release gate has {} unwaived error(s) besides {FOUR_EYES_RULE}; correct or waive them first",
                findings.len()
            ),
            ApproveError::Unacknowledged(ids) => {
                let ids: Vec<String> = ids.iter().map(ToString::to_string).collect();
                write!(f, "unacknowledged safety waivers: {}", ids.join(", "))
            }
            ApproveError::Command(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ApproveError {}

/// The name of a party role as the file writes it (`checker`).
pub fn role_name(role: &PartyRole) -> String {
    serde_json::to_value(role)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "a role of a newer format".to_owned())
}

impl Session {
    /// The open revision's label, if the project has an open revision.
    pub fn open_revision_label(&self) -> Option<String> {
        self.query()
            .table::<Revision>()
            .iter()
            .find(|(_, r)| r.is_open())
            .map(|(_, r)| r.label.as_str().to_owned())
    }

    /// The released revision labelled `label` (not a withdrawal record), which `revision.withdraw` names
    /// (`ew revision withdraw <label>`).
    pub fn released_revision(&self, label: &str) -> Option<Id<Revision>> {
        self.query()
            .table::<Revision>()
            .iter()
            .find(|(_, r)| {
                r.state == ew_schema::RevisionState::Released
                    && !r.is_withdrawal()
                    && r.label.as_str() == label
            })
            .map(|(_, r)| r.id)
    }

    /// The content digest of the saved generation (doc 15 §5.4; WP-S1.18 D8) with that generation: refused when the
    /// session has unsaved edits or no saved generation.
    ///
    /// # Errors
    ///
    /// [`GateError`]: no project directory, unsaved edits, an incomplete save, no manifest, pending hashes.
    pub fn content_digest(&self) -> Result<(Sha256, u64), GateError> {
        let Some(p) = self.persistence.as_ref() else {
            return Err(GateError::NoProject);
        };
        if self.is_dirty() || !p.unsaved.is_empty() || p.recovered {
            return Err(GateError::Unsaved);
        }
        if p.store.is_incomplete() {
            return Err(GateError::IncompleteSave);
        }
        let manifest = p.store.manifest().ok_or(GateError::NoManifest)?;
        let digest = content_digest(manifest).map_err(GateError::Digest)?;
        Ok((Sha256(digest), manifest.generation))
    }

    /// The release gate on the saved generation for the open revision `label` (doc 15 §5.5; WP-S1.18 D10): the
    /// project's release scheme (`pre-delivery` by default) in release mode, cold, with the gate-only EW-REL rules and
    /// the generation's content digest, joined by the integrity findings of opening the project (`integrity`, as `ew
    /// check` joins them). A read-only session may run it; approving and releasing need the write lock.
    ///
    /// # Errors
    ///
    /// [`GateError`]: no saved generation in hand, or `label` is not the open revision's.
    pub fn release_gate(&self, label: &str, integrity: &[checks::IntegrityItem]) -> Result<Gate, GateError> {
        let (digest, generation) = self.content_digest()?;
        let q = self.query_over(self.engine.model());
        let table = q.table::<Revision>();
        let Some(open) = table.iter().map(|(_, r)| r).find(|r| r.is_open()) else {
            return Err(GateError::NoOpenRevision {
                label: label.to_owned(),
            });
        };
        if open.label.as_str() != label {
            return Err(GateError::NotTheOpenRevision {
                label: label.to_owned(),
                open: open.label.as_str().to_owned(),
            });
        }
        let scheme = self
            .project_check_scheme(checks::CheckMode::Release)
            .unwrap_or(&checks::PRE_DELIVERY);
        let input = checks::GateInput {
            content_digest: Some(digest),
        };
        let report = checks::gate(&q, scheme, &input).with_integrity(integrity.iter().cloned());
        let packs = checks::pack_versions(&report);
        let safety_waivers = checks::safety_waivers(&report)
            .iter()
            .filter_map(|w| w.parse::<Id<Waiver>>().ok())
            .collect();
        Ok(Gate {
            revision: open.id,
            label: label.to_owned(),
            author: open.author.clone(),
            generation,
            content_digest: digest,
            report,
            packs,
            safety_waivers,
        })
    }

    /// Records the checker's sign-off on the open revision (doc 15 §5.4; WP-S1.18 D9): after the release gate on the
    /// saved generation, `revision.approve` with the gate's content digest, scheme and pack versions, from the
    /// session's origin. Returns the gate it approved; the session is then dirty with the sign-off (in
    /// `revisions.jsonl`, which is not content, so saving it leaves the digest as it is).
    ///
    /// Refused, in this order: no saved generation or a label that is not the open revision's ([`GateError`]); a
    /// read-only session; an open revision without an author; the checker's role equal to the author's; open gate
    /// errors other than EW-REL-001's (listed); a safety-kind waiver in force that `approval.waivers_ack` does not
    /// list; the command's own refusal (its exposure: `HumanOnly`).
    ///
    /// # Errors
    ///
    /// [`ApproveError`].
    pub fn approve(
        &mut self,
        approval: &Approval,
        integrity: &[checks::IntegrityItem],
    ) -> Result<(Gate, Receipt), ApproveError> {
        let gate = self
            .release_gate(&approval.label, integrity)
            .map_err(ApproveError::Gate)?;
        if !matches!(
            self.store().map(ew_store::Store::access),
            Some(crate::Access::Edit)
        ) {
            return Err(ApproveError::ReadOnly);
        }
        let Some(author) = &gate.author else {
            return Err(ApproveError::NoAuthor {
                label: gate.label.clone(),
            });
        };
        if *author == approval.checker {
            return Err(ApproveError::FourEyes {
                role: approval.checker.clone(),
            });
        }
        let blocked: Vec<checks::Finding> = gate.approval_blockers().into_iter().cloned().collect();
        if !blocked.is_empty() {
            return Err(ApproveError::Blocked(blocked));
        }
        let missing: Vec<Id<Waiver>> = gate
            .safety_waivers
            .iter()
            .filter(|w| !approval.waivers_ack.contains(w))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(ApproveError::Unacknowledged(missing));
        }
        let ident = |s: &str| {
            s.parse::<Ident>()
                .map_err(|e| ApproveError::Command(CmdError::internal(e.to_string())))
        };
        let cmd = ApproveRevision {
            revision: gate.revision,
            checker: approval.checker.clone(),
            date: approval.date,
            scheme: ident(gate.report.scheme)?,
            packs: gate
                .packs
                .iter()
                .map(|p| {
                    Ok(ApprovedPack {
                        id: ident(p.id)?,
                        version: ident(p.version)?,
                        sha256: Sha256(p.sha256),
                    })
                })
                .collect::<Result<_, ApproveError>>()?,
            waivers_ack: approval.waivers_ack.clone(),
            content_digest: gate.content_digest,
        };
        let before = self.engine.model().clone();
        let result = self.engine.execute(&cmd);
        self.committed(&before, ApproveRevision::NAME);
        let receipt = result.map_err(ApproveError::Command)?;
        Ok((gate, receipt))
    }

    /// Refuses `revision.approve` outside [`Session::approve`] (its parameters are the gate's): with the exposure error
    /// for an origin its exposure refuses, else as rejected.
    pub(crate) fn approval_only(&self, name: &str, origin: &crate::Origin) -> Result<(), CmdError> {
        if name != ApproveRevision::NAME {
            return Ok(());
        }
        let exposure = ApproveRevision::meta().exposure;
        if !exposure.allows(origin) {
            return Err(CmdError::Exposure {
                command: ApproveRevision::NAME.into(),
                exposure,
                origin: origin.clone(),
            });
        }
        Err(CmdError::Rejected {
            source: ApproveRevision::NAME.to_owned(),
            reason: "the sign-off is recorded only by the approval on the saved generation (Revision ▸ Approve, ew \
                     revision approve), which runs the release gate first (WP-S1.18 D9)"
                .to_owned(),
        })
    }
}
