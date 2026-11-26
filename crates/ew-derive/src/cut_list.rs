// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cut_rows()`: everything the fitter cuts to length (doc 13 §9.3, FR-MFG-030), in five groups, each piece once.
//!
//! - **Rails** and **Ducts**: one row per part and piece length, the carriers of every mounting surface in the legend's
//!   order; a duct's `unit` accessories (its cover, `carrier_accessories`, the enclosure device's choices applied) each
//!   a row beside it, one piece of the duct's length per duct.
//! - **Cables**: one row per cable in natural order of its tag, its length as entered; a cable of its part's own length
//!   (pre-assembled) is not cut and is left out.
//! - **Wires**: the wire records (`wire_record_set`) cut in the shop, one row per wire part (else size and colour) and
//!   length; a cable's core is cut with its cable and a field wire on site, both left out.
//! - **Tubes**: the tube rows (`tube_rows`), one row per tube part (else material, size and colour) and length.
//!
//! A piece is rounded up once, here, to the project's cut increment ([`cut_to`]); a wire's length is the wire list's and
//! never rounded again. The pieces without a length count on one row per part with no length. Rails and ducts come from
//! the layout: a project without one lists none.

use ew_ids::Id;
use ew_mltext::{NaturalKey, natural_key};
use ew_schema::{Carrier, CarrierKind, ColourSpec, Csa, Device, Enclosure, LengthSource, MountingSurface, PartRef};
use ew_units::Dec3;

use crate::Q;
use crate::fluid_rows::TubeRows;
use crate::layout::{CarrierLabel, carrier_label, carrier_length, carriers_in_order};
use crate::part_text::printed_number;
use crate::tag::short_tag;
use crate::wire_record::{WireLength, WireRecordSet};

/// The cut increment when the project sets none (doc 13 §9.3): 5 mm.
const CUT_INCREMENT: i128 = 5_000_000;

/// A length in nanometres rounded up to the project's cut increment (`workshop.cut_increment`, else 5 mm, doc 13 §9.3);
/// a length of zero or less as it is.
pub fn cut_to(q: &Q, nm: i128) -> i128 {
    let step = q
        .project()
        .settings
        .workshop
        .cut_increment
        .map(|n| i128::from(n.0))
        .filter(|n| *n > 0)
        .unwrap_or(CUT_INCREMENT);
    if nm <= 0 {
        return nm;
    }
    nm.saturating_add(step - 1) / step * step
}

/// The five groups of the cut list, in its order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum CutGroup {
    /// The rails.
    Rails,
    /// The ducts and their covers.
    Ducts,
    /// The cables.
    Cables,
    /// The wires cut in the shop.
    Wires,
    /// The tubes.
    Tubes,
}

/// What a cut piece is cut from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CutWhat {
    /// A part: a rail's, a duct's or its cover's, a cable's, a wire's, a tube's.
    Part(PartRef),
    /// A wire without a part: its size and colour.
    Wire {
        /// The size.
        csa: Option<Csa>,
        /// The colour.
        colour: Option<ColourSpec>,
    },
    /// A tube without a part: its material, size and colour as the tube list prints them.
    Tube {
        /// The material's code (`PU`).
        material: String,
        /// The size (`6×4`).
        size: String,
        /// The colour.
        colour: Option<ColourSpec>,
    },
    /// A carrier without a part.
    Unknown,
}

/// What a row of the cut list is for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CutFor {
    /// Carriers, by their labels ("Rail 1").
    Carriers(Vec<CarrierLabel>),
    /// A cable.
    Cable(Id<Device>),
    /// Wires or tubes by their numbers or tags, a run of three or more compressed ("101.1 … 101.3, 102.6").
    Names(String),
}

/// One row of the cut list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CutRow {
    /// What it is cut from.
    pub what: CutWhat,
    /// What it is for.
    pub what_for: CutFor,
    /// The pieces.
    pub pieces: usize,
    /// The piece length in nanometres, rounded up to the cut increment (a wire's as the wire list prints it); `None`
    /// for pieces without a length.
    pub length: Option<i128>,
}

/// `cut_rows()`: the cut list's rows by group, in its order (see the module documentation).
pub fn cut_rows(q: &Q) -> Vec<(CutGroup, Vec<CutRow>)> {
    let (rails, ducts) = carrier_rows(q);
    vec![
        (CutGroup::Rails, rails),
        (CutGroup::Ducts, ducts),
        (CutGroup::Cables, cable_rows(q)),
        (CutGroup::Wires, wire_rows(q)),
        (CutGroup::Tubes, tube_rows(q)),
    ]
}

/// Carriers of one kind, part and piece length, in the order of their first carrier.
type CarrierGroup = (CarrierKind, Option<PartRef>, Option<i128>, Vec<Id<Carrier>>);

