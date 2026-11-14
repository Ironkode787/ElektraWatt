// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EPLAN part data as the front ends reach it (doc 14 §5.2, §5.3; FR-IMP-025 to FR-IMP-028):
//!
//! - **the reader** ([`parts_import`]): an EDZ or a parts XML through the front door into the parts of a target, by
//!   `ew-import-eplan`'s drafts and `ew-import-catalog`'s placer, the function-definition table the user may extend
//!   ([`fndef_map`]);
//! - **a library** ([`import_into_library`]): the write of [`crate::catalog::import_parts_into_library`], the terms
//!   notice's entry in its report ([`terms_entry`]);
//! - **a project** ([`Session::update_parts_from_import`]): the previewed update of the project's part copies from
//!   the parts a file holds (FR-PRT-014, AC-38). An import never creates a part in a project; the copies the file
//!   names (by manufacturer and normalised part number) are updated, field by field, through `library.update`.

use std::path::Path;

use ew_commands::commands::library_copy_hash;
use ew_derive::ew_ids::{Id, Uuid};
use ew_import_eplan::{IMPORTER_ID, IMPORTER_VERSION};
use ew_schema::{FreeProp, LibCopy, LibDocument, LibRef, Manufacturer, MechData, Part, PartKind, Sha256};

pub use ew_import_eplan::{FndefMap, PartsRead};

use crate::catalog::{
    CatalogError, CatalogTarget, Date, ImportError, ImportInput, ImportReport, LibraryImport, Limits,
    PartsImport, ReportEntry, Severity, pn_norm,
};
use crate::library_ops::{ApiError, UpdatePlan};
use crate::{Session, library};

/// The file of the user's own function-definition rows below the configuration directory's `imports` folder.
pub const FNDEF_MAP_FILE: &str = "fndef-map.toml";

/// The largest `fndef-map.toml` read (a table of a few hundred rows).
const MAX_MAP_BYTES: u64 = 1024 * 1024;

/// The table that maps EPLAN's function definitions: the shipped one, and over it the rows of
/// `<config dir>/imports/fndef-map.toml` when that file exists.
///
/// # Errors
///
/// The text of the problem: the file cannot be read, is above 1 MiB or is not a table of rows.
pub fn fndef_map(config_dir: Option<&Path>) -> Result<FndefMap, String> {
    let shipped = FndefMap::shipped();
    let Some(dir) = config_dir else {
        return Ok(shipped);
    };
    let path = dir.join(crate::catalog::MAPS_DIR).join(FNDEF_MAP_FILE);
    match std::fs::metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(shipped),
        _ => fndef_map_file(&path),
    }
}

