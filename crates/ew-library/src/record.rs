// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Library record lines and the files of a `.ewlib` directory (doc 07 §2.2, WP-S1.19 §7.1, §7.2).
//!
//! A record line is the item's own document: `$k`, `id` = the item ID, `$v` when not 1, `version`, then the item's
//! members in declaration order, compact (the canonical form of doc 05 §3.1). It carries no `library_id`, no copy ID
//! and no hash: the reader adds them when it hands a [`LibDocument`] out, with the copy ID
//! `v5(NS_LIBCOPY, library ‖ item)` (ADR-0004 amendment 2) and the hash of the item's compact JSON (EW-CORE-014).

use std::path::{Component, Path, PathBuf};

use ew_format::Limits;
use ew_format::ew_schema::{Kind, LibDocument, Sha256, normalize_nfc, parse_value};
use ew_ids::Uuid;
use serde_json::{Map, Value};

/// The record directories of a library and their kinds, in byte order of the names.
pub(crate) const KIND_DIRS: [(&str, Kind); 5] = [
    ("fndefs", Kind::Fndef),
    ("forms", Kind::Form),
    ("macros", Kind::Macro),
    ("parts", Kind::Part),
    ("symbols", Kind::Symbol),
];

/// The manufacturer master file's stem (`manufacturers.jsonl`, `manufacturers.<generation>.jsonl`).
pub(crate) const MANUFACTURERS: &str = "manufacturers";

/// The longest path inside a library (doc 05 §2.1's bound for paths inside a project).
const MAX_PATH: usize = 1024;

/// Which records file of a library: the kind and, except for manufacturers, the shard (the item ID's last byte).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct ShardKey {
    /// The kind of every line.
    pub kind: Kind,
    /// The shard; `None` for the one manufacturers file.
    pub shard: Option<u8>,
}

impl ShardKey {
    /// The shard of `item` of `kind`.
    pub fn of(kind: Kind, item: Uuid) -> ShardKey {
        let shard = (kind != Kind::Manufacturer).then(|| item.as_bytes()[15]);
        ShardKey { kind, shard }
    }

    /// The file name of this shard at `generation`: `parts/3f.7.jsonl`, `manufacturers.7.jsonl`.
    pub fn file(&self, generation: u64) -> String {
        match self.shard {
            Some(s) => format!("{}/{s:02x}.{generation}.jsonl", kind_dir(self.kind)),
            None => format!("{MANUFACTURERS}.{generation}.jsonl"),
        }
    }
}

/// The directory of a record kind.
pub(crate) fn kind_dir(kind: Kind) -> &'static str {
    KIND_DIRS
        .iter()
        .find(|(_, k)| *k == kind)
        .map_or(MANUFACTURERS, |(d, _)| d)
}

/// What a path listed in `library.json` is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PathRole {
    /// A records file.
    Records(ShardKey),
    /// Any other file (assets, `REUSE.toml`, `LICENSES/`): kept, not read.
    Other,
}

/// Classifies a path of `library.json`; `Err` for a path that is not safe to join to the library directory.
pub(crate) fn classify(path: &str) -> Result<PathRole, String> {
    let bad = || format!("unsafe path \"{path}\"");
    if path.is_empty()
        || path.len() > MAX_PATH
        || path.starts_with('/')
        || path.contains(['\\', ':', '\0'])
        || path.split('/').any(|c| c.is_empty() || c == "." || c == "..")
    {
        return Err(bad());
    }
    let generation_ok = |g: &str| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()) && g.len() <= 16;
    if let Some(rest) = path.strip_prefix(MANUFACTURERS).and_then(|r| r.strip_prefix('.')) {
        let Some(stem) = rest.strip_suffix("jsonl") else {
            return Ok(PathRole::Other);
        };
        let ok = stem.is_empty() || stem.strip_suffix('.').is_some_and(generation_ok);
        if ok {
            return Ok(PathRole::Records(ShardKey {
                kind: Kind::Manufacturer,
                shard: None,
            }));
        }
        return Ok(PathRole::Other);
    }
    if let Some((dir, name)) = path.split_once('/')
        && let Some((_, kind)) = KIND_DIRS.iter().find(|(d, _)| *d == dir)
        && let Some(stem) = name.strip_suffix(".jsonl")
    {
        let (hex, generation) = match stem.split_once('.') {
            Some((h, g)) => (h, Some(g)),
            None => (stem, None),
        };
        let hex_ok = hex.len() == 2 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if hex_ok && generation.is_none_or(generation_ok) {
            let shard = u8::from_str_radix(hex, 16).map_err(|_| bad())?;
            return Ok(PathRole::Records(ShardKey {
                kind: *kind,
                shard: Some(shard),
            }));
        }
    }
    Ok(PathRole::Other)
}

