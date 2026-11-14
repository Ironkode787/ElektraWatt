// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The exchanged objects of an image with their field values (doc 10 §4.2, §4.4), the snapshot of the last exchange
//! and the drift between the two.
//!
//! An image is compared field by field. [`objects`] names the fields: a station's `name`, `type`, `location` and
//! `firmware`; a module's `name`, `position`, `order_number`, `type_name` and, per direction, `start.input`,
//! `bits.input`, `start.output`, `bits.output`; a channel's `type`, `io`, `number` and `bits`; a tag's `name`,
//! `data_type`, `address` (with its area letter, `Q0.1`) and `comment` (`comment.<language>` for a text with
//! languages); a bus node's `bus`, `address`, `mask`, `router`, `device_name` and `network` (the subnet's name).
//! [`class`] says which field group a field belongs to (doc 10 §2.2): the module starts, symbols and comments, and
//! the PLC's own data have an owner; every other field is the schematic's and never written by an import.

use std::collections::{BTreeMap, BTreeSet};

use ew_caex::{ExchangeImage, IoType, XItem};
use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{AnyId, ExchangeObject, ExchangeRecord, FieldGroup, ForeignId, PlcConfigProject};

use crate::image::{Projection, exchange_image};

/// One exchanged object of an image.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Object {
    /// What it is.
    pub object: ExchangeObject,
    /// Its ID in the image.
    pub id: String,
    /// Its name in words: a station's or module's name, `<module> channel <n>`, a tag's name, `<station> <label>`.
    pub name: String,
    /// The image ID of the station it belongs to (a station's own; a tag's when its channel is in the image).
    pub station: Option<String>,
    /// The image ID of what it is part of: a module's station, a channel's module, a tag's channel, a node's
    /// station.
    pub parent: Option<String>,
    /// The IEC product designation (stations and modules).
    pub designation: Option<String>,
    /// The position among its siblings: a module's slot, a channel's number.
    pub position: Option<u32>,
    /// Whether a module only holds other modules (a rack): it has items below it and neither a range nor a
    /// channel.
    pub container: bool,
    /// The exchanged values by field name.
    pub fields: BTreeMap<String, String>,
}

/// Which field group a field of an object belongs to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FieldClass {
    /// A group with an owner (doc 10 §2.2).
    Group(FieldGroup),
    /// The schematic's: structure, designations, parts and derived values. An import lists a difference and never
    /// writes it.
    Ecad,
}

/// The field group of `field` of an object of kind `object`.
pub fn class(object: &ExchangeObject, field: &str) -> FieldClass {
    let group = match (object, field) {
        (ExchangeObject::Module, "start.input" | "start.output") => FieldGroup::ModuleStarts,
        (ExchangeObject::Tag, "name" | "data_type") => FieldGroup::Symbols,
        (ExchangeObject::Tag, f) if f == "comment" || f.starts_with("comment.") => FieldGroup::Symbols,
        (ExchangeObject::Station, "firmware") => FieldGroup::PlcData,
        (ExchangeObject::Node, "address" | "mask" | "router" | "device_name") => FieldGroup::PlcData,
        _ => return FieldClass::Ecad,
    };
    FieldClass::Group(group)
}

/// The kind of an exchanged object in words (`bus node`).
pub fn object_words(object: &ExchangeObject) -> &'static str {
    match object {
        ExchangeObject::Station => "station",
        ExchangeObject::Module => "module",
        ExchangeObject::Channel => "channel",
        ExchangeObject::Tag => "tag",
        ExchangeObject::Node => "bus node",
        ExchangeObject::Instrument => "instrument",
        ExchangeObject::Unknown(_) => "object",
    }
}

/// A field of an exchanged object in words, as a preview and a refusal name it (`output start`, `comment (en-GB)`).
pub fn field_words(object: &ExchangeObject, field: &str) -> String {
    if let Some(lang) = field.strip_prefix("comment.") {
        return format!("comment ({lang})");
    }
    let words = match (object, field) {
        (ExchangeObject::Tag, "name") => "symbol",
        (_, "name") => "designation",
        (_, "data_type") => "data type",
        (_, "start.input") => "input start",
        (_, "start.output") => "output start",
        (_, "bits.input") => "input length",
        (_, "bits.output") => "output length",
        (_, "position") => "slot",
        (_, "order_number") => "order number",
        (_, "type_name") => "type name",
        (ExchangeObject::Station, "type") => "type identifier",
        (_, "io") => "direction",
        (_, "bits") => "length",
        (_, "mask") => "subnet mask",
        (_, "router") => "gateway",
        (_, "device_name") => "device name",
        (_, "network") => "subnet",
        (_, other) => other,
    };
    words.to_owned()
}

