// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cable marks → cables with cores, for projects without QET 0.200 cable data (doc 14 §4.11 "Cables, older").
//!
//! QET draws a cable as marks across its wires: elements without terminals (`section1conducteur` and the like) set in
//! a row, the first labelled with the cable's tag (`-6001W1`, letter W), the others without a label. The labelled
//! mark heads a cable device (the `Cable` aspect) whose main function its mark draws; the unlabelled marks of the same
//! definition in its row draw auxiliary functions of it, and so do those of a folio without a cable mark of its own
//! that continue the cable the folio before it heads last (a cable drawn on over a second folio). Every wire a mark of
//! the cable crosses becomes a core of it, numbered from 1 in the order of the marks along their rows, folio by
//! folio; a wire two marks cross is one core. A mark crosses a wire whose stored route runs through the mark's
//! insertion point across the mark's row.

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{Conductor, Connection, ConnectionKind, Device, Page};
use ew_units::{Nm, Pt2};

use super::cx::Ids;
use super::elements::{Elem, infer_tag};

/// One cable the marks make: its device, the source key of its heading mark, and each mark's page, insertion point
/// and whether its row runs across (left to right), in core order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CableMarks {
    /// The cable device.
    pub device: Id<Device>,
    /// The project-wide key of its heading mark.
    pub key: String,
    /// The marks: page, insertion point, row across.
    pub marks: Vec<(Id<Page>, Pt2, bool)>,
}

/// Whether the label `label` (on folio `folio`) is a cable's tag: its letter code is `W`.
fn cable_tag(label: &str, folio: &str) -> bool {
    let (tag, _, _) = infer_tag(label, folio, "");
    tag.letter.as_str() == "W"
}

/// The row of a mark: its folio, definition, whether it runs across (an unturned or half-turned element) and its
/// coordinate across the row.
fn row_of(e: &Elem<'_>) -> (usize, String, bool, i64) {
    let across = e.el.orientation % 2 == 0;
    let at = if across { e.pos.y.0 } else { e.pos.x.0 };
    (e.diagram, e.el.type_path.clone(), across, at)
}

/// The cables of the cable marks among `elems` (elements without terminals; `folio` gives a diagram's folio label): for
/// each element of a cable, the index of the mark that heads it.
pub(crate) fn group(elems: &[Elem<'_>], folio: impl Fn(usize) -> String) -> BTreeMap<usize, usize> {
    let marks: Vec<usize> = (0..elems.len())
        .filter(|&i| elems[i].terminal_count() == 0)
        .collect();
    let mut head_of: BTreeMap<usize, usize> = BTreeMap::new();
    // The heads, and the rows they head.
    let mut rows: BTreeMap<(usize, String, bool, i64), usize> = BTreeMap::new();
    for &i in &marks {
        if elems[i]
            .el
            .info_nonempty("label")
            .is_some_and(|l| cable_tag(l, &folio(elems[i].diagram)))
        {
            rows.entry(row_of(&elems[i])).or_insert(i);
            head_of.insert(i, i);
        }
    }
    // The last head of each folio, by definition and row direction: what a folio without a head of its own continues.
    let mut last: BTreeMap<(usize, String, bool), usize> = BTreeMap::new();
    for ((d, path, across, _), &h) in &rows {
        last.insert((*d, path.clone(), *across), h);
    }
    let headed: Vec<usize> = rows.keys().map(|(d, ..)| *d).collect();
    for &i in &marks {
        if head_of.contains_key(&i) || elems[i].el.info_nonempty("label").is_some() {
            continue;
        }
        let (d, path, across, at) = row_of(&elems[i]);
        let head = rows.get(&(d, path.clone(), across, at)).copied().or_else(|| {
            (!headed.contains(&d))
                .then(|| last.get(&(d.checked_sub(1)?, path, across)).copied())
                .flatten()
        });
        if let Some(h) = head {
            head_of.insert(i, h);
        }
    }
    head_of
}

/// Whether the route `route` runs through `p` across a row that runs across (`across`, the wire then vertical) or
/// down, within one QET px.
fn crosses(route: &[Pt2], p: Pt2, across: bool) -> bool {
    const PX: i128 = 264_584;
    let near = |u: Nm, v: Nm| (i128::from(u.0) - i128::from(v.0)).abs() <= PX;
    let within = |v: Nm, a: Nm, b: Nm| {
        let (v, a, b) = (i128::from(v.0), i128::from(a.0), i128::from(b.0));
        a.min(b) - PX <= v && v <= a.max(b) + PX
    };
    route.windows(2).any(|s| {
        if across {
            near(s[0].x, s[1].x) && near(s[0].x, p.x) && within(p.y, s[0].y, s[1].y)
        } else {
            near(s[0].y, s[1].y) && near(s[0].y, p.y) && within(p.x, s[0].x, s[1].x)
        }
    })
}

/// The cores of `cables` among `connections`: each crossed connection becomes a core of its cable (its kind
/// `Core(conductor)`), and the conductors are returned in cable then core order.
pub(crate) fn cores(cables: &[CableMarks], connections: &mut [Connection], ids: &Ids) -> Vec<Conductor> {
    let mut out = Vec::new();
    for cable in cables {
        let mut n: u32 = 0;
        for (page, at, across) in &cable.marks {
            let Some(c) = connections.iter_mut().find(|c| {
                c.page == Some(*page)
                    && matches!(c.kind, ConnectionKind::Wire)
                    && crosses(&c.route, *at, *across)
            }) else {
                continue;
            };
            n += 1;
            let key = format!("qet:cable:{}#core{n}", cable.key);
            let id: Id<Conductor> = ids.id(&key);
            c.kind = ConnectionKind::Core(id);
            out.push(Conductor {
                id,
                v: 1,
                cable: cable.device,
                core: Ident::from_input(&n.to_string())
                    .unwrap_or_else(|_| unreachable!("a decimal is an identifier")),
                colour: None,
                csa: None,
                shield: false,
                pair: None,
                source_ref: Some(ids.source_ref(&key)),
                extra: Default::default(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    #[test]
    fn a_mark_crosses_the_wire_through_its_point_across_its_row() {
        let vertical = [p(100, 0), p(100, 1_000_000)];
        assert!(crosses(&vertical, p(100, 500_000), true));
        assert!(!crosses(&vertical, p(100, 500_000), false));
        assert!(!crosses(&vertical, p(900_000, 500_000), true));
        assert!(!crosses(&vertical, p(100, 2_000_000), true));
    }

    #[test]
    fn a_cable_tag_has_the_letter_w() {
        assert!(cable_tag("-6001W1", "6001"));
        assert!(cable_tag("W3", "1"));
        assert!(!cable_tag("-6001K1", "6001"));
        assert!(!cable_tag("1,5 mm²", "1"));
    }
}
