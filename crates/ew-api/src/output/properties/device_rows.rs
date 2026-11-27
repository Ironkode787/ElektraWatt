// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A device's derived rows: the tag, the structure per aspect, the main location and the connection
//! points.
//!
//! Not here: the provenance of the tag row (`properties.rs`); a connection's rows (`connection_rows.rs`).

use std::collections::BTreeSet;

use ew_derive::{DeviceTag, MainRep, Q, ResolvedStructure, SlotSource, StructureOf, cp_label};
use ew_ids::{Id, Uuid};
use ew_rds::{Aspect, RdsSyntax};
use ew_schema::{Device, Kind};
use serde_json::Value;

use super::DerivedRow;
use super::context::{SheetCx, member_of_aspect};
use super::fluid_rows::{fluid_port_rows, source_medium_row};
use super::sections::{Built, derived, merge};
use super::{Provenance, RowAction, RowValue, device_tag_text, folio_of, main_location, tag_provenance};
use crate::Session;
use crate::output::fields::{Category, EditRoute, FieldType};

/// The part use of `device` whose part has variants (its first), with its index and the part copy.
/// The gland of the cable `d` where it leaves a cabinet (the first cabinet by space order); `None` for a device that
/// is no cable leaving one.
pub(crate) fn cable_gland(q: &Q, d: Id<Device>) -> Option<ew_derive::layout::CableGland> {
    q.table::<ew_schema::LayoutSpace>().iter().find_map(|(space, _)| {
        q.get::<ew_derive::layout::CableGlands>(space)
            .iter()
            .find(|g| g.cable == d)
            .cloned()
    })
}

pub(crate) fn variant_use(
    q: &Q,
    device: Id<Device>,
) -> Option<(
    usize,
    ew_schema::PartUse,
    std::sync::Arc<ew_schema::LibCopy<ew_schema::Part>>,
)> {
    let dev = q.devices().get(device)?;
    dev.parts.iter().enumerate().find_map(|(i, u)| {
        let copy = q.parts().get(u.part.copy)?;
        (!copy.item.variants.is_empty()).then(|| (i, u.clone(), copy.clone()))
    })
}

/// The words of a part's variant `key` in the Variant row: "Blue · EW-GEN-TRM-1-BU", or the part's own number alone
/// (`None`, or a key the part does not hold, which EW-CORE-085 reports).
pub(crate) fn variant_label(part: &ew_schema::Part, key: Option<&str>) -> String {
    match key.and_then(|k| part.variant(k)) {
        Some(v) => format!(
            "{} · {}",
            v.name.neutral_or_first().unwrap_or(v.key.as_str()),
            part.with_variant(Some(v.key.as_str())).part_number.as_str()
        ),
        None => part.part_number.as_str().to_owned(),
    }
}

/// The device's `parts` with the variant `text` given to its part use that has variants: one of the row's words, or a
/// variant's key; the part's own number (or an empty text) clears it. Refused in words for a device without such a
/// part and for a variant the part does not have. The Variant row of Properties and the strip editor's Variant cell
/// both write it (`fields.set` of `parts`).
pub(crate) fn variant_parts(q: &Q, device: Id<Device>, text: &str) -> Result<Value, crate::Text> {
    let Some((at, _, part)) = variant_use(q, device) else {
        return Err(text!("refusal-variant-no-part"));
    };
    let key = if text.is_empty() || text == variant_label(&part.item, None) {
        None
    } else {
        let found = part.item.variants.iter().find(|v| {
            v.key.as_str() == text
                || variant_label(&part.item, Some(v.key.as_str())) == text
                || v.name.neutral_or_first() == Some(text)
                || part.item.with_variant(Some(v.key.as_str())).part_number.as_str() == text
                || v.name.languages().any(|l| v.name.get(l) == Some(text))
        });
        match found {
            Some(v) => Some(v.key.as_str().to_owned()),
            None => {
                return Err(text!(
                    "refusal-variant-unknown",
                    part.item.part_number.as_str(),
                    text
                ));
            }
        }
    };
    let Some(dev) = q.devices().get(device) else {
        return Err(text!("refusal-variant-no-part"));
    };
    let mut uses = serde_json::to_value(&dev.parts).unwrap_or(Value::Null);
    match key {
        Some(k) => uses[at]["part"]["variant"] = Value::String(k),
        None => {
            if let Some(o) = uses[at]["part"].as_object_mut() {
                o.remove("variant");
            }
        }
    }
    Ok(uses)
}