fn put(fields: &mut BTreeMap<String, String>, name: &str, value: Option<impl ToString>) {
    if let Some(v) = value.map(|v| v.to_string()).filter(|v| !v.is_empty()) {
        fields.insert(name.to_owned(), v);
    }
}

/// A tag's address with its area letter (`Q0.1`): a table's `%Q0.1` without the prefix, an AML address (`0.1`)
/// with the letter of its direction.
pub(crate) fn tag_address(io: Option<IoType>, address: &str) -> String {
    let text = address.trim().trim_start_matches('%');
    if text.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return text.to_ascii_uppercase();
    }
    let area = match io {
        Some(IoType::Input) => "I",
        Some(IoType::Output) => "Q",
        None => "",
    };
    format!("{area}{text}")
}

/// The exchanged objects of `image`: stations, their modules (at any depth) with their channels, the bus nodes,
/// then the tags, in the image's order.
pub fn objects(image: &ExchangeImage) -> Vec<Object> {
    let mut out = Vec::new();
    // the station and the words of every channel, for the tags
    let mut channels: BTreeMap<&str, (&str, String)> = BTreeMap::new();
    for station in &image.stations {
        let mut fields = BTreeMap::new();
        put(
            &mut fields,
            "name",
            station.iec.designation.as_ref().or(Some(&station.name)),
        );
        put(&mut fields, "type", station.type_identifier.as_ref());
        put(&mut fields, "location", station.iec.location.as_ref());
        put(&mut fields, "firmware", station.firmware.as_ref());
        out.push(Object {
            object: ExchangeObject::Station,
            id: station.id.clone(),
            name: station.name.clone(),
            station: Some(station.id.clone()),
            parent: None,
            designation: station.iec.designation.clone(),
            position: None,
            container: false,
            fields,
        });
        let mut stack: Vec<&XItem> = station.items.iter().rev().collect();
        while let Some(item) = stack.pop() {
            stack.extend(item.children.iter().rev());
            let mut fields = BTreeMap::new();
            put(
                &mut fields,
                "name",
                item.iec.designation.as_ref().or(Some(&item.name)),
            );
            put(&mut fields, "position", Some(item.position));
            put(&mut fields, "order_number", item.order_number.as_ref());
            put(&mut fields, "type_name", item.type_name.as_ref());
            for a in &item.addresses {
                let dir = match a.io {
                    IoType::Input => "input",
                    IoType::Output => "output",
                };
                fields
                    .entry(format!("start.{dir}"))
                    .or_insert(a.start.to_string());
                fields.entry(format!("bits.{dir}")).or_insert(a.bits.to_string());
            }
            out.push(Object {
                object: ExchangeObject::Module,
                id: item.id.clone(),
                name: item.name.clone(),
                station: Some(station.id.clone()),
                parent: Some(station.id.clone()),
                designation: item.iec.designation.clone(),
                position: Some(item.position),
                container: !item.children.is_empty() && item.addresses.is_empty() && item.channels.is_empty(),
                fields,
            });
            for c in &item.channels {
                let name = format!("{} channel {}", item.name, c.number);
                channels.insert(&c.id, (&station.id, name.clone()));
                let mut fields = BTreeMap::new();
                put(&mut fields, "type", Some(c.kind.word()));
                put(&mut fields, "io", Some(c.io.word()));
                put(&mut fields, "number", Some(c.number));
                put(&mut fields, "bits", Some(c.bits));
                out.push(Object {
                    object: ExchangeObject::Channel,
                    id: c.id.clone(),
                    name,
                    station: Some(station.id.clone()),
                    parent: Some(item.id.clone()),
                    designation: None,
                    position: Some(c.number),
                    container: false,
                    fields,
                });
            }
        }
        for interface in &station.interfaces {
            let Some(node) = &interface.node else {
                continue;
            };
            let mut fields = BTreeMap::new();
            put(&mut fields, "bus", node.bus.as_ref());
            put(&mut fields, "address", node.address.as_ref());
            put(&mut fields, "mask", node.mask.as_ref());
            put(&mut fields, "router", node.router.as_ref());
            put(&mut fields, "device_name", node.device_name.as_ref());
            let network = node
                .network
                .as_ref()
                .and_then(|id| image.networks.iter().find(|n| n.id == *id))
                .map(|n| &n.name);
            put(&mut fields, "network", network);
            out.push(Object {
                object: ExchangeObject::Node,
                id: node.id.clone(),
                name: format!("{} {}", station.name, interface.label),
                station: Some(station.id.clone()),
                parent: Some(station.id.clone()),
                designation: Some(interface.label.clone()),
                position: None,
                container: false,
                fields,
            });
        }
    }
    for tag in &image.tags {
        let mut fields = BTreeMap::new();
        put(&mut fields, "name", Some(&tag.name));
        put(&mut fields, "data_type", tag.data_type.as_ref());
        put(
            &mut fields,
            "address",
            tag.address.as_ref().map(|a| tag_address(tag.io, a)),
        );
        for (lang, text) in &tag.comment {
            let field = if lang.is_empty() {
                "comment".to_owned()
            } else {
                format!("comment.{lang}")
            };
            put(&mut fields, &field, Some(text));
        }
        let of = tag.channel.as_deref().and_then(|c| channels.get(c));
        out.push(Object {
            object: ExchangeObject::Tag,
            id: tag.id.clone(),
            name: tag.name.clone(),
            station: of.map(|(s, _)| (*s).to_owned()),
            parent: tag.channel.clone(),
            designation: None,
            position: None,
            container: false,
            fields,
        });
    }
    out
}

