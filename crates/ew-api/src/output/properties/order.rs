// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The order of records in shell output: a record's printed label and its place (page, then y, then x).
//!
//! Not here: the property sheet (`properties.rs`); the selection's description (`selection.rs`).

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{PageOrder, PageRef, Q};
use ew_schema::{Connection, Device, Function, Kind, Page, Representation};

use super::device_tag_text;
use super::header::record_title;
use super::selection::wire_number_text;

/// A record's place: the position of its page in the page order, then y, then x (page nm, y down); `None` for a
/// record drawn nowhere.
type Place = Option<(usize, i64, i64)>;

/// The place of a drawn point.
fn place_at(q: &Q, page: Id<Page>, at: ew_derive::ew_units::Pt2) -> Place {
    let pos = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .position(|e| e.page == PageRef::Drawn(page))?;
    Some((pos, at.y.0, at.x.0))
}

/// The earliest place of a set of representations.
fn place_of_reps(q: &Q, reps: impl Iterator<Item = Id<Representation>>) -> Place {
    reps.filter_map(|r| q.reps().get(r).and_then(|rep| place_at(q, rep.page, rep.pos)))
        .min()
}

/// The printed label of a function: the label of its first connection point (`-102K1:A1`), else its device's tag.
pub(super) fn function_label(q: &Q, f: Id<Function>) -> String {
    let first = q.get::<ew_derive::FunctionCps>(f).iter().map(|c| c.idx).min();
    match first {
        Some(idx) => ew_derive::cp_label(q, ew_derive::CpRef { function: f, idx }),
        None => q
            .functions()
            .get(f)
            .and_then(|x| x.device)
            .map(|d| device_tag_text(q, d))
            .unwrap_or_default(),
    }
}

/// The order key of a record of `kind` in shell output: its printed label and its place (see [`order_records`]).
pub(crate) fn order_key(q: &Q, kind: Kind, id: Uuid, source: Option<&str>) -> (String, Place) {
    match kind {
        Kind::Device => {
            let d: Id<Device> = Id::from_uuid(id);
            let functions: Vec<Id<Function>> = q.idx().functions_of_device(d).collect();
            let reps: Vec<Id<Representation>> = functions
                .into_iter()
                .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
                .collect();
            let place = place_of_reps(q, reps.into_iter());
            (device_tag_text(q, d), place)
        }
        Kind::Function => {
            let f: Id<Function> = Id::from_uuid(id);
            (
                function_label(q, f),
                place_of_reps(q, q.idx().reps_of_function(f)),
            )
        }
        Kind::Representation => {
            let r: Id<Representation> = Id::from_uuid(id);
            let rep = q.reps().get(r);
            let label = rep
                .as_ref()
                .and_then(|x| x.functions.first().copied())
                .map(|f| function_label(q, f))
                .unwrap_or_default();
            (label, rep.and_then(|x| place_at(q, x.page, x.pos)))
        }
        Kind::Junction => {
            let place = q
                .junctions()
                .get(Id::from_uuid(id))
                .and_then(|j| place_at(q, j.page, j.at));
            (String::new(), place)
        }
        Kind::Connection => {
            let c: Id<Connection> = Id::from_uuid(id);
            let place = q.connections().get(c).and_then(|conn| {
                let page = conn.page?;
                let end = |e: &ew_schema::Endpoint| match e {
                    ew_schema::Endpoint::Cp(f, idx) => ew_derive::cp_point(q, *f, *idx, page),
                    ew_schema::Endpoint::Junction(j) => q.junctions().get(*j).map(|j| j.at),
                    ew_schema::Endpoint::Unknown(_) => None,
                };
                let at = conn.route.first().copied().or_else(|| end(&conn.a))?;
                place_at(q, page, at)
            });
            (wire_number_text(q, c), place)
        }
        Kind::Page => {
            let place = place_at(q, Id::from_uuid(id), ew_derive::ew_units::Pt2::ORIGIN);
            (record_title(q, id, source).unwrap_or_default(), place)
        }
        _ => (record_title(q, id, source).unwrap_or_default(), None),
    }
}

/// `ids` (records of `kind`) in the order shell output lists records: by the printed
/// label in natural order (a device's tag, a function's or representation's first connection point, a connection's
/// wire number, a page's folio and title, a node's path, else a name), then by place (page order, then y, then x; a
/// record drawn nowhere after the drawn ones), and by ID only as the last key. Never by ID alone: the IDs one
/// transaction mints are not in creation order (UUIDv7's random bits within one timestamp), so a UUID order can list
/// two new records the other way round in a replay, and the alias `@n` would then name the other record.
pub(crate) fn order_records(q: &Q, kind: Kind, ids: &mut [Uuid], source: Option<&str>) {
    let mut keyed: Vec<((String, Place), Uuid)> = ids
        .iter()
        .map(|id| (order_key(q, kind, *id, source), *id))
        .collect();
    keyed.sort_by(|((la, pa), ia), ((lb, pb), ib)| {
        ew_mltext::natural_cmp(la, lb)
            .then_with(|| la.cmp(lb))
            .then_with(|| pa.is_none().cmp(&pb.is_none()))
            .then_with(|| pa.cmp(pb))
            .then_with(|| ia.cmp(ib))
    });
    for (slot, (_, id)) in ids.iter_mut().zip(keyed) {
        *slot = id;
    }
}
