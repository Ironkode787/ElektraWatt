// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The derived per-user search index (WP-S1.19 D5; doc 07 §4.3, §4.7): one SQLite file in the cache directory,
//! never authoritative, rebuilt per library when its generation or a file hash changes, and from scratch when it is
//! missing, unreadable or of another schema. It is opened with `trusted_schema=OFF` and in defensive mode.
//!
//! Tables: `library(lib, generation, path)`, `file(lib, path, sha256)`, `part` with the FTS5 tables `part_text`
//! (unicode61, prefixes 2 and 3) and `part_pn` (trigram), `part_fn` (the template's FnDefs, whose category and contact
//! kind are joined from `item` at query time, so a FnDef changed in another library is never stale), and `item` with
//! `item_text` for symbols, FnDefs, forms, macros and manufacturers. `part_prop` and `part_cp` come with 1.0.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ew_format::ew_schema::{ItemStatus, Kind, LibDocument, Part, PartKind};
use ew_ids::Uuid;
use rusqlite::config::DbConfig;
use rusqlite::types::Value as Sql;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params, params_from_iter};
use serde_json::Value;

use crate::LibraryError;
use crate::record::Entry;
use crate::search::{
    FnCompat, ItemHit, ItemQuery, OrderKey, PartHit, PartQuery, active, pn_norm, quantise, sort_by_order,
};
use crate::tiers::Tier;

/// The index file's name in the cache directory; the version in the name lets a newer build start fresh.
pub const INDEX_FILE: &str = "library-index-v1.sqlite";

/// The schema version (`PRAGMA user_version`).
const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE library(lib TEXT PRIMARY KEY, generation INTEGER NOT NULL, path TEXT NOT NULL);
CREATE TABLE file(lib TEXT NOT NULL, path TEXT NOT NULL, sha256 BLOB NOT NULL, PRIMARY KEY(lib, path));
CREATE TABLE part(id INTEGER PRIMARY KEY, lib TEXT NOT NULL, item TEXT NOT NULL, version TEXT NOT NULL,
  sha256 BLOB NOT NULL, file TEXT NOT NULL, mfr_lib TEXT NOT NULL, mfr_item TEXT NOT NULL, mfr_fallback TEXT NOT NULL,
  pn TEXT NOT NULL, pn_norm TEXT NOT NULL, type TEXT, kind TEXT NOT NULL, category TEXT NOT NULL, quality INTEGER,
  status TEXT NOT NULL, licence TEXT, descr TEXT NOT NULL);
CREATE UNIQUE INDEX part_lib_item ON part(lib, item);
CREATE INDEX part_lib_file ON part(lib, file);
CREATE INDEX part_pn_norm ON part(pn_norm);
CREATE VIRTUAL TABLE part_text USING fts5(pn, type, manufacturer, aliases, descr, keywords, class_names,
  tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3');
CREATE VIRTUAL TABLE part_pn USING fts5(pn_norm, tokenize = 'trigram');
CREATE TABLE part_fn(part INTEGER NOT NULL, fn_idx INTEGER NOT NULL, fndef_lib TEXT NOT NULL,
  fndef_item TEXT NOT NULL, cp_count INTEGER NOT NULL, main INTEGER NOT NULL);
CREATE INDEX part_fn_part ON part_fn(part);
CREATE TABLE item(id INTEGER PRIMARY KEY, lib TEXT NOT NULL, item TEXT NOT NULL, kind TEXT NOT NULL,
  version TEXT NOT NULL, sha256 BLOB NOT NULL, file TEXT NOT NULL, key TEXT NOT NULL, name TEXT NOT NULL,
  status TEXT NOT NULL, licence TEXT, cat0 TEXT, cat1 TEXT, contact TEXT);
CREATE UNIQUE INDEX item_kind_lib_item ON item(kind, lib, item);
CREATE INDEX item_lib_file ON item(lib, file);
CREATE VIRTUAL TABLE item_text USING fts5(key, name, keywords, aliases,
  tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3');
";

const TABLES: [&str; 9] = [
    "library",
    "file",
    "part",
    "part_text",
    "part_pn",
    "part_fn",
    "item",
    "item_text",
    "part_lib_item",
];

/// The state of one library in the index after [`Registry::open`](crate::Registry::open).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum IndexState {
    /// Nothing changed since the last build.
    Fresh,
    /// Changed files were reindexed.
    Rebuilt,
    /// Reindexing failed; the library's rows are those of the last good build, or none.
    Failed(String),
}

/// What the index holds for a library: its generation, its path and its files' hashes.
pub(crate) type LibState = (u64, String, BTreeMap<String, [u8; 32]>);

/// A manufacturer as the index's full text names it: key, name, aliases and brands.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct MfrText {
    pub key: String,
    pub name: String,
    pub aliases: String,
}

