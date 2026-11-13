// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-SAF rules (doc 10 §2.9, §3.10; doc 15 §4.5.1), in the 1.0 content of the EW-PLC pack: EW-SAF-001 (the
//! F-destination address), EW-SAF-002 (the 1oo2 pair) and EW-SAF-003 (the test pulse). They read the safety data of
//! modules and channels (`PlcModule.safety`, `Channel.safety`) and the derived targets; SIL and PL are fields, never
//! calculated (FR-PLC-041).

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Channel, ChannelKind, Device, Endpoint, Evaluation, Kind};
use ew_derive::{ChannelTargets, Q};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::plc::channel_name;
use crate::rules::term::full_tag;

// ---------------------------------------------------------------------------------------------------------
// EW-SAF-001

/// EW-SAF-001 "F-address" (doc 15 §4.5.1): safety, o r, error, project (FR-PLC-042).
pub const F_ADDRESS: RuleMeta = RuleMeta {
    id: "EW-SAF-001",
    code: "110101",
    pack: "EW-PLC",
    title: "F-address",
    kind: RuleKind::Safety,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-saf-001",
};

/// EW-SAF-001 over every F-module (a `plc_module` with `safety`): an F-destination address that is missing
/// (discriminator `missing`), outside the range of the module's device description (`range`), or used by two or more
/// F-modules of the project (`duplicate`: plant-wide uniqueness within the project covers every logical net). One
/// finding per module, and one per shared address with every module as a target.
#[derive(Clone, Copy, Debug)]
pub struct FAddress;

