// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The import driver: inputs → tables → emitters → sorted batch.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

use ew_interop_api::{
    Code, ImportBatch, ImportContext, ImportError, ImportInput, ImportReport, Severity, SourceInfo,
};
use ew_schema::{ImportLineage, LibDocument, LineageAnchor, Record, Sha256};
use sha2::Digest;

use crate::csv::{self, CsvError, TableLimits};
use crate::emit::{self, Cx};
use crate::ids;
use crate::library::BuiltinLibrary;
use crate::schema;
use crate::table::{Findings, Table};
use crate::{IMPORTER_ID, IMPORTER_VERSION, LINEAGE_LABEL, MAX_TABLE_FILES};

/// The golden CSV bootstrap (`ew import csv`, FR-IMP-022, doc 20 §3.2 path (a)).
///
/// Unlike [`ew_interop_api::Importer`], which reads one file, it reads the set of table files of one
/// `tables/` directory, given as [`ImportInput`]s (file names only, never paths; the host reads the files).
#[derive(Clone, Debug)]
pub struct TablesImporter {
    library: BuiltinLibrary,
    limits: TableLimits,
}

impl TablesImporter {
    /// An importer resolving symbol, FnDef and form keys in `library`, under the default [`TableLimits`].
    pub fn new(library: BuiltinLibrary) -> TablesImporter {
        TablesImporter {
            library,
            limits: TableLimits::default(),
        }
    }

    /// The same importer under `limits` (per table file; `ImportContext::limits.max_input_bytes` bounds every
    /// file and their sum as well).
    pub fn with_limits(self, limits: TableLimits) -> TablesImporter {
        TablesImporter { limits, ..self }
    }

