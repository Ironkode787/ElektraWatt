// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gen-maps`, its first half: the action map `docs/map/actions.md`, one row per action of the view-model's registry
//! with its entry in the table, the method its handler calls with its file, and the text keys that name it.
//!
//! The code builds an action's text keys from its ID at run time (`action-file-new` from `file.new`, and likewise
//! `tooltip-`, `bar-`, `valve-`, `menu-`, `form-hint-…-<field>`), so a search for the ID never finds its texts; this
//! map is where both meet. It is read from the source text, nothing is built (xtask links no GPL crate): the entries
//! of `crates/ew-viewmodel/src/actions/table/` in the order of its `SECTIONS`, each `act(…)` call's ID and handler,
//! the handler's method among the crate's `fn`s, and every key of the English catalogue whose words contain an
//! action's words (the longest such action takes the key). Not here: the persona harness's action list
//! (`target/reference/actions.txt`, written by the registry's own test, `generate.rs`); the crate map
//! (`crate_map.rs`).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// The generated map, relative to the workspace root.
pub(crate) const OUTPUT: &str = "docs/map/actions.md";
/// The action table's sections.
const TABLE_DIR: &str = "crates/ew-viewmodel/src/actions/table";
/// The view-model's source, where the handlers' methods are.
const SOURCE_DIR: &str = "crates/ew-viewmodel/src";
/// The crates whose `texts/en-GB/` folders hold the texts the window shows: every crate's but the rule messages'
/// (`crates/ew-checks`). The Dutch folders hold the same keys.
const CATALOGUE: &str = "crates";
/// The crate whose texts are the rule messages.
const RULE_MESSAGES: &str = "ew-checks";
/// The arguments of `act(…)` before its handler: ID, kind, category, shortcuts, enabled, menu slot.
const ARGS_BEFORE_HANDLER: usize = 6;

/// One entry of the action table.
#[derive(Debug, PartialEq)]
struct Entry {
    id: String,
    /// The table file, relative to the root.
    file: String,
    /// The handler's text, a closure.
    handler: String,
}

/// The section names of `table.rs`'s `SECTIONS`, in registry order (`file::ACTIONS,` gives `file`).
fn sections(table_rs: &str) -> Result<Vec<String>> {
    let start = table_rs.find("SECTIONS").context("table.rs has no SECTIONS")?;
    let body = &table_rs[start..];
    let end = body.find("];").context("SECTIONS is not closed")?;
    let names: Vec<String> = body[..end]
        .lines()
        .filter_map(|l| l.trim().strip_suffix("::ACTIONS,"))
        .map(str::to_owned)
        .collect();
    if names.is_empty() {
        bail!("SECTIONS names no section");
    }
    Ok(names)
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The text from `open` (just after an opening parenthesis) to its matching close, string literals and line comments
/// skipped; `None` when it is not closed.
fn balanced(text: &str, open: usize) -> Option<&str> {
    let b = text.as_bytes();
    let (mut depth, mut i) = (1usize, open);
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open..i]);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// `args` split at its commas outside brackets and strings.
fn top_level_args(args: &str) -> Vec<&str> {
    let b = args.as_bytes();
    let (mut out, mut depth, mut from, mut i) = (Vec::new(), 0usize, 0usize, 0usize);
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                out.push(&args[from..i]);
                from = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if !args[from..].trim().is_empty() {
        out.push(&args[from..]);
    }
    out
}

/// The entries of one table file, in their order.
fn entries_of(file: &str, text: &str) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut from = 0;
    while let Some(at) = text[from..].find("act(").map(|i| i + from) {
        from = at + 4;
        if at > 0 && is_ident(b[at - 1]) {
            continue;
        }
        let args = balanced(text, from).with_context(|| format!("{file}: an act( that is not closed"))?;
        let parts = top_level_args(args);
        let id = parts[0].trim();
        let Some(id) = id.strip_prefix('"').and_then(|i| i.strip_suffix('"')) else {
            bail!("{file}: an act( whose first argument is not the ID: {id}");
        };
        if parts.len() <= ARGS_BEFORE_HANDLER {
            bail!("{file}: act(\"{id}\") has no handler");
        }
        out.push(Entry {
            id: id.to_owned(),
            file: file.to_owned(),
            handler: parts[ARGS_BEFORE_HANDLER..].join(",").trim().to_owned(),
        });
    }
    Ok(out)
}

