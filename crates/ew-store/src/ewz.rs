// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `.ewz` transport (doc 05 §12): one verified generation of a project as a
//! deterministic ZIP ([`crate::zip`]), its strict reader, and the all-or-nothing extract through the [`Vfs`] seam.
//!
//! **Layout** ([`pack`]). `mimetype` first and stored (`application/vnd.elektrawatt.project+zip`), `manifest.json`
//! second, then every file the manifest lists and `.gitattributes` and `.gitignore` when present, in strictly
//! increasing byte order of their paths, deflated, DOS time 1980-01-01 (the container's fixed fields). The packed
//! generation is [`Store::read_generation`]'s, read back from the disk and verified, never a model in memory; strays,
//! foreign files, `.ew/`, the lock files, temps and `.git/` never enter. The same project and build give the same
//! bytes. At most 65,535 entries (the container has no ZIP64). Before it hands the bytes out, [`pack`] reads them back
//! through [`read`] and refuses an archive the reader would refuse ([`PackError::Refused`]).
//!
//! **As found** ([`pack_as_found`]), the one exception, used by `ew fmt --repair` alone
//! for its backup: the generation's files as the store loaded them, a file edited outside ElektraWatt since the last
//! save included, with `manifest.json` the loaded manifest whose hash of each listed file is that of its bytes (a
//! pending hash too). It reads every listed file again and refuses one whose bytes are not those the store loaded, so
//! the backup is the project as the repair found it. It shares [`pack`] and the git files with [`pack_store`], which
//! stays strict: an export hands over a verified generation only.
//!
//! **The reader** ([`read`]) accepts only that shape, under [`ZipLimits::default`] (2 GiB expanded, 512 MiB per entry,
//! 200 : 1 per entry above 64 KiB): the container's own rules first (names, duplicates ignoring case, sizes, CRC-32),
//! then `mimetype` with its value, `manifest.json` that parses, and from the third entry on each entry deflated, in
//! strictly increasing byte order, a file the manifest lists with its SHA-256 or one of the two git files, every
//! listed file present, and every name one that Windows can hold. A manifest that lists a path which is not a project
//! file (a lock, a temp, `.git/…`, `.ew/…`, a foreign or stray name) is refused, so an extract never writes a name the
//! project format does not own.
//!
//! **Extract** ([`extract`]) writes through the seam only: the target must not exist; a sibling staging directory
//! `<target>.tmp-ewz` is created exclusively ([`Vfs::create_dir`]), every directory below it the same way, every file
//! with [`Vfs::create_new`] and [`Vfs::sync`], every directory synced, every file read back and compared with the
//! verified entry; then the target's absence is checked again and the staging directory renamed to the target
//! ([`Vfs::rename`]) and the parent synced. Any failure removes what the call created (the files, then the
//! directories), and a failure of the parent's sync moves the target back first, so the target is never half a
//! project and no staging directory is left behind. A staging directory that exists already is never touched. The one
//! exception: when the parent's sync fails and the move back fails too, the complete target stays and the error is
//! [`ExtractError::NotDurable`], which names it.
//!
//! **The residual race.** Between the second absence check and the rename, another process can make the target. On
//! Windows the rename then fails and nothing is left; on Linux and macOS `rename` replaces an empty directory made in
//! that instant (a non-empty one makes it fail). That window, an empty directory replaced by the complete project, is
//! accepted: no file of anyone is lost.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use ew_format::{FileHash, FilePath, Limits, MANIFEST, Manifest, PathClass, classify, sha256};

use crate::store::{GenerationError, Store};
use crate::vfs::{EntryKind, Vfs, join};
use crate::zip::{self, Method, ZipError, ZipLimits};

