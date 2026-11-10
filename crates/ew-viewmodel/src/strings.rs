// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The texts of the shell, by key, in the UI language (spec section 5.2).
//!
//! Every user-facing text of `ew-viewmodel` and `ew-ui` is a message of the en-GB Fluent file
//! `locales/en-GB/shell.ftl`; no other literal reaches a screen, a menu, a banner or a transcript. The code names a key
//! with [`key!`](crate::key), checked at compile time against [`KEYS`] (the key list `cargo xtask settle` generates
//! from that file), or, for a key built at run time, with [`Key::run_time`]. A text may take the arguments
//! `{ $p0 }`, `{ $p1 }`, … that [`format()`] fills in, always as strings; [`text`] shows them as `{0}`, `{1}`, ….
//!
//! The UI language is process-wide ([`set_locale`]): `en` (en-GB, the source and the fallback of every other message)
//! and the pseudo-locale `en-XA` of the tests (D11), generated at load from en-GB and never shipped as a file.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, PoisonError, RwLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};

/// The key list that `build.rs` writes from the en-GB locale file.
mod keys {
    include!(concat!(env!("OUT_DIR"), "/keys.rs"));
}

pub use keys::KEYS;

/// A text key: a key of [`KEYS`] that [`text`], [`format()`] and [`format_count`] look up.
///
/// A key written in the code is [`key!`](crate::key)`("…")`, checked at compile time: a misspelt key fails the build.
/// A key built at run time (a family such as `action-<id>` or `tool-<name>`) goes through [`Key::run_time`], the one
/// door that skips the check; its miss shows `?`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key<'a>(&'a str);

impl<'a> Key<'a> {
    /// A key built at run time, unchecked: a key without a text shows `?`.
    pub const fn run_time(key: &'a str) -> Self {
        Self(key)
    }

    /// The key as text.
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

/// A count text key: a key of [`KEYS`] with its `-one` text, the only key [`format_count`] takes. It is
/// made by [`count_key!`](crate::count_key), which checks both texts at compile time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CountKey(&'static str);

impl CountKey {
    /// The constructor behind [`count_key!`](crate::count_key); call the macro instead, which checks both texts.
    #[doc(hidden)]
    pub const fn checked_by_macro(key: &'static str) -> Self {
        Self(key)
    }

