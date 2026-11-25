// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus and PLC rows of the property sheet: a channel's binding and a net's master in words, a bus port's
//! address shown and read back, and a bus line's cable and plugs (doc 10 §2.7).
//!
//! Not here: the PLC typed editors (`inputs.rs`); the fluid rows (`fluid_rows.rs`).

use ew_derive::Q;
use ew_ids::{Id, Uuid};
use ew_schema::Connection;
use serde_json::Value;

use super::DerivedRow;
use super::context::SheetCx;
use super::order::function_label;
use super::sections::{Built, derived, merge};
use super::{Provenance, RowAction, RowValue, device_tag_text};
use crate::Session;
use crate::output::fields::{Category, EditRoute, FieldType, InputError, ParsedInput};

/// A channel's binding row and a logical net's master row in words: the pin as the canvas names it (`-1A2:1`), never
/// the endpoint's JSON; a value that names no CP keeps its text.
pub(super) fn binding_in_words(q: &Q, built: &mut [Built]) {
    for b in built
        .iter_mut()
        .filter(|b| b.row.path == "binding" || b.row.path == "master")
    {
        if let RowValue::One { json, text } = &mut b.row.value
            && let Ok(ew_schema::Endpoint::Cp(f, idx)) =
                serde_json::from_value::<ew_schema::Endpoint>(json.clone())
        {
            *text = ew_derive::cp_label(q, ew_derive::CpRef::new(f, idx));
        }
    }
}

/// A bus port's address row in words, as reports print it (`ew_derive::net_address_text`): `TBD` for the
/// open item, nothing for a port without an address, `3`, `cpx-1 192.168.0.10`, `pos 2 alias 1001`, `port 3`.
pub(super) fn bus_address_in_words(built: &mut [Built]) {
    for b in built.iter_mut().filter(|b| b.row.path == "bus_port.address") {
        if let RowValue::One { json, text } = &mut b.row.value
            && let Ok(address) = serde_json::from_value::<ew_schema::AddrValue>(json.clone())
        {
            *text = ew_derive::bus_address(&address).unwrap_or_default();
        }
    }
}

impl Session {
    /// The text key of the form in which the address of the bus port function `function` is typed, by its bus:
    /// `hint-address-profibus`,
    /// `-profinet`, `-ethercat`, `-io-link`, else `hint-address-other`; `None` for a function without a bus port.
    pub fn bus_address_form(&self, function: Uuid) -> Option<&'static str> {
        let q = self.query();
        let port = q.functions().get(Id::from_uuid(function))?.bus_port.clone()?;
        Some(match port.bus {
            ew_schema::BusSystem::ProfibusDp => "hint-address-profibus",
            ew_schema::BusSystem::Profinet => "hint-address-profinet",
            ew_schema::BusSystem::EtherCat => "hint-address-ethercat",
            ew_schema::BusSystem::IoLink => "hint-address-io-link",
            _ => "hint-address-other",
        })
    }
}

