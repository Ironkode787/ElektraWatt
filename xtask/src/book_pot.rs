// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FT-1481 (ID-1409): the book's translation template `docs/book/po/messages.pot` is current. `ci/build-book.sh --pot`
//! makes it with mdbook-xgettext, which this offline test does not run; it compares instead the letters and digits of
//! each hand-written page (the pages `ci/build-book.sh` reads: every page under `docs/book/src/` but the generated
//! ones of the root `.gitignore`, and `SUMMARY.md` without their lines) with those of the template's messages that
//! point into the page, in the order of their lines. A paragraph that changed, came or went changes them; a change of
//! punctuation or of white space alone does not, and is not a message the translators miss. A list's markers are not
//! part of a message.

use std::collections::BTreeMap;
use std::path::Path;

/// The template's messages by page: each message's line and text, in the template's order.
type Messages = BTreeMap<String, Vec<(usize, String)>>;

/// A PO string's text: the quoted parts of `lines` joined, with `\"`, `\\`, `\n` and `\t` read.
fn po_text(lines: &[&str]) -> String {
    let mut out = String::new();
    for l in lines {
        let inner = l.trim().trim_start_matches('"').trim_end_matches('"');
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            }
        }
    }
    out
}

/// The template's messages by the page each reference names (`#: src/x.md:3 src/y.md:7`), the header left out.
fn messages(pot: &str) -> Messages {
    let mut out = Messages::new();
    for entry in pot.replace("\r\n", "\n").split("\n\n") {
        let lines: Vec<&str> = entry.lines().collect();
        let refs: Vec<(String, usize)> = lines
            .iter()
            .filter_map(|l| l.strip_prefix("#: "))
            .flat_map(str::split_whitespace)
            .filter_map(|r| {
                let (page, line) = r.rsplit_once(':')?;
                Some((page.to_owned(), line.parse().ok()?))
            })
            .collect();
        let Some(start) = lines.iter().position(|l| l.starts_with("msgid ")) else {
            continue;
        };
        let end = lines.iter().position(|l| l.starts_with("msgstr")).unwrap_or(lines.len());
        let mut id = vec![&lines[start]["msgid ".len()..]];
        id.extend_from_slice(&lines[start + 1..end]);
        let text = po_text(&id);
        for (page, line) in refs {
            out.entry(page).or_default().push((line, text.clone()));
        }
    }
    for list in out.values_mut() {
        list.sort_by_key(|(line, _)| *line);
    }
    out
}

