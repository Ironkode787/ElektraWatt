// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The project reader (doc 05 §3, §6): which files to read, and the records, kept lines, findings and
//! rewrite set of a project's files. Pure: the caller does the I/O (`ew-store` through its `Vfs`).

use std::collections::{BTreeMap, BTreeSet};

use ew_schema::{AnyId, Kind, KindStatus, LibDocument, MalformedVariant, Project, Record, SchemaError};
use serde_json::Value;

use crate::finding::{Finding, FindingKind, Findings};
use crate::layout::RecordAnchors;
use crate::limits::Limits;
use crate::line::jsonl_file;
use crate::manifest::{FileHash, Manifest, sha256};
use crate::merge::{merge_record, no_base, record_key, within_limits};
use crate::migrate::{Migration, MigrationError};
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
    /// (EW-CORE-006), a schema violation (EW-CORE-007), a duplicate (EW-CORE-009) or a line above a limit
    /// (EW-CORE-019), the finding's kind in [`KeptLine::kind`]. Always written verbatim, so nothing is lost.
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
    /// The kind of the finding that quarantined a [`Kept::Quarantined`] line (EW-CORE-005, -006, -007, -009 or a
    /// limit); `None` for the other kept lines. In memory only: the line is written back byte for byte whatever its
    /// kind (WP-S1.17, decision log ID-1185).
    pub kind: Option<FindingKind>,
}

impl KeptLine {
    /// The line as text for messages and display (invalid UTF-8 replaced by U+FFFD); never for writing.
    pub fn text_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.text)
    }

    /// Whether the line gave EW-CORE-005 (not one JSON value, or not UTF-8): a broken line, which `ew fmt --repair`
    /// discards ([`Finding::is_broken_line`](crate::Finding::is_broken_line)).
    pub fn is_broken(&self) -> bool {
        self.kind == Some(FindingKind::JsonSyntax)
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
    /// The record files and library documents in which a migration step changed a value ([`load_migrated`]), compared
    /// value by value: rewritten at the next save like [`Loaded::rewrite`], but kept apart from it and given no
    /// `NonCanonical` finding. Empty for [`load`].
    pub migrated: BTreeSet<FilePath>,
    /// The findings, sorted, capped at [`Limits::max_findings`].
    pub findings: Vec<Finding>,
    /// Whether a finding forces a read-only session.
    pub read_only: bool,
    /// Whether a finding other than a broken line (EW-CORE-005 at line level) forces a read-only session, the findings
    /// beyond the cap included: what `ew fmt --repair` refuses (WP-S1.17, decision log ID-1185).
    pub read_only_beyond_repair: bool,
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
    let mut loader = Loader::new(manifest, limits, None);
    for (path, bytes) in files {
        loader.file(path, bytes);
    }
    loader.place();
    loader.finish()
}

/// [`load`] of a project of an older format (doc 05 §10.2, WP-S1.17 D10): every record and library document runs
/// through `migration` ([`Migration::record`]) after its parse and before its typed reading, and is read, placed and
/// checked as migrated. The files in which a step changed a value are in [`Loaded::migrated`]; a line or document kept
/// verbatim there (an unknown kind, a malformed variant, a quarantined record) keeps its migrated value's text, never
/// its old one.
///
/// # Errors
///
/// The first [`MigrationError`], with its file and line: a step that fails, a kind step that is missing, or a library
/// document of a kind this reader keeps unchanged whose value a step changed. Nothing is loaded then.
pub fn load_migrated(
    manifest: Option<&Manifest>,
    files: &BTreeMap<FilePath, Vec<u8>>,
    limits: &Limits,
    migration: &Migration<'_>,
) -> Result<Loaded, MigrationError> {
    let mut loader = Loader::new(manifest, limits, Some(migration));
    for (path, bytes) in files {
        loader.file(path, bytes);
        if let Some(e) = loader.failed.take() {
            return Err(e);
        }
    }
    loader.place();
    Ok(loader.finish())
}

