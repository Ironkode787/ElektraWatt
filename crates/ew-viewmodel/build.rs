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
    let mut out = String::from(
        "/// Every text key of the shell, in the order of the en-GB file: the keys `key!` and `count_key!` check at\n\
         /// compile time.\n\
         pub const KEYS: &[&str] = &[\n",
    );
    // A message starts a line with `<id> =`; a term starts with `-`, a comment with `#`, a continuation with a space.
    for line in ftl.lines() {
        let Some((id, _)) = line.split_once(" =") else {
            continue;
        };
        let mut chars = id.chars();
        if chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            let _ = writeln!(out, "    {id:?},");
        }
    }
    out.push_str("];\n");
    let dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    std::fs::write(Path::new(&dir).join("keys.rs"), out).expect("the key list is written");
}
