// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The SQL sandbox target (WP-S6.04 D9, FT-223; doc 18 §3.12, NFR-SEC-06): the input as one SQL text (lossy UTF-8)
//! through a sandboxed `Reader` with the report profile (`Profile::REPORT`: 2 s, 100,000 rows) over the SQL projection
//! of the golden project, built once per process. Properties: whatever the text, nothing panics; the result is rows
//! within the profile's row cap or an `SqlError`; the projection's image is unchanged; and the same text run twice
//! gives the same result unless the deadline stopped one of the runs (no clock leaks into a result).
//!
//! The seeds are the hostile SQL of the corpus (`crates/ew-testkit/hostile/sql/`, the same bytes) and a few queries
//! that read the golden project's views.

use ew_api::sql::{Profile, Projection, SqlError};

use crate::golden;

thread_local! {
    /// The golden project's projection and the SHA-256 of its image.
    static PROJECTION: (Projection, [u8; 32]) = {
        let projection = golden::session(0)
            .sql_projection(None)
            .expect("the golden project's projection builds");
        let digest = ew_format::sha256(projection.image());
        (projection, digest)
    };
}

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// The rows of the result; `None` when the sandbox refused or stopped the statement.
    pub rows: Option<usize>,
}

/// Runs one input.
///
/// # Panics
///
/// When a result passes the row cap, the image changes, or two runs of the same text differ without a deadline.
pub fn check(input: &[u8]) -> Outcome {
    let sql = String::from_utf8_lossy(input);
    PROJECTION.with(|(projection, digest)| {
        let reader = projection
            .reader(Profile::REPORT)
            .expect("a reader opens on the image");
        let first = reader.query(&sql);
        if let Ok(set) = &first {
            assert!(
                set.rows.len() <= Profile::REPORT.max_rows,
                "{sql}: past the row cap"
            );
        }
        let second = reader.query(&sql);
        let stopped = |r: &Result<_, SqlError>| matches!(r, Err(SqlError::Deadline { .. }));
        if !stopped(&first) && !stopped(&second) {
            assert_eq!(first, second, "{sql}: two runs gave two results");
        }
        assert_eq!(
            &ew_format::sha256(projection.image()),
            digest,
            "{sql}: the image changed"
        );
        Outcome {
            rows: first.ok().map(|set| set.rows.len()),
        }
    })
}