/// One accessory row of a strip's Properties ([`strip_accessory_rows`]): the row's name (`accessory.<kind>`), the
/// accessory copy the declaration names, the part counted (empty: left off) and the parts it comes with.
pub(crate) type StripAccessoryRow = (String, Id<ew_schema::Part>, String, Vec<Id<ew_schema::Part>>);

/// The accessories of a strip its terminals' parts bring, one per kind (doc 07 §5, Properties ▸ Accessories): the
/// row's name (`accessory.<kind>`), the accessory copy the declaration names, the part counted (empty: left off) and
/// the parts it comes with.
pub(crate) fn strip_accessory_rows(q: &Q, strip: Id<Device>) -> Vec<StripAccessoryRow> {
    let Some(device) = q.devices().get(strip) else {
        return Vec::new();
    };
    // The row's name is the kind's own word (`end_plate`); a kind of a newer format has no row.
    let name = |kind: &Option<ew_schema::AccessoryKind>| match kind {
        Some(ew_schema::AccessoryKind::Unknown(_)) | None => None,
        Some(k) => serde_json::to_value(k)
            .ok()
            .and_then(|v| v.as_str().map(|w| format!("accessory.{w}"))),
    };
    let mut out: Vec<StripAccessoryRow> = Vec::new();
    let mut push = |row: StripAccessoryRow| {
        if !out.iter().any(|o| o.0 == row.0) {
            out.push(row);
        }
    };
    if let Some(b) = ew_derive::strip_bracket(q, device)
        && let Some(d) = &b.declared
        && let Some(n) = name(&d.kind)
    {
        push((n, d.of, ew_derive::printed_number(q, &b.part), vec![d.with]));
    }
    for a in ew_derive::strip_accessories(q, device) {
        if let Some(n) = name(&a.kind) {
            push((n, a.of, ew_derive::printed_number(q, &a.part), a.with.clone()));
        }
    }
    // Left off: a choice without a part, of an accessory its terminals' parts name.
    for c in device.accessory_choices.iter().filter(|c| c.part.is_none()) {
        let found = q.parts().iter().find_map(|(_, copy)| {
            copy.item
                .accessories
                .iter()
                .find(|a| ew_derive::accessory_copy(a) == c.of)
                .map(|a| (a.kind.clone(), copy.id))
        });
        if let Some((kind, from)) = found
            && let Some(n) = name(&kind)
        {
            push((n, c.of, String::new(), vec![from]));
        }
    }
    out
}

