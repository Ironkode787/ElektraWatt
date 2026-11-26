// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire's own values (doc 13 §2.3): what a person set for one wire, read the way every list and check reads it.
//!
//! A wire's level-7 values are [`PhysicalWire::overrides`] (its own connections first, then the carrier, then the
//! others); its ends set by hand are [`hand_ends`], on the wire's own connections only. [`wire_values`] gives both as one
//! value, with every member by its name in the file ([`members`]), so that a member a newer format added follows the
//! wire like the ones this build knows. The normaliser that keeps the values with their wires
//! (`ew-commands`' `wire_values`) and the command fuzz's oracle read them here.

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_schema::{Connection, EndOverride, WireOverrides};
use serde_json::Value;

use crate::q::Q;
use crate::treatment::end_through;
use crate::wires::{EndKind, PhysicalWire};

/// An end of a wire on a device, where it is set by hand (doc 13 §2.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HandEnd {
    /// The slot: the own connection that holds the end, else the wire's first own connection, and whether in its
    /// `end_a` (the end reached through its endpoint `a`) or `end_b`. An edit, Reset and a quick fix write here.
    pub at: (Id<Connection>, bool),
    /// What the slot holds; `None`: the rule's.
    pub value: Option<EndOverride>,
}

/// The ends of `wire` set by hand, in the order of `wire.ends`: for each end on a device, the first slot of the wire's
/// own connections (in [`PhysicalWire::own`] order) that holds it, else the slot on the first own connection that an
/// edit writes. `None` for an end made outside the project and for every end of a wire without a connection of its
/// own. A slot on a connection several wires share is never read.
pub fn hand_ends(q: &Q, wire: &PhysicalWire) -> [Option<HandEnd>; 2] {
    let conns = q.connections();
    let own: Vec<(usize, Id<Connection>)> = wire
        .own
        .iter()
        .filter_map(|c| wire.constituents.iter().position(|x| x == c).map(|k| (k, *c)))
        .collect();
    let slot = |i: usize| -> Option<HandEnd> {
        if wire.ends[i].kind != EndKind::Device {
            return None;
        }
        let stored = own.iter().find_map(|(k, c)| {
            let o = &conns.get(*c)?.overrides;
            [(true, &o.end_a), (false, &o.end_b)]
                .into_iter()
                .find(|(a, end)| end.is_some() && end_through(q, wire, *k, *a) == Some(i))
                .map(|(a, end)| HandEnd {
                    at: (*c, a),
                    value: end.clone(),
                })
        });
        stored.or_else(|| {
            own.first().and_then(|(k, c)| {
                [true, false]
                    .into_iter()
                    .find(|a| end_through(q, wire, *k, *a) == Some(i))
                    .map(|a| HandEnd {
                        at: (*c, a),
                        value: None,
                    })
            })
        })
    };
    [slot(0), slot(1)]
}

/// What a person set for one wire.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WireValues {
    /// Every level-7 member but the ends, by its name in the file ([`members`]).
    pub members: BTreeMap<String, Value>,
    /// The ends set by hand, in the order of the wire's ends.
    pub ends: [Option<EndOverride>; 2],
}

/// The values of `wire` as it reads them: [`PhysicalWire::overrides`] and [`hand_ends`].
pub fn wire_values(q: &Q, wire: &PhysicalWire) -> WireValues {
    WireValues {
        members: members(&wire.overrides),
        ends: hand_ends(q, wire).map(|h| h.and_then(|h| h.value)),
    }
}

/// The members of `o` that are set, by their names in the file, the ends left out: `csa`, `colour`, `length`,
/// `show_marker`, each flag as `flags.shielded`, `flags.twisted`, an unknown member by its key (doc 05: a newer
/// format's member is kept and written back).
pub fn members(o: &WireOverrides) -> BTreeMap<String, Value> {
    let Ok(Value::Object(map)) = serde_json::to_value(o) else {
        return BTreeMap::new();
    };
    let mut out = BTreeMap::new();
    for (k, v) in map {
        match (k.as_str(), v) {
            ("end_a" | "end_b", _) => {}
            ("flags", Value::Object(flags)) => {
                out.extend(flags.into_iter().map(|(f, v)| (format!("flags.{f}"), v)));
            }
            (_, v) => {
                out.insert(k, v);
            }
        }
    }
    out
}

/// `o` with exactly `members` set ([`members`]' names), its ends kept.
pub fn with_members(
    o: &WireOverrides,
    members: &BTreeMap<String, Value>,
) -> Result<WireOverrides, serde_json::Error> {
    let mut map = serde_json::Map::new();
    let mut flags = serde_json::Map::new();
    for (k, v) in members {
        match k.strip_prefix("flags.") {
            Some(f) => {
                flags.insert(f.to_owned(), v.clone());
            }
            None => {
                map.insert(k.clone(), v.clone());
            }
        }
    }
    if !flags.is_empty() {
        map.insert("flags".to_owned(), Value::Object(flags));
    }
    let mut out: WireOverrides = serde_json::from_value(Value::Object(map))?;
    out.end_a.clone_from(&o.end_a);
    out.end_b.clone_from(&o.end_b);
    Ok(out)
}
