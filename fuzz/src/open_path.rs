// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The open-path target (FT-297, doc 18 §3.12 "the migration chain"): a project written in an old shape,
//! opened by `ew-store`.
//!
//! The format's upgrade chain on `serde_json::Value` (WP-S1.17) is not built; what exists is the open path
//! that takes an older project of the same format (the typed revisions, ADR-0011 amendment 4: a reserved kind
//! typed in place, members in byte-wise order, an unknown `$k` or `$v`, a record in the wrong file) and the
//! rewrite of such files at the next save. This target feeds that path: the golden project, its records
//! changed by a list of [`Op`]s (the shapes an older build wrote or a hand edit leaves: members in byte-wise
//! order, a removed or foreign member, another `$k` or `$v`, duplicated, swapped, cut, moved and deleted lines,
//! CRLF, a BOM, a missing file), the manifest recomputed so that the change reaches the reader (or left stale),
//! then [`Store::open`]. Properties:
//!
//! - the open never panics (a refusal, [`ew_store::OpenError`], is fine);
//! - the model it returns passes `Model::integrity_check`;
//! - when the session can edit: a save without an edit succeeds, a reader opens the result with an equal model
//!   (`Model::content_eq`) and with nothing left to rewrite (no `NonCanonical`, `LineEndings`, `WrongFile` or
//!   `ForeignEdit` finding), and a second save writes nothing.
//!
//! Input: a mode byte (bit 0: leave the manifest stale; bit 1: open read-only), then operations of four bytes
//! `[file][line][op][argument]`, at most [`MAX_OPS`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use ew_format::{FileHash, FilePath, FindingKind, Limits, Manifest, sha256};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{
    Access, Changes, Identity, ManualClock, OpenMode, Opened, ProcessProbe, ProcessState, Store,
    StoreOptions, Vfs, join,
};
use serde_json::Value;

use crate::golden;

/// Mode bit 0: the manifest keeps the golden project's hashes.
pub const MODE_STALE_MANIFEST: u8 = 1;
/// Mode bit 1: open read-only (no lock, no save).
pub const MODE_READ_ONLY: u8 = 2;
/// The most operations of one input.
pub const MAX_OPS: usize = 16;

/// The number of operations ([`Op::of`] takes the byte modulo this).
pub const OP_KINDS: u8 = 15;

/// The `$k` values an operation sets: known kinds, a reserved one, an unknown one, an empty one.
const KINDS: [&str; 8] = [
    "revision",
    "page",
    "device",
    "project",
    "function",
    "from_the_future",
    "",
    "$k",
];

/// One change to the golden files.
#[derive(Clone, Copy, Debug)]
pub struct Op {
    /// Selects a file of [`golden::paths`] (modulo their number).
    pub file: u8,
    /// Selects a line of the file (modulo their number).
    pub line: u8,
    /// The kind of change (modulo [`OP_KINDS`]).
    pub op: u8,
    /// A parameter.
    pub arg: u8,
}

/// A decoded input.
#[derive(Debug, Default)]
pub struct Case {
    /// The mode byte.
    pub mode: u8,
    /// The operations, in order.
    pub ops: Vec<Op>,
}

/// Decodes an input (see the module).
pub fn decode(input: &[u8]) -> Case {
    let Some((&mode, rest)) = input.split_first() else {
        return Case::default();
    };
    let ops = rest
        .chunks_exact(4)
        .take(MAX_OPS)
        .map(|c| Op {
            file: c[0],
            line: c[1],
            op: c[2],
            arg: c[3],
        })
        .collect();
    Case { mode, ops }
}

/// The inverse of [`decode`] for the seed corpus.
pub fn encode(mode: u8, ops: &[Op]) -> Vec<u8> {
    let mut out = vec![mode];
    for op in ops {
        out.extend_from_slice(&[op.file, op.line, op.op, op.arg]);
    }
    out
}

/// The text of a file as lines without their LF (a final empty piece is dropped).
fn lines_of(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = bytes.split(|b| *b == b'\n').map(<[u8]>::to_vec).collect();
    if lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
    }
    lines
}

fn join_lines(lines: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for line in lines {
        out.extend_from_slice(line);
        out.push(b'\n');
    }
    out
}

/// A JSON value for a `set member` operation.
fn value_of(pick: u8) -> Value {
    match pick % 10 {
        0 => Value::Null,
        1 => Value::Bool(true),
        2 => Value::from(0),
        3 => Value::from(-1),
        4 => Value::from(u64::MAX),
        5 => Value::from(""),
        6 => Value::from("x"),
        7 => Value::Array(Vec::new()),
        8 => Value::Object(serde_json::Map::new()),
        _ => Value::from(1.5),
    }
}

