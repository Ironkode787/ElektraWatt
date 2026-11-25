// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cable section of a cable part as a form (New cable type…): the number of cores, the cross-section, with
//! or without a protective conductor, how the cores are marked, shielded or not, as texts checked one by one, and the
//! cable as it will read (`3G1.5 · BN BU GNYE`). The outer diameter is the technical value `outer_diameter`.
//!
//! **How the cores are marked.** `hd308`: the harmonised colours of HD 308 S2 up to five cores, numbered black cores
//! beyond (the Cable form's written type, [`crate::window::written_cores`]); `numbers`: numbered black cores, the
//! green-yellow last with a protective conductor; `own`: a table of core and colour the person fills in, which starts
//! as the cores the other marking gave. A shielded cable has a shield after its cores, as the starter cables do.
//!
//! **A bus cable** (New bus cable…): `bus` names the bus systems the cable is made for (`BusSection.cable_for`).
//! Choosing a bus fills the cores as the bus has them ([`bus_cores`]: PROFIBUS A green and B red with a shield, an
//! Ethernet cable's four or eight cores by pair, …) in the own marking; a bus cable's cores need no cross-section.

use ew_schema::{BusSection, CableCore};
use ew_units::Dec3;
use serde_json::{Value, json};

use crate::Session;
use crate::part_editor::{DraftMessage, message};
use crate::window::TypeCore;

/// The members of the cable section, in the form's order (`part.field`'s `cable.<member>`; the own marking's table is
/// `cable.core.<row>` and `cable.colour.<row>`).
pub const CABLE_MEMBERS: [&str; 7] = ["bus", "cores", "csa", "pe", "marking", "shield", "diameter"];

/// The cores a cable made for the bus system `bus` (as the file spells it) has: (designation, colour codes) in pair
/// order, and whether it is shielded; `n` asks for that many cores (an Ethernet cable has four or eight), `None` for
/// the bus's usual number. `None` for a bus without such cores or another number.
pub fn bus_cores(bus: &str, n: Option<usize>) -> Option<(Vec<(&'static str, &'static str)>, bool)> {
    let (cores, shield): (&[(&str, &str)], bool) = match (bus, n) {
        ("profibus_dp", None | Some(2)) => (&[("A", "GN"), ("B", "RD")], true),
        ("profinet" | "ethercat" | "ethernet_ip" | "modbus_tcp", None | Some(4)) => (
            &[("TD+", "YE"), ("TD-", "OG"), ("RD+", "WH"), ("RD-", "BU")],
            true,
        ),
        // Eight cores as T568B pairs them: 1-2, 3-6, 5-4, 7-8.
        ("profinet" | "ethercat" | "ethernet_ip" | "modbus_tcp", Some(8)) => (
            &[
                ("1", "WHOG"),
                ("2", "OG"),
                ("3", "WHGN"),
                ("6", "GN"),
                ("5", "WHBU"),
                ("4", "BU"),
                ("7", "WHBN"),
                ("8", "BN"),
            ],
            true,
        ),
        ("canopen", None | Some(3)) => (&[("CAN_H", "WH"), ("CAN_L", "BN"), ("CAN_GND", "GN")], true),
        ("asi", None | Some(2)) => (&[("ASI+", "BN"), ("ASI-", "BU")], false),
        ("io_link", None | Some(3)) => (&[("L+", "BN"), ("CQ", "BK"), ("L-", "BU")], false),
        _ => return None,
    };
    Some((cores.to_vec(), shield))
}

/// Whether the cores of a cable for the bus systems `bus` are numbered in pairs (an Ethernet cable's twisted pairs).
fn paired(bus: &str) -> bool {
    bus.split_whitespace()
        .any(|b| matches!(b, "profinet" | "ethercat" | "ethernet_ip" | "modbus_tcp"))
}

/// How the cores of a cable are marked, as `cable.marking` spells it.
pub const CABLE_MARKINGS: [&str; 3] = ["hd308", "numbers", "own"];

/// The technical value that holds a cable's outer diameter.
const DIAMETER: &str = "outer_diameter";

