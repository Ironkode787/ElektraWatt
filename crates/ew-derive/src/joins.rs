// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! How a physical wire is joined to the others on its CPs (doc 08 §3.2, FR-CON-005, FR-CON-006; D-20): the
//! junction it passes with its style and wiring, and the daisy chains at its ends. The wire list prints it so
//! that the shop reads which CP is wired to which: "W13/W14 star at F55:2", "W7/W8 chain on -101F1:1".
//!
//! # Rule
//!
//! - **Junction.** The first junction the wire passes (`PhysicalWire::junctions`, path order) with its style
//!   (dot or bevel) and its effective wiring (the stored one, else the style's default: Dot star, Bevel chain,
//!   doc 08 §3.2). The CPs named with it:
//!   - **Star:** the entering CP, the CP every wire of the star starts from: the CP at the far end of the
//!     junction's feed arm (arm 1) when that is an end of the wire, else the end that every wire through the
//!     junction shares (a nested star); none when neither decides.
//!   - **Chain:** the ends of the wire that are also an end of another wire through the junction (the CPs that
//!     carry two conductors of the chain, `-102F1:1` for W20 and W21).
//! - **Daisy chain** (FR-CON-006): an end of the wire on a device CP that two or more drawn `Wire` or `Core`
//!   connections touch (two conductors landed on one clamp, `-101F1:1` for W7 and W8). A junction's arms meet at
//!   the junction, not at the CP, so a star centre such as F55:2 is not a daisy chain.
//!
//! The CPs keep the order of the wire's ends, so the caller passes the oriented wire (`orient_wire`).

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{ConnectionKind, Junction, JunctionStyle, JunctionWiring, Page};

use crate::nets::{CpRef, ends};
use crate::wires::{EndKind, PhysicalWire, PhysicalWiresOf};
use crate::{Cell, CellOf, Q};

/// The drawn style of a junction.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum JoinStyle {
    /// A dot.
    Dot,
    /// A bevelled corner.
    Bevel,
    /// A style of a newer format.
    Other,
}

impl JoinStyle {
    /// The lower-case name: `dot`, `bevel`, `junction`.
    pub fn name(self) -> &'static str {
        match self {
            JoinStyle::Dot => "dot",
            JoinStyle::Bevel => "bevel",
            JoinStyle::Other => "junction",
        }
    }
}

/// The effective wiring of a junction.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum JoinWiring {
    /// The entering CP is wired to the first CP of every other arm.
    Star,
    /// Consecutive CPs are wired.
    Chain,
}

impl JoinWiring {
    /// The lower-case name: `star`, `chain`.
    pub fn name(self) -> &'static str {
        match self {
            JoinWiring::Star => "star",
            JoinWiring::Chain => "chain",
        }
    }
}

/// The junction a wire passes, as the wire list prints it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JunctionJoin {
    /// The junction.
    pub junction: Id<Junction>,
    /// Its page.
    pub page: Id<Page>,
    /// The frame cell of its position; `None` outside the drawing area.
    pub cell: Option<Cell>,
    /// Its style.
    pub style: JoinStyle,
    /// Its effective wiring.
    pub wiring: JoinWiring,
    /// Star: the entering CP (at most one); chain: the wire's ends shared with another wire of the junction.
    pub cps: Vec<CpRef>,
}

/// How a wire is joined to the others (see the module documentation).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WireJoin {
    /// The first junction the wire passes.
    pub junction: Option<JunctionJoin>,
    /// The ends that are daisy chains, in the order of the wire's ends.
    pub daisy: Vec<CpRef>,
}

/// The junction and daisy chains of `w` (see the module documentation).
pub fn wire_join(q: &Q, w: &PhysicalWire) -> WireJoin {
    WireJoin {
        junction: w.junctions.first().and_then(|j| junction_join(q, w, *j)),
        daisy: daisy_ends(q, w),
    }
}

