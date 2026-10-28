// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The arithmetic of the two work bounds of [`super::svg_hazard`] (FT-470, FT-471; doc 18 §11.1 TM-18): upper bounds
//! of what usvg 0.48.1 (with svgtypes 0.16.1, kurbo 0.13.1, simplecss 0.2.2 and roxmltree 0.21.1) does with an
//! admitted document, computed from the raw text without parsing it the way usvg does.
//!
//! - **Marker applications (FT-470).** usvg's `marker::convert` draws a marker at the first vertex (`marker-start`),
//!   the last (`marker-end`) and every vertex in between (`marker-mid`) of the path it makes of a `path`, `line`,
//!   `polyline`, `polygon`, `rect`, `circle` or `ellipse`, and converts the marker's content once per vertex.
//!   [`path_segments`] and [`shape_segments`] bound the segments of that path: one per command, one more for the
//!   move-to svgtypes inserts after a close-path, and for every elliptical arc the cubic pieces kurbo cuts it into
//!   ([`arc_pieces`]: an arc whose radii are too small for its end points is scaled up, so a tiny radius and a long
//!   chord make thousands of pieces). [`declaration_markers`] reads which markers a declaration list names.
//! - **Selector matching (FT-471).** usvg matches every simplecss rule against every element it parses (the
//!   document's elements and every clone a `use` makes), and simplecss walks the ancestors once per descendant
//!   combinator without memoisation, so a selector with `k` descendant combinators costs up to
//!   `C(ancestors, 0) + … + C(ancestors, k)` calls on one element. [`Selector`] is a selector's shape as the
//!   scanner can know it; [`selector_costs`] computes, per number of ancestors, the most work matching it can take
//!   ([`Stats`] holds the document facts each call's cost depends on).
//!
//! All arithmetic saturates at `u64::MAX`; a saturated bound is over every limit.

use std::collections::BTreeMap;

use super::{MAX_SVG_ARC_PIECES, MAX_SVG_DEPTH};

/// The marker targets an element's declarations name, per property: `marker-start`, `marker-mid`, `marker-end`.
pub(super) type MarkerTargets = [Vec<String>; 3];

/// The pieces kurbo 0.13.1 cuts an elliptical arc of (largest, already scaled) radius `radius` into, at most, plus
/// one: `Arc::append_iter` with usvg's tolerance 0.1 makes `ceil(max(3.999999, (1.1163 r / 0.1)^(1/6)) × sweep / 2π)`
/// cubics with `sweep ≤ 2π`. The radius is doubled first, a margin for rounding in the scaled radius and the current
/// point. A radius that is not finite gives a bound over every limit.
pub(super) fn radius_pieces(radius: f64) -> u64 {
    let n = (1.1163 * 2.0 * radius.abs() / 0.1).powf(1.0 / 6.0).max(4.0);
    if !n.is_finite() || n >= 1.0e15 {
        return u64::MAX / 8;
    }
    // In range: `n` is finite, at least 4 and below 1e15.
    n.ceil() as u64 + 1
}

/// The pieces of the elliptical arc svgtypes 0.16.1 hands kurbo (`SimplifyingPathParser`): from `from` to `to`
/// with radii `rx`, `ry`. A radius of at most 1e-5 makes a line (`SvgArc::is_straight_line`); otherwise kurbo
/// scales both radii by `sqrt(rf)` when `rf = p.x²/rx² + p.y²/ry² > 1` (SVG F.6.6.2), where `p` is half the chord
/// rotated, so `sqrt(rf) ≤ |half chord| / min(rx, ry)`. `slack` widens the half chord by the uncertainty of the
/// current point (see [`path_segments`]).
fn arc_pieces(from: (f64, f64), to: (f64, f64), rx: f64, ry: f64, slack: f64) -> (u64, f64) {
    let (rx, ry) = (rx.abs(), ry.abs());
    if rx <= 1.0e-5 || ry <= 1.0e-5 {
        return (1, 0.0);
    }
    let half = (to.0 - from.0).hypot(to.1 - from.1) / 2.0 + slack;
    let scaled = rx.max(ry) * (half / rx.min(ry)).max(1.0);
    (radius_pieces(scaled), scaled)
}

