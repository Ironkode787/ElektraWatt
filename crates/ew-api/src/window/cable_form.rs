// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Cable form below the shell (FT-1685): the selected wires as a cable's cores, in one step.
//!
//! - **The type** is what a person writes: `3G1.5` (three cores, the last green-yellow), `4x0.75` (no green-yellow),
//!   `2 x 1,5 mm²`, or a cable part's number ([`Session::layout_part_offers`]). A written type's cores follow the
//!   harmonised colours of HD 308 S2 up to five cores (`2x`: BN BU; `3x`: BN BK GY; `4x`: BU BN BK GY; `2G`: BN
//!   GNYE; `3G`: BN BU GNYE; `4G`: BN BK GY GNYE; `5G`: BN BK GY BU GNYE), and are numbered black cores beyond, the
//!   green-yellow last; each core takes the written cross-section. A part's cores are the part's, as New cable…
//!   makes them.
//! - **The rows** are the wires with the core each gets ([`Session::cable_form`]): the cores of the cable one of the
//!   wires already belongs to, then the other wires; a wire keeps its core, a new one takes the type's next free core.
//!   A row whose core is emptied leaves the cable.
//! - **Apply** ([`Session::cable_apply`]): a new cable is one `cable.create` whose conductors list their wires (one
//!   transaction, one undo step); an existing cable is one `fields.set_many` (its tag, its cores' cross-section and
//!   colour where the type changes them, the wires that change core or leave) and, for cores the cable does not have
//!   yet, one `conductor.add` with their wires.

use std::collections::BTreeSet;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{ColourSpec, Conductor, Connection, ConnectionKind, Csa, Device};
use ew_derive::ew_units::Dec3;
use serde_json::{Value, json};

use crate::output::Uuid;
use crate::{AnyCommand, Session};

/// One core of a cable type: its identification, its colour codes and its cross-section (the file spelling of a
/// `Csa`, `1.5`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TypeCore {
    /// The core's identification (`BN`, `1`).
    pub core: String,
    /// Its colour codes (one or two); empty when the type gives none.
    pub colour: Vec<String>,
    /// Its cross-section, when the type gives one.
    pub csa: Option<String>,
}

/// What the Cable form shows for a selection ([`Session::cable_form`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CableForm {
    /// The cable one of the wires is a core of, if any (the form then edits it).
    pub cable: Option<Uuid>,
    /// That cable's tag as printed, else empty.
    pub tag: String,
    /// That cable's type in writing (`3G1.5`), else a proposal for the wires (`2x1.5`).
    pub type_text: String,
    /// The wires and the core each has or gets, in order.
    pub rows: Vec<(Uuid, String)>,
}

/// Why [`Session::cable_apply`] compiles no command: a text key and the texts it names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CableRefused {
    /// `error-cable-type` (the typed type), `error-cable-core-unknown` (the core, the type's cores) or
    /// `error-cable-core-twice` (the core).
    pub key: &'static str,
    /// The texts the key's pattern takes, in order.
    pub args: Vec<String>,
}

/// The written type `text` (module documentation): its core count, whether it has a green-yellow core and its
/// cross-section; `None` when it is not a type.
fn written_type(text: &str) -> Option<(usize, bool, Dec3)> {
    let t: String = text
        .trim()
        .to_lowercase()
        .replace('×', "x")
        .replace(',', ".")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let t = t
        .strip_suffix("mm²")
        .or_else(|| t.strip_suffix("mm2"))
        .unwrap_or(&t);
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    let count: usize = t.get(..digits)?.parse().ok()?;
    let pe = match t.get(digits..=digits)? {
        "g" => true,
        "x" => false,
        _ => return None,
    };
    let csa: Dec3 = t.get(digits + 1..)?.parse().ok()?;
    ((1..=99).contains(&count) && csa > Dec3(0) && (count > 1 || !pe)).then_some((count, pe, csa))
}