/// The method a handler closure calls first (`|s, p| s.act_new(p)` gives `act_new`); `None` for a closure that calls
/// nothing.
fn handler_method(handler: &str) -> Option<&str> {
    // The body follows the closure's parameters.
    let body = handler.splitn(3, '|').nth(2).unwrap_or(handler);
    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if is_ident(b[i]) && (i == 0 || !is_ident(b[i - 1])) {
            let start = i;
            while i < b.len() && is_ident(b[i]) {
                i += 1;
            }
            let word = &body[start..i];
            let call = body[i..].trim_start().starts_with('(');
            let constructor = word.starts_with(|c: char| c.is_ascii_uppercase());
            if call && !constructor && !matches!(word, "if" | "match" | "while" | "for" | "return") {
                return Some(word);
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Every `fn <name>` of the files: name to the files that define it. A handler's method is the shell's: where a name
/// is defined in an `impl ShellModel` and elsewhere too, only the shell's places are kept.
fn definitions(files: &[(String, String)]) -> BTreeMap<String, Vec<String>> {
    let mut all: BTreeMap<String, Vec<(String, bool)>> = BTreeMap::new();
    for (file, text) in files {
        let mut in_shell = false;
        for line in text.lines() {
            if line.starts_with("impl") {
                in_shell = line.contains("ShellModel");
            }
            let mut rest = line;
            while let Some(at) = rest.find("fn ") {
                let before_ok = at == 0 || !is_ident(rest.as_bytes()[at - 1]);
                let after = &rest[at + 3..];
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                let follows = after[name.len()..].trim_start();
                if before_ok && !name.is_empty() && (follows.starts_with('(') || follows.starts_with('<')) {
                    all.entry(name).or_default().push((file.clone(), in_shell));
                }
                rest = after;
            }
        }
    }
    all.into_iter()
        .map(|(name, places)| {
            let shell = places.iter().any(|(_, s)| *s);
            let kept = places
                .into_iter()
                .filter(|(_, s)| *s || !shell)
                .map(|(p, _)| p)
                .collect::<std::collections::BTreeSet<String>>()
                .into_iter()
                .collect();
            (name, kept)
        })
        .collect()
}

/// The message keys of a Fluent catalogue, in its order.
fn catalogue_keys(ftl: &str) -> Vec<&str> {
    ftl.lines()
        .filter_map(|l| l.split_once(" ="))
        .map(|(k, _)| k)
        .filter(|k| {
            !k.is_empty()
                && k.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
        .collect()
}

/// The words of an ID as its keys spell them (`file.open_recent` gives `file-open-recent`).
fn words(id: &str) -> String {
    id.replace(['.', '_'], "-")
}

/// For each key, the action whose words it contains at word boundaries, the longest such action where several do
/// (`bar-view-zoom-fit` goes to `view.zoom_fit`, not `view.zoom`): the action's index to its keys, in key order.
fn keys_by_action<'k>(ids: &[&str], keys: &[&'k str]) -> BTreeMap<usize, Vec<&'k str>> {
    let by_words: BTreeMap<String, usize> = ids.iter().enumerate().map(|(i, id)| (words(id), i)).collect();
    let mut out: BTreeMap<usize, Vec<&'k str>> = BTreeMap::new();
    for key in keys {
        let parts: Vec<&str> = key.split('-').collect();
        let mut best: Option<(usize, usize)> = None;
        for start in 0..parts.len() {
            for end in start + 1..=parts.len() {
                if let Some(&action) = by_words.get(&parts[start..end].join("-")) {
                    let len = end - start;
                    if best.is_none_or(|(l, _)| len > l) {
                        best = Some((len, action));
                    }
                }
            }
        }
        if let Some((_, action)) = best {
            out.entry(action).or_default().push(key);
        }
    }
    for list in out.values_mut() {
        list.sort_unstable();
    }
    out
}

/// Every `.rs` file under `dir`, relative to `root`, sorted, with its text (line ends as LF).
pub(crate) fn rust_files(root: &Path, dir: &str) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut stack = vec![root.join(dir)];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).with_context(|| format!("reading {}", d.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?
                    .replace("\r\n", "\n");
                out.push((rel, text));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The map's text.
fn generate(root: &Path) -> Result<String> {
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel))
            .with_context(|| format!("reading {rel}"))
            .map(|t| t.replace("\r\n", "\n"))
    };
    let mut entries = Vec::new();
    for section in sections(&read(&format!("{TABLE_DIR}.rs"))?)? {
        let file = format!("{TABLE_DIR}/{section}.rs");
        entries.extend(entries_of(&file, &read(&file)?)?);
    }
    let defs = definitions(&rust_files(root, SOURCE_DIR)?);
    let names = |dir: &str| -> Result<Vec<String>> {
        let mut names: Vec<String> = std::fs::read_dir(root.join(dir))
            .with_context(|| format!("reading {dir}"))?
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect();
        names.sort();
        Ok(names)
    };
    let mut ftl = String::new();
    for krate in names(CATALOGUE)? {
        let folder = format!("{CATALOGUE}/{krate}/texts/en-GB");
        if krate == RULE_MESSAGES || !root.join(&folder).is_dir() {
            continue;
        }
        for area in names(&folder)?.into_iter().filter(|name| name.ends_with(".ftl")) {
            ftl.push_str(&read(&format!("{folder}/{area}"))?);
        }
    }
    let keys = catalogue_keys(&ftl);
    let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    let by_action = keys_by_action(&ids, &keys);

    let mut out = String::from(
        "# The action map\n\n\
         Generated by `cargo xtask gen-maps` (`xtask/src/action_map.rs`) from the action table; do not edit it. \
         One row per action, in registry order: its ID; its entry in `crates/ew-viewmodel/src/actions/table/`; the \
         method its handler calls, with the file of each `fn` of that name in `crates/ew-viewmodel/src` \
         (\"inline\" where the closure calls none); the text keys that name it. No line numbers, so that the map \
         changes only when an action, a handler's file or a key does: search the name in the file it names. The code builds an action's keys \
         from its ID (`action-file-new` is the label of `file.new`; also `tooltip-`, `bar-`, `valve-`, `menu-`, \
         `form-hint-<action>-<field>`), so a search for the ID finds them only here.\n\n\
         Every key stands in a file of the `texts/en-GB/` folder of the crate that says it and, in Dutch, in its \
         `texts/nl-NL/`.\n\n\
         | Action | Entry | Handler | Text keys (in `texts/en-GB/` and `texts/nl-NL/`) |\n|---|---|---|---|\n",
    );
    for (i, e) in entries.iter().enumerate() {
        let handler = match handler_method(&e.handler) {
            Some(name) => match defs.get(name) {
                Some(places) => format!(
                    "`{name}` {}",
                    places
                        .iter()
                        .map(|p| format!("`{p}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                None => format!("`{name}`"),
            },
            None => "inline".to_owned(),
        };
        let keys = by_action.get(&i).map_or_else(String::new, |k| {
            k.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")
        });
        writeln!(out, "| `{}` | `{}` | {handler} | {keys} |", e.id, e.file)?;
    }
    for (i, e) in entries.iter().enumerate() {
        let label = format!("action-{}", words(&e.id));
        if !by_action.get(&i).is_some_and(|k| k.contains(&label.as_str())) {
            bail!(
                "{}: the action {} has no label {label} in the texts the window shows",
                e.file,
                e.id
            );
        }
    }
    Ok(out)
}

/// The problems of `gen-maps --check` for this map: empty when the committed map is the one the tree gives.
pub(crate) fn violations(root: &Path) -> Result<Vec<String>> {
    let wanted = generate(root)?;
    let have = std::fs::read_to_string(root.join(OUTPUT))
        .ok()
        .map(|t| t.replace("\r\n", "\n"));
    Ok(if have.as_deref() == Some(wanted.as_str()) {
        Vec::new()
    } else {
        vec![format!("{OUTPUT} differs; run `cargo xtask gen-maps`")]
    })
}

/// Writes the map.
pub(crate) fn write(root: &Path) -> Result<()> {
    let path = root.join(OUTPUT);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    crate::repo::write_atomic(&path, &generate(root)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sections_come_in_their_order() {
        let table = "pub const SECTIONS: &[&[ActionMeta]] = &[\n    file::ACTIONS,\n    edit::ACTIONS,\n];\n";
        assert_eq!(sections(table).unwrap(), ["file", "edit"]);
    }

    #[test]
    fn an_entry_gives_its_id_and_handler() {
        let text = "const A: &[ActionMeta] = &[\n    with_params(\n        \"dir?:path\",\n        act(\n            \
                    \"file.new\",\n            Command,\n            Menu::File,\n            &[\"Ctrl+N\"],\n            \
                    always,\n            slot(Menu::File),\n            |s, p| s.act_new(p),\n        ),\n    ),\n    \
                    hidden(act(\"wizard.back\", View, Menu::Project, &[], wizard_open, None, |s, _| {\n        \
                    s.act_wizard_step(-1)\n    })),\n];\n";
        let entries = entries_of("t.rs", text).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "file.new");
        assert_eq!(handler_method(&entries[0].handler), Some("act_new"));
        assert_eq!(entries[1].id, "wizard.back");
        assert_eq!(handler_method(&entries[1].handler), Some("act_wizard_step"));
        assert_eq!(handler_method("|s, _| Outcome::Done"), None);
    }

    #[test]
    fn a_key_goes_to_the_longest_action_it_names() {
        let ids = ["view.zoom", "view.zoom_fit", "file.new"];
        let keys = [
            "action-view-zoom",
            "bar-view-zoom-fit",
            "form-hint-file-new-dir",
            "menu-file",
        ];
        let by = keys_by_action(&ids, &keys);
        assert_eq!(by[&0], ["action-view-zoom"]);
        assert_eq!(by[&1], ["bar-view-zoom-fit"]);
        assert_eq!(by[&2], ["form-hint-file-new-dir"]);
    }

    #[test]
    fn a_method_is_found_where_it_is_defined() {
        let files = vec![
            (
                "a.rs".to_owned(),
                "impl Walker {\n    fn walk(&mut self) {}\n}\n".to_owned(),
            ),
            (
                "b.rs".to_owned(),
                "impl ShellModel {\n    pub(crate) fn walk(&mut self) {}\n}\n".to_owned(),
            ),
        ];
        assert_eq!(definitions(&files)["walk"], ["b.rs"]);
    }
}