/// Whether `c` is white space to svgtypes' `Stream::skip_spaces`.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

/// A cursor over path data that reads numbers and flags as svgtypes 0.16.1 does (`Stream::parse_list_number`,
/// `parse_flag`).
struct PathCursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl PathCursor<'_> {
    fn skip_spaces(&mut self) {
        while self.i < self.b.len() && is_space(self.b[self.i]) {
            self.i += 1;
        }
    }

    /// `Stream::parse_number` then the list separator: an optional sign, digits or a `.`, a fraction, an exponent
    /// (an `e` or `E` not followed by `m` or `x`), read by `f64::from_str` and refused when not finite.
    fn list_number(&mut self) -> Option<f64> {
        self.skip_spaces();
        let start = self.i;
        let at = |k: usize| self.b.get(k).copied();
        let mut i = self.i;
        if matches!(at(i), Some(b'+' | b'-')) {
            i += 1;
        }
        match at(i)? {
            b'0'..=b'9' => {
                while at(i).is_some_and(|c| c.is_ascii_digit()) {
                    i += 1;
                }
            }
            b'.' => {}
            _ => return None,
        }
        if at(i) == Some(b'.') {
            i += 1;
            while at(i).is_some_and(|c| c.is_ascii_digit()) {
                i += 1;
            }
        }
        if matches!(at(i), Some(b'e' | b'E')) {
            let next = at(i + 1)?;
            if next != b'm' && next != b'x' {
                i += 1;
                match at(i)? {
                    b'+' | b'-' => {
                        i += 1;
                        while at(i).is_some_and(|c| c.is_ascii_digit()) {
                            i += 1;
                        }
                    }
                    b'0'..=b'9' => {
                        while at(i).is_some_and(|c| c.is_ascii_digit()) {
                            i += 1;
                        }
                    }
                    _ => return None,
                }
            }
        }
        let value: f64 = std::str::from_utf8(&self.b[start..i]).ok()?.parse().ok()?;
        if !value.is_finite() {
            return None;
        }
        self.i = i;
        self.skip_spaces();
        if self.b.get(self.i) == Some(&b',') {
            self.i += 1;
        }
        Some(value)
    }

    /// `parse_flag`: one `0` or `1`, then an optional comma and white space.
    fn flag(&mut self) -> Option<()> {
        self.skip_spaces();
        match self.b.get(self.i)? {
            b'0' | b'1' => {
                self.i += 1;
                if self.b.get(self.i) == Some(&b',') {
                    self.i += 1;
                }
                self.skip_spaces();
                Some(())
            }
            _ => None,
        }
    }
}

/// The number of runs of ASCII digits in `text`: an upper bound of the numbers svgtypes reads from it (every number
/// holds a digit, and two numbers never share a run).
pub(super) fn digit_runs(text: &str) -> u64 {
    let b = text.as_bytes();
    let mut runs = 0u64;
    for (i, c) in b.iter().enumerate() {
        if c.is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_digit()) {
            runs += 1;
        }
    }
    runs
}

/// The bound of [`path_segments`] for path data the cursor could not follow: every number a segment, every `z` a
/// segment; when an arc may be among them, unbounded (its pieces cannot be shown to stay within
/// [`MAX_SVG_ARC_PIECES`]).
fn rough_segments(rest: &str, arcs: bool) -> u64 {
    if arcs {
        return u64::MAX;
    }
    let numbers = digit_runs(rest);
    let closes = rest.bytes().filter(|c| matches!(c, b'z' | b'Z')).count() as u64;
    numbers.saturating_add(closes).saturating_add(1)
}

