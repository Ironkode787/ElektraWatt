// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC section of a new part as a form (doc 10 §2.6): what the part is in the PLC model, its channels,
//! the pin pattern, the supply pins, the allowed slots and a station's module limit, as texts checked one by one.
//!
//! **What it makes.** The `plc` section with the I/O range the channels occupy (a digital channel one bit, an
//! analogue one a word: `bit_of` `n` or `16n`), the pins as the part's CP pattern (the channel pins in channel order,
//! then the supply pins with their potential roles) and a black-box main function over that pattern, as the starter
//! cards carry: so a card made here gets its functions and connection points as EW-GEN-DI-1 does, and N1 and N3 give
//! its channels and their pins. A card with inputs and outputs has a second kind of channels the other way:
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
    /// The supply's L+ pins, a list split at commas and spaces ([`PlcRows::supply_pairs`]).
    pub l_plus: String,
    /// The supply's M pins, a list as the L+ pins'.
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

    /// The pin names member `m` (`l_plus` or `m`) lists, split at commas and spaces.
    fn supply_list(&self, m: &str) -> Vec<&str> {
        self.text(m)
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
            .collect()
    }

    /// The supply groups the form gives, each as its (L+ pin, M pin): the pins of the two lists by their places, or
    /// the one pin of a list with each pin of the other (two L+ pins over one M); empty without supply pins, `None`
    /// when the lists do not pair so (two L+ pins and three M pins).
    pub fn supply_pairs(&self) -> Option<Vec<(&str, &str)>> {
        let (l, m) = (self.supply_list("l_plus"), self.supply_list("m"));
        match (l.len(), m.len()) {
            (0, _) | (_, 0) => Some(Vec::new()),
            (a, b) if a == b => Some(l.into_iter().zip(m).collect()),
            (_, 1) => Some(l.into_iter().map(|x| (x, m[0])).collect()),
            (1, _) => Some(m.into_iter().map(|y| (l[0], y)).collect()),
            _ => None,
        }
    }

    /// The supply pins the part gets, in the order of its pins: each group's L+ pin, then its M pin, each pin once.
    pub fn supply_pins(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for (l, m) in self.supply_pairs().unwrap_or_default() {
            for pin in [l, m] {
                if !out.contains(&pin) {
                    out.push(pin);
                }
            }
        }
        out
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
        say("role", key!("error-part-plc-role"), &[role]);
    }
    let kind = rows.kind.trim();
    let count = rows.count.trim();
    if !kind.is_empty() && !word_ok(kind, &channel_kind_words()) {
        say("kind", key!("error-part-plc-kind"), &[kind]);
    }
    match count.parse::<u16>() {
        _ if count.is_empty() => {}
        Ok(n) if (1..=MAX_CHANNELS).contains(&n) => {
            if kind.is_empty() {
                say("kind", key!("error-part-plc-kind-missing"), &[]);
            }
        }
        _ => say(
            "count",
            key!("error-part-plc-count"),
            &[count, &MAX_CHANNELS.to_string()],
        ),
    }
    if !kind.is_empty() && count.is_empty() {
        say("count", key!("error-part-plc-count-missing"), &[]);
    }
    // A second kind of channels: checked as the first, and going the other way.
    let (kind2, count2) = (rows.kind2.trim(), rows.count2.trim());
    if !kind2.is_empty() && !word_ok(kind2, &channel_kind_words()) {
        say("kind2", key!("error-part-plc-kind"), &[kind2]);
    }
    match count2.parse::<u16>() {
        _ if count2.is_empty() => {}
        Ok(n) if (1..=MAX_CHANNELS).contains(&n) => {
            if kind2.is_empty() {
                say("kind2", key!("error-part-plc-kind-missing"), &[]);
            }
        }
        _ => say(
            "count2",
            key!("error-part-plc-count"),
            &[count2, &MAX_CHANNELS.to_string()],
        ),
    }
    if !kind2.is_empty() && count2.is_empty() {
        say("count2", key!("error-part-plc-count-missing"), &[]);
    }
    // Either way, only not the first kind again.
    if !kind2.is_empty() && word_ok(kind2, &channel_kind_words()) {
        match (way(kind), way(kind2)) {
            (Some(_), Some(_)) if kind2 != kind => {}
            _ => say("kind2", key!("error-part-plc-kind2-way"), &[]),
        }
    }
    if rows.all_channels().is_some_and(|n| n > MAX_CHANNELS) {
        say(
            "count2",
            key!("error-part-plc-count"),
            &[count2, &MAX_CHANNELS.to_string()],
        );
    }
    let t = rows.data_type.trim();
    if !t.is_empty()
        && let Err(e) = Ident::from_input(t)
    {
        say("data_type", key!("error-part-value"), &[&e.to_string()]);
    }
    // The supply pins are lists: each name a pin of its own.
    let (l_plus, m) = (rows.supply_list("l_plus"), rows.supply_list("m"));
    for (member, pins) in [("l_plus", &l_plus), ("m", &m)] {
        if let Some(e) = pins.iter().find_map(|p| Ident::from_input(p).err()) {
            say(member, key!("error-part-value"), &[&e.to_string()]);
        }
    }
    if l_plus.is_empty() != m.is_empty() {
        let missing = if l_plus.is_empty() { "l_plus" } else { "m" };
        say(missing, key!("error-part-plc-supply"), &[]);
    } else if rows.supply_pairs().is_none() {
        say(
            "m",
            key!("error-part-plc-supply-pairs"),
            &[&l_plus.len().to_string(), &m.len().to_string()],
        );
    }
    if let Some(n) = rows.all_channels().filter(|n| (1..=MAX_CHANNELS).contains(n)) {
        let pattern = rows.pin_of.trim();
        if !pattern.is_empty() {
            let map = PinMap::Pattern(pattern.to_owned());
            let mut seen: Vec<String> = Vec::new();
            for c in 0..n {
                match channel_pin(&map, c) {
                    None => {
                        say("pin_of", key!("error-part-plc-pin"), &[pattern, &c.to_string()]);
                        break;
                    }
                    Some(p) if seen.contains(&p) => {
                        say("pin_of", key!("error-part-plc-pin-twice"), &[&p]);
                        break;
                    }
                    Some(p) => seen.push(p),
                }
            }
        }
    }
    if slots_of(rows.slots.trim()).is_none() {
        say("slots", key!("error-part-plc-slots"), &[rows.slots.trim()]);
    }
    let max = rows.max_modules.trim();
    if !max.is_empty() && max.parse::<u16>().map_or(true, |n| n == 0) {
        say("max_modules", key!("error-part-plc-max"), &[max]);
    }
    out
}

