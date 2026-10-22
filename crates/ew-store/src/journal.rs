// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The crash journal (doc 05 §8.1, ADR-0010): one JSON Lines segment per base generation in the OS state
//! directory, never in the project.
//!
//! A segment is `<state>/journal/<project id>-<8 hex of SHA-256(project path)>/<session>-<base>.jsonl`:
//!
//! ```text
//! {"j":"open","v":1,"project":"…","path":"<project path>","base":231,"base_manifest":"sha256:…","app_version":"…","session":"…"}
//! {"j":"tx","seq":17,"t":1792241530117,"cmd":"connection.set_overrides","origin":"gui","ops":[{"k":["connection","…"],"set":{…},"was":{…}}]}
//! {"j":"tx","seq":18,"t":1792241531002,"cmd":"project.company.set","origin":"gui","ops":[…],"assets":[{"ref":{"sha256":"sha256:…","format":"svg","size":[80,20]},"b64":"PHN2Zy…"}]}
//! {"j":"save-begin","gen":232,"writes":17,"removes":["devices/9f.jsonl"]}
//! {"j":"save-done","gen":232}
//! ```
//!
//! Every entry is appended and synced at once, which meets "within 1 s" (NFR-REL-02) without a group
//! commit timer. A successful save starts a new segment and deletes the old one; a clean close deletes the
//! session's segments (NFR-PRIV-05). Snapshot slots (doc 05 §8.1 "Snapshots") are not in v0: recovery
//! replays the whole segment onto its base generation.
//!
//! **Assets** (FT-337). A transaction that brings assets into the model's asset store (doc 05 §13.3) journals
//! them in `assets`, each with its reference and its bytes in base64, so that a recovery restores the bytes that
//! its ops name. An asset whose bytes do not hash to its reference, or do not sniff as its format, is dropped on
//! reading (its line still counts).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use ew_format::ew_schema::{AnyId, AssetRef, Kind, base64_decode, base64_encode};
use ew_format::{FilePath, hash_text, sha256};
use ew_model::{AnyKey, AnyRow, Asset, Model};
use serde_json::{Map, Value};

use crate::rows::{row_value, value_row};
use crate::vfs::{EntryKind, Vfs};

/// How long a leftover journal is kept (doc 05 §1: pruned after 30 days, NFR-PRIV-05).
pub const KEEP_LEFTOVERS: Duration = Duration::from_secs(30 * 24 * 3600);

/// A compact JSON object with members in the given order (journal lines are read by `ew recover` and in
/// support cases; the order of doc 05 §8.1 keeps them readable).
fn line(members: &[(&str, Value)]) -> String {
    let mut out = String::from("{");
    for (i, (k, v)) in members.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&Value::from(*k).to_string());
        out.push(':');
        out.push_str(&v.to_string());
    }
    out.push_str("}\n");
    out
}

/// The journal directory of a project: `<state>/journal/<project id>-<8 hex of SHA-256(path)>` (doc 05
/// §8.1). `path` should be the canonical project path; only its text is hashed (with `\` as `/`).
pub fn journal_dir(state: &Path, project: AnyId, path: &Path) -> PathBuf {
    let text = path.to_string_lossy().replace('\\', "/");
    let hash = hash_text(&sha256(text.as_bytes()));
    state.join("journal").join(format!("{project}-{}", &hash[7..15]))
}

/// One field-level change of a row (doc 05 §8.1 `ops`).
#[derive(Clone, PartialEq, Debug)]
pub enum JournalOp {
    /// A new row.
    Put {
        /// The key.
        key: AnyKey,
        /// The row's members (without `$k`).
        row: Value,
    },
    /// A removed row.
    Del {
        /// The key.
        key: AnyKey,
        /// The removed row's members.
        row: Value,
    },
    /// Changed members: `set` holds the after-values, `was` the before-values; `null` means absent.
    Set {
        /// The key.
        key: AnyKey,
        /// After-values.
        set: Map<String, Value>,
        /// Before-values.
        was: Map<String, Value>,
    },
}

