// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Value rendering rules (doc 09 §3.6): the display formatting of typed cells, applied late
//! (FR-I18N-009), so the same [`Cell`] prints "0.75 mm²" in an English PDF and "0,75 mm²" in an nl-NL one
//! while XLSX keeps the number.
//!
//! # Not modelled yet
//!
//! - The project date format (FR-PRJ-023): dates print as `DD-MM-YYYY`, the format of the golden title
//!   block; the ISO 8601 option arrives with the project setting.
//! - Number formats beyond the decimal separator: the ICU format of the output language also owns
//!   grouping; thousands separators are not printed.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{LangTag, MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{ColourCode, Device, OrderUnit, Unit};
use ew_derive::ew_units::Dec3;
use ew_derive::{DeviceTag, DisplayedTag, PageOrder, PageRef, Q, folio_label};

use crate::rowset::{Cell, MissingReason, QtyUnit};

/// Which form a [`Cell::Tag`] prints in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TagForm {
    /// The full designation (`=P01+J-E-2416-102G1`).
    Full,
    /// The displayed form (FR-NUM-002): the structure of the page omitted.
    #[default]
    Displayed,
}

/// The context of one rendering.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RenderOptions {
    /// The output language; `None` prints neutral text and the "." decimal separator.
    pub lang: Option<LangTag>,
    /// The form of tags.
    pub tag_form: TagForm,
    /// The text of [`Cell::Tbd`].
    pub tbd: String,
    /// The page being drawn, for the displayed form of tags (the structure of that page is omitted).
    pub page: Option<PageRef>,
}

impl Default for RenderOptions {
    fn default() -> RenderOptions {
        RenderOptions {
            lang: None,
            tag_form: TagForm::Displayed,
            tbd: "TBD".to_owned(),
            page: None,
        }
    }
}

/// Prints one cell (doc 09 §3.6).
///
/// Reads `PageOrder` for folios, so it must be called at draw or export time and **never from inside
/// pagination** (`page_order` runs the pagination: a read from there is a query cycle).
pub fn render(q: &Q, cell: &Cell, opts: &RenderOptions) -> String {
    match cell {
        Cell::Text(ml) => text(ml, opts.lang.as_ref()),
        Cell::Ident(s) | Cell::Address(s) => s.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Bool(b) => if *b { "yes" } else { "no" }.to_owned(),
        Cell::Tag(id) => tag(q, *id, opts),
        Cell::Folio(p) => folio(q, *p),
        Cell::Folios(ps) => folios(q, ps),
        Cell::CellRef(p, c) => format!("{}-{c}", folio(q, *p)),
        Cell::Qty(v, unit) => {
            let n = decimal(*v, uses_comma(opts.lang.as_ref()));
            match unit_symbol(unit) {
                "" => n,
                s => format!("{n} {s}"),
            }
        }
        Cell::Colour(c) => colour(c),
        Cell::Date(d) => format!("{:02}-{:02}-{:04}", d.day(), d.month(), d.year()),
        Cell::Range(ds) => compress_range(q, ds),
        Cell::Tbd => opts.tbd.clone(),
        Cell::Missing(MissingReason::Length) => "length missing".to_owned(),
        Cell::Missing(MissingReason::Part) => "part missing".to_owned(),
        Cell::Missing(MissingReason::CrossSection) => "cross-section missing".to_owned(),
        Cell::Missing(MissingReason::Colour) => "colour missing".to_owned(),
        Cell::Stale(v, _) => format!("≈ {}", render(q, v, opts)),
        Cell::Empty => String::new(),
    }
}

/// The text in `lang`, else the neutral text, else the first language's text, else "".
fn text(ml: &MlText, lang: Option<&LangTag>) -> String {
    lang.and_then(|l| ml.text_for(l))
        .or_else(|| ml.as_neutral())
        .or_else(|| ml.languages().next().and_then(|l| ml.get(l)))
        .unwrap_or_default()
        .to_owned()
}

