// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts import from CSV and XLSX (MVP), BMEcat and ETIM xChange (1.x), behind `ew-interop-api`.
//!
//! WP-S1.19 part 4 (D11, FR-PRT-016): one general parts import for every supplier list.
//!
//! - **The column map** ([`ColumnMap`]): `member ← header` for the members `manufacturer`, `part_number`,
//!   `type_designation`, `order_number`, `gtin`, `description` (one column per language), `kind`, `unit`,
//!   `tech_data.<key>` and `free_props.<key>`, and an XLSX sheet by name; saved per source as a TOML file whose text
//!   this crate reads and prints ([`ColumnMap::from_toml`], [`ColumnMap::to_toml`], [`check_source_key`]).
//! - **The tables**: CSV (UTF-8 with or without a byte-order mark, `,` or `;` found from the header line) and XLSX
//!   (its own reader over miniz_oxide and quick-xml: the first or the named sheet, each value as its displayed text, a
//!   formula's cached value; every unpacked entry capped, the shared strings bounded by the loader limits),
//!   told apart by content ([`SourceFormat::detect`]).
//! - **[`parse_parts`]**: the rows into part documents of a target library (a writable library, or the project for
//!   project-local copies), deduplicated on (manufacturer key, [`pn_norm`]), the manufacturer resolved through the
//!   master list it is given (aliases and brands reported), with a row report of new, updated, unchanged, duplicate
//!   and rejected rows. "A / B" part numbers give two flagged candidates (FR-PRT-004).
//!
//! The importer is pure (doc 14 §2.1): it reads only the bytes it is handed, never the file system, the clock or the
//! network; the import date is a parameter. It never writes: the host writes the parts into a library (a front-end
//! operation, `ew lib import parts`) or dispatches `parts.import` for a project.
//!
//! Depends on `ew-interop-api` only (ARCH §3 rule 4); the record types come through its `ew_schema` and `ew_ids`
//! re-exports.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod map;
mod parts;
mod table;
mod xlsx;

/// The importer contract this crate implements, re-exported for the hosts (`ImportInput`, `ImportReport`, ...).
pub use ew_interop_api;
pub use map::{ColumnMap, MapError, Member, check_source_key};
pub use parts::{
    CatalogTarget, FIRST_VERSION, IMPORTER_ID, IMPORTER_VERSION, PartsImport, RowCounts, RowOutcome,
    RowStatus, manufacturer_item, namespace, parse_parts, part_item, pn_norm,
};
/// The readers of the parts import as the table round trip uses them: a
/// table of a CSV or an XLSX sheet as header and cell texts ([`sheet::read_sheet`]), every record of one whole, without a
/// header or a cut to its width ([`sheet::read_rows`], the round trip's key/value sheets), and a workbook's sheet names,
/// hidden ones included ([`sheet::sheet_names`]). No second reader: these are the readers [`parse_parts`] reads with.
pub mod sheet {
    pub use crate::table::{SheetTable, read_rows, read_sheet, sheet_names};
}

pub use table::{MAX_CELL_BYTES, SourceFormat, TableColumn, table_columns, table_limits};
