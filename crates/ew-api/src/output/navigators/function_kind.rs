// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A function's FnDef name and its kind in words ("NO contact", "changeover"), which the navigators, the
//! properties, the catalogue and the part card print: a [`Text`] in every language of the session's texts.
//!
//! Not here: the device rows that show them (`devices.rs`); a device's short tag (`navigators.rs`).

use ew_derive::Q;
use ew_ids::Id;
use ew_mltext::MlText;
use ew_schema::{FnDef, Function, TemplateFn, template_entry_of};

use crate::{Text, TextArg};

/// The name of a function's FnDef: its key, else the copy's UUID text.
pub(crate) fn fndef_name(q: &Q, f: &Function) -> String {
    fndef_key(q, f).unwrap_or_else(|| f.def.as_uuid().to_string())
}

/// The key of a function's FnDef (its copy's, else the starter FnDef's); `None` for a keyless or missing FnDef.
fn fndef_key(q: &Q, f: &Function) -> Option<String> {
    q.fndefs()
        .get(f.def)
        .and_then(|c| c.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
        .or_else(|| {
            crate::output::editing::starter_fndef(f.def)
                .and_then(|c| c.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
        })
}

/// The words of a function's kind: the FnDef's last category entry, then the one before it
/// lower-cased, then the CP designations of the function's template entry (else of the FnDef, in `idx` order) joined
/// by `-`. Category `["Relay", "Contact", "NO"]` with CPs 13 and 14 gives `NO contact 13-14`; a last entry equal to
/// the CP designations run together is dropped and the one before it capitalised (`["Relay", "Coil", "A1A2"]` with
/// CPs A1 and A2 gives `Coil A1-A2`); a last entry `CO` reads `Changeover` (`Changeover contact 11-12-14`);
/// a category of fewer
/// than two non-empty entries, or a missing FnDef copy, gives [`fndef_name`]. The Properties grid's "Part function"
/// row and the picker's meta line read it.
pub(crate) fn function_kind_text(q: &Q, f: &Function) -> Text {
    function_kind_or_key(q, f).unwrap_or_else(|| super::plain(fndef_name(q, f)))
}

/// [`function_kind_text`] without its last fallback, the copy's UUID text: the category's words, else the FnDef's key,
/// else `None` (the live description's `<kind>`).
pub(crate) fn function_kind_or_key(q: &Q, f: &Function) -> Option<Text> {
    let fndefs = q.fndefs();
    let def = fndefs
        .get(f.def)
        .map(|c| &c.item)
        .or_else(|| crate::output::editing::starter_fndef(f.def).map(|c| &c.item));
    let Some(def) = def else {
        return fndef_key(q, f).map(super::plain);
    };
    let cps = template_cp_designations(q, f, def).unwrap_or_else(|| {
        let mut d: Vec<(u16, &str)> = def.cps.iter().map(|c| (c.idx, c.designation.as_str())).collect();
        d.sort_unstable();
        d.into_iter().map(|(_, s)| s.to_owned()).collect()
    });
    kind_text(def, &cps).or_else(|| fndef_key(q, f).map(super::plain))
}

/// The kind words of a FnDef with the given CP designations (see [`function_kind_text`]) in every language of the
/// session's texts: `None` when its category has fewer than two non-empty entries. The place catalogue reads it for a
/// template entry not yet placed.
///
/// A kind of [`KIND_WORDS`] reads in its words. Else, by the generic rule, a last entry that only repeats the CP
/// designations run together (a coil's `A1A2`) is dropped and the entry before it leads (`Coil A1-A2`, not `A1A2
/// coil A1-A2`); any other last entry leads (spelt out by [`LAST_ENTRY_WORDS`]: `Changeover contact 11-12-14` beside
/// `NO contact 13-14`) and the one before it follows, lower-cased. Each entry reads in its segment's words
/// (`kind-segment-<entry>`), two of them joined by `kind-generic` (a Dutch segment carries its own joint:
/// "NO-contact", "Wisselcontact"); an entry without words reads as written, and so does a leading entry whose words
/// end in their joint where no second words follow ("3P manualswitch", not "3P- manualswitch"). The kind leads with
/// a capital. The FnDef keeps its category.
pub(crate) fn kind_text(def: &FnDef, cps: &[String]) -> Option<Text> {
    let (a, b, c) = &def.category;
    let cat: Vec<&str> = [a, b, c]
        .iter()
        .map(|i| i.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    let [.., before, last] = cat.as_slice() else {
        return None;
    };
    let cps = cps.join("-");
    let catalogue = crate::catalogue();
    let words = |tag: &str| -> String {
        let say = |t: &Text| catalogue.text_or_english(t, tag);
        if let Some((key, with_cps)) = kind_words(before, last) {
            return joined(&[&say(&super::key_text(key)), if with_cps { &cps } else { "" }]);
        }
        let (lead, follow) = if !cps.is_empty() && last.eq_ignore_ascii_case(&cps.replace('-', "")) {
            (capitalised(before), None)
        } else {
            let last = LAST_ENTRY_WORDS
                .iter()
                .find(|(entry, _)| last.eq_ignore_ascii_case(entry))
                .map_or(*last, |(_, key)| crate::text_english(key).unwrap_or(key));
            (capitalised(last), Some(before.to_lowercase()))
        };
        let segment = |entry: &str| segment_text(entry).map(|t| say(&t));
        let (head, rest) = match (segment(&lead), follow.as_deref().and_then(segment)) {
            (Some(a), Some(b)) => (say(&text!("kind-generic", a, b)).trim_end().to_owned(), None),
            (Some(a), None) if a.ends_with('-') => (lead.clone(), follow),
            (Some(a), None) => (a.trim_end().to_owned(), follow),
            (None, Some(b)) => (format!("{lead} {b}"), None),
            (None, None) => (lead.clone(), follow),
        };
        joined(&[&super::with_capital(&head), rest.as_deref().unwrap_or(""), &cps])
    };
    Some(Text::bare(TextArg::Ml(MlText::from_langs(
        (catalogue.tags().into_iter()).filter_map(|tag| Some((tag.parse().ok()?, words(tag)))),
    ))))
}

/// The words of a category entry as a segment of the generic rule (`kind-segment-<entry>`, lower-cased): the entry
/// names the key; `None` for an entry without words.
fn segment_text(entry: &str) -> Option<Text> {
    let key = format!("kind-segment-{}", entry.to_lowercase());
    let i = crate::TEXTS
        .binary_search_by(|(k, _)| (*k).cmp(key.as_str()))
        .ok()?;
    let (key, english) = crate::TEXTS[i];
    Some(Text::checked_by_macro(
        key,
        english,
        Some(crate::catalogue),
        Vec::new(),
    ))
}

/// The parts that are not empty, joined by spaces.
fn joined(parts: &[&str]) -> String {
    let parts: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    parts.join(" ")
}

/// A word with its first letter upper-cased.
fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The display words of FnDef categories by their last two entries, where the generic rule of [`kind_text`]
/// reads poorly ("Generic decoration", "Point ip 1"): the entry before the last, the last entry, the words in sentence
/// case as the key of the session's text, and whether the CP designations follow. Display text only: the FnDef keeps its
/// category.
const KIND_WORDS: &[(&str, &str, &str, bool)] = &[
    ("Decoration", "Generic", key!("kind-words-decoration"), false),
    ("Overview", "Generic", key!("kind-words-overview"), false),
    ("BlackBox", "Generic", key!("kind-words-black-box"), false),
    ("Ip", "Point", key!("kind-words-interruption-point"), true),
    ("Arrow", "Point", key!("kind-words-external-reference"), true),
    (
        "ProtectiveEarth",
        "Point",
        key!("kind-words-earth-connection"),
        true,
    ),
    (
        "FunctionalEarth",
        "Point",
        key!("kind-words-functional-earth-connection"),
        true,
    ),
    (
        "BusConnector",
        "Profibus",
        key!("kind-words-profibus-connector"),
        true,
    ),
    ("BusPort", "Profibus", key!("kind-words-profibus-port"), true),
    ("PlugPin", "Single", key!("kind-words-plug-pin"), true),
    (
        "Strip",
        "Segment",
        key!("kind-words-terminal-strip-segment"),
        false,
    ),
    (
        "FeedThrough",
        "1Level",
        key!("kind-words-feed-through-terminal"),
        true,
    ),
    ("PE", "1Level", key!("kind-words-pe-terminal"), true),
    ("MCB", "1P", key!("kind-words-1p-mcb"), true),
    ("MCB", "1P+N", key!("kind-words-1p-n-mcb"), true),
    ("ManualSwitch", "1P", key!("kind-words-1p-manual-switch"), true),
    ("ManualSwitch", "2P", key!("kind-words-2p-manual-switch"), true),
    (
        "PositionSwitch",
        "NO",
        key!("kind-words-no-position-switch"),
        true,
    ),
    (
        "SelectorSwitch",
        "NO",
        key!("kind-words-no-selector-switch"),
        true,
    ),
    ("EnclosureHeater", "1P", key!("kind-words-enclosure-heater"), true),
    (
        "DoubleActing",
        "Cylinder",
        key!("kind-words-double-acting-cylinder"),
        true,
    ),
    (
        "ProcessValve",
        "TwoWay",
        key!("kind-words-two-way-process-valve"),
        true,
    ),
    ("Valve52", "SolenoidSpring", key!("kind-words-5-2-valve"), true),
];

/// The word of a category's last entry where the generic rule of [`kind_text`] spells it out (`CO` reads
/// "changeover"); in the same localisation as [`KIND_WORDS`].
const LAST_ENTRY_WORDS: &[(&str, &str)] = &[("CO", key!("kind-segment-changeover"))];

/// The key of the display words of a FnDef category by its last two entries ([`KIND_WORDS`]), with whether the CP
/// designations follow; `None` for every other category, which keeps the generic rule.
fn kind_words(before: &str, last: &str) -> Option<(&'static str, bool)> {
    KIND_WORDS
        .iter()
        .find(|(b, l, _, _)| *b == before && *l == last)
        .map(|(_, _, key, with_cps)| (*key, *with_cps))
}

/// A function's kind in the navigators' own words, without its CP designations ("Interruption point"): the
/// [`KIND_WORDS`] entry of its FnDef's category; `None` for a category the table does not name or a missing
/// FnDef copy.
pub(crate) fn function_kind_words(q: &Q, f: &Function) -> Option<&'static str> {
    let fndefs = q.fndefs();
    let def = fndefs
        .get(f.def)
        .map(|c| &c.item)
        .or_else(|| crate::output::editing::starter_fndef(f.def).map(|c| &c.item))?;
    let (a, b, c) = &def.category;
    let cat: Vec<&str> = [a, b, c]
        .iter()
        .map(|i| i.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    let [.., before, last] = cat.as_slice() else {
        return None;
    };
    kind_words(before, last).map(|(key, _)| crate::text_english(key).unwrap_or(key))
}

/// The Properties grid's "Part function" text of a function: [`function_kind_text`] when the function
/// records `template_entry` and that entry resolves among its device's part uses (`ew_schema::template_entry_of`);
/// `None` otherwise (the row is hidden).
pub(crate) fn part_function_text(q: &Q, f: &Function) -> Option<Text> {
    f.template_entry?;
    template_entry_resolves(q, f).then(|| function_kind_text(q, f))
}

/// Whether a function's device has a part use whose template has an entry naming the function's FnDef copy.
fn template_entry_resolves(q: &Q, f: &Function) -> bool {
    let Some(device) = f.device.and_then(|d| q.devices().get(d).cloned()) else {
        return false;
    };
    let parts = q.parts();
    let uses = device
        .parts
        .iter()
        .filter_map(|u| parts.get(u.part.copy).map(|p| ((), &p.item)));
    template_entry_of(uses, f.template_entry, |t| names_def(t, f)).is_some()
}

/// Whether a template entry names the function's FnDef copy (`v5(NS_LIBCOPY, library ‖ item)`).
fn names_def(t: &TemplateFn, f: &Function) -> bool {
    let copy: Id<FnDef> = ew_ids::v5::lib_copy_id(t.fndef.library.as_uuid(), t.fndef.item.as_uuid());
    copy == f.def
}

/// The CP designations of a template-origin function's entry, in function CP order (`ew_schema::template_entry_of`
/// and `TemplateFn::function_cps`); `None` when the function follows no template entry or the entry selects none.
fn template_cp_designations(q: &Q, f: &Function, def: &FnDef) -> Option<Vec<String>> {
    let device = q.devices().get(f.device?)?;
    let parts = q.parts();
    let uses = device
        .parts
        .iter()
        .filter_map(|u| parts.get(u.part.copy).map(|p| ((), &p.item)));
    let ((), part, n) = template_entry_of(uses, f.template_entry, |t| names_def(t, f))?;
    let pattern: Vec<u16> = part.cp_pattern.iter().map(|c| c.idx).collect();
    let def_idx: Vec<u16> = def.cps.iter().map(|c| c.idx).collect();
    let mut pairs = part.function_template[n].function_cps(&pattern, &def_idx);
    pairs.sort_by_key(|t| t.idx);
    let out: Vec<String> = pairs
        .iter()
        .filter_map(|t| part.cp_pattern.iter().find(|c| c.idx == t.pattern))
        .map(|c| c.designation.as_str().to_owned())
        .collect();
    (!out.is_empty()).then_some(out)
}