/// The Git conflict-marker prefixes of a JSON Lines file (`|||||||` is the diff3 base marker).
const LINE_MARKERS: [&[u8]; 4] = [b"<<<<<<<", b"=======", b">>>>>>>", b"|||||||"];

/// Where [`unmark`] is in a file.
enum Hunk {
    Outside,
    Ours,
    Base,
    Theirs,
}

/// The lines of a JSON Lines `body` with its conflict markers resolved (WP-S1.17 D18, FR-COL-025; doc 05 §11.4), each
/// with its line number in the file (a hunk's lines take the number of its `<<<<<<<` line); `None` when the body
/// holds no marker, or when a hunk is not whole (a marker out of place, an open hunk) or a side holds a line that is
/// not a record with a `$k` and an `id`, a key twice, or more lines than the limit: the file then stays EW-CORE-006.
///
/// Per hunk, by `($k, id)`: with a diff3 base (`|||||||`), [`merge_record`] merges the three sides (clean parts merge,
/// true conflicts become conflict records); without one, D18's no-base rule ([`no_base`]): records equal on both sides
/// are kept, records that differ keep ours with a conflict of every differing member, and a record on one side only is
/// kept with a conflict whose other side is `"deleted"`. A merged line that would pass a line limit leaves the file
/// EW-CORE-006 as well.
fn unmark(body: &[u8], limits: &Limits) -> Option<Vec<(usize, Vec<u8>)>> {
    let lines: Vec<&[u8]> = body.split(|b| *b == b'\n').collect();
    if lines.len() > limits.max_lines_per_file
        || !lines
            .iter()
            .any(|l| LINE_MARKERS.iter().any(|m| l.starts_with(m)))
    {
        return None;
    }
    let mut out = Vec::new();
    let (mut state, mut start) = (Hunk::Outside, 0);
    let (mut ours, mut base, mut theirs): (Vec<&[u8]>, Option<Vec<&[u8]>>, Vec<&[u8]>) =
        (Vec::new(), None, Vec::new());
    for (i, raw) in lines.into_iter().enumerate() {
        let n = i + 1;
        let line = raw.strip_suffix(b"\r").unwrap_or(raw);
        let marker = LINE_MARKERS.iter().position(|m| line.starts_with(m));
        state = match (state, marker) {
            (Hunk::Outside, None) => {
                out.push((n, raw.to_vec()));
                Hunk::Outside
            }
            (Hunk::Outside, Some(0)) => {
                start = n;
                (ours, base, theirs) = (Vec::new(), None, Vec::new());
                Hunk::Ours
            }
            (Hunk::Ours, Some(3)) => {
                base = Some(Vec::new());
                Hunk::Base
            }
            (Hunk::Ours | Hunk::Base, Some(1)) => Hunk::Theirs,
            (Hunk::Theirs, Some(2)) => {
                for value in merge_hunk(base.as_deref(), &ours, &theirs, limits)? {
                    out.push((start, value.into_bytes()));
                }
                Hunk::Outside
            }
            (Hunk::Ours, None) => {
                ours.push(line);
                Hunk::Ours
            }
            (Hunk::Base, None) => {
                base.get_or_insert_with(Vec::new).push(line);
                Hunk::Base
            }
            (Hunk::Theirs, None) => {
                theirs.push(line);
                Hunk::Theirs
            }
            _ => return None,
        };
    }
    matches!(state, Hunk::Outside).then_some(out)
}

/// The records of one hunk side by `($k, id)`, read as the loader reads a line and NFC-normalised; blank lines are
/// skipped. `None` when a line is not a record with a key, or a key is there twice.
fn hunk_side(lines: &[&[u8]], limits: &Limits) -> Option<BTreeMap<(String, String), Value>> {
    let mut out = BTreeMap::new();
    for line in lines.iter().filter(|l| !l.is_empty()) {
        let (_, mut value) = parse_line(line, limits).ok()?;
        ew_schema::normalize_nfc(&mut value);
        if out.insert(record_key(&value)?, value).is_some() {
            return None;
        }
    }
    Some(out)
}

