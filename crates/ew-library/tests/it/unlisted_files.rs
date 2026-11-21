// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A records file of a library directory that `library.json` does not list is not read, and the library says so in
//! its notes (`ew lib list` prints them), instead of the file being silently not there.

use ew_library::{LibraryTiers, Registry};

#[test]
fn a_records_file_the_manifest_does_not_list_is_named_in_the_library_notes() {
    let root = ew_testdir::Tmp::new("ew-lib-unlisted");
    let tiers = LibraryTiers {
        system: Vec::new(),
        community: root.join("packs"),
        company: vec![root.join("company")],
        user: root.join("libraries"),
        cache: root.join("cache"),
    };
    let dir = root.join("company").join("mine.ewlib");
    let lib = Registry::open(&tiers)
        .unwrap()
        .create(&dir, "mine", "Mine", "CC0-1.0")
        .unwrap();
    std::fs::create_dir_all(dir.join("symbols")).unwrap();
    std::fs::write(dir.join("symbols").join("ab.jsonl"), "{}\n").unwrap();

    let reg = Registry::open(&tiers).unwrap();
    let info = reg.libraries().into_iter().find(|l| l.id == lib).unwrap();
    assert!(
        info.findings
            .iter()
            .any(|f| f.starts_with("symbols/ab.jsonl: library.json does not list it, so it is not read")),
        "{:?}",
        info.findings
    );
}
