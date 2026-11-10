// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writes the tables of the compiled-in content from the `content/` folder, which is the one place a starter item
//! stands: every `*.json` of `content/symbols`, `content/symbols/qet10` and `content/fndefs` as a starter item in
//! key order, and the records files of `content/libraries/ew-starter-parts.ewlib` in path order.

// A build script reads the directories cargo gives it.
#![allow(clippy::disallowed_methods)]

use std::fmt::Write as _;
use std::path::Path;

/// The starter's folders under `content/` with the kind of the items they hold.
const STARTER: &[(&str, &str)] = &[
    ("symbols", "Symbol"),
    ("symbols/qet10", "Symbol"),
    ("fndefs", "Fndef"),
];

const PARTS: &str = "libraries/ew-starter-parts.ewlib";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let content = Path::new(&manifest).join("../../content");
    let out = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    let out = Path::new(&out);

    // (key, kind, path under `content/`)
    let mut items: Vec<(String, &str, String)> = Vec::new();
    for (dir, kind) in STARTER {
        println!("cargo:rerun-if-changed={}", content.join(dir).display());
        for entry in std::fs::read_dir(content.join(dir)).expect("a starter folder is readable") {
            let name = entry
                .expect("a starter folder entry is readable")
                .file_name()
                .to_string_lossy()
                .into_owned();
            if let Some(key) = name.strip_suffix(".json") {
                items.push((key.to_owned(), kind, format!("{dir}/{name}")));
            }
        }
    }
    items.sort();
    let mut text = String::from("&[\n");
    for (key, kind, path) in &items {
        let _ = writeln!(
            text,
            "    StarterItem {{ key: {key:?}, kind: StarterKind::{kind}, json: content!({path:?}) }},"
        );
    }
    text.push_str("]\n");
    std::fs::write(out.join("starter_items.rs"), text).expect("the starter table is written");

    let mut files = Vec::new();
    println!("cargo:rerun-if-changed={}", content.join(PARTS).display());
    records(&content.join(PARTS), "", &mut files);
    files.sort();
    let mut text = String::from("&[\n");
    for path in files.iter().filter(|p| *p != "library.json") {
        let full = format!("{PARTS}/{path}");
        let _ = writeln!(text, "    ({path:?}, content!({full:?})),");
    }
    text.push_str("]\n");
    std::fs::write(out.join("starter_parts.rs"), text).expect("the starter parts table is written");
}

/// The files below `dir`, as paths with `/` relative to the library's folder.
fn records(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("the starter parts library is readable") {
        let entry = entry.expect("a library entry is readable");
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.path().is_dir() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
            records(&entry.path(), &format!("{path}/"), out);
        } else {
            out.push(path);
        }
    }
}
