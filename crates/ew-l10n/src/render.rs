// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one renderer over Fluent (architecture §18 point 4): a language's Fluent sources as one [`Language`], and a
//! message or one of its attributes formatted with its arguments. The window (`ew-viewmodel`'s `strings`) and the rule
//! messages (`ew-checks`' `messages`) build their languages here and keep their own fallback (a language, then en-GB,
//! then what each caller writes for a text that has none). Where the two rendered differently the difference is a
//! parameter of this module: the functions a pattern may call and whether Fluent's `NUMBER` is one of them
//! ([`Language::new`]), and whether a pattern that resolves with an error is still taken ([`OnError`]). Neither inserts
//! Unicode isolation marks around an argument: the arguments are tags and names in a project's own text, and the
//! marks would change the bytes a text prints. The pseudo-locale of the window's tests ([`pseudo`]) and the decimal
//! comma of a pattern's `DECIMAL` ([`decimal`]) are here too.

use std::borrow::Cow;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use fluent_syntax::ast::Entry;

use crate::{Text, TextArg};

/// A function a pattern may call, by its name: `NAME($x)` gives `f(tag, x)` for a string `x`, with `tag` the tag of the
/// language that formats it; another value passes unchanged, and no argument gives none.
pub type Function = (&'static str, fn(&str, &str) -> String);

/// An argument of a pattern.
#[derive(Clone, Copy, Debug)]
pub enum Value<'a> {
    /// A string.
    Text(&'a str),
    /// A number written as text, for `NUMBER($x, type: "ordinal")`; a text that is no number stays a string.
    Number(&'a str),
}

/// What [`Language::format`] does with a pattern that resolves with an error (an argument it lacks, a function it does
/// not know).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnError {
    /// The text Fluent writes, a missing argument as `{$name}`: the window.
    Keep,
    /// No text, so that the caller falls back: the rule messages.
    Miss,
}

/// One language's texts: its Fluent sources as one bundle.
pub struct Language {
    bundle: FluentBundle<FluentResource>,
    keys: Vec<String>,
    tag: String,
}

impl Language {
    /// The language `tag` (a BCP 47 tag; one that does not parse is the root language, which only plural rules
    /// read) from `sources`, in order: a source that does not parse still gives its entries that parse ([`parse_errors`]
    /// names what does not), and a key twice keeps the first. A pattern may call `functions`, and Fluent's `NUMBER` too
    /// when `builtins`.
    pub fn new<'s>(
        tag: &str,
        sources: impl IntoIterator<Item = &'s str>,
        functions: &[Function],
        builtins: bool,
    ) -> Language {
        let id: unic_langid::LanguageIdentifier = tag.parse().unwrap_or_default();
        let mut bundle = FluentBundle::new_concurrent(vec![id]);
        bundle.set_use_isolating(false);
        if builtins {
            let _ = bundle.add_builtins();
        }
        for (name, f) in functions {
            let (tag, f) = (tag.to_owned(), *f);
            let _ = bundle.add_function(name, move |positional, _| match positional.first() {
                Some(FluentValue::String(s)) => FluentValue::from(f(&tag, s)),
                Some(other) => other.clone(),
                None => FluentValue::None,
            });
        }
        let mut keys = Vec::new();
        for source in sources {
            let resource = FluentResource::try_new(source.to_owned()).unwrap_or_else(|(r, _)| r);
            keys.extend(resource.entries().filter_map(|e| match e {
                Entry::Message(m) => Some(m.id.name.to_owned()),
                _ => None,
            }));
            let _ = bundle.add_resource(resource);
        }
        Language {
            bundle,
            keys,
            tag: tag.to_owned(),
        }
    }

    /// The key of every message of its sources, in their order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.keys.iter().map(String::as_str)
    }

    /// Whether it has the message `key`.
    pub fn has_message(&self, key: &str) -> bool {
        self.bundle.has_message(key)
    }

    /// Whether the message `key` has a value (`attribute` `None`) or the attribute `attribute`.
    pub fn has(&self, key: &str, attribute: Option<&str>) -> bool {
        let Some(message) = self.bundle.get_message(key) else {
            return false;
        };
        match attribute {
            None => message.value().is_some(),
            Some(name) => message.get_attribute(name).is_some(),
        }
    }

    /// The value of the message `key` (`attribute` `None`) or its attribute `attribute`, formatted with `args`; `None`
    /// when it has none, or by `on_error` when the pattern resolves with an error.
    pub fn format(
        &self,
        key: &str,
        attribute: Option<&str>,
        args: &[(&str, Value<'_>)],
        on_error: OnError,
    ) -> Option<String> {
        let message = self.bundle.get_message(key)?;
        let pattern = match attribute {
            None => message.value()?,
            Some(name) => message.get_attribute(name)?.value(),
        };
        let mut fluent_args = FluentArgs::with_capacity(args.len());
        for (name, value) in args {
            match value {
                Value::Text(s) => fluent_args.set(*name, *s),
                Value::Number(s) => fluent_args.set(*name, FluentValue::try_number(s)),
            }
        }
        let mut errors = Vec::new();
        let text = self
            .bundle
            .format_pattern(pattern, Some(&fluent_args), &mut errors);
        (on_error == OnError::Keep || errors.is_empty()).then(|| text.into_owned())
    }

    /// `text` formatted: the value of its key with its arguments, the positional ones as `$p0`, `$p1`, … and the named
    /// ones by their names; an argument that is a text, or a list of them, is formatted in this language first. `None`
    /// when the key has no value here, or by `on_error` when a pattern resolves with an error (with [`OnError::Keep`] an
    /// argument that is a text without a value here is written as its English).
    pub fn text(&self, text: &Text, on_error: OnError) -> Option<String> {
        let mut owned: Vec<(Cow<'_, str>, Formatted)> = Vec::new();
        for (i, arg) in text.args().iter().enumerate() {
            owned.push((Cow::Owned(format!("p{i}")), self.arg(arg, on_error)?));
        }
        for (name, arg) in text.named() {
            owned.push((Cow::Borrowed(*name), self.arg(arg, on_error)?));
        }
        let args: Vec<(&str, Value<'_>)> = owned
            .iter()
            .map(|(name, value)| match value {
                Formatted::Words(s) => (name.as_ref(), Value::Text(s)),
                Formatted::Number(s) => (name.as_ref(), Value::Number(s)),
            })
            .collect();
        self.format(text.key(), None, &args, on_error)
    }

    /// An argument of a text as its pattern reads it.
    fn arg(&self, arg: &TextArg, on_error: OnError) -> Option<Formatted> {
        Some(match arg {
            TextArg::Number(n) => Formatted::Number(n.to_string()),
            other => Formatted::Words(self.words(other, on_error)?),
        })
    }

    /// An argument in words: a value as it is, a number in digits, a text and each item of a list in this language.
    fn words(&self, arg: &TextArg, on_error: OnError) -> Option<String> {
        let text = |t: &Text| match (self.text(t, on_error), on_error) {
            (Some(s), _) => Some(s),
            (None, OnError::Keep) => Some(t.to_string()),
            (None, OnError::Miss) => None,
        };
        match arg {
            TextArg::Plain(s) => Some(s.clone()),
            TextArg::Number(n) => Some(n.to_string()),
            TextArg::Ml(m) => Some(crate::in_language(m, &self.tag)),
            TextArg::Text(t) => text(t),
            TextArg::List(list) => {
                let n = list.items.len();
                let mut out = String::new();
                for (i, item) in list.items.iter().enumerate() {
                    match i {
                        0 => {}
                        i if i + 1 == n => out.push_str(&text(&list.last)?),
                        _ => out.push_str(&text(&list.between)?),
                    }
                    out.push_str(&self.words(item, on_error)?);
                }
                Some(out)
            }
        }
    }
}

/// An argument of a text, formatted for its pattern.
enum Formatted {
    /// Words, a string to the pattern.
    Words(String),
    /// A number written as text, a number to the pattern.
    Number(String),
}

/// The parse errors of the Fluent source `source` (empty when it parses): for the tests.
pub fn parse_errors(source: &str) -> Vec<String> {
    match FluentResource::try_new(source.to_owned()) {
        Ok(_) => Vec::new(),
        Err((_, errors)) => errors.iter().map(|e| format!("{e:?}")).collect(),
    }
}

/// The languages that write a decimal comma (`1,5 mm²`), by tag.
const DECIMAL_COMMA: &[&str] = &["nl-NL"];

/// `text` with each decimal point between two digits written as the language `tag` writes it: the function `DECIMAL` of
/// a rule's pattern, which wraps an argument that holds a measured value (`1.5 mm²` is `1,5 mm²` in nl-NL).
pub fn decimal(tag: &str, text: &str) -> String {
    if !DECIMAL_COMMA.iter().any(|l| l.eq_ignore_ascii_case(tag)) {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    chars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let digit = |j: Option<usize>| j.and_then(|j| chars.get(j)).is_some_and(char::is_ascii_digit);
            if *c == '.' && digit(i.checked_sub(1)) && digit(Some(i + 1)) {
                ','
            } else {
                *c
            }
        })
        .collect()
}

/// `text` in the pseudo-locale of the window's tests: every letter accented, padded by 35 % of its length (rounded up)
/// and bracketed with `⟦ ⟧`; the arguments `{0}` … `{9}` stay as they are.
pub fn pseudo(text: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::List;

    /// A text with named arguments, a number, a nested text and a list renders each in the language: the list by its
    /// joining rule, the number for an ordinal; a nested key the language lacks fails with `Miss` and is written in
    /// English with `Keep`.
    #[test]
    fn a_text_renders_its_arguments_in_the_language() {
        let nl = Language::new(
            "nl-NL",
            [
                "msg = { $who } heeft { $items } op de { NUMBER($nth, type: \"ordinal\") ->\n    [one] {$nth}ste\n   *[other] {$nth}e\n} plaats\nword = apparaat\ncomma = {\", \"}\nand = {\" en \"}\n",
            ],
            &[],
            true,
        );
        let word = |key| Text::checked_by_macro(key, "device", Vec::new());
        let sep = |key, english| Text::checked_by_macro(key, english, Vec::new());
        let items = List::new(
            vec!["-K1".into(), word("word").into(), "-K3".into()],
            sep("comma", ", "),
            sep("and", " and "),
        );
        let text = Text::checked_by_macro("msg", "?", Vec::new())
            .with("who", word("word"))
            .with("items", items.clone())
            .with("nth", TextArg::Number(3));
        assert_eq!(
            nl.text(&text, OnError::Miss).as_deref(),
            Some("apparaat heeft -K1, apparaat en -K3 op de 3e plaats")
        );
        assert_eq!(TextArg::List(items).to_string(), "-K1, device and -K3");
        let missing = text.with("who", word("no-such-word"));
        assert_eq!(nl.text(&missing, OnError::Miss), None);
        assert!(
            nl.text(&missing, OnError::Keep)
                .is_some_and(|s| s.starts_with("device heeft"))
        );
    }
}
