// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC section of a new part as a form (FT-1766, doc 10 §2.6): what the part is in the PLC model, its channels,
//! the pin pattern, the supply pins, the allowed slots and a station's module limit, as texts checked one by one.
//!
//! **What it makes.** The `plc` section with the I/O range the channels occupy (a digital channel one bit, an
//! analogue one a word: `bit_of` `n` or `16n`), the pins as the part's CP pattern (the channel pins in channel order,
//! then the supply pins with their potential roles) and a black-box main function over that pattern, as the starter
//! cards carry: so a card made here gets its functions and connection points as EW-GEN-DI-1 does, and N1 and N3 give
//! its channels and their pins. A part's channels are of one kind (`ChannelSpec`): a card with inputs and outputs is
//! two parts.

use ew_derive::channel_pin;
use ew_mltext::Ident;
use ew_schema::{
    ChannelKind, ChannelSpec, CpData, IoClass, IoDir, IoRange, MapSpec, PinMap, PlcRole, PlcSection,
    PotentialType, StationLimits, SupplyGroup, SupplyKind, SupplyPins,
};
use serde_json::Value;

use crate::part_editor::{DraftMessage, enum_words, message};

/// The members of the PLC form, in its order (`part.field`'s `plc.<member>`).
pub const PLC_MEMBERS: [&str; 10] = [
    "role",
    "kind",
    "count",
    "data_type",
    "designation",
    "pin_of",
    "l_plus",
    "m",
    "slots",
    "max_modules",
];

/// The most channels the form takes for one part.
pub const MAX_CHANNELS: u16 = 256;

/// The PLC section of a draft as the texts of [`PLC_MEMBERS`]; `role` empty: the part has none.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PlcRows {
    /// `station`, `module` or empty.
    pub role: String,
    /// The channel kind (`di`, `do`, `ai` …) or empty.
    pub kind: String,
    /// The number of channels, or empty.
    pub count: String,
    /// The data type of a channel's tag ("Bool").
    pub data_type: String,
    /// The channel designation pattern over `{n}`.
    pub designation: String,
    /// The pin pattern (`{n+1}`, `X2:{n+1}`).
    pub pin_of: String,
    /// The supply's L+ pin.
    pub l_plus: String,
    /// The supply's M pin.
    pub m: String,
    /// The allowed slots (`1-8, 10`).
    pub slots: String,
    /// A station's most modules.
    pub max_modules: String,
}

impl PlcRows {
    /// The text of member `m`; `""` for another.
    pub fn text(&self, m: &str) -> &str {
        match m {
            "role" => &self.role,
            "kind" => &self.kind,
            "count" => &self.count,
            "data_type" => &self.data_type,
            "designation" => &self.designation,
            "pin_of" => &self.pin_of,
            "l_plus" => &self.l_plus,
            "m" => &self.m,
            "slots" => &self.slots,
            "max_modules" => &self.max_modules,
            _ => "",
        }
    }

    pub(crate) fn slot(&mut self, m: &str) -> Option<&mut String> {
        match m {
            "role" => Some(&mut self.role),
            "kind" => Some(&mut self.kind),
            "count" => Some(&mut self.count),
            "data_type" => Some(&mut self.data_type),
            "designation" => Some(&mut self.designation),
            "pin_of" => Some(&mut self.pin_of),
            "l_plus" => Some(&mut self.l_plus),
            "m" => Some(&mut self.m),
            "slots" => Some(&mut self.slots),
            "max_modules" => Some(&mut self.max_modules),
            _ => None,
        }
    }

    /// The rows a new part's form starts with: no section; when one is chosen, a card's usual values are offered.
    pub(crate) fn new_part() -> PlcRows {
        PlcRows {
            data_type: "Bool".to_owned(),
            designation: "{n}".to_owned(),
            pin_of: "{n+1}".to_owned(),
            ..PlcRows::default()
        }
    }

    /// The channel count, when it is a number.
    fn channel_count(&self) -> Option<u16> {
        self.count.trim().parse().ok()
    }

    /// The pin of each channel by the pin pattern: `(channel, pin)` in channel order; `None` without channels or
    /// pattern, or when the pattern gives no pin for a channel.
    pub fn pins(&self) -> Option<Vec<(u16, String)>> {
        let n = self.channel_count().filter(|n| *n > 0)?;
        let pattern = PinMap::Pattern(self.pin_of.trim().to_owned());
        if self.pin_of.trim().is_empty() {
            return None;
        }
        (0..n).map(|c| channel_pin(&pattern, c).map(|p| (c, p))).collect()
    }

