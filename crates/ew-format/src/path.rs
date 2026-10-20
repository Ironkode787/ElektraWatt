// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Project-relative paths and what each path of a project directory is (doc 05 §2.1, §4 `files`, §15).

use std::fmt;

use ew_schema::AnyId;

/// The manifest's file name (doc 05 §4).
pub const MANIFEST: &str = "manifest.json";
/// The lock file's name (doc 05 §7).
pub const LOCK: &str = ".~ew.lock";
/// The prefix of a lock renamed away by a takeover (doc 05 §7.4).
pub const STALE_LOCK_PREFIX: &str = ".~ew.lock.stale-";
/// The infix of a temporary file: `<name>.tmp-<generation>` (doc 05 §5.2 step 4).
pub const TEMP_INFIX: &str = ".tmp-";

/// The longest internal path the writer produces (NFR-PLAT-05: ≤ 120 characters below the root).
pub const MAX_INTERNAL_PATH: usize = 120;
/// The longest path a reader accepts in a manifest (paths of a newer format are kept, not produced).
pub const MAX_PATH: usize = 1024;

/// The root JSONL files of doc 05 §2.1, in the order of the layout.
pub const ROOT_FILES: [&str; 6] = [
    "project.jsonl",
    "structure.jsonl",
    "revisions.jsonl",
    "report-blocks.jsonl",
    "waivers.jsonl",
    "comments.jsonl",
];

/// The library directories under `lib/` whose items are pretty JSON documents (doc 05 §3.4).
pub const LIB_JSON_DIRS: [&str; 7] = [
    "symbols",
    "fndefs",
    "forms",
    "parts",
    "macros",
    "manufacturers",
    "plc-profiles",
];

/// A path inside the project directory, relative to its root, with `/` separators: the form of the
/// manifest's `files` keys (doc 05 §4). Paths order byte-wise, which is the manifest order.
///
/// [`FilePath::new`] rejects every path that could leave the project directory or name a different file on
/// another platform: absolute paths, `.` and `..` segments, empty segments, backslashes, `:`, NUL and other
/// control characters, and paths over [`MAX_PATH`] bytes.
///
/// ```
/// use ew_format::FilePath;
///
/// assert_eq!(FilePath::new("pages/x.jsonl").unwrap().as_str(), "pages/x.jsonl");
/// assert!(FilePath::new("../x").is_err());
/// assert!(FilePath::new("/etc/passwd").is_err());
/// assert!(FilePath::new("a\\b").is_err());
/// assert!(FilePath::new("c:/x").is_err());
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilePath(String);

/// A path that [`FilePath::new`] rejects.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PathError {
    /// The rejected text (truncated to 200 bytes on a character boundary).
    pub path: String,
    /// Why it is rejected.
    pub reason: &'static str,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid project path \"{}\": {}", self.path, self.reason)
    }
}

impl std::error::Error for PathError {}

