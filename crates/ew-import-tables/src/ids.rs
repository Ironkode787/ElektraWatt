// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The identity rule of the golden bootstrap (doc 20 §3.2, doc 14 §2.3, ADR-0004 amendment 4).
//!
//! Every record of table row `{key}` in table `{table}` has the ID `v5(ns, "{table}|{key}")`, where `ns` is the
//! namespace of the fixed import lineage `Fixed("golden")` of importer `csv`, and carries
//! `source_ref = {lineage, key: "{table}|{key}"}`.

use ew_ids::v5::{self, LineageAnchor};
use ew_ids::{Id, Uuid};
use ew_schema::{ImportLineage, Project, SourceRef};

use crate::{IMPORTER_ID, LINEAGE_LABEL};

/// The namespace of the golden lineage: `lineage_namespace("csv", Fixed("golden"))`, i.e.
/// `v5(NS_IMPORT, "csv␟fixed␟golden")`.
pub fn golden_namespace() -> Uuid {
    v5::lineage_namespace(IMPORTER_ID, &LineageAnchor::Fixed(LINEAGE_LABEL))
        .expect("the importer ID is a valid lineage importer name")
}

/// The source key `"{table}|{key}"` of a table row.
pub fn source_key(table: &str, key: &str) -> String {
    format!("{table}|{key}")
}

/// The record ID of row `key` of `table`: `v5(golden_namespace(), "{table}|{key}")`.
pub fn record_id<T>(table: &str, key: &str) -> Id<T> {
    v5::imported_id(golden_namespace(), &source_key(table, key))
}

/// The ID of the golden import lineage record: `v5(ns, "import_lineage|golden")`.
pub fn lineage_id() -> Id<ImportLineage> {
    record_id("import_lineage", LINEAGE_LABEL)
}

/// The ID of the golden project, the row `key` of `project.csv`: `v5(ns, "project|{key}")`.
pub fn project_id(key: &str) -> Id<Project> {
    record_id("project", key)
}

/// The provenance of row `key` of `table`.
pub fn source_ref(table: &str, key: &str) -> SourceRef {
    SourceRef {
        lineage: lineage_id(),
        key: source_key(table, key),
        extra: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_namespace_is_the_documented_one() {
        // doc 14 §2.3: v5(NS_IMPORT, "csv␟fixed␟golden").
        assert_eq!(
            golden_namespace(),
            Uuid::new_v5(&v5::NS_IMPORT, "csv\u{1f}fixed\u{1f}golden".as_bytes())
        );
    }

    #[test]
    fn record_ids_hash_table_and_key() {
        let id: Id<()> = record_id("devices", "-101F1");
        assert_eq!(id.as_uuid(), Uuid::new_v5(&golden_namespace(), b"devices|-101F1"));
        assert_eq!(source_ref("devices", "-101F1").key, "devices|-101F1");
    }
}
