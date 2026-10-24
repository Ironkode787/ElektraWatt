// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Wire marker tool (`marker`; WP-S2.04 §10.5, §10.7; doc 06 §4.5): a rubber line across the wires whose
//! crossing places one group marker (`marker.place`) over them, in the order the line crosses them.
//!
//! **The crossing rule** ([`crossed_connections`]). For each connection of the page tables, in table order, and each
//! segment `P→Q` of its polyline: the segment counts when it is not parallel to `A→B` and the two closed segments
//! intersect (the four orientation signs; a segment lying on the line is parallel and never counts, one that ends on
//! the line does). Its parameter along `A→B` is `t = cross(P−A, Q−P) / cross(B−A, Q−P)`; a connection's key is its
//! smallest `t` with the segment that gives it, compared as a fraction in `i128`. The crossings are sorted by `t`,
//! ties by the tables' connection order.
//!
//! **The states.** All points are page nm, exact: the crossing ends are the exact page points of the pointer or of
//! the grid cursor, never snapped; upon release the marker point is the crossing line's midpoint, fine-snapped with
//! `FineKind::Text`. The overlay is in sheet nm.
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | armed | `PointerMove`, `CursorMove` to p | armed (at p) | `Transient`: a grid marker |
//! | armed | `Press` primary at p | crossing (from p, to p) | `Transient`: the line and its crossings |
//! | armed | `Activate` with a known point p | crossing (from p, to p) | `Transient` |
//! | armed | `Activate` before any point; `Release` | armed | `None` |
//! | crossing | `PointerMove`, `CursorMove` to p | crossing (to p) | `Transient`: the dashed line, a `cp` marker per crossing, a `<n> crossed` label |
//! | crossing | `Release` primary, or `Activate` | typing, or armed (at = to) when nothing is crossed | typing: `Transient` (the line and a grid marker at the marker point); nothing: `Cancel` |
//! | typing | `Activate` or `Key(Finish)` | armed (at = the marker point) | `Commit(marker.place …)` |
//! | typing | `Dwell { ms ≥ 80 }` | typing | `Preview(marker.place …)` |
//! | typing | any move, `Press`, `Release`, `Back` | typing | `None` |
//! | crossing or typing | `Escape` | armed | `Cancel` |
//! | armed | `Escape` | — | `None` (the host returns to Select) |
//!
//! **The fields.** `marker.fields` sets the cross-section (`csa`) and the colours (each entry one or two codes); the
//! ghost says them (`2.5 BK BU`). The angle is 90° when every crossed segment is horizontal, else 0.

use ew_api::AnyCommand;
use ew_api::output::PageTables;
use ew_api::output::ew_dl::EntityRef;
use serde_json::{Map, Value, json};

use super::{
    Anchor, Button, CanvasEvent, FineKind, Ghost, MarkerKind, Overlay, OverlayItem, Style, Token, Tool,
    ToolCx, ToolKey, ToolOutput,
};

/// `a × b` in the plane, in `i128`.
fn cross(a: (i128, i128), b: (i128, i128)) -> i128 {
    a.0 * b.1 - a.1 * b.0
}

/// The point `a + b`, in `i128`.
fn sub(a: (i128, i128), b: (i128, i128)) -> (i128, i128) {
    (a.0 - b.0, a.1 - b.1)
}

/// `num / den` rounded to the nearest integer, halves to the even neighbour; `den` is positive.
fn div_round_half_even(num: i128, den: i128) -> i128 {
    let q = num.div_euclid(den);
    let r = num.rem_euclid(den);
    match (2 * r).cmp(&den) {
        std::cmp::Ordering::Greater => q + 1,
        std::cmp::Ordering::Equal if q % 2 != 0 => q + 1,
        _ => q,
    }
}

/// An `i128` as an `i64`, saturating.
fn sat(v: i128) -> i64 {
    i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
}

