// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `terminal_diagram`: the terminal diagram of one strip (FR-REP-009; from folio 903 of the standard layout).

use std::collections::BTreeSet;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{Device, Endpoint, ReportBlock, Side, Terminal};
use ew_derive::{Cp, CpRef, DevicePartLines, FunctionCps, PotentialNameOf, PotentialPartition, Q, short_tag};

use super::conn::{
    Bridge, WireRec, Wires, bridges, end_text, terminal_state, terminal_wired, text_cell, wires_by_cp,
};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

/// The terminal diagram.
///
/// The strips are the devices of the block's manual selection (`manual.include`) that have a strip aspect,
/// else every strip in natural order of its tag; one row per terminal of a strip in `(seq, ID)` order, and for a block
/// of several levels (doc 08 §6.3, FR-TRM-004) one row per level ([`row_levels`]): the level's designation, potential,
/// sides, bridge and state, its CPs without a level on its first row.
/// Row ID: `(strip, terminal, level)`, the level being 1 for a terminal of one level.
///
/// `strip` is the strip's tag, so that two strips' terminals of one number read apart (FT-1805). `label` is the
/// terminal's function text (the "1+" of the reference), `group` the caption of the strip
/// group that holds its `seq`. `side1` and `side2` list what is wired to the terminal's CPs of that side
/// (a CP without a side lists on side 1): the far end of
/// each wire, `via` the cable and core when the wire is a core. `bridge` is the label of the bridge the terminal belongs to (`BR1`).
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
            ColumnDef::new("strip", CellType::Tag, "Strip", "Klemmenstrook").weighted(8),
            ColumnDef::new("terminal", CellType::Ident, "Terminal", "Klem").weighted(7),
            ColumnDef::new("label", CellType::Text, "Function text", "Functietekst").weighted(8),
            ColumnDef::new("group", CellType::Text, "Group", "Groep").weighted(9),
            ColumnDef::new("part", CellType::Ident, "Part", "Artikel").weighted(11),
            ColumnDef::new("potential", CellType::Ident, "Potential", "Potentiaal").weighted(9),
            ColumnDef::new(
                "side1",
                CellType::Text,
                "Side 1 (clamp a)",
                "Zijde 1 (aansluiting a)",
            )
            .weighted(21),
            ColumnDef::new(
                "side2",
                CellType::Text,
                "Side 2 (clamp b)",
                "Zijde 2 (aansluiting b)",
            )
            .weighted(24),
            ColumnDef::new("bridge", CellType::Ident, "Bridge", "Brug").weighted(6),
            ColumnDef::new("state", CellType::Text, "State", "Status").weighted(7),
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

                let cps: Vec<(CpRef, Cp)> = q
                    .idx()
                    .functions_of_device(t)
                    .flat_map(|f| {
                        q.get::<FunctionCps>(f)
                            .iter()
                            .map(|c| (CpRef::new(f, c.idx), c.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                for (level, first) in row_levels(term, &cps) {
                    // The CPs of this row: the level's, and on the first row those without a level.
                    let here: Vec<&(CpRef, Cp)> = cps
                        .iter()
                        .filter(|(_, c)| level.is_none() || c.level == level || (first && c.level.is_none()))
                        .collect();
                    let (mut side1, mut side2) = (Vec::new(), Vec::new());
                    let mut potential = Cell::Empty;
                    for (r, cp) in &here {
                        if potential == Cell::Empty
                            && let Some(p) = partition.potential_at(r.key())
                        {
                            potential = Cell::Ident(q.get::<PotentialNameOf>(p.id).name.text.clone());
                        }
                        let Some(recs) = by_cp.get(r) else { continue };
                        // A CP whose part and definition give it no side lists on side 1, never nowhere.
                        let target = match cp.side {
                            Some(Side::Side2) => &mut side2,
                            _ => &mut side1,
                        };
                        for rec in recs {
                            target.push(target_text(q, rec, *r));
                        }
                    }
                    let bridge = bridges.iter().find(|b| {
                        b.terminals.contains(&t) && (level.is_none() || bridged_at(q, b, &here))
                    });
                    let wired = match level {
                        None => terminal_wired(q, t, &by_cp),
                        Some(_) => here.iter().any(|(r, _)| by_cp.contains_key(r)),
                    };
                    let state = terminal_state(&dev.state, wired, bridge.is_some());
                    let mut r = Row::data(
                        RowId(vec![
                            RowKey::Entity(strip.as_uuid()),
                            RowKey::Entity(t.as_uuid()),
                            RowKey::Index(level.map_or(1, u32::from)),
                        ]),
                        vec![
                            Cell::Tag(strip),
                            Cell::Ident(term.level_designation(level).as_str().to_owned()),
                            if dev.function_text.is_empty() {
                                Cell::Empty
                            } else {
                                Cell::Text(dev.function_text.clone())
                            },
                            group.clone(),
                            part.clone(),
                            potential,
                            text_cell(side1.join(", ")),
                            text_cell(side2.join(", ")),
                            bridge.map_or(Cell::Empty, |b| Cell::Ident(b.label.clone())),
                            Cell::Text(MlText::from_langs(
                                [("en-GB", state.0), ("nl-NL", state.1)]
                                    .into_iter()
                                    .filter_map(|(tag, text)| Some((tag.parse().ok()?, text))),
                            )),
                        ],
                    );
                    r.sources = vec![t.cast(), strip.cast()];
                    set.rows.push(r);
                }
            }
        }
        set
    }
}

/// The rows of a terminal as (level, whether it is the first row): one row (`None`) for a terminal of one level, else
/// one per level (doc 08 §6.3, FR-TRM-004): the levels it lists in their order, else those its CPs carry, ascending.
fn row_levels(term: &Terminal, cps: &[(CpRef, Cp)]) -> Vec<(Option<u8>, bool)> {
    let mut levels: Vec<u8> = Vec::new();
    let listed = term.levels.iter().map(|l| l.level);
    let carried: BTreeSet<u8> = cps.iter().filter_map(|(_, c)| c.level).collect();
    for l in listed.chain(carried) {
        if !levels.contains(&l) {
            levels.push(l);
        }
    }
    if levels.len() < 2 {
        return vec![(None, true)];
    }
    levels
        .into_iter()
        .enumerate()
        .map(|(i, l)| (Some(l), i == 0))
        .collect()
}

/// Whether one of the connections of bridge `b` ends at one of the CPs `here`.
fn bridged_at(q: &Q, b: &Bridge, here: &[&(CpRef, Cp)]) -> bool {
    let connections = q.connections();
    b.connections.iter().filter_map(|c| connections.get(*c)).any(|c| {
        [&c.a, &c.b].into_iter().any(|e| match e {
            Endpoint::Cp(f, i) => here.iter().any(|(r, _)| *r == CpRef::new(*f, *i)),
            _ => false,
        })
    })
}

/// The strips of a block: the strip devices of its manual selection, else all strips by natural tag (shared with
/// `terminal_overview`).
pub(super) fn strips_of(q: &Q, block: &ReportBlock) -> Vec<Id<Device>> {
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
        .map(|s| (natural_key(&short_tag(q, s)), s))
        .collect();
    keyed.sort();
    keyed.dedup();
    keyed.into_iter().map(|(_, s)| s).collect()
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
            short_tag(q, core.cable),
            core.core.as_str()
        ));
    }
    text
}