    /// The importer ID, also `ImportLineage.importer`.
    pub fn id(&self) -> &'static str {
        IMPORTER_ID
    }

    /// The importer version.
    pub fn version(&self) -> &'static str {
        IMPORTER_VERSION
    }

    /// Imports the tables `inputs` into a new golden project.
    ///
    /// The bootstrap creates the project, whose ID is `v5(ns, "project|<key>")` ([`ids::project_id`]):
    /// `ctx.project` must be nil or the derived ID. `ctx.lineage` must be absent or the golden lineage.
    ///
    /// # Errors
    ///
    /// [`ImportError::LimitExceeded`] for more than [`MAX_TABLE_FILES`] files, a file or a total above the
    /// limits, a table above the row, column or cell limits; [`ImportError::Malformed`] for a repeated file
    /// name, a missing or unusable `project.csv`, or a `ctx.project` that is neither nil nor the derived project ID;
    /// [`ImportError::Unsupported`] for another lineage.
    /// Everything else is a report entry.
    pub fn import(&self, inputs: &[ImportInput], ctx: &ImportContext) -> Result<ImportBatch, ImportError> {
        let ns = ids::golden_namespace();
        if let Some(l) = ctx.lineage
            && l.namespace != ns
        {
            return Err(ImportError::Unsupported {
                message: "the golden CSV bootstrap imports into the fixed lineage Fixed(\"golden\") only"
                    .into(),
            });
        }
        if inputs.len() > MAX_TABLE_FILES {
            return Err(ImportError::LimitExceeded {
                what: "table files".into(),
                limit: MAX_TABLE_FILES as u64,
            });
        }
        let max_total = ctx.limits.max_input_bytes;
        let mut total: u64 = 0;
        let mut by_name: BTreeMap<&str, &ImportInput> = BTreeMap::new();
        for input in inputs {
            total = total.saturating_add(input.bytes().len() as u64);
            if by_name.insert(input.name(), input).is_some() {
                return Err(ImportError::Malformed {
                    message: format!("table file {} given twice", input.name()),
                    line: None,
                });
            }
        }
        if total > max_total {
            return Err(ImportError::LimitExceeded {
                what: "input size".into(),
                limit: max_total,
            });
        }
        let limits = TableLimits {
            max_file_bytes: self.limits.max_file_bytes.min(max_total),
            ..self.limits
        };

        let mut findings = Findings::default();
        let mut tables: BTreeMap<&'static str, Table> = BTreeMap::new();
        for (name, input) in &by_name {
            let Some(stem) = name.strip_suffix(".csv") else {
                findings.at(
                    Code::DROPPED_BY_DESIGN,
                    Severity::Info,
                    name,
                    None,
                    None,
                    "not a .csv file; ignored",
                );
                continue;
            };
            let Some(spec) = schema::ALL.iter().find(|s| s.name == stem) else {
                findings.at(
                    Code::DROPPED_BY_DESIGN,
                    Severity::Warning,
                    name,
                    None,
                    None,
                    "unknown table; ignored",
                );
                continue;
            };
            match csv::parse(input.bytes(), &limits) {
                Ok(t) => {
                    if let Some(t) = Table::new(*spec, name, Arc::clone(input.bytes()), t, &mut findings) {
                        tables.insert(spec.name, t);
                    }
                }
                Err(e) => match limit_error(&e) {
                    Some(what) => {
                        return Err(ImportError::LimitExceeded {
                            what: format!("{what} of {name}"),
                            limit: limit_value(&e),
                        });
                    }
                    None => findings.at(
                        Code::MALFORMED_ITEM,
                        Severity::Error,
                        name,
                        None,
                        error_line(&e),
                        format!("table refused: {e}"),
                    ),
                },
            }
        }

        // The project row decides the project ID, which the project-local parts need.
        let project_key = tables
            .get("project")
            .and_then(|t| t.rows().next())
            .map(|r| r.key().to_owned())
            .filter(|k| !k.is_empty())
            .ok_or_else(|| ImportError::Malformed {
                message: "project.csv with a project row is required".into(),
                line: None,
            })?;
        let project_id = ids::project_id(&project_key);
        let nil = ew_ids::Id::from_uuid(ew_ids::Uuid::nil());
        if ctx.project != nil && ctx.project != project_id {
            return Err(ImportError::Malformed {
                message: format!(
                    "EW-IMP-002: the import context targets project {} but project.csv key `{project_key}` gives project {project_id}; the bootstrap creates its own project",
                    ctx.project
                ),
                line: Some(2),
            });
        }
        let mut cx = Cx::new(&self.library, project_id);
        cx.findings = findings;

        let t = |name: &str| tables.get(name);
        emit::parts::emit(
            &mut cx,
            t("parts"),
            t("part_cps"),
            t("part_templates"),
            t("cable_cores"),
        );
        emit::project::emit_structure(&mut cx, t("structure"));
        emit::project::emit_external_refs(&mut cx, t("external_refs"));
        emit::project::emit_sections(&mut cx, t("sections"));
        emit::project::emit_pages(&mut cx, t("pages"));
        emit::devices::emit_devices(&mut cx, t("devices"));
        emit::devices::emit_terminals(&mut cx, t("terminals"));
        emit::devices::emit_functions(&mut cx, t("functions"));
        emit::devices::emit_representations(&mut cx, t("representations"));
        emit::connect::register_junctions(&mut cx, t("junctions"));
        emit::connect::emit_conductors(&mut cx, t("conductors"));
        emit::connect::emit_connections(&mut cx, t("connections"));
        emit::connect::emit_junctions(&mut cx, t("junctions"));
        emit::connect::emit_potentials(&mut cx, t("potentials"));
        emit::connect::emit_bonds(&mut cx, t("bonds"));
        emit::project::emit_report_blocks(&mut cx, t("report_blocks"));
        let project = emit::project::emit_project(&mut cx, t("project"), t("parties"));
        if project.is_none() {
            return Err(ImportError::Malformed {
                message: "the project row of project.csv is unusable (see the report)".into(),
                line: None,
            });
        }
        for name in schema::KEPT_ONLY {
            if let Some(table) = tables.get(name) {
                cx.findings.at(
                    Code::DROPPED_BY_DESIGN,
                    Severity::Info,
                    &table.file,
                    None,
                    None,
                    format!(
                        "{} rows read; schema v0 has no record shape for them yet",
                        table.len()
                    ),
                );
            }
        }

        // The manifest digest: file names and SHA-256 of every input, in name order.
        let mut manifest = String::new();
        for (name, input) in &by_name {
            let _ = write!(manifest, "{name}\0");
            for b in input.sha256() {
                let _ = write!(manifest, "{b:02x}");
            }
            manifest.push('\n');
        }
        let digest: [u8; 32] = sha2::Sha256::digest(manifest.as_bytes()).into();

        cx.records.push(Record::ImportLineage(ImportLineage {
            id: ids::lineage_id(),
            v: 1,
            importer: IMPORTER_ID.parse().expect("a valid identifier"),
            source_name: "tables".into(),
            anchor: LineageAnchor::Fixed(LINEAGE_LABEL.parse().expect("a valid identifier")),
            namespace: ew_ids::Id::from_uuid(ns),
            last_source_sha256: Some(Sha256(digest)),
            last_run: None,
            extra: Default::default(),
        }));

        let mut records = std::mem::take(&mut cx.records);
        records.sort_by_key(|r| (r.kind().and_then(|k| k.rank()), r.id()));
        let mut lib = self.library.closure(&cx.used_lib);
        lib.append(&mut cx.local_lib);
        lib.sort_by_key(|d: &LibDocument| (d.kind().map(|k| k.name()), crate::library::doc_id(d)));

        let mut report = ImportReport::new(
            IMPORTER_ID,
            IMPORTER_VERSION,
            SourceInfo {
                name: "tables".into(),
                sha256: digest,
                size: total,
                format: "golden-csv".into(),
                format_version: Some("1".into()),
                detail: None,
            },
        );
        for r in &records {
            if let Some(k) = r.kind_name() {
                *report.stats.per_kind.entry(k.to_owned()).or_default() += 1;
            }
        }
        for d in &lib {
            if let Some(k) = d.kind() {
                *report.stats.per_kind.entry(k.name().to_owned()).or_default() += 1;
            }
        }
        let entries = std::mem::take(&mut cx.findings).finish();
        report.stats.dropped = entries
            .iter()
            .filter(|e| e.severity == Severity::Error && e.message.starts_with("row dropped"))
            .count() as u32;
        report.entries = entries;
        Ok(ImportBatch { records, lib, report })
    }
}

