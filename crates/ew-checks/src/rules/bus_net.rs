// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The 1.0 rules of the EW-BUS pack (doc 10 §3.8, §3.10; doc 15 §4.5.2): EW-BUS-002 (address range), -003 (PROFINET
//! name and IP), -005 (rate), -006 (segment limits), -007 (no master), -009 (wrong bus cable or connector) and -010
//! (net mismatch). Ranges, lengths and station counts are data of this module with `verified = false` (FR-CHK-046,
//! NFR-COMP-05), so the rules that read them report warnings; the part's own range or rates win where it states them.

use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{
    AddrValue, BusPort, BusSystem, ConnectionKind, Endpoint, Function, Kind, LengthSource, LogicalNet,
    NetAddress, PartBusPort, PortRole,
};
use ew_derive::{BusSegments, NetMaster, NetMembersOf, Q, SegmentMember};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::bus::end_words;
use crate::rules::term::full_tag;

/// A port as a message names it ([`end_words`]'s text).
fn port_words(q: &Q, f: Id<Function>) -> String {
    end_words(q, f).0
}

/// The part port that a drawn port comes from: the first port with the same interface and label among the bus
/// sections of its device's parts.
fn part_port(q: &Q, function: &Function, port: &BusPort) -> Option<PartBusPort> {
    let device = q.devices().get(function.device?)?;
    let parts = q.parts();
    device.parts.iter().find_map(|u| {
        parts
            .get(u.part.copy)?
            .item
            .sections
            .bus
            .as_ref()?
            .ports
            .iter()
            .find(|p| p.interface == port.interface && p.port_label == port.port_label)
            .cloned()
    })
}

/// The ports of a logical net with their bus port records, in ID order.
fn net_ports(q: &Q, net: Id<LogicalNet>) -> Vec<(Id<Function>, Function, BusPort)> {
    let functions = q.functions();
    q.get::<NetMembersOf>(net)
        .ports
        .iter()
        .filter_map(|f| {
            let func = functions.get(*f)?;
            let port = func.bus_port.clone()?;
            Some((*f, (**func).clone(), port))
        })
        .collect()
}

fn net_name(q: &Q, net: Id<LogicalNet>) -> String {
    q.table::<LogicalNet>()
        .get(net)
        .map_or_else(String::new, |n| n.name.to_string())
}

// ---------------------------------------------------------------------------------------------------------
// EW-BUS-002

/// EW-BUS-002 "Address outside the range" (doc 15 §4.5.2): conflict, o r, warning, lnet (FR-BUS-018).
pub const ADDRESS_RANGE: RuleMeta = RuleMeta {
    id: "EW-BUS-002",
    code: "111002",
    pack: "EW-BUS",
    title: "Address outside the range",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Lnet,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-bus-002",
};

/// The PROFIBUS DP station addresses (0–125, doc 10 §3.8), unverified: a warning at most.
const PROFIBUS_RANGE: (u16, u16) = (0, 125);

/// EW-BUS-002: a PROFIBUS station address outside the range its part's port states (`PartBusPort.address_range`,
/// FR-BUS-009), else outside 0–125 (unverified data). One warning per port, target the port.
#[derive(Clone, Copy, Debug)]
pub struct AddressRange;

