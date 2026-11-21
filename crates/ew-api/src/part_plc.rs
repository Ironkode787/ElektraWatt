// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC section of a new part as a form (FT-1766, doc 10 §2.6): what the part is in the PLC model, its channels,
//! the pin pattern, the supply pins, the allowed slots and a station's module limit, as texts checked one by one.
//!
//! **What it makes.** The `plc` section with the I/O range the channels occupy (a digital channel one bit, an
//! analogue one a word: `bit_of` `n` or `16n`), the pins as the part's CP pattern (the channel pins in channel order,
//! then the supply pins with their potential roles) and a black-box main function over that pattern, as the starter
//! cards carry: so a card made here gets its functions and connection points as EW-GEN-DI-1 does, and N1 and N3 give
//! its channels and their pins. A card with inputs and outputs has a second kind of channels the other way (FT-1766):
//! its channels follow the first kind's in number (`PlcSection::more_channels`), each kind's from bit 0 of its
//! direction's range (`bit_of` a list), its pins by the one pin pattern over every channel.

use ew_derive::channel_pin;
use ew_mltext::Ident;
use ew_schema::{
    ChannelKind, ChannelSpec, CpData, IoClass, IoDir, IoRange, MapSpec, PinMap, PlcRole, PlcSection,
    PotentialType, StationLimits, SupplyGroup, SupplyKind, SupplyPins,
};
use serde_json::Value;

use crate::part_editor::{DraftMessage, enum_words, message, stored_word};

/// The members of the PLC form, in its order (`part.field`'s `plc.<member>`).
pub const PLC_MEMBERS: [&str; 12] = [
    "role",
    "kind",
    "count",
    "kind2",
    "count2",
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
    /// The kind of a card's channels the other way (`do` beside `di`), or empty.
    pub kind2: String,
    /// The number of those channels, or empty.
    pub count2: String,
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
            "kind2" => &self.kind2,
            "count2" => &self.count2,
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
            "kind2" => Some(&mut self.kind2),
            "count2" => Some(&mut self.count2),
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

    /// The count of the second kind's channels, when a second kind is named and its count is a number.
    fn second_count(&self) -> Option<u16> {
        (!self.kind2.trim().is_empty())
            .then(|| self.count2.trim().parse().ok())
            .flatten()
    }

    /// The channels of both kinds together.
    fn all_channels(&self) -> Option<u16> {
        let n = self.channel_count()?;
        Some(n.saturating_add(self.second_count().unwrap_or(0)))
    }

    /// The pin of each channel by the pin pattern: `(channel, pin)` in channel order, both kinds'; `None` without
    /// channels or pattern, or when the pattern gives no pin for a channel.
    pub fn pins(&self) -> Option<Vec<(u16, String)>> {
        let n = self.all_channels().filter(|n| *n > 0)?;
        let pattern = PinMap::Pattern(self.pin_of.trim().to_owned());
        if self.pin_of.trim().is_empty() {
            return None;
        }
        (0..n).map(|c| channel_pin(&pattern, c).map(|p| (c, p))).collect()
    }

    /// The I/O the channels occupy: (direction word `in` or `out`, bits), the first kind's, then the second kind's;
    /// empty when a kind has no process data in the form.
    pub fn io(&self) -> Vec<(&'static str, u32)> {
        let mut out = Vec::new();
        for (kind, count) in [
            (self.kind.trim(), self.channel_count()),
            (self.kind2.trim(), self.second_count()),
        ] {
            if kind.is_empty() && out.len() == 1 {
                break;
            }
            let (Some((dir, per)), Some(n)) = (way(kind), count) else {
                return Vec::new();
            };
            out.push((dir, u32::from(n) * per));
        }
        out
    }
}

