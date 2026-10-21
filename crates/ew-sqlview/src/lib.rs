// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only SQL projection: materialisation, `api_v1_*` views, sandbox, `ew_natural` collation (ADR-0017,
//! ARCH §10.2, doc 17 §6).
//!
//! # Shape
//!
//! ```text
//! let projection = Projection::build(&q, &BuildInput { checks: Some(&report), ..Default::default() })?;
//! let reader = projection.reader(Profile::EW_SQL)?;       // one sandboxed connection per consumer
//! let rows = reader.query("SELECT count(*) FROM api_v1_terminal")?;
//! std::fs::write("dump.sqlite", projection.image())?;      // `ew sql dump`
//! ```
//!
//! - **A derived artefact** (ADR-0017 decision 1). [`Projection::build`] reads the model through tracked
//!   queries on the caller's `Q` (a fork at one revision) and writes the internal `t_*` tables of an
//!   in-memory SQLite database, then keeps its serialised image. It is never written by users and never read
//!   back into the model (decision 7).
//! - **Public contract** (decision 2). Consumers see only the `api_v1_*` views of [`schema::VIEWS`].
//! - **Sandbox** (decision 4). Every consumer reads through its own [`Reader`]: a read-only connection on a
//!   read-only copy of the image with the statement gate, the authorizer, the limits, the deadline and the
//!   result caps of its [`Profile`].
//! - **File-backed XL projections** (ADR-0017 decision 1, NFR-PERF-13) and the per-table refresh of the GUI
//!   projection (decision 3) are not built yet: every build is a full, in-memory build.

#![forbid(unsafe_code)]

mod conn;
mod error;
mod functions;
mod materialise;
mod result;
mod rows;
mod sandbox;
#[cfg(test)]
mod sandbox_tests;
pub mod schema;

use ew_derive::Q;

pub use error::{BuildError, SqlError};
pub use functions::NATURAL;
pub use materialise::BuildInput;
pub use result::{ResultSet, Value, csv_field};
pub use sandbox::{LIMITS, Profile, Reader};

/// A built projection: the serialised image of the SQLite database holding the `t_*` tables and the
/// `api_v1_*` views.
#[derive(Clone)]
pub struct Projection {
    image: Vec<u8>,
    counts: Vec<(&'static str, usize)>,
}

impl std::fmt::Debug for Projection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Projection")
            .field("bytes", &self.image.len())
            .field("counts", &self.counts)
            .finish()
    }
}

impl Projection {
    /// Materialises the model of `q` (and the findings of `input.checks`) into a new projection.
    ///
    /// # Errors
    ///
    /// [`BuildError`] when SQLite fails while the tables are written.
    pub fn build(q: &Q, input: &BuildInput<'_>) -> Result<Projection, BuildError> {
        let tables = materialise::tables(q, input);
        let mut conn = rusqlite::Connection::open_in_memory()?;
        conn.execute_batch(&schema::ddl())?;
        let tx = conn.transaction()?;
        let mut counts = Vec::with_capacity(tables.len());
        for (view, rows) in &tables {
            let placeholders = (1..=view.columns.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!("INSERT INTO {} VALUES ({placeholders})", view.table());
            let mut stmt = tx.prepare(&sql)?;
            for row in rows {
                stmt.execute(rusqlite::params_from_iter(row.iter()))?;
            }
            counts.push((view.name, rows.len()));
        }
        tx.commit()?;
        let image = conn.serialize("main")?.to_vec();
        Ok(Projection { image, counts })
    }

    /// The SQLite database image: a complete database file (`ew sql dump <out.sqlite>`).
    pub fn image(&self) -> &[u8] {
        &self.image
    }

    /// The number of rows of every view, in view-name order.
    pub fn counts(&self) -> &[(&'static str, usize)] {
        &self.counts
    }

    /// A sandboxed reader with the limits of `profile`.
    ///
    /// # Errors
    ///
    /// [`SqlError::Sql`] when SQLite cannot open the copy.
    pub fn reader(&self, profile: Profile) -> Result<Reader, SqlError> {
        Reader::open(&self.image, profile)
    }
}
