// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 3 (D9, ID-1219 ruling 2): every key's English text equals the committed snapshot
//! `tests/expected/en-GB-texts.txt`, one `key = text` line per key (a backslash, a line feed and a tab escaped as
//! `\\`, `\n` and `\t`), taken at the base of part 3 before the table moved to Fluent.

use ew_viewmodel::strings;

const SNAPSHOT: &str = include_str!("../expected/en-GB-texts.txt");

/// `text` with a backslash, a line feed and a tab escaped.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

/// LO-1: the English text of every key equals its line in the snapshot, in the snapshot's order.
#[test]
fn every_key_has_its_snapshot_text() {
    let have: String = strings::TEXTS
        .iter()
        .map(|(key, text)| format!("{key} = {}\n", escape(text)))
        .collect();
    if std::env::var_os("EW_BLESS").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected/en-GB-texts.txt");
        std::fs::write(path, &have).unwrap();
        return;
    }
    assert_eq!(have, SNAPSHOT.replace("\r\n", "\n"));
}