    /// The I/O the channels occupy: (direction word `in` or `out`, bits); `None` for a kind without process data in
    /// the form.
    pub fn io(&self) -> Option<(&'static str, u32)> {
        let n = u32::from(self.channel_count()?);
        let (dir, per) = match self.kind.trim() {
            "di" | "counter" => ("in", 1),
            "do" => ("out", 1),
            "ai" => ("in", 16),
            "ao" => ("out", 16),
            _ => return None,
        };
        Some((dir, n * per))
    }
}

/// The PLC roles the form offers ([`PlcRole`]'s values).
pub fn plc_role_words() -> Vec<String> {
    enum_words::<PlcRole>()
}

/// The channel kinds the form offers ([`ChannelKind`]'s values).
pub fn channel_kind_words() -> Vec<String> {
    enum_words::<ChannelKind>()
}

/// `plc.<member>` as the member.
pub(crate) fn plc_field(field: &str) -> Option<&'static str> {
    let m = field.strip_prefix("plc.")?;
    PLC_MEMBERS.into_iter().find(|x| *x == m)
}

/// The allowed slots of a text such as `1-8, 10`; `None` when it does not read so.
fn slots_of(t: &str) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    for part in t.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b): (u16, u16) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
                if a > b || b - a > MAX_CHANNELS {
                    return None;
                }
                out.extend(a..=b);
            }
            None => out.push(part.parse().ok()?),
        }
    }
    out.sort_unstable();
    out.dedup();
    Some(out)
}

/// The messages of the PLC form, member by member (none without a role).
pub(crate) fn check_plc(rows: &PlcRows) -> Vec<DraftMessage> {
    let mut out = Vec::new();
    let role = rows.role.trim();
    if role.is_empty() {
        return out;
    }
    let mut say =
        |m: &str, key: &'static str, args: &[&str]| out.push(message(&format!("plc.{m}"), key, args));
    let word_ok = |t: &str, words: &[String]| words.iter().any(|w| w == t);
    if !word_ok(role, &plc_role_words()) {
        say("role", ew_commands::text_key!("error-part-plc-role"), &[role]);
    }
    let kind = rows.kind.trim();
    let count = rows.count.trim();
    if !kind.is_empty() && !word_ok(kind, &channel_kind_words()) {
        say("kind", ew_commands::text_key!("error-part-plc-kind"), &[kind]);
    }
    match count.parse::<u16>() {
        _ if count.is_empty() => {}
        Ok(n) if (1..=MAX_CHANNELS).contains(&n) => {
            if kind.is_empty() {
                say("kind", ew_commands::text_key!("error-part-plc-kind-missing"), &[]);
            }
        }
        _ => say(
            "count",
            ew_commands::text_key!("error-part-plc-count"),
            &[count, &MAX_CHANNELS.to_string()],
        ),
    }
    if !kind.is_empty() && count.is_empty() {
        say(
            "count",
            ew_commands::text_key!("error-part-plc-count-missing"),
            &[],
        );
    }
    for m in ["data_type", "l_plus", "m"] {
        let t = rows.text(m).trim();
        if !t.is_empty()
            && let Err(e) = Ident::from_input(t)
        {
            say(m, ew_commands::text_key!("error-part-value"), &[&e.to_string()]);
        }
    }
    if rows.l_plus.trim().is_empty() != rows.m.trim().is_empty() {
        let missing = if rows.l_plus.trim().is_empty() {
            "l_plus"
        } else {
            "m"
        };
        say(missing, ew_commands::text_key!("error-part-plc-supply"), &[]);
    }
    if let Some(n) = rows.channel_count().filter(|n| (1..=MAX_CHANNELS).contains(n)) {
        let pattern = rows.pin_of.trim();
        if !pattern.is_empty() {
            let map = PinMap::Pattern(pattern.to_owned());
            let mut seen: Vec<String> = Vec::new();
            for c in 0..n {
                match channel_pin(&map, c) {
                    None => {
                        say(
                            "pin_of",
                            ew_commands::text_key!("error-part-plc-pin"),
                            &[pattern, &c.to_string()],
                        );
                        break;
                    }
                    Some(p) if seen.contains(&p) => {
                        say(
                            "pin_of",
                            ew_commands::text_key!("error-part-plc-pin-twice"),
                            &[&p],
                        );
                        break;
                    }
                    Some(p) => seen.push(p),
                }
            }
        }
    }
    if slots_of(rows.slots.trim()).is_none() {
        say(
            "slots",
            ew_commands::text_key!("error-part-plc-slots"),
            &[rows.slots.trim()],
        );
    }
    let max = rows.max_modules.trim();
    if !max.is_empty() && max.parse::<u16>().map_or(true, |n| n == 0) {
        say(
            "max_modules",
            ew_commands::text_key!("error-part-plc-max"),
            &[max],
        );
    }
    out
}

