// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project settings members as queries: `settings_member(name)` (ARCH §6.4, ADR-0002 amendment 4, decision log
//! ID-221 and ID-226).
//!
//! The project row carries one revision stamp. A query that read it directly would be invalidated by every
//! edit of the row, a colour-profile edit included. `settings_member(m)` is one projection memo per member
//! over that stamp: an edit of the row re-executes the few member memos, and each member that did not change
//! cuts off, so its readers stay valid. No stamp is added to `ew-model` (ADR-0001 is not amended).
//!
//! Readers take a member through [`member`], which reads it as a value part (`Q::get_value_part`): a commit
//! that marks the member memo does not mark its readers; only a member whose value changed reaches them.

use std::sync::Arc;

use ew_schema::IpPairing;
use serde_json::Value;

use crate::Q;
use crate::query::{Policy, Query};

/// A settings member a derivation reads.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Member {
    /// `ProjectSettings.ip_pairing` (typed).
    IpPairing,
    /// `colour_profile` (a provisional member of `ProjectSettings.extra`, doc 03 §5.1).
    ColourProfile,
    /// `size_profile` (provisional, `extra`).
    SizeProfile,
    /// `wire_default` (provisional, `extra`).
    WireDefault,
    /// `numbering` (provisional, `extra`).
    Numbering,
    /// `frames` (provisional, `extra`).
    Frames,
    /// `xref_format` (provisional, `extra`).
    XrefFormat,
    /// The pinned derivation profile key, `Project.derivation_profile` (ADR-0003).
    DerivationProfile,
}

impl Member {
    /// The member's name as stored (the key in `ProjectSettings.extra` for the provisional members).
    pub fn name(self) -> &'static str {
        match self {
            Member::IpPairing => "ip_pairing",
            Member::ColourProfile => "colour_profile",
            Member::SizeProfile => "size_profile",
            Member::WireDefault => "wire_default",
            Member::Numbering => "numbering",
            Member::Frames => "frames",
            Member::XrefFormat => "xref_format",
            Member::DerivationProfile => "derivation_profile",
        }
    }
}

/// `settings_member(m)`: the member's value as JSON, `None` when absent. The pinned derivation profile is its
/// key as a JSON string; `ip_pairing` is its typed value written as JSON. Policy Memo: a cut-off point, not a
/// cost saving (ARCH §6.4).
#[derive(Debug)]
pub struct SettingsMember;

impl Query for SettingsMember {
    type Key = Member;
    type Value = Option<Value>;
    const FAMILY: &'static str = "settings_member";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        // The JSON text length is a fair estimate of a small JSON tree's heap.
        std::mem::size_of::<Self::Value>() + value.as_ref().map_or(0, |v| v.to_string().len())
    }

    fn run(q: &Q, member: &Member) -> Arc<Option<Value>> {
        let project = q.project();
        let value = match member {
            Member::IpPairing => project
                .settings
                .ip_pairing
                .as_ref()
                .map(|p| serde_json::to_value(p).expect("a typed settings member serialises")),
            Member::DerivationProfile => Some(Value::String(project.derivation_profile.as_str().to_owned())),
            other => project.settings.extra.get(other.name()).cloned(),
        };
        Arc::new(value)
    }
}

/// The value of member `m`, read as a cut-off point (see the module documentation).
pub(crate) fn member(q: &Q, m: Member) -> Arc<Option<Value>> {
    q.get_value_part::<SettingsMember>(m)
}

/// The project's `ip_pairing` setting, typed; `None` when absent.
pub(crate) fn ip_pairing(q: &Q) -> Option<IpPairing> {
    member(q, Member::IpPairing)
        .as_ref()
        .as_ref()
        .map(|v| serde_json::from_value(v.clone()).expect("a typed settings member parses back"))
}

/// The pinned derivation profile key (`Project.derivation_profile`).
pub(crate) fn profile_key(q: &Q) -> String {
    match member(q, Member::DerivationProfile).as_ref() {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}