/// The index connection.
pub(crate) struct Index {
    conn: Connection,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Index")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;
    conn.execute_batch("PRAGMA trusted_schema = OFF; PRAGMA foreign_keys = OFF;")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

fn valid(conn: &Connection) -> rusqlite::Result<bool> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version != SCHEMA_VERSION {
        return Ok(false);
    }
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master")?;
    let names: BTreeSet<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    if !TABLES.iter().all(|t| names.contains(*t)) {
        return Ok(false);
    }
    // A cheap read of every table proves the pages are readable at all.
    for t in ["library", "file", "part", "item"] {
        conn.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get::<_, i64>(0))?;
    }
    Ok(true)
}

fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)?;
    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
}

fn remove_index_files(path: &Path) {
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let mut p = path.as_os_str().to_owned();
        p.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
}

impl Index {
    /// The index file `path`: opened and checked, else deleted and created empty (never trusted). The second value is
    /// true when the file was created from scratch.
    pub fn open_file(path: &Path) -> Result<(Index, bool), LibraryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existed = path.exists();
        if existed {
            let attempt = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .and_then(|c| {
                configure(&c)?;
                Ok(c)
            })
            .and_then(|c| valid(&c).map(|ok| (c, ok)));
            if let Ok((conn, true)) = attempt {
                return Ok((
                    Index {
                        conn,
                        path: Some(path.to_path_buf()),
                    },
                    false,
                ));
            }
            // Corrupt, truncated, foreign or of another schema: deleted and rebuilt.
            remove_index_files(path);
        }
        let conn = Connection::open(path)?;
        configure(&conn)?;
        create_schema(&conn)?;
        Ok((
            Index {
                conn,
                path: Some(path.to_path_buf()),
            },
            true,
        ))
    }

    /// An index in memory (a registry without a cache directory).
    pub fn in_memory() -> Result<Index, LibraryError> {
        let conn = Connection::open_in_memory()?;
        configure(&conn)?;
        create_schema(&conn)?;
        Ok(Index { conn, path: None })
    }

    /// The file, `None` in memory.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Empties the index (a full rebuild follows).
    pub fn clear(&mut self) -> Result<(), LibraryError> {
        self.conn.execute_batch(
            "BEGIN; DELETE FROM library; DELETE FROM file; DELETE FROM part; DELETE FROM part_text;
             DELETE FROM part_pn; DELETE FROM part_fn; DELETE FROM item; DELETE FROM item_text; COMMIT;",
        )?;
        Ok(())
    }

    /// Removes every library the index holds that is not in `keep`.
    pub fn retain(&mut self, keep: &BTreeSet<Uuid>) -> Result<(), LibraryError> {
        let held: Vec<String> = {
            let mut stmt = self.conn.prepare("SELECT lib FROM library")?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for lib in held {
            let gone = Uuid::try_parse(&lib).map_or(true, |u| !keep.contains(&u));
            if gone {
                let tx = self.conn.transaction()?;
                delete_library(&tx, &lib)?;
                tx.commit()?;
            }
        }
        Ok(())
    }

    /// The generation, path and file hashes the index holds for `lib`.
    pub fn state(&self, lib: Uuid) -> Result<Option<LibState>, LibraryError> {
        let l = lib.hyphenated().to_string();
        let row: Option<(i64, String)> = self
            .conn
            .query_row("SELECT generation, path FROM library WHERE lib = ?1", [&l], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        let Some((generation, path)) = row else {
            return Ok(None);
        };
        let mut stmt = self
            .conn
            .prepare("SELECT path, sha256 FROM file WHERE lib = ?1")?;
        let mut files = BTreeMap::new();
        for r in stmt.query_map([&l], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)))? {
            let (p, h) = r?;
            let h: [u8; 32] = h
                .try_into()
                .map_err(|_| LibraryError::Index("a bad file hash".into()))?;
            files.insert(p, h);
        }
        Ok(Some((u64::try_from(generation).unwrap_or(0), path, files)))
    }

    /// Reindexes library `lib` in one transaction: the files of `removed` lose their rows, the entries of `added`
    /// (by file) are inserted, and the library row and the file table become `generation`, `path`, `files`.
    /// `whole` first deletes every row of the library. Returns a message per item that could not be indexed.
    #[allow(clippy::too_many_arguments)]
    pub fn reindex(
        &mut self,
        lib: Uuid,
        whole: bool,
        generation: u64,
        path: &str,
        files: &BTreeMap<String, [u8; 32]>,
        removed: &[String],
        added: &[(String, Vec<Entry>)],
        mfr: &dyn Fn(&Transaction<'_>, Uuid, Uuid) -> Option<MfrText>,
    ) -> Result<Vec<String>, LibraryError> {
        let l = lib.hyphenated().to_string();
        let tx = self.conn.transaction()?;
        if whole {
            delete_library(&tx, &l)?;
        }
        for f in removed {
            delete_file(&tx, &l, f)?;
        }
        let mut notes = Vec::new();
        // Items first, so that parts of the same build find their manufacturers.
        for (file, entries) in added {
            for e in entries {
                if !matches!(e.doc, LibDocument::Part(_))
                    && let Err(m) = insert_item(&tx, &l, file, e)
                {
                    notes.push(format!("{file}: item {}: {m}", e.item));
                }
            }
        }
        for (file, entries) in added {
            for e in entries {
                if let LibDocument::Part(c) = &e.doc {
                    let m = mfr(
                        &tx,
                        c.item.manufacturer.library.as_uuid(),
                        c.item.manufacturer.item.as_uuid(),
                    );
                    if let Err(m) = insert_part(&tx, &l, file, e, &c.item, m) {
                        notes.push(format!("{file}: part {}: {m}", e.item));
                    }
                }
            }
        }
        tx.execute("DELETE FROM file WHERE lib = ?1", [&l])?;
        for (p, h) in files {
            tx.execute(
                "INSERT INTO file(lib, path, sha256) VALUES (?1, ?2, ?3)",
                params![&l, p, h.as_slice()],
            )?;
        }
        tx.execute(
            "INSERT INTO library(lib, generation, path) VALUES (?1, ?2, ?3)
             ON CONFLICT(lib) DO UPDATE SET generation = excluded.generation, path = excluded.path",
            params![&l, i64::try_from(generation).unwrap_or(i64::MAX), path],
        )?;
        tx.commit()?;
        Ok(notes)
    }

    /// The manufacturer `(lib, item)` from the indexed items, with the aliases and brands its item row holds.
    pub fn manufacturer(tx: &Transaction<'_>, lib: Uuid, item: Uuid) -> Option<MfrText> {
        tx.query_row(
            "SELECT item.key, item.name, item_text.aliases FROM item JOIN item_text ON item_text.rowid = item.id
             WHERE item.kind = 'manufacturer' AND item.lib = ?1 AND item.item = ?2",
            [lib.hyphenated().to_string(), item.hyphenated().to_string()],
            |r| {
                Ok(MfrText {
                    key: r.get(0)?,
                    name: r.get(1)?,
                    aliases: r.get::<_, Option<String>>(2)?.unwrap_or_default().trim().to_owned(),
                })
            },
        )
        .optional()
        .ok()
        .flatten()
    }

    /// The part search of D6; `tier_of` gives each library's tier (`None`: not installed, skipped).
    pub fn search_parts(
        &self,
        q: &PartQuery,
        tier_of: &dyn Fn(Uuid) -> Option<Tier>,
    ) -> Result<Vec<PartHit>, LibraryError> {
        let norm = pn_norm(&q.text);
        let fts = fts_query(&q.text);
        let mut found: BTreeMap<i64, RawPart> = BTreeMap::new();
        let compat = q.compatible_with.as_ref();
        if q.text.trim().is_empty() {
            self.part_tier(0, "part p", "1", vec![], "0.0", compat, &mut found)?;
        } else {
            if !norm.is_empty() {
                self.part_tier(
                    1,
                    "part p",
                    "p.pn_norm = ?6",
                    vec![Sql::Text(norm.clone())],
                    "0.0",
                    compat,
                    &mut found,
                )?;
                let hi = format!("{norm}\u{10FFFF}");
                self.part_tier(
                    2,
                    "part p",
                    "p.pn_norm > ?6 AND p.pn_norm < ?7",
                    vec![Sql::Text(norm.clone()), Sql::Text(hi)],
                    "0.0",
                    compat,
                    &mut found,
                )?;
            }
            if norm.chars().count() >= 3 {
                self.part_tier(
                    3,
                    "part_pn JOIN part p ON p.id = part_pn.rowid",
                    "part_pn MATCH ?6",
                    vec![Sql::Text(format!("\"{}\"", norm.replace('"', "\"\"")))],
                    "0.0",
                    compat,
                    &mut found,
                )?;
            }
            if let Some(fts) = fts {
                self.part_tier(
                    4,
                    "part_text JOIN part p ON p.id = part_text.rowid",
                    "part_text MATCH ?6",
                    vec![Sql::Text(fts)],
                    "bm25(part_text, 10.0, 6.0, 3.0, 3.0, 1.0, 2.0, 1.0)",
                    compat,
                    &mut found,
                )?;
            }
        }
        let held: BTreeSet<(Uuid, Uuid)> = q.held.iter().copied().collect();
        let boost: BTreeSet<(Uuid, Uuid)> = q.boost.iter().copied().collect();
        let mut keyed = Vec::new();
        let fndef = compat.and_then(|c| c.fndef);
        for raw in found.into_values() {
            let Some(tier) = tier_of(raw.lib) else {
                continue;
            };
            if !q.tiers.is_empty() && !q.tiers.contains(&tier) {
                continue;
            }
            let kind: PartKind = serde_json::from_str(&raw.kind).unwrap_or(PartKind::Unknown(Value::Null));
            if !q.kinds.is_empty() && !q.kinds.contains(&kind) {
                continue;
            }
            let levels: Vec<&str> = raw.category.split('\u{1F}').filter(|s| !s.is_empty()).collect();
            if !q.category.is_empty()
                && (levels.len() < q.category.len() || q.category.iter().zip(&levels).any(|(a, b)| a != b))
            {
                continue;
            }
            let status: ItemStatus = serde_json::from_str(&raw.status).unwrap_or_default();
            if !q.include_inactive && !active(&status) {
                continue;
            }
            let same_fndef = fndef.map(|_| raw.same_fndef);
            let fit = match (raw.compatible, same_fndef) {
                (_, Some(true)) => 0,
                (Some(false), _) => 2,
                _ => 1,
            };
            let key = OrderKey {
                fit,
                rank_tier: raw.rank_tier,
                not_held: !held.contains(&(raw.lib, raw.item)),
                not_boosted: !boost.contains(&(raw.lib, raw.item)),
                score: raw.score,
                part_number: raw.pn.clone(),
                manufacturer_key: raw.mfr_key.clone(),
                library: raw.lib,
                item: raw.item,
            };
            keyed.push((
                key,
                PartHit {
                    library: raw.lib,
                    item: raw.item,
                    version: raw.version,
                    tier,
                    part_number: raw.pn,
                    manufacturer: raw.mfr_name,
                    description: raw.descr,
                    kind,
                    status,
                    compatible: raw.compatible,
                    same_fndef,
                    rank_tier: raw.rank_tier,
                },
            ));
        }
        let mut hits = sort_by_order(keyed);
        if q.limit > 0 {
            hits.truncate(usize::try_from(q.limit).unwrap_or(usize::MAX));
        }
        Ok(hits)
    }

    #[allow(clippy::too_many_arguments)]
    fn part_tier(
        &self,
        tier: u8,
        from: &str,
        cond: &str,
        extra: Vec<Sql>,
        score: &str,
        compat: Option<&FnCompat>,
        found: &mut BTreeMap<i64, RawPart>,
    ) -> Result<(), LibraryError> {
        let sql = format!(
            "SELECT p.id, p.lib, p.item, p.version, p.pn, p.descr, p.kind, p.status, p.category,
                    COALESCE(m.name, p.mfr_fallback), COALESCE(m.key, p.mfr_fallback), {score},
                    CASE WHEN ?1 IS NULL THEN NULL ELSE EXISTS(
                      SELECT 1 FROM part_fn f JOIN item i
                        ON i.kind = 'fndef' AND i.lib = f.fndef_lib AND i.item = f.fndef_item
                      WHERE f.part = p.id AND i.cat0 = ?1 AND i.cat1 = ?2 AND i.contact = ?3) END,
                    CASE WHEN ?4 IS NULL THEN 0 ELSE EXISTS(
                      SELECT 1 FROM part_fn g WHERE g.part = p.id AND g.fndef_lib = ?4 AND g.fndef_item = ?5) END
             FROM {from}
             LEFT JOIN item m ON m.kind = 'manufacturer' AND m.lib = p.mfr_lib AND m.item = p.mfr_item
             WHERE {cond}"
        );
        let mut params: Vec<Sql> = match compat {
            Some(c) => vec![
                Sql::Text(c.category[0].clone()),
                Sql::Text(c.category[1].clone()),
                Sql::Text(json_text(&c.contact_kind)),
            ],
            None => vec![Sql::Null, Sql::Null, Sql::Null],
        };
        match compat.and_then(|c| c.fndef) {
            Some((lib, item)) => params.extend([
                Sql::Text(lib.hyphenated().to_string()),
                Sql::Text(item.hyphenated().to_string()),
            ]),
            None => params.extend([Sql::Null, Sql::Null]),
        }
        params.extend(extra);
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                RawPartRow {
                    lib: r.get(1)?,
                    item: r.get(2)?,
                    version: r.get(3)?,
                    pn: r.get(4)?,
                    descr: r.get(5)?,
                    kind: r.get(6)?,
                    status: r.get(7)?,
                    category: r.get(8)?,
                    mfr_name: r.get(9)?,
                    mfr_key: r.get(10)?,
                    score: r.get(11)?,
                    compatible: r.get::<_, Option<bool>>(12)?,
                    same_fndef: r.get::<_, bool>(13)?,
                },
            ))
        })?;
        for row in rows {
            let (id, row) = row?;
            if found.get(&id).is_some_and(|f| f.rank_tier <= tier) {
                continue;
            }
            let (Ok(lib), Ok(item)) = (Uuid::try_parse(&row.lib), Uuid::try_parse(&row.item)) else {
                continue;
            };
            found.insert(
                id,
                RawPart {
                    lib,
                    item,
                    version: row.version,
                    pn: row.pn,
                    descr: row.descr,
                    kind: row.kind,
                    status: row.status,
                    category: row.category,
                    mfr_name: row.mfr_name,
                    mfr_key: row.mfr_key,
                    score: quantise(row.score),
                    compatible: row.compatible,
                    same_fndef: row.same_fndef,
                    rank_tier: tier,
                },
            );
        }
        Ok(())
    }

    /// The item search: exact key (1), key prefix (2), full text over key, name, keywords and aliases (4).
    pub fn search_items(
        &self,
        q: &ItemQuery,
        tier_of: &dyn Fn(Uuid) -> Option<Tier>,
    ) -> Result<Vec<ItemHit>, LibraryError> {
        let text = q.text.trim().to_lowercase();
        let mut found: BTreeMap<i64, (RawItemRow, u8, i64)> = BTreeMap::new();
        let mut run = |tier: u8,
                       from: &str,
                       cond: &str,
                       p: Vec<Sql>,
                       score: &str|
         -> Result<(), LibraryError> {
            let sql = format!(
                "SELECT i.id, i.lib, i.item, i.kind, i.version, i.key, i.name, i.status, {score} FROM {from} WHERE {cond}"
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(p), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    RawItemRow {
                        lib: r.get(1)?,
                        item: r.get(2)?,
                        kind: r.get(3)?,
                        version: r.get(4)?,
                        key: r.get(5)?,
                        name: r.get(6)?,
                        status: r.get(7)?,
                    },
                    r.get::<_, f64>(8)?,
                ))
            })?;
            for row in rows {
                let (id, row, score) = row?;
                if found.get(&id).is_some_and(|f| f.1 <= tier) {
                    continue;
                }
                found.insert(id, (row, tier, quantise(score)));
            }
            Ok(())
        };
        if text.is_empty() {
            run(0, "item i", "1", vec![], "0.0")?;
        } else {
            run(
                1,
                "item i",
                "lower(i.key) = ?1",
                vec![Sql::Text(text.clone())],
                "0.0",
            )?;
            let n = i64::try_from(text.chars().count()).unwrap_or(i64::MAX);
            run(
                2,
                "item i",
                "substr(lower(i.key), 1, ?2) = ?1 AND lower(i.key) <> ?1",
                vec![Sql::Text(text.clone()), Sql::Integer(n)],
                "0.0",
            )?;
            if let Some(fts) = fts_query(&q.text) {
                run(
                    4,
                    "item_text JOIN item i ON i.id = item_text.rowid",
                    "item_text MATCH ?1",
                    vec![Sql::Text(fts)],
                    "bm25(item_text, 10.0, 3.0, 2.0, 3.0)",
                )?;
            }
        }
        let mut keyed = Vec::new();
        for (row, rank_tier, score) in found.into_values() {
            let (Ok(lib), Ok(item)) = (Uuid::try_parse(&row.lib), Uuid::try_parse(&row.item)) else {
                continue;
            };
            let Some(tier) = tier_of(lib) else {
                continue;
            };
            let Some(kind) = Kind::from_name(&row.kind) else {
                continue;
            };
            if (!q.tiers.is_empty() && !q.tiers.contains(&tier))
                || (!q.kinds.is_empty() && !q.kinds.contains(&kind))
            {
                continue;
            }
            let status: ItemStatus = serde_json::from_str(&row.status).unwrap_or_default();
            if !q.include_inactive && !active(&status) {
                continue;
            }
            keyed.push((
                OrderKey {
                    fit: 1,
                    rank_tier,
                    not_held: true,
                    not_boosted: true,
                    score,
                    part_number: row.key.clone(),
                    manufacturer_key: row.kind.clone(),
                    library: lib,
                    item,
                },
                ItemHit {
                    library: lib,
                    item,
                    version: row.version,
                    tier,
                    kind,
                    key: row.key,
                    name: row.name,
                    status,
                    rank_tier,
                },
            ));
        }
        let mut hits = sort_by_order(keyed);
        if q.limit > 0 {
            hits.truncate(usize::try_from(q.limit).unwrap_or(usize::MAX));
        }
        Ok(hits)
    }

    /// The number of indexed parts and items (tests, `ew lib index`).
    pub fn counts(&self) -> Result<(u64, u64), LibraryError> {
        let n = |t: &str| -> Result<u64, LibraryError> {
            let c: i64 = self
                .conn
                .query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))?;
            Ok(u64::try_from(c).unwrap_or(0))
        };
        Ok((n("part")?, n("item")?))
    }
}

