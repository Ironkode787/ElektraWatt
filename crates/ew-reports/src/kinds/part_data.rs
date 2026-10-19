// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `part_data`: the embedded PART DATA box (FR-REP-028; golden 201, 202, 902).

use std::collections::BTreeSet;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey};
use ew_derive::ew_schema::{Device, ReportBlock};
use ew_derive::{DevicePartLines, MainRep, PartLine, Q};

use super::parts::{part_cells, tag_key};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

/// The PART DATA box.
///
/// Source devices: the block's `manual.include` devices, else the root ancestors of the devices drawn on the
/// embedded page. A source with configured part lines lists one row per order code; otherwise it lists the
/// part lines of the device and of those descendants that have no representation of their own (a
/// cabinet's mounting plate; a valve that the drawings show as its own symbol is not repeated in the box).
#[derive(Clone, Copy, Debug)]
pub struct PartData;

impl ReportKindDef for PartData {
    fn name(&self) -> &'static str {
        "part_data"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("label", CellType::Text, "Label"),
            ColumnDef::new("manufacturer", CellType::Ident, "Manufacturer"),
            ColumnDef::new("part_number", CellType::Ident, "Part number"),
            ColumnDef::new("description", CellType::Text, "Description"),
        ]
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for d in source_devices(q, block) {
            let lines = q.get::<DevicePartLines>(d);
            let configured: Vec<(usize, &PartLine)> =
                lines.iter().enumerate().filter(|(_, l)| l.configured).collect();
            if !configured.is_empty() {
                for (n, (i, line)) in configured.into_iter().enumerate() {
                    let label = Cell::Text(MlText::neutral(&format!("Order Code {}", n + 1)));
                    push_row(q, &mut set, d, line, i, label);
                }
                continue;
            }
            for (i, line) in lines.iter().enumerate() {
                push_row(q, &mut set, d, line, i, Cell::Empty);
            }
            for child in descendants(q, d)
                .into_iter()
                .filter(|c| q.get::<MainRep>(*c).is_none())
            {
                for (i, line) in q.get::<DevicePartLines>(child).iter().enumerate() {
                    push_row(q, &mut set, d, line, i, Cell::Empty);
                }
            }
        }
        set
    }
}

fn push_row(q: &Q, set: &mut RowSet, source: Id<Device>, line: &PartLine, index: usize, label: Cell) {
    let Some(copy) = q.parts().get(line.part) else {
        return;
    };
    let [manufacturer, part_number, description] = part_cells(&copy.item);
    let id = RowId(vec![
        RowKey::Entity(source.as_uuid()),
        RowKey::Entity(line.device.as_uuid()),
        RowKey::Entity(line.part.as_uuid()),
        RowKey::Index(index as u32),
    ]);
    let mut r = Row::data(id, vec![label, manufacturer, part_number, description]);
    r.sources = vec![line.device.cast()];
    set.rows.push(r);
}

/// The source devices in natural order of full tag, then ID.
fn source_devices(q: &Q, block: &ReportBlock) -> Vec<Id<Device>> {
    let included: BTreeSet<Id<Device>> = block
        .manual
        .include
        .as_slice()
        .iter()
        .map(|id| id.cast::<Device>())
        .filter(|d| q.devices().contains(*d))
        .collect();
    let set = if !included.is_empty() {
        included
    } else if let Some(embed) = &block.embedded {
        let mut roots = BTreeSet::new();
        for rep in q.idx().reps_by_page(embed.page) {
            let Some(rep) = q.reps().get(rep) else { continue };
            for f in &rep.functions {
                if let Some(d) = q.functions().get(*f).and_then(|f| f.device) {
                    roots.insert(root_of(q, d));
                }
            }
        }
        roots
    } else {
        BTreeSet::new()
    };
    sorted(q, set)
}

fn sorted(q: &Q, set: BTreeSet<Id<Device>>) -> Vec<Id<Device>> {
    let mut v: Vec<(NaturalKey, Id<Device>)> = set.into_iter().map(|d| (tag_key(q, d), d)).collect();
    v.sort();
    v.into_iter().map(|(_, d)| d).collect()
}

/// The topmost ancestor of a device (`Device.parent`), cycle-safe.
fn root_of(q: &Q, device: Id<Device>) -> Id<Device> {
    let mut seen = BTreeSet::from([device]);
    let mut current = device;
    while let Some(parent) = q.devices().get(current).and_then(|d| d.parent) {
        if !seen.insert(parent) {
            break;
        }
        current = parent;
    }
    current
}

/// Every descendant of a device, in natural order of full tag, cycle-safe.
fn descendants(q: &Q, device: Id<Device>) -> Vec<Id<Device>> {
    let mut seen = BTreeSet::from([device]);
    let mut found = BTreeSet::new();
    let mut stack = vec![device];
    while let Some(d) = stack.pop() {
        for child in q.idx().device_children(d) {
            if seen.insert(child) {
                found.insert(child);
                stack.push(child);
            }
        }
    }
    sorted(q, found)
}