/// An upper bound of the segments usvg 0.48.1 makes of the path data `d` (in its original case): svgtypes'
/// `SimplifyingPathParser` gives one segment per command (a move-to's further pairs are line-tos), a move-to before
/// any command but a move-to or close-path that follows a close-path, none for a repeated close-path, and an arc's
/// cubic pieces ([`arc_pieces`]); tiny-skia's `PathBuilder` adds none. The cursor follows svgtypes' grammar and
/// arithmetic (relative coordinates add to the current point in `f64`); where svgtypes would stop at an error, the
/// rest is bounded by [`rough_segments`] instead, so a difference between the two readers only makes the bound larger.
/// After an arc the current point is kurbo's last piece end, not the arc's end point: the bound tracks a slack of a
/// millionth of the arc's scale and coordinates, which widens the chord of a later arc. An arc of more than
/// [`MAX_SVG_ARC_PIECES`] pieces makes the path unbounded (`u64::MAX`): svgtypes' buffer drain is quadratic in them.
pub(super) fn path_segments(d: &str) -> u64 {
    let mut s = PathCursor {
        b: d.as_bytes(),
        i: 0,
    };
    let mut prev_cmd: Option<u8> = None;
    let (mut x, mut y, mut mx, mut my) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    // The uncertainty of the current point and of the subpath's start.
    let (mut slack, mut slack_m) = (0.0f64, 0.0f64);
    let mut after_close = false;
    let mut segments = 0u64;
    loop {
        s.skip_spaces();
        let start = s.i;
        let Some(&first) = s.b.get(start) else {
            return segments;
        };
        let rough = |segments: u64, arcs: bool| segments.saturating_add(rough_segments(&d[start..], arcs));
        let is_cmd = b"MmZzLlHhVvCcSsQqTtAa".contains(&first);
        let (cmd, implicit_move) = match prev_cmd {
            None if matches!(first, b'M' | b'm') => {
                s.i += 1;
                (first, false)
            }
            None => return rough(segments, true),
            Some(_) if is_cmd => {
                s.i += 1;
                (first, false)
            }
            Some(p) if matches!(first, b'0'..=b'9' | b'.' | b'-' | b'+') => match p {
                b'Z' | b'z' => return rough(segments, true),
                b'M' => (b'L', true),
                b'm' => (b'l', true),
                p => (p, false),
            },
            Some(_) => return rough(segments, true),
        };
        let lower = cmd.to_ascii_lowercase();
        let relative = cmd.is_ascii_lowercase();
        let count = match lower {
            b'm' | b'l' | b't' => 2,
            b'h' | b'v' => 1,
            b's' | b'q' => 4,
            b'c' => 6,
            b'a' => 7,
            _ => 0,
        };
        let mut args = [0.0f64; 7];
        for (k, arg) in args.iter_mut().enumerate().take(count) {
            let ok = if lower == b'a' && (k == 3 || k == 4) {
                s.flag().is_some()
            } else {
                s.list_number().map(|v| *arg = v).is_some()
            };
            if !ok {
                return rough(segments, lower == b'a' || d[start..].contains(['a', 'A']));
            }
        }
        // A command other than a move-to or close-path right after a close-path: svgtypes inserts a move-to.
        if after_close && !matches!(lower, b'm' | b'z') {
            segments = segments.saturating_add(1);
        }
        let (dx, dy) = if relative { (x, y) } else { (0.0, 0.0) };
        match lower {
            b'm' => {
                let (bx, by) = if relative && after_close {
                    (mx, my)
                } else {
                    (dx, dy)
                };
                x = args[0] + bx;
                y = args[1] + by;
                (mx, my) = (x, y);
                if !relative {
                    slack = 0.0;
                }
                slack_m = slack;
                segments = segments.saturating_add(1);
            }
            b'z' => {
                if !after_close {
                    segments = segments.saturating_add(1);
                }
                (x, y) = (mx, my);
                slack = slack_m;
            }
            b'a' => {
                let to = (args[5] + dx, args[6] + dy);
                let to_slack = if relative { slack } else { 0.0 };
                let (pieces, scale) = arc_pieces((x, y), to, args[0], args[1], slack + to_slack);
                // svgtypes drains an arc's pieces from the front of a buffer: quadratic in their number.
                if pieces > MAX_SVG_ARC_PIECES {
                    return u64::MAX;
                }
                segments = segments.saturating_add(pieces);
                // kurbo's last piece ends near, not at, the end point.
                slack = to_slack + 1.0e-6 * (scale + x.abs() + y.abs() + to.0.abs() + to.1.abs());
                (x, y) = to;
            }
            _ => {
                let (ex, ey) = match lower {
                    b'l' | b't' => (args[0], args[1]),
                    b'h' => (args[0], if relative { 0.0 } else { y }),
                    b'v' => (if relative { 0.0 } else { x }, args[0]),
                    b's' | b'q' => (args[2], args[3]),
                    _ => (args[4], args[5]),
                };
                x = ex + dx;
                y = ey + dy;
                // An absolute command with both coordinates fixes the point exactly.
                if !relative && !matches!(lower, b'h' | b'v') {
                    slack = 0.0;
                }
                segments = segments.saturating_add(1);
            }
        }
        after_close = lower == b'z';
        prev_cmd = Some(if implicit_move {
            if relative { b'm' } else { b'M' }
        } else {
            cmd
        });
    }
}