/// The media type in the `mimetype` entry.
pub const MIMETYPE: &str = "application/vnd.elektrawatt.project+zip";
/// The first entry, stored.
pub const MIMETYPE_ENTRY: &str = "mimetype";
/// The extension of an `.ewz` file.
pub const EXTENSION: &str = "ewz";
/// The git files packed beside the manifest's files when present (doc 05 §11.1).
pub const GIT_FILES: [&str; 2] = [".gitattributes", ".gitignore"];
/// The suffix of the extract's staging directory beside the target.
pub const STAGING_SUFFIX: &str = ".tmp-ewz";
/// The refusal of every writer given an `.ewz` (D9).
pub const READ_ONLY: &str = "an .ewz opens read-only; extract it first";
/// The most entries an `.ewz` holds (the container has no ZIP64; doc 05 §12's 100,000 is lowered, D8).
pub const MAX_ENTRIES: usize = 65_535;

/// Whether `path` names an `.ewz`: its extension is `ewz` (any case) and it is not a directory.
pub fn is_ewz(vfs: &dyn Vfs, path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
        && !matches!(vfs.stat(path), Ok(s) if s.kind == EntryKind::Dir)
}

/// The default target of extracting `archive` (doc 05 §12): `<stem>.ew` beside it; `None` for a path without a file
/// name.
pub fn default_target(archive: &Path) -> Option<PathBuf> {
    let stem = archive.file_stem()?;
    let mut name = stem.to_owned();
    name.push(".ew");
    Some(archive.with_file_name(name))
}

/// The staging directory of extracting into `target`: `<target>.tmp-ewz` beside it.
pub fn staging_of(target: &Path) -> PathBuf {
    let mut s = target.as_os_str().to_owned();
    s.push(STAGING_SUFFIX);
    PathBuf::from(s)
}

/// An `.ewz` read and verified ([`read`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ewz {
    /// The manifest.
    pub manifest: Manifest,
    /// Every entry but `mimetype`, by project-relative path: `manifest.json`, the files it lists and the git files.
    pub entries: BTreeMap<String, Vec<u8>>,
}

impl Ewz {
    /// The generation the archive holds.
    pub fn generation(&self) -> u64 {
        self.manifest.generation
    }
}

/// Why bytes are not an `.ewz` this reader opens.
#[derive(Debug)]
pub enum EwzError {
    /// The file could not be read.
    Io(io::Error),
    /// The file is larger than the expanded bound.
    TooLarge {
        /// Its size in bytes.
        len: u64,
    },
    /// The container: see [`ZipError`].
    Zip(ZipError),
    /// The first entry is not `mimetype`, stored, with [`MIMETYPE`].
    NoMimetype,
    /// The second entry is not `manifest.json`, deflated.
    NoManifest,
    /// `manifest.json` does not parse (it says why).
    Manifest(String),
    /// The manifest lists a path that is not a project file.
    NotProjectFile {
        /// The path.
        path: String,
    },
    /// An entry after the manifest is stored.
    Stored {
        /// The entry.
        name: String,
    },
    /// An entry is not in byte order after the one before it.
    Order {
        /// The entry.
        name: String,
        /// The entry before it.
        after: String,
    },
    /// An entry's name cannot be a file name on Windows (it says why).
    Unportable {
        /// The entry.
        name: String,
        /// What is wrong.
        why: &'static str,
    },
    /// An entry is neither a file the manifest lists nor a git file.
    NotListed {
        /// The entry.
        name: String,
    },
    /// A listed file's hash is pending (a merged project).
    Pending {
        /// The path.
        path: String,
    },
    /// An entry does not have the manifest's hash.
    Hash {
        /// The path.
        path: String,
    },
    /// A listed file is not in the archive.
    Missing {
        /// The path.
        path: String,
    },
}

impl fmt::Display for EwzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EwzError::Io(e) => write!(f, "it could not be read: {e}"),
            EwzError::TooLarge { len } => {
                write!(f, "it is {len} bytes, more than the 2 GiB an .ewz may hold")
            }
            EwzError::Zip(e) => e.fmt(f),
            EwzError::NoMimetype => write!(
                f,
                "the first entry is not `{MIMETYPE_ENTRY}`, stored, holding `{MIMETYPE}`"
            ),
            EwzError::NoManifest => write!(f, "the second entry is not `{MANIFEST}`, deflated"),
            EwzError::Manifest(why) => f.write_str(why),
            EwzError::NotProjectFile { path } => {
                write!(f, "the manifest lists `{path}`, which is not a project file")
            }
            EwzError::Stored { name } => write!(f, "the entry `{name}` is not deflated"),
            EwzError::Order { name, after } => {
                write!(f, "the entry `{name}` is not in byte order after `{after}`")
            }
            EwzError::Unportable { name, why } => {
                write!(f, "the entry name `{name}` is not allowed: {why}")
            }
            EwzError::NotListed { name } => {
                write!(f, "the entry `{name}` is not a file the manifest lists")
            }
            EwzError::Pending { path } => {
                write!(f, "the manifest's hash of `{path}` is pending (a merged project)")
            }
            EwzError::Hash { path } => write!(f, "the entry `{path}` does not have the manifest's hash"),
            EwzError::Missing { path } => {
                write!(f, "`{path}` is listed in the manifest but not in the archive")
            }
        }
    }
}