impl Rule for FAddress {
    fn meta(&self) -> &'static RuleMeta {
        &F_ADDRESS
    }

    fn run(&self, q: &Q, _scope: &Scope) -> Vec<Finding> {
        let mut out = Vec::new();
        let mut by_address: BTreeMap<u16, Vec<Id<Device>>> = BTreeMap::new();
        for (id, d) in q.devices().iter() {
            let Some(safety) = d.aspects.plc_module.as_ref().and_then(|m| m.safety.as_ref()) else {
                continue;
            };
            let module = full_tag(q, id, "an F-module");
            match safety.f_dest {
                None => out.push(
                    Finding::new(
                        &F_ADDRESS,
                        [Target::new(Kind::Device, id)],
                        "missing",
                        module.clone(),
                        format!("{module} is an F-module without an F-destination address; enter it"),
                    )
                    .arg("form", "missing")
                    .arg("module", module),
                ),
                Some(a) => {
                    by_address.entry(a).or_default().push(id);
                    if let Some(r) = safety.range.as_ref().filter(|r| !r.contains(a)) {
                        out.push(
                            Finding::new(
                                &F_ADDRESS,
                                [Target::new(Kind::Device, id)],
                                "range",
                                module.clone(),
                                format!(
                                    "{module}: the F-destination address {a} is outside {}..{}, the range of its \
                                     device description; enter an address inside it",
                                    r.min, r.max
                                ),
                            )
                            .arg("form", "range")
                            .arg("module", module)
                            .arg("address", a.to_string())
                            .arg("min", r.min.to_string())
                            .arg("max", r.max.to_string()),
                        );
                    }
                }
            }
        }
        for (address, modules) in by_address {
            if modules.len() < 2 {
                continue;
            }
            let mut names: Vec<String> = modules.iter().map(|m| full_tag(q, *m, "an F-module")).collect();
            names.sort_by(|a, b| ew_derive::ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)));
            let list = names.join(", ");
            out.push(
                Finding::new(
                    &F_ADDRESS,
                    modules.iter().map(|m| Target::new(Kind::Device, *m)),
                    format!("duplicate:{address}"),
                    names[0].clone(),
                    format!(
                        "the F-destination address {address} is used by {list}; give each F-module its own address"
                    ),
                )
                .arg("form", "duplicate")
                .arg("address", address.to_string())
                .arg("modules", list),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-SAF-002

/// EW-SAF-002 "1oo2 pair" (doc 15 §4.5.1): safety, f r, error, module (FR-PLC-043).
pub const PAIR: RuleMeta = RuleMeta {
    id: "EW-SAF-002",
    code: "110102",
    pack: "EW-PLC",
    title: "1oo2 pair",
    kind: RuleKind::Safety,
    modes: Modes::letters("fr"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-saf-002",
};

/// EW-SAF-002 over the 1oo2 channels of a module: a channel without a pair, or whose pair is not a 1oo2 channel that
/// names it back (`incomplete`, one finding per channel), and a complete pair whose two channels reach different
/// devices (`devices`, one finding per pair, on the channel with the lower ID, both channels as targets).
#[derive(Clone, Copy, Debug)]
pub struct Pair;

impl Rule for Pair {
    fn meta(&self) -> &'static RuleMeta {
        &PAIR
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let channels = q.table::<Channel>();
        let mut out = Vec::new();
        for id in q.get::<ew_derive::ModuleChannelIds>(module).iter().map(|(_, id)| *id) {
            let Some(c) = channels.get(id) else {
                continue;
            };
            let Some(s) = c.safety.as_ref().filter(|s| s.evaluation == Evaluation::OneOoTwo) else {
                continue;
            };
            let name = channel_name(q, module, c.id, c.number);
            let partner = s.pair.and_then(|p| channels.get(p));
            let complete = partner.is_some_and(|p| {
                p.safety
                    .as_ref()
                    .is_some_and(|ps| ps.evaluation == Evaluation::OneOoTwo && ps.pair == Some(c.id))
            });
            let Some(p) = partner.filter(|_| complete) else {
                out.push(
                    Finding::new(
                        &PAIR,
                        [Target::new(Kind::Channel, c.id)],
                        "incomplete",
                        name.clone(),
                        format!(
                            "{name} is a 1oo2 channel without a partner that names it back; pair it with the other \
                             channel of the sensor"
                        ),
                    )
                    .arg("form", "incomplete")
                    .arg("channel", name),
                );
                continue;
            };
            if p.id < c.id {
                continue;
            }
            let device = |ch: &Channel| {
                q.get::<ChannelTargets>(ch.module)
                    .iter()
                    .find(|t| t.channel == ch.id)
                    .and_then(|t| t.device)
            };
            let (a, b) = (device(c), device(p));
            if let (Some(a), Some(b)) = (a, b) {
                if a != b {
                    let other = channel_name(q, p.module, p.id, p.number);
                    let (ta, tb) = (full_tag(q, a, "a device"), full_tag(q, b, "a device"));
                    out.push(
                        Finding::new(
                            &PAIR,
                            [Target::new(Kind::Channel, c.id), Target::new(Kind::Channel, p.id)],
                            "devices",
                            name.clone(),
                            format!(
                                "the 1oo2 pair {name} and {other} reads two devices, {ta} and {tb}; wire both \
                                 channels to the one two-channel sensor"
                            ),
                        )
                        .arg("form", "devices")
                        .arg("channel", name)
                        .arg("other", other)
                        .arg("first", ta)
                        .arg("second", tb),
                    );
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-SAF-003

/// EW-SAF-003 "Test pulse" (doc 15 §4.5.1): safety, f r, warning, module (FR-PLC-043).
pub const TEST_PULSE: RuleMeta = RuleMeta {
    id: "EW-SAF-003",
    code: "110103",
    pack: "EW-PLC",
    title: "Test pulse",
    kind: RuleKind::Safety,
    modes: Modes::letters("fr"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-saf-003",
};

/// EW-SAF-003: an F-DI channel whose safety data names a test-pulse pin (`SafetyChannel.test_pulse`, the module's
/// sensor supply), whose target device has no CP on the signal of that pin: the sensor is not supplied from the
/// module's test pulse. One finding per channel; a channel without a target is EW-PLC-009's.
#[derive(Clone, Copy, Debug)]
pub struct TestPulse;

impl Rule for TestPulse {
    fn meta(&self) -> &'static RuleMeta {
        &TEST_PULSE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let channels = q.table::<Channel>();
        let mut out = Vec::new();
        for t in q.get::<ChannelTargets>(module).iter() {
            let Some(c) = channels.get(t.channel) else {
                continue;
            };
            let (Some(Endpoint::Cp(pf, pidx)), Some(device)) =
                (c.safety.as_ref().and_then(|s| s.test_pulse.clone()), t.device)
            else {
                continue;
            };
            if c.kind != ChannelKind::Di {
                continue;
            }
            let supplied = signal_devices(q, pf, pidx).contains(&device);
            if supplied {
                continue;
            }
            let name = channel_name(q, module, c.id, c.number);
            let sensor = full_tag(q, device, "the sensor");
            out.push(
                Finding::new(
                    &TEST_PULSE,
                    [Target::new(Kind::Channel, c.id)],
                    "pulse",
                    name.clone(),
                    format!(
                        "{name}: the sensor {sensor} is not supplied from the module's test-pulse pin; wire its supply \
                         to that pin"
                    ),
                )
                .arg("channel", name)
                .arg("sensor", sensor),
            );
        }
        out
    }
}

/// The devices with a CP on the signal of CP `idx` of `function`: the stitch graph's net and signal edges from it.
fn signal_devices(q: &Q, function: Id<ew_derive::ew_schema::Function>, idx: u16) -> std::collections::BTreeSet<Id<Device>> {
    use ew_derive::Level;
    let graph = q.get::<ew_derive::StitchGraphQ>(());
    let mut out = std::collections::BTreeSet::new();
    let Some(start) = graph.node_of(ew_derive::EndpointKey::Cp(function, idx)) else {
        return out;
    };
    let mut seen = std::collections::BTreeSet::from([start]);
    let mut stack = vec![start];
    while let Some(n) = stack.pop() {
        for m in &graph.nodes[n].members {
            if let Some(cp) = ew_derive::CpRef::of(*m) {
                if let Some(d) = q.functions().get(cp.function).and_then(|f| f.device) {
                    out.insert(d);
                }
            }
        }
        for e in graph.edges.iter().filter(|e| matches!(e.level, Level::Net | Level::Signal)) {
            let next = if e.a == n {
                e.b
            } else if e.b == n {
                e.a
            } else {
                continue;
            };
            if seen.insert(next) {
                stack.push(next);
            }
        }
    }
    out
}
