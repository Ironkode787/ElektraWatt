// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Revisions (doc 03 §18, doc 15 §5; FR-REV-001, FR-REV-012): the `revision` record (rank 60,
//! `revisions.jsonl`).
//!
//! Implemented members are the two cases of FT-165: an open revision with label, date and state, and a
//! historic released revision without an archive (no release record), and on a withdrawal record the ID
//! of the revision it withdraws (`withdraws: RevisionId?`, doc 03 §18). WP-S1.18 part 1 (D2, FT-302) types
//! `reason` (on a withdrawal record), `release` ([`ReleaseRecord`]) and `sign_off` ([`SignOff`]), appended
//! after the earlier members; `purpose` (1.0) stays in `extra`, so a record written by a newer build keeps it
//! through a load and a save. Typing members of the reserved kind is additive within format 0.1 (ADR-0011
//! amendment 4, decision log ID-334).
//!
//! **Typed from `extra`.** A build before part 1 kept these members in `extra` and wrote them after the known
//! members in byte-wise order of their names, nested objects too. The typed members therefore follow that order
//! (`reason`, `release`, `sign_off`, and the members of each typed object in byte order of their names), and
//! every member of a typed object is optional: a line such a build wrote, with a partial or a complete payload,
//! loads into the typed members and writes back byte for byte. Completeness is the release gate's question
//! (WP-S1.18 part 3), not the reader's.
//!
//! **The label format** ([`LabelFormat`], `ProjectSettings.revision_label`, FR-REV-002): the order and the next
//! label of each of the three forms; absent means `v_major_minor`, the golden form.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::json::{Extra, de_extra, is_v1, v1};
use crate::proxy::{IdentS, MlTextS, UuidS};
use crate::track::open_enum;
use crate::value::{Date, Sha256, SourceRef};
use crate::{Page, PartyRole, SchemaError, Waiver};

/// One managed project revision (doc 03 §18, rank 60, `revisions.jsonl`; FR-REV-001).
///
/// The title block's "Revision" cell prints the current revision: the open one if there is one, else the
/// latest released one (D-05, FR-REV-003). A released revision is immutable (FR-REV-009); one without a
/// release record is historic (released outside EW, so it has no as-built archive and no page digests).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Revision {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Revision>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The label in the project's one format ("V2.0"; FR-REV-002).
    #[schemars(with = "IdentS")]
    pub label: Ident,
    /// The revision date; an open revision may have none yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<Date>,
    /// The author, as the role of a project party (doc 03 `Party`: form cells and revisions refer to
    /// parties by role, FR-PRJ-003).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<PartyRole>,
    /// The checker, as the role of a project party; never the author's role (D-34, FR-REV-005).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checker: Option<PartyRole>,
    /// What the revision changes ("Changes on 101 and 106").
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
    /// The state.
    pub state: RevisionState,
    /// The pages the revision declares as changed, each with an optional description (FR-REV-012), in
    /// authored order, at most one note per page. They decide the changed pages of a release whose
    /// predecessor has no page digests (doc 15 §5.6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_notes: Vec<PageNote>,
    /// On a withdrawal record: the released revision it withdraws (doc 03 §18, FR-REV-009). The withdrawn
    /// record itself stays as it was (EW-INV-020); derivations leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub withdraws: Option<Id<Revision>>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// On a withdrawal record: why the revision is withdrawn (doc 15 §5.10; written by `revision.withdraw`).
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub reason: MlText,
    /// The release record: written in the atomic release step together with the as-built archive (doc 05
    /// §14.1, FR-REV-006). A historic released revision has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<ReleaseRecord>,
    /// The checker's sign-off, bound to the content digest of the saved project (doc 15 §5.4, FR-REV-021):
    /// "signed off" is an open revision whose sign-off matches the current content digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_off: Option<SignOff>,
    /// Unknown members (`purpose` until it is typed) and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl Revision {
    /// Whether the revision is the open one.
    pub fn is_open(&self) -> bool {
        self.state == RevisionState::Open
    }

    /// Whether the revision is a withdrawal record (state `withdrawn`, FR-REV-009).
    pub fn is_withdrawal(&self) -> bool {
        self.state == RevisionState::Withdrawn
    }
}

