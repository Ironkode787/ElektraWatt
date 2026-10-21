// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `part_data`: the embedded PART DATA box (FR-REP-028; golden 201, 202, 902).

use std::collections::BTreeSet;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey};
use ew_derive::ew_schema::{Device, ReportBlock};
use ew_derive::{DevicePartLines, MainRep, PartLine, Q};

use super::parts::{line_id, part_cells, tag_key};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

/// The PART DATA box.
///
/// Source devices: the block's `manual.include` devices, else the root ancestors of the devices drawn on the
/// embedded page. A source with an assembly aspect that has configs lists one row per order code (doc 11
/// §5.5 step 6, D-36): the config's label ("Order Code 1"; "Order Code n" when it has none), and the
/// manufacturer, part number and description of its configured part, or the order code alone without one.
/// Otherwise it lists the part lines of the device and of those descendants that have no representation of
/// their own (a cabinet's mounting plate; a valve that the drawings show as its own symbol is not repeated in
/// the box).
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
            if push_configs(q, &mut set, d) {
                continue;
            }
            let lines = q.get::<DevicePartLines>(d);
            for i in 0..lines.len() {
                push_row(q, &mut set, d, &lines, i, Cell::Empty);
            }
            for child in descendants(q, d)
                .into_iter()
                .filter(|c| q.get::<MainRep>(*c).is_none())
            {
                let child_lines = q.get::<DevicePartLines>(child);
                for i in 0..child_lines.len() {
                    push_row(q, &mut set, d, &child_lines, i, Cell::Empty);
                }
            }
        }
        set
    }
}

/// The order-code rows of `source`'s assembly configs; `false` when it has none.
fn push_configs(q: &Q, set: &mut RowSet, source: Id<Device>) -> bool {
    let Some(dev) = q.devices().get(source) else {
        return false;
    };
    let Some(assembly) = dev.aspects.assembly.as_ref().filter(|a| !a.configs.is_empty()) else {
        return false;
    };
    for (n, config) in assembly.configs.iter().enumerate() {
        let label = if config.label.is_empty() {
            MlText::neutral(&format!("Order Code {}", n + 1))
        } else {
            config.label.clone()
        };
        let copy = config.part.as_ref().and_then(|p| q.parts().get(p.copy));
        let [manufacturer, part_number, description] = match copy {
            Some(copy) => part_cells(&copy.item),
            None => [
                Cell::Empty,
                Cell::Ident(config.order_code.as_str().to_owned()),
                Cell::Empty,
            ],
        };
        let id = RowId(vec![
            RowKey::Entity(source.as_uuid()),
            RowKey::Name("config".to_owned()),
            RowKey::Index(u32::try_from(n).unwrap_or(u32::MAX)),
        ]);
        let mut row = Row::data(
            id,
            vec![Cell::Text(label), manufacturer, part_number, description],
        );
        row.sources = vec![source.cast()];
        set.rows.push(row);
    }
    true
}

fn push_row(q: &Q, set: &mut RowSet, source: Id<Device>, lines: &[PartLine], index: usize, label: Cell) {
    let line = &lines[index];
    let Some(copy) = q.parts().get(line.part) else {
        return;
    };
    let [manufacturer, part_number, description] = part_cells(&copy.item);
    let prefix = RowId(vec![
        RowKey::Entity(source.as_uuid()),
        RowKey::Entity(line.device.as_uuid()),
    ]);
    let id = line_id(prefix, lines, index);
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