fn colour(c: &ColourCode) -> String {
    match c {
        ColourCode::Iec(i) => i.as_str().to_owned(),
        ColourCode::Company(i) | ColourCode::Foreign(i) => i.as_str().to_owned(),
        ColourCode::Unknown(_) => String::new(),
    }
}

fn tag(q: &Q, id: Id<Device>, opts: &RenderOptions) -> String {
    let t = q.get::<DeviceTag>(id);
    let Some(t) = t.as_ref() else {
        return String::new();
    };
    match (opts.tag_form, opts.page) {
        (TagForm::Full, _) => t.full.clone(),
        (TagForm::Displayed, Some(PageRef::Drawn(p))) => (*q.get::<DisplayedTag>((id, p))).clone(),
        (TagForm::Displayed, _) => t.short(),
    }
}

/// The folio label as the page order prints it.
fn folio(q: &Q, p: PageRef) -> String {
    if let Some(e) = q.get::<PageOrder>(()).entry(p) {
        return e.folio.clone();
    }
    match p {
        PageRef::Drawn(id) => q.pages().get(id).map(|pg| folio_label(pg)).unwrap_or_default(),
        _ => String::new(),
    }
}

/// The Folios column of the BOM (doc 20 §14.1): runs of at least three pages that are consecutive in the
/// page order print "first–last" with an en dash and no spaces, shorter runs as a ", " list.
///
/// The doc's rows: "-201U1, -202U1" on "201, 202" (two pages: a list) and "-201U3-Q1 … -202U2-Q4" on
/// "203–206" (four consecutive pages: a range).
fn folios(q: &Q, ps: &[PageRef]) -> String {
    let order = q.get::<PageOrder>(());
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < ps.len() {
        let mut j = i;
        while j + 1 < ps.len() {
            match (order.position(ps[j]), order.position(ps[j + 1])) {
                (Some(a), Some(b)) if b == a + 1 => j += 1,
                _ => break,
            }
        }
        if j - i + 1 >= 3 {
            out.push(format!("{}–{}", folio(q, ps[i]), folio(q, ps[j])));
        } else {
            out.extend(ps[i..=j].iter().map(|p| folio(q, *p)));
        }
        i = j + 1;
    }
    out.join(", ")
}

/// The typographic symbol of a unit; empty for a pure number.
fn unit_symbol(u: &QtyUnit) -> &'static str {
    match u {
        QtyUnit::None => "",
        QtyUnit::Unit(u) => match u {
            Unit::Mm2 => "mm²",
            Unit::A => "A",
            Unit::V => "V",
            Unit::Hz => "Hz",
            Unit::Bar => "bar",
            Unit::DegC => "°C",
            Unit::Kg => "kg",
            Unit::W => "W",
            Unit::M => "m",
            Unit::Piece => "pc",
            Unit::Set => "set",
            Unit::Unknown(_) => "",
        },
        QtyUnit::Order(o) => match o {
            OrderUnit::Piece => "pc",
            OrderUnit::Metre => "m",
            OrderUnit::Set => "set",
            OrderUnit::Unknown(_) => "",
        },
    }
}

/// Whether the language writes a decimal comma.
fn uses_comma(lang: Option<&LangTag>) -> bool {
    lang.is_some_and(|l| {
        matches!(
            l.language(),
            "nl" | "de" | "fr" | "it" | "es" | "pt" | "da" | "sv" | "nb" | "fi" | "pl" | "cs"
        )
    })
}

/// A decimal without trailing fractional zeros or a trailing separator and without grouping:
/// 750 → "0.75", 1500 → "1.5", 2000 → "2", -5 → "-0.005".
pub fn decimal(v: Dec3, comma: bool) -> String {
    let abs = v.0.unsigned_abs();
    let (int, frac) = (abs / 1000, abs % 1000);
    let mut s = String::new();
    if v.0 < 0 {
        s.push('-');
    }
    s.push_str(&int.to_string());
    if frac != 0 {
        s.push(if comma { ',' } else { '.' });
        s.push_str(format!("{frac:03}").trim_end_matches('0'));
    }
    s
}

