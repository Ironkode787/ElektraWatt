// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew fmt` (WP-S1.22 D9, FR-EXT-008): the project files in their canonical encoding and line order (doc 05 §3).
//!
//! - **The rewrite set** ([`plan`]) is what the loader found not in canonical form or holding a record of another
//!   file (`ew_store::Store::canonical_rewrite`): the save plan with no changed record, never a full rewrite, so a file
//!   already canonical is not written (ruling 6 of cycle 41).
//! - **Kept lines** ([`kept_lines`]): quarantined lines, lines of unknown kinds and untouched malformed values stay in
//!   the files verbatim, as in every save (ADR-0011); the open's findings name them, so they are reported.
//! - **The content check** ([`verify_content`]) reads each file of the set back and compares the content of its lines
//!   with the content of the canonical bytes, over the whole set (a record may move to another file): only form may
//!   change, never content, else nothing is written.
//! - **The write** ([`format()`]) is a save of the session's store: the save protocol's temps, intent and renames, so a
//!   crash between files leaves either the old or the new generation, never a torn file (ruling 1 of cycle 41).
//!
//! # Form and content
//!
//! Form, which `ew fmt` may change: the order of the lines and of an object's members, white space, string escapes,
//! a byte order mark, CR line ends, empty lines, a number's spelling (`1.0` and `1`, `1e2` and `100`) and the Unicode
//! normalisation of a string (NFC, doc 05 §3.1). Content, which it may never change: anything else, such as a member
//! dropped or added (`"x":null` and an absent `x` are different content here, so a file holding one is refused rather
//! than rewritten), a value, or the order of an array's items. A line that is not one JSON value compares by its bytes
//! (CR line end aside).

use std::collections::BTreeMap;
use std::fmt;
use std::io;

use ew_store::ew_format::{FilePath, FindingKind, PathClass, classify};
use serde_json::Value;

use crate::{OpenReport, SaveError, Session};

/// A file of the rewrite set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Rewrite {
    /// The file, relative to the project directory (`devices/3f.jsonl`).
    pub file: String,
    /// `true` when nothing is left in it and the save removes it (its records belong in other files).
    pub removed: bool,
}

/// A line (or a file) the canonical form keeps verbatim, as the open reported it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeptLine {
    /// The file, relative to the project directory.
    pub file: String,
    /// The 1-based line; `None` for a whole file (a library document, a file above a limit).
    pub line: Option<u32>,
    /// The finding's rule (`EW-CORE-007`).
    pub code: String,
    /// Why it is kept, as the open's finding says it.
    pub reason: String,
}

/// What `ew fmt` writes: the rewrite set with the canonical bytes of each file.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Plan {
    files: BTreeMap<FilePath, Option<Vec<u8>>>,
}

impl Plan {
    /// The files of the set in path order.
    pub fn files(&self) -> Vec<Rewrite> {
        self.files
            .iter()
            .map(|(path, bytes)| Rewrite {
                file: path.as_str().to_owned(),
                removed: bytes.is_none(),
            })
            .collect()
    }

    /// Whether every file is already canonical.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// What [`format()`] wrote.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Formatted {
    /// The generation on disk afterwards (unchanged when nothing was written).
    pub generation: u64,
    /// The files rewritten or removed, in path order.
    pub files: Vec<Rewrite>,
}

/// Why `ew fmt` writes nothing.
#[derive(Debug)]
pub enum FmtError {
    /// The session has no project directory.
    NoProject,
    /// The session has unsaved changes: a format writes only the generation on disk.
    Dirty,
    /// A record has no file or no JSON form.
    Render(SaveError),
    /// A file of the set cannot be read back for the content check.
    Read {
        /// The file, relative to the project directory.
        file: String,
        /// The error.
        error: io::Error,
    },
    /// The canonical form changes content, not only form.
    Content {
        /// The file, relative to the project directory.
        file: String,
        /// The 1-based line: of the file on disk, or of its canonical bytes when `added`.
        line: u32,
        /// `true`: the canonical bytes hold this line and the files on disk do not; `false`: the line on disk is not
        /// in the canonical bytes.
        added: bool,
    },
    /// The save failed.
    Save(SaveError),
}

impl fmt::Display for FmtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FmtError::NoProject => f.write_str("the session has no project directory"),
            FmtError::Dirty => {
                f.write_str("the session has unsaved changes; a format writes only the saved project")
            }
            FmtError::Render(e) => write!(f, "the canonical form cannot be written: {e}"),
            FmtError::Read { file, error } => write!(f, "{file} cannot be read back: {error}"),
            FmtError::Content {
                file,
                line,
                added: false,
            } => write!(
                f,
                "{file} line {line}: the canonical form would change the content of this line, not only its form"
            ),
            FmtError::Content {
                file,
                line,
                added: true,
            } => write!(
                f,
                "{file}: the canonical form would hold content that no file on disk holds (its line {line})"
            ),
            FmtError::Save(e) => write!(f, "the save failed: {e}"),
        }
    }
}

