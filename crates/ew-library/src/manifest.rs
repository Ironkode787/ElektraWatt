// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `library.json`, the manifest of a `.ewlib` directory (doc 07 §2.2, WP-S1.19 §7.1), written last.

use std::collections::BTreeMap;

use ew_format::{FileHash, hash_text};
use ew_ids::Uuid;
use serde_json::{Map, Value};

/// The manifest's file name.
pub const MANIFEST: &str = "library.json";

/// The format a library is written in (doc 05 §10.1).
pub const FORMAT: &str = "0.1";

/// `library.json` (doc 07 §2.2): identity, version, licence, format stamps, generation, dependencies and the
/// SHA-256 of every file. Pretty JSON with one file per line, sorted by path; no wall-clock time.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LibManifest {
    /// The library ID.
    pub id: Uuid,
    /// The human key (`ew-starter-parts`, `company-std`).
    pub key: String,
    /// The display name.
    pub name: String,
    /// The library's semantic version.
    pub version: String,
    /// The default SPDX licence expression of its items.
    pub licence: String,
    /// The writer's format.
    pub format: String,
    /// The lowest format a reader needs.
    pub min_reader: String,
    /// +1 per write.
    pub generation: u64,
    /// `[{library, version_req}]`, kept as written.
    pub depends: Vec<Value>,
    /// Every file except the manifest, the lock files and temporary files, by path.
    pub files: BTreeMap<String, [u8; 32]>,
    /// Members of a newer format, written after the known ones in byte-wise key order.
    pub extra: BTreeMap<String, Value>,
}

