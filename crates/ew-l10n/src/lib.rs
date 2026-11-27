// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A text for a person by key (architecture §18): a [`Text`] is a key of the text files of the crate that says it and
//! its arguments; the command line and `ew-api` print its English, the window renders the key in its language, an
//! argument that is itself a [`Text`] in its language too.
//!
//! A crate that says something keeps its words in `texts/<language>/<area>.ftl`. Its `build.rs` calls
//! [`build::generate`], which writes the crate's key table from its en-GB files, and its `lib.rs` invokes
//! `ew_l10n::texts!(crate)` ([`texts!`]) before its modules, which gives the crate `text!` (a [`Text`]) and `key!` (a key
//! as it is), checked when the code is built: a key the table lacks, or a wrong number of arguments, fails the build.
//! The table holds each one-line text that takes only positional arguments (`{ $p0 }`, `{ $p1 }`, …) as its English
//! with the arguments written `{0}`, `{1}`, …, which a [`Text`] keeps for [`fmt::Display`].
//!
//! The words in a language are rendered at the edge by the one renderer over Fluent, [`render`].

use std::fmt;

pub mod build;
pub mod render;

/// A text by key, with its arguments: [`fmt::Display`] writes it in English.
#[derive(Clone)]
pub struct Text {
    key: &'static str,
    args: Vec<TextArg>,
    /// The arguments by name (`{ $device }`), in the order they were given: a pattern with named variables.
    named: Vec<(&'static str, TextArg)>,
    /// The English of the key in its crate's table, the arguments written `{0}`, `{1}`, …; `?` for a key the table
    /// does not hold. A key has one entry in one crate, so it follows from the key: equality and `Debug` leave it out.
    english: &'static str,
    /// The catalogue of the crate whose text files hold the key: whichever catalogue a text is printed through, a key
    /// that catalogue lacks is rendered by this one ([`render::Catalogue::text_or_english`]). `None` for a bare value
    /// and for a key of a front end's own files. It follows from the key as `english` does.
    home: Option<Home>,
}

/// The catalogue of the crate that owns a key: its `catalogue()`, which `texts!` gives it.
pub type Home = fn() -> &'static render::Catalogue;

/// An argument of a [`Text`]: a value written as it is (a tag, a name), a name a person wrote in several languages, a
/// number, a text by key, or a list of them.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TextArg {
    /// A value that no language changes.
    Plain(String),
    /// Words by key, in the reader's language.
    Text(Text),
    /// A name a person wrote in several languages (a medium's name): the reader's language exactly, else the neutral
    /// text or the first language.
    Ml(ew_mltext::MlText),
    /// A number, which a pattern may select on or write as an ordinal (`NUMBER($n, type: "ordinal")`).
    Number(i64),
    /// Arguments joined by the joining rule of the reader's language.
    List(List),
}

/// Arguments joined into one: each item in the reader's language, `between` between two items, `last` between the
/// last two (`a, b and c`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct List {
    /// The items, in order.
    pub items: Vec<TextArg>,
    /// The words between two items (`, `).
    pub between: Box<Text>,
    /// The words between the last two items (` and `); `between` where the language has none of its own.
    pub last: Box<Text>,
}

impl List {
    /// `items` joined by `between`, the last two by `last`.
    pub fn new(items: Vec<TextArg>, between: Text, last: Text) -> List {
        List {
            items,
            between: Box::new(between),
            last: Box::new(last),
        }
    }
}

impl Text {
    /// The constructor behind the `text!` macro of a crate's [`texts!`], which checks the key and the number of
    /// arguments and passes the key's English from the crate's table and the crate's catalogue as its `home`; call the
    /// macro instead.
    #[doc(hidden)]
    pub fn checked_by_macro(
        key: &'static str,
        english: &'static str,
        home: Option<Home>,
        args: Vec<TextArg>,
    ) -> Text {
        Text {
            key,
            args,
            named: Vec::new(),
            english,
            home,
        }
    }

