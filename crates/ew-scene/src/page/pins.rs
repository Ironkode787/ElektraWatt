// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pin texts of a black box and the notes beside a plug's pins (see the module documentation of `page`, "Pin
//! texts"): where each stands, its style, and the built-in measures the session sizes a box by.

#[allow(clippy::wildcard_imports)]
use super::*;

/// The distance of a black box's pin text from its pin, inward along the pin's normal: 1 mm.
pub const PIN_TEXT_GAP: Nm = Nm(1_000_000);

/// The text style of a black box's pin texts and of the notes beside a plug's pins.
pub const PIN_TEXT_STYLE: &str = "data.small";

/// The length in nanometres of the pin text `text` on one line in [`PIN_TEXT_STYLE`] of the built-in table with the
/// bundled fonts: how far a text of a top or bottom side, turned a quarter, reaches into the box, as the session sizes
/// a drawn box. The page prints the text in the project's table, so the two differ where a project changes the style.
pub fn builtin_pin_text_length(text: &str) -> i64 {
    let Some(style) = ew_text::builtin().resolve(PIN_TEXT_STYLE) else {
        return 0;
    };
    ew_text::bundled().shape(text, &style, None).advance().0
}

/// The height in nanometres of one line of pin text ([`PIN_TEXT_STYLE`] of the built-in table with the bundled fonts:
/// its ascent and descent, as the scene's text box measures it), as [`builtin_pin_text_length`] measures; `None`
/// without the style.
pub fn builtin_pin_text_line() -> Option<i64> {
    let style = ew_text::builtin().resolve(PIN_TEXT_STYLE)?;
    let m = ew_text::bundled().vertical_metrics(&style);
    Some(m.ascent.0.saturating_add(m.descent.0))
}

/// A placed pin text: its entity, text, point, angle and alignment, and for a text in a card's cell the width it keeps
/// within.
pub(crate) type PlacedPin = (EntityRef, String, Pt2, Angle2, Align, Option<Nm>);

/// The cells of the PLC cards drawn in one row (doc 10 §3.6, "A card drawn in cells"): a thin line between each two
/// neighbouring channel pins, at the middle between them, across the box from edge to edge. The labels layer draws them
/// with the pin texts, which tell which pins are a card's channels.
pub(crate) fn cell_dividers(view: &PageView) -> Vec<Item> {
    let stroke = wire_stroke(view.grid);
    let thin = Stroke {
        width: Nm(stroke.width.0 / 2),
        ..stroke
    };
    let mut out = Vec::new();
    for rv in sorted_reps(view) {
        let pins = cell_pins(rv);
        let Some(outline) = rv.rep.box_.as_ref().and_then(|b| {
            b.outline
                .iter()
                .map(|p| Rect::from_point(rv.rep.pos + rv.rep.orient.apply(*p)))
                .reduce(Rect::union)
        }) else {
            continue;
        };
        for pair in pins.windows(2) {
            let x = Nm(pair[0].0.x.0 + (pair[1].0.x.0 - pair[0].0.x.0) / 2);
            out.push(
                Item::path(
                    Path::polyline(&[Pt2::new(x, outline.min().y), Pt2::new(x, outline.max().y)]),
                    Some(thin.clone()),
                    None,
                )
                .with_entity(EntityRef(rv.rep.id.as_uuid())),
            );
        }
    }
    out
}

/// The channel pins of a PLC card drawn in one row (its [`PinText`]s with a `cell`), on the page and with the way their
/// wires leave, from left to right; empty for any other representation. Only a pin whose wire leaves up or down on the
/// page has a cell: a card's box turned a quarter keeps its pin texts as any box does.
fn cell_pins(rv: &RepView) -> Vec<(Pt2, Dir4, &PinText)> {
    let Some(b) = &rv.rep.box_ else {
        return Vec::new();
    };
    let mut out: Vec<(Pt2, Dir4, &PinText)> = b
        .cps
        .iter()
        .filter_map(|cp| {
            let pin = rv.pins.iter().find(|p| p.idx == cp.idx && p.cell.is_some())?;
            let dir = symbol::transform_dir(&cp.dir, rv.rep.orient);
            if !matches!(dir, Dir4::Up | Dir4::Down) {
                return None;
            }
            Some((rv.rep.pos + rv.rep.orient.apply(cp.pos), dir, pin))
        })
        .collect();
    out.sort_by_key(|(at, _, pin)| (at.x, pin.idx));
    out
}