    /// The key as text (the text for any count but one).
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Whether `key` is a key of [`KEYS`]: the compile-time check of [`key!`](crate::key) (lengths first, then bytes).
pub const fn has_key(key: &str) -> bool {
    let wanted = key.as_bytes();
    let mut i = 0;
    while i < KEYS.len() {
        let candidate = KEYS[i].as_bytes();
        if candidate.len() == wanted.len() {
            let mut j = 0;
            while j < wanted.len() && candidate[j] == wanted[j] {
                j += 1;
            }
            if j == wanted.len() {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// The text key `$k`, a string literal, checked at compile time against [`KEYS`](crate::strings::KEYS).
///
/// ```
/// use ew_viewmodel::{key, strings};
/// assert_eq!(strings::text(key!("menu-file")), "File");
/// ```
///
/// A key without a text fails the build:
///
/// ```compile_fail
/// let _ = ew_viewmodel::key!("menu-flie");
/// ```
#[macro_export]
macro_rules! key {
    ($k:literal) => {
        const {
            assert!(
                $crate::strings::has_key($k),
                concat!("unknown text key: ", $k)
            );
            $crate::strings::Key::run_time($k)
        }
    };
}

/// The count text key `$k`, a string literal, for [`format_count`](crate::strings::format_count): checked at compile
/// time for both `$k` (any count but one) and `$k-one` (exactly one).
///
/// ```
/// use ew_viewmodel::{count_key, key, strings};
/// assert_eq!(strings::format_count(count_key!("toast-recovered"), 1), strings::text(key!("toast-recovered-one")));
/// ```
///
/// A key without its `-one` text fails the build:
///
/// ```compile_fail
/// let _ = ew_viewmodel::count_key!("menu-file");
/// ```
///
/// and [`format_count`](crate::strings::format_count) takes no other key:
///
/// ```compile_fail
/// use ew_viewmodel::{key, strings};
/// let _ = strings::format_count(key!("toast-recovered"), 2);
/// ```
#[macro_export]
macro_rules! count_key {
    ($k:literal) => {
        const {
            assert!(
                $crate::strings::has_key($k),
                concat!("unknown text key: ", $k)
            );
            assert!(
                $crate::strings::has_key(concat!($k, "-one")),
                concat!("unknown text key: ", $k, "-one")
            );
            $crate::strings::CountKey::checked_by_macro($k)
        }
    };
}

/// The en-GB Fluent source of the shell's texts: the source language, and the fallback of every other message.
const EN_GB: &str = include_str!("../locales/en-GB/shell.ftl");

/// The nl-NL Fluent source of the shell's texts (D12): the settings value `nl`.
const NL_NL: &str = include_str!("../locales/nl-NL/shell.ftl");

/// The tag of the pseudo-locale of the tests (D11).
pub const PSEUDO: &str = "en-XA";

/// The arguments a text may take: `{ $p0 }` to `{ $p9 }`.
const ARGS: usize = 10;

type Bundle = FluentBundle<FluentResource>;

/// A loaded UI language: its tag, its bundle (none for the pseudo-locale, whose texts are generated) and every
/// message formatted once, the arguments shown as `{0}`, `{1}`, … (bounded: keys × loaded languages).
struct Locale {
    tag: String,
    bundle: Option<Bundle>,
    texts: BTreeMap<&'static str, &'static str>,
}

impl Locale {
    /// The language `tag` from the Fluent source `source`. A resource that does not parse is a defect the tests
    /// catch (`every_shipped_resource_parses`); the entries that parse are still served.
    fn from_source(tag: &str, source: &str) -> Locale {
        let id: unic_langid::LanguageIdentifier = tag.parse().unwrap_or_default();
        let mut bundle = Bundle::new_concurrent(vec![id]);
        bundle.set_use_isolating(false);
        // ID-1278 ruling 9: `IN_SENTENCE($p0)` puts a label mid-sentence ([`in_sentence`]), for a language whose
        // sentence lowers a name ("Voorbeeld: apparaat plaatsen") where en-GB keeps it.
        let _ = bundle.add_function("IN_SENTENCE", |positional, _| match positional.first() {
            Some(FluentValue::String(s)) => FluentValue::from(in_sentence(s)),
            Some(other) => other.clone(),
            None => FluentValue::None,
        });
        let resource = FluentResource::try_new(source.to_owned()).unwrap_or_else(|(r, _)| r);
        // A duplicate entry keeps the first; the tests reject duplicates.
        let _ = bundle.add_resource(resource);
        let mut args = FluentArgs::new();
        for i in 0..ARGS {
            args.set(format!("p{i}"), FluentValue::from(format!("{{{i}}}")));
        }
        let mut texts = BTreeMap::new();
        // The keys of en-GB only: no code names another.
        for key in KEYS {
            let Some(pattern) = bundle.get_message(key).and_then(|m| m.value()) else {
                continue;
            };
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, Some(&args), &mut errors);
            texts.insert(*key, &*Box::leak(text.into_owned().into_boxed_str()));
        }
        Locale {
            tag: tag.to_owned(),
            bundle: Some(bundle),
            texts,
        }
    }

    /// The pseudo-locale en-XA from `base` (D11): every text's letters accented, padded by 35 % of its length (rounded
    /// up) and bracketed with `⟦ ⟧`; the arguments `{0}`, `{1}`, … stay as they are.
    fn pseudo(base: &Locale) -> Locale {
        let texts = base
            .texts
            .iter()
            .map(|(k, t)| {
                let t: &'static str = Box::leak(pseudo_text(t).into_boxed_str());
                (*k, t)
            })
            .collect();
        Locale {
            tag: PSEUDO.to_owned(),
            bundle: None,
            texts,
        }
    }

    /// The text of `key` with `args`, or `None` for a key this language has no message for.
    fn format(&self, key: &str, args: &[&str]) -> Option<String> {
        let template = *self.texts.get(key)?;
        let Some(bundle) = &self.bundle else {
            // The pseudo-locale: its texts are templates with `{0}`, `{1}`, ….
            let mut out = template.to_owned();
            for (i, arg) in args.iter().enumerate() {
                out = out.replace(&format!("{{{i}}}"), arg);
            }
            return Some(out);
        };
        let pattern = bundle.get_message(key)?.value()?;
        let mut fluent_args = FluentArgs::new();
        for i in 0..ARGS {
            let value = args
                .get(i)
                .map_or_else(|| format!("{{{i}}}"), |a| (*a).to_owned());
            fluent_args.set(format!("p{i}"), FluentValue::from(value));
        }
        let mut errors = Vec::new();
        Some(
            bundle
                .format_pattern(pattern, Some(&fluent_args), &mut errors)
                .into_owned(),
        )
    }
}

/// `text` in the pseudo-locale (D11): see [`Locale::pseudo`].
fn pseudo_text(text: &str) -> String {
    const FROM: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const TO: &str = "áƀçďéƒĝĥíĵķĺɱñóþǫŕšţúṽŵẋýžÁƁÇĎÉƑĜĤÍĴĶĹṀÑÓÞǪŔŠŢÚṼŴẊÝŽ";
    let mut out = String::from('⟦');
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        // An argument `{0}` … `{9}` stays as it is.
        let bytes = rest.as_bytes();
        if c == '{' && bytes.len() >= 3 && bytes[1].is_ascii_digit() && bytes[2] == b'}' {
            out.push_str(&rest[..3]);
            rest = &rest[3..];
            continue;
        }
        match FROM.find(c) {
            Some(i) => out.extend(TO.chars().nth(i)),
            None => out.push(c),
        }
        rest = &rest[c.len_utf8()..];
    }
    let pad = (text.chars().count() * 35).div_ceil(100);
    out.extend(std::iter::repeat_n('~', pad));
    out.push('⟧');
    out
}

/// en-GB, loaded once.
fn en_gb() -> &'static Locale {
    static EN: OnceLock<Locale> = OnceLock::new();
    EN.get_or_init(|| Locale::from_source("en-GB", EN_GB))
}

/// nl-NL, loaded once on its first use; its tag is the settings value `nl`.
fn nl_nl() -> &'static Locale {
    static NL: OnceLock<Locale> = OnceLock::new();
    NL.get_or_init(|| Locale::from_source("nl", NL_NL))
}

/// The languages loaded besides en-GB: the pseudo-locale once selected, and the test languages of `testkit`.
static LOADED: RwLock<Vec<&'static Locale>> = RwLock::new(Vec::new());

/// The UI language now; `None` is en-GB.
static CURRENT: RwLock<Option<&'static Locale>> = RwLock::new(None);

/// Counted up by every [`set_locale`] that selects a language: a cache of translated text keeps it beside what else
/// it was built for, so a language change rebuilds it.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The language generation: it changes whenever [`set_locale`] selects a language.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// The UI language now.
fn current() -> &'static Locale {
    CURRENT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .unwrap_or_else(en_gb)
}

/// The loaded language `tag`, if any.
fn loaded(tag: &str) -> Option<&'static Locale> {
    LOADED
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .copied()
        .find(|l| l.tag == tag)
}

/// Makes `tag` the UI language of the process: `en` (en-GB), `nl` (nl-NL), `en-XA` (the pseudo-locale, generated on
/// its first selection) or a language a test registered. Returns whether `tag` is one; another tag changes nothing.
pub fn set_locale(tag: &str) -> bool {
    let locale = match tag {
        "en" | "en-GB" => None,
        "nl" | "nl-NL" => Some(nl_nl()),
        PSEUDO => Some(loaded(PSEUDO).unwrap_or_else(|| {
            let pseudo: &'static Locale = Box::leak(Box::new(Locale::pseudo(en_gb())));
            LOADED
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .push(pseudo);
            pseudo
        })),
        other => match loaded(other) {
            Some(l) => Some(l),
            None => return false,
        },
    };
    *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = locale;
    GENERATION.fetch_add(1, Ordering::Relaxed);
    true
}

/// The tag of the UI language now: `en`, `nl`, `en-XA` or a test language's tag.
pub fn locale() -> &'static str {
    match *CURRENT.read().unwrap_or_else(PoisonError::into_inner) {
        Some(l) => &l.tag,
        None => "en",
    }
}

/// The languages View ▸ Language offers, as (settings value, the language's own name): the shipped ones
/// ([`crate::settings::LANGUAGES`]), then the test languages of `testkit` under another tag. The pseudo-locale is never
/// offered.
pub fn languages() -> Vec<(&'static str, &'static str)> {
    let name = |l: &'static Locale| l.texts.get("language-name").copied().unwrap_or("?");
    let shipped = crate::settings::LANGUAGES;
    let mut out: Vec<(&'static str, &'static str)> = shipped
        .iter()
        .map(|tag| (*tag, name(if *tag == "nl" { nl_nl() } else { en_gb() })))
        .collect();
    out.extend(
        LOADED
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|l| l.tag != PSEUDO && !shipped.contains(&l.tag.as_str()))
            .map(|l| (l.tag.as_str(), name(l))),
    );
    out
}