    /// A value that stands where a text is and has no words of its own (a list of texts joined, a reader's message
    /// as it is): it reads as the value, in the reader's language. Its key is empty and no text file holds it.
    pub fn bare(value: impl Into<TextArg>) -> Text {
        Text {
            key: "",
            args: vec![value.into()],
            named: Vec::new(),
            english: "{0}",
            home: None,
        }
    }

    /// Whether it is a bare value ([`Text::bare`]).
    pub fn is_bare(&self) -> bool {
        self.key.is_empty()
    }

    /// The same text with the argument `name` (a variable `{ $name }` of the key's pattern) set to `value`; a second
    /// value of one name replaces the first.
    #[must_use]
    pub fn with(mut self, name: &'static str, value: impl Into<TextArg>) -> Text {
        let value = value.into();
        match self.named.iter_mut().find(|(k, _)| *k == name) {
            Some(slot) => slot.1 = value,
            None => self.named.push((name, value)),
        }
        self
    }

    /// The key: a key of the table of the crate that made the text, and of its text files.
    pub fn key(&self) -> &'static str {
        self.key
    }

    /// The arguments, in the order of `{0}`, `{1}`, ….
    pub fn args(&self) -> &[TextArg] {
        &self.args
    }

    /// The arguments by name, in the order they were given.
    pub fn named(&self) -> &[(&'static str, TextArg)] {
        &self.named
    }

    /// The catalogue of the crate that owns its key; `None` for a bare value and a key of a front end's own files.
    pub fn home(&self) -> Option<&'static render::Catalogue> {
        self.home.map(|home| home())
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rest = self.english;
        while let Some(open) = rest.find('{') {
            f.write_str(&rest[..open])?;
            let after = &rest[open + 1..];
            let digits = after.bytes().take_while(u8::is_ascii_digit).count();
            match after[digits..].strip_prefix('}') {
                Some(tail) if digits > 0 => {
                    let i: usize = after[..digits].parse().unwrap_or(usize::MAX);
                    match self.args.get(i) {
                        Some(arg) => write!(f, "{arg}")?,
                        None => f.write_str("?")?,
                    }
                    rest = tail;
                }
                _ => {
                    f.write_str("{")?;
                    rest = after;
                }
            }
        }
        f.write_str(rest)
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Text");
        d.field("key", &self.key).field("args", &self.args);
        if !self.named.is_empty() {
            d.field("named", &self.named);
        }
        d.finish()
    }
}

impl PartialEq for Text {
    fn eq(&self, other: &Text) -> bool {
        self.key == other.key && self.args == other.args && self.named == other.named
    }
}

impl Eq for Text {}

/// A text equals the English it prints (the command line's words, [`fmt::Display`]).
impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        let english = self.to_string();
        english.as_str() == other
    }
}

impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        <Text as PartialEq<str>>::eq(self, other)
    }
}

impl PartialEq<String> for Text {
    fn eq(&self, other: &String) -> bool {
        <Text as PartialEq<str>>::eq(self, other.as_str())
    }
}

impl fmt::Display for TextArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextArg::Plain(s) => f.write_str(s),
            TextArg::Text(t) => write!(f, "{t}"),
            TextArg::Number(n) => write!(f, "{n}"),
            TextArg::Ml(m) => f.write_str(&in_language(m, "en-GB")),
            TextArg::List(list) => {
                let n = list.items.len();
                for (i, item) in list.items.iter().enumerate() {
                    match i {
                        0 => {}
                        i if i + 1 == n => write!(f, "{}", list.last)?,
                        _ => write!(f, "{}", list.between)?,
                    }
                    write!(f, "{item}")?;
                }
                Ok(())
            }
        }
    }
}

impl From<String> for TextArg {
    fn from(s: String) -> TextArg {
        TextArg::Plain(s)
    }
}

impl From<&String> for TextArg {
    fn from(s: &String) -> TextArg {
        TextArg::Plain(s.clone())
    }
}

impl From<&str> for TextArg {
    fn from(s: &str) -> TextArg {
        TextArg::Plain(s.to_owned())
    }
}