struct RawPartRow {
    lib: String,
    item: String,
    version: String,
    pn: String,
    descr: String,
    kind: String,
    status: String,
    category: String,
    mfr_name: String,
    mfr_key: String,
    score: f64,
    compatible: Option<bool>,
    same_fndef: bool,
}

struct RawPart {
    lib: Uuid,
    item: Uuid,
    version: String,
    pn: String,
    descr: String,
    kind: String,
    status: String,
    category: String,
    mfr_name: String,
    mfr_key: String,
    score: i64,
    compatible: Option<bool>,
    same_fndef: bool,
    rank_tier: u8,
}

struct RawItemRow {
    lib: String,
    item: String,
    kind: String,
    version: String,
    key: String,
    name: String,
    status: String,
}

/// The FTS5 query of a typed text: every word as a quoted prefix term, all required; `None` without words.
fn fts_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// A value's compact JSON text (`"no"` for a contact kind).
fn json_text<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// Every string of a JSON value (the languages of an `MlText`), joined by " / ".
pub(crate) fn texts(v: &Value) -> String {
    fn walk<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
        match v {
            Value::String(s) => out.push(s),
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(m) => m.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out);
    out.join(" / ")
}

fn delete_rows(tx: &Transaction<'_>, cond: &str, p: &[&str]) -> rusqlite::Result<()> {
    tx.execute(
        &format!("DELETE FROM part_text WHERE rowid IN (SELECT id FROM part WHERE {cond})"),
        params_from_iter(p),
    )?;
    tx.execute(
        &format!("DELETE FROM part_pn WHERE rowid IN (SELECT id FROM part WHERE {cond})"),
        params_from_iter(p),
    )?;
    tx.execute(
        &format!("DELETE FROM part_fn WHERE part IN (SELECT id FROM part WHERE {cond})"),
        params_from_iter(p),
    )?;
    tx.execute(&format!("DELETE FROM part WHERE {cond}"), params_from_iter(p))?;
    tx.execute(
        &format!("DELETE FROM item_text WHERE rowid IN (SELECT id FROM item WHERE {cond})"),
        params_from_iter(p),
    )?;
    tx.execute(&format!("DELETE FROM item WHERE {cond}"), params_from_iter(p))?;
    Ok(())
}