impl JournalOp {
    /// The diff of one row, `None` when nothing changed.
    pub fn diff(
        key: AnyKey,
        before: Option<&AnyRow>,
        after: Option<&AnyRow>,
    ) -> Result<Option<JournalOp>, String> {
        let value = |r: &AnyRow| row_value(r).map_err(|e| e.to_string());
        Ok(match (before, after) {
            (None, None) => None,
            (None, Some(a)) => Some(JournalOp::Put { key, row: value(a)? }),
            (Some(b), None) => Some(JournalOp::Del { key, row: value(b)? }),
            (Some(b), Some(a)) => {
                let (Value::Object(bv), Value::Object(av)) = (value(b)?, value(a)?) else {
                    return Err("a row is not a JSON object".into());
                };
                let mut set = Map::new();
                let mut was = Map::new();
                let names: std::collections::BTreeSet<&String> = bv.keys().chain(av.keys()).collect();
                for name in names {
                    let (old, new) = (bv.get(name), av.get(name));
                    if old != new {
                        set.insert(name.clone(), new.cloned().unwrap_or(Value::Null));
                        was.insert(name.clone(), old.cloned().unwrap_or(Value::Null));
                    }
                }
                (!set.is_empty()).then_some(JournalOp::Set { key, set, was })
            }
        })
    }

    fn key(&self) -> AnyKey {
        match self {
            JournalOp::Put { key, .. } | JournalOp::Del { key, .. } | JournalOp::Set { key, .. } => *key,
        }
    }

    fn to_value(&self) -> Value {
        let k = Value::Array(vec![
            Value::from(self.key().0.name()),
            Value::from(self.key().1.to_string()),
        ]);
        let mut m = Map::new();
        m.insert("k".into(), k);
        match self {
            JournalOp::Put { row, .. } => {
                m.insert("put".into(), row.clone());
            }
            JournalOp::Del { row, .. } => {
                m.insert("del".into(), row.clone());
            }
            JournalOp::Set { set, was, .. } => {
                m.insert("set".into(), Value::Object(set.clone()));
                m.insert("was".into(), Value::Object(was.clone()));
            }
        }
        Value::Object(m)
    }

    fn from_value(v: &Value) -> Option<JournalOp> {
        let k = v.get("k")?.as_array()?;
        let kind = Kind::from_name(k.first()?.as_str()?)?;
        let id: AnyId = k.get(1)?.as_str()?.parse().ok()?;
        let key = (kind, id);
        if let Some(row) = v.get("put") {
            return Some(JournalOp::Put {
                key,
                row: row.clone(),
            });
        }
        if let Some(row) = v.get("del") {
            return Some(JournalOp::Del {
                key,
                row: row.clone(),
            });
        }
        Some(JournalOp::Set {
            key,
            set: v.get("set")?.as_object()?.clone(),
            was: v.get("was")?.as_object()?.clone(),
        })
    }
}

/// One entry of a segment after its `open` line.
#[derive(Clone, PartialEq, Debug)]
pub enum Entry {
    /// A committed transaction.
    Tx {
        /// Its sequence number in the session.
        seq: u64,
        /// Epoch milliseconds (for the recovery dialog only).
        t: u64,
        /// The command name.
        cmd: String,
        /// The origin (`gui`, `cli`, …).
        origin: String,
        /// The row changes.
        ops: Vec<JournalOp>,
        /// The assets the transaction brought (absent in older segments: none).
        assets: Vec<Asset>,
    },
    /// A save passed its commit point.
    SaveBegin {
        /// The new generation.
        generation: u64,
        /// The files the save removes.
        removes: Vec<String>,
    },
    /// A save completed.
    SaveDone {
        /// The generation.
        generation: u64,
    },
}

/// A segment read back.
#[derive(Clone, PartialEq, Debug)]
pub struct Segment {
    /// Its file.
    pub path: PathBuf,
    /// The session that wrote it.
    pub session: String,
    /// The base generation.
    pub base: u64,
    /// The base manifest's hash (`None`: the project had no manifest).
    pub base_manifest: Option<String>,
    /// The entries up to the first torn line.
    pub entries: Vec<Entry>,
    /// The modification time of the segment file.
    pub modified: Option<SystemTime>,
}