/// Changes the JSON object of `line` with `edit`; a line that is not one is left alone.
fn edit_object(line: &mut Vec<u8>, edit: impl FnOnce(&mut serde_json::Map<String, Value>)) {
    if let Ok(Value::Object(mut map)) = serde_json::from_slice::<Value>(line) {
        edit(&mut map);
        if let Ok(bytes) = serde_json::to_vec(&Value::Object(map)) {
            *line = bytes;
        }
    }
}

/// Applies `op` to the files (keyed by path); `deleted` collects the files that are removed afterwards.
fn apply(files: &mut BTreeMap<String, Vec<u8>>, deleted: &mut BTreeSet<String>, op: Op) {
    let paths = golden::paths();
    let path = &paths[usize::from(op.file) % paths.len()];
    let Some(bytes) = files.get(path).cloned() else {
        return;
    };
    let mut lines = lines_of(&bytes);
    let at = |n: usize| if n == 0 { 0 } else { usize::from(op.line) % n };
    match op.op % OP_KINDS {
        // The shape of an older build: members in byte-wise key order.
        0 => {
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            edit_object(&mut lines[i], |_| {});
        }
        1 => {
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            edit_object(&mut lines[i], |m| {
                let keys: Vec<String> = m.keys().cloned().collect();
                if let Some(key) = keys.get(usize::from(op.arg) % keys.len().max(1)) {
                    m.remove(key);
                }
            });
        }
        2 => {
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            edit_object(&mut lines[i], |m| {
                m.insert(
                    "$k".to_owned(),
                    Value::from(KINDS[usize::from(op.arg) % KINDS.len()]),
                );
            });
        }
        3 => {
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            edit_object(&mut lines[i], |m| {
                m.insert("$v".to_owned(), Value::from(op.arg));
            });
        }
        4 => {
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            edit_object(&mut lines[i], |m| {
                let keys: Vec<String> = m.keys().cloned().collect();
                let key = keys
                    .get(usize::from(op.arg & 0x0f) % keys.len().max(1))
                    .cloned()
                    .unwrap_or_else(|| "extra".to_owned());
                m.insert(key, value_of(op.arg >> 4));
            });
        }
        5 => {
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            let copy = lines[i].clone();
            lines.insert(i, copy);
        }
        6 => {
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            lines.remove(i);
        }
        7 => {
            if lines.len() < 2 {
                return;
            }
            let i = at(lines.len() - 1);
            lines.swap(i, i + 1);
        }
        8 => {
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            let keep = lines[i].len() * usize::from(op.arg) / 256;
            lines[i].truncate(keep);
        }
        9 => {
            let mut out = Vec::new();
            for line in &lines {
                out.extend_from_slice(line);
                out.extend_from_slice(b"\r\n");
            }
            files.insert(path.clone(), out);
            return;
        }
        10 => {
            let mut out = join_lines(&lines);
            out.pop();
            files.insert(path.clone(), out);
            return;
        }
        11 => {
            let mut out = b"\xEF\xBB\xBF".to_vec();
            out.extend_from_slice(&join_lines(&lines));
            files.insert(path.clone(), out);
            return;
        }
        12 => {
            // Move a line to another file: a record in the wrong file.
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            let line = lines.remove(i);
            let target = &paths[usize::from(op.arg) % paths.len()];
            if target.ends_with(".jsonl") && target != path {
                let mut other = lines_of(files.get(target).map_or(&[][..], Vec::as_slice));
                other.push(line);
                files.insert(target.clone(), join_lines(&other));
            } else {
                lines.insert(i, line);
            }
        }
        13 => {
            deleted.insert(path.clone());
            return;
        }
        _ => {
            if lines.is_empty() {
                return;
            }
            let i = at(lines.len());
            lines.insert(i, Vec::new());
        }
    }
    files.insert(path.clone(), join_lines(&lines));
}

