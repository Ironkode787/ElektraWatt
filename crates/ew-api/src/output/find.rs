// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Find and replace (WP-S2.05 part 2, spec §7.1 and §7.4; D11, D16): [`Session::find`] searches the strings of the
//! project's records and [`FindResults::replace_command`] turns the replaceable hits into the parameters of one
//! `fields.set_many`.
//!
//! **What is searched** ([`FindField`]): a device's function text (the neutral text and every language), its tag as
//! displayed on its main page, its technical data as `key=value`, its process tag, the part number and manufacturer
//! of each part it uses (`<part number> (<manufacturer>)`), and a drawn page's title and description override. A
//! searched string is taken in NFC; [`FindHit::before`] is that string and the hit's ranges are byte ranges in it.
//!
//! **Replaceable** are the authored values only: a function text, a frozen tag's text, an identifier technical value,
//! a process tag, a page title or description. A derived tag, a quantity and a library copy's part number are found
//! but not replaced, each with its reason (D11, D16). A replacement may change only the part of the string that is
//! stored: a tag's structure prefix and a technical value's `key=` are shown, not written, so a match that reaches
//! into them makes the hit not replaceable.
//!
//! **Order** (§7.1, §9 item 3): page order (the main representation's page for a device, the page itself for a page
//! field; records without a page last), then the anchor cell (row, column), then the subject in natural order, then
//! the row path (the language is its last segment), then the record ID. At most 10,000 hits; beyond, `truncated`.
//!
//! The search runs outside any transaction over the memoised queries (`device_tag`, `main_rep`, `rep_anchor_cell`,
//! `page_order`); `fields.set_many` receives explicit values and searches nothing (the transaction-read rule,
//! ADR-0008 amendment 4), so a command log replays a replace without a regular-expression engine.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{Cell, DeviceTag, MainRep, PageOrder, PageRef, Q, RepAnchorCell, TagSource};
use ew_schema::{Device, Function, Kind, Page, Representation};
use serde_json::{Value, json};

use super::fields::{FieldType, format_field_value};
use crate::Session;

/// The most hits a search returns; beyond, the results are `truncated` and replace is refused.
const MAX_HITS: usize = 10_000;

/// The longest regular expression a search compiles, in characters.
const MAX_PATTERN: usize = 1_000;

/// What a search looks for (§7.1).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FindQuery {
    /// The text, or the regular expression when `regex` is set. Empty: no hits.
    pub text: String,
    /// `text` is a regular expression (the `regex` crate's syntax).
    pub regex: bool,
    /// Compare case-sensitively; otherwise by Unicode simple case folding.
    pub match_case: bool,
    /// A match counts only when the characters around it are not alphanumeric or `_`.
    pub whole_word: bool,
    /// The fields searched; empty: all five.
    pub fields: Vec<FindField>,
    /// Only records on pages with these printed folio labels; empty: every page and the records without one.
    pub folios: Vec<String>,
    /// Only these records and the records they reach (a representation reaches its functions and their devices);
    /// empty: every record.
    pub within: Vec<Uuid>,
    /// The replacement: literal for plain text, with `$1`-style expansion for a regular expression.
    pub replace: Option<String>,
}

/// A searched field (FR-UI-020's scopes, FR-DEV-032's list).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum FindField {
    /// A device's function text, every language.
    FunctionText,
    /// A device's tag as displayed on its main page.
    Tag,
    /// A device's technical data as `key=value`.
    Property,
    /// A page's title and description override, a device's process tag.
    Text,
    /// The part number and manufacturer of a device's part copies.
    PartNumber,
}

