// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts import's front door (WP-S1.19 part 4, D11): the column maps saved per source, the input read with its
//! size checked first, the manufacturer master from the libraries, and the two targets:
//!
//! - **a project** ([`Session::catalog_parts`]): the parts as project-local copies, ready for `parts.import` (as
//!   built: a held key is skipped, nothing is updated);
//! - **a writable library** ([`import_into_library`]): a library write through [`library::Registry::write`] under
//!   the library's lock, the import date a parameter (doc 07 §3.6). Changed content under a held version is refused
//!   before anything is written unless the import bumps it (doc 05 §13.1).
//!
//! A library write is a front-end operation (WP-S1.19 §13.2 item 7): it takes the registry by `&mut`, which only the
//! shell and the CLI hold ([`Session::libraries_mut`]); no command, command log, script or MCP tool reaches it.

use std::path::{Path, PathBuf};

pub use ew_import_catalog::ew_interop_api::{
    ImportError, ImportInput, ImportReport, Limits, ReportEntry, Severity,
};
pub use ew_schema::Date;
use ew_schema::{Kind, LibCopy, LibDocument, LibRef, Manufacturer, Part};

pub use ew_import_catalog::{
    CatalogTarget, ColumnMap, IMPORTER_ID, MapError, Member, PartsImport, RowCounts, RowOutcome, RowStatus,
    SourceFormat, check_source_key, parse_parts, pn_norm,
};

use crate::import::PARTS_IMPORT;
use crate::{AnyCommand, Session, library};

/// The directory of the saved column maps below the configuration directory.
pub const MAPS_DIR: &str = "imports";

/// Why a parts import did not happen.
#[derive(Debug)]
pub enum CatalogError {
    /// A file could not be read or written (its name, the error).
    Io(String, std::io::Error),
    /// A source key that is not a file name.
    SourceKey(String),
    /// The column map is not usable.
    Map(MapError),
    /// The table was refused as a whole.
    Import(ImportError),
    /// The library refused the write (read-only, locked, stale lock, other content under a published version, I/O).
    Library(library::LibraryError),
    /// Parts changed under their held version and the import does not bump: `line N: <part number> (<version>)`,
    /// in row order. Nothing was written.
    NeedsBump(Vec<String>),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatalogError::Io(name, e) => write!(f, "{name}: {e}"),
            CatalogError::SourceKey(m) => f.write_str(m),
            CatalogError::Map(e) => write!(f, "{e}"),
            CatalogError::Import(e) => write!(f, "{e}"),
            CatalogError::Library(e) => write!(f, "{e}"),
            CatalogError::NeedsBump(rows) => write!(
                f,
                "{} parts changed under their published version (give the import `--bump` to write them as the next patch version): {}",
                rows.len(),
                rows.join("; ")
            ),
        }
    }
}

impl std::error::Error for CatalogError {}

impl From<MapError> for CatalogError {
    fn from(e: MapError) -> CatalogError {
        CatalogError::Map(e)
    }
}

impl From<ImportError> for CatalogError {
    fn from(e: ImportError) -> CatalogError {
        CatalogError::Import(e)
    }
}

