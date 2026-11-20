// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The supply marks as the window's Potential panel shows them (H9): a mark's name and definition, the supply's
//! phase voltage, where a set of points comes from and a mark's group.
//!
//! Not here: the place catalogue (`catalogue.rs`); the page tables' interruption points (`tables.rs`).

use std::sync::Arc;

use ew_derive::ew_ids::{Id, Uuid};
use ew_model::EndpointKey;

use crate::Session;

/// A supply mark as the window's Potential panel shows it (H9): see [`Session::supply_point`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SupplyPoint {
    /// The mark's name (`L1`, `PE-EXT`).
    pub name: String,
    /// The potential definition at the mark, if one stands there.
    pub def: Option<Uuid>,
    /// That definition's type as its stored word (`l1`, `l_plus`), empty without one.
    pub ptype: String,
    /// That definition's voltage as typed (`230 V AC`, `24 V`), empty without one.
    pub voltage: String,
    /// Whether the potential the mark is on has a type already: from its definition, or from another point of it (a
    /// mark that continues a potential drawn elsewhere).
    pub typed: bool,
}

impl Session {
    /// The supply mark `point` (an interruption point's function), as the Potential panel shows it; `None` for a
    /// function that is no interruption point.
    pub fn supply_point(&self, point: Uuid) -> Option<SupplyPoint> {
        let q = self.query();
        let f: Id<ew_derive::ew_schema::Function> = Id::from_uuid(point);
        let ew_derive::ew_schema::FunctionKind::Ip(ip) = &q.functions().get(f)?.kind else {
            return None;
        };
        let at = EndpointKey::Cp(f, 0);
        let def = q
            .potential_defs()
            .iter()
            .filter(|(_, d)| EndpointKey::of(&d.at) == Some(at))
            .min_by_key(|(id, _)| *id)
            .map(|(id, d)| (id, Arc::clone(d)));
        let typed = ew_derive::potential_id_at(&q, at).is_some_and(|p| {
            q.get::<ew_derive::PotentialAttrsOf>(p).ptype.is_some()
                || q.get::<ew_derive::PotentialFlow>(p)
                    .as_ref()
                    .as_ref()
                    .is_some_and(|e| e.ptype.is_some())
        });
        let word = |t: &ew_derive::ew_schema::PotentialType| {
            serde_json::to_value(t)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        let voltage = |d: &ew_derive::ew_schema::PotentialDef| {
            let current = match d.env.ac_dc {
                Some(ew_derive::ew_schema::AcDc::Ac) => " AC",
                Some(ew_derive::ew_schema::AcDc::Dc) => " DC",
                _ => "",
            };
            d.voltage.map(|v| format!("{v} V{current}")).unwrap_or_default()
        };
        Some(SupplyPoint {
            name: ip.name.to_string(),
            def: def.as_ref().map(|(id, _)| id.as_uuid()),
            ptype: def
                .as_ref()
                .and_then(|(_, d)| d.ptype.as_ref())
                .map(word)
                .unwrap_or_default(),
            voltage: def.as_ref().map(|(_, d)| voltage(d)).unwrap_or_default(),
            typed,
        })
    }

    /// The voltage of a phase of the project's incoming supply, as typed (`400 V AC`; `24 V DC` for a DC supply);
    /// `None` without supply settings.
    pub fn supply_phase_voltage(&self) -> Option<String> {
        let q = self.query();
        let s = q.project().supply.clone()?;
        let current = if s.frequency == ew_derive::ew_units::Dec3::ZERO {
            "DC"
        } else {
            "AC"
        };
        Some(format!("{} V {current}", s.u_nom))
    }

    /// Where the interruption points `points` (their functions) come from when every one names the same external
    /// reference: its location's code and the device there (`+Supply`, `-X0`); `None` otherwise (Where the
    /// supply comes from… shows it to change it).
    pub fn supply_source(&self, points: &[Uuid]) -> Option<(String, String)> {
        let q = self.query();
        let mut shared = None;
        for f in points {
            let ew_schema::FunctionKind::Ip(ip) = &q.functions().get(Id::from_uuid(*f))?.kind else {
                return None;
            };
            let x = ip.external?;
            if *shared.get_or_insert(x) != x {
                return None;
            }
        }
        let x = q.external_refs().get(shared?).cloned()?;
        let location = q.structure_nodes().get(x.location)?.code.as_str().to_owned();
        Some((location, x.device.as_str().to_owned()))
    }

    /// The other points of the group of the marked supply point `point` (`IpSpec::group`): the points of its
    /// place with its group, in ID order; empty for a point in no group.
    pub fn supply_group(&self, point: Uuid) -> Vec<Uuid> {
        let q = self.query();
        let spec = |f: &ew_schema::Function| match &f.kind {
            ew_schema::FunctionKind::Ip(ip) => ip.external.zip(ip.group),
            _ => None,
        };
        let Some(mine) = q.functions().get(Id::from_uuid(point)).and_then(|f| spec(f)) else {
            return Vec::new();
        };
        q.functions()
            .iter()
            .filter(|(id, f)| id.as_uuid() != point && spec(f) == Some(mine))
            .map(|(id, _)| id.as_uuid())
            .collect()
    }
}
