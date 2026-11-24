// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one generator of the key tables and of the list of text files (architecture §18 point 5): the build script of
//! every crate that owns a text folder calls [`generate`], and the crate reads what it writes through
//! [`texts!`](crate::texts) or `include!`.

use std::fmt::Write as _;
use std::path::Path;

/// Writes the tables of the text folder `folder` (relative to the crate's manifest; it holds `<language>/<area>.ftl`)
/// into the build's output directory, and has cargo run the build script again when the folder changes:
///
/// - `texts.rs`: `TEXTS`, every one-line text of the en-GB files that takes only positional arguments (`{ $p0 }`,
///   `{ $p1 }`, …), sorted by key, with its English: the arguments written `{0}`, `{1}`, … and a literal brace
///   (`{"{"}`, `{"}"}`) as the brace;
/// - `keys.rs`: `KEYS`, every text key of the en-GB files, sorted by bytes;
/// - `sources.rs`: `SOURCES`, every file of every language as (language, its text), the languages and each
///   language's files in name order.
///
/// A message starts a line with `<id> =`; a term starts with `-`, a comment with `#`, a continuation with a space.
#[allow(
    clippy::disallowed_methods,
    reason = "a build script reads the output directory cargo gives it"
)]
pub fn generate(folder: &str) {
    println!("cargo:rerun-if-changed={folder}");
    let names = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("the text folder is readable")
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect();
        names.sort();
        names
    };
    let languages: Vec<String> = names(Path::new(folder))
        .into_iter()
        .filter(|l| Path::new(folder).join(l).is_dir())
        .collect();
    let files = |language: &str| -> Vec<String> {
        names(&Path::new(folder).join(language))
            .into_iter()
            .filter(|f| f.ends_with(".ftl"))
            .collect()
    };
    let mut texts: Vec<(String, String)> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for file in files("en-GB") {
        let path = Path::new(folder).join("en-GB").join(file);
        let ftl = std::fs::read_to_string(path).expect("an en-GB text file is readable");
        let lines: Vec<&str> = ftl.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let Some((id, value)) = line.split_once(" =") else {
                continue;
            };
            let mut chars = id.chars();
            let is_id = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if !is_id {
                continue;
            }
            keys.push(id.to_owned());
            // A value that continues on the next line (an indented line) is not one line.
            let continues = lines.get(i + 1).is_some_and(|next| next.starts_with(' '));
            if continues {
                continue;
            }
            if let Some(text) = template(value.trim()) {
                texts.push((id.to_owned(), text));
            }
        }
    }
    texts.sort();
    keys.sort_unstable_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let mut out = String::from(
        "/// Every one-line text of this crate's en-GB text files that takes only positional arguments, sorted by key: \
         the key\n/// and its English, the arguments written `{0}`, `{1}`, ….\n\
         pub const TEXTS: &[(&str, &str)] = &[\n",
    );
    for (id, text) in &texts {
        let _ = writeln!(out, "    ({id:?}, {text:?}),");
    }
    out.push_str("];\n");
    let dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    std::fs::write(Path::new(&dir).join("texts.rs"), out).expect("the text table is written");
    let mut out = String::from(
        "/// Every text key of this crate's en-GB text files, sorted by bytes: the list a key is checked against when \
         the code\n/// is built.\npub const KEYS: &[&str] = &[\n",
    );
    for key in &keys {
        let _ = writeln!(out, "    {key:?},");
    }
    out.push_str("];\n");
    std::fs::write(Path::new(&dir).join("keys.rs"), out).expect("the key list is written");
    let mut out = String::from(
        "/// This crate's Fluent text files as (language, the file's text), the languages and each language's files in \
         name\n/// order.\npub const SOURCES: &[(&str, &str)] = &[\n",
    );
    for language in &languages {
        for file in files(language) {
            let path = format!("/{folder}/{language}/{file}");
            let _ = writeln!(
                out,
                "    ({language:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {path:?}))),"
            );
        }
    }
    out.push_str("];\n");
    std::fs::write(Path::new(&dir).join("sources.rs"), out).expect("the source list is written");
}

/// The table's form of a one-line Fluent value, or `None` when it holds anything but positional arguments and
/// literal braces.
fn template(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = value;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = open + rest[open..].find('}')?;
        let inner = rest[open + 1..close].trim();
        if let Some(n) = inner
            .strip_prefix("$p")
            .filter(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
        {
            out.push('{');
            out.push_str(n);
            out.push('}');
            rest = &rest[close + 1..];
        } else if inner == "\"{\"" {
            out.push('{');
            rest = &rest[close + 1..];
        } else if rest[open..].starts_with("{\"}\"}") {
            out.push('}');
            rest = &rest[open + "{\"}\"}".len()..];
        } else {
            return None;
        }
    }
    out.push_str(rest);
    Some(out)
}
