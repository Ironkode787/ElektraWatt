// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Terminal strips navigator (doc 16 §5.2): each strip's terminals in mounting order with the accessories between them.
//!
//! Not here: the strip editor's columns (`columns.rs`) and its edits (`output/strip_edit.rs`); cables (`cables.rs`).

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_ids::{Id, Uuid};
use ew_mltext::{NaturalKey, natural_key};
use ew_schema::{Device, DeviceState};

use super::{Builder, NavRole, base_facets, cell_of, device_drawables, folios, row_entity, state_word};
use ew_derive::short_tag;

/// The strips of the project in natural tag order.
fn strips(q: &Q) -> Vec<Id<Device>> {
    let mut v: Vec<(NaturalKey, Id<Device>)> = q
        .devices()
        .ids()
        .filter(|d| q.devices().get(*d).is_some_and(|x| x.aspects.strip.is_some()))
        .map(|d| (natural_key(&short_tag(q, d)), d))
        .collect();
    v.sort();
    v.into_iter().map(|(_, d)| d).collect()
}

/// Terminal strips: strip → its rows in mounting order: the accessories placed before the
/// first terminal, then each terminal (from the strip's terminal diagram) followed by the accessories placed after it.
/// The table has a leading `strip` column, `pos` (the terminal's `seq`, an accessory's `<after_seq>.<k>`) and
/// `bridge_part` (the part number of the terminal's bridge). Every node of a strip carries the facet `strip` (the
/// case-folded strip tag).
pub(super) fn strips_table(q: &Q, b: &mut Builder) {
    let folios = folios(q);
    // The bridge parts by terminal: the part number of the group's part, when it has one.
    let mut bridge_parts: BTreeMap<Id<Device>, String> = BTreeMap::new();
    for (_, _, g) in ew_reports::bridge_groups(q) {
        let number = g
            .part
            .as_ref()
            .and_then(|p| q.parts().get(p.copy))
            .map(|c| c.item.part_number.as_str().to_owned())
            .unwrap_or_default();
        for t in &g.terminals {
            bridge_parts.entry(*t).or_insert_with(|| number.clone());
        }
    }
    for strip in strips(q) {
        let Some(set) =
            ew_reports::project_rows(q, &ew_schema::ReportKind::TerminalDiagram, Some(strip.as_uuid()))
        else {
            continue;
        };
        let tag = short_tag(q, strip);
        let strip_facet = ("strip".to_owned(), tag.to_lowercase());
        let mut terminals = Vec::new();
        for (i, r) in set.rows.iter().enumerate() {
            if r.kind != ew_reports::RowKind::Data {
                continue;
            }
            let Some(t) = row_entity(&r.id, 1) else { continue };
            terminals.push((i, t));
        }
        let spare = |t: Uuid| {
            q.devices()
                .get(Id::from_uuid(t))
                .is_some_and(|d| d.state == DeviceState::Spare)
        };
        let n_spare = terminals.iter().filter(|(_, t)| spare(*t)).count();
        let drawables = device_drawables(q, &folios, strip);
        // The strip's own drawables are its strip segments.
        let strip_drawn = !drawables.is_empty();
        let mut badges = Vec::new();
        if n_spare > 0 {
            badges.push(format!("{n_spare} spare"));
        }
        let mut facets = base_facets(NavRole::Strip, &drawables);
        facets.push(strip_facet.clone());
        let snode = b.push(
            None,
            NavRole::Strip,
            &format!("strip:{}", strip.as_uuid()),
            tag.clone(),
            badges,
            vec![strip.as_uuid()],
            drawables,
            None,
            facets,
        );
        // The accessories in (after_seq, list index) order, each with its `pos` and the index of the terminal
        // it follows (`None`: before the first terminal).
        let seqs: Vec<u32> = terminals
            .iter()
            .map(|(_, t)| {
                q.devices()
                    .get(Id::from_uuid(*t))
                    .and_then(|d| d.aspects.terminal.as_ref())
                    .map_or(0, |a| a.seq)
            })
            .collect();
        let accessories = accessory_rows(q, strip, &seqs);
        let mut next_accessory = 0;
        let mut push_accessories = |b: &mut Builder, after: Option<usize>| {
            while let Some(a) = accessories.get(next_accessory)
                && a.after == after
            {
                let cells = vec![
                    tag.clone(),
                    a.pos.clone(),
                    String::new(),
                    a.word.to_owned(),
                    String::new(),
                    a.part.clone(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ];
                let mut facets = vec![("kind".to_owned(), "accessory".to_owned()), strip_facet.clone()];
                if !a.part.is_empty() {
                    facets.push(("part".to_owned(), a.part.to_lowercase()));
                }
                b.push(
                    Some(snode),
                    NavRole::Accessory,
                    &format!("accessory:{}", a.index),
                    a.word.to_owned(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Some(cells),
                    facets,
                );
                next_accessory += 1;
            }
        };
        push_accessories(b, None);
        // `bridge_part` follows `bridge`; the list's `strip` cell is the row's first already.
        let bridge_part_at = set.column("bridge").map_or(usize::MAX, |c| c + 1);
        let strip_at = set.column("strip");
        // The terminal diagram's Level column is the list's alone: the navigator's rows name a level by its number.
        let level_at = set.column("level");
        for (k, (i, t)) in terminals.into_iter().enumerate() {
            let texts = ew_reports::csv::row_texts(q, &set, i);
            let id: Id<Device> = Id::from_uuid(t);
            // A row of a block of two levels is that level (a persona read the two rows as two terminals): it selects
            // the symbol of its level, and a level from 2 has a key of its own.
            let level = level_at
                .and_then(|c| texts.get(c))
                .and_then(|l| l.parse::<u8>().ok());
            let mut drawables = device_drawables(q, &folios, id);
            let drawn = !drawables.is_empty();
            if let Some(l) = level {
                drawables.retain(|d| {
                    q.reps()
                        .get(Id::from_uuid(d.record))
                        .is_none_or(|r| ew_derive::rep_level(q, r).unwrap_or(1) == l)
                });
            }
            let mut facets = base_facets(NavRole::Terminal, &drawables);
            facets.push(strip_facet.clone());
            let mut badges = Vec::new();
            // A terminal with no representation of its own on a strip that no strip segment draws is unplaced
            // (a strip's segments draw all its terminals).
            if !drawn && !strip_drawn {
                badges.push("unplaced".to_owned());
                facets.push(("unplaced".to_owned(), String::new()));
            }
            if let Some(d) = q.devices().get(id) {
                if d.state == DeviceState::Spare {
                    badges.push("spare".to_owned());
                }
                facets.push(("state".to_owned(), state_word(&d.state).to_owned()));
            }
            let potential = cell_of(&set, &texts, "potential");
            if !potential.is_empty() {
                facets.push(("potential".to_owned(), potential.to_lowercase()));
            }
            let part = cell_of(&set, &texts, "part");
            if !part.is_empty() {
                facets.push(("part".to_owned(), part.to_lowercase()));
            }
            let label = cell_of(&set, &texts, "terminal").to_owned();
            let mut cells = vec![tag.clone(), seqs[k].to_string()];
            let bridge_part = bridge_parts.get(&id).cloned().unwrap_or_default();
            let n = texts.len();
            for (c, text) in texts.into_iter().enumerate() {
                if c == bridge_part_at {
                    cells.push(bridge_part.clone());
                }
                if Some(c) != strip_at && Some(c) != level_at {
                    cells.push(text);
                }
            }
            if bridge_part_at >= n {
                cells.push(bridge_part);
            }
            let key = match level.filter(|l| *l >= 2) {
                Some(l) => format!("terminal:{t}:{l}"),
                None => format!("terminal:{t}"),
            };
            b.push(
                Some(snode),
                NavRole::Terminal,
                &key,
                label,
                badges,
                vec![t],
                drawables,
                Some(cells),
                facets,
            );
            push_accessories(b, Some(k));
        }
    }
}

/// One accessory row of a strip.
struct AccessoryRow {
    /// Its index in `Strip.accessories`.
    index: usize,
    /// The index of the terminal (in mounting order) it follows; `None` before the first terminal.
    after: Option<usize>,
    /// `<after_seq>.<k>`.
    pos: String,
    /// The kind word (`End bracket`).
    word: &'static str,
    /// The part number, empty when the copy is missing.
    part: String,
}

/// The accessory rows of `strip` over its terminals' `seqs` (mounting order): each placed after the terminal whose
/// `seq` is the greatest not above its `after_seq` (before the first terminal when none is), in (after_seq, list
/// index) order; `k` counts from 1 among equal `after_seq` in list order.
fn accessory_rows(q: &Q, strip: Id<Device>, seqs: &[u32]) -> Vec<AccessoryRow> {
    let Some(aspect) = q.devices().get(strip).and_then(|d| d.aspects.strip.clone()) else {
        return Vec::new();
    };
    let mut order: Vec<(u32, usize)> = aspect
        .accessories
        .iter()
        .enumerate()
        .map(|(i, a)| (a.after_seq, i))
        .collect();
    order.sort_unstable();
    let mut out = Vec::with_capacity(order.len());
    let mut k = 0u32;
    let mut last: Option<u32> = None;
    for (after_seq, index) in order {
        k = if last == Some(after_seq) { k + 1 } else { 1 };
        last = Some(after_seq);
        let a = &aspect.accessories[index];
        let after = seqs.iter().rposition(|s| *s <= after_seq);
        let part = q
            .parts()
            .get(a.part.copy)
            .map(|c| c.item.part_number.as_str().to_owned())
            .unwrap_or_default();
        out.push(AccessoryRow {
            index,
            after,
            pos: format!("{after_seq}.{k}"),
            word: crate::output::strip_edit::accessory_word(&a.kind),
            part,
        });
    }
    // Stable by placement, so that the rows follow the terminals in one pass.
    out.sort_by_key(|a| a.after.map_or(0, |x| x + 1));
    out
}
