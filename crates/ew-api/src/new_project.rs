// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New projects (FT-307, FR-PRJ-009, FR-PRJ-010; doc 07 §2.8, doc 05 §5.5): [`Session::new_project`] starts a
//! session over a project that has no directory yet, from the built-in basic project or from a company basic
//! project directory. The front-end writes it with [`Session::create`] (Save As into an empty directory).
//!
//! **The built-in basic project** holds the project record with the shipped defaults (the derivation profile of
//! this release, the identifier set EN 81346-2:2009, ADR-0026) and one empty page, folio 1, on the built-in
//! frame. **A company basic project** is any project directory of kind [`NewProjectKind::Basic`] (the
//! company-standard wizard writes one): its records are copied with new identities, `v5(new project ID, source
//! ID)` per record (doc 07 §2.8; library copies keep theirs, their IDs derive from library and item), and the
//! project record keeps its schemes, profiles and settings under a new project ID and the requested kind. A
//! reference between records follows the new IDs. Unknown records are carried over the same way.
//!
//! A project made from a directory records `Project.basic_project = {id, generation}`: the basic project's project
//! ID and its manifest generation when it was copied (D14, doc 07 §2.8; WP-S1.19 part 4). The built-in basic
//! project is no directory and records nothing.
//!
//! **Save as basic project** (FR-MAC-011, D14): [`Session::save_as_basic`] writes the session's model as a new
//! basic project directory under new identities, optionally without its pages, its devices or both.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ew_commands::{AnyCommand, CmdError};
use ew_derive::ew_ids::{Id, IdSource, Uuid, V7Generator, v5};
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_units::Grid;
use ew_model::{Asset, Model};
use ew_schema::{
    AssetFormat, AssetRef, BasicProjectRef, Extra, Folio, Page, PageType, Project, ProjectKind,
    ProjectSettings, Record, SchemaError, StructureAssignment,
};
use ew_store::ew_format::{self, EntryKind, Limits, Manifest};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{OpenError, Store, StoreOptions};
use serde_json::Value;

use crate::{Session, SessionOptions};

/// Where a new project starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjectTemplate<'a> {
    /// The built-in basic project.
    Builtin,
    /// A company basic project: a project directory, which is only read (no lock, no journal).
    Directory(&'a Path),
}

/// What kind of project [`Session::new_project`] makes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NewProjectKind {
    /// A schematic project, the result of "new project".
    #[default]
    Schematic,
    /// A basic project (`Project.kind = Basic`, FR-PRJ-026): what the company-standard wizard builds on.
    Basic,
}

/// The derivation profile a new project pins (the profile of this release, FR-PRJ-011).
const DERIVATION_PROFILE: &str = "2027.1";

/// The identifier set of the shipped defaults (ADR-0026).
const IDENTIFIER_SET: &str = "EN81346-2:2009";

/// Lends a `&mut dyn IdSource` to a [`V7Generator`].
struct ByRef<'s>(&'s mut (dyn IdSource + Send));

impl IdSource for ByRef<'_> {
    fn now(&mut self) -> ew_derive::ew_ids::UnixTime {
        self.0.now()
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), ew_derive::ew_ids::EntropyError> {
        self.0.fill(buf)
    }
}

fn kind_of(kind: NewProjectKind) -> ProjectKind {
    match kind {
        NewProjectKind::Schematic => ProjectKind::Schematic,
        NewProjectKind::Basic => ProjectKind::Basic,
    }
}

