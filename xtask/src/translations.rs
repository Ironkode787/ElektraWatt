// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The translation status as a rule of `cargo xtask rules` (NFR-I18N-07), in two parts:
//!
//! - **The program's texts.** Every key of a crate's English text files (`<crate>/texts/en-GB/*.ftl`) has its entry in
//!   each other language folder of that crate's `texts/` (`nl-NL`): a file comparison, so the rules of any check catch
//!   a key written in English only, whichever crate's tests the check runs. A key is a line's `<id> =` whose id starts
//!   with a letter, as the crates' key tables read them (`ew_l10n::build`); terms (`-…`) and attributes are not keys.
//! - **The guide.** The Dutch guide translates every message of the introduction and of the tutorials (the pages of
//!   `src/tutorials/`) that the template `messages.pot` holds, none of them empty or fuzzy.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};

/// The template and the Dutch translation of the guide.
const TEMPLATE: &str = "docs/book/po/messages.pot";
const DUTCH: &str = "docs/book/po/nl.po";

/// The pages the Dutch guide translates whole: the introduction and the tutorials.
const WHOLE: &[&str] = &["src/introduction.md", "src/tutorials/"];

/// The language every key is written in first.
const ENGLISH: &str = "en-GB";

/// The violations: each key of a crate's English texts that another language folder of the crate lacks, then each
/// message of a page of [`WHOLE`] that `nl.po` leaves untranslated.
pub(crate) fn check(root: &Path, files: &[(String, Vec<u8>)]) -> Result<Vec<String>> {
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).with_context(|| format!("reading {rel}"));
    let template = crate::book_po::parse(&read(TEMPLATE)?)?;
    let dutch = crate::book_po::parse(&read(DUTCH)?)?;
    let mut out = missing_keys(files);
    out.extend(untranslated(&template, &dutch));
    Ok(out)
}

/// The keys of the English text files among `files` (path and bytes) that a language folder beside them lacks, each
/// named with the folder and the English file that holds it.
fn missing_keys(files: &[(String, Vec<u8>)]) -> Vec<String> {
    // Per text folder (`crates/x/texts`), per language: each key with the file that holds it.
    let mut folders: BTreeMap<&str, BTreeMap<&str, BTreeMap<String, &str>>> = BTreeMap::new();
    for (path, bytes) in files {
        let Some((folder, rest)) = path.split_once("/texts/") else {
            continue;
        };
        let Some((language, file)) = rest.split_once('/') else {
            continue;
        };
        if !file.ends_with(".ftl") || file.contains('/') {
            continue;
        }
        let keys = folders.entry(folder).or_default().entry(language).or_default();
        for key in String::from_utf8_lossy(bytes).lines().filter_map(key_of) {
            keys.insert(key.to_owned(), file);
        }
    }
    let mut out = Vec::new();
    for (folder, languages) in &folders {
        let Some(english) = languages.get(ENGLISH) else {
            continue;
        };
        for (language, keys) in languages.iter().filter(|(l, _)| **l != ENGLISH) {
            let have: BTreeSet<&String> = keys.keys().collect();
            for (key, file) in english.iter().filter(|(k, _)| !have.contains(k)) {
                out.push(format!(
                    "{folder}/texts/{language}: `{key}` ({ENGLISH}/{file}) has no entry; write it in the same area's \
                     file"
                ));
            }
        }
    }
    out
}

/// The key a line of a text file declares: its `<id> =`, the id starting with a letter.
fn key_of(line: &str) -> Option<&str> {
    let (id, _) = line.split_once(" =")?;
    let mut chars = id.chars();
    (chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    .then_some(id)
}

/// The messages of `template` on a page of [`WHOLE`] that `dutch` has no translation of, each named.
fn untranslated(template: &[crate::book_po::Entry], dutch: &[crate::book_po::Entry]) -> Vec<String> {
    // A reference reads `src\tutorials/...` when the template was made on Windows.
    let whole = |refs: &[String]| {
        refs.iter()
            .any(|r| WHOLE.iter().any(|w| r.replace('\\', "/").starts_with(w)))
    };
    let done: BTreeSet<&str> = dutch
        .iter()
        .filter(|e| !e.obsolete && !e.msgstr.is_empty() && !e.fuzzy())
        .map(|e| e.msgid.as_str())
        .collect();
    template
        .iter()
        .filter(|e| !e.msgid.is_empty() && whole(&e.refs) && !done.contains(e.msgid.as_str()))
        .map(|e| {
            format!(
                "{DUTCH}: \"{}\" ({}) is not translated: the introduction and the tutorials are translated whole",
                e.msgid.lines().next().unwrap_or_default(),
                e.refs.first().map_or("", String::as_str)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{missing_keys, untranslated};
    use crate::book_po::parse;

    /// A key of a crate's English texts that its Dutch folder lacks is named with the folder and the English file; a
    /// key the Dutch has in another file of the folder, a term, an attribute, a comment, a Dutch-only key and a
    /// folder without English are not.
    #[test]
    fn an_english_only_key_is_named() {
        let file = |path: &str, text: &str| (path.to_owned(), text.as_bytes().to_vec());
        let files = vec![
            file(
                "crates/a/texts/en-GB/menu.ftl",
                "# A comment = no key\nmenu-file = File\nmenu-edit = Edit\n-term = EW\nlong =\n    .tip = A tip\n",
            ),
            file("crates/a/texts/en-GB/tools.ftl", "tool-wire = Wire\n"),
            file("crates/a/texts/nl-NL/menu.ftl", "menu-file = Bestand\nonly-dutch = Alleen\n"),
            file("crates/a/texts/nl-NL/other.ftl", "long =\n    .tip = Een tip\n"),
            file("crates/b/texts/nl-NL/x.ftl", "x = X\n"),
            file("crates/a/src/texts/en-GB/z.rs", "z = 1\n"),
        ];
        let named = |key: &str, english: &str| {
            format!(
                "crates/a/texts/nl-NL: `{key}` (en-GB/{english}) has no entry; write it in the same area's file"
            )
        };
        assert_eq!(
            missing_keys(&files),
            [named("menu-edit", "menu.ftl"), named("tool-wire", "tools.ftl")]
        );
    }

    /// A tutorial's message without a translation or with a fuzzy one is named; one of another page is not.
    #[test]
    fn an_untranslated_tutorial_message_is_named() {
        let template = parse(
            "msgid \"\"\nmsgstr \"\"\n\n#: src/tutorials/one.md:1\nmsgid \"Place\"\nmsgstr \"\"\n\n\
             #: src\\tutorials/one.md:3\nmsgid \"Wire\"\nmsgstr \"\"\n\n#: src/how-to/x.md:1\nmsgid \"Other\"\n\
             msgstr \"\"\n\n#: src/introduction.md:1\nmsgid \"Hello\"\nmsgstr \"\"\n",
        )
        .unwrap();
        let dutch = parse(
            "msgid \"\"\nmsgstr \"\"\n\n#: src/tutorials/one.md:1\nmsgid \"Place\"\nmsgstr \"Plaats\"\n\n\
             #, fuzzy\nmsgid \"Wire\"\nmsgstr \"Draad\"\n\nmsgid \"Hello\"\nmsgstr \"\"\n",
        )
        .unwrap();
        assert_eq!(
            untranslated(&template, &dutch),
            [
                "docs/book/po/nl.po: \"Wire\" (src\\tutorials/one.md:3) is not translated: the introduction and the \
                 tutorials are translated whole",
                "docs/book/po/nl.po: \"Hello\" (src/introduction.md:1) is not translated: the introduction and the \
                 tutorials are translated whole",
            ]
        );
    }
}