/// One searched string with at least one match (§7.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FindHit {
    /// The record.
    pub record: Uuid,
    /// Its kind (`device`, `page`).
    pub kind: String,
    /// The row path of the string in the record (`function_text`, `function_text.de`, `tech_data[2].value`,
    /// `tag.frozen.text`, `derived.tag`, `parts[0].part`, `title`).
    pub row_path: String,
    /// The key of the field's label (`find-field-function-text`).
    pub label_key: String,
    /// What the record is: the device's tag, or the page's folio and title.
    pub subject: String,
    /// The folio of the record's page, for the jump.
    pub folio: Option<String>,
    /// The record's cell on that page (`204-B3`), for the jump.
    pub cell: Option<String>,
    /// The searched string (NFC).
    pub before: String,
    /// The byte ranges of the matches in `before`, ascending.
    pub ranges: Vec<(usize, usize)>,
    /// The string after the replacement, when the query has one and the hit is replaceable.
    pub after: Option<String>,
    /// Whether a replace writes this hit, else why not.
    pub replaceable: Result<(), String>,
}

/// The result of [`Session::find`].
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FindResults {
    /// The hits in order (§7.1), at most 10,000.
    pub hits: Vec<FindHit>,
    /// More than 10,000 strings matched: the hits are the first 10,000 and replace is refused.
    pub truncated: bool,
    /// The query could not run (a pattern that does not compile or is too long, an unknown folio): no hits.
    pub error: Option<String>,
    /// The `fields.set_many` parameters of the replace, computed with the hits.
    command: Option<Value>,
}

impl FindHit {
    /// Whether a replace changes this hit's string: it has an `after` that differs from `before`.
    pub fn changes(&self) -> bool {
        self.after.as_deref().is_some_and(|after| after != self.before)
    }
}

impl FindResults {
    /// How many hits a replace changes ([`FindHit::changes`]); the replace card's count.
    pub fn changes(&self) -> usize {
        self.hits.iter().filter(|h| h.changes()).count()
    }

    /// The parameters of the one `fields.set_many` that writes every replaceable hit's `after` (§7.1): one entry per
    /// (kind, path, value) holding every record that gets that value, entries ordered by (kind rank, path, first
    /// target ID); hits on one record and member are merged into one value first. `None` when no hit has `after`,
    /// when the results are truncated or in error, or when the replace would change nothing.
    pub fn replace_command(&self) -> Option<Value> {
        self.command.clone()
    }
}

/// How a replaceable string is written back into its record's settable member.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Target {
    /// A multi-language member (`function_text`, `title`, `description_override`): the neutral text or one language.
    Ml {
        member: &'static str,
        lang: Option<String>,
    },
    /// A plain string member (`process_tag`).
    Plain { member: &'static str },
    /// The text of a frozen tag (`tag.frozen`).
    Frozen,
    /// The identifier value of the technical datum at this index (`tech_data`).
    TechValue(usize),
}

impl Target {
    fn member(&self) -> &'static str {
        match self {
            Target::Ml { member, .. } | Target::Plain { member } => member,
            Target::Frozen => "tag.frozen",
            Target::TechValue(_) => "tech_data",
        }
    }
}

/// One searched string before matching.
struct Candidate {
    record: Uuid,
    kind: Kind,
    row_path: String,
    label_key: &'static str,
    subject: String,
    page: Option<Id<Page>>,
    cell: Option<Cell>,
    text: String,
    /// The byte range of `text` a replace writes (a tag's product designation, a technical value).
    span: (usize, usize),
    /// Why a replace never writes this string.
    fixed: Option<&'static str>,
    target: Option<Target>,
}

/// The matcher of a query.
#[derive(Debug)]
enum Matcher {
    /// Plain text: the needle, and its case-folded characters when the search ignores case.
    Plain {
        needle: String,
        folded: Option<Vec<char>>,
    },
    /// A compiled regular expression.
    Regex(regex::Regex),
}

/// The characters of `text` case-folded (`char::to_lowercase`), each with the byte range of the character it came
/// from and whether it is the first and the last folded character of that character.
fn fold(text: &str) -> Vec<(char, usize, usize, bool, bool)> {
    let mut out = Vec::new();
    for (at, c) in text.char_indices() {
        let end = at + c.len_utf8();
        let lower: Vec<char> = c.to_lowercase().collect();
        let n = lower.len();
        for (k, l) in lower.into_iter().enumerate() {
            out.push((l, at, end, k == 0, k + 1 == n));
        }
    }
    out
}

