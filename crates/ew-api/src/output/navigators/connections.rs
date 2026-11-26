// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Connections navigator (doc 16 §5.2): the wire list's physical-wire rows under their potential, then the bridges.
//!
//! Not here: the Potentials navigator, which reuses the wire rows (`potentials.rs`); a connection's drawables
//! (`navigators.rs`).

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_ids::{Id, Uuid};
use ew_schema::Device;

use super::{
    Builder, Drawable, NavRole, base_facets, cell_of, connection_drawables, folios, group, row_entity,
    row_segment,
};
use super::{plain, plains};

/// One data row of the wire list as the Connections and Potentials navigators show it.
pub(super) struct WireListRow {
    /// The row ID's key segment.
    segment: String,
    /// The label: the wire number, else `<from> <from CP> → <to> <to CP>`.
    label: String,
    /// The cells.
    cells: Vec<String>,
    /// The connections the row stands for; a wire row's first the one its cells set (`row_targets`).
    records: Vec<Uuid>,
    /// Their kinds for edit routing, aligned with `records`: `connection` for a wire's own connection, `shared` for one
    /// another wire stands for as well (a star's feed arm, a chain's arm at a CP), `bridge` for a bridge row's.
    kinds: Vec<&'static str>,
    /// Those that are drawn.
    drawables: Vec<Drawable>,
    /// The potential cell.
    pub(super) potential: String,
    /// Whether it is a bridge row (after the "Bridges" heading).
    pub(super) bridge: bool,
}

/// The wire list's data rows (Connections): a physical-wire row stands for the constituent connections of its
/// physical wire (`physical_wires_of` of every potential), a bridge row for the bridge connections of its strip and
/// group.
pub(super) fn wire_list_rows(q: &Q) -> Vec<WireListRow> {
    use ew_derive::{PhysicalWiresOf, PotentialPartition};
    use ew_schema::{Connection, ConnectionKind, Endpoint};
    let Some(set) = ew_reports::project_rows(q, &ew_schema::ReportKind::WireList, None) else {
        return Vec::new();
    };
    let folios = folios(q);
    let partition = q.get::<PotentialPartition>(());
    let mut constituents: BTreeMap<Uuid, Vec<Id<Connection>>> = BTreeMap::new();
    for p in partition.potentials() {
        for w in &q.get::<PhysicalWiresOf>(p.id).wires {
            constituents
                .entry(w.id.as_uuid())
                .or_insert_with(|| w.constituents.clone());
        }
    }
    // How many wires stand for each connection.
    let mut wires_of: BTreeMap<Id<Connection>, usize> = BTreeMap::new();
    for cons in constituents.values() {
        let mut own = cons.clone();
        own.sort();
        own.dedup();
        for c in own {
            *wires_of.entry(c).or_default() += 1;
        }
    }
    let alone = |c: &Id<Connection>| wires_of.get(c).is_some_and(|n| *n == 1);
    let strip_of = |e: &Endpoint| -> Option<Id<Device>> {
        let Endpoint::Cp(f, _) = e else { return None };
        let d = q.functions().get(*f)?.device?;
        q.devices().get(d)?.aspects.terminal.as_ref().map(|t| t.strip)
    };
    let mut bridges: BTreeMap<(Uuid, u32), Vec<Id<Connection>>> = BTreeMap::new();
    for (_, c) in q.connections().iter() {
        let ConnectionKind::Bridge(spec) = &c.kind else {
            continue;
        };
        if let Some(strip) = strip_of(&c.a).or_else(|| strip_of(&c.b)) {
            bridges
                .entry((strip.as_uuid(), u32::from(spec.group)))
                .or_default()
                .push(c.id);
        }
    }
    let mut out = Vec::new();
    let mut after_heading = false;
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            after_heading = true;
            continue;
        }
        let texts = ew_reports::csv::row_texts(q, &set, i);
        let mut cons: Vec<Id<Connection>> = if after_heading {
            let group = r.id.0.iter().find_map(|k| match k {
                ew_reports::RowKey::Index(g) => Some(*g),
                _ => None,
            });
            match (row_entity(&r.id, 0), group) {
                (Some(strip), Some(g)) => bridges.get(&(strip, g)).cloned().unwrap_or_default(),
                _ => Vec::new(),
            }
        } else {
            row_entity(&r.id, 0)
                .and_then(|w| constituents.get(&w).cloned())
                .unwrap_or_default()
        };
        cons.sort();
        cons.dedup();
        // A wire row's first record is the one its cells set (`table_edit::row_targets`): of the connections no other
        // wire stands for, the first in the order the wire takes its values (doc 08 §3.8): the carrier, then its
        // constituents in path order.
        if !after_heading && let Some(carrier) = row_entity(&r.id, 0) {
            let order = constituents.get(&carrier).map(Vec::as_slice).unwrap_or_default();
            let set = (cons.iter().copied())
                .filter(|c| c.as_uuid() == carrier)
                .chain(order.iter().copied())
                .find(|c| alone(c));
            if let Some(at) = set.and_then(|s| cons.iter().position(|c| *c == s)) {
                let c = cons.remove(at);
                cons.insert(0, c);
            }
        }
        let kinds = (cons.iter())
            .map(|c| match (after_heading, alone(c)) {
                (true, _) => "bridge",
                (false, true) => "connection",
                (false, false) => "shared",
            })
            .collect();
        let number = cell_of(&set, &texts, "no");
        let label = if number.is_empty() {
            format!(
                "{} {} \u{2192} {} {}",
                cell_of(&set, &texts, "from"),
                cell_of(&set, &texts, "from_cp"),
                cell_of(&set, &texts, "to"),
                cell_of(&set, &texts, "to_cp")
            )
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
        } else {
            number.to_owned()
        };
        out.push(WireListRow {
            segment: row_segment(&r.id),
            label,
            potential: cell_of(&set, &texts, "potential").to_owned(),
            records: cons.iter().map(|c| c.as_uuid()).collect(),
            kinds,
            drawables: connection_drawables(q, &folios, &cons),
            cells: texts,
            bridge: after_heading,
        });
    }
    out
}