/// An upper bound of the magnitude in user units of an SVG length attribute value (lower case): a number with no
/// unit, `px`, `in`, `cm`, `mm`, `pt` or `pc` at usvg's default 96 dpi; anything else (`em`, `ex` and `%` depend on
/// font sizes and viewports, which nested `em` sizes can make as large as `f32::MAX`; text that does not read as a
/// number) is `f32::MAX`. `auto` and an empty value are 0 (usvg then uses the other radius or none).
pub(super) fn length_bound(value: &str) -> f64 {
    let v = value.trim_ascii();
    if v.is_empty() || v == "auto" {
        return 0.0;
    }
    let mut cursor = PathCursor {
        b: v.as_bytes(),
        i: 0,
    };
    let Some(number) = cursor.list_number() else {
        return f64::from(f32::MAX);
    };
    let factor = match v[cursor.i..].trim_ascii() {
        "" | "px" => 1.0,
        "in" => 96.0,
        "cm" => 96.0 / 2.54,
        "mm" => 96.0 / 25.4,
        "pt" => 96.0 / 72.0,
        "pc" => 16.0,
        _ => return f64::from(f32::MAX),
    };
    (number.abs() * factor).min(f64::from(f32::MAX))
}

/// An upper bound of the segments of the path usvg makes of a shape element (`name` its lower-case local name,
/// `attrs` its local attribute names and values in their original case), or 0 for an element that is no shape:
/// `path` per [`path_segments`]; `line` 2; `polyline` one per number of `points` (a point takes two), `polygon` one
/// more for its close; `rect` 5 without radii, else four lines, four quarter arcs of the larger radius and its move
/// and close; `circle` and `ellipse` four quarter arcs of the (larger) radius, a move and a close. Radii are bounded
/// by [`length_bound`].
pub(super) fn shape_segments(name: &str, attrs: &[(&str, String)]) -> u64 {
    let value = |key: &str| {
        attrs
            .iter()
            .find(|(a, _)| *a == key)
            .map(|(_, v)| v.to_ascii_lowercase())
    };
    let radius = |keys: &[&str]| -> Option<f64> {
        keys.iter()
            .filter_map(|k| value(k))
            .map(|v| length_bound(&v))
            .reduce(f64::max)
    };
    let round = |r: f64| radius_pieces(r).saturating_mul(4).saturating_add(2);
    match name {
        "path" => attrs
            .iter()
            .find(|(a, _)| *a == "d")
            .map_or(0, |(_, d)| path_segments(d)),
        "line" => 2,
        "polyline" => value("points").map_or(0, |p| digit_runs(&p)),
        "polygon" => value("points").map_or(0, |p| digit_runs(&p).saturating_add(1)),
        "rect" => match radius(&["rx", "ry"]) {
            Some(r) => round(r).saturating_add(4),
            None => 5,
        },
        "circle" => round(radius(&["r"]).unwrap_or(0.0)),
        "ellipse" => round(radius(&["rx", "ry"]).unwrap_or(0.0)),
        _ => 0,
    }
}

