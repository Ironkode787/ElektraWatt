// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers of the `ew-store` integration tests: the golden project in a [`MemVfs`], store options
//! with a fixed identity, reopening after a crash, and a deterministic edit batch.

#![allow(dead_code)] // each test file uses a different subset

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use ew_model::{AnyKey, AnyRow, Model, ModelError, Op, Rev};
use ew_store::ew_format::ew_schema::{Device, Kind, Page, Record};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{
    Access, Identity, ManualClock, OpenError, Opened, ProcessProbe, ProcessState, ReadOnlyReason, Store,
    StoreOptions, Vfs,
};

/// A probe that knows exactly one live process.
#[derive(Debug)]
pub struct TestProbe {
    /// The only pid reported alive.
    pub alive: u32,
}

impl ProcessProbe for TestProbe {
    fn probe(&self, pid: u32) -> ProcessState {
        if pid == self.alive {
            ProcessState::Alive { started: None }
        } else {
            ProcessState::Dead
        }
    }
}

/// How often [`try_reopen`] met `OpenError::SaveInProgress` behind a stale lock and took over regardless.
pub static SAVE_IN_PROGRESS: AtomicUsize = AtomicUsize::new(0);

/// The project root in the in-memory file system.
pub fn root() -> PathBuf {
    PathBuf::from("/p")
}

fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
        let path = e.expect("a directory entry").path();
        if path.is_dir() {
            walk(&path, base, out);
        } else {
            let rel = path
                .strip_prefix(base)
                .expect("below the base")
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, std::fs::read(&path).expect("a readable file"));
        }
    }
}

/// Every file of the golden project `golden/J-E-2416.ew/`, by relative path with `/`.
pub fn fixture() -> BTreeMap<String, Vec<u8>> {
    static FIXTURE: std::sync::OnceLock<BTreeMap<String, Vec<u8>>> = std::sync::OnceLock::new();
    FIXTURE
        .get_or_init(|| {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
            let mut out = BTreeMap::new();
            walk(&dir, &dir, &mut out);
            assert!(!out.is_empty(), "the golden fixture {} is missing", dir.display());
            out
        })
        .clone()
}

/// A [`MemVfs`] holding the golden project under `/p`, durable, and an empty state directory `/state`.
pub fn mem_project(dir_sync: DirSync) -> (Arc<MemVfs>, PathBuf) {
    let vfs = Arc::new(MemVfs::new(dir_sync));
    let root = root();
    for (rel, bytes) in fixture() {
        vfs.put(&ew_store::join(&root, &rel), &bytes);
    }
    vfs.put_dir(Path::new("/state"));
    (vfs, root)
}

/// Store options on `vfs` for a process with id `pid`; every other pid is dead.
pub fn options(vfs: &Arc<MemVfs>, pid: u32) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    let shared: Arc<dyn Vfs> = Arc::clone(vfs) as Arc<dyn Vfs>;
    o.vfs = shared;
    o.clock = Arc::new(ManualClock::fixed());
    o.probe = Arc::new(TestProbe { alive: pid });
    o.identity = Identity {
        session: ew_store::lock::new_session_id(),
        user: "<os user>".into(),
        host: "<host>".into(),
        pid,
        pid_started: UNIX_EPOCH + Duration::from_secs(1_792_222_918),
        app_version: "ElektraWatt test".into(),
    };
    o.state_dir = Some(PathBuf::from("/state"));
    o
}

/// Opens the project as process `pid`; a stale lock (its owner is dead) is taken over.
///
/// # Errors
///
/// A description of the failure when the project does not open at all.
pub fn try_reopen(vfs: &Arc<MemVfs>, pid: u32) -> Result<Opened, String> {
    let root = root();
    let opened = match Store::open(&root, options(vfs, pid)) {
        Ok(o) => o,
        Err(OpenError::SaveInProgress) => {
            // A crashed save leaves an intent behind a stale lock, and the plain open gives up instead of
            // reporting the stale lock; go on with the takeover the caller would be offered (counted).
            SAVE_IN_PROGRESS.fetch_add(1, Ordering::Relaxed);
            return Store::take_over(&root, options(vfs, pid))
                .map_err(|e| format!("Store::take_over failed: {e} ({e:?})"));
        }
        Err(e) => return Err(format!("Store::open failed: {e} ({e:?})")),
    };
    if let Access::ReadOnly(ReadOnlyReason::LockedBy(info)) = &opened.report.access {
        if info.stale {
            drop(opened);
            return Store::take_over(&root, options(vfs, pid))
                .map_err(|e| format!("Store::take_over failed: {e} ({e:?})"));
        }
    }
    Ok(opened)
}

/// [`try_reopen`], panicking when the project does not open.
///
/// # Panics
///
/// When the project does not open at all.
pub fn reopen(vfs: &Arc<MemVfs>, pid: u32) -> Opened {
    try_reopen(vfs, pid).unwrap_or_else(|e| panic!("reopen as pid {pid}: {e}"))
}

fn page_value(page: &Page) -> serde_json::Value {
    let mut v = serde_json::to_value(page).expect("a page serialises");
    v["$k"] = "page".into();
    v
}

