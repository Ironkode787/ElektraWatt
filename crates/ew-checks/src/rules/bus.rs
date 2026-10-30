// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-BUS rules of the MVP (doc 15 §4.5.2; doc 10 §3.8, §3.10; WP-S1.21 D12): EW-BUS-001 (a station address
//! missing or duplicate in a logical net) and EW-BUS-004 (the termination at the segment ends). They read the bus
//! model of D11: the ports (`Function.bus_port`), `net_members(lnet)` and `bus_segments(pnet)`.

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{AddrValue, BusPort, Function, FunctionKind, Kind, NetAddress, OnOff, PortRole};
use ew_derive::{BusSegments, NetMembersOf, Q, SegmentMember, external_partner_text};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

/// A port as printed: a connector by its tag ("-3X1"), any other port by its tag and interface ("-201U1:DP"); a port
/// on no device by its interface.
pub(crate) fn port_name(q: &Q, function: &Function, port: &BusPort) -> String {
    match function.device {
        Some(d) if port.role == PortRole::Passive => full_tag(q, d, "a connector"),
        Some(d) => format!("{}:{}", full_tag(q, d, "a device"), port.interface),
        None => port.interface.as_str().to_owned(),
    }
}

/// An end of a segment as printed: a port by [`port_name`]; an external interruption point by its name and the
/// partner its external reference names ("external end DP (+J-E-206 / JE-N1000-03)").
fn end_name(q: &Q, f: Id<Function>) -> String {
    let Some(function) = q.functions().get(f) else {
        return "a segment end".to_owned();
    };
    if let Some(port) = &function.bus_port {
        return port_name(q, function, port);
    }
    match &function.kind {
        FunctionKind::Ip(ip) => match ip.external {
            Some(e) => format!("external end {} ({})", ip.name, external_partner_text(q, e)),
            None => format!("end {}", ip.name),
        },
        _ => "a segment end".to_owned(),
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-BUS-001

/// EW-BUS-001 "Bus address missing or duplicate" (doc 15 §4.5.2): completeness, o r, error, lnet (FR-BUS-015,
/// FR-CHK-034); supersedes EW-CORE-044 for bus addresses.
pub const BUS_ADDRESS: RuleMeta = RuleMeta {
    id: "EW-BUS-001",
    code: "111001",
    pack: "EW-BUS",
    title: "Bus address missing or duplicate",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Lnet,
    supersedes: &["EW-CORE-044"],
    release: Release::Mvp,
};

/// EW-BUS-001: over the ports of the logical net (`net_members(lnet)`), one finding per port whose station address
/// is the open item `tbd` (an open item that blocks release, D-15; Golden A's EF-03), with the port function as
/// target and discriminator `address`, which supersedes EW-CORE-044's finding for the same port; and one finding
/// per PROFIBUS address that two or more ports share, with those ports as targets and discriminator `duplicate`. A
/// port without an address (`not_applicable`) and an address form of a newer format are not checked. An error in
/// both modes.
#[derive(Clone, Copy, Debug)]
pub struct BusAddress;

impl Rule for BusAddress {
    fn meta(&self) -> &'static RuleMeta {
        &BUS_ADDRESS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Lnet(net) = *scope else {
            return Vec::new();
        };
        let net_name = q
            .table::<ew_derive::ew_schema::LogicalNet>()
            .get(net)
            .map_or_else(String::new, |n| n.name.as_str().to_owned());
        let functions = q.functions();
        let mut out = Vec::new();
        let mut by_address: BTreeMap<u8, Vec<Id<Function>>> = BTreeMap::new();
        for f in &q.get::<NetMembersOf>(net).ports {
            let Some(function) = functions.get(*f) else {
                continue;
            };
            let Some(port) = &function.bus_port else {
                continue;
            };
            match &port.address {
                AddrValue::Tbd => {
                    let name = port_name(q, function, port);
                    out.push(
                        Finding::new(
                            &BUS_ADDRESS,
                            [Target::new(Kind::Function, *f)],
                            "address",
                            name.clone(),
                            format!("{name}: the station address is TBD; enter it before release"),
                        )
                        .arg("form", "tbd")
                        .arg("port", name),
                    );
                }
                AddrValue::Value(NetAddress::Profibus(n)) => by_address.entry(*n).or_default().push(*f),
                _ => {}
            }
        }
        for (address, ports) in by_address {
            if ports.len() < 2 {
                continue;
            }
            let names: Vec<String> = ports
                .iter()
                .filter_map(|f| {
                    let function = functions.get(*f)?;
                    let port = function.bus_port.as_ref()?;
                    Some(port_name(q, function, port))
                })
                .collect();
            let list = names.join(", ");
            out.push(
                Finding::new(
                    &BUS_ADDRESS,
                    ports.iter().map(|f| Target::new(Kind::Function, *f)),
                    "duplicate",
                    list.clone(),
                    format!(
                        "{list}: station address {address} is used more than once in logical net {net_name}; give \
                         each port its own address"
                    ),
                )
                .arg("form", "duplicate")
                .arg("ports", list)
                .arg("address", address.to_string())
                .arg("net", net_name.clone()),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-BUS-004

/// EW-BUS-004 "Termination" (doc 15 §4.5.2): conflict, o r, error (an unknown end: info), pnet (FR-BUS-016).
pub const TERMINATION: RuleMeta = RuleMeta {
    id: "EW-BUS-004",
    code: "111004",
    pack: "EW-BUS",
    title: "Termination",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Pnet,
    supersedes: &[],
    release: Release::Mvp,
};

/// The terminating element an end claims, walking inward from it: the index into the segment's functions.
fn nearest_terminating(ports: &[(Id<Function>, Option<OnOff>)], from_start: bool) -> Option<usize> {
    let mut indices: Vec<usize> = (0..ports.len()).collect();
    if !from_start {
        indices.reverse();
    }
    indices.into_iter().find(|i| ports[*i].1.is_some())
}

/// EW-BUS-004 over the segments of the physical net (`bus_segments(pnet)`, doc 10 §3.8): at each end the nearest
/// terminating element (a port with a termination switch), walking inward, must be On, and every other terminating
/// element of the segment Off. Findings, with the port function as target:
/// - an end's element that is Off (discriminator `end`), an error;
/// - an element inside the segment that is On (`middle`), an error;
/// - an end of this project without any terminating element left to claim (`missing`, the end as target), an
///   error;
/// - an external end (an external interruption point: the partner is outside the project, its termination
///   unknown), one info (`external`, the interruption point as target; Golden A's EF-16);
/// - an element whose state is of a newer format (`unknown`), an info.
///
/// An external end claims no element: the partner terminates its own end. A segment with one end (a lone port) and
/// a ring (no end) are checked only for elements that are On inside them.
#[derive(Clone, Copy, Debug)]
pub struct Termination;

impl Rule for Termination {
    fn meta(&self) -> &'static RuleMeta {
        &TERMINATION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Pnet(net) = *scope else {
            return Vec::new();
        };
        let functions = q.functions();
        let mut out = Vec::new();
        for segment in q.get::<BusSegments>(net).iter() {
            let ports: Vec<(Id<Function>, Option<OnOff>)> = segment
                .functions()
                .map(|f| {
                    let state = functions
                        .get(f)
                        .and_then(|x| x.bus_port.as_ref().and_then(|p| p.termination.clone()));
                    (f, state)
                })
                .collect();
            let first = segment.members.iter().find_map(|m| match m {
                SegmentMember::Function(f) => Some(*f),
                SegmentMember::Cable(_) => None,
            });
            let last = segment.members.iter().rev().find_map(|m| match m {
                SegmentMember::Function(f) => Some(*f),
                SegmentMember::Cable(_) => None,
            });
            let mut claimed: Vec<usize> = Vec::new();
            // The two walk ends of a line (the start end and the end the walk finishes at).
            let line_ends: Vec<(Id<Function>, bool)> = match (first, last) {
                (Some(a), Some(b)) if a != b && segment.ends.contains(&a) && segment.ends.contains(&b) => {
                    vec![(a, true), (b, false)]
                }
                _ => Vec::new(),
            };
            for (end, from_start) in line_ends {
                if segment.external_ends.contains(&end) {
                    let name = end_name(q, end);
                    out.push(
                        Finding::new(
                            &TERMINATION,
                            [Target::new(Kind::Function, end)],
                            "external",
                            name.clone(),
                            format!(
                                "{name}: the termination at the external end is unknown; check that the partner \
                                 terminates the segment"
                            ),
                        )
                        .with_severity(Severity::Info)
                        .arg("form", "external")
                        .arg("port", name),
                    );
                    continue;
                }
                let available: Vec<(Id<Function>, Option<OnOff>)> = ports
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        if claimed.contains(&i) {
                            (p.0, None)
                        } else {
                            p.clone()
                        }
                    })
                    .collect();
                match nearest_terminating(&available, from_start) {
                    Some(i) => claimed.push(i),
                    None => {
                        let name = end_name(q, end);
                        out.push(
                            Finding::new(
                                &TERMINATION,
                                [Target::new(Kind::Function, end)],
                                "missing",
                                name.clone(),
                                format!(
                                    "{name}: no terminating element at this segment end; terminate the segment \
                                     here"
                                ),
                            )
                            .arg("form", "missing")
                            .arg("port", name),
                        );
                    }
                }
            }
            for (i, (f, state)) in ports.iter().enumerate() {
                let Some(state) = state else { continue };
                let at_end = claimed.contains(&i);
                let (discriminator, form, severity, text) = match (state, at_end) {
                    (OnOff::On, true) | (OnOff::Off, false) => continue,
                    (OnOff::Off, true) => (
                        "end",
                        "end",
                        Severity::Error,
                        "the termination is off at the segment end; switch it on",
                    ),
                    (OnOff::On, false) => (
                        "middle",
                        "middle",
                        Severity::Error,
                        "the termination is on inside the segment; switch it off",
                    ),
                    (OnOff::Unknown(_), _) => (
                        "unknown",
                        "unknown",
                        Severity::Info,
                        "the termination state is of a newer format; check it",
                    ),
                };
                let name = end_name(q, *f);
                out.push(
                    Finding::new(
                        &TERMINATION,
                        [Target::new(Kind::Function, *f)],
                        discriminator,
                        name.clone(),
                        format!("{name}: {text}"),
                    )
                    .with_severity(severity)
                    .arg("form", form)
                    .arg("port", name),
                );
            }
        }
        out
    }
}
