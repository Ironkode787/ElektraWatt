// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `manifest.json` (doc 05 §4): format stamps, generation and the SHA-256 of every file.

use std::collections::BTreeMap;
use std::fmt;

use ew_schema::AnyId;
use serde_json::{Map, Value};
use sha2::Digest;

use crate::limits::Limits;
use crate::path::{FilePath, MANIFEST};

/// The SHA-256 of `bytes`.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    sha2::Sha256::digest(bytes).into()
}

/// The hash form of the format: `sha256:<64 lower-case hex digits>` (doc 05 §3.2).
pub fn hash_text(hash: &[u8; 32]) -> String {
    let mut out = String::with_capacity(71);
    out.push_str("sha256:");
    for b in hash {
        out.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
    }
    out
}

/// The hash of one listed file.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FileHash {
    /// `sha256:<hex>`.
    Sha256([u8; 32]),
    /// `sha256:pending`: written by the merge driver where both sides changed the file (doc 05 §11.3); only
    /// valid in a manifest with `merged: true`.
    Pending,
}

impl FileHash {
    /// Whether this is the hash of `bytes`.
    pub fn matches(&self, bytes: &[u8]) -> bool {
        matches!(self, FileHash::Sha256(h) if *h == sha256(bytes))
    }
}

impl fmt::Display for FileHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileHash::Sha256(h) => f.write_str(&hash_text(h)),
            FileHash::Pending => f.write_str("sha256:pending"),
        }
    }
}

/// `manifest.json` (doc 05 §4).
///
/// The text form ([`Manifest::to_text`]) is pretty JSON with one file per line, sorted by path, so that
/// line merges stay local. It carries no wall-clock time.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Manifest {
    /// The writer's two-part format (`"0.1"`, doc 05 §10.1).
    pub format: String,
    /// The lowest format a reader needs.
    pub min_reader: String,
    /// The program and version that wrote this generation (`"ElektraWatt 0.0.0"`). The fixture comparison
    /// of doc 20 §3.2 ignores it (doc 05 §17.1).
    pub generator: String,
    /// The project row's ID.
    pub project: AnyId,
    /// +1 per save (doc 05 §4); at most 2⁵³ − 1.
    pub generation: u64,
    /// Every file except the manifest, the git files, `.ew/`, the lock files, `.~ew.requests/` and temps,
    /// by path.
    pub files: BTreeMap<FilePath, FileHash>,
    /// `true` only when written by the merge driver: the hashes are not authoritative. Written only when
    /// `true`.
    pub merged: bool,
    /// `true` when written by a development build with an unreleased schema (doc 05 §10.4). Written only
    /// when `true`.
    pub unstable: bool,
    /// Members of a newer format, kept and written after the known ones in byte-wise key order (doc 05
    /// §1 principle 6).
    pub extra: BTreeMap<String, Value>,
}

