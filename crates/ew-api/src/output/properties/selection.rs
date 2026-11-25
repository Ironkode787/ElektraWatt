// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The selection's live description (NFR-A11Y-02) and the wire-colour view: what the canvas says of a selected
//! record, and a connection's physical wires with their printed numbers and colours.
//!
//! Not here: the property sheet (`properties.rs`); the order of records in shell output (`order.rs`).

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::{ConnWireProps, MainRep, PotentialNameOf, Q, WireNumbers, cp_label};
use ew_ids::{Id, Uuid};
use ew_model::EndpointKey;
use ew_schema::{Connection, Device};
use serde_json::Value;

use super::connection_rows::potential_of;
use super::context::SheetCx;
use super::{device_tag_text, folio_of, ml_text};
use crate::Session;

/// What the canvas's live region says of one selected record (NFR-A11Y-02), as values: the shell words
/// them in the UI language.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SelectionWords {
    /// A device, or a function or representation of one device.
    Device {
        /// The printed tag: as displayed on its main page, else the short tag.
        tag: String,
        /// The kind of its main function as the navigators print it (the FnDef's category); `None` without a
        /// main function or without a copy of its FnDef.
        kind: Option<String>,
        /// The folio of its main representation; `None` while the device is not placed.
        folio: Option<String>,
        /// The connection points of its functions.
        cps: usize,
        /// The connections that end at those connection points, each once.
        wires: usize,
    },
    /// A connection.
    Wire {
        /// Its printed wire number (the property sheet's title); `None` while it has none.
        number: Option<String>,
        /// The name of its potential, if it has one.
        potential: Option<String>,
        /// Its first end.
        from: WireEnd,
        /// Its second end.
        to: WireEnd,
    },
    /// A page item.
    PageItem {
        /// Its variant: `graphic`, `free_text`, `wire_marker`, `image`, `surface_view`, or `unknown`.
        kind: String,
        /// The printed text of a free text or a wire marker.
        text: Option<String>,
    },
    /// A junction.
    Junction {
        /// The folio of its page.
        folio: Option<String>,
        /// The connections that meet at it.
        wires: usize,
    },
    /// A representation of an interruption point.
    InterruptionPoint {
        /// Its pairing name.
        name: String,
        /// The folio of its page.
        folio: Option<String>,
    },
    /// A structure box.
    StructureBox {
        /// Its caption in the project's source language; `None` without one.
        caption: Option<String>,
        /// The folio of its page.
        folio: Option<String>,
    },
}

/// One end of a selected wire in the live description.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WireEnd {
    /// A connection point with its printed label (`-102F1:2`).
    Point(String),
    /// A connection point whose label is empty.
    UnnamedPoint,
    /// A junction.
    Junction,
    /// An end of a newer format, kept verbatim.
    Unknown,
}

impl Session {
    /// The live description's values of the record `id` in the current snapshot: a device (or a
    /// function or representation of exactly one device), a connection, a page item, a junction, an interruption
    /// point's representation or a structure box; `None` for any other record and for an unknown ID.
    pub fn selection_words(&self, id: Uuid) -> Option<SelectionWords> {
        selection_words(&self.query(), id)
    }
}

