// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the assembly list cannot name (doc 13 §6.3): EW-MFG-020, a strip without an end plate after its last terminal;
//! EW-MFG-021, a bridge without a part.

use ew_derive::layout::{StripItemKind, StripLayout};
use ew_derive::{Q, accessory_part, bridge_groups};
use ew_schema::{AccessoryKind, AccessoryPer, Kind};

use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

// ---------------------------------------------------------------------------------------------------------
// EW-MFG-020

/// EW-MFG-020 "Strip without an end plate" (doc 15 §4.5.5): completeness, o r, warning, device.
pub const NO_END_PLATE: RuleMeta = RuleMeta {
    id: "EW-MFG-020",
    message: key!("rule-ew-mfg-020"),
    code: "140020",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-MFG-020: a strip whose terminals have parts and after whose last terminal (`strip_layout`) no end plate stands:
/// none of its terminals' parts names one, it was left off, or none was added. A strip whose terminals have no part asks
/// nothing: the part decides whether one is needed. One finding per strip, target the strip. Its quick fix, where one
/// of its terminals' parts names an end plate, adds that end plate as the strip's own after the last terminal
/// (`fields.set` of `strip.accessories`).
#[derive(Clone, Copy, Debug)]
pub struct NoEndPlate;

impl Rule for NoEndPlate {
    fn meta(&self) -> &'static RuleMeta {
        &NO_END_PLATE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(strip) = *scope else {
            return Vec::new();
        };
        let Some(dev) = q.devices().get(strip).cloned() else {
            return Vec::new();
        };
        let Some(aspect) = dev.aspects.strip.as_ref() else {
            return Vec::new();
        };
        let layout = q.get::<StripLayout>(strip);
        let devices = q.devices();
        let terminals: Vec<_> = layout
            .iter()
            .filter_map(|i| match i.kind {
                StripItemKind::Terminal(t) => devices.get(t).cloned(),
                _ => None,
            })
            .collect();
        let Some(last) = terminals.last() else {
            return Vec::new();
        };
        if terminals.iter().all(|t| t.parts.is_empty()) {
            return Vec::new();
        }
        let at = layout
            .iter()
            .rposition(|i| matches!(i.kind, StripItemKind::Terminal(_)))
            .unwrap_or(0);
        let plate_after = layout[at + 1..].iter().any(|i| match &i.kind {
            StripItemKind::Accessory(a) => a.kind == Some(AccessoryKind::EndPlate),
            StripItemKind::Own(n) => aspect
                .accessories
                .get(*n)
                .is_some_and(|a| a.kind == AccessoryKind::EndPlate),
            StripItemKind::Terminal(_) => false,
        });
        if plate_after {
            return Vec::new();
        }
        let terminal = last
            .aspects
            .terminal
            .as_ref()
            .map(|t| t.designation.as_str().to_owned())
            .unwrap_or_default();
        let tag = full_tag(q, strip, ew_derive::text::record_name(Kind::Device));
        let mut f = Finding::new(&NO_END_PLATE, [Target::new(Kind::Device, strip)], "", tag.clone())
            .arg("strip", tag)
            .arg("terminal", terminal.clone());
        // The end plate its terminals' parts name, added as the strip's own after the last terminal.
        let parts = q.parts();
        let declared = terminals.iter().flat_map(|t| t.parts.iter()).find_map(|u| {
            parts.get(u.part.copy)?.item.accessories.iter().find_map(|a| {
                (a.per == AccessoryPer::Strip && a.kind == Some(AccessoryKind::EndPlate))
                    .then(|| accessory_part(q, a))
                    .filter(|copy| parts.contains(*copy))
            })
        });
        if let Some(copy) = declared {
            let seq = last.aspects.terminal.as_ref().map_or(0, |t| t.seq);
            let mut list: Vec<serde_json::Value> = aspect
                .accessories
                .iter()
                .filter_map(|a| serde_json::to_value(a).ok())
                .collect();
            list.push(serde_json::json!({"after_seq": seq, "kind": "end_plate", "part": {"copy": copy}}));
            let number = parts
                .get(copy)
                .map(|c| c.item.part_number.as_str().to_owned())
                .unwrap_or_default();
            f = f.with_fix(QuickFix {
                label: text!("fix-add-end-plate", number, terminal),
                command: "fields.set".to_owned(),
                params: serde_json::json!({
                    "kind": "device",
                    "targets": [strip.to_string()],
                    "path": "strip.accessories",
                    "value": list,
                }),
            });
        }
        vec![f]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-MFG-021

/// EW-MFG-021 "Bridge without a part" (doc 15 §4.5.5): completeness, o r, warning (in release too: a project made
/// before a bridge could have a part releases as it did), device.
pub const BRIDGE_WITHOUT_PART: RuleMeta = RuleMeta {
    id: "EW-MFG-021",
    message: key!("rule-ew-mfg-021"),
    code: "140021",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-MFG-021: a bridge group of a strip whose carrier connection names no part (`BridgeGroup::part`), so the parts
/// list and the assembly list cannot name it. One finding per bridge, target the strip, discriminator the group
/// number; named as the lists name it (`BR1`, `bridge_groups`).
#[derive(Clone, Copy, Debug)]
pub struct BridgeWithoutPart;

impl Rule for BridgeWithoutPart {
    fn meta(&self) -> &'static RuleMeta {
        &BRIDGE_WITHOUT_PART
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(strip) = *scope else {
            return Vec::new();
        };
        let designation = |t: Option<&ew_ids::Id<ew_schema::Device>>| {
            t.and_then(|t| q.devices().get(*t))
                .and_then(|d| {
                    d.aspects
                        .terminal
                        .as_ref()
                        .map(|a| a.designation.as_str().to_owned())
                })
                .unwrap_or_default()
        };
        bridge_groups(q)
            .into_iter()
            .filter(|(_, s, g)| *s == strip && g.part.is_none())
            .map(|(label, _, g)| {
                let tag = full_tag(q, strip, ew_derive::text::record_name(Kind::Device));
                Finding::new(
                    &BRIDGE_WITHOUT_PART,
                    [Target::new(Kind::Device, strip)],
                    g.group.to_string(),
                    tag.clone(),
                )
                .arg("bridge", label)
                .arg("strip", tag)
                .arg("first", designation(g.terminals.first()))
                .arg("last", designation(g.terminals.last()))
            })
            .collect()
    }
}