/// The decimal number `number` (`268.4`, written with a point) with the UI language's decimal separator (ID-1389, W51
/// finding L: `268,4` in Dutch).
pub fn decimal(number: &str) -> String {
    number.replace('.', text(key!("number-decimal-point")))
}

/// The language of the check rules' messages for the UI language now (D10): `nl-NL` for Dutch, else `en-GB`
/// (`ew-checks` falls back to en-GB per message).
pub fn rule_language() -> &'static str {
    match locale() {
        "nl" => "nl-NL",
        _ => "en-GB",
    }
}

/// Test hook (`testkit`, ruling 5): loads the Fluent source `source` as the UI language `tag`, which
/// View ▸ Language then offers and [`set_locale`] selects; a message it lacks falls back to en-GB. A tag loaded
/// before is replaced for later selections.
#[cfg(feature = "testkit")]
pub fn register_locale_for_tests(tag: &str, source: &str) {
    let locale: &'static Locale = Box::leak(Box::new(Locale::from_source(tag, source)));
    let mut all = LOADED.write().unwrap_or_else(PoisonError::into_inner);
    all.retain(|l| l.tag != tag);
    all.push(locale);
}

/// The parse errors of the en-GB Fluent source (empty when it parses): for the tests.
#[doc(hidden)]
pub fn en_gb_errors() -> Vec<String> {
    match FluentResource::try_new(EN_GB.to_owned()) {
        Ok(_) => Vec::new(),
        Err((_, errors)) => errors.iter().map(|e| format!("{e:?}")).collect(),
    }
}

/// The text of `key` in the UI language (en-GB where it has none), or `None` for a key that has none.
pub fn lookup(key: &str) -> Option<&'static str> {
    current()
        .texts
        .get(key)
        .or_else(|| en_gb().texts.get(key))
        .copied()
}

/// The headers of navigator `nav`'s column `key` (`nav` its word, `pages`) in each shipped language, en-GB first: its
/// `column-<nav>-<key>` text, else `column-<key>` (a header copied in one language pastes in another).
pub fn column_labels(nav: &str, key: &str) -> Vec<&'static str> {
    let key = key.replace('_', "-");
    [en_gb(), nl_nl()]
        .into_iter()
        .filter_map(|l| {
            l.texts
                .get(format!("column-{nav}-{key}").as_str())
                .or_else(|| l.texts.get(format!("column-{key}").as_str()))
                .copied()
        })
        .collect()
}

/// The line kinds of a fluid line (T25), in schema order: the Line kind row shows each as its
/// `choice-<kind>` text.
pub const LINE_KINDS: &[&str] = &["working", "pilot", "exhaust", "drain", "flexible"];

/// The words the humanised labels replace (`csa` → "cross-section").
const LABEL_WORDS: &[(&str, &str)] = &[
    ("csa", "cross-section"),
    ("seq", "sequence"),
    ("qty", "quantity"),
    ("id", "ID"),
    ("plc", "PLC"),
    ("ip", "IP"),
    ("cp", "connection point"),
];

/// The label of the field `path` of `kind` (list indices allowed) in the UI language, the one source of the property
/// grid's row labels, the collision dialog's field headings and the tools' texts that name a field: the
/// field's own `field-<kind>-<path>` text (`.` and `_` as `-`, a list item as `-item`; a row whose humanised name would
/// not read well, "Wire number", "Colour override"), else its last member's `field-word-<member>` text ("Number",
/// "Nummer"), else that member humanised (underscores to spaces, the word list applied, the first letter capitalised:
/// an open set's key, a part's technical data). A list item carries its position (`Connection point 2`).
pub fn field_label(kind: &str, path: &str) -> String {
    let mut generic = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' && chars.peek().is_some_and(char::is_ascii_digit) {
            for d in chars.by_ref() {
                if d == ']' {
                    break;
                }
            }
            generic.push_str("[]");
        } else {
            generic.push(c);
        }
    }
    // A list item carries its position (`Connection point 2`), so the items of a list differ.
    let item = path
        .strip_suffix(']')
        .and_then(|p| p.rsplit_once('['))
        .and_then(|(_, n)| n.parse::<usize>().ok())
        .map(|n| format!(" {}", n + 1))
        .unwrap_or_default();
    let own = format!(
        "field-{}-{}",
        kind.replace('_', "-"),
        generic.replace("[]", "-item").replace(['.', '_'], "-")
    );
    if let Some(label) = lookup(&own) {
        return format!("{label}{item}");
    }
    let last = generic
        .trim_end_matches("[]")
        .rsplit(['.', '['])
        .find(|s| !s.is_empty() && *s != "]")
        .unwrap_or(&generic)
        .trim_end_matches(']');
    let word = format!("field-word-{}", last.replace('_', "-"));
    // A list's item by its singular where the list has one ("Connection point 2").
    let singular = (!item.is_empty())
        .then(|| lookup(&format!("{word}-item")))
        .flatten();
    if let Some(label) = singular.or_else(|| lookup(&word)) {
        return format!("{label}{item}");
    }
    let words: Vec<String> = last
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            LABEL_WORDS
                .iter()
                .find(|(k, _)| *k == w)
                .map_or_else(|| w.to_owned(), |(_, v)| (*v).to_owned())
        })
        .collect();
    let text = words.join(" ");
    let mut out = String::with_capacity(text.len());
    let mut first = true;
    for c in text.chars() {
        if first {
            out.extend(c.to_uppercase());
            first = false;
        } else {
            out.push(c);
        }
    }
    out.push_str(&item);
    out
}

/// The word of a macro variable's type (W40a 5e): the words `macro.insert`'s coercion refusals use for it (a
/// `tag_counter` takes "a tag counter (an integer from 1)", so it is "tag counter"); a type this version does not
/// know is its key with spaces. The window never prints a raw type key.
pub fn var_type_word(ty: &str) -> String {
    let word = match ty {
        "text" => "text",
        "ident" => "identifier",
        "number" => "number",
        "bool" => "true or false",
        "state" => "state",
        "part" => "part reference",
        "process_tag" => "process tag",
        "channel_binding" => "channel number",
        "address" => "address",
        "tag_counter" => "tag counter",
        "device_ref" => "device",
        other => return other.replace('_', " "),
    };
    word.to_owned()
}