fn selection_words(q: &Q, id: Uuid) -> Option<SelectionWords> {
    if let Some(conn) = q.connections().get(Id::from_uuid(id)) {
        let end = |e: &ew_schema::Endpoint| match e {
            ew_schema::Endpoint::Cp(function, idx) => {
                let label = cp_label(
                    q,
                    ew_derive::CpRef {
                        function: *function,
                        idx: *idx,
                    },
                );
                if label.is_empty() {
                    WireEnd::UnnamedPoint
                } else {
                    WireEnd::Point(label)
                }
            }
            ew_schema::Endpoint::Junction(_) => WireEnd::Junction,
            ew_schema::Endpoint::Unknown(_) => WireEnd::Unknown,
        };
        return Some(SelectionWords::Wire {
            number: wire_number(q, conn.id),
            potential: potential_of(q, conn).map(|p| q.get::<PotentialNameOf>(p).name.text.clone()),
            from: end(&conn.a),
            to: end(&conn.b),
        });
    }
    if let Some(item) = q.page_items().get(Id::from_uuid(id)) {
        use ew_schema::PageItemKind as K;
        let (kind, text) = match &item.kind {
            K::Graphic(_) => ("graphic", None),
            K::FreeText(t) => ("free_text", Some(ew_derive::free_text(q, item.page, t))),
            // A cable's mark is named by its cable.
            K::WireMarker(_) => match ew_derive::cable_mark(q, item) {
                Some(mark) => ("cable_mark", Some(mark.tag)),
                None => ("wire_marker", ew_derive::group_marker(q, item).map(|g| g.text)),
            },
            K::Image(_) => ("image", None),
            K::SurfaceView(_) => ("surface_view", None),
            K::Underlay(u) => ("underlay", Some(u.name.clone()).filter(|n| !n.is_empty())),
            K::Unknown(_) => ("unknown", None),
        };
        return Some(SelectionWords::PageItem {
            kind: kind.to_owned(),
            text,
        });
    }
    if let Some(j) = q.junctions().get(Id::from_uuid(id)) {
        return Some(SelectionWords::Junction {
            folio: folio_of(q, j.page),
            wires: j.sequence.len(),
        });
    }
    if let Some(b) = q.structure_boxes().get(Id::from_uuid(id)) {
        let source = SheetCx::new(q).source;
        return Some(SelectionWords::StructureBox {
            caption: b
                .caption
                .as_ref()
                .and_then(|c| serde_json::to_value(c).ok())
                .map(|c| ml_text(&c, source.as_deref()))
                .filter(|c| !c.trim().is_empty()),
            folio: folio_of(q, b.page),
        });
    }
    if let Some(rep) = q.reps().get(Id::from_uuid(id)) {
        let pairing = q.get::<ew_derive::IpPairingQ>(());
        if let Some(ip) = rep.functions.iter().find_map(|f| pairing.ips.get(f)) {
            return Some(SelectionWords::InterruptionPoint {
                name: ip.name.clone(),
                folio: folio_of(q, rep.page),
            });
        }
    }
    let device = selected_device(q, id)?;
    // The category's text, else the FnDef's key; neither gives `None` ("Device").
    let kind = (*q.get::<ew_derive::MainFunction>(device))
        .and_then(|f| q.functions().get(f))
        .and_then(|f| crate::output::navigators::function_kind_or_key(q, f));
    let folio = (*q.get::<MainRep>(device))
        .and_then(|r| q.reps().get(r).map(|rep| rep.page))
        .and_then(|page| folio_of(q, page));
    let (mut cps, mut wires) = (0, BTreeSet::new());
    for f in q.idx().functions_of_device(device) {
        for cp in q.get::<ew_derive::FunctionCps>(f).iter() {
            cps += 1;
            wires.extend(q.idx().connections_at_cp(f, cp.idx));
        }
    }
    Some(SelectionWords::Device {
        tag: device_tag_text(q, device),
        kind,
        folio,
        cps,
        wires: wires.len(),
    })
}

/// The device a selected record names: the device itself, a function's device, or the one device whose functions a
/// representation draws.
fn selected_device(q: &Q, id: Uuid) -> Option<Id<Device>> {
    if q.devices().get(Id::from_uuid(id)).is_some() {
        return Some(Id::from_uuid(id));
    }
    if let Some(f) = q.functions().get(Id::from_uuid(id)) {
        return f.device;
    }
    let rep = q.reps().get(Id::from_uuid(id))?;
    let devices: BTreeSet<Id<Device>> = rep
        .functions
        .iter()
        .filter_map(|f| q.functions().get(*f).and_then(|f| f.device))
        .collect();
    match devices.len() {
        1 => devices.into_iter().next(),
        _ => None,
    }
}

/// A physical wire: its carrier connection and its effective properties.
type Wire = (Id<Connection>, ew_derive::EffectiveProps);

/// The physical wires of a connection, `(carrier, effective properties)`, in the order of their printed wire numbers
/// (natural order; an unnumbered wire after the numbered ones), ties by carrier ID: the "first wire" whose
/// number and properties the sheet prints is one a person can see, never the smallest of IDs one transaction minted.
pub(super) fn wires_of(q: &Q, conn: Id<Connection>) -> Vec<Wire> {
    let numbers = q.get::<WireNumbers>(());
    let mut wires: Vec<(Option<String>, Wire)> = q
        .get::<ConnWireProps>(conn)
        .iter()
        .map(|w| (numbers.number(w.0).map(|n| n.label.clone()), w.clone()))
        .collect();
    wires.sort_by(|(a, wa), (b, wb)| number_order(a.as_deref(), b.as_deref()).then_with(|| wa.0.cmp(&wb.0)));
    wires.into_iter().map(|(_, w)| w).collect()
}

/// A company colour code the project's colour profile declares (FR-CON-019): its IEC 60757 base and its shade.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompanyColour {
    /// The code (`DBU`).
    pub code: String,
    /// The IEC 60757 base code (`BU`).
    pub base: String,
    /// `light` or `dark`; `None` for the base colour itself.
    pub shade: Option<String>,
}

