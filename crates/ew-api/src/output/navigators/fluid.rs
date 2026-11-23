// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid navigators (doc 11 §7): Fluid (valve terminals, fluid devices, media), the tube table and the medium
//! table, and the session's media and line ends they read.
//!
//! Not here: a medium's highlight (`highlight.rs`); the fluid rows of Properties (`output/properties`).

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_mltext::{NaturalKey, natural_key};
use ew_derive::ew_schema::Device;

use super::{
    Builder, NavRole, base_facets, cell_of, connection_drawable, device_drawables, folios, group, row_entity,
    row_segment, sorted,
};
use crate::Session;
use ew_derive::short_tag;

/// The medium a zone's key names.
fn medium_of(key: &ew_derive::MediumKey) -> Id<ew_derive::ew_schema::Medium> {
    use ew_derive::MediumKey;
    match key {
        MediumKey::Defined(m) | MediumKey::Regulated(m, _) | MediumKey::Vacuum(m) | MediumKey::Exhaust(m) => {
            *m
        }
    }
}

/// The fluid lines of the nets that carry `medium` (its zones, doc 11 §5.3), sorted.
pub(super) fn medium_lines(q: &Q, medium: Id<ew_derive::ew_schema::Medium>) -> Vec<Uuid> {
    let zones = q.get::<ew_derive::MediumZones>(());
    let mut out: Vec<Uuid> = q
        .get::<ew_derive::FluidNets>(())
        .nets
        .iter()
        .filter(|n| {
            zones
                .media
                .get(&n.id)
                .is_some_and(|keys| keys.iter().any(|(k, _)| medium_of(k) == medium))
        })
        .flat_map(|n| n.connections.iter().map(|c| c.as_uuid()))
        .collect();
    out.sort();
    out
}

impl Session {
    /// The project's media by name (natural order): their IDs and names.
    pub fn media(&self) -> Vec<(Uuid, String)> {
        let q = self.query();
        let mut out: Vec<(NaturalKey, Uuid, String)> = q
            .table::<ew_derive::ew_schema::Medium>()
            .iter()
            .map(|(id, m)| {
                let name = m.name.neutral_or_first().unwrap_or_default().to_owned();
                (natural_key(&name), id.as_uuid(), name)
            })
            .collect();
        out.sort();
        out.into_iter().map(|(_, id, name)| (id, name)).collect()
    }

    /// The one medium the net of fluid line `line` carries (doc 11 §5.3); `None` for no medium or a mixed net.
    pub fn line_medium(&self, line: Uuid) -> Option<Uuid> {
        let q = self.query();
        let id: Id<ew_derive::ew_schema::Connection> = Id::from_uuid(line);
        let nets = q.get::<ew_derive::FluidNets>(());
        let net = nets.nets.iter().find(|n| n.connections.contains(&id))?;
        let zones = q.get::<ew_derive::MediumZones>(());
        let media: std::collections::BTreeSet<_> = zones
            .media
            .get(&net.id)?
            .iter()
            .map(|(k, _)| medium_of(k))
            .collect();
        match media.iter().collect::<Vec<_>>().as_slice() {
            [m] => Some(m.as_uuid()),
            _ => None,
        }
    }

    /// The connection point at an end of fluid line `line` (its end a, else its end b): the function and the CP index
    /// `fluid.medium.define` takes; `None` for a line between junctions only.
    pub fn line_end_cp(&self, line: Uuid) -> Option<(Uuid, u16)> {
        use ew_derive::ew_schema::Endpoint;
        let q = self.query();
        let c = q.connections().get(Id::from_uuid(line))?;
        [&c.a, &c.b].into_iter().find_map(|e| match e {
            Endpoint::Cp(f, idx) => Some((f.as_uuid(), *idx)),
            _ => None,
        })
    }
}

