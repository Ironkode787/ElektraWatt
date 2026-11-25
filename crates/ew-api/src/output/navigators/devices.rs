// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Devices navigator's tree (doc 16 §5.2): functions, locations, devices, their functions and representations.
//!
//! Not here: the words of a function's kind (`function_kind.rs`); the shared node helpers (`navigators.rs`); the
//! unplaced devices (`unplaced.rs`).

use std::collections::BTreeMap;

use ew_derive::{Q, StructureOf, short_tag};
use ew_ids::{Id, Uuid};
use ew_mltext::{NaturalKey, natural_key};
use ew_rds::Aspect;
use ew_schema::{Device, DeviceState, FnRole, Function, Page, Representation};

use super::function_kind::{fndef_name, function_kind_text};
use super::{
    Builder, Drawable, EARTH_CONNECTION, Keyed, NOT_DRAWN, NavRole, Ranked, drawable, drawn_as_members,
    folios, sorted, state_word,
};

/// One device of the Devices table: the device row with its cells (from the structure overview).
struct DeviceRow {
    cells: Vec<String>,
    part: String,
}

/// A device's row label: its tag; an earth connection, which has none, by its pin's designation (`PE`).
fn row_label(q: &Q, d: Id<Device>) -> String {
    let tag = short_tag(q, d);
    if !tag.is_empty() {
        return tag;
    }
    crate::output::properties::untagged_name(q, d)
}

/// The Devices tree: function nodes, location nodes inside them, the devices that are not children of another
/// listed device in natural tag order; child devices under their parent (position, then natural tag); under each
/// device its functions (main first, FnDef name, ID), under each function its representations. Terminals are not
/// listed (they are the Terminal strips navigator's rows, as in the structure overview).
pub(super) fn devices_table(q: &Q, b: &mut Builder, lang: Option<&str>) {
    let Some(set) = ew_reports::project_rows(q, &ew_schema::ReportKind::StructureOverview, None) else {
        return;
    };
    let part_col = set.column("part");
    let mut rows: BTreeMap<Id<Device>, DeviceRow> = BTreeMap::new();
    for (i, r) in set.rows.iter().enumerate() {
        let Some(&src) = r.sources.first() else { continue };
        let id: Id<Device> = Id::from_uuid(src.as_uuid());
        if !q.devices().contains(id) {
            continue;
        }
        let cells = ew_reports::csv::row_texts(q, &set, i);
        let part = part_col.and_then(|c| cells.get(c).cloned()).unwrap_or_default();
        rows.insert(id, DeviceRow { cells, part });
    }
    let folios = folios(q);
    let tag = |d: Id<Device>| row_label(q, d);
    let aspect_text = |d: Id<Device>, a: Aspect| q.get::<StructureOf>(d).designation(|x| x == a).to_string();
    // The top-level devices by (function, location), in natural tag order.
    let top: Vec<Id<Device>> = rows
        .keys()
        .copied()
        .filter(|d| {
            q.devices()
                .get(*d)
                .and_then(|x| x.parent)
                .is_none_or(|p| !rows.contains_key(&p))
        })
        .collect();
    let mut groups: BTreeMap<(String, String), Vec<Ranked>> = BTreeMap::new();
    for d in top {
        let key = (aspect_text(d, Aspect::Function), aspect_text(d, Aspect::Location));
        groups.entry(key).or_default().push((natural_key(&tag(d)), d));
    }
    let mut group_list: Vec<((String, String), Vec<Ranked>)> = groups.into_iter().collect();
    group_list.sort_by(|a, b| {
        let k = |s: &str| (s.is_empty(), natural_key(s));
        k(&a.0.0).cmp(&k(&b.0.0)).then_with(|| k(&a.0.1).cmp(&k(&b.0.1)))
    });
    let mut fnodes: BTreeMap<String, u32> = BTreeMap::new();
    let none = super::words_in(&text!("nav-none"), lang);
    for ((fun, loc), mut devices) in group_list {
        let flabel = if fun.is_empty() { none.clone() } else { fun.clone() };
        let fseg = format!("function:{}", if fun.is_empty() { "-" } else { &fun });
        let fnode = match fnodes.get(&fseg) {
            Some(n) => *n,
            None => {
                let n = b.push(
                    None,
                    NavRole::Function,
                    &fseg,
                    flabel,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    vec![("kind".to_owned(), "function".to_owned())],
                );
                fnodes.insert(fseg.clone(), n);
                n
            }
        };
        let llabel = if loc.is_empty() { none.clone() } else { loc.clone() };
        let lseg = format!("location:{}", if loc.is_empty() { "-" } else { &loc });
        let lnode = b.push(
            Some(fnode),
            NavRole::Location,
            &lseg,
            llabel,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            vec![("kind".to_owned(), "location".to_owned())],
        );
        devices.sort();
        for (_, d) in devices {
            device_node(q, b, &rows, &folios, &tag, lnode, d);
        }
    }
}