/// The marker targets a declaration list (a `style` attribute or a rule body, lower case) names, per property. A
/// list with a comment, a quote, a backslash or a `;` inside parentheses can hide a declaration boundary from a split
/// on `;`, so there every `url(#…)` target counts for all three properties (when the list names a marker at all); a
/// declaration called `marker` (the shorthand) or any other name with `marker` in it counts for all three as well.
pub(super) fn declaration_markers(text: &str) -> MarkerTargets {
    let mut out: MarkerTargets = Default::default();
    if !text.contains("marker") {
        return out;
    }
    let semicolon_in_parentheses = {
        let mut depth = 0usize;
        let mut found = false;
        for c in text.bytes() {
            match c {
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                b';' if depth > 0 => found = true,
                _ => {}
            }
        }
        found
    };
    if text.contains("/*") || text.contains(['"', '\'', '\\']) || semicolon_in_parentheses {
        let targets = super::url_targets(text);
        for kind in &mut out {
            kind.extend(targets.iter().cloned());
        }
        return out;
    }
    for declaration in text.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !name.contains("marker") {
            continue;
        }
        let targets = super::url_targets(value);
        let kinds: &[usize] = match name {
            "marker-start" => &[0],
            "marker-mid" => &[1],
            "marker-end" => &[2],
            _ => &[0, 1, 2],
        };
        for &k in kinds {
            out[k].extend(targets.iter().cloned());
        }
    }
    out
}

/// The marker targets of a marker presentation attribute (lower-case local name and value): `marker-start`,
/// `marker-mid` and `marker-end` name their own property; `marker`, which usvg ignores as an attribute, counts for
/// all three.
pub(super) fn attribute_markers(name: &str, value: &str, out: &mut MarkerTargets) {
    let kinds: &[usize] = match name {
        "marker-start" => &[0],
        "marker-mid" => &[1],
        "marker-end" => &[2],
        "marker" => &[0, 1, 2],
        _ => return,
    };
    let targets = super::url_targets(value);
    for &k in kinds {
        out[k].extend(targets.iter().cloned());
    }
}

/// The combinator before a compound selector, as simplecss 0.2.2 matches it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Comb {
    /// The first compound.
    First,
    /// White space: the matcher tries every ancestor.
    Descendant,
    /// `>`: the parent.
    Child,
    /// `+`: the previous sibling element.
    Adjacent,
    /// A combinator the scanner cannot tell: counted as every ancestor, the element itself and the previous
    /// sibling, which is at least as much work as any of the three.
    Any,
}

/// A compound selector's cost factors: its text length (type-name comparisons), the attributes its attribute,
/// class and id subselectors read (`class`, `id` or the bracketed name, lower case), and its pseudo-classes
/// (`:first-child` looks for the previous sibling).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Compound {
    pub(super) len: u64,
    pub(super) named: Vec<String>,
    pub(super) pseudo: u64,
}

/// What the scanner knows of a selector: its exact compounds and combinators (a plain group without quotes,
/// parentheses or backslashes), or only a number of compounds that each cost at most one compound's factors and
/// are joined by [`Comb::Any`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Shape {
    Exact(Vec<(Comb, Compound)>),
    Uniform { parts: u64, compound: Compound },
}

/// One selector a style sheet may hold, `copies` times.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Selector {
    pub(super) shape: Shape,
    pub(super) copies: u64,
}

