// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Assemblies (doc 03 §7, FR-DEV-020 … FR-DEV-023, D-36, D-41): the break marks of an overview drawn over
//! several pages and the order-code text of an assembly device.
//!
//! # Break marks
//!
//! An assembly's overview is drawn by the overview representations of its members: the assembly device
//! itself, then its direct children in `Device.position` order (ties by tag natural order, then ID). A member
//! takes part with its overview representation ([`overview_rep`]). The members
//! form *segments*, the maximal runs of consecutive members drawn on one page. With two or more segments the
//! overview continues across pages, and every open end gets a break mark (doc 11 §5.5 step 5, FR-DEV-023):
//! a [`BreakSide::Right`] mark after the last representation of every segment but the last, and a
//! [`BreakSide::Left`] mark before the first representation of every segment but the first. The marks are
//! derived, never stored, so moving a module to another page moves its marks (store intent, derive
//! presentation).

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{NaturalKey, natural_key};
use ew_schema::{Device, LibCopy, Page, Part, RepType, Representation};

use crate::Q;
use crate::query::{Policy, Query};
use crate::tag::{DeviceTag, MainRep};

/// Which end of an overview segment a break mark closes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum BreakSide {
    /// Before the segment's first representation: the overview continues from an earlier page.
    Left,
    /// After the segment's last representation: the overview continues on a later page.
    Right,
}

/// One break mark of an assembly overview.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BreakMark {
    /// The page it is drawn on.
    pub page: Id<Page>,
    /// The representation whose side it is drawn at.
    pub rep: Id<Representation>,
    /// The side.
    pub side: BreakSide,
    /// The assembly device.
    pub assembly: Id<Device>,
}

/// `assembly_break_marks()`: every break mark of every assembly overview (see the module documentation),
/// sorted by page, representation and side.
#[derive(Debug)]
pub struct AssemblyBreakMarks;

impl Query for AssemblyBreakMarks {
    type Key = ();
    type Value = Vec<BreakMark>;
    const FAMILY: &'static str = "assembly_break_marks";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<Vec<BreakMark>> {
        let mut out = Vec::new();
        for (head, dev) in q.devices().iter() {
            if dev.aspects.assembly.is_none() {
                continue;
            }
            // The drawn members as (page, representation), in member order.
            let drawn: Vec<(Id<Page>, Id<Representation>)> = assembly_members(q, head)
                .into_iter()
                .filter_map(|m| {
                    let rep_id = overview_rep(q, m)?;
                    Some((q.reps().get(rep_id)?.page, rep_id))
                })
                .collect();
            let mut segments: Vec<Vec<(Id<Page>, Id<Representation>)>> = Vec::new();
            for d in drawn {
                match segments.last_mut() {
                    Some(seg) if seg.last().is_some_and(|l| l.0 == d.0) => seg.push(d),
                    _ => segments.push(vec![d]),
                }
            }
            let n = segments.len();
            if n < 2 {
                continue;
            }
            for (i, seg) in segments.iter().enumerate() {
                if let (true, Some(&(page, rep))) = (i > 0, seg.first()) {
                    out.push(BreakMark {
                        page,
                        rep,
                        side: BreakSide::Left,
                        assembly: head,
                    });
                }
                if let (true, Some(&(page, rep))) = (i + 1 < n, seg.last()) {
                    out.push(BreakMark {
                        page,
                        rep,
                        side: BreakSide::Right,
                        assembly: head,
                    });
                }
            }
        }
        out.sort();
        out.dedup();
        Arc::new(out)
    }
}

/// The members of the assembly `head`'s overview: `head`, then its direct children in `Device.position` order
/// (ties by tag natural order, then ID). A valve or a blanking plate is no member: it is a glyph of its module's
/// position (doc 11 §5.5 step 3), also when the module is the terminal itself (the starter valve terminal).
pub fn assembly_members(q: &Q, head: Id<Device>) -> Vec<Id<Device>> {
    let mut children: Vec<(u32, NaturalKey, Id<Device>)> = q
        .idx()
        .device_children(head)
        .filter_map(|c| {
            let d = q.devices().get(c)?;
            if d.kind == Some(ew_schema::DeviceKind::BlankingPlate)
                || !crate::fluid::valve_functions(q, c).is_empty()
            {
                return None;
            }
            let tag = q.get::<DeviceTag>(c);
            let full = tag.as_ref().as_ref().map(|t| t.full.clone()).unwrap_or_default();
            Some((d.position.unwrap_or(u32::MAX), natural_key(&full), c))
        })
        .collect();
    children.sort();
    std::iter::once(head)
        .chain(children.into_iter().map(|(_, _, c)| c))
        .collect()
}

/// The overview representation a member draws itself with: its main representation when that is an overview
/// representation, else the smallest-ID overview representation of its main function (one the overview generator
/// added to a module already drawn elsewhere, doc 11 §5.5).
pub fn overview_rep(q: &Q, device: Id<Device>) -> Option<Id<Representation>> {
    let reps = q.reps();
    let is_overview = |r: Id<Representation>| reps.get(r).is_some_and(|x| x.rep_type == RepType::Overview);
    if let Some(main) = (*q.get::<MainRep>(device)).filter(|r| is_overview(*r)) {
        return Some(main);
    }
    let f = (*q.get::<crate::MainFunction>(device))?;
    q.idx().reps_of_function(f).filter(|r| is_overview(*r)).min()
}

/// The order-code text of a device (the `order_code` symbol slot, doc 20 §11.1, D-36): the order code of
/// its assembly aspect's first config; empty for a device without one.
pub fn order_code_text(q: &Q, device: Id<Device>) -> String {
    q.devices()
        .get(device)
        .and_then(|d| d.aspects.assembly.as_ref())
        .and_then(|a| a.configs.first())
        .map(|c| c.order_code.as_str().to_owned())
        .unwrap_or_default()
}

/// The configured part of a device's order code: the part copy of its assembly aspect's first config, if the
/// config names one and the project holds it. The `manufacturer` slot beside an `order_code` slot prints its
/// manufacturer, not the main part's.
pub fn order_code_part(q: &Q, device: Id<Device>) -> Option<Arc<LibCopy<Part>>> {
    let copy = q
        .devices()
        .get(device)
        .and_then(|d| d.aspects.assembly.as_ref())
        .and_then(|a| a.configs.first())
        .and_then(|c| c.part.as_ref())
        .map(|p| p.copy)?;
    q.parts().get(copy).cloned()
}
