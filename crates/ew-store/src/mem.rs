// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An in-memory [`Vfs`] with fault injection (doc 05 §5.6).
//!
//! [`MemVfs`] numbers every [`Vfs`] call and can kill the "process" at a call ([`Fault::Kill`]), cut the
//! power at a call ([`Fault::PowerLoss`]) or fail a single call ([`Fault::Error`]). It models the
//! durability contract documented on [`Vfs`]:
//!
//! - a file's data are durable only after [`Vfs::sync`] of the file;
//! - a name (created, renamed, removed, a directory made or removed) is durable only after
//!   [`Vfs::sync_dir`] of its directory ([`DirSync::Explicit`], ext4 and APFS), or at once
//!   ([`DirSync::Implicit`], NTFS's metadata log);
//! - on power loss the file system returns to its durable state plus a prefix of the still-pending name
//!   changes (an ordered metadata journal, ext4 `data=ordered`): the namespace changes of the prefix are
//!   applied in order, files whose data were never synced come back empty.
//!
//! A process kill, by contrast, loses nothing: the OS page cache survives it.

// FT-271: `State::holds` is looked up and counted down by path; it is never iterated.
#[allow(clippy::disallowed_types)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::vfs::{EntryKind, Stat, Vfs};

/// A fault of the harness, armed with [`MemVfs::set_fault`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fault {
    /// No fault.
    None,
    /// Process kill: call number `at` (1-based count of [`Vfs`] calls since the last
    /// [`MemVfs::reset_calls`]) and every later call fail with [`io::ErrorKind::Other`] ("killed") and
    /// change nothing; the state stays as it is (the OS page cache survives a process kill).
    Kill {
        /// The first failing call.
        at: u64,
    },
    /// Power loss: before call `at` runs, the state reverts to its durable state, with the first `keep`
    /// pending namespace operations applied (ordered metadata journal, ext4 `data=ordered` style); that
    /// call and every later one fail.
    PowerLoss {
        /// The first failing call.
        at: u64,
        /// How many pending namespace operations reached the disk.
        keep: usize,
    },
    /// One failing call: call `at` returns this error kind and changes nothing; later calls work.
    Error {
        /// The failing call.
        at: u64,
        /// The error kind it returns.
        kind: io::ErrorKind,
    },
}

/// When a name change becomes durable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirSync {
    /// Names are durable only after [`Vfs::sync_dir`] of their directory (ext4, APFS).
    Explicit,
    /// Every namespace change is durable at once; [`Vfs::sync_dir`] is a no-op (NTFS's metadata log).
    Implicit,
}

/// What a name refers to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Node {
    /// A regular file: an index into the inode table.
    File(usize),
    /// A symbolic link (never followed).
    Link,
}

/// A file: its current data, the data that survive a power loss, and its modification time.
#[derive(Clone, Debug)]
struct Inode {
    data: Vec<u8>,
    durable_data: Option<Vec<u8>>,
    modified: SystemTime,
}

/// A namespace: names of files and links, and the set of directories.
#[derive(Clone, Debug, Default)]
struct Ns {
    names: BTreeMap<String, Node>,
    dirs: BTreeSet<String>,
}

/// One namespace change, with the inode it moves where that matters.
#[derive(Clone, Debug)]
enum NsOp {
    Create { path: String, node: Node },
    Rename { from: String, to: String, node: Node },
    Remove { path: String },
    MkDir { path: String },
    RmDir { path: String },
}

impl NsOp {
    /// The names this change affects; it is durable when the directory of any of them is synced.
    fn names(&self) -> Vec<&str> {
        match self {
            NsOp::Create { path, .. }
            | NsOp::Remove { path }
            | NsOp::MkDir { path }
            | NsOp::RmDir { path } => vec![path],
            NsOp::Rename { from, to, .. } => vec![from, to],
        }
    }
}

impl Ns {
    fn apply(&mut self, op: &NsOp) {
        match op {
            NsOp::Create { path, node } => {
                self.names.insert(path.clone(), *node);
            }
            NsOp::Rename { from, to, node } => {
                self.names.remove(from);
                self.names.insert(to.clone(), *node);
            }
            NsOp::Remove { path } => {
                self.names.remove(path);
            }
            NsOp::MkDir { path } => {
                self.dirs.insert(path.clone());
            }
            NsOp::RmDir { path } => {
                self.dirs.remove(path);
            }
        }
    }

    fn is_dir(&self, path: &str) -> bool {
        is_root(path) || self.dirs.contains(path)
    }

