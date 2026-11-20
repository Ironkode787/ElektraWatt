// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Interruption points navigator (§7.1): each name with its points, from the interruption-point pairing.
//!
//! Not here: the other providers (their own files beside this one).

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{NaturalKey, natural_key};
use ew_derive::ew_schema::Representation;

use super::{Builder, NavRole, base_facets, drawable, folios, group, sorted};

/// Interruption points (§7.1): name → point rows, from the interruption-point pairing.
pub(super) fn ips_table(q: &Q, b: &mut Builder) {
    use ew_derive::ew_schema::IpDirection;
    use ew_derive::{IpPairingQ, RepAnchorCell};
    let pairing = q.get::<IpPairingQ>(());
    let folios = folios(q);
    let page_cell = |rep: Option<Id<Representation>>| -> String {
        let Some(r) = rep.and_then(|r| q.reps().get(r)) else {
            return String::new();
        };
        let folio = folios.get(&r.page).map(|x| x.1.clone()).unwrap_or_default();
        match q.get::<RepAnchorCell>(r.id).as_ref() {
            Some(c) => format!("{folio}-{c}"),
            None => folio,
        }
    };
    let mut names: BTreeMap<String, u32> = BTreeMap::new();
    let mut points: Vec<(NaturalKey, &ew_derive::IpInfo)> =
        pairing.ips.values().map(|i| (natural_key(&i.name), i)).collect();
    points.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.function.cmp(&b.1.function)));
    for (_, info) in points {
        let gnode = match names.get(&info.name) {
            Some(n) => *n,
            None => {
                let n = group(
                    b,
                    None,
                    NavRole::Name,
                    &format!("name:{}", info.name),
                    info.name.clone(),
                );
                names.insert(info.name.clone(), n);
                n
            }
        };
        let partner = if info.external {
            "external".to_owned()
        } else {
            pairing
                .partners(info.function)
                .iter()
                .map(|p| page_cell(pairing.ips.get(p).and_then(|x| x.rep)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let direction = match info.direction {
            IpDirection::Source => "source",
            IpDirection::Target => "target",
            IpDirection::Undirected => "undirected",
            IpDirection::Unknown(_) => "unknown",
        };
        let pc = page_cell(info.rep);
        let cells = vec![
            info.name.clone(),
            pc.clone(),
            direction.to_owned(),
            partner,
            if info.external { "yes" } else { "no" }.to_owned(),
        ];
        let drawables = sorted(
            info.rep
                .and_then(|r| drawable(q, &folios, r, true))
                .into_iter()
                .collect(),
        );
        let facets = base_facets(NavRole::Ip, &drawables);
        let badges = if info.external {
            vec!["external".to_owned()]
        } else {
            Vec::new()
        };
        b.push(
            Some(gnode),
            NavRole::Ip,
            &format!("ip:{}", info.function.as_uuid()),
            if pc.is_empty() { info.name.clone() } else { pc },
            badges,
            vec![info.function.as_uuid()],
            drawables,
            Some(cells),
            facets,
        );
    }
}
