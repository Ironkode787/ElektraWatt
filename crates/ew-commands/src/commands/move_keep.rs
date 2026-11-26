// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cross-page move's pairs of interruption points (doc 06 §3.4): `rep.move` with a target page and `keep` keeps
//! each wire whose other end stays behind as a pair of interruption points. [`plan`] says which wires are cut and
//! which junctions go with the moved symbols, before anything is written; [`cut`] makes one pair once the symbols
//! and the junctions have moved.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_model::EndpointKey;
use ew_schema::{
    Connection, Endpoint, Extra, Function, FunctionKind, IpDirection, Junction, NumberSpec, Page, SymbolDef,
    WireOverrides,
};
use ew_units::{Grid, Nm, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CmdError;
use crate::geometry::{end_point, on_grid, split_route};
use crate::proxy::UuidS;
use crate::tx::TxBuilder;
use ew_l10n::Text;

/// The interruption points of a cross-page move (`rep.move`'s `keep`): the two forms of EW's interruption-point
/// symbol and the library documents they need.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeepAsIps {
    /// The source form, drawn on the page left.
    #[schemars(with = "UuidS")]
    pub source: Id<SymbolDef>,
    /// The target form, drawn on the page arrived on.
    #[schemars(with = "UuidS")]
    pub target: Id<SymbolDef>,
    /// The library documents of the two forms and their FnDef (`lib/` JSON form), embedded when the move cuts a
    /// wire; empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

/// A wire the move cuts: the connection, its end that stays on the page left and its end that moves (a moved
/// symbol's connection point, or a junction that goes with them).
pub(super) struct Cut {
    connection: Id<Connection>,
    stay: Endpoint,
    moving: Endpoint,
}

impl Cut {
    /// The wire cut.
    pub fn connection(&self) -> Id<Connection> {
        self.connection
    }
}

/// A wire at a junction and its far end.
type Far = (Id<Connection>, Endpoint);

/// What a cross-page move with `keep` does to the wires that cross: the wires it cuts and the junctions that go to the
/// new page.
#[derive(Default)]
pub(super) struct Plan {
    /// The wires cut, in connection ID order.
    pub cuts: Vec<Cut>,
    /// The junctions that move, each with more of its wires at moved symbols than at symbols that stay.
    pub junctions: BTreeSet<Id<Junction>>,
}

impl Plan {
    /// Whether connection `c` is cut.
    pub fn cuts(&self, c: Id<Connection>) -> bool {
        self.cuts.iter().any(|k| k.connection == c)
    }
}

/// The plan of a move of the symbols drawing the CPs `moved` says (true for a CP a moved symbol draws), from the
/// wires `crossing` (one end at a moved CP, the other not, on the page left):
/// a wire between a moved CP and a CP that stays is cut; a junction stays with the larger part (at least as many of its
/// wires at CPs that stay as at moved ones: it stays, and each wire from it to a moved CP is cut; else it moves, and each
/// wire from it to a CP that stays is cut). Refused in words: a junction joined to another junction.
pub(super) fn plan(
    tx: &TxBuilder<'_>,
    crossing: &[Id<Connection>],
    moved: &dyn Fn(&Endpoint) -> bool,
) -> Result<Plan, CmdError> {
    let mut plan = Plan::default();
    let mut junctions: BTreeSet<Id<Junction>> = BTreeSet::new();
    for &cid in crossing {
        let c = tx.require::<Connection>(cid)?;
        let (stay, moving) = if moved(&c.a) { (&c.b, &c.a) } else { (&c.a, &c.b) };
        match stay {
            Endpoint::Junction(j) => {
                junctions.insert(*j);
            }
            _ => plan.cuts.push(Cut {
                connection: cid,
                stay: stay.clone(),
                moving: moving.clone(),
            }),
        }
    }
    for j in junctions {
        let page = tx.require::<Junction>(j)?.page;
        let wires = tx.connections_at(EndpointKey::Junction(j));
        // The junction's wires to CPs that stay and to moved CPs, each with its far end.
        let (mut stays, mut moves): (Vec<Far>, Vec<Far>) = Default::default();
        for cid in &wires {
            let c = tx.require::<Connection>(*cid)?;
            if c.page != Some(page) {
                continue;
            }
            let far = if c.a == Endpoint::Junction(j) { &c.b } else { &c.a };
            if matches!(far, Endpoint::Junction(_)) {
                return tx.reject(text!(
                    "refusal-rep-move-keep-junctions",
                    crate::words::wire(tx, *cid)
                ));
            }
            if moved(far) {
                moves.push((*cid, far.clone()));
            } else {
                stays.push((*cid, far.clone()));
            }
        }
        if stays.len() >= moves.len() {
            plan.cuts.extend(moves.into_iter().map(|(connection, far)| Cut {
                connection,
                stay: Endpoint::Junction(j),
                moving: far,
            }));
        } else {
            plan.junctions.insert(j);
            plan.cuts.extend(stays.into_iter().map(|(connection, far)| Cut {
                connection,
                stay: far,
                moving: Endpoint::Junction(j),
            }));
        }
    }
    plan.cuts.sort_by_key(|k| k.connection);
    Ok(plan)
}

/// The grid points of `path` from its first vertex toward its last, the last left out: along each horizontal or
/// vertical segment every grid point in order, of any other segment its first vertex alone.
fn path_points(path: &[Pt2], grid: Grid) -> Vec<Pt2> {
    let mut out: Vec<Pt2> = Vec::new();
    for w in path.windows(2) {
        let (p, q) = (w[0], w[1]);
        let along = |fixed: Nm, from: Nm, to: Nm, horizontal: bool| -> Vec<Pt2> {
            if !on_grid(grid, fixed) {
                return Vec::new();
            }
            let (k0, k1) = (grid.snap(from), grid.snap(to));
            let step = if k1 >= k0 { 1 } else { -1 };
            let mut ks = Vec::new();
            let mut k = k0;
            while k != k1 {
                ks.push(k);
                k += step;
            }
            ks.into_iter()
                .filter_map(|k| grid.at(k))
                .map(|v| {
                    if horizontal {
                        Pt2::new(v, fixed)
                    } else {
                        Pt2::new(fixed, v)
                    }
                })
                .collect()
        };
        let points = if p.y == q.y && p.x != q.x {
            along(p.y, p.x, q.x, true)
        } else if p.x == q.x && p.y != q.y {
            along(p.x, p.y, q.y, false)
        } else {
            vec![p]
        };
        for pt in points {
            if out.last() != Some(&pt) {
                out.push(pt);
            }
        }
    }
    out
}

/// The name of a new pair for the wire `c`: its potential's (a fallback name included; `IP` for a wire the potentials
/// do not know), else the first of its series ([`super::next_in_series`]) that no interruption point of the project
/// and none of `taken` has: the rule of a new interruption point (doc 06 §3.5).
fn pair_name(tx: &TxBuilder<'_>, c: &Connection, taken: &BTreeSet<String>) -> Ident {
    let q = ew_derive::Q::new(tx.base_model());
    let base = [&c.a, &c.b]
        .into_iter()
        .find_map(|e| match e {
            Endpoint::Cp(f, i) => ew_derive::potential_id_at(&q, EndpointKey::Cp(*f, *i)),
            _ => None,
        })
        .map(|p| q.get::<ew_derive::PotentialNameOf>(p).name.text.clone())
        .filter(|n| n != "?" && n.parse::<Ident>().is_ok())
        .unwrap_or_else(|| "IP".to_owned());
    let names: BTreeSet<String> = tx
        .iter::<Function>()
        .iter()
        .filter_map(|f| match &f.kind {
            FunctionKind::Ip(spec) => Some(spec.name.to_string()),
            _ => None,
        })
        .chain(taken.iter().cloned())
        .collect();
    let mut name = base;
    for _ in 0..=names.len() {
        if !names.contains(&name) {
            break;
        }
        name = super::next_in_series(&name);
    }
    name.parse::<Ident>()
        .unwrap_or_else(|_| Ident::from_input("IP").expect("IP is an identifier"))
}

/// Keeps the wire of `k` as a pair of interruption points, after the move put the moved symbols and junctions on
/// `to`: the source point on `from` on the free grid point of the wire's old path nearest to its moved end
/// (`old` gives an end's point before the move; `taken` the points of the page left that a pin, a junction or a
/// source point of this move holds), the wire shortened to it; the target point on `to` at the moving end, joined to
/// it by a wire of length zero with the old wire's kind (the wire keeps its values as `own_values` says, doc 13 §2.3).
/// `names` gathers the pairs' names. The notice that
/// names the pair. Refused in words: no free grid point left on the path.
#[allow(clippy::too_many_arguments)]
pub(super) fn cut(
    tx: &mut TxBuilder<'_>,
    k: &Cut,
    keep: &KeepAsIps,
    (from, to): (Id<Page>, Id<Page>),
    old: &dyn Fn(&Endpoint) -> Option<Pt2>,
    taken: &mut BTreeSet<Pt2>,
    names: &mut BTreeSet<String>,
    words: Text,
) -> Result<Text, CmdError> {
    let c = tx.require::<Connection>(k.connection)?;
    let grid = tx.require::<Page>(from)?.grid;
    let (Some(left), Some(stay)) = (old(&k.moving), old(&k.stay)) else {
        return Err(CmdError::internal("rep.move: a cut wire's end has no point"));
    };
    // The path from the moved end to the end that stays (a route runs from end a to end b).
    let moving_is_b = c.b == k.moving;
    let path: Vec<Pt2> = if c.route.is_empty() {
        vec![left, stay]
    } else {
        let mut r = c.route.clone();
        if moving_is_b {
            r.reverse();
        }
        r
    };
    let Some(at) = path_points(&path, grid).into_iter().find(|p| !taken.contains(p)) else {
        return tx.reject(text!("refusal-rep-move-keep-no-place", words));
    };
    taken.insert(at);
    let name = pair_name(tx, &c, names);
    names.insert(name.to_string());
    let (source, idx) =
        super::connection::ip_at(tx, from, keep.source, name.clone(), IpDirection::Source, at)?;
    // The wire on the page left ends at the source point, its route shortened to it.
    let route = if c.route.is_empty() {
        Vec::new()
    } else {
        // The route from the end that stays to the moved end.
        let mut sm = c.route.clone();
        if !moving_is_b {
            sm.reverse();
        }
        let mut kept = split_route(&sm, at).map_or(sm, |(first, _)| first);
        if !moving_is_b {
            kept.reverse();
        }
        kept
    };
    let ip_end = Endpoint::Cp(source, idx);
    tx.update::<Connection>(k.connection, |w| {
        if moving_is_b {
            w.b = ip_end.clone();
        } else {
            w.a = ip_end.clone();
        }
        w.route = route;
    })?;
    // The target point at the moving end on the new page, joined to it by a wire of length zero.
    let Some(arrive) = end_point(tx, &k.moving, to) else {
        return Err(CmdError::internal(
            "rep.move: a cut wire's moving end is not on the new page",
        ));
    };
    let (target, tidx) =
        super::connection::ip_at(tx, to, keep.target, name.clone(), IpDirection::Target, arrive)?;
    let joined: Id<Connection> = tx.mint();
    tx.insert(Connection {
        id: joined,
        v: 1,
        a: Endpoint::Cp(target, tidx),
        b: k.moving.clone(),
        page: Some(to),
        route: Vec::new(),
        kind: c.kind.clone(),
        overrides: WireOverrides::default(),
        number: NumberSpec::default(),
        source_ref: None,
        computed: false,
        extra: Extra::new(),
    })?;
    // A junction that moved keeps its wire order with the new wire in the cut one's place.
    if let Endpoint::Junction(j) = &k.moving {
        tx.update::<Junction>(*j, |j| {
            for s in &mut j.sequence {
                if *s == k.connection {
                    *s = joined;
                }
            }
        })?;
    }
    Ok(text!("notice-rep-move-keep-pair", words, name.to_string()))
}
