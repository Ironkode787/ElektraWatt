// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`Registry`]: the opened tiers, the one resolver and the search index (WP-S1.19 §5.2, §7).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ew_format::Limits;
use ew_format::ew_schema::{Kind, LibDocument, LibRef};
use ew_ids::Uuid;
use rusqlite::Transaction;
use serde_json::Value;

use crate::index::{INDEX_FILE, Index, IndexState, LibState, MfrText, texts};
use crate::lock::{self, Identity, LockPolicy};
use crate::manifest::{LibManifest, MANIFEST, hash_matches};
use crate::record::{
    Entry, Parsed, PathRole, ShardKey, classify, identity, join, parse_records, read_file, record_line,
    records_file,
};
use crate::search::{ItemHit, ItemQuery, PartHit, PartQuery};
use crate::starter;
use crate::{LibraryError, LibraryTiers, Tier};

/// The pseudo path of the compiled-in starter library in the index.
const STARTER_PATH: &str = "<compiled-in>";
/// The pseudo file of the compiled-in starter library.
const STARTER_FILE: &str = "starter";

/// What [`Registry::libraries`] reports per library.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LibraryInfo {
    /// The library ID.
    pub id: Uuid,
    /// The human key.
    pub key: String,
    /// The display name.
    pub name: String,
    /// The tier.
    pub tier: Tier,
    /// The library's version.
    pub version: String,
    /// The default licence of its items.
    pub licence: String,
    /// Whether the registry accepts writes (company and user tiers).
    pub writable: bool,
    /// The directory; `None` for the compiled-in starter.
    pub path: Option<PathBuf>,
    /// The manifest generation.
    pub generation: u64,
    /// The index state after the last open, write or rebuild.
    pub index: IndexState,
    /// Files and lines skipped (the library stays usable without them), and other notes.
    pub findings: Vec<String>,
}

/// A fault-injection point of [`Registry::write`] (doc 05 §5.6's harness for the library writer): the write stops
/// there as a killed process would, leaving the files as they are, the lock included.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WriteFault {
    /// After every changed shard is in place, before the manifest is written.
    AfterShards,
    /// After the new manifest's temporary file is written, before its rename.
    BeforeManifestRename,
}

/// What a [`Registry::write`] did.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WriteReport {
    /// The library's generation after the write (unchanged when nothing changed).
    pub generation: u64,
    /// Items new to the library.
    pub added: Vec<Uuid>,
    /// Items written at a new version.
    pub updated: Vec<Uuid>,
    /// Items the library already held with the same version and content.
    pub unchanged: Vec<Uuid>,
    /// Files written.
    pub written: Vec<String>,
    /// Files removed (replaced shards and leftovers of an interrupted write).
    pub removed: Vec<String>,
}

#[derive(Debug)]
struct Lib {
    info: LibraryInfo,
    /// `None` for the compiled-in starter.
    manifest: Option<LibManifest>,
}

impl Lib {
    fn dir(&self) -> Option<&Path> {
        self.info.path.as_deref()
    }

    /// The manifest's records files by shard.
    fn shards(&self) -> BTreeMap<ShardKey, Vec<String>> {
        let mut out: BTreeMap<ShardKey, Vec<String>> = BTreeMap::new();
        if let Some(m) = &self.manifest {
            for path in m.files.keys() {
                if let Ok(PathRole::Records(k)) = classify(path) {
                    out.entry(k).or_default().push(path.clone());
                }
            }
        }
        out
    }

    /// The files the index tracks: the manifest's, or the starter's pseudo file.
    fn files(&self) -> BTreeMap<String, [u8; 32]> {
        match &self.manifest {
            Some(m) => m.files.clone(),
            None => BTreeMap::from([(STARTER_FILE.to_owned(), starter_hash())]),
        }
    }

    fn index_path(&self) -> String {
        self.dir()
            .map_or_else(|| STARTER_PATH.to_owned(), |d| d.display().to_string())
    }
}

/// The hash over the compiled-in starter's keys and texts: changes whenever a starter item does.
fn starter_hash() -> [u8; 32] {
    let mut all = Vec::new();
    for i in starter::items() {
        all.extend_from_slice(i.key.as_bytes());
        all.push(0);
        all.extend_from_slice(i.json.as_bytes());
        all.push(0);
    }
    ew_format::sha256(&all)
}

/// The starter's entries.
fn starter_entries() -> Vec<Entry> {
    starter::items()
        .iter()
        .filter_map(|i| {
            let doc = LibDocument::from_json(i.json).ok()?;
            let (_, item, version) = identity(&doc)?;
            let line = record_line(&doc).ok()?;
            Some(Entry {
                item,
                version,
                line,
                doc,
            })
        })
        .collect()
}

