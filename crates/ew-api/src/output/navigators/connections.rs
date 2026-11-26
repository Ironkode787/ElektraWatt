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
    use ew_schema::{Connection, ConnectionKind, Endpoint};
    let Some(set) = ew_reports::project_rows(q, &ew_schema::ReportKind::WireList, None) else {
        return Vec::new();
    };
    let folios = folios(q);
    // Each wire's record: its constituents and where it keeps its own values (`WireRecord::own`).
    let records = q.get::<ew_derive::WireRecordSet>(());
    let by_wire: BTreeMap<Uuid, &ew_derive::WireRecord> =
        records.iter().map(|r| (r.wire.id.as_uuid(), r)).collect();
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
                .and_then(|w| by_wire.get(&w))
                .map(|w| w.wire.constituents.clone())
                .unwrap_or_default()
        };
        cons.sort();
        cons.dedup();
        // A wire row's first record is the one its cells set (`table_edit::row_targets`): the first connection where
        // the wire keeps its own values.
        let own: &[Id<Connection>] = (!after_heading)
            .then(|| row_entity(&r.id, 0).and_then(|w| by_wire.get(&w)))
            .flatten()
            .map_or(&[], |w| w.own.as_slice());
        if let Some(at) = own.first().and_then(|s| cons.iter().position(|c| c == s)) {
            let c = cons.remove(at);
            cons.insert(0, c);
        }
        let kinds = (cons.iter())
            .map(|c| match (after_heading, own.contains(c)) {
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

impl crate::Session {
    /// Where the end `end` (0 the From end, 1 the To end) of the wire a Connections row stands for (its records, the
    /// wire's constituent connections) is set by hand (doc 13 §2.3, `EndRecord::hand_at`): the wire's own connection
    /// that holds it, else its first one (`WireRecord::own`), and whether in its `end_a` (else `end_b`); `None` for an end made outside the
    /// project, for a wire without a connection of its own and for records that are no wire's.
    pub fn wire_end(&self, records: &[Uuid], end: usize) -> Option<(Uuid, bool)> {
        let q = self.query();
        let mut own: Vec<Uuid> = records.to_vec();
        own.sort();
        own.dedup();
        let set = q.get::<ew_derive::WireRecordSet>(());
        let record = set.iter().find(|r| {
            let mut c: Vec<Uuid> = r.wire.constituents.iter().map(|c| c.as_uuid()).collect();
            c.sort();
            c.dedup();
            c == own
        })?;
        record.ends.get(end)?.hand_at.map(|(c, a)| (c.as_uuid(), a))
    }
}
