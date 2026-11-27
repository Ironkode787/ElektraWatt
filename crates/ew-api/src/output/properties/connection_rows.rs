// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A connection's derived rows: the seven wire properties with their sources, the wire's length, its two ends, the
//! wire number, the potential (also an interruption point's) and the route in frame cells.
//!
//! Not here: a bus line's rows (`bus_rows.rs`); a fluid line's rows (`fluid_rows.rs`).

use std::collections::BTreeSet;

use ew_derive::{
    EndTreatment, NameSource, PotentialNameOf, PotentialPartition, PropSource, Q, TreatedBy, WireLength,
    WireNumbers, WireRecordSet, cp_label,
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
        out.extend(way_row(q, &conns));
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

/// The chip of a row of a wire's own values on a stretch several wires share (a star's feed arm, a chain's arm at a
/// clamp): no one wire's value shows there, and none is set there.
const SHARED_STRETCH: &str =
    "several wires share this stretch: set a wire's own values in the Connections table";

/// The wire whose own values a row of connection `c` shows and sets (doc 13 §2.3): the one wire `c` stands for;
/// `Err(())` on a stretch several wires share; `Ok(None)` for a connection of no wire.
pub(crate) fn wire_alone<'a>(
    q: &Q,
    records: &'a [ew_derive::WireRecord],
    c: Id<Connection>,
) -> Result<Option<&'a ew_derive::WireRecord>, ()> {
    match wires_of(q, c).as_slice() {
        [] => Ok(None),
        [(wire, _)] => Ok(records.iter().find(|r| r.wire.id == *wire)),
        _ => Err(()),
    }
}