/// The form's texts of a held PLC section (the members it shows; a pin list, which the form cannot show as a
/// pattern, leaves the pattern empty and is kept).
pub(crate) fn rows_of(s: &PlcSection) -> PlcRows {
    let channels = s.channels.as_ref();
    let second = s.more_channels.first();
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
        l_plus: supply_text(&s.supply_groups, |p| &p.l_plus),
        m: supply_text(&s.supply_groups, |p| &p.m),
        slots: slots_text(&s.allowed_slots),
        max_modules: s
            .limits
            .as_ref()
            .and_then(|l| l.max_modules)
            .map_or_else(String::new, |n| n.to_string()),
    }
}

/// One kind of supply pin of every group (`pin` picks it), as the form's list: the groups' pins in order, joined by
/// commas, or the one pin alone when every group has it, so that [`PlcRows::supply_pairs`] gives the groups back.
fn supply_text(groups: &[SupplyGroup], pin: impl Fn(&SupplyPins) -> &Ident) -> String {
    let pins: Vec<&str> = groups.iter().map(|g| pin(&g.cps).as_str()).collect();
    match pins.as_slice() {
        [first, ..] if pins.iter().all(|p| p == first) => (*first).to_owned(),
        _ => pins.join(", "),
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
    [
        "role", "kind", "count", "kind2", "count2", "pin_of", "l_plus", "m",
    ]
    .iter()
    .all(|m| a.text(m).trim() == b.text(m).trim())
}

/// Keeps in a section made from the form what the held section (`held`) has and the form does not show: the unbound
/// state, the valve positions, the IO-Link window, the ratings, unknown members, a station's I/O lengths, what a supply
/// group of the same pins supplies and feeds, a pin list, and the I/O and channel map of a channel kind the form does not
/// derive.
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
    // Every group is in the form; a group of the same pins keeps what it supplies and the channels it feeds.
    for g in &mut s.supply_groups {
        if let Some(old) = o
            .supply_groups
            .iter()
            .find(|x| x.cps.l_plus == g.cps.l_plus && x.cps.m == g.cps.m)
        {
            g.kind.clone_from(&old.kind);
            g.feeds.clone_from(&old.feeds);
            g.extra.clone_from(&old.extra);
            g.cps.extra.clone_from(&old.cps.extra);
        }
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
    // A card's second kind of channels, the other way.
    let kind2: Option<ChannelKind> = kind
        .as_ref()
        .and(rows.second_count().filter(|n| *n > 0))
        .and_then(|_| serde_json::from_value(word(&rows.kind2)).ok());
    let analogue = |k: &Option<ChannelKind>| matches!(k, Some(ChannelKind::Ai | ChannelKind::Ao));
    let classes = [analogue(&kind), analogue(&kind2)];
    let ways = rows.io();
    // A second kind the same way lies in its direction's one range after the first kind's bits, from the next word;
    // the range is of the first kind's class (doc 10 §2.6). The other way, from bit 0 of its own range.
    let same_way = kind2.is_some() && matches!(ways.as_slice(), [(a, _), (b, _)] if a == b);
    let word = |bits: u32| bits.div_ceil(16).saturating_mul(16);
    let second_at = if same_way { word(ways[0].1) } else { 0 };
    let ranges: Vec<((&str, u32), bool)> = if same_way {
        vec![((ways[0].0, word(second_at.saturating_add(ways[1].1))), classes[0])]
    } else {
        ways.into_iter().zip(classes).collect()
    };
    let io: Vec<IoRange> = ranges
        .into_iter()
        .filter(|_| kind.is_some())
        .map(|((dir, bits), analog)| IoRange {
            dir: if dir == "in" { IoDir::In } else { IoDir::Out },
            bits,
            class: if analog { IoClass::Analog } else { IoClass::Digital },
            extra: Default::default(),
        })
        .collect();
    let analog = analogue(&kind);
    // One kind's channels lie on bit `n` (a word each, `16n`); a second kind's from `second_at`, so the map is a list.
    let bit_of = match (&kind, &kind2, count, rows.second_count()) {
        (Some(_), Some(_), Some(n), Some(m)) => {
            let per = |a: bool| if a { 16 } else { 1 };
            let first = (0..u32::from(n)).map(|k| k * per(analog));
            let second = (0..u32::from(m)).map(|k| second_at + k * per(analogue(&kind2)));
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
    // A supply group of each pair of the lists, its pins after the channels' pins, each pin once.
    let supply: Vec<(Ident, Ident)> = rows
        .supply_pairs()
        .unwrap_or_default()
        .into_iter()
        .map(|(l, m)| {
            (
                Ident::from_input(l).expect("checked"),
                Ident::from_input(m).expect("checked"),
            )
        })
        .collect();
    for (l, m) in &supply {
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

#[cfg(test)]
mod supply_tests {
    use super::*;

    /// A card of four inputs with the supply pins `l_plus` and `m` as typed.
    fn card(l_plus: &str, m: &str) -> PlcRows {
        PlcRows {
            role: "module".to_owned(),
            kind: "di".to_owned(),
            count: "4".to_owned(),
            l_plus: l_plus.to_owned(),
            m: m.to_owned(),
            ..PlcRows::new_part()
        }
    }

    /// The (L+, M) pins of each supply group of a section.
    fn groups(s: &PlcSection) -> Vec<(String, String)> {
        s.supply_groups
            .iter()
            .map(|g| (g.cps.l_plus.as_str().to_owned(), g.cps.m.as_str().to_owned()))
            .collect()
    }

    /// A supply pin field is a list, by commas or spaces; each L+ pin becomes a pin and a group, with the M
    /// pin of its place or the one M pin of them all, and the form reads the groups back as it was typed.
    #[test]
    fn a_supply_field_takes_a_list_and_makes_a_pin_of_each() {
        let rows = card("1L+, 2L+", "M");
        assert_eq!(check_plc(&rows), Vec::new());
        let (section, cps) = plc_section(&rows).expect("a section");
        let pair = |l: &str, m: &str| (l.to_owned(), m.to_owned());
        assert_eq!(groups(&section), vec![pair("1L+", "M"), pair("2L+", "M")]);
        let names: Vec<&str> = cps.iter().map(|c| c.designation.as_str()).collect();
        assert_eq!(names[4..], ["1L+", "M", "2L+"], "{names:?}");
        let back = rows_of(&section);
        assert_eq!((back.l_plus.as_str(), back.m.as_str()), ("1L+, 2L+", "M"));
        // By places, spaces as commas.
        let (section, _) = plc_section(&card("1L+ 2L+", "1M 2M")).expect("a section");
        assert_eq!(groups(&section), vec![pair("1L+", "1M"), pair("2L+", "2M")]);
    }

    /// Lists that do not pair are refused at the M pin in words, never made into one pin named with the comma.
    #[test]
    fn supply_lists_that_do_not_pair_are_refused() {
        let messages = check_plc(&card("L+, L+2", "M, M2, M3"));
        let keys: Vec<(&str, &str)> = messages.iter().map(|m| (m.field.as_str(), m.key)).collect();
        assert_eq!(keys, [("plc.m", "error-part-plc-supply-pairs")]);
    }
}