/// A choice value of a form dialog in words (doc 16a §8, ruling 2): the text of `choice-<value>`
/// (`_` as `-`) in the UI language, else [`choice_words`]. The parameter keeps the raw word; only the window shows
/// this.
pub fn choice_text(value: &str) -> String {
    match lookup(&format!("choice-{}", value.replace('_', "-"))) {
        Some(text) if !value.is_empty() => text.to_owned(),
        _ => choice_words(value),
    }
}

/// A check scheme's name in the UI language's words: the wizard's `choice-check_scheme-<name>` text, else
/// the name itself.
pub fn scheme_text(name: &'static str) -> &'static str {
    lookup(&format!("choice-check_scheme-{name}")).unwrap_or(name)
}

/// A choice value of the field `path` of `kind` in words: the field's own `choice-<kind>-<path>-<value>`
/// text (`.` and `_` as `-`, a list item as `-item`), where one word means two things in two fields (a terminal
/// level's `main` is no main function), else [`choice_text`].
pub fn field_choice_text(kind: &str, path: &str, value: &str) -> String {
    let generic = generic_field_path(path);
    let own = format!(
        "choice-{}-{}-{}",
        kind.replace('_', "-"),
        generic.replace("[]", "-item").replace(['.', '_'], "-"),
        value.replace('_', "-")
    );
    match lookup(&own) {
        Some(text) if !value.is_empty() => text.to_owned(),
        _ => choice_text(value),
    }
}

/// The words of the choice `value` of the field `path` of `kind` in every shipped language, as
/// [`field_choice_text`] picks them (a table cell copied in one language and pasted in the other).
pub fn field_choice_texts(kind: &str, path: &str, value: &str) -> Vec<&'static str> {
    let own = format!(
        "choice-{}-{}-{}",
        kind.replace('_', "-"),
        generic_field_path(path)
            .replace("[]", "-item")
            .replace(['.', '_'], "-"),
        value.replace('_', "-")
    );
    let generic = format!("choice-{}", value.replace('_', "-"));
    [en_gb(), nl_nl()]
        .into_iter()
        .filter_map(|l| {
            l.texts
                .get(own.as_str())
                .or_else(|| l.texts.get(generic.as_str()))
                .copied()
        })
        .collect()
}

/// A field path with every list index as `[]` (`levels[0].role` is `levels[].role`).
fn generic_field_path(path: &str) -> String {
    let mut generic = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' && chars.peek().is_some_and(char::is_ascii_digit) {
            for d in chars.by_ref() {
                if d == ']' {
                    break;
                }
            }
            generic.push_str("[]");
        } else {
            generic.push(c);
        }
    }
    generic
}

/// A value's snake_case word in sentence case, with the hyphenated compounds of the standard's usage (`multi_line` is
/// "Multi-line", `page_based` "Page-based"): the words of a value of an open set (a symbol's name), which has no key.
pub fn choice_words(value: &str) -> String {
    let words = match value {
        "multi_line" => "multi-line".to_owned(),
        "single_line" => "single-line".to_owned(),
        "user_defined" => "user-defined".to_owned(),
        "page_based" => "page-based".to_owned(),
        // The symbol editor's tools by their names in the bar.
        "cp" => "connection point".to_owned(),
        "slot" => "text slot".to_owned(),
        "link" => "link point".to_owned(),
        // `symbol.role`'s potential roles, as printed.
        "l1" | "l2" | "l3" | "l" | "n" | "pe" | "pen" | "fe" | "m" => value.to_uppercase(),
        "l_plus" => "L+".to_owned(),
        "l_minus" => "L-".to_owned(),
        other => other.replace('_', " "),
    };
    let mut chars = words.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// A record kind as a heading in the UI language (`Potential definition`, `Potentiaaldefinitie`): its
/// `record-kind-<kind>` text, else [`kind_label`] with a capital (W49.3-F3).
pub fn kind_heading(kind: &str) -> String {
    let key = format!("record-kind-{}", kind.replace('_', "-"));
    lookup(&key).map_or_else(|| sentence_start(&kind_label(kind)).into_owned(), str::to_owned)
}

/// A record kind in the plural as a heading in the UI language (`Devices`, `Apparaten`): its
/// `record-kinds-<kind>` text, else [`kind_plural`] with a capital.
pub fn kinds_heading(kind: &str) -> String {
    let key = format!("record-kinds-{}", kind.replace('_', "-"));
    lookup(&key).map_or_else(|| sentence_start(&kind_plural(kind)).into_owned(), str::to_owned)
}

/// An accessory kind of the strip editor in the UI language: the text of `accessory-<kind>` (`end_clamp`
/// is "End bracket", "Eindsteun"), `accessory-other` for a kind this version does not know. The words of
/// `ew_api::output::accessory_kind_word` in en-GB, which the table round trip keeps.
pub fn accessory_text(kind: &str) -> &'static str {
    lookup(&format!("accessory-{}", kind.replace('_', "-")))
        .unwrap_or_else(|| text(crate::key!("accessory-other")))
}

/// A record kind in words, singular (`potential definition`).
pub fn kind_label(kind: &str) -> String {
    match kind {
        "potential_def" => "potential definition".into(),
        "external_ref" => "external reference".into(),
        "plc_config_project" => "PLC configuration project".into(),
        other => other.replace('_', " "),
    }
}

/// A record kind in words, plural (`devices`, `potential definitions`).
pub fn kind_plural(kind: &str) -> String {
    let one = kind_label(kind);
    if one.ends_with("box") {
        format!("{one}es")
    } else {
        format!("{one}s")
    }
}

/// The key of the title of the command `name`: `command-<name>` with every `.` as `-` (a Fluent identifier has no dot).
fn command_key(name: &str) -> String {
    format!("command-{}", name.replace('.', "-"))
}

/// The human title of the command `name` ("Place device" for `device.place`), the text of its key `command-<name>`;
/// the name itself for a command without one.
pub fn command_title(name: &str) -> &str {
    lookup(&command_key(name)).unwrap_or(name)
}

