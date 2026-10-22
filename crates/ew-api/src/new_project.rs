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
//! Not recorded yet: `Project.basic_project = {id, version}` (doc 07 §2.8) has no member in the schema.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_derive::ew_ids::{Id, IdSource, Uuid, V7Generator, v5};
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_units::Grid;
use ew_schema::{
    Extra, Folio, Page, PageType, Project, ProjectKind, ProjectSettings, Record, StructureAssignment,
};
use ew_store::OpenError;
use ew_store::ew_format::{self, EntryKind, Limits, Manifest};
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
        let (project, records, lib) = match template {
            ProjectTemplate::Builtin => builtin(id, kind),
            ProjectTemplate::Directory(dir) => from_directory(dir, id, kind)?,
        };
        Session::from_records(project, records, lib, options).map_err(|error| OpenError::Model {
            error,
            findings: Vec::new(),
        })
    }
}

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

/// Reads the typed records of a project directory without opening a store: no lock, no journal.
fn read_records(dir: &Path) -> Result<(Project, Vec<Record>, Vec<ew_schema::LibDocument>), OpenError> {
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
    let loaded = ew_format::load(manifest.as_ref(), &files, &limits);
    let findings = loaded.findings;
    let Some(project) = loaded.project else {
        return Err(OpenError::NoProject(findings));
    };
    Ok((project, loaded.records, loaded.lib))
}

/// Copies a basic project directory into a new project: new project ID, new record IDs, the requested kind.
fn from_directory(
    dir: &Path,
    new_id: Id<Project>,
    kind: NewProjectKind,
) -> Result<(Project, Vec<Record>, Vec<ew_schema::LibDocument>), OpenError> {
    let (mut project, records, lib) = read_records(dir)?;
    // The record IDs to replace: the instantiated records. Library copies and the project itself are not among
    // them (the project record gets `new_id`).
    let mut map: BTreeMap<Uuid, Uuid> = BTreeMap::new();
    let values: Vec<Value> = records
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
        .collect();
    let id_of = |v: &Value| {
        v.get("id")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
    };
    let is_channel = |v: &Value| v.get("$k").and_then(Value::as_str) == Some("channel");
    for v in values.iter().filter(|v| !is_channel(v)) {
        if let Some(old) = id_of(v) {
            let new: Id<()> = v5::imported_id(new_id.as_uuid(), &old.hyphenated().to_string());
            map.insert(old, new.as_uuid());
        }
    }
    // A PLC channel's ID is derived from its module's: `v5(module, "channel", number)` (ADR-0004 amendment 2).
    for v in values.iter().filter(|v| is_channel(v)) {
        let module = v
            .get("module")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
            .and_then(|m| map.get(&m).copied());
        let number = v
            .get("number")
            .and_then(Value::as_u64)
            .and_then(|n| u16::try_from(n).ok());
        if let (Some(old), Some(module), Some(number)) = (id_of(v), module, number) {
            let new: Id<()> = v5::plc_channel_id(Id::<()>::from_uuid(module), number);
            map.insert(old, new.as_uuid());
        }
    }
    let old_project = project.id.as_uuid();
    let mut out = Vec::with_capacity(values.len());
    for (record, mut v) in records.into_iter().zip(values) {
        remap(&mut v, &map);
        // A record that does not read back from its own JSON (it should not happen) is kept as it was.
        out.push(serde_json::from_value::<Record>(v).unwrap_or(record));
    }
    // The project record: new identity and kind; references to old record IDs follow the map.
    let mut pv = serde_json::to_value(&project).unwrap_or(Value::Null);
    remap(&mut pv, &map);
    if let Ok(p) = serde_json::from_value::<Project>(pv) {
        project = p;
    }
    debug_assert_ne!(old_project, new_id.as_uuid());
    project.id = new_id;
    project.kind = kind_of(kind);
    Ok((project, out, lib))
}

/// Replaces every string (and object key) that is a mapped UUID.
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
        Value::Array(items) => {
            // A set of IDs is sorted by its elements' text (doc 05 §3.2) and is read strictly: it stays sorted
            // when its elements are replaced.
            let strings = |items: &[Value]| -> Option<Vec<String>> {
                items.iter().map(|i| i.as_str().map(str::to_owned)).collect()
            };
            let before = strings(items);
            items.iter_mut().for_each(|i| remap(i, map));
            if let (Some(before), Some(after)) = (before, strings(items))
                && before.windows(2).all(|w| w[0] <= w[1])
                && before != after
            {
                items.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            }
        }
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
