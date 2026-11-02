// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `.ewz` reader target (WP-S1.17 part 2, D9; decision log ID-1175 ruling 7): arbitrary bytes through
//! `ew_store::ewz::read`, the strict reader that every reader's `.ewz` open and `ew extract` go through, under the
//! default bounds of `ZipLimits` (2 GiB expanded, 512 MiB per entry, 200 : 1 per entry above 64 KiB). Property: whatever
//! the bytes, nothing panics; and when the entries of an archive the reader accepts pack again
//! (`ew_store::ewz::pack`, which reads its own output back), that archive reads to the same entries and manifest.

use std::collections::BTreeMap;

use ew_store::ewz;

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// Whether the reader accepted the bytes as an `.ewz` (then the re-pack was checked).
    pub accepted: bool,
}

/// Runs one input.
///
/// # Panics
///
/// When the reader accepts the bytes, their entries pack again and the new archive does not read to the same entries.
pub fn check(data: &[u8]) -> Outcome {
    let Ok(archive) = ewz::read(data) else {
        return Outcome { accepted: false };
    };
    let mut files = archive.entries.clone();
    let manifest = files
        .remove("manifest.json")
        .expect("an accepted archive holds its manifest");
    let mut extra = BTreeMap::new();
    for name in ewz::GIT_FILES {
        if let Some(data) = files.remove(name) {
            extra.insert(name.to_owned(), data);
        }
    }
    // The pack may refuse, in words, what this input's own deflate let through: the same data deflated anew can pass
    // the 200 : 1 bound (stored blocks of zeros). What it packs must read back to the same archive.
    if let Ok(again) = ewz::pack(&manifest, &files, &extra) {
        let reread = ewz::read(&again).unwrap_or_else(|e| panic!("a re-packed archive does not read: {e}"));
        assert_eq!(reread, archive, "a re-packed archive reads to the same entries");
    }
    Outcome { accepted: true }
}