/// One hunk's merged records and conflict records as compact JSON lines ([`unmark`]).
fn merge_hunk(
    base: Option<&[&[u8]]>,
    ours: &[&[u8]],
    theirs: &[&[u8]],
    limits: &Limits,
) -> Option<Vec<String>> {
    let o = match base {
        Some(lines) => Some(hunk_side(lines, limits)?),
        None => None,
    };
    let (a, b) = (hunk_side(ours, limits)?, hunk_side(theirs, limits)?);
    let keys: BTreeSet<&(String, String)> = o
        .iter()
        .flat_map(BTreeMap::keys)
        .chain(a.keys())
        .chain(b.keys())
        .collect();
    let mut out = Vec::new();
    for key in keys {
        let merged = match &o {
            Some(o) => merge_record(o.get(key), a.get(key), b.get(key)),
            None => no_base(a.get(key), b.get(key)),
        };
        for value in merged.merged.iter().chain(merged.conflict.iter()) {
            let line = value.to_string();
            if !within_limits(&line, limits) {
                return None;
            }
            out.push(line);
        }
    }
    Some(out)
}

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
    /// Whether a migration step changed a value of the file.
    migrated: bool,
}

struct Loader<'a> {
    manifest: Option<&'a Manifest>,
    limits: &'a Limits,
    findings: Findings,
    out: Loaded,
    seen: BTreeMap<(String, AnyId), Origin>,
    project_at: Option<Origin>,
    placed: Vec<Placed>,
    /// The migration of [`load_migrated`].
    migration: Option<&'a Migration<'a>>,
    /// Its first error: nothing more is read.
    failed: Option<MigrationError>,
    /// The record-level conflict records read, held until every file is read: one whose target is not a record of
    /// the project is kept verbatim (WP-S1.17 D17).
    conflicts: Vec<HeldConflict>,
}

/// A record-level conflict record waiting for its target ([`Loader::conflicts`]).
struct HeldConflict {
    file: FilePath,
    line: u32,
    key: LineKey,
    text: Vec<u8>,
    target: (String, AnyId),
    record: Record,
}