/// One crossing of the crossing line by a connection: its table ID, its key as a fraction, the segment's
/// horizontality and the crossing point on the sheet's page (nm, rounded).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Crossing {
    id: EntityRef,
    /// The key `num / den`, `den > 0`.
    num: i128,
    den: i128,
    horizontal: bool,
    at: (i64, i64),
}

/// The crossings of the segment `from`–`to` (page nm), sorted (see [`crossed_connections`]).
fn crossings(tables: &PageTables, from: (i64, i64), to: (i64, i64)) -> Vec<Crossing> {
    let a = (i128::from(from.0), i128::from(from.1));
    let b = (i128::from(to.0), i128::from(to.1));
    let d = sub(b, a);
    let mut out: Vec<Crossing> = Vec::new();
    for c in &tables.connections {
        let mut best: Option<(i128, i128, bool)> = None;
        for w in c.polyline.windows(2) {
            let p = (i128::from(w[0].0), i128::from(w[0].1));
            let q = (i128::from(w[1].0), i128::from(w[1].1));
            let e = sub(q, p);
            let mut den = cross(d, e);
            // A segment parallel to the line (one lying on it included) never counts.
            if den == 0 {
                continue;
            }
            // The two closed segments intersect: no strict separation on either line.
            let o1 = cross(d, sub(p, a)).signum();
            let o2 = cross(d, sub(q, a)).signum();
            if o1 * o2 > 0 {
                continue;
            }
            let o3 = cross(e, sub(a, p)).signum();
            let o4 = cross(e, sub(b, p)).signum();
            if o3 * o4 > 0 {
                continue;
            }
            let mut num = cross(sub(p, a), e);
            if den < 0 {
                num = -num;
                den = -den;
            }
            let better = match best {
                None => true,
                Some((bn, bd, _)) => num * bd < bn * den,
            };
            if better {
                best = Some((num, den, w[0].1 == w[1].1));
            }
        }
        if let Some((num, den, horizontal)) = best {
            out.push(Crossing {
                id: EntityRef(c.id),
                num,
                den,
                horizontal,
                at: (
                    sat(a.0 + div_round_half_even(num * d.0, den)),
                    sat(a.1 + div_round_half_even(num * d.1, den)),
                ),
            });
        }
    }
    // A stable sort: equal keys keep the tables' connection order.
    out.sort_by(|x, y| (x.num * y.den).cmp(&(y.num * x.den)));
    out
}

/// The connections of `tables` that the segment `from`–`to` (page nm) crosses, in the order of the crossing along it
/// from `from`, each once at its first crossing; with each, whether the crossed segment is horizontal.
pub fn crossed_connections(tables: &PageTables, from: (i64, i64), to: (i64, i64)) -> Vec<(EntityRef, bool)> {
    crossings(tables, from, to)
        .into_iter()
        .map(|c| (c.id, c.horizontal))
        .collect()
}

/// The midpoint of `a` and `b`: `(a + b).div_euclid(2)` per axis, in `i128`.
fn midpoint(a: (i64, i64), b: (i64, i64)) -> (i64, i64) {
    (
        sat((i128::from(a.0) + i128::from(b.0)).div_euclid(2)),
        sat((i128::from(a.1) + i128::from(b.1)).div_euclid(2)),
    )
}

/// The marker tool's state.
#[derive(Clone, PartialEq, Eq, Debug)]
enum State {
    /// No point fixed yet, or the point the next crossing line starts at (page nm).
    Armed { at: Option<(i64, i64)> },
    /// The crossing line's ends (page nm).
    Crossing { from: (i64, i64), to: (i64, i64) },
    /// The crossed connections, the marker point (page nm) and its angle (`0` or `90_000`).
    Typing {
        connections: Vec<EntityRef>,
        at: (i64, i64),
        angle: i32,
    },
}

/// The Wire marker tool (Shift+W): a rubber line across the wires, its fields in the tool strip, Enter places them.
#[derive(Clone, Debug)]
pub struct MarkerTool {
    state: State,
    csa: Option<String>,
    colours: Vec<Vec<String>>,
    /// How many connections the crossing line meets now (the ghost's count while crossing).
    crossed: usize,
}

