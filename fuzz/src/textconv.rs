// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The textconv target (WP-S6.04 D9; doc 18 §3.12: git runs textconv on fetched content): one selector byte, the rest
//! as one project file through `ew_api::output::textconv`, without a session (even) or with the golden project's
//! session (odd), built once per process. Property: whatever the bytes, nothing panics and a text results; without a
//! session it names no record by the model, and with one it names at most one record per line.

use ew_api::Session;
use ew_api::output::textconv;

use crate::golden;

thread_local! {
    /// The golden project's session.
    static SESSION: Session = golden::session(0);
}

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// The lines of the text.
    pub lines: usize,
    /// The records the session named.
    pub named: usize,
}

/// Runs one input.
///
/// # Panics
///
/// When the session-less text names a record by the model, or more records are named than lines printed.
pub fn check(input: &[u8]) -> Outcome {
    let (selector, bytes) = input.split_first().map_or((0, input), |(s, r)| (*s, r));
    let (text, named) = if selector % 2 == 0 {
        let (text, named) = textconv(None, bytes);
        assert_eq!(named, 0, "a record named without a session");
        (text, named)
    } else {
        SESSION.with(|s| textconv(Some(s), bytes))
    };
    let lines = text.lines().count();
    assert!(named <= lines, "{named} records named in {lines} lines");
    Outcome { lines, named }
}
