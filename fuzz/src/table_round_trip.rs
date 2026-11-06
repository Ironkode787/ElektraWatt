// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The table round trip's read (WP-S6.04 D9; WP-S3.08 part 2): one selector byte, the rest as a round-trip file read
//! by `Session::table_import` against the golden project's session (built once per process). The format is the bytes'
//! own, as in `ew import table` (the reader takes a ZIP signature for a workbook, anything else for CSV); the
//! selector's bit 0 says whether the table key is given (`--table`, needed for a CSV and for a workbook whose `_ew`
//! sheet names none) and its other bits pick the key among the round trip's navigators. Properties: whatever the
//! bytes, nothing panics; the read returns its changes or a refusal; and the model is unchanged (the read is pure:
//! nothing is committed).

use ew_api::Session;
use ew_api::output::{ImportTableOptions, TABLE_NAVIGATORS};

use crate::golden;

thread_local! {
    /// The golden project's session.
    static SESSION: Session = golden::session(0);
}

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// Whether the read returned its changes (else it refused the file).
    pub read: bool,
}

/// The table key a selector picks (`nav:<navigator>`), and whether it is given.
pub fn key(selector: u8) -> Option<String> {
    let kind = TABLE_NAVIGATORS[usize::from(selector >> 1) % TABLE_NAVIGATORS.len()];
    selector.is_multiple_of(2).then(|| format!("nav:{}", kind.word()))
}

/// Runs one input.
///
/// # Panics
///
/// When the read changes the model.
pub fn check(input: &[u8]) -> Outcome {
    let (selector, bytes) = input.split_first().map_or((0, input), |(s, r)| (*s, r));
    let options = ImportTableOptions {
        table: key(selector),
        preset: None,
        take_file: false,
    };
    SESSION.with(|s| {
        let before = s.world();
        let read = s.table_import(bytes, &options).is_ok();
        assert!(
            s.world().model.content_eq(&before.model),
            "the table read changed the model"
        );
        Outcome { read }
    })
}
