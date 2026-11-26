// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The marker of a wire's end (doc 13 §5.1, §5.2): what the printed sleeve on each end says, by the shop's marking
//! (`WorkshopSettings.marking`, the far end by default), unless a person set it by hand (`EndOverride.marker`), and
//! the rule that shortens a text to the wire markers' part of the workshop stock (`sections.marker.lines`, `chars`).
//! [`mark`] fills the ends of the records of `wire_record_set()`.

use ew_schema::{EndMarker, Marking};

use crate::joins::{cp_pin, cp_tag_from};
use crate::nets::CpRef;
use crate::q::Q;
use crate::tag::short_tag;
use crate::wire_record::WireRecord;
use crate::wires::EndKind;

/// What an end's marker says.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MarkerText {
    /// One to three lines, shortened to the marker part where it names its lines and characters.
    Lines(Vec<String>),
    /// No marker, and why.
    NotMarked(NotMarked),
}

/// Why an end carries no marker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotMarked {
    /// A cable's core, while the shop does not mark cores.
    Core,
    /// A jumper on one device, while the shop does not mark jumpers.
    Jumper,
    /// An end made outside the project.
    Outside,
    /// Set by hand to none.
    ByHand,
    /// The marking names nothing for it (a wire without a number, under wire-number marking).
    Nothing,
}

/// Who decided an end's marker.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MarkedBy {
    /// The shop's marking.
    #[default]
    Setting,
    /// A person, by hand.
    Hand,
}

/// The marker part a wire's marker is printed on: the first marker of the workshop stock that marks wires, with the lines
/// and characters per line it holds (`None` where it names none).
pub fn wire_marker_part(q: &Q) -> Option<(Option<u8>, Option<u8>)> {
    let parts = q.parts();
    q.project().settings.workshop.stock.iter().find_map(|r| {
        let m = parts.get(r.copy)?.item.sections.marker.as_ref()?;
        (m.marks == ew_schema::MarkTarget::Wire).then_some((m.lines, m.chars))
    })
}

/// A wire end as a marker names it, seen from the end `from` (doc 13 §5.2): its device or terminal without the
/// structure the two ends share, and a device's pin (`-101Q1:1`, `-101G1:+#1`); a terminal by its tag alone, its clamp
/// and side never printed; an external end by its point's name.
pub fn marker_end_text(q: &Q, end: CpRef, kind: EndKind, from: CpRef) -> String {
    if kind != EndKind::Device {
        return crate::joins::cp_label(q, end);
    }
    let tag = cp_tag_from(q, end, Some(from));
    let terminal = q
        .functions()
        .get(end.function)
        .and_then(|f| f.device)
        .and_then(|d| q.devices().get(d))
        .is_some_and(|d| d.aspects.terminal.is_some());
    let pin = cp_pin(q, end);
    match (terminal, tag.is_empty(), pin.is_empty()) {
        (true, _, _) | (false, false, true) => tag,
        (false, true, _) => pin,
        (false, false, false) => format!("{tag}:{pin}"),
    }
}

/// The rule that shortens a marker's lines to a marker part of `lines` lines of `chars` characters (doc 13 §5.2): as
/// made; a line beginning with the product sign `-` without it; a line too long broken after its last `:`, else its
/// last `-` or `+`, that leaves the first part within the line. What still does not fit keeps its text whole, and
/// the second value says so: a character is never cut.
pub fn shorten(made: &[String], lines: Option<u8>, chars: Option<u8>) -> (Vec<String>, bool) {
    let max_lines = lines.map_or(usize::MAX, usize::from);
    let max_chars = chars.map_or(usize::MAX, usize::from);
    let fits = |v: &[String]| v.len() <= max_lines && v.iter().all(|l| l.chars().count() <= max_chars);
    if fits(made) {
        return (made.to_vec(), true);
    }
    let unsigned: Vec<String> = made
        .iter()
        .map(|l| l.strip_prefix('-').unwrap_or(l).to_owned())
        .collect();
    if fits(&unsigned) {
        return (unsigned, true);
    }
    let mut broken: Vec<String> = Vec::new();
    for line in &unsigned {
        let mut rest = line.clone();
        while rest.chars().count() > max_chars {
            let at = {
                let within = |c: char| {
                    rest.char_indices()
                        .filter(|(i, x)| *x == c && rest[..i + x.len_utf8()].chars().count() <= max_chars)
                        .map(|(i, x)| i + x.len_utf8())
                        .last()
                };
                within(':').or_else(|| match (within('-'), within('+')) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                })
            };
            match at {
                Some(at) if at < rest.len() => {
                    broken.push(rest[..at].to_owned());
                    rest = rest[at..].to_owned();
                }
                _ => break,
            }
        }
        broken.push(rest);
    }
    if fits(&broken) {
        (broken, true)
    } else {
        (made.to_vec(), false)
    }
}