impl Session {
    /// A session over a new project, not yet in a directory: from the built-in basic project or from a company
    /// basic project directory (module documentation). The project ID is a new UUIDv7 from `options.ids` (the
    /// system clock and entropy when it is `None`). The session holds no store: write it with
    /// [`Session::create`], which marks it saved.
    ///
    /// # Errors
    ///
    /// [`OpenError`] when a basic project directory cannot be read (`Io`, `NotADirectory`, `NoProject`, `Format`)
    /// or its records do not form a model (`Model`).
    pub fn new_project(
        template: ProjectTemplate<'_>,
        kind: NewProjectKind,
        mut options: SessionOptions,
    ) -> Result<Session, OpenError> {
        let id: Id<Project> = match options.ids.as_mut() {
            Some(ids) => V7Generator::new(ByRef(ids.as_mut())).next(),
            None => Id::new_v7(),
        };
        let (project, records, lib, assets) = match template {
            ProjectTemplate::Builtin => {
                let (project, records, lib) = builtin(id, kind);
                (project, records, lib, Vec::new())
            }
            ProjectTemplate::Directory(dir) => from_directory(dir, id, kind)?,
        };
        let mut model = Model::from_records(project, records, lib).map_err(|error| OpenError::Model {
            error,
            findings: Vec::new(),
        })?;
        // The basic project's assets (its company logo) come along unchanged: they are named by content.
        for asset in assets {
            model.put_asset(asset);
        }
        Ok(Session::new(model, options))
    }

    /// Save as basic project (FR-MAC-011, D14): writes the session's model into `dir` (a directory without a
    /// project) as a new project of kind [`NewProjectKind::Basic`] under a new project ID (a UUIDv7 of the system
    /// clock) and new record IDs, as [`Session::new_project`] copies a directory. Schemes, profiles, frames and
    /// forms, sections, structure, report blocks and library copies are kept; `omit_pages` leaves out every page
    /// (and what the pages own), `omit_devices` every device, function, representation and connection, and the
    /// junctions, conductors and bonds of the wiring. The omission runs `edit.delete` on a scratch session over a
    /// copy of the model, so the delete policies decide what goes along (a report block embedded in a page goes
    /// with the page; a block that lists omitted devices, such as the terminal diagram of one strip, stays and
    /// loses them from its list); the session itself, its history and its project directory are not touched. The
    /// new project records no `basic_project`.
    ///
    /// # Errors
    ///
    /// [`SaveAsBasicError::Omit`] when the delete is refused (a forbid link); [`SaveAsBasicError::Write`] when
    /// `dir` holds a project, the lock is held or the write fails.
    pub fn save_as_basic(
        &self,
        dir: &Path,
        omit_pages: bool,
        omit_devices: bool,
        store: StoreOptions,
    ) -> Result<(), SaveAsBasicError> {
        let mut scratch = Session::new(self.engine.model().clone(), SessionOptions::default());
        let targets: Vec<Value> = {
            let q = scratch.query();
            let mut targets = Vec::new();
            let mut add = |kind: &str, ids: Vec<Uuid>| {
                targets.extend(
                    ids.into_iter()
                        .map(|id| serde_json::json!({"kind": kind, "id": id.hyphenated().to_string()})),
                );
            };
            if omit_devices {
                add("device", q.devices().ids().map(|i| i.as_uuid()).collect());
                add("function", q.functions().ids().map(|i| i.as_uuid()).collect());
                add("representation", q.reps().ids().map(|i| i.as_uuid()).collect());
                add("connection", q.connections().ids().map(|i| i.as_uuid()).collect());
                add("junction", q.junctions().ids().map(|i| i.as_uuid()).collect());
                add("conductor", q.conductors().ids().map(|i| i.as_uuid()).collect());
                add("bond", q.bonds().ids().map(|i| i.as_uuid()).collect());
            }
            if omit_pages {
                add("page", q.pages().ids().map(|i| i.as_uuid()).collect());
            }
            targets
        };
        if !targets.is_empty() {
            let delete = AnyCommand {
                name: "edit.delete".into(),
                params: serde_json::json!({ "targets": targets }),
            };
            let preview = scratch.preview(&delete).map_err(SaveAsBasicError::Omit)?;
            scratch.commit(preview).map_err(SaveAsBasicError::Omit)?;
        }
        // The scratch model as files in memory, read back as a template: the copy of a directory, exactly.
        let mem = Arc::new(MemVfs::new(DirSync::Implicit));
        let mut in_memory = store.clone();
        in_memory.vfs = mem.clone();
        in_memory.state_dir = None;
        let root = Path::new("/basic.ew");
        Store::create(root, scratch.engine.model(), in_memory)
            .map_err(SaveAsBasicError::Write)?
            .close();
        let (template, _) = read_records_in_memory(&mem.files(root)).map_err(SaveAsBasicError::Write)?;
        let (mut project, records, lib, assets) =
            copy_template(template, Id::new_v7(), NewProjectKind::Basic).map_err(SaveAsBasicError::Write)?;
        project.basic_project = None;
        let mut model = Model::from_records(project, records, lib).map_err(|error| {
            SaveAsBasicError::Write(OpenError::Model {
                error,
                findings: Vec::new(),
            })
        })?;
        for asset in assets {
            model.put_asset(asset);
        }
        Store::create(dir, &model, store)
            .map_err(SaveAsBasicError::Write)?
            .close();
        Ok(())
    }
}

