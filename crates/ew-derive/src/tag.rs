// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device tags (doc 03 §8.2–§8.5, FR-NUM-001, FR-NUM-002, FR-DEV-021, ADR-0026, D-27, D-41).
//!
//! **Agreement with the command layer.** The counter of a tag is assigned by the online-numbering
//! normaliser of `ew-commands` (ID-48) in the scope (page of the main representation, letter). For the
//! printed tags to be unique wherever the counters are, `device_tag` uses the same two rules as
//! `ew_commands::numbering`: the main function is the device's function with role `Main` and the smallest
//! ID, and the main representation is the smallest-ID representation that draws it, contact images
//! excluded (doc 03 §5.1: a contact image never counts as the main representation). The richer rule of
//! doc 03 §8.2 step 4 (lowest page order, row, column among the counted representation types) replaces
//! both at once with WP-S1.11; `tests/agreement.rs` holds the two layers together until then.
//!
//! **Schemes the command layer does not number.** The command layer numbers per (page of the main
//! representation, letter) and nothing else (ID-48). A scheme is therefore applied only when that makes
//! its tags unique wherever the counters are: its counter scope is `"page"` and its format and every
//! rule's format contain `{page}`, `{letter}` and `{counter}`. Any other scheme (from the project's
//! settings or named by `TagScheme::Named`) is skipped with a [`DeriveNote::SchemeNotApplied`] and the
//! pinned profile's default scheme formats the tag (decision log ID-99; until WP-S1.11 numbers every
//! scheme).

use std::sync::Arc;

use ew_ids::Id;
use ew_rds::{Aspect, Designation, RdsSyntax};
use ew_schema::{Device, DeviceKind, FnRole, Function, RepType, Representation, TagScheme, TagSign};

use crate::Q;
use crate::cell::RepAnchorCell;
use crate::note::{DeriveNote, SchemeIssue};
use crate::pages::folio_label;
use crate::profile::{self, CounterScope, NumberingScheme, NumberingSettings};
use crate::query::{Policy, Query};
use crate::structure::{PageStructure, ResolvedStructure, StructureOf};

/// `main_function(device)`: the device's function with role `Main` and the smallest ID.
#[derive(Debug)]
pub struct MainFunction;

impl Query for MainFunction {
    type Key = Id<Device>;
    type Value = Option<Id<Function>>;
    const FAMILY: &'static str = "main_function";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<Id<Function>>> {
        let functions = q.functions();
        Arc::new(
            q.idx()
                .functions_of_device(*key)
                .find(|f| functions.get(*f).is_some_and(|f| f.role == FnRole::Main)),
        )
    }
}

/// `main_rep(device)`: the smallest-ID representation that draws the main function and is not a contact
/// image (doc 03 §5.1); `None` for an unplaced device.
#[derive(Debug)]
pub struct MainRep;

impl Query for MainRep {
    type Key = Id<Device>;
    type Value = Option<Id<Representation>>;
    const FAMILY: &'static str = "main_rep";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<Id<Representation>>> {
        let Some(f) = *q.get::<MainFunction>(*key) else {
            return Arc::new(None);
        };
        let reps = q.reps();
        Arc::new(
            q.idx()
                .reps_of_function(f)
                .find(|r| reps.get(*r).is_some_and(|r| r.rep_type != RepType::ContactImage)),
        )
    }
}

/// How the product designation of a tag was obtained.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TagSource {
    /// A frozen tag (doc 03 §8.5); listed by the numbering report.
    Frozen,
    /// A child tag: the parent's product designation, the separator, letter and counter.
    Child,
    /// A terminal: its strip's designation and the terminal designation as connection point.
    Terminal,
    /// A numbering scheme's format.
    Scheme {
        /// The scheme key.
        key: String,
        /// The format that applied (the scheme's or a rule's).
        format: String,
    },
}

