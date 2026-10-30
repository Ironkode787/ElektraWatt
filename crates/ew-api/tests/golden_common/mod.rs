// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden model (doc 20) loaded through the CSV bootstrap (`ew-import-tables`) and
//! `Session::from_records`, for the connectivity acceptance tests (WP-S1.07). The recipe is the one of
//! `tests/golden_csv.rs`.
//!
//! The bootstrap reads `colour_profile.csv` and `size_profile.csv` but does not emit them yet (decision log
//! ID-90). [`with_profiles`] adds them to `ProjectSettings.extra` in the doc 03 §5.1 shape, as the importer
//! will once it emits them, so that the wire-property rules of doc 20 §7.1–§7.2 can be tested.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::{CmdLog, Session, SessionOptions};
use ew_derive::ew_ids::Id;
use ew_derive::{DeviceTag, PageOrder, PotentialPartition, PropSource, Q, WirePropsOf, XrefTextOf};
use ew_import_tables::ew_interop_api::{ImportBatch, ImportInput};
use ew_import_tables::{BuiltinLibrary, TablesImporter, bootstrap_context};
use ew_reports::{BlockRows, RowSet, RowSetStatus};
use ew_schema::{CircuitClass, Device, LibDocument, Record, ReportBlock, Representation};

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    paths.sort();
    paths
}

pub fn library() -> BuiltinLibrary {
    let mut docs = Vec::new();
    for sub in ["symbols/qet10", "fndefs", "forms"] {
        for p in files(&repo().join("content").join(sub), "json") {
            docs.push(LibDocument::from_json(&std::fs::read_to_string(&p).unwrap()).unwrap());
        }
    }
    BuiltinLibrary::new(docs).unwrap()
}

/// The golden import batch.
pub fn batch() -> ImportBatch {
    let inputs: Vec<ImportInput> = files(&repo().join("golden/J-E-2416/tables"), "csv")
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            ImportInput::new(&name, std::fs::read(p).unwrap()).unwrap()
        })
        .collect();
    TablesImporter::new(library())
        .import(&inputs, &bootstrap_context())
        .expect("the golden tables import")
}

/// The golden import batch in `mode`, with the text `from` (which occurs once) replaced by `to` in the table
/// `file` (D-17: a core colour "BR").
pub fn batch_with(
    file: &str,
    from: &str,
    to: &str,
    mode: ew_import_tables::ew_interop_api::ImportMode,
) -> Result<ImportBatch, String> {
    let mut inputs = Vec::new();
    for p in files(&repo().join("golden/J-E-2416/tables"), "csv") {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let mut bytes = std::fs::read(&p).unwrap();
        if name == file {
            let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
            if text.matches(from).count() != 1 {
                return Err(format!("{file}: `{from}` does not occur exactly once"));
            }
            bytes = text.replace(from, to).into_bytes();
        }
        inputs.push(ImportInput::new(&name, bytes).map_err(|e| e.to_string())?);
    }
    let mut ctx = bootstrap_context();
    ctx.mode = mode;
    TablesImporter::new(library())
        .import(&inputs, &ctx)
        .map_err(|e| format!("{e:?}"))
}

/// A session over the batch; `edit` may change the project record and the record list first.
pub fn session_with(
    batch: &ImportBatch,
    edit: impl FnOnce(&mut ew_schema::Project, &mut Vec<Record>),
) -> Session {
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let mut project = project.expect("the batch holds the project");
    edit(&mut project, &mut records);
    Session::from_records(project, records, batch.lib.clone(), SessionOptions::default())
        .expect("the golden batch passes the model's integrity check")
}

/// A session over the batch with the oracle on: every memoised read is also computed cold and compared
/// ([`ew_api::MemoMode::Oracle`]; read the mismatches with `take_oracle_mismatches`).
pub fn session_oracle(batch: &ImportBatch) -> Session {
    session_oracle_with(batch, |_, _| {})
}

/// [`session_oracle`] with the project record and the record list edited first.
pub fn session_oracle_with(
    batch: &ImportBatch,
    edit: impl FnOnce(&mut ew_schema::Project, &mut Vec<Record>),
) -> Session {
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let mut project = project.expect("the batch holds the project");
    edit(&mut project, &mut records);
    let options = SessionOptions {
        memo: ew_api::MemoMode::Oracle,
        ..SessionOptions::default()
    };
    Session::from_records(project, records, batch.lib.clone(), options)
        .expect("the golden batch passes the model's integrity check")
}

