// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `terminal_diagram`: the terminal diagram of one strip (FR-REP-009; golden 903, 904, 905).

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{Device, DeviceState, ReportBlock, Side};
use ew_derive::{CpRef, DevicePartLines, FunctionCps, PotentialNameOf, PotentialPartition, Q};

use super::conn::{WireRec, Wires, bridges, end_text, tag_short, text_cell};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

/// The terminal diagram.
///
/// The strips are the devices of the block's manual selection (`manual.include`) that have a strip aspect,
/// else every strip in natural order of its tag; one row per terminal of a strip in `(seq, ID)` order.
/// Row ID: `(strip, terminal, level)`, the level being 1 (multi-level terminals get a row per level with
/// their level terminals, not modelled yet).
///
/// `label` is the terminal's function text (the "1+" of the reference), `group` the caption of the strip
/// group that holds its `seq`. `side1` and `side2` list what is wired to the terminal's CPs of that side
/// (clamp a is side 1, clamp b side 2 on the golden strips): the far end of each wire, `via` the cable and
/// core when the wire is a core. `bridge` is the label of the bridge the terminal belongs to (`BR1`).
/// `state` is `spare`, `reserved` or `not fitted` as authored, else `used` when the terminal has a wire or
/// a bridge and `unused` otherwise (doc 08 §6.6).
///
/// Wire numbers (doc 08 §5) are not derived yet, so a target prints no number.
#[derive(Clone, Copy, Debug)]
pub struct TerminalDiagram;

impl ReportKindDef for TerminalDiagram {
    fn name(&self) -> &'static str {
        "terminal_diagram"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("terminal", CellType::Ident, "Terminal").weighted(7),
            ColumnDef::new("label", CellType::Text, "Function text").weighted(8),
            ColumnDef::new("group", CellType::Text, "Group").weighted(9),
            ColumnDef::new("part", CellType::Ident, "Part").weighted(11),
            ColumnDef::new("potential", CellType::Ident, "Potential").weighted(9),
            ColumnDef::new("side1", CellType::Text, "Side 1 (clamp a)").weighted(21),
            ColumnDef::new("side2", CellType::Text, "Side 2 (clamp b)").weighted(24),
            ColumnDef::new("bridge", CellType::Ident, "Bridge").weighted(6),
            ColumnDef::new("state", CellType::Text, "State").weighted(7),
        ]
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let strips = strips_of(q, block);
        let wires = Wires::of(q);
        let by_cp = wires_by_cp(&wires);
        let bridges = bridges(q);
        let partition = q.get::<PotentialPartition>(());

        let mut set = RowSet::new(columns);
        for strip in strips {
            let Some(strip_dev) = q.devices().get(strip) else {
                continue;
            };
            let aspect = strip_dev.aspects.strip.as_ref();
            let mut terminals: Vec<(u32, Id<Device>)> = q
                .idx()
                .terminals_of_strip(strip)
                .filter_map(|t| Some((q.devices().get(t)?.aspects.terminal.as_ref()?.seq, t)))
                .collect();
            terminals.sort_unstable();
            for (seq, t) in terminals {
                let Some(dev) = q.devices().get(t) else { continue };
                let Some(term) = dev.aspects.terminal.as_ref() else {
                    continue;
                };
                let group = aspect
                    .and_then(|a| a.groups.iter().find(|g| g.from_seq <= seq && seq <= g.to_seq))
                    .map_or(Cell::Empty, |g| Cell::Text(g.label.clone()));
                let part = q
                    .get::<DevicePartLines>(t)
                    .first()
                    .and_then(|l| q.parts().get(l.part))
                    .map_or(Cell::Empty, |c| {
                        super::parts::part_number_cell(c.item.part_number.as_str())
                    });

                let (mut side1, mut side2) = (Vec::new(), Vec::new());
                let mut potential = Cell::Empty;
                let mut wired = false;
                for f in q.idx().functions_of_device(t) {
                    for cp in q.get::<FunctionCps>(f).iter() {
                        let r = CpRef::new(f, cp.idx);
                        if potential == Cell::Empty
                            && let Some(p) = partition.potential_at(r.key())
                        {
                            potential = Cell::Ident(q.get::<PotentialNameOf>(p.id).name.text.clone());
                        }
                        let Some(recs) = by_cp.get(&r) else { continue };
                        wired = true;
                        let target = match cp.side {
                            Some(Side::Side1) => &mut side1,
                            Some(Side::Side2) => &mut side2,
                            _ => continue,
                        };
                        for rec in recs {
                            target.push(target_text(q, rec, r));
                        }
                    }
                }
                let bridge = bridges.iter().find(|b| b.terminals.contains(&t));
                let state = match &dev.state {
                    DeviceState::Spare => "spare",
                    DeviceState::Reserved => "reserved",
                    DeviceState::NotFitted => "not fitted",
                    DeviceState::Used | DeviceState::Unknown(_) => {
                        if wired || bridge.is_some() {
                            "used"
                        } else {
                            "unused"
                        }
                    }
                };
                let mut r = Row::data(
                    RowId(vec![
                        RowKey::Entity(strip.as_uuid()),
                        RowKey::Entity(t.as_uuid()),
                        RowKey::Index(1),
                    ]),
                    vec![
                        Cell::Ident(term.designation.as_str().to_owned()),
                        if dev.function_text.is_empty() {
                            Cell::Empty
                        } else {
                            Cell::Text(dev.function_text.clone())
                        },
                        group,
                        part,
                        potential,
                        text_cell(side1.join(", ")),
                        text_cell(side2.join(", ")),
                        bridge.map_or(Cell::Empty, |b| Cell::Ident(b.label.clone())),
                        Cell::Text(MlText::neutral(state)),
                    ],
                );
                r.sources = vec![t.cast(), strip.cast()];
                set.rows.push(r);
            }
        }
        set
    }
}