/// The most cores the form takes.
const MAX_CORES: usize = 99;

/// The cable section of a draft (module documentation).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CableRows {
    /// The number of cores without the shield, as typed.
    pub cores: String,
    /// The cross-section in mm², as typed (`1.5`, `1,5`).
    pub csa: String,
    /// Whether one core is the green-yellow protective conductor.
    pub pe: bool,
    /// One of [`CABLE_MARKINGS`].
    pub marking: String,
    /// The own marking's cores: their designation and colour, as typed, one row per core.
    pub table: Vec<(String, String)>,
    /// Whether the cable has a shield.
    pub shield: bool,
    /// The bus systems the cable is made for, as the file spells them, separated by spaces; empty for none.
    pub bus: String,
}

impl CableRows {
    /// What a new cable type starts as: three cores of 1.5 mm² with a protective conductor, HD 308 colours.
    pub(crate) fn new_cable() -> CableRows {
        CableRows {
            cores: "3".to_owned(),
            csa: "1.5".to_owned(),
            pe: true,
            marking: "hd308".to_owned(),
            table: Vec::new(),
            shield: false,
            bus: String::new(),
        }
    }

    /// After the bus changed, or the count of a bus cable: the cores the bus has ([`bus_cores`]) in the own marking,
    /// without a cross-section; nothing for a bus without such cores.
    fn fill_bus_cores(&mut self, n: Option<usize>) {
        let Some(bus) = self.bus.split_whitespace().next() else {
            return;
        };
        let Some((cores, shield)) = bus_cores(bus, n) else {
            return;
        };
        self.cores = cores.len().to_string();
        self.csa = String::new();
        self.pe = false;
        self.marking = "own".to_owned();
        self.shield = shield;
        self.table = cores
            .iter()
            .map(|(c, colour)| ((*c).to_owned(), (*colour).to_owned()))
            .collect();
    }

    /// The text of member `m` of [`CABLE_MEMBERS`] (a choice as its word, `true` or `false`); `""` for another and for
    /// the diameter, which is a technical value.
    pub fn text(&self, m: &str) -> &str {
        let yes = |b: bool| if b { "true" } else { "false" };
        match m {
            "bus" => &self.bus,
            "cores" => &self.cores,
            "csa" => &self.csa,
            "pe" => yes(self.pe),
            "marking" => &self.marking,
            "shield" => yes(self.shield),
            _ => "",
        }
    }