fn base_name(p: &Path) -> String {
    p.file_name()
        .map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// The file of the map saved for `source_key`: `<config_dir>/imports/<source key>.toml` (D11).
///
/// # Errors
///
/// [`CatalogError::SourceKey`] for a key that is not a file name ([`check_source_key`]).
pub fn map_path(config_dir: &Path, source_key: &str) -> Result<PathBuf, CatalogError> {
    check_source_key(source_key).map_err(CatalogError::SourceKey)?;
    Ok(config_dir.join(MAPS_DIR).join(format!("{source_key}.toml")))
}

/// Saves `map` for `source_key` (creating `imports/`), replacing an earlier one; returns the file.
///
/// # Errors
///
/// [`CatalogError::SourceKey`], or [`CatalogError::Io`] when the file cannot be written.
pub fn save_column_map(
    config_dir: &Path,
    source_key: &str,
    map: &ColumnMap,
) -> Result<PathBuf, CatalogError> {
    let path = map_path(config_dir, source_key)?;
    let io = |e| CatalogError::Io(format!("{source_key}.toml"), e);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    std::fs::write(&path, map.to_toml()).map_err(io)?;
    Ok(path)
}

/// The largest map file read (a map names a few dozen columns).
const MAX_MAP_BYTES: u64 = 1024 * 1024;

/// Reads the map file `path`.
///
/// # Errors
///
/// [`CatalogError::Io`] when it cannot be read or is larger than 1 MiB, [`CatalogError::Map`] when it is not a map.
pub fn read_column_map(path: &Path) -> Result<ColumnMap, CatalogError> {
    let name = base_name(path);
    let len = std::fs::metadata(path)
        .map_err(|e| CatalogError::Io(name.clone(), e))?
        .len();
    if len > MAX_MAP_BYTES {
        return Err(CatalogError::Io(
            name,
            std::io::Error::other(format!("larger than {MAX_MAP_BYTES} bytes")),
        ));
    }
    let text = std::fs::read_to_string(path).map_err(|e| CatalogError::Io(name, e))?;
    Ok(ColumnMap::from_toml(&text)?)
}

/// The map saved for `source_key` ([`save_column_map`]).
///
/// # Errors
///
/// As [`map_path`] and [`read_column_map`].
pub fn load_column_map(config_dir: &Path, source_key: &str) -> Result<ColumnMap, CatalogError> {
    read_column_map(&map_path(config_dir, source_key)?)
}

/// Reads the table `path` as an import input, named by its base name (never a path). A file larger than
/// `limits.max_input_bytes` (the loader's 512 MiB, doc 05 §3.6) is refused before it is read.
///
/// # Errors
///
/// [`CatalogError::Io`] when it cannot be read, [`CatalogError::Import`] for an oversized file.
pub fn read_input(path: &Path, limits: &Limits) -> Result<ImportInput, CatalogError> {
    let name = base_name(path);
    let len = std::fs::metadata(path)
        .map_err(|e| CatalogError::Io(name.clone(), e))?
        .len();
    if len > limits.max_input_bytes {
        return Err(CatalogError::Import(ImportError::LimitExceeded {
            what: format!("file size of {name}"),
            limit: limits.max_input_bytes,
        }));
    }
    let bytes = std::fs::read(path).map_err(|e| CatalogError::Io(name.clone(), e))?;
    ImportInput::new(&name, bytes)
        .map_err(|e| CatalogError::Io(name, std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))
}

/// Every manufacturer record of the installed libraries (deprecated ones included, so an old list still resolves),
/// in the search's order: the master list a parts import resolves manufacturer names through.
pub fn manufacturer_master(reg: &library::Registry) -> Vec<LibCopy<Manufacturer>> {
    let q = library::ItemQuery {
        kinds: vec![Kind::Manufacturer],
        include_inactive: true,
        ..library::ItemQuery::default()
    };
    reg.search_items(&q)
        .into_iter()
        .filter_map(|h| {
            let r = LibRef {
                library: ew_derive::ew_ids::Id::from_uuid(h.library),
                item: ew_derive::ew_ids::Id::from_uuid(h.item),
                version_req: format!("={}", h.version),
                extra: Default::default(),
            };
            match reg.resolve(&r).ok()? {
                LibDocument::Manufacturer(m) => Some(m),
                _ => None,
            }
        })
        .collect()
}

/// The part `item` of `library` as the registry holds it, at any version.
fn held_part(
    reg: &library::Registry,
    library: ew_derive::ew_ids::Uuid,
    item: ew_derive::ew_ids::Uuid,
) -> Option<LibCopy<Part>> {
    let r = LibRef {
        library: ew_derive::ew_ids::Id::from_uuid(library),
        item: ew_derive::ew_ids::Id::from_uuid(item),
        version_req: "*".into(),
        extra: Default::default(),
    };
    match reg.resolve(&r).ok()? {
        LibDocument::Part(p) => Some(p),
        _ => None,
    }
}

/// A parts import into a library: what was read and, when anything was written, the write's report.
#[derive(Debug)]
pub struct LibraryImport {
    /// The import: parts, new manufacturers, rows and report.
    pub import: PartsImport,
    /// The write; `None` when the import held no part or manufacturer to write.
    pub write: Option<library::WriteReport>,
}

/// Imports a parts table into the writable library `library` (`ew lib import parts`, D11, §7.2): the parts with
/// their new manufacturers, through [`library::Registry::write`] (the lock, the shards, the manifest last). A second
/// import of a changed list updates the items in place (same item IDs); a part whose content changed under its
/// held version is refused for the whole file ([`CatalogError::NeedsBump`], nothing written) unless `bump`, which
/// writes it as the next patch version. The master list is every manufacturer of the registry.
///
/// The library's lock (doc 05 §7.4): a lock another live session holds refuses the write
/// ([`library::LibraryError::Locked`]); a stale one refuses it too ([`library::LibraryError::Stale`]) unless
/// `take_over`, the user's explicit confirmation, takes it over first. A live lock is never taken over.
///
/// A front-end operation (module documentation).
///
/// # Errors
///
/// [`CatalogError::Import`] for a refused table, [`CatalogError::NeedsBump`], [`CatalogError::Library`] for a
/// library that is not installed (`NotFound`), read-only, locked or stale-locked, or a write that fails.
pub fn import_into_library(
    reg: &mut library::Registry,
    library: ew_derive::ew_ids::Uuid,
    input: &ImportInput,
    map: &ColumnMap,
    as_of: Option<Date>,
    bump: bool,
    take_over: bool,
) -> Result<LibraryImport, CatalogError> {
    let info = reg
        .libraries()
        .into_iter()
        .find(|l| l.id == library)
        .ok_or(CatalogError::Library(library::LibraryError::NotFound))?;
    if !info.writable {
        return Err(CatalogError::Library(library::LibraryError::ReadOnly));
    }
    let master = manufacturer_master(reg);
    let import = {
        let held = |item| held_part(reg, library, item);
        let target = CatalogTarget {
            library,
            as_of,
            bump,
            limits: Limits::default(),
            existing: &held,
            local_manufacturers: false,
        };
        parse_parts(input, map, &master, &target)?
    };
    let stale: Vec<String> = import
        .needs_bump()
        .iter()
        .map(|r| {
            let version = match &r.status {
                RowStatus::Updated { from, .. } => from.as_str(),
                _ => "",
            };
            format!("line {}: {} ({version})", r.line, r.part_number)
        })
        .collect();
    if !stale.is_empty() {
        return Err(CatalogError::NeedsBump(stale));
    }
    let docs: Vec<LibDocument> = import
        .manufacturers
        .iter()
        .chain(&import.parts)
        .cloned()
        .collect();
    if docs.is_empty() {
        return Ok(LibraryImport { import, write: None });
    }
    if take_over {
        reg.take_over_stale(library).map_err(CatalogError::Library)?;
    }
    let write = reg.write(library, &docs).map_err(CatalogError::Library)?;
    Ok(LibraryImport {
        import,
        write: Some(write),
    })
}

impl Session {
    /// Reads a parts table for this project (`ew import parts --map`, D11): the parts as project-local copies (the
    /// project's ID as their library), the manufacturers resolved through `master` ([`manufacturer_master`] of
    /// [`Session::libraries`] in the shell, of the `--company` tiers in the CLI), the project's own part copies as
    /// what the target holds. Returns the import and the `parts.import` command of its
    /// new parts (`None` when there is none): `parts.import` creates them in one undoable step and skips a key the
    /// project holds; it never updates a part (updating is the previewed `library.update`). The command embeds the
    /// manufacturer copies the new parts name (WP-S1.19 D10): the manufacturers the import creates and the project's
    /// copies of the master records they resolve to (`parts.import` skips one the project holds).
    ///
    /// # Errors
    ///
    /// [`ImportError`] for a table refused as a whole.
    pub fn catalog_parts(
        &self,
        input: &ImportInput,
        map: &ColumnMap,
        master: &[LibCopy<Manufacturer>],
        as_of: Option<Date>,
    ) -> Result<(PartsImport, Option<AnyCommand>), ImportError> {
        let query = self.query();
        let project = query.project().id.as_uuid();
        let held: std::collections::BTreeMap<_, _> = query
            .parts()
            .iter()
            .filter(|(_, c)| c.library_id.as_uuid() == project)
            .map(|(_, c)| (c.item_id.as_uuid(), c.clone()))
            .collect();
        let existing = |item| held.get(&item).map(|c| (**c).clone());
        let target = CatalogTarget {
            library: project,
            as_of,
            bump: false,
            limits: Limits::default(),
            existing: &existing,
            local_manufacturers: true,
        };
        let import = parse_parts(input, map, master, &target)?;
        let new: std::collections::BTreeSet<_> = import
            .rows
            .iter()
            .filter(|r| r.status == RowStatus::New)
            .filter_map(|r| r.item)
            .collect();
        let parts: Vec<&LibDocument> = import
            .parts
            .iter()
            .filter(|d| matches!(d, LibDocument::Part(p) if new.contains(&p.item_id.as_uuid())))
            .collect();
        let named: std::collections::BTreeSet<_> = parts
            .iter()
            .filter_map(|d| match d {
                LibDocument::Part(p) => Some(p.item.manufacturer.item),
                _ => None,
            })
            .collect();
        let makers = import
            .manufacturers
            .iter()
            .filter(|d| matches!(d, LibDocument::Manufacturer(m) if named.contains(&m.item_id)));
        let copies: Vec<serde_json::Value> = if parts.is_empty() {
            Vec::new()
        } else {
            parts
                .into_iter()
                .chain(makers)
                .map(|d| serde_json::to_value(d).unwrap_or_default())
                .collect()
        };
        let cmd = (!copies.is_empty()).then(|| AnyCommand {
            name: PARTS_IMPORT.into(),
            params: serde_json::json!({ "copies": copies }),
        });
        Ok((import, cmd))
    }
}