/// The project directory of a case, in the in-memory file system, and the root path.
fn project(case: &Case) -> (Arc<MemVfs>, PathBuf) {
    let mut files: BTreeMap<String, Vec<u8>> = golden::files().clone();
    let mut deleted = BTreeSet::new();
    for op in &case.ops {
        apply(&mut files, &mut deleted, *op);
    }
    let mut manifest = Manifest::parse(&golden::files()["manifest.json"], &Limits::default())
        .expect("the golden manifest parses");
    if case.mode & MODE_STALE_MANIFEST == 0 {
        // The files as written by the older build: its manifest lists them with their own hashes, so that the
        // reader meets the records rather than a foreign edit. A deleted file keeps its listing.
        for (rel, bytes) in &files {
            if let Ok(path) = FilePath::new(rel.as_str()) {
                if manifest.files.contains_key(&path) {
                    manifest.files.insert(path, FileHash::Sha256(sha256(bytes)));
                }
            }
        }
    }
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    let root = PathBuf::from("/p");
    for (rel, bytes) in &files {
        if !deleted.contains(rel) && rel != "manifest.json" {
            vfs.put(&join(&root, rel), bytes);
        }
    }
    vfs.put(&join(&root, "manifest.json"), manifest.to_text().as_bytes());
    vfs.put_dir(std::path::Path::new("/state"));
    (vfs, root)
}

/// A probe that knows exactly one live process.
#[derive(Debug)]
struct OneAlive(u32);

impl ProcessProbe for OneAlive {
    fn probe(&self, pid: u32) -> ProcessState {
        if pid == self.0 {
            ProcessState::Alive { started: None }
        } else {
            ProcessState::Dead
        }
    }
}

/// Store options on `vfs` for process `pid`.
fn options(vfs: &Arc<MemVfs>, pid: u32, mode: OpenMode) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt fuzz");
    let shared: Arc<dyn Vfs> = Arc::clone(vfs) as Arc<dyn Vfs>;
    o.vfs = shared;
    o.clock = Arc::new(ManualClock::fixed());
    o.probe = Arc::new(OneAlive(pid));
    o.identity = Identity {
        session: ew_store::lock::new_session_id(),
        user: "<os user>".into(),
        host: "<host>".into(),
        pid,
        pid_started: UNIX_EPOCH + Duration::from_secs(1_792_222_918),
        app_version: "ElektraWatt fuzz".into(),
    };
    o.mode = mode;
    o.state_dir = Some(PathBuf::from("/state"));
    o
}

/// The findings that a saved project must not leave: what the next save is documented to rewrite.
fn pending_rewrite(kind: FindingKind) -> bool {
    matches!(
        kind,
        FindingKind::NonCanonical
            | FindingKind::LineEndings
            | FindingKind::WrongFile
            | FindingKind::ForeignEdit
    )
}

/// How far an input got (for the replay test; the fuzz target ignores it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The store refused to open the project.
    Refused,
    /// The session was read-only (a read-only open, or an integrity finding forced it): the model was checked.
    ReadOnly,
    /// The session could edit: saved twice, reopened and compared; the first save rewrote this many files.
    Reopened {
        /// The files the first save wrote.
        rewritten: usize,
    },
}

/// Runs the properties over one input; panics (with the failure) when one does not hold.
pub fn check(input: &[u8]) -> Outcome {
    let case = decode(input);
    let (vfs, root) = project(&case);
    let mode = if case.mode & MODE_READ_ONLY != 0 {
        OpenMode::ReadOnly
    } else {
        OpenMode::Edit
    };
    let Ok(Opened {
        mut store,
        model,
        report,
    }) = Store::open(&root, options(&vfs, 1, mode))
    else {
        return Outcome::Refused;
    };
    if let Err(e) = model.integrity_check() {
        panic!("the opened model fails its integrity check: {e:?}");
    }
    if report.access != Access::Edit {
        return Outcome::ReadOnly;
    }

    let saved = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .unwrap_or_else(|e| panic!("a save without an edit fails: {e}"));
    let again = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .unwrap_or_else(|e| panic!("a second save fails: {e}"));
    assert!(
        again.written.is_empty() && again.removed.is_empty() && !again.manifest_written,
        "a second save without an edit wrote {:?}, removed {:?}, manifest {} (the first wrote {:?})",
        again.written,
        again.removed,
        again.manifest_written,
        saved.written
    );
    store.close();

    let reopened = Store::open(&root, options(&vfs, 2, OpenMode::Edit))
        .unwrap_or_else(|e| panic!("a project saved by the store does not open: {e}"));
    assert!(
        reopened.model.content_eq(&model),
        "the model changed over save and reopen (findings of the first open: {:?})",
        report.findings
    );
    let pending: Vec<_> = reopened
        .report
        .findings
        .iter()
        .filter(|f| pending_rewrite(f.kind))
        .collect();
    assert!(
        pending.is_empty(),
        "a saved project still has files to rewrite: {pending:?} (findings of the first open: {:?})",
        report.findings
    );
    Outcome::Reopened {
        rewritten: saved.written.len(),
    }
}