fn delete_file(tx: &Transaction<'_>, lib: &str, file: &str) -> rusqlite::Result<()> {
    delete_rows(tx, "lib = ?1 AND file = ?2", &[lib, file])
}

fn delete_library(tx: &Transaction<'_>, lib: &str) -> rusqlite::Result<()> {
    delete_rows(tx, "lib = ?1", &[lib])?;
    tx.execute("DELETE FROM file WHERE lib = ?1", [lib])?;
    tx.execute("DELETE FROM library WHERE lib = ?1", [lib])?;
    Ok(())
}

fn copy_hash(doc: &LibDocument) -> Vec<u8> {
    ew_format::library_copy_hash(doc).map_or_else(Vec::new, |h| h.to_vec())
}

fn insert_item(tx: &Transaction<'_>, lib: &str, file: &str, e: &Entry) -> Result<(), String> {
    let kind = e.doc.kind().ok_or("an unknown kind")?;
    let item_value = serde_json::to_value(&e.doc).map_err(|x| x.to_string())?;
    let str_at = |v: &Value, path: &[&str]| -> Option<String> {
        path.iter()
            .try_fold(v, |v, k| v.get(*k))?
            .as_str()
            .map(str::to_owned)
    };
    let key = if kind == Kind::Manufacturer {
        str_at(&item_value, &["key"])
    } else {
        str_at(&item_value, &["meta", "key"])
    }
    .unwrap_or_default();
    let name = item_value
        .get("name")
        .map(texts)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| key.clone());
    let mut keywords = item_value.get("keywords").map(texts).unwrap_or_default();
    if let Some(d) = item_value.get("description").map(texts).filter(|d| !d.is_empty()) {
        keywords = format!("{keywords} {d}");
    }
    let mut aliases = item_value.get("aliases").map(texts).unwrap_or_default();
    if let Some(b) = item_value.get("brands").map(texts).filter(|b| !b.is_empty()) {
        aliases = format!("{aliases} {b}");
    }
    let status = item_value
        .get("meta")
        .and_then(|m| m.get("status"))
        .cloned()
        .unwrap_or(Value::from("released"));
    let licence = str_at(&item_value, &["meta", "licence"]);
    let (cat0, cat1, contact) = if kind == Kind::Fndef {
        let cat = item_value.get("category");
        let level = |i: usize| {
            cat.and_then(|c| c.get(i))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        (
            level(0),
            level(1),
            item_value.get("contact_kind").map(ToString::to_string),
        )
    } else {
        (None, None, None)
    };
    tx.execute(
        "INSERT INTO item(lib, item, kind, version, sha256, file, key, name, status, licence, cat0, cat1, contact)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(kind, lib, item) DO NOTHING",
        params![
            lib,
            e.item.hyphenated().to_string(),
            kind.name(),
            e.version,
            copy_hash(&e.doc),
            file,
            key,
            name,
            status.to_string(),
            licence,
            cat0,
            cat1,
            contact
        ],
    )
    .map_err(|x| x.to_string())?;
    if tx.changes() == 0 {
        return Err("listed twice in the library; the first is kept".to_owned());
    }
    let id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO item_text(rowid, key, name, keywords, aliases) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, key, name, keywords, aliases],
    )
    .map_err(|x| x.to_string())?;
    Ok(())
}

