// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The project reader (doc 05 §3, §6): which files to read, and the records, kept lines, findings and
//! rewrite set of a project's files. Pure: the caller does the I/O (`ew-store` through its `Vfs`).

use std::collections::{BTreeMap, BTreeSet};

use ew_schema::{AnyId, Kind, LibDocument, MalformedVariant, Project, Record, SchemaError};
use serde_json::Value;

use crate::finding::{Finding, FindingKind, Findings};
use crate::layout::RecordAnchors;
use crate::limits::Limits;
use crate::line::jsonl_file;
use crate::manifest::{FileHash, Manifest, sha256};
use crate::order::LineKey;
use crate::path::{FilePath, OpaqueKind, PathClass, classify};

/// What a directory entry is, as the caller's file system reports it without following links.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// A symbolic link or a junction: never followed (NFR-SEC-16).
    Link,
    /// Anything else (a device, a socket, a placeholder the OS cannot classify).
    Other,
}

/// The files of a project directory to read, from the manifest and a listing (doc 05 §6.1, §6.2).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ReadPlan {
    /// The files to read and hand to [`load`], in path order.
    pub read: Vec<FilePath>,
    /// Findings of the plan itself: EW-CORE-003 for a listed file that is absent or not a regular file,
    /// EW-CORE-004 for an unlisted [`PathClass::Stray`](crate::PathClass::Stray) file and for an unlisted file
    /// of the layout while a manifest is present, [`FindingKind::LimitExceeded`](crate::FindingKind) for more
    /// than [`Limits::max_files`] files.
    pub findings: Vec<Finding>,
}

/// Plans the reads of an open (doc 05 §6.1):
///
/// - with a manifest, exactly the listed files are read (the manifest is authoritative, doc 05 §15 row
///   12): a listed file that `present` lacks or that is not [`EntryKind::File`] is EW-CORE-003 and not
///   read; an unlisted record or library file of the layout, and an unlisted stray, is EW-CORE-004 and not
///   read (a file removed by an interrupted save, a conflict copy);
/// - without one (`manifest == None`: missing or unusable, EW-CORE-001 is the caller's), every record
///   file, library document and opaque file of the layout among `present` is read; strays are EW-CORE-004.
///
/// `present` maps project-relative paths (`/` separators) of every entry found below the root (the caller
/// lists the layout's directories) to their kind. Temps, intents, the manifest, locks and ignored paths
/// are never read here.
pub fn plan(manifest: Option<&Manifest>, present: &BTreeMap<String, EntryKind>, limits: &Limits) -> ReadPlan {
    let mut plan = ReadPlan::default();
    if present.len() > limits.max_files {
        plan.findings.push(Finding::new(
            FindingKind::LimitExceeded,
            None,
            None,
            format!(
                "{} directory entries, more than the limit of {}",
                present.len(),
                limits.max_files
            ),
        ));
        return plan;
    }
    match manifest {
        Some(manifest) => plan_listed(manifest, present, &mut plan),
        None => plan_unlisted(present, &mut plan),
    }
    plan.read.sort();
    plan.findings.sort();
    plan
}

/// A finding about the entry `name`, whose [`FilePath`] may not exist (a name `FilePath::new` rejects).
fn entry_finding(kind: FindingKind, name: &str, message: &str) -> Finding {
    match FilePath::new(name) {
        Ok(path) => Finding::new(kind, Some(path), None, message),
        Err(e) => Finding::new(kind, None, None, format!("{message} ({e})")),
    }
}

const NOT_REGULAR: &str = "is not a regular file; links are not followed";

fn plan_listed(manifest: &Manifest, present: &BTreeMap<String, EntryKind>, plan: &mut ReadPlan) {
    for path in manifest.files.keys() {
        match present.get(path.as_str()) {
            Some(EntryKind::File) => plan.read.push(path.clone()),
            Some(_) => plan.findings.push(Finding::new(
                FindingKind::MissingFile,
                Some(path.clone()),
                None,
                format!("listed in the manifest but {NOT_REGULAR} (NFR-SEC-16)"),
            )),
            None => plan.findings.push(Finding::new(
                FindingKind::MissingFile,
                Some(path.clone()),
                None,
                "listed in the manifest but missing",
            )),
        }
    }
    for (name, kind) in present {
        if *kind == EntryKind::Dir {
            continue;
        }
        let listed = FilePath::new(name.as_str()).is_ok_and(|p| manifest.files.contains_key(&p));
        if listed {
            continue;
        }
        if matches!(
            classify(name),
            PathClass::Records(_) | PathClass::LibDocument { .. } | PathClass::Opaque(_) | PathClass::Stray
        ) {
            plan.findings.push(entry_finding(
                FindingKind::Orphan,
                name,
                "not listed in the manifest (an orphan or a conflict copy); not read",
            ));
        }
    }
}