/// A bus port's address as its row shows it (`bus_address_in_words`), read back: `TBD`, nothing (no
/// address), a number (a PROFIBUS station address), `port <n>` (an IO-Link port), `pos <n>` and `alias <n>` (EtherCAT),
/// else a PROFINET device name with its IPv4 address and, after `mask` and `gateway`, those; `None` for a JSON object,
/// the field's own form, parsed as before.
pub(super) fn bus_address_input(text: &str) -> Option<Result<ParsedInput, InputError>> {
    let t = text.trim();
    if t.starts_with('{') || t.starts_with('"') {
        return None;
    }
    let done = |value: Value| {
        Some(Ok(ParsedInput {
            value,
            notes: Vec::new(),
        }))
    };
    let bad = || Some(Err(InputError::Invalid(text!("refusal-input-bus-address", t))));
    if t.is_empty() {
        return done(Value::String("not_applicable".to_owned()));
    }
    if t.eq_ignore_ascii_case("tbd") {
        return done(Value::String("tbd".to_owned()));
    }
    if let Ok(n) = t.parse::<u8>() {
        return done(serde_json::json!({"value": {"profibus": n}}));
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    let number = |w: Option<&&str>| w.and_then(|w| w.parse::<u16>().ok());
    match words.as_slice() {
        ["port", n] => {
            return match n.parse::<u8>() {
                Ok(n) => done(serde_json::json!({"value": {"io_link_port": n}})),
                Err(_) => bad(),
            };
        }
        [first, ..] if *first == "pos" || *first == "alias" => {
            let (mut position, mut alias) = (None, None);
            for pair in words.chunks(2) {
                match pair {
                    ["pos", _] if position.is_none() => position = number(pair.get(1)),
                    ["alias", _] if alias.is_none() => alias = number(pair.get(1)),
                    _ => return bad(),
                }
                if position.is_none() && alias.is_none() {
                    return bad();
                }
            }
            let mut e = serde_json::Map::new();
            if let Some(p) = position {
                e.insert("position".into(), p.into());
            }
            if let Some(a) = alias {
                e.insert("alias".into(), a.into());
            }
            return done(serde_json::json!({"value": {"ethercat": e}}));
        }
        _ => {}
    }
    // A PROFINET device name, its IPv4 address, then `mask <ip>` and `gateway <ip>`.
    let Ok(name) = ew_mltext::Ident::from_input(words[0]) else {
        return bad();
    };
    let mut p = serde_json::Map::new();
    p.insert("name".into(), Value::String(name.as_str().to_owned()));
    let mut rest = words[1..].iter().copied();
    while let Some(w) = rest.next() {
        let (member, ip) = match w {
            "mask" => ("mask", rest.next()),
            "gateway" => ("gateway", rest.next()),
            other => ("ip", Some(other)),
        };
        let Some(ip) = ip.and_then(|s| s.parse::<std::net::Ipv4Addr>().ok()) else {
            return bad();
        };
        if p.contains_key(member) {
            return bad();
        }
        p.insert(member.into(), Value::String(ip.to_string()));
    }
    done(serde_json::json!({"value": {"profinet": p}}))
}

/// A bus line's end as the bus lists name it: the port's device tag and its interface (`-101A1:DP`), another function
/// by its first connection point.
pub(crate) fn port_words(q: &Q, f: Id<ew_schema::Function>) -> String {
    match q
        .functions()
        .get(f)
        .and_then(|x| Some((x.device?, x.bus_port.as_ref()?.interface.clone())))
    {
        Some((d, interface)) => format!("{}:{interface}", device_tag_text(q, d)),
        None => function_label(q, f),
    }
}

/// The rows of bus lines (doc 10 §2.7): the cable that carries them (its tag) and the plugs at their ends (`-101A1:DP:
/// EW-GEN-PLG-1 · -300A1:DP: none`). Editing either opens the Cable form of the selected lines (`bus.cable`). None
/// when a target is no bus line.
pub(super) fn bus_line_rows(q: &Q, cx: &SheetCx<'_>, conns: &[Id<Connection>]) -> Vec<Built> {
    let lines: Vec<(std::sync::Arc<Connection>, ew_schema::BusLine)> = conns
        .iter()
        .filter_map(|c| {
            let conn = q.connections().get(*c)?.clone();
            match &conn.kind {
                ew_schema::ConnectionKind::Bus(l) => {
                    let l = l.clone();
                    Some((conn, l))
                }
                _ => None,
            }
        })
        .collect();
    if lines.is_empty() || lines.len() != conns.len() {
        return Vec::new();
    }
    // No cable or plug reads as a dash, in every language.
    let none = "\u{2014}".to_owned();
    let end = |e: &ew_schema::Endpoint, plug: &Option<Box<ew_schema::BusPlug>>| {
        let at = match e {
            ew_schema::Endpoint::Cp(f, _) => port_words(q, *f),
            _ => String::new(),
        };
        let part = plug
            .as_ref()
            .and_then(|p| q.parts().get(p.part.copy))
            .map_or_else(|| none.clone(), |p| p.item.part_number.to_string());
        format!("{at}: {part}")
    };
    let mut cable_per = Vec::new();
    let mut plug_per = Vec::new();
    for (conn, line) in &lines {
        let cable = line.cable.map_or_else(|| none.clone(), |c| device_tag_text(q, c));
        cable_per.push((Some(Value::String(cable)), Provenance::Authored));
        let plugs = format!(
            "{} \u{b7} {}",
            end(&conn.a, &line.plug_a),
            end(&conn.b, &line.plug_b)
        );
        plug_per.push((Some(Value::String(plugs)), Provenance::Authored));
    }
    let route = || EditRoute::Command {
        command: "bus.cable".to_owned(),
    };
    let row = |name: &str, per| {
        let (value, provenance) = merge(cx, &FieldType::Text, per);
        derived(DerivedRow {
            kind: "connection",
            name,
            partner: Some((&format!("derived.{name}"), route(), false)),
            category: Category::Part,
            ty: FieldType::Text,
            value,
            provenance,
            actions: vec![RowAction::Edit],
        })
    };
    vec![row("bus_cable", cable_per), row("bus_plugs", plug_per)]
}