fn junction_join(q: &Q, w: &PhysicalWire, j: Id<Junction>) -> Option<JunctionJoin> {
    let row = q.junctions().get(j)?;
    let style = match row.style {
        JunctionStyle::Dot => JoinStyle::Dot,
        JunctionStyle::Bevel(_) => JoinStyle::Bevel,
        JunctionStyle::Unknown(_) => JoinStyle::Other,
    };
    // Absent or unknown wiring: the style's default (Dot: star, Bevel: chain, unknown: star), as the
    // extraction resolves it (`physical_wires_of`).
    let wiring = match (&row.wiring, &row.style) {
        (Some(JunctionWiring::Star), _) => JoinWiring::Star,
        (Some(JunctionWiring::Chain), _) => JoinWiring::Chain,
        (_, JunctionStyle::Bevel(_)) => JoinWiring::Chain,
        _ => JoinWiring::Star,
    };
    let end_cps = [w.ends[0].cp, w.ends[1].cp];
    // The other wires through the junction (the same extraction, so the same bond closure).
    let siblings: Vec<PhysicalWire> = w
        .potentials
        .first()
        .map(|p| {
            q.get::<PhysicalWiresOf>(*p)
                .wires
                .iter()
                .filter(|x| x.id != w.id && x.junctions.contains(&j))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let cps = match wiring {
        JoinWiring::Star => {
            let feed = row
                .sequence
                .first()
                .and_then(|c| q.connections().get(*c))
                .and_then(|c| ends(c))
                .and_then(|(a, b)| {
                    let far = if a == EndpointKey::Junction(j) { b } else { a };
                    CpRef::of(far)
                })
                .filter(|cp| end_cps.contains(cp));
            let shared = || {
                end_cps.iter().copied().find(|cp| {
                    !siblings.is_empty() && siblings.iter().all(|s| s.ends.iter().any(|e| e.cp == *cp))
                })
            };
            feed.or_else(shared).into_iter().collect()
        }
        JoinWiring::Chain => {
            let mut v: Vec<CpRef> = Vec::new();
            for cp in end_cps {
                if !v.contains(&cp) && siblings.iter().any(|s| s.ends.iter().any(|e| e.cp == cp)) {
                    v.push(cp);
                }
            }
            v
        }
    };
    Some(JunctionJoin {
        junction: j,
        page: row.page,
        cell: q.get::<CellOf>((row.page, row.at)).as_ref().clone(),
        style,
        wiring,
        cps,
    })
}

fn daisy_ends(q: &Q, w: &PhysicalWire) -> Vec<CpRef> {
    let connections = q.connections();
    let mut out: Vec<CpRef> = Vec::new();
    for end in w.ends.iter().filter(|e| e.kind == EndKind::Device) {
        let drawn = q
            .idx()
            .connections_at(end.cp.key())
            .filter(|c| {
                connections
                    .get(*c)
                    .is_some_and(|c| matches!(c.kind, ConnectionKind::Wire | ConnectionKind::Core(_)))
            })
            .count();
        if drawn >= 2 && !out.contains(&end.cp) {
            out.push(end.cp);
        }
    }
    out
}

/// A CP as one text, as the junction notation prints it (doc 08 §3.8 "Wire-end locus"): a device CP
/// `tag:designation` with `#n` where the function's printed designations repeat (`-102G1:−#1`, the n-th CP of
/// that designation in index order), a terminal `strip:terminal/clamp` (`-X1031:1/b`, the designation that of the CP's
/// level, `level_short_tag`) with `#n` where the
/// function's clamps of one level repeat a letter (a third-party FnDef: `-X1:1/a#2`), an interruption
/// point its name; empty for an unknown function.
pub fn cp_label(q: &Q, cp: CpRef) -> String {
    let Some(f) = q.functions().get(cp.function) else {
        return String::new();
    };
    if f.device.is_none() {
        return match &f.kind {
            ew_schema::FunctionKind::Ip(ip) => ip.name.as_str().to_owned(),
            _ => String::new(),
        };
    }
    let short = cp_tag_from(q, cp, None);
    match cp_pin_parts(q, cp) {
        Some(Pin::Clamp(clamp)) => format!("{short}/{clamp}"),
        Some(Pin::Designation(pin)) if !pin.is_empty() => {
            // A device without a tag (an earth connection) is named by its pin alone: `PE`.
            if short.is_empty() {
                pin
            } else {
                format!("{short}:{pin}")
            }
        }
        Some(Pin::TerminalDesignation(_) | Pin::Designation(_)) | None => short,
    }
}

/// The pin of a CP as a list prints it in a CP column beside its device or terminal (doc 13 §3.2): a device CP its
/// designation with `#n` where the function's printed designations repeat (`+#1`), as [`cp_label`] does; a terminal
/// CP its clamp letter, with `#n` where the level's clamps repeat a letter (`a`, `a#2`), else the CP's designation;
/// empty for an interruption point and an unknown function or CP.
pub fn cp_pin(q: &Q, cp: CpRef) -> String {
    match cp_pin_parts(q, cp) {
        Some(Pin::Clamp(p) | Pin::Designation(p) | Pin::TerminalDesignation(p)) => p,
        None => String::new(),
    }
}

/// The device or terminal at a CP by its tag, as a wire's end prints it against its other end at `other`
/// (doc 13 §3.2): without the structure the two ends' devices share, so a wire inside one location reads
/// `-104A1-A2` and an end in another location `+FLD-104B1`. A terminal prints its level's own number
/// (`level_short_tag`). Without a device at `other` (an external end, or `None`) the tag prints without structure;
/// empty for a CP of no device.
pub fn cp_tag_from(q: &Q, cp: CpRef, other: Option<CpRef>) -> String {
    let device_at = |cp: CpRef| q.functions().get(cp.function).and_then(|f| f.device);
    let Some(device) = device_at(cp) else {
        return String::new();
    };
    let level = q
        .get::<crate::FunctionCps>(cp.function)
        .iter()
        .find(|c| c.idx == cp.idx)
        .and_then(|c| c.level);
    let Some(tag) = crate::tag::level_tag(q, device, level) else {
        return String::new();
    };
    match other
        .and_then(device_at)
        .and_then(|d| q.get::<crate::DeviceTag>(d).as_ref().clone())
    {
        Some(far) => tag.displayed(&far.structure),
        None => tag.short(),
    }
}

/// The pin part of a CP's label (see [`cp_label`], [`cp_pin`]).
enum Pin {
    /// A terminal's clamp letter, with its `#n`.
    Clamp(String),
    /// A terminal CP without a clamp letter: its designation.
    TerminalDesignation(String),
    /// A device CP's designation, with its `#n`; may be empty.
    Designation(String),
}

/// The pin part of a CP; `None` for a function without a device and an unknown function or CP.
fn cp_pin_parts(q: &Q, cp: CpRef) -> Option<Pin> {
    let device = q.functions().get(cp.function)?.device?;
    let cps = q.get::<crate::FunctionCps>(cp.function);
    let this = cps.iter().find(|c| c.idx == cp.idx)?;
    let terminal = q
        .devices()
        .get(device)
        .is_some_and(|d| d.aspects.terminal.is_some());
    if terminal {
        let Some(ew_schema::ClampKey::Clamp(ch)) = &this.clamp else {
            return Some(Pin::TerminalDesignation(this.designation.clone()));
        };
        // A clamp letter that repeats on the function (a third-party FnDef naming clamp `a` on both sides; the
        // starter `ew.fn.terminal` names side 2 `b`) takes `#n` like a repeated device designation, the
        // n-th CP of that level and clamp in index order, so that each clamp prints a distinct label (`-X1:1/a#1`,
        // `-X1:1/a#2`).
        let mut same: Vec<u16> = cps
            .iter()
            .filter(|c| c.level == this.level && c.clamp == this.clamp)
            .map(|c| c.idx)
            .collect();
        same.sort_unstable();
        return Some(Pin::Clamp(match same.iter().position(|i| *i == cp.idx) {
            Some(n) if same.len() > 1 => format!("{ch}#{}", n + 1),
            _ => ch.to_string(),
        }));
    }
    if this.designation.is_empty() {
        return Some(Pin::Designation(String::new()));
    }
    Some(Pin::Designation(
        match designation_nth(&cps, this.idx, &this.designation) {
            Some(n) => format!("{}#{n}", this.designation),
            None => this.designation.clone(),
        },
    ))
}

/// The 1-based position of CP `idx` among the function's CPs of `designation` in index order, when the designation
/// is shared by more than one CP (the label's `#n`); `None` when it is not shared.
fn designation_nth(cps: &[crate::Cp], idx: u16, designation: &str) -> Option<u32> {
    let mut same: Vec<u16> = cps
        .iter()
        .filter(|c| c.designation == designation)
        .map(|c| c.idx)
        .collect();
    same.sort_unstable();
    match same.iter().position(|i| *i == idx) {
        Some(n) if same.len() > 1 => u32::try_from(n + 1).ok(),
        _ => None,
    }
}

impl CpRef {
    /// A device CP's label in parts, for a sentence that names the pin in words: the device's short
    /// tag, the CP's designation and, when the function's CPs share that designation, the CP's 1-based position
    /// among them (the `#n` of [`cp_label`]). No words of any language: the caller's string table or Fluent pattern
    /// builds the sentence ("-1A4 pin − (2nd)"). `None` for a terminal's clamp, an interruption point, a CP
    /// without a designation and an unknown function or CP; those keep [`cp_label`].
    pub fn label_parts(self, q: &Q) -> Option<(String, String, Option<u32>)> {
        let f = q.functions().get(self.function)?;
        let device = f.device?;
        if q.devices()
            .get(device)
            .is_some_and(|d| d.aspects.terminal.is_some())
        {
            return None;
        }
        let cps = q.get::<crate::FunctionCps>(self.function);
        let this = cps.iter().find(|c| c.idx == self.idx)?;
        if this.designation.is_empty() {
            return None;
        }
        let short = q
            .get::<crate::DeviceTag>(device)
            .as_ref()
            .as_ref()
            .map(|t| t.short())
            .unwrap_or_default();
        let nth = designation_nth(&cps, this.idx, &this.designation);
        Some((short, this.designation.clone(), nth))
    }
}
