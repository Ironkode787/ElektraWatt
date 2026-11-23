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
//! The words in a language are rendered at the edge by the one renderer over Fluent, [`fluent`].

use std::fmt;

pub mod build;
pub mod fluent;

/// A text by key, with its arguments: [`fmt::Display`] writes it in English.
#[derive(Clone)]
pub struct Text {
    key: &'static str,
    args: Vec<TextArg>,
    /// The English of the key in its crate's table, the arguments written `{0}`, `{1}`, …; `?` for a key the table
    /// does not hold. A key has one entry in one crate, so it follows from the key: equality and `Debug` leave it out.
    english: &'static str,
}

/// An argument of a [`Text`]: a value written as it is (a tag, a number, a name), or a text by key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TextArg {
    /// A value that no language changes.
    Plain(String),
    /// Words by key, in the reader's language.
    Text(Text),
}

impl Text {
    /// The constructor behind the `text!` macro of a crate's [`texts!`], which checks the key and the number of
    /// arguments and passes the key's English from the crate's table; call the macro instead.
    #[doc(hidden)]
    pub fn checked_by_macro(key: &'static str, english: &'static str, args: Vec<TextArg>) -> Text {
        Text { key, args, english }
    }

    /// The key: a key of the table of the crate that made the text, and of its text files.
    pub fn key(&self) -> &'static str {
        self.key
    }

    /// The arguments, in the order of `{0}`, `{1}`, ….
    pub fn args(&self) -> &[TextArg] {
        &self.args
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
        f.debug_struct("Text")
            .field("key", &self.key)
            .field("args", &self.args)
            .finish()
    }
}

impl PartialEq for Text {
    fn eq(&self, other: &Text) -> bool {
        self.key == other.key && self.args == other.args
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
/// - `TEXTS`, its key table, and `text_english(key)`, the English of a key of it;
/// - `text!("key", args…)`, the [`Text`] of a key of the table with as many arguments (each a `String`, a `&str` or a
///   [`Text`]) as its English takes;
/// - `key!("key")`, a key of the table as it is, for a crate that hands a key to a front end.
///
/// Both macros fail the build on a key the table lacks, and `text!` on a wrong number of arguments; the documentation
/// of each crate that declares its folder shows the failure.
#[macro_export]
macro_rules! texts {
    ($c:tt) => {
        $crate::texts!(@ $c $);
    };
    (@ $c:tt $d:tt) => {
        include!(concat!(env!("OUT_DIR"), "/texts.rs"));

        /// The English text of `key`, its arguments written `{0}`, `{1}`, …, or `None` for a key that [`TEXTS`] does
        /// not hold.
        pub const fn text_english(key: &str) -> Option<&'static str> {
            $crate::english_in(TEXTS, key)
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
                $crate::Text::checked_by_macro(key, english, vec![$d($crate::TextArg::from($d a)),*])
            }};
        }

        /// The text key `$k`, a string literal of this crate's [`TEXTS`], for a crate that hands a key to a front
        /// end: a key that the table lacks fails the build.
        #[macro_export]
        macro_rules! key {
            ($d k:literal) => {
                const {
                    assert!(
                        $d $c::text_english($d k).is_some(),
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
        Text::checked_by_macro(key, english_in(TABLE, key).unwrap_or("?"), args)
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
        assert_eq!(t, Text::checked_by_macro("words-page", "?", vec!["101".into()]));
        assert_eq!(
            format!("{t:?}"),
            "Text { key: \"words-page\", args: [Plain(\"101\")] }"
        );
    }
}