impl MarkerTool {
    /// The tool, armed, without fields.
    pub fn new() -> MarkerTool {
        MarkerTool {
            state: State::Armed { at: None },
            csa: None,
            colours: Vec::new(),
            crossed: 0,
        }
    }

    /// The fields (the host parses them): `None` leaves a field as it is, `Some(None)`/`Some(vec![])` clears it.
    pub fn set_fields(&mut self, csa: Option<Option<String>>, colours: Option<Vec<Vec<String>>>) {
        if let Some(csa) = csa {
            self.csa = csa;
        }
        if let Some(colours) = colours {
            self.colours = colours;
        }
    }

    /// The fields' text (`2.5 BK BU`); `None` when both fields are empty.
    fn field_text(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if let Some(csa) = &self.csa {
            parts.push(csa.clone());
        }
        for colours in &self.colours {
            parts.push(colours.join("+"));
        }
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    /// The current point (page nm).
    fn point(&self) -> Option<(i64, i64)> {
        match self.state {
            State::Armed { at } => at,
            State::Crossing { to, .. } => Some(to),
            State::Typing { at, .. } => Some(at),
        }
    }

    /// The number of connections the ghost says: the crossed ones, 0 in the armed state.
    fn connection_count(&self) -> usize {
        match &self.state {
            State::Typing { connections, .. } => connections.len(),
            State::Crossing { .. } => self.crossed,
            State::Armed { .. } => 0,
        }
    }

    /// The overlay of the state: the crossing line with its crossings, else the point's grid marker.
    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let Some(at) = self.point() else {
            return Overlay::default();
        };
        let sheet = |p: (i64, i64)| cx.snapper.page_to_sheet(p);
        let mut items = Vec::new();
        if let State::Crossing { from, to } = &self.state {
            items.push(OverlayItem::Line {
                a: sheet(*from),
                b: sheet(*to),
                style: Style::dashed(Token::Ghost),
            });
            let crossed = crossings(&cx.page.tables, *from, *to);
            for c in &crossed {
                items.push(OverlayItem::Marker {
                    at: sheet(c.at),
                    kind: MarkerKind::Cp,
                });
            }
            items.push(OverlayItem::Label {
                at: sheet(*to),
                text: crate::strings::format("marker-crossed", &[&crossed.len().to_string()]),
                anchor: Anchor::Right,
            });
            return Overlay::new(items);
        }
        items.push(OverlayItem::Marker {
            at: sheet(at),
            kind: MarkerKind::Grid,
        });
        Overlay::new(items)
    }

    /// The overlay at the current point, after a field changed.
    pub fn refresh(&mut self, cx: &ToolCx<'_>) -> Overlay {
        self.overlay(cx)
    }

    /// The `marker.place` of the typed marker.
    fn command(
        &self,
        cx: &ToolCx<'_>,
        connections: &[EntityRef],
        at: (i64, i64),
        angle: i32,
    ) -> Box<AnyCommand> {
        let ids: Vec<String> = connections.iter().map(|e| e.0.to_string()).collect();
        let mut params = Map::new();
        params.insert("page".into(), Value::from(cx.page.tables.page.to_string()));
        params.insert("connections".into(), json!(ids));
        params.insert("at".into(), json!([at.0, at.1]));
        if angle == 90_000 {
            params.insert("angle".into(), json!(90_000));
        }
        if let Some(csa) = &self.csa {
            params.insert("csa".into(), Value::from(csa.clone()));
        }
        if !self.colours.is_empty() {
            params.insert("colours".into(), json!(self.colours));
        }
        Box::new(AnyCommand {
            name: "marker.place".into(),
            params: Value::Object(params),
        })
    }

