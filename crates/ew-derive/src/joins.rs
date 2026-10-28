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
/// that designation in index order), a terminal `strip:terminal/clamp` (`-X1031:1/b`) with `#n` where the
/// function's clamps of one level repeat a letter (a third-party FnDef: `-X1:1/a#2`), an interruption
/// point its name; empty for an unknown function.
pub fn cp_label(q: &Q, cp: CpRef) -> String {
    let Some(f) = q.functions().get(cp.function) else {
        return String::new();
    };
    let Some(device) = f.device else {
        return match &f.kind {
            ew_schema::FunctionKind::Ip(ip) => ip.name.as_str().to_owned(),
            _ => String::new(),
        };
    };
    let short = q
        .get::<crate::DeviceTag>(device)
        .as_ref()
        .as_ref()
        .map(|t| t.short())
        .unwrap_or_default();
    let cps = q.get::<crate::FunctionCps>(cp.function);
    let Some(this) = cps.iter().find(|c| c.idx == cp.idx) else {
        return short;
    };
    let terminal = q
        .devices()
        .get(device)
        .is_some_and(|d| d.aspects.terminal.is_some());
    if terminal {
        let Some(ew_schema::ClampKey::Clamp(ch)) = &this.clamp else {
            return short;
        };
        // A clamp letter that repeats on the function (a third-party FnDef naming clamp `a` on both sides; the
        // starter `ew.fn.terminal` names side 2 `b` since FT-505) takes `#n` like a repeated device designation, the
        // n-th CP of that level and clamp in index order, so that each clamp prints a distinct label (`-X1:1/a#1`,
        // `-X1:1/a#2`).
        let mut same: Vec<u16> = cps
            .iter()
            .filter(|c| c.level == this.level && c.clamp == this.clamp)
            .map(|c| c.idx)
            .collect();
        same.sort_unstable();
        return match same.iter().position(|i| *i == cp.idx) {
            Some(n) if same.len() > 1 => format!("{short}/{ch}#{}", n + 1),
            _ => format!("{short}/{ch}"),
        };
    }
    if this.designation.is_empty() {
        return short;
    }
    let mut same: Vec<u16> = cps
        .iter()
        .filter(|c| c.designation == this.designation)
        .map(|c| c.idx)
        .collect();
    same.sort_unstable();
    match same.iter().position(|i| *i == cp.idx) {
        Some(n) if same.len() > 1 => format!("{short}:{}#{}", this.designation, n + 1),
        _ => format!("{short}:{}", this.designation),
    }
}
