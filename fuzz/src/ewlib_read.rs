// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `.ewlib` reader target (WP-S6.04, ID-1354): a library directory of arbitrary files through `ew-library`'s own
//! reader, which parses `library.json`, the records shards and `library.lock` with its own code, beside the loader's
//! `LibDocument` reader.
//!
//! The input is a set of files with a mode byte ([`decode`]): `library.json`, `library.lock` and records files named by
//! [`path_of`]. Mode bit 0 replaces the manifest by one computed from the files (a fixed identity, generation 1, every
//! file's SHA-256), so the shards' lines get past the hash check. Each input is written to a fresh directory below the
//! system's temporary folder (the reader reads files only) and removed afterwards. Properties, whatever the bytes:
//!
//! - nothing panics: [`LibManifest::parse`], [`LockRecord::parse`], `Registry::open` over the directory as a company
//!   library with a fresh index, every document of every kind it then hands out, and the writer's lock taken over the
//!   input's `library.lock`;
//! - a manifest that parses writes back to a text that parses to the same manifest, and a lock record likewise;
//! - every document the registry hands out has a JSON form that reads again;
//! - a `library.lock` of the input is another session's: taking the lock is refused (held or stale) and leaves the
//!   file's bytes as they were.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ew_api::library::lock::{self, Identity, LockRecord};
use ew_api::library::{LibManifest, LibraryTiers, LockPolicy, MANIFEST, Registry};
use ew_schema::{Kind, LibDocument};

/// The selector of the manifest's section.
pub const MANIFEST_SELECTOR: u8 = 0xFF;
/// The selector of the lock's section.
pub const LOCK_SELECTOR: u8 = 0xFE;
/// Mode bit 0: the manifest computed from the files.
pub const MODE_COMPUTED_MANIFEST: u8 = 1;

/// The library's directory name, below the input's folder.
const LIBRARY: &str = "fuzz.ewlib";
/// The record kinds of a library, with their directories (`manufacturers` has one file and no shard).
const KINDS: [(Kind, &str); 7] = [
    (Kind::Fndef, "fndefs"),
    (Kind::Form, "forms"),
    (Kind::Macro, "macros"),
    (Kind::Part, "parts"),
    (Kind::PlcProfile, "plc-profiles"),
    (Kind::Symbol, "symbols"),
    (Kind::Manufacturer, "manufacturers"),
];

/// The records file of a section: the kind is its selector modulo 7, with the shard and generation of its first two
/// bytes (`parts/3f.2.jsonl`, `manufacturers.2.jsonl`).
pub fn path_of(selector: u8, shard: u8, generation: u8) -> String {
    match KINDS[usize::from(selector) % KINDS.len()] {
        (Kind::Manufacturer, dir) => format!("{dir}.{generation}.jsonl"),
        (_, dir) => format!("{dir}/{shard:02x}.{generation}.jsonl"),
    }
}

/// The selector, shard and generation of a records file's path, as [`path_of`] reads them; `None` for another path.
pub fn selector_of(path: &str) -> Option<(u8, u8, u8)> {
    let (stem, generation) = path.strip_suffix(".jsonl")?.rsplit_once('.')?;
    let generation: u8 = generation.parse().ok()?;
    let (dir, shard) = match stem.split_once('/') {
        Some((dir, shard)) => (dir, u8::from_str_radix(shard, 16).ok()?),
        None => (stem, 0),
    };
    let kind = KINDS.iter().position(|(_, d)| *d == dir)?;
    Some((u8::try_from(kind).ok()?, shard, generation))
}

/// One input, decoded.
#[derive(Debug, Default)]
pub struct Case {
    /// The mode byte.
    pub mode: u8,
    /// The records files by path.
    pub files: BTreeMap<String, Vec<u8>>,
    /// `library.json`'s bytes.
    pub manifest: Option<Vec<u8>>,
    /// `library.lock`'s bytes.
    pub lock: Option<Vec<u8>>,
}

/// Decodes `[mode] ([selector] [len: u16 le] [bytes])*` (a length past the end takes the rest); a records section's
/// bytes start with its shard and generation ([`path_of`]).
pub fn decode(input: &[u8]) -> Case {
    let mut case = Case::default();
    let Some((&mode, mut rest)) = input.split_first() else {
        return case;
    };
    case.mode = mode;
    while let [selector, lo, hi, tail @ ..] = rest {
        let len = usize::from(u16::from_le_bytes([*lo, *hi])).min(tail.len());
        let (bytes, after) = tail.split_at(len);
        match *selector {
            MANIFEST_SELECTOR => case.manifest = Some(bytes.to_vec()),
            LOCK_SELECTOR => case.lock = Some(bytes.to_vec()),
            s => {
                if let [shard, generation, content @ ..] = bytes {
                    case.files
                        .insert(path_of(s, *shard, *generation), content.to_vec());
                }
            }
        }
        rest = after;
    }
    case
}