/// A session over the batch as imported (the bootstrap emits the colour and size profiles since cycle 12).
pub fn session(batch: &ImportBatch) -> Session {
    session_with(batch, |_, _| {})
}

/// A session over the batch without the colour and size profiles (the built-in defaults apply).
pub fn session_without_profiles(batch: &ImportBatch) -> Session {
    session_with(batch, |p, _| {
        p.settings.extra.remove("colour_profile");
        p.settings.extra.remove("size_profile");
    })
}

/// A session with the golden colour and size profiles in `ProjectSettings.extra`.
pub fn session_with_profiles(batch: &ImportBatch) -> Session {
    session_with(batch, |p, _| with_profiles(p))
}

/// The rows of one committed golden table by key: column name → cell (plain split, quoted fields kept
/// with their quotes stripped).
pub fn table(name: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let text = std::fs::read_to_string(repo().join("golden/J-E-2416/tables").join(name)).unwrap();
    let mut lines = text.lines();
    let header: Vec<String> = split(lines.next().unwrap());
    lines
        .map(|l| {
            let row: BTreeMap<String, String> = header.iter().cloned().zip(split(l)).collect();
            (row["key"].clone(), row)
        })
        .collect()
}

fn split(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Adds `colour_profile` and `size_profile` to `project`. The bootstrap emits both members since cycle 12
/// (`ew-import-tables`, FT-159), so on a project of the golden batch this changes nothing; it restores them on
/// a project that lost them.
pub fn with_profiles(project: &mut ew_schema::Project) {
    let (colour, size) = profiles_json();
    project.settings.extra.insert("colour_profile".into(), colour);
    project.settings.extra.insert("size_profile".into(), size);
}

/// The two profile members of the golden batch's project settings (doc 03 §5.1, doc 08 §4.2) as JSON.
pub fn profiles_json() -> (serde_json::Value, serde_json::Value) {
    for r in batch().records {
        if let Record::Project(p) = r {
            let member = |name: &str| {
                p.settings
                    .extra
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| panic!("the golden project has a `{name}` settings member"))
            };
            return (member("colour_profile"), member("size_profile"));
        }
    }
    panic!("the golden batch holds the project")
}

/// The source key (`connections|W29`, `functions|-102F2`, …) of a record.
pub fn source_key(r: &Record) -> Option<&str> {
    let s = match r {
        Record::Representation(x) => x.source_ref.as_ref(),
        Record::Connection(x) => x.source_ref.as_ref(),
        Record::Device(x) => x.source_ref.as_ref(),
        Record::Function(x) => x.source_ref.as_ref(),
        Record::Page(x) => x.source_ref.as_ref(),
        Record::Junction(x) => x.source_ref.as_ref(),
        Record::PotentialDef(x) => x.source_ref.as_ref(),
        Record::Bond(x) => x.source_ref.as_ref(),
        Record::Conductor(x) => x.source_ref.as_ref(),
        Record::ReportBlock(x) => x.source_ref.as_ref(),
        Record::StructureNode(x) => x.source_ref.as_ref(),
        Record::ExternalRef(x) => x.source_ref.as_ref(),
        Record::LogicalNet(x) => x.source_ref.as_ref(),
        Record::PhysicalNet(x) => x.source_ref.as_ref(),
        _ => None,
    }?;
    Some(s.key.as_str())
}