/// Pushes one wire-list row as a connection node.
pub(super) fn push_wire_row(b: &mut Builder, parent: u32, w: &WireListRow, table_row: bool) -> u32 {
    let mut facets = base_facets(NavRole::Connection, &w.drawables);
    for p in w
        .potential
        .split('\u{21C4}')
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        facets.push(("potential".to_owned(), p.to_lowercase()));
    }
    let n = b.push(
        Some(parent),
        NavRole::Connection,
        &format!("connection:{}", w.segment),
        plain(w.label.clone()),
        Vec::new(),
        w.records.clone(),
        w.drawables.clone(),
        table_row.then(|| plains(w.cells.clone())),
        facets,
    );
    b.nodes[n as usize].kinds = w.kinds.iter().map(|k| (*k).to_owned()).collect();
    n
}

/// Connections: potential → physical-wire rows; a `Bridges` group → the bridge rows.
pub(super) fn connections_table(q: &Q, b: &mut Builder) {
    let rows = wire_list_rows(q);
    let mut groups: BTreeMap<String, u32> = BTreeMap::new();
    let mut bridges: Option<u32> = None;
    for w in &rows {
        let parent = if w.bridge {
            *bridges.get_or_insert_with(|| {
                group(b, None, NavRole::Section, "section:Bridges", text!("nav-bridges"))
            })
        } else {
            // A wire without a potential under "(none)", its key in English.
            let label = if w.potential.is_empty() {
                text!("nav-none")
            } else {
                plain(w.potential.clone())
            };
            let name = label.to_string();
            match groups.get(&name) {
                Some(g) => *g,
                None => {
                    let g = group(b, None, NavRole::Potential, &format!("potential:{name}"), label);
                    groups.insert(name, g);
                    g
                }
            }
        };
        push_wire_row(b, parent, w, true);
    }
}