/// A wire's Length row (doc 13 §3.3, doc 12 §9): the length typed on it, in millimetres, with the chip "typed by hand"
/// and Reset where the wire keeps it; a core's "cut with its cable -103W1"; a routed or stale length with how it is
/// made ([`length_note`]); else empty, with what gives it one. A length typed here goes where the Connections table
/// writes it, the wire's first own connection (`WireRecord::own`), through `Session::layout_edit`; on a stretch several
/// wires share the row is read-only and says so.
fn length_row(q: &Q, cx: &SheetCx<'_>, conns: &[Id<Connection>]) -> Built {
    let records = q.get::<WireRecordSet>(());
    let mut per = Vec::new();
    let (mut can_override, mut can_reset, mut shared) = (false, false, false);
    for c in conns {
        let record = match wire_alone(q, &records, *c) {
            Err(()) => {
                shared = true;
                per.push((
                    None,
                    Provenance::Derived {
                        via: SHARED_STRETCH.into(),
                    },
                ));
                continue;
            }
            Ok(r) => r,
        };
        let Some(record) = record else {
            per.push((None, Provenance::Authored));
            continue;
        };
        let note = length_note(q, record);
        per.push(match record.length {
            WireLength::Typed(n) => {
                if record.wire.override_sources.length.is_some()
                    && record.wire.override_sources.length == record.own().first().copied()
                {
                    can_reset = true;
                } else {
                    can_override = true;
                }
                (Some(Value::from(n.0)), Provenance::Overridden { note })
            }
            WireLength::WithCable(_) => (None, Provenance::Derived { via: note }),
            WireLength::Routed { length, .. } | WireLength::Stale { length, .. } => {
                can_override = true;
                (Some(Value::from(length.0)), Provenance::Derived { via: note })
            }
            WireLength::Missing => {
                can_override = true;
                (None, Provenance::Derived { via: note })
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
    if can_override && !shared {
        actions.push(RowAction::Override);
    }
    if can_reset && !shared {
        actions.push(RowAction::Reset);
    }
    derived(DerivedRow {
        kind: "connection",
        name: "length",
        partner: (!shared).then(|| {
            (
                "derived.length",
                EditRoute::Command {
                    command: "fields.set".to_owned(),
                },
                false,
            )
        }),
        category: Category::Electrical,
        ty: FieldType::Nm,
        value,
        provenance,
        actions,
    })
}

/// A wire's From end and To end (doc 13 §4.5), each a treatment row and a part row under its own heading: the treatment
/// (`ferrule`, `twin_ferrule`, `ring_lug`, `bare`) with the chip "set by hand" and Reset, else the rule's, unverified,
/// and what it assumed (a connector's contact, an end the table cannot decide and an end made outside the project have
/// none of the four); the part with "set by hand" and Reset, else the workshop stock's. The selected connections must be
/// of one wire: a row then sets that wire's end by hand from any of them, through `Session::layout_edit`, on the wire's
/// own connection that holds it (`EndRecord::hand_at`). On a stretch several wires share the rows are read-only and say
/// so.
fn end_rows(q: &Q, cx: &SheetCx<'_>, conns: &[Id<Connection>]) -> Vec<Built> {
    let records = q.get::<WireRecordSet>(());
    let shared = conns.iter().any(|c| wire_alone(q, &records, *c).is_err());
    let wire_of = |c: &Id<Connection>| wire_alone(q, &records, *c).ok().flatten();
    let mut wires: Vec<Id<Connection>> = conns
        .iter()
        .filter_map(|c| wire_of(c).map(|r| r.wire.id))
        .collect();
    wires.dedup();
    let one_wire = wires.len() == 1 && conns.iter().all(|c| wire_of(c).is_some());
    let mut out = Vec::new();
    for (i, category, names) in [
        (0, Category::FromEnd, ("from_end", "from_part", "from_marker")),
        (1, Category::ToEnd, ("to_end", "to_part", "to_marker")),
    ] {
        let (mut ends, mut parts, mut markers) = (Vec::new(), Vec::new(), Vec::new());
        let (mut by_hand_end, mut by_hand_part, mut by_hand_marker, mut holder) =
            (false, false, false, false);
        for c in conns {
            let Some(record) = wire_of(c) else {
                let p = if shared {
                    Provenance::Derived {
                        via: SHARED_STRETCH.into(),
                    }
                } else {
                    Provenance::Authored
                };
                ends.push((None, p.clone()));
                parts.push((None, p.clone()));
                markers.push((None, p));
                continue;
            };
            let end_marker = &record.ends[i];
            by_hand_marker |= end_marker.marked_by == ew_derive::MarkedBy::Hand;
            markers.push((
                match &end_marker.marker {
                    ew_derive::MarkerText::Lines(lines) => Some(Value::from(lines.join(" / "))),
                    ew_derive::MarkerText::NotMarked(_) => None,
                },
                match end_marker.marked_by {
                    ew_derive::MarkedBy::Hand => Provenance::Overridden {
                        note: "set by hand".into(),
                    },
                    ew_derive::MarkedBy::Setting => Provenance::Derived {
                        via: "workshop marking".into(),
                    },
                },
            ));
            let end = &record.ends[i];
            holder |= end.hand_at.is_some();
            let word = match end.treatment {
                EndTreatment::Ferrule { .. } => Some("ferrule"),
                EndTreatment::TwinFerrule { .. } => Some("twin_ferrule"),
                EndTreatment::RingLug => Some("ring_lug"),
                EndTreatment::Bare => Some("bare"),
                EndTreatment::Connector | EndTreatment::External | EndTreatment::Unknown { .. } => None,
            };
            let set_by_hand = || Provenance::Overridden {
                note: "set by hand".into(),
            };
            let hand_end = end.treated_by == TreatedBy::Hand;
            by_hand_end |= hand_end;
            by_hand_part |= end.part_by_hand;
            ends.push((
                word.map(Value::from),
                match end.treated_by {
                    TreatedBy::Hand => set_by_hand(),
                    TreatedBy::Rule { row, assumed } => Provenance::Derived {
                        via: rule_words(row, assumed.class, assumed.clamp).into(),
                    },
                    TreatedBy::Outside => Provenance::Derived {
                        via: "made outside the project".into(),
                    },
                },
            ));
            let part = end.part.map(|p| serde_json::json!({ "copy": p.to_string() }));
            parts.push((
                part,
                if end.part_by_hand {
                    set_by_hand()
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
        let editable = one_wire && holder;
        for (name, ty, per, by_hand) in [
            (names.0, treatment, ends, by_hand_end),
            (names.1, part, parts, by_hand_part),
            (
                names.2,
                FieldType::Optional(Box::new(FieldType::Text)),
                markers,
                by_hand_marker,
            ),
        ] {
            let (value, provenance) = merge(cx, &ty, per);
            let mut actions = Vec::new();
            if !shared {
                actions.push(RowAction::Edit);
            }
            if by_hand && !shared {
                actions.push(RowAction::Reset);
            }
            let path = format!("derived.{name}");
            out.push(derived(DerivedRow {
                kind: "connection",
                name,
                partner: editable.then(|| {
                    (
                        path.as_str(),
                        EditRoute::Command {
                            command: "fields.set".to_owned(),
                        },
                        false,
                    )
                }),
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

/// The chip of an end the rule table's row `row` decided (doc 13 §4.2): "rule", "(unverified)" while the row's own flag
/// (`TreatmentRule::verified`) says no panel builder has confirmed it, and what it assumed: the wire fine-stranded
/// (`class`), the clamp's kind (`clamp`).
fn rule_words(row: u8, class: bool, clamp: bool) -> &'static str {
    let verified = ew_derive::TREATMENT_RULES
        .iter()
        .any(|r| r.row == row && r.verified);
    match (verified, class, clamp) {
        (false, true, true) => "rule (unverified): fine-stranded wire, clamp kind not known",
        (false, true, false) => "rule (unverified): fine-stranded wire",
        (false, false, true) => "rule (unverified): clamp kind not known",
        (false, false, false) => "rule (unverified)",
        (true, true, true) => "rule: fine-stranded wire, clamp kind not known",
        (true, true, false) => "rule: fine-stranded wire",
        (true, false, true) => "rule: clamp kind not known",
        (true, false, false) => "rule",
    }
}

/// Where the end `end` (0 the From end, 1 the To end) of the wire of connection `c` is set by hand
/// (`EndRecord::hand_at`): the wire's own connection that holds it, whether in `end_a`, and what it holds now.
pub(crate) fn end_holder(
    q: &Q,
    c: Id<Connection>,
    end: usize,
) -> Option<(Id<Connection>, bool, Option<ew_schema::EndOverride>)> {
    let records = q.get::<WireRecordSet>(());
    let (holder, at_a) = wire_alone(q, &records, c)
        .ok()
        .flatten()?
        .ends
        .get(end)?
        .hand_at?;
    let o = &q.connections().get(holder)?.overrides;
    Some((holder, at_a, if at_a { o.end_a.clone() } else { o.end_b.clone() }))
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

/// A wire's Way row (doc 12 §12.5): the ducts its stored way runs along and "door" where it crosses into one ("Duct 1,
/// door"), "direct" over the fronts, with the chip "routed" or "stale", and Show on its cabinet's page; for the
/// connections of one wire (the Connections table selects a wire's every stretch, a trunk it shares included), none
/// for connections of several wires or of no wire, or of a wire without a stored way.
fn way_row(q: &Q, conns: &[Id<Connection>]) -> Option<Built> {
    let records = q.get::<WireRecordSet>(());
    let wires: Vec<&ew_derive::WireRecord> = records
        .iter()
        .filter(|r| conns.iter().all(|c| r.wire.constituents.contains(c)))
        .collect();
    let [r] = wires.as_slice() else { return None };
    let way = ew_derive::layout::stored_way(q, r.wire.id)?;
    let text = ew_derive::text::way_words(q, &way.way);
    // Show on 902: the cabinet's panel page, with the wire selected, which shows its way there.
    let folio = ew_derive::layout::cabinet_page(q, way.space)
        .and_then(|p| q.pages().get(p).map(|p| ew_derive::folio_label(p)));
    let link = folio.map(|folio| super::RowLink {
        label: ew_derive::text!("way-show-on", folio.clone()),
        actions: vec![
            ("page.goto", serde_json::json!({ "folio": folio })),
            ("select.set", serde_json::json!({ "ids": [r.wire.id.as_uuid()] })),
        ],
    });
    let mut built = derived(DerivedRow {
        kind: "connection",
        name: "way",
        partner: None,
        category: Category::Electrical,
        ty: FieldType::Text,
        value: RowValue::One {
            json: Value::String(text.clone()),
            text,
        },
        provenance: Provenance::Derived {
            via: ew_derive::text::wire_length_source(&r.length).to_string(),
        },
        actions: Vec::new(),
    });
    built.row.link = link;
    Some(built)
}

/// The chip of a wire's Length row (doc 12 §9): how its length is made, in the words of the derivation, in English.
/// A typed length says what Route gives where it laid the wire; a missing one of a cabinet's wire what Route does or
/// why it finds no way, naming the cabinet's page.
fn length_note(q: &Q, r: &ew_derive::WireRecord) -> String {
    use ew_derive::layout::{
        CabinetWire, Planned, RouteScope, RouteStatus, cabinet_page, plan_routes, route_status,
        routed_length, space_of_wire,
    };
    let text = match r.length {
        WireLength::Typed(_) => match route_status(q, &r.wire) {
            RouteStatus::Holds { run, door } | RouteStatus::Stale { run, door, .. } => ew_derive::text!(
                "length-typed-routed",
                format!(
                    "{} mm",
                    ew_derive::mm_exact(i128::from(routed_length(q, run, door).0))
                )
            ),
            RouteStatus::None => ew_derive::text!("length-typed-by-hand"),
        },
        WireLength::WithCable(cable) => ew_derive::text!("length-with-cable", device_tag_text(q, cable)),
        WireLength::Routed { .. } | WireLength::Stale { .. } => {
            ew_derive::text::length_words(q, r.length).unwrap_or_else(|| ew_derive::text!("length-none"))
        }
        WireLength::Missing => match space_of_wire(q, r) {
            Some((space, CabinetWire::Routed)) => {
                let plan = plan_routes(q, space, &RouteScope::Wires([r.wire.id].into_iter().collect()));
                let page =
                    cabinet_page(q, space).and_then(|p| q.pages().get(p).map(|p| ew_derive::folio_label(p)));
                match (plan.wires.first().map(|w| &w.planned), page) {
                    (Some(Planned::NoWay(why)), _) => {
                        ew_derive::text!("length-no-way", ew_derive::text::no_way(q, *why))
                    }
                    (_, Some(page)) => ew_derive::text!("length-no-way-yet", page),
                    (_, None) => ew_derive::text!("length-none"),
                }
            }
            _ => ew_derive::text!("length-none"),
        },
    };
    text.to_string()
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

#[cfg(test)]
mod tests {
    use super::rule_words;

    /// A rule's chip says "unverified" exactly when its row of the table says so.
    #[test]
    fn a_rule_says_unverified_from_its_own_flag() {
        for r in &ew_derive::TREATMENT_RULES {
            for (class, clamp) in [(false, false), (true, false), (false, true), (true, true)] {
                let words = rule_words(r.row, class, clamp);
                assert_eq!(
                    words.contains("(unverified)"),
                    !r.verified,
                    "row {}: {words}",
                    r.row
                );
                assert!(words.starts_with("rule"), "{words}");
            }
        }
        // A row the table does not have is not confirmed.
        assert_eq!(rule_words(u8::MAX, false, false), "rule (unverified)");
    }
}