/// The opened tiers and the search index (WP-S1.19 §5.2): the one resolver of library references.
pub struct Registry {
    tiers: LibraryTiers,
    libs: Vec<Lib>,
    starter: BTreeMap<Uuid, LibDocument>,
    index: Mutex<Index>,
    me: Identity,
    policy: LockPolicy,
    limits: Limits,
    fault: Option<WriteFault>,
    findings: Vec<String>,
    held_locks: BTreeSet<PathBuf>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("libraries", &self.libs.len())
            .field("findings", &self.findings)
            .finish_non_exhaustive()
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        for dir in std::mem::take(&mut self.held_locks) {
            let _ = lock::release(&dir, &self.me);
        }
    }
}

/// The library directories a tier path names: itself when it holds `library.json`, else its `*.ewlib` children
/// (directories, never links) in byte order of their names.
fn library_dirs(path: &Path) -> Vec<PathBuf> {
    if path.join(MANIFEST).is_file() {
        return vec![path.to_path_buf()];
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.file_name().to_string_lossy().ends_with(".ewlib"))
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

/// Reads and checks `dir`'s manifest.
fn open_manifest(dir: &Path, limits: &Limits) -> Result<LibManifest, String> {
    let bytes = read_file(dir, MANIFEST, limits.max_manifest_bytes)?;
    let m = LibManifest::parse(&bytes)?;
    if !m.readable() {
        return Err(format!(
            "library.json needs reader format {} (this reader: {})",
            m.min_reader,
            crate::manifest::FORMAT
        ));
    }
    if m.files.len() > limits.max_files {
        return Err(format!(
            "library.json lists {} files, more than the limit",
            m.files.len()
        ));
    }
    Ok(m)
}

/// Whether `version` satisfies the semantic-version requirement `req`: `*` or empty, `^X[.Y[.Z]]` (also bare, as
/// Cargo reads it), `~X[.Y[.Z]]`, `=X.Y.Z`. A requirement or version this reader cannot parse matches nothing.
pub fn version_matches(req: &str, version: &str) -> bool {
    fn parse(v: &str) -> Option<Vec<u64>> {
        let core = v.split(['-', '+']).next()?;
        let parts: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
        parts.filter(|p| (1..=3).contains(&p.len()))
    }
    let Some(v) = parse(version).filter(|v| v.len() == 3) else {
        return false;
    };
    let req = req.trim();
    if req.is_empty() || req == "*" {
        return true;
    }
    let (op, rest) = match req.as_bytes()[0] {
        b'^' | b'~' | b'=' => (req.as_bytes()[0], req[1..].trim()),
        _ => (b'^', req),
    };
    let Some(r) = parse(rest) else {
        return false;
    };
    let lower: Vec<u64> = (0..3).map(|i| r.get(i).copied().unwrap_or(0)).collect();
    if v < lower {
        return false;
    }
    match op {
        b'=' => r.len() == 3 && v == lower,
        b'~' => {
            if r.len() == 1 {
                v[0] == r[0]
            } else {
                v[0] == r[0] && v[1] == r[1]
            }
        }
        _ => {
            // Caret: the leftmost non-zero component of the requirement stays.
            match (r.len(), lower[0], lower[1]) {
                (1, _, _) => v[0] == r[0],
                (2, 0, _) => v[0] == 0 && v[1] == r[1],
                (2, _, _) => v[0] == r[0],
                (_, 0, 0) => v == lower,
                (_, 0, _) => v[0] == 0 && v[1] == lower[1],
                _ => v[0] == lower[0],
            }
        }
    }
}

/// Every `LibRef`-shaped object inside a JSON value (a macro's fragment).
fn refs_in(v: &Value, out: &mut Vec<(Uuid, Uuid, String)>) {
    match v {
        Value::Object(m) => {
            if let (Some(Value::String(l)), Some(Value::String(i)), Some(Value::String(r))) =
                (m.get("library"), m.get("item"), m.get("version_req"))
                && let (Ok(l), Ok(i)) = (Uuid::try_parse(l), Uuid::try_parse(i))
            {
                out.push((l, i, r.clone()));
            }
            m.values().for_each(|x| refs_in(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| refs_in(x, out)),
        _ => {}
    }
}

/// The references a document pulls into its closure (doc 05 §13.2): a part its template FnDefs, its macros and its
/// manufacturer; a symbol its FnDef; a macro the symbols, FnDefs, parts and forms its fragment names.
fn closure_refs(doc: &LibDocument) -> Vec<(Uuid, Uuid, String)> {
    let r = |x: &LibRef| (x.library.as_uuid(), x.item.as_uuid(), x.version_req.clone());
    match doc {
        LibDocument::Part(c) => {
            let mut out: Vec<_> = c.item.function_template.iter().map(|t| r(&t.fndef)).collect();
            out.extend(c.item.macros.iter().map(|m| r(&m.macro_ref)));
            out.push(r(&c.item.manufacturer));
            out
        }
        LibDocument::Symbol(c) => vec![r(&c.item.fndef)],
        LibDocument::Macro(c) => {
            let mut out = Vec::new();
            if let Ok(v) = serde_json::to_value(&c.item.variants) {
                refs_in(&v, &mut out);
            }
            out
        }
        _ => Vec::new(),
    }
}

/// The closure order of §12 item 3 after the item itself: FnDefs, symbols, parts, forms, manufacturers, macros.
fn closure_rank(doc: &LibDocument) -> u8 {
    match doc.kind() {
        Some(Kind::Fndef) => 0,
        Some(Kind::Symbol) => 1,
        Some(Kind::Part) => 2,
        Some(Kind::Form) => 3,
        Some(Kind::Manufacturer) => 4,
        _ => 5,
    }
}

fn io(e: std::io::Error) -> LibraryError {
    LibraryError::Io(e.to_string())
}

/// Writes `bytes` to `path` through a temporary sibling: write, flush, sync, rename.
fn write_atomic(path: &Path, bytes: &[u8], session: &str) -> Result<(), LibraryError> {
    let tmp = temp_name(path, session);
    write_temp(&tmp, bytes)?;
    std::fs::rename(&tmp, path).map_err(io)
}

fn temp_name(path: &Path, session: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".tmp-{session}"));
    path.with_file_name(name)
}

fn write_temp(tmp: &Path, bytes: &[u8]) -> Result<(), LibraryError> {
    let mut f = std::fs::File::create(tmp).map_err(io)?;
    f.write_all(bytes).map_err(io)?;
    f.flush().map_err(io)?;
    f.sync_all().map_err(io)
}

/// Syncs a directory where the platform can (not on Windows).
fn sync_dir(dir: &Path) {
    if cfg!(unix)
        && let Ok(f) = std::fs::File::open(dir)
    {
        let _ = f.sync_all();
    }
}

/// Whether `child` lies at or under `parent` (lexically, both made absolute where possible).
fn under(child: &Path, parent: &Path) -> bool {
    let abs = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    abs(child).starts_with(abs(parent))
}

impl Registry {
    /// Opens the tiers and the index, reindexing what changed (WP-S1.19 §7.3). A library that cannot be read is
    /// left out with a finding ([`Registry::findings`]); an index that cannot be used is deleted and rebuilt.
    pub fn open(tiers: &LibraryTiers) -> Result<Registry, LibraryError> {
        let path = tiers.cache.join(INDEX_FILE);
        let mut last = None;
        for _ in 0..2 {
            let (index, _) = Index::open_file(&path)?;
            let mut reg = Registry::with_index(tiers.clone(), index);
            match reg.reindex_all(false) {
                Ok(()) => return Ok(reg),
                Err(e) => {
                    // A failing index is never trusted: deleted and built once more from scratch.
                    drop(reg);
                    for suffix in ["", "-journal", "-wal", "-shm"] {
                        let mut p = path.as_os_str().to_owned();
                        p.push(suffix);
                        let _ = std::fs::remove_file(PathBuf::from(p));
                    }
                    last = Some(e);
                }
            }
        }
        Err(last.unwrap_or(LibraryError::Index("the index could not be built".into())))
    }

    /// A registry of the compiled-in starter only, with the index in memory (no tiers configured).
    pub fn starter_only() -> Registry {
        let index = Index::in_memory().unwrap_or_else(|e| panic!("an in-memory SQLite database: {e}"));
        let mut reg = Registry::with_index(LibraryTiers::default(), index);
        reg.libs.truncate(1);
        if let Err(e) = reg.reindex_all(false) {
            reg.findings.push(format!("index: {e}"));
        }
        reg
    }

    fn with_index(tiers: LibraryTiers, index: Index) -> Registry {
        let limits = Limits::default();
        let mut findings = Vec::new();
        let starter_docs: BTreeMap<Uuid, LibDocument> =
            starter_entries().into_iter().map(|e| (e.item, e.doc)).collect();
        let mut libs = vec![Lib {
            info: LibraryInfo {
                id: starter::LIBRARY,
                key: starter::KEY.to_owned(),
                name: "ElektraWatt starter library".to_owned(),
                tier: Tier::System,
                version: "0.1.0".to_owned(),
                licence: "CC-BY-SA-4.0".to_owned(),
                writable: false,
                path: None,
                generation: 0,
                index: IndexState::Fresh,
                findings: Vec::new(),
            },
            manifest: None,
        }];
        let mut seen: BTreeSet<Uuid> = BTreeSet::from([starter::LIBRARY]);
        let mut paths: Vec<(Tier, PathBuf)> = Vec::new();
        paths.extend(tiers.system.iter().map(|p| (Tier::System, p.clone())));
        paths.push((Tier::Community, tiers.community.clone()));
        paths.extend(tiers.company.iter().map(|p| (Tier::Company, p.clone())));
        paths.push((Tier::User, tiers.user.clone()));
        for (tier, p) in paths {
            if p.as_os_str().is_empty() {
                continue;
            }
            for dir in library_dirs(&p) {
                match open_manifest(&dir, &limits) {
                    Ok(m) => {
                        if !seen.insert(m.id) {
                            findings.push(format!(
                                "{}: library {} is installed twice; the first one is used",
                                dir.display(),
                                m.id
                            ));
                            continue;
                        }
                        let mut notes = Vec::new();
                        for path in m.files.keys() {
                            if let Err(e) = classify(path) {
                                notes.push(e);
                            }
                        }
                        libs.push(Lib {
                            info: LibraryInfo {
                                id: m.id,
                                key: m.key.clone(),
                                name: m.name.clone(),
                                tier,
                                version: m.version.clone(),
                                licence: m.licence.clone(),
                                writable: tier.writable(),
                                path: Some(dir),
                                generation: m.generation,
                                index: IndexState::Fresh,
                                findings: notes,
                            },
                            manifest: Some(m),
                        });
                    }
                    Err(e) => findings.push(format!("{}: {e}; the library is not used", dir.display())),
                }
            }
        }
        Registry {
            tiers,
            libs,
            starter: starter_docs,
            index: Mutex::new(index),
            me: Identity::current(concat!("ElektraWatt ", env!("CARGO_PKG_VERSION"))),
            policy: LockPolicy::default(),
            limits,
            fault: None,
            findings,
            held_locks: BTreeSet::new(),
        }
    }

    fn index(&self) -> std::sync::MutexGuard<'_, Index> {
        self.index
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Reads one records file of `lib`: checked against the manifest's hash, parsed; the skipped lines as notes.
    fn load_file(&self, lib: &Lib, path: &str) -> Result<Parsed, String> {
        let Some(dir) = lib.dir() else {
            return Ok((starter_entries(), Vec::new(), Vec::new()));
        };
        let Ok(PathRole::Records(key)) = classify(path) else {
            return Ok((Vec::new(), Vec::new(), Vec::new()));
        };
        let bytes = read_file(dir, path, self.limits.max_file_bytes)?;
        let expected = lib.manifest.as_ref().and_then(|m| m.files.get(path));
        if !expected.is_some_and(|h| hash_matches(h, &bytes)) {
            return Err(format!(
                "{path}: does not match its hash in library.json; skipped"
            ));
        }
        let (entries, skipped, kept) = parse_records(&bytes, key.kind, lib.info.id, &self.limits)?;
        Ok((
            entries,
            skipped.into_iter().map(|m| format!("{path}: {m}")).collect(),
            kept,
        ))
    }

    /// Reindexes every library (`whole`: from an empty index).
    fn reindex_all(&mut self, whole: bool) -> Result<(), LibraryError> {
        let keep: BTreeSet<Uuid> = self.libs.iter().map(|l| l.info.id).collect();
        {
            let mut index = self.index();
            if whole {
                index.clear()?;
            }
            index.retain(&keep)?;
        }
        // The manufacturers of every library that will be reindexed, so that a part finds a manufacturer of a
        // library indexed after its own.
        let mut todo = Vec::new();
        let mut fresh_libs = Vec::new();
        for (i, lib) in self.libs.iter().enumerate() {
            let state = self.index().state(lib.info.id)?;
            let files = lib.files();
            let fresh = state
                .as_ref()
                .is_some_and(|(g, p, f)| *g == lib.info.generation && *p == lib.index_path() && *f == files);
            if fresh {
                fresh_libs.push(i);
            } else {
                todo.push((i, state));
            }
        }
        for i in fresh_libs {
            self.libs[i].info.index = IndexState::Fresh;
        }
        let mut mfrs: BTreeMap<(Uuid, Uuid), MfrText> = BTreeMap::new();
        for (i, _) in &todo {
            let lib = &self.libs[*i];
            for (key, paths) in lib.shards() {
                if key.kind != Kind::Manufacturer {
                    continue;
                }
                for p in paths {
                    if let Ok((entries, _, _)) = self.load_file(lib, &p) {
                        for e in entries {
                            if let LibDocument::Manufacturer(c) = &e.doc {
                                let name = serde_json::to_value(&c.item.name)
                                    .map(|v| texts(&v))
                                    .unwrap_or_default();
                                let mut aliases: Vec<&str> =
                                    c.item.aliases.iter().map(|a| a.as_str()).collect();
                                aliases.extend(c.item.brands.iter().map(|b| b.as_str()));
                                mfrs.insert(
                                    (lib.info.id, e.item),
                                    MfrText {
                                        key: c.item.key.as_str().to_owned(),
                                        name,
                                        aliases: aliases.join(" "),
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
        for (i, state) in todo {
            self.reindex_lib(i, state, &mfrs)?;
        }
        Ok(())
    }

    /// Reindexes library `i` from the index's state `state` (`None`: not indexed).
    fn reindex_lib(
        &mut self,
        i: usize,
        state: Option<LibState>,
        mfrs: &BTreeMap<(Uuid, Uuid), MfrText>,
    ) -> Result<(), LibraryError> {
        let lib = &self.libs[i];
        let files = lib.files();
        let path = lib.index_path();
        let whole = state.as_ref().is_none_or(|(_, p, _)| *p != path);
        let old = match (&state, whole) {
            (Some((_, _, f)), false) => f.clone(),
            _ => BTreeMap::new(),
        };
        let removed: Vec<String> = old
            .iter()
            .filter(|(p, h)| files.get(*p) != Some(*h))
            .map(|(p, _)| p.clone())
            .collect();
        let mut tracked = files.clone();
        let mut added = Vec::new();
        let mut notes = Vec::new();
        for (p, h) in &files {
            if old.get(p) == Some(h) {
                continue;
            }
            let records = lib.manifest.is_none() || matches!(classify(p), Ok(PathRole::Records(_)));
            if !records {
                continue;
            }
            match self.load_file(lib, p) {
                Ok((entries, skipped, _)) => {
                    notes.extend(skipped);
                    added.push((p.clone(), entries));
                }
                Err(m) => {
                    notes.push(m);
                    // Retried at the next open.
                    tracked.insert(p.clone(), [0; 32]);
                }
            }
        }
        let id = lib.info.id;
        let generation = lib.info.generation;
        let lookup = |tx: &Transaction<'_>, l: Uuid, item: Uuid| {
            mfrs.get(&(l, item))
                .cloned()
                .or_else(|| Index::manufacturer(tx, l, item))
        };
        let result = self
            .index()
            .reindex(id, whole, generation, &path, &tracked, &removed, &added, &lookup);
        match result {
            Ok(more) => {
                notes.extend(more);
                let lib = &mut self.libs[i];
                lib.info.index = IndexState::Rebuilt;
                lib.info.findings.extend(notes);
                Ok(())
            }
            Err(e) => {
                self.libs[i].info.index = IndexState::Failed(e.to_string());
                Err(e)
            }
        }
    }

    /// The installed libraries in tier order, the compiled-in starter first.
    pub fn libraries(&self) -> Vec<LibraryInfo> {
        self.libs.iter().map(|l| l.info.clone()).collect()
    }

    /// The tiers this registry was opened on.
    pub fn tiers(&self) -> &LibraryTiers {
        &self.tiers
    }

    /// Libraries that could not be used, with the reason.
    pub fn findings(&self) -> &[String] {
        &self.findings
    }

    /// The index file; `None` for an index in memory. It is always under the tiers' cache directory (IX-4).
    pub fn index_path(&self) -> Option<PathBuf> {
        self.index().path().map(Path::to_path_buf)
    }

    /// The number of indexed parts and other items.
    pub fn index_counts(&self) -> Result<(u64, u64), LibraryError> {
        self.index().counts()
    }

    /// Sets the lock timing (`lock.stale_after`, the takeover's settle wait).
    pub fn set_lock_policy(&mut self, policy: LockPolicy) {
        self.policy = policy;
    }

    /// Arms (or, with `None`, disarms) a fault of the next writes: the fault-injection harness of IX-2.
    pub fn set_write_fault(&mut self, fault: Option<WriteFault>) {
        self.fault = fault;
    }

    fn lib(&self, id: Uuid) -> Option<&Lib> {
        self.libs.iter().find(|l| l.info.id == id)
    }

    fn tier_of(&self, id: Uuid) -> Option<Tier> {
        self.lib(id).map(|l| l.info.tier)
    }

    /// The document of `r` in its library, with copy metadata, if the library holds the item at a version the
    /// reference accepts.
    pub fn resolve(&self, r: &LibRef) -> Result<LibDocument, LibraryError> {
        self.resolve_parts(r.library.as_uuid(), r.item.as_uuid(), &r.version_req)
    }

    fn resolve_parts(&self, library: Uuid, item: Uuid, req: &str) -> Result<LibDocument, LibraryError> {
        let lib = self.lib(library).ok_or(LibraryError::NotFound)?;
        let doc = if lib.manifest.is_none() {
            self.starter.get(&item).cloned()
        } else {
            let shards = lib.shards();
            let mut found = None;
            for kind in [
                Kind::Part,
                Kind::Symbol,
                Kind::Fndef,
                Kind::Form,
                Kind::Macro,
                Kind::Manufacturer,
            ] {
                for p in shards.get(&ShardKey::of(kind, item)).into_iter().flatten() {
                    let (entries, _, _) = self.load_file(lib, p).map_err(LibraryError::Format)?;
                    if let Some(e) = entries.into_iter().find(|e| e.item == item) {
                        found = Some(e.doc);
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            found
        };
        let doc = doc.ok_or(LibraryError::NotFound)?;
        let (_, _, version) = identity(&doc).ok_or(LibraryError::NotFound)?;
        if !version_matches(req, &version) {
            return Err(LibraryError::NotFound);
        }
        Ok(doc)
    }

    /// The document of `r` and everything it pulls in (doc 05 §13.2), the item first, then FnDefs, symbols, parts,
    /// forms, manufacturers and macros, each by item ID. A reference that no installed library resolves is
    /// [`LibraryError::NotFound`]: a closure is complete or none.
    pub fn closure(&self, r: &LibRef) -> Result<Vec<LibDocument>, LibraryError> {
        let first = self.resolve(r)?;
        let start = (r.library.as_uuid(), r.item.as_uuid());
        let mut seen: BTreeSet<(Uuid, Uuid)> = BTreeSet::from([start]);
        let mut queue: Vec<(Uuid, Uuid, String)> = closure_refs(&first);
        let mut rest: Vec<LibDocument> = Vec::new();
        while let Some((l, i, req)) = queue.pop() {
            if !seen.insert((l, i)) {
                continue;
            }
            let doc = self.resolve_parts(l, i, &req)?;
            queue.extend(closure_refs(&doc));
            rest.push(doc);
        }
        rest.sort_by_key(|d| {
            let (l, i, _) = identity(d).unwrap_or_default();
            (closure_rank(d), i, l)
        });
        let mut out = vec![first];
        out.extend(rest);
        Ok(out)
    }

    /// The part search of D6 over every installed library; an index error gives no hits.
    pub fn search(&self, q: &PartQuery) -> Vec<PartHit> {
        self.try_search(q).unwrap_or_default()
    }

    /// [`Registry::search`] with the index error.
    pub fn try_search(&self, q: &PartQuery) -> Result<Vec<PartHit>, LibraryError> {
        let tier_of = |id: Uuid| self.tier_of(id);
        self.index().search_parts(q, &tier_of)
    }

    /// The search over symbols, FnDefs, forms, macros and manufacturers; an index error gives no hits.
    pub fn search_items(&self, q: &ItemQuery) -> Vec<ItemHit> {
        let tier_of = |id: Uuid| self.tier_of(id);
        self.index().search_items(q, &tier_of).unwrap_or_default()
    }

    /// Rebuilds the whole index from the libraries.
    pub fn rebuild_index(&mut self) -> Result<(), LibraryError> {
        for l in &mut self.libs {
            l.info.findings.clear();
        }
        self.reindex_all(true)
    }

    /// Takes over the stale lock of a writable library after the user confirmed it (doc 05 §7.4): the lock is then
    /// this registry's until its next [`Registry::write`] ends (or the registry is dropped).
    pub fn take_over_stale(&mut self, library: Uuid) -> Result<(), LibraryError> {
        let lib = self.lib(library).ok_or(LibraryError::NotFound)?;
        if !lib.info.writable {
            return Err(LibraryError::ReadOnly);
        }
        let dir = lib.dir().ok_or(LibraryError::ReadOnly)?.to_path_buf();
        lock::take_over(&dir, &self.me, &self.policy)?;
        self.held_locks.insert(dir);
        Ok(())
    }

    /// Writes documents into a writable library (WP-S1.19 §7.2): under the library's lock, every changed shard as a
    /// new file, then `library.json` with the next generation, last; then the files the old manifest named and the
    /// new one does not, and leftovers of an interrupted write, are removed. Each document's `library_id` must be
    /// the library's; its copy ID and hash are recomputed. A document of other content under a version the library
    /// holds is [`LibraryError::Immutable`] and nothing is written. Lines of a rewritten shard that this reader
    /// cannot type are kept as they are.
    pub fn write(&mut self, library: Uuid, docs: &[LibDocument]) -> Result<WriteReport, LibraryError> {
        let i = self
            .libs
            .iter()
            .position(|l| l.info.id == library)
            .ok_or(LibraryError::NotFound)?;
        if !self.libs[i].info.writable {
            return Err(LibraryError::ReadOnly);
        }
        let dir = self.libs[i].dir().ok_or(LibraryError::ReadOnly)?.to_path_buf();
        // The new lines by shard, checked before anything is locked or read.
        let mut incoming: BTreeMap<ShardKey, BTreeMap<Uuid, (String, String)>> = BTreeMap::new();
        for doc in docs {
            let kind = doc
                .kind()
                .ok_or_else(|| LibraryError::Format("a document of an unknown kind".into()))?;
            let (lib_id, item, version) =
                identity(doc).ok_or_else(|| LibraryError::Format("no identity".into()))?;
            if lib_id != library {
                return Err(LibraryError::Format(format!(
                    "item {item} names library {lib_id}, not {library}"
                )));
            }
            doc.validate()
                .map_err(|e| LibraryError::Format(format!("item {item}: {e}")))?;
            let line = record_line(doc).map_err(LibraryError::Format)?;
            let shard = incoming.entry(ShardKey::of(kind, item)).or_default();
            if shard.insert(item, (version, line)).is_some() {
                return Err(LibraryError::Format(format!("item {item} is given twice")));
            }
        }
        lock::acquire(&dir, &self.me, &self.policy)?;
        self.held_locks.insert(dir.clone());
        let result = self.write_locked(i, &dir, incoming);
        if !matches!(result, Err(LibraryError::Io(ref m)) if m.starts_with("injected fault")) {
            let _ = lock::release(&dir, &self.me);
            self.held_locks.remove(&dir);
        }
        result
    }

    fn write_locked(
        &mut self,
        i: usize,
        dir: &Path,
        incoming: BTreeMap<ShardKey, BTreeMap<Uuid, (String, String)>>,
    ) -> Result<WriteReport, LibraryError> {
        // The manifest on disk now, not the one of the open: another writer may have written since.
        let base = open_manifest(dir, &self.limits).map_err(LibraryError::Format)?;
        if base.id != self.libs[i].info.id {
            return Err(LibraryError::Format("library.json names another library".into()));
        }
        self.libs[i].manifest = Some(base.clone());
        let shards = self.libs[i].shards();
        let mut report = WriteReport {
            generation: base.generation,
            ..WriteReport::default()
        };
        let generation = base.generation + 1;
        let mut new_files: Vec<(String, Vec<u8>)> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        for (key, items) in &incoming {
            let mut lines: BTreeMap<Uuid, (String, String)> = BTreeMap::new();
            let mut kept: Vec<String> = Vec::new();
            for p in shards.get(key).into_iter().flatten() {
                let (entries, _, raw) = self.load_file(&self.libs[i], p).map_err(|m| {
                    LibraryError::Format(format!("{m}; the shard must be repaired before a write"))
                })?;
                for e in entries {
                    lines.insert(e.item, (e.version, e.line));
                }
                kept.extend(raw);
            }
            let mut changed = false;
            for (item, (version, line)) in items {
                match lines.get(item) {
                    Some((v, l)) if v == version && l == line => report.unchanged.push(*item),
                    Some((v, _)) if v == version => {
                        return Err(LibraryError::Immutable {
                            item: *item,
                            version: version.clone(),
                        });
                    }
                    Some(_) => {
                        report.updated.push(*item);
                        changed = true;
                    }
                    None => {
                        report.added.push(*item);
                        changed = true;
                    }
                }
                lines.insert(*item, (version.clone(), line.clone()));
            }
            if changed {
                let mut all: Vec<(Uuid, String)> = lines.into_iter().map(|(id, (_, l))| (id, l)).collect();
                all.extend(kept.into_iter().map(|l| (kept_line_id(&l), l)));
                let bytes = records_file(all.iter().map(|(id, l)| (*id, l.as_str())));
                new_files.push((key.file(generation), bytes));
                replaced.extend(shards.get(key).into_iter().flatten().cloned());
            }
        }
        for v in [&mut report.added, &mut report.updated, &mut report.unchanged] {
            v.sort();
        }
        if new_files.is_empty() {
            return Ok(report);
        }
        let session = self.me.session.clone();
        for (path, bytes) in &new_files {
            let full = join(dir, path);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
            }
            write_atomic(&full, bytes, &session)?;
            report.written.push(path.clone());
        }
        for (path, _) in &new_files {
            if let Some(parent) = join(dir, path).parent() {
                sync_dir(parent);
            }
        }
        if self.fault == Some(WriteFault::AfterShards) {
            return Err(LibraryError::Io("injected fault after the shards".into()));
        }
        let mut manifest = base.clone();
        manifest.generation = generation;
        for p in &replaced {
            manifest.files.remove(p);
        }
        for (path, bytes) in &new_files {
            manifest.files.insert(path.clone(), ew_format::sha256(bytes));
        }
        let manifest_path = dir.join(MANIFEST);
        let tmp = temp_name(&manifest_path, &session);
        write_temp(&tmp, manifest.to_text().as_bytes())?;
        if self.fault == Some(WriteFault::BeforeManifestRename) {
            return Err(LibraryError::Io(
                "injected fault before the manifest's rename".into(),
            ));
        }
        std::fs::rename(&tmp, &manifest_path).map_err(io)?;
        sync_dir(dir);
        report.generation = generation;
        report.written.push(MANIFEST.to_owned());
        // The commit point is behind us: what the new manifest does not name goes.
        report.removed = self.remove_unreferenced(dir, &base, &manifest);
        let lib = &mut self.libs[i];
        lib.info.generation = generation;
        lib.manifest = Some(manifest);
        let state = self.index().state(self.libs[i].info.id)?;
        self.reindex_lib(i, state, &BTreeMap::new())?;
        Ok(report)
    }

    /// Removes the files `old` named and `new` does not, and the records and temporary files no manifest names
    /// (an interrupted write's leftovers); never another file.
    fn remove_unreferenced(&self, dir: &Path, old: &LibManifest, new: &LibManifest) -> Vec<String> {
        let mut gone: BTreeSet<String> = old
            .files
            .keys()
            .filter(|p| !new.files.contains_key(*p))
            .cloned()
            .collect();
        let mut scan = |rel_dir: Option<&str>| {
            let at = rel_dir.map_or_else(|| dir.to_path_buf(), |d| dir.join(d));
            let Ok(entries) = std::fs::read_dir(&at) else {
                return;
            };
            for e in entries.flatten() {
                if !e.file_type().is_ok_and(|t| t.is_file()) {
                    continue;
                }
                let name = e.file_name().to_string_lossy().into_owned();
                let rel = rel_dir.map_or_else(|| name.clone(), |d| format!("{d}/{name}"));
                let leftover = name.contains(".tmp-") || matches!(classify(&rel), Ok(PathRole::Records(_)));
                if leftover && !new.files.contains_key(&rel) && rel != MANIFEST {
                    gone.insert(rel);
                }
            }
        };
        scan(None);
        for (d, _) in crate::record::KIND_DIRS {
            scan(Some(d));
        }
        let mut removed = Vec::new();
        for rel in gone {
            if classify(&rel).is_ok() && std::fs::remove_file(join(dir, &rel)).is_ok() {
                removed.push(rel);
            }
        }
        removed
    }

    /// Creates an empty library in `dir` (which must not exist or be empty) with a new ID, at generation 1, and adds
    /// it to the registry when `dir` lies in the user tier or a company path. Refused inside a read-only tier.
    pub fn create(&mut self, dir: &Path, key: &str, name: &str, licence: &str) -> Result<Uuid, LibraryError> {
        let read_only = self.tiers.system.iter().any(|p| under(dir, p))
            || (!self.tiers.community.as_os_str().is_empty() && under(dir, &self.tiers.community));
        if read_only {
            return Err(LibraryError::ReadOnly);
        }
        let key_ok = !key.is_empty()
            && key.len() <= 64
            && key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b));
        if !key_ok {
            return Err(LibraryError::Format(format!(
                "library key \"{key}\": use 1 to 64 of a-z, 0-9, '.', '_', '-'"
            )));
        }
        if name.trim().is_empty() || licence.trim().is_empty() {
            return Err(LibraryError::Format(
                "a library needs a name and a licence".into(),
            ));
        }
        if dir.exists() && std::fs::read_dir(dir).map_err(io)?.next().is_some() {
            return Err(LibraryError::Format(format!("{} is not empty", dir.display())));
        }
        std::fs::create_dir_all(dir).map_err(io)?;
        let id = ew_ids::Id::<()>::new_v7().as_uuid();
        let manifest = LibManifest::new(id, key, name.trim(), licence.trim());
        write_atomic(
            &dir.join(MANIFEST),
            manifest.to_text().as_bytes(),
            &self.me.session,
        )?;
        let tier = if !self.tiers.user.as_os_str().is_empty() && under(dir, &self.tiers.user) {
            Some(Tier::User)
        } else if self.tiers.company.iter().any(|p| under(dir, p)) {
            Some(Tier::Company)
        } else {
            None
        };
        if let Some(tier) = tier {
            self.libs.push(Lib {
                info: LibraryInfo {
                    id,
                    key: key.to_owned(),
                    name: manifest.name.clone(),
                    tier,
                    version: manifest.version.clone(),
                    licence: manifest.licence.clone(),
                    writable: true,
                    path: Some(dir.to_path_buf()),
                    generation: manifest.generation,
                    index: IndexState::Fresh,
                    findings: Vec::new(),
                },
                manifest: Some(manifest),
            });
            let i = self.libs.len() - 1;
            let state = self.index().state(id)?;
            self.reindex_lib(i, state, &BTreeMap::new())?;
        }
        Ok(id)
    }
}

/// The item ID a kept (untyped) line names, for its place in the shard; the nil ID when it names none.
fn kept_line_id(line: &str) -> Uuid {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|v| v.get("id")?.as_str().and_then(|s| Uuid::try_parse(s).ok()))
        .unwrap_or_else(Uuid::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_requirements() {
        for (req, v, ok) in [
            ("*", "1.2.3", true),
            ("", "0.0.1", true),
            ("^1", "1.9.0", true),
            ("^1", "2.0.0", false),
            ("^0.1", "0.1.7", true),
            ("^0.1", "0.2.0", false),
            ("^1.2.3", "1.2.2", false),
            ("^1.2.3", "1.4.0", true),
            ("^0.2.3", "0.2.9", true),
            ("^0.2.3", "0.3.0", false),
            ("^0.0.3", "0.0.4", false),
            ("1.2", "1.3.0", true),
            ("~1.2", "1.2.9", true),
            ("~1.2", "1.3.0", false),
            ("=1.2.3", "1.2.3", true),
            ("=1.2.3", "1.2.4", false),
            ("^x", "1.0.0", false),
            ("^1", "1.0", false),
        ] {
            assert_eq!(version_matches(req, v), ok, "{req} {v}");
        }
    }

    #[test]
    fn kept_lines_sort_by_their_id_else_last() {
        let id = Uuid::from_u128(5);
        assert_eq!(kept_line_id(&format!("{{\"id\":\"{id}\"}}")), id);
        assert_eq!(kept_line_id("not json"), Uuid::max());
    }
}