fn insert_part(
    tx: &Transaction<'_>,
    lib: &str,
    file: &str,
    e: &Entry,
    part: &Part,
    mfr: Option<MfrText>,
) -> Result<(), String> {
    let fallback = part
        .manufacturer
        .extra
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mfr = mfr.unwrap_or_else(|| MfrText {
        key: fallback.clone(),
        name: fallback.clone(),
        aliases: String::new(),
    });
    let pn = part.part_number.as_str().to_owned();
    let type_designation = part.type_designation.as_ref().map(|t| t.as_str().to_owned());
    let descr = texts(&serde_json::to_value(&part.description).map_err(|x| x.to_string())?);
    let category = part
        .category
        .as_ref()
        .map(|(a, b, c)| format!("{}\u{1F}{}\u{1F}{}", a.as_str(), b.as_str(), c.as_str()))
        .unwrap_or_default();
    let mut keywords: Vec<String> = Vec::new();
    keywords.extend(part.meta.key.as_ref().map(|k| k.as_str().to_owned()));
    keywords.extend(part.order_number.as_ref().map(|k| k.as_str().to_owned()));
    keywords.extend(part.gtin.as_ref().map(|k| k.as_str().to_owned()));
    for p in part.free_props.iter() {
        keywords.push(texts(&serde_json::to_value(&p.value).map_err(|x| x.to_string())?));
    }
    tx.execute(
        "INSERT INTO part(lib, item, version, sha256, file, mfr_lib, mfr_item, mfr_fallback, pn, pn_norm, type, kind,
                          category, quality, status, licence, descr)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, NULL, ?14, ?15, ?16)
         ON CONFLICT(lib, item) DO NOTHING",
        params![
            lib,
            e.item.hyphenated().to_string(),
            e.version,
            copy_hash(&e.doc),
            file,
            part.manufacturer.library.as_uuid().hyphenated().to_string(),
            part.manufacturer.item.as_uuid().hyphenated().to_string(),
            fallback,
            pn,
            pn_norm(&pn),
            type_designation,
            json_text(&part.kind),
            category,
            json_text(&part.meta.status),
            part.meta.licence,
            descr
        ],
    )
    .map_err(|x| x.to_string())?;
    if tx.changes() == 0 {
        return Err("listed twice in the library; the first is kept".to_owned());
    }
    let id = tx.last_insert_rowid();
    let manufacturer_text = if mfr.name == mfr.key {
        mfr.name.clone()
    } else {
        format!("{} {}", mfr.name, mfr.key)
    };
    tx.execute(
        "INSERT INTO part_text(rowid, pn, type, manufacturer, aliases, descr, keywords, class_names)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '')",
        params![
            id,
            pn,
            type_designation,
            manufacturer_text,
            mfr.aliases,
            descr,
            keywords.join(" ")
        ],
    )
    .map_err(|x| x.to_string())?;
    tx.execute(
        "INSERT INTO part_pn(rowid, pn_norm) VALUES (?1, ?2)",
        params![id, pn_norm(&pn)],
    )
    .map_err(|x| x.to_string())?;
    for (i, t) in part.function_template.iter().enumerate() {
        tx.execute(
            "INSERT INTO part_fn(part, fn_idx, fndef_lib, fndef_item, cp_count, main) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                i64::try_from(i).unwrap_or(i64::MAX),
                t.fndef.library.as_uuid().hyphenated().to_string(),
                t.fndef.item.as_uuid().hyphenated().to_string(),
                i64::try_from(t.cps.len()).unwrap_or(i64::MAX),
                t.main
            ],
        )
        .map_err(|x| x.to_string())?;
    }
    Ok(())
}