    /// Drops the names whose directory did not reach the disk (their inodes are orphaned).
    fn prune_orphans(&mut self) {
        // Parents sort before their children, so one ordered pass is enough for directories.
        let dirs: Vec<String> = self.dirs.iter().cloned().collect();
        for d in dirs {
            if parent_of(&d).is_some_and(|p| !self.is_dir(&p)) {
                self.dirs.remove(&d);
            }
        }
        let names: Vec<String> = self.names.keys().cloned().collect();
        for n in names {
            if parent_of(&n).is_some_and(|p| !self.is_dir(&p)) {
                self.names.remove(&n);
            }
        }
    }
}

struct State {
    inodes: Vec<Inode>,
    cur: Ns,
    dur: Ns,
    pending: Vec<NsOp>,
    dir_sync: DirSync,
    calls: u64,
    fault: Fault,
    dead: bool,
    // FT-271: lookup by path only (`hold`, `rename`), never iterated.
    #[allow(clippy::disallowed_types)]
    holds: HashMap<String, u32>,
    read_only: bool,
    noreplace_unsupported: bool,
    now: SystemTime,
    trace: Option<Vec<String>>,
}

/// An in-memory [`Vfs`] with fault injection and a model of what survives a power loss.
pub struct MemVfs {
    state: Mutex<State>,
}

impl fmt::Debug for MemVfs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MemVfs { .. }")
    }
}

impl Default for MemVfs {
    fn default() -> MemVfs {
        MemVfs::new(DirSync::Explicit)
    }
}

fn is_root(path: &str) -> bool {
    path.is_empty() || path == "/"
}

/// The parent directory of a normalised path; `None` for the root.
fn parent_of(path: &str) -> Option<String> {
    if is_root(path) {
        return None;
    }
    Some(match path.rfind('/') {
        None => String::new(),
        Some(0) => "/".to_owned(),
        Some(i) => path[..i].to_owned(),
    })
}

/// The prefix that every direct and indirect child of directory `dir` starts with.
fn child_prefix(dir: &str) -> String {
    if dir.is_empty() {
        String::new()
    } else if dir.ends_with('/') {
        dir.to_owned()
    } else {
        format!("{dir}/")
    }
}

/// The path as text: the prefix, `/` for the root, the normal components joined by `/`; `..` is refused.
fn norm(path: &Path) -> io::Result<String> {
    let mut s = String::new();
    for c in path.components() {
        match c {
            Component::Prefix(p) => s.push_str(&p.as_os_str().to_string_lossy()),
            Component::RootDir => s.push('/'),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "`..` in a path"));
            }
            Component::Normal(n) => {
                if !(s.is_empty() || s.ends_with('/')) {
                    s.push('/');
                }
                s.push_str(&n.to_string_lossy());
            }
        }
    }
    Ok(s)
}

fn not_found(path: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{path}: not found"))
}

fn killed() -> io::Error {
    io::Error::other("killed")
}

impl State {
    fn record(&mut self, op: NsOp) {
        self.cur.apply(&op);
        match self.dir_sync {
            DirSync::Implicit => self.dur.apply(&op),
            DirSync::Explicit => self.pending.push(op),
        }
    }

    fn new_inode(&mut self, data: Vec<u8>) -> usize {
        self.inodes.push(Inode {
            data,
            durable_data: None,
            modified: self.now,
        });
        self.inodes.len() - 1
    }

    /// Creates `dir` and its missing parents as pending changes.
    fn make_dirs(&mut self, dir: &str) -> io::Result<()> {
        let mut chain = Vec::new();
        let mut cur = Some(dir.to_owned());
        while let Some(d) = cur {
            if is_root(&d) {
                break;
            }
            cur = parent_of(&d);
            chain.push(d);
        }
        chain.reverse();
        for d in chain {
            if self.cur.dirs.contains(&d) {
                continue;
            }
            if self.cur.names.contains_key(&d) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{d}: not a directory"),
                ));
            }
            self.record(NsOp::MkDir { path: d });
        }
        Ok(())
    }

    /// Creates `dir` and its parents durably at once (test setup).
    fn make_dirs_durable(&mut self, dir: &str) {
        let mut cur = Some(dir.to_owned());
        while let Some(d) = cur {
            if is_root(&d) {
                break;
            }
            self.cur.dirs.insert(d.clone());
            self.dur.dirs.insert(d.clone());
            cur = parent_of(&d);
        }
    }

    fn power_loss(&mut self, keep: usize) {
        let pending = std::mem::take(&mut self.pending);
        for op in pending.iter().take(keep) {
            self.dur.apply(op);
        }
        self.dur.prune_orphans();
        for inode in &mut self.inodes {
            inode.data = inode.durable_data.clone().unwrap_or_default();
        }
        self.cur = self.dur.clone();
    }

    fn file_index(&self, path: &str) -> io::Result<usize> {
        match self.cur.names.get(path) {
            Some(Node::File(i)) => Ok(*i),
            Some(Node::Link) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{path}: a link"),
            )),
            None if self.cur.dirs.contains(path) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{path}: a directory"),
            )),
            None => Err(not_found(path)),
        }
    }

    fn require_parent(&self, path: &str) -> io::Result<()> {
        match parent_of(path) {
            Some(p) if !self.cur.is_dir(&p) => Err(not_found(&p)),
            _ => Ok(()),
        }
    }

    fn exists(&self, path: &str) -> bool {
        self.cur.names.contains_key(path) || self.cur.dirs.contains(path)
    }
}