/// A `manifest.json` that cannot be used (EW-CORE-001).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ManifestError {
    /// Larger than [`Limits::max_manifest_bytes`].
    TooLarge {
        /// The size in bytes.
        len: u64,
    },
    /// Git conflict markers (doc 05 §11.4: the manifest conflicts on the `generation` line).
    ConflictMarkers,
    /// Not UTF-8 or not one JSON object (with duplicate member names rejected).
    Json {
        /// The parser's message.
        message: String,
    },
    /// A member is missing or has the wrong form.
    Invalid {
        /// The member (`files`, `generation`, `files/pages/x.jsonl`, …).
        member: String,
        /// What is wrong.
        message: String,
    },
    /// More entries in `files` than [`Limits::max_files`].
    TooManyFiles {
        /// The number of entries.
        count: usize,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::TooLarge { len } => write!(f, "manifest.json is too large ({len} bytes)"),
            ManifestError::ConflictMarkers => f.write_str("manifest.json holds git conflict markers"),
            ManifestError::Json { message } => write!(f, "manifest.json is not valid JSON: {message}"),
            ManifestError::Invalid { member, message } => {
                write!(f, "manifest.json member \"{member}\": {message}")
            }
            ManifestError::TooManyFiles { count } => {
                write!(f, "manifest.json lists {count} files, more than the limit")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

impl Manifest {
    /// A manifest without files.
    pub fn new(format: &str, generator: &str, project: AnyId, generation: u64) -> Manifest {
        Manifest {
            format: format.to_owned(),
            min_reader: format.to_owned(),
            generator: generator.to_owned(),
            project,
            generation,
            files: BTreeMap::new(),
            merged: false,
            unstable: false,
            extra: BTreeMap::new(),
        }
    }

    /// Reads `manifest.json`.
    ///
    /// # Errors
    ///
    /// [`ManifestError`] for a manifest that cannot be used (EW-CORE-001).
    pub fn parse(bytes: &[u8], limits: &Limits) -> Result<Manifest, ManifestError> {
        let len = bytes.len() as u64;
        if len > limits.max_manifest_bytes {
            return Err(ManifestError::TooLarge { len });
        }
        let text = std::str::from_utf8(bytes).map_err(|e| ManifestError::Json {
            message: format!("not UTF-8: {e}"),
        })?;
        if text.split('\n').any(has_conflict_marker) {
            return Err(ManifestError::ConflictMarkers);
        }
        let value = ew_schema::parse_value(text).map_err(|e| ManifestError::Json {
            message: match e {
                ew_schema::SchemaError::Json { message } => message,
                other => other.to_string(),
            },
        })?;
        let Value::Object(mut members) = value else {
            return Err(invalid("", "the manifest is not a JSON object"));
        };

        let format = take_string(&mut members, "format")?;
        let min_reader = take_string(&mut members, "min_reader")?;
        let generator = take_string(&mut members, "generator")?;
        let project = take_string(&mut members, "project")?
            .parse::<AnyId>()
            .map_err(|_| invalid("project", "not a canonical lower-case UUID"))?;
        let generation = match members.remove("generation") {
            None => return Err(invalid("generation", "missing")),
            Some(v) => v
                .as_u64()
                .filter(|g| *g <= MAX_GENERATION)
                .ok_or_else(|| invalid("generation", "not an integer from 0 to 2^53 - 1"))?,
        };
        let merged = take_flag(&mut members, "merged")?;
        let unstable = take_flag(&mut members, "unstable")?;
        let files = match members.remove("files") {
            None => return Err(invalid("files", "missing")),
            Some(Value::Object(map)) => parse_files(map, merged, limits)?,
            Some(_) => return Err(invalid("files", "not an object")),
        };
        let mut extra = BTreeMap::new();
        for (key, value) in members {
            ew_schema::check_value(&value).map_err(|e| invalid(&key, &e.to_string()))?;
            extra.insert(key, value);
        }
        Ok(Manifest {
            format,
            min_reader,
            generator,
            project,
            generation,
            files,
            merged,
            unstable,
            extra,
        })
    }

    /// The text of `manifest.json` (doc 05 §4).
    pub fn to_text(&self) -> String {
        let mut members: Vec<String> = vec![
            format!("  \"format\": {}", json_string(&self.format)),
            format!("  \"min_reader\": {}", json_string(&self.min_reader)),
            format!("  \"generator\": {}", json_string(&self.generator)),
            format!("  \"project\": {}", json_string(&self.project.to_string())),
            format!("  \"generation\": {}", self.generation),
        ];
        if self.files.is_empty() {
            members.push("  \"files\": {}".to_owned());
        } else {
            let entries: Vec<String> = self
                .files
                .iter()
                .map(|(path, hash)| {
                    format!(
                        "    {}: {}",
                        json_string(path.as_str()),
                        json_string(&hash.to_string())
                    )
                })
                .collect();
            members.push(format!(
                "  \"files\": {{
{}
  }}",
                entries.join(
                    ",
"
                )
            ));
        }
        if self.merged {
            members.push("  \"merged\": true".to_owned());
        }
        if self.unstable {
            members.push("  \"unstable\": true".to_owned());
        }
        for (key, value) in &self.extra {
            members.push(format!("  {}: {}", json_string(key), json_string(value)));
        }
        format!(
            "{{
{}
}}
",
            members.join(
                ",
"
            )
        )
    }
}

/// The largest generation: 2^53 - 1 (doc 05 §3.2).
const MAX_GENERATION: u64 = (1 << 53) - 1;

fn invalid(member: &str, message: &str) -> ManifestError {
    ManifestError::Invalid {
        member: member.to_owned(),
        message: message.to_owned(),
    }
}

/// Compact JSON text of a string or value (never fails for these inputs).
fn json_string<T: serde::Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn has_conflict_marker(line: &str) -> bool {
    ["<<<<<<<", "=======", ">>>>>>>"]
        .iter()
        .any(|m| line.starts_with(m))
}

fn take_string(members: &mut Map<String, Value>, name: &str) -> Result<String, ManifestError> {
    match members.remove(name) {
        None => Err(invalid(name, "missing")),
        Some(Value::String(s)) => Ok(s),
        Some(_) => Err(invalid(name, "not a string")),
    }
}

fn take_flag(members: &mut Map<String, Value>, name: &str) -> Result<bool, ManifestError> {
    match members.remove(name) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(b),
        Some(_) => Err(invalid(name, "not a boolean")),
    }
}

/// `sha256:<64 lower-case hex digits>` or, in a merged manifest, `sha256:pending`.
fn parse_hash(text: &str, merged: bool) -> Option<FileHash> {
    let hex = text.strip_prefix("sha256:")?;
    if hex == "pending" {
        return merged.then_some(FileHash::Pending);
    }
    let b = hex.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let nibble = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; 32];
    for (slot, pair) in out.iter_mut().zip(b.chunks_exact(2)) {
        *slot = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(FileHash::Sha256(out))
}

fn parse_files(
    map: Map<String, Value>,
    merged: bool,
    limits: &Limits,
) -> Result<BTreeMap<FilePath, FileHash>, ManifestError> {
    if map.len() > limits.max_files {
        return Err(ManifestError::TooManyFiles { count: map.len() });
    }
    let mut files = BTreeMap::new();
    for (key, value) in map {
        let member = format!("files/{key}");
        if key == MANIFEST {
            return Err(invalid(&member, "the manifest does not list itself"));
        }
        let path = FilePath::new(key.as_str()).map_err(|e| invalid(&member, &e.to_string()))?;
        let Value::String(text) = value else {
            return Err(invalid(&member, "not a string"));
        };
        let hash = parse_hash(&text, merged).ok_or_else(|| {
            invalid(
                &member,
                "not sha256:<64 lower-case hex digits> (sha256:pending only in a merged manifest)",
            )
        })?;
        files.insert(path, hash);
    }
    Ok(files)
}