impl Segment {
    /// Reads a segment; `None` when its `open` line is unreadable.
    pub fn parse(path: PathBuf, bytes: &[u8], modified: Option<SystemTime>) -> Option<Segment> {
        let mut lines = bytes.split(|&b| b == b'\n');
        let open: Value = serde_json::from_slice(lines.next()?).ok()?;
        if open.get("j")?.as_str()? != "open" {
            return None;
        }
        let mut seg = Segment {
            path,
            session: open.get("session")?.as_str()?.to_owned(),
            base: open.get("base")?.as_u64()?,
            base_manifest: open
                .get("base_manifest")
                .and_then(Value::as_str)
                .map(str::to_owned),
            entries: Vec::new(),
            modified,
        };
        // A line is complete only with its LF: the last piece of a split is torn unless empty.
        let pieces: Vec<&[u8]> = lines.collect();
        let complete = pieces.len().saturating_sub(1);
        for piece in &pieces[..complete] {
            let Ok(v) = serde_json::from_slice::<Value>(piece) else {
                break;
            };
            let entry = match v.get("j").and_then(Value::as_str) {
                Some("tx") => {
                    let ops: Option<Vec<JournalOp>> = v
                        .get("ops")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().map(JournalOp::from_value).collect())
                        .unwrap_or(None);
                    let Some(ops) = ops else { break };
                    Entry::Tx {
                        seq: v.get("seq").and_then(Value::as_u64).unwrap_or(0),
                        t: v.get("t").and_then(Value::as_u64).unwrap_or(0),
                        cmd: v.get("cmd").and_then(Value::as_str).unwrap_or("").to_owned(),
                        origin: v.get("origin").and_then(Value::as_str).unwrap_or("").to_owned(),
                        ops,
                        assets: v
                            .get("assets")
                            .and_then(Value::as_array)
                            .map(|a| a.iter().filter_map(asset_from_value).collect())
                            .unwrap_or_default(),
                    }
                }
                Some("save-begin") => Entry::SaveBegin {
                    generation: v.get("gen").and_then(Value::as_u64).unwrap_or(0),
                    removes: v
                        .get("removes")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
                        .unwrap_or_default(),
                },
                Some("save-done") => Entry::SaveDone {
                    generation: v.get("gen").and_then(Value::as_u64).unwrap_or(0),
                },
                _ => break,
            };
            seg.entries.push(entry);
        }
        Some(seg)
    }

    /// The transactions not in generation `disk_gen`: those after the last `save-begin` of `disk_gen`
    /// (that save passed its commit point, so its snapshot is on disk), else all of them.
    pub fn unsaved(&self, disk_gen: u64) -> Vec<&Entry> {
        let start = self
            .entries
            .iter()
            .rposition(|e| matches!(e, Entry::SaveBegin { generation, .. } if *generation == disk_gen))
            .map_or(0, |i| i + 1);
        self.entries[start..]
            .iter()
            .filter(|e| matches!(e, Entry::Tx { .. }))
            .collect()
    }

    /// Epoch milliseconds of the last unsaved transaction against generation `disk_gen` (0 without one).
    pub fn last_change(&self, disk_gen: u64) -> u64 {
        self.unsaved(disk_gen)
            .iter()
            .filter_map(|e| match e {
                Entry::Tx { t, .. } => Some(*t),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// The removals of the last `save-begin` of generation `generation`.
    pub fn removes_of(&self, generation: u64) -> Vec<String> {
        self.entries
            .iter()
            .rev()
            .find_map(|e| match e {
                Entry::SaveBegin {
                    generation: g,
                    removes,
                } if *g == generation => Some(removes.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
}

/// The journal of one session.
#[derive(Debug)]
pub struct Journal {
    vfs: Arc<dyn Vfs>,
    dir: PathBuf,
    session: String,
    project: AnyId,
    path_text: String,
    app_version: String,
    segment: Option<PathBuf>,
    seq: u64,
}

impl Journal {
    /// The journal of `session` for the project `project` at `path`, under the state directory `state`.
    /// The directory is keyed by the canonical form of `path` ([`Vfs::canonicalize`]), so that one project
    /// reached through two paths has one journal; when the path cannot be canonicalised, by `path` as given.
    pub fn new(
        vfs: Arc<dyn Vfs>,
        state: &Path,
        project: AnyId,
        path: &Path,
        session: &str,
        app_version: &str,
    ) -> Journal {
        let path = vfs.canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        Journal {
            vfs,
            dir: journal_dir(state, project, &path),
            session: session.to_owned(),
            project,
            path_text: path.to_string_lossy().into_owned(),
            app_version: app_version.to_owned(),
            segment: None,
            seq: 0,
        }
    }

    /// The journal directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn append(&self, text: &str) -> io::Result<()> {
        let Some(seg) = &self.segment else {
            return Ok(());
        };
        self.vfs.append(seg, text.as_bytes())?;
        self.vfs.sync(seg)
    }

    /// Starts the segment of base generation `base` and deletes the session's previous segment.
    pub fn start(&mut self, base: u64, base_manifest: Option<&[u8; 32]>) -> io::Result<()> {
        self.vfs.create_dir_all(&self.dir)?;
        let path = self.dir.join(format!("{}-{base}.jsonl", self.session));
        let text = line(&[
            ("j", "open".into()),
            ("v", 1.into()),
            ("project", self.project.to_string().into()),
            ("path", self.path_text.clone().into()),
            ("base", base.into()),
            (
                "base_manifest",
                base_manifest.map_or(Value::Null, |h| hash_text(h).into()),
            ),
            ("app_version", self.app_version.clone().into()),
            ("session", self.session.clone().into()),
        ]);
        if self.vfs.stat(&path).is_ok() {
            self.vfs.remove_file(&path)?;
        }
        self.vfs.append(&path, text.as_bytes())?;
        self.vfs.sync(&path)?;
        self.vfs.sync_dir(&self.dir)?;
        // `create_dir_all` may have made `journal/` and the project's directory: their names are durable
        // only with their parents synced (`<state>/journal`, `<state>`).
        let mut parent = self.dir.parent();
        for _ in 0..2 {
            let Some(dir) = parent else { break };
            self.vfs.sync_dir(dir)?;
            parent = dir.parent();
        }
        let old = self.segment.replace(path);
        if let Some(old) = old {
            if Some(&old) != self.segment.as_ref() {
                let _ = self.vfs.remove_file(&old);
            }
        }
        Ok(())
    }

    /// Journals a committed transaction: the field-level diff of `keys` from `before` to `after`.
    pub fn tx(
        &mut self,
        t: SystemTime,
        cmd: &str,
        origin: &str,
        before: &Model,
        after: &Model,
        keys: &[AnyKey],
    ) -> io::Result<()> {
        let mut ops = Vec::new();
        for key in keys {
            let op = JournalOp::diff(*key, before.row(key).as_ref(), after.row(key).as_ref())
                .map_err(io::Error::other)?;
            ops.extend(op.map(|o| o.to_value()));
        }
        if ops.is_empty() {
            return Ok(());
        }
        let assets: Vec<Value> = after
            .assets()
            .filter(|a| before.asset(a.sha256()).is_none())
            .map(asset_to_value)
            .collect::<Result<_, _>>()?;
        self.seq += 1;
        let mut members = vec![
            ("j", "tx".into()),
            ("seq", self.seq.into()),
            ("t", crate::clock::epoch_ms(t).into()),
            ("cmd", cmd.into()),
            ("origin", origin.into()),
            ("ops", Value::Array(ops)),
        ];
        if !assets.is_empty() {
            members.push(("assets", Value::Array(assets)));
        }
        self.append(&line(&members))
    }

    /// Journals that a save of generation `generation` passed its commit point (doc 05 §5.2 step 5).
    pub fn save_begin(&mut self, generation: u64, writes: usize, removes: &[FilePath]) -> io::Result<()> {
        self.append(&line(&[
            ("j", "save-begin".into()),
            ("gen", generation.into()),
            ("writes", writes.into()),
            (
                "removes",
                Value::Array(removes.iter().map(|p| Value::from(p.as_str())).collect()),
            ),
        ]))
    }

    /// Journals a completed save and starts the segment of the new base (doc 05 §5.2 step 9).
    pub fn save_done(&mut self, generation: u64, manifest: &[u8; 32]) -> io::Result<()> {
        self.append(&line(&[("j", "save-done".into()), ("gen", generation.into())]))?;
        self.start(generation, Some(manifest))
    }

    /// The segments of other sessions of this project (leftovers of a crash), newest base first; segments
    /// older than [`KEEP_LEFTOVERS`] are deleted on the way (NFR-PRIV-05).
    pub fn leftovers(&self, now: SystemTime) -> Vec<Segment> {
        let Ok(entries) = self.vfs.list(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (name, kind) in entries {
            if kind != EntryKind::File || !name.ends_with(".jsonl") || name.starts_with(&self.session) {
                continue;
            }
            let path = self.dir.join(&name);
            let modified = self.vfs.stat(&path).ok().and_then(|s| s.modified);
            if modified.is_some_and(|m| now.duration_since(m).is_ok_and(|age| age > KEEP_LEFTOVERS)) {
                let _ = self.vfs.remove_file(&path);
                continue;
            }
            let Ok(bytes) = self.vfs.read(&path) else { continue };
            if let Some(seg) = Segment::parse(path, &bytes, modified) {
                out.push(seg);
            }
        }
        out.sort_by(|a, b| b.base.cmp(&a.base).then_with(|| b.modified.cmp(&a.modified)));
        out
    }

    /// Deletes leftover segments (after a recovery or a "discard").
    pub fn discard(&self, segments: &[Segment]) {
        for seg in segments {
            let _ = self.vfs.remove_file(&seg.path);
        }
    }

    /// A clean close: deletes this session's segment and the directory when it is empty (NFR-PRIV-05).
    pub fn close(&mut self) {
        if let Some(seg) = self.segment.take() {
            let _ = self.vfs.remove_file(&seg);
        }
        if self.vfs.list(&self.dir).is_ok_and(|e| e.is_empty()) {
            let _ = self.vfs.remove_dir(&self.dir);
        }
    }
}

/// A field whose current value differs from the journalled before-value: the base moved and both sides
/// changed it (doc 05 §8.2 step 4; resolved in the merge panel, not in v0). Also a library copy that the
/// replay removes from the moved base while a record of the result still names it through a `LibRef`
/// (the delete-time rule EW-INV-021, ADR-0009 amendment 7): the other side named the copy that this side
/// deleted.
#[derive(Clone, PartialEq, Debug)]
pub struct Collision {
    /// The row.
    pub key: AnyKey,
    /// The member (`""` for a whole-row put or delete); for a removed library copy, the `LibRef` link
    /// (`kind.field`) through which a record still names it.
    pub member: String,
}

/// The journal form of an asset: `{"ref": <AssetRef>, "b64": <bytes>}`.
fn asset_to_value(asset: &Asset) -> io::Result<Value> {
    let reference = serde_json::to_value(asset.reference()).map_err(io::Error::other)?;
    let mut m = Map::new();
    m.insert("ref".into(), reference);
    m.insert("b64".into(), Value::from(base64_encode(asset.bytes())));
    Ok(Value::Object(m))
}

/// An asset read back from its journal form; `None` when the bytes do not hash to the reference or do not sniff
/// as its format.
fn asset_from_value(v: &Value) -> Option<Asset> {
    let reference: AssetRef = serde_json::from_value(v.get("ref")?.clone()).ok()?;
    let bytes = base64_decode(v.get("b64")?.as_str()?).ok()?;
    if sha256(&bytes) != reference.sha256.0 {
        return None;
    }
    let (format, size) = ew_format::ew_schema::sniff(&bytes).ok()?;
    (format == reference.format && size == reference.size).then(|| Asset::new(reference, bytes))
}

/// Replays journalled transactions onto `model` (doc 05 §8.2). Each transaction becomes one batch of ops
/// applied as a new revision on the history path (`Model::apply_history`): the journal holds committed
/// transactions, undos and redos among them. With `check` (the base moved), every before-value is compared
/// with the model first and differences are returned as collisions instead of being applied; and the
/// replay's net removals are judged against the moved base as an edit on it would be (EW-INV-021): a library
/// copy of the moved base that the replay removes while a record of the result names it through a `LibRef`
/// is a collision, not a removal. The judgement is net, so a copy that the journal embeds and an undo removes
/// again is no collision.
pub fn replay(model: &Model, txs: &[&Entry], check: bool) -> Result<(Model, Vec<AnyKey>), ReplayError> {
    let base = model;
    let mut model = model.clone();
    let mut touched = std::collections::BTreeSet::new();
    let mut collisions = Vec::new();
    for entry in txs {
        let Entry::Tx { ops, assets, .. } = entry else {
            continue;
        };
        for asset in assets {
            model.put_asset(asset.clone());
        }
        let mut batch = Vec::new();
        for op in ops {
            let key = op.key();
            let current = model.row(&key);
            let current_value = current
                .as_ref()
                .map(row_value)
                .transpose()
                .map_err(|e| ReplayError::Invalid(e.to_string()))?;
            let after = match op {
                JournalOp::Put { row, .. } => {
                    if check && current_value.as_ref().is_some_and(|c| c != row) {
                        collisions.push(Collision {
                            key,
                            member: String::new(),
                        });
                    }
                    Some(value_row(key.0, row.clone()).map_err(ReplayError::Invalid)?)
                }
                JournalOp::Del { row, .. } => {
                    if check && current_value.as_ref() != Some(row) {
                        collisions.push(Collision {
                            key,
                            member: String::new(),
                        });
                    }
                    None
                }
                JournalOp::Set { set, was, .. } => {
                    let Some(Value::Object(mut obj)) = current_value.clone() else {
                        collisions.push(Collision {
                            key,
                            member: String::new(),
                        });
                        continue;
                    };
                    for (name, new) in set {
                        let old = obj.get(name).cloned().unwrap_or(Value::Null);
                        if check && was.get(name).cloned().unwrap_or(Value::Null) != old {
                            collisions.push(Collision {
                                key,
                                member: name.clone(),
                            });
                        }
                        if new.is_null() {
                            obj.remove(name);
                        } else {
                            obj.insert(name.clone(), new.clone());
                        }
                    }
                    Some(value_row(key.0, Value::Object(obj)).map_err(ReplayError::Invalid)?)
                }
            };
            touched.insert(key);
            batch.push(match (current, after) {
                (None, Some(a)) => ew_model::Op::insert(a),
                (Some(b), None) => ew_model::Op::delete(b),
                (Some(b), Some(a)) => ew_model::Op::update(b, a),
                (None, None) => continue,
            });
        }
        if !collisions.is_empty() {
            continue;
        }
        let rev = ew_model::Rev(model.rev().0 + 1);
        model = model
            .apply_history(&batch, rev)
            .map_err(|e| ReplayError::Invalid(e.to_string()))?;
    }
    if check && collisions.is_empty() {
        collisions = lib_ref_collisions(base, &model, &touched);
    }
    if !collisions.is_empty() {
        return Err(ReplayError::Collisions(collisions));
    }
    Ok((model, touched.into_iter().collect()))
}

/// The library copies of `base` that `after` no longer holds while one of its records names them through a
/// `LibRef` link, among the `touched` keys: one collision per copy, at its first such link.
fn lib_ref_collisions(
    base: &Model,
    after: &Model,
    touched: &std::collections::BTreeSet<AnyKey>,
) -> Vec<Collision> {
    touched
        .iter()
        .filter(|key| base.contains(key) && !after.contains(key))
        .filter_map(|key| {
            let (link, _) = after.referrers(key).into_iter().find(|(link, _)| link.lib_ref)?;
            Some(Collision {
                key: *key,
                member: link.path(),
            })
        })
        .collect()
}

/// Why a replay failed.
#[derive(Clone, PartialEq, Debug)]
pub enum ReplayError {
    /// The base moved and fields changed on both sides.
    Collisions(Vec<Collision>),
    /// A journalled row does not read back, or the replayed model breaks an invariant.
    Invalid(String),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Collisions(c) => write!(f, "{} fields changed on disk and in the journal", c.len()),
            ReplayError::Invalid(m) => write!(f, "the journal does not replay: {m}"),
        }
    }
}

impl std::error::Error for ReplayError {}

/// Summary of a leftover journal for the recovery dialog (doc 05 §8.3).
#[derive(Clone, PartialEq, Debug)]
pub struct RecoveryInfo {
    /// The session that crashed.
    pub session: String,
    /// The base generation of its segment.
    pub base: u64,
    /// Whether the project on disk is still that base (same manifest hash): replay without collisions.
    pub base_matches: bool,
    /// The number of unsaved transactions.
    pub transactions: usize,
    /// Epoch milliseconds of the last change.
    pub last_change_ms: u64,
}

/// The unsaved transactions of the newest leftover segment against the disk state.
pub(crate) fn summarise(
    segments: &[Segment],
    disk_gen: u64,
    disk_manifest: Option<&[u8; 32]>,
) -> Option<RecoveryInfo> {
    let seg = segments.first()?;
    let txs = seg.unsaved(disk_gen);
    if txs.is_empty() {
        return None;
    }
    let disk_text = disk_manifest.map(hash_text);
    let saved_into_disk = seg
        .entries
        .iter()
        .any(|e| matches!(e, Entry::SaveBegin { generation, .. } if *generation == disk_gen));
    Some(RecoveryInfo {
        session: seg.session.clone(),
        base: seg.base,
        base_matches: saved_into_disk || (seg.base == disk_gen && seg.base_manifest == disk_text),
        transactions: txs.len(),
        last_change_ms: txs
            .iter()
            .filter_map(|e| match e {
                Entry::Tx { t, .. } => Some(*t),
                _ => None,
            })
            .max()
            .unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPEN: &str = r#"{"j":"open","v":1,"project":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","path":"p","base":3,"base_manifest":null,"app_version":"t","session":"s"}"#;
    const TX: &str = r#"{"j":"tx","seq":1,"t":5,"cmd":"c","origin":"gui","ops":[{"k":["page","8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a"],"set":{"title":"x"},"was":{"title":null}}]}"#;

    fn seg(text: &str) -> Segment {
        Segment::parse(PathBuf::from("s-3.jsonl"), text.as_bytes(), None).unwrap()
    }

    #[test]
    fn a_torn_last_line_ends_the_segment() {
        let full = format!("{OPEN}\n{TX}\n{TX}\n");
        assert_eq!(seg(&full).entries.len(), 2);
        // The last line lacks its LF: torn, even when its JSON happens to be complete.
        let torn = format!("{OPEN}\n{TX}\n{TX}");
        assert_eq!(seg(&torn).entries.len(), 1);
        let garbled = format!("{OPEN}\n{TX}\n{{\"j\":\"tx\",\"se\n{TX}\n");
        assert_eq!(
            seg(&garbled).entries.len(),
            1,
            "replay stops at the first torn line"
        );
        assert!(Segment::parse(PathBuf::from("x"), b"{\"j\":\"tx\"}\n", None).is_none());
    }

    #[test]
    fn unsaved_counts_from_the_last_save_of_the_disk_generation() {
        let text = format!(
            "{OPEN}\n{TX}\n{{\"j\":\"save-begin\",\"gen\":4,\"writes\":1,\"removes\":[\"devices/9f.jsonl\"]}}\n{TX}\n"
        );
        let s = seg(&text);
        assert_eq!(s.unsaved(3).len(), 2, "the save of 4 never reached the disk");
        assert_eq!(s.unsaved(4).len(), 1, "generation 4 holds the first transaction");
        assert_eq!(s.removes_of(4), vec!["devices/9f.jsonl".to_owned()]);
        assert_eq!(s.last_change(4), 5);
    }

    #[test]
    fn ops_round_trip_through_their_json() {
        let key = (
            Kind::Page,
            "8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a".parse().unwrap(),
        );
        let mut set = Map::new();
        set.insert("title".into(), Value::from("x"));
        let mut was = Map::new();
        was.insert("title".into(), Value::Null);
        let op = JournalOp::Set { key, set, was };
        assert_eq!(JournalOp::from_value(&op.to_value()), Some(op));
    }
}