    /// A move to `at` (sheet nm): the exact page point, never snapped.
    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let p = cx.snapper.sheet_to_page(at);
        let changed = match &mut self.state {
            State::Armed { at } => {
                *at = Some(p);
                true
            }
            State::Crossing { from, to } => {
                *to = p;
                let from = *from;
                let to = *to;
                self.crossed = crossings(&cx.page.tables, from, to).len();
                true
            }
            State::Typing { .. } => false,
        };
        if changed {
            ToolOutput::Transient(self.overlay(cx))
        } else {
            ToolOutput::None
        }
    }

    /// A click at the current point: start the crossing line, or take the crossed wires.
    fn click(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        match &self.state {
            State::Armed { at: Some(at) } => {
                self.state = State::Crossing { from: *at, to: *at };
                self.crossed = 0;
                ToolOutput::Transient(self.overlay(cx))
            }
            State::Armed { at: None } => ToolOutput::None,
            State::Crossing { from, to } => {
                let (from, to) = (*from, *to);
                self.take(from, to, cx)
            }
            State::Typing { .. } => ToolOutput::None,
        }
    }

    /// The release or `Activate` of the crossing line: typing over its crossings, else back to armed.
    fn take(&mut self, from: (i64, i64), to: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let crossed = crossings(&cx.page.tables, from, to);
        if crossed.is_empty() {
            self.state = State::Armed { at: Some(to) };
            self.crossed = 0;
            return ToolOutput::Cancel;
        }
        let mid = midpoint(from, to);
        let at = cx
            .snapper
            .snap_fine(cx.snapper.page_to_sheet(mid), FineKind::Text, false)
            .1;
        let angle = if crossed.iter().all(|c| c.horizontal) {
            90_000
        } else {
            0
        };
        let connections: Vec<EntityRef> = crossed.iter().map(|c| c.id).collect();
        self.state = State::Typing {
            connections,
            at,
            angle,
        };
        self.crossed = 0;
        let sheet = |p: (i64, i64)| cx.snapper.page_to_sheet(p);
        ToolOutput::Transient(Overlay::new(vec![
            OverlayItem::Line {
                a: sheet(from),
                b: sheet(to),
                style: Style::dashed(Token::Ghost),
            },
            OverlayItem::Marker {
                at: sheet(at),
                kind: MarkerKind::Grid,
            },
        ]))
    }

    /// Enter or `tool.finish` in the typing state.
    fn finish(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let State::Typing {
            connections,
            at,
            angle,
        } = &self.state
        else {
            return ToolOutput::None;
        };
        let (connections, at, angle) = (connections.clone(), *at, *angle);
        self.state = State::Armed { at: Some(at) };
        self.crossed = 0;
        ToolOutput::Commit(self.command(cx, &connections, at, angle))
    }

    /// The dwell: the typing marker's preview.
    fn preview(&self, cx: &ToolCx<'_>) -> ToolOutput {
        match &self.state {
            State::Typing {
                connections,
                at,
                angle,
            } => ToolOutput::Preview(self.command(cx, connections, *at, *angle)),
            _ => ToolOutput::None,
        }
    }
}

impl Default for MarkerTool {
    fn default() -> MarkerTool {
        MarkerTool::new()
    }
}

