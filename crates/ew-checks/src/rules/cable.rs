// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE cable rules of the 03x block (doc 15 §4.2; content in doc 08 §8.1; WP-S1.16 part 3): EW-CORE-031,
//! -032, -034, -036 and -037. EW-CORE-033 is in [`core`](super::core).
//!
//! # Determinism (WP-S1.16 §10)
//!
//! Texts name cables, cores, wires and external locations by their printed text; lists inside a message are in
//! natural order of that text. IDs enter only targets and fingerprints.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::natural_cmp;
use ew_derive::ew_schema::{
    CableCore, ColourCode, ColourSpec, Conductor, Connection, ConnectionKind, Device, Function, FunctionKind,
    Kind, LibCopy, Part, StructureNode,
};
use ew_derive::{
    CpRef, DeviceTag, EndKind, EndpointKey, IpInfoOf, Member, PhysicalWire, PhysicalWiresOf,
    PotentialPartition, Q, SettingsMember, StructurePath, WireNumbers, company_codes, csa_text,
};

use crate::engine::touched_ids;
use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};

// ---------------------------------------------------------------------------------------------------------
// Shared texts

/// A cable as messages print it: its displayed tag without structure (`-106W1`), else `cable`.
pub(crate) fn cable_text(q: &Q, cable: Id<Device>) -> String {
    q.get::<DeviceTag>(cable)
        .as_ref()
        .as_ref()
        .map_or_else(|| "cable".to_owned(), |t| t.short())
}

/// A conductor as messages print it: `<cable>/<core>` (`-106W1/BU`), as EW-POT-005 prints a core wire.
fn core_text(q: &Q, c: &Conductor) -> String {
    format!("{}/{}", cable_text(q, c.cable), c.core.as_str())
}

/// One colour code as written: the IEC or company code, a foreign code as stored.
fn code_text(c: &ColourCode) -> String {
    match c {
        ColourCode::Iec(c) => c.as_str().to_owned(),
        ColourCode::Company(c) | ColourCode::Foreign(c) => c.as_str().to_owned(),
        ColourCode::Unknown(_) => "?".to_owned(),
    }
}

/// A colour specification as written (`GNYE`, `BR`).
fn colour_spec_text(c: &ColourSpec) -> String {
    c.codes().iter().map(code_text).collect()
}

