// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-FL pack, its MVP rules (doc 15 §4.5.3, doc 11 §6; WP-S1.20 D16): EW-FL-001, -002, -004, -011 and -013.
//!
//! # Determinism
//!
//! Texts name devices by their printed tag (`-201U3-Q1`), ports by their designation and channels as "channel 1 of
//! -201U3"; lists are in position, designation or channel-number order. IDs enter only targets and fingerprints.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Channel, ContactKind, Device, DeviceState, FluidPortRole, Function, Kind};
use ew_derive::{
    ChannelsByCp, DeviceTag, EndpointKey, FluidNets, FunctionCps, Q, ValveSlot, ValveSlots, driven_device,
    reached_device, valve_solenoids,
};

use crate::engine::touched_ids;
use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};

// ---------------------------------------------------------------------------------------------------------
// Shared texts

/// A device as the messages print it: its displayed tag without structure (`-201U3-Q1`), else `device`.
fn tag(q: &Q, d: Id<Device>) -> String {
    q.get::<DeviceTag>(d)
        .as_ref()
        .as_ref()
        .map_or_else(|| "device".to_owned(), |t| t.short())
}

/// A device's full tag, the findings' subject (`=P01+J-E-2416-201U3-Q1`).
fn full(q: &Q, d: Id<Device>) -> String {
    super::term::full_tag(q, d, "device")
}

/// A channel as the messages print it: "channel 1 of -201U3".
fn channel_text(q: &Q, c: &Channel) -> String {
    format!("channel {} of {}", c.number, tag(q, c.module))
}

/// The same in nl-NL: "kanaal 1 van -201U3".
fn channel_text_nl(q: &Q, c: &Channel) -> String {
    format!("kanaal {} van {}", c.number, tag(q, c.module))
}

/// "A, B and C".
fn and_list(items: &[String], and: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} {and} {last}", init.join(", ")),
    }
}

/// The contact kind of a function's FnDef.
fn contact_kind(q: &Q, f: Id<Function>) -> Option<ContactKind> {
    q.functions()
        .get(f)
        .and_then(|f| q.fndefs().get(f.def))
        .map(|d| d.item.contact_kind.clone())
}