/// The cores of a written type (module documentation).
fn written_cores(count: usize, pe: bool, csa: Dec3) -> Vec<TypeCore> {
    let coloured: &[&str] = match (count, pe) {
        (2, false) => &["BN", "BU"],
        (3, false) => &["BN", "BK", "GY"],
        (4, false) => &["BU", "BN", "BK", "GY"],
        (2, true) => &["BN", "GNYE"],
        (3, true) => &["BN", "BU", "GNYE"],
        (4, true) => &["BN", "BK", "GY", "GNYE"],
        (5, true) => &["BN", "BK", "GY", "BU", "GNYE"],
        _ => &[],
    };
    let csa = Some(csa.to_string());
    let colour_of = |code: &str| {
        if code == "GNYE" {
            vec!["GN".to_owned(), "YE".to_owned()]
        } else {
            vec![code.to_owned()]
        }
    };
    if !coloured.is_empty() {
        return coloured
            .iter()
            .map(|c| TypeCore {
                core: (*c).to_owned(),
                colour: colour_of(c),
                csa: csa.clone(),
            })
            .collect();
    }
    let numbered = count - usize::from(pe);
    let mut cores: Vec<TypeCore> = (1..=numbered)
        .map(|n| TypeCore {
            core: n.to_string(),
            colour: vec!["BK".to_owned()],
            csa: csa.clone(),
        })
        .collect();
    if pe {
        cores.push(TypeCore {
            core: "GNYE".to_owned(),
            colour: colour_of("GNYE"),
            csa,
        });
    }
    cores
}