/// What the wire-colour view reads: the effective colour of each connection asked for, as its codes,
/// and the company codes of the colour profile.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WireColours {
    /// Each connection's effective colour (its first wire's, as the property sheet prints it): the codes in order,
    /// one or two (a foreign code by its spelling); a connection without a known colour is not listed.
    pub connections: BTreeMap<Uuid, Vec<String>>,
    /// The declared company codes, in the profile's order.
    pub company: Vec<CompanyColour>,
}

impl Session {
    /// The effective colours of `connections` and the profile's company codes, for the wire-colour view.
    /// Reads memoised queries only; a connection that forms no physical wire, or whose colour is unset, is
    /// left out.
    pub fn wire_colours(&self, connections: &[Uuid]) -> WireColours {
        let q = self.query();
        let mut out = WireColours::default();
        for id in connections {
            let Some((_, props)) = wires_of(&q, Id::from_uuid(*id)).into_iter().next() else {
                continue;
            };
            let Some(spec) = props.colour.as_ref().map(|c| &c.value) else {
                continue;
            };
            let codes: Option<Vec<String>> = spec.codes().iter().map(colour_code_text).collect();
            if let Some(codes) = codes.filter(|c| !c.is_empty()) {
                out.connections.insert(*id, codes);
            }
        }
        let profile = q.project().settings.extra.get("colour_profile").cloned();
        if let Some(profile) = profile {
            let declared = profile.get("codes").and_then(Value::as_array);
            for (code, base) in ew_derive::company_codes(&profile) {
                let shade = declared
                    .and_then(|d| {
                        d.iter()
                            .find(|c| c.get("code").and_then(Value::as_str) == Some(code.as_str()))
                    })
                    .and_then(|c| c.get("shade"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                out.company.push(CompanyColour { code, base, shade });
            }
        }
        out
    }
}

/// A colour code as the wire-colour view lists it: an IEC or company code and a foreign code by their spelling; a
/// form of a newer format as `?`, which resolves to no colour, so the connection is drawn as the unresolved pattern
/// rather than left out.
pub(super) fn colour_code_text(code: &ew_schema::ColourCode) -> Option<String> {
    use ew_schema::ColourCode;
    match code {
        ColourCode::Foreign(f) => Some(f.as_str().to_owned()),
        ColourCode::Unknown(_) => Some("?".to_owned()),
        other => serde_json::to_value(other).ok()?.as_str().map(str::to_owned),
    }
}

/// The order of printed wire numbers: natural order, then bytes; an unnumbered wire (`None`) after every numbered one.
pub(super) fn number_order(a: Option<&str>, b: Option<&str>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a), Some(b)) => ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)),
        _ => a.is_none().cmp(&b.is_none()),
    }
}

/// The printed wire number of a connection (its first wire's), `?` when unassigned.
pub(super) fn wire_number_text(q: &Q, conn: Id<Connection>) -> String {
    wire_number(q, conn).unwrap_or_else(|| "?".to_owned())
}

/// The words the findings name connection `c` by where its sheet's title is in none of them: a fluid line as the
/// tube list names it (its tag, else its two ends, in English), a wire without a number by its ends' connection points
/// (`-G1:+ → -S1:1`); `None` for a numbered wire, whose number is its title.
pub(super) fn connection_filter(q: &Q, c: Id<Connection>) -> Option<String> {
    let conn = q.connections().get(c)?;
    if matches!(conn.kind, ew_schema::ConnectionKind::Fluid(_)) {
        let rows = q.get::<ew_derive::TubeRows>(());
        let row = rows.iter().find(|r| r.constituents.contains(&c))?;
        let en = "en-GB".parse().ok()?;
        return row.name().text_for(&en).map(str::to_owned);
    }
    if wire_number(q, c).is_some() {
        return None;
    }
    let ends: Vec<String> = [&conn.a, &conn.b]
        .into_iter()
        .filter_map(|e| EndpointKey::of(e).and_then(ew_derive::CpRef::of))
        .map(|cp| cp_label(q, cp))
        .collect();
    (!ends.is_empty()).then(|| ends.join(" → "))
}

/// The printed wire number of a connection (its first wire's), `None` when unassigned.
pub(super) fn wire_number(q: &Q, conn: Id<Connection>) -> Option<String> {
    let numbers = q.get::<WireNumbers>(());
    wires_of(q, conn)
        .first()
        .and_then(|(w, _)| numbers.number(*w).map(|n| n.label.clone()))
        .or_else(|| numbers.number(conn).map(|n| n.label.clone()))
}