impl std::error::Error for EwzError {}

impl From<ZipError> for EwzError {
    fn from(e: ZipError) -> EwzError {
        EwzError::Zip(e)
    }
}

/// Whether a listed path is one an `.ewz` may hold: a record file, a library document or an opaque file (assets
/// included); never the manifest, a lock, a temp, a git or `.ew/` path, a stray or a foreign name.
fn is_project_file(path: &str) -> bool {
    matches!(
        classify(path),
        PathClass::Records(_) | PathClass::LibDocument { .. } | PathClass::Opaque(_)
    )
}

/// Why `name` cannot be a path on Windows, where the container's own rules let it through: a segment that is a device
/// name (`CON`, `PRN`, `AUX`, `NUL`, `COM1` … `COM9`, `LPT1` … `LPT9`, any case, before its first `.`), one that ends
/// with a dot or a space, or one of the characters `<>"|?*`.
fn unportable(name: &str) -> Option<&'static str> {
    for segment in name.split('/') {
        if segment
            .chars()
            .any(|c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
        {
            return Some("a character Windows does not allow in a file name");
        }
        if segment.ends_with('.') || segment.ends_with(' ') {
            return Some("a name that ends with a dot or a space");
        }
        let base = segment.split('.').next().unwrap_or(segment).to_ascii_uppercase();
        let device = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || base.len() == 4
                && (base.starts_with("COM") || base.starts_with("LPT"))
                && matches!(base.as_bytes()[3], b'1'..=b'9');
        if device {
            return Some("a Windows device name");
        }
    }
    None
}

/// Reads an `.ewz` and checks it (see the module documentation): `Ok` only for exactly the shape [`pack`] writes, with
/// every listed file present and matching its hash.
///
/// # Errors
///
/// [`EwzError`]: the container, its bounds, the order, the set of entries, the manifest, a hash.
pub fn read(bytes: &[u8]) -> Result<Ewz, EwzError> {
    let entries = zip::read(bytes, &ZipLimits::default())?;
    let mut entries = entries.into_iter();
    match entries.next() {
        Some(e)
            if e.name == MIMETYPE_ENTRY && e.method == Method::Stored && e.data == MIMETYPE.as_bytes() => {}
        _ => return Err(EwzError::NoMimetype),
    }
    let manifest_bytes = match entries.next() {
        Some(e) if e.name == MANIFEST && e.method == Method::Deflated => e.data,
        _ => return Err(EwzError::NoManifest),
    };
    let manifest = Manifest::parse(&manifest_bytes, &Limits::default())
        .map_err(|e| EwzError::Manifest(e.to_string()))?;
    if let Some(path) = manifest.files.keys().find(|p| !is_project_file(p.as_str())) {
        return Err(EwzError::NotProjectFile {
            path: path.to_string(),
        });
    }
    let mut out: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    out.insert(MANIFEST.to_owned(), manifest_bytes);
    let mut before: Option<String> = None;
    for e in entries {
        if e.method != Method::Deflated {
            return Err(EwzError::Stored { name: e.name });
        }
        if let Some(after) = before.take() {
            if after.as_bytes() >= e.name.as_bytes() {
                return Err(EwzError::Order { name: e.name, after });
            }
        }
        if let Some(why) = unportable(&e.name) {
            return Err(EwzError::Unportable { name: e.name, why });
        }
        if !GIT_FILES.contains(&e.name.as_str()) {
            let listed = FilePath::new(e.name.as_str())
                .ok()
                .and_then(|p| manifest.files.get(&p));
            match listed {
                None => return Err(EwzError::NotListed { name: e.name }),
                Some(FileHash::Pending) => return Err(EwzError::Pending { path: e.name }),
                Some(FileHash::Sha256(h)) if *h != sha256(&e.data) => {
                    return Err(EwzError::Hash { path: e.name });
                }
                Some(FileHash::Sha256(_)) => {}
            }
        }
        before = Some(e.name.clone());
        out.insert(e.name, e.data);
    }
    if let Some(path) = manifest.files.keys().find(|p| !out.contains_key(p.as_str())) {
        return Err(EwzError::Missing {
            path: path.to_string(),
        });
    }
    Ok(Ewz {
        manifest,
        entries: out,
    })
}

