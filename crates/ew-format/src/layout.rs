// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which file a record lives in (doc 05 §2.2: the kind registry's ownership rules and anchors).

use std::collections::BTreeMap;

use ew_schema::{AnchorRef, AnyId, FileRule, Function, Kind, LibDocument, Record};
use serde_json::Value;

use crate::error::FormatError;
use crate::path::FilePath;

/// The facts about other records that an indirect anchor needs (doc 05 §2.2, anchor column). `ew-store`
/// answers them from the model; [`RecordAnchors`] from a set of records.
pub trait Anchors {
    /// The device of function `f`: `Some(Some(d))`, `Some(None)` for a function without a device (an
    /// interruption point), `None` when no function `f` exists.
    fn function_device(&self, f: AnyId) -> Option<Option<AnyId>>;
    /// The page of junction `j`, `None` when no junction `j` exists.
    fn junction_page(&self, j: AnyId) -> Option<AnyId>;
    /// The page of the representation of function `f` (an interruption point has exactly one, EW-INV-013);
    /// with several, the page of the representation with the smallest ID; `None` without one.
    fn function_rep_page(&self, f: AnyId) -> Option<AnyId>;
}

/// The file of an interruption-point function: the page of its representation; without one (a broken
/// invariant), the shard of its own ID, so that the record is still written somewhere deterministic.
fn ip_function_file(f: AnyId, anchors: &dyn Anchors) -> FilePath {
    anchors
        .function_rep_page(f)
        .map_or_else(|| FilePath::shard(f), FilePath::page)
}

/// The file of a record anchored at "the device owning" function `f` (a page-less connection, a definition
/// or bond at a connection point): that device's shard. A function without a device (an interruption
/// point) has no device, so the record follows the function into its page file; an unknown function (a
/// dangling reference, EW-CORE-008) gives the shard of the referenced ID.
fn function_anchor_file(f: AnyId, anchors: &dyn Anchors) -> FilePath {
    match anchors.function_device(f) {
        Some(Some(device)) => FilePath::shard(device),
        Some(None) => ip_function_file(f, anchors),
        None => FilePath::shard(f),
    }
}

/// The file of a record anchored at junction `j`: the junction's page; for an unknown junction, the shard
/// of the referenced ID.
fn junction_anchor_file(j: AnyId, anchors: &dyn Anchors) -> FilePath {
    anchors
        .junction_page(j)
        .map_or_else(|| FilePath::shard(j), FilePath::page)
}

/// The file a record of a known kind lives in (doc 05 §2.2):
///
/// | Kind | File |
/// |---|---|
/// | `project`, `import_lineage` | `project.jsonl` |
/// | `structure_node`, `section`, `external_ref` | `structure.jsonl` |
/// | `page` | `pages/<own id>.jsonl` |
/// | `device` | `devices/<last byte of own id>.jsonl` |
/// | `function` | the shard of its device; an interruption point (no device) the page of its representation |
/// | `representation`, `junction`, embedded `report_block` | the page file of their page |
/// | `connection` | its page file; page-less: the file of endpoint `a`'s anchor |
/// | `conductor` | the shard of its cable |
/// | `potential_def`, `bond` | the file of endpoint `at` / `a`'s anchor |
/// | page-oriented `report_block` | `report-blocks.jsonl` |
///
/// The anchor of a connection-point endpoint is the shard of the function's device, or for a function
/// without a device the function's own file; a junction endpoint's anchor is the junction's page. A
/// reference that `anchors` cannot resolve (a dangling ID) falls back to the shard of the referenced ID, and
/// an endpoint of a newer format to the shard of the record's own ID, so every record of a known kind has
/// exactly one deterministic file.
///
/// A record of a reserved or unknown kind ([`Record::Unknown`]) has a file only where its rule needs no
/// resolution ([`unknown_record_file`]); the writer otherwise keeps it in the file it was read from.
///
/// # Errors
///
/// [`FormatError::NoFile`] for a record whose file cannot be determined.
pub fn record_file(record: &Record, anchors: &dyn Anchors) -> Result<FilePath, FormatError> {
    let own = || record.id().map(AnyId::from_uuid);
    let endpoint_file = |anchor: AnchorRef| match anchor {
        AnchorRef::DeviceOfFunction(f) => Some(function_anchor_file(f.cast(), anchors)),
        AnchorRef::PageOfJunction(j) => Some(junction_anchor_file(j.cast(), anchors)),
        _ => None,
    };
    let file = match record {
        Record::Project(_) | Record::ImportLineage(_) => FilePath::project(),
        Record::StructureNode(_) | Record::Section(_) | Record::ExternalRef(_) => FilePath::structure(),
        Record::Page(r) => FilePath::page(r.id.cast()),
        Record::Device(r) => FilePath::shard(r.id.cast()),
        Record::Function(r) => match r.device {
            Some(d) => FilePath::shard(d.cast()),
            None => ip_function_file(r.id.cast(), anchors),
        },
        Record::Representation(r) => FilePath::page(r.page.cast()),
        Record::Junction(r) => FilePath::page(r.page.cast()),
        Record::Conductor(r) => FilePath::shard(r.cable.cast()),
        Record::ReportBlock(r) => match &r.embedded {
            Some(e) => FilePath::page(e.page.cast()),
            None => FilePath::report_blocks(),
        },
        Record::Connection(r) => match r.page {
            Some(p) => FilePath::page(p.cast()),
            None => endpoint_file(record.anchor_ref())
                .or_else(|| own().map(FilePath::shard))
                .ok_or_else(|| no_file(record))?,
        },
        Record::PotentialDef(_) | Record::Bond(_) => endpoint_file(record.anchor_ref())
            .or_else(|| own().map(FilePath::shard))
            .ok_or_else(|| no_file(record))?,
        Record::Unknown(value) => unknown_record_file(value).ok_or_else(|| no_file(record))?,
    };
    Ok(file)
}