impl Rule for AddressRange {
    fn meta(&self) -> &'static RuleMeta {
        &ADDRESS_RANGE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Lnet(net) = *scope else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (f, function, port) in net_ports(q, net) {
            let AddrValue::Value(NetAddress::Profibus(a)) = port.address else {
                continue;
            };
            let (min, max) = part_port(q, &function, &port)
                .and_then(|p| p.address_range)
                .map_or(PROFIBUS_RANGE, |r| (r.min, r.max));
            if (min..=max).contains(&u16::from(a)) {
                continue;
            }
            let name = port_words(q, f);
            out.push(
                Finding::new(
                    &ADDRESS_RANGE,
                    [Target::new(Kind::Function, f)],
                    "range",
                    name.clone(),
                    format!("{name}: the station address {a} is outside {min}..{max}; enter an address inside the range"),
                )
                .arg("port", name)
                .arg("address", a.to_string())
                .arg("min", min.to_string())
                .arg("max", max.to_string()),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-BUS-003

/// EW-BUS-003 "PROFINET name or IP" (doc 15 §4.5.2): conflict, o r, error, lnet (FR-BUS-017).
pub const PROFINET: RuleMeta = RuleMeta {
    id: "EW-BUS-003",
    code: "111003",
    pack: "EW-BUS",
    title: "PROFINET name or IP",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Lnet,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-bus-003",
};

/// Whether `name` is a DNS-compatible PROFINET device name (IEC 61158-6-10, unverified): at most 240 characters, labels
/// of 1 to 63 lower-case letters, digits and hyphens separated by dots, no label starting or ending with a hyphen.
fn valid_station_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 240
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// The network of an address under a mask.
fn network(ip: Ipv4Addr, mask: Ipv4Addr) -> u32 {
    u32::from(ip) & u32::from(mask)
}

/// EW-BUS-003 over the PROFINET ports of a logical net: a device name that is not DNS-compatible (`name`, an underscore
/// or a capital letter), a device name or an IP address that two ports share (`duplicate_name`, `duplicate_ip`), and an
/// IP address outside the subnet of the net's master port (`subnet`: the master's address under the master's mask; a
/// net whose master has no address checks no subnet).
#[derive(Clone, Copy, Debug)]
pub struct Profinet;

impl Rule for Profinet {
    fn meta(&self) -> &'static RuleMeta {
        &PROFINET
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Lnet(net) = *scope else {
            return Vec::new();
        };
        let net_text = net_name(q, net);
        let ports: Vec<(Id<Function>, ew_derive::ew_schema::ProfinetAddress)> = net_ports(q, net)
            .into_iter()
            .filter_map(|(f, _, p)| match p.address {
                AddrValue::Value(NetAddress::Profinet(a)) => Some((f, a)),
                _ => None,
            })
            .collect();
        let master = match q.get::<NetMembersOf>(net).master {
            Some(NetMaster::Port(m)) => ports
                .iter()
                .find(|(f, _)| *f == m)
                .and_then(|(_, a)| a.ip.zip(a.mask)),
            _ => None,
        };
        let mut out = Vec::new();
        let mut names: BTreeMap<String, Vec<Id<Function>>> = BTreeMap::new();
        let mut ips: BTreeMap<Ipv4Addr, Vec<Id<Function>>> = BTreeMap::new();
        for (f, a) in &ports {
            let port = port_words(q, *f);
            names.entry(a.name.to_string()).or_default().push(*f);
            if !valid_station_name(a.name.as_str()) {
                out.push(
                    Finding::new(
                        &PROFINET,
                        [Target::new(Kind::Function, *f)],
                        "name",
                        port.clone(),
                        format!(
                            "{port}: the device name {} is not DNS-compatible; use lower-case letters, digits, hyphens \
                             and dots",
                            a.name
                        ),
                    )
                    .arg("form", "name")
                    .arg("port", port.clone())
                    .arg("name", a.name.to_string()),
                );
            }
            if let Some(ip) = a.ip {
                ips.entry(ip).or_default().push(*f);
                if let Some((mip, mmask)) = master.filter(|(mip, _)| *mip != ip) {
                    if network(ip, mmask) != network(mip, mmask) {
                        out.push(
                            Finding::new(
                                &PROFINET,
                                [Target::new(Kind::Function, *f)],
                                "subnet",
                                port.clone(),
                                format!(
                                    "{port}: the IP address {ip} is outside the subnet of the master, {mip} with mask \
                                     {mmask}; enter an address of that subnet"
                                ),
                            )
                            .arg("form", "subnet")
                            .arg("port", port)
                            .arg("ip", ip.to_string())
                            .arg("master_ip", mip.to_string())
                            .arg("mask", mmask.to_string()),
                        );
                    }
                }
            }
        }
        let shared = names
            .into_iter()
            .map(|(k, v)| ("duplicate_name", "device name", k, v))
            .chain(
                ips.into_iter()
                    .map(|(k, v)| ("duplicate_ip", "IP address", k.to_string(), v)),
            );
        for (form, what, value, functions) in shared {
            if functions.len() < 2 {
                continue;
            }
            let mut words: Vec<String> = functions.iter().map(|f| port_words(q, *f)).collect();
            words.sort_by(|a, b| ew_derive::ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)));
            let list = words.join(", ");
            out.push(
                Finding::new(
                    &PROFINET,
                    functions.iter().map(|f| Target::new(Kind::Function, *f)),
                    format!("{form}:{value}"),
                    words[0].clone(),
                    format!(
                        "the {what} {value} is used by {list} in logical net {net_text}; give each port its own"
                    ),
                )
                .arg("form", form)
                .arg("value", value)
                .arg("ports", list)
                .arg("net", net_text.clone()),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-BUS-005

/// EW-BUS-005 "Rate not supported" (doc 15 §4.5.2): conflict, o r, error, lnet (FR-BUS-009, FR-BUS-018).
pub const RATE: RuleMeta = RuleMeta {
    id: "EW-BUS-005",
    code: "111005",
    pack: "EW-BUS",
    title: "Rate not supported",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Lnet,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-bus-005",
};

/// EW-BUS-005: the logical net's rate is not among the rates a port's part supports (`PartBusPort.rates`, the GSD
/// facts of FR-BUS-009). A net without a rate, and a part that states no rates, are not judged. One finding per port.
#[derive(Clone, Copy, Debug)]
pub struct Rate;

impl Rule for Rate {
    fn meta(&self) -> &'static RuleMeta {
        &RATE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Lnet(net) = *scope else {
            return Vec::new();
        };
        let Some(rate) = q.table::<LogicalNet>().get(net).and_then(|n| n.rate) else {
            return Vec::new();
        };
        let net_text = net_name(q, net);
        let mut out = Vec::new();
        for (f, function, port) in net_ports(q, net) {
            let Some(pp) = part_port(q, &function, &port).filter(|p| !p.rates.is_empty()) else {
                continue;
            };
            if pp.rates.contains(&rate) {
                continue;
            }
            let name = port_words(q, f);
            out.push(
                Finding::new(
                    &RATE,
                    [Target::new(Kind::Function, f)],
                    "rate",
                    name.clone(),
                    format!(
                        "{name} does not support {} bit/s, the rate of logical net {net_text}; choose a rate every \
                         station supports",
                        rate.0
                    ),
                )
                .arg("port", name)
                .arg("rate", rate.0.to_string())
                .arg("net", net_text.clone()),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-BUS-006

/// EW-BUS-006 "Segment limits" (doc 15 §4.5.2): conflict, f r, warning, pnet (FR-BUS-018).
pub const SEGMENT_LIMITS: RuleMeta = RuleMeta {
    id: "EW-BUS-006",
    code: "111006",
    pack: "EW-BUS",
    title: "Segment limits",
    kind: RuleKind::Conflict,
    modes: Modes::letters("fr"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Pnet,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-bus-006",
};

/// The most stations of an RS-485 PROFIBUS segment (unverified).
const PROFIBUS_STATIONS: usize = 32;

/// The longest PROFIBUS copper segment in metres at a rate in bit/s (IEC 61158, unverified): 1200 m up to 93.75 kbit/s,
/// 1000 m at 187.5 kbit/s, 400 m at 500 kbit/s, 200 m at 1.5 Mbit/s, 100 m above.
fn profibus_length_m(rate: u32) -> u64 {
    match rate {
        0..=93_750 => 1200,
        93_751..=187_500 => 1000,
        187_501..=500_000 => 400,
        500_001..=1_500_000 => 200,
        _ => 100,
    }
}

/// EW-BUS-006 over the segments of a PROFIBUS DP physical net (`bus_segments(pnet)`): more stations than a segment
/// takes (`stations`: ports that are not passive), a longer segment than the net's rate allows (`length`: the summed
/// manual lengths of its cables), and a branch at a rate above 1.5 Mbit/s (`stub`). Warnings: the limits are
/// unverified data. A net without a rate checks the stations only.
#[derive(Clone, Copy, Debug)]
pub struct SegmentLimits;

impl Rule for SegmentLimits {
    fn meta(&self) -> &'static RuleMeta {
        &SEGMENT_LIMITS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Pnet(pnet) = *scope else {
            return Vec::new();
        };
        let functions = q.functions();
        let mut out = Vec::new();
        for segment in q.get::<BusSegments>(pnet).iter() {
            let net = segment
                .logical_net
                .and_then(|n| q.table::<LogicalNet>().get(n).cloned());
            if net.as_ref().is_some_and(|n| n.bus != BusSystem::ProfibusDp) {
                continue;
            }
            let Some(start) = segment.functions().next() else {
                continue;
            };
            let subject = port_words(q, start);
            let stations = segment
                .functions()
                .filter(|f| {
                    functions
                        .get(*f)
                        .and_then(|x| x.bus_port.as_ref())
                        .is_some_and(|p| p.role != PortRole::Passive)
                })
                .count();
            let targets: Vec<Target> = segment
                .functions()
                .map(|f| Target::new(Kind::Function, f))
                .collect();
            if stations > PROFIBUS_STATIONS {
                let message = format!(
                    "the segment from {subject} has {stations} stations, more than the {PROFIBUS_STATIONS} a PROFIBUS \
                     segment takes; split it with a repeater"
                );
                let (n, limit) = (stations.to_string(), PROFIBUS_STATIONS.to_string());
                out.push(
                    Finding::new(
                        &SEGMENT_LIMITS,
                        targets.clone(),
                        "stations",
                        subject.clone(),
                        message,
                    )
                    .arg("form", "stations")
                    .arg("segment", subject.clone())
                    .arg("count", n)
                    .arg("limit", limit),
                );
            }
            let Some(rate) = net.as_ref().and_then(|n| n.rate) else {
                continue;
            };
            let length_nm: i64 = segment
                .members
                .iter()
                .filter_map(|m| match m {
                    SegmentMember::Cable(c) => {
                        q.devices()
                            .get(*c)
                            .and_then(|d| match d.aspects.cable.as_ref()?.length {
                                LengthSource::Manual(nm) => Some(nm.0),
                                _ => None,
                            })
                    }
                    SegmentMember::Function(_) => None,
                })
                .sum();
            let metres = u64::try_from(length_nm.div_euclid(1_000_000_000)).unwrap_or(0);
            let limit = profibus_length_m(rate.0);
            if metres > limit {
                let message = format!(
                    "the segment from {subject} is {metres} m long, more than the {limit} m PROFIBUS allows at {} bit/s; \
                     shorten it or lower the rate",
                    rate.0
                );
                out.push(
                    Finding::new(
                        &SEGMENT_LIMITS,
                        targets.clone(),
                        "length",
                        subject.clone(),
                        message,
                    )
                    .arg("form", "length")
                    .arg("segment", subject.clone())
                    .arg("metres", metres.to_string())
                    .arg("limit", limit.to_string())
                    .arg("rate", rate.0.to_string()),
                );
            }
            if segment.branched && rate.0 > 1_500_000 {
                let message = format!(
                    "the segment from {subject} branches, and PROFIBUS allows no stubs above 1.5 Mbit/s; run the line \
                     through every station"
                );
                out.push(
                    Finding::new(&SEGMENT_LIMITS, targets.clone(), "stub", subject.clone(), message)
                        .arg("form", "stub")
                        .arg("segment", subject.clone()),
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-BUS-007

/// EW-BUS-007 "No master" (doc 15 §4.5.2): completeness, o r, warning, lnet (FR-BUS-018).
pub const NO_MASTER: RuleMeta = RuleMeta {
    id: "EW-BUS-007",
    code: "111007",
    pack: "EW-BUS",
    title: "No master",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Lnet,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-bus-007",
};

/// EW-BUS-007: a logical net that names no master and has no port with role master or controller. An external
/// partner named as the master counts (doc 10 §8 item 11). Target the net.
#[derive(Clone, Copy, Debug)]
pub struct NoMaster;

impl Rule for NoMaster {
    fn meta(&self) -> &'static RuleMeta {
        &NO_MASTER
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Lnet(net) = *scope else {
            return Vec::new();
        };
        if q.get::<NetMembersOf>(net).master.is_some()
            || net_ports(q, net)
                .iter()
                .any(|(_, _, p)| matches!(p.role, PortRole::Master | PortRole::Controller))
        {
            return Vec::new();
        }
        let name = net_name(q, net);
        vec![
            Finding::new(
                &NO_MASTER,
                [Target::new(Kind::LogicalNet, net)],
                "master",
                format!("logical net {name}"),
                format!(
                    "logical net {name} has no master; name its master port, or the external partner that is"
                ),
            )
            .arg("net", name),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-BUS-009

/// EW-BUS-009 "Wrong bus cable" (doc 15 §4.5.2): conflict, o r, warning, pnet (FR-BUS-018).
pub const WRONG_CABLE: RuleMeta = RuleMeta {
    id: "EW-BUS-009",
    code: "111009",
    pack: "EW-BUS",
    title: "Wrong bus cable",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Pnet,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-bus-009",
};

/// EW-BUS-009 over the `Bus` connections at the ports of a physical net: a cable whose part is made for other bus
/// systems than the port's (`BusSection.cable_for`; a cable part that says nothing is not judged; discriminator
/// `cable`, target the cable and the port), and two ports mated without a cable whose parts name different connector
/// types (`connector`, both ports as targets).
#[derive(Clone, Copy, Debug)]
pub struct WrongCable;

impl Rule for WrongCable {
    fn meta(&self) -> &'static RuleMeta {
        &WRONG_CABLE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Pnet(pnet) = *scope else {
            return Vec::new();
        };
        let functions = q.functions();
        let mut ports: Vec<Id<Function>> = functions
            .iter()
            .filter(|(_, f)| f.bus_port.as_ref().is_some_and(|p| p.physical_net == Some(pnet)))
            .map(|(id, _)| id)
            .collect();
        ports.sort();
        let mut seen: BTreeSet<Id<ew_derive::ew_schema::Connection>> = BTreeSet::new();
        let mut out = Vec::new();
        for f in &ports {
            let Some(function) = functions.get(*f) else {
                continue;
            };
            let Some(port) = function.bus_port.as_ref() else {
                continue;
            };
            let mut at: Vec<_> = q
                .get::<ew_derive::FunctionCps>(*f)
                .iter()
                .flat_map(|cp| q.idx().connections_at_cp(*f, cp.idx).collect::<Vec<_>>())
                .collect();
            at.sort();
            at.dedup();
            for c in at {
                let Some(conn) = q.connections().get(c) else {
                    continue;
                };
                let ConnectionKind::Bus(line) = &conn.kind else {
                    continue;
                };
                let name = port_words(q, *f);
                if let Some(cable) = line.cable {
                    let made_for = q.devices().get(cable).and_then(|d| {
                        d.parts.iter().find_map(|u| {
                            let b = q.parts().get(u.part.copy)?.item.sections.bus.clone()?;
                            (!b.cable_for.is_empty()).then_some(b.cable_for)
                        })
                    });
                    if made_for.is_some_and(|list| !list.contains(&port.bus)) {
                        let tag = full_tag(q, cable, "the cable");
                        out.push(
                            Finding::new(
                                &WRONG_CABLE,
                                [Target::new(Kind::Device, cable), Target::new(Kind::Function, *f)],
                                "cable",
                                name.clone(),
                                format!("{name} is connected by {tag}, which is not a cable for its bus; use a bus cable"),
                            )
                            .arg("form", "cable")
                            .arg("port", name)
                            .arg("cable", tag),
                        );
                    }
                    continue;
                }
                if !seen.insert(c) {
                    continue;
                }
                let other = match (&conn.a, &conn.b) {
                    (Endpoint::Cp(a, _), Endpoint::Cp(b, _)) if a == f => *b,
                    (Endpoint::Cp(a, _), Endpoint::Cp(b, _)) if b == f => *a,
                    _ => continue,
                };
                let Some(other_fn) = functions.get(other) else {
                    continue;
                };
                let Some(other_port) = other_fn.bus_port.as_ref() else {
                    continue;
                };
                let mine = part_port(q, function, port).and_then(|p| p.connector);
                let theirs = part_port(q, other_fn, other_port).and_then(|p| p.connector);
                if let (Some(a), Some(b)) = (mine, theirs) {
                    if a != b {
                        let other_name = port_words(q, other);
                        out.push(
                            Finding::new(
                                &WRONG_CABLE,
                                [Target::new(Kind::Function, *f), Target::new(Kind::Function, other)],
                                "connector",
                                name.clone(),
                                format!(
                                    "{name} ({a}) is mated with {other_name} ({b}); the connector types differ"
                                ),
                            )
                            .arg("form", "connector")
                            .arg("port", name)
                            .arg("connector", a.to_string())
                            .arg("other", other_name)
                            .arg("other_connector", b.to_string()),
                        );
                    }
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-BUS-010

/// EW-BUS-010 "Net mismatch" (doc 15 §4.5.2): conflict, o r, warning, pnet (FR-BUS-024).
pub const NET_MISMATCH: RuleMeta = RuleMeta {
    id: "EW-BUS-010",
    code: "111010",
    pack: "EW-BUS",
    title: "Net mismatch",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Pnet,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-bus-010",
};

/// EW-BUS-010 over the segments of a physical net: the ports of one segment (joined by `Bus` connections) that name
/// different logical nets. One finding per segment, every port that names a net a target, the nets by name.
#[derive(Clone, Copy, Debug)]
pub struct NetMismatch;

impl Rule for NetMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &NET_MISMATCH
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Pnet(pnet) = *scope else {
            return Vec::new();
        };
        let functions = q.functions();
        let mut out = Vec::new();
        for segment in q.get::<BusSegments>(pnet).iter() {
            let named: Vec<(Id<Function>, Id<LogicalNet>)> = segment
                .functions()
                .filter_map(|f| Some((f, functions.get(f)?.bus_port.as_ref()?.logical_net?)))
                .collect();
            let nets: BTreeSet<Id<LogicalNet>> = named.iter().map(|(_, n)| *n).collect();
            if nets.len() < 2 {
                continue;
            }
            let mut names: Vec<String> = nets.iter().map(|n| net_name(q, *n)).collect();
            names.sort_by(|a, b| ew_derive::ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)));
            let list = names.join(", ");
            let subject = port_words(q, named[0].0);
            out.push(
                Finding::new(
                    &NET_MISMATCH,
                    named.iter().map(|(f, _)| Target::new(Kind::Function, *f)),
                    "nets",
                    subject.clone(),
                    format!(
                        "the ports joined with {subject} belong to the logical nets {list}; put every port of the segment \
                         in one logical net"
                    ),
                )
                .arg("port", subject)
                .arg("nets", list),
            );
        }
        out
    }
}
