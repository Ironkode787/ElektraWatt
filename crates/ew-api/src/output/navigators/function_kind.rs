// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A function's FnDef name and its kind in words ("NO contact", "changeover"), which the navigators, the
//! properties, the catalogue and the part card print.
//!
//! Not here: the device rows that show them (`devices.rs`); a device's short tag (`navigators.rs`).

use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{FnDef, Function, TemplateFn, template_entry_of};

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

/// The human text of a function's kind: the FnDef's last category entry, then the one before it
/// lower-cased, then the CP designations of the function's template entry (else of the FnDef, in `idx` order) joined
/// by `-`. Category `["Relay", "Contact", "NO"]` with CPs 13 and 14 gives `NO contact 13-14`; a last entry equal to
/// the CP designations run together is dropped and the one before it capitalised (`["Relay", "Coil", "A1A2"]` with
/// CPs A1 and A2 gives `Coil A1-A2`); a last entry `CO` reads `Changeover` (FT-752: `Changeover contact 11-12-14`);
/// a category of fewer
/// than two non-empty entries, or a missing FnDef copy, gives [`fndef_name`]. The Properties grid's "Part function"
/// row and the picker's meta line read it.
pub(crate) fn function_kind_text(q: &Q, f: &Function) -> String {
    function_kind_or_key(q, f).unwrap_or_else(|| fndef_name(q, f))
}

/// [`function_kind_text`] without its last fallback, the copy's UUID text: the category's text, else the FnDef's key,
/// else `None` (the live description's `<kind>`).
pub(crate) fn function_kind_or_key(q: &Q, f: &Function) -> Option<String> {
    let fndefs = q.fndefs();
    let def = fndefs
        .get(f.def)
        .map(|c| &c.item)
        .or_else(|| crate::output::editing::starter_fndef(f.def).map(|c| &c.item));
    let Some(def) = def else {
        return fndef_key(q, f);
    };
    let cps = template_cp_designations(q, f, def).unwrap_or_else(|| {
        let mut d: Vec<(u16, &str)> = def.cps.iter().map(|c| (c.idx, c.designation.as_str())).collect();
        d.sort_unstable();
        d.into_iter().map(|(_, s)| s.to_owned()).collect()
    });
    kind_text(def, &cps).or_else(|| fndef_key(q, f))
}

/// The kind text of a FnDef with the given CP designations (see [`function_kind_text`]): `None` when its category has
/// fewer than two non-empty entries. The place catalogue reads it for a template entry not yet placed.
pub(crate) fn kind_text(def: &FnDef, cps: &[String]) -> Option<String> {
    let (a, b, c) = &def.category;
    let cat: Vec<&str> = [a, b, c]
        .iter()
        .map(|i| i.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    let [.., before, last] = cat.as_slice() else {
        return None;
    };
    // FT-723: a kind outside the contacts and coils reads in plain words from the display mapping, the CP designations
    // following as the rule below adds them where the mapping keeps them.
    if let Some((words, with_cps)) = kind_words(before, last) {
        let mut out = words.to_owned();
        if with_cps && !cps.is_empty() {
            out.push(' ');
            out.push_str(&cps.join("-"));
        }
        return Some(out);
    }
    // A last entry that only repeats the CP designations run together (a coil's
    // `A1A2`) is dropped, and the entry before it leads, capitalised: `Coil A1-A2`, not `A1A2 coil A1-A2`.
    let joined: String = cps.concat();
    let mut out = if !joined.is_empty() && last.eq_ignore_ascii_case(&joined) {
        let mut chars = before.chars();
        chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    } else {
        // The change-over kind is spelled out; FT-752: in sentence case (`Changeover contact 11-12-14` beside
        // `NO contact 13-14`). The FnDef keeps its category.
        let last = LAST_ENTRY_WORDS
            .iter()
            .find(|(entry, _)| last.eq_ignore_ascii_case(entry))
            .map_or(*last, |(_, word)| *word);
        let mut chars = last.chars();
        let last: String = chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect())
            .unwrap_or_default();
        format!("{last} {}", before.to_lowercase())
    };
    if !cps.is_empty() {
        out.push(' ');
        out.push_str(&cps.join("-"));
    }
    Some(out)
}

/// The display words of FnDef categories by their last two entries (FT-723), where the generic rule of [`kind_text`]
/// reads poorly ("Generic decoration", "Point ip 1"): the entry before the last, the last entry, the words in sentence
/// case, and whether the CP designations follow. Display text only: the FnDef keeps its category. English only for
/// now: this table is what the shell's Fluent tables localise (FT-752).
const KIND_WORDS: &[(&str, &str, &str, bool)] = &[
    ("Decoration", "Generic", "Decoration", false),
    ("Overview", "Generic", "Overview", false),
    ("BlackBox", "Generic", "Black box", false),
    ("Ip", "Point", "Interruption point", true),
    ("Arrow", "Point", "External reference", true),
    ("ProtectiveEarth", "Point", "Earth connection", true),
    ("FunctionalEarth", "Point", "Functional earth connection", true),
    ("BusConnector", "Profibus", "PROFIBUS connector", true),
    ("BusPort", "Profibus", "PROFIBUS port", true),
    ("PlugPin", "Single", "Plug pin", true),
    ("Strip", "Segment", "Terminal strip segment", false),
    ("FeedThrough", "1Level", "Feed-through terminal", true),
    ("PE", "1Level", "PE terminal", true),
    ("MCB", "1P", "1P MCB", true),
    ("MCB", "1P+N", "1P+N MCB", true),
    ("ManualSwitch", "1P", "1P manual switch", true),
    ("ManualSwitch", "2P", "2P manual switch", true),
    ("PositionSwitch", "NO", "NO position switch", true),
    ("SelectorSwitch", "NO", "NO selector switch", true),
    ("EnclosureHeater", "1P", "Enclosure heater", true),
    ("DoubleActing", "Cylinder", "Double-acting cylinder", true),
    ("ProcessValve", "TwoWay", "Two-way process valve", true),
    ("Valve52", "SolenoidSpring", "5/2 valve", true),
];

/// The word of a category's last entry where the generic rule of [`kind_text`] spells it out (`CO` reads
/// "changeover"); in the same localisation as [`KIND_WORDS`].
const LAST_ENTRY_WORDS: &[(&str, &str)] = &[("CO", "changeover")];

/// The display words of a FnDef category by its last two entries ([`KIND_WORDS`]), with whether the CP designations
/// follow; `None` for every other category, which keeps the generic rule.
fn kind_words(before: &str, last: &str) -> Option<(&'static str, bool)> {
    KIND_WORDS
        .iter()
        .find(|(b, l, _, _)| *b == before && *l == last)
        .map(|(_, _, words, with_cps)| (*words, *with_cps))
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
    kind_words(before, last).map(|(words, _)| words)
}

/// The Properties grid's "Part function" text of a function: [`function_kind_text`] when the function
/// records `template_entry` and that entry resolves among its device's part uses (`ew_schema::template_entry_of`);
/// `None` otherwise (the row is hidden).
pub(crate) fn part_function_text(q: &Q, f: &Function) -> Option<String> {
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
    let copy: Id<FnDef> =
        ew_derive::ew_ids::v5::lib_copy_id(t.fndef.library.as_uuid(), t.fndef.item.as_uuid());
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