/// Encodes sections as [`decode`] reads them: `(selector, bytes)`, each at most 65,535 bytes (a records section's
/// bytes with its shard and generation first).
pub fn encode(mode: u8, sections: &[(u8, &[u8])]) -> Vec<u8> {
    let mut out = vec![mode];
    for (selector, bytes) in sections {
        let len = u16::try_from(bytes.len()).expect("a section of at most 65535 bytes");
        out.push(*selector);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(bytes);
    }
    out
}

/// The manifest of [`MODE_COMPUTED_MANIFEST`]: the fixed identity, generation 1 and every file's SHA-256.
pub fn computed(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let id = ew_ids::Uuid::from_u128(0x0190_f5e0_6c1d_7a42_8b00_0000_0000_0001);
    let mut manifest = LibManifest::new(id, "fuzz", "Fuzz", "CC0-1.0");
    manifest.files = files
        .iter()
        .map(|(p, bytes)| (p.clone(), ew_format::sha256(bytes)))
        .collect();
    manifest.to_text().into_bytes()
}

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    /// `library.json` parsed.
    pub manifest: bool,
    /// The registry listed the library.
    pub listed: bool,
    /// Documents the registry handed out from the library.
    pub documents: usize,
}

/// A fresh folder for one input, below the system's temporary folder.
fn fresh() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("ew-fuzz-ewlib-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn write(dir: &Path, rel: &str, bytes: &[u8]) {
    let path = rel.split('/').fold(dir.to_path_buf(), |p, s| p.join(s));
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("a folder of the input");
    std::fs::write(&path, bytes).expect("a file of the input");
}

/// Runs one input.
///
/// # Panics
///
/// When a property of the module documentation fails.
pub fn check(input: &[u8]) -> Outcome {
    let case = decode(input);
    let manifest = if case.mode & MODE_COMPUTED_MANIFEST != 0 {
        Some(computed(&case.files))
    } else {
        case.manifest.clone()
    };
    let mut outcome = Outcome::default();
    if let Some(Ok(m)) = manifest.as_deref().map(LibManifest::parse) {
        outcome.manifest = true;
        assert_eq!(
            LibManifest::parse(m.to_text().as_bytes()).as_ref(),
            Ok(&m),
            "a parsed manifest does not survive its own text"
        );
    }
    if let Some(r) = case.lock.as_deref().and_then(LockRecord::parse) {
        assert_eq!(
            LockRecord::parse(r.to_text().as_bytes()),
            Some(r),
            "a parsed lock record does not survive its own text"
        );
    }

    let root = fresh();
    let dir = root.join(LIBRARY);
    std::fs::create_dir_all(&dir).expect("the library folder");
    for (path, bytes) in &case.files {
        write(&dir, path, bytes);
    }
    if let Some(m) = &manifest {
        write(&dir, MANIFEST, m);
    }
    if let Some(l) = &case.lock {
        write(&dir, lock::LOCK, l);
    }
    let tiers = LibraryTiers::under(
        &root.join("data"),
        &root.join("cache"),
        Vec::new(),
        vec![dir.clone()],
    );
    if let Ok(reg) = Registry::open(&tiers) {
        let ours: Vec<_> = reg
            .libraries()
            .into_iter()
            .filter(|l| l.tier == ew_api::library::Tier::Company)
            .collect();
        outcome.listed = !ours.is_empty();
        for library in ours {
            for (kind, _) in KINDS {
                for doc in reg.documents(library.id, kind) {
                    let value = serde_json::to_value(&doc).expect("a handed-out document has a JSON form");
                    LibDocument::load(value).expect("a handed-out document reads again");
                    outcome.documents += 1;
                }
            }
        }
    }
    if let Some(bytes) = &case.lock {
        let me = Identity::current("ew-fuzz");
        let taken = lock::acquire(&dir, &me, &LockPolicy::default());
        assert!(taken.is_err(), "another session's lock was taken over: {taken:?}");
        assert_eq!(
            std::fs::read(dir.join(lock::LOCK)).ok().as_ref(),
            Some(bytes),
            "taking the lock changed another session's lock file"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
    outcome
}
