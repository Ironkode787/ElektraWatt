// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text-style tables (ADR-0032 decision 5): `ew_text::builtin()` holds the same rows as
//! `ew_schema::BUILTIN_TEXT_STYLES`, and the renderers of `ew-scene` and `ew-api` resolve styles only through the
//! table their caller passes (WP-S1.12 text styles, TS-7).

use std::path::{Path, PathBuf};

use ew_schema::{BUILTIN_TEXT_STYLES, TextWeight, qet_px_em};
use ew_text::{Weight, builtin, builtin_rows};

#[test]
fn the_builtin_table_of_ew_text_equals_the_schema_table() {
    let schema: Vec<(&str, u16, bool)> = BUILTIN_TEXT_STYLES
        .iter()
        .map(|(k, px, w)| (*k, *px, *w == TextWeight::Bold))
        .collect();
    let text: Vec<(&str, u16, bool)> = builtin_rows()
        .iter()
        .map(|r| (r.key, r.qet_px, r.weight == Weight::Bold))
        .collect();
    assert_eq!(text, schema);
    assert_eq!(builtin().rows().len(), BUILTIN_TEXT_STYLES.len());
    for ((key, px, _), row) in BUILTIN_TEXT_STYLES.iter().zip(builtin().rows()) {
        assert_eq!((row.key.as_str(), row.em), (*key, qet_px_em(*px)));
    }
}

/// The code lines of a source file: comment lines dropped, and nothing from the first `#[cfg(test)]` on.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .take_while(|(_, l)| l.trim() != "#[cfg(test)]")
        .filter(|(_, l)| !l.trim_start().starts_with("//"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir)
        .expect("read the source directory")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// TS-7: no `ew_text::builtin()` in the sources of `ew-scene` and `ew-api`, written out or imported, so a renderer
/// cannot ignore the project's styles (ADR-0032 risk).
#[test]
fn no_renderer_source_names_the_builtin_text_style_table() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for dir in ["ew-scene/src", "ew-api/src"] {
        rust_files(&root.join(dir), &mut files);
    }
    files.sort();
    assert!(files.len() > 2, "the scan found the sources");
    let mut hits = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("read a source file");
        for (n, line) in code_lines(&text) {
            let imported = line.contains("use ew_text::")
                && line
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .any(|w| w == "builtin");
            if line.contains("ew_text::builtin") || imported {
                hits.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(hits.is_empty(), "{hits:#?}");
}
