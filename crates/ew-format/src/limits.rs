// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The loader's hostile-input limits (doc 05 §3.6, NFR-SEC-02).

/// The limits of the project reader. Parsing is linear in the input under every limit: each file is read
/// once, each line is scanned once for its nesting depth before it is parsed, and no limit is checked by
/// a quadratic pass.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// The longest line of a JSON Lines file, without its LF (doc 05 §3.6: 16 MiB).
    pub max_line_bytes: usize,
    /// The largest file (doc 05 §3.6: 512 MiB).
    pub max_file_bytes: u64,
    /// The most files in a project directory (doc 05 §3.6: 100,000), counted over the manifest's
    /// `files` and over a directory listing.
    pub max_files: usize,
    /// The deepest nesting of arrays and objects in one JSON value (doc 05 §3.6: 64).
    pub max_depth: usize,
    /// The most lines of one JSON Lines file (a bound on the work per file; not in doc 05).
    pub max_lines_per_file: usize,
    /// The largest `manifest.json` (not in doc 05: 100,000 entries of at most 1 KiB each).
    pub max_manifest_bytes: u64,
    /// The most findings kept; the rest are counted ([`Findings`](crate::Findings)).
    pub max_findings: usize,
}

/// Doc 05 §3.6: 16 MiB.
pub const MAX_LINE_BYTES: usize = 16 << 20;
/// Doc 05 §3.6: 512 MiB.
pub const MAX_FILE_BYTES: u64 = 512 << 20;
/// Doc 05 §3.6: 100,000 files.
pub const MAX_FILES: usize = 100_000;
/// Doc 05 §3.6: nesting depth 64.
pub const MAX_DEPTH: usize = 64;
/// Lines per JSON Lines file (this crate's bound: 4 Mi lines, far beyond an XL shard).
pub const MAX_LINES_PER_FILE: usize = 4 << 20;
/// The largest manifest: 100 MiB.
pub const MAX_MANIFEST_BYTES: u64 = 100 << 20;
/// Findings kept per load.
pub const MAX_FINDINGS: usize = 1_000;

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_line_bytes: MAX_LINE_BYTES,
            max_file_bytes: MAX_FILE_BYTES,
            max_files: MAX_FILES,
            max_depth: MAX_DEPTH,
            max_lines_per_file: MAX_LINES_PER_FILE,
            max_manifest_bytes: MAX_MANIFEST_BYTES,
            max_findings: MAX_FINDINGS,
        }
    }
}