/// The text of `key`; `?` for a key without a text (a missing text is a bug a test catches).
pub fn text(key: Key<'_>) -> &'static str {
    lookup(key.as_str()).unwrap_or("?")
}

/// The text of `key` in the UI language with its arguments `{ $p0 }`, `{ $p1 }`, … from `args`, each passed as a
/// string (an argument `args` lacks shows as `{0}`, `{1}`, … as [`text`] shows it); `?` for a key without a text.
pub fn format(key: Key<'_>, args: &[&str]) -> String {
    current()
        .format(key.as_str(), args)
        .or_else(|| en_gb().format(key.as_str(), args))
        .unwrap_or_else(|| "?".to_owned())
}

/// A sentence `text` without its closing full stop, so that another part can follow it inside the same sentence
/// (every toast text ends with its full stop, doc 16a §8 item 5).
pub fn without_stop(text: &str) -> &str {
    text.strip_suffix('.').unwrap_or(text)
}

/// A column or field label inside a sentence (sentence case): its first letter lower-cased ("Address" reads
/// "address", "Function text" "function text"), unless the label starts with an abbreviation, two capitals in a row
/// ("DT", "PLC address"), which stays as it is.
pub fn in_sentence(label: &str) -> String {
    let mut chars = label.chars();
    match (chars.next(), chars.next()) {
        (Some(first), second) if first.is_uppercase() && !second.is_some_and(char::is_uppercase) => {
            first.to_lowercase().chain(label.chars().skip(1)).collect()
        }
        _ => label.to_owned(),
    }
}

/// A part of a message the window shows on its own line (FT-1075, ID-1182: a toast's line under its title, a
/// Properties refusal), starting with a capital: the text of a command or a check stays lowercase after its title or
/// its prefix (doc 16a §8.1), and the window capitalises where the part stands alone. A rule code first ("EW-CORE-052:
/// ") is left as it is and the words after it start with the capital; a designation, a code or a digit first ("-K1
/// exists", "W464: …", "4 errors": no lowercase letter first, or a first word that holds a digit) is left as it is.
pub fn sentence_start(part: &str) -> Cow<'_, str> {
    /// A rule code with its colon, `EW-[A-Z]+-[0-9]+:` (item 2): a designation ("-K1:") is none.
    fn is_code(word: &str) -> bool {
        word.strip_prefix("EW-")
            .and_then(|w| w.strip_suffix(':'))
            .and_then(|w| w.split_once('-'))
            .is_some_and(|(area, number)| {
                !area.is_empty()
                    && area.chars().all(|ch| ch.is_ascii_uppercase())
                    && !number.is_empty()
                    && number.chars().all(|ch| ch.is_ascii_digit())
            })
    }
    // The leading codes, however many, stay as they are.
    let mut start = 0;
    while let Some((code, _)) = part[start..].split_once(' ')
        && is_code(code)
    {
        start += code.len() + 1;
    }
    let (codes, rest) = part.split_at(start);
    let first_word = rest.split(char::is_whitespace).next().unwrap_or_default();
    match rest.chars().next() {
        Some(first) if first.is_lowercase() && !first_word.chars().any(|c| c.is_ascii_digit()) => Cow::Owned(
            codes
                .chars()
                .chain(first.to_uppercase())
                .chain(rest.chars().skip(1))
                .collect(),
        ),
        _ => Cow::Borrowed(part),
    }
}

/// A count-dependent string: `<key>-one` for exactly one, `<key>` with the count as `{0}` otherwise. The key is a
/// [`CountKey`], made by [`count_key!`](crate::count_key), which checks both texts at compile time.
pub fn format_count(key: CountKey, count: usize) -> String {
    if count == 1 {
        text(Key::run_time(&format!("{}-one", key.as_str()))).to_owned()
    } else {
        format(Key::run_time(key.as_str()), &[&count.to_string()])
    }
}