/// A device node with its child devices, functions and representations.
fn device_node(
    q: &Q,
    b: &mut Builder,
    rows: &BTreeMap<Id<Device>, DeviceRow>,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    tag: &dyn Fn(Id<Device>) -> String,
    parent: u32,
    d: Id<Device>,
) {
    let Some(row) = rows.get(&d) else { return };
    let Some(dev) = q.devices().get(d) else { return };
    // The functions: main first, then FnDef name, then ID.
    let mut fns: Vec<(bool, String, Id<Function>)> = q
        .idx()
        .functions_of_device(d)
        .filter_map(|f| {
            q.functions()
                .get(f)
                .map(|x| (x.role != FnRole::Main, fndef_name(q, x), f))
        })
        .collect();
    fns.sort();
    let fn_drawables: Vec<(Id<Function>, bool, Vec<Drawable>)> = fns
        .iter()
        .map(|(aux, _, f)| {
            // An internal coil is drawn in its valve's symbol
            let drawn = ew_derive::coil_host(q, *f).unwrap_or(*f);
            let reps = q
                .idx()
                .reps_of_function(drawn)
                .filter_map(|r| drawable(q, folios, r, !aux))
                .collect();
            (*f, !aux, sorted(reps))
        })
        .collect();
    let all: Vec<Keyed> = fn_drawables
        .iter()
        .flat_map(|(_, _, ds)| ds.iter())
        .filter_map(|x| {
            let r: Id<Representation> = Id::from_uuid(x.record);
            drawable(q, folios, r, x.main)
        })
        .collect();
    let mut drawables = sorted(all);
    // A cable is drawn where its cores run, a strip where its terminals stand (both read "not drawn").
    if drawables.is_empty() {
        drawables = drawn_as_members(q, folios, dev);
    }
    let label = tag(d);
    let mut badges = Vec::new();
    let mut facets = vec![("kind".to_owned(), "device".to_owned())];
    if drawables.is_empty() {
        badges.push(NOT_DRAWN.to_owned());
        facets.push(("unplaced".to_owned(), String::new()));
    }
    if dev.tag.frozen.is_some() {
        badges.push("frozen".to_owned());
        facets.push(("frozen".to_owned(), String::new()));
    }
    if dev.state == DeviceState::Spare {
        badges.push("spare".to_owned());
    }
    // An earth or ground symbol's device says what it is: a connection to earth, not a device to buy.
    if ew_derive::layout::is_earth_connection(q, dev) {
        badges.push(EARTH_CONNECTION.to_owned());
    }
    facets.push(("state".to_owned(), state_word(&dev.state).to_owned()));
    if !row.part.is_empty() {
        facets.push(("part".to_owned(), row.part.to_lowercase()));
    }
    let mut pages: Vec<String> = drawables.iter().map(|x| x.folio.to_lowercase()).collect();
    pages.sort();
    pages.dedup();
    facets.extend(pages.into_iter().map(|p| ("page".to_owned(), p)));
    let node = b.push(
        Some(parent),
        NavRole::Device,
        &format!("device:{}", d.as_uuid()),
        label,
        badges,
        vec![d.as_uuid()],
        drawables,
        Some(row.cells.clone()),
        facets,
    );

    // Child devices (by `parent`), in position order, then natural tag.
    let mut children: Vec<(Option<u32>, NaturalKey, Id<Device>)> = rows
        .keys()
        .copied()
        .filter(|c| q.devices().get(*c).and_then(|x| x.parent) == Some(d))
        .map(|c| {
            let pos = q.devices().get(c).and_then(|x| x.position);
            (pos, natural_key(&tag(c)), c)
        })
        .collect();
    children.sort_by(|a, b| (a.0.is_none(), a.0, &a.1, a.2).cmp(&(b.0.is_none(), b.0, &b.1, b.2)));
    for (_, _, c) in children {
        device_node(q, b, rows, folios, tag, node, c);
    }
    for ((_, name, f), (_, main, ds)) in fns.iter().zip(fn_drawables) {
        let mut badges = vec![if main { "main" } else { "aux" }.to_owned()];
        let mut facets = vec![("kind".to_owned(), "function".to_owned())];
        if ds.is_empty() {
            badges.push("unplaced".to_owned());
            facets.push(("unplaced".to_owned(), String::new()));
        }
        let mut pages: Vec<String> = ds.iter().map(|x| x.folio.to_lowercase()).collect();
        pages.sort();
        pages.dedup();
        facets.extend(pages.into_iter().map(|p| ("page".to_owned(), p)));
        // The function's kind in words (`Coil A1-A2`, `NO contact 13-14`), its FnDef key
        // only for a FnDef without a category (doc 16a P2); the order stays the FnDef key's.
        let label = q
            .functions()
            .get(*f)
            .map_or_else(|| name.clone(), |x| function_kind_text(q, x));
        let fnode = b.push(
            Some(node),
            NavRole::Function,
            &format!("function:{}", f.as_uuid()),
            label,
            badges,
            vec![f.as_uuid()],
            ds.clone(),
            None,
            facets,
        );
        for x in ds {
            let facets = vec![
                ("kind".to_owned(), "representation".to_owned()),
                ("page".to_owned(), x.folio.to_lowercase()),
            ];
            let badges = vec![if x.main { "main" } else { "aux" }.to_owned()];
            b.push(
                Some(fnode),
                NavRole::Representation,
                &format!("representation:{}", x.record),
                x.place(),
                badges,
                vec![x.record],
                vec![x.clone()],
                None,
                facets,
            );
        }
    }
}