impl std::error::Error for FmtError {}

/// The rewrite set of the session's project with the canonical bytes of each file; nothing is written.
///
/// # Errors
///
/// [`FmtError::NoProject`] without a project directory; [`FmtError::Render`] for a record without a file or a JSON
/// form.
pub fn plan(session: &Session) -> Result<Plan, FmtError> {
    let store = session.store().ok_or(FmtError::NoProject)?;
    let files = store.canonical_rewrite().map_err(FmtError::Render)?;
    Ok(Plan { files })
}

/// The lines and files the open kept verbatim, which a format keeps too: quarantined lines (EW-CORE-005, -006, -007,
/// -009, a limit), malformed values of known variants (EW-CORE-017) and unknown kinds, members or files of a newer
/// format (EW-CORE-016), in the report's order.
pub fn kept_lines(report: &OpenReport) -> Vec<KeptLine> {
    report
        .findings
        .iter()
        .filter(|f| {
            matches!(
                f.kind,
                FindingKind::JsonSyntax
                    | FindingKind::ConflictMarkers
                    | FindingKind::SchemaViolation
                    | FindingKind::DuplicateRecord
                    | FindingKind::LimitExceeded
                    | FindingKind::MalformedVariant
                    | FindingKind::NewerData
            )
        })
        .filter_map(|f| {
            Some(KeptLine {
                file: f.file.as_ref()?.as_str().to_owned(),
                line: f.line,
                code: f.kind.code().unwrap_or_default().to_owned(),
                reason: f.message.clone(),
            })
        })
        .collect()
}

/// Checks that `plan` changes only form (the module documentation): every line of the files of the set on disk is in
/// their canonical bytes as often as on disk, and the canonical bytes hold nothing else.
///
/// # Errors
///
/// [`FmtError::Content`] naming the first line whose content would change; [`FmtError::Read`] when a file cannot be
/// read back.
pub fn verify_content(session: &Session, plan: &Plan) -> Result<(), FmtError> {
    let store = session.store().ok_or(FmtError::NoProject)?;
    let mut disk: BTreeMap<Content, Vec<(String, u32)>> = BTreeMap::new();
    let mut canonical: BTreeMap<Content, Vec<(String, u32)>> = BTreeMap::new();
    for (path, bytes) in &plan.files {
        let full = ew_store::join(store.root(), path.as_str());
        let old = match store.vfs().read(&full) {
            Ok(bytes) => bytes,
            // A file the set creates (the target of a moved record).
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                return Err(FmtError::Read {
                    file: path.as_str().to_owned(),
                    error,
                });
            }
        };
        for (line, content) in contents(path, &old) {
            disk.entry(content)
                .or_default()
                .push((path.as_str().to_owned(), line));
        }
        for (line, content) in contents(path, bytes.as_deref().unwrap_or_default()) {
            canonical
                .entry(content)
                .or_default()
                .push((path.as_str().to_owned(), line));
        }
    }
    for (content, places) in &disk {
        let kept = canonical.remove(content).unwrap_or_default();
        if kept.len() < places.len() {
            let (file, line) = places[kept.len()].clone();
            return Err(FmtError::Content {
                file,
                line,
                added: false,
            });
        }
        if kept.len() > places.len() {
            let (file, line) = kept[places.len()].clone();
            return Err(FmtError::Content {
                file,
                line,
                added: true,
            });
        }
    }
    if let Some((file, line)) = canonical.into_values().flatten().min() {
        return Err(FmtError::Content {
            file,
            line,
            added: true,
        });
    }
    Ok(())
}

/// Rewrites the rewrite set of the session's project in canonical form through the save protocol, after the content
/// check; a project already canonical writes nothing. The session must be a fresh editing open (the CLI refuses a
/// read-only one and one with a crashed session's journal before it gets here).
///
/// # Errors
///
/// [`FmtError`]: the content check refuses, a file cannot be read back, or the save fails (with
/// [`SaveError::Incomplete`] of a new generation the generation is decided and completes by roll-forward).
pub fn format(session: &mut Session) -> Result<Formatted, FmtError> {
    if session.store().is_none() {
        return Err(FmtError::NoProject);
    }
    if session.is_dirty() {
        return Err(FmtError::Dirty);
    }
    let plan = plan(session)?;
    verify_content(session, &plan)?;
    let files = plan.files();
    if plan.is_empty() {
        let generation = session.store().map_or(0, ew_store::Store::generation);
        return Ok(Formatted { generation, files });
    }
    let saved = session.save().map_err(FmtError::Save)?;
    Ok(Formatted {
        generation: saved.generation,
        files,
    })
}