fn truncated(s: &str) -> String {
    let mut end = s.len().min(200);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

impl FilePath {
    /// A validated project path.
    ///
    /// # Errors
    ///
    /// [`PathError`] for a path that could leave the project directory or is not portable (see the type).
    pub fn new(path: impl Into<String>) -> Result<FilePath, PathError> {
        let path = path.into();
        let fail = |reason| {
            Err(PathError {
                path: truncated(&path),
                reason,
            })
        };
        if path.is_empty() {
            return fail("empty");
        }
        if path.len() > MAX_PATH {
            return fail("too long");
        }
        if path.starts_with('/') {
            return fail("absolute");
        }
        if path.chars().any(|c| c == '\\' || c == ':' || c.is_control()) {
            return fail("backslash, colon or control character");
        }
        if path.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
            return fail("empty, `.` or `..` segment");
        }
        Ok(FilePath(path))
    }

    /// A path this crate builds itself (always valid).
    fn known(path: String) -> FilePath {
        debug_assert!(FilePath::new(path.clone()).is_ok(), "{path}");
        FilePath(path)
    }

    /// The path text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `project.jsonl`.
    pub fn project() -> FilePath {
        FilePath::known("project.jsonl".into())
    }

    /// `structure.jsonl`.
    pub fn structure() -> FilePath {
        FilePath::known("structure.jsonl".into())
    }

    /// `report-blocks.jsonl`.
    pub fn report_blocks() -> FilePath {
        FilePath::known("report-blocks.jsonl".into())
    }

    /// A root file by name (one of [`ROOT_FILES`]).
    pub fn root(name: &'static str) -> FilePath {
        FilePath::known(name.into())
    }

    /// `pages/<id>.jsonl`.
    pub fn page(id: AnyId) -> FilePath {
        FilePath::known(format!("pages/{id}.jsonl"))
    }

    /// `devices/<xx>.jsonl`, `xx` the anchor ID's last byte in lower-case hex (doc 05 §2.2).
    pub fn shard(anchor: AnyId) -> FilePath {
        FilePath::known(format!("devices/{}.jsonl", anchor.shard_name()))
    }

    /// `layout/<id>.jsonl`.
    pub fn layout(space: AnyId) -> FilePath {
        FilePath::known(format!("layout/{space}.jsonl"))
    }

    /// `imports/<id>.jsonl`.
    pub fn imports(run: AnyId) -> FilePath {
        FilePath::known(format!("imports/{run}.jsonl"))
    }

    /// `lib/<dir>/<id>.json`.
    pub fn lib(dir: &str, id: AnyId) -> FilePath {
        FilePath::known(format!("lib/{dir}/{id}.json"))
    }

    /// The temporary name of this file in a save of `generation`: `<path>.tmp-<generation>`.
    pub fn temp(&self, generation: u64) -> String {
        format!("{}{TEMP_INFIX}{generation}", self.0)
    }

    /// The name a save of `generation` moves a file to that it finds at this path although the manifest
    /// it loaded does not list it (another user's file, a sync client's copy), before writing its own file
    /// here: `<dir>/<stem>.unlisted-<generation>.<ext>`, with `-<n>` after the generation for `n > 0` when
    /// that name is taken. [`classify`] reports every such name as [`PathClass::Stray`], so the writer never
    /// reads, rewrites or deletes it, and every open reports it as EW-CORE-004 until the user merges or
    /// deletes it (review cycle 8, owner ruling: never rename over a file that is not in the baseline).
    ///
    /// ```
    /// use ew_format::{FilePath, PathClass, classify};
    ///
    /// let shard = FilePath::new("devices/00.jsonl").unwrap();
    /// assert_eq!(shard.unlisted(7, 0).as_str(), "devices/00.unlisted-7.jsonl");
    /// assert_eq!(shard.unlisted(7, 2).as_str(), "devices/00.unlisted-7-2.jsonl");
    /// assert_eq!(classify(shard.unlisted(7, 0).as_str()), PathClass::Stray);
    /// ```
    pub fn unlisted(&self, generation: u64, n: u32) -> FilePath {
        let name = self.file_name();
        let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
        let suffix = if n == 0 {
            format!("unlisted-{generation}")
        } else {
            format!("unlisted-{generation}-{n}")
        };
        let renamed = if ext.is_empty() {
            format!("{stem}.{suffix}")
        } else {
            format!("{stem}.{suffix}.{ext}")
        };
        let dir = self.dir();
        FilePath::known(if dir.is_empty() {
            renamed
        } else {
            format!("{dir}/{renamed}")
        })
    }

    /// The directory part (`pages` for `pages/x.jsonl`), `""` for a root file.
    pub fn dir(&self) -> &str {
        self.0.rsplit_once('/').map_or("", |(d, _)| d)
    }

    /// The last segment.
    pub fn file_name(&self) -> &str {
        self.0.rsplit_once('/').map_or(&self.0, |(_, n)| n)
    }

    /// Whether the path is short enough for an internal name (NFR-PLAT-05).
    pub fn is_portable(&self) -> bool {
        self.0.len() <= MAX_INTERNAL_PATH
    }
}