/// Why [`Session::save_as_basic`] wrote nothing.
#[derive(Debug)]
pub enum SaveAsBasicError {
    /// The pages or devices to leave out cannot be deleted (a forbid link).
    Omit(CmdError),
    /// The basic project cannot be written (or the copy cannot be read back).
    Write(OpenError),
}

impl std::fmt::Display for SaveAsBasicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveAsBasicError::Omit(e) => write!(f, "the pages or devices cannot be left out: {e}"),
            SaveAsBasicError::Write(e) => write!(f, "the basic project cannot be written: {e}"),
        }
    }
}

impl std::error::Error for SaveAsBasicError {}

/// The built-in basic project: the record with the shipped defaults and the empty first page.
fn builtin(id: Id<Project>, kind: NewProjectKind) -> (Project, Vec<Record>, Vec<ew_schema::LibDocument>) {
    let project = Project {
        id,
        v: 1,
        master: ew_schema::MasterData::default(),
        parties: Vec::new(),
        supply: None,
        control_voltage: None,
        rated_consumption: None,
        settings: ProjectSettings {
            identifier_set: IDENTIFIER_SET.parse().ok(),
            ..ProjectSettings::default()
        },
        languages: None,
        derivation_profile: DERIVATION_PROFILE
            .parse()
            .unwrap_or_else(|_| unreachable!("the profile name is an identifier")),
        kind: kind_of(kind),
        source_ref: None,
        basic_project: None,
        extra: Extra::new(),
    };
    let page = Page {
        id: v5::imported_id(id.as_uuid(), "ew.builtin.page.1"),
        v: 1,
        folio: Folio {
            section: None,
            number: 1,
            label: None,
            extra: Extra::new(),
        },
        kind: PageType::MultiLine,
        structure: StructureAssignment::default(),
        form: None,
        title: MlText::default(),
        description_override: None,
        scale: ew_schema::Scale::default(),
        grid: Grid::QET_10PX,
        source_ref: None,
        extra: Extra::new(),
    };
    (project, vec![Record::Page(page)], Vec::new())
}

