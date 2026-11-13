// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The instrument list (doc 10 §4.7, FR-PLC-050): rows of tag, description, signal type, range, field location and
//! preferred station, read from a table, and the spare channels they are given.
//!
//! [`read_rows`] takes a table's rows as texts (the header first; the host reads the CSV or the workbook) and finds
//! the columns by their header, without regard to case: `tag`; `description`; `signal type`, `signal` or `type`;
//! `range`; `field location` or `location`; `preferred station` or `station`. Only the tag and the signal type are
//! required. A signal type is `DI`, `DO`, `AI` or `AO`.
//!
//! [`assign`] gives each row, in list order, the first spare channel of its kind: of the preferred station when the
//! row names one by its tag, then of every station of the configuration project in allocation order, by slot and
//! channel number. A row without a spare channel of its kind gets none: it is an open item (EW-PRC-002).

use std::collections::BTreeSet;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Channel, ChannelKind, Device, PlcConfigProject};
use ew_derive::{DeviceTag, EffectiveState, PlcRows, Q};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The signal type of an instrument.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    /// A digital input.
    Di,
    /// A digital output.
    Do,
    /// An analogue input.
    Ai,
    /// An analogue output.
    Ao,
}

impl Signal {
    /// The signal's word (`DI`, `DO`, `AI`, `AO`).
    pub fn word(self) -> &'static str {
        match self {
            Signal::Di => "DI",
            Signal::Do => "DO",
            Signal::Ai => "AI",
            Signal::Ao => "AO",
        }
    }

    fn kind(self) -> ChannelKind {
        match self {
            Signal::Di => ChannelKind::Di,
            Signal::Do => ChannelKind::Do,
            Signal::Ai => ChannelKind::Ai,
            Signal::Ao => ChannelKind::Ao,
        }
    }
}

/// One row of an instrument list.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstrumentRow {
    /// The process tag ("TA05-V-40").
    pub tag: String,
    /// The description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The signal type.
    pub signal: Signal,
    /// The signal range ("4..20 mA").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub range: String,
    /// The field location.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub location: String,
    /// The preferred station, by its tag ("-201U1").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub station: String,
}

/// The instrument rows of a table's `rows` (the header first). A row without a tag is skipped.
///
/// # Errors
///
/// The reason the rows are no instrument list: no header, no tag or signal-type column, a signal type that is none
/// (with its row), a tag that two rows carry.
pub fn read_rows(rows: &[Vec<String>]) -> Result<Vec<InstrumentRow>, String> {
    let Some(header) = rows.first() else {
        return Err("the table is empty: an instrument list starts with the header row".to_owned());
    };
    let column = |names: &[&str]| {
        header
            .iter()
            .position(|h| names.iter().any(|n| h.trim().eq_ignore_ascii_case(n)))
    };
    let (Some(tag), Some(signal)) = (column(&["tag"]), column(&["signal type", "signal", "type"])) else {
        return Err(
            "the table has no column `tag` or no column `signal type`: it is not an instrument list"
                .to_owned(),
        );
    };
    let description = column(&["description"]);
    let range = column(&["range"]);
    let location = column(&["field location", "location"]);
    let station = column(&["preferred station", "station"]);
    let cell = |row: &[String], i: Option<usize>| -> String {
        i.and_then(|i| row.get(i))
            .map(|t| t.trim().to_owned())
            .unwrap_or_default()
    };
    let mut out: Vec<InstrumentRow> = Vec::new();
    for (n, row) in rows.iter().enumerate().skip(1) {
        let tag = cell(row, Some(tag));
        if tag.is_empty() {
            continue;
        }
        let word = cell(row, Some(signal));
        let signal = match word.to_ascii_uppercase().as_str() {
            "DI" => Signal::Di,
            "DO" | "DQ" => Signal::Do,
            "AI" => Signal::Ai,
            "AO" | "AQ" => Signal::Ao,
            _ => {
                return Err(format!(
                    "row {}: the signal type of {tag} is `{word}`; it is DI, DO, AI or AO",
                    n + 1
                ));
            }
        };
        if out.iter().any(|r| r.tag == tag) {
            return Err(format!("row {}: the tag {tag} is in the list twice", n + 1));
        }
        out.push(InstrumentRow {
            tag,
            description: cell(row, description),
            signal,
            range: cell(row, range),
            location: cell(row, location),
            station: cell(row, station),
        });
    }
    Ok(out)
}

/// The spare channel each of `rows` is given in configuration project `cfg`, in the rows' order; `None` for a row
/// without one. A channel in `taken` (one an earlier list reserved) is passed over.
pub fn assign(
    q: &Q,
    cfg: Id<PlcConfigProject>,
    rows: &[InstrumentRow],
    taken: &BTreeSet<Id<Channel>>,
) -> Vec<Option<Id<Channel>>> {
    let devices = q.devices();
    let stations: Vec<Id<Device>> = ew_derive::stations(q)
        .into_iter()
        .filter(|s| {
            devices
                .get(*s)
                .and_then(|d| d.aspects.plc_station.as_ref())
                .is_some_and(|p| p.config_project == cfg)
        })
        .collect();
    // every spare channel, in station, slot and channel order
    let spare: Vec<(Id<Device>, ChannelKind, Id<Channel>)> = stations
        .iter()
        .flat_map(|s| {
            q.get::<PlcRows>(*s)
                .iter()
                .filter(|r| r.state == EffectiveState::Spare && !taken.contains(&r.channel))
                .map(|r| (r.station, r.kind.clone(), r.channel))
                .collect::<Vec<_>>()
        })
        .collect();
    let named = |text: &str| -> Option<Id<Device>> {
        let text = text.trim();
        stations.iter().copied().find(|s| {
            q.get::<DeviceTag>(*s)
                .as_ref()
                .as_ref()
                .is_some_and(|t| t.short() == text || t.product == text || t.full == text)
        })
    };
    let mut used: BTreeSet<Id<Channel>> = BTreeSet::new();
    rows.iter()
        .map(|row| {
            let kind = row.signal.kind();
            let preferred = named(&row.station);
            let free = |station: Option<Id<Device>>| {
                spare
                    .iter()
                    .find(|(s, k, c)| *k == kind && !used.contains(c) && station.is_none_or(|p| p == *s))
                    .map(|(_, _, c)| *c)
            };
            let channel = preferred.and_then(|p| free(Some(p))).or_else(|| free(None));
            used.extend(channel);
            channel
        })
        .collect()
}
