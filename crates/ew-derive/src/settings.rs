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

    /// Every member, in declaration order.
    pub const ALL: [Member; 8] = [
        Member::IpPairing,
        Member::ColourProfile,
        Member::SizeProfile,
        Member::WireDefault,
        Member::Numbering,
        Member::Frames,
        Member::XrefFormat,
        Member::DerivationProfile,
    ];

    /// The member stored under `name` ([`Member::name`]).
    pub fn from_name(name: &str) -> Option<Member> {
        Member::ALL.into_iter().find(|m| m.name() == name)
    }
}

/// Checks a value for a settings member with the parser its derivations read it with, so that a command
/// can refuse what the derivation would only note: `Err` carries the detail of the `MalformedShim` note
/// (or, for `ip_pairing`, the record type's error) that the value would cause.
///
/// Checked: `numbering` ([`crate::NumberingSettings`]; every scheme must also be one that `device_tag`
/// applies, the `SchemeNotApplied` rule of decision log ID-99: counter scope `"page"` and formats with
/// `{page}`, `{letter}` and `{counter}`), `xref_format` (the `XrefFormat` shape with its `structure` and
/// `same_page` values), `colour_profile`, `size_profile`, `wire_default` and `ip_pairing`. `frames` and
/// `derivation_profile` have no checker here and are refused with a message saying so.
pub fn check_member(member: Member, value: &Value) -> Result<(), String> {
    match member {
        Member::Numbering => {
            let settings = serde_json::from_value::<crate::NumberingSettings>(value.clone())
                .map_err(|e| e.to_string())?;
            for scheme in &settings.schemes {
                if let Some(issue) = crate::tag::scheme_issue(scheme) {
                    return Err(format!(
                        "scheme `{}` would not be applied ({}); until WP-S1.11 a scheme needs counter \
                         scope \"page\" and formats with {{page}}, {{letter}} and {{counter}}",
                        scheme.key,
                        issue.describe()
                    ));
                }
            }
            Ok(())
        }
        Member::XrefFormat => crate::xref::check_format_member(value),
        Member::ColourProfile | Member::SizeProfile | Member::WireDefault => {
            crate::wire_profile::check_member(member.name(), value)
        }
        Member::IpPairing => serde_json::from_value::<IpPairing>(value.clone())
            .map(drop)
            .map_err(|e| e.to_string()),
        Member::Frames | Member::DerivationProfile => {
            Err(format!("`{}` has no value checker", member.name()))
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn member_names_round_trip() {
        for m in Member::ALL {
            assert_eq!(Member::from_name(m.name()), Some(m));
        }
        assert_eq!(Member::from_name("identifier_set"), None);
    }

    #[test]
    fn check_member_uses_the_derivation_parsers() {
        let numbering = json!({"default": "page-based", "schemes": [
            {"key": "page-based", "format": "{page}{letter}{counter}", "rules": []}]});
        assert_eq!(check_member(Member::Numbering, &numbering), Ok(()));
        assert!(
            check_member(Member::Numbering, &json!({"schemes": []})).is_err(),
            "no default"
        );
        assert!(check_member(Member::Numbering, &json!(7)).is_err());
        let flat = json!({"default": "flat", "schemes": [{"key": "flat", "format": "{letter}{counter}"}]});
        let err = check_member(Member::Numbering, &flat).unwrap_err();
        assert!(err.contains("`flat`") && err.contains("{page}"), "{err}");

        let xref = json!({"name": "QET", "ip": "{page}-{row}{column}", "aux_to_main": "({page}-{row}{column})",
            "main_to_aux": "∞ {list}", "matrix": {"per_row": 2, "max": 6, "overflow": "…"}});
        assert_eq!(check_member(Member::XrefFormat, &xref), Ok(()));
        let mut bad = xref.clone();
        bad["structure"] = json!("sometimes");
        let err = check_member(Member::XrefFormat, &bad).unwrap_err();
        assert!(err.contains("sometimes"), "{err}");

        assert_eq!(
            check_member(Member::ColourProfile, &json!({"key": "k", "rows": []})),
            Ok(())
        );
        assert!(check_member(Member::ColourProfile, &json!({"rows": 3})).is_err());
        assert!(check_member(Member::SizeProfile, &json!({"rows": [{"class": "control"}]})).is_err());
        assert!(check_member(Member::Frames, &json!({})).is_err());
    }
}