/// The letters and digits of `text`, in order.
fn signature(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// A page's text as its messages hold it: without its list markers (`- `, `* `, `1. `), without the fenced code blocks
/// that hold no string literal (mdbook-xgettext extracts a code block, whole, only for its string literals; a `"` is
/// taken for one), and for the table of contents `SUMMARY.md` without the chapters' paths.
fn without_markers(rel: &str, page: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut block: Option<Vec<&str>> = None;
    for l in page.lines() {
        let fence = l.trim_start().starts_with("```");
        match (&mut block, fence) {
            (None, true) => block = Some(vec![l]),
            (None, false) => kept.push(l),
            (Some(lines), false) => lines.push(l),
            (Some(lines), true) => {
                lines.push(l);
                if lines.iter().any(|l| l.contains('"')) {
                    kept.append(lines);
                }
                block = None;
            }
        }
    }
    kept.into_iter()
        .map(|l| {
            if rel != "src/SUMMARY.md" {
                return l.to_owned();
            }
            let mut out = String::new();
            let mut rest = l;
            while let Some(at) = rest.find("](") {
                out.push_str(&rest[..at]);
                rest = rest[at..].split_once(')').map_or("", |(_, r)| r);
            }
            out.push_str(rest);
            out
        })
        .map(|l| {
            let t = l.trim_start();
            let rest = t
                .strip_prefix("- ")
                .or_else(|| t.strip_prefix("* "))
                .or_else(|| {
                    let digits = t.len() - t.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                    (digits > 0).then(|| t[digits..].strip_prefix(". ")).flatten()
                });
            rest.unwrap_or(t).to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The pages of the book the template is made from: path under `docs/book/` (`src/introduction.md`) and text, the
/// generated pages of `gitignore` left out and their lines dropped from `SUMMARY.md`.
fn pages(book: &Path, gitignore: &str) -> Vec<(String, String)> {
    let generated: Vec<String> = gitignore
        .lines()
        .filter_map(|l| l.trim().strip_prefix("/docs/book/"))
        .filter(|p| p.starts_with("src/reference/"))
        .map(str::to_owned)
        .collect();
    let mut out = Vec::new();
    let mut stack = vec![book.join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("the book's folder reads") {
            let path = entry.expect("an entry").path();
            let rel = path
                .strip_prefix(book)
                .expect("under the book")
                .to_string_lossy()
                .replace('\\', "/");
            if generated.iter().any(|g| rel == g.trim_end_matches('/') || rel.starts_with(g.as_str())) {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if rel.ends_with(".md") {
                let mut text = std::fs::read_to_string(&path).expect("a page reads").replace("\r\n", "\n");
                if rel == "src/SUMMARY.md" {
                    text = text
                        .lines()
                        .filter(|l| {
                            !generated
                                .iter()
                                .any(|g| l.contains(&format!("]({}", &g["src/".len()..])))
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                }
                out.push((rel, text));
            }
        }
    }
    out.sort();
    out
}

/// The pages whose words the template does not hold as they stand, each named with what to run.
fn stale(pot: &str, pages: &[(String, String)]) -> Vec<String> {
    let messages = messages(pot);
    let mut out = Vec::new();
    for (rel, text) in pages {
        let page = signature(&without_markers(rel, text));
        let held: String = messages
            .get(rel)
            .map(|m| m.iter().map(|(_, t)| signature(t)).collect())
            .unwrap_or_default();
        if page != held {
            out.push(format!(
                "docs/book/po/messages.pot is stale for docs/book/{rel}: its messages do not hold the page's words as \
                 they stand; run `bash ci/build-book.sh --pot`"
            ));
        }
    }
    out
}

/// FT-1481: the committed template holds the words of every hand-written page as they stand.
#[test]
fn the_books_translation_template_is_current() {
    let root = crate::workspace_root();
    let pot = std::fs::read_to_string(root.join("docs/book/po/messages.pot")).expect("the template reads");
    let gitignore = std::fs::read_to_string(root.join(".gitignore")).expect(".gitignore reads");
    let found = stale(&pot, &pages(&root.join("docs/book"), &gitignore));
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// A changed, an added and a removed paragraph each make the template stale; punctuation alone does not.
#[test]
fn a_changed_paragraph_makes_the_template_stale() {
    let pot = "msgid \"\"\nmsgstr \"\"\n\"Language: en\\n\"\n\n#: src/a.md:1\nmsgid \"Title\"\nmsgstr \"\"\n\n\
               #: src/a.md:3\nmsgid \"\"\n\"One \\\"quoted\\\" \"\n\"paragraph.\"\nmsgstr \"\"\n\n\
               #: src/a.md:6\nmsgid \"Step one\"\nmsgstr \"\"\n";
    let page = |text: &str| vec![("src/a.md".to_owned(), text.to_owned())];
    let current = "# Title\n\nOne \"quoted\"\nparagraph.\n\n1. Step one\n";
    assert_eq!(stale(pot, &page(current)), Vec::<String>::new());
    assert_eq!(
        stale(pot, &page("# Title\n\nOne quoted paragraph!\n\n1. Step one\n")),
        Vec::<String>::new()
    );
    let message = "docs/book/po/messages.pot is stale for docs/book/src/a.md: its messages do not hold the page's \
                   words as they stand; run `bash ci/build-book.sh --pot`";
    for changed in [
        "# Title\n\nOne quoted sentence.\n\n1. Step one\n",
        "# Title\n\nOne \"quoted\"\nparagraph.\n\nAnother.\n\n1. Step one\n",
        "# Title\n\n1. Step one\n",
    ] {
        assert_eq!(stale(pot, &page(changed)), vec![message.to_owned()], "{changed}");
    }
}