/// Fluid (FR-UI-006, doc 11 §7): **Valve terminals** → each valve terminal → its modules in position order → their
/// valves, **Fluid devices** (every other device at a fluid line's end), each device with its lines (the tube list's
/// rows that end at it: `<tag> → <other end>`), then **Media**, each medium with its zones' line count. A line row's
/// records are its connections, so selecting it selects the line on the page; a medium row highlights its zones. In
/// table mode the navigator lists its tubes: a tube's first row carries the tube list's cells in `lang`.
pub(super) fn fluid_table(q: &Q, b: &mut Builder, lang: Option<&str>) {
    use ew_derive::TubeRows;
    use ew_derive::ew_schema::Medium;
    let folios = folios(q);
    let tubes = q.get::<TubeRows>(());
    // The tube list's cells by the row's record (the tube), each given to one row only.
    let mut cells: BTreeMap<Uuid, Vec<String>> = BTreeMap::new();
    if let Some(set) = ew_reports::project_rows(q, "tube_list", None) {
        for (i, r) in set.rows.iter().enumerate() {
            if let (ew_reports::RowKind::Data, Some(record)) = (&r.kind, row_entity(&r.id, 0)) {
                cells.insert(record, texts_in(q, &set, i, lang));
            }
        }
    }
    let cells = std::cell::RefCell::new(cells);
    // The lines at each device.
    let mut lines: BTreeMap<Id<Device>, Vec<usize>> = BTreeMap::new();
    for (i, t) in tubes.iter().enumerate() {
        for d in [t.from.device, t.to.device].into_iter().flatten() {
            let v = lines.entry(d).or_default();
            if !v.contains(&i) {
                v.push(i);
            }
        }
    }
    let line_rows = |b: &mut Builder, parent: u32, device: Id<Device>| {
        for i in lines.get(&device).into_iter().flatten() {
            let t = &tubes[*i];
            let other = if t.from.device == Some(device) {
                &t.to
            } else {
                &t.from
            };
            let records: Vec<Uuid> = t.constituents.iter().map(|c| c.as_uuid()).collect();
            let drawables = sorted(
                t.constituents
                    .iter()
                    .filter_map(|c| connection_drawable(q, &folios, *c))
                    .collect(),
            );
            let facets = base_facets(NavRole::Connection, &drawables);
            let row_cells = {
                let mut cells = cells.borrow_mut();
                cells
                    .remove(&t.id.as_uuid())
                    .or_else(|| records.iter().find_map(|r| cells.remove(r)))
            };
            let n = b.push(
                Some(parent),
                NavRole::Connection,
                &format!("line:{}", t.id.as_uuid()),
                format!("{} → {}", t.tag, other.text).trim().to_owned(),
                Vec::new(),
                records,
                drawables,
                row_cells,
                facets,
            );
            b.nodes[n as usize].kinds = vec!["connection".to_owned()];
        }
    };
    let device_row = |b: &mut Builder, parent: Option<u32>, d: Id<Device>| -> u32 {
        let drawables = device_drawables(q, &folios, d);
        let facets = base_facets(NavRole::Device, &drawables);
        b.push(
            parent,
            NavRole::Device,
            &format!("device:{}", d.as_uuid()),
            short_tag(q, d),
            Vec::new(),
            vec![d.as_uuid()],
            drawables,
            None,
            facets,
        )
    };
    let ranked = |mut v: Vec<Id<Device>>| {
        v.sort_by_cached_key(|d| (natural_key(&short_tag(q, *d)), *d));
        v
    };
    let terminals = ranked(
        q.devices()
            .iter()
            .map(|(id, _)| id)
            .filter(|id| ew_derive::is_fluid_terminal(q, *id))
            .collect(),
    );
    let mut listed: std::collections::BTreeSet<Id<Device>> = std::collections::BTreeSet::new();
    if !terminals.is_empty() {
        let g = group(
            b,
            None,
            NavRole::Section,
            "fluid:terminals",
            "Valve terminals".to_owned(),
        );
        for vt in terminals {
            listed.insert(vt);
            let vnode = device_row(b, Some(g), vt);
            line_rows(b, vnode, vt);
            let mut modules: Vec<(Option<u32>, Id<Device>)> = q
                .idx()
                .device_children(vt)
                .map(|c| (q.devices().get(c).and_then(|d| d.position), c))
                .collect();
            modules.sort();
            for (_, m) in modules {
                listed.insert(m);
                let mnode = device_row(b, Some(vnode), m);
                line_rows(b, mnode, m);
                let mut valves: Vec<(Option<u32>, Id<Device>)> = q
                    .idx()
                    .device_children(m)
                    .map(|c| (q.devices().get(c).and_then(|d| d.position), c))
                    .collect();
                valves.sort();
                for (_, v) in valves {
                    listed.insert(v);
                    let n = device_row(b, Some(mnode), v);
                    line_rows(b, n, v);
                }
            }
        }
    }
    let others = ranked(lines.keys().copied().filter(|d| !listed.contains(d)).collect());
    if !others.is_empty() {
        let g = group(
            b,
            None,
            NavRole::Section,
            "fluid:devices",
            "Fluid devices".to_owned(),
        );
        for d in others {
            let n = device_row(b, Some(g), d);
            line_rows(b, n, d);
        }
    }
    let media = q.table::<Medium>();
    let mut ranked_m: Vec<(NaturalKey, Id<Medium>, String)> = media
        .iter()
        .map(|(id, m)| {
            let name = m.name.neutral_or_first().unwrap_or_default().to_owned();
            (natural_key(&name), id, name)
        })
        .collect();
    ranked_m.sort();
    if !ranked_m.is_empty() {
        let g = group(b, None, NavRole::Section, "fluid:media", "Media".to_owned());
        // A medium's badge counts its consumers, as the medium table's Count does (FT-1761: the badge counted its
        // lines, "1 lines", beside a Count of 2).
        let rows = q.get::<ew_derive::MediumRows>(());
        for (_, id, name) in ranked_m {
            let count = rows
                .iter()
                .find(|r| r.medium == id)
                .map_or(0, |r| r.consumers.len());
            let n = b.push(
                Some(g),
                NavRole::Medium,
                &format!("medium:{}", id.as_uuid()),
                name,
                vec![format!("{count} consumers")],
                vec![id.as_uuid()],
                Vec::new(),
                None,
                vec![("kind".to_owned(), NavRole::Medium.word().to_owned())],
            );
            b.nodes[n as usize].kinds = vec!["medium".to_owned()];
        }
    }
}

