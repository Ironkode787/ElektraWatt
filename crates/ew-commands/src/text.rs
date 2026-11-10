// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A command's refusal by key: the key of the window's text files and its arguments. The command line and `ew-api`
//! print its English; the window renders the key in its language, an argument that is itself a [`Text`] (a record in
//! words, [`crate::words`]) in its language too.
//!
//! A text stands in one place, the window's locale files: its English in
//! `crates/ew-viewmodel/locales/en-GB/shell.ftl`, its Dutch under the same key in the nl-NL file. `build.rs` reads the
//! English into [`TEXTS`], the arguments written `{0}`, `{1}`, …. Code names a key with [`text!`], which checks at
//! compile time that the key is in [`TEXTS`] and that it is given as many arguments as its text takes.

use std::fmt;

/// A text by key, with its arguments: [`fmt::Display`] writes it in English.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Text {
    key: &'static str,
    args: Vec<TextArg>,
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
    /// The constructor behind the crate's `text!` macro, which checks the key and the number of arguments; call the
    /// macro instead.
    #[doc(hidden)]
    pub fn checked_by_macro(key: &'static str, args: Vec<TextArg>) -> Text {
        Text { key, args }
    }

    /// The key: a key of [`TEXTS`] and of the window's text files.
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
        let mut rest = english(self.key).unwrap_or("?");
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

/// The English text of `key`, or `None` for a key that [`TEXTS`] does not hold. [`TEXTS`] is sorted by key.
pub const fn english(key: &str) -> Option<&'static str> {
    let (mut low, mut high) = (0, TEXTS.len());
    while low < high {
        let mid = low + (high - low) / 2;
        let o = order(TEXTS[mid].0, key);
        if o == 0 {
            return Some(TEXTS[mid].1);
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

/// The [`Text`] of the key `$k`, a string literal, with the arguments that follow it (each a `String`, a `&str` or a
/// [`Text`]): a key that [`TEXTS`] lacks, or a wrong number of arguments, fails the build.
macro_rules! text {
    ($k:literal $(, $a:expr)* $(,)?) => {
        $crate::text::Text::checked_by_macro(
            const {
                let Some(template) = $crate::text::english($k) else {
                    panic!(concat!("unknown text key: ", $k));
                };
                assert!(
                    $crate::text::arity(template) == <[&str]>::len(&[$(stringify!($a)),*]),
                    concat!("the wrong number of arguments for ", $k)
                );
                $k
            },
            vec![$($crate::text::TextArg::from($a)),*],
        )
    };
}
pub(crate) use text;

/// The text key `$k`, a string literal, for a crate that hands a key to a front end: a key that [`TEXTS`] lacks fails
/// the build.
#[macro_export]
macro_rules! text_key {
    ($k:literal) => {
        const {
            assert!(
                $crate::text_english($k).is_some(),
                concat!("unknown text key: ", $k)
            );
            $k
        }
    };
}

// The table `TEXTS`, which `build.rs` writes from the window's en-GB locale file.
include!(concat!(env!("OUT_DIR"), "/texts.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    /// A text writes its arguments in place, a text argument in English; the table is sorted, so each key is found.
    #[test]
    fn a_text_writes_its_arguments_in_english() {
        let t = text!("refusal-words-page", "101");
        assert_eq!(t.to_string(), "page 101");
        let t = text!(
            "refusal-connection-start-on-other-page",
            text!("refusal-words-page", "2"),
            text!("refusal-words-page", "3")
        );
        assert_eq!(
            t.to_string(),
            "the wire's start is drawn on page 2, not on page 3; start the wire on page 3"
        );
        assert!(TEXTS.windows(2).all(|w| w[0].0 < w[1].0), "sorted, no key twice");
        assert!(TEXTS.iter().all(|(k, t)| english(k) == Some(*t)));
        // A literal brace of the locale file is a brace here.
        assert_eq!(
            english("refusal-fields-master-cp"),
            Some("a master is a CP: {\"cp\": [function, index]}")
        );
        assert_eq!(arity("a {0} b {2} {1}"), 3);
        assert_eq!(arity("no arguments"), 0);
    }

    /// A hand-built text (the macro refuses both at compile time) writes `?` for a missing argument and for
    /// a key that the table does not hold.
    #[test]
    fn a_hand_built_text_writes_a_question_mark_for_what_is_missing() {
        let short = Text::checked_by_macro("refusal-connection-start-on-other-page", vec!["page 2".into()]);
        assert_eq!(
            short.to_string(),
            "the wire's start is drawn on page 2, not on ?; start the wire on ?"
        );
        assert_eq!(Text::checked_by_macro("no-such-key", vec![]).to_string(), "?");
    }

    /// The `.rs` files under `dir`, recursively.
    fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// No refusal of the crate goes without its key. `tx.reject` and `CmdError::rejected` take a [`Text`]; besides
    /// them, no source builds a `CmdError::Rejected` itself (with `source:`) or one without a text (`text: None`).
    /// `CmdError::invalid_params` and the helpers around it take a [`Text`] too, so no source builds a
    /// `CmdError::InvalidParams` itself (with `message:`), and none builds a [`Text`] but through `text!`.
    #[test]
    fn every_refusal_of_the_crate_is_keyed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        sources(&root, &mut files);
        files.sort();
        let mut unkeyed = Vec::new();
        for file in files {
            let name = file
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&file).unwrap();
            if name == "text.rs" {
                continue;
            }
            if name == "error.rs" {
                continue;
            }
            for (at, _) in text.match_indices("Rejected {") {
                let fields = &text[at..at + text[at..].find('}').unwrap_or(text.len() - at)];
                if fields.contains("source:") || fields.contains("text: None") {
                    let line = text[..at].lines().count();
                    unkeyed.push(format!("{name}:{line}"));
                }
            }
            for (at, _) in text.match_indices("InvalidParams {") {
                let fields = &text[at..at + text[at..].find('}').unwrap_or(text.len() - at)];
                if fields.contains("message:") {
                    let line = text[..at].lines().count();
                    unkeyed.push(format!("{name}:{line}"));
                }
            }
            for (i, line) in text.lines().enumerate() {
                let line = line.trim_start();
                if !line.starts_with("//")
                    && (line.contains(".reject(format!")
                        || line.contains(".reject(\"")
                        || line.contains("checked_by_macro"))
                {
                    unkeyed.push(format!("{name}:{}", i + 1));
                }
            }
        }
        assert_eq!(unkeyed, Vec::<String>::new());
    }
}
