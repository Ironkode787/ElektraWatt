// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writes the English texts of the window's en-GB locale file as a table that `text!` checks when the code is built
//! and that the command line prints. The locale file is the one place a text stands: a refusal is written there,
//! with its Dutch in the nl-NL file beside it, and nowhere else.
//!
//! A message is taken when it is one line and takes only positional arguments (`{ $p0 }`, `{ $p1 }`, …), which the
//! table writes `{0}`, `{1}`, …; a literal brace (`{"{"}`, `{"}"}`) is written as the brace.

// A build script reads the output directory cargo gives it.
#![allow(clippy::disallowed_methods)]

use std::fmt::Write as _;
use std::path::Path;

/// The window's en-GB texts. They live with the view model, which loads them; this crate only reads the file.
const SOURCE: &str = "../ew-viewmodel/locales/en-GB/shell.ftl";

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

fn main() {
    println!("cargo:rerun-if-changed={SOURCE}");
    println!("cargo:rerun-if-changed=build.rs");
    let ftl = std::fs::read_to_string(SOURCE).expect("the en-GB locale file is readable");
    let lines: Vec<&str> = ftl.lines().collect();
    let mut texts: Vec<(&str, String)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some((id, value)) = line.split_once(" =") else {
            continue;
        };
        let mut chars = id.chars();
        let is_id = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        // A value that continues on the next line (an indented line) is not one line.
        let continues = lines.get(i + 1).is_some_and(|next| next.starts_with(' '));
        if !is_id || continues {
            continue;
        }
        if let Some(text) = template(value.trim()) {
            texts.push((id, text));
        }
    }
    texts.sort();
    let mut out = String::from(
        "/// Every one-line text of the en-GB locale file that takes only positional arguments, sorted by key: the key\n\
         /// and its English, the arguments written `{0}`, `{1}`, ….\n\
         pub const TEXTS: &[(&str, &str)] = &[\n",
    );
    for (id, text) in &texts {
        let _ = writeln!(out, "    ({id:?}, {text:?}),");
    }
    out.push_str("];\n");
    let dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    std::fs::write(Path::new(&dir).join("texts.rs"), out).expect("the text table is written");
}