/// The printed cells of row `i` of `set` in language `lang` (`None`: neutral, as the CSV prints them).
fn texts_in(q: &Q, set: &ew_reports::RowSet, i: usize, lang: Option<&str>) -> Vec<String> {
    let opts = ew_reports::render::RenderOptions {
        lang: lang.and_then(|l| l.parse().ok()),
        ..Default::default()
    };
    set.rows[i]
        .cells
        .iter()
        .map(|c| ew_reports::render::render(q, c, &opts))
        .collect()
}

/// The tube table (doc 11 §7, FR-FLU-011): the tube list's rows (`tube_list`), one per tube, its effective tube spec
/// and length with their sources; the tube part and the length are edited in their cells (`fields.set` `tube_part`,
/// `tube_length` on the line).
pub(super) fn tubes_table(q: &Q, b: &mut Builder, lang: Option<&str>) {
    let Some(set) = ew_reports::project_rows(q, "tube_list", None) else {
        return;
    };
    let folios = folios(q);
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        let Some(record) = row_entity(&r.id, 0) else {
            continue;
        };
        let texts = texts_in(q, &set, i, lang);
        let label = cell_of(&set, &texts, "tag").to_owned();
        let drawables = sorted(
            connection_drawable(q, &folios, Id::from_uuid(record))
                .into_iter()
                .collect(),
        );
        let facets = base_facets(NavRole::Connection, &drawables);
        let n = b.push(
            None,
            NavRole::Connection,
            &format!("tube:{}", row_segment(&r.id)),
            label,
            Vec::new(),
            vec![record],
            drawables,
            Some(texts),
            facets,
        );
        b.nodes[n as usize].kinds = vec!["connection".to_owned()];
    }
}

/// The medium table (doc 11 §7): the medium overview's rows (`medium_overview`), one per medium; a row highlights the
/// medium's zones on the page (pattern and colour, doc 11 §7's highlight).
pub(super) fn media_table(q: &Q, b: &mut Builder, lang: Option<&str>) {
    let Some(set) = ew_reports::project_rows(q, "medium_overview", None) else {
        return;
    };
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        let Some(record) = row_entity(&r.id, 0) else {
            continue;
        };
        let texts = texts_in(q, &set, i, lang);
        let label = cell_of(&set, &texts, "medium").to_owned();
        let n = b.push(
            None,
            NavRole::Medium,
            &format!("medium:{}", row_segment(&r.id)),
            label,
            Vec::new(),
            vec![record],
            Vec::new(),
            Some(texts),
            vec![("kind".to_owned(), NavRole::Medium.word().to_owned())],
        );
        b.nodes[n as usize].kinds = vec!["medium".to_owned()];
    }
}