/// Joins a classified relative path to the library directory.
pub(crate) fn join(dir: &Path, path: &str) -> PathBuf {
    path.split('/').fold(dir.to_path_buf(), |p, c| p.join(c))
}

/// Reads one file of a library under the loader's discipline (doc 05 §3.6): every component below `dir` must be a
/// directory or, last, a regular file, never a symbolic link or junction; at most `limits.max_file_bytes`.
pub(crate) fn read_file(dir: &Path, path: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    let full = join(dir, path);
    let mut at = dir.to_path_buf();
    let comps: Vec<Component<'_>> = Path::new(path).components().collect();
    for (i, c) in comps.iter().enumerate() {
        at.push(c.as_os_str());
        let meta = std::fs::symlink_metadata(&at).map_err(|e| format!("{path}: {e}"))?;
        let last = i + 1 == comps.len();
        if meta.file_type().is_symlink() || (last && !meta.is_file()) || (!last && !meta.is_dir()) {
            return Err(format!("{path}: not a regular file (links are not followed)"));
        }
        if last && meta.len() > max_bytes {
            return Err(format!(
                "{path}: larger than the loader limit ({} bytes)",
                meta.len()
            ));
        }
    }
    std::fs::read(&full).map_err(|e| format!("{path}: {e}"))
}

/// One record of a library: the item, its version, its canonical line and the document the reader hands out.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Entry {
    /// The item ID.
    pub item: Uuid,
    /// The item's version.
    pub version: String,
    /// The canonical record line, without LF.
    pub line: String,
    /// The document with copy metadata.
    pub doc: LibDocument,
}

/// The copy ID of `item` of `library`: `v5(NS_LIBCOPY, library ‖ item)`.
pub(crate) fn copy_id(library: Uuid, item: Uuid) -> Uuid {
    ew_ids::v5::lib_copy_id::<()>(library, item).as_uuid()
}

macro_rules! each_copy {
    ($doc:expr, $c:ident => $body:expr, $unknown:expr) => {
        match $doc {
            LibDocument::Part($c) => $body,
            LibDocument::Symbol($c) => $body,
            LibDocument::Fndef($c) => $body,
            LibDocument::Form($c) => $body,
            LibDocument::Macro($c) => $body,
            LibDocument::Manufacturer($c) => $body,
            LibDocument::Unknown(_) => $unknown,
        }
    };
}

/// `(library, item, version)` of a document; `None` for an unknown kind.
pub(crate) fn identity(doc: &LibDocument) -> Option<(Uuid, Uuid, String)> {
    each_copy!(doc, c => Some((c.library_id.as_uuid(), c.item_id.as_uuid(), c.version.clone())), None)
}

/// The canonical record line of a document: `$k`, `id` = the item ID, `$v` when not 1, `version`, the item's members.
pub(crate) fn record_line(doc: &LibDocument) -> Result<String, String> {
    let kind = doc.kind().ok_or("a document of an unknown kind")?;
    let (item, v, version, members) = each_copy!(doc, c => (
        c.item_id.as_uuid(),
        c.v,
        c.version.clone(),
        serde_json::to_string(&c.item).map_err(|e| e.to_string())?,
    ), return Err("a document of an unknown kind".to_owned()));
    let mut out = format!(
        "{{\"$k\":{},\"id\":\"{}\"",
        Value::from(kind.name()),
        item.hyphenated()
    );
    if v != 1 {
        out.push_str(&format!(",\"$v\":{v}"));
    }
    out.push_str(&format!(",\"version\":{}", Value::from(version)));
    let inner = members
        .strip_prefix('{')
        .and_then(|m| m.strip_suffix('}'))
        .ok_or("an item that is not a JSON object")?;
    if !inner.is_empty() {
        out.push(',');
        out.push_str(inner);
    }
    out.push('}');
    Ok(out)
}

/// Sets the copy metadata a library document gets when the reader hands it out: `library_id`, the copy ID and the
/// hash of the item's own members.
pub(crate) fn with_copy_metadata(mut doc: LibDocument, library: Uuid) -> LibDocument {
    let hash = ew_format::library_copy_hash(&doc);
    each_copy!(&mut doc, c => {
        c.library_id = ew_ids::Id::from_uuid(library);
        c.id = ew_ids::Id::from_uuid(copy_id(library, c.item_id.as_uuid()));
        if let Some(h) = hash {
            c.sha256 = Sha256(h);
        }
    }, {});
    doc
}

