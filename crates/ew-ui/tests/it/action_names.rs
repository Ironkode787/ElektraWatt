// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window names its actions by plain strings (`Event::Action("file.open", …)`); this test reads the window's
//! source and fails on a name the registry does not hold, so that a typo or a renamed action is caught here and not
//! when a person presses the button. Not here: whether an action is enabled or what it does (the view-model's tests).

use std::path::Path;

/// The places where the window's source names an action: the text before the name, and how many arguments stand
/// between that text and the name.
const NAMED_AT: &[(&str, usize)] = &[
    ("Event::Action(", 0),
    ("Event::SymbolAsk(", 0),
    ("DialogAnswer::Action(", 0),
    ("actions::find(", 0),
    ("dispatch(", 0),
    // `action(events, "…", params)` and `ask(events, "…", params)`, the window's helpers.
    ("action(", 1),
    ("ask(", 1),
    ("routed(", 1),
    ("action_button(", 1),
    ("push(", 0),
    ("apply: ", 0),
    ("action == ", 0),
    ("action != ", 0),
];

/// The string literal at `at` after skipping `skip` arguments, if one stands there.
fn literal_after(text: &str, mut at: usize, skip: usize) -> Option<&str> {
    let b = text.as_bytes();
    for _ in 0..skip {
        let mut depth = 0usize;
        loop {
            match b.get(at)? {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' if depth == 0 => return None,
                b')' | b']' | b'}' => depth -= 1,
                b',' if depth == 0 => break,
                _ => {}
            }
            at += 1;
        }
        at += 1;
    }
    let rest = text[at..].trim_start();
    let inner = rest.strip_prefix('"')?;
    Some(&inner[..inner.find('"')?])
}

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn every_action_the_window_names_is_in_the_registry() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let (mut named, mut unknown) = (Vec::new(), Vec::new());
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for (head, skip) in NAMED_AT {
            for (at, _) in text.match_indices(head) {
                let before = text[..at].chars().next_back();
                if head.starts_with(|c: char| c.is_ascii_lowercase())
                    && before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    continue;
                }
                // Only a dotted name is an action's; `push("text")` of a plain word is not.
                let Some(name) = literal_after(&text, at + head.len(), *skip).filter(|n| n.contains('.'))
                else {
                    continue;
                };
                named.push(name.to_owned());
                if ew_viewmodel::actions::find(name).is_none() {
                    let line = text[..at].lines().count();
                    let rel = file.strip_prefix(&src).unwrap_or(file);
                    unknown.push(format!("src/{}:{line}: {name}", rel.display()));
                }
            }
        }
    }
    assert!(unknown.is_empty(), "not in the registry:\n{}", unknown.join("\n"));
    // The scan reads what it should: the window's Open and its Escape are among the names it found.
    assert!(named.iter().any(|n| n == "file.open") && named.iter().any(|n| n == "ui.dismiss"));
}
