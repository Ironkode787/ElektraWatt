// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A device's derived rows (§6.4, D15): the tag, the structure per aspect, the main location and the connection
//! points.
//!
//! Not here: the provenance of the tag row (`properties.rs`); a connection's rows (`connection_rows.rs`).

use std::collections::BTreeSet;

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_rds::{Aspect, RdsSyntax};
use ew_derive::{DeviceTag, MainRep, Q, ResolvedStructure, SlotSource, StructureOf, cp_label};
use ew_schema::{Device, Kind};
use serde_json::Value;

use super::context::{SheetCx, member_of_aspect};
use super::fluid_rows::{fluid_port_rows, source_medium_row};
use super::sections::{Built, derived, merge};
use super::{Provenance, RowAction, RowValue, device_tag_text, folio_of, main_location, tag_provenance};
use crate::Session;
use crate::output::fields::{Category, EditRoute, FieldType};

impl Session {
    /// A device's derived rows: the tag, the structure per aspect, the main location and the connection points.
    pub(super) fn derived_rows_of_device(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid]) -> Vec<Built> {
        let editable = |path: &str| Self::editable(Kind::Device, path);
        let mut out = Vec::new();
        let devices: Vec<Id<Device>> = targets.iter().map(|t| Id::from_uuid(*t)).collect();
        // The tag.
        let mut per = Vec::new();
        let mut products: BTreeSet<String> = BTreeSet::new();
        let mut all_frozen = true;
        let mut any_frozen = false;
        for d in &devices {
            let tag = q.get::<DeviceTag>(*d);
            let text = device_tag_text(q, *d);
            let provenance = tag_provenance(q, *d, tag.as_ref().as_ref());
            if matches!(provenance, Provenance::Frozen { .. }) {
                any_frozen = true;
            } else {
                all_frozen = false;
            }
            products.insert(
                tag.as_ref()
                    .as_ref()
                    .map(|t| t.product.clone())
                    .unwrap_or_default(),
            );
            per.push((Some(Value::String(text)), provenance));
        }
        let (mut value, provenance) = merge(cx, &FieldType::Ident, per);
        // The value is the product designation (what Freeze writes), the text the displayed tag.
        if let (RowValue::One { json, .. }, [product]) =
            (&mut value, products.iter().collect::<Vec<_>>().as_slice())
        {
            *json = Value::String((*product).clone());
        }
        // One device's Tag field is typed into: an existing device's tag proposes Join,
        // another text freezes the typed tag (the partner write, as Override would).
        let mut actions = Vec::new();
        if devices.len() == 1 {
            actions.push(RowAction::Edit);
        }
        if any_frozen {
            actions.push(RowAction::Unfreeze);
        }
        // Freeze on several devices is one `fields.set_many` with each device's own product designation (part 2,
        // §7.5).
        if !all_frozen {
            actions.push(RowAction::Freeze);
        }
        out.push(derived(
            "device",
            "tag",
            Some(("tag.frozen", editable("tag.frozen"), true)),
            Category::Identity,
            FieldType::Ident,
            value,
            provenance,
            actions,
        ));
        // The structure, per aspect with a node on any target.
        let structures: Vec<std::sync::Arc<ResolvedStructure>> =
            devices.iter().map(|d| q.get::<StructureOf>(*d)).collect();
        let mut aspects: BTreeSet<Aspect> = structures.iter().flat_map(|s| s.slots.keys().copied()).collect();
        // rc.2, F12: the location is what a drafter sets first, so its row is there on every device, set or not.
        aspects.insert(Aspect::Location);
        for aspect in aspects {
            let member = member_of_aspect(aspect);
            let mut per = Vec::new();
            let mut explicit = false;
            let mut inherited = false;
            for (d, s) in devices.iter().zip(&structures) {
                let Some(slot) = s.slots.get(&aspect) else {
                    per.push((None, Provenance::Authored));
                    continue;
                };
                let text = s.render_aspect(aspect, &RdsSyntax::default());
                let provenance = match slot.source {
                    SlotSource::Explicit => {
                        explicit = true;
                        Provenance::Authored
                    }
                    SlotSource::MainRepPage => {
                        inherited = true;
                        let folio = (*q.get::<MainRep>(*d))
                            .and_then(|r| q.reps().get(r))
                            .and_then(|r| folio_of(q, r.page))
                            .unwrap_or_default();
                        Provenance::Inherited {
                            from: format!("page {folio}"),
                        }
                    }
                    SlotSource::Parent => {
                        inherited = true;
                        let dev = q.devices().get(*d);
                        let from = dev
                            .and_then(|x| x.aspects.terminal.as_ref().map(|t| t.strip).or(x.parent))
                            .map(|p| device_tag_text(q, p))
                            .unwrap_or_default();
                        Provenance::Inherited { from }
                    }
                    // The structure box around the main representation (FR-PRJ-020).
                    SlotSource::StructureBox => {
                        inherited = true;
                        let folio = (*q.get::<MainRep>(*d))
                            .and_then(|r| q.reps().get(r))
                            .and_then(|r| folio_of(q, r.page))
                            .unwrap_or_default();
                        Provenance::Inherited {
                            from: format!("structure box on page {folio}"),
                        }
                    }
                };
                per.push((Some(Value::String(text)), provenance));
            }
            let (value, provenance) = merge(cx, &FieldType::Text, per);
            // The row stands for its stored member: it is typed into as that member is.
            let mut actions = vec![RowAction::Edit];
            if inherited {
                actions.push(RowAction::Override);
            }
            if explicit {
                actions.push(RowAction::Reset);
            }
            let path = format!("structure.{member}");
            out.push(derived(
                "device",
                &format!("structure.{member}"),
                Some((&path, editable(&path), false)),
                Category::Structure,
                FieldType::Slot,
                value,
                provenance,
                actions,
            ));
        }
        // A valve's position on its valve terminal (the persona round: Position stood under Structure, found only
        // through the filter, and typing into it did nothing, since `assembly.attach` owns the member and the row was
        // read-only): one row in Main for valves and the children of a module with valve positions, typed into to move
        // one valve to another position (`assembly.attach` through `Session::layout_edit`).
        // Per device: its module with valve positions (if any) and its position there; `None` for no valve.
        type Seat = (Option<Id<Device>>, Option<u32>);
        let seats: Vec<Option<Seat>> = devices
            .iter()
            .map(|d| {
                let dev = q.devices().get(*d)?;
                let module = dev.parent.filter(|p| ew_derive::valve_positions(q, *p).is_some());
                (module.is_some() || ew_derive::device_fits_terminal(q, *d))
                    .then_some((module, module.and(dev.position)))
            })
            .collect();
        if !seats.is_empty() && seats.iter().all(Option::is_some) {
            let per: Vec<(Option<Value>, Provenance)> = seats
                .iter()
                .flatten()
                .map(|(module, position)| match module {
                    Some(m) => (
                        position.map(|p| Value::String(p.to_string())),
                        Provenance::Derived {
                            via: format!("child of {}", device_tag_text(q, *m)),
                        },
                    ),
                    None => (
                        None,
                        Provenance::Derived {
                            via: "on no valve terminal".to_owned(),
                        },
                    ),
                })
                .collect();
            let (value, provenance) = merge(cx, &FieldType::Text, per);
            let actions = if devices.len() == 1 {
                vec![RowAction::Edit]
            } else {
                Vec::new()
            };
            out.push(derived(
                "device",
                "position",
                Some((
                    "derived.position",
                    EditRoute::Command {
                        command: "assembly.attach".to_owned(),
                    },
                    false,
                )),
                Category::Structure,
                FieldType::Text,
                value,
                provenance,
                actions,
            ));
        }
        // A valve terminal's positions (a terminal drawn before as a free box shows its positions nowhere on
        // its page): each with what holds it, read-only; the window words the blanking plates and the empty ones.
        if let [one] = devices.as_slice() {
            let cells = ew_derive::terminal_cells(q, *one);
            if !cells.is_empty() {
                let text = cells
                    .iter()
                    .map(|c| {
                        let what = match &c.slot {
                            ew_derive::ValveSlot::Valve { device, .. } => device_tag_text(q, *device),
                            ew_derive::ValveSlot::Blank { .. } => "blanking plate".to_owned(),
                            ew_derive::ValveSlot::Empty => "empty".to_owned(),
                        };
                        format!("{} {what}", c.position)
                    })
                    .collect::<Vec<_>>()
                    .join(" · ");
                out.push(derived(
                    "device",
                    "valve_positions",
                    None,
                    Category::Structure,
                    FieldType::Text,
                    RowValue::One {
                        json: Value::String(text.clone()),
                        text,
                    },
                    Provenance::Derived {
                        via: "its valve positions".to_owned(),
                    },
                    Vec::new(),
                ));
            }
        }
        // The PLC a bus device belongs to ("no such field in the switch's properties"): a station names
        // its PLC, read-only; a bus device in no PLC (JSON null) is given to one by choosing it, the PLCs by ID
        // (`plc.add_to_plc`, which the window runs).
        if let [one] = devices.as_slice() {
            let row = |value, options, actions: Vec<RowAction>| {
                let partner = (!actions.is_empty()).then(|| {
                    let route = EditRoute::Command {
                        command: "plc.add_to_plc".to_owned(),
                    };
                    ("derived.plc", route, false)
                });
                derived(
                    "device",
                    "plc",
                    partner,
                    Category::Identity,
                    FieldType::Enum { options },
                    value,
                    Provenance::Derived {
                        via: "its PLC station".to_owned(),
                    },
                    actions,
                )
            };
            match crate::output::rack::plc_offer(q, *one) {
                crate::output::rack::PlcOffer::Station(name) => out.push(row(
                    RowValue::One {
                        json: Value::String(name.clone()),
                        text: name,
                    },
                    Vec::new(),
                    Vec::new(),
                )),
                crate::output::rack::PlcOffer::BusDevice => {
                    let options = q
                        .table::<ew_schema::PlcConfigProject>()
                        .iter()
                        .map(|(id, _)| id.as_uuid().to_string())
                        .collect();
                    out.push(row(
                        RowValue::One {
                            json: Value::Null,
                            text: "in no PLC".to_owned(),
                        },
                        options,
                        vec![RowAction::Edit],
                    ));
                }
                _ => {}
            }
        }
        // A fluid source's medium (the persona re-test started at the air supply and found no medium there): the medium
        // its port's net carries, with where it is defined, chosen among the project's media as a tube's is.
        if let [one] = devices.as_slice()
            && let Some(row) = source_medium_row(q, cx, *one)
        {
            out.push(row);
        }
        // Any other fluid device's ports with the medium each carries and from where.
        if let [one] = devices.as_slice() {
            out.extend(fluid_port_rows(q, *one));
        }
        // A stand-alone valve's solenoids: one row each, where its coil is drawn (a link that goes there, or
        // draws it while it is not) and the PLC output that drives it.
        if let [one] = devices.as_slice() {
            for (k, s) in crate::output::rack::solenoids(q, *one).into_iter().enumerate() {
                let via = s
                    .output
                    .as_ref()
                    .map_or_else(|| "no PLC output".to_owned(), |o| format!("PLC output {o}"));
                // A terminal valve's solenoid says where it is driven from: an output inside the terminal,
                // or a pin of a multipin terminal's plug; the window words it.
                let seat = match &s.seat {
                    None => Value::Null,
                    Some(crate::output::rack::SolenoidSeat::Terminal { terminal, channel }) => {
                        serde_json::json!({
                        "kind": "terminal", "terminal": terminal, "output": s.output,
                        "channel": channel.map(|c| c.to_string()) })
                    }
                    Some(crate::output::rack::SolenoidSeat::Multipin {
                        terminal,
                        terminal_device,
                        pin,
                        plug,
                    }) => {
                        serde_json::json!({ "kind": "multipin", "terminal": terminal, "pin": pin, "plug": plug,
                            "terminal_device": terminal_device.to_string() })
                    }
                };
                let value = RowValue::One {
                    json: serde_json::json!({
                        "name": s.name, "function": s.function.to_string(), "drawn": s.drawn, "seat": seat }),
                    text: s.drawn.clone().unwrap_or_default(),
                };
                out.push(derived(
                    "device",
                    &format!("solenoid[{k}]"),
                    None,
                    Category::Structure,
                    FieldType::Link {
                        kind: "solenoid".into(),
                    },
                    value,
                    Provenance::Derived { via },
                    Vec::new(),
                ));
            }
        }
        // A multipin valve terminal's plug: where it is drawn (a link that goes there), or that it is not, which
        // draws it on an electrical page.
        if let [one] = devices.as_slice()
            && let Some(plug) = crate::output::rack::terminal_plug(q, *one)
        {
            let value = RowValue::One {
                json: serde_json::json!({ "terminal": one.as_uuid().to_string(), "drawn": plug.drawn }),
                text: plug.drawn.clone().unwrap_or_default(),
            };
            out.push(derived(
                "device",
                "plug",
                None,
                Category::Structure,
                FieldType::Link { kind: "plug".into() },
                value,
                Provenance::Derived {
                    via: "its plug".to_owned(),
                },
                Vec::new(),
            ));
        }
        // The main location.
        let per: Vec<(Option<Value>, Provenance)> = devices
            .iter()
            .map(|d| {
                let loc = main_location(q, *d);
                (loc.map(Value::String), Session::main_location_provenance())
            })
            .collect();
        let (value, provenance) = merge(cx, &FieldType::Text, per);
        out.push(derived(
            "device",
            "main_location",
            None,
            Category::Placement,
            FieldType::Text,
            value,
            provenance,
            Vec::new(),
        ));
        // The connection points of the device's functions, one read-only row each with the label
        // the junction notation prints (`-X1:1/a`, `-102G1:−#1`), in natural order of the label; over several
        // devices the n-th rows merge.
        // One device's row names its connection point too (`function`, `idx` and whether it is marked unused), so that
        // it can be marked unused or used again (FT-1649).
        let per_device: Vec<Vec<(String, Value)>> = devices
            .iter()
            .map(|d| {
                let mut labels: Vec<(String, Value)> = q
                    .idx()
                    .functions_of_device(*d)
                    .flat_map(|function| {
                        q.get::<ew_derive::FunctionCps>(function)
                            .iter()
                            .map(|cp| {
                                let label = cp_label(
                                    q,
                                    ew_derive::CpRef {
                                        function,
                                        idx: cp.idx,
                                    },
                                );
                                let point = serde_json::json!({
                                    "label": label,
                                    "function": function.as_uuid().to_string(),
                                    "idx": cp.idx,
                                    "unused": cp.unused,
                                });
                                (label, point)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();
                labels.sort_by(|(a, _), (b, _)| ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)));
                labels
            })
            .collect();
        let rows = per_device.iter().map(Vec::len).max().unwrap_or(0);
        for n in 0..rows {
            let per: Vec<(Option<Value>, Provenance)> = per_device
                .iter()
                .map(|labels| {
                    (
                        labels.get(n).map(|(l, _)| Value::String(l.clone())),
                        Provenance::Derived {
                            via: "function definition".into(),
                        },
                    )
                })
                .collect();
            let (mut value, provenance) = merge(cx, &FieldType::Text, per);
            if let ([labels], RowValue::One { json, .. }) = (per_device.as_slice(), &mut value)
                && let Some((_, point)) = labels.get(n)
            {
                json.clone_from(point);
            }
            out.push(derived(
                "device",
                &format!("cp[{n}]"),
                None,
                Category::ConnectionPoints,
                FieldType::Text,
                value,
                provenance,
                Vec::new(),
            ));
        }
        out
    }
}
