// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Structure identifiers navigator (§7.1): each aspect with its structure nodes nested by parent.
//!
//! Not here: the Devices tree's function and location groups (`devices.rs`).

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::natural_key;
use ew_derive::ew_rds::Aspect;

use super::{Builder, NavRole, group, row_entity, tokens};

/// Structure identifiers (§7.1): aspect → structure nodes (nested by `parent`), the structure-node rows of the
/// structure overview with a `usage` column (the devices and pages that name the node). A node's aliases are filter
/// tokens (they are never printed, D-03).
pub(super) fn structure_table(q: &Q, b: &mut Builder) {
    use ew_derive::ew_schema::{Open, StructureNode};
    use ew_derive::{DevicesByStructure, PageStructure};
    let Some(set) = ew_reports::project_rows(q, "structure_overview", None) else {
        return;
    };
    let by_device = q.get::<DevicesByStructure>(());
    let mut by_page: BTreeMap<Id<StructureNode>, usize> = BTreeMap::new();
    for p in q.pages().ids() {
        for s in q.get::<PageStructure>(p).slots.values() {
            *by_page.entry(s.node).or_default() += 1;
        }
    }
    // The structure-node rows, by node.
    let mut rows: Vec<(Id<StructureNode>, Vec<String>)> = Vec::new();
    for (i, r) in set.rows.iter().enumerate() {
        let Some(u) = row_entity(&r.id, 0) else { continue };
        let id: Id<StructureNode> = Id::from_uuid(u);
        if !q.structure_nodes().contains(id) {
            continue;
        }
        let mut texts = ew_reports::csv::row_texts(q, &set, i);
        let usage = by_device.get(&id).map_or(0, Vec::len) + by_page.get(&id).copied().unwrap_or(0);
        texts.push(usage.to_string());
        rows.push((id, texts));
    }
    let aspect_of = |id: Id<StructureNode>| match q.structure_nodes().get(id).map(|n| n.aspect.clone()) {
        Some(Open::Known(a)) => Some(a),
        _ => None,
    };
    let mut aspects: Vec<Option<Aspect>> = rows.iter().map(|(id, _)| aspect_of(*id)).collect();
    aspects.sort();
    aspects.dedup();
    // Aspects in declaration order, an unknown aspect last.
    aspects.sort_by_key(|a| (a.is_none(), *a));
    let mut placed: BTreeMap<Id<StructureNode>, u32> = BTreeMap::new();
    for aspect in aspects {
        let (role, word) = match aspect {
            Some(Aspect::Function) => (NavRole::Function, "function"),
            Some(Aspect::Location) => (NavRole::Location, "location"),
            Some(a) => (NavRole::StructureNode, a.as_str()),
            None => (NavRole::StructureNode, "unknown"),
        };
        let anode = group(b, None, role, &format!("aspect:{word}"), word.to_owned());
        let mine: Vec<&(Id<StructureNode>, Vec<String>)> =
            rows.iter().filter(|(id, _)| aspect_of(*id) == aspect).collect();
        // Parents first (by depth), then natural designation; a parent outside the aspect puts the node under it.
        let mut order = mine;
        order.sort_by_key(|(id, t)| {
            (
                depth(q, *id),
                natural_key(t.first().map_or("", String::as_str)),
                *id,
            )
        });
        for (id, texts) in order {
            let parent = q
                .structure_nodes()
                .get(*id)
                .and_then(|n| n.parent)
                .and_then(|p| placed.get(&p).copied())
                .unwrap_or(anode);
            let facets = vec![("kind".to_owned(), "structure_node".to_owned())];
            let n = b.push(
                Some(parent),
                NavRole::StructureNode,
                &format!("structure_node:{}", id.as_uuid()),
                texts.first().cloned().unwrap_or_default(),
                Vec::new(),
                vec![id.as_uuid()],
                Vec::new(),
                Some(texts.clone()),
                facets,
            );
            if let Some(node) = q.structure_nodes().get(*id) {
                let t = &mut b.nodes[n as usize].tokens;
                t.extend(node.aliases.iter().flat_map(|a| tokens(a.as_str())));
                t.sort();
                t.dedup();
            }
            placed.insert(*id, n);
        }
    }
}

/// The depth of a structure node (0 at the top), bounded by the table's size.
fn depth(q: &Q, id: Id<ew_derive::ew_schema::StructureNode>) -> usize {
    let mut d = 0;
    let mut at = q.structure_nodes().get(id).and_then(|n| n.parent);
    while let Some(p) = at {
        d += 1;
        if d > q.structure_nodes().len() {
            break;
        }
        at = q.structure_nodes().get(p).and_then(|n| n.parent);
    }
    d
}