/// `device` with its `function_text` set to `text`.
pub fn device_with_text(device: &Device, text: &str) -> Device {
    let mut v = serde_json::to_value(device).expect("a device serialises");
    v["$k"] = "device".into();
    v["function_text"] = text.into();
    match Record::from_value(v) {
        Ok(Record::Device(d)) => d,
        other => panic!("the edited device does not read back: {other:?}"),
    }
}

/// A copy of `page` with the ID `00000000-0000-7000-8000-<n as 12 hex digits>` and folio number `folio`.
pub fn page_copy(page: &Page, n: u64, folio: u32) -> Page {
    let mut v = page_value(page);
    v["id"] = format!("00000000-0000-7000-8000-{n:012x}").into();
    v["folio"]["number"] = folio.into();
    match Record::from_value(v) {
        Ok(Record::Page(p)) => p,
        other => panic!("the copied page does not read back: {other:?}"),
    }
}

/// The smallest folio number above every folio number of the model, at least `at_least`.
pub fn unused_folio(model: &Model, at_least: u32) -> u32 {
    model
        .table::<Page>()
        .iter()
        .map(|(_, p)| p.folio.number + 1)
        .max()
        .unwrap_or(0)
        .max(at_least)
}

/// Applies `ops` as the next revision and adds their keys to `keys`.
pub fn apply(model: &Model, ops: &[Op], keys: &mut BTreeSet<AnyKey>) -> Result<Model, ModelError> {
    let next = model.apply(ops, Rev(model.rev().0 + 1))?;
    keys.extend(ops.iter().map(Op::key));
    Ok(next)
}

/// An edit batch that makes a save write, create and remove files: the `nth` device (ID order) gets the
/// function text `text`.
pub fn edit_device(model: &Model, nth: usize, text: &str) -> (Model, BTreeSet<AnyKey>) {
    let mut keys = BTreeSet::new();
    let rows: Vec<_> = model
        .table::<Device>()
        .iter()
        .map(|(_, d)| Arc::clone(d))
        .collect();
    for row in rows.iter().skip(nth) {
        let after = device_with_text(row, text);
        let ops = [Op::update(AnyRow::Device(Arc::clone(row)), AnyRow::of(after))];
        if let Ok(m) = apply(model, &ops, &mut keys) {
            return (m, keys);
        }
        keys.clear();
    }
    panic!("no device accepts the edit");
}

/// The deterministic edit of the fault-injection tests: (a) the first device gets the function text
/// `"edited"`, (b) the first page is copied under a new ID and an unused folio number, (c) the page with the
/// largest ID is removed with everything that depends on it. A save of it writes, creates and removes files.
pub fn edit(model: &Model) -> (Model, BTreeSet<AnyKey>) {
    let mut keys = BTreeSet::new();
    let (mut m, k) = edit_device(model, 0, "edited");
    keys.extend(k);
    // (b) A copy of the first page that is not the page removed in (c).
    let pages: Vec<_> = m.table::<Page>().iter().map(|(_, p)| Arc::clone(p)).collect();
    let folio = unused_folio(&m, 777);
    let copy = page_copy(&pages[0], 0xff, folio);
    m = apply(&m, &[Op::insert(AnyRow::of(copy))], &mut keys).expect("the page copy inserts");
    // (c) Remove the page with the largest ID of the original pages (not the copy).
    let mut removed = false;
    for page in pages.iter().rev() {
        let key: AnyKey = (Kind::Page, page.id.cast());
        let Ok(ops) = m.delete_plan(&key) else { continue };
        let mut try_keys = keys.clone();
        if let Ok(next) = apply(&m, &ops, &mut try_keys) {
            m = next;
            keys = try_keys;
            removed = true;
            break;
        }
    }
    assert!(removed, "no page can be removed");
    (m, keys)
}

/// The project's files without the lock and the stale-lock records.
pub fn project_files(vfs: &MemVfs, root: &Path) -> BTreeMap<String, Vec<u8>> {
    vfs.files(root)
        .into_iter()
        .filter(|(name, _)| name != ".~ew.lock" && !name.starts_with(".~ew.lock.stale-"))
        .collect()
}

/// The names of `files` that are temps of a save (`*.tmp-<generation>`).
pub fn temps(files: &BTreeMap<String, Vec<u8>>) -> Vec<&str> {
    files
        .keys()
        .filter(|n| n.contains(".tmp-"))
        .map(String::as_str)
        .collect()
}

/// Runs `f` over `items` on several threads (the sweeps are independent cases) and returns the results in
/// the order of the items.
pub fn par_map<T: Send, R: Send>(items: Vec<T>, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let n = items.len();
    let queue = std::sync::Mutex::new(items.into_iter().enumerate().collect::<Vec<_>>());
    let out = std::sync::Mutex::new(Vec::with_capacity(n));
    let threads = std::thread::available_parallelism()
        .map_or(2, usize::from)
        .clamp(1, 8);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let next = queue
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .pop();
                    let Some((i, item)) = next else { break };
                    let r = f(item);
                    out.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push((i, r));
                }
            });
        }
    });
    let mut out = out
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, r)| r).collect()
}