/// Reads the file `path` through `vfs` ([`read`]); a file larger than the expanded bound (2 GiB) is refused before it
/// is read.
///
/// # Errors
///
/// [`EwzError::Io`], [`EwzError::TooLarge`], else [`read`]'s.
pub fn read_file(vfs: &dyn Vfs, path: &Path) -> Result<Ewz, EwzError> {
    let len = vfs.stat(path).map_err(EwzError::Io)?.len;
    if len > ZipLimits::default().max_total_bytes {
        return Err(EwzError::TooLarge { len });
    }
    read(&vfs.read(path).map_err(EwzError::Io)?)
}

/// Why a project was not packed.
#[derive(Debug)]
pub enum PackError {
    /// The generation could not be read back and verified ([`Store::read_generation`]).
    Generation(GenerationError),
    /// A git file could not be read.
    Io {
        /// The path.
        path: PathBuf,
        /// The error.
        error: io::Error,
    },
    /// The archive would not be one the reader opens (it says why): too many entries, a name, a bound.
    Refused(String),
    /// A listed file's bytes are not those the store loaded: it changed on disk since the project was opened
    /// ([`pack_as_found`]).
    ChangedSinceOpen {
        /// The project-relative path.
        path: String,
    },
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::Generation(e) => e.fmt(f),
            PackError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            PackError::ChangedSinceOpen { path } => {
                write!(f, "{path} changed on disk since the project was opened")
            }
            PackError::Refused(why) => write!(
                f,
                "this project cannot be packed into an .ewz ({why}); share the project folder instead"
            ),
        }
    }
}

impl std::error::Error for PackError {}

/// An `.ewz` packed from a store ([`pack_store`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Packed {
    /// The archive's bytes.
    pub bytes: Vec<u8>,
    /// The generation packed.
    pub generation: u64,
    /// The files packed: `manifest.json`, the files it lists and the git files present.
    pub files: usize,
}

