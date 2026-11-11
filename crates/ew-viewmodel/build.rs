// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writes the list of the shell's text keys from the en-GB locale file, so that `key!` and `count_key!` check a key
//! when the code is built. The locale file is the one place a text stands.

// A build script reads the output directory cargo gives it.
#![allow(clippy::disallowed_methods)]

use std::fmt::Write as _;
use std::path::Path;

const SOURCE: &str = "locales/en-GB/shell.ftl";

fn main() {
    println!("cargo:rerun-if-changed={SOURCE}");
    println!("cargo:rerun-if-changed=build.rs");
    let ftl = std::fs::read_to_string(SOURCE).expect("the en-GB locale file is readable");
    // A message starts a line with `<id> =`; a term starts with `-`, a comment with `#`, a continuation with a space.
    let keys: Vec<&str> = ftl
        .lines()
        .filter_map(|line| line.split_once(" =").map(|(id, _)| id))
        .filter(|id| {
            let mut chars = id.chars();
            chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let mut out = String::new();
    list(
        &mut out,
        "/// Every text key of the shell, in the order of the en-GB file.",
        "KEYS",
        &keys,
    );
    list(
        &mut out,
        "/// The keys of [`KEYS`] sorted by bytes: the list `key!` and `count_key!` search at compile time.",
        "SORTED_KEYS",
        &sorted,
    );
    let dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    std::fs::write(Path::new(&dir).join("keys.rs"), out).expect("the key list is written");
}

/// Writes `pub const <name>: &[&str]` with `keys` to `out`, under the doc line `doc`.
fn list(out: &mut String, doc: &str, name: &str, keys: &[&str]) {
    let _ = writeln!(out, "{doc}\npub const {name}: &[&str] = &[");
    for key in keys {
        let _ = writeln!(out, "    {key:?},");
    }
    out.push_str("];\n");
}