/// The functions of a device on a FnDef of contact kind `kind`, in ID order.
fn functions_of_kind(q: &Q, d: Id<Device>, kind: &ContactKind) -> Vec<Id<Function>> {
    let mut out: Vec<Id<Function>> = q
        .idx()
        .functions_of_device(d)
        .filter(|f| contact_kind(q, *f).as_ref() == Some(kind))
        .collect();
    out.sort();
    out
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-001

/// EW-FL-001 "Working port open" (doc 15 §4.5.3): completeness, o r, warning, device.
pub const WORKING_PORT_OPEN: RuleMeta = RuleMeta {
    id: "EW-FL-001",
    code: "120001",
    pack: "EW-FL",
    title: "Working port open",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-FL-001 (FR-FLU-018, WP-S1.20 D16): per placed valve function (a function on a FnDef of contact kind `valve`
/// with a representation) of a valve not in state `not_fitted`, its working ports (`working2`, `working4`) whose fluid
/// net has no other member and that carry no closure (`fluid.port.close`): one finding listing them, target the
/// function.
#[derive(Clone, Copy, Debug)]
pub struct WorkingPortOpen;

impl Rule for WorkingPortOpen {
    fn meta(&self) -> &'static RuleMeta {
        &WORKING_PORT_OPEN
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(d) = *scope else {
            return Vec::new();
        };
        let Some(dev) = q.devices().get(d) else {
            return Vec::new();
        };
        if dev.state == DeviceState::NotFitted {
            return Vec::new();
        }
        let nets = q.get::<FluidNets>(());
        let mut out = Vec::new();
        for f in functions_of_kind(q, d, &ContactKind::Valve) {
            if q.idx().reps_of_function(f).next().is_none() {
                continue;
            }
            let Some(func) = q.functions().get(f) else {
                continue;
            };
            let open: Vec<String> = q
                .get::<FunctionCps>(f)
                .iter()
                .filter(|cp| {
                    matches!(
                        cp.port_role,
                        Some(FluidPortRole::Working2 | FluidPortRole::Working4)
                    )
                })
                .filter(|cp| {
                    !func
                        .cp_overrides
                        .iter()
                        .any(|o| o.idx == cp.idx && o.closure.is_some())
                })
                .filter(|cp| {
                    nets.net_of(EndpointKey::Cp(f, cp.idx))
                        .is_none_or(|n| n.members.len() < 2)
                })
                .map(|cp| cp.designation.clone())
                .collect();
            let valve = tag(q, d);
            let finding = match open.as_slice() {
                [] => continue,
                [port] => Finding::new(
                    &WORKING_PORT_OPEN,
                    [Target::new(Kind::Function, f)],
                    "open",
                    full(q, d),
                    format!("{valve}: working port {port} is open; connect, plug or blank it"),
                )
                .arg("form", "one")
                .arg("port", port.clone()),
                ports => Finding::new(
                    &WORKING_PORT_OPEN,
                    [Target::new(Kind::Function, f)],
                    "open",
                    full(q, d),
                    format!(
                        "{valve}: working ports {} are open; connect, plug or blank them",
                        and_list(ports, "and")
                    ),
                )
                .arg("form", "many")
                .arg("ports", and_list(ports, "and"))
                .arg("ports_nl", and_list(ports, "en")),
            };
            out.push(finding.arg("valve", valve));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-002

/// EW-FL-002 "Coil bindings against solenoids" (doc 15 §4.5.3): conflict, o r, error (excess) / warning (unbound),
/// device.
pub const COIL_BINDINGS: RuleMeta = RuleMeta {
    id: "EW-FL-002",
    code: "120002",
    pack: "EW-FL",
    title: "Coil bindings against solenoids",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-FL-002 (FR-FLU-017, FI-03, WP-S1.20 D16): per valve whose valve part declares solenoids and that has internal
/// coils (non-physical CPs of functions on a FnDef of contact kind `coil`; physical coils are skipped): an internal
/// coil CP of a declared solenoid that more than one channel binds (error), a channel bound to an internal coil the
/// part does not declare (error), and, on a valve in state `used`, a declared solenoid whose internal coil no channel
/// binds (warning). Spare, reserved and not-fitted valves give no warning.
#[derive(Clone, Copy, Debug)]
pub struct CoilBindings;

impl Rule for CoilBindings {
    fn meta(&self) -> &'static RuleMeta {
        &COIL_BINDINGS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(d) = *scope else {
            return Vec::new();
        };
        let Some(dev) = q.devices().get(d) else {
            return Vec::new();
        };
        let parts = q.parts();
        let Some(declared) = dev.parts.iter().find_map(|u| {
            parts
                .get(u.part.copy)
                .and_then(|c| valve_solenoids(&c.item).map(<[_]>::to_vec))
        }) else {
            return Vec::new();
        };
        // the internal coil CPs: (designation, function, index), in function and index order
        let mut coils: Vec<(String, Id<Function>, u16)> = Vec::new();
        for f in functions_of_kind(q, d, &ContactKind::Coil) {
            for cp in q.get::<FunctionCps>(f).iter().filter(|cp| !cp.physical) {
                coils.push((cp.designation.clone(), f, cp.idx));
            }
        }
        if coils.is_empty() {
            return Vec::new();
        }
        let bound = q.get::<ChannelsByCp>(());
        let channels = q.table::<Channel>();
        let valve = tag(q, d);
        let mut out = Vec::new();
        let mut served: BTreeSet<String> = BTreeSet::new();
        for (designation, f, idx) in &coils {
            let mut at: Vec<(u16, Id<Channel>, Channel)> = bound
                .at_cp(*f, *idx)
                .filter_map(|c| channels.get(c).map(|x| (x.number, c, Channel::clone(x))))
                .collect();
            at.sort_by_key(|(n, c, _)| (*n, *c));
            let declares = declared.iter().any(|s| s.as_str() == designation);
            if declares && !at.is_empty() {
                served.insert(designation.clone());
            }
            if declares && at.len() > 1 {
                let names: Vec<String> = at.iter().map(|(_, _, c)| channel_text(q, c)).collect();
                let names_nl: Vec<String> = at.iter().map(|(_, _, c)| channel_text_nl(q, c)).collect();
                let mut targets = vec![Target::new(Kind::Function, *f)];
                targets.extend(at.iter().map(|(_, c, _)| Target::new(Kind::Channel, *c)));
                out.push(
                    Finding::new(
                        &COIL_BINDINGS,
                        targets,
                        format!("excess:{designation}"),
                        full(q, d),
                        format!(
                            "{valve}: solenoid {designation} is bound to {} channels: {}",
                            at.len(),
                            names.join(", ")
                        ),
                    )
                    .arg("form", "excess")
                    .arg("valve", valve.clone())
                    .arg("solenoid", designation.clone())
                    .arg("count", at.len().to_string())
                    .arg("channels", names.join(", "))
                    .arg("channels_nl", names_nl.join(", ")),
                );
            }
            if !declares {
                for (_, c, ch) in &at {
                    out.push(
                        Finding::new(
                            &COIL_BINDINGS,
                            [Target::new(Kind::Function, *f), Target::new(Kind::Channel, *c)],
                            "undeclared",
                            full(q, d),
                            format!(
                                "{valve}: {} drives a coil that the part does not declare",
                                channel_text(q, ch)
                            ),
                        )
                        .arg("form", "undeclared")
                        .arg("valve", valve.clone())
                        .arg("channel", channel_text(q, ch))
                        .arg("channel_nl", channel_text_nl(q, ch)),
                    );
                }
            }
        }
        if dev.state == DeviceState::Used {
            for s in &declared {
                if served.contains(s.as_str()) {
                    continue;
                }
                out.push(
                    Finding::new(
                        &COIL_BINDINGS,
                        [Target::new(Kind::Device, d)],
                        format!("unbound:{s}"),
                        full(q, d),
                        format!("{valve}: solenoid {s} has no channel"),
                    )
                    .with_severity(Severity::Warning)
                    .arg("form", "unbound")
                    .arg("valve", valve.clone())
                    .arg("solenoid", s.as_str()),
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-004

/// EW-FL-004 "Spare positions" (doc 15 §4.5.3): hygiene, o r (WP-S1.20 D16, §13 O2), info, device.
pub const SPARE_POSITIONS: RuleMeta = RuleMeta {
    id: "EW-FL-004",
    code: "120004",
    pack: "EW-FL",
    title: "Spare positions",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-FL-004 (FR-FLU-017, WP-S1.20 D16): one finding per position of a module that holds a spare valve or a blanking
/// plate (`valve_slots`), target the valve or the plate. It runs in the release run too, where AC-21 counts EF-14.
#[derive(Clone, Copy, Debug)]
pub struct SparePositions;

impl Rule for SparePositions {
    fn meta(&self) -> &'static RuleMeta {
        &SPARE_POSITIONS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(m) = *scope else {
            return Vec::new();
        };
        let slots = q.get::<ValveSlots>(m);
        let module = tag(q, m);
        let mut out = Vec::new();
        for (i, slot) in slots.slots.iter().enumerate() {
            let p = (i + 1).to_string();
            let (device, form, message) = match slot {
                ValveSlot::Valve {
                    device,
                    state: DeviceState::Spare,
                } => (
                    *device,
                    "valve",
                    format!("{module}: position {p} holds a spare valve, {}", tag(q, *device)),
                ),
                ValveSlot::Blank { device } => (
                    *device,
                    "blank",
                    format!(
                        "{module}: position {p} holds a blanking plate, {}",
                        tag(q, *device)
                    ),
                ),
                _ => continue,
            };
            out.push(
                Finding::new(
                    &SPARE_POSITIONS,
                    [Target::new(Kind::Device, device)],
                    form,
                    full(q, device),
                    message,
                )
                .arg("form", form)
                .arg("module", module.clone())
                .arg("position", p)
                .arg("device", tag(q, device)),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-011

/// EW-FL-011 "`Drives` target mismatch" (doc 15 §4.5.3): conflict, o, warning, device.
pub const DRIVES_MISMATCH: RuleMeta = RuleMeta {
    id: "EW-FL-011",
    code: "120011",
    pack: "EW-FL",
    title: "Drives target mismatch",
    kind: RuleKind::Conflict,
    modes: Modes::letters("o"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-FL-011 (FR-FLU-029, WP-S1.20 D16): a valve with a `drives` link whose target differs from the device its
/// working-port lines reach (`reached_device`), when that device exists; target the valve.
#[derive(Clone, Copy, Debug)]
pub struct DrivesMismatch;

impl Rule for DrivesMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &DRIVES_MISMATCH
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(d) = *scope else {
            return Vec::new();
        };
        let (Some(target), Some(reached)) = (driven_device(q, d), reached_device(q, d)) else {
            return Vec::new();
        };
        if target == reached {
            return Vec::new();
        }
        let (valve, target, reached) = (tag(q, d), tag(q, target), tag(q, reached));
        vec![
            Finding::new(
                &DRIVES_MISMATCH,
                [Target::new(Kind::Device, d)],
                "drives",
                full(q, d),
                format!("{valve} drives {target}, but its working-port lines reach {reached}"),
            )
            .arg("valve", valve)
            .arg("target", target)
            .arg("device", reached),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-013

/// EW-FL-013 "Occupancy" (doc 15 §4.5.3): conflict, p o, error (empty: warning), device.
pub const OCCUPANCY: RuleMeta = RuleMeta {
    id: "EW-FL-013",
    code: "120013",
    pack: "EW-FL",
    title: "Occupancy",
    kind: RuleKind::Conflict,
    modes: Modes::letters("po"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// The modules among a device's children, by position: position → modules in ID order.
fn modules_by_position(q: &Q, parent: Id<Device>) -> BTreeMap<u32, Vec<Id<Device>>> {
    let devices = q.devices();
    let mut out: BTreeMap<u32, Vec<Id<Device>>> = BTreeMap::new();
    let mut children: Vec<Id<Device>> = q.idx().device_children(parent).collect();
    children.sort();
    for c in children {
        if ew_derive::valve_positions(q, c).is_none() {
            continue;
        }
        if let Some(p) = devices.get(c).and_then(|d| d.position) {
            out.entry(p).or_default().push(c);
        }
    }
    out
}

/// The device a position of a module is held by, if any.
fn holder(slot: &ValveSlot) -> Option<Id<Device>> {
    match slot {
        ValveSlot::Valve { device, .. } | ValveSlot::Blank { device } => Some(*device),
        ValveSlot::Empty => None,
    }
}

/// T13: "<parent>: position <p> holds both <a> and <b>", target `b`.
fn both(q: &Q, parent: Id<Device>, p: u32, a: Id<Device>, b: Id<Device>, form: &str) -> Finding {
    let (parent_t, a_t, b_t) = (tag(q, parent), tag(q, a), tag(q, b));
    Finding::new(
        &OCCUPANCY,
        [Target::new(Kind::Device, b)],
        form,
        full(q, b),
        format!("{parent_t}: position {p} holds both {a_t} and {b_t}"),
    )
    .arg("form", "both")
    .arg("parent", parent_t)
    .arg("position", p.to_string())
    .arg("a", a_t)
    .arg("b", b_t)
}

/// T12: "<module>: position <p> is outside its <n> valve positions", target the child; `tail` is the refusal's
/// proposal (`; free positions: 1, 3` or `; every position is taken`), empty in a report.
fn outside(q: &Q, module: Id<Device>, n: u16, p: u32, child: Id<Device>, tail: (&str, String)) -> Finding {
    let module_t = tag(q, module);
    let (form, free) = tail;
    let suffix = match form {
        "outside_free" => format!("; free positions: {free}"),
        "outside_full" => "; every position is taken".to_owned(),
        _ => String::new(),
    };
    Finding::new(
        &OCCUPANCY,
        [Target::new(Kind::Device, child)],
        "outside",
        full(q, child),
        format!("{module_t}: position {p} is outside its {n} valve positions{suffix}"),
    )
    .arg("form", form)
    .arg("module", module_t)
    .arg("position", p.to_string())
    .arg("count", n.to_string())
    .arg("free", free)
}

/// EW-FL-013 (FR-FLU-027, FI-02, FI-07; WP-S1.20 D16): per module (`valve_slots`), a second child at a position (T13)
/// and a child outside 1 … n (T12), errors, and each empty position (T19), a warning; per device, two modules of its
/// children at one position (FI-07, T13), an error. Prevent mode refuses a batch that puts a touched device at a held
/// position or outside its module's positions (the two error cases), the latter with the free positions or "every
/// position is taken".
#[derive(Clone, Copy, Debug)]
pub struct Occupancy;

impl Rule for Occupancy {
    fn meta(&self) -> &'static RuleMeta {
        &OCCUPANCY
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(d) = *scope else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let slots = q.get::<ValveSlots>(d);
        for (p, child) in &slots.second {
            if let Some(a) = usize::try_from(*p)
                .ok()
                .and_then(|p| slots.slots.get(p - 1))
                .and_then(holder)
            {
                out.push(both(q, d, *p, a, *child, "both"));
            }
        }
        for (p, child) in &slots.outside {
            out.push(outside(q, d, slots.count, *p, *child, ("outside", String::new())));
        }
        let module = tag(q, d);
        for (i, slot) in slots.slots.iter().enumerate() {
            if *slot != ValveSlot::Empty {
                continue;
            }
            let p = i + 1;
            out.push(
                Finding::new(
                    &OCCUPANCY,
                    [Target::new(Kind::Device, d)],
                    format!("empty:{p}"),
                    full(q, d),
                    format!("{module}: position {p} is empty; fit a valve or a blanking plate"),
                )
                .with_severity(Severity::Warning)
                .arg("form", "empty")
                .arg("module", module.clone())
                .arg("position", p.to_string()),
            );
        }
        for (p, modules) in modules_by_position(q, d) {
            if let [a, rest @ ..] = modules.as_slice() {
                for b in rest {
                    out.push(both(q, d, p, *a, *b, "modules"));
                }
            }
        }
        out
    }

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let devices = after.devices();
        let mut out = Vec::new();
        for t in touched_ids::<Device>(touched, Kind::Device) {
            let Some(dev) = devices.get(t) else { continue };
            let (Some(parent), Some(p)) = (dev.parent, dev.position) else {
                continue;
            };
            let unchanged = before
                .devices()
                .get(t)
                .is_some_and(|b| b.parent == dev.parent && b.position == dev.position);
            if unchanged {
                continue;
            }
            // FI-02: a valve or plate at a held position, or outside the module's positions
            let slots = after.get::<ValveSlots>(parent);
            if slots.outside.contains(&(p, t)) {
                let free: Vec<String> = slots
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| **s == ValveSlot::Empty)
                    .map(|(i, _)| (i + 1).to_string())
                    .collect();
                let tail = if free.is_empty() {
                    ("outside_full", String::new())
                } else {
                    ("outside_free", free.join(", "))
                };
                out.push(outside(after, parent, slots.count, p, t, tail));
            }
            if slots.second.contains(&(p, t))
                && let Some(a) = usize::try_from(p)
                    .ok()
                    .and_then(|i| slots.slots.get(i - 1))
                    .and_then(holder)
            {
                out.push(both(after, parent, p, a, t, "both"));
            }
            // FI-07: a module at a position another module of the same parent holds
            if ew_derive::valve_positions(after, t).is_some()
                && let Some(others) = modules_by_position(after, parent).get(&p)
                && let Some(a) = others.iter().find(|m| **m != t)
            {
                out.push(both(after, parent, p, *a, t, "modules"));
            }
        }
        out
    }
}
