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
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{LangTag, MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{ColourCode, ColourSpec, DecimalSeparator, Device, OrderUnit, Unit};
use ew_derive::ew_units::Dec3;
use ew_derive::{DeviceTag, DisplayedTag, PageOrder, PageRef, Policy, Q, Query, folio_label};

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
    /// The output language; `None` prints neutral text and the decimal separator of the project's formatting
    /// settings (the point when it sets none).
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
            let n = decimal(*v, writes_comma(q, opts.lang.as_ref()));
            match unit_symbol(unit) {
                "" => n,
                s => format!("{n} {s}"),
            }
        }
        Cell::Colour(c) => colour_text(c),
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

/// The codes of a colour joined without a separator (`GNYE`); a code of a newer format prints its raw value.
/// The printed text and the sort and group text of a colour cell are this one text.
pub(crate) fn colour_text(c: &ColourSpec) -> String {
    c.codes()
        .iter()
        .map(|c| match c {
            ColourCode::Iec(i) => i.as_str().to_owned(),
            ColourCode::Company(i) | ColourCode::Foreign(i) => i.as_str().to_owned(),
            ColourCode::Unknown(v) => v.to_string(),
        })
        .collect()
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

/// Whether a quantity prints with a decimal comma: the output language decides when there is one, else the
/// project's `settings.formatting.decimal_separator` (FT-366; absent or `point`: the point).
fn writes_comma(q: &Q, lang: Option<&LangTag>) -> bool {
    match lang {
        Some(_) => uses_comma(lang),
        None => matches!(
            q.project().settings.formatting.decimal_separator,
            Some(DecimalSeparator::Comma)
        ),
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
pub type Class = (String, Id<Device>);

/// One tagged device of a class: the natural key of its short tag, the device and the short tag.
pub type ClassMember = (NaturalKey, Id<Device>, String);

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

/// `tag_class_members()`: every tagged device of the project grouped by its tag [`Class`], each class
/// ordered by the natural order of the short tag (ties by device ID).
///
/// [`compress_range`] reads it, so a report that prints many [`Cell::Range`] cells walks the devices once
/// per [`Q`] instead of once per cell.
#[derive(Debug)]
pub struct TagClassMembers;

impl Query for TagClassMembers {
    type Key = ();
    type Value = BTreeMap<Class, Vec<ClassMember>>;
    const FAMILY: &'static str = "tag_class_members";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _key: &()) -> Arc<Self::Value> {
        let mut members: BTreeMap<Class, Vec<ClassMember>> = BTreeMap::new();
        let all: Vec<Id<Device>> = q.devices().iter().map(|(id, _)| id).collect();
        for id in all {
            let Some(c) = class_of(q, id) else { continue };
            if let Some(s) = short_tag(q, id) {
                members.entry(c).or_default().push((natural_key(&s), id, s));
            }
        }
        for list in members.values_mut() {
            list.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        }
        Arc::new(members)
    }
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
    // Every tagged device of those classes, ordered per class (memoised for the whole project).
    let all = q.get::<TagClassMembers>(());
    let mut runs: Vec<(NaturalKey, Id<Device>, String)> = Vec::new();
    for class in &classes {
        let Some(list) = all.get(class) else { continue };
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

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use ew_derive::RecordingTracker;
    use ew_derive::ew_ids::Uuid;
    use ew_derive::ew_schema::{Project, Record};
    use serde_json::json;

    use super::*;

    fn uid(n: u128) -> String {
        Uuid::from_u128(n).hyphenated().to_string()
    }

    fn dev(n: u128) -> Id<Device> {
        Id::from_uuid(Uuid::from_u128(n))
    }

    /// Strip `-X1` with terminals 1 to 6 (devices 0x11 to 0x16) and a second strip `-X2` with one terminal
    /// (0x21).
    fn model() -> ew_model::Model {
        let project: Project =
            serde_json::from_value(json!({"id": uid(1), "derivation_profile": "2027.1"})).unwrap();
        let mut records = Vec::new();
        for (strip, text) in [(0x10u128, "X1"), (0x20, "X2")] {
            records.push(json!({
                "$k": "device", "id": uid(strip),
                "tag": {"letter": "X", "frozen": {"text": text}},
                "strip": {},
            }));
        }
        let terminals = [
            (0x11u128, 0x10u128, 1u32),
            (0x12, 0x10, 2),
            (0x13, 0x10, 3),
            (0x14, 0x10, 4),
            (0x15, 0x10, 5),
            (0x16, 0x10, 6),
            (0x21, 0x20, 1),
        ];
        for (n, strip, seq) in terminals {
            records.push(json!({
                "$k": "device", "id": uid(n), "tag": {"letter": "X"},
                "terminal": {"strip": uid(strip), "seq": seq, "designation": seq.to_string()},
            }));
        }
        let records: Vec<Record> = records
            .into_iter()
            .map(|r| serde_json::from_value(r).unwrap())
            .collect();
        ew_model::Model::from_records(project, records, []).unwrap()
    }

    #[test]
    fn a_run_of_three_terminals_compresses_and_a_gap_breaks_it() {
        let m = model();
        let q = Q::new(&m);
        let t = |ns: &[u128]| compress_range(&q, &ns.iter().map(|n| dev(*n)).collect::<Vec<_>>());
        assert_eq!(t(&[0x11, 0x12, 0x13]), "-X1:1 … 3");
        assert_eq!(t(&[0x13, 0x11, 0x12, 0x14]), "-X1:1 … 4");
        assert_eq!(t(&[0x11, 0x12]), "-X1:1, -X1:2");
        // 0x14 is missing: the run is cut, and the single device after the gap stands alone.
        assert_eq!(t(&[0x11, 0x12, 0x13, 0x15, 0x16]), "-X1:1 … 3, -X1:5, -X1:6");
        // Another strip is another class.
        assert_eq!(t(&[0x11, 0x21]), "-X1:1, -X2:1");
        assert_eq!(t(&[]), "");
        assert_eq!(t(&[0x999]), "");
    }

    #[test]
    fn tag_class_members_is_evaluated_once_for_many_ranges() {
        let m = model();
        let rec = Rc::new(RecordingTracker::new());
        let mut q = Q::with_tracker(&m, Box::new(Rc::clone(&rec)));
        for _ in 0..5 {
            assert_eq!(
                compress_range(&q, &[dev(0x11), dev(0x12), dev(0x13)]),
                "-X1:1 … 3"
            );
            assert_eq!(compress_range(&q, &[dev(0x21)]), "-X2:1");
        }
        assert_eq!(q.diagnostics().cached("tag_class_members"), 1);
        let evaluated = rec.evaluated();
        let runs = evaluated
            .iter()
            .filter(|(f, _)| *f == "tag_class_members")
            .count();
        assert_eq!(runs, 1, "{evaluated:?}");
    }
}
