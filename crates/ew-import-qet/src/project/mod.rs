// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The importer of whole QElectroTech projects (`.qet`, doc 14 §4), lenient mode (ADR-0009, doc 14 §3.3):
//! everything that cannot be mapped becomes a report entry and never aborts the import; only a file that
//! cannot be read at all is an [`ImportError`].
//!
//! # Pipeline
//!
//! 1. **Limits** ([`crate::xml::prescan`]) over the whole project under [`ImportContext::limits`], then the
//!    whole import runs on one parser thread sized for the measured depth ([`crate::xml::with_parser_stack`]).
//! 2. **Read** (`read`): the DOM into the owned `model::QetProject`.
//! 3. **Definitions**: every embedded `<definition>` through the element reader (symbol + FnDef in the
//!    per-project "QET import" library, black box with EW-IMP-119 where no semantics resolve).
//! 4. **Pages** (`pages`): diagrams → pages, the project record, title-block lifting.
//! 5. **Elements** (`elements`): placed elements → devices, functions, representations; slaves → auxiliary
//!    functions; folio reports → interruption points; terminal elements → strips and terminals
//!    (`strips`); junction elements → junctions.
//! 6. **Conductors** (`wires`): connections between function CPs and junctions.
//! 7. **Batch**: records sorted by kind rank, then ID; library documents deduplicated by copy ID and sorted.
//!
//! The mapping table is in the crate README.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::{Id, Uuid, v5};
use ew_interop_api::{
    Code, Detection, ImportBatch, ImportContext, ImportError, ImportInput, ImportMode, ImportReport,
    Importer, Limits, LineageInfo, Severity, SourceInfo,
};
use ew_mltext::Ident;
use ew_schema::{ImportLineage, LibDocument, LineageAnchor, Project, Record, Sha256};

use crate::element::{ElementImport, ElementSource, LINEAGE_KEY, read_on_stack};

pub(crate) mod builtins;
pub(crate) mod cx;
pub(crate) mod elements;
pub(crate) mod model;
pub(crate) mod pages;
pub(crate) mod read;
pub(crate) mod strips;
pub(crate) mod wires;

use builtins::Builtins;
use cx::{Findings, Ids, MAX_PROJECT_ENTRIES};

/// The source key of the project record a project import creates when the context names no project.
pub const PROJECT_KEY: &str = "qet:project";

/// A context for importing a `.qet` into a new project: a nil project ID (the importer then mints the
/// project ID `v5(namespace, "qet:project")`, [`PROJECT_KEY`]), a new lineage, no `as_of` date, the
/// default [`Limits`] (doc 14 §11: the project limits) and [`ImportMode::Lenient`] (the QET project import is
/// lenient, doc 14 §4.16).
pub fn new_project_context() -> ImportContext {
    let mut ctx = ImportContext::new(Id::from_uuid(Uuid::nil()));
    ctx.mode = ImportMode::Lenient;
    ctx
}

/// The importer of `.qet` project files.
///
/// It carries the built-in starter documents the batch may reference ([`QetProjectImporter::with_library`]):
/// the interruption-point FnDef `ew.fn.ip` and the QET-compatible frame form. Without them, folio reports
/// are dropped (EW-IMP-003) and pages get no frame form (the derivation layer's default frame applies).
#[derive(Clone, Debug, Default)]
pub struct QetProjectImporter {
    builtins: Builtins,
}

impl QetProjectImporter {
    /// An importer without built-in documents.
    pub fn new() -> QetProjectImporter {
        QetProjectImporter::default()
    }

    /// An importer resolving built-in keys in `docs` (the starter content: `content/fndefs`,
    /// `content/forms`, `content/symbols/qet10`). Documents without a key and parts are ignored.
    pub fn with_library(docs: impl IntoIterator<Item = LibDocument>) -> QetProjectImporter {
        QetProjectImporter {
            builtins: Builtins::new(docs),
        }
    }
}