impl MemVfs {
    /// An empty file system with the given directory-durability model.
    pub fn new(dir_sync: DirSync) -> MemVfs {
        MemVfs {
            state: Mutex::new(State {
                inodes: Vec::new(),
                cur: Ns::default(),
                dur: Ns::default(),
                pending: Vec::new(),
                dir_sync,
                calls: 0,
                fault: Fault::None,
                dead: false,
                // FT-271: the empty `holds` map above; lookup only.
                #[allow(clippy::disallowed_types)]
                holds: HashMap::new(),
                read_only: false,
                noreplace_unsupported: false,
                now: UNIX_EPOCH + Duration::from_secs(1_792_222_930),
                trace: None,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Counts a call and applies the fault; `Ok` hands out the state.
    fn enter(&self, op: &str, paths: &[&Path], mutating: bool) -> io::Result<MutexGuard<'_, State>> {
        let mut s = self.lock();
        s.calls += 1;
        let n = s.calls;
        if let Some(t) = &mut s.trace {
            let shown: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
            t.push(format!("{n}: {op} {}", shown.join(" -> ")));
        }
        if s.dead {
            return Err(killed());
        }
        match s.fault {
            Fault::Kill { at } if n >= at => {
                s.dead = true;
                return Err(killed());
            }
            Fault::PowerLoss { at, keep } if n >= at => {
                s.power_loss(keep);
                s.dead = true;
                return Err(killed());
            }
            Fault::Error { at, kind } if n == at => {
                return Err(io::Error::new(kind, "injected error"));
            }
            _ => {}
        }
        if mutating && s.read_only {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only file system",
            ));
        }
        Ok(s)
    }

    /// Arms a fault (replacing the previous one).
    pub fn set_fault(&self, fault: Fault) {
        self.lock().fault = fault;
    }

    /// Starts (or stops, and clears) recording a line per [`Vfs`] call (`<number>: <operation> <paths>`).
    pub fn set_trace(&self, on: bool) {
        self.lock().trace = on.then(Vec::new);
    }

    /// The recorded call lines, empty when tracing is off.
    pub fn trace(&self) -> Vec<String> {
        self.lock().trace.clone().unwrap_or_default()
    }

    /// The number of [`Vfs`] calls since the last [`MemVfs::reset_calls`].
    pub fn calls(&self) -> u64 {
        self.lock().calls
    }

    /// Resets the call counter to 0.
    pub fn reset_calls(&self) {
        self.lock().calls = 0;
    }

    /// After a kill or a power loss: a new process starts, so the dead flag and the fault are cleared.
    pub fn restart(&self) {
        let mut s = self.lock();
        s.dead = false;
        s.fault = Fault::None;
    }

    /// Test setup: writes a file, creating its parents; data and name are durable at once. No call is
    /// counted and no fault applies.
    pub fn put(&self, path: &Path, data: &[u8]) {
        let p = norm(path).expect("a path without `..`");
        let mut s = self.lock();
        if let Some(parent) = parent_of(&p) {
            s.make_dirs_durable(&parent);
        }
        let i = s.new_inode(data.to_vec());
        s.inodes[i].durable_data = Some(data.to_vec());
        s.cur.names.insert(p.clone(), Node::File(i));
        s.dur.names.insert(p, Node::File(i));
    }

    /// Test setup: creates a directory and its parents, durable at once.
    pub fn put_dir(&self, path: &Path) {
        let p = norm(path).expect("a path without `..`");
        self.lock().make_dirs_durable(&p);
    }

    /// Test setup: an entry of kind [`EntryKind::Link`] (reading it fails with `InvalidInput`, `stat`
    /// reports a link), durable at once.
    pub fn put_link(&self, path: &Path) {
        let p = norm(path).expect("a path without `..`");
        let mut s = self.lock();
        if let Some(parent) = parent_of(&p) {
            s.make_dirs_durable(&parent);
        }
        s.cur.names.insert(p.clone(), Node::Link);
        s.dur.names.insert(p, Node::Link);
    }

    /// The current files below `root` (links and directories excluded), by `/`-separated path relative to
    /// `root`.
    pub fn files(&self, root: &Path) -> BTreeMap<String, Vec<u8>> {
        let r = norm(root).expect("a path without `..`");
        let prefix = child_prefix(&r);
        let s = self.lock();
        s.cur
            .names
            .iter()
            .filter_map(|(name, node)| {
                let rel = name.strip_prefix(&prefix)?;
                match node {
                    Node::File(i) if !rel.is_empty() => Some((rel.to_owned(), s.inodes[*i].data.clone())),
                    _ => None,
                }
            })
            .collect()
    }

    /// The next `times` [`Vfs::rename`] calls from or to `path` fail with
    /// [`io::ErrorKind::ResourceBusy`], as antivirus holding a file does on Windows.
    pub fn hold(&self, path: &Path, times: u32) {
        let p = norm(path).expect("a path without `..`");
        self.lock().holds.insert(p, times);
    }

    /// A read-only share: every mutating call fails with [`io::ErrorKind::PermissionDenied`].
    pub fn set_read_only(&self, yes: bool) {
        self.lock().read_only = yes;
    }

    /// Whether [`Vfs::rename_noreplace`] fails with [`io::ErrorKind::Unsupported`].
    pub fn set_noreplace_unsupported(&self, yes: bool) {
        self.lock().noreplace_unsupported = yes;
    }

    /// The modification time given to files created or appended to from now on (default 2026-10-17T07:42:10Z).
    pub fn set_now(&self, t: SystemTime) {
        self.lock().now = t;
    }

    fn rename_inner(&self, from: &Path, to: &Path, replace: bool) -> io::Result<()> {
        let mut s = self.enter(
            if replace { "rename" } else { "rename_noreplace" },
            &[from, to],
            true,
        )?;
        let (f, t) = (norm(from)?, norm(to)?);
        if !replace && s.noreplace_unsupported {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "no rename without replacement",
            ));
        }
        if replace {
            for p in [&f, &t] {
                if let Some(n) = s.holds.get_mut(p.as_str()) {
                    if *n > 0 {
                        *n -= 1;
                        return Err(io::Error::new(io::ErrorKind::ResourceBusy, format!("{p}: held")));
                    }
                }
            }
        }
        let Some(&node) = s.cur.names.get(&f) else {
            return Err(if s.cur.dirs.contains(&f) {
                io::Error::new(io::ErrorKind::InvalidInput, "directory renames are not supported")
            } else {
                not_found(&f)
            });
        };
        s.require_parent(&t)?;
        if s.cur.dirs.contains(&t) {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{t}: a directory"),
            ));
        }
        if s.cur.names.contains_key(&t) && (!replace || f == t) {
            return if f == t && replace {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{t}: exists"),
                ))
            };
        }
        s.record(NsOp::Rename { from: f, to: t, node });
        Ok(())
    }
}

