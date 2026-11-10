// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one list of commands (§9 item 9): every `impl Command` under `src/commands/` is an entry of the
//! `commands!` invocation, read from the sources. The macro catches a listed type that does not exist and the
//! registry a duplicate name; only a type that is never listed would pass silently.

use std::collections::BTreeSet;
use std::path::Path;

/// The `module::Type` entries of the `commands!` invocation in `mod_rs`, by type.
fn listed(mod_rs: &str) -> BTreeSet<String> {
    let body = mod_rs
        .split("\ncommands! {")
        .nth(1)
        .expect("mod.rs invokes commands!");
    let body = body.split("\n}").next().unwrap_or_default();
    body.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .filter_map(|l| l.trim_end_matches(',').split_once("::"))
        .map(|(_, ty)| ty.to_owned())
        .collect()
}

/// Whether `c` continues an identifier.
fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The byte offsets of `word` in `text` as a whole word (no identifier character on either side).
fn word_at(text: &str, word: &str) -> Vec<usize> {
    text.match_indices(word)
        .map(|(at, _)| at)
        .filter(|&at| {
            !text[..at].chars().next_back().is_some_and(ident_char)
                && !text[at + word.len()..].chars().next().is_some_and(ident_char)
        })
        .collect()
}

/// The type a line implements `Command` for, in any spelling: `impl Command for X`, `impl crate::Command for X`,
/// `impl<T: Bound> super::Command for X<T>` (the pattern `\bimpl\b[^{]*\bCommand\s+for\s+(\w+)`).
fn impl_command_type(line: &str) -> Option<String> {
    let start = *word_at(line, "impl").first()?;
    let head = line[start..].split('{').next().unwrap_or_default();
    word_at(head, "Command").into_iter().find_map(|at| {
        let rest = head[at + "Command".len()..].strip_prefix(char::is_whitespace)?;
        let rest = rest.trim_start().strip_prefix("for")?;
        let rest = rest.strip_prefix(char::is_whitespace)?.trim_start();
        let ty: String = rest.chars().take_while(|&c| ident_char(c)).collect();
        (!ty.is_empty()).then_some(ty)
    })
}

/// The failure texts of the types that implement `Command` in `files` (path, source) but are not in `list`.
fn unlisted(files: &[(String, String)], list: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for (path, source) in files {
        for line in source.lines() {
            let Some(ty) = impl_command_type(line) else {
                continue;
            };
            if !list.contains(&ty) {
                out.push(format!(
                    "{path}: {ty} implements Command but commands! does not list it"
                ));
            }
        }
    }
    out
}

/// Every `.rs` file under `dir`, as (path relative to the crate, `/`-separated; source), sorted by path.
fn sources(crate_dir: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("the commands directory reads")
        .map(|e| e.expect("a directory entry reads").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            sources(crate_dir, &path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let rel = path.strip_prefix(crate_dir).expect("under the crate");
            let rel: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            let source = std::fs::read_to_string(&path).expect("a source file reads");
            out.push((rel.join("/"), source));
        }
    }
}

#[test]
fn every_command_type_is_in_the_commands_list() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(crate_dir, &crate_dir.join("src/commands"), &mut files);
    let mod_rs = &files
        .iter()
        .find(|(p, _)| p == "src/commands/mod.rs")
        .expect("src/commands/mod.rs")
        .1;
    let list = listed(mod_rs);
    // The scan sees the commands: one known entry and its impl.
    assert!(list.contains("CreatePage"), "{list:?}");
    assert!(
        files
            .iter()
            .any(|(p, s)| p == "src/commands/page.rs" && s.contains("impl Command for CreatePage ")),
        "the scan reads src/commands/page.rs"
    );
    assert_eq!(unlisted(&files, &list), Vec::<String>::new());
}

#[test]
fn an_unlisted_command_type_is_named_with_its_file() {
    let mod_rs = "use x;\n\ncommands! {\n    page::CreatePage,\n    // WP-x\n    page::DeletePages,\n}\n";
    let list = listed(mod_rs);
    assert_eq!(
        list,
        BTreeSet::from(["CreatePage".to_owned(), "DeletePages".to_owned()])
    );
    let files = vec![(
        "src/commands/page.rs".to_owned(),
        "impl Command for CreatePage {\n}\nimpl Command for RenamePage {\n}\n".to_owned(),
    )];
    assert_eq!(
        unlisted(&files, &list),
        ["src/commands/page.rs: RenamePage implements Command but commands! does not list it"]
    );
    // Every spelling of the impl: a path to the trait, a generic impl, a nested one; never another trait.
    let files = vec![(
        "src/commands/device/mod.rs".to_owned(),
        "impl crate::Command for PathTrait {\n}\n\
         impl super::Command for DeletePages {\n}\n\
         impl<T: Clone> Command for Generic<T> {\n}\n\
         \x20   impl<'a> crate::command::Command  for  Nested<'a> where 'a: 'static {}\n\
         impl DynCommand for NotACommand {\n}\n\
         impl CommandLike for NotEither {\n}\n"
            .to_owned(),
    )];
    assert_eq!(
        unlisted(&files, &list),
        [
            "src/commands/device/mod.rs: PathTrait implements Command but commands! does not list it",
            "src/commands/device/mod.rs: Generic implements Command but commands! does not list it",
            "src/commands/device/mod.rs: Nested implements Command but commands! does not list it",
        ]
    );
}