/// Whether the first start tag of `head` is `<project` (after an optional BOM, the XML declaration,
/// comments and white space).
fn root_is_project(head: &[u8]) -> bool {
    let mut rest = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    loop {
        rest = rest.trim_ascii_start();
        if let Some(r) = rest.strip_prefix(b"<?") {
            match r.windows(2).position(|w| w == b"?>") {
                Some(at) => rest = &r[at + 2..],
                None => return false,
            }
        } else if let Some(r) = rest.strip_prefix(b"<!--") {
            match r.windows(3).position(|w| w == b"-->") {
                Some(at) => rest = &r[at + 3..],
                None => return false,
            }
        } else {
            break;
        }
    }
    rest.strip_prefix(b"<project")
        .is_some_and(|r| matches!(r.first(), Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')))
}

impl Importer for QetProjectImporter {
    fn id(&self) -> &'static str {
        crate::IMPORTER_ID
    }

    fn version(&self) -> &'static str {
        crate::IMPORTER_VERSION
    }

    /// `Certain` when the root element in the head is `<project>`; `Maybe(30)` for a name ending in `.qet`;
    /// else `No`.
    fn detect(&self, head: &[u8], name: &str) -> Detection {
        if root_is_project(head) {
            Detection::Certain
        } else if name.to_ascii_lowercase().ends_with(".qet") {
            Detection::Maybe(30)
        } else {
            Detection::No
        }
    }

    fn import(&self, input: &ImportInput, ctx: &mut ImportContext) -> Result<ImportBatch, ImportError> {
        let limits = ctx.limits;
        let size = input.bytes().len() as u64;
        if size > limits.max_input_bytes {
            return Err(ImportError::LimitExceeded {
                what: "input size".into(),
                limit: limits.max_input_bytes,
            });
        }
        let bytes = input
            .bytes()
            .strip_prefix(b"\xEF\xBB\xBF")
            .unwrap_or(input.bytes());
        let xml = std::str::from_utf8(bytes).map_err(|e| ImportError::Unsupported {
            message: format!("not UTF-8 at byte {}", e.valid_up_to()),
        })?;
        let depth = crate::xml::prescan(xml, &limits)?;
        let ctx = &*ctx;
        crate::xml::with_parser_stack(depth, || self.run(xml, input, ctx, &limits))
    }
}

/// One embedded definition after the element reader.
pub(crate) struct Def {
    /// The reader's result; `None` when the definition was refused (reported).
    pub import: Option<ElementImport>,
}