/// The strips of a block: the strip devices of its manual selection, else all strips by natural tag.
fn strips_of(q: &Q, block: &ReportBlock) -> Vec<Id<Device>> {
    let is_strip = |d: Id<Device>| q.devices().get(d).is_some_and(|d| d.aspects.strip.is_some());
    let selected: Vec<Id<Device>> = block
        .manual
        .include
        .as_slice()
        .iter()
        .map(|a| Id::<Device>::from_uuid(a.as_uuid()))
        .filter(|d| is_strip(*d))
        .collect();
    let mut strips = if selected.is_empty() {
        q.devices().ids().filter(|d| is_strip(*d)).collect()
    } else {
        selected
    };
    let mut keyed: Vec<(NaturalKey, Id<Device>)> = strips
        .drain(..)
        .map(|s| (natural_key(&tag_short(q, s)), s))
        .collect();
    keyed.sort();
    keyed.dedup();
    keyed.into_iter().map(|(_, s)| s).collect()
}

/// The physical wires by the CPs they end at.
fn wires_by_cp(wires: &Wires) -> BTreeMap<CpRef, Vec<&WireRec>> {
    let mut map: BTreeMap<CpRef, Vec<&WireRec>> = BTreeMap::new();
    for rec in &wires.wires {
        for end in &rec.wire.ends {
            map.entry(end.cp).or_default().push(rec);
        }
    }
    map
}

/// What the wire `rec` ending at `at` leads to: the far end, and the cable and core when it is a core.
fn target_text(q: &Q, rec: &WireRec, at: CpRef) -> String {
    let far = rec
        .wire
        .ends
        .iter()
        .find(|e| e.cp != at)
        .or_else(|| rec.wire.ends.first());
    let mut text = far.map(|e| end_text(q, e)).unwrap_or_default();
    if let Some(core) = rec.wire.core.and_then(|c| q.conductors().get(c)) {
        text.push_str(&format!(
            " via {} {}",
            tag_short(q, core.cable),
            core.core.as_str()
        ));
    }
    text
}