/// The bytes of the `.ewz` of `manifest_bytes` (`manifest.json`), `files` (the files it lists) and `git` (the git files
/// present), in the layout of the module documentation, read back through [`read`] before they are returned.
///
/// # Errors
///
/// [`PackError::Refused`] for more than [`MAX_ENTRIES`] entries, a git file of another name, a name or a size the
/// container refuses, and an archive the reader refuses.
pub fn pack(
    manifest_bytes: &[u8],
    files: &BTreeMap<String, Vec<u8>>,
    git: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>, PackError> {
    let count = 2 + files.len() + git.len();
    if count > MAX_ENTRIES {
        return Err(PackError::Refused(format!(
            "it has {count} entries, more than the {MAX_ENTRIES} an .ewz holds"
        )));
    }
    if let Some(name) = git.keys().find(|n| !GIT_FILES.contains(&n.as_str())) {
        return Err(PackError::Refused(format!("`{name}` is not a git file")));
    }
    let mut rest: BTreeMap<&str, &[u8]> = files.iter().map(|(p, d)| (p.as_str(), d.as_slice())).collect();
    for (name, data) in git {
        rest.insert(name.as_str(), data.as_slice());
    }
    let mut list: Vec<(&str, Method, &[u8])> = vec![
        (MIMETYPE_ENTRY, Method::Stored, MIMETYPE.as_bytes()),
        (MANIFEST, Method::Deflated, manifest_bytes),
    ];
    list.extend(rest.into_iter().map(|(p, d)| (p, Method::Deflated, d)));
    let bytes = zip::write(&list).map_err(|e| PackError::Refused(e.to_string()))?;
    read(&bytes).map_err(|e| PackError::Refused(e.to_string()))?;
    Ok(bytes)
}

/// Packs the store's current generation ([`Store::read_generation`]: read back from the disk and verified) with the
/// git files present in its directory (a regular file only; a link or a directory of that name is not packed).
///
/// # Errors
///
/// [`PackError`]: the generation, a git file, the archive.
pub fn pack_store(store: &Store) -> Result<Packed, PackError> {
    let generation = store.read_generation().map_err(PackError::Generation)?;
    let git = git_files(store)?;
    let files: BTreeMap<String, Vec<u8>> = generation
        .files
        .into_iter()
        .map(|(p, d)| (p.as_str().to_owned(), d))
        .collect();
    let bytes = pack(&generation.manifest_bytes, &files, &git)?;
    Ok(Packed {
        bytes,
        generation: generation.generation,
        files: 1 + files.len() + git.len(),
    })
}

/// The git files present in the store's directory, read through its seam: a regular file only (a link or a directory
/// of that name is not packed), what [`pack_store`] and [`pack_as_found`] pack beside the listed files.
fn git_files(store: &Store) -> Result<BTreeMap<String, Vec<u8>>, PackError> {
    let vfs = &**store.vfs();
    let mut git = BTreeMap::new();
    for name in GIT_FILES {
        let path = join(store.root(), name);
        match vfs.stat(&path) {
            Ok(s) if s.kind == EntryKind::File => {
                let data = vfs.read(&path).map_err(|error| PackError::Io {
                    path: path.clone(),
                    error,
                })?;
                git.insert(name.to_owned(), data);
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(PackError::Io { path, error }),
        }
    }
    Ok(git)
}

/// The backup of `ew fmt --repair` ([`pack_as_found`]): the store's generation with its files as found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AsFound {
    /// The archive: the store's generation, its files counted as [`pack_store`] counts them.
    pub packed: Packed,
    /// How many listed files differ from the generation as ElektraWatt saved it (their bytes do not have the
    /// manifest's hash, or that hash is pending): the files edited outside ElektraWatt since the last save.
    pub edited: usize,
}

/// Packs the store's generation as found (the module documentation): every file the
/// loaded manifest lists, read again through the store's seam and each refused unless its bytes are those the store
/// loaded, and the git files present ([`pack_store`]'s); `manifest.json` is its bytes as read when no listed file was
/// edited, else the loaded manifest ([`Store::manifest`], [`Manifest::to_text`]) with each listed file's hash set to
/// that of its bytes (a pending hash too), its generation and every other member as they are. Unlisted files are
/// neither read nor packed. For the backups of `ew fmt --repair` and of an upgrade alone: an export
/// packs a verified generation ([`pack_store`]).
///
/// # Errors
///
/// [`PackError::Generation`] for no manifest ([`GenerationError::NoManifest`]), an incomplete save
/// ([`GenerationError::Incomplete`]) and a listed file that cannot be read ([`GenerationError::Io`]);
/// [`PackError::ChangedSinceOpen`] for a listed file whose bytes are not those the store loaded;
/// [`PackError::Io`] for a git file; [`PackError::Refused`] for an archive the reader would refuse (the entry cap
/// among them).
pub fn pack_as_found(store: &Store) -> Result<AsFound, PackError> {
    let loaded = store
        .manifest()
        .ok_or(PackError::Generation(GenerationError::NoManifest))?;
    if store.is_incomplete() {
        return Err(PackError::Generation(GenerationError::Incomplete));
    }
    let vfs = &**store.vfs();
    let mut manifest = loaded.clone();
    let mut files = BTreeMap::new();
    let mut edited = 0;
    for (path, listed) in &loaded.files {
        let full = join(store.root(), path.as_str());
        let bytes = vfs
            .read(&full)
            .map_err(|error| PackError::Generation(GenerationError::Io { path: full, error }))?;
        let hash = sha256(&bytes);
        if store.loaded_hash(path) != Some(hash) {
            return Err(PackError::ChangedSinceOpen {
                path: path.to_string(),
            });
        }
        if *listed != FileHash::Sha256(hash) {
            edited += 1;
        }
        manifest.files.insert(path.clone(), FileHash::Sha256(hash));
        files.insert(path.as_str().to_owned(), bytes);
    }
    let git = git_files(store)?;
    // With no file edited the manifest's bytes are packed as read, so that the archive extracts byte-equal to the
    // folder (an upgrade's backup); else its hashes are set to the files' as above.
    let manifest_text = match store.manifest_bytes() {
        Some(read) if edited == 0 => read.to_vec(),
        _ => manifest.to_text().into_bytes(),
    };
    let bytes = pack(&manifest_text, &files, &git)?;
    Ok(AsFound {
        packed: Packed {
            bytes,
            generation: store.generation(),
            files: 1 + files.len() + git.len(),
        },
        edited,
    })
}

/// Why an extract did not happen.
#[derive(Debug)]
pub enum ExtractError {
    /// The target exists: an `.ewz` is extracted into a new folder only.
    Exists(PathBuf),
    /// The staging directory exists (another extract, or an interrupted one); it is not touched.
    StagingExists(PathBuf),
    /// A file operation failed; what the call created is removed.
    Io {
        /// The path.
        path: PathBuf,
        /// The error.
        error: io::Error,
    },
    /// A file did not read back as it was written; what the call created is removed.
    Verify(PathBuf),
    /// The parent folder did not sync, and the target could not be renamed back: the complete target stays, but
    /// it may not survive a power failure. The one error after which something of the call is left.
    NotDurable {
        /// The parent folder that did not sync.
        parent: PathBuf,
        /// The extracted target that stays.
        target: PathBuf,
        /// The sync's error.
        error: io::Error,
    },
}

impl fmt::Display for ExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtractError::Exists(p) => write!(
                f,
                "{} already exists: an .ewz is extracted into a new folder only; nothing was written",
                p.display()
            ),
            ExtractError::StagingExists(p) => write!(
                f,
                "{} already exists (another extract, or an interrupted one): remove it and try again; nothing was written",
                p.display()
            ),
            ExtractError::Io { path, error } => {
                write!(f, "{}: {error}; nothing was extracted", path.display())
            }
            ExtractError::Verify(p) => write!(
                f,
                "{} did not read back as it was written; nothing was extracted",
                p.display()
            ),
            ExtractError::NotDurable {
                parent,
                target,
                error,
            } => write!(
                f,
                "{}: {error}; {} was extracted whole and stays, but it may not survive a power failure",
                parent.display(),
                target.display()
            ),
        }
    }
}