/// The last exchange of a configuration project, as its `exchange_record`s and `foreign_id`s hold it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Snapshot {
    /// The exchanged values per object: by the object's kind name and the record it is.
    pub records: BTreeMap<(&'static str, AnyId), BTreeMap<String, String>>,
    /// The record an ID of the exchange file was given to, with its object's kind name.
    pub foreign: BTreeMap<String, (&'static str, AnyId)>,
}

/// The snapshot of the last exchange of `cfg` (doc 10 §4.2); empty before the first exchange. A record of a kind
/// of a newer format is left out.
pub fn snapshot(q: &Q, cfg: Id<PlcConfigProject>) -> Snapshot {
    let mut out = Snapshot::default();
    for (_, r) in q.table::<ExchangeRecord>().iter() {
        if r.config_project == cfg && !r.object.name().is_empty() {
            out.records.insert((r.object.name(), r.target), r.fields.clone());
        }
    }
    for (_, r) in q.table::<ForeignId>().iter() {
        if r.config_project == cfg && !r.object.name().is_empty() {
            out.foreign.insert(r.foreign.clone(), (r.object.name(), r.target));
        }
    }
    out
}

/// What the project's exchange image has per object: the kind name and the record, with the object.
pub fn ours(projection: &Projection) -> Vec<(&'static str, AnyId, Object)> {
    objects(&projection.image)
        .into_iter()
        .filter_map(|o| {
            let (_, target) = projection.targets.get(&o.id)?;
            Some((o.object.name(), *target, o))
        })
        .collect()
}

/// The fields of configuration project `cfg` that differ from its last exchange (EW-PLC-016, FR-PLC-035): per
/// object, its kind, the record it is, its name in words and the names of the differing fields. Only what was
/// exchanged is compared: the stations of the snapshot with everything below them (an object added since or gone
/// since is reported with no field name), and every other object of the snapshot by itself. Empty before the
/// first exchange.
pub fn drift(q: &Q, cfg: Id<PlcConfigProject>) -> Vec<(ExchangeObject, AnyId, String, Vec<String>)> {
    let snap = snapshot(q, cfg);
    let mut records: BTreeMap<(&'static str, AnyId), &BTreeMap<String, String>> = snap
        .records
        .iter()
        .filter(|((kind, _), _)| *kind != ExchangeObject::Instrument.name())
        .map(|(k, v)| (*k, v))
        .collect();
    if records.is_empty() {
        return Vec::new();
    }
    let Some(projection) = exchange_image(q, cfg, None) else {
        return Vec::new();
    };
    let exchanged: BTreeSet<String> = projection
        .targets
        .iter()
        .filter(|(_, (o, t))| {
            *o == ExchangeObject::Station && records.contains_key(&(ExchangeObject::Station.name(), *t))
        })
        .map(|(id, _)| id.clone())
        .collect();
    let mut out = Vec::new();
    for (kind, target, object) in ours(&projection) {
        match records.remove(&(kind, target)) {
            Some(fields) => {
                let differing: Vec<String> = fields
                    .keys()
                    .chain(object.fields.keys())
                    .filter(|f| fields.get(*f) != object.fields.get(*f))
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                if !differing.is_empty() {
                    out.push((object.object, target, object.name, differing));
                }
            }
            None if object.station.as_ref().is_some_and(|s| exchanged.contains(s)) => {
                out.push((object.object, target, object.name, Vec::new()));
            }
            None => {}
        }
    }
    // what the snapshot holds and the project no longer has
    for ((kind, target), fields) in records {
        let object = [
            ExchangeObject::Station,
            ExchangeObject::Module,
            ExchangeObject::Channel,
            ExchangeObject::Tag,
            ExchangeObject::Node,
        ]
        .into_iter()
        .find(|o| o.name() == kind);
        if let Some(object) = object {
            let name = fields.get("name").cloned().unwrap_or_default();
            out.push((object, target, name, Vec::new()));
        }
    }
    out
}