/// The stored state of a revision (doc 03 §4.2, doc 15 §5.1). "Signed off" is not a state: it is an open
/// revision whose sign-off matches the current content digest.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "RevisionState")]
pub enum RevisionState {
    /// The revision that collects the current edits; at most one (EW-INV-016).
    Open,
    /// Released: by `revision.release`, or historic (released before the project came to EW). Immutable.
    Released,
    /// A withdrawal record: it names the released revision it withdraws (`withdraws`, FR-REV-009).
    Withdrawn,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One page a revision declares as changed (FR-REV-012).
///
/// `page` is not a link (`ew-model` lists it among the fields that are not links): a released revision
/// is immutable, so its notes keep naming a page that a later revision deletes, and a note on a page that
/// does not exist selects nothing.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PageNote {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// What changed on the page.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Content (doc 15 §5.3, §5.4; WP-S1.18 D3, D8)

/// The project files that are not content (doc 15 §5.4; WP-S1.18 D8): the content digest leaves them out, so an
/// edit of them neither opens the next revision (`revision.ensure_open`, D3) nor voids a sign-off. Revision records
/// (their page notes included) and transmittals live in `revisions.jsonl`, comments in `comments.jsonl`; view state
/// (`.ew/`) is never in the manifest.
pub const NOT_CONTENT_FILES: [&str; 2] = ["revisions.jsonl", "comments.jsonl"];

/// Whether a record of `kind` is content: whether its file is covered by the content digest (every manifest file
/// except [`NOT_CONTENT_FILES`]; the project row, library copies and waivers included). This is the set of content
/// edits that `revision.ensure_open` (WP-S1.18 D3) and the stale sign-off (D9) share.
///
/// ```
/// use ew_schema::{Kind, is_content_kind};
///
/// assert!(is_content_kind(Kind::Device) && is_content_kind(Kind::Project) && is_content_kind(Kind::Waiver));
/// assert!(!is_content_kind(Kind::Revision) && !is_content_kind(Kind::Comment));
/// ```
pub fn is_content_kind(kind: crate::Kind) -> bool {
    !matches!(kind.file_rule(), crate::FileRule::RootFile(file) if NOT_CONTENT_FILES.contains(&file))
}

// ---------------------------------------------------------------------------------------------------------
// Sign-off and release record (doc 03 §18, doc 15 §5.4; WP-S1.18 D2)

/// The checker's sign-off on the open revision (doc 15 §5.4, FR-REV-005, FR-REV-021), written by
/// `revision.approve` and copied into [`ReleaseRecord::approval`] at release (FR-REV-020).
///
/// Every member is optional, so a partial sign-off written while the member was untyped loads and writes back
/// as it is; the release gate (EW-REL-001) judges whether it is complete and matches the current content
/// digest. Members in byte order of their names, the order of the untyped form.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct SignOff {
    /// The checker, as the role of a project party; never the revision's author (D-34).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checker: Option<PartyRole>,
    /// The content digest the checker approved: SHA-256 over the sorted `(path, sha256)` entries of the saved
    /// manifest, without `revisions.jsonl` and `comments.jsonl` (doc 15 §5.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<Sha256>,
    /// The date of the approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<Date>,
    /// The rule packs of the release-mode run the checker saw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packs: Option<Vec<PackVersion>>,
    /// The check scheme of that run (`pre-delivery`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub scheme: Option<Ident>,
    /// The waivers of safety-kind rules the checker acknowledged, by ID. Not links: a sign-off is history, and
    /// a waiver change voids it anyway (`waivers.jsonl` is in the content digest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<UuidS>>")]
    pub waivers_ack: Option<Vec<Id<Waiver>>>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One rule pack of a check run: its ID, its version and the SHA-256 of its definition (doc 15 §3.9). Every
/// member is optional; members in byte order of their names.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PackVersion {
    /// The pack ID (`EW-CORE`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub id: Option<Ident>,
    /// The SHA-256 of the pack's definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<Sha256>,
    /// The pack version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub version: Option<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The release record of a revision released by ElektraWatt (doc 03 §18; FR-REV-006, FR-REV-020, FR-REV-022),
/// written in the same atomic step as the as-built archive (doc 05 §14.1).
///
/// Every member is optional, so a partial record written while the member was untyped loads and writes back as
/// it is; members in byte order of their names, the order of the untyped form.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ReleaseRecord {
    /// The sign-off, copied at release (FR-REV-020).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<SignOff>,
    /// The as-built archive (ADR-0013).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<ArchiveRef>,
    /// The pages the release changed, in page order (frozen; doc 15 §5.6). Not links: a released revision is
    /// immutable, so it keeps naming a page that a later revision deletes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<UuidS>>")]
    pub changed_pages: Option<Vec<Id<Page>>>,
    /// The content digest at the released generation, after the freeze.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<Sha256>,
    /// The derivation profile of the release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub derivation_profile: Option<Ident>,
    /// The released generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    /// The git tag `ew/rev/<label>`, when the project is a git repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub git_tag: Option<Ident>,
    /// The digest of every page at the release, by page ID (doc 15 §5.6; WP-S1.18 D6). The keys are not links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<BTreeMap<String, Digest128>>")]
    pub page_digests: Option<BTreeMap<Id<Page>, Digest128>>,
    /// The release date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_on: Option<Date>,
    /// The rule packs of the gate's run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_packs: Option<Vec<PackVersion>>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The as-built archive of a release: its file name and its SHA-256 (ADR-0013, doc 05 §14.2). Every member is
/// optional; members in byte order of their names.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ArchiveRef {
    /// The file name, `<label>.ewarchive`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The SHA-256 of the archive file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<Sha256>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A 128-bit digest, written as 32 lower-case hexadecimal digits: a page digest, the first 128 bits of a
/// SHA-256 (doc 15 §5.6; WP-S1.18 D6).
///
/// ```
/// use ew_schema::Digest128;
///
/// let d: Digest128 = serde_json::from_str("\"0123456789abcdef0123456789abcdef\"").unwrap();
/// assert_eq!(d.0, 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef);
/// assert_eq!(serde_json::to_string(&d).unwrap(), "\"0123456789abcdef0123456789abcdef\"");
/// assert!(serde_json::from_str::<Digest128>("\"0123456789ABCDEF0123456789ABCDEF\"").is_err());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Digest128(pub u128);

impl fmt::Display for Digest128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

impl FromStr for Digest128 {
    type Err = SchemaError;