fn plan_unlisted(present: &BTreeMap<String, EntryKind>, plan: &mut ReadPlan) {
    for (name, kind) in present {
        if *kind == EntryKind::Dir {
            continue;
        }
        match classify(name) {
            PathClass::Records(_) | PathClass::LibDocument { .. } | PathClass::Opaque(_) => {
                match (kind, FilePath::new(name.as_str())) {
                    (EntryKind::File, Ok(path)) => plan.read.push(path),
                    (EntryKind::File, Err(_)) => plan.findings.push(entry_finding(
                        FindingKind::Orphan,
                        name,
                        "not a valid project path; not read",
                    )),
                    _ => plan.findings.push(entry_finding(
                        FindingKind::MissingFile,
                        name,
                        &format!("{NOT_REGULAR} (NFR-SEC-16)"),
                    )),
                }
            }
            PathClass::Stray => plan.findings.push(entry_finding(
                FindingKind::Orphan,
                name,
                "a file of a layout directory with a non-canonical name (an orphan or a conflict copy); not read",
            )),
            _ => {}
        }
    }
}

/// Why a line is kept verbatim instead of (or besides) being a record of the model.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Kept {
    /// A record of an unknown or reserved kind (doc 05 §10.3): its [`Record::Unknown`] is also in
    /// [`Loaded::records`]; the writer keeps the line in this file, in its sort position.
    UnknownKind,
    /// A record of a known kind with a malformed variant (EW-CORE-017), whose record is also in
    /// [`Loaded::records`]: written verbatim while the model's record equals `original`, rewritten in
    /// canonical form once it changes (doc 05 §10.3, ADR-0011 amendment 2).
    Untouched {
        /// The record as loaded.
        original: Box<Record>,
    },
    /// A line that is not in [`Loaded::records`]: a syntax error (EW-CORE-005), a conflict marker line
    /// (EW-CORE-006), a schema violation (EW-CORE-007) or a duplicate (EW-CORE-009). Always written
    /// verbatim, so nothing is lost.
    Quarantined,
}

/// One line kept verbatim.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeptLine {
    /// The file it was read from.
    pub file: FilePath,
    /// The 1-based line number.
    pub line: u32,
    /// The sort key: from `$k` and `id` where the line has them, else [`LineKey::keyless`]; `seq` is 0 for
    /// [`Kept::UnknownKind`] and [`Kept::Untouched`], the line number for [`Kept::Quarantined`].
    pub key: LineKey,
    /// The original bytes, without the line end (a trailing CR is removed): UTF-8 text, except for a
    /// [`Kept::Quarantined`] line that is not UTF-8 (EW-CORE-005), which is kept and written back byte for
    /// byte. [`KeptLine::text_lossy`] gives a printable form.
    pub text: Vec<u8>,
    /// Why it is kept.
    pub kept: Kept,
}

impl KeptLine {
    /// The line as text for messages and display (invalid UTF-8 replaced by U+FFFD); never for writing.
    pub fn text_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.text)
    }
}

/// A library document with a malformed variant, written verbatim while the model's copy equals `original`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeptDocument {
    /// Its file.
    pub file: FilePath,
    /// The original bytes.
    pub text: Vec<u8>,
    /// The document as loaded.
    pub original: Box<LibDocument>,
}

