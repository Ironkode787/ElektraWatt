// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A connection's derived rows: the seven wire properties with their sources, the wire's length, its two ends, the
//! wire number, the potential (also an interruption point's) and the route in frame cells.
//!
//! Not here: a bus line's rows (`bus_rows.rs`); a fluid line's rows (`fluid_rows.rs`).

use std::collections::BTreeSet;

use ew_derive::{
    EndTreatment, NameSource, PotentialNameOf, PotentialPartition, PropSource, Q, TreatedBy, WireLength,
    WireNumbers, WireRecordSet, cp_label, end_connection,
};
use ew_ids::{Id, Uuid};
use ew_model::EndpointKey;
use ew_schema::{Connection, Function, Kind, PotentialDef};
use serde_json::Value;

use super::DerivedRow;
use super::bus_rows::bus_line_rows;
use super::context::SheetCx;
use super::header::record_title;
use super::sections::{Built, derived, merge, profile_name};
use super::selection::{wire_number_text, wires_of};
use super::{CABLE_SOURCE_ROW, Provenance, RowAction, RowValue, SourceLink, device_tag_text, ml_text};
use crate::Session;
use crate::output::fields::{Category, EditRoute, FieldType};

/// Doc 16a §8.5: the route points of connection `c` as the frame cells they lie in (`D5`), as
/// cross-references print them, never as nanometres; a point outside the frame grid by its two lengths.
pub(super) fn route_in_cells(q: &Q, c: Id<Connection>, built: &mut [Built]) {
    let Some(conn) = q.connections().get(c) else {
        return;
    };
    for b in built.iter_mut() {
        let Some(i) = b
            .row
            .path
            .strip_prefix("route[")
            .and_then(|r| r.strip_suffix(']'))
            .and_then(|i| i.parse::<usize>().ok())
        else {
            continue;
        };
        let (Some(point), RowValue::One { text, .. }) = (conn.route.get(i), &mut b.row.value) else {
            continue;
        };
        let cell = conn
            .page
            .and_then(|page| q.get::<ew_derive::CellOf>((page, *point)).as_ref().clone());
        *text = cell.map_or_else(
            || {
                format!(
                    "{}, {}",
                    ew_derive::mm_padded(point.x.0),
                    ew_derive::mm_padded(point.y.0)
                )
            },
            |cell| cell.to_string(),
        );
    }
}

/// The potential name of a connection's first end, if it has one.
pub(super) fn potential_of(q: &Q, conn: &Connection) -> Option<ew_derive::PotentialId> {
    let partition = q.get::<PotentialPartition>(());
    [&conn.a, &conn.b]
        .into_iter()
        .filter_map(EndpointKey::of)
        .find_map(|at| partition.potential_at(at).map(|p| p.id))
}

/// The Potential row of a `kind` section whose targets end at `ends` (a wire's two ends, an interruption point's
/// connection point): the name of the potential at each target, from its definition or from the connectivity;
/// the definition is where its name, type and voltage are set.
pub(super) fn potential_row(q: &Q, cx: &SheetCx<'_>, kind: &str, ends: &[Vec<EndpointKey>]) -> Built {
    let partition = q.get::<PotentialPartition>(());
    let mut per = Vec::new();
    let mut defs = BTreeSet::new();
    for ends in ends {
        let Some(p) = ends.iter().find_map(|at| partition.potential_at(*at)) else {
            defs.insert(None);
            per.push((
                None,
                Provenance::Derived {
                    via: "connectivity".into(),
                },
            ));
            continue;
        };
        let names = q.get::<PotentialNameOf>(p.id);
        let via = match names.name.source {
            NameSource::Def(def) => {
                defs.insert(Some(def));
                let at = q
                    .potential_defs()
                    .get(def)
                    .and_then(|d| EndpointKey::of(&d.at))
                    .and_then(ew_derive::CpRef::of)
                    .map(|cp| cp_label(q, cp))
                    .unwrap_or_default();
                format!("potential definition at {at}")
            }
            _ => {
                defs.insert(None);
                "connectivity".to_owned()
            }
        };
        per.push((
            Some(Value::String(names.name.text.clone())),
            Provenance::Derived { via },
        ));
    }
    let (value, provenance) = merge(cx, &FieldType::Text, per);
    let mut built = derived(DerivedRow {
        kind,
        name: "potential",
        partner: None,
        category: Category::Electrical,
        ty: FieldType::Text,
        value,
        provenance,
        actions: Vec::new(),
    });
    if let [Some(def)] = defs.into_iter().collect::<Vec<_>>().as_slice() {
        built.row.source = Some(SourceLink::Record {
            id: def.as_uuid(),
            row: "potential_def/name".into(),
        });
    }
    built
}