/// The rails' and the ducts' rows (with each duct's covers beside it).
fn carrier_rows(q: &Q) -> (Vec<CutRow>, Vec<CutRow>) {
    let table = q.table::<Carrier>();
    let mut surfaces: Vec<Id<MountingSurface>> = q.table::<MountingSurface>().iter().map(|(id, _)| id).collect();
    surfaces.sort();
    // Carriers grouped by kind, part and piece length, in the order of their first carrier.
    let mut groups: Vec<CarrierGroup> = Vec::new();
    for c in surfaces.into_iter().flat_map(|s| carriers_in_order(q, s)) {
        let Some(rec) = table.get(c) else { continue };
        if !matches!(rec.kind, CarrierKind::Rail | CarrierKind::Duct) {
            continue;
        }
        let length = carrier_length(q, c).map(|l| cut_to(q, l));
        match groups
            .iter_mut()
            .find(|g| g.0 == rec.kind && g.1 == rec.part && g.2 == length)
        {
            Some(g) => g.3.push(c),
            None => groups.push((rec.kind.clone(), rec.part.clone(), length, vec![c])),
        }
    }
    let (mut rails, mut ducts) = (Vec::new(), Vec::new());
    for (kind, part, length, carriers) in groups {
        let labels: Vec<CarrierLabel> = carriers.iter().filter_map(|c| carrier_label(q, *c)).collect();
        let row = |what: CutWhat| CutRow {
            what,
            what_for: CutFor::Carriers(labels.clone()),
            pieces: carriers.len(),
            length,
        };
        if kind == CarrierKind::Rail {
            rails.push(row(part.map_or(CutWhat::Unknown, CutWhat::Part)));
            continue;
        }
        ducts.push(row(part.map_or(CutWhat::Unknown, CutWhat::Part)));
        for a in duct_accessories(q, &carriers) {
            ducts.push(row(CutWhat::Part(a)));
        }
    }
    (rails, ducts)
}

/// The parts the ducts `carriers` bring with them per unit (`carrier_accessories`, the choices of the first duct's
/// enclosure device applied), each once.
fn duct_accessories(q: &Q, carriers: &[Id<Carrier>]) -> Vec<PartRef> {
    let Some(device) = carriers.first().and_then(|c| {
        let surface = q.table::<Carrier>().get(*c)?.surface;
        let enclosure = q.table::<MountingSurface>().get(surface)?.enclosure;
        let device = q.table::<Enclosure>().get(enclosure)?.device;
        q.devices().get(device).cloned()
    }) else {
        return Vec::new();
    };
    let mut out: Vec<PartRef> = Vec::new();
    for a in crate::carrier_accessories(q, &device, carriers) {
        if !out.contains(&a.part) {
            out.push(a.part);
        }
    }
    out
}

/// A cable keyed by its tag: the tag's natural key, the cable, its part and its cut length.
type KeyedCable = (NaturalKey, Id<Device>, Option<PartRef>, Option<i128>);

/// The cables' rows, in natural order of their tags.
fn cable_rows(q: &Q) -> Vec<CutRow> {
    let mut cables: Vec<KeyedCable> = q
        .devices()
        .iter()
        .filter_map(|(id, d)| {
            let length = match &d.aspects.cable.as_ref()?.length {
                LengthSource::Manual(n) => Some(cut_to(q, i128::from(n.0))),
                LengthSource::Missing => None,
                LengthSource::FromPart | LengthSource::Unknown(_) => return None,
            };
            let part = d.parts.first().map(|u| u.part.clone());
            Some((natural_key(&short_tag(q, id)), id, part, length))
        })
        .collect();
    cables.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    cables
        .into_iter()
        .map(|(_, id, part, length)| CutRow {
            what: part.map_or(CutWhat::Unknown, CutWhat::Part),
            what_for: CutFor::Cable(id),
            pieces: 1,
            length,
        })
        .collect()
}

/// The names `listed` in the order `order` gives them, a run of three or more names that stand next to each other
/// there printed "first … last", the runs joined by ", ".
fn name_runs(order: &[String], listed: &[String]) -> String {
    let mut runs: Vec<String> = Vec::new();
    let mut i = 0;
    while i < order.len() {
        if !listed.contains(&order[i]) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < order.len() && listed.contains(&order[j + 1]) {
            j += 1;
        }
        if j - i >= 2 {
            runs.push(format!("{} … {}", order[i], order[j]));
        } else {
            runs.extend(order[i..=j].iter().cloned());
        }
        i = j + 1;
    }
    runs.join(", ")
}

/// The key a wire is cut by: its wire part, else its size and colour.
type WireKey = (Option<PartRef>, Option<Csa>, Option<ColourSpec>);

/// The wires of one key and length: the pieces and the wires' numbers.
type WireGroup = ((WireKey, Option<i128>), usize, Vec<String>);