/// The direction of the channels of kind `kind` and the bits each takes in its range (`di` in, one bit; `ao` out, a
/// word); `None` for a kind without process data in the form.
fn way(kind: &str) -> Option<(&'static str, u32)> {
    Some(match kind {
        "di" | "counter" => ("in", 1),
        "do" => ("out", 1),
        "ai" => ("in", 16),
        "ao" => ("out", 16),
        _ => return None,
    })
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
    // A second kind of channels (FT-1766): checked as the first, and going the other way.
    let (kind2, count2) = (rows.kind2.trim(), rows.count2.trim());
    if !kind2.is_empty() && !word_ok(kind2, &channel_kind_words()) {
        say("kind2", ew_commands::text_key!("error-part-plc-kind"), &[kind2]);
    }
    match count2.parse::<u16>() {
        _ if count2.is_empty() => {}
        Ok(n) if (1..=MAX_CHANNELS).contains(&n) => {
            if kind2.is_empty() {
                say("kind2", ew_commands::text_key!("error-part-plc-kind-missing"), &[]);
            }
        }
        _ => say(
            "count2",
            ew_commands::text_key!("error-part-plc-count"),
            &[count2, &MAX_CHANNELS.to_string()],
        ),
    }
    if !kind2.is_empty() && count2.is_empty() {
        say(
            "count2",
            ew_commands::text_key!("error-part-plc-count-missing"),
            &[],
        );
    }
    if !kind2.is_empty() && word_ok(kind2, &channel_kind_words()) {
        match (way(kind), way(kind2)) {
            (Some((a, _)), Some((b, _))) if a != b => {}
            _ => say("kind2", ew_commands::text_key!("error-part-plc-kind2-way"), &[]),
        }
    }
    if rows.all_channels().is_some_and(|n| n > MAX_CHANNELS) {
        say(
            "count2",
            ew_commands::text_key!("error-part-plc-count"),
            &[count2, &MAX_CHANNELS.to_string()],
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
    if let Some(n) = rows.all_channels().filter(|n| (1..=MAX_CHANNELS).contains(n)) {
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

/// The form's texts of a held PLC section (the members it shows; a pin list, which the form cannot show as a
/// pattern, leaves the pattern empty and is kept).
pub(crate) fn rows_of(s: &PlcSection) -> PlcRows {
    let channels = s.channels.as_ref();
    let second = s.more_channels.first();
    let supply = s.supply_groups.first();
    PlcRows {
        role: stored_word(&s.role),
        kind: channels.map_or_else(String::new, |c| stored_word(&c.kind)),
        count: channels.map_or_else(String::new, |c| c.count.to_string()),
        kind2: second.map_or_else(String::new, |c| stored_word(&c.kind)),
        count2: second.map_or_else(String::new, |c| c.count.to_string()),
        data_type: channels
            .and_then(|c| c.data_type.as_ref())
            .map_or_else(String::new, |d| d.as_str().to_owned()),
        designation: channels.and_then(|c| c.designation.clone()).unwrap_or_default(),
        pin_of: match &s.pin_of {
            Some(PinMap::Pattern(p)) => p.clone(),
            _ => String::new(),
        },
        l_plus: supply.map_or_else(String::new, |g| g.cps.l_plus.as_str().to_owned()),
        m: supply.map_or_else(String::new, |g| g.cps.m.as_str().to_owned()),
        slots: slots_text(&s.allowed_slots),
        max_modules: s
            .limits
            .as_ref()
            .and_then(|l| l.max_modules)
            .map_or_else(String::new, |n| n.to_string()),
    }
}

/// Allowed slots as the form writes them: runs as `1-8`, single slots alone (`1-8, 10`).
fn slots_text(slots: &[u16]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < slots.len() {
        let mut j = i;
        while j + 1 < slots.len() && slots[j + 1] == slots[j] + 1 {
            j += 1;
        }
        out.push(if j > i {
            format!("{}-{}", slots[i], slots[j])
        } else {
            slots[i].to_string()
        });
        i = j + 1;
    }
    out.join(", ")
}

/// Whether two forms give the same pins: the same role, channels, pin pattern and supply pins.
pub(crate) fn same_pins(a: &PlcRows, b: &PlcRows) -> bool {
    ["role", "kind", "count", "kind2", "count2", "pin_of", "l_plus", "m"]
        .iter()
        .all(|m| a.text(m).trim() == b.text(m).trim())
}

/// Keeps in a section made from the form what the held section (`held`) has and the form does not show: the unbound
/// state, the valve positions, the IO-Link window, the ratings, unknown members, a station's I/O lengths, the further
/// supply groups, a pin list, and the I/O and channel map of a channel kind the form does not derive.
pub(crate) fn keep_unshown(s: &mut PlcSection, held: Option<&PlcSection>, rows: &PlcRows) {
    let Some(o) = held else {
        return;
    };
    s.unbound.clone_from(&o.unbound);
    s.positions.clone_from(&o.positions);
    s.iolink.clone_from(&o.iolink);
    s.ratings.clone_from(&o.ratings);
    s.extra.clone_from(&o.extra);
    if let Some(old) = &o.limits {
        let mut l = s.limits.clone().unwrap_or_default();
        l.max_input_bytes = old.max_input_bytes;
        l.max_output_bytes = old.max_output_bytes;
        l.extra.clone_from(&old.extra);
        s.limits = (l != StationLimits::default()).then_some(l);
    }
    if s.supply_groups.len() <= 1 {
        s.supply_groups.extend(o.supply_groups.iter().skip(1).cloned());
    }
    if rows.pin_of.trim().is_empty() && matches!(o.pin_of, Some(PinMap::List(_))) {
        s.pin_of.clone_from(&o.pin_of);
    }
    if rows.io().is_empty() {
        s.io.clone_from(&o.io);
        s.bit_of.clone_from(&o.bit_of);
    }
    if let (Some(c), Some(old)) = (s.channels.as_mut(), o.channels.as_ref()) {
        c.extra.clone_from(&old.extra);
    }
    if let (Some(c), Some(old)) = (s.more_channels.first_mut(), o.more_channels.first()) {
        c.extra.clone_from(&old.extra);
    }
}

/// The PLC section and the CP pattern of a checked form; `None` without a role.
pub(crate) fn plc_section(rows: &PlcRows) -> Option<(PlcSection, Vec<CpData>)> {
    // No role is no section (an empty word reads as a role of a newer format, which a form never makes).
    if rows.role.trim().is_empty() {
        return None;
    }
    let role: PlcRole = serde_json::from_value(Value::String(rows.role.trim().to_owned())).ok()?;
    let word = |t: &str| Value::String(t.trim().to_owned());
    let ident = |t: &str| (!t.trim().is_empty()).then(|| Ident::from_input(t.trim()).expect("checked"));
    let count = rows.channel_count().filter(|n| *n > 0);
    let kind: Option<ChannelKind> = count.and_then(|_| serde_json::from_value(word(&rows.kind)).ok());
    // A card's second kind of channels, the other way (FT-1766).
    let kind2: Option<ChannelKind> = kind
        .as_ref()
        .and(rows.second_count().filter(|n| *n > 0))
        .and_then(|_| serde_json::from_value(word(&rows.kind2)).ok());
    let analogue = |k: &Option<ChannelKind>| matches!(k, Some(ChannelKind::Ai | ChannelKind::Ao));
    let classes = [analogue(&kind), analogue(&kind2)];
    let io: Vec<IoRange> = rows
        .io()
        .into_iter()
        .zip(classes)
        .filter(|_| kind.is_some())
        .map(|((dir, bits), analog)| IoRange {
            dir: if dir == "in" { IoDir::In } else { IoDir::Out },
            bits,
            class: if analog {
                IoClass::Analog
            } else {
                IoClass::Digital
            },
            extra: Default::default(),
        })
        .collect();
    let analog = analogue(&kind);
    // One kind's channels lie on bit `n` (a word each, `16n`); a second kind's, the other way, from bit 0 of its own
    // range, so the map is a list.
    let bit_of = match (&kind, &kind2, count, rows.second_count()) {
        (Some(_), Some(_), Some(n), Some(m)) => {
            let per = |a: bool| if a { 16 } else { 1 };
            let first = (0..u32::from(n)).map(|k| k * per(analog));
            let second = (0..u32::from(m)).map(|k| k * per(analogue(&kind2)));
            Some(MapSpec::List(first.chain(second).collect()))
        }
        (Some(_), ..) => Some(MapSpec::Expr(if analog { "16n" } else { "n" }.to_owned())),
        _ => None,
    };
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
    let designation = (!rows.designation.trim().is_empty()).then(|| rows.designation.trim().to_owned());
    let section = PlcSection {
        role,
        io,
        channels: kind.clone().zip(count).map(|(kind, count)| ChannelSpec {
            count,
            kind,
            data_type: ident(&rows.data_type),
            designation: designation.clone(),
            extra: Default::default(),
        }),
        bit_of,
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
        // The second kind takes the form's data type where it is of the first kind's class (digital or analogue),
        // and the one name pattern, over its own channels from 0.
        more_channels: kind2
            .clone()
            .zip(rows.second_count())
            .map(|(k, count)| ChannelSpec {
                count,
                data_type: ident(&rows.data_type).filter(|_| analogue(&kind2) == analog),
                kind: k,
                designation,
                extra: Default::default(),
            })
            .into_iter()
            .collect(),
        extra: Default::default(),
    };
    Some((section, cps))
}