/// The content of a project's files (doc 05 §6.1 "parse").
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Loaded {
    /// The first `project` record.
    pub project: Option<Project>,
    /// The first occurrence of every `($k, id)` other than the project, in (file, line) order: known
    /// kinds and [`Record::Unknown`] records of unknown and reserved kinds.
    pub records: Vec<Record>,
    /// The library documents of known kinds, and of unknown kinds as [`LibDocument::Unknown`], in path
    /// order.
    pub lib: Vec<LibDocument>,
    /// Lines kept verbatim, in (file, line) order.
    pub kept: Vec<KeptLine>,
    /// Library documents kept verbatim while untouched.
    pub kept_docs: Vec<KeptDocument>,
    /// Files kept by hash only, never rewritten: assets, untrusted content, profile TOML, library documents
    /// of unknown kinds and listed files of no known class (a newer format).
    pub opaque: BTreeMap<FilePath, [u8; 32]>,
    /// The SHA-256 of every file read, as found on disk.
    pub hashes: BTreeMap<FilePath, [u8; 32]>,
    /// Files to rewrite at the next save although no record in them changes: non-canonical files (a BOM,
    /// CRLF, blank lines, a missing final LF, non-NFC text, a member out of order, lines out of order) and
    /// the source and target files of a record or document in the wrong file (EW-CORE-010).
    pub rewrite: BTreeSet<FilePath>,
    /// The findings, sorted, capped at [`Limits::max_findings`].
    pub findings: Vec<Finding>,
    /// Whether a finding forces a read-only session.
    pub read_only: bool,
}

/// Reads the files of a project (doc 05 §3, §6.2, §6.3).
///
/// `files` holds the bytes of every file of a [`ReadPlan`]; `manifest` the manifest they were planned
/// from (`None` without a usable one). The findings, in short (see [`FindingKind`](crate::FindingKind)):
///
/// - **hashes**: a file whose bytes do not match its manifest hash is `LineEndings` when CRLF → LF makes it
///   match, else `ForeignEdit`; a `Pending` hash in a merged manifest is `ForeignEdit` too;
/// - **record files**: one record per line; per line, in this order: the line-length limit, conflict
///   markers (EW-CORE-006), UTF-8 and JSON syntax with duplicate members rejected (EW-CORE-005 for a syntax
///   error, EW-CORE-007 for a duplicate member), the nesting depth (`LimitExceeded`), NFC normalisation
///   ([`ew_schema::normalize_nfc`]), [`Record::load`] (EW-CORE-007 on error, EW-CORE-017 per malformed
///   variant), duplicates (EW-CORE-009) and EW-CORE-016 once per unknown `$k` per file;
/// - **library documents**: the same per document with [`LibDocument::load`], the file name's ID and
///   directory checked against the document (EW-CORE-010), and the copy hash (EW-CORE-014);
/// - **assets**: the hash in the name (EW-CORE-015);
/// - **canonical form**: a record file or library document whose bytes differ from what the writer would
///   write for its content is added to [`Loaded::rewrite`] with a `NonCanonical` finding;
/// - **placement**: a record whose file ([`record_file`](crate::record_file) over
///   [`RecordAnchors`](crate::RecordAnchors) of all loaded records) is not the file it was read from is
///   EW-CORE-010, and both files are added to [`Loaded::rewrite`].
pub fn load(manifest: Option<&Manifest>, files: &BTreeMap<FilePath, Vec<u8>>, limits: &Limits) -> Loaded {
    let mut loader = Loader {
        manifest,
        limits,
        findings: Findings::new(limits.max_findings),
        out: Loaded::default(),
        seen: BTreeMap::new(),
        project_at: None,
        placed: Vec::new(),
    };
    for (path, bytes) in files {
        loader.file(path, bytes);
    }
    loader.place();
    loader.finish()
}

/// The Git conflict-marker prefixes of a JSON Lines file (`|||||||` is the diff3 base marker).
const LINE_MARKERS: [&[u8]; 4] = [b"<<<<<<<", b"=======", b">>>>>>>", b"|||||||"];

/// The file and line a `($k, id)` was first read from (no line for a library document).
type Origin = (FilePath, Option<u32>);

/// Where a record was read from, for the placement check after all files are read.
struct Placed {
    file: FilePath,
    line: u32,
    target: Target,
}

enum Target {
    Project,
    /// An index into [`Loaded::records`].
    Record(usize),
}

/// What is collected while one record file is read.
struct FileState {
    /// The lines the writer would write for this file, for the canonical check.
    lines: Vec<(LineKey, Vec<u8>)>,
    /// The unknown `$k` values already reported for this file (one EW-CORE-016 each).
    unknown_kinds: BTreeSet<String>,
    non_canonical: bool,
}

struct Loader<'a> {
    manifest: Option<&'a Manifest>,
    limits: &'a Limits,
    findings: Findings,
    out: Loaded,
    seen: BTreeMap<(String, AnyId), Origin>,
    project_at: Option<Origin>,
    placed: Vec<Placed>,
}

