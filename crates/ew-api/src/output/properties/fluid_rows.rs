// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid rows of the property sheet (doc 11 §7): a fluid line's derived rows, a source's medium, a fluid
//! device's ports and media, and the session's fluid sources and media in words.
//!
//! Not here: the fluid navigators (`output/navigators/fluid.rs`); the bus rows (`bus_rows.rs`).

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{Q, cp_label};
use ew_model::EndpointKey;
use ew_schema::{Connection, Device, Function};
use serde_json::Value;

use super::context::{SheetCx, neutral_or_first};
use super::sections::{Built, derived, merge};
use super::{Provenance, RowAction, RowValue, SourceLink, device_tag_text, rep_location};
use crate::Session;
use crate::output::fields::{Category, EditRoute, FieldType};

/// A fluid line's derived rows (doc 11 §7, the persona round: the medium was found only through the command search):
/// **Medium**, the medium its net carries, with where it is defined, chosen among the project's media (one
/// `fluid.medium.define` at the line's end through [`Session::layout_edit`]); **Tube**, its effective tube
/// specification (`6×4 mm PU`) with the source of its values (the project's default tube, its medium's, its tube part's,
/// its own override; doc 11 §5.4), read-only.
pub(super) fn fluid_line_rows(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid]) -> Vec<Built> {
    use ew_derive::ew_schema::{Medium, MediumDef};
    use ew_derive::{FluidNets, TubePropsOf, TubeSource};
    let media = q.table::<Medium>();
    let name = |m: Id<Medium>| {
        media
            .get(m)
            .map(|m| neutral_or_first(&m.name))
            .unwrap_or_default()
    };
    let nets = q.get::<FluidNets>(());
    let net_of = |at: Option<EndpointKey>| at.and_then(|at| nets.net_of(at)).map(|n| n.id);
    let mut medium_per = Vec::new();
    let mut tube_per = Vec::new();
    for t in targets {
        let id: Id<Connection> = Id::from_uuid(*t);
        let props = q.get::<TubePropsOf>(id);
        let net = q
            .connections()
            .get(id)
            .and_then(|c| net_of(EndpointKey::of(&c.a)));
        // The medium and where it is defined: a definition in the line's own net, else the valves carry it there.
        let medium = props.medium.as_ref().map(ew_derive::MediumKey::root);
        let def = q
            .table::<MediumDef>()
            .iter()
            .map(|(_, d)| d)
            .find(|d| net.is_some() && net_of(EndpointKey::of(&d.at)) == net)
            .and_then(|d| EndpointKey::of(&d.at).and_then(ew_derive::CpRef::of))
            .map(|cp| cp_label(q, cp));
        let via = match (medium, def) {
            (Some(_), Some(at)) => format!("medium definition at {at}"),
            (Some(_), None) => "carried through the valves".to_owned(),
            (None, _) => "no medium definition".to_owned(),
        };
        medium_per.push((
            medium.map(|m| Value::String(m.as_uuid().to_string())),
            Provenance::Derived { via },
        ));
        // The effective tube and the source of its size and material.
        let size = props.size_text();
        let text = [
            (!size.is_empty()).then(|| format!("{size} mm")),
            props.material.as_ref().map(|m| m.value.code().to_owned()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
        let mut sources: Vec<String> = Vec::new();
        for s in [
            &props.od.as_ref().map(|v| &v.source),
            &props.id.as_ref().map(|v| &v.source),
            &props.material.as_ref().map(|v| &v.source),
        ]
        .into_iter()
        .flatten()
        {
            let words = match s {
                TubeSource::Project => "project default tube".to_owned(),
                TubeSource::Medium(m) => format!("medium '{}'", name(*m)),
                TubeSource::Part(p) => format!(
                    "tube part {}",
                    q.parts()
                        .get(*p)
                        .map(|c| c.item.part_number.as_str().to_owned())
                        .unwrap_or_default()
                ),
                TubeSource::Override => "override".to_owned(),
            };
            if !sources.contains(&words) {
                sources.push(words);
            }
        }
        let provenance = if sources.is_empty() {
            Provenance::Derived {
                via: "no tube specification".to_owned(),
            }
        } else {
            Provenance::Derived {
                via: sources.join(" · "),
            }
        };
        tube_per.push(((!text.is_empty()).then_some(Value::String(text)), provenance));
    }
    let options: Vec<String> = media.iter().map(|(id, _)| id.as_uuid().to_string()).collect();
    // Tubes of one medium defined at different ports read as that medium with each place it is defined at, not as
    // "varies" (the re-test: two tubes set at once read "varies (2)").
    let mut vias: Vec<String> = Vec::new();
    for (_, p) in &medium_per {
        if let Provenance::Derived { via } = p
            && !vias.contains(via)
        {
            vias.push(via.clone());
        }
    }
    if vias.len() > 1 && medium_per.windows(2).all(|w| w[0].0 == w[1].0) {
        let via = vias.join(" · ");
        for (_, p) in &mut medium_per {
            *p = Provenance::Derived { via: via.clone() };
        }
    }
    let (medium_value, medium_provenance) = merge(cx, &FieldType::Text, medium_per);
    // The medium's name stands for its ID.
    let medium_value = match medium_value {
        RowValue::One { json, .. } => {
            let text = json
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .map(|u| name(Id::from_uuid(u)))
                .unwrap_or_default();
            RowValue::One { json, text }
        }
        other => other,
    };
    let medium = derived(
        "connection",
        "medium",
        Some((
            "derived.medium",
            EditRoute::Command {
                command: "fluid.medium.define".to_owned(),
            },
            false,
        )),
        Category::Part,
        FieldType::Enum { options },
        medium_value,
        medium_provenance,
        vec![RowAction::Edit],
    );
    let (tube_value, tube_provenance) = merge(cx, &FieldType::Text, tube_per);
    let tube = derived(
        "connection",
        "tube",
        None,
        Category::Part,
        FieldType::Text,
        tube_value,
        tube_provenance,
        Vec::new(),
    );
    vec![medium, tube]
}

/// The fluid port of a source device, by what it is (rc.2, F9: a source drawn in the symbol editor or made in the part
/// editor is one too, whatever its category): a device whose only fluid port is a supply or plain port (no consumer's,
/// no sensing port), on a function that is no interruption point; `None` for any other device.
pub(crate) fn source_port(q: &Q, device: Id<Device>) -> Option<(Id<ew_derive::ew_schema::Function>, u16)> {
    use ew_derive::ew_schema::{ContactKind, FluidPortRole};
    let (functions, fndefs) = (q.functions(), q.fndefs());
    let mut ports = q.idx().functions_of_device(device).flat_map(|f| {
        let ip = functions
            .get(f)
            .and_then(|x| fndefs.get(x.def))
            .is_some_and(|d| d.item.contact_kind == ContactKind::Terminal);
        q.get::<ew_derive::FunctionCps>(f)
            .iter()
            .filter(|c| ew_derive::is_fluid(&c.discipline))
            .map(|c| (f, c.idx, ip, c.port_role.clone()))
            .collect::<Vec<_>>()
    });
    let (f, idx, ip, role) = ports.next()?;
    let plain = matches!(role, None | Some(FluidPortRole::Generic | FluidPortRole::Supply1));
    (ports.next().is_none() && plain && !ip).then_some((f, idx))
}

/// The medium the fluid net at `at` carries (one medium, else none) and the connection point of the medium definition
/// in that net (`None`: it is carried there through the valves, or there is none).
fn medium_at(q: &Q, at: EndpointKey) -> (Option<Id<ew_derive::ew_schema::Medium>>, Option<ew_derive::CpRef>) {
    use ew_derive::ew_schema::MediumDef;
    use ew_derive::{Effective, MediumZones, node_net};
    let net = node_net(q, at);
    let medium = match net.map(|n| q.get::<MediumZones>(()).effective(n)) {
        Some(Effective::One(k)) => Some(k.root()),
        _ => None,
    };
    let def = q
        .table::<MediumDef>()
        .iter()
        .map(|(_, d)| d)
        .find(|d| net.is_some() && EndpointKey::of(&d.at).and_then(|k| node_net(q, k)) == net)
        .and_then(|d| EndpointKey::of(&d.at).and_then(ew_derive::CpRef::of));
    (medium, def)
}

/// A source device's **Medium** row ([`source_port`]): the medium its port's net carries and where it is defined, set
/// as a tube's (`fluid.medium.define` at the port through [`Session::layout_edit`]); `None` for any other device.
/// Without a medium its chip says to choose one (rc.2, F9).
pub(super) fn source_medium_row(q: &Q, cx: &SheetCx<'_>, device: Id<Device>) -> Option<Built> {
    use ew_derive::ew_schema::Medium;
    let (f, idx) = source_port(q, device)?;
    let (medium, def) = medium_at(q, EndpointKey::Cp(f, idx));
    let via = match (medium, def) {
        (Some(_), Some(at)) => format!("medium definition at {}", cp_label(q, at)),
        (Some(_), None) => "carried through the valves".to_owned(),
        (None, _) => "no medium yet".to_owned(),
    };
    let media = q.table::<Medium>();
    let options: Vec<String> = media.iter().map(|(id, _)| id.as_uuid().to_string()).collect();
    // A medium given to this source at its own port is its own (the rc.2 re-test: the row said "derived" for the
    // medium just chosen); one that reaches it from elsewhere is derived.
    let own = def.is_some_and(|at| at.function == f && at.idx == idx);
    let provenance = if own {
        Provenance::Authored
    } else {
        Provenance::Derived { via }
    };
    let per = vec![(medium.map(|m| Value::String(m.as_uuid().to_string())), provenance)];
    let (value, provenance) = merge(cx, &FieldType::Text, per);
    let value = match value {
        // As the list offered it: its name and its pressure ("Compressed air, 6 bar").
        RowValue::One { json, .. } => {
            let text = medium
                .and_then(|m| media.get(m))
                .map(|m| medium_words(m))
                .unwrap_or_default();
            RowValue::One { json, text }
        }
        other => other,
    };
    Some(derived(
        "device",
        "medium",
        Some((
            "derived.medium",
            EditRoute::Command {
                command: "fluid.medium.define".to_owned(),
            },
            false,
        )),
        Category::Part,
        FieldType::Enum { options },
        value,
        provenance,
        vec![RowAction::Edit],
    ))
}

/// The media of a fluid device, for its menu (rc.2, F9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeviceMedia {
    /// Whether it is a source: its Medium row and menu set the medium.
    pub source: bool,
    /// Each distinct medium its fluid ports carry, in port order, with its words (`Compressed air, 6 bar`); `None` for
    /// a port that carries none.
    pub carried: Vec<Option<(Uuid, String)>>,
}

impl Session {
    /// The media of the fluid device `device` ([`DeviceMedia`]); `None` for a device without a fluid port.
    pub fn device_media(&self, device: Uuid) -> Option<DeviceMedia> {
        let q = self.query();
        let id = Id::from_uuid(device);
        let media = q.table::<ew_derive::ew_schema::Medium>();
        let mut ports: Vec<(String, EndpointKey)> = q
            .idx()
            .functions_of_device(id)
            .flat_map(|f| {
                q.get::<ew_derive::FunctionCps>(f)
                    .iter()
                    .filter(|c| ew_derive::is_fluid(&c.discipline))
                    .map(|c| (c.designation.clone(), EndpointKey::Cp(f, c.idx)))
                    .collect::<Vec<_>>()
            })
            .collect();
        if ports.is_empty() {
            return None;
        }
        ports.sort_by(|a, b| ew_mltext::natural_cmp(&a.0, &b.0));
        let mut carried = Vec::new();
        for (_, at) in ports {
            let one = medium_at(&q, at)
                .0
                .and_then(|m| media.get(m).map(|x| (m.as_uuid(), medium_words(x))));
            if !carried.contains(&one) {
                carried.push(one);
            }
        }
        Some(DeviceMedia {
            source: source_port(&q, id).is_some(),
            carried,
        })
    }

    /// The fluid sources of the project that are drawn (the air supplies, where a medium is chosen; rc.2): each its tag
    /// and where its first drawing stands (`201-B4`), in natural order of the tag.
    pub fn fluid_sources(&self) -> Vec<(String, String)> {
        let q = self.query();
        let mut out: Vec<(String, String)> = q
            .devices()
            .iter()
            .map(|(id, _)| id)
            .filter(|d| source_port(&q, *d).is_some())
            .filter_map(|d| {
                let rep = q
                    .idx()
                    .functions_of_device(d)
                    .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
                    .min()?;
                Some((device_tag_text(&q, d), rep_location(&q, rep)?))
            })
            .collect();
        out.sort_by(|a, b| ew_mltext::natural_cmp(&a.0, &b.0));
        out
    }

    /// The medium `medium` in words ([`medium_words`]); `None` for an ID that names none.
    pub fn medium_text(&self, medium: Uuid) -> Option<String> {
        let q = self.query();
        let media = q.table::<ew_derive::ew_schema::Medium>();
        media.get(Id::from_uuid(medium)).map(|m| medium_words(m))
    }
}

/// A medium in words: its name and its nominal pressure (`Compressed air, 6 bar`).
pub(crate) fn medium_words(m: &ew_derive::ew_schema::Medium) -> String {
    let name = neutral_or_first(&m.name);
    if m.p_nom == ew_derive::ew_units::Dec3::ZERO {
        name
    } else {
        format!("{name}, {} bar", m.p_nom)
    }
}

/// The fluid ports of a device that is no source (rc.2, F9), one read-only row each (`port[n]`, in port order): the
/// medium the port carries in words and where it is defined, linked to the Medium row of the source whose definition
/// gives that medium; the value names the port's designation and the medium (`null` for none, which the window writes
/// as what to do).
pub(super) fn fluid_port_rows(q: &Q, device: Id<Device>) -> Vec<Built> {
    use ew_derive::ew_schema::{Medium, MediumDef};
    if source_port(q, device).is_some() {
        return Vec::new();
    }
    let (media, defs, functions) = (q.table::<Medium>(), q.table::<MediumDef>(), q.functions());
    let mut ports: Vec<(Id<Function>, u16, String)> = q
        .idx()
        .functions_of_device(device)
        .flat_map(|f| {
            q.get::<ew_derive::FunctionCps>(f)
                .iter()
                .filter(|c| ew_derive::is_fluid(&c.discipline))
                .map(|c| (f, c.idx, c.designation.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    ports.sort_by(|a, b| ew_mltext::natural_cmp(&a.2, &b.2).then_with(|| (a.0, a.1).cmp(&(b.0, b.1))));
    ports
        .into_iter()
        .enumerate()
        .map(|(n, (f, idx, designation))| {
            let (medium, def) = medium_at(q, EndpointKey::Cp(f, idx));
            let via = match (medium, def) {
                (Some(_), Some(at)) => format!("medium definition at {}", cp_label(q, at)),
                (Some(_), None) => "carried through the valves".to_owned(),
                (None, _) => "no medium at its source".to_owned(),
            };
            // The source: the device whose port defines the medium (the first definition of it).
            let source = medium
                .and_then(|m| {
                    defs.iter()
                        .map(|(_, d)| d)
                        .filter(|d| d.medium == m)
                        .min_by_key(|d| d.id)
                })
                .and_then(|d| EndpointKey::of(&d.at).and_then(ew_derive::CpRef::of))
                .and_then(|cp| functions.get(cp.function)?.device);
            let value = RowValue::One {
                json: serde_json::json!({
                    "designation": designation,
                    "medium": medium.map(|m| m.as_uuid().to_string()),
                }),
                text: medium
                    .and_then(|m| media.get(m))
                    .map(|m| medium_words(m))
                    .unwrap_or_default(),
            };
            let mut b = derived(
                "device",
                &format!("port[{n}]"),
                None,
                Category::Part,
                FieldType::Text,
                value,
                Provenance::Derived { via },
                Vec::new(),
            );
            b.row.source = source.map(|d| SourceLink::Record {
                id: d.as_uuid(),
                row: "device/derived.medium".to_owned(),
            });
            b
        })
        .collect()
}