    /// The number of cores, when it is one the form takes.
    fn count(&self) -> Option<usize> {
        self.cores
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=MAX_CORES).contains(n))
    }

    /// The cross-section, when it is a positive number of mm².
    fn size(&self) -> Option<Dec3> {
        self.csa
            .trim()
            .replace(',', ".")
            .parse::<Dec3>()
            .ok()
            .filter(|d| *d > Dec3(0))
    }

    /// Whether a cross-section is needed: always but for a bus cable's own cores, which may leave it out.
    fn needs_size(&self) -> bool {
        self.bus.trim().is_empty() || self.marking != "own"
    }

    /// The cores the marking `marking` gives (the own marking: the table's), without the shield; empty while the
    /// count or a needed cross-section does not parse.
    fn cores_of(&self, marking: &str) -> Vec<TypeCore> {
        let Some(n) = self.count() else {
            return Vec::new();
        };
        let size = self.size();
        let optional = !self.bus.trim().is_empty() && marking == "own" && self.csa.trim().is_empty();
        let Some(csa) = size.or_else(|| optional.then_some(Dec3(0))) else {
            return Vec::new();
        };
        match marking {
            "numbers" => crate::window::numbered_cores(n, self.pe, csa),
            "own" => self
                .table
                .iter()
                .map(|(core, colour)| TypeCore {
                    core: core.trim().to_owned(),
                    colour: own_colour(colour),
                    csa: size.map(|s| s.to_string()),
                })
                .collect(),
            _ => crate::window::written_cores(n, self.pe, csa),
        }
    }

    /// After the count, the protective conductor or the marking changed: the own marking's table has one row per core,
    /// a row the table lacks taken from what HD 308 gives (switching to the own marking, from what the marking before
    /// gave, `before`).
    fn fit_table(&mut self, before: &str) {
        if self.marking != "own" {
            return;
        }
        let Some(n) = self.count() else {
            return;
        };
        let from = if before == "own" { "hd308" } else { before };
        let proposed = self.cores_of(from);
        if before != "own" {
            self.table.clear();
        }
        self.table.truncate(n);
        // A row added takes a core the table does not name yet: the proposal's, else the next free number.
        let mut free = proposed
            .into_iter()
            .map(|c| (c.core, c.colour.concat()))
            .chain((1..=MAX_CORES).map(|k| (k.to_string(), String::new())));
        while self.table.len() < n {
            let Some(row) =
                free.find(|(core, _)| !self.table.iter().any(|r| r.0.trim().eq_ignore_ascii_case(core)))
            else {
                break;
            };
            self.table.push(row);
        }
    }

    /// The cable as it will read (`3G1.5 · BN BU GNYE`): the cores and the cross-section (`G` with a green-yellow
    /// core, `X` without), then the cores' designations; `None` while the count or the cross-section does not parse.
    pub fn preview(&self) -> Option<String> {
        let size = self.size().map(|s| s.to_string()).unwrap_or_default();
        let cores = self.cores_of(&self.marking);
        if cores.is_empty() {
            return None;
        }
        let green_yellow = cores.iter().any(|c| c.colour == ["GN", "YE"]);
        let names: Vec<&str> = cores.iter().map(|c| c.core.as_str()).collect();
        Some(format!(
            "{}{}{size} \u{b7} {}",
            cores.len(),
            if green_yellow { "G" } else { "X" },
            names.join(" ")
        ))
    }
}

/// An own colour as its codes: the typed text in capitals, two codes joined by `+`, `/` or nothing (`GNYE`); empty
/// for none.
fn own_colour(text: &str) -> Vec<String> {
    let t: String = text
        .trim()
        .to_uppercase()
        .chars()
        .filter(|c| !matches!(c, '+' | '/' | ' ' | '-'))
        .collect();
    if t.is_empty() {
        Vec::new()
    } else if t.len() == 4 && t.is_ascii() {
        vec![t[..2].to_owned(), t[2..].to_owned()]
    } else {
        vec![t]
    }
}

/// `cable.<member>`, `cable.core.<row>` or `cable.colour.<row>` as (member, row).
pub(crate) fn cable_field(field: &str) -> Option<(&'static str, Option<usize>)> {
    let rest = field.strip_prefix("cable.")?;
    if let Some((m, row)) = rest.split_once('.') {
        let m = ["core", "colour"].into_iter().find(|x| *x == m)?;
        return Some((m, Some(row.parse().ok()?)));
    }
    CABLE_MEMBERS.into_iter().find(|x| *x == rest).map(|m| (m, None))
}