/// `bytes` with every `\r\n` replaced by `\n`.
fn crlf_to_lf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Whether the JSON text nests arrays and objects deeper than `max`: one linear scan that ignores brackets
/// inside strings, run before the text reaches a recursive parser.
fn exceeds_depth(text: &[u8], max: usize) -> bool {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for &b in text {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' | b'{' => {
                depth += 1;
                if depth > max {
                    return true;
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

fn line_number(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The finding for a text that is not one JSON value: syntax (EW-CORE-005) or a duplicate member
/// (EW-CORE-007).
fn parse_failure(e: SchemaError) -> (FindingKind, String) {
    match e {
        SchemaError::Json { message } => (FindingKind::JsonSyntax, message),
        other => (FindingKind::SchemaViolation, other.to_string()),
    }
}

impl Loader<'_> {
    fn find(&mut self, kind: FindingKind, file: &FilePath, line: Option<u32>, message: impl Into<String>) {
        self.findings
            .push(Finding::new(kind, Some(file.clone()), line, message));
    }

    fn opaque(&mut self, path: &FilePath, hash: [u8; 32]) {
        self.out.opaque.insert(path.clone(), hash);
    }

    fn file(&mut self, path: &FilePath, bytes: &[u8]) {
        let hash = sha256(bytes);
        self.out.hashes.insert(path.clone(), hash);
        self.check_hash(path, bytes, &hash);
        if bytes.len() as u64 > self.limits.max_file_bytes {
            self.find(
                FindingKind::LimitExceeded,
                path,
                None,
                format!(
                    "{} bytes, more than the limit of {}; not read",
                    bytes.len(),
                    self.limits.max_file_bytes
                ),
            );
            self.opaque(path, hash);
            return;
        }
        match classify(path.as_str()) {
            PathClass::Records(_) => self.record_file(path, bytes),
            PathClass::LibDocument { .. } => self.lib_document(path, bytes, hash),
            PathClass::Opaque(OpaqueKind::Asset { sha256: named }) => {
                if hash != named {
                    self.find(
                        FindingKind::AssetMismatch,
                        path,
                        None,
                        "the content does not match the hash in the file name",
                    );
                }
                self.opaque(path, hash);
            }
            PathClass::Opaque(_) => self.opaque(path, hash),
            _ => {
                self.opaque(path, hash);
                self.find(
                    FindingKind::NewerData,
                    path,
                    None,
                    "a file this reader does not know is kept unchanged",
                );
            }
        }
    }

    fn check_hash(&mut self, path: &FilePath, bytes: &[u8], hash: &[u8; 32]) {
        let Some(listed) = self.manifest.and_then(|m| m.files.get(path)) else {
            return;
        };
        match listed {
            FileHash::Sha256(expected) if expected != hash => {
                if sha256(&crlf_to_lf(bytes)) == *expected {
                    self.find(
                        FindingKind::LineEndings,
                        path,
                        None,
                        "CRLF line ends; the content matches the manifest hash with LF; rewritten at the next save",
                    );
                } else {
                    self.find(
                        FindingKind::ForeignEdit,
                        path,
                        None,
                        "changed outside ElektraWatt (the hash differs from the manifest)",
                    );
                }
            }
            FileHash::Sha256(_) => {}
            FileHash::Pending => self.find(
                FindingKind::ForeignEdit,
                path,
                None,
                "the manifest hash is pending after a merge",
            ),
        }
    }

    // ---- JSON Lines files -------------------------------------------------------------------------------

    fn record_file(&mut self, path: &FilePath, bytes: &[u8]) {
        let mut st = FileState {
            lines: Vec::new(),
            unknown_kinds: BTreeSet::new(),
            non_canonical: bytes.is_empty(),
        };
        let mut data = bytes;
        if let Some(rest) = data.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
            data = rest;
            st.non_canonical = true;
        }
        if !data.is_empty() {
            let body = match data.strip_suffix(b"\n") {
                Some(body) => body,
                None => {
                    st.non_canonical = true;
                    data
                }
            };
            for (i, raw) in body.split(|b| *b == b'\n').enumerate() {
                let n = i + 1;
                if n > self.limits.max_lines_per_file {
                    self.find(
                        FindingKind::LimitExceeded,
                        path,
                        None,
                        format!(
                            "more than {} lines; the rest of the file is not read",
                            self.limits.max_lines_per_file
                        ),
                    );
                    // Part of the file was not read: never schedule it for a rewrite (the finding also
                    // forces a read-only session).
                    return;
                }
                self.line(path, n, raw, &mut st);
            }
        }
        if st.non_canonical || jsonl_file(st.lines) != bytes {
            self.out.rewrite.insert(path.clone());
            self.find(
                FindingKind::NonCanonical,
                path,
                None,
                "not in canonical form; rewritten at the next save",
            );
        }
    }

    /// Keeps a line verbatim (its bytes), without a record.
    fn quarantine(&mut self, path: &FilePath, line: u32, key: LineKey, text: Vec<u8>, st: &mut FileState) {
        st.lines.push((key.clone(), text.clone()));
        self.out.kept.push(KeptLine {
            file: path.clone(),
            line,
            key,
            text,
            kept: Kept::Quarantined,
        });
    }

    fn line(&mut self, path: &FilePath, n: usize, raw: &[u8], st: &mut FileState) {
        let ln = line_number(n);
        let mut bytes = raw;
        if let Some(rest) = bytes.strip_suffix(b"\r") {
            bytes = rest;
            st.non_canonical = true;
        }
        if bytes.is_empty() {
            st.non_canonical = true;
            return;
        }
        let keyless = LineKey::keyless().with_seq(n as u64);
        if bytes.len() > self.limits.max_line_bytes {
            self.find(
                FindingKind::LimitExceeded,
                path,
                Some(ln),
                format!(
                    "line of {} bytes, more than the limit of {}",
                    bytes.len(),
                    self.limits.max_line_bytes
                ),
            );
            self.quarantine(path, ln, keyless, bytes.to_vec(), st);
            return;
        }
        if LINE_MARKERS.iter().any(|m| bytes.starts_with(m)) {
            self.find(
                FindingKind::ConflictMarkers,
                path,
                Some(ln),
                "a git conflict marker line",
            );
            self.quarantine(path, ln, keyless, bytes.to_vec(), st);
            return;
        }
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(e) => {
                self.find(
                    FindingKind::JsonSyntax,
                    path,
                    Some(ln),
                    format!("the line is not UTF-8: {e}"),
                );
                self.quarantine(path, ln, keyless, bytes.to_vec(), st);
                return;
            }
        };
        if exceeds_depth(bytes, self.limits.max_depth) {
            self.find(
                FindingKind::LimitExceeded,
                path,
                Some(ln),
                format!("nesting deeper than {}", self.limits.max_depth),
            );
            self.quarantine(path, ln, keyless, bytes.to_vec(), st);
            return;
        }
        match ew_schema::parse_value(text) {
            Ok(value) => self.record(path, n, text, value, st),
            Err(e) => {
                let (kind, message) = parse_failure(e);
                self.find(kind, path, Some(ln), message);
                self.quarantine(path, ln, keyless, bytes.to_vec(), st);
            }
        }
    }

    fn record(&mut self, path: &FilePath, n: usize, text: &str, mut value: Value, st: &mut FileState) {
        let ln = line_number(n);
        let base_key = LineKey::of_value(&value);
        let quarantine_key = base_key.clone().with_seq(n as u64);
        if ew_schema::normalize_nfc(&mut value) {
            st.non_canonical = true;
        }
        let (record, malformed) = match Record::load(value) {
            Ok(loaded) => loaded,
            Err(e) => {
                self.find(FindingKind::SchemaViolation, path, Some(ln), e.to_string());
                self.quarantine(path, ln, quarantine_key, text.as_bytes().to_vec(), st);
                return;
            }
        };
        for m in &malformed {
            self.find(FindingKind::MalformedVariant, path, Some(ln), m.to_string());
        }
        let (Some(kind_name), Some(id)) = (
            record.kind_name().map(str::to_owned),
            record.id().map(AnyId::from_uuid),
        ) else {
            self.find(
                FindingKind::SchemaViolation,
                path,
                Some(ln),
                "missing or invalid id",
            );
            self.quarantine(path, ln, quarantine_key, text.as_bytes().to_vec(), st);
            return;
        };
        let is_project = matches!(record, Record::Project(_));
        let first = self
            .seen
            .get(&(kind_name.clone(), id))
            .cloned()
            .or_else(|| self.project_at.clone().filter(|_| is_project));
        if let Some((first_file, first_line)) = first {
            let at = first_line.map_or_else(|| first_file.to_string(), |l| format!("{first_file}:{l}"));
            self.find(
                FindingKind::DuplicateRecord,
                path,
                Some(ln),
                format!("{kind_name}:{id} duplicates a record first read at {at}; the first is kept"),
            );
            self.quarantine(path, ln, quarantine_key, text.as_bytes().to_vec(), st);
            return;
        }
        self.seen
            .insert((kind_name.clone(), id), (path.clone(), Some(ln)));

        let kept = if matches!(record, Record::Unknown(_)) {
            if Kind::from_name(&kind_name).is_none() && st.unknown_kinds.insert(kind_name.clone()) {
                self.find(
                    FindingKind::NewerData,
                    path,
                    Some(ln),
                    format!(
                        "a record of the kind \"{kind_name}\", unknown to this reader, is kept unchanged"
                    ),
                );
            }
            Some(Kept::UnknownKind)
        } else if malformed.is_empty() {
            None
        } else {
            Some(Kept::Untouched {
                original: Box::new(record.clone()),
            })
        };
        match kept {
            Some(kept) => {
                st.lines.push((base_key.clone(), text.as_bytes().to_vec()));
                self.out.kept.push(KeptLine {
                    file: path.clone(),
                    line: ln,
                    key: base_key,
                    text: text.as_bytes().to_vec(),
                    kept,
                });
            }
            None => match (LineKey::of(&record), crate::record_line(&record)) {
                (Some(key), Ok(line)) => st.lines.push((key, line.into_bytes())),
                _ => st.non_canonical = true,
            },
        }
        let known = record.kind().is_some();
        match record {
            Record::Project(project) => {
                self.out.project = Some(project);
                self.project_at = Some((path.clone(), Some(ln)));
                self.placed.push(Placed {
                    file: path.clone(),
                    line: ln,
                    target: Target::Project,
                });
            }
            record => {
                if known {
                    self.placed.push(Placed {
                        file: path.clone(),
                        line: ln,
                        target: Target::Record(self.out.records.len()),
                    });
                }
                self.out.records.push(record);
            }
        }
    }

    // ---- library documents ------------------------------------------------------------------------------

    fn lib_document(&mut self, path: &FilePath, bytes: &[u8], hash: [u8; 32]) {
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(e) => {
                self.find(
                    FindingKind::JsonSyntax,
                    path,
                    None,
                    format!("the document is not UTF-8: {e}"),
                );
                self.opaque(path, hash);
                return;
            }
        };
        if exceeds_depth(bytes, self.limits.max_depth) {
            self.find(
                FindingKind::LimitExceeded,
                path,
                None,
                format!("nesting deeper than {}", self.limits.max_depth),
            );
            self.opaque(path, hash);
            return;
        }
        let mut value = match ew_schema::parse_value(text) {
            Ok(value) => value,
            Err(e) => {
                let (kind, message) = parse_failure(e);
                self.find(kind, path, None, message);
                self.opaque(path, hash);
                return;
            }
        };
        let normalised = ew_schema::normalize_nfc(&mut value);
        let (doc, malformed) = match LibDocument::load(value) {
            Ok(loaded) => loaded,
            Err(e) => {
                self.find(FindingKind::SchemaViolation, path, None, e.to_string());
                self.opaque(path, hash);
                return;
            }
        };
        if let LibDocument::Unknown(unknown) = &doc {
            let k = unknown.get("$k").and_then(Value::as_str).unwrap_or("");
            if Kind::from_name(k).is_none() {
                let message = format!(
                    "a library document of the kind \"{k}\", unknown to this reader, is kept unchanged"
                );
                self.find(FindingKind::NewerData, path, None, message);
            }
            self.opaque(path, hash);
            self.out.lib.push(doc);
            return;
        }
        self.known_lib_document(path, bytes, hash, doc, &malformed, normalised);
    }

    fn known_lib_document(
        &mut self,
        path: &FilePath,
        bytes: &[u8],
        hash: [u8; 32],
        doc: LibDocument,
        malformed: &[MalformedVariant],
        normalised: bool,
    ) {
        let Some((kind, id)) = crate::lib_key(&doc) else {
            self.opaque(path, hash);
            return;
        };
        let identity = (kind.name().to_owned(), id);
        if let Some((first_file, _)) = self.seen.get(&identity).cloned() {
            self.find(
                FindingKind::DuplicateRecord,
                path,
                None,
                format!(
                    "{}:{id} duplicates the document of {first_file}; the first is kept",
                    kind.name()
                ),
            );
            self.opaque(path, hash);
            return;
        }
        self.seen.insert(identity, (path.clone(), None));
        if let Some(expected) = crate::lib_file(&doc).filter(|expected| expected != path) {
            self.find(
                FindingKind::WrongFile,
                path,
                None,
                format!("the document belongs in {expected}; moved at the next save"),
            );
            self.out.rewrite.insert(path.clone());
            self.out.rewrite.insert(expected);
        }
        if !copy_hash_matches(&doc) {
            self.find(
                FindingKind::LibraryHash,
                path,
                None,
                "the library copy's content does not match its sha256",
            );
        }
        if malformed.is_empty() {
            let canonical =
                !normalised && crate::lib_document_text(&doc).is_ok_and(|text| text.as_bytes() == bytes);
            if !canonical {
                self.out.rewrite.insert(path.clone());
                self.find(
                    FindingKind::NonCanonical,
                    path,
                    None,
                    "not in canonical form; rewritten at the next save",
                );
            }
        } else {
            for m in malformed {
                self.find(FindingKind::MalformedVariant, path, None, m.to_string());
            }
            self.out.kept_docs.push(KeptDocument {
                file: path.clone(),
                text: bytes.to_vec(),
                original: Box::new(doc.clone()),
            });
        }
        self.out.lib.push(doc);
    }

    // ---- placement and result ---------------------------------------------------------------------------

    /// EW-CORE-010 for every record whose file is not the one it was read from.
    fn place(&mut self) {
        let anchors = RecordAnchors::new(&self.out.records);
        let mut moves: Vec<(FilePath, u32, FilePath, String)> = Vec::new();
        for placed in &self.placed {
            let (expected, what) = match placed.target {
                Target::Project => (Ok(FilePath::project()), "project".to_owned()),
                Target::Record(i) => {
                    let record = &self.out.records[i];
                    let what = format!(
                        "{}:{}",
                        record.kind_name().unwrap_or("?"),
                        record
                            .id()
                            .map(|u| u.hyphenated().to_string())
                            .unwrap_or_default()
                    );
                    (crate::record_file(record, &anchors), what)
                }
            };
            match expected {
                Ok(expected) if expected != placed.file => {
                    moves.push((placed.file.clone(), placed.line, expected, what));
                }
                _ => {}
            }
        }
        for (file, line, expected, what) in moves {
            self.find(
                FindingKind::WrongFile,
                &file,
                Some(line),
                format!("{what} belongs in {expected}; moved at the next save"),
            );
            self.out.rewrite.insert(file);
            self.out.rewrite.insert(expected);
        }
    }

    fn finish(mut self) -> Loaded {
        self.out.read_only = self.findings.forces_read_only();
        self.out.findings = self.findings.into_vec();
        self.out
    }
}

/// EW-CORE-014: the `sha256` of a library copy is the hash of its item's compact JSON.
fn copy_hash_matches(doc: &LibDocument) -> bool {
    fn check<T: serde::Serialize>(item: &T, claimed: &ew_schema::Sha256) -> bool {
        serde_json::to_string(item).is_ok_and(|text| sha256(text.as_bytes()) == claimed.0)
    }
    match doc {
        LibDocument::Part(c) => check(&c.item, &c.sha256),
        LibDocument::Symbol(c) => check(&c.item, &c.sha256),
        LibDocument::Fndef(c) => check(&c.item, &c.sha256),
        LibDocument::Form(c) => check(&c.item, &c.sha256),
        LibDocument::Unknown(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_scan_counts_brackets_outside_strings_only() {
        assert!(!exceeds_depth(b"[[1]]", 2));
        assert!(exceeds_depth(b"[[[1]]]", 2));
        assert!(!exceeds_depth(br#"{"a":"[[[[","b":"\"[[[["}"#, 1));
        // An escaped backslash before the closing quote ends the string.
        assert!(exceeds_depth(br#"["\\",[1]]"#, 1));
        // Closing brackets without openers never underflow.
        assert!(!exceeds_depth(b"]]]]", 1));
    }

    #[test]
    fn crlf_becomes_lf_and_lone_cr_stays() {
        assert_eq!(crlf_to_lf(b"a\r\nb\rc\r\n\r\n"), b"a\nb\rc\n\n");
        assert_eq!(crlf_to_lf(b"\r\r\n"), b"\r\n");
    }
}