/// The content of one line (or of a whole library document) for the comparison of [`verify_content`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Content {
    /// One JSON value in a canonical spelling: members in key order, numbers by value, strings in NFC.
    Value(String),
    /// Bytes that are not one JSON value.
    Raw(Vec<u8>),
}

/// The contents of `bytes` as the file `path`: one per non-empty line of a record file (a byte order mark and CR line
/// ends aside), one for a non-empty library document. Each with its 1-based line.
fn contents(path: &FilePath, bytes: &[u8]) -> Vec<(u32, Content)> {
    let data = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if matches!(classify(path.as_str()), PathClass::LibDocument { .. }) {
        return if data.is_empty() {
            Vec::new()
        } else {
            vec![(1, content_of(data))]
        };
    }
    data.split(|b| *b == b'\n')
        .enumerate()
        .filter_map(|(i, line)| {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                return None;
            }
            Some((u32::try_from(i + 1).unwrap_or(u32::MAX), content_of(line)))
        })
        .collect()
}

fn content_of(bytes: &[u8]) -> Content {
    let value = std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| ew_schema::parse_value(text).ok());
    match value {
        Some(mut value) => {
            ew_schema::normalize_nfc(&mut value);
            let mut out = String::new();
            spell(&value, &mut out);
            Content::Value(out)
        }
        None => Content::Raw(bytes.to_vec()),
    }
}

/// The largest integer a binary64 float holds exactly with all smaller ones (2^53).
const EXACT_FLOAT_INTEGERS: f64 = 9_007_199_254_740_992.0;

/// Writes `value` in the canonical spelling of [`Content::Value`].
fn spell(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.push_str(&i.to_string());
            } else if let Some(u) = n.as_u64() {
                out.push_str(&u.to_string());
            } else {
                let f = n.as_f64().unwrap_or(f64::NAN);
                if f.fract() == 0.0 && f.abs() < EXACT_FLOAT_INTEGERS {
                    // A whole number spelled as a float (`1.0`, `1e2`): the same value as the integer.
                    out.push_str(&(f as i64).to_string());
                } else {
                    out.push_str(&f.to_string());
                }
            }
        }
        Value::String(s) => out.push_str(&Value::String(s.clone()).to_string()),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                spell(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut members: Vec<(&String, &Value)> = map.iter().collect();
            members.sort_by(|a, b| a.0.cmp(b.0));
            out.push('{');
            for (i, (key, item)) in members.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                spell(item, out);
            }
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(path: &str, bytes: &str) -> Vec<(u32, Content)> {
        contents(&FilePath::new(path.to_owned()).unwrap(), bytes.as_bytes())
    }

    #[test]
    fn form_compares_equal_and_content_does_not() {
        let a = records("devices/3f.jsonl", "{\"b\":1,\"a\":\"\u{e9}\",\"c\":[1,2]}\n");
        // Member order, white space, a CR line end, an escape, NFD and a float spelling of 1 are form.
        let b = records(
            "devices/3f.jsonl",
            "\u{feff}\n{ \"a\" : \"e\u{301}\", \"c\": [1, 2], \"b\": 1.0 }\r\n\n",
        );
        assert_eq!(a[0].1, b[0].1);
        assert_eq!(b[0].0, 2, "the line numbers of the file on disk");
        // An array's order, a dropped member and a changed value are content.
        for other in [
            "{\"b\":1,\"a\":\"\u{e9}\",\"c\":[2,1]}",
            "{\"a\":\"\u{e9}\",\"c\":[1,2]}",
            "{\"b\":2,\"a\":\"\u{e9}\",\"c\":[1,2]}",
            "{\"b\":1,\"a\":\"\u{e9}\",\"c\":[1,2],\"d\":null}",
        ] {
            assert_ne!(a[0].1, records("devices/3f.jsonl", other)[0].1, "{other}");
        }
        // A line that is not one JSON value compares by its bytes.
        assert_eq!(
            records("devices/3f.jsonl", "<<<<<<< ours\n")[0].1,
            Content::Raw(b"<<<<<<< ours".to_vec())
        );
        // Large integers keep their value; floats that are not whole compare by value.
        assert_ne!(
            records("p.jsonl", "9007199254740993")[0].1,
            records("p.jsonl", "9007199254740992")[0].1
        );
        assert_eq!(records("p.jsonl", "0.5")[0].1, records("p.jsonl", "5e-1")[0].1);
    }

    #[test]
    fn a_library_document_is_one_value() {
        let doc = "{\n  \"$k\": \"symbol\",\n  \"lines\": [1,\n 2]\n}\n";
        let found = records("lib/symbols/0de5c40f-dcf9-560b-98a1-71a96115aaea.json", doc);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 1);
    }
}