impl fmt::Display for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl AsRef<str> for FilePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A JSON Lines record file of the layout (doc 05 §2.1, §2.2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum RecordFile {
    /// One of [`ROOT_FILES`].
    Root(&'static str),
    /// `pages/<id>.jsonl`.
    Page(AnyId),
    /// `devices/<xx>.jsonl`.
    Shard(u8),
    /// `layout/<id>.jsonl`.
    Layout(AnyId),
    /// `imports/<id>.jsonl`.
    Imports(AnyId),
    /// `exchange/<id>.jsonl` (1.0).
    Exchange(AnyId),
}

/// What a file kept only by its hash is (doc 05 §2.1): the writer never rewrites or deletes it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum OpaqueKind {
    /// `assets/<sha256>.<ext>`: content-addressed; the name must match the bytes (EW-CORE-015).
    Asset {
        /// The hash in the name.
        sha256: [u8; 32],
    },
    /// `lib/profiles/<id>.toml` (1.0): a profile in its TOML source form.
    ProfileToml,
    /// `rules/**`, `reports/*.sql`, `scripts/**`, `jobs/**`, `trust.sig`: untrusted content (doc 05 §13.4).
    Untrusted,
}

/// What a path of a project directory is (doc 05 §2.1, §4, §15).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PathClass {
    /// A JSON Lines record file.
    Records(RecordFile),
    /// A library document `lib/<dir>/<id>.json` of one of [`LIB_JSON_DIRS`].
    LibDocument {
        /// The directory under `lib/`.
        dir: &'static str,
        /// The item ID in the file name.
        id: AnyId,
    },
    /// A file the writer keeps by hash only.
    Opaque(OpaqueKind),
    /// `manifest.json`.
    Manifest,
    /// `manifest.json.tmp-<g>`: the intent of an interrupted save (doc 05 §5.3).
    Intent {
        /// The generation it would complete.
        generation: u64,
    },
    /// `<path>.tmp-<g>`: a temporary file of a save.
    Temp {
        /// The file it replaces.
        target: String,
        /// The generation of the save.
        generation: u64,
    },
    /// `.~ew.lock`.
    Lock,
    /// `.~ew.lock.stale-<session>`.
    StaleLock,
    /// Not project data and not in the manifest: `.ew/**`, `.~ew.requests/**`, `.gitattributes`,
    /// `.gitignore`, `.git/**`.
    Ignored,
    /// A file in an ElektraWatt place whose name is not canonical: a sync-conflict copy, an orphan or a
    /// hand-made file (EW-CORE-004 when the manifest does not list it).
    Stray,
    /// Anything else (a user's notes beside the data): ignored unless the manifest lists it.
    Foreign,
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

fn parse_sha256_hex(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in b.chunks_exact(2).enumerate() {
        out[i] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(out)
}

/// A canonical (lower-case hyphenated) UUID followed by `ext`.
fn id_file(name: &str, ext: &str) -> Option<AnyId> {
    name.strip_suffix(ext)?.parse().ok()
}

/// Splits `<target>.tmp-<digits>`.
fn temp_parts(path: &str) -> Option<(&str, u64)> {
    let (target, generation) = path.rsplit_once(TEMP_INFIX)?;
    if target.is_empty() || generation.is_empty() || !generation.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if generation.len() > 1 && generation.starts_with('0') {
        return None;
    }
    Some((target, generation.parse().ok()?))
}

/// Classifies a project-relative path (`/` separators).
///
/// ```
/// use ew_format::{PathClass, RecordFile, classify};
///
/// assert_eq!(classify("devices/c2.jsonl"), PathClass::Records(RecordFile::Shard(0xc2)));
/// assert_eq!(classify("devices/C2.jsonl"), PathClass::Stray);
/// assert!(matches!(classify("manifest.json.tmp-7"), PathClass::Intent { generation: 7 }));
/// assert_eq!(classify("README.txt"), PathClass::Foreign);
/// ```
pub fn classify(path: &str) -> PathClass {
    if path == MANIFEST {
        return PathClass::Manifest;
    }
    if path == LOCK {
        return PathClass::Lock;
    }
    if path.starts_with(STALE_LOCK_PREFIX) && !path.contains('/') {
        return PathClass::StaleLock;
    }
    let first = path.split('/').next().unwrap_or("");
    if matches!(first, ".ew" | ".~ew.requests" | ".git") && path.len() > first.len()
        || matches!(path, ".gitattributes" | ".gitignore" | ".git")
    {
        return PathClass::Ignored;
    }
    if let Some((target, generation)) = temp_parts(path) {
        if target == MANIFEST {
            return PathClass::Intent { generation };
        }
        return PathClass::Temp {
            target: target.to_owned(),
            generation,
        };
    }
    let segments: Vec<&str> = path.split('/').collect();
    match segments.as_slice() {
        [name] => {
            if let Some(root) = ROOT_FILES.iter().find(|r| **r == *name) {
                PathClass::Records(RecordFile::Root(root))
            } else if *name == "trust.sig" {
                PathClass::Opaque(OpaqueKind::Untrusted)
            } else if name.ends_with(".jsonl") || name.starts_with("manifest") {
                PathClass::Stray
            } else {
                PathClass::Foreign
            }
        }
        ["pages", name] => {
            id_file(name, ".jsonl").map_or(PathClass::Stray, |id| PathClass::Records(RecordFile::Page(id)))
        }
        ["layout", name] => {
            id_file(name, ".jsonl").map_or(PathClass::Stray, |id| PathClass::Records(RecordFile::Layout(id)))
        }
        ["imports", name] => {
            id_file(name, ".jsonl").map_or(PathClass::Stray, |id| PathClass::Records(RecordFile::Imports(id)))
        }
        ["exchange", name] => id_file(name, ".jsonl").map_or(PathClass::Stray, |id| {
            PathClass::Records(RecordFile::Exchange(id))
        }),
        ["devices", name] => {
            let b = name.as_bytes();
            if b.len() == 8 && name.ends_with(".jsonl") {
                match (hex_nibble(b[0]), hex_nibble(b[1])) {
                    (Some(h), Some(l)) => PathClass::Records(RecordFile::Shard((h << 4) | l)),
                    _ => PathClass::Stray,
                }
            } else {
                PathClass::Stray
            }
        }
        ["lib", "profiles", name] => {
            if id_file(name, ".toml").is_some() {
                PathClass::Opaque(OpaqueKind::ProfileToml)
            } else {
                PathClass::Stray
            }
        }
        ["lib", dir, name] => match LIB_JSON_DIRS.iter().find(|d| **d == *dir) {
            Some(dir) => {
                id_file(name, ".json").map_or(PathClass::Stray, |id| PathClass::LibDocument { dir, id })
            }
            None => PathClass::Stray,
        },
        ["assets", name] => {
            let asset = name.split_once('.').and_then(|(hash, ext)| {
                let sha256 = parse_sha256_hex(hash)?;
                matches!(ext, "svg" | "png" | "jpg" | "glb").then_some(sha256)
            });
            asset.map_or(PathClass::Stray, |sha256| {
                PathClass::Opaque(OpaqueKind::Asset { sha256 })
            })
        }
        ["rules", ..] | ["scripts", ..] | ["jobs", ..] => PathClass::Opaque(OpaqueKind::Untrusted),
        ["reports", name] if name.ends_with(".sql") => PathClass::Opaque(OpaqueKind::Untrusted),
        [
            "pages" | "devices" | "layout" | "imports" | "exchange" | "lib" | "assets" | "reports",
            ..,
        ] => PathClass::Stray,
        _ => PathClass::Foreign,
    }
}

impl RecordFile {
    /// The file's path.
    pub fn path(self) -> FilePath {
        match self {
            RecordFile::Root(name) => FilePath::root(name),
            RecordFile::Page(id) => FilePath::page(id),
            RecordFile::Shard(b) => FilePath::known(format!("devices/{b:02x}.jsonl")),
            RecordFile::Layout(id) => FilePath::layout(id),
            RecordFile::Imports(id) => FilePath::imports(id),
            RecordFile::Exchange(id) => FilePath::known(format!("exchange/{id}.jsonl")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a";

    #[test]
    fn layout_paths_classify_as_themselves() {
        let id: AnyId = ID.parse().unwrap();
        for p in [
            FilePath::project(),
            FilePath::structure(),
            FilePath::report_blocks(),
            FilePath::page(id),
            FilePath::shard(id),
            FilePath::layout(id),
            FilePath::imports(id),
        ] {
            let PathClass::Records(rf) = classify(p.as_str()) else {
                panic!("{p} is not a record file");
            };
            assert_eq!(rf.path(), p);
            assert!(p.is_portable());
        }
        assert_eq!(
            classify(FilePath::lib("parts", id).as_str()),
            PathClass::LibDocument { dir: "parts", id }
        );
    }

    #[test]
    fn unlisted_names_are_strays_of_every_writer_owned_path() {
        let id: AnyId = ID.parse().unwrap();
        let mut owned: Vec<FilePath> = ROOT_FILES.iter().map(|r| FilePath::root(r)).collect();
        owned.extend([
            FilePath::page(id),
            FilePath::shard(id),
            FilePath::layout(id),
            FilePath::imports(id),
            RecordFile::Exchange(id).path(),
        ]);
        owned.extend(LIB_JSON_DIRS.iter().map(|d| FilePath::lib(d, id)));
        for path in owned {
            for (g, n) in [(1, 0), (u64::MAX, 0), (12, 3), (12, u32::MAX)] {
                let moved = path.unlisted(g, n);
                assert_eq!(classify(moved.as_str()), PathClass::Stray, "{moved}");
                assert_eq!(FilePath::new(moved.as_str()), Ok(moved.clone()));
                assert_eq!(moved.dir(), path.dir());
                assert!(moved.is_portable(), "{moved}");
            }
        }
    }

    #[test]
    fn strays_temps_and_ignored() {
        assert_eq!(
            classify("pages/8E41C0D2-1B7F-5A39-A6C4-0F5E2D913B7A.jsonl"),
            PathClass::Stray
        );
        assert_eq!(
            classify(&format!("pages/{ID} (conflicted copy).jsonl")),
            PathClass::Stray
        );
        assert_eq!(classify("devices/c2-HOST.jsonl"), PathClass::Stray);
        assert_eq!(classify("project 2.jsonl"), PathClass::Stray);
        assert_eq!(
            classify("devices/c2.jsonl.tmp-12"),
            PathClass::Temp {
                target: "devices/c2.jsonl".into(),
                generation: 12
            }
        );
        assert_eq!(classify("devices/c2.jsonl.tmp-012"), PathClass::Stray);
        assert_eq!(classify(".ew/u-1234abcd/view.json"), PathClass::Ignored);
        assert_eq!(classify(".gitignore"), PathClass::Ignored);
        assert_eq!(classify(LOCK), PathClass::Lock);
        assert_eq!(classify(".~ew.lock.stale-abc"), PathClass::StaleLock);
        assert_eq!(classify("notes.md"), PathClass::Foreign);
        let asset = format!("assets/{}.svg", "ab".repeat(32));
        assert!(matches!(
            classify(&asset),
            PathClass::Opaque(OpaqueKind::Asset { .. })
        ));
        assert_eq!(classify("assets/logo.svg"), PathClass::Stray);
        assert_eq!(
            classify("rules/x/y.rhai"),
            PathClass::Opaque(OpaqueKind::Untrusted)
        );
    }

    #[test]
    fn rejects_escaping_paths() {
        for bad in [
            "", "/a", "a//b", "a/./b", "a/../b", "..", "a\\b", "c:x", "a\0b", "a/",
        ] {
            assert!(FilePath::new(bad).is_err(), "{bad:?}");
        }
    }
}