fn limit_error(e: &CsvError) -> Option<&'static str> {
    match e {
        CsvError::TooLarge { .. } => Some("file size"),
        CsvError::TooManyRows { .. } => Some("row count"),
        CsvError::TooManyCells { .. } => Some("cell count"),
        CsvError::TooManyColumns { .. } => Some("column count"),
        CsvError::CellTooLong { .. } => Some("cell size"),
        _ => None,
    }
}

fn limit_value(e: &CsvError) -> u64 {
    match e {
        CsvError::TooLarge { limit } => *limit,
        CsvError::TooManyRows { limit }
        | CsvError::TooManyCells { limit }
        | CsvError::TooManyColumns { limit, .. }
        | CsvError::CellTooLong { limit, .. } => *limit as u64,
        _ => 0,
    }
}

fn error_line(e: &CsvError) -> Option<u32> {
    match e {
        CsvError::TooManyColumns { line, .. }
        | CsvError::CellTooLong { line, .. }
        | CsvError::NotUtf8 { line }
        | CsvError::EmptyLine { line }
        | CsvError::StrayQuote { line }
        | CsvError::StrayCr { line }
        | CsvError::UnterminatedQuote { line }
        | CsvError::RaggedRow { line, .. } => Some(*line),
        _ => None,
    }
}
