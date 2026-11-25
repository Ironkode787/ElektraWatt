// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The re-export rule of `cargo xtask rules` (ADR-0035 decision 4, architecture §3 rule 7: a crate names what it
//! uses): no `pub use` of another workspace crate, or of an item of one, outside the facades of [`FACADES`], so that
//! every item has one path and a search on its crate finds its users.

/// The crates that re-export workspace crates, each with why: their users are confined to them.
pub(crate) const FACADES: &[(&str, &str)] = &[
    (
        "ew-api",
        "the shell's one door into the workspace: `bin/ew` and the view model reach the session through it",
    ),
    (
        "ew-viewmodel",
        "the window's one door: `ew-ui` reaches the session and its types through it",
    ),
    (
        "ew-interop-api",
        "the importers' one door: an importer reaches the records and the IDs through it",
    ),
];

/// The violations among `files` (path and bytes) of the workspace crates `crates` (their package names): each
/// `pub use` of another of them, by file and line, outside a crate of [`FACADES`].
pub(crate) fn check(files: &[(String, Vec<u8>)], crates: &[String]) -> Vec<String> {
    let idents: Vec<String> = crates.iter().map(|c| c.replace('-', "_")).collect();
    let mut out = Vec::new();
    for (path, bytes) in files {
        let Some(owner) = owner(path) else { continue };
        if FACADES.iter().any(|(facade, _)| *facade == owner) {
            continue;
        }
        let own = owner.replace('-', "_");
        let text = String::from_utf8_lossy(bytes);
        let lines: Vec<&str> = text.lines().collect();
        let tests = test_modules(&lines);
        for (i, line) in lines.iter().enumerate() {
            if tests.contains(&i) {
                continue;
            }
            let Some(rest) = without_attributes(line.trim_start()).strip_prefix("pub use ") else {
                continue;
            };
            let rest = rest.trim_start().trim_start_matches("::");
            let named = rest.split([':', ';', ' ', '{']).next().unwrap_or("");
            if named != own && idents.iter().any(|c| c == named) {
                out.push(format!(
                    "{path}:{}: `pub use {named}` re-exports another workspace crate; name it where it is used (a \
                     facade is {})",
                    i + 1,
                    FACADES.iter().map(|(f, _)| *f).collect::<Vec<_>>().join(", ")
                ));
            }
        }
    }
    out
}

/// `line` without the attributes that stand before an item on its line (`#[doc(inline)] pub use …`).
fn without_attributes(mut line: &str) -> &str {
    while let Some(rest) = line.strip_prefix("#[") {
        let Some(end) = rest.find(']') else {
            return line;
        };
        line = rest[end + 1..].trim_start();
    }
    line
}

/// The indices of the lines of the test modules among `lines`: a `mod` that a `#[cfg(test)]` line stands before,
/// to its closing brace. A test module's `pub use` re-exports nothing.
fn test_modules(lines: &[&str]) -> std::collections::BTreeSet<usize> {
    let mut out = std::collections::BTreeSet::new();
    let mut i = 0;
    while i < lines.len() {
        let opens = lines[i].trim() == "#[cfg(test)]"
            && lines.get(i + 1).is_some_and(|l| {
                let l = l.trim_start();
                (l.starts_with("mod ") || l.starts_with("pub mod ") || l.starts_with("pub(crate) mod "))
                    && l.trim_end().ends_with('{')
            });
        if !opens {
            i += 1;
            continue;
        }
        let (mut opened, mut closed) = (0, 0);
        let mut j = i + 1;
        while j < lines.len() {
            opened += lines[j].matches('{').count();
            closed += lines[j].matches('}').count();
            out.insert(j);
            j += 1;
            if closed >= opened {
                break;
            }
        }
        i = j;
    }
    out
}

/// The package whose source `path` is (`crates/<name>/src/…`, `bin/<name>/src/…`); `None` for any other file.
fn owner(path: &str) -> Option<&str> {
    let rest = path
        .strip_prefix("crates/")
        .or_else(|| path.strip_prefix("bin/"))?;
    let (name, rest) = rest.split_once('/')?;
    (rest.starts_with("src/") && rest.ends_with(".rs")).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::check;

    /// A re-export of another workspace crate or of its item is named, outside a facade, also behind an attribute on
    /// its line and with a leading `::`; a crate's own items, an external crate's, a facade's and a test module's are
    /// not.
    #[test]
    fn a_reexport_of_another_workspace_crate_is_named() {
        let crates: Vec<String> = ["ew-a", "ew-b", "ew-api"].map(String::from).to_vec();
        let file = |path: &str, text: &str| (path.to_owned(), text.as_bytes().to_vec());
        let files = vec![
            file(
                "crates/ew-a/src/lib.rs",
                "pub use ew_b;\npub use ew_b::{X, Y};\n    pub use ew_b::Z;\npub use crate::m::W;\npub use ew_a::V;\n\
                 pub use serde::Serialize;\npub(crate) use ew_b::Q;\n#[doc(inline)] pub use ew_b::R;\n\
                 pub use ::ew_b::S;\n#[cfg(test)]\nmod tests {\n    pub use ew_b::T;\n    fn f() {}\n}\n\
                 pub use ew_b::U;\n",
            ),
            file("crates/ew-api/src/lib.rs", "pub use ew_b;\n"),
            file("crates/ew-a/tests/it.rs", "pub use ew_b;\n"),
        ];
        let v = check(&files, &crates);
        let lines: Vec<&str> = v.iter().map(|s| s.split(": ").next().unwrap()).collect();
        assert_eq!(
            lines,
            [
                "crates/ew-a/src/lib.rs:1",
                "crates/ew-a/src/lib.rs:2",
                "crates/ew-a/src/lib.rs:3",
                "crates/ew-a/src/lib.rs:8",
                "crates/ew-a/src/lib.rs:9",
                "crates/ew-a/src/lib.rs:15"
            ]
        );
    }
}