impl Vfs for MemVfs {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        let s = self.enter("read", &[path], false)?;
        let p = norm(path)?;
        let i = s.file_index(&p)?;
        Ok(s.inodes[i].data.clone())
    }

    fn stat(&self, path: &Path) -> io::Result<Stat> {
        let s = self.enter("stat", &[path], false)?;
        let p = norm(path)?;
        match s.cur.names.get(&p) {
            Some(Node::File(i)) => Ok(Stat {
                kind: EntryKind::File,
                len: s.inodes[*i].data.len() as u64,
                modified: Some(s.inodes[*i].modified),
            }),
            Some(Node::Link) => Ok(Stat {
                kind: EntryKind::Link,
                len: 0,
                modified: None,
            }),
            None if s.cur.is_dir(&p) => Ok(Stat {
                kind: EntryKind::Dir,
                len: 0,
                modified: None,
            }),
            None => Err(not_found(&p)),
        }
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<(String, EntryKind)>> {
        let s = self.enter("list", &[dir], false)?;
        let d = norm(dir)?;
        if !s.cur.is_dir(&d) {
            return Err(if s.cur.names.contains_key(&d) {
                io::Error::new(io::ErrorKind::NotADirectory, format!("{d}: not a directory"))
            } else {
                not_found(&d)
            });
        }
        let prefix = child_prefix(&d);
        let direct = |name: &str| -> Option<String> {
            let rest = name.strip_prefix(&prefix)?;
            (!rest.is_empty() && !rest.contains('/')).then(|| rest.to_owned())
        };
        let mut out: BTreeMap<String, EntryKind> = BTreeMap::new();
        for (name, node) in &s.cur.names {
            if let Some(n) = direct(name) {
                out.insert(
                    n,
                    match node {
                        Node::File(_) => EntryKind::File,
                        Node::Link => EntryKind::Link,
                    },
                );
            }
        }
        for name in &s.cur.dirs {
            if let Some(n) = direct(name) {
                out.insert(n, EntryKind::Dir);
            }
        }
        Ok(out.into_iter().collect())
    }

    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        let mut s = self.enter("create_dir_all", &[dir], false)?;
        let d = norm(dir)?;
        if s.read_only && !s.cur.is_dir(&d) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only file system",
            ));
        }
        s.make_dirs(&d)
    }

    fn create_new(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let mut s = self.enter("create_new", &[path], true)?;
        let p = norm(path)?;
        s.require_parent(&p)?;
        if s.exists(&p) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{p}: exists"),
            ));
        }
        let i = s.new_inode(data.to_vec());
        s.record(NsOp::Create {
            path: p,
            node: Node::File(i),
        });
        Ok(())
    }

    fn append(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let mut s = self.enter("append", &[path], true)?;
        let p = norm(path)?;
        match s.cur.names.get(&p).copied() {
            Some(Node::File(i)) => {
                let now = s.now;
                let inode = &mut s.inodes[i];
                inode.data.extend_from_slice(data);
                inode.modified = now;
                Ok(())
            }
            Some(Node::Link) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{p}: a link"),
            )),
            None if s.cur.dirs.contains(&p) => Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{p}: a directory"),
            )),
            None => {
                s.require_parent(&p)?;
                let i = s.new_inode(data.to_vec());
                s.record(NsOp::Create {
                    path: p,
                    node: Node::File(i),
                });
                Ok(())
            }
        }
    }

    fn sync(&self, path: &Path) -> io::Result<()> {
        let mut s = self.enter("sync", &[path], true)?;
        let p = norm(path)?;
        let i = s.file_index(&p)?;
        s.inodes[i].durable_data = Some(s.inodes[i].data.clone());
        Ok(())
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        let mut s = self.enter("sync_dir", &[dir], false)?;
        let d = norm(dir)?;
        if !s.cur.is_dir(&d) {
            return Err(not_found(&d));
        }
        if s.dir_sync == DirSync::Implicit {
            return Ok(());
        }
        let pending = std::mem::take(&mut s.pending);
        let (now_durable, still): (Vec<NsOp>, Vec<NsOp>) = pending.into_iter().partition(|op| {
            op.names()
                .iter()
                .any(|n| parent_of(n).as_deref() == Some(d.as_str()))
        });
        for op in &now_durable {
            s.dur.apply(op);
        }
        s.pending = still;
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.rename_inner(from, to, true)
    }

    fn rename_noreplace(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.rename_inner(from, to, false)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        let mut s = self.enter("remove_file", &[path], true)?;
        let p = norm(path)?;
        if s.cur.names.contains_key(&p) {
            s.record(NsOp::Remove { path: p });
            Ok(())
        } else if s.cur.dirs.contains(&p) {
            Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{p}: a directory"),
            ))
        } else {
            Err(not_found(&p))
        }
    }

    fn remove_dir(&self, dir: &Path) -> io::Result<()> {
        let mut s = self.enter("remove_dir", &[dir], true)?;
        let d = norm(dir)?;
        if !s.cur.dirs.contains(&d) {
            return Err(if s.cur.names.contains_key(&d) {
                io::Error::new(io::ErrorKind::NotADirectory, format!("{d}: not a directory"))
            } else {
                not_found(&d)
            });
        }
        let prefix = child_prefix(&d);
        let has_child = s
            .cur
            .names
            .keys()
            .chain(s.cur.dirs.iter())
            .any(|n| n.starts_with(&prefix));
        if has_child {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                format!("{d}: not empty"),
            ));
        }
        s.record(NsOp::RmDir { path: d });
        Ok(())
    }

    fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()> {
        let mut s = self.enter("set_modified", &[path], true)?;
        let p = norm(path)?;
        let i = s.file_index(&p)?;
        s.inodes[i].modified = time;
        Ok(())
    }

    /// The normalised path (`.` dropped, `/` separators); no links are resolved (links are never followed
    /// here). `NotFound` when nothing is at the path.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        let s = self.enter("canonicalize", &[path], false)?;
        let p = norm(path)?;
        if s.cur.is_dir(&p) || s.cur.names.contains_key(&p) {
            Ok(PathBuf::from(p))
        } else {
            Err(not_found(&p))
        }
    }
}
