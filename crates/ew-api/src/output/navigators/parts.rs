// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Parts in use navigator (doc 16 §5.2): manufacturer, part, the devices it counts (the parts summary).
//!
//! Not here: the other providers (their own files beside this one).

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_ids::Id;
use ew_schema::Device;

use super::{Builder, NavRole, base_facets, cell_of, device_drawables, folios, group, row_segment};
use ew_derive::short_tag;

/// Parts in use: manufacturer → part (the parts summary's rows in component mode) → the devices it counts.
pub(super) fn parts_table(q: &Q, b: &mut Builder, lang: Option<&str>) {
    let Some(set) = ew_reports::project_rows(q, &ew_schema::ReportKind::PartsSummary, None) else {
        return;
    };
    let folios = folios(q);
    let mut makers: BTreeMap<String, u32> = BTreeMap::new();
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        let texts = ew_reports::csv::row_texts(q, &set, i);
        let maker = cell_of(&set, &texts, "manufacturer").to_owned();
        let maker = if maker.is_empty() {
            super::words_in(&text!("nav-none"), lang)
        } else {
            maker
        };
        let mnode = match makers.get(&maker) {
            Some(n) => *n,
            None => {
                let n = group(
                    b,
                    None,
                    NavRole::Manufacturer,
                    &format!("manufacturer:{maker}"),
                    maker.clone(),
                );
                makers.insert(maker, n);
                n
            }
        };
        let devices: Vec<Id<Device>> = r.sources.iter().map(|s| Id::from_uuid(s.as_uuid())).collect();
        let number = cell_of(&set, &texts, "part_number").to_owned();
        let mut facets = vec![("kind".to_owned(), "part".to_owned())];
        if !number.is_empty() {
            facets.push(("part".to_owned(), number.to_lowercase()));
        }
        let pnode = b.push(
            Some(mnode),
            NavRole::Part,
            &format!("part:{}", row_segment(&r.id)),
            number.clone(),
            Vec::new(),
            devices.iter().map(|d| d.as_uuid()).collect(),
            Vec::new(),
            Some(texts),
            facets,
        );
        for d in devices {
            let drawables = device_drawables(q, &folios, d);
            let mut facets = base_facets(NavRole::Device, &drawables);
            if !number.is_empty() {
                facets.push(("part".to_owned(), number.to_lowercase()));
            }
            b.push(
                Some(pnode),
                NavRole::Device,
                &format!("device:{}", d.as_uuid()),
                short_tag(q, d),
                Vec::new(),
                vec![d.as_uuid()],
                drawables,
                None,
                facets,
            );
        }
    }
}