/// Every record's ID (as text) by its source key, such as `"connections|W29"` → the connection ID.
pub fn ids_by_key(batch: &ImportBatch) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for r in &batch.records {
        let Some(k) = source_key(r) else { continue };
        let v = serde_json::to_value(r).unwrap();
        if let Some(id) = v.get("id").and_then(|i| i.as_str()) {
            out.insert(k.to_owned(), id.to_owned());
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// Report block rows (shared by the connectivity report tests and the acceptance runner)

/// The ID of the report block with the source key `report_blocks|{key}`.
pub fn block(batch: &ImportBatch, key: &str) -> Id<ReportBlock> {
    let want = format!("report_blocks|{key}");
    batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if source_key(r) == Some(want.as_str()) => Some(b.id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no report block `{key}`"))
}

/// A `Q` over the session's snapshot without the report engine.
pub fn plain(s: &Session) -> Q {
    Q::new(&s.world().model)
}

/// One row as printed texts (neutral language, no page).
#[allow(unused_imports)]
pub use ew_reports::csv::row_texts as texts;

/// The derived rows of a report block (the block must derive).
pub fn rows_of(q: &Q, batch: &ImportBatch, key: &str) -> std::sync::Arc<RowSet> {
    let set = q.get::<BlockRows>(block(batch, key));
    assert_eq!(set.status, RowSetStatus::Derived, "block {key}");
    set
}

/// The CSV writer of the golden dialect: the one `ew export` uses (`ew_reports::csv`).
#[allow(unused_imports)]
pub use ew_reports::csv::{csv_field, rows_csv};

// ---------------------------------------------------------------------------------------------------------
// Effective wire properties

pub fn csa_text(csa: &ew_schema::Csa) -> String {
    match csa {
        ew_schema::Csa::Mm2(d) => serde_json::to_string(d).unwrap().trim_matches('"').to_owned(),
        other => format!("{other:?}"),
    }
}

/// `["GN","YE"]` as `GNYE`.
pub fn colour_text(c: &ew_schema::ColourSpec) -> String {
    serde_json::to_string(c)
        .unwrap()
        .replace(['[', ']', '"', ','], "")
}

/// The effective properties of a wire: colour text, colour source, csa text, csa source, class.
pub type Props = (String, PropSource, String, PropSource, Option<CircuitClass>);

pub fn all_props(q: &Q) -> BTreeMap<String, Props> {
    let part = q.get::<PotentialPartition>(());
    let mut out = BTreeMap::new();
    for p in part.potentials() {
        let map = q.get::<WirePropsOf>(p.id);
        for (id, e) in &map.wires {
            let colour = e.colour.as_ref().expect("a colour");
            let csa = e.csa.as_ref().expect("a cross-section");
            let entry = (
                colour_text(&colour.value),
                colour.source.clone(),
                csa_text(&csa.value),
                csa.source.clone(),
                e.class.clone(),
            );
            if let Some(prev) = out.insert(id.to_string(), entry.clone()) {
                assert_eq!(
                    prev, entry,
                    "wire {id} has different properties under different potentials"
                );
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// Command-log variants and what a replay changes (Golden B, doc 20 §2.2)

pub fn variant(name: &str) -> PathBuf {
    repo().join("golden/J-E-2416/variants").join(name)
}

/// Replays a variant file; the number of lines that committed.
pub fn run(s: &mut Session, name: &str) -> usize {
    let text = std::fs::read_to_string(variant(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    run_log(s, &text, name).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Replays command-log text as origin `cmdlog:<file>`; the number of lines that committed.
pub fn run_log(s: &mut Session, text: &str, file: &str) -> Result<usize, String> {
    let log = CmdLog::parse(text).map_err(|e| e.to_string())?;
    s.run_script(&log, file)
        .map(|r| r.len())
        .map_err(|e| e.to_string())
}

/// The printed (short) tag of every device, by device ID.
pub fn tags(s: &Session) -> BTreeMap<Id<Device>, String> {
    let q = s.query();
    q.devices()
        .ids()
        .map(|id| {
            let tag = q.get::<DeviceTag>(id);
            (id, tag.as_ref().as_ref().expect("a tag").short())
        })
        .collect()
}

/// The printed cross-reference text of every representation that has one.
pub fn xrefs(s: &Session) -> BTreeMap<Id<Representation>, String> {
    let q = s.query();
    q.reps()
        .ids()
        .filter_map(|id| {
            let t = q.get::<XrefTextOf>(id);
            (!t.text.is_empty()).then(|| (id, t.text.clone()))
        })
        .collect()
}

/// The folio labels of `page_order` in order.
pub fn folios(s: &Session) -> Vec<String> {
    let q = s.query();
    q.get::<PageOrder>(())
        .entries
        .iter()
        .map(|e| e.folio.clone())
        .collect()
}

/// The folio number of the page a representation is on.
pub fn folio_of(s: &Session, rep: Id<Representation>) -> u32 {
    let q = s.query();
    let page = q.reps().get(rep).expect("a representation").page;
    q.pages().get(page).expect("its page").folio.number
}
