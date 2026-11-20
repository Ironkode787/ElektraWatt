// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Potentials navigator (§7.1): the potential overview's rows with the wire rows that carry each potential.
//!
//! Not here: the wire rows themselves (`connections.rs`); a potential's highlight (`highlight.rs`).

use ew_derive::Q;
use ew_derive::ew_ids::Id;

use super::connections::{push_wire_row, wire_list_rows};
use super::{Builder, NavOpen, NavRole, cell_of, folio_pages, row_entity, row_segment};

/// Potentials (§7.1): the potential overview's rows, each with the physical-wire rows of the wire list whose potential
/// cell names it (a bond wire's `A ⇄ B` names both).
pub(super) fn potentials_table(q: &Q, b: &mut Builder) {
    let Some(set) = ew_reports::project_rows(q, "potential_overview", None) else {
        return;
    };
    let wires = wire_list_rows(q);
    let pages = folio_pages(q);
    let partition = q.get::<ew_derive::PotentialPartition>(());
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        let Some(record) = row_entity(&r.id, 0) else {
            continue;
        };
        let texts = ew_reports::csv::row_texts(q, &set, i);
        let name = cell_of(&set, &texts, "potential").to_owned();
        let bond =
            r.id.0
                .iter()
                .any(|k| matches!(k, ew_reports::RowKey::Name(n) if n == "bond"));
        let mut facets = vec![("kind".to_owned(), "potential".to_owned())];
        let first = cell_of(&set, &texts, "pages")
            .split([',', '\u{2013}'])
            .map(str::trim)
            .find(|s| !s.is_empty())
            .map(str::to_owned);
        if !bond {
            facets.push(("potential".to_owned(), name.to_lowercase()));
        }
        if let Some(f) = &first {
            facets.push(("page".to_owned(), f.to_lowercase()));
        }
        let segment = format!("potential:{}", row_segment(&r.id));
        let n = b.push(
            None,
            NavRole::Potential,
            &segment,
            name.clone(),
            Vec::new(),
            vec![record],
            Vec::new(),
            Some(texts),
            facets,
        );
        // The overview's row entity: the bond, the potential's definition, or (a potential without one) its CP.
        let kind = if bond {
            "bond"
        } else if q.potential_defs().contains(Id::from_uuid(record)) {
            "potential_def"
        } else {
            "cp"
        };
        b.nodes[n as usize].kinds = vec![kind.to_owned()];
        if kind == "cp" {
            b.nodes[n as usize].define_at = partition
                .potentials()
                .flat_map(|p| p.cps.iter())
                .find(|cp| cp.id().as_uuid() == record)
                .map(|cp| (cp.function.as_uuid(), cp.idx));
        }
        b.nodes[n as usize].open = first.and_then(|f| pages.get(&f)).map(|p| NavOpen {
            page: *p,
            select: Vec::new(),
        });
        if bond {
            continue;
        }
        for w in wires
            .iter()
            .filter(|w| !w.bridge && w.potential.split('\u{21C4}').map(str::trim).any(|p| p == name))
        {
            push_wire_row(b, n, w, false);
        }
    }
}