/// The connection points of the interruption point `f`; none for another function.
pub(super) fn ip_ends(q: &Q, f: Id<Function>) -> Vec<EndpointKey> {
    if !q
        .functions()
        .get(f)
        .is_some_and(|x| matches!(x.kind, ew_schema::FunctionKind::Ip(_)))
    {
        return Vec::new();
    }
    q.get::<ew_derive::FunctionCps>(f)
        .iter()
        .map(|cp| EndpointKey::Cp(f, cp.idx))
        .collect()
}

/// The definitions of the potential at the first of `ends` that lies on one, in ID order.
pub(super) fn potential_defs_at(q: &Q, ends: &[EndpointKey]) -> Vec<Id<PotentialDef>> {
    let partition = q.get::<PotentialPartition>(());
    ends.iter()
        .find_map(|at| partition.potential_at(*at))
        .map(|p| p.defs.clone())
        .unwrap_or_default()
}

impl Session {
    /// A connection's derived rows: the seven wire properties, the length, the wire number and the potential.
    pub(super) fn derived_rows_of_connection(
        q: &Q,
        cx: &SheetCx<'_>,
        targets: &[Uuid],
        jsons: &[Value],
    ) -> Vec<Built> {
        let fields_set = |kind: &str, path: &str| EditRoute::FieldsSet {
            kind: kind.to_owned(),
            path: path.to_owned(),
        };
        let editable = |path: &str| Self::editable(Kind::Connection, path);
        let mut out = Vec::new();
        let conns: Vec<Id<Connection>> = targets.iter().map(|t| Id::from_uuid(*t)).collect();
        let numbers = q.get::<WireNumbers>(());
        // The seven wire properties.
        type Getter = fn(&ew_derive::EffectiveProps) -> Option<(Value, PropSource)>;
        macro_rules! prov {
            ($p:expr) => {
                $p.as_ref().and_then(|p| {
                    serde_json::to_value(&p.value)
                        .ok()
                        .map(|v| (v, p.source.clone()))
                })
            };
        }
        let props: [(&str, &str, FieldType, Getter); 7] = [
            ("csa", "overrides.csa", FieldType::Csa, |e| prov!(e.csa)),
            ("colour", "overrides.colour", FieldType::Colour, |e| {
                prov!(e.colour)
            }),
            (
                "wire_part",
                "overrides.wire_part",
                FieldType::Link { kind: "part".into() },
                |e| prov!(e.wire_part),
            ),
            ("insulation", "overrides.insulation", FieldType::Ident, |e| {
                prov!(e.insulation)
            }),
            ("function", "overrides.function", FieldType::Ident, |e| {
                prov!(e.function)
            }),
            ("shielded", "overrides.flags.shielded", FieldType::Bool, |e| {
                prov!(e.shielded)
            }),
            ("twisted", "overrides.flags.twisted", FieldType::Bool, |e| {
                prov!(e.twisted)
            }),
        ];
        for (name, path, ty, get) in props {
            let mut per = Vec::new();
            let mut can_override = false;
            let mut can_reset = false;
            let mut links: Vec<Option<SourceLink>> = Vec::new();
            for c in &conns {
                let wires = wires_of(q, *c);
                let first = wires.first().and_then(|(_, e)| get(e));
                // Two wires with different values: varies.
                let distinct: BTreeSet<String> = wires
                    .iter()
                    .map(|(_, e)| get(e).map_or_else(|| "null".into(), |(v, _)| v.to_string()))
                    .collect();
                let Some((value, source)) = first else {
                    per.push((None, Provenance::Authored));
                    can_override = true;
                    continue;
                };
                if distinct.len() > 1 {
                    per.push((
                        Some(Value::String(format!("‹{}›", distinct.len()))),
                        Provenance::Varies {
                            count: distinct.len(),
                        },
                    ));
                    can_override = true;
                    continue;
                }
                let class = wires.first().and_then(|(_, e)| e.class.clone());
                let potential = q
                    .connections()
                    .get(*c)
                    .and_then(|conn| potential_of(q, conn))
                    .map(|p| q.get::<PotentialNameOf>(p).name.text.clone())
                    .unwrap_or_else(|| "?".into());
                // The source a person can go to: none for the wire's own override.
                let link = (!matches!(&source, PropSource::Override(at) if at == c))
                    .then(|| source_link(q, &source, name));
                if !links.contains(&link) {
                    links.push(link);
                }
                // Why no protective device's rating sizes (or checks) the wire's size (doc 08 §4.2); a PE wire takes
                // its ends' ratings and is not said.
                let pe = matches!(class, Some(ew_schema::CircuitClass::Pe));
                let unrated = (name == "csa" && !pe)
                    .then(|| ew_derive::unrated(q, *c))
                    .flatten();
                let provenance = match &source {
                    // A size set by hand behind a breaker no supply reaches is checked against no rating: said.
                    PropSource::Override(at) if at == c => {
                        can_reset = true;
                        let mut note = "override (CDP)".to_owned();
                        if let Some(ew_derive::Unrated::UnfedProtection(device)) = unrated {
                            note.push_str(&format!(
                                " · no supply found upstream of {}, so its rating is not used",
                                device_tag_text(q, device)
                            ));
                        }
                        Provenance::Overridden { note }
                    }
                    PropSource::Override(at) => {
                        can_override = true;
                        let on = numbers
                            .number(*at)
                            .map_or_else(|| "another connection".to_owned(), |n| n.label.clone());
                        Provenance::Overridden {
                            note: format!("override on {on}"),
                        }
                    }
                    // A core's cross-section and colour are the cable's: changed at the cable.
                    other @ PropSource::Conductor(_) => Provenance::Derived {
                        via: prop_via(q, other, &potential, class.as_ref()),
                    },
                    other => {
                        can_override = true;
                        let mut via = prop_via(q, other, &potential, class.as_ref());
                        // A size from the profile that no protective device's rating sizes says why at the size: a
                        // device that nothing feeds, no supply, or no protective device upstream.
                        if other.level() < 3
                            && let Some(why) = unrated
                        {
                            via.push_str(&match why {
                                ew_derive::Unrated::UnfedProtection(device) => format!(
                                    " · no supply found upstream of {}, so its rating is not used",
                                    device_tag_text(q, device)
                                ),
                                ew_derive::Unrated::NoSupply => {
                                    " · no supply found upstream, so no protective device sizes it".to_owned()
                                }
                                ew_derive::Unrated::NoProtection => {
                                    " · no protective device upstream sizes it".to_owned()
                                }
                            });
                        }
                        Provenance::Derived { via }
                    }
                };
                per.push((Some(value), provenance));
            }
            let (value, provenance) = merge(cx, &ty, per);
            let mut actions = Vec::new();
            if can_override {
                actions.push(RowAction::Override);
            }
            if can_reset {
                actions.push(RowAction::Reset);
            }
            let settable = if name == "shielded" || name == "twisted" {
                "overrides.flags"
            } else {
                path
            };
            let route = match editable(settable) {
                EditRoute::FieldsSet { .. } => fields_set("connection", settable),
                other => other,
            };
            let varies = matches!(provenance, Provenance::Varies { .. });
            let mut built = derived(DerivedRow {
                kind: "connection",
                name,
                partner: Some((path, route, false)),
                category: Category::Electrical,
                ty,
                value,
                provenance,
                actions,
            });
            if let ([Some(link)], false) = (links.as_slice(), varies) {
                built.row.source = Some(link.clone());
            }
            out.push(built);
        }
        out.push(length_row(q, cx, &conns));
        out.extend(end_rows(q, cx, &conns));
        // The wire number.
        let mut per = Vec::new();
        let mut any_frozen = false;
        let mut any_free = false;
        for (c, j) in conns.iter().zip(jsons) {
            // A cable's core has no wire number (FR-NUM-019): its cable and core name it (`-1W1 BN`), never `?`.
            let core = match q.connections().get(*c).map(|conn| conn.kind.clone()) {
                Some(ew_schema::ConnectionKind::Core(x)) => record_title(q, x.as_uuid(), None),
                // A bus line has no wire number either: its cable names it (it read `?`).
                Some(ew_schema::ConnectionKind::Bus(line)) => line.cable.map(|x| device_tag_text(q, x)),
                _ => None,
            };
            let printed = core.clone().unwrap_or_else(|| wire_number_text(q, *c));
            let frozen = j.get("number").and_then(|n| n.get("frozen")).is_some();
            if frozen {
                any_frozen = true;
            } else if printed != "?" && core.is_none() {
                any_free = true;
            }
            let provenance = if frozen {
                Provenance::Frozen {
                    note: "frozen".into(),
                }
            } else {
                Provenance::Derived {
                    via: "wire numbering".into(),
                }
            };
            per.push((Some(Value::String(printed)), provenance));
        }
        let (value, provenance) = merge(cx, &FieldType::Ident, per);
        let mut actions = Vec::new();
        if any_free && conns.len() == 1 {
            actions.push(RowAction::Freeze);
        }
        if any_frozen {
            actions.push(RowAction::Unfreeze);
        }
        out.push(derived(DerivedRow {
            kind: "connection",
            name: "number",
            partner: Some(("number", editable("number"), true)),
            category: Category::Identity,
            ty: FieldType::Ident,
            value,
            provenance,
            actions,
        }));
        // The potential.
        let ends: Vec<Vec<EndpointKey>> = conns
            .iter()
            .map(|c| {
                q.connections()
                    .get(*c)
                    .map(|conn| {
                        [&conn.a, &conn.b]
                            .into_iter()
                            .filter_map(EndpointKey::of)
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect();
        out.push(potential_row(q, cx, "connection", &ends));
        out.extend(bus_line_rows(q, cx, &conns));
        out
    }
}

/// A wire's Length row (doc 13 §3.3): the length typed on it, in millimetres, with the chip "typed by hand" and Reset
/// where this connection holds it; a core's "cut with its cable -103W1"; else empty, with the chip "no length: type it
/// here, or the fitter measures the wire". A length typed here is `overrides.length` of the selected connection.
fn length_row(q: &Q, cx: &SheetCx<'_>, conns: &[Id<Connection>]) -> Built {
    let records = q.get::<WireRecordSet>(());
    let mut per = Vec::new();
    let (mut can_override, mut can_reset) = (false, false);
    for c in conns {
        let record = wires_of(q, *c)
            .first()
            .and_then(|(wire, _)| records.iter().find(|r| r.wire.id == *wire));
        let Some(record) = record else {
            per.push((None, Provenance::Authored));
            continue;
        };
        per.push(match record.length {
            WireLength::Typed(n) => {
                if record.wire.override_sources.length == Some(*c) {
                    can_reset = true;
                } else {
                    can_override = true;
                }
                (
                    Some(Value::from(n.0)),
                    Provenance::Overridden {
                        note: "typed by hand".into(),
                    },
                )
            }
            WireLength::WithCable(cable) => (
                None,
                Provenance::Derived {
                    via: format!("cut with its cable {}", device_tag_text(q, cable)),
                },
            ),
            WireLength::Missing => {
                can_override = true;
                (
                    None,
                    Provenance::Derived {
                        via: "no length: type it here, or the fitter measures the wire".into(),
                    },
                )
            }
        });
    }
    let (mut value, provenance) = merge(cx, &FieldType::Nm, per);
    if let RowValue::One { json, text } = &mut value
        && let Some(nm) = json.as_i64()
    {
        *text = format!("{} mm", crate::output::layout_edit::mm_text(q, i128::from(nm)));
    }
    let mut actions = Vec::new();
    if can_override {
        actions.push(RowAction::Override);
    }
    if can_reset {
        actions.push(RowAction::Reset);
    }
    let route = match Session::editable(Kind::Connection, "overrides.length") {
        EditRoute::FieldsSet { .. } => EditRoute::FieldsSet {
            kind: "connection".to_owned(),
            path: "overrides.length".to_owned(),
        },
        other => other,
    };
    derived(DerivedRow {
        kind: "connection",
        name: "length",
        partner: Some(("overrides.length", route, false)),
        category: Category::Electrical,
        ty: FieldType::Nm,
        value,
        provenance,
        actions,
    })
}

/// A wire's From end and To end (doc 13 §4.5), each an End row and a Part row under its own heading: the treatment
/// (`ferrule`, `twin_ferrule`, `ring_lug`, `bare`) with the chip "set by hand" and Reset where the selected connection
/// holds it, else the rule's, unverified, and what it assumed (a connector's contact, an end the table cannot decide and
/// an end made outside the project have none of the four); the part with "set by hand", else the workshop stock's. A
/// row sets the end by hand where the one selected connection is the one that lands on the end (`overrides.end_a` when
/// it lands there by its endpoint `a`, else `end_b`); with another selection it is read-only, and the Connections
/// table's end cells set it.
fn end_rows(q: &Q, cx: &SheetCx<'_>, conns: &[Id<Connection>]) -> Vec<Built> {
    let records = q.get::<WireRecordSet>(());
    let mut out = Vec::new();
    for (i, category, names) in [
        (0, Category::FromEnd, ("from_end", "from_part")),
        (1, Category::ToEnd, ("to_end", "to_part")),
    ] {
        let (mut ends, mut parts) = (Vec::new(), Vec::new());
        let (mut can_reset_end, mut can_reset_part) = (false, false);
        let mut at: Option<(Id<Connection>, bool)> = None;
        for c in conns {
            let record = wires_of(q, *c)
                .first()
                .and_then(|(wire, _)| records.iter().find(|r| r.wire.id == *wire));
            let Some(record) = record else {
                ends.push((None, Provenance::Authored));
                parts.push((None, Provenance::Authored));
                continue;
            };
            let end = &record.ends[i];
            let holder = end_connection(q, &record.wire, i);
            if conns.len() == 1 && holder.is_some_and(|(h, _)| h == *c) {
                at = holder;
            }
            let word = match end.treatment {
                EndTreatment::Ferrule { .. } => Some("ferrule"),
                EndTreatment::TwinFerrule { .. } => Some("twin_ferrule"),
                EndTreatment::RingLug => Some("ring_lug"),
                EndTreatment::Bare => Some("bare"),
                EndTreatment::Connector | EndTreatment::External | EndTreatment::Unknown { .. } => None,
            };
            let hand_part = ew_derive::end_override(q, &record.wire, i)
                .and_then(|o| o.part)
                .is_some();
            let by = |hand: bool| match (hand, end.treated_by) {
                (true, _) => Provenance::Overridden {
                    note: "set by hand".into(),
                },
                (false, TreatedBy::Rule { assumed, .. }) => Provenance::Derived {
                    via: match (assumed.class, assumed.clamp) {
                        (true, true) => "rule (unverified): fine-stranded wire, clamp kind not known",
                        (true, false) => "rule (unverified): fine-stranded wire",
                        (false, true) => "rule (unverified): clamp kind not known",
                        (false, false) => "rule (unverified)",
                    }
                    .into(),
                },
                (false, TreatedBy::Outside) => Provenance::Derived {
                    via: "made outside the project".into(),
                },
                (false, TreatedBy::Hand) => Provenance::Overridden {
                    note: "set by hand".into(),
                },
            };
            let hand_end = end.treated_by == TreatedBy::Hand;
            can_reset_end |= hand_end && at.is_some();
            can_reset_part |= hand_part && at.is_some();
            ends.push((word.map(Value::from), by(hand_end)));
            let part = end.part.map(|p| serde_json::json!({ "copy": p.to_string() }));
            parts.push((
                part,
                if hand_part {
                    by(true)
                } else {
                    Provenance::Derived {
                        via: "workshop stock".into(),
                    }
                },
            ));
        }
        let treatment = FieldType::Optional(Box::new(FieldType::Enum {
            options: ["ferrule", "twin_ferrule", "ring_lug", "bare"]
                .map(str::to_owned)
                .to_vec(),
        }));
        let part = FieldType::Optional(Box::new(FieldType::Link { kind: "part".into() }));
        for (name, member, ty, per, can_reset) in [
            (names.0, "treatment", treatment, ends, can_reset_end),
            (names.1, "part", part, parts, can_reset_part),
        ] {
            let (value, provenance) = merge(cx, &ty, per);
            let path = at.map(|(_, a)| format!("overrides.end_{}.{member}", if a { "a" } else { "b" }));
            let route = at.map(|(_, a)| EditRoute::FieldsSet {
                kind: "connection".to_owned(),
                path: format!("overrides.end_{}", if a { "a" } else { "b" }),
            });
            let mut actions = vec![RowAction::Override];
            if can_reset {
                actions.push(RowAction::Reset);
            }
            out.push(derived(DerivedRow {
                kind: "connection",
                name,
                partner: path.as_deref().zip(route).map(|(p, r)| (p, r, false)),
                category,
                ty,
                value,
                provenance,
                actions,
            }));
        }
    }
    out
}

/// The row that holds the source of the wire property `name` (`csa`, `colour`, `shielded`): the same
/// property of the connection that holds the override, of the cable's core or of the potential definition; else the
/// project's profile or wire default, where a profile is chosen when the value is the built-in default.
fn source_link(q: &Q, source: &PropSource, name: &str) -> SourceLink {
    let project = |row: &str| SourceLink::Project {
        row: format!("project/settings.{row}"),
    };
    let profile = if name == "colour" {
        "colour_profile"
    } else {
        "size_profile"
    };
    match source {
        PropSource::Override(at) => SourceLink::Record {
            id: at.as_uuid(),
            row: format!("connection/derived.{name}"),
        },
        // A core's value is its cable's: the cable's Parts row, whose part gives the cores.
        PropSource::Conductor(c) => match q.conductors().get(*c) {
            Some(core) => SourceLink::Record {
                id: core.cable.as_uuid(),
                row: CABLE_SOURCE_ROW.to_owned(),
            },
            None => SourceLink::Record {
                id: c.as_uuid(),
                row: format!("conductor/{name}"),
            },
        },
        PropSource::PotentialDef(d) => SourceLink::Record {
            id: d.as_uuid(),
            row: match name {
                "shielded" | "twisted" => format!("potential_def/props.flags.{name}"),
                _ => format!("potential_def/props.{name}"),
            },
        },
        PropSource::ColourRow { .. } => project("colour_profile"),
        // A size from the rating of the protecting device: that device's Parts row, whose part gives the rating.
        PropSource::SizeRow {
            rule: ew_derive::SizeRule::Rating { device, .. },
            ..
        } => SourceLink::Record {
            id: device.as_uuid(),
            row: CABLE_SOURCE_ROW.to_owned(),
        },
        PropSource::SizeRow { .. } => project("size_profile"),
        PropSource::ProjectDefault => project("wire_default"),
        PropSource::BuiltIn { .. } => project(profile),
    }
}

/// The chip text of a wire property's source below level 7, in the words of the derivation's `prop-via-*` texts (the
/// SQL view's form of the same fact is `prop-source-*`, beside them), in English.
pub(super) fn prop_via(
    q: &Q,
    source: &PropSource,
    potential: &str,
    class: Option<&ew_schema::CircuitClass>,
) -> String {
    let src = q.project().languages.as_ref().map(|l| l.source.to_string());
    match source {
        PropSource::Conductor(c) => {
            let core = q.conductors().get(*c);
            let cable = core.map(|k| device_tag_text(q, k.cable)).unwrap_or_default();
            let core_id = core
                .and_then(|k| serde_json::to_value(&k.core).ok())
                .map(|v| ml_text(&v, src.as_deref()))
                .unwrap_or_default();
            ew_derive::text!("prop-via-core", cable, core_id)
        }
        PropSource::PotentialDef(d) => {
            let name = q
                .potential_defs()
                .get(*d)
                .map(|x| x.name.as_str().to_owned())
                .unwrap_or_default();
            ew_derive::text!("prop-via-potential", name)
        }
        PropSource::ColourRow { profile, .. } => ew_derive::text!(
            "prop-via-colour-profile",
            potential,
            profile_name(q, "colour_profile", profile, src.as_deref())
        ),
        PropSource::SizeRow {
            rule: ew_derive::SizeRule::Rating { device, current },
            ..
        } => ew_derive::text!(
            "prop-via-rating",
            potential,
            device_tag_text(q, *device),
            current.to_string(),
            ew_derive::current_table_basis()
        ),
        PropSource::SizeRow {
            profile,
            class: row_class,
            ..
        } => {
            let class = serde_json::to_value(class.unwrap_or(row_class))
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            ew_derive::text!(
                "prop-via-size-profile",
                potential,
                profile_name(q, "size_profile", profile, src.as_deref()),
                class
            )
        }
        PropSource::ProjectDefault => ew_derive::text!("prop-source-project"),
        PropSource::BuiltIn { profile } => ew_derive::text!("prop-via-built-in", profile.as_str()),
        PropSource::Override(_) => ew_derive::text!("prop-via-override"),
    }
    .to_string()
}