/// The wires' rows: per wire part (else size and colour) and length, by size, colour, part number, then length (no
/// length last).
fn wire_rows(q: &Q) -> Vec<CutRow> {
    let records = q.get::<WireRecordSet>(());
    let numbers: Vec<String> = records.iter().filter_map(|r| r.number.clone()).collect();
    // Per key and length: the pieces and the numbers of the wires, in the order of their first wire.
    let mut groups: Vec<WireGroup> = Vec::new();
    for r in records.iter() {
        if r.flags.field || matches!(r.length, WireLength::WithCable(_)) {
            continue;
        }
        let key: WireKey = match &r.wire_part {
            Some(p) => (Some(p.clone()), None, None),
            None => (
                None,
                r.csa.as_ref().map(|c| c.value.clone()),
                r.colour.as_ref().map(|c| c.value.clone()),
            ),
        };
        let entry = (key, r.length.value().map(|n| i128::from(n.0)));
        match groups.iter_mut().find(|g| g.0 == entry) {
            Some(g) => {
                g.1 += 1;
                g.2.extend(r.number.clone());
            }
            None => groups.push((entry, 1, r.number.clone().into_iter().collect())),
        }
    }
    groups.sort_by_cached_key(|((part_csa_colour, length), _, _)| {
        let (part, csa, colour) = part_csa_colour;
        (
            csa.as_ref().and_then(csa_mm2),
            colour.as_ref().map(crate::colour_text).unwrap_or_default(),
            part.as_ref().map(|p| natural_key(&printed_number(q, p))),
            length.is_none(),
            *length,
        )
    });
    groups
        .into_iter()
        .map(|(((part, csa, colour), length), pieces, names)| CutRow {
            what: part.map_or(CutWhat::Wire { csa, colour }, CutWhat::Part),
            what_for: CutFor::Names(name_runs(&numbers, &names)),
            pieces,
            length,
        })
        .collect()
}

/// A cross-section's square millimetres, for the order of the rows; `None` for another unit.
fn csa_mm2(c: &Csa) -> Option<Dec3> {
    match c {
        Csa::Mm2(d) => Some(*d),
        _ => None,
    }
}

/// The order of a tube row: the part number (else material, size and colour), then its length, none last.
type TubeOrder = (NaturalKey, NaturalKey, String, bool, Option<i128>);

/// The tubes' rows: per tube part (else material, size and colour) and length, in natural order of the part number
/// (else material, size, colour), then length (no length last).
fn tube_rows(q: &Q) -> Vec<CutRow> {
    let tubes = q.get::<TubeRows>(());
    let mut rows: Vec<(TubeOrder, CutRow)> = Vec::new();
    for t in tubes.iter() {
        let what = match &t.props.part {
            Some(p) => CutWhat::Part(PartRef::new(p.value)),
            None => CutWhat::Tube {
                material: t
                    .props
                    .material
                    .as_ref()
                    .map(|m| m.value.code().to_owned())
                    .unwrap_or_default(),
                size: t.props.size_text(),
                colour: t.props.colour.as_ref().map(|c| c.value.clone()),
            },
        };
        let length = t.length.map(|n| cut_to(q, i128::from(n.0)));
        match rows.iter_mut().find(|(_, r)| r.what == what && r.length == length) {
            Some((_, r)) => {
                r.pieces += 1;
                if let CutFor::Names(names) = &mut r.what_for
                    && !t.tag.is_empty()
                {
                    if !names.is_empty() {
                        names.push_str(", ");
                    }
                    names.push_str(&t.tag);
                }
            }
            None => {
                let key = match &what {
                    CutWhat::Part(p) => (
                        natural_key(&printed_number(q, p)),
                        natural_key(""),
                        String::new(),
                    ),
                    CutWhat::Tube {
                        material,
                        size,
                        colour,
                    } => (
                        natural_key(material),
                        natural_key(size),
                        colour.as_ref().map(crate::colour_text).unwrap_or_default(),
                    ),
                    _ => (natural_key(""), natural_key(""), String::new()),
                };
                rows.push((
                    (key.0, key.1, key.2, length.is_none(), length),
                    CutRow {
                        what,
                        what_for: CutFor::Names(t.tag.clone()),
                        pieces: 1,
                        length,
                    },
                ));
            }
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::name_runs;

    /// Three or more wires that stand next to each other in the numbering order print as a run; fewer, and the wires
    /// of other rows between, as a list.
    #[test]
    fn a_run_of_three_names_is_compressed() {
        let order: Vec<String> = ["101.1", "101.2", "101.3", "101.4", "101.5", "102.1"]
            .map(str::to_owned)
            .to_vec();
        let listed = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            name_runs(&order, &listed(&["101.1", "101.2", "101.3", "101.5", "102.1"])),
            "101.1 … 101.3, 101.5, 102.1"
        );
        assert_eq!(name_runs(&order, &listed(&["101.4", "101.5"])), "101.4, 101.5");
        assert_eq!(name_runs(&order, &[]), "");
    }
}