/// `bytes` with every `\r\n` replaced by `\n`, a lone `\r` kept: the line-end normalisation under which a file equals
/// its manifest hash (doc 05 §6.2, [`FindingKind::LineEndings`](crate::FindingKind)), and under which a save finds a
/// file it replaces or removes unchanged.
///
/// ```
/// assert_eq!(ew_format::crlf_to_lf(b"a\r\nb\rc\r\n"), b"a\nb\rc\n");
/// ```
pub fn crlf_to_lf(bytes: &[u8]) -> Vec<u8> {
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
pub(crate) fn exceeds_depth(text: &[u8], max: usize) -> bool {
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

/// Why a line or a library document was not read as a JSON value: the kind of its finding and the finding's text.
pub(crate) struct Fault {
    pub(crate) kind: FindingKind,
    pub(crate) message: String,
}

/// One line of a JSON Lines file (without its line end and CR) as the loader reads it (doc 05 §3.6), in this order: the
/// line-length limit, a conflict marker (EW-CORE-006), UTF-8, the nesting depth, and JSON syntax with duplicate members
/// rejected. The merge core reads its three inputs with it (decision log ID-1268 ruling 5).
pub(crate) fn parse_line<'b>(bytes: &'b [u8], limits: &Limits) -> Result<(&'b str, Value), Fault> {
    if bytes.len() > limits.max_line_bytes {
        return Err(Fault {
            kind: FindingKind::LimitExceeded,
            message: format!(
                "line of {} bytes, more than the limit of {}",
                bytes.len(),
                limits.max_line_bytes
            ),
        });
    }
    if LINE_MARKERS.iter().any(|m| bytes.starts_with(m)) {
        return Err(Fault {
            kind: FindingKind::ConflictMarkers,
            message: "a git conflict marker line".to_owned(),
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|e| Fault {
        kind: FindingKind::JsonSyntax,
        message: format!("the line is not UTF-8: {e}"),
    })?;
    if exceeds_depth(bytes, limits.max_depth) {
        return Err(Fault {
            kind: FindingKind::LimitExceeded,
            message: format!("nesting deeper than {}", limits.max_depth),
        });
    }
    match ew_schema::parse_value(text) {
        Ok(value) => Ok((text, value)),
        Err(e) => {
            let (kind, message) = parse_failure(e);
            Err(Fault { kind, message })
        }
    }
}

/// A library document's bytes as the loader reads them (doc 05 §3.6): a leading BOM skipped, then UTF-8, the nesting
/// depth, and JSON syntax with duplicate members rejected. The merge core reads its three inputs with it (decision log
/// ID-1268 ruling 5).
pub(crate) fn parse_document(bytes: &[u8], limits: &Limits) -> Result<Value, Fault> {
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = std::str::from_utf8(body).map_err(|e| Fault {
        kind: FindingKind::JsonSyntax,
        message: format!("the document is not UTF-8: {e}"),
    })?;
    if exceeds_depth(bytes, limits.max_depth) {
        return Err(Fault {
            kind: FindingKind::LimitExceeded,
            message: format!("nesting deeper than {}", limits.max_depth),
        });
    }
    ew_schema::parse_value(text).map_err(|e| {
        let (kind, message) = parse_failure(e);
        Fault { kind, message }
    })
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

impl<'a> Loader<'a> {
    fn new(
        manifest: Option<&'a Manifest>,
        limits: &'a Limits,
        migration: Option<&'a Migration<'a>>,
    ) -> Loader<'a> {
        Loader {
            manifest,
            limits,
            findings: Findings::new(limits.max_findings),
            out: Loaded::default(),
            seen: BTreeMap::new(),
            project_at: None,
            placed: Vec::new(),
            migration,
            failed: None,
            conflicts: Vec::new(),
        }
    }

    /// `value` through the migration of [`load_migrated`], and whether a step changed it (compared value by value);
    /// unchanged without one. A step's error is kept with its file and line (`failed`) and gives `None`.
    fn migrate(&mut self, path: &FilePath, line: Option<u32>, value: Value) -> Option<(Value, bool)> {
        let Some(migration) = self.migration else {
            return Some((value, false));
        };
        let before = value.clone();
        match migration.record(value) {
            Ok(value) => {
                let changed = value != before;
                Some((value, changed))
            }
            Err(e) => {
                self.failed.get_or_insert(e.in_file(path, line));
                None
            }
        }
    }

    fn find(&mut self, kind: FindingKind, file: &FilePath, line: Option<u32>, message: impl Into<String>) {
        self.findings
            .push(Finding::new(kind, Some(file.clone()), line, message));
    }

    /// A library document a migration step changed that would end opaque refuses the open (ID-1221 ruling 2, ID-1230
    /// ruling 2): kept by its hash only, it would never be written, and the step's change would be lost.
    fn refuse_opaque(&mut self, path: &FilePath, message: String) {
        self.failed.get_or_insert(MigrationError {
            file: Some(path.clone()),
            line: None,
            pointer: String::new(),
            message,
        });
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
            migrated: false,
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
            // Conflict markers whose hunks hold whole records load as conflict records (D18, FR-COL-025); the file is
            // rewritten at the next save. Any other marker stays EW-CORE-006 below.
            if let Some(lines) = unmark(body, self.limits) {
                st.non_canonical = true;
                for (n, raw) in lines {
                    self.line(path, n, &raw, &mut st);
                    if self.failed.is_some() {
                        return;
                    }
                }
                self.finish_record_file(path, bytes, st);
                return;
            }
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
                if self.failed.is_some() {
                    return;
                }
            }
        }
        self.finish_record_file(path, bytes, st);
    }

    /// The canonical check of a record file read whole.
    fn finish_record_file(&mut self, path: &FilePath, bytes: &[u8], st: FileState) {
        // A migrated file is rewritten from its migrated values: no `NonCanonical` finding (ID-1221 ruling 4).
        if st.migrated {
            self.out.migrated.insert(path.clone());
        } else if st.non_canonical || jsonl_file(st.lines) != bytes {
            self.out.rewrite.insert(path.clone());
            self.find(
                FindingKind::NonCanonical,
                path,
                None,
                "not in canonical form; rewritten at the next save",
            );
        }
    }

    /// Keeps a line verbatim (its bytes), without a record, with the kind of the finding that quarantined it.
    fn quarantine(
        &mut self,
        path: &FilePath,
        line: u32,
        kind: FindingKind,
        key: LineKey,
        text: Vec<u8>,
        st: &mut FileState,
    ) {
        st.lines.push((key.clone(), text.clone()));
        self.out.kept.push(KeptLine {
            file: path.clone(),
            line,
            key,
            text,
            kept: Kept::Quarantined,
            kind: Some(kind),
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
        match parse_line(bytes, self.limits) {
            Ok((text, value)) => self.record(path, n, text, value, st),
            Err(fault) => {
                let keyless = LineKey::keyless().with_seq(n as u64);
                self.find(fault.kind, path, Some(ln), fault.message);
                self.quarantine(path, ln, fault.kind, keyless, bytes.to_vec(), st);
            }
        }
    }

    fn record(&mut self, path: &FilePath, n: usize, text: &str, value: Value, st: &mut FileState) {
        let ln = line_number(n);
        let Some((mut value, migrated)) = self.migrate(path, Some(ln), value) else {
            return;
        };
        st.migrated |= migrated;
        let base_key = LineKey::of_value(&value);
        let quarantine_key = base_key.clone().with_seq(n as u64);
        if ew_schema::normalize_nfc(&mut value) {
            st.non_canonical = true;
        }
        // A line kept verbatim keeps its migrated value's text (ID-1221 ruling 2), never its old one.
        let migrated_text;
        let text = if migrated {
            migrated_text = value.to_string();
            migrated_text.as_str()
        } else {
            text
        };
        // A waiver that fails its typing (no `reason`, a timestamp in `date`) waives nothing: the finding names it so
        // the dead waiver is seen (doc 15 §2.5, EW-CORE-081 cannot report a waiver it never reads).
        let waiver = value.get("$k").and_then(Value::as_str) == Some("waiver");
        let (record, malformed) = match Record::load(value) {
            Ok(loaded) => loaded,
            Err(e) => {
                let message = if waiver {
                    format!("unreadable waiver, kept verbatim; the finding it names is not waived: {e}")
                } else {
                    e.to_string()
                };
                self.find(FindingKind::SchemaViolation, path, Some(ln), message);
                self.quarantine(
                    path,
                    ln,
                    FindingKind::SchemaViolation,
                    quarantine_key,
                    text.as_bytes().to_vec(),
                    st,
                );
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
            self.quarantine(
                path,
                ln,
                FindingKind::SchemaViolation,
                quarantine_key,
                text.as_bytes().to_vec(),
                st,
            );
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
            self.quarantine(
                path,
                ln,
                FindingKind::DuplicateRecord,
                quarantine_key,
                text.as_bytes().to_vec(),
                st,
            );
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
                    kind: None,
                });
            }
            None => match (LineKey::of(&record), crate::record_line(&record)) {
                (Some(key), Ok(line)) => st.lines.push((key, line.into_bytes())),
                _ => st.non_canonical = true,
            },
        }
        let known = record.kind().is_some();
        if let Record::Conflict(conflict) = &record {
            if let Some(kind) = conflict.target_kind() {
                let target = (kind.to_owned(), id);
                self.conflicts.push(HeldConflict {
                    file: path.clone(),
                    line: ln,
                    key: quarantine_key,
                    text: text.as_bytes().to_vec(),
                    target,
                    record,
                });
                return;
            }
        }
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
        // A leading UTF-8 BOM (an editor's, doc 05 §3.1: the writer never writes one) is a shape, as in a JSON
        // Lines file: the document is read without it and, being unlike its canonical text, rewritten at the
        // next save. JSON has no BOM, so without this the document would be a syntax error, opaque, and every
        // record naming it a dangling reference that refuses the whole project (fuzz finding, cycle 22).
        let value = match parse_document(bytes, self.limits) {
            Ok(value) => value,
            Err(fault) => {
                self.find(fault.kind, path, None, fault.message);
                self.opaque(path, hash);
                return;
            }
        };
        let before = self.migration.is_some().then(|| value.clone());
        let Some((mut value, migrated)) = self.migrate(path, None, value) else {
            return;
        };
        // A step that changed a copy's item is not an edit: its hash follows the migrated item (ID-1230 ruling 3).
        if let Some(before) = &before {
            rehash_migrated_copy(before, &mut value);
        }
        let normalised = ew_schema::normalize_nfc(&mut value);
        // A document kept verbatim keeps its migrated value's text (ID-1221 ruling 2), never its old one.
        let migrated_text = migrated.then(|| crate::pretty(&value.to_string()).into_bytes());
        let (doc, malformed) = match LibDocument::load(value) {
            Ok(loaded) => loaded,
            Err(e) => {
                if migrated {
                    self.refuse_opaque(
                        path,
                        format!("a migration step gave a library document this version cannot read: {e}"),
                    );
                    return;
                }
                self.find(FindingKind::SchemaViolation, path, None, e.to_string());
                self.opaque(path, hash);
                return;
            }
        };
        if let LibDocument::Unknown(unknown) = &doc {
            if migrated {
                self.refuse_opaque(
                    path,
                    "a migration step changed a library document of a kind this version keeps unchanged"
                        .to_owned(),
                );
                return;
            }
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
        self.known_lib_document(path, bytes, hash, doc, &malformed, normalised, migrated_text);
    }

    #[allow(clippy::too_many_arguments)]
    fn known_lib_document(
        &mut self,
        path: &FilePath,
        bytes: &[u8],
        hash: [u8; 32],
        doc: LibDocument,
        malformed: &[MalformedVariant],
        normalised: bool,
        migrated_text: Option<Vec<u8>>,
    ) {
        let Some((kind, id)) = crate::lib_key(&doc) else {
            self.opaque(path, hash);
            return;
        };
        let identity = (kind.name().to_owned(), id);
        if let Some((first_file, _)) = self.seen.get(&identity).cloned() {
            if migrated_text.is_some() {
                self.refuse_opaque(
                    path,
                    format!(
                        "once migrated, {}:{id} duplicates the document of {first_file}; remove one of the two files \
                         and open the project again",
                        kind.name()
                    ),
                );
                return;
            }
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
        if migrated_text.is_some() {
            self.out.migrated.insert(path.clone());
        }
        if malformed.is_empty() {
            let canonical =
                !normalised && crate::lib_document_text(&doc).is_ok_and(|text| text.as_bytes() == bytes);
            if migrated_text.is_none() && !canonical {
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
                text: migrated_text.unwrap_or_else(|| bytes.to_vec()),
                original: Box::new(doc.clone()),
            });
        }
        self.out.lib.push(doc);
    }

    // ---- placement and result ---------------------------------------------------------------------------

    /// The held record-level conflicts (WP-S1.17 D17): one whose target is a record of an implemented kind joins the
    /// records; one whose target the project does not hold, or holds only as a record of a kind this reader keeps
    /// verbatim, is kept verbatim and reported (EW-CORE-007, the line quarantined).
    fn hold_conflicts(&mut self) {
        for held in std::mem::take(&mut self.conflicts) {
            let kind = &held.target.0;
            let typed = Kind::from_name(kind)
                .filter(|k| k.status() == KindStatus::Implemented && !k.is_library() && *k != Kind::Conflict);
            if typed.is_some() && self.seen.contains_key(&held.target) {
                self.placed.push(Placed {
                    file: held.file,
                    line: held.line,
                    target: Target::Record(self.out.records.len()),
                });
                self.out.records.push(held.record);
                continue;
            }
            let what = if typed.is_some() {
                format!(
                    "a conflict record whose target, a record of the kind \"{kind}\", is not in the project"
                )
            } else {
                format!(
                    "a conflict record whose target is of the kind \"{kind}\", which this reader keeps unchanged"
                )
            };
            self.find(
                FindingKind::SchemaViolation,
                &held.file,
                Some(held.line),
                format!("{what}; kept verbatim"),
            );
            self.out.kept.push(KeptLine {
                file: held.file,
                line: held.line,
                key: held.key,
                text: held.text,
                kept: Kept::Quarantined,
                kind: Some(FindingKind::SchemaViolation),
            });
        }
        self.out
            .kept
            .sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    }

    /// EW-CORE-010 for every record whose file is not the one it was read from.
    fn place(&mut self) {
        self.hold_conflicts();
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
        self.out.read_only_beyond_repair = self.findings.forces_read_only_beyond_repair();
        self.out.findings = self.findings.into_vec();
        self.out
    }
}

/// The SHA-256 of a library copy's item: the hash of the compact JSON of the item's own members (doc 05
/// §13.2, EW-CORE-014); `None` for a document of an unknown kind. `ew-commands`' `library.embed` computes
/// the same value (`ew_commands::commands::library_copy_hash`); an agreement test in `ew-api` keeps the two
/// equal.
pub fn library_copy_hash(doc: &LibDocument) -> Option<[u8; 32]> {
    fn hash<T: serde::Serialize>(item: &T) -> Option<[u8; 32]> {
        serde_json::to_string(item)
            .ok()
            .map(|text| sha256(text.as_bytes()))
    }
    match doc {
        LibDocument::Part(c) => hash(&c.item),
        LibDocument::Symbol(c) => hash(&c.item),
        LibDocument::Fndef(c) => hash(&c.item),
        LibDocument::Form(c) => hash(&c.item),
        LibDocument::Macro(c) => hash(&c.item),
        LibDocument::Manufacturer(c) => hash(&c.item),
        LibDocument::Unknown(_) => None,
    }
}

/// A library copy's `sha256` after its migration (doc 05 §10.2, WP-S1.17 D10; ID-1230 ruling 3, ID-1250): a step
/// that changed the copy's item is not an edit, so when the item of `migrated` (its members without the copy envelope)
/// differs from that of `before`, the same document before the chain, `migrated`'s `sha256` becomes
/// [`library_copy_hash`] of its item after NFC normalisation, as the loader reads it. Else, or when `migrated` does not
/// read as a library document of a known kind, `migrated` stays as it is. The loader ([`load_migrated`]) and a pasted
/// fragment's library copies (`ew-api`'s `parse_fragment_with`) both call it, so a copy migrated in either reads with
/// the same hash.
pub fn rehash_migrated_copy(before: &Value, migrated: &mut Value) {
    if copy_item(before) == copy_item(migrated) {
        return;
    }
    let mut normalised = migrated.clone();
    ew_schema::normalize_nfc(&mut normalised);
    let hash = LibDocument::load(normalised)
        .ok()
        .and_then(|(doc, _)| library_copy_hash(&doc));
    if let (Some(hash), Some(members)) = (hash, migrated.as_object_mut()) {
        members.insert(
            "sha256".to_owned(),
            Value::from(ew_schema::Sha256(hash).to_string()),
        );
    }
}

/// A library copy's own item members: the document's members without the copy envelope of
/// [`LibCopy`](ew_schema::LibCopy) (`$v`, `id`, `library_id`, `item_id`, `version`, `sha256`).
fn copy_item(value: &Value) -> Option<serde_json::Map<String, Value>> {
    const ENVELOPE: [&str; 6] = ["$v", "id", "library_id", "item_id", "version", "sha256"];
    value.as_object().map(|members| {
        members
            .iter()
            .filter(|(k, _)| !ENVELOPE.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}

/// EW-CORE-014: the `sha256` of a library copy is the hash of its item's compact JSON.
fn copy_hash_matches(doc: &LibDocument) -> bool {
    let claimed = match doc {
        LibDocument::Part(c) => &c.sha256,
        LibDocument::Symbol(c) => &c.sha256,
        LibDocument::Fndef(c) => &c.sha256,
        LibDocument::Form(c) => &c.sha256,
        LibDocument::Macro(c) => &c.sha256,
        LibDocument::Manufacturer(c) => &c.sha256,
        LibDocument::Unknown(_) => return true,
    };
    library_copy_hash(doc) == Some(claimed.0)
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