fn no_file(record: &Record) -> FormatError {
    FormatError::NoFile {
        kind: record.kind_name().unwrap_or("").to_owned(),
        id: record.id().map(AnyId::from_uuid),
    }
}

/// The file of a record of a reserved or unknown kind, where the registry's rule needs nothing but the
/// record itself: a fixed root file, `imports/<own id>.jsonl`, or the page file named by a `page` member
/// for the page-owned kinds. `None` otherwise (layout, shard, exchange and conflict kinds, and kinds the
/// registry does not know): such a record stays in the file it was read from (doc 05 §10.3).
pub fn unknown_record_file(value: &Value) -> Option<FilePath> {
    let kind = Kind::from_name(value.get("$k")?.as_str()?)?;
    let member_id = |name: &str| value.get(name)?.as_str()?.parse::<AnyId>().ok();
    match kind.file_rule() {
        FileRule::ProjectFile => Some(FilePath::project()),
        FileRule::StructureFile => Some(FilePath::structure()),
        FileRule::ReportBlocksFile => Some(FilePath::report_blocks()),
        FileRule::RootFile(name) => Some(FilePath::root(name)),
        FileRule::ImportsFile => member_id("id").map(FilePath::imports),
        FileRule::PageFile => member_id("page").map(FilePath::page),
        _ => None,
    }
}

/// The directory under `lib/` of a library kind.
pub fn lib_dir(kind: Kind) -> Option<&'static str> {
    match kind.file_rule() {
        FileRule::LibraryDocument(dir) => Some(dir),
        _ => None,
    }
}

/// The kind and ID of a library document of a known kind.
pub fn lib_key(doc: &LibDocument) -> Option<(Kind, AnyId)> {
    Some(match doc {
        LibDocument::Part(d) => (Kind::Part, d.id.cast()),
        LibDocument::Symbol(d) => (Kind::Symbol, d.id.cast()),
        LibDocument::Fndef(d) => (Kind::Fndef, d.id.cast()),
        LibDocument::Form(d) => (Kind::Form, d.id.cast()),
        LibDocument::Unknown(_) => return None,
    })
}

/// The file of a library document: `lib/<dir>/<id>.json` (doc 05 §2.1); `None` for a document of an
/// unknown kind, which the writer keeps as it was read.
pub fn lib_file(doc: &LibDocument) -> Option<FilePath> {
    let (kind, id) = lib_key(doc)?;
    Some(FilePath::lib(lib_dir(kind)?, id))
}

/// [`Anchors`] over a set of records, for writing a project from records alone (an importer's batch, the
/// golden bootstrap) without a model.
#[derive(Clone, Debug, Default)]
pub struct RecordAnchors {
    function_device: BTreeMap<AnyId, Option<AnyId>>,
    junction_page: BTreeMap<AnyId, AnyId>,
    rep_page: BTreeMap<AnyId, (AnyId, AnyId)>,
}

impl RecordAnchors {
    /// The anchors of `records`.
    pub fn new<'a>(records: impl IntoIterator<Item = &'a Record>) -> RecordAnchors {
        let mut a = RecordAnchors::default();
        for record in records {
            match record {
                Record::Function(Function { id, device, .. }) => {
                    a.function_device.insert(id.cast(), device.map(|d| d.cast()));
                }
                Record::Junction(j) => {
                    a.junction_page.insert(j.id.cast(), j.page.cast());
                }
                Record::Representation(r) => {
                    let rep: AnyId = r.id.cast();
                    for f in &r.functions {
                        let entry = a.rep_page.entry(f.cast()).or_insert((rep, r.page.cast()));
                        if rep < entry.0 {
                            *entry = (rep, r.page.cast());
                        }
                    }
                }
                _ => {}
            }
        }
        a
    }
}

impl Anchors for RecordAnchors {
    fn function_device(&self, f: AnyId) -> Option<Option<AnyId>> {
        self.function_device.get(&f).copied()
    }

    fn junction_page(&self, j: AnyId) -> Option<AnyId> {
        self.junction_page.get(&j).copied()
    }

    fn function_rep_page(&self, f: AnyId) -> Option<AnyId> {
        self.rep_page.get(&f).map(|(_, page)| *page)
    }
}
