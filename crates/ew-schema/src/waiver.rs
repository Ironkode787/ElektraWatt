// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Waivers (doc 03 §19, doc 15 §2.5; FR-CHK-009): the `waiver` record (rank 61, `waivers.jsonl`).
//!
//! A waiver accepts one finding, matched by its 128-bit fingerprint, with a recorded reason, author and date. Typed
//! within format 0.1 (ADR-0011 amendment 4, decision log ID-334; WP-S1.16 D7): the member names are doc 15 §2.5's
//! (`rule`, `target.finding`, `reason`, `author`, `date`), which the check engine already read from the reserved
//! records, so a `waivers.jsonl` written before the kind was typed loads unchanged. `expires` (FT-184) and a 1.0 part
//! target (`target.library`, `target.item`) are not typed yet: they stay in `extra`, so a record written by a newer
//! build keeps them through a load and a save.

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::json::{Extra, de_extra, is_v1, v1};
use crate::proxy::{IdentS, MlTextS, UuidS};
use crate::value::Date;

/// One waiver (doc 03 §19, doc 15 §2.5, rank 61, `waivers.jsonl`; FR-CHK-009).
///
/// It is written only by the `HumanOnly` command `check.waive` (WP-S1.16 D7): the author and the date are that
/// command's parameters, never read from a clock or the operating system. The check engine marks a finding whose
/// rule and fingerprint match as waived; a waiver that matches nothing is reported (EW-CORE-081), never deleted.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Waiver {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Waiver>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The rule ID of the waived finding (`EW-CORE-041`).
    #[schemars(with = "IdentS")]
    pub rule: Ident,
    /// What the waiver covers.
    pub target: WaiverTarget,
    /// Why the finding is accepted (at least 10 characters, doc 15 §2.5).
    #[schemars(with = "MlTextS")]
    pub reason: MlText,
    /// Who waived it: the author's display name as the GUI recorded it (FT-494). `check.waive` always sets it; a
    /// record written while the kind was reserved may lack it and still loads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub author: Option<Ident>,
    /// When it was waived. `check.waive` always sets it; a record written while the kind was reserved may lack it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<Date>,
    /// Unknown members (`expires` until FT-184 types it) and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The target of a waiver (doc 03 §19 `target: Fingerprint | PartSelector`): the finding fingerprint, the MVP
/// target. A part selector (`{library, item}`, 1.0) stays in `extra`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WaiverTarget {
    /// The finding fingerprint, as 32 lower-case hex digits (FR-CHK-044).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding: Option<String>,
    /// Unknown members (the 1.0 part selector).
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl WaiverTarget {
    /// The fingerprint target `finding` (32 hex digits).
    pub fn finding(fingerprint: impl Into<String>) -> WaiverTarget {
        WaiverTarget {
            finding: Some(fingerprint.into()),
            extra: Extra::default(),
        }
    }
}