/// The shipped table with the rows of the file `path` over it (`ew import edz --fndef-map`).
///
/// # Errors
///
/// The text of the problem: the file cannot be read, is above 1 MiB or is not a table of rows.
pub fn fndef_map_file(path: &Path) -> Result<FndefMap, String> {
    let name = path
        .file_name()
        .map_or_else(|| FNDEF_MAP_FILE.to_owned(), |n| n.to_string_lossy().into_owned());
    let len = std::fs::metadata(path).map_err(|e| format!("{name}: {e}"))?.len();
    if len > MAX_MAP_BYTES {
        return Err(format!("{name}: larger than {MAX_MAP_BYTES} bytes"));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{name}: {e}"))?;
    FndefMap::shipped()
        .with_user(&text)
        .map_err(|e| format!("{name}: {e}"))
}

/// The report entry of the Data Portal terms notice (EW-IMP-201): a front end that showed the notice before an EDZ
/// import adds it to the import's report.
pub fn terms_entry(file: &str) -> ReportEntry {
    ReportEntry::new(
        crate::import::Code::imp(201),
        Severity::Info,
        "the EPLAN Data Portal terms notice was shown before this EDZ import",
    )
    .at(crate::import::SourceLocation {
        file: file.to_owned(),
        ..Default::default()
    })
}

/// The Data Portal terms notice, in English, for the command line (the window has its own, in both languages).
pub const TERMS_NOTICE: &str = "Notice: EPLAN Data Portal terms restrict the data downloaded from the Data Portal to EPLAN products. \
ElektraWatt reads this file from your disk only, never downloads or shares it, and records its source on every part. \
Make sure your terms allow this use; prefer the manufacturer's own download.";

/// Reads `input` (an EDZ, an EPLAN parts XML or a part XML) into the parts of `target`, resolving manufacturers through
/// `master`. The file's findings and the placer's make one report, whose source is the file as `ew-import-eplan`
/// detected it.
///
/// # Errors
///
/// [`ImportError`] when the file as a whole cannot be read ([`ew_import_eplan::read_parts`]).
pub fn parts_import(
    input: &ImportInput,
    fndefs: &FndefMap,
    master: &[LibCopy<Manufacturer>],
    target: &CatalogTarget<'_>,
) -> Result<PartsImport, ImportError> {
    let read = ew_import_eplan::read_parts(input, fndefs, &target.limits)?;
    let mut report = ImportReport::new(
        IMPORTER_ID,
        IMPORTER_VERSION,
        ew_import_catalog::ew_interop_api::SourceInfo {
            name: input.name().to_owned(),
            sha256: input.sha256(),
            size: input.bytes().len() as u64,
            format: read.format.to_owned(),
            format_version: None,
            detail: None,
        },
    );
    report.entries = read.entries;
    report.stats.suppressed = read.suppressed;
    let mut placer = ew_import_catalog::Placer::new(report, input.name(), master, target);
    for draft in &read.drafts {
        placer.place(draft);
    }
    Ok(placer.finish())
}

/// Imports an EDZ or a parts XML into the writable library `library` ([`crate::catalog::import_parts_into_library`]):
/// the parts with their new manufacturers, under the library's lock, changed parts as the next patch version with
/// `bump`. `terms` adds the terms notice's entry (EW-IMP-201) to the report: the front end showed it.
///
/// # Errors
///
/// As [`crate::catalog::import_into_library`].
pub fn import_into_library(
    reg: &mut library::Registry,
    library: Uuid,
    input: &ImportInput,
    fndefs: &FndefMap,
    write: (Option<Date>, bool, bool),
    terms: bool,
) -> Result<LibraryImport, CatalogError> {
    let mut done = crate::catalog::import_parts_into_library(
        reg,
        library,
        (input.name(), input.bytes().len() as u64),
        &Limits::default(),
        write,
        &|target, master| parts_import(input, fndefs, master, target),
    )?;
    if terms {
        done.import.report.entries.insert(0, terms_entry(input.name()));
    }
    Ok(done)
}

// -------------------------------------------------------------------------------------------------------------------
// A project's part copies

/// The part copies an import updates: what matched, what did not, and the plan (a previewed `library.update`).
#[derive(Debug)]
pub struct PartUpdates {
    /// The plan of the copies the file changes; `None` when no matched copy differs.
    pub plan: Option<UpdatePlan>,
    /// The part numbers of the file the project holds a copy of, each with the copy's ID, in file order.
    pub matched: Vec<(String, Uuid)>,
    /// The part numbers of the file the project holds no copy of.
    pub unmatched: Vec<String>,
    /// The part numbers of the file that two or more of the project's copies carry: left alone.
    pub ambiguous: Vec<String>,
}

/// The held copy `held` with what the imported part `new` knows laid over it: the description, the numbers, the
/// pins, the function template, the free properties, the size, the importer's own members and its provenance. What
/// the file does not carry (technical data, category, contacts, sections, the potential roles of unchanged pins) stays.
fn merged(held: &Part, new: &Part) -> Part {
    let mut out = held.clone();
    if !new.description.is_empty() {
        out.description = new.description.clone();
    }
    if new.type_designation.is_some() {
        out.type_designation = new.type_designation.clone();
    }
    if new.order_number.is_some() {
        out.order_number = new.order_number.clone();
    }
    if new.kind != PartKind::Device {
        out.kind = new.kind.clone();
    }
    if !new.cp_pattern.is_empty() {
        let mut cps = ew_schema::KeyedList::new();
        for cp in &new.cp_pattern {
            let same = held
                .cp_pattern
                .iter()
                .find(|h| h.idx == cp.idx && h.designation == cp.designation);
            cps.insert(match same {
                Some(h) => {
                    let mut kept = h.clone();
                    kept.extra.extend(cp.extra.clone());
                    kept
                }
                None => cp.clone(),
            });
        }
        out.cp_pattern = cps;
    }
    if !new.function_template.is_empty() {
        out.function_template = new.function_template.clone();
    }
    for f in &new.free_props {
        let f: FreeProp = f.clone();
        out.free_props.insert(f);
    }
    if let Some(n) = &new.mech {
        let mut m: MechData = out.mech.clone().unwrap_or_default();
        m.width = n.width.or(m.width);
        m.height = n.height.or(m.height);
        m.depth = n.depth.or(m.depth);
        out.mech = Some(m);
    }
    out.extra.extend(new.extra.clone());
    for s in &new.meta.sources {
        if !out.meta.sources.contains(s) {
            out.meta.sources.push(s.clone());
        }
    }
    out.meta.as_of = new.meta.as_of.or(held.meta.as_of);
    if new.meta.licence.is_some() {
        out.meta.licence = new.meta.licence.clone();
    }
    out.meta.foreign_terms |= new.meta.foreign_terms;
    out
}

impl Session {
    /// Reads `input` (an EDZ, a parts XML or a part XML) for this project's part copies
    /// ([`Session::update_parts_from_import`]): its manufacturers resolve through the installed libraries and the
    /// project's own manufacturer copies, the parts are project-local ones, and nothing is written.
    ///
    /// # Errors
    ///
    /// [`crate::import::Refusal`] for a file refused as a whole or an importer that panicked.
    pub fn eplan_parts(
        &self,
        input: &ImportInput,
        fndefs: &FndefMap,
        as_of: Option<Date>,
    ) -> Result<PartsImport, crate::import::Refusal> {
        let q = self.query();
        let mut master = crate::catalog::manufacturer_master(self.libraries());
        master.extend(q.manufacturers().iter().map(|(_, c)| (**c).clone()));
        let none = |_: Uuid| None;
        let target = CatalogTarget {
            library: q.project().id.as_uuid(),
            as_of,
            bump: false,
            limits: Limits::default(),
            existing: &none,
            local_manufacturers: false,
            shared: &[],
        };
        let sizes = [input.bytes().len() as u64];
        crate::import::front_door(input.name(), &sizes, &target.limits, None, || {
            parts_import(input, fndefs, &master, &target)
        })
    }

    /// The previewed update of this project's part copies from the parts of `import` (an EDZ or parts XML read by
    /// [`parts_import`]; FR-PRT-014, AC-38): each part of the import is matched to the project's copy of the same
    /// normalised part number, laid over it ([`merged`]) as the copy's next patch version, and the plan of the
    /// differing copies is made as [`Session::update_from_documents`] makes it (the card's field diff, the devices and
    /// the BOM rows touched). A part the project holds no copy of is listed, never created; a part number two copies
    /// carry is left alone.
    ///
    /// # Errors
    ///
    /// [`ApiError::Cmd`] when the update is refused (a connected CP left unmapped, see the class of the change).
    pub fn update_parts_from_import(&self, import: &PartsImport) -> Result<PartUpdates, ApiError> {
        let q = self.query();
        let mut matched = Vec::new();
        let mut unmatched = Vec::new();
        let mut ambiguous = Vec::new();
        let mut docs: Vec<LibDocument> = Vec::new();
        for doc in &import.parts {
            let LibDocument::Part(new) = doc else {
                continue;
            };
            let number = new.item.part_number.as_str().to_owned();
            let norm = pn_norm(&number);
            let hits: Vec<_> = q
                .parts()
                .iter()
                .filter(|(_, c)| pn_norm(c.item.part_number.as_str()) == norm)
                .map(|(_, c)| c.clone())
                .collect();
            let [held] = hits.as_slice() else {
                if hits.is_empty() {
                    unmatched.push(number);
                } else {
                    ambiguous.push(number);
                }
                continue;
            };
            matched.push((number, held.id.as_uuid()));
            let item = merged(&held.item, &new.item);
            if item == held.item {
                continue;
            }
            let Some(version) = ew_import_catalog::bump_patch(&held.version) else {
                continue;
            };
            let mut copy = LibCopy {
                id: held.id,
                v: held.v,
                library_id: held.library_id,
                item_id: held.item_id,
                version,
                sha256: Sha256([0; 32]),
                item,
            };
            let hash = library_copy_hash(&LibDocument::Part(copy.clone())).unwrap_or([0; 32]);
            copy.sha256 = Sha256(hash);
            docs.push(LibDocument::Part(copy));
        }
        // The starter function definitions the new templates name, which the project does not hold yet, travel with
        // the update (`update_from_documents` embeds what an updated copy names).
        let mut named: Vec<LibRef> = Vec::new();
        for d in &docs {
            if let LibDocument::Part(c) = d {
                named.extend(c.item.function_template.iter().map(|t| t.fndef.clone()));
            }
        }
        for r in named {
            let held: Id<ew_schema::FnDef> = Id::from_uuid(
                ew_derive::ew_ids::v5::lib_copy_id::<()>(r.library.as_uuid(), r.item.as_uuid()).as_uuid(),
            );
            if q.fndefs().get(held).is_none()
                && let Ok(doc) = self.libraries().resolve(&r)
            {
                docs.push(doc);
            }
        }
        let plan = match self.update_from_documents(&docs, &[]) {
            Ok(plan) => Some(plan),
            Err(ApiError::NothingToDo) => None,
            Err(e) => return Err(e),
        };
        Ok(PartUpdates {
            plan,
            matched,
            unmatched,
            ambiguous,
        })
    }
}