impl QetProjectImporter {
    /// The import on the parser thread.
    fn run(
        &self,
        xml: &str,
        input: &ImportInput,
        ctx: &ImportContext,
        limits: &Limits,
    ) -> Result<ImportBatch, ImportError> {
        let mut findings = Findings::new(input.name(), MAX_PROJECT_ENTRIES);
        let project = read::read_project(xml, &mut findings, limits)?;

        // Lineage: the context's, else a new one anchored on the QET 0.200+ project UUID, else on the file
        // hash (doc 14 §2.3; QET 0.100 has no project UUID).
        let mut records: Vec<Record> = Vec::new();
        let project_uuid = project.uuid.as_deref().and_then(|u| Uuid::parse_str(u).ok());
        let lineage = match ctx.lineage {
            Some(l) => l,
            None => {
                let anchor = match project_uuid {
                    Some(u) => v5::LineageAnchor::QetProjectUuid(u),
                    None => v5::LineageAnchor::FirstImportSha256(input.sha256()),
                };
                let namespace = v5::lineage_namespace(crate::IMPORTER_ID, &anchor)
                    .unwrap_or_else(|_| unreachable!("\"qet\" is a valid importer name"));
                let id: Id<ImportLineage> = v5::imported_id(namespace, LINEAGE_KEY);
                records.push(Record::ImportLineage(ImportLineage {
                    id,
                    v: 1,
                    importer: Ident::from_input(crate::IMPORTER_ID)
                        .unwrap_or_else(|_| unreachable!("\"qet\" is an identifier")),
                    source_name: input.name().to_owned(),
                    anchor: match project_uuid {
                        Some(u) => LineageAnchor::QetProjectUuid(Id::from_uuid(u)),
                        None => LineageAnchor::FirstImportSha256(Sha256(input.sha256())),
                    },
                    namespace: Id::from_uuid(namespace),
                    last_source_sha256: Some(Sha256(input.sha256())),
                    last_run: None,
                    extra: BTreeMap::new(),
                }));
                LineageInfo { id, namespace }
            }
        };
        let ids = Ids {
            namespace: lineage.namespace,
            lineage: lineage.id,
        };
        let project_id: Id<Project> = if ctx.project.as_uuid() == Uuid::nil() {
            ids.id(PROJECT_KEY)
        } else {
            ctx.project
        };

        // Definitions, by collection path.
        let mut defs: BTreeMap<String, Def> = BTreeMap::new();
        for d in &project.definitions {
            if defs.contains_key(&d.path) {
                let loc = findings.loc(None, Some(d.path.clone()), Some(d.line));
                findings.push(
                    Code::MALFORMED_ITEM,
                    Severity::Warning,
                    loc,
                    "collection path given twice; the first definition is used",
                );
                continue;
            }
            let import = match xml.get(d.range.clone()) {
                Some(text) => {
                    let src = ElementSource::new(input.name(), Some(&d.path)).map_err(|e| {
                        ImportError::Unsupported {
                            message: e.to_string(),
                        }
                    })?;
                    match read_on_stack(text, &src, &lineage, Id::from_uuid(project_id.as_uuid()), limits) {
                        Ok(el) => Some(el),
                        Err(e) => {
                            let loc = findings.loc(None, Some(d.path.clone()), Some(d.line));
                            findings.push(
                                Code::MALFORMED_ITEM,
                                Severity::Error,
                                loc,
                                format!("element definition refused: {e}"),
                            );
                            None
                        }
                    }
                }
                None => None,
            };
            defs.insert(d.path.clone(), Def { import });
        }

        let mut used_builtins: BTreeSet<String> = BTreeSet::new();
        let pages = pages::map(
            &project,
            &ids,
            project_id,
            &self.builtins,
            &mut used_builtins,
            &mut findings,
        );
        let counts = wires::conductor_counts(&project);
        let mapped = elements::map(
            &elements::Inputs {
                project: &project,
                pages: &pages,
                defs: &defs,
                counts: &counts,
                ids: &ids,
                builtins: &self.builtins,
            },
            &mut used_builtins,
            &mut findings,
        );
        // The findings of the definitions that placed elements use (an unused definition in the collection
        // is not part of the drawing).
        // A font substitution is reported once per message (family), not once per definition (doc 14 §4.10).
        let mut fonts_seen: BTreeSet<&str> = BTreeSet::new();
        for path in &mapped.used_defs {
            if let Some(el) = defs.get(path).and_then(|d| d.import.as_ref()) {
                for e in &el.entries {
                    if e.code == Code::FONT_SUBSTITUTED && !fonts_seen.insert(e.message.as_str()) {
                        continue;
                    }
                    findings.entry(e.clone());
                }
            }
        }
        let wired = wires::map_conductors(&project, &mapped.registry, &ids, &mut findings);

        records.push(Record::Project(pages.project.clone()));
        records.extend(pages.records.iter().cloned());
        records.extend(mapped.records);
        records.extend(wired.connections.into_iter().map(Record::Connection));
        records.extend(wired.junctions.into_iter().map(Record::Junction));
        records.sort_by_key(|r| (r.kind().and_then(|k| k.rank()), r.id()));

        // Library: the definitions that placed elements use, deduplicated by copy ID, and the built-ins.
        let mut lib: BTreeMap<(u8, Uuid), LibDocument> = BTreeMap::new();
        for path in &mapped.used_defs {
            let Some(el) = defs.get(path).and_then(|d| d.import.as_ref()) else {
                continue;
            };
            lib.insert(
                (1, el.symbol.id.as_uuid()),
                LibDocument::Symbol(el.symbol.clone()),
            );
            if let Some(f) = &el.fndef {
                lib.insert((0, f.id.as_uuid()), LibDocument::Fndef(f.clone()));
            }
        }
        for d in self.builtins.closure(&used_builtins) {
            let key = match &d {
                LibDocument::Fndef(c) => (0, c.id.as_uuid()),
                LibDocument::Symbol(c) => (1, c.id.as_uuid()),
                LibDocument::Form(c) => (2, c.id.as_uuid()),
                _ => continue,
            };
            lib.insert(key, d);
        }
        let lib: Vec<LibDocument> = lib.into_values().collect();

        let mut report = ImportReport::new(
            crate::IMPORTER_ID,
            crate::IMPORTER_VERSION,
            SourceInfo {
                name: input.name().to_owned(),
                sha256: input.sha256(),
                size: input.bytes().len() as u64,
                format: "qet-project".to_owned(),
                format_version: project.version.clone(),
                detail: Some(format!(
                    "{} diagrams, {} embedded definitions",
                    project.diagrams.len(),
                    project.definitions.len()
                )),
            },
        );
        for r in &records {
            *report
                .stats
                .per_kind
                .entry(r.kind_name().unwrap_or("?").to_owned())
                .or_default() += 1;
        }
        for d in &lib {
            let k = d.kind().map_or("?", |k| k.name());
            *report.stats.per_kind.entry(k.to_owned()).or_default() += 1;
        }
        report.stats.lost_connections = wired.lost;
        report.stats.dropped = mapped.dropped + wired.lost;
        report.stats.inferred = mapped.inferred;
        let (entries, suppressed) = findings.finish();
        report.entries = entries;
        report.stats.suppressed = suppressed;
        Ok(ImportBatch { records, lib, report })
    }
}