/// Whether `c` is a word character for the whole-word rule.
fn wordy(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether the match `start..end` of `text` stands alone (no word character right before or after it).
fn whole(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back().is_some_and(wordy);
    let after = text[end..].chars().next().is_some_and(wordy);
    !before && !after
}

impl Matcher {
    /// The matcher of `q`, or the error text of a pattern that is too long or does not compile.
    fn new(q: &FindQuery) -> Result<Matcher, String> {
        if q.regex {
            if q.text.chars().count() > MAX_PATTERN {
                return Err(format!(
                    "the regular expression is longer than {MAX_PATTERN} characters"
                ));
            }
            regex::RegexBuilder::new(&q.text)
                .case_insensitive(!q.match_case)
                .size_limit(1 << 20)
                .dfa_size_limit(1 << 20)
                .nest_limit(64)
                .build()
                .map(Matcher::Regex)
                .map_err(|e| format!("the regular expression does not compile: {e}"))
        } else {
            let needle = ew_mltext::to_nfc(&q.text).into_owned();
            let folded = (!q.match_case).then(|| needle.chars().flat_map(char::to_lowercase).collect());
            Ok(Matcher::Plain { needle, folded })
        }
    }

    /// The matches of `text` as byte ranges, each with its replacement text when `replace` is given.
    fn run(&self, text: &str, whole_word: bool, replace: Option<&str>) -> Vec<((usize, usize), String)> {
        let keep = |s: usize, e: usize| !whole_word || whole(text, s, e);
        let mut out = Vec::new();
        match self {
            Matcher::Plain { needle, folded: None } => {
                if needle.is_empty() {
                    return out;
                }
                for (s, m) in text.match_indices(needle.as_str()) {
                    let e = s + m.len();
                    if keep(s, e) {
                        out.push(((s, e), replace.unwrap_or_default().to_owned()));
                    }
                }
            }
            Matcher::Plain {
                folded: Some(needle), ..
            } => {
                if needle.is_empty() {
                    return out;
                }
                let hay = fold(text);
                let n = needle.len();
                let mut i = 0;
                while i + n <= hay.len() {
                    let fits = hay[i].3
                        && hay[i + n - 1].4
                        && hay[i..i + n].iter().zip(needle).all(|(h, c)| h.0 == *c);
                    if fits {
                        let (s, e) = (hay[i].1, hay[i + n - 1].2);
                        if keep(s, e) {
                            out.push(((s, e), replace.unwrap_or_default().to_owned()));
                        }
                        i += n;
                    } else {
                        i += 1;
                    }
                }
            }
            Matcher::Regex(re) => {
                for caps in re.captures_iter(text) {
                    let Some(m) = caps.get(0) else { continue };
                    if !keep(m.start(), m.end()) {
                        continue;
                    }
                    let mut with = String::new();
                    if let Some(r) = replace {
                        caps.expand(r, &mut with);
                    }
                    out.push(((m.start(), m.end()), with));
                }
            }
        }
        out
    }
}

/// `text` with each matched range replaced by its replacement, in NFC (the replacement text is as typed).
fn replaced(text: &str, matches: &[((usize, usize), String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for ((s, e), with) in matches {
        out.push_str(&text[at..*s]);
        out.push_str(with);
        at = *e;
    }
    out.push_str(&text[at..]);
    ew_mltext::to_nfc(&out).into_owned()
}

/// The NFC strings of a multi-language value with their language (`None` for the neutral text), languages in byte
/// order.
fn ml_strings(value: Option<&Value>) -> Vec<(Option<String>, String)> {
    match value {
        Some(Value::String(s)) => vec![(None, s.clone())],
        Some(Value::Object(m)) => {
            let mut v: Vec<(Option<String>, String)> = m
                .iter()
                .filter_map(|(l, t)| t.as_str().map(|t| (Some(l.clone()), t.to_owned())))
                .collect();
            v.sort();
            v
        }
        _ => Vec::new(),
    }
}

/// A row path below a multi-language member: the member, or `<member>.<lang>`.
fn ml_path(member: &str, lang: Option<&str>) -> String {
    match lang {
        Some(l) => format!("{member}.{l}"),
        None => member.to_owned(),
    }
}

/// The records `within` names and the records they reach (§6.2): a representation its functions and their devices, a
/// function its device.
fn reach(q: &Q, within: &[Uuid]) -> BTreeSet<Uuid> {
    let mut out = BTreeSet::new();
    for id in within {
        out.insert(*id);
        let functions: Vec<Id<Function>> = match q.reps().get(Id::<Representation>::from_uuid(*id)) {
            Some(r) => r.functions.clone(),
            None if q.functions().contains(Id::from_uuid(*id)) => vec![Id::from_uuid(*id)],
            None => Vec::new(),
        };
        for f in functions {
            out.insert(f.as_uuid());
            if let Some(d) = q.functions().get(f).and_then(|f| f.device) {
                out.insert(d.as_uuid());
            }
        }
    }
    out
}

impl Session {
    /// Searches the project (§7.1): the hits of `query` in order, at most 10,000, with the replacement of each
    /// replaceable hit when the query has one, and the `fields.set_many` parameters of that replace
    /// ([`FindResults::replace_command`]). Deterministic; runs outside any transaction and adds no memoised family.
    pub fn find(&self, query: &FindQuery) -> FindResults {
        self.find_capped(query, MAX_HITS)
    }

    /// [`Session::find`] with at most `cap` hits (the unit test of the truncation rule uses a small cap).
    fn find_capped(&self, query: &FindQuery, cap: usize) -> FindResults {
        let mut results = FindResults::default();
        if query.text.is_empty() {
            return results;
        }
        let matcher = match Matcher::new(query) {
            Ok(m) => m,
            Err(e) => {
                results.error = Some(e);
                return results;
            }
        };
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let drawn: Vec<(Id<Page>, String)> = order
            .entries
            .iter()
            .filter_map(|e| match e.page {
                PageRef::Drawn(p) => Some((p, e.folio.clone())),
                _ => None,
            })
            .collect();
        let position: BTreeMap<Id<Page>, usize> =
            drawn.iter().enumerate().map(|(i, (p, _))| (*p, i)).collect();
        let folio_of = |p: Id<Page>| drawn.iter().find(|(d, _)| *d == p).map(|(_, f)| f.clone());
        let mut pages_in: Option<BTreeSet<Id<Page>>> = None;
        if !query.folios.is_empty() {
            let mut set = BTreeSet::new();
            for label in &query.folios {
                match drawn.iter().find(|(_, f)| f == label) {
                    Some((p, _)) => {
                        set.insert(*p);
                    }
                    None => {
                        results.error = Some(format!("no page {label}"));
                        return results;
                    }
                }
            }
            pages_in = Some(set);
        }
        let within = (!query.within.is_empty()).then(|| reach(&q, &query.within));
        let all = query.fields.is_empty();
        let wants = |f: FindField| all || query.fields.contains(&f);
        let source = q.project().languages.as_ref().map(|l| l.source.to_string());
        let page_ok = |p: Option<Id<Page>>| match (&pages_in, p) {
            (None, _) => true,
            (Some(set), Some(p)) => set.contains(&p),
            (Some(_), None) => false,
        };
        let in_scope = |id: Uuid| within.as_ref().is_none_or(|w| w.contains(&id));

        let mut candidates: Vec<Candidate> = Vec::new();
        // Devices.
        for (d, device) in q.devices().iter() {
            if !in_scope(d.as_uuid()) {
                continue;
            }
            let main = *q.get::<MainRep>(d);
            let page = main.and_then(|r| q.reps().get(r).map(|rep| rep.page));
            if !page_ok(page) {
                continue;
            }
            let cell = main.and_then(|r| (*q.get::<RepAnchorCell>(r)).clone());
            let subject = super::properties::device_tag_text(&q, d);
            let Ok(json) = serde_json::to_value(&**device) else {
                continue;
            };
            let base = |row_path: String, label_key: &'static str, text: String| Candidate {
                record: d.as_uuid(),
                kind: Kind::Device,
                row_path,
                label_key,
                subject: subject.clone(),
                page,
                cell: cell.clone(),
                span: (0, text.len()),
                text,
                fixed: None,
                target: None,
            };
            if wants(FindField::FunctionText) {
                for (lang, text) in ml_strings(json.get("function_text")) {
                    let text = ew_mltext::to_nfc(&text).into_owned();
                    candidates.push(Candidate {
                        target: Some(Target::Ml {
                            member: "function_text",
                            lang: lang.clone(),
                        }),
                        ..base(
                            ml_path("function_text", lang.as_deref()),
                            "find-field-function-text",
                            text,
                        )
                    });
                }
            }
            if wants(FindField::Tag) && !subject.is_empty() {
                let tag = q.get::<DeviceTag>(d);
                let text = ew_mltext::to_nfc(&subject).into_owned();
                let mut c = base("derived.tag".to_owned(), "find-field-tag", text);
                match tag.as_ref().as_ref() {
                    Some(t) if matches!(t.source, TagSource::Frozen) => {
                        c.row_path = "tag.frozen.text".to_owned();
                        let tail = match &t.cp {
                            Some(cp) => format!("{}:{cp}", t.product),
                            None => t.product.clone(),
                        };
                        match c.text.strip_suffix(tail.as_str()) {
                            Some(prefix) => {
                                c.span = (prefix.len(), prefix.len() + t.product.len());
                                c.target = Some(Target::Frozen);
                            }
                            None => c.fixed = Some("the displayed tag does not end with the frozen text"),
                        }
                    }
                    _ => c.fixed = Some("derived: change the numbering or freeze the tag"),
                }
                candidates.push(c);
            }
            if wants(FindField::Property)
                && let Some(Value::Array(items)) = json.get("tech_data")
            {
                for (i, item) in items.iter().enumerate() {
                    let key = item.get("key").and_then(Value::as_str).unwrap_or_default();
                    let row_path = format!("tech_data[{i}].value");
                    match item.get("value") {
                        Some(Value::String(v)) => {
                            let text = ew_mltext::to_nfc(&format!("{key}={v}")).into_owned();
                            let prefix = ew_mltext::to_nfc(&format!("{key}=")).len();
                            let mut c = base(row_path, "find-field-property", text);
                            c.span = (prefix.min(c.text.len()), c.text.len());
                            c.target = Some(Target::TechValue(i));
                            candidates.push(c);
                        }
                        Some(v @ Value::Object(_)) => {
                            let shown = format_field_value(&FieldType::Quantity, v);
                            let text = ew_mltext::to_nfc(&format!("{key}={shown}")).into_owned();
                            let mut c = base(row_path, "find-field-property", text);
                            c.fixed = Some("a quantity: edit it in the property grid");
                            candidates.push(c);
                        }
                        _ => {}
                    }
                }
            }
            if wants(FindField::Text)
                && let Some(Value::String(t)) = json.get("process_tag")
            {
                let text = ew_mltext::to_nfc(t).into_owned();
                candidates.push(Candidate {
                    target: Some(Target::Plain {
                        member: "process_tag",
                    }),
                    ..base("process_tag".to_owned(), "find-field-process-tag", text)
                });
            }
            if wants(FindField::PartNumber)
                && let Some(Value::Array(parts)) = json.get("parts")
            {
                for (i, part) in parts.iter().enumerate() {
                    let copy = part
                        .get("part")
                        .and_then(|p| p.get("copy"))
                        .and_then(Value::as_str)
                        .and_then(|s| Uuid::parse_str(s).ok());
                    let Some(p) = copy.and_then(|c| q.parts().get(Id::from_uuid(c))) else {
                        continue;
                    };
                    let text = ew_mltext::to_nfc(&format!(
                        "{} ({})",
                        p.item.part_number.as_str(),
                        ew_derive::manufacturer_name(&p.item.manufacturer)
                    ))
                    .into_owned();
                    let mut c = base(format!("parts[{i}].part"), "find-field-part-number", text);
                    c.fixed = Some("library copy: update from library");
                    candidates.push(c);
                }
            }
        }
        // Pages: the title and the description override.
        if wants(FindField::Text) {
            for (p, folio) in &drawn {
                if !in_scope(p.as_uuid()) || !page_ok(Some(*p)) {
                    continue;
                }
                let Some(page) = q.pages().get(*p) else {
                    continue;
                };
                let Ok(json) = serde_json::to_value(&**page) else {
                    continue;
                };
                let title = ml_strings(json.get("title"));
                let shown = title
                    .iter()
                    .find(|(l, _)| l.is_none() || l.as_deref() == source.as_deref())
                    .or(title.first())
                    .map_or("", |(_, t)| t.as_str());
                let subject = format!("{folio} {shown}").trim_end().to_owned();
                for (member, label_key) in [
                    ("title", "find-field-title"),
                    ("description_override", "find-field-description"),
                ] {
                    for (lang, text) in ml_strings(json.get(member)) {
                        let text = ew_mltext::to_nfc(&text).into_owned();
                        candidates.push(Candidate {
                            record: p.as_uuid(),
                            kind: Kind::Page,
                            row_path: ml_path(member, lang.as_deref()),
                            label_key,
                            subject: subject.clone(),
                            page: Some(*p),
                            cell: None,
                            span: (0, text.len()),
                            text,
                            fixed: None,
                            target: Some(Target::Ml { member, lang }),
                        });
                    }
                }
            }
        }

        // Matching.
        let replace = query.replace.as_deref();
        let mut found: Vec<(Candidate, FindHit)> = Vec::new();
        for c in candidates {
            let matches = matcher.run(&c.text, query.whole_word, replace);
            if matches.is_empty() {
                continue;
            }
            let replaceable = match c.fixed {
                Some(why) => Err(why.to_owned()),
                None if matches.iter().any(|((s, e), _)| *s < c.span.0 || *e > c.span.1) => {
                    Err(match c.target {
                        Some(Target::Frozen) => {
                            "the match reaches into the structure prefix or the :cp suffix: change the structure"
                        }
                        _ => "the match reaches into the key: edit it in the property grid",
                    }
                    .to_owned())
                }
                None => Ok(()),
            };
            let mut after = match (replace, &replaceable) {
                (Some(_), Ok(())) => Some(replaced(&c.text, &matches)),
                _ => None,
            };
            // NFC may recompose the replacement with the unchanged text beside the stored part (`=` and a combining
            // slash make `≠`); such a replace would write a different part than the one shown.
            let mut replaceable = replaceable;
            if let Some(a) = &after
                && !(a.starts_with(&c.text[..c.span.0])
                    && a.len() >= c.span.0 + (c.text.len() - c.span.1)
                    && a.ends_with(&c.text[c.span.1..]))
            {
                replaceable = Err(
                    "the replacement would merge with the part of the string that is not stored: \
                     edit it in the property grid"
                        .to_owned(),
                );
                after = None;
            }
            let hit = FindHit {
                record: c.record,
                kind: c.kind.name().to_owned(),
                row_path: c.row_path.clone(),
                label_key: c.label_key.to_owned(),
                subject: c.subject.clone(),
                folio: c.page.and_then(folio_of),
                cell: match (c.page.and_then(folio_of), &c.cell) {
                    (Some(f), Some(cell)) => Some(format!("{f}-{cell}")),
                    _ => None,
                },
                before: c.text.clone(),
                ranges: matches.iter().map(|(r, _)| *r).collect(),
                after,
                replaceable,
            };
            found.push((c, hit));
        }
        // The order rule (§7.1).
        found.sort_by(|(a, ha), (b, hb)| {
            let pos = |c: &Candidate| c.page.and_then(|p| position.get(&p).copied());
            let page_key = |c: &Candidate| (pos(c).is_none(), pos(c).unwrap_or(0));
            let cell_key = |c: &Candidate| c.cell.as_ref().map(|x| (x.row, x.column));
            page_key(a)
                .cmp(&page_key(b))
                .then_with(|| cell_key(a).cmp(&cell_key(b)))
                .then_with(|| ew_mltext::natural_cmp(&a.subject, &b.subject))
                .then_with(|| a.subject.cmp(&b.subject))
                .then_with(|| ew_mltext::natural_cmp(&ha.row_path, &hb.row_path))
                .then_with(|| ha.row_path.cmp(&hb.row_path))
                .then_with(|| a.record.cmp(&b.record))
        });
        if found.len() > cap {
            found.truncate(cap);
            results.truncated = true;
        }
        if replace.is_some() && !results.truncated {
            results.command = replace_params(&q, &found);
        }
        results.hits = found.into_iter().map(|(_, h)| h).collect();
        results
    }
}

/// The `fields.set_many` parameters of the replaceable hits with `after` (§7.1), or `None` when nothing changes.
fn replace_params(q: &Q, found: &[(Candidate, FindHit)]) -> Option<Value> {
    // The new value of each (record, member), starting from the record's current member, hits applied in order.
    let mut members: BTreeMap<(Uuid, &'static str), (Kind, Value, Value)> = BTreeMap::new();
    for (c, hit) in found {
        let (Some(after), Some(target)) = (&hit.after, &c.target) else {
            continue;
        };
        let tail = c.text.len() - c.span.1;
        let stored = after[c.span.0..after.len() - tail].to_owned();
        let member = target.member();
        let entry = members.entry((c.record, member)).or_insert_with(|| {
            let current = current_member(q, c.kind, c.record, member);
            (c.kind, current.clone(), current)
        });
        let value = &mut entry.2;
        match target {
            Target::Ml { lang: None, .. } | Target::Plain { .. } => *value = Value::String(stored),
            Target::Ml { lang: Some(l), .. } => {
                if let Value::Object(m) = value {
                    m.insert(l.clone(), Value::String(stored));
                }
            }
            Target::Frozen => {
                if let Value::Object(m) = value {
                    m.insert("text".to_owned(), Value::String(stored));
                }
            }
            Target::TechValue(i) => {
                if let Some(Value::Object(item)) = value.as_array_mut().and_then(|a| a.get_mut(*i)) {
                    item.insert("value".to_owned(), Value::String(stored));
                }
            }
        }
    }
    // One entry per (kind, path, value) with every record that gets it.
    let mut entries: Vec<(Kind, &'static str, Value, Vec<Uuid>)> = Vec::new();
    for ((record, member), (kind, before, after)) in members {
        if before == after {
            continue;
        }
        match entries
            .iter_mut()
            .find(|(k, m, v, _)| *k == kind && *m == member && *v == after)
        {
            Some(e) => e.3.push(record),
            None => entries.push((kind, member, after, vec![record])),
        }
    }
    if entries.is_empty() {
        return None;
    }
    for e in &mut entries {
        e.3.sort();
    }
    entries.sort_by(|a, b| (a.0.rank(), a.1, a.3.first()).cmp(&(b.0.rank(), b.1, b.3.first())));
    let edits: Vec<Value> = entries
        .into_iter()
        .map(|(kind, member, value, targets)| {
            let targets: Vec<String> = targets.iter().map(ToString::to_string).collect();
            json!({"kind": kind.name(), "targets": targets, "path": member, "value": value})
        })
        .collect();
    Some(json!({ "edits": edits }))
}

/// The JSON of `member` (a dotted path) of the record `id`, `null` when absent.
fn current_member(q: &Q, kind: Kind, id: Uuid, member: &str) -> Value {
    let json = match kind {
        Kind::Device => q
            .devices()
            .get(Id::<Device>::from_uuid(id))
            .and_then(|d| serde_json::to_value(&**d).ok()),
        Kind::Page => q
            .pages()
            .get(Id::<Page>::from_uuid(id))
            .and_then(|p| serde_json::to_value(&**p).ok()),
        _ => None,
    };
    let mut cur = json.as_ref();
    for seg in member.split('.') {
        cur = cur.and_then(|v| v.get(seg));
    }
    cur.cloned().unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str, case: bool) -> Matcher {
        Matcher::new(&FindQuery {
            text: text.into(),
            match_case: case,
            ..FindQuery::default()
        })
        .unwrap()
    }

    #[test]
    fn plain_matches_are_non_overlapping_and_fold_case() {
        let m = plain("aa", true);
        let r: Vec<_> = m.run("aaaa", false, None).into_iter().map(|x| x.0).collect();
        assert_eq!(r, [(0, 2), (2, 4)]);
        let m = plain("SPARE", false);
        let r: Vec<_> = m
            .run("Spare, spare", false, None)
            .into_iter()
            .map(|x| x.0)
            .collect();
        assert_eq!(r, [(0, 5), (7, 12)]);
        // A character whose folding is two characters is matched whole: `İ` folds to `i̇`.
        let m = plain("i\u{307}x", false);
        let r: Vec<_> = m.run("\u{130}X", false, None).into_iter().map(|x| x.0).collect();
        assert_eq!(r, [(0, 3)]);
        assert!(
            plain("i", false).run("\u{130}", false, None).is_empty(),
            "never half a character"
        );
    }

    #[test]
    fn whole_words_and_replacements() {
        let m = plain("spare", false);
        let hits = m.run("Spare spares _spare spare.", true, Some("Reserve"));
        let r: Vec<_> = hits.iter().map(|x| x.0).collect();
        assert_eq!(r, [(0, 5), (20, 25)]);
        assert_eq!(
            replaced("Spare spares _spare spare.", &hits),
            "Reserve spares _spare Reserve."
        );
        let re = Matcher::new(&FindQuery {
            text: "(Spare)".into(),
            regex: true,
            match_case: true,
            ..FindQuery::default()
        })
        .unwrap();
        let hits = re.run("Spare", false, Some("$1 (unused)"));
        assert_eq!(replaced("Spare", &hits), "Spare (unused)");
    }

    /// The replaced string is NFC: a combining mark in the replacement composes with the character before it.
    #[test]
    fn the_replaced_string_is_in_nfc() {
        let hits = [((4, 5), "\u{301}".to_owned())];
        assert_eq!(replaced("Cafe-", &hits), "Caf\u{e9}");
    }

    /// §7.1: beyond the cap the hits are the first `cap` in order, `truncated` is set and replace is refused.
    #[test]
    fn beyond_the_cap_the_results_are_truncated_and_replace_is_refused() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
        let s = Session::open_read_only(&dir, "ElektraWatt test").unwrap().0;
        let q = FindQuery {
            text: "Spare".into(),
            fields: vec![FindField::FunctionText],
            replace: Some("Reserve".into()),
            ..FindQuery::default()
        };
        let full = s.find(&q);
        assert!(full.hits.len() > 5 && !full.truncated);
        assert!(full.replace_command().is_some());
        let capped = s.find_capped(&q, 5);
        assert!(capped.truncated);
        assert_eq!(capped.hits, full.hits[..5]);
        assert_eq!(capped.replace_command(), None);
    }

    #[test]
    fn bad_patterns_are_errors() {
        let long = FindQuery {
            text: "a".repeat(1_001),
            regex: true,
            ..FindQuery::default()
        };
        assert!(Matcher::new(&long).is_err());
        let bad = FindQuery {
            text: "(".into(),
            regex: true,
            ..FindQuery::default()
        };
        assert!(Matcher::new(&bad).unwrap_err().contains("does not compile"));
    }
}
