// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden builders of Golden A (doc 20 §3.2, WP-S6.01): build path (a), the CSV bootstrap, and build
//! path (b), the builder DSL that constructs the same project through commands only.
//!
//! - **Path (a)**, [`bootstrap`]: `ew-import-tables` over `golden/J-E-2416/tables/` with the built-in
//!   library of `content/` (the recipe of `ew import csv`). Its records carry the v5 IDs of the fixed import
//!   lineage, so the result is reproducible; path (c), the checked-in fixture, is its committed output.
//! - **Path (b)**, [`build`]: the [`dsl::Builder`] issues one command per step (`structure.create_node`,
//!   `page.create`, `device.create`, `terminal.add`, `cable.create`, `device.place`, `box.black`,
//!   `ip.place`, `connection.draw`, `junction.edit`, `potential.define`, `bond.create`,
//!   `report_block.create`, `fields.set`, `assembly.attach`, `valve.add`, `plc.bind_channel`,
//!   `revision.create`, `library.embed`, …) against a session, starting from the [`Shell`]: the project
//!   row (a new project is `ew new`) and the project-local library items (the parts, which are created in
//!   the project, not embedded); the builder's first command, `library.embed`, embeds the non-local copies
//!   (symbols, function definitions, forms; FT-278). The content comes from the same source of truth as path (a): the
//!   records of the bootstrap batch are the typed description, translated into commands in dependency
//!   order; the commands mint their own IDs, so path (b) agrees with (a) by content (the acceptance SQL and
//!   its expected CSVs), never by ID (doc 20 §3.2: "all columns except `id` and `source_ref`").
//!
//! The builder never injects a record: what no command can write yet is not written, and [`Built`] lists
//! it (`unwritten`), so the acceptance entry can say exactly what path (b) leaves out.
//!
//! The builder does not depend on `ew-api` or `ew-commands` (both are dev-only edges of this crate, doc 04
//! §3): it speaks to a [`dsl::CommandSink`], a function from a command name and its JSON parameters to the
//! keys the command created, which the caller implements over its `Session` (the parameters are the JSON
//! form of the command log, doc 17 §3.5, so [`Built::script`] replays with `ew apply`).

pub mod dsl;
mod translate;

use std::path::{Path, PathBuf};

use ew_import_tables::ew_interop_api::{ImportBatch, ImportInput};
use ew_import_tables::{BuiltinLibrary, TablesImporter, bootstrap_context};
use ew_schema::{LibDocument, Project, Record};

pub use translate::{Built, build};

/// The directory of Golden A below the repository root.
pub const GOLDEN_A: &str = "golden/J-E-2416";

/// The project settings members that path (b) sets with `project.settings.set` instead of taking them
/// from the shell (the members that command edits, doc 03 §5.1).
pub const SETTINGS_BY_COMMAND: [&str; 4] = ["numbering", "colour_profile", "size_profile", "xref_format"];

fn files(dir: &Path, ext: &str) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if path.extension().is_some_and(|e| e == ext) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

/// The built-in library of the golden project: the `@qet10` symbol edition, the function definitions and
/// the forms of `content/` (doc 20 §2.1 rule 4).
///
/// # Errors
///
/// A directory or file that does not read, or a document that does not parse.
pub fn builtin_library(repo: &Path) -> Result<BuiltinLibrary, String> {
    let mut docs = Vec::new();
    for sub in ["symbols/qet10", "fndefs", "forms"] {
        for path in files(&repo.join("content").join(sub), "json")? {
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            docs.push(LibDocument::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))?);
        }
    }
    BuiltinLibrary::new(docs).map_err(|e| format!("the built-in library: {e}"))
}

/// Build path (a): the CSV bootstrap of Golden A (`ew import csv` over `tables/`).
///
/// # Errors
///
/// A table or library file that does not read, or a refused import.
pub fn bootstrap(repo: &Path) -> Result<ImportBatch, String> {
    let mut inputs = Vec::new();
    for path in files(&repo.join(GOLDEN_A).join("tables"), "csv")? {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        inputs.push(ImportInput::new(&name, bytes).map_err(|e| format!("{name}: {e:?}"))?);
    }
    TablesImporter::new(builtin_library(repo)?)
        .import(&inputs, &bootstrap_context())
        .map_err(|e| format!("the golden tables do not import: {e:?}"))
}

/// Whether `doc` is a project-local library item: its `library_id` is the project's (doc 05 §13.2). A
/// document of an unknown kind is kept verbatim, never embedded.
fn is_local(doc: &LibDocument, project: ew_ids::Uuid) -> bool {
    let library = match doc {
        LibDocument::Part(c) => c.library_id.as_uuid(),
        LibDocument::Symbol(c) => c.library_id.as_uuid(),
        LibDocument::Fndef(c) => c.library_id.as_uuid(),
        LibDocument::Form(c) => c.library_id.as_uuid(),
        LibDocument::Macro(c) => c.library_id.as_uuid(),
        LibDocument::Manufacturer(c) => c.library_id.as_uuid(),
        LibDocument::Unknown(_) => return true,
    };
    library == project
}

fn project_of(batch: &ImportBatch) -> &Project {
    batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::Project(p) => Some(p),
            _ => None,
        })
        .expect("a bootstrap batch holds the project row")
}

/// The library copies of `batch` that are not project-local, in the batch's order: what the builder embeds
/// with `library.embed` (FT-278).
///
/// # Panics
///
/// When the batch holds no project row (a bootstrap batch always does).
pub fn embedded_copies(batch: &ImportBatch) -> Vec<LibDocument> {
    let project = project_of(batch).id.as_uuid();
    batch
        .lib
        .iter()
        .filter(|d| !is_local(d, project))
        .cloned()
        .collect()
}

/// The starting point of path (b): the project row and the project-local library items.
///
/// No command creates a project row (a new project is `ew new`), and the project-local parts are created in
/// the project, not embedded (`library.embed` refuses them), so path (b) starts from them. The non-local
/// copies are not part of the shell: the builder embeds them with `library.embed` (FT-278). The project row
/// is the bootstrap's without its import provenance, without the supply's origin (`project.supply.set`) and
/// without the [`SETTINGS_BY_COMMAND`] members, which the builder sets through `project.settings.set`; its
/// ID stays the bootstrap's, because the project-local parts name the project as their library (doc 05
/// §13.2).
#[derive(Clone, Debug)]
pub struct Shell {
    /// The project row.
    pub project: Project,
    /// The project-local library items (the parts).
    pub lib: Vec<LibDocument>,
}

/// The [`Shell`] of `batch`.
///
/// # Panics
///
/// When the batch holds no project row (a bootstrap batch always does).
pub fn shell(batch: &ImportBatch) -> Shell {
    let mut project = project_of(batch).clone();
    project.source_ref = None;
    // The supply's origin names an external reference, which the builder creates by command later and
    // then names with `project.supply.set`.
    if let Some(origin) = project.supply.as_mut().and_then(|s| s.origin.as_mut()) {
        origin.source = None;
    }
    for member in SETTINGS_BY_COMMAND {
        project.settings.extra.remove(member);
    }
    let id = project.id.as_uuid();
    let lib = batch.lib.iter().filter(|d| is_local(d, id)).cloned().collect();
    Shell { project, lib }
}