/// The time from epoch milliseconds `last_ms` to `now_ms` in words: `just now` under a minute (also for a
/// `last_ms` after `now_ms`, a clock skew, never a negative), then whole minutes, hours or days, rounded down.
pub fn elapsed_text(last_ms: u64, now_ms: u64) -> String {
    const MINUTE: u64 = 60_000;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    let span = now_ms.saturating_sub(last_ms);
    let count = |unit: u64| usize::try_from(span / unit).unwrap_or(usize::MAX);
    if span < MINUTE {
        text(key!("elapsed-just-now")).to_owned()
    } else if span < HOUR {
        format_count(count_key!("elapsed-minutes"), count(MINUTE))
    } else if span < DAY {
        format_count(count_key!("elapsed-hours"), count(HOUR))
    } else {
        format_count(count_key!("elapsed-days"), count(DAY))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    const NOW: u64 = 1_793_000_000_000;

    /// Every key with its en-GB text.
    fn texts() -> impl Iterator<Item = (&'static str, &'static str)> {
        KEYS.iter().map(|k| (*k, en_gb().texts[k]))
    }

    /// A part shown on its own line starts with a capital; a designation, a code or a digit first
    /// is left as it is, and a rule code first leaves the words after it to start with the capital.
    #[test]
    fn a_part_on_its_own_line_starts_with_a_capital() {
        for (part, shown) in [
            (
                "the interruption point W464 is already here",
                "The interruption point W464 is already here",
            ),
            (
                "interruption point W464: a star source is a source point",
                "Interruption point W464: a star source is a source point",
            ),
            ("draw on a schematic page", "Draw on a schematic page"),
            ("on -101F1: refused", "On -101F1: refused"),
            (
                "EW-CORE-052: a second source of 0V",
                "EW-CORE-052: A second source of 0V",
            ),
            ("W464: a star source", "W464: a star source"),
            // FT-1113 (item 2): a designation with a colon is no rule code, and a chain of codes is skipped.
            ("-K1: the coil of the relay", "-K1: the coil of the relay"),
            (
                "EW-CORE-052: EW-POT-011: a second source",
                "EW-CORE-052: EW-POT-011: A second source",
            ),
            ("-K1 exists", "-K1 exists"),
            ("k1 exists", "k1 exists"),
            ("4 errors, 25 warnings", "4 errors, 25 warnings"),
            ("Already a capital", "Already a capital"),
            ("", ""),
        ] {
            assert_eq!(sentence_start(part), shown, "{part}");
        }
    }

    /// Whether `name` stands in `text` as a key's name: not inside a word, a chord (`+`), a tag (`-X1`) or a key.
    fn names_key(text: &str, name: &str) -> bool {
        let part = |c: char| c.is_alphanumeric() || matches!(c, '+' | '-' | '_' | '{' | '}');
        text.match_indices(name).any(|(i, _)| {
            let before = text[..i].chars().next_back();
            let after = text[i + name.len()..].chars().next();
            !before.is_some_and(part) && !after.is_some_and(part)
        })
    }

    /// FT-1081 (doc 16a §8 item 7): a key has one name in every text, the form on its keycap. No text names
    /// the Escape key in full, and the long forms `short_display` shortens ("Arrow Up") stand in none.
    #[test]
    fn ft1081_no_text_names_a_key_by_its_long_form() {
        for (key, text) in texts() {
            assert!(!names_key(text, "Escape"), "{key}: {text}");
            assert!(!text.contains("Arrow "), "{key}: {text}");
        }
        // The short form is the one every action's key takes.
        assert_eq!(crate::keymap::short_display("Escape"), "Esc");
        assert_eq!(crate::keymap::short_display("Alt+Delete"), "Alt+Del");
    }

    /// The scan's word test: a name inside a word, a chord or a tag is no key's name.
    #[test]
    fn ft964_the_scan_finds_a_key_name_only_as_a_word() {
        assert!(names_key("Click or Enter to place", "Enter"));
        assert!(names_key("Esc done", "Esc"));
        assert!(names_key("(R)", "R"));
        assert!(!names_key("Escape", "Esc"));
        assert!(!names_key("Shift+Enter", "Enter"));
        assert!(!names_key("-X1031:21", "X"));
        assert!(!names_key("Click or {0} to place", "0"));
    }

    /// Cycle 35 play-test: the preview card and chip name the command by its title, not its ID.
    #[test]
    fn the_preview_texts_name_the_command_by_its_title() {
        assert_eq!(command_title("device.place"), "Place device");
        assert_eq!(
            format(key!("preview-chip"), &[command_title("device.place")]),
            "Preview of Place device"
        );
        assert_eq!(
            format(key!("card-title-properties"), &[command_title("fields.set")]),
            "Preview of Set property"
        );
        // A command without a title keeps its ID.
        assert_eq!(command_title("plugin.custom"), "plugin.custom");
    }

    /// RD43 §9 item 9: every built-in command has its title key, and every `command-` key of `KEYS` names a built-in
    /// command.
    #[test]
    fn the_command_title_keys_are_the_builtin_commands() {
        let keys: Vec<String> = ew_api::CommandRegistry::builtin()
            .names()
            .map(command_key)
            .collect();
        let missing: Vec<String> = keys
            .iter()
            .filter(|k| lookup(k).is_none())
            .map(|k| format!("the key {k} has no text"))
            .collect();
        assert_eq!(missing, Vec::<String>::new());
        let table: Vec<&str> = KEYS
            .iter()
            .copied()
            .filter(|k| k.starts_with("command-"))
            .collect();
        assert!(table.contains(&"command-device-place"), "{table:?}");
        let stray: Vec<String> = table
            .iter()
            .filter(|k| !keys.iter().any(|n| n == *k))
            .map(|k| format!("the key {k} names no built-in command"))
            .collect();
        assert_eq!(stray, Vec::<String>::new());
    }

    /// FT-1263 (C49-p6b#4): a key that has a `-one` text is used with its count, through `count_key!`, never as a
    /// plain `key!` whose text would read "1 records", unless its file names the `-one` key too (a branch that says
    /// one itself); `KEPT` names the other uses (file, key) that never show a count of one.
    #[test]
    fn a_key_with_a_one_text_is_used_with_its_count() {
        const KEPT: &[(&str, &str)] = &[];
        fn scan(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("the sources") {
                let path = entry.expect("a directory entry").path();
                if path.is_dir() {
                    scan(&path, files);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    files.push(path);
                }
            }
        }
        let counted: Vec<&str> = KEYS
            .iter()
            .filter_map(|k| k.strip_suffix("-one"))
            .filter(|k| has_key(k))
            .collect();
        assert!(counted.contains(&"toast-recovered"), "{counted:?}");
        // The window's sources too, which use the shell's keys.
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        for dir in ["ew-viewmodel/src", "ew-ui/src"] {
            scan(&src.join(dir), &mut files);
        }
        files.sort();
        assert!(
            files.iter().any(|f| f.starts_with(src.join("ew-ui/src"))),
            "the window's sources"
        );
        let mut plain = Vec::new();
        let mut seen = 0;
        for file in &files {
            let name = file
                .strip_prefix(&src)
                .unwrap_or(file)
                .display()
                .to_string()
                .replace('\\', "/");
            let text = std::fs::read_to_string(file).expect("a source");
            for (at, _) in text.match_indices("key!(\"") {
                if text[..at].ends_with("count_") {
                    continue;
                }
                let rest = &text[at + "key!(\"".len()..];
                let key = &rest[..rest.find('"').unwrap_or(0)];
                // A file that also names the `-one` key branches on one itself.
                let branches = text.contains(&format!("key!(\"{key}-one\")"));
                seen += usize::from(counted.contains(&key) && branches);
                if counted.contains(&key) && !branches && !KEPT.contains(&(name.as_str(), key)) {
                    plain.push(format!("{name}: key!(\"{key}\")"));
                }
            }
        }
        assert!(seen > 0, "the scan finds the branches that say one themselves");
        assert_eq!(plain, Vec::<String>::new());
    }

    /// The part editor's field labels are built at run time as `part-field-<f>` over `ew_api::DRAFT_FIELDS`
    /// (`describe.rs`, `part_dialog.rs`): every field has its text.
    #[test]
    fn every_draft_field_has_a_label() {
        let missing: Vec<String> = ew_api::DRAFT_FIELDS
            .iter()
            .filter(|f| lookup(&format!("part-field-{f}")).is_none())
            .map(|f| format!("the draft field {f} has no part-field-{f} text"))
            .collect();
        assert_eq!(missing, Vec::<String>::new());
        assert_eq!(lookup("part-field-part_number"), Some("Part number"));
    }

    /// RD43 §5: every key of `KEYS` is there once (`key!` checks against it), and the en-GB file parses with a
    /// message for every key.
    #[test]
    fn the_text_keys_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        let twice: Vec<String> = KEYS
            .iter()
            .filter(|key| !seen.insert(**key))
            .map(|key| format!("the key {key} is in KEYS twice"))
            .collect();
        assert_eq!(twice, Vec::<String>::new());
        assert_eq!(en_gb_errors(), Vec::<String>::new());
        let missing: Vec<&str> = KEYS
            .iter()
            .copied()
            .filter(|k| !en_gb().texts.contains_key(k))
            .collect();
        assert_eq!(missing, Vec::<&str>::new());
    }

    /// D12: the nl-NL file parses, has a message for every key and a Dutch name.
    #[test]
    fn the_dutch_file_parses_with_every_key() {
        if let Err((_, errors)) = FluentResource::try_new(NL_NL.to_owned()) {
            panic!("{errors:?}");
        }
        let missing: Vec<&str> = KEYS
            .iter()
            .copied()
            .filter(|k| !nl_nl().texts.contains_key(k))
            .collect();
        assert_eq!(missing, Vec::<&str>::new());
        assert_eq!(nl_nl().texts["language-name"], "Nederlands");
    }

    /// RD43 §5: a key built at run time looks up the same text as the literal key, and a miss shows `?`.
    #[test]
    fn a_run_time_key_reads_the_same_table() {
        let name = String::from("menu-file");
        assert_eq!(text(Key::run_time(&name)), text(key!("menu-file")));
        assert_eq!(key!("menu-file").as_str(), "menu-file");
        assert!(has_key("menu-file"));
        assert!(!has_key("menu-fil"));
        assert!(!has_key("menu-file-"));
        assert!(!has_key("menu-fild"));
        assert!(!has_key(""));
        assert_eq!(text(Key::run_time("no-such-key")), "?");
    }

    /// Cycle 35 play-test: the refusal of a page type without symbols names the remedy, as the paste refusal does.
    #[test]
    fn the_page_type_refusal_names_the_remedy() {
        assert_eq!(
            text(key!("reason-page-type-no-symbols")),
            "This page type takes no symbols or wires: draw on a schematic page"
        );
        // A drawn table of contents by name (Golden A's is a generated report page, so the window test cannot
        // reach this kind).
        assert_eq!(
            text(key!("reason-page-type-no-symbols-contents")),
            "A table of contents takes no symbols or wires: draw on a schematic page"
        );
    }

    /// A record of a kind without words of its own is "an item" ("een element"), not the jargon "record".
    #[test]
    fn the_kind_fallback_names_an_item() {
        assert_eq!(en_gb().texts["refusal-words-kind-other"], "an item");
        assert_eq!(nl_nl().texts["refusal-words-kind-other"], "een element");
    }

    /// Every field of every record kind's catalogue with its value type: (kind, path, type), the kinds read from the
    /// `record-kind-*` texts.
    fn catalogue_fields() -> Vec<(String, String, ew_api::output::FieldType)> {
        let mut out = Vec::new();
        for line in include_str!("../locales/en-GB/shell.ftl").lines() {
            let Some((kind, _)) = line
                .strip_prefix("record-kind-")
                .and_then(|l| l.split_once(" = "))
            else {
                continue;
            };
            let kind = kind.replace('-', "_");
            for s in ew_api::output::field_catalogue(&kind)
                .iter()
                .flat_map(|c| c.iter())
            {
                out.push((kind.clone(), s.path.clone(), s.ty.clone()));
            }
        }
        out
    }

    /// The keys of the nl-NL file.
    fn nl_keys() -> BTreeSet<&'static str> {
        include_str!("../locales/nl-NL/shell.ftl")
            .lines()
            .filter_map(|l| l.split_once(" = ").map(|(k, _)| k))
            .collect()
    }

    /// Every field label the property grid, the collision dialog or a tool shows comes from one
    /// source, a key of both languages: the field's own `field-<kind>-<path>` text, else its last member's
    /// `field-word-<member>` (the record version `$v` alone is no word). The en-GB labels are the ones of before;
    /// nl-NL's are its own.
    #[test]
    fn every_field_label_has_a_key_in_both_languages() {
        let fields = catalogue_fields();
        assert!(
            fields
                .iter()
                .any(|(k, p, _)| k == "device" && p == "function_text")
        );
        let nl = nl_keys();
        let mut missing = BTreeSet::new();
        for (kind, path, _) in &fields {
            let generic = path.replace("[]", "-item").replace(['.', '_'], "-");
            let own = format!("field-{}-{generic}", kind.replace('_', "-"));
            let last = path
                .trim_end_matches("[]")
                .rsplit(['.', '['])
                .find(|s| !s.is_empty())
                .unwrap_or(path)
                .trim_end_matches(']');
            if last == "$v" {
                continue;
            }
            let word = format!("field-word-{}", last.replace('_', "-"));
            if !has_key(&own) && !has_key(&word) {
                missing.insert(format!("en-GB: {kind} {path}"));
            }
            if !nl.contains(own.as_str()) && !nl.contains(word.as_str()) {
                missing.insert(format!("nl-NL: {kind} {path}"));
            }
        }
        assert_eq!(missing, BTreeSet::new());
        assert_eq!(field_label("device", "function_text"), "Function text");
        assert_eq!(field_label("connection", "overrides.colour"), "Colour override");
        assert_eq!(field_label("function", "cps[1]"), "Connection point 2");
        assert_eq!(field_label("function", "cps"), "Connection points");
        // The humanised English labels written by hand.
        let members = [
            "pe",
            "pe_bond",
            "cpu",
            "sha256",
            "bom_basis",
            "idx",
            "ac_dc",
            "den",
            "parties",
            "termination",
        ];
        assert_eq!(
            members.map(|m| field_label("", m)),
            [
                "PE",
                "PE bond",
                "CPU",
                "SHA-256",
                "BOM basis",
                "Index",
                "AC/DC",
                "Grid step divisor",
                "Parties",
                "Termination"
            ]
        );
        assert!(set_locale("nl"));
        // C50-p5#6 and FIX50b's ruling 1: the Dutch compounds and words.
        let members = [
            "wire_part",
            "symbol_cp",
            "xref_format",
            "bom_basis",
            "tag_case",
            "user_defined",
            "ours",
            "theirs",
            "git_tag",
            "den",
            "num",
            "potential",
        ];
        assert_eq!(
            members.map(|m| field_label("", m)),
            [
                "Draadartikel",
                "Symboolaansluitpunt",
                "Kruisverwijzingsnotatie",
                "Stuklijstbasis",
                "Kenmerk in hoofdletters",
                "Door de gebruiker gedefinieerd",
                "Onze versie",
                "Hun versie",
                "Git-tag",
                "Rasterdeler",
                "Rasterfactor",
                "Potentiaal"
            ]
        );
        assert_eq!(
            field_label("connection", "overrides.wire_part"),
            "Draadartikel overschreven"
        );
        assert_eq!(field_label("device", "function_text"), "Functietekst");
        assert_eq!(
            field_label("connection", "overrides.colour"),
            "Aderkleur overschreven"
        );
        assert_eq!(field_label("page", "section"), "Sectie");
        assert_eq!(field_label("function", "cps[1]"), "Aansluitpunt 2");
        // A part's technical data is an open set: a key it does not name keeps its humanised words.
        assert_eq!(field_label("part", "tech_data.rated_current"), "Rated current");
        assert!(set_locale("en"));
    }

    /// Every word of the format's enumerated fields has its `choice-<value>` in en-GB and nl-NL, so
    /// the Properties pane's choices and values read in the UI language.
    #[test]
    fn every_enumerated_field_word_has_a_choice_text_in_both_languages() {
        fn words(t: &ew_api::output::FieldType, out: &mut BTreeSet<String>) {
            use ew_api::output::FieldType as F;
            match t {
                F::Enum { options } => out.extend(options.iter().cloned()),
                F::Optional(i) | F::List(i) => words(i, out),
                F::Group(nodes) => nodes.iter().for_each(|n| words(&n.ty, out)),
                _ => {}
            }
        }
        let mut all = BTreeSet::new();
        for (_, _, ty) in catalogue_fields() {
            words(&ty, &mut all);
        }
        assert!(all.contains("l_plus"), "{all:?}");
        let nl = nl_keys();
        let mut missing = BTreeSet::new();
        for w in &all {
            let key = format!("choice-{}", w.replace('_', "-"));
            if !has_key(&key) {
                missing.insert(format!("en-GB: {key}"));
            }
            if !nl.contains(key.as_str()) {
                missing.insert(format!("nl-NL: {key}"));
            }
        }
        assert_eq!(missing, BTreeSet::new());
        assert_eq!(
            ["l_plus", "tn_c_s", "multi_line"].map(choice_text),
            ["L+", "TN-C-S", "Multi-line"]
        );
        assert!(set_locale("nl"));
        assert_eq!(
            ["l_plus", "spare", "multi_line", "strip_segment", "end_clamp"].map(choice_text),
            [
                "L+",
                "Reserve",
                "Meerlijnschema",
                "Klemmenstrooksegment",
                "Eindsteun"
            ]
        );
        // One word, two meanings, each field its own words; the merge's sides.
        let fields = [
            ("function", "role", "main"),
            ("device", "terminal.levels[0].role", "main"),
            ("device", "terminal.levels[1].role", "aux"),
            ("conflict", "side", "theirs"),
            ("conflict", "side", "ours"),
        ];
        assert_eq!(
            fields.map(|(k, p, v)| field_choice_text(k, p, v)),
            [
                "Hoofdfunctie",
                "Hoofdniveau",
                "Hulpniveau",
                "Hun versie",
                "Onze versie"
            ]
        );
        assert_eq!(choice_text("theirs"), "Versie op schijf");
        assert!(set_locale("en"));
        assert_eq!(
            fields.map(|(k, p, v)| field_choice_text(k, p, v)),
            ["Main", "Main level", "Auxiliary level", "Theirs", "Ours"]
        );
        // One English word for the accessory (16a §8 item 4).
        assert_eq!(
            [choice_text("end_clamp"), accessory_text("end_clamp").to_owned()],
            ["End bracket", "End bracket"]
        );
    }

    /// The form dialogs show doc 16a §8's words, never the raw parameter value.
    #[test]
    fn choice_text_gives_the_display_words() {
        assert_eq!(choice_text("multi_line"), "Multi-line");
        assert_eq!(choice_text("table_of_contents"), "Table of contents");
        assert_eq!(choice_text("location"), "Location");
        assert_eq!(choice_text("user_defined"), "User-defined");
        assert_eq!(choice_text("page_based"), "Page-based");
        assert_eq!(choice_text("project"), "Project");
        assert_eq!(choice_text(""), "");
        for value in crate::shell::PAGE_TYPES
            .iter()
            .chain(crate::shell::STRUCTURE_KINDS)
        {
            let text = choice_text(value);
            assert!(!text.contains('_'), "{value}: {text}");
            assert!(text.starts_with(|c: char| c.is_uppercase()), "{value}: {text}");
        }
    }

    /// `elapsed_text`: (name, last change, expected text) against `NOW`.
    #[test]
    fn elapsed_text_table() {
        let cases = [
            ("now", NOW, "just now"),
            ("just under a minute", NOW - 59_999, "just now"),
            ("one minute", NOW - 60_000, "1 minute ago"),
            ("minutes round down", NOW - 12 * 60_000 - 5, "12 minutes ago"),
            ("just under an hour", NOW - 3_599_999, "59 minutes ago"),
            ("one hour", NOW - 3_600_000, "1 hour ago"),
            ("hours round down", NOW - 5 * 3_600_000 - 1, "5 hours ago"),
            ("one day", NOW - 86_400_000, "1 day ago"),
            ("forty days", NOW - 40 * 86_400_000, "40 days ago"),
            ("since the epoch", 0, "20752 days ago"),
            // A last change in the future (clock skew) is never negative.
            ("a millisecond ahead", NOW + 1, "just now"),
            ("a day ahead", NOW + 86_400_000, "just now"),
        ];
        for (name, last, want) in cases {
            assert_eq!(elapsed_text(last, NOW), want, "case {name}");
        }
    }

    #[test]
    fn format_count_is_singular_for_one_and_plural_otherwise() {
        assert_eq!(
            format_count(count_key!("toast-recovered"), 1),
            "Recovered 1 change; save to keep it."
        );
        assert_eq!(
            format_count(count_key!("toast-recovered"), 3),
            "Recovered 3 changes; save to keep them."
        );
        assert_eq!(
            format_count(count_key!("toast-recovered"), 0),
            "Recovered 0 changes; save to keep them."
        );
        assert_eq!(
            format_count(count_key!("marker-recovered"), 1),
            "Recovered 1 change from the journal"
        );
        assert_eq!(
            format_count(count_key!("marker-recovered"), 2),
            "Recovered 2 changes from the journal"
        );
    }

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(format(key!("status-page"), &["103", "3", "36"]), "103 (3 of 36)");
        assert_eq!(
            format(key!("window-title"), &["J-E-2416.ew"]),
            "J-E-2416.ew - ElektraWatt"
        );
    }

    #[test]
    fn field_labels_are_exact_or_humanised() {
        assert_eq!(field_label("device", "function_text"), "Function text");
        assert_eq!(field_label("device", "parts[0].part"), "Part");
        assert_eq!(field_label("device", "parts[1].qty"), "Quantity");
        assert_eq!(
            field_label("connection", "overrides.csa"),
            "Cross-section override"
        );
        assert_eq!(field_label("connection", "derived.csa"), "Cross-section");
        assert_eq!(field_label("device", "tag.counter"), "Tag counter");
        assert_eq!(field_label("project", "settings.checks"), "Checks");
        assert_eq!(field_label("device", "plc_module"), "PLC module");
        assert_eq!(
            field_label("device", "derived.structure.location"),
            "Effective location"
        );
        assert_eq!(field_label("device", "parts[1]"), "Part use 2");
        assert_eq!(field_label("device", "derived.cp[0]"), "Connection point 1");
        assert_eq!(kind_plural("device"), "devices");
        assert_eq!(kind_plural("potential_def"), "potential definitions");
        assert_eq!(kind_plural("structure_box"), "structure boxes");
    }

    #[test]
    fn an_unknown_key_has_no_text() {
        assert_eq!(lookup("no-such-key"), None);
        assert_eq!(text(Key::run_time("no-such-key")), "?");
    }
}