/// The conductors of a cable in printed order (core text in natural order, then bytes, then the ID).
fn conductors_of(q: &Q, cable: Id<Device>) -> Vec<(Id<Conductor>, Conductor)> {
    let conductors = q.conductors();
    let mut out: Vec<(Id<Conductor>, Conductor)> = q
        .idx()
        .conductors_of_cable(cable)
        .filter_map(|id| conductors.get(id).map(|c| (id, Conductor::clone(c))))
        .collect();
    out.sort_by(|(ia, a), (ib, b)| {
        natural_cmp(a.core.as_str(), b.core.as_str())
            .then_with(|| a.core.as_str().cmp(b.core.as_str()))
            .then_with(|| ia.cmp(ib))
    });
    out
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-031

/// EW-CORE-031 "Invalid core colour" (doc 15 §4.2): conflict, p o r, error, cable.
pub const INVALID_CORE_COLOUR: RuleMeta = RuleMeta {
    id: "EW-CORE-031",
    code: "003031",
    pack: "EW-CORE",
    title: "Invalid core colour",
    kind: RuleKind::Conflict,
    modes: Modes::letters("por"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Cable,
    supersedes: &[],
    release: Release::Mvp,
};

/// The codes of a conductor's colour that are neither IEC 60757 codes nor company codes the project's colour
/// profile declares (`declared`), as written, in the specification's order; empty for a valid colour, no colour, or
/// a code of a newer format (not judged).
fn invalid_codes(c: &Conductor, declared: &BTreeSet<String>) -> Vec<String> {
    let Some(colour) = &c.colour else {
        return Vec::new();
    };
    colour
        .codes()
        .iter()
        .filter(|code| match code {
            ColourCode::Iec(_) | ColourCode::Unknown(_) => false,
            ColourCode::Company(x) => !declared.contains(x.as_str()),
            ColourCode::Foreign(_) => true,
        })
        .map(code_text)
        .collect()
}

/// The company codes the project's colour profile declares.
fn declared_codes(q: &Q) -> BTreeSet<String> {
    q.get::<SettingsMember>(Member::ColourProfile)
        .as_ref()
        .as_ref()
        .map(|v| company_codes(v).into_iter().map(|(code, _)| code).collect())
        .unwrap_or_default()
}

/// The EW-CORE-031 finding of conductor `id`, or `None` for a valid colour.
fn colour_finding(q: &Q, id: Id<Conductor>, c: &Conductor, declared: &BTreeSet<String>) -> Option<Finding> {
    let bad = invalid_codes(c, declared);
    if bad.is_empty() {
        return None;
    }
    let core = core_text(q, c);
    let shown = c.colour.as_ref().map(colour_spec_text).unwrap_or_default();
    let codes = bad
        .iter()
        .map(|b| format!("\"{b}\""))
        .collect::<Vec<_>>()
        .join(", ");
    Some(Finding::new(
        &INVALID_CORE_COLOUR,
        [Target::new(Kind::Conductor, id)],
        "colour",
        core.clone(),
        format!(
            "core {core} has the colour {shown}: {codes} is not an IEC 60757 code or a company code of the colour profile"
        ),
    ))
}

/// EW-CORE-031: a cable core whose colour holds a code that is neither an IEC 60757 code nor a company code the
/// project's colour profile declares (FR-CHK-032, D-17, MF-11). A foreign code (`{"foreign": "BR"}`), which only a
/// lenient import stores, and an undeclared company code are reported; a code of a newer format is not judged.
/// One finding per conductor: target the conductor, discriminator `colour`. Prevent mode refuses a batch that gives
/// a touched conductor such a colour (FT-261); a conductor that already held it is reported by the online and
/// release runs, not refused, so a lenient import's project stays editable.
///
/// The "use BN" quick fix of the import finding EW-IMP-008 is not carried here: its alias table
/// (`ew_import_tables::COLOUR_ALIASES`) is outside this crate's allowed dependencies (WP-S1.16 part 3 report).
#[derive(Clone, Copy, Debug)]
pub struct InvalidCoreColour;

impl Rule for InvalidCoreColour {
    fn meta(&self) -> &'static RuleMeta {
        &INVALID_CORE_COLOUR
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cable(cable) = *scope else {
            return Vec::new();
        };
        let declared = declared_codes(q);
        conductors_of(q, cable)
            .iter()
            .filter_map(|(id, c)| colour_finding(q, *id, c, &declared))
            .collect()
    }

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let declared = declared_codes(after);
        let declared_before = declared_codes(before);
        let mut out = Vec::new();
        for id in touched_ids::<Conductor>(touched, Kind::Conductor) {
            let Some(c) = after.conductors().get(id).map(|c| Conductor::clone(c)) else {
                continue;
            };
            let Some(finding) = colour_finding(after, id, &c, &declared) else {
                continue;
            };
            let held = before
                .conductors()
                .get(id)
                .is_some_and(|b| b.colour == c.colour && !invalid_codes(b, &declared_before).is_empty());
            if !held {
                out.push(finding);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-032

/// EW-CORE-032 "Core differs from the part" (doc 15 §4.2): conflict, o r, warning, cable.
pub const CORE_DIFFERS_FROM_PART: RuleMeta = RuleMeta {
    id: "EW-CORE-032",
    code: "003032",
    pack: "EW-CORE",
    title: "Core differs from the part",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Cable,
    supersedes: &[],
    release: Release::Mvp,
};

/// The cable part of a cable device: the first of its part uses whose copy defines cable cores.
fn cable_part(q: &Q, cable: Id<Device>) -> Option<(Id<Part>, std::sync::Arc<LibCopy<Part>>)> {
    let device = q.devices().get(cable)?;
    let parts = q.parts();
    device.parts.iter().find_map(|u| {
        parts
            .get(u.part.copy)
            .filter(|p| !p.item.sections.cable_cores.is_empty())
            .map(|p| (u.part.copy, p.clone()))
    })
}

/// The members in which a conductor differs from its core definition, as `member: conductor ≠ part` texts. A member
/// the conductor leaves unset (unknown) is not compared.
fn core_differences(c: &Conductor, def: &CableCore) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(colour) = &c.colour
        && def.colour.as_ref() != Some(colour)
    {
        let part = def
            .colour
            .as_ref()
            .map_or_else(|| "none".to_owned(), colour_spec_text);
        out.push(format!("colour {} ≠ {part}", colour_spec_text(colour)));
    }
    if let Some(csa) = &c.csa
        && def.csa.as_ref() != Some(csa)
    {
        let text = |x: &ew_derive::ew_schema::Csa| csa_text(x).unwrap_or_else(|| "?".to_owned());
        let part = def.csa.as_ref().map_or_else(|| "none".to_owned(), text);
        out.push(format!("cross-section {} ≠ {part}", text(csa)));
    }
    if c.shield != def.shield {
        let word = |s: bool| if s { "shield" } else { "core" };
        out.push(format!("kind {} ≠ {}", word(c.shield), word(def.shield)));
    }
    if let Some(pair) = c.pair
        && def.pair != Some(pair)
    {
        let part = def.pair.map_or_else(|| "none".to_owned(), |p| p.to_string());
        out.push(format!("pair {pair} ≠ {part}"));
    }
    out
}

/// EW-CORE-032: a core of a cable whose part defines cable cores (`sections.cable_cores`, FR-CBL-004) that differs
/// from the part's core of the same identification: its colour, cross-section, shield flag or pair, where the core
/// sets them, or a core identification the part does not define (FR-CHK-032, D-17). The cable part is the first part
/// use of the cable whose copy defines cores; a cable without one is not judged. One finding per conductor: targets
/// the conductor and the part copy, discriminator `core`.
#[derive(Clone, Copy, Debug)]
pub struct CoreDiffersFromPart;

impl Rule for CoreDiffersFromPart {
    fn meta(&self) -> &'static RuleMeta {
        &CORE_DIFFERS_FROM_PART
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cable(cable) = *scope else {
            return Vec::new();
        };
        let Some((part_id, part)) = cable_part(q, cable) else {
            return Vec::new();
        };
        let number = part.item.part_number.as_str().to_owned();
        let mut out = Vec::new();
        for (id, c) in conductors_of(q, cable) {
            let core = core_text(q, &c);
            let def = part.item.sections.cable_cores.iter().find(|d| d.core == c.core);
            let message = match def {
                None => format!("core {core} is not a core of the cable part {number}"),
                Some(def) => {
                    let diffs = core_differences(&c, def);
                    if diffs.is_empty() {
                        continue;
                    }
                    format!(
                        "core {core} differs from the core {} of the cable part {number}: {}",
                        def.core.as_str(),
                        diffs.join(", ")
                    )
                }
            };
            out.push(Finding::new(
                &CORE_DIFFERS_FROM_PART,
                [Target::new(Kind::Conductor, id), Target::new(Kind::Part, part_id)],
                "core",
                core,
                message,
            ));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-034

/// EW-CORE-034 "External connection without cable" (doc 15 §4.2): completeness, o r, warning, page.
pub const EXTERNAL_WITHOUT_CABLE: RuleMeta = RuleMeta {
    id: "EW-CORE-034",
    code: "003034",
    pack: "EW-CORE",
    title: "External connection without cable",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Page,
    supersedes: &[],
    release: Release::Mvp,
};

/// A location as a designation prints it: `+` and its path's codes (`+J-E-206`).
fn location_text(q: &Q, node: Id<StructureNode>) -> String {
    let path = q.get::<StructurePath>(node);
    if path.codes.is_empty() {
        return "a location".to_owned();
    }
    format!("+{}", path.codes.join("+"))
}

/// A wire as a message prints it: its wire number, else its connection ID's end texts.
fn wire_label(q: &Q, w: &PhysicalWire) -> String {
    q.get::<WireNumbers>(())
        .number(w.id)
        .map(|n| n.label.clone())
        .unwrap_or_else(|| {
            format!(
                "{}–{}",
                ew_derive::cp_label(q, w.ends[0].cp),
                ew_derive::cp_label(q, w.ends[1].cp)
            )
        })
}

/// The device an inside wire end belongs to as the message prints it: a terminal's strip, else the device.
fn inside_device(q: &Q, cp: CpRef) -> Option<Id<Device>> {
    let device = q.functions().get(cp.function)?.device?;
    let strip = q
        .devices()
        .get(device)
        .and_then(|d| d.aspects.terminal.as_ref().map(|t| t.strip));
    Some(strip.unwrap_or(device))
}

/// EW-CORE-034: a wired external connection point (an interruption point with `IpSpec.external`, FR-CBL-015) whose
/// wire is not a cable core (FR-CHK-033, D-19, EF-07, MF-27). The external points of one page are grouped by the
/// external reference's location: one finding per page and location, "+J-E-206 → 1X1", listing the inside devices
/// (a terminal by its strip) and the wires in natural order. An external point without a connection point or a
/// connection is not wired and not reported (EW-CORE-051 owns pairing). Targets: the location node and the points'
/// functions; discriminator `external`; shown on the page.
#[derive(Clone, Copy, Debug)]
pub struct ExternalWithoutCable;

impl Rule for ExternalWithoutCable {
    fn meta(&self) -> &'static RuleMeta {
        &EXTERNAL_WITHOUT_CABLE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Page(page) = *scope else {
            return Vec::new();
        };
        let mut ips: BTreeSet<Id<Function>> = BTreeSet::new();
        for r in q.idx().reps_by_page(page) {
            if let Some(rep) = q.reps().get(r) {
                ips.extend(rep.functions.iter().copied());
            }
        }
        let partition = q.get::<PotentialPartition>(());
        let external_refs = q.external_refs();
        // location → (point functions, inside devices, wire labels)
        type Group = (BTreeSet<Id<Function>>, BTreeSet<Id<Device>>, Vec<String>);
        let mut groups: BTreeMap<Id<StructureNode>, Group> = BTreeMap::new();
        for f in ips {
            let Some(function) = q.functions().get(f) else {
                continue;
            };
            let FunctionKind::Ip(spec) = &function.kind else {
                continue;
            };
            let Some(location) = spec
                .external
                .and_then(|x| external_refs.get(x).map(|r| r.location))
            else {
                continue;
            };
            // Only the point's home page reports it (its first representation's page).
            let info = q.get::<IpInfoOf>(f);
            if info.page != Some(page) {
                continue;
            }
            let Some(cp) = info.cp else {
                continue;
            };
            let at = EndpointKey::Cp(cp.function, cp.idx);
            let Some(potential) = partition.potential_at(at).map(|p| p.id) else {
                continue;
            };
            let wires = q.get::<PhysicalWiresOf>(potential);
            for w in wires
                .wires
                .iter()
                .filter(|w| w.ends.iter().any(|e| e.kind == EndKind::External && e.cp == cp))
            {
                if w.core.is_some() {
                    continue;
                }
                let group = groups.entry(location).or_default();
                group.0.insert(f);
                for e in w.ends.iter().filter(|e| e.kind == EndKind::Device) {
                    group.1.extend(inside_device(q, e.cp));
                }
                group.2.push(wire_label(q, w));
            }
        }
        let mut out = Vec::new();
        for (location, (functions, devices, mut wires)) in groups {
            let at = location_text(q, location);
            let mut inside: Vec<String> = devices.into_iter().map(|d| cable_text(q, d)).collect();
            sort_natural(&mut inside);
            sort_natural(&mut wires);
            wires.dedup();
            let subject = format!("{at} → {}", inside.join(", "));
            let message = format!(
                "the external connection {subject} has no cable: {} {} not a cable core; add the cable and assign its cores",
                if wires.len() == 1 { "wire" } else { "wires" },
                and_list(&wires, if wires.len() == 1 { "is" } else { "are" }),
            );
            out.push(
                Finding::new(
                    &EXTERNAL_WITHOUT_CABLE,
                    std::iter::once(Target::new(Kind::StructureNode, location))
                        .chain(functions.iter().map(|f| Target::new(Kind::Function, *f))),
                    "external",
                    subject,
                    message,
                )
                .keyed_on(&[Target::new(Kind::StructureNode, location)])
                .on_page(page),
            );
        }
        out
    }
}

/// `a, b, c <verb>`.
fn and_list(items: &[String], verb: &str) -> String {
    format!("{} {verb}", items.join(", "))
}

/// Sorts texts in natural order, then by bytes.
fn sort_natural(items: &mut [String]) {
    items.sort_by(|a, b| natural_cmp(a, b).then_with(|| a.cmp(b)));
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-036

/// EW-CORE-036 "Core assigned twice" (doc 15 §4.2): conflict, p o, error, cable.
pub const CORE_ASSIGNED_TWICE: RuleMeta = RuleMeta {
    id: "EW-CORE-036",
    code: "003036",
    pack: "EW-CORE",
    title: "Core assigned twice",
    kind: RuleKind::Conflict,
    modes: Modes::letters("po"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Cable,
    supersedes: &[],
    release: Release::Mvp,
};

/// The physical wires that use conductor `core`: the IDs of the wires whose constituents include a connection of
/// kind `Core(core)`, with those connections. Each connection's wire is found through the potential of its first
/// end (a core connection joins its two ends, so both lie in one potential).
fn wires_of_core(q: &Q, core: Id<Conductor>) -> BTreeMap<Id<Connection>, BTreeSet<Id<Connection>>> {
    let partition = q.get::<PotentialPartition>(());
    let mut out: BTreeMap<Id<Connection>, BTreeSet<Id<Connection>>> = BTreeMap::new();
    let mut connections: Vec<Id<Connection>> = q
        .connections()
        .iter()
        .filter(|(_, c)| c.kind == ConnectionKind::Core(core))
        .map(|(id, _)| id)
        .collect();
    connections.sort();
    for id in connections {
        let Some(c) = q.connections().get(id) else {
            continue;
        };
        let potential = [&c.a, &c.b]
            .into_iter()
            .filter_map(EndpointKey::of)
            .find_map(|e| partition.potential_at(e).map(|p| p.id));
        let wire = potential.and_then(|p| {
            q.get::<PhysicalWiresOf>(p)
                .wires
                .iter()
                .find(|w| w.constituents.contains(&id))
                .map(|w| w.id)
        });
        // A connection outside every extracted wire counts as a wire of its own.
        out.entry(wire.unwrap_or(id)).or_default().insert(id);
    }
    out
}

/// The EW-CORE-036 finding of a conductor used by more than one physical wire, or `None`.
fn twice_finding(q: &Q, id: Id<Conductor>, c: &Conductor) -> Option<Finding> {
    let wires = wires_of_core(q, id);
    if wires.len() < 2 {
        return None;
    }
    let core = core_text(q, c);
    let numbers = q.get::<WireNumbers>(());
    let mut labels: Vec<String> = wires
        .keys()
        .map(|w| {
            numbers.number(*w).map_or_else(
                || {
                    q.connections()
                        .get(*w)
                        .map(|c| {
                            let end = |e: &ew_derive::ew_schema::Endpoint| {
                                EndpointKey::of(e).map_or_else(
                                    || "?".to_owned(),
                                    |k| match k {
                                        EndpointKey::Cp(f, i) => ew_derive::cp_label(q, CpRef::new(f, i)),
                                        EndpointKey::Junction(_) => "junction".to_owned(),
                                    },
                                )
                            };
                            format!("{}–{}", end(&c.a), end(&c.b))
                        })
                        .unwrap_or_default()
                },
                |n| n.label.clone(),
            )
        })
        .collect();
    sort_natural(&mut labels);
    let connections: Vec<Target> = wires
        .values()
        .flatten()
        .map(|c| Target::new(Kind::Connection, *c))
        .collect();
    Some(
        Finding::new(
            &CORE_ASSIGNED_TWICE,
            std::iter::once(Target::new(Kind::Conductor, id)).chain(connections),
            "assigned",
            core.clone(),
            format!(
                "core {core} is assigned to {} wires ({}); a core carries one wire",
                wires.len(),
                labels.join(", ")
            ),
        )
        .keyed_on(&[Target::new(Kind::Conductor, id)]),
    )
}

/// EW-CORE-036: a cable core assigned to two or more physical wires (invariant CI-05, doc 08 §2.4; FR-CBL-003,
/// FR-CBL-011). Constituents of one physical wire (both legs of an interruption-point pair, a star feed arm) may
/// share the core. Since every assignment names one core, "more assignments than cores" is the same condition
/// counted per cable. One finding per conductor: the conductor and the connections that assign it as targets, the
/// fingerprint keyed on the conductor, discriminator `assigned`. Prevent mode refuses a batch that assigns a touched
/// connection's core to a second wire; online mode reports what a lenient import holds.
#[derive(Clone, Copy, Debug)]
pub struct CoreAssignedTwice;

impl Rule for CoreAssignedTwice {
    fn meta(&self) -> &'static RuleMeta {
        &CORE_ASSIGNED_TWICE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cable(cable) = *scope else {
            return Vec::new();
        };
        conductors_of(q, cable)
            .iter()
            .filter_map(|(id, c)| twice_finding(q, *id, c))
            .collect()
    }

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let mut cores: BTreeSet<Id<Conductor>> = BTreeSet::new();
        for id in touched_ids::<Connection>(touched, Kind::Connection) {
            if let Some(ConnectionKind::Core(k)) = after.connections().get(id).map(|c| c.kind.clone()) {
                cores.insert(k);
            }
        }
        let mut out = Vec::new();
        for core in cores {
            let Some(c) = after.conductors().get(core).map(|c| Conductor::clone(c)) else {
                continue;
            };
            let Some(finding) = twice_finding(after, core, &c) else {
                continue;
            };
            let before_wires = before
                .conductors()
                .get(core)
                .map_or(0, |_| wires_of_core(before, core).len());
            if wires_of_core(after, core).len() > before_wires.max(1) {
                out.push(finding);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-037

/// EW-CORE-037 "Shield not connected" (doc 15 §4.2): completeness, o, warning, cable.
pub const SHIELD_NOT_CONNECTED: RuleMeta = RuleMeta {
    id: "EW-CORE-037",
    code: "003037",
    pack: "EW-CORE",
    title: "Shield not connected",
    kind: RuleKind::Completeness,
    modes: Modes::letters("o"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Cable,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-037: a shield conductor (`Conductor.shield`) that no connection assigns, so it is connected at neither
/// end (FR-CBL-007). A shield connection always has two ends in the model; a shield laid on at one end only is drawn
/// as a core connection to that end's CP and counts as connected. Only a cable wired core by core is judged: one with
/// at least one conductor assigned; a cable drawn as a whole (single-line, FR-CBL-008), whose cores no connection
/// names, has no modelled shield connection either. One finding per shield: target the conductor, discriminator
/// `shield`.
#[derive(Clone, Copy, Debug)]
pub struct ShieldNotConnected;

impl Rule for ShieldNotConnected {
    fn meta(&self) -> &'static RuleMeta {
        &SHIELD_NOT_CONNECTED
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cable(cable) = *scope else {
            return Vec::new();
        };
        let shields: Vec<(Id<Conductor>, Conductor)> = conductors_of(q, cable)
            .into_iter()
            .filter(|(_, c)| c.shield)
            .collect();
        if shields.is_empty() {
            return Vec::new();
        }
        let used: BTreeSet<Id<Conductor>> = q
            .connections()
            .iter()
            .filter_map(|(_, c)| match c.kind {
                ConnectionKind::Core(k) => Some(k),
                _ => None,
            })
            .collect();
        // A cable none of whose conductors is assigned is drawn as a whole (single-line, FR-CBL-008): its cores'
        // connections are not modelled, so its shield is not judged (Golden A's -3W1, doc 08 §8.1: 0).
        let wired = q.idx().conductors_of_cable(cable).any(|c| used.contains(&c));
        if !wired {
            return Vec::new();
        }
        shields
            .into_iter()
            .filter(|(id, _)| !used.contains(id))
            .map(|(id, c)| {
                let core = core_text(q, &c);
                Finding::new(
                    &SHIELD_NOT_CONNECTED,
                    [Target::new(Kind::Conductor, id)],
                    "shield",
                    core.clone(),
                    format!("shield {core} is connected at neither end; connect it at one end or both"),
                )
            })
            .collect()
    }
}