    fn from_str(s: &str) -> Result<Digest128, SchemaError> {
        let b = s.as_bytes();
        if b.len() != 32 || !b.iter().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(crate::track::invalid(format!(
                "`{s}` is not a 128-bit digest (32 lower-case hexadecimal digits)"
            )));
        }
        u128::from_str_radix(s, 16)
            .map(Digest128)
            .map_err(crate::track::invalid)
    }
}

impl Serialize for Digest128 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Digest128 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = Cow::<str>::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Digest128 {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Digest128")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A 128-bit digest as 32 lower-case hexadecimal digits (a page digest, doc 15 §5.6).",
            "type": "string",
            "pattern": "^[0-9a-f]{32}$"
        })
    }
}

// ---------------------------------------------------------------------------------------------------------
// The label format (doc 15 §5.2, FR-REV-002; WP-S1.18 D2)

/// The project's one revision label format (doc 15 §5.2, FR-REV-002), `ProjectSettings.revision_label`;
/// absent means [`LabelFormat::VMajorMinor`], the golden form, so every file written before the setting existed
/// reads as it did.
///
/// Labels increase in the format's own order ([`LabelFormat::order_key`]): `V2.0 < V10.0`, `09 < 10`, `Z < AA`.
/// That is not the natural (text) order everywhere: natural order puts `AA` before `Z`, so every ranking of
/// revisions orders by this key (labels outside the format, which it does not order, by their natural order).
/// The proposal for the next revision bumps the major part ([`LabelFormat::next`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "LabelFormat")]
pub enum LabelFormat {
    /// `V{major}.{minor}`: V1.0, V2.0, V2.1 (decimal parts without leading zeros).
    VMajorMinor,
    /// `{nn}`: 00, 01 … 99, 100 (decimal, at least two digits: one leading zero below ten).
    TwoDigit,
    /// `{A}`: A, B … Z, AA, AB … (the letters without I and O, counted like spreadsheet columns).
    Letter,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The letters of [`LabelFormat::Letter`]: A to Z without I and O.
const LABEL_LETTERS: &[u8; 24] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

/// A decimal number without leading zeros (`0` itself allowed), within `u64`.
fn plain_decimal(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) || (b.len() > 1 && b[0] == b'0') {
        return None;
    }
    s.parse().ok()
}