impl Tool for MarkerTool {
    fn name(&self) -> &'static str {
        "marker"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "armed",
            State::Crossing { .. } => "crossing",
            State::Typing { .. } => "typing",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, .. } => self.moved(*at, cx),
            CanvasEvent::CursorMove { at } => self.moved(*at, cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                ..
            } => {
                let _ = self.moved(*at, cx);
                self.click(cx)
            }
            CanvasEvent::Activate => self.click(cx),
            CanvasEvent::Release {
                button: Button::Primary,
                ..
            } => match &self.state {
                State::Crossing { from, to } => {
                    let (from, to) = (*from, *to);
                    self.take(from, to, cx)
                }
                _ => ToolOutput::None,
            },
            CanvasEvent::Key(ToolKey::Finish) => self.finish(cx),
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => self.preview(cx),
            CanvasEvent::Escape => match &self.state {
                State::Crossing { to, .. } => {
                    self.state = State::Armed { at: Some(*to) };
                    self.crossed = 0;
                    ToolOutput::Cancel
                }
                State::Typing { at, .. } => {
                    self.state = State::Armed { at: Some(*at) };
                    self.crossed = 0;
                    ToolOutput::Cancel
                }
                State::Armed { .. } => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "hint-marker-armed",
            State::Crossing { .. } => "hint-marker-crossing",
            State::Typing { .. } => "hint-marker-typing",
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::Marker {
            connections: self.connection_count(),
            text: self.field_text(),
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_api::output::{TableConnection, TableEnd};

    /// A UUID-typed value from a number (the type is inferred from the field).
    fn uuid<T: std::str::FromStr>(n: u128) -> T
    where
        T::Err: std::fmt::Debug,
    {
        format!("{:08x}-0000-4000-8000-{:012x}", n >> 48, n & 0xffff_ffff_ffff)
            .parse()
            .unwrap()
    }

    /// A page with the connections of `paths` and nothing else.
    fn page(paths: &[&[(i64, i64)]]) -> PageTables {
        PageTables {
            folio: "101".into(),
            page: uuid(1),
            grid: (1_000_000, 1),
            reps: Vec::new(),
            cps: Vec::new(),
            connections: paths
                .iter()
                .enumerate()
                .map(|(i, polyline)| TableConnection {
                    id: uuid(0x100 + i as u128),
                    a: TableEnd::Junction { id: uuid(2) },
                    b: TableEnd::Junction { id: uuid(3) },
                    kind: "wire".into(),
                    polyline: polyline.to_vec(),
                    stored_route: true,
                })
                .collect(),
            junctions: Vec::new(),
            items: Vec::new(),
            structure_boxes: Vec::new(),
        }
    }

    /// The IDs of [`crossed_connections`] as their UUID text.
    fn crossed(t: &PageTables, from: (i64, i64), to: (i64, i64)) -> Vec<String> {
        crossed_connections(t, from, to)
            .into_iter()
            .map(|(e, _)| e.0.to_string())
            .collect()
    }

    #[test]
    fn crossing_order_follows_the_line() {
        // Two vertical wires at x = 10 and x = 20, and one lying on the crossing line.
        let t = page(&[&[(10, 0), (10, 30)], &[(20, 0), (20, 30)], &[(0, 15), (30, 15)]]);
        let w10 = t.connections[0].id.to_string();
        let w20 = t.connections[1].id.to_string();
        // Left to right: w10, then w20.
        assert_eq!(crossed(&t, (0, 15), (30, 15)), [w10.clone(), w20.clone()]);
        // Right to left: w20, then w10.
        assert_eq!(crossed(&t, (30, 15), (0, 15)), [w20, w10]);
        // The wire lying on the line is never crossed; both vertical ones are, and their crossed segments are
        // vertical.
        let both = crossed_connections(&t, (0, 15), (30, 15));
        assert_eq!(both.len(), 2);
        assert!(both.iter().all(|(_, horizontal)| !*horizontal));
        // A line across the top ends before the wires start: no crossing.
        assert!(crossed(&t, (0, 35), (30, 35)).is_empty());
    }

    #[test]
    fn a_segment_ending_on_the_line_is_crossed() {
        // A wire whose end lies exactly on the crossing line.
        let t = page(&[&[(10, 0), (10, 15)]]);
        let id = t.connections[0].id.to_string();
        assert_eq!(crossed(&t, (0, 15), (30, 15)), vec![id.clone()]);
        // The crossing point is the wire's end, rounded to nm.
        let hits = crossings(&t, (0, 15), (30, 15));
        assert_eq!(hits[0].at, (10, 15));
        assert!(!hits[0].horizontal);
        // A horizontal line that ends on the wire's end crosses it too (the end is the crossing point).
        assert_eq!(crossed(&t, (0, 15), (10, 15)), [id]);
        // A wire that stops just short is not crossed.
        let t = page(&[&[(10, 0), (10, 14)]]);
        assert!(crossed_connections(&t, (0, 15), (30, 15)).is_empty());
    }
}