/// The PLC section and the CP pattern of a checked form; `None` without a role.
pub(crate) fn plc_section(rows: &PlcRows) -> Option<(PlcSection, Vec<CpData>)> {
    let role: PlcRole = serde_json::from_value(Value::String(rows.role.trim().to_owned())).ok()?;
    let word = |t: &str| Value::String(t.trim().to_owned());
    let ident = |t: &str| (!t.trim().is_empty()).then(|| Ident::from_input(t.trim()).expect("checked"));
    let count = rows.channel_count().filter(|n| *n > 0);
    let kind: Option<ChannelKind> = count.and_then(|_| serde_json::from_value(word(&rows.kind)).ok());
    let io = rows.io().filter(|_| kind.is_some()).map(|(dir, bits)| IoRange {
        dir: if dir == "in" { IoDir::In } else { IoDir::Out },
        bits,
        class: if matches!(kind, Some(ChannelKind::Ai | ChannelKind::Ao)) {
            IoClass::Analog
        } else {
            IoClass::Digital
        },
        extra: Default::default(),
    });
    let analog = matches!(kind, Some(ChannelKind::Ai | ChannelKind::Ao));
    let pins = rows.pins().filter(|_| kind.is_some()).unwrap_or_default();
    let mut cps: Vec<CpData> = Vec::new();
    let mut add_cp = |designation: &str, role: Option<PotentialType>| {
        if cps.iter().any(|c| c.designation.as_str() == designation) {
            return;
        }
        let idx = u16::try_from(cps.len()).unwrap_or(u16::MAX);
        cps.push(CpData {
            idx,
            designation: Ident::from_input(designation).expect("checked"),
            potential_role: role,
            level: None,
            side: None,
            clamp: None,
            clamp_type: None,
            csa_min: None,
            csa_max: None,
            max_conductors: None,
            twin_ferrule: false,
            extra: Default::default(),
        });
    };
    for (_, pin) in &pins {
        add_cp(pin, None);
    }
    let supply = ident(&rows.l_plus).zip(ident(&rows.m));
    if let Some((l, m)) = &supply {
        add_cp(l.as_str(), Some(PotentialType::LPlus));
        add_cp(m.as_str(), Some(PotentialType::M));
    }
    let section = PlcSection {
        role,
        io: io.into_iter().collect(),
        channels: kind.clone().zip(count).map(|(kind, count)| ChannelSpec {
            count,
            kind,
            data_type: ident(&rows.data_type),
            designation: (!rows.designation.trim().is_empty()).then(|| rows.designation.trim().to_owned()),
            extra: Default::default(),
        }),
        bit_of: kind
            .is_some()
            .then(|| MapSpec::Expr(if analog { "16n" } else { "n" }.to_owned())),
        unbound: None,
        positions: None,
        pin_of: (!pins.is_empty()).then(|| PinMap::Pattern(rows.pin_of.trim().to_owned())),
        supply_groups: supply
            .into_iter()
            .map(|(l_plus, m)| SupplyGroup {
                kind: SupplyKind::Electronics,
                cps: SupplyPins {
                    l_plus,
                    m,
                    extra: Default::default(),
                },
                feeds: None,
                extra: Default::default(),
            })
            .collect(),
        allowed_slots: slots_of(rows.slots.trim()).unwrap_or_default(),
        limits: rows.max_modules.trim().parse().ok().map(|n| StationLimits {
            max_modules: Some(n),
            ..StationLimits::default()
        }),
        iolink: None,
        ratings: None,
        extra: Default::default(),
    };
    Some((section, cps))
}