/// Whether `c` separates compounds (white space or a combinator character).
fn separates(c: char) -> bool {
    c.is_whitespace() || matches!(c, '>' | '+' | '~')
}

/// The cost factors of one compound's text.
fn compound_of(text: &str) -> Compound {
    let b = text.as_bytes();
    let mut c = Compound {
        len: text.len() as u64,
        ..Compound::default()
    };
    for (i, ch) in b.iter().enumerate() {
        match ch {
            b'.' => c.named.push("class".to_owned()),
            b'#' => c.named.push("id".to_owned()),
            b'[' => {
                let name: String = text[i + 1..]
                    .chars()
                    .take_while(|c| !(c.is_whitespace() || matches!(c, '=' | '~' | '|' | ']')))
                    .collect();
                c.named.push(name);
            }
            b':' => c.pseudo += 1,
            _ => {}
        }
    }
    c
}

/// The shape of one selector group of a plain style sheet (lower case, comments removed): exact when the group
/// holds no quote, parenthesis or backslash (simplecss's compounds then hold no white space and no combinator
/// character, so every compound is one of the scanner's parts), [`uniform`] otherwise.
pub(super) fn group_shape(group: &str) -> Shape {
    if group.contains(['"', '\'', '(', '\\']) {
        return uniform(group);
    }
    let mut parts: Vec<(Comb, Compound)> = Vec::new();
    let mut separator = String::new();
    let mut rest = group;
    while !rest.is_empty() {
        let sep_len = rest.find(|c: char| !separates(c)).unwrap_or(rest.len());
        separator.push_str(&rest[..sep_len]);
        rest = &rest[sep_len..];
        if rest.is_empty() {
            break;
        }
        let part_len = rest.find(separates).unwrap_or(rest.len());
        let comb = if parts.is_empty() {
            Comb::First
        } else if separator.contains('~') || (separator.contains('>') && separator.contains('+')) {
            Comb::Any
        } else if separator.contains('>') {
            Comb::Child
        } else if separator.contains('+') {
            Comb::Adjacent
        } else {
            Comb::Descendant
        };
        parts.push((comb, compound_of(&rest[..part_len])));
        separator.clear();
        rest = &rest[part_len..];
    }
    Shape::Exact(parts)
}

/// The conservative shape of text that may hold a selector: as many compounds as it has parts (split at white
/// space, `>`, `+`, `~`, `,`, `{` and `}`), each costing what the whole text could ([`compound_of`] of the whole
/// text: its length, every `.` a class read, `#` an id read, `[` a read of the attribute it names, `:` a
/// pseudo-class), joined by [`Comb::Any`].
pub(super) fn uniform(text: &str) -> Shape {
    let parts = text
        .split(|c: char| separates(c) || matches!(c, ',' | '{' | '}'))
        .filter(|p| !p.is_empty())
        .count() as u64;
    Shape::Uniform {
        parts,
        compound: compound_of(text),
    }
}

/// The class the last compound of a plain selector group requires (its first `.name`), when the name is plain
/// ASCII (`[a-z0-9_-]`, followed by an ASCII character that ends a simplecss identifier, or the end): the rule can
/// only match elements with that class token. `None` when the group may match an element without a class.
pub(super) fn last_class(group: &str) -> Option<String> {
    if group.contains(['"', '\'', '(', '\\']) {
        return None;
    }
    let last = group.rsplit(separates).find(|p| !p.is_empty())?;
    let dot = last.find('.')?;
    let rest = &last.as_bytes()[dot + 1..];
    let len = rest
        .iter()
        .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
        .count();
    let ends = rest.get(len).is_none_or(|c| c.is_ascii() && *c != b'\\');
    (len > 0 && ends).then(|| last[dot + 1..dot + 1 + len].to_owned())
}

