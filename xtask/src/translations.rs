// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The translation status as a rule of `cargo xtask rules` (NFR-I18N-07): the Dutch guide translates every message of
//! the introduction and of the tutorials (the pages of `src/tutorials/`) that the template `messages.pot` holds, none
//! of them empty or fuzzy. The window's Dutch texts need no rule here: `ew-viewmodel`'s test
//! `every_shipped_locale_has_every_key` asks every key of every shipped locale.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result};

/// The template and the Dutch translation of the guide.
const TEMPLATE: &str = "docs/book/po/messages.pot";
const DUTCH: &str = "docs/book/po/nl.po";

/// The pages the Dutch guide translates whole: the introduction and the tutorials.
const WHOLE: &[&str] = &["src/introduction.md", "src/tutorials/"];

/// The violations: each message of a page of [`WHOLE`] that `nl.po` leaves untranslated, by its first line.
pub(crate) fn check(root: &Path) -> Result<Vec<String>> {
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).with_context(|| format!("reading {rel}"));
    let template = crate::book_po::parse(&read(TEMPLATE)?)?;
    let dutch = crate::book_po::parse(&read(DUTCH)?)?;
    Ok(untranslated(&template, &dutch))
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
    use super::untranslated;
    use crate::book_po::parse;

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