/// A derived device tag: the full designation and what is needed for the displayed one.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tag {
    /// The device.
    pub device: Id<Device>,
    /// The device's resolved structure.
    pub structure: ResolvedStructure,
    /// The product designation without its sign: `101F1`, `201U3-Q1`, `S51`, `X1031`.
    pub product: String,
    /// The connection point after `:` (terminals: the terminal designation).
    pub cp: Option<String>,
    /// Whether pages print the product sign (`false` only for a frozen tag with a hidden sign).
    pub sign_on_pages: bool,
    /// Where the product designation came from.
    pub source: TagSource,
    /// The full designation: `=P01+J-E-2416-101F1`, `=P01+J-E-2416-X1031:1`.
    pub full: String,
}

impl Tag {
    /// The displayed tag on a page with structure `page`: every aspect whose node equals the page's node
    /// is omitted (FR-NUM-002). A tag whose sign is hidden prints without "-" when no structure is left
    /// in front of it (`S51`); a structure prefix keeps the sign, so the text stays a designation.
    pub fn displayed(&self, page: &ResolvedStructure) -> String {
        let mut kept = false;
        let mut d = self.structure.designation(|a| {
            let keep = page.node(a) != self.structure.node(a);
            kept |= keep;
            keep
        });
        if !kept && !self.sign_on_pages {
            return match &self.cp {
                Some(cp) => format!("{}:{cp}", self.product),
                None => self.product.clone(),
            };
        }
        push_product(&mut d, &self.product, self.cp.as_deref());
        d.format(&RdsSyntax::default())
    }

    /// The displayed tag as a report prints it when no page applies: the structure omitted entirely.
    pub fn short(&self) -> String {
        self.displayed(&self.structure)
    }
}

fn push_product(d: &mut Designation, product: &str, cp: Option<&str>) {
    if !product.is_empty() {
        let _ = d.push(Aspect::Product, product);
    }
    if cp.is_some() {
        let _ = d.set_connection_point(cp.filter(|c| !c.is_empty()));
    }
}

/// `device_tag(device)`: the derived tag (doc 03 §8.2); `None` for a missing device.
///
/// Product designation, first match wins:
/// 1. a **frozen** tag (wins over every other rule, doc 03 §8.2): its text; the sign is hidden on pages for
///    `TagSign::Hidden`;
/// 2. a **terminal** (`Terminal` aspect): its strip's product designation, the terminal designation as
///    connection point (`X1031` + `:1`), the strip's structure;
/// 3. scheme **`Child`**: the parent's product designation, the scheme's `child_separator`, the letter and
///    the counter (`201U3-Q1`, FR-DEV-021); the parent's structure;
/// 4. otherwise the device's numbering scheme (the project's `settings.numbering`, else the pinned
///    profile's): the format of the first rule whose selector matches, else the scheme's format, with
///    `{letter}`, `{counter}`, `{page}` (folio label of the main representation's page), `{row}` and
///    `{column}` (its anchor cell) and `{subcounter}` (empty: no sub-counter exists yet). A value that
///    cannot be derived prints "?" (an unnumbered or unplaced device); an unknown token prints verbatim.
///
/// Letters print as stored: display never re-maps a letter through the identifier set (ADR-0026
/// decision 4); the set takes part only in conformance checks and renumbering.
#[derive(Debug)]
pub struct DeviceTag;

impl Query for DeviceTag {
    type Key = Id<Device>;
    type Value = Option<Tag>;
    const FAMILY: &'static str = "device_tag";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<Tag>> {
        let Some(dev) = q.devices().get(*key) else {
            return Arc::new(None);
        };
        let structure = (*q.get::<StructureOf>(*key)).clone();
        let (product, cp, sign_on_pages, source) = product_designation(q, *key, dev);
        let mut d = structure.designation(|_| true);
        push_product(&mut d, &product, cp.as_deref());
        let full = d.format(&RdsSyntax::default());
        Arc::new(Some(Tag {
            device: *key,
            structure,
            product,
            cp,
            sign_on_pages,
            source,
            full,
        }))
    }
}