/// The cable section of a held part's cores (`cable_cores`): the count and the common cross-section of its cores, a
/// green-yellow core as the protective conductor, the marking that gives exactly these cores (else the own marking
/// with them in its table), a shield, and the bus systems of its bus section's `cable_for`.
pub(crate) fn rows_of(cores: &[CableCore], bus: Option<&BusSection>) -> CableRows {
    let bus = bus
        .map(|b| {
            b.cable_for
                .iter()
                .map(crate::part_editor::stored_word)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    if cores.is_empty() {
        return CableRows {
            bus,
            ..CableRows::new_cable()
        };
    }
    let plain: Vec<&CableCore> = cores.iter().filter(|c| !c.shield).collect();
    let csas: Vec<String> = plain
        .iter()
        .filter_map(|c| match &c.csa {
            Some(ew_schema::Csa::Mm2(d)) => Some(d.to_string()),
            _ => None,
        })
        .collect();
    let codes = |c: &CableCore| -> Vec<String> {
        serde_json::to_value(&c.colour)
            .ok()
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default()
    };
    let mut rows = CableRows {
        cores: if plain.is_empty() {
            String::new()
        } else {
            plain.len().to_string()
        },
        csa: csas.first().cloned().unwrap_or_default(),
        pe: plain.iter().any(|c| codes(c) == ["GN", "YE"]),
        marking: "own".to_owned(),
        table: plain
            .iter()
            .map(|c| (c.core.as_str().to_owned(), codes(c).concat()))
            .collect(),
        shield: cores.iter().any(|c| c.shield),
        bus,
    };
    let held: Vec<(String, Vec<String>)> = plain
        .iter()
        .map(|c| (c.core.as_str().to_owned(), codes(c)))
        .collect();
    let same_size = csas.len() == plain.len() && csas.iter().all(|c| Some(c) == csas.first());
    if same_size
        && let Some(m) = ["hd308", "numbers"].into_iter().find(|m| {
            let made: Vec<(String, Vec<String>)> =
                rows.cores_of(m).into_iter().map(|c| (c.core, c.colour)).collect();
            made == held
        })
    {
        rows.marking = m.to_owned();
        rows.table.clear();
    }
    rows
}

impl Session {
    /// Sets member `m` (row `row` of the own marking's table) of the cable section `rows` to `text`, the table kept in
    /// step with the count and the marking; the diameter is set as the technical value `outer_diameter` of `tech`
    /// (a bare number in millimetres). Every member's message again, in the form's order.
    pub(crate) fn set_cable_field(
        &self,
        rows: &mut CableRows,
        tech: &mut Vec<(String, String)>,
        (m, row): (&str, Option<usize>),
        text: &str,
    ) -> Vec<DraftMessage> {
        let before = rows.marking.clone();
        let t = text.trim();
        match (m, row) {
            // A bus chosen fills the cores the bus has (module documentation).
            ("bus", _) => {
                rows.bus = t.to_owned();
                rows.fill_bus_cores(None);
                return self.check_cable(rows);
            }
            ("cores", _) => {
                rows.cores = text.to_owned();
                if rows.marking == "own" {
                    rows.fill_bus_cores(rows.count());
                }
            }
            ("csa", _) => rows.csa = text.to_owned(),
            ("pe", _) => rows.pe = t == "true",
            ("shield", _) => rows.shield = t == "true",
            ("marking", _) => rows.marking = t.to_owned(),
            ("diameter", _) => {
                tech.retain(|(k, _)| k != DIAMETER);
                if !t.is_empty() {
                    let mm = if t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
                        format!("{} mm", t.replace(',', "."))
                    } else {
                        t.to_owned()
                    };
                    tech.push((DIAMETER.to_owned(), mm));
                    tech.sort_by(|a, b| a.0.cmp(&b.0));
                }
            }
            ("core", Some(i)) => {
                if let Some(r) = rows.table.get_mut(i) {
                    r.0 = text.to_owned();
                }
            }
            ("colour", Some(i)) => {
                if let Some(r) = rows.table.get_mut(i) {
                    r.1 = text.to_owned();
                }
            }
            _ => {}
        }
        rows.fit_table(&before);
        self.check_cable(rows)
    }

    /// The messages of the cable section `rows`: the count a whole number from 1 to 99, the cross-section a positive
    /// number, a protective conductor only beside another core, a marking of [`CABLE_MARKINGS`]; the own marking's
    /// cores each named, once, with a colour the project knows or none.
    pub(crate) fn check_cable(&self, rows: &CableRows) -> Vec<DraftMessage> {
        let mut out = Vec::new();
        match rows.count() {
            None => out.push(message(
                "cable.cores",
                key!("error-cable-section-cores"),
                &[rows.cores.trim(), &MAX_CORES.to_string()],
            )),
            Some(1) if rows.pe => out.push(message("cable.pe", key!("error-cable-section-pe"), &[])),
            Some(_) => {}
        }
        let bus_words = crate::part_editor::bus_words();
        if let Some(word) = rows
            .bus
            .split_whitespace()
            .find(|w| !bus_words.iter().any(|b| b == w))
        {
            out.push(message("cable.bus", key!("error-part-bus-system"), &[word]));
        }
        if rows.size().is_none() && (rows.needs_size() || !rows.csa.trim().is_empty()) {
            out.push(message(
                "cable.csa",
                key!("error-cable-section-csa"),
                &[rows.csa.trim()],
            ));
        }
        if !CABLE_MARKINGS.contains(&rows.marking.as_str()) {
            out.push(message(
                "cable.marking",
                key!("error-part-value"),
                &[&rows.marking],
            ));
        }
        if rows.marking == "own" {
            for (i, (core, colour)) in rows.table.iter().enumerate() {
                let core = core.trim();
                let twice = rows.table[..i]
                    .iter()
                    .any(|(c, _)| c.trim().eq_ignore_ascii_case(core));
                if core.is_empty() || ew_mltext::Ident::from_input(core).is_err() || twice {
                    out.push(message(
                        &format!("cable.core.{i}"),
                        key!("error-cable-section-core"),
                        &[&(i + 1).to_string(), core],
                    ));
                }
                let known = self.typed_colours(colour).is_ok_and(|c| c.len() <= 1);
                if !known {
                    out.push(message(
                        &format!("cable.colour.{i}"),
                        key!("error-cable-section-colour"),
                        &[&(i + 1).to_string(), colour.trim()],
                    ));
                }
            }
        }
        out
    }

    /// The part's `cable_cores` of the cable section `rows` (module documentation); the first message when it does not
    /// parse.
    pub(crate) fn cable_cores(&self, rows: &CableRows) -> Result<Vec<CableCore>, DraftMessage> {
        if let Some(m) = self.check_cable(rows).into_iter().next() {
            return Err(m);
        }
        let own = rows.marking == "own";
        let mut cores: Vec<Value> = rows
            .cores_of(&rows.marking)
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let mut v = json!({ "core": c.core });
                // An own colour in the project's codes (a profile's colour name is its code).
                let colour = if own {
                    let typed = rows.table.get(i).map_or("", |r| r.1.as_str());
                    self.typed_colours(typed)
                        .ok()
                        .and_then(|mut t| t.pop())
                        .unwrap_or_default()
                } else {
                    c.colour
                };
                if !colour.is_empty() {
                    v["colour"] = json!(colour);
                }
                if let Some(csa) = c.csa {
                    v["csa"] = json!(csa);
                }
                // An Ethernet cable's cores come in twisted pairs, in the order the table lists them.
                if paired(&rows.bus) {
                    v["pair"] = json!(i / 2 + 1);
                }
                v
            })
            .collect();
        if rows.shield {
            cores.push(json!({ "core": "SHIELD", "shield": true }));
        }
        serde_json::from_value(Value::Array(cores))
            .map_err(|e| message("cable.cores", key!("error-part-value"), &[&e.to_string()]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three markings give the cores a panel builder expects, and the preview reads them.
    #[test]
    fn each_marking_gives_its_cores_and_reads_as_the_cable() {
        let mut rows = CableRows::new_cable();
        assert_eq!(rows.preview().as_deref(), Some("3G1.5 \u{b7} BN BU GNYE"));
        rows.marking = "numbers".to_owned();
        rows.cores = "4".to_owned();
        assert_eq!(rows.preview().as_deref(), Some("4G1.5 \u{b7} 1 2 3 GNYE"));
        rows.fit_table("numbers");
        rows.marking = "own".to_owned();
        rows.fit_table("numbers");
        assert_eq!(rows.table.len(), 4);
        // More cores: the rows added are named as no row is yet.
        rows.cores = "6".to_owned();
        rows.fit_table("own");
        let names: Vec<&str> = rows.table.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(names, ["1", "2", "3", "GNYE", "4", "5"]);
        rows.cores = "4".to_owned();
        rows.fit_table("own");
        rows.table[0].1 = "wh".to_owned();
        rows.pe = false;
        rows.table[3] = ("4".to_owned(), "BK".to_owned());
        assert_eq!(rows.preview().as_deref(), Some("4X1.5 \u{b7} 1 2 3 4"));
        rows.cores = "nine".to_owned();
        assert_eq!(rows.preview(), None);
    }
}