impl LibManifest {
    /// A new library's manifest at generation 1, without files.
    pub fn new(id: Uuid, key: &str, name: &str, licence: &str) -> LibManifest {
        LibManifest {
            id,
            key: key.to_owned(),
            name: name.to_owned(),
            version: "0.1.0".to_owned(),
            licence: licence.to_owned(),
            format: FORMAT.to_owned(),
            min_reader: FORMAT.to_owned(),
            generation: 1,
            depends: Vec::new(),
            files: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }

    /// Reads a manifest; the message names the member that is wrong.
    pub fn parse(bytes: &[u8]) -> Result<LibManifest, String> {
        let text = std::str::from_utf8(bytes).map_err(|_| "library.json is not UTF-8".to_owned())?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let value = ew_format::ew_schema::parse_value(text).map_err(|e| format!("library.json: {e}"))?;
        let Value::Object(mut map) = value else {
            return Err("library.json is not a JSON object".to_owned());
        };
        fn string(map: &mut Map<String, Value>, key: &str) -> Result<String, String> {
            match map.remove(key) {
                Some(Value::String(s)) => Ok(s),
                Some(_) => Err(format!("library.json member \"{key}\" is not a string")),
                None => Err(format!("library.json has no member \"{key}\"")),
            }
        }
        let id_text = string(&mut map, "id")?;
        let id =
            Uuid::try_parse(&id_text).map_err(|_| format!("library.json \"id\" is not a UUID: {id_text}"))?;
        let key = string(&mut map, "key")?;
        let name = string(&mut map, "name")?;
        let version = string(&mut map, "version")?;
        let licence = string(&mut map, "licence")?;
        let format = string(&mut map, "format")?;
        let min_reader = string(&mut map, "min_reader")?;
        let generation = match map.remove("generation") {
            Some(Value::Number(n)) => n
                .as_u64()
                .filter(|g| *g <= ew_format::ew_schema::MAX_SAFE_INTEGER as u64)
                .ok_or_else(|| "library.json \"generation\" is not a safe non-negative integer".to_owned())?,
            _ => return Err("library.json \"generation\" is missing or not a number".to_owned()),
        };
        let depends = match map.remove("depends") {
            None => Vec::new(),
            Some(Value::Array(a)) => a,
            Some(_) => return Err("library.json \"depends\" is not an array".to_owned()),
        };
        let mut files = BTreeMap::new();
        match map.remove("files") {
            None => {}
            Some(Value::Object(f)) => {
                for (path, hash) in f {
                    let Value::String(h) = hash else {
                        return Err(format!("library.json files/{path}: the hash is not a string"));
                    };
                    let parsed =
                        parse_hash(&h).ok_or_else(|| format!("library.json files/{path}: bad hash {h}"))?;
                    files.insert(path, parsed);
                }
            }
            Some(_) => return Err("library.json \"files\" is not an object".to_owned()),
        }
        Ok(LibManifest {
            id,
            key,
            name,
            version,
            licence,
            format,
            min_reader,
            generation,
            depends,
            files,
            extra: map.into_iter().collect(),
        })
    }

    /// The canonical text: pretty JSON, known members in order, `depends` and every extra member compact on one
    /// line, `files` one per line in path order, LF and a final LF.
    pub fn to_text(&self) -> String {
        let s = |v: &str| Value::from(v).to_string();
        let mut out = String::from("{\n");
        out.push_str(&format!("  \"id\": {},\n", s(&self.id.hyphenated().to_string())));
        out.push_str(&format!("  \"key\": {},\n", s(&self.key)));
        out.push_str(&format!("  \"name\": {},\n", s(&self.name)));
        out.push_str(&format!("  \"version\": {},\n", s(&self.version)));
        out.push_str(&format!("  \"licence\": {},\n", s(&self.licence)));
        out.push_str(&format!("  \"format\": {},\n", s(&self.format)));
        out.push_str(&format!("  \"min_reader\": {},\n", s(&self.min_reader)));
        out.push_str(&format!("  \"generation\": {},\n", self.generation));
        out.push_str(&format!(
            "  \"depends\": {},\n",
            Value::Array(self.depends.clone())
        ));
        if self.files.is_empty() {
            out.push_str("  \"files\": {}");
        } else {
            out.push_str("  \"files\": {\n");
            let n = self.files.len();
            for (i, (path, hash)) in self.files.iter().enumerate() {
                out.push_str(&format!("    {}: {}", s(path), s(&hash_text(hash))));
                out.push_str(if i + 1 < n { ",\n" } else { "\n" });
            }
            out.push_str("  }");
        }
        for (k, v) in &self.extra {
            out.push_str(&format!(",\n  {}: {}", s(k), v));
        }
        out.push_str("\n}\n");
        out
    }

    /// Whether this reader can read the library: its `min_reader` is at most [`FORMAT`] (doc 05 §10.1).
    pub fn readable(&self) -> bool {
        fn parts(v: &str) -> Option<(u64, u64)> {
            let (a, b) = v.split_once('.')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        }
        match (parts(&self.min_reader), parts(FORMAT)) {
            (Some(need), Some(have)) => need <= have,
            _ => false,
        }
    }
}

/// `sha256:<64 lower-case hex digits>` as bytes.
pub(crate) fn parse_hash(text: &str) -> Option<[u8; 32]> {
    let hex = text.strip_prefix("sha256:")?.as_bytes();
    if hex.len() != 64 {
        return None;
    }
    // Each byte a digit or `a`-`f` itself: `u8::from_str_radix` would also take a sign (`+f`) or upper case.
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; 32];
    for (b, pair) in out.iter_mut().zip(hex.chunks_exact(2)) {
        *b = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    Some(out)
}

/// Whether `bytes` hash to `hash`.
pub(crate) fn hash_matches(hash: &[u8; 32], bytes: &[u8]) -> bool {
    FileHash::Sha256(*hash).matches(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LibManifest {
        let mut m = LibManifest::new(
            Uuid::from_u128(0x0192f3a4_5b6c_7d8e_8f01_23456789abd0),
            "company-std",
            "Company standard",
            "CC0-1.0",
        );
        m.generation = 3;
        m.files.insert("parts/3f.3.jsonl".into(), ew_format::sha256(b"a"));
        m.files
            .insert("manufacturers.2.jsonl".into(), ew_format::sha256(b"b"));
        m.extra.insert("zz_later".into(), Value::from(7));
        m
    }

    #[test]
    fn the_text_round_trips_and_is_canonical() {
        let m = sample();
        let text = m.to_text();
        assert!(text.ends_with("}\n"));
        let back = LibManifest::parse(text.as_bytes()).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.to_text(), text, "a second write gives the same bytes");
        // Files in path order, one per line.
        let a = text.find("manufacturers.2.jsonl").unwrap();
        let b = text.find("parts/3f.3.jsonl").unwrap();
        assert!(a < b, "{text}");
    }

    #[test]
    fn a_manifest_without_files_round_trips() {
        let m = LibManifest::new(Uuid::from_u128(1), "k", "N", "CC0-1.0");
        assert_eq!(LibManifest::parse(m.to_text().as_bytes()).unwrap(), m);
    }

    #[test]
    fn bad_manifests_are_refused_with_the_member() {
        assert!(LibManifest::parse(b"[]").unwrap_err().contains("object"));
        let m = sample().to_text();
        let dup = m.replacen("\"key\"", "\"name\": \"x\",\n  \"key\"", 1);
        assert!(
            LibManifest::parse(dup.as_bytes()).is_err(),
            "duplicate member names"
        );
        let no_id = m.replacen("\"id\"", "\"idx\"", 1);
        assert!(
            LibManifest::parse(no_id.as_bytes())
                .unwrap_err()
                .contains("\"id\"")
        );
        let bad_hash = m.replacen("sha256:", "sha1:", 1);
        assert!(
            LibManifest::parse(bad_hash.as_bytes())
                .unwrap_err()
                .contains("bad hash")
        );
    }

    #[test]
    fn a_newer_min_reader_is_not_readable() {
        let mut m = sample();
        assert!(m.readable());
        m.min_reader = "0.2".into();
        assert!(!m.readable());
        m.min_reader = "x".into();
        assert!(!m.readable());
    }

    #[test]
    fn hashes_parse_in_lower_case_only() {
        let h = ew_format::sha256(b"x");
        assert_eq!(parse_hash(&hash_text(&h)), Some(h));
        assert_eq!(
            parse_hash(&hash_text(&h).to_uppercase().replace("SHA256", "sha256")),
            None
        );
        assert_eq!(parse_hash("sha256:00"), None);
        // Review C35 nit: a sign is not a hex digit.
        let plus = format!("sha256:+f{}", &hash_text(&h)["sha256:".len() + 2..]);
        assert_eq!(plus.len(), hash_text(&h).len());
        assert_eq!(parse_hash(&plus), None);
        let space = format!("sha256: f{}", &hash_text(&h)["sha256:".len() + 2..]);
        assert_eq!(parse_hash(&space), None);
        assert!(hash_matches(&h, b"x"));
        assert!(!hash_matches(&h, b"y"));
    }
}