fn counter_text(counter: Option<u32>) -> String {
    counter.map_or_else(|| "?".to_owned(), |c| c.to_string())
}

/// The tag of the device another tag is built from (a terminal's strip, a child's parent), through
/// [`Q::try_get`]; `Err` on a cycle, which is noted.
fn tag_of(q: &Q, device: Id<Device>) -> Result<Option<Tag>, ()> {
    match q.try_get::<DeviceTag>(device) {
        Ok(t) => Ok((*t).clone()),
        Err(cycle) => {
            q.note_cycle(cycle);
            Err(())
        }
    }
}

/// Why the command layer's counters would not make `scheme`'s tags unique, if they would not.
pub(crate) fn scheme_issue(scheme: &NumberingScheme) -> Option<SchemeIssue> {
    if scheme.counter_scope != CounterScope::Named("page".to_owned()) {
        return Some(SchemeIssue::CounterScope);
    }
    let formats =
        std::iter::once(scheme.format.as_str()).chain(scheme.rules.iter().map(|r| r.format.as_str()));
    for f in formats {
        if !f.contains("{page}") {
            return Some(SchemeIssue::FormatWithoutPage);
        }
        if !(f.contains("{letter}") && f.contains("{counter}")) {
            return Some(SchemeIssue::FormatWithoutCounter);
        }
    }
    None
}

/// The scheme that formats a numbered tag: the requested one ([`profile::scheme_for`]: the named scheme,
/// else the project's default, else the profile's), or the pinned profile's default scheme when the command
/// layer's counters would not keep the requested one's tags unique (noted, decision log ID-99). A named key
/// the settings do not define is noted too.
fn applied_scheme<'a>(
    q: &'a Q,
    settings: Option<&'a NumberingSettings>,
    named: Option<&str>,
) -> &'a NumberingScheme {
    let profile = q.profile();
    if let (Some(key), Some(settings)) = (named, settings) {
        if !settings.schemes.iter().any(|s| s.key == key) {
            q.note(DeriveNote::SchemeNotApplied {
                scheme: key.to_owned(),
                issue: SchemeIssue::UnknownKey,
            });
        }
    }
    let requested = profile::scheme_for(settings, profile, named);
    if std::ptr::eq(requested, &profile.numbering) {
        return requested;
    }
    match scheme_issue(requested) {
        None => requested,
        Some(issue) => {
            q.note(DeriveNote::SchemeNotApplied {
                scheme: requested.key.clone(),
                issue,
            });
            &profile.numbering
        }
    }
}

fn product_designation(q: &Q, id: Id<Device>, dev: &Device) -> (String, Option<String>, bool, TagSource) {
    if let Some(frozen) = &dev.tag.frozen {
        let sign = !matches!(frozen.sign, TagSign::Hidden);
        return (frozen.text.as_str().to_owned(), None, sign, TagSource::Frozen);
    }
    if let Some(t) = &dev.aspects.terminal {
        // The strip edge of the inheritance graph (EW-INV-015): a cycle prints "?" instead of panicking.
        let (product, sign) = match tag_of(q, t.strip) {
            Ok(Some(s)) => (s.product, s.sign_on_pages),
            Ok(None) => (String::new(), true),
            Err(()) => ("?".to_owned(), true),
        };
        return (
            product,
            Some(t.designation.as_str().to_owned()),
            sign,
            TagSource::Terminal,
        );
    }
    let settings = q.get::<profile::NumberingSettingsOf>(());
    let letter = dev.tag.letter.as_str();
    if dev.tag.scheme == TagScheme::Child {
        let scheme = profile::scheme_for(settings.as_ref().as_ref(), q.profile(), None);
        // The parent edge of the inheritance graph (EW-INV-015): a cycle prints "?" for the parent.
        let parent = dev.parent.map(|p| match tag_of(q, p) {
            Ok(t) => t.map(|t| t.product),
            Err(()) => Some("?".to_owned()),
        });
        let own = format!("{letter}{}", counter_text(dev.tag.counter));
        let product = match parent.flatten() {
            Some(p) if !p.is_empty() => format!("{p}{}{own}", scheme.child_separator),
            _ => own,
        };
        return (product, None, true, TagSource::Child);
    }
    let named = match &dev.tag.scheme {
        TagScheme::Named(k) => Some(k.as_str()),
        _ => None,
    };
    let scheme = applied_scheme(q, settings.as_ref().as_ref(), named);
    let format = rule_format(scheme, dev);
    let product = render_format(q, id, dev, format);
    (
        product,
        None,
        true,
        TagSource::Scheme {
            key: scheme.key.clone(),
            format: format.to_owned(),
        },
    )
}