impl std::error::Error for ExtractError {}

/// What [`extract`] created so far, removed again on a failure.
struct Made {
    /// The files, in creation order.
    files: Vec<PathBuf>,
    /// The directories, in creation order (the staging directory first).
    dirs: Vec<PathBuf>,
    /// The same directories, for lookup.
    known: BTreeSet<PathBuf>,
}

impl Made {
    /// Removes the files, then the directories deepest first; best effort (the error that stopped the extract is the
    /// one reported).
    fn undo(&self, vfs: &dyn Vfs) {
        for f in self.files.iter().rev() {
            let _ = vfs.remove_file(f);
        }
        for d in self.dirs.iter().rev() {
            let _ = vfs.remove_dir(d);
        }
    }
}

fn io_at(path: &Path) -> impl FnOnce(io::Error) -> ExtractError + '_ {
    move |error| ExtractError::Io {
        path: path.to_path_buf(),
        error,
    }
}

/// `Ok` when nothing is at `target`.
fn absent(vfs: &dyn Vfs, target: &Path) -> Result<(), ExtractError> {
    match vfs.stat(target) {
        Ok(_) => Err(ExtractError::Exists(target.to_path_buf())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ExtractError::Io {
            path: target.to_path_buf(),
            error,
        }),
    }
}

/// Writes every entry below the staging directory, synced, and reads each back.
fn fill(vfs: &dyn Vfs, ewz: &Ewz, staging: &Path, made: &mut Made) -> Result<(), ExtractError> {
    for (rel, data) in &ewz.entries {
        let mut dir = staging.to_path_buf();
        let segments: Vec<&str> = rel.split('/').collect();
        for segment in &segments[..segments.len() - 1] {
            dir.push(segment);
            if !made.known.contains(&dir) {
                vfs.create_dir(&dir).map_err(io_at(&dir))?;
                made.known.insert(dir.clone());
                made.dirs.push(dir.clone());
            }
        }
        let path = join(staging, rel);
        vfs.create_new(&path, data).map_err(io_at(&path))?;
        made.files.push(path.clone());
        vfs.sync(&path).map_err(io_at(&path))?;
    }
    for dir in made.dirs.iter().rev() {
        vfs.sync_dir(dir).map_err(io_at(dir))?;
    }
    for (rel, data) in &ewz.entries {
        let path = join(staging, rel);
        if vfs.read(&path).map_err(io_at(&path))? != *data {
            return Err(ExtractError::Verify(path));
        }
    }
    Ok(())
}

