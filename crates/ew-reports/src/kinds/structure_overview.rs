// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `structure_overview`: the structure-identifier and device-tag overview (FR-REP-017; golden 910).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{Device, ReportBlock, StructureNode};
use ew_derive::{
    DevicePartLines, DeviceTag, Q, ResolvedSlot, ResolvedStructure, SlotSource, StructureOf, StructurePath,
};

use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowSet};

/// The structure-identifier and device-tag overview.
///
/// One row per structure node, then one per device that is not a terminal (terminals are listed by the
/// terminal reports).
#[derive(Clone, Copy, Debug)]
pub struct StructureOverview;

impl ReportKindDef for StructureOverview {
    fn name(&self) -> &'static str {
        "structure_overview"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("designation", CellType::Ident, "Designation"),
            ColumnDef::new("kind", CellType::Text, "Kind"),
            ColumnDef::new("function_text", CellType::Text, "Function text"),
            ColumnDef::new("part", CellType::Ident, "Part"),
            ColumnDef::new("location", CellType::Ident, "Location (+)"),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());

        let mut nodes: Vec<((NaturalKey, Id<StructureNode>), Row)> = Vec::new();
        for id in q.structure_nodes().ids() {
            let path = q.get::<StructurePath>(id);
            let designation = match path.aspect {
                Some(aspect) => {
                    // Reuse the structure renderer: a one-slot structure prints the path with its sign.
                    let mut st = ResolvedStructure::default();
                    st.slots.insert(
                        aspect,
                        ResolvedSlot {
                            node: id,
                            codes: path.codes.clone(),
                            source: SlotSource::Explicit,
                        },
                    );
                    st.render(&Default::default())
                }
                None => path.codes.join("/"),
            };
            let mut r = Row::data(
                RowId::entity(id),
                vec![
                    Cell::Ident(designation.clone()),
                    Cell::Text(MlText::neutral("Structure")),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                ],
            );
            r.sources = vec![id.cast()];
            nodes.push(((natural_key(&designation), id), r));
        }
        nodes.sort_by(|a, b| a.0.cmp(&b.0));
        set.rows.extend(nodes.into_iter().map(|(_, r)| r));

        let mut devices: Vec<((NaturalKey, Id<Device>), Row)> = Vec::new();
        for id in q.devices().ids() {
            let Some(dev) = q.devices().get(id) else { continue };
            if dev.aspects.terminal.is_some() {
                continue;
            }
            let tag = q.get::<DeviceTag>(id);
            let full = tag.as_ref().as_ref().map(|t| t.full.clone()).unwrap_or_default();
            let function_text = if dev.function_text.is_empty() {
                Cell::Empty
            } else {
                Cell::Text(dev.function_text.clone())
            };
            let part = q
                .get::<DevicePartLines>(id)
                .first()
                .and_then(|l| {
                    q.parts()
                        .get(l.part)
                        .map(|c| c.item.part_number.as_str().to_owned())
                })
                .map_or(Cell::Empty, Cell::Ident);
            let location = q
                .get::<StructureOf>(id)
                .designation(|a| a.as_str() == "location")
                .to_string();
            let mut r = Row::data(
                RowId::entity(id),
                vec![
                    Cell::Ident(full.clone()),
                    Cell::Text(MlText::neutral("Device")),
                    function_text,
                    part,
                    if location.is_empty() {
                        Cell::Empty
                    } else {
                        Cell::Ident(location)
                    },
                ],
            );
            r.sources = vec![id.cast()];
            devices.push(((natural_key(&full), id), r));
        }
        devices.sort_by(|a, b| a.0.cmp(&b.0));
        set.rows.extend(devices.into_iter().map(|(_, r)| r));
        set
    }
}