/// Whether a JSON text nests deeper than `max` (a linear scan outside strings, before any parse).
fn exceeds_depth(bytes: &[u8], max: usize) -> bool {
    let (mut depth, mut in_string, mut escape) = (0usize, false, false);
    for &b in bytes {
        if in_string {
            match (escape, b) {
                (true, _) => escape = false,
                (false, b'\\') => escape = true,
                (false, b'"') => in_string = false,
                _ => {}
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

/// Parses one record line of `kind` of `library` into an [`Entry`].
pub(crate) fn parse_line(line: &str, kind: Kind, library: Uuid, limits: &Limits) -> Result<Entry, String> {
    if line.len() > limits.max_line_bytes {
        return Err("a line longer than the loader limit".to_owned());
    }
    if exceeds_depth(line.as_bytes(), limits.max_depth) {
        return Err("a line nested deeper than the loader limit".to_owned());
    }
    let mut value = parse_value(line).map_err(|e| e.to_string())?;
    normalize_nfc(&mut value);
    let Value::Object(mut map) = value else {
        return Err("a line that is not a JSON object".to_owned());
    };
    match map.remove("$k") {
        Some(Value::String(k)) if k == kind.name() => {}
        Some(Value::String(k)) => return Err(format!("a `{k}` line in a `{}` file", kind.name())),
        _ => return Err("a line without `$k`".to_owned()),
    }
    let item = match map.remove("id") {
        Some(Value::String(s)) => Uuid::try_parse(&s).map_err(|_| format!("`id` is not a UUID: {s}"))?,
        _ => return Err("a line without `id`".to_owned()),
    };
    let version = match map.get("version") {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("a line without `version`".to_owned()),
    };
    let mut doc = Map::new();
    doc.insert("$k".into(), Value::from(kind.name()));
    doc.insert(
        "id".into(),
        Value::from(copy_id(library, item).hyphenated().to_string()),
    );
    doc.insert("library_id".into(), Value::from(library.hyphenated().to_string()));
    doc.insert("item_id".into(), Value::from(item.hyphenated().to_string()));
    doc.insert("sha256".into(), Value::from(ew_format::hash_text(&[0; 32])));
    doc.extend(map);
    let (doc, _malformed) = LibDocument::load(Value::Object(doc)).map_err(|e| e.to_string())?;
    if doc.kind() != Some(kind) {
        return Err(format!("a line this reader cannot type as `{}`", kind.name()));
    }
    let doc = with_copy_metadata(doc, library);
    let line = record_line(&doc)?;
    Ok(Entry {
        item,
        version,
        line,
        doc,
    })
}

/// A parsed records file: the entries, the notes of the skipped lines, the skipped lines verbatim.
pub(crate) type Parsed = (Vec<Entry>, Vec<String>, Vec<String>);

/// Parses a records file: the entries, a message per line that is skipped (the file stays usable without it), and
/// the skipped lines verbatim, which a writer keeps when it rewrites the file.
pub(crate) fn parse_records(
    bytes: &[u8],
    kind: Kind,
    library: Uuid,
    limits: &Limits,
) -> Result<Parsed, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "not UTF-8".to_owned())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut entries = Vec::new();
    let mut skipped = Vec::new();
    let mut kept = Vec::new();
    for (n, line) in text.split('\n').enumerate() {
        if n >= limits.max_lines_per_file {
            return Err("more lines than the loader limit".to_owned());
        }
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        match parse_line(line, kind, library, limits) {
            Ok(e) => entries.push(e),
            Err(m) => {
                skipped.push(format!("line {}: {m}", n + 1));
                kept.push(line.to_owned());
            }
        }
    }
    Ok((entries, skipped, kept))
}

/// The bytes of a records file: the lines sorted by item ID, LF after each.
pub(crate) fn records_file<'a>(lines: impl IntoIterator<Item = (Uuid, &'a str)>) -> Vec<u8> {
    let mut lines: Vec<(Uuid, &str)> = lines.into_iter().collect();
    lines.sort_by_key(|(id, _)| *id);
    let mut out = Vec::new();
    for (_, l) in lines {
        out.extend_from_slice(l.as_bytes());
        out.push(b'\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIB: Uuid = Uuid::from_u128(0x0192f3a4_5b6c_7d8e_8f01_23456789abd0);
    const ITEM: Uuid = Uuid::from_u128(0x0192f3a4_5b6c_7d8e_8f01_23456789ab3f);

    fn manufacturer_line() -> String {
        format!(
            r#"{{"$k":"manufacturer","id":"{ITEM}","version":"1.0.0","meta":{{"key":"se"}},"key":"SE","name":"Schneider Electric","aliases":["Schneider"]}}"#
        )
    }

    #[test]
    fn paths_classify_and_unsafe_paths_are_refused() {
        let rec = |kind, shard| Ok(PathRole::Records(ShardKey { kind, shard }));
        assert_eq!(classify("parts/3f.jsonl"), rec(Kind::Part, Some(0x3f)));
        assert_eq!(classify("parts/3f.12.jsonl"), rec(Kind::Part, Some(0x3f)));
        assert_eq!(classify("symbols/00.1.jsonl"), rec(Kind::Symbol, Some(0)));
        assert_eq!(classify("manufacturers.jsonl"), rec(Kind::Manufacturer, None));
        assert_eq!(classify("manufacturers.4.jsonl"), rec(Kind::Manufacturer, None));
        assert_eq!(classify("parts/3F.jsonl"), Ok(PathRole::Other));
        assert_eq!(classify("parts/3f.x.jsonl"), Ok(PathRole::Other));
        assert_eq!(classify("assets/ab.svg"), Ok(PathRole::Other));
        assert_eq!(classify("REUSE.toml"), Ok(PathRole::Other));
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "parts/../x",
            "a//b",
            "c:x",
            "a\\b",
            "./a",
        ] {
            assert!(classify(bad).is_err(), "{bad}");
        }
        assert_eq!(ShardKey::of(Kind::Part, ITEM).file(7), "parts/3f.7.jsonl");
        assert_eq!(
            ShardKey::of(Kind::Manufacturer, ITEM).file(7),
            "manufacturers.7.jsonl"
        );
    }

    #[test]
    fn a_line_gets_copy_metadata_and_renders_canonically() {
        let line = manufacturer_line();
        let e = parse_line(&line, Kind::Manufacturer, LIB, &Limits::default()).unwrap();
        assert_eq!(e.item, ITEM);
        assert_eq!(e.version, "1.0.0");
        let (lib, item, version) = identity(&e.doc).unwrap();
        assert_eq!((lib, item, version.as_str()), (LIB, ITEM, "1.0.0"));
        let LibDocument::Manufacturer(c) = &e.doc else {
            panic!()
        };
        assert_eq!(c.id.as_uuid(), copy_id(LIB, ITEM));
        assert_eq!(
            Some(c.sha256.0),
            ew_format::library_copy_hash(&e.doc),
            "EW-CORE-014"
        );
        // The canonical line: `$k`, `id`, `version`, then the members in declaration order.
        assert_eq!(
            e.line,
            format!(
                r#"{{"$k":"manufacturer","id":"{ITEM}","version":"1.0.0","meta":{{"key":"se"}},"key":"SE","name":"Schneider Electric","aliases":["Schneider"]}}"#
            )
        );
        // Member order in the input does not matter.
        let shuffled = format!(
            r#"{{"name":"Schneider Electric","key":"SE","version":"1.0.0","aliases":["Schneider"],"id":"{ITEM}","$k":"manufacturer","meta":{{"key":"se"}}}}"#
        );
        let e2 = parse_line(&shuffled, Kind::Manufacturer, LIB, &Limits::default()).unwrap();
        assert_eq!(e2.line, e.line);
        assert_eq!(e2.doc, e.doc);
    }

    #[test]
    fn bad_lines_are_skipped_with_the_reason() {
        let ok = manufacturer_line();
        let text = format!(
            "{ok}\n{{\"$k\":\"part\",\"id\":\"{ITEM}\",\"version\":\"1\"}}\nnot json\n{{\"$k\":\"manufacturer\",\"version\":\"1.0.0\"}}\n\n"
        );
        let (entries, skipped, kept) =
            parse_records(text.as_bytes(), Kind::Manufacturer, LIB, &Limits::default()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(kept.len(), 3, "kept verbatim for a rewrite");
        assert_eq!(kept[1], "not json");
        assert_eq!(skipped.len(), 3, "{skipped:?}");
        assert!(skipped[0].starts_with("line 2: a `part` line"), "{skipped:?}");
        assert!(skipped[2].contains("without `id`"), "{skipped:?}");
    }

    #[test]
    fn limits_apply_before_parsing() {
        let limits = Limits {
            max_depth: 2,
            ..Limits::default()
        };
        let deep = format!(r#"{{"$k":"manufacturer","id":"{ITEM}","version":"1","x":[[[1]]]}}"#);
        assert!(
            parse_line(&deep, Kind::Manufacturer, LIB, &limits)
                .unwrap_err()
                .contains("deeper")
        );
        assert!(exceeds_depth(b"[[[1]]]", 2));
        assert!(!exceeds_depth(br#"{"a":"[[[["}"#, 1));
        let short = Limits {
            max_line_bytes: 10,
            ..Limits::default()
        };
        assert!(
            parse_line(&manufacturer_line(), Kind::Manufacturer, LIB, &short)
                .unwrap_err()
                .contains("longer")
        );
    }

    #[test]
    fn records_files_sort_by_item() {
        let a = Uuid::from_u128(2);
        let b = Uuid::from_u128(1);
        assert_eq!(records_file([(a, "A"), (b, "B")]), b"B\nA\n");
    }
}