/// Extracts a verified `.ewz` into the new directory `target`, all or nothing (see the module documentation).
///
/// # Errors
///
/// [`ExtractError`]: the target or the staging directory exists, a file operation failed, a file did not read back;
/// in every case nothing of this call is left under either name, except for [`ExtractError::NotDurable`]: the parent's
/// sync failed and the target could not be moved back, so the complete target stays.
pub fn extract(vfs: &dyn Vfs, ewz: &Ewz, target: &Path) -> Result<(), ExtractError> {
    if target.file_name().is_none() {
        return Err(ExtractError::Io {
            path: target.to_path_buf(),
            error: io::Error::new(io::ErrorKind::InvalidInput, "not a folder name"),
        });
    }
    absent(vfs, target)?;
    let staging = staging_of(target);
    match vfs.create_dir(&staging) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ExtractError::StagingExists(staging));
        }
        Err(error) => {
            return Err(ExtractError::Io { path: staging, error });
        }
    }
    let mut made = Made {
        files: Vec::new(),
        dirs: vec![staging.clone()],
        known: BTreeSet::new(),
    };
    made.known.insert(staging.clone());
    let placed = fill(vfs, ewz, &staging, &mut made)
        .and_then(|()| absent(vfs, target))
        .and_then(|()| vfs.rename(&staging, target).map_err(io_at(target)));
    if let Err(e) = placed {
        made.undo(vfs);
        return Err(e);
    }
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if let Err(error) = vfs.sync_dir(parent) {
        // Not durable: take it back, so that a failed extract leaves no target.
        if vfs.rename(target, &staging).is_err() {
            return Err(ExtractError::NotDurable {
                parent: parent.to_path_buf(),
                target: target.to_path_buf(),
                error,
            });
        }
        made.undo(vfs);
        return Err(ExtractError::Io {
            path: parent.to_path_buf(),
            error,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_names_are_refused() {
        assert_eq!(unportable("pages/a.jsonl"), None);
        assert_eq!(unportable("rules/con"), Some("a Windows device name"));
        assert_eq!(unportable("rules/Com1.txt"), Some("a Windows device name"));
        assert_eq!(unportable("rules/com0"), None);
        assert_eq!(unportable("rules/console"), None);
        assert_eq!(
            unportable("rules/a./b"),
            Some("a name that ends with a dot or a space")
        );
        assert_eq!(
            unportable("rules/a "),
            Some("a name that ends with a dot or a space")
        );
        assert_eq!(
            unportable("rules/a?b"),
            Some("a character Windows does not allow in a file name")
        );
    }

    #[test]
    fn the_default_target_is_beside_the_archive() {
        assert_eq!(
            default_target(Path::new("/w/J-E-2416.ewz")),
            Some(PathBuf::from("/w/J-E-2416.ew"))
        );
        assert_eq!(staging_of(Path::new("/w/p.ew")), PathBuf::from("/w/p.ew.tmp-ewz"));
    }
}
