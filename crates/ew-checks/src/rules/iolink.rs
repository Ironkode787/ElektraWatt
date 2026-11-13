// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-IOL rules of the EW-BUS pack's 1.0 content (doc 10 §3.8, §3.10; doc 15 §4.5.2; FR-BUS-019): EW-IOL-001 (port
//! class and mode) and EW-IOL-002 (process data and cable length). They read the port windows of an IO-Link master
//! (`ew_derive::port_windows`), the master part's window and the IO-Link data of the connected device's part.

use ew_derive::ew_schema::{Channel, ConnectionKind, Endpoint, Kind, LengthSource, PortClass, PortMode};
use ew_derive::{ChannelTargets, Q, iolink_device, module_part, part_confidence, port_windows};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::plc::channel_name;
use crate::rules::term::full_tag;

fn class_word(c: &PortClass) -> &'static str {
    match c {
        PortClass::A => "A",
        PortClass::B => "B",
        PortClass::Unknown(_) => "?",
    }
}

fn mode_word(m: &PortMode) -> &'static str {
    match m {
        PortMode::IoLink => "IO-Link",
        PortMode::Di => "DI",
        PortMode::Dq => "DQ",
        PortMode::Deactivated => "deactivated",
        PortMode::Unknown(_) => "other",
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-IOL-001

/// EW-IOL-001 "IO-Link port class" (doc 15 §4.5.2): conflict, o r, error, module (FR-BUS-019).
pub const PORT_CLASS: RuleMeta = RuleMeta {
    id: "EW-IOL-001",
    code: "111101",
    pack: "EW-BUS",
    title: "IO-Link port class",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-iol-001",
};

/// EW-IOL-001 over the ports of an IO-Link master: a connected IO-Link device of class B on a class-A port (`class`),
/// and an IO-Link device on a port in another mode than IO-Link (`mode`). One finding per port, target the channel.
#[derive(Clone, Copy, Debug)]
pub struct PortClassRule;

impl Rule for PortClassRule {
    fn meta(&self) -> &'static RuleMeta {
        &PORT_CLASS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let (Some(windows), Some((copy, plc))) = (port_windows(q, module), module_part(q, module)) else {
            return Vec::new();
        };
        let Some(window) = plc.iolink.as_ref() else {
            return Vec::new();
        };
        let confidence = part_confidence(&copy, "plc");
        let targets = q.get::<ChannelTargets>(module);
        let mut out = Vec::new();
        for w in &windows {
            let Some(device) = targets.iter().find(|t| t.channel == w.channel).and_then(|t| t.device) else {
                continue;
            };
            let Some(data) = iolink_device(q, device) else {
                continue;
            };
            let channel = channel_name(q, module, w.channel, w.number);
            let tag = full_tag(q, device, "the device");
            if w.mode != PortMode::IoLink {
                let mode = mode_word(&w.mode);
                out.push(
                    Finding::new(
                        &PORT_CLASS,
                        [Target::new(Kind::Channel, w.channel)],
                        "mode",
                        channel.clone(),
                        format!(
                            "{channel} is in mode {mode}, but the IO-Link device {tag} is connected; set the port to \
                             IO-Link"
                        ),
                    )
                    .arg("form", "mode")
                    .arg("channel", channel)
                    .arg("mode", mode)
                    .arg("device", tag),
                );
            } else if data.class == PortClass::B && window.class == PortClass::A {
                out.push(
                    Finding::new(
                        &PORT_CLASS,
                        [Target::new(Kind::Channel, w.channel)],
                        "class",
                        channel.clone(),
                        format!(
                            "{channel} is a class-{} port, but {tag} needs class {}; connect it to a class-B port",
                            class_word(&window.class),
                            class_word(&data.class)
                        ),
                    )
                    .arg("form", "class")
                    .arg("channel", channel)
                    .arg("port_class", class_word(&window.class))
                    .arg("device", tag)
                    .arg("device_class", class_word(&data.class))
                    .reading(&confidence),
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-IOL-002

/// EW-IOL-002 "IO-Link limits" (doc 15 §4.5.2): conflict, o r, warning, module (FR-BUS-019).
pub const LIMITS: RuleMeta = RuleMeta {
    id: "EW-IOL-002",
    code: "111102",
    pack: "EW-BUS",
    title: "IO-Link limits",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-iol-002",
};

/// EW-IOL-002 over the ports of an IO-Link master: a connected device whose process data is larger than the part's
/// port window (`window`, per direction), and a cable at the port's pin longer than the part's maximum (`cable`: a core
/// of a cable whose manual length exceeds `IoLinkWindow.max_cable_m`). One finding per port and kind, target the
/// channel.
#[derive(Clone, Copy, Debug)]
pub struct Limits;

impl Rule for Limits {
    fn meta(&self) -> &'static RuleMeta {
        &LIMITS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let (Some(windows), Some((copy, plc))) = (port_windows(q, module), module_part(q, module)) else {
            return Vec::new();
        };
        let Some(window) = plc.iolink.as_ref() else {
            return Vec::new();
        };
        let confidence = part_confidence(&copy, "plc");
        let channels = q.table::<Channel>();
        let mut out = Vec::new();
        for w in &windows {
            let channel = channel_name(q, module, w.channel, w.number);
            if let Some(d) = &w.device {
                for (dir, have, room) in [
                    ("input", u32::from(d.input_bits), u32::from(window.input_bytes) * 8),
                    ("output", u32::from(d.output_bits), u32::from(window.output_bytes) * 8),
                ] {
                    if have > room {
                        out.push(
                            Finding::new(
                                &LIMITS,
                                [Target::new(Kind::Channel, w.channel)],
                                format!("window:{dir}"),
                                channel.clone(),
                                format!(
                                    "{channel}: the device's {have} {dir} bits exceed the port window of {room} bits; \
                                     use a master with a larger window"
                                ),
                            )
                            .arg("form", "window")
                            .arg("channel", channel.clone())
                            .arg("dir", dir)
                            .arg("have", have.to_string())
                            .arg("room", room.to_string())
                            .reading(&confidence),
                        );
                    }
                }
            }
            let Some(max) = window.max_cable_m else {
                continue;
            };
            let Some(Endpoint::Cp(f, idx)) = channels.get(w.channel).and_then(|c| c.binding.clone()) else {
                continue;
            };
            let mut at: Vec<_> = q.idx().connections_at_cp(f, idx).collect();
            at.sort();
            for c in at {
                let Some(ConnectionKind::Core(core)) = q.connections().get(c).map(|c| c.kind.clone()) else {
                    continue;
                };
                let Some(cable) = q.conductors().get(core).map(|c| c.cable) else {
                    continue;
                };
                let Some(LengthSource::Manual(nm)) =
                    q.devices().get(cable).and_then(|d| d.aspects.cable.as_ref().map(|c| c.length.clone()))
                else {
                    continue;
                };
                let metres = nm.0.div_euclid(1_000_000_000);
                if metres > i64::from(max) {
                    let tag = full_tag(q, cable, "the cable");
                    out.push(
                        Finding::new(
                            &LIMITS,
                            [Target::new(Kind::Channel, w.channel), Target::new(Kind::Device, cable)],
                            "cable",
                            channel.clone(),
                            format!(
                                "{channel}: the cable {tag} is {metres} m long, more than the {max} m an IO-Link port \
                                 allows; shorten it"
                            ),
                        )
                        .arg("form", "cable")
                        .arg("channel", channel.clone())
                        .arg("cable", tag)
                        .arg("metres", metres.to_string())
                        .arg("max", max.to_string())
                        .reading(&confidence),
                    );
                }
            }
        }
        out
    }
}