/// Where the text `text` of a pin that a wire leaves along `dir` stands (module documentation, "Pin texts"): its
/// anchor from the pin, its angle and its alignment, inside the body [`PIN_TEXT_GAP`] from the pin. The one rule for a
/// black box's pin texts and for the number text the symbol editor gives a new connection point; `None` for a pin of
/// an unknown direction.
pub fn pin_text_place(dir: &Dir4, text: &str) -> Option<(Pt2, Angle2, Align)> {
    let g = PIN_TEXT_GAP;
    // A text of signs only ("+", "−") reads the same horizontally, and turned a minus would read as a tick.
    let signs = !text.chars().any(char::is_alphanumeric);
    Some(match dir {
        Dir4::Up if signs => (Pt2::new(Nm::ZERO, g), Angle2::ZERO, Align::TopCentre),
        Dir4::Down if signs => (Pt2::new(Nm::ZERO, Nm(-g.0)), Angle2::ZERO, Align::BottomCentre),
        Dir4::Up => (Pt2::new(Nm::ZERO, g), Angle2(90_000), Align::MiddleRight),
        Dir4::Down => (Pt2::new(Nm::ZERO, Nm(-g.0)), Angle2(90_000), Align::MiddleLeft),
        Dir4::Left => (Pt2::new(g, Nm::ZERO), Angle2::ZERO, Align::MiddleLeft),
        Dir4::Right => (Pt2::new(Nm(-g.0), Nm::ZERO), Angle2::ZERO, Align::MiddleRight),
        Dir4::Unknown(_) => return None,
    })
}

/// The pin texts of the page's black boxes, by representation ID then box-CP order (module documentation, "Pin
/// texts").
pub(crate) fn pin_texts(view: &PageView) -> Vec<PlacedPin> {
    let mut out = Vec::new();
    for rv in sorted_reps(view) {
        let rep = &rv.rep;
        let (Some(def), Some(b)) = (&rv.symbol, &rep.box_) else {
            continue;
        };
        if rv.pins.is_empty() || !def.cps.is_empty() {
            continue;
        }
        // A card drawn in cells: each channel pin's texts level in its cell, the designation and address next to the
        // pin and the function text under it inside the box, kept within the cell's width.
        let cells = cell_pins(rv);
        let pitch = cells
            .windows(2)
            .map(|w| w[1].0.x.0 - w[0].0.x.0)
            .filter(|d| *d > 0)
            .min();
        for (at, dir, pin) in &cells {
            let (Some(function_text), Some(pitch)) = (&pin.cell, pitch) else {
                continue;
            };
            let g = PIN_TEXT_GAP;
            let lines = |first: &str, second: &str| match (first.is_empty(), second.is_empty()) {
                (true, _) => second.to_owned(),
                (_, true) => first.to_owned(),
                _ => format!("{first}\n{second}"),
            };
            let (text, offset, align) = match dir {
                Dir4::Up => (
                    lines(&pin.text, function_text),
                    Pt2::new(Nm::ZERO, g),
                    Align::TopCentre,
                ),
                Dir4::Down => (
                    lines(function_text, &pin.text),
                    Pt2::new(Nm::ZERO, Nm(-g.0)),
                    Align::BottomCentre,
                ),
                _ => continue,
            };
            if !text.is_empty() {
                let width = Nm(pitch.saturating_sub(2 * g.0));
                out.push((
                    EntityRef(rep.id.as_uuid()),
                    text,
                    *at + offset,
                    Angle2::ZERO,
                    align,
                    Some(width),
                ));
            }
        }
        for cp in &b.cps {
            if pitch.is_some() && cells.iter().any(|(_, _, p)| p.idx == cp.idx) {
                continue;
            }
            let Some(pin) = rv.pins.iter().find(|p| p.idx == cp.idx) else {
                continue;
            };
            if pin.text.is_empty() {
                continue;
            }
            let at = rep.pos + rep.orient.apply(cp.pos);
            let Some((offset, angle, align)) =
                pin_text_place(&symbol::transform_dir(&cp.dir, rep.orient), &pin.text)
            else {
                continue;
            };
            out.push((
                EntityRef(rep.id.as_uuid()),
                pin.text.clone(),
                at + offset,
                angle,
                align,
                None,
            ));
        }
    }
    out
}

/// Where the text beside a plug's pin stands (module documentation of [`Builder::plug_notes`]): its anchor, angle
/// and alignment.
pub(crate) fn plug_note_place(n: &PlugNoteView) -> (Pt2, Angle2, Align) {
    let gap = Nm(1_000_000);
    let side = Nm(700_000);
    let out = if n.forward { gap } else { Nm(-gap.0) };
    match (n.vertical, n.forward) {
        (true, false) => (
            Pt2::new(n.at.x - side, n.at.y + out),
            Angle2(90_000),
            Align::MiddleLeft,
        ),
        (true, true) => (
            Pt2::new(n.at.x - side, n.at.y + out),
            Angle2(90_000),
            Align::MiddleRight,
        ),
        (false, true) => (
            Pt2::new(n.at.x + out, n.at.y - Nm(gap.0 / 2)),
            Angle2(0),
            Align::BottomLeft,
        ),
        (false, false) => (
            Pt2::new(n.at.x + out, n.at.y - Nm(gap.0 / 2)),
            Angle2(0),
            Align::BottomRight,
        ),
    }
}