impl LabelFormat {
    /// The format's pattern as doc 15 §5.2 writes it (`V{major}.{minor}`), for messages; a newer format's value
    /// as its JSON text.
    pub fn pattern(&self) -> Cow<'static, str> {
        match self {
            LabelFormat::VMajorMinor => Cow::Borrowed("V{major}.{minor}"),
            LabelFormat::TwoDigit => Cow::Borrowed("{nn}"),
            LabelFormat::Letter => Cow::Borrowed("{A}"),
            LabelFormat::Unknown(v) => Cow::Owned(v.to_string()),
        }
    }

    /// The position of `label` in the format's order: `(major, minor)` for `V{major}.{minor}`, `(n, 0)` for
    /// `{nn}`, and `(count, 0)` for `{A}` (A = 1, Z = 24, AA = 25). `None` when the label is not in the format,
    /// and for a newer format, whose order this build does not know.
    pub fn order_key(&self, label: &str) -> Option<(u64, u64)> {
        match self {
            LabelFormat::VMajorMinor => {
                let (major, minor) = label.strip_prefix('V')?.split_once('.')?;
                Some((plain_decimal(major)?, plain_decimal(minor)?))
            }
            LabelFormat::TwoDigit => {
                let b = label.as_bytes();
                if b.len() < 2 || !b.iter().all(u8::is_ascii_digit) {
                    return None;
                }
                let n: u64 = label.parse().ok()?;
                (format!("{n:02}") == label).then_some((n, 0))
            }
            LabelFormat::Letter => {
                if label.is_empty() {
                    return None;
                }
                let mut n: u64 = 0;
                for c in label.bytes() {
                    let digit = LABEL_LETTERS.iter().position(|l| *l == c)?;
                    n = n.checked_mul(24)?.checked_add(u64::try_from(digit).ok()? + 1)?;
                }
                Some((n, 0))
            }
            LabelFormat::Unknown(_) => None,
        }
    }

    /// The label of the format's first revision: `V1.0`, `01` or `A`; `None` for a newer format.
    pub fn first(&self) -> Option<Ident> {
        let text = match self {
            LabelFormat::VMajorMinor => "V1.0",
            LabelFormat::TwoDigit => "01",
            LabelFormat::Letter => "A",
            LabelFormat::Unknown(_) => return None,
        };
        text.parse().ok()
    }

    /// The proposal after `label`, bumping the major part (doc 15 §5.2): `V2.0` and `V2.3` → `V3.0`, `02` →
    /// `03`, `H` → `J`, `Z` → `AA`. `None` when `label` is not in the format, for a newer format, and on overflow.
    ///
    /// ```
    /// use ew_schema::LabelFormat;
    ///
    /// let next = |f: LabelFormat, l: &str| f.next(l).map(|n| n.as_str().to_owned());
    /// assert_eq!(next(LabelFormat::VMajorMinor, "V2.0").as_deref(), Some("V3.0"));
    /// assert_eq!(next(LabelFormat::TwoDigit, "02").as_deref(), Some("03"));
    /// assert_eq!(next(LabelFormat::Letter, "H").as_deref(), Some("J"));
    /// assert!(LabelFormat::VMajorMinor.next("2").is_none());
    /// ```
    pub fn next(&self, label: &str) -> Option<Ident> {
        let (n, _) = self.order_key(label)?;
        let n = n.checked_add(1)?;
        let text = match self {
            LabelFormat::VMajorMinor => format!("V{n}.0"),
            LabelFormat::TwoDigit => format!("{n:02}"),
            LabelFormat::Letter => {
                let mut out = Vec::new();
                let mut rest = n;
                while rest > 0 {
                    let digit = usize::try_from((rest - 1) % 24).ok()?;
                    out.push(LABEL_LETTERS[digit]);
                    rest = (rest - 1) / 24;
                }
                out.reverse();
                String::from_utf8(out).ok()?
            }
            LabelFormat::Unknown(_) => return None,
        };
        text.parse().ok()
    }

    /// The label of the next revision after `labels` (the labels of the existing revisions): the proposal after
    /// the greatest label in the format, or the first label when none is in the format. Labels out of the
    /// format are passed over (they are EW-REL-004's findings, not an input). `None` for a newer format.
    pub fn next_after<'a>(&self, labels: impl IntoIterator<Item = &'a str>) -> Option<Ident> {
        if matches!(self, LabelFormat::Unknown(_)) {
            return None;
        }
        match labels
            .into_iter()
            .filter_map(|l| self.order_key(l).map(|k| (k, l)))
            .max()
        {
            Some((_, greatest)) => self.next(greatest),
            None => self.first(),
        }
    }
}

open_enum!(RevisionState, LabelFormat);
