// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Versioned derivation profiles (ADR-0003 "Versioned derivation profiles", ARCH §6.6, FR-PRJ-011) and the
//! project's numbering settings (doc 03 §5.1).
//!
//! A [`DerivationProfile`] is data: the shipped profiles live in `profiles/<version>.toml` of this crate
//! and are parsed once. They sit here, next to the derivation code, and not in `content/`, because a
//! profile is not a library item: it is never embedded into a project as a copy, it is referenced by its
//! version key, and every shipped profile keeps its code paths alive in this crate forever (ADR-0003,
//! "Negative"). `content/` holds library items with copy identities (doc 07 §2).
//!
//! The project's own schemes win over the profile's defaults (doc 03 §5.1, FR-PRJ-008): until `ew-schema`
//! types `ProjectSettings.numbering` (WP-S1.11), it is read from `ProjectSettings.extra["numbering"]`
//! with the JSON shape of doc 03 §5.1.

use std::sync::{Arc, OnceLock};

use ew_schema::{Project, RepType};
use serde::Deserialize;

use crate::Q;
use crate::note::DeriveNote;
use crate::query::{Policy, Query};

/// The shipped profiles, oldest first: `(version, TOML source)`.
const SHIPPED: &[(&str, &str)] = &[("2027.1", include_str!("../profiles/2027.1.toml"))];

/// One derivation profile.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationProfile {
    /// The version key, `"2027.1"`.
    pub version: String,
    /// The identifier set (ADR-0026), a key of `ew_rds::IdentifierSet::shipped`.
    pub identifier_set: String,
    /// The default device-numbering scheme.
    pub numbering: NumberingScheme,
    /// The cross-reference format.
    pub xref: XrefFormat,
    /// The title-block binding set.
    pub title_block: TitleBlockBindings,
    /// The table-of-contents layout.
    pub toc: TocLayout,
}

/// A device-numbering scheme (doc 03 §5.1 `DeviceNumberingScheme`, §8.2).
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
pub struct NumberingScheme {
    /// The key (`"page-based"`).
    pub key: String,
    /// The default format: tokens `{letter}`, `{counter}`, `{subcounter}`, `{page}`, `{column}`, `{row}`
    /// and literal text. Formats never contain the product sign "-".
    pub format: String,
    /// Per-class rules, first match wins.
    #[serde(default)]
    pub rules: Vec<NumberingRule>,
    /// The counter scope (only `"page"` is implemented by the command layer, ID-48).
    #[serde(default = "page_scope")]
    pub counter_scope: CounterScope,
    /// The representation types that can be the main representation; `None` is every type except
    /// contact images and panel fronts.
    #[serde(default)]
    pub counted_rep_types: Option<Vec<RepType>>,
    /// Fill gaps when choosing the next counter (doc 03 §8.6).
    #[serde(default = "yes")]
    pub reuse_gaps: bool,
    /// Number new devices at commit.
    #[serde(default = "yes")]
    pub online: bool,
    /// The text between a parent's and a child's product designation.
    #[serde(default = "dash")]
    pub child_separator: String,
}

/// One rule of a numbering scheme: a selector and the format it applies.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
pub struct NumberingRule {
    /// What the rule matches; every given member must match.
    #[serde(default)]
    pub select: RuleSelect,
    /// The format.
    pub format: String,
}

/// The selector of a [`NumberingRule`].
#[derive(Clone, PartialEq, Eq, Debug, Default, Deserialize)]
pub struct RuleSelect {
    /// A device aspect name: `"strip"`, `"terminal"`, `"cable"`.
    #[serde(default)]
    pub aspect: Option<String>,
    /// A letter code.
    #[serde(default)]
    pub letter: Option<String>,
    /// A `DeviceKind` in its snake_case spelling.
    #[serde(default)]
    pub kind: Option<String>,
}

/// The counter scope of a numbering scheme (FR-NUM-006).
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(untagged)]
pub enum CounterScope {
    /// `"project"`, `"page"`, `"row"` or `"column"`.
    Named(String),
    /// `{"structure": <aspect>}`.
    Structure {
        /// The aspect name.
        structure: String,
    },
}

/// The cross-reference format (doc 03 §5.1 `XrefFormat`).
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XrefFormat {
    /// The key.
    pub key: String,
    /// Interruption-point text.
    pub ip: String,
    /// Auxiliary → main.
    pub aux_to_main: String,
    /// Main → auxiliaries, with `{list}`.
    pub main_to_aux: String,
    /// `"when_different"` (the default) or `"always"`.
    #[serde(default = "default_structure")]
    pub structure: String,
    /// `"full"` (the default) or `"zone_only"`.
    #[serde(default = "default_same_page")]
    pub same_page: String,
    /// The contact matrix.
    pub matrix: XrefMatrix,
}

fn default_structure() -> String {
    "when_different".to_owned()
}

fn default_same_page() -> String {
    "full".to_owned()
}

/// The matrix of main → auxiliary references.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XrefMatrix {
    /// Entries per row.
    pub per_row: u8,
    /// Entries in total.
    pub max: u8,
    /// The overflow mark.
    pub overflow: String,
}

/// The title-block binding set: the placeholder paths that `title_block(page)` resolves.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TitleBlockBindings {
    /// The paths, `scope.segment(.segment)*`.
    pub paths: Vec<String>,
}

/// The layout facts of the table of contents that page order depends on.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TocLayout {
    /// Data rows per TOC page.
    pub rows_per_page: u32,
}

fn page_scope() -> CounterScope {
    CounterScope::Named("page".to_owned())
}

fn yes() -> bool {
    true
}

fn dash() -> String {
    "-".to_owned()
}