/// The spans of `text` that may each be one whole simplecss 0.2.2 selector. simplecss starts a statement only at
/// the start, after a `}` (a block's end, also after error recovery), after a `;` (an at-rule without a block) or
/// after a `,` (the next selector of a list), and a valid selector ends only at a `,`, a `{`, a `/` that starts no
/// comment, or the end. Outside an attribute selector's brackets and `:lang(…)`, a valid selector holds none of `}`,
/// `;`, `"`, `'`, `(`, `)`, `!`, `%`, `@` or `=` (the tokenizer fails on them and the selector is dropped). So the
/// text is cut at every one of these characters and at the ends; a span that ends at a `,`, `{`, `/` or the end is a
/// candidate, a span that ends at one of the failing characters holds no valid selector. A `[…]` (with quoted
/// strings inside it) and a `:lang(…)` are kept whole; an unclosed one fails the selector and is a cut.
pub(super) fn selector_spans(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let (mut start, mut i) = (0, 0);
    // `close[j]`: the `]` a bracket scan that is outside a quote at `j` reaches, quoted values skipped whole, filled
    // from the end so every scan is one look-up (linear in the text however many brackets fail).
    let mut close: Vec<Option<usize>> = vec![None; b.len() + 1];
    let mut next_quote: [Option<usize>; 2] = [None, None];
    for j in (0..b.len()).rev() {
        close[j] = match b[j] {
            b']' => Some(j),
            b'"' | b'\'' => {
                let q = usize::from(b[j] == b'\'');
                next_quote[q].and_then(|k| close[k + 1])
            }
            _ => close[j + 1],
        };
        match b[j] {
            b'"' => next_quote[0] = Some(j),
            b'\'' => next_quote[1] = Some(j),
            _ => {}
        }
    }
    let last_paren = text.rfind(')');
    while i < b.len() {
        match b[i] {
            b'[' => match close[i + 1] {
                Some(end) => i = end + 1,
                None => {
                    i += 1;
                    start = i;
                }
            },
            b'(' if text[..i].ends_with(":lang") && last_paren.is_some_and(|p| p > i) => {
                // A `)` follows: the scan stops at the first one, past every `:lang(` before it.
                i += text[i + 1..].find(')').map_or(1, |k| k + 2);
            }
            b'{' | b',' | b'/' => {
                out.push(&text[start..i]);
                i += 1;
                start = i;
            }
            b'}' | b';' | b'"' | b'\'' | b'(' | b')' | b'!' | b'%' | b'@' | b'=' => {
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    out.push(&text[start..]);
    out.retain(|span| !span.trim().is_empty());
    out
}

/// The candidate selectors of a style sheet that is not plain (`text` its whole text in lower case, `clean` the
/// same without its comments): the [`selector_spans`] of `clean`, and of every comment's content after its first
/// `{`, `}` or `;` (error recovery and at-rules count braces and look for `;` without skipping comments, so a
/// statement can start right after such a character inside a comment, never before the first; it then ends at the
/// comment's closing `*` and `/` at the latest, the `*` a universal selector). A statement that starts outside a
/// comment skips a comment whole, as `clean` does. `None` when the sheet has a backslash or a comment closed right
/// before a `*` (`*/*`, which simplecss can read as the start of another comment): then a selector can span
/// anything.
pub(super) fn sheet_selectors<'a>(text: &'a str, clean: &'a str) -> Option<Vec<&'a str>> {
    if text.contains('\\') || text.contains("*/*") {
        return None;
    }
    let mut out = selector_spans(clean);
    let mut rest = text;
    while let Some(open) = rest.find("/*") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("*/") else {
            break;
        };
        if let Some(landing) = after[..close].find(['{', '}', ';']) {
            out.extend(selector_spans(&after[landing + 1..=close]));
        }
        rest = &after[close + 2..];
    }
    Some(out)
}

/// Whether a declaration list (lower case) may hold the `font` shorthand, which usvg expands into fourteen
/// attributes: a `font` not inside a longer identifier.
pub(super) fn may_hold_font_shorthand(text: &str) -> bool {
    let ident = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_') || c >= 0x80;
    let b = text.as_bytes();
    text.match_indices("font")
        .any(|(i, _)| !(i > 0 && ident(b[i - 1])) && !b.get(i + 4).is_some_and(|c| ident(*c)))
}

/// The document facts a match call's cost depends on.
pub(super) struct Stats<'a> {
    /// The most attributes an element has (roxmltree finds an attribute by a linear search).
    pub(super) attrs: u64,
    /// The longest value of each attribute name (lower case).
    pub(super) value_len: &'a BTreeMap<String, u64>,
    /// The most nodes that are not elements (text, comments, CDATA, processing instructions) between an element and
    /// its previous sibling element, plus one: what `prev_sibling_element` walks.
    pub(super) sibling_walk: u64,
}

/// The most work one `match_selector` call of a compound takes: one step, its text (type-name comparison), per
/// attribute read the search through the element's attributes and the comparison with the longest such value, and
/// per pseudo-class a walk to the previous sibling.
fn compound_cost(c: &Compound, s: &Stats<'_>) -> u64 {
    let read = |len: u64| 1u64.saturating_add(s.attrs).saturating_add(len);
    let named = c.named.iter().fold(0u64, |acc, name| {
        acc.saturating_add(read(s.value_len.get(name).copied().unwrap_or(0)))
    });
    1u64.saturating_add(c.len)
        .saturating_add(named)
        .saturating_add(c.pseudo.saturating_mul(s.sibling_walk))
}

/// The number of ancestor counts the bounds distinguish: an element has 0 to `MAX_SVG_DEPTH - 1` ancestors.
pub(super) const DEPTHS: usize = MAX_SVG_DEPTH;

/// For each number of ancestors `a`, the most work simplecss 0.2.2's `Selector::matches` can take on one element
/// with `a` ancestors (`matches_impl`, which backtracks without memoisation): `g(i, a)` for the compound `i` is its
/// own cost plus, by the combinator before it, the parent walk and `g(i − 1, a')` for every ancestor (`a' < a`), the
/// parent's `g(i − 1, a − 1)`, the sibling walk and `g(i − 1, a)` for the previous sibling (which has as many
/// ancestors), or all of these for [`Comb::Any`]. Assuming every compound but the first matches makes this an upper
/// bound of the real work.
pub(super) fn selector_costs(shape: &Shape, s: &Stats<'_>) -> [u64; DEPTHS] {
    let step = |prev: &[u64; DEPTHS], comb: Comb, cost: u64| -> [u64; DEPTHS] {
        let mut next = [0u64; DEPTHS];
        // `below` is the sum of `prev[a']` for every `a' < a`.
        let mut below = 0u64;
        for a in 0..DEPTHS {
            let walk = a as u64;
            next[a] = cost.saturating_add(match comb {
                Comb::First => 0,
                Comb::Descendant => walk.saturating_add(below),
                Comb::Child => 1u64.saturating_add(if a > 0 { prev[a - 1] } else { 0 }),
                Comb::Adjacent => s.sibling_walk.saturating_add(prev[a]),
                Comb::Any => walk
                    .saturating_add(s.sibling_walk)
                    .saturating_add(below)
                    .saturating_add(prev[a]),
            });
            below = below.saturating_add(prev[a]);
        }
        next
    };
    let mut g = [0u64; DEPTHS];
    match shape {
        Shape::Exact(parts) => {
            for (comb, compound) in parts {
                g = step(&g, *comb, compound_cost(compound, s));
            }
        }
        Shape::Uniform { parts, compound } => {
            let cost = compound_cost(compound, s);
            for i in 0..*parts {
                g = step(&g, if i == 0 { Comb::First } else { Comb::Any }, cost);
                if g[0] == u64::MAX {
                    break;
                }
            }
        }
    }
    g
}

#[cfg(test)]
mod tests;