impl From<Text> for TextArg {
    fn from(t: Text) -> TextArg {
        TextArg::Text(t)
    }
}

impl From<List> for TextArg {
    fn from(l: List) -> TextArg {
        TextArg::List(l)
    }
}

/// `m` in the language `tag`: its text of exactly that tag, else its neutral text or its first language.
pub(crate) fn in_language(m: &ew_mltext::MlText, tag: &str) -> String {
    tag.parse::<ew_mltext::LangTag>()
        .ok()
        .and_then(|t| m.text_for(&t))
        .or_else(|| m.neutral_or_first())
        .unwrap_or_default()
        .to_owned()
}

/// Whether `key` is in `keys`, a crate's `KEYS` sorted by bytes: the lookup behind a crate's `has_key`, which `key!`
/// runs when the code is built.
#[doc(hidden)]
pub const fn key_in(keys: &[&str], key: &str) -> bool {
    let (mut low, mut high) = (0, keys.len());
    while low < high {
        let mid = low + (high - low) / 2;
        let o = order(keys[mid], key);
        if o == 0 {
            return true;
        }
        if o < 0 {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    false
}

/// How `a` orders against `b` by bytes, in a const context: negative, zero or positive.
const fn order(a: &str, b: &str) -> i32 {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut i = 0;
    while i < a.len() && i < b.len() {
        if a[i] != b[i] {
            return a[i] as i32 - b[i] as i32;
        }
        i += 1;
    }
    a.len() as i32 - b.len() as i32
}

/// The English text of `key` in `table`, a crate's `TEXTS` sorted by key, or `None` for a key the table does not hold:
/// the lookup behind a crate's `text_english`, which `text!` and `key!` run when the code is built.
#[doc(hidden)]
pub const fn english_in(table: &[(&str, &'static str)], key: &str) -> Option<&'static str> {
    let (mut low, mut high) = (0, table.len());
    while low < high {
        let mid = low + (high - low) / 2;
        let o = order(table[mid].0, key);
        if o == 0 {
            return Some(table[mid].1);
        }
        if o < 0 {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    None
}

/// The number of arguments `template` takes: one more than its highest `{n}`.
#[doc(hidden)]
pub const fn arity(template: &str) -> usize {
    let b = template.as_bytes();
    let mut most = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'{' {
            let mut j = i + 1;
            let mut n = 0;
            while j < b.len() && b[j].is_ascii_digit() {
                n = n * 10 + (b[j] - b'0') as usize;
                j += 1;
            }
            if j > i + 1 && j < b.len() && b[j] == b'}' && n + 1 > most {
                most = n + 1;
            }
        }
        i += 1;
    }
    most
}

/// Declares the text folder of the crate that invokes it as `ew_l10n::texts!(crate);`, once, at the top of its `lib.rs`
/// before its modules; its `build.rs` has called [`build::generate`] on the folder. The `crate` it is given names the
/// crate in the macros it makes, so that they find its table from any crate. The crate gets
///
/// - `TEXTS`, its key table, and `text_english(key)`, the English of a key of it; `KEYS`, every key of its en-GB
///   files, and `has_key(key)`; `SOURCES`, its text files, which a front end that shows its texts loads;
/// - `text!("key", args…)`, the [`Text`] of a key of the table with as many arguments (each a `String`, a `&str` or a
///   [`Text`]) as its English takes;
/// - `key!("key")`, a key of its text files as it is: for a crate that hands a key to a front end, and for a pattern
///   whose arguments are named ([`Text::with`]).
///
/// `text!` fails the build on a key the table lacks and on a wrong number of arguments, `key!` on a key the text files
/// lack; the documentation of each crate that declares its folder shows the failure.
#[macro_export]
macro_rules! texts {
    ($c:tt) => {
        $crate::texts!(@ $c $);
    };
    (@ $c:tt $d:tt) => {
        include!(concat!(env!("OUT_DIR"), "/texts.rs"));
        include!(concat!(env!("OUT_DIR"), "/keys.rs"));
        include!(concat!(env!("OUT_DIR"), "/sources.rs"));

        /// The English text of `key`, its arguments written `{0}`, `{1}`, …, or `None` for a key that [`TEXTS`] does
        /// not hold.
        pub const fn text_english(key: &str) -> Option<&'static str> {
            $crate::english_in(TEXTS, key)
        }

        /// Whether `key` is a key of this crate's en-GB text files ([`KEYS`]).
        pub const fn has_key(key: &str) -> bool {
            $crate::key_in(KEYS, key)
        }

        /// This crate's catalogue: its text files ([`SOURCES`]) in every language, built once; a pattern's
        /// `DECIMAL($p0)` writes a measured value with the language's decimal separator ([`$crate::render::decimal`]).
        pub fn catalogue() -> &'static $crate::render::Catalogue {
            static CATALOGUE: ::std::sync::OnceLock<$crate::render::Catalogue> = ::std::sync::OnceLock::new();
            CATALOGUE.get_or_init(|| {
                $crate::render::Catalogue::new(&[SOURCES], &[("DECIMAL", $crate::render::decimal)], false)
            })
        }

        /// The text of the key `$k`, a string literal of this crate's [`TEXTS`], with the arguments that follow it
        /// (each a `String`, a `&str` or a text): a key that the table lacks, or a wrong number of arguments, fails
        /// the build.
        #[macro_export]
        macro_rules! text {
            ($d k:literal $d(, $d a:expr)* $d(,)?) => {{
                let (key, english) = const {
                    let Some(template) = $d $c::text_english($d k) else {
                        panic!(concat!("unknown text key: ", $d k));
                    };
                    assert!(
                        $crate::arity(template) == <[&str]>::len(&[$d(stringify!($d a)),*]),
                        concat!("the wrong number of arguments for ", $d k)
                    );
                    ($d k, template)
                };
                $crate::Text::checked_by_macro(
                    key,
                    english,
                    Some($d $c::catalogue),
                    vec![$d($crate::TextArg::from($d a)),*],
                )
            }};
        }

        /// The text key `$k`, a string literal of this crate's [`KEYS`]: a key that the text files lack fails the
        /// build.
        #[macro_export]
        macro_rules! key {
            ($d k:literal) => {
                const {
                    assert!(
                        $d $c::has_key($d k),
                        concat!("unknown text key: ", $d k)
                    );
                    $d k
                }
            };
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small table as a build script writes one: sorted by key, the arguments written `{0}`, `{1}`, ….
    const TABLE: &[(&str, &str)] = &[
        (
            "start-on-other-page",
            "the wire's start is drawn on {0}, not on {1}; start the wire on {1}",
        ),
        ("words-page", "page {0}"),
    ];

    /// The text of `key` in [`TABLE`] with `args`, built by hand.
    fn text(key: &'static str, args: Vec<TextArg>) -> Text {
        Text::checked_by_macro(key, english_in(TABLE, key).unwrap_or("?"), None, args)
    }

    /// A hand-built text (the macro refuses both at compile time) writes `?` for a missing argument and for
    /// a key that the table does not hold.
    #[test]
    fn a_hand_built_text_writes_a_question_mark_for_what_is_missing() {
        let short = text("start-on-other-page", vec!["page 2".into()]);
        assert_eq!(
            short.to_string(),
            "the wire's start is drawn on page 2, not on ?; start the wire on ?"
        );
        assert_eq!(text("no-such-key", vec![]).to_string(), "?");
        assert_eq!(english_in(TABLE, "no-such-key"), None);
        assert_eq!(arity("a {0} b {2} {1}"), 3);
        assert_eq!(arity("no arguments"), 0);
    }

    /// Equality and `Debug` are those of the key and the arguments, as a derived pair would give them.
    #[test]
    fn a_text_is_its_key_and_its_arguments() {
        let t = text("words-page", vec!["101".into()]);
        assert_eq!(
            t,
            Text::checked_by_macro("words-page", "?", None, vec!["101".into()])
        );
        assert_eq!(
            format!("{t:?}"),
            "Text { key: \"words-page\", args: [Plain(\"101\")] }"
        );
    }
}