/// The format for a device: the first matching rule's, else the scheme's.
fn rule_format<'s>(scheme: &'s NumberingScheme, dev: &Device) -> &'s str {
    scheme
        .rules
        .iter()
        .find(|r| {
            let s = &r.select;
            s.aspect.as_deref().is_none_or(|a| has_aspect(dev, a))
                && s.letter.as_deref().is_none_or(|l| l == dev.tag.letter.as_str())
                && s.kind
                    .as_deref()
                    .is_none_or(|k| dev.kind.as_ref().is_some_and(|dk| kind_name(dk) == Some(k)))
        })
        .map_or(scheme.format.as_str(), |r| r.format.as_str())
}

/// Whether a device has the named aspect. Typed aspects are checked by field; aspects that this format
/// version keeps in `extra` (`assembly`, `plc_station`, …) by member presence.
fn has_aspect(dev: &Device, name: &str) -> bool {
    match name {
        "strip" => dev.aspects.strip.is_some(),
        "terminal" => dev.aspects.terminal.is_some(),
        "cable" => dev.aspects.cable.is_some(),
        other => dev.extra.contains_key(other),
    }
}

fn kind_name(kind: &DeviceKind) -> Option<&str> {
    match kind {
        DeviceKind::Psu => Some("psu"),
        DeviceKind::Terminal => Some("terminal"),
        DeviceKind::BlankingPlate => Some("blanking_plate"),
        DeviceKind::Unknown(v) => v.as_str(),
    }
}

/// Renders a numbering format for a device.
fn render_format(q: &Q, id: Id<Device>, dev: &Device, format: &str) -> String {
    let main_rep = *q.get::<MainRep>(id);
    let rep = main_rep.and_then(|r| q.reps().get(r));
    let page = rep.and_then(|r| q.pages().get(r.page));
    let cell = || main_rep.and_then(|r| (*q.get::<RepAnchorCell>(r)).clone());
    let mut out = String::new();
    let mut rest = format;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let Some(close) = after.find('}') else {
            out.push_str(after);
            rest = "";
            break;
        };
        let token = &after[1..close];
        match token {
            "letter" => out.push_str(dev.tag.letter.as_str()),
            "counter" => out.push_str(&counter_text(dev.tag.counter)),
            "subcounter" => {}
            "page" => match page {
                Some(p) => out.push_str(&folio_label(p)),
                None => out.push('?'),
            },
            "column" => match cell() {
                Some(c) => out.push_str(&c.column_label),
                None => out.push('?'),
            },
            "row" => match cell() {
                Some(c) => out.push_str(&c.row_label),
                None => out.push('?'),
            },
            _ => out.push_str(&after[..=close]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// `displayed_tag(device, page)`: the device's tag as printed on a page (FR-NUM-002); empty for a
/// missing device.
#[derive(Debug)]
pub struct DisplayedTag;

impl Query for DisplayedTag {
    type Key = (Id<Device>, Id<ew_schema::Page>);
    type Value = String;
    const FAMILY: &'static str = "displayed_tag";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<String> {
        let tag = q.get::<DeviceTag>(key.0);
        let Some(tag) = tag.as_ref() else {
            return Arc::new(String::new());
        };
        let page = q.get::<PageStructure>(key.1);
        Arc::new(tag.displayed(&page))
    }
}
