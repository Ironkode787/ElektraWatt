// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Cables navigator (doc 16 §5.2): the cable overview's cables and their cores.
//!
//! Not here: the strips (`strips.rs`); the bus nets a cable runs in (`bus.rs`).

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_derive::ew_ids::{Id, Uuid};

use super::{
    Builder, NavRole, base_facets, cell_of, connection_drawables, device_drawables, folios, row_entity,
};

// Cables: the per-cable rows of the cable overview as cable rows, the core rows as conductor rows under them;
/// the headings are dropped.
pub(super) fn cables_table(q: &Q, b: &mut Builder) {
    use ew_derive::ew_schema::{Conductor, ConnectionKind};
    let Some(set) =
        ew_reports::project_rows(q, &ew_derive::ew_schema::ReportKind::CableOverviewDiagrams, None)
    else {
        return;
    };
    let folios = folios(q);
    let partition = q.get::<ew_derive::PotentialPartition>(());
    let mut cables: BTreeMap<Uuid, u32> = BTreeMap::new();
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        let Some(cable) = row_entity(&r.id, 0) else {
            continue;
        };
        let texts = ew_reports::csv::row_texts(q, &set, i);
        let part = cell_of(&set, &texts, "part").to_lowercase();
        match row_entity(&r.id, 1) {
            None => {
                let drawables = device_drawables(q, &folios, Id::from_uuid(cable));
                let mut facets = base_facets(NavRole::Cable, &drawables);
                if !part.is_empty() {
                    facets.push(("part".to_owned(), part));
                }
                let label = cell_of(&set, &texts, "cable").to_owned();
                let mut texts = texts;
                texts.extend([String::new(), String::new(), String::new()]);
                let n = b.push(
                    None,
                    NavRole::Cable,
                    &format!("cable:{cable}"),
                    label,
                    Vec::new(),
                    vec![cable],
                    drawables,
                    Some(texts),
                    facets,
                );
                cables.insert(cable, n);
            }
            Some(core) => {
                let Some(&parent) = cables.get(&cable) else {
                    continue;
                };
                let cid: Id<Conductor> = Id::from_uuid(core);
                let mut cons: Vec<Id<ew_derive::ew_schema::Connection>> = q
                    .connections()
                    .iter()
                    .filter(|(_, c)| matches!(c.kind, ConnectionKind::Core(x) if x == cid))
                    .map(|(_, c)| c.id)
                    .collect();
                cons.sort();
                let drawables = connection_drawables(q, &folios, &cons);
                let facets = base_facets(NavRole::Conductor, &drawables);
                // The potential of the core's wires (the first end of its first connection that has one), its shield
                // flag and pair.
                let potential = cons
                    .iter()
                    .filter_map(|c| q.connections().get(*c))
                    .flat_map(|c| [&c.a, &c.b])
                    .find_map(|e| {
                        let key = ew_derive::EndpointKey::of(e)?;
                        let p = partition.potential_at(key)?;
                        Some(q.get::<ew_derive::PotentialNameOf>(p.id).name.text.clone())
                    })
                    .unwrap_or_default();
                let conductor = q.conductors().get(cid);
                let shield = conductor.map_or("", |c| if c.shield { "yes" } else { "no" });
                let pair = conductor
                    .and_then(|c| c.pair)
                    .map_or_else(String::new, |p| p.to_string());
                let label = cell_of(&set, &texts, "core").to_owned();
                let mut texts = texts;
                texts.extend([potential, shield.to_owned(), pair]);
                b.push(
                    Some(parent),
                    NavRole::Conductor,
                    &format!("conductor:{core}"),
                    label,
                    Vec::new(),
                    vec![cable, core],
                    drawables,
                    Some(texts),
                    facets,
                );
            }
        }
    }
}