/// Fills the marker of every end of `records` (doc 13 §5.1): set by hand, else none for an end made outside the
/// project, a core or a jumper the shop does not mark, else the shop's marking, shortened to the wire markers' part.
pub(crate) fn mark(q: &Q, records: &mut [WireRecord]) {
    let workshop = &q.project().settings.workshop;
    let part = wire_marker_part(q);
    let (lines, chars) = part.unwrap_or((None, None));
    for r in records.iter_mut() {
        let device_of = |e: usize| q.functions().get(r.wire.ends[e].cp.function).and_then(|f| f.device);
        let jumper = device_of(0).is_some() && device_of(0) == device_of(1);
        let core = r.wire.core.and_then(|c| q.conductors().get(c));
        let potential = r
            .wire
            .potentials
            .first()
            .map(|p| q.get::<crate::PotentialNameOf>(*p).name.text.clone());
        for i in 0..2 {
            let (here, there) = (r.wire.ends[i], r.wire.ends[1 - i]);
            let by_hand = r.ends[i].hand_at.and_then(|(c, a)| {
                let o = &q.connections().get(c)?.overrides;
                let end = if a { o.end_a.as_ref() } else { o.end_b.as_ref() }?;
                end.marker.clone()
            });
            let text = |e: &crate::wires::WireEnd, from: CpRef| marker_end_text(q, e.cp, e.kind, from);
            let (made, by) = match by_hand {
                Some(EndMarker::Text(t)) => (Ok(vec![t.as_str().to_owned()]), MarkedBy::Hand),
                Some(EndMarker::None) => (Err(NotMarked::ByHand), MarkedBy::Hand),
                _ if here.kind != EndKind::Device => (Err(NotMarked::Outside), MarkedBy::Setting),
                _ if core.is_some() && !workshop.mark_cores => (Err(NotMarked::Core), MarkedBy::Setting),
                _ if jumper && !workshop.mark_jumpers => (Err(NotMarked::Jumper), MarkedBy::Setting),
                _ => {
                    let number = r.number.clone().or_else(|| {
                        core.map(|c| format!("{} {}", short_tag(q, c.cable), c.core.as_str()))
                    });
                    let lines = match workshop.marking {
                        Marking::WireNumber => number.map(|n| vec![n]),
                        Marking::BothEnds => Some(vec![text(&here, there.cp), text(&there, here.cp)]),
                        Marking::Potential => potential.clone().map(|p| vec![p]),
                        Marking::Destination | Marking::Unknown(_) => Some(vec![text(&there, here.cp)]),
                    };
                    (lines.ok_or(NotMarked::Nothing), MarkedBy::Setting)
                }
            };
            let (marker, fits) = match made {
                Ok(lines_made) => {
                    let (shown, fits) = shorten(&lines_made, lines, chars);
                    (MarkerText::Lines(shown), fits)
                }
                Err(why) => (MarkerText::NotMarked(why), true),
            };
            r.ends[i].marker = marker;
            r.ends[i].marked_by = by;
            r.ends[i].marker_fits = fits;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| (*l).to_owned()).collect()
    }

    /// Each step of the shortening of doc 13 §5.2, and a text that does not fit kept whole.
    #[test]
    fn the_shortening_takes_its_steps_in_order() {
        assert_eq!(shorten(&v(&["-101Q1:1"]), Some(1), Some(10)), (v(&["-101Q1:1"]), true));
        assert_eq!(shorten(&v(&["-104A1-A2:L+"]), Some(1), Some(11)), (v(&["104A1-A2:L+"]), true));
        assert_eq!(
            shorten(&v(&["+CAB-104A1-A2:L+"]), Some(2), Some(12)),
            (v(&["+CAB-104A1-", "A2:L+"]), true)
        );
        assert_eq!(
            shorten(&v(&["+CAB-104A1-A2:L+"]), Some(1), Some(12)),
            (v(&["+CAB-104A1-A2:L+"]), false)
        );
        assert_eq!(shorten(&v(&["anything"]), None, None), (v(&["anything"]), true));
    }
}
