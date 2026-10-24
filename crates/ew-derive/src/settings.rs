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
/// Checked:
/// - `numbering` ([`crate::NumberingSettings`]): `default` must name one of `schemes` (otherwise the
///   profile's scheme applies silently, decision log ID-103); every format must use only the known tokens
///   (an unknown one prints into tags); every scheme must be one that `device_tag` applies (the
///   `SchemeNotApplied` rule of decision log ID-99: counter scope `"page"` and formats with `{page}`,
///   `{letter}` and `{counter}`);
/// - `xref_format`: the `XrefFormat` shape, its `structure` and `same_page` values, and only known tokens in
///   `ip`, `aux_to_main` and `main_to_aux`;
/// - `colour_profile`: every colour code an IEC 60757 code or a company code that the profile's `codes`
///   declare on an IEC 60757 base; `size_profile`: every cross-section a positive number (mm², AWG or kcmil);
/// - `wire_default` and `ip_pairing`: their shapes.
///
/// `frames` and `derivation_profile` have no checker here and are refused with a message saying so.
pub fn check_member(member: Member, value: &Value) -> Result<(), String> {
    match member {
        Member::Numbering => {
            let settings = serde_json::from_value::<crate::NumberingSettings>(value.clone())
                .map_err(|e| e.to_string())?;
            // A default that names no scheme falls back to the profile's scheme (decision log ID-103).
            if !settings.schemes.iter().any(|s| s.key == settings.default) {
                return Err(format!(
                    "the default `{}` names no scheme of `schemes`; tags would silently use the profile's \
                     default scheme",
                    settings.default
                ));
            }
            for scheme in &settings.schemes {
                let formats = std::iter::once(&scheme.format).chain(scheme.rules.iter().map(|r| &r.format));
                for f in formats {
                    if let Some(token) = crate::tag::unknown_numbering_token(f) {
                        return Err(format!(
                            "scheme `{}`: format {f:?} has the unknown token {token:?}, which would print into \
                             tags; tokens are {{letter}}, {{counter}}, {{subcounter}}, {{page}}, {{column}}, \
                             {{row}}",
                            scheme.key
                        ));
                    }
                }
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

/// The company colour codes that [`check_member`] accepts for the `colour_profile` value `colour_profile`, as
/// `(code, base IEC 60757 code)` in the order the profile's `codes` declare them (doc 08 §4.5: `("DBU", "BU")`);
/// empty for a value [`check_member`] would refuse, and for a profile that declares none (WP-S2.05 §4.2: the
/// property grid's colour editor accepts exactly these besides the IEC codes).
pub fn company_codes(colour_profile: &Value) -> Vec<(String, String)> {
    if check_member(Member::ColourProfile, colour_profile).is_err() {
        return Vec::new();
    }
    colour_profile
        .get("codes")
        .and_then(Value::as_array)
        .map(|codes| {
            codes
                .iter()
                .filter_map(|c| {
                    let code = c.get("code")?.as_str()?;
                    let base = c.get("base")?.as_str()?;
                    Some((code.to_owned(), base.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
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

/// The drawing settings of the project: the member `drawing` of `ProjectSettings.extra` (WP-S2.04 D16, §8.6),
/// typed. Absent, not an object, or a member of an unknown value: the as-built defaults.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct DrawingSettings {
    /// `crossings: "hops"`: crossing wires are drawn as hops (FR-CON-031); else plain.
    pub hops: bool,
    /// `marker_mode` (FR-CON-024, WP-S2.04 D16): which wires carry a marker; `None` (absent or an unknown value) is
    /// the as-built rule, a connection's `show_marker`.
    pub marker_mode: Option<crate::items::MarkerMode>,
}

/// `drawing_settings()`: the project's [`DrawingSettings`] (WP-S2.04 part 2, §8.6). Policy Memo: a cut-off point
/// over the project row, so an edit of another settings member leaves the readers (the wires layer of every page)
/// valid.
#[derive(Debug)]
pub struct DrawingSettingsQ;

impl Query for DrawingSettingsQ {
    type Key = ();
    type Value = DrawingSettings;
    const FAMILY: &'static str = "drawing_settings";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<DrawingSettings> {
        let project = q.project();
        let drawing = project.settings.extra.get("drawing");
        let hops = drawing.and_then(|d| d.get("crossings")).and_then(Value::as_str) == Some("hops");
        let marker_mode = drawing
            .and_then(|d| d.get("marker_mode"))
            .and_then(Value::as_str)
            .and_then(crate::items::MarkerMode::from_name);
        Arc::new(DrawingSettings { hops, marker_mode })
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
    fn company_codes_are_the_declared_ones_of_an_accepted_profile() {
        let profile = json!({"key": "k", "codes": [
            {"code": "DBU", "base": "BU", "shade": "dark"},
            {"code": "LGN", "base": "GN"}
        ], "rows": [{"colour": ["DBU"]}]});
        assert_eq!(
            company_codes(&profile),
            [
                ("DBU".to_owned(), "BU".to_owned()),
                ("LGN".to_owned(), "GN".to_owned())
            ]
        );
        assert!(company_codes(&json!({"key": "k", "rows": []})).is_empty());
        // A value check_member refuses (an IEC code declared as a company code) has none.
        let bad = json!({"key": "k", "codes": [{"code": "BU", "base": "BU"}], "rows": []});
        assert!(check_member(Member::ColourProfile, &bad).is_err());
        assert!(company_codes(&bad).is_empty());
        assert!(company_codes(&Value::Null).is_empty());
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

    fn err(member: Member, value: Value) -> String {
        check_member(member, &value).expect_err("refused")
    }

    #[test]
    fn numbering_that_would_fall_back_or_print_a_bogus_token_is_refused() {
        let scheme = |format: &str| json!({"key": "page-based", "format": format});
        let e = err(
            Member::Numbering,
            json!({"default": "other", "schemes": [scheme("{page}{letter}{counter}")]}),
        );
        assert!(e.contains("`other` names no scheme"), "{e}");
        let e = err(Member::Numbering, json!({"default": "page-based", "schemes": []}));
        assert!(e.contains("names no scheme"), "{e}");
        let e = err(
            Member::Numbering,
            json!({"default": "page-based", "schemes": [scheme("{page}{letter}{counter}{bogus}")]}),
        );
        assert!(e.contains("{bogus}"), "{e}");
        let e = err(
            Member::Numbering,
            json!({"default": "page-based", "schemes": [scheme("{page}{letter}{counter}{page")]}),
        );
        assert!(e.contains("unknown token \"{page\""), "{e}");
        let rule = json!({"default": "page-based", "schemes": [{"key": "page-based",
            "format": "{page}{letter}{counter}",
            "rules": [{"select": {"aspect": "strip"}, "format": "{letter}{page}{cnt}"}]}]});
        assert!(err(Member::Numbering, rule).contains("{cnt}"));
    }

    #[test]
    fn xref_formats_with_unknown_tokens_are_refused() {
        let xref = |ip: &str| {
            json!({"ip": ip, "aux_to_main": "({page}-{row}{column})", "main_to_aux": "∞ {list}",
                "matrix": {"per_row": 2, "max": 6, "overflow": "…"}})
        };
        assert_eq!(check_member(Member::XrefFormat, &xref("%F-%l%c %%")), Ok(()));
        assert!(err(Member::XrefFormat, xref("{page}-{bogus}")).contains("{bogus}"));
        assert!(err(Member::XrefFormat, xref("%F-%Z")).contains("%Z"));
        assert!(err(Member::XrefFormat, xref("{page")).contains("unknown token"));
    }

    #[test]
    fn colour_codes_and_cross_sections_are_checked() {
        let colour =
            |codes: Value, colour: Value| json!({"key": "k", "codes": codes, "rows": [{"colour": colour}]});
        let dbu = json!([{"code": "DBU", "base": "BU", "shade": "dark"}]);
        assert_eq!(
            check_member(Member::ColourProfile, &colour(dbu.clone(), json!(["DBU"]))),
            Ok(())
        );
        assert_eq!(
            check_member(Member::ColourProfile, &colour(json!([]), json!(["GN", "YE"]))),
            Ok(())
        );
        assert!(
            err(Member::ColourProfile, colour(json!([]), json!(["DBU"]))).contains("neither an IEC 60757")
        );
        assert!(err(Member::ColourProfile, colour(dbu, json!([{"foreign": "BR"}]))).contains("foreign"));
        let no_base = json!([{"code": "DBU"}]);
        assert!(err(Member::ColourProfile, colour(no_base, json!(["DBU"]))).contains("base"));
        let iec = json!([{"code": "BK", "base": "BK"}]);
        assert!(err(Member::ColourProfile, colour(iec, json!(["BK"]))).contains("is an IEC 60757 code"));

        let size = |csa: Value| json!({"key": "s", "rows": [{"class": "control", "csa": csa}]});
        assert_eq!(check_member(Member::SizeProfile, &size(json!("0.75"))), Ok(()));
        assert_eq!(
            check_member(Member::SizeProfile, &size(json!({"awg": 18}))),
            Ok(())
        );
        assert!(err(Member::SizeProfile, size(json!("thick"))).contains("not a positive number"));
        assert!(err(Member::SizeProfile, size(json!("0"))).contains("not a positive number"));
    }
}