/// The Variant row's choices: the part's own number, then each variant's words.
pub(crate) fn variant_options(part: &ew_schema::Part) -> Vec<String> {
    std::iter::once(variant_label(part, None))
        .chain(
            part.variants
                .iter()
                .map(|v| variant_label(part, Some(v.key.as_str()))),
        )
        .collect()
}

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
            // A device without a tag (an earth connection) shows none in its tag row; its chip says why.
            let untagged = tag
                .as_ref()
                .as_ref()
                .is_some_and(|t| t.source == ew_derive::TagSource::EarthConnection);
            let text = if untagged {
                String::new()
            } else {
                device_tag_text(q, *d)
            };
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
        // Freeze on several devices is one `fields.set_many` with each device's own product designation.
        if !all_frozen {
            actions.push(RowAction::Freeze);
        }
        out.push(derived(DerivedRow {
            kind: "device",
            name: "tag",
            partner: Some(("tag.frozen", editable("tag.frozen"), true)),
            category: Category::Identity,
            ty: FieldType::Ident,
            value,
            provenance,
            actions,
        }));
        // The structure, per aspect with a node on any target.
        let structures: Vec<std::sync::Arc<ResolvedStructure>> =
            devices.iter().map(|d| q.get::<StructureOf>(*d)).collect();
        let mut aspects: BTreeSet<Aspect> = structures.iter().flat_map(|s| s.slots.keys().copied()).collect();
        // The location is what a drafter sets first, so its row is there on every device, set or not.
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
            out.push(derived(DerivedRow {
                kind: "device",
                name: &format!("structure.{member}"),
                partner: Some((&path, editable(&path), false)),
                category: Category::Structure,
                ty: FieldType::Slot,
                value,
                provenance,
                actions,
            }));
        }
        // A valve's position on its valve terminal (`assembly.attach` owns the member, so the
        // schema's own row under Structure is read-only): one row in Main for valves and the children of a module with valve positions, typed into to move
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
            out.push(derived(DerivedRow {
                kind: "device",
                name: "position",
                partner: Some((
                    "derived.position",
                    EditRoute::Command {
                        command: "assembly.attach".to_owned(),
                    },
                    false,
                )),
                category: Category::Structure,
                ty: FieldType::Text,
                value,
                provenance,
                actions,
            }));
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
                out.push(derived(DerivedRow {
                    kind: "device",
                    name: "valve_positions",
                    partner: None,
                    category: Category::Structure,
                    ty: FieldType::Text,
                    value: RowValue::One {
                        json: Value::String(text.clone()),
                        text,
                    },
                    provenance: Provenance::Derived {
                        via: "its valve positions".to_owned(),
                    },
                    actions: Vec::new(),
                }));
            }
        }
        // The variant of one device's part (doc 07 §2.5, §5): the part's own numbers, then each variant by name and
        // number; choosing one writes it into the part use (`fields.set` of `parts`, through `Session::layout_edit`).
        if let [one] = devices.as_slice()
            && let Some((_, use_, part)) = variant_use(q, *one)
        {
            let options = variant_options(&part.item);
            let current = variant_label(&part.item, use_.part.variant.as_ref().map(|v| v.as_str()));
            out.push(derived(DerivedRow {
                kind: "device",
                name: "variant",
                partner: Some((
                    "derived.variant",
                    EditRoute::Command {
                        command: "fields.set".to_owned(),
                    },
                    false,
                )),
                category: Category::Identity,
                ty: FieldType::Enum { options },
                value: RowValue::One {
                    json: Value::String(current.clone()),
                    text: current,
                },
                provenance: Provenance::Authored,
                actions: vec![RowAction::Edit],
            }));
        }
        // A strip's accessories its terminals' parts bring: one row per kind, the part counted, typed over to take
        // another part, emptied to leave it off the strip (the strip's choice, through `Session::layout_edit`).
        if let [one] = devices.as_slice() {
            for (name, _, part, with) in strip_accessory_rows(q, *one) {
                let with: Vec<String> = with
                    .iter()
                    .filter_map(|p| q.parts().get(*p).map(|c| c.item.part_number.as_str().to_owned()))
                    .collect();
                out.push(derived(DerivedRow {
                    kind: "device",
                    name: &name,
                    partner: Some((
                        &name,
                        EditRoute::Command {
                            command: "fields.set".to_owned(),
                        },
                        false,
                    )),
                    category: Category::Part,
                    ty: FieldType::Optional(Box::new(FieldType::Text)),
                    value: RowValue::One {
                        json: Value::String(part.clone()),
                        text: part,
                    },
                    provenance: Provenance::Derived {
                        via: format!("with {}", with.join(", ")),
                    },
                    actions: vec![RowAction::Edit],
                }));
            }
        }
        // A cable's gland where it leaves its cabinet (doc 12 §3.9): its part, chosen or proposed from the cable's outer
        // diameter, typed over to choose another gland of the project and emptied to take the proposal back; and the
        // hole it is placed in, or not placed.
        if let [one] = devices.as_slice()
            && let Some(g) = cable_gland(q, *one)
        {
            let mm = |n: ew_units::Nm| crate::output::layout_edit::mm_text(q, i128::from(n.0));
            // How the gland is had, in words the window renders (a key and its arguments).
            let (part, via) = match &g.part {
                Ok(p) if g.chosen => (ew_derive::printed_number(q, p), None),
                Ok(p) => (
                    ew_derive::printed_number(q, p),
                    Some(g.od.map_or_else(
                        || text!("via-gland-proposed"),
                        |od| text!("via-gland-proposed-for", mm(od)),
                    )),
                ),
                Err(ew_derive::layout::NoGland::NoDiameter) => {
                    (String::new(), Some(text!("via-gland-no-diameter")))
                }
                Err(ew_derive::layout::NoGland::NoneSeals { od }) => {
                    (String::new(), Some(text!("via-gland-none-seals", mm(*od))))
                }
            };
            out.push(derived(DerivedRow {
                kind: "device",
                name: "gland",
                partner: Some((
                    "derived.gland",
                    EditRoute::Command {
                        command: "fields.set".to_owned(),
                    },
                    false,
                )),
                category: Category::Part,
                ty: FieldType::Optional(Box::new(FieldType::Text)),
                value: RowValue::One {
                    json: Value::String(part.clone()),
                    text: part,
                },
                provenance: via.map_or(Provenance::Authored, |via| Provenance::Told { via }),
                actions: vec![RowAction::Edit],
            }));
            let hole = g.placed.and_then(|f| {
                let feature = q.table::<ew_schema::SurfaceFeature>().get(f)?.clone();
                let no = q
                    .get::<ew_derive::layout::SurfaceFeatures>(feature.surface)
                    .iter()
                    .find(|h| {
                        h.owners
                            .iter()
                            .any(|o| matches!(o, ew_derive::layout::Owner::ByHand { feature: x } if *x == f))
                    })?
                    .no;
                let page = ew_derive::layout::surface_page(q, feature.surface).and_then(|p| folio_of(q, p));
                Some((format!("H{no}"), page))
            });
            let (text, via) = match hole {
                Some((no, Some(page))) => (no, text!("via-gland-hole-on-page", page)),
                Some((no, None)) => (no, text!("via-gland-hole-placed")),
                None => (String::new(), text!("via-gland-hole-not-placed")),
            };
            out.push(derived(DerivedRow {
                kind: "device",
                name: "gland_hole",
                partner: None,
                category: Category::Part,
                ty: FieldType::Text,
                value: RowValue::One {
                    json: Value::String(text.clone()),
                    text,
                },
                provenance: Provenance::Told { via },
                actions: Vec::new(),
            }));
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
                derived(DerivedRow {
                    kind: "device",
                    name: "plc",
                    partner,
                    category: Category::Identity,
                    ty: FieldType::Enum { options },
                    value,
                    provenance: Provenance::Derived {
                        via: "its PLC station".to_owned(),
                    },
                    actions,
                })
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
        // A fluid source's medium: the medium
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
                out.push(derived(DerivedRow {
                    kind: "device",
                    name: &format!("solenoid[{k}]"),
                    partner: None,
                    category: Category::Structure,
                    ty: FieldType::Link {
                        kind: "solenoid".into(),
                    },
                    value,
                    provenance: Provenance::Derived { via },
                    actions: Vec::new(),
                }));
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
            out.push(derived(DerivedRow {
                kind: "device",
                name: "plug",
                partner: None,
                category: Category::Structure,
                ty: FieldType::Link { kind: "plug".into() },
                value,
                provenance: Provenance::Derived {
                    via: "its plug".to_owned(),
                },
                actions: Vec::new(),
            }));
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
        out.push(derived(DerivedRow {
            kind: "device",
            name: "main_location",
            partner: None,
            category: Category::Placement,
            ty: FieldType::Text,
            value,
            provenance,
            actions: Vec::new(),
        }));
        // The connection points of the device's functions, one read-only row each with the label
        // the junction notation prints (`-X1:1/a`, `-102G1:−#1`), in natural order of the label; over several
        // devices the n-th rows merge.
        // One device's row names its connection point too (`function`, `idx` and whether it is marked unused), so that
        // it can be marked unused or used again.
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
            out.push(derived(DerivedRow {
                kind: "device",
                name: &format!("cp[{n}]"),
                partner: None,
                category: Category::ConnectionPoints,
                ty: FieldType::Text,
                value,
                provenance,
                actions: Vec::new(),
            }));
        }
        out
    }
}