/// Whether the pinned profile was found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProfileStatus {
    /// The project's profile ships with this build.
    Pinned,
    /// The project pins a profile this build does not ship (a newer file); derivations use the newest
    /// shipped profile instead, and the caller reports it.
    Unknown {
        /// The key the project asked for.
        requested: String,
    },
}

/// Every shipped profile, oldest first.
///
/// # Panics
///
/// If a shipped profile does not parse, which the crate's tests rule out.
pub fn shipped() -> &'static [DerivationProfile] {
    static PROFILES: OnceLock<Vec<DerivationProfile>> = OnceLock::new();
    PROFILES.get_or_init(|| {
        SHIPPED
            .iter()
            .map(|(version, src)| {
                let p: DerivationProfile = toml::from_str(src)
                    .unwrap_or_else(|e| panic!("shipped profile {version} does not parse: {e}"));
                assert_eq!(p.version, *version, "profile file and key disagree");
                p
            })
            .collect()
    })
}

/// The shipped profile with this version key.
pub fn by_version(version: &str) -> Option<&'static DerivationProfile> {
    shipped().iter().find(|p| p.version == version)
}

/// The profile for a pinned key: the matching shipped one, else the newest shipped one.
pub(crate) fn resolve(version: &str) -> (&'static DerivationProfile, ProfileStatus) {
    match by_version(version) {
        Some(p) => (p, ProfileStatus::Pinned),
        None => (
            shipped().last().expect("at least one shipped profile"),
            ProfileStatus::Unknown {
                requested: version.to_owned(),
            },
        ),
    }
}

/// The project's numbering settings (doc 03 §5.1 `numbering`): the default scheme key and the schemes.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
pub struct NumberingSettings {
    /// The key of the default scheme.
    pub default: String,
    /// The schemes, keyed by `key`.
    #[serde(default)]
    pub schemes: Vec<NumberingScheme>,
}

/// The project's numbering settings, if present and readable. A member that does not parse is ignored
/// (the profile's default applies); the settings check reports it.
pub fn numbering_settings(project: &Project) -> Option<NumberingSettings> {
    let value = project.settings.extra.get("numbering")?;
    serde_json::from_value(value.clone()).ok()
}

/// `numbering_settings()`: [`numbering_settings`] parsed once per [`Q`]. A member that is present but does
/// not parse yields `None` (the profile's default applies) and a [`DeriveNote::MalformedShim`] for
/// `settings.numbering` with the parser's message.
#[derive(Debug)]
pub struct NumberingSettingsOf;

impl Query for NumberingSettingsOf {
    type Key = ();
    type Value = Option<NumberingSettings>;
    const FAMILY: &'static str = "numbering_settings";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, _key: &()) -> Arc<Option<NumberingSettings>> {
        let project = q.project();
        let Some(value) = project.settings.extra.get("numbering") else {
            return Arc::new(None);
        };
        match serde_json::from_value(value.clone()) {
            Ok(settings) => Arc::new(Some(settings)),
            Err(e) => {
                q.note(DeriveNote::MalformedShim {
                    member: "settings.numbering".into(),
                    detail: e.to_string(),
                });
                Arc::new(None)
            }
        }
    }
}

/// The scheme a tag with `named` (`None` for the default scheme) uses: the project's scheme of that key,
/// else the profile's default scheme.
pub fn scheme_for<'a>(
    settings: Option<&'a NumberingSettings>,
    profile: &'a DerivationProfile,
    named: Option<&str>,
) -> &'a NumberingScheme {
    let Some(settings) = settings else {
        return &profile.numbering;
    };
    let key = named.unwrap_or(settings.default.as_str());
    settings
        .schemes
        .iter()
        .find(|s| s.key == key)
        .or_else(|| settings.schemes.iter().find(|s| s.key == settings.default))
        .unwrap_or(&profile.numbering)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_profile_2027_1_parses() {
        let p = by_version("2027.1").expect("2027.1 ships");
        assert_eq!(p.identifier_set, ew_rds::EN_81346_2_2009);
        assert!(ew_rds::IdentifierSet::by_key(&p.identifier_set).is_some());
        assert_eq!(p.numbering.key, "page-based");
        assert_eq!(p.numbering.format, "{page}{letter}{counter}");
        assert!(p.numbering.rules.is_empty());
        assert_eq!(p.numbering.child_separator, "-");
        assert_eq!(p.xref.ip, "{page}-{row}{column}");
        assert_eq!(p.toc.rows_per_page, 23);
        assert!(p.title_block.paths.iter().any(|x| x == "project.logo"));
        assert!(p.title_block.paths.iter().any(|x| x == "project.notice"));
    }

    #[test]
    fn unknown_profile_falls_back_to_the_newest() {
        let (p, status) = resolve("2099.9");
        assert_eq!(p.version, shipped().last().unwrap().version);
        assert_eq!(
            status,
            ProfileStatus::Unknown {
                requested: "2099.9".into()
            }
        );
        assert_eq!(resolve("2027.1").1, ProfileStatus::Pinned);
    }

    #[test]
    fn project_numbering_settings_parse_from_extra() {
        let json = r#"{"default":"page-based","schemes":[{"key":"page-based","format":"{page}{letter}{counter}",
            "rules":[{"select":{"aspect":"strip"},"format":"{letter}{page}{counter}"}]}]}"#;
        let s: NumberingSettings = serde_json::from_str(json).unwrap();
        let p = by_version("2027.1").unwrap();
        let scheme = scheme_for(Some(&s), p, None);
        assert_eq!(scheme.rules.len(), 1);
        assert_eq!(scheme.rules[0].select.aspect.as_deref(), Some("strip"));
        assert_eq!(scheme.counter_scope, CounterScope::Named("page".into()));
        assert_eq!(scheme_for(None, p, None).key, "page-based");
    }
}