/// The tag class of a device: its letter code and its assembly root (the top of the `parent` chain), or
/// its strip for a terminal.
type Class = (String, Id<Device>);

fn class_of(q: &Q, id: Id<Device>) -> Option<Class> {
    let devices = q.devices();
    let dev = devices.get(id)?;
    let letter = dev.tag.letter.as_str().to_owned();
    if let Some(t) = &dev.aspects.terminal {
        return Some((letter, t.strip));
    }
    let mut seen = BTreeSet::new();
    let mut cur = id;
    seen.insert(cur);
    while let Some(p) = devices.get(cur).and_then(|d| d.parent) {
        if !seen.insert(p) || devices.get(p).is_none() {
            break;
        }
        cur = p;
    }
    Some((letter, cur))
}

fn short_tag(q: &Q, id: Id<Device>) -> Option<String> {
    q.get::<DeviceTag>(id).as_ref().as_ref().map(|t| t.short())
}

/// The compressed tag list of a [`Cell::Range`] (FR-REP-046, doc 09 §3.6).
///
/// Tags print in displayed form without structure. The class of a tag is its device's letter code plus
/// its assembly root (a terminal: its strip). Within a class the devices are ordered by the natural order
/// of the short tag (ties by device ID); a maximal run of the given devices that is consecutive in that
/// order, with no other device of the class between, prints "first … last" from three devices on
/// ("…" is U+2026 with spaces) and as a ", " list below that. A terminal run drops the strip prefix of its
/// last element ("-X1031:1 … 20"). Runs of several classes are ordered by the natural order of their first
/// tag.
pub fn compress_range(q: &Q, devices: &[Id<Device>]) -> String {
    let given: BTreeSet<Id<Device>> = devices.iter().copied().collect();
    let mut classes: BTreeSet<Class> = BTreeSet::new();
    for d in &given {
        if short_tag(q, *d).is_some() {
            if let Some(c) = class_of(q, *d) {
                classes.insert(c);
            }
        }
    }
    if classes.is_empty() {
        return String::new();
    }
    // Every tagged device of those classes, ordered per class.
    type Entry = (NaturalKey, Id<Device>, String);
    let mut members: BTreeMap<Class, Vec<Entry>> = BTreeMap::new();
    let all: Vec<Id<Device>> = q.devices().iter().map(|(id, _)| id).collect();
    for id in all {
        let Some(c) = class_of(q, id) else { continue };
        if !classes.contains(&c) {
            continue;
        }
        if let Some(s) = short_tag(q, id) {
            members.entry(c).or_default().push((natural_key(&s), id, s));
        }
    }
    let mut runs: Vec<(NaturalKey, Id<Device>, String)> = Vec::new();
    for list in members.values_mut() {
        list.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut i = 0;
        while i < list.len() {
            if !given.contains(&list[i].1) {
                i += 1;
                continue;
            }
            let mut j = i;
            while j + 1 < list.len() && given.contains(&list[j + 1].1) {
                j += 1;
            }
            let text = if j - i + 1 >= 3 {
                let (first, last) = (&list[i].2, &list[j].2);
                let tail = match (first.split_once(':'), last.split_once(':')) {
                    (Some((a, _)), Some((b, t))) if a == b => t,
                    _ => last.as_str(),
                };
                format!("{first} … {tail}")
            } else {
                list[i..=j]
                    .iter()
                    .map(|e| e.2.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            runs.push((list[i].0.clone(), list[i].1, text));
            i = j + 1;
        }
    }
    runs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    runs.into_iter().map(|r| r.2).collect::<Vec<_>>().join(", ")
}