/// The colour codes of a colour specification in their file spelling.
fn codes(c: &ColourSpec) -> Vec<String> {
    serde_json::to_value(c)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// The file spelling of a cross-section in mm² (`1.5`); `None` for another unit.
fn mm2(csa: &Csa) -> Option<String> {
    match csa {
        Csa::Mm2(d) => Some(d.to_string()),
        _ => None,
    }
}

impl Session {
    /// The page the connection `wire` is drawn on, if any.
    pub fn wire_page(&self, wire: Uuid) -> Option<Uuid> {
        let page = self
            .query()
            .connections()
            .get(Id::<Connection>::from_uuid(wire))?
            .page?;
        Some(page.as_uuid())
    }

    /// The cable whose core the connection `wire` is, if any.
    pub fn cable_of_wire(&self, wire: Uuid) -> Option<Uuid> {
        let q = self.query();
        let ConnectionKind::Core(core) = q.connections().get(Id::from_uuid(wire))?.kind else {
            return None;
        };
        q.conductors().get(core).map(|c| c.cable.as_uuid())
    }

    /// The cores of the cable type `text` (module documentation), with the part's copy ID when `text` names a cable
    /// part; `None` when it is neither a written type nor a cable part's number.
    pub fn cable_type(&self, text: &str) -> Option<(Vec<TypeCore>, Option<Uuid>)> {
        if let Some((count, pe, csa)) = written_type(text) {
            return Some((written_cores(count, pe, csa), None));
        }
        let offer = self
            .layout_part_offers("cable")
            .into_iter()
            .find(|o| o.number.eq_ignore_ascii_case(text.trim()))?;
        let command = self.create_cable(offer.part)?;
        let cores = command
            .params
            .get("conductors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|c| c.get("shield") != Some(&Value::Bool(true)))
            .map(|c| TypeCore {
                core: c["core"].as_str().unwrap_or_default().to_owned(),
                colour: c["colour"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                csa: c["csa"].as_str().map(str::to_owned),
            })
            .collect();
        Some((cores, Some(offer.part)))
    }

    /// The conductors of `cable` in core order (natural order of the core).
    fn conductors_of(&self, cable: Uuid) -> Vec<Conductor> {
        let q = self.query();
        let id: Id<Device> = Id::from_uuid(cable);
        let mut out: Vec<Conductor> = q
            .conductors()
            .iter()
            .filter(|(_, c)| c.cable == id)
            .map(|(_, c)| c.as_ref().clone())
            .collect();
        out.sort_by(|a, b| a.core.natural_cmp(&b.core));
        out
    }

    /// The type of `cable` in writing, from its conductors: their count, `G` with a green-yellow core else `x`, and
    /// their cross-section when they share one (`3G1.5`, `4x`).
    pub fn cable_type_text(&self, cable: Uuid) -> String {
        let cores: Vec<Conductor> = self
            .conductors_of(cable)
            .into_iter()
            .filter(|c| !c.shield)
            .collect();
        let pe = cores
            .iter()
            .any(|c| c.colour.as_ref().map(codes) == Some(vec!["GN".to_owned(), "YE".to_owned()]));
        let csas: BTreeSet<Option<String>> = cores.iter().map(|c| c.csa.as_ref().and_then(mm2)).collect();
        let csa = match csas.into_iter().collect::<Vec<_>>().as_slice() {
            [Some(one)] => one.clone(),
            _ => String::new(),
        };
        format!("{}{}{csa}", cores.len(), if pe { "G" } else { "x" })
    }

    /// The Cable form of the wires `wires` (module documentation). Without a cable among them the type proposed is
    /// `<n>x<cross-section>` with the first wire's effective cross-section in mm² (1.5 when it has none).
    pub fn cable_form(&self, wires: &[Uuid]) -> CableForm {
        let q = self.query();
        let cable = wires.iter().find_map(|w| self.cable_of_wire(*w));
        let (tag, type_text, mut rows) = match cable {
            Some(c) => {
                let mut rows: Vec<(Uuid, String)> = Vec::new();
                for conductor in self.conductors_of(c) {
                    let mut on: Vec<Uuid> = q
                        .connections()
                        .iter()
                        .filter(|(_, x)| x.kind == ConnectionKind::Core(conductor.id))
                        .map(|(id, _)| id.as_uuid())
                        .collect();
                    on.sort();
                    rows.extend(on.into_iter().map(|w| (w, conductor.core.as_str().to_owned())));
                }
                (
                    self.device_tag(c).unwrap_or_default(),
                    self.cable_type_text(c),
                    rows,
                )
            }
            None => {
                let csa = wires
                    .first()
                    .and_then(|w| {
                        q.get::<ew_derive::ConnWireProps>(Id::from_uuid(*w))
                            .first()
                            .and_then(|(_, e)| e.csa.as_ref().and_then(|p| mm2(&p.value)))
                    })
                    .unwrap_or_else(|| "1.5".to_owned());
                (String::new(), format!("{}x{csa}", wires.len()), Vec::new())
            }
        };
        for w in wires {
            if !rows.iter().any(|(r, _)| r == w) {
                rows.push((*w, String::new()));
            }
        }
        let cores = self.cable_type(&type_text).map(|(c, _)| c).unwrap_or_default();
        CableForm {
            cable,
            tag,
            type_text,
            rows: assign_cores(&cores, &rows),
        }
    }

    /// The command or commands of the Cable form (module documentation): the cable `cable` (none: a new one), the tag
    /// typed when it is to be frozen (its parsed `tag.frozen` value), the type `type_text` and the rows (a wire and its
    /// core; an empty core leaves the cable). Nothing to change gives no command.
    ///
    /// # Errors
    ///
    /// [`CableRefused`] for a type that is neither written nor a cable part's number, a core that is neither the
    /// type's nor the cable's, or one core given to two wires.
    pub fn cable_apply(
        &self,
        cable: Option<Uuid>,
        frozen_tag: Option<Value>,
        type_text: &str,
        rows: &[(Uuid, String)],
    ) -> Result<Vec<AnyCommand>, CableRefused> {
        let refused = |key: &'static str, args: Vec<String>| CableRefused { key, args };
        let Some((cores, part)) = self.cable_type(type_text) else {
            return Err(refused(
                ew_commands::text_key!("error-cable-type"),
                vec![type_text.trim().to_owned()],
            ));
        };
        let existing: Vec<Conductor> = cable.map(|c| self.conductors_of(c)).unwrap_or_default();
        let known = |core: &str| {
            cores.iter().any(|c| c.core == core) || existing.iter().any(|c| c.core.as_str() == core)
        };
        let mut seen = BTreeSet::new();
        for (_, core) in rows.iter().filter(|(_, core)| !core.is_empty()) {
            if !known(core) {
                let all: Vec<&str> = cores.iter().map(|c| c.core.as_str()).collect();
                return Err(refused(
                    ew_commands::text_key!("error-cable-core-unknown"),
                    vec![core.clone(), all.join(", ")],
                ));
            }
            if !seen.insert(core.clone()) {
                return Err(refused(
                    ew_commands::text_key!("error-cable-core-twice"),
                    vec![core.clone()],
                ));
            }
        }
        let wires_of = |core: &str| -> Vec<String> {
            rows.iter()
                .filter(|(_, c)| c == core)
                .map(|(w, _)| w.to_string())
                .collect()
        };
        let spec = |c: &TypeCore| {
            let mut s = json!({ "core": c.core, "wires": wires_of(&c.core) });
            if !c.colour.is_empty() {
                s["colour"] = json!(c.colour);
            }
            if let Some(csa) = &c.csa {
                s["csa"] = json!(csa);
            }
            s
        };
        let Some(cable) = cable else {
            let mut params = match part.and_then(|p| self.create_cable(p)) {
                Some(command) => command.params,
                None => json!({}),
            };
            params["conductors"] = Value::Array(cores.iter().map(spec).collect());
            if let Some(tag) = frozen_tag {
                params["frozen_tag"] = tag;
            }
            return Ok(vec![AnyCommand {
                name: "cable.create".into(),
                params,
            }]);
        };
        let q = self.query();
        let mut edits: Vec<Value> = Vec::new();
        let edit = |kind: &str, target: String, path: &str, value: Value| json!({"kind": kind, "targets": [target], "path": path, "value": value});
        if let Some(tag) = frozen_tag {
            edits.push(edit("device", cable.to_string(), "tag.frozen", tag));
        }
        // The type's cross-section and colour on the cores the cable has.
        for c in &existing {
            let Some(t) = cores.iter().find(|t| t.core == c.core.as_str()) else {
                continue;
            };
            if t.csa.is_some() && t.csa != c.csa.as_ref().and_then(mm2) {
                edits.push(edit("conductor", c.id.to_string(), "csa", json!(t.csa)));
            }
            if c.colour.is_none() && !t.colour.is_empty() {
                edits.push(edit("conductor", c.id.to_string(), "colour", json!(t.colour)));
            }
        }
        // The wires that change core or leave the cable.
        let mut added: Vec<Value> = Vec::new();
        for (wire, core) in rows {
            let now = q
                .connections()
                .get(Id::<Connection>::from_uuid(*wire))
                .map(|c| c.kind.clone());
            let target = if core.is_empty() {
                match now {
                    Some(ConnectionKind::Core(x)) if existing.iter().any(|c| c.id == x) => {
                        ConnectionKind::Wire
                    }
                    _ => continue,
                }
            } else if let Some(c) = existing.iter().find(|c| c.core.as_str() == core) {
                ConnectionKind::Core(c.id)
            } else {
                if let Some(t) = cores.iter().find(|t| t.core == *core) {
                    added.push(spec(t));
                }
                continue;
            };
            if now.as_ref() != Some(&target) {
                let value = serde_json::to_value(&target).unwrap_or_default();
                edits.push(edit("connection", wire.to_string(), "kind", value));
            }
        }
        let mut out = Vec::new();
        if !added.is_empty() {
            out.push(AnyCommand {
                name: "conductor.add".into(),
                params: json!({ "cable": cable.to_string(), "conductors": added }),
            });
        }
        if !edits.is_empty() {
            out.push(AnyCommand {
                name: "fields.set_many".into(),
                params: json!({ "edits": edits }),
            });
        }
        Ok(out)
    }
}

/// The rows with their cores: a row keeps a core of `cores` it has, and a row without one takes the next of `cores`
/// no row has; with none left it stays without.
pub fn assign_cores(cores: &[TypeCore], rows: &[(Uuid, String)]) -> Vec<(Uuid, String)> {
    let mut taken: BTreeSet<String> = rows
        .iter()
        .filter(|(_, c)| cores.iter().any(|t| t.core == *c))
        .map(|(_, c)| c.clone())
        .collect();
    rows.iter()
        .map(|(w, c)| {
            if taken.contains(c) {
                return (*w, c.clone());
            }
            let next = cores
                .iter()
                .find(|t| !taken.contains(&t.core))
                .map(|t| t.core.clone())
                .unwrap_or_default();
            taken.insert(next.clone());
            (*w, next)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cores(text: &str) -> Vec<String> {
        let (n, pe, csa) = written_type(text).unwrap();
        written_cores(n, pe, csa).into_iter().map(|c| c.core).collect()
    }

    #[test]
    fn a_written_type_gives_the_harmonised_cores() {
        assert_eq!(cores("3G1.5"), ["BN", "BU", "GNYE"]);
        assert_eq!(cores("2 x 1,5 mm²"), ["BN", "BU"]);
        assert_eq!(cores("2g1.5"), ["BN", "GNYE"]);
        assert_eq!(cores("4x0.75"), ["BU", "BN", "BK", "GY"]);
        assert_eq!(cores("7G1.5"), ["1", "2", "3", "4", "5", "6", "GNYE"]);
        assert_eq!(
            written_type("3G1.5").map(|t| t.2.to_string()),
            Some("1.5".to_owned())
        );
        for not in ["", "G1.5", "3G", "3Y1.5", "0x1.5", "1G1.5", "3x0"] {
            assert_eq!(written_type(not), None, "{not}");
        }
    }

    #[test]
    fn rows_keep_their_core_and_new_ones_take_the_next_free() {
        let t: Vec<TypeCore> = written_cores(3, true, Dec3(1_500));
        let (a, b, c) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
        let rows = vec![(a, "BU".to_owned()), (b, String::new()), (c, String::new())];
        assert_eq!(
            assign_cores(&t, &rows),
            [(a, "BU".to_owned()), (b, "BN".to_owned()), (c, "GNYE".to_owned())]
        );
    }
}
