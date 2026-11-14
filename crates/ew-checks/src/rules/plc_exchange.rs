// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The two rules of the EW-PLC pack that read the PLC exchange (doc 10 §3.10, §4.4, §4.7; doc 15 §4.5.1): EW-PLC-016,
//! exchange drift (FR-PLC-035), and EW-PRC-002, an instrument-list tag without device or channel (FR-PLC-050).

use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{
    Channel, Device, ExchangeDirection, ExchangeFormat, ExchangeObject, ExchangeRecord, Kind,
    PlcConfigProject,
};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::plc::{cfg_name, channel_name};
use crate::rules::term::full_tag;

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-016

/// EW-PLC-016 "Exchange drift" (doc 15 §4.5.1): hygiene, o, info, cfg (FR-PLC-035).
pub const EXCHANGE_DRIFT: RuleMeta = RuleMeta {
    id: "EW-PLC-016",
    code: "110016",
    pack: "EW-PLC",
    title: "Exchange drift",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("o"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Cfg,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-plc-016",
};

/// EW-PLC-016: a configuration project whose exchange image differs from the snapshot of its last exchange (doc 10
/// §4.4): a station, module, channel, tag or bus node of the exchanged stations with another value, added since or
/// gone since. One finding per configuration project, target the configuration project; it names the exchange (its
/// direction, format and date), counts the differing objects and names the first three. After an export the project
/// is as exchanged; after an import a value the merge kept on the project's side differs, which is the pending
/// export. Whether a foreign file is newer needs the file and is `ew plc status`'s (doc 10 §4.4).
#[derive(Clone, Copy, Debug)]
pub struct ExchangeDrift;

impl Rule for ExchangeDrift {
    fn meta(&self) -> &'static RuleMeta {
        &EXCHANGE_DRIFT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cfg(cfg) = *scope else {
            return Vec::new();
        };
        let Some(stamp) = q
            .table::<PlcConfigProject>()
            .get(cfg)
            .and_then(|c| c.exchange.clone())
        else {
            return Vec::new();
        };
        let drift = ew_plc::drift(q, cfg);
        if drift.is_empty() {
            return Vec::new();
        }
        let name = cfg_name(q, cfg);
        let direction = match stamp.direction {
            ExchangeDirection::Import => "import",
            _ => "export",
        };
        // the format's key in the message patterns, and its English words
        let (format, format_words) = match stamp.format {
            ExchangeFormat::Aml => ("aml", "AML"),
            ExchangeFormat::TiaXlsx => ("tia_xlsx", "tag table"),
            ExchangeFormat::Plcopen => ("plcopen", "PLCopen"),
            ExchangeFormat::Unknown(_) => ("other", "PLC"),
        };
        let channels = q.table::<Channel>();
        let objects: Vec<String> = drift
            .iter()
            .take(3)
            .map(
                |(object, target, words, _)| match (object, channels.get(target.cast())) {
                    (ExchangeObject::Channel, Some(c)) => channel_name(q, c.module, c.id, c.number),
                    _ => words.clone(),
                },
            )
            .collect();
        let list = objects.join(", ");
        let count = drift.len();
        let more = count - objects.len();
        vec![
            Finding::new(
                &EXCHANGE_DRIFT,
                [Target::new(Kind::PlcConfigProject, cfg)],
                "drift",
                name.clone(),
                format!(
                    "configuration project {name} changed since the {format_words} {direction} of {}: {count} object(s) \
                     differ ({list}{}); export it again, or import the PLC's file",
                    stamp.date,
                    if more > 0 { format!(" and {more} more") } else { String::new() }
                ),
            )
            .arg("cfg", name)
            .arg("format", format)
            .arg("direction", direction)
            .arg("date", stamp.date.to_string())
            .arg("count", count.to_string())
            .arg("objects", list)
            .arg(
                "more",
                if more > 0 { more.to_string() } else { "none".to_owned() },
            ),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PRC-002

/// EW-PRC-002 "Open instrument tag" (doc 15 §4.5.1): completeness, o, info, project (FR-PLC-050).
pub const OPEN_INSTRUMENT_TAG: RuleMeta = RuleMeta {
    id: "EW-PRC-002",
    code: "110202",
    pack: "EW-PLC",
    title: "Open instrument tag",
    kind: RuleKind::Completeness,
    modes: Modes::letters("o"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-prc-002",
};

/// EW-PRC-002: a row of an instrument list (an `exchange_record` of object `instrument`, doc 10 §4.7) whose
/// placeholder device is gone (discriminator `device`, target the configuration project) or that has no channel: none
/// was spare when the list was applied, or its channel is gone (`channel`, target the device). It is not a row of the open-item table yet: the table's rules run in release mode of
/// the shipped scheme, and this rule is online only and of the 1.0 content.
#[derive(Clone, Copy, Debug)]
pub struct OpenInstrumentTag;

impl Rule for OpenInstrumentTag {
    fn meta(&self) -> &'static RuleMeta {
        &OPEN_INSTRUMENT_TAG
    }

    fn run(&self, q: &Q, _scope: &Scope) -> Vec<Finding> {
        let channels = q.table::<Channel>();
        let devices = q.devices();
        let mut out = Vec::new();
        for (_, r) in q.table::<ExchangeRecord>().iter() {
            if r.object != ExchangeObject::Instrument {
                continue;
            }
            let tag = r.fields.get("tag").cloned().unwrap_or_default();
            let signal = r.fields.get("signal").cloned().unwrap_or_default();
            let device: Id<Device> = r.target.cast();
            let finding = if !devices.contains(device) {
                Finding::new(
                    &OPEN_INSTRUMENT_TAG,
                    [Target::new(Kind::PlcConfigProject, r.config_project)],
                    format!("device:{tag}"),
                    tag.clone(),
                    format!(
                        "instrument tag {tag} has no device any more; apply the instrument list again, or remove \
                         the tag from it"
                    ),
                )
                .arg("form", "device")
            } else if r
                .fields
                .get("channel")
                .and_then(|c| c.parse::<Id<Channel>>().ok())
                .is_none_or(|c| !channels.contains(c))
            {
                let name = full_tag(q, device, "a device");
                Finding::new(
                    &OPEN_INSTRUMENT_TAG,
                    [Target::new(Kind::Device, device)],
                    format!("channel:{tag}"),
                    name.clone(),
                    format!(
                        "instrument tag {tag} ({signal}) has no channel: no spare {signal} channel was free; add a \
                         module or free a channel, then apply the instrument list again"
                    ),
                )
                .arg("form", "channel")
                .arg("device", name)
            } else {
                continue;
            };
            out.push(finding.arg("tag", tag).arg("signal", signal));
        }
        out
    }
}