/// The project directory's files below `root`, as the format's listing: project-relative paths with `/`.
fn list_files(root: &Path) -> Result<BTreeMap<String, EntryKind>, OpenError> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, EntryKind>) -> Result<(), OpenError> {
        let io = |path: &Path, error| OpenError::Io {
            path: path.to_path_buf(),
            error,
        };
        for entry in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
            let entry = entry.map_err(|e| io(dir, e))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|e| io(&path, e))?;
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let name = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            if kind.is_dir() {
                out.insert(name, EntryKind::Dir);
                walk(root, &path, out)?;
            } else if kind.is_file() {
                out.insert(name, EntryKind::File);
            } else if kind.is_symlink() {
                out.insert(name, EntryKind::Link);
            } else {
                out.insert(name, EntryKind::Other);
            }
        }
        Ok(())
    }
    if !root.is_dir() {
        return Err(OpenError::NotADirectory(root.to_path_buf()));
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

/// The records, library documents and assets of a basic project directory.
type Template = (Project, Vec<Record>, Vec<ew_schema::LibDocument>, Vec<Asset>);

/// Reads the typed records of a project directory without opening a store: no lock, no journal. A directory whose
/// files the format reader could not read in full is an error, not a partial template. Its assets are the asset
/// files whose bytes match their name and sniff as the format of their extension (doc 05 §13.3). Also returns the
/// manifest's generation (0 without a usable manifest).
fn read_records(dir: &Path) -> Result<(Template, u64), OpenError> {
    let limits = Limits::default();
    let listing = list_files(dir)?;
    let manifest_path = dir.join("manifest.json");
    let manifest = match std::fs::read(&manifest_path) {
        Ok(bytes) => Manifest::parse(&bytes, &limits).ok(),
        Err(_) => None,
    };
    let plan = ew_format::plan(manifest.as_ref(), &listing, &limits);
    let mut files = BTreeMap::new();
    for path in plan.read {
        let full: PathBuf = dir.join(path.as_str());
        let bytes = std::fs::read(&full).map_err(|error| OpenError::Io { path: full, error })?;
        files.insert(path, bytes);
    }
    load_template(manifest.as_ref(), &files, plan.findings, &limits)
}

/// Reads the typed records of a project held in memory: `all` maps project-relative paths to their bytes (what
/// [`ew_store::mem::MemVfs::files`] gives after a [`Store::create`]).
fn read_records_in_memory(all: &BTreeMap<String, Vec<u8>>) -> Result<(Template, u64), OpenError> {
    let limits = Limits::default();
    let manifest = all
        .get("manifest.json")
        .and_then(|bytes| Manifest::parse(bytes, &limits).ok());
    let listing: BTreeMap<String, EntryKind> = all.keys().map(|k| (k.clone(), EntryKind::File)).collect();
    let plan = ew_format::plan(manifest.as_ref(), &listing, &limits);
    let mut files = BTreeMap::new();
    for path in plan.read {
        let Some(bytes) = all.get(path.as_str()) else {
            return Err(OpenError::Format(format!(
                "the file {} of the basic project is missing",
                path.as_str()
            )));
        };
        files.insert(path, bytes.clone());
    }
    load_template(manifest.as_ref(), &files, plan.findings, &limits)
}

/// The template of the files a read plan named, and the manifest's generation.
fn load_template(
    manifest: Option<&Manifest>,
    files: &BTreeMap<ew_format::FilePath, Vec<u8>>,
    mut findings: Vec<ew_format::Finding>,
    limits: &Limits,
) -> Result<(Template, u64), OpenError> {
    let loaded = ew_format::load(manifest, files, limits);
    let unreadable = loaded.read_only;
    findings.extend(loaded.findings);
    if unreadable || findings.iter().any(|f| f.kind.forces_read_only()) {
        let first = findings
            .iter()
            .find(|f| f.kind.forces_read_only())
            .map_or_else(String::new, |f| format!(": {}", f.message));
        return Err(OpenError::Format(format!(
            "the basic project has files that cannot be read in full{first}"
        )));
    }
    let Some(project) = loaded.project else {
        return Err(OpenError::NoProject(findings));
    };
    let assets = loaded
        .opaque
        .iter()
        .filter_map(|(path, hash)| template_asset(path.as_str(), hash, files.get(path)?))
        .collect();
    let generation = manifest.map_or(0, |m| m.generation);
    Ok(((project, loaded.records, loaded.lib, assets), generation))
}

/// The asset of the file `path` with `bytes` hashing to `hash`, when it is an asset file of a known format whose name
/// is its hash.
fn template_asset(path: &str, hash: &[u8; 32], bytes: &[u8]) -> Option<Asset> {
    let ew_format::PathClass::Opaque(ew_format::OpaqueKind::Asset { sha256 }) = ew_format::classify(path)
    else {
        return None;
    };
    let format = AssetFormat::from_ext(path.rsplit_once('.')?.1)?;
    let (sniffed, size) = ew_schema::sniff(bytes).ok()?;
    (sha256 == *hash && sniffed == format).then(|| {
        Asset::new(
            AssetRef {
                sha256: ew_schema::Sha256(*hash),
                format,
                size,
            },
            bytes,
        )
    })
}

/// The most unsorted sets one record may need put in order again.
const MAX_SORTS: usize = 64;

/// Reads `value` back as a record. A set of IDs that the replacement left out of order (a set is sorted by its
/// elements' JSON text, doc 05 section 3.2) is put in order where the typed reader says so, and nowhere else:
/// an ordered list (the arms of a junction, the functions of a representation) keeps its order.
fn read_back(mut value: Value) -> Result<Record, OpenError> {
    let cannot =
        |why: String| OpenError::Format(format!("a record of the basic project cannot be copied: {why}"));
    for _ in 0..MAX_SORTS {
        match Record::from_value(value.clone()) {
            Ok(record) => return Ok(record),
            Err(SchemaError::UnsortedList { path }) => {
                let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
                let Some(Value::Array(items)) = value.pointer_mut(parent) else {
                    return Err(cannot(format!("{path} is not in a list")));
                };
                let before = items.clone();
                items.sort_by_key(|i| i.to_string());
                if *items == before {
                    return Err(cannot(format!(
                        "the list at \"{parent}\" is not a set that sorting orders"
                    )));
                }
            }
            Err(e) => return Err(cannot(e.to_string())),
        }
    }
    Err(cannot("too many lists out of order".to_owned()))
}

/// Copies a basic project directory into a new project: new project ID, new record IDs, the requested kind, and
/// `basic_project` naming the directory's project and its manifest generation (D14).
fn from_directory(dir: &Path, new_id: Id<Project>, kind: NewProjectKind) -> Result<Template, OpenError> {
    let (template, generation) = read_records(dir)?;
    let basic = template.0.id.cast();
    let (mut project, records, lib, assets) = copy_template(template, new_id, kind)?;
    project.basic_project = Some(BasicProjectRef {
        id: basic,
        generation,
        extra: Extra::new(),
    });
    Ok((project, records, lib, assets))
}

/// Copies a template under a new project ID: new record IDs (module documentation) and the requested kind.
fn copy_template(
    template: Template,
    new_id: Id<Project>,
    kind: NewProjectKind,
) -> Result<Template, OpenError> {
    let (project, records, lib, assets) = template;
    let json = |r: &Record| {
        serde_json::to_value(r)
            .map_err(|e| OpenError::Format(format!("a record of the basic project cannot be copied: {e}")))
    };
    // The record IDs to replace: the instantiated records. Library copies and the project itself are not among
    // them (the project record gets `new_id`).
    let mut map: BTreeMap<Uuid, Uuid> = BTreeMap::new();
    let values = records.iter().map(json).collect::<Result<Vec<Value>, _>>()?;
    let id_of = |v: &Value| {
        v.get("id")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
    };
    for (v, record) in values.iter().zip(&records) {
        if matches!(record, Record::Channel(_)) {
            continue;
        }
        if let Some(old) = id_of(v) {
            let new: Id<()> = v5::imported_id(new_id.as_uuid(), &old.hyphenated().to_string());
            map.insert(old, new.as_uuid());
        }
    }
    // A PLC channel's ID is derived from its module's: `v5(module, "channel", number)` (ADR-0004 amendment 2).
    for (v, record) in values.iter().zip(&records) {
        let Record::Channel(channel) = record else {
            continue;
        };
        let module = map.get(&channel.module.as_uuid()).copied();
        if let (Some(old), Some(module)) = (id_of(v), module) {
            let new: Id<()> = v5::plc_channel_id(Id::<()>::from_uuid(module), channel.number);
            map.insert(old, new.as_uuid());
        }
    }
    let mut out = Vec::with_capacity(values.len());
    for mut v in values {
        remap(&mut v, &map);
        out.push(read_back(v)?);
    }
    // The project record: new identity and kind; references to old record IDs follow the map.
    let mut pv = json(&Record::Project(project))?;
    remap(&mut pv, &map);
    let Record::Project(mut project) = read_back(pv)? else {
        return Err(OpenError::Format(
            "the project record of the basic project cannot be copied".to_owned(),
        ));
    };
    project.id = new_id;
    project.kind = kind_of(kind);
    Ok((project, out, lib, assets))
}

/// Replaces every string (and object key) that is a mapped UUID. Arrays keep their order.
fn remap(v: &mut Value, map: &BTreeMap<Uuid, Uuid>) {
    let swap = |s: &str| -> Option<String> {
        Uuid::parse_str(s)
            .ok()
            .and_then(|u| map.get(&u))
            .map(|n| n.hyphenated().to_string())
    };
    match v {
        Value::String(s) => {
            if let Some(n) = swap(s) {
                *s = n;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|i| remap(i, map)),
        Value::Object(obj) => {
            let keys: Vec<String> = obj.keys().cloned().collect();
            for key in keys {
                if let Some(mut value) = obj.remove(&key) {
                    remap(&mut value, map);
                    obj.insert(swap(&key).unwrap_or(key), value);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "0190a000-0000-7000-8000-00000000000a";
    const B: &str = "0190a000-0000-7000-8000-00000000000b";
    const C: &str = "0190a000-0000-7000-8000-00000000000c";
    const D: &str = "0190a000-0000-7000-8000-00000000000d";

    /// A report block whose include and exclude sets are given in the order the remap left them.
    fn block(include: &[&str], exclude: &[&str]) -> Value {
        json!({
            "$k": "report_block",
            "id": "0190a000-0000-7000-8000-0000000000b1",
            "source": {"kind": "toc"},
            "manual": {"include": include, "exclude": exclude},
            // an ordered list beside the sets: its order is data, and it is not sorted
            "sort": [{"key": "zeta"}, {"key": "alpha"}, {"key": "mid"}],
            "form": "0190a000-0000-7000-8000-0000000000f1",
        })
    }

    /// FT-344: a set of IDs that the remap left unsorted (a report block's include and exclude lists) is put in
    /// order at the typed reader's JSON pointer, while an ordered list beside it keeps its order.
    #[test]
    fn read_back_sorts_an_unsorted_id_set_and_keeps_ordered_lists() {
        let value = block(&[C, A, B], &[D, B]);
        // the reader refuses the value as it is: this is the branch under test
        let Err(SchemaError::UnsortedList { path }) = Record::from_value(value.clone()) else {
            panic!("the include list is out of order for the strict reader");
        };
        // the reader names the second element of whichever set it reads first
        assert!(
            ["/manual/include/1", "/manual/exclude/1"].contains(&path.as_str()),
            "{path}"
        );

        let Record::ReportBlock(b) = read_back(value).expect("read back") else {
            panic!("a report block");
        };
        let ids = |set: &ew_schema::Set<ew_schema::AnyId>| -> Vec<String> {
            set.as_slice()
                .iter()
                .map(|id| id.as_uuid().hyphenated().to_string())
                .collect()
        };
        assert_eq!(ids(&b.manual.include), [A, B, C], "the include set is sorted");
        assert_eq!(ids(&b.manual.exclude), [B, D], "the exclude set is sorted too");
        let keys: Vec<&str> = b.sort.iter().map(|k| k.key.as_str()).collect();
        assert_eq!(keys, ["zeta", "alpha", "mid"], "the ordered list keeps its order");
    }

    /// A record whose sets are in order is read as it is, and a value the reader refuses for another reason is
    /// an error that names the copy, not a panic.
    #[test]
    fn read_back_leaves_a_sorted_record_alone_and_reports_other_errors() {
        let Record::ReportBlock(b) = read_back(block(&[A, B], &[])).expect("read back") else {
            panic!("a report block");
        };
        assert_eq!(b.manual.include.as_slice().len(), 2);
        let err = read_back(json!({"$k": "report_block", "id": "not an id"})).expect_err("a bad record");
        assert!(err.to_string().contains("cannot be copied"), "{err}");
    }
}
