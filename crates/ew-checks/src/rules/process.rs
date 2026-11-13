// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-PRC-001, the process-tag rule of the EW-PLC pack's 1.0 content (doc 10 §3.4, §3.10; doc 15 §4.5.1;
//! FR-PLC-049). EW-PRC-002 (an instrument-list tag without device or channel) needs the instrument-list import of
//! the PLC exchange and is not built.

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Device, Kind};
use ew_derive::{Q, process_tag_scheme};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

/// EW-PRC-001 "Process-tag scheme" (doc 15 §4.5.1): hygiene, o r, warning, project (FR-PLC-049).
pub const PROCESS_TAG: RuleMeta = RuleMeta {
    id: "EW-PRC-001",
    code: "110201",
    pack: "EW-PLC",
    title: "Process-tag scheme",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-prc-001",
};

/// EW-PRC-001 over the devices' process tags: a tag that does not match the project's process-tag scheme
/// (`ProjectSettings.process_tag_scheme`, a regular expression the whole tag matches; discriminator `scheme`, target
/// the device), and a tag that two or more devices carry (`duplicate`, every device a target: a valve and the device
/// it drives carrying the same tag, doc 11 §3.3). Without a scheme, or with one that does not compile (refused by
/// `project.settings.set`), only duplicates are reported.
#[derive(Clone, Copy, Debug)]
pub struct ProcessTag;

impl Rule for ProcessTag {
    fn meta(&self) -> &'static RuleMeta {
        &PROCESS_TAG
    }

    fn run(&self, q: &Q, _scope: &Scope) -> Vec<Finding> {
        let scheme = q
            .project()
            .settings
            .process_tag_scheme
            .as_deref()
            .and_then(|s| process_tag_scheme(s).ok().map(|r| (s.to_owned(), r)));
        let mut by_tag: BTreeMap<String, Vec<Id<Device>>> = BTreeMap::new();
        let mut out = Vec::new();
        for (id, d) in q.devices().iter() {
            let Some(tag) = d.process_tag.as_ref().map(|t| t.as_str().to_owned()) else {
                continue;
            };
            by_tag.entry(tag.clone()).or_default().push(id);
            if let Some((text, re)) = &scheme {
                if !re.is_match(&tag) {
                    let device = full_tag(q, id, "a device");
                    out.push(
                        Finding::new(
                            &PROCESS_TAG,
                            [Target::new(Kind::Device, id)],
                            "scheme",
                            device.clone(),
                            format!(
                                "{device}: the process tag {tag} does not match the project's scheme {text}; correct \
                                 the tag"
                            ),
                        )
                        .arg("form", "scheme")
                        .arg("device", device)
                        .arg("tag", tag.clone())
                        .arg("scheme", text.clone()),
                    );
                }
            }
        }
        for (tag, devices) in by_tag {
            if devices.len() < 2 {
                continue;
            }
            let mut names: Vec<String> = devices.iter().map(|d| full_tag(q, *d, "a device")).collect();
            names.sort_by(|a, b| ew_derive::ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)));
            let list = names.join(", ");
            out.push(
                Finding::new(
                    &PROCESS_TAG,
                    devices.iter().map(|d| Target::new(Kind::Device, *d)),
                    format!("duplicate:{tag}"),
                    names[0].clone(),
                    format!(
                        "the process tag {tag} is carried by {list}; keep it on the one field device and track it from \
                         the others"
                    ),
                )
                .arg("form", "duplicate")
                .arg("tag", tag)
                .arg("devices", list),
            );
        }
        out
    }
}
