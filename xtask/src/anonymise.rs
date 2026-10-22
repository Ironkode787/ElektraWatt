// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-anonymisation` and `denylist-hash`: the anonymisation guard of NFR-PRIV-09
//! (docs/design/20-golden-project.md §3.5, 19 LR-12).
//!
//! Every committed text file is normalised (Unicode NFC, lower-case), split into words at every character that
//! is not alphanumeric, and every word n-gram for n = 1, 2, 3 is hashed with SHA-256 (the words joined by one
//! space, UTF-8). A digest found in `ci/denylist.sha256` fails the check. Only hashes are committed, so the
//! confidential tokens never enter the repository; the messages name file and line, never the matched words,
//! because CI logs are public.
//!
//! The list ships without entries until the maintainers add them; the check then prints [`EMPTY_WARNING`]
//! and passes (an owner action must not block CI), and `--require-entries` turns the warning into a failure.
//! Files are decoded by [`decode`]: UTF-16 by its byte-order mark, everything else as lossy UTF-8, binary
//! files (NUL byte in the first 8 KiB) skipped.

// FT-271: the denylist is only asked `contains` and `is_empty`; it is never iterated.
#[allow(clippy::disallowed_types)]
use std::collections::HashSet;

use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

/// A SHA-256 digest.
pub type Hash = [u8; 32];

/// The digests of the denylist. Membership only: nothing iterates it, so its order never reaches output.
// FT-271: the one place that names the hash set; the rest of the module uses this alias.
#[allow(clippy::disallowed_types)]
pub type Denylist = HashSet<Hash>;

/// The longest n-gram hashed.
pub const MAX_N: usize = 3;

/// Parses the denylist: one lower-case hex SHA-256 digest per line; `#` comments and blank lines are ignored.
pub fn parse_denylist(text: &str) -> Result<Denylist> {
    let mut set = Denylist::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse_hex(line) {
            Some(hash) => {
                set.insert(hash);
            }
            None => bail!(
                "line {}: expected a lower-case hex SHA-256 digest (64 characters 0-9, a-f)",
                i + 1
            ),
        }
    }
    Ok(set)
}

fn parse_hex(s: &str) -> Option<Hash> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in s.as_bytes().chunks(2).enumerate() {
        let nibble = |b: u8| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        };
        out[i] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(out)
}

/// Lower-case hex of a digest.
pub fn to_hex(hash: &Hash) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// The words of a text after normalisation, stored in one buffer to avoid an allocation per word.
pub struct Words {
    buf: String,
    /// Byte range in `buf` and 1-based line number of each word.
    spans: Vec<(usize, usize, usize)>,
}

impl Words {
    /// Normalises `text` (NFC, lower-case) and splits it into words at every non-alphanumeric character.
    pub fn new(text: &str) -> Self {
        let mut buf = String::with_capacity(text.len());
        let mut spans = Vec::new();
        let mut line = 1;
        let mut start: Option<(usize, usize)> = None;
        for c in text.nfc() {
            for lc in c.to_lowercase() {
                if lc.is_alphanumeric() {
                    if start.is_none() {
                        start = Some((buf.len(), line));
                    }
                    buf.push(lc);
                } else if let Some((s, l)) = start.take() {
                    spans.push((s, buf.len(), l));
                }
            }
            if c == '\n' {
                line += 1;
            }
        }
        if let Some((s, l)) = start {
            spans.push((s, buf.len(), l));
        }
        Words { buf, spans }
    }

    fn word(&self, i: usize) -> &[u8] {
        let (s, e, _) = self.spans[i];
        &self.buf.as_bytes()[s..e]
    }

    /// Calls `f(first_word_index, n, digest)` for every n-gram, n = 1 ..= [`MAX_N`]. The hasher state of the
    /// (n-1)-gram is extended for the n-gram, so each word is hashed once per n-gram that contains it.
    pub fn for_each_ngram(&self, mut f: impl FnMut(usize, usize, Hash)) {
        for i in 0..self.spans.len() {
            let mut hasher = Sha256::new();
            for n in 1..=MAX_N {
                let j = i + n - 1;
                if j >= self.spans.len() {
                    break;
                }
                if n > 1 {
                    hasher.update(b" ");
                }
                hasher.update(self.word(j));
                f(i, n, hasher.clone().finalize().into());
            }
        }
    }

    /// The line on which word `i` starts.
    pub fn line(&self, i: usize) -> usize {
        self.spans[i].2
    }
}

/// The (line, n) of every n-gram of `text` whose digest is in `denylist`.
pub fn scan(text: &str, denylist: &Denylist) -> Vec<(usize, usize)> {
    let words = Words::new(text);
    let mut hits = Vec::new();
    words.for_each_ngram(|i, n, hash| {
        if denylist.contains(&hash) {
            hits.push((words.line(i), n));
        }
    });
    hits
}

/// The digests of every 1-, 2- and 3-gram of `phrase`, with n and the 1-based index of the first word.
pub fn phrase_digests(phrase: &str) -> Vec<(usize, usize, String)> {
    let words = Words::new(phrase);
    let mut out = Vec::new();
    words.for_each_ngram(|i, n, hash| out.push((n, i + 1, to_hex(&hash))));
    out.sort();
    out
}

/// The warning printed while `ci/denylist.sha256` has no entries: the guard cannot match anything then.
pub const EMPTY_WARNING: &str = "check-anonymisation: WARNING: ci/denylist.sha256 has no entries; the guard is \
inactive until the maintainers add hashes (owner action, NFR-PRIV-09)";

/// Bytes at the start of a file that are searched for a NUL byte to recognise binary files.
const BINARY_PROBE: usize = 8 * 1024;

/// Decodes a committed file for scanning, or `None` for a binary file.
///
/// A file that starts with a UTF-16 byte-order mark is decoded as UTF-16 (little or big endian). Otherwise a
/// file with a NUL byte in its first 8 KiB is binary and skipped. Everything else is decoded as UTF-8, and
/// invalid sequences become U+FFFD instead of hiding the whole file, so Latin-1 text is still scanned.
pub fn decode(bytes: &[u8]) -> Option<String> {
    let utf16 = |rest: &[u8], little_endian: bool| {
        let units = rest.chunks_exact(2).map(|p| {
            if little_endian {
                u16::from_le_bytes([p[0], p[1]])
            } else {
                u16::from_be_bytes([p[0], p[1]])
            }
        });
        char::decode_utf16(units)
            .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect::<String>()
    };
    match bytes {
        [0xff, 0xfe, rest @ ..] => Some(utf16(rest, true)),
        [0xfe, 0xff, rest @ ..] => Some(utf16(rest, false)),
        _ if bytes[..bytes.len().min(BINARY_PROBE)].contains(&0) => None,
        _ => Some(String::from_utf8_lossy(bytes).into_owned()),
    }
}

/// Checks `files` (path, content) against `denylist`. Files under `LICENSES/` are skipped.
pub fn check(files: &[(String, String)], denylist: &Denylist) -> Vec<String> {
    let mut v = Vec::new();
    for (path, text) in files {
        if path.starts_with("LICENSES/") {
            continue;
        }
        for (line, n) in scan(text, denylist) {
            v.push(format!(
                "{path}:{line}: a {n}-word sequence matches an entry of ci/denylist.sha256; replace it with a \
                 placeholder such as `<customer>` (NFR-PRIV-09)"
            ));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SHA-256 of "acme panels" (computed independently with Python: hashlib.sha256(b"acme panels")).
    const ACME_PANELS: &str = "9065121ffd4eeaafa52851b8417c4527f53caf0a9c499f34bae489424de26e78";

    #[test]
    fn normalisation_and_word_split() {
        let w = Words::new("Hello, WORLD!\nfoo_bar  Ünïcode é");
        let words: Vec<&str> = (0..w.spans.len())
            .map(|i| std::str::from_utf8(w.word(i)).unwrap())
            .collect();
        assert_eq!(words, ["hello", "world", "foo", "bar", "ünïcode", "é"]);
        assert_eq!(w.line(0), 1);
        assert_eq!(w.line(2), 2);
        // NFD input normalises to the same word as NFC input.
        let nfd = Words::new("e\u{301}");
        assert_eq!(nfd.word(0), "é".as_bytes());
    }

    #[test]
    fn denylist_hash_matches_independent_digest() {
        let digests = phrase_digests("ACME  Panels");
        let two: Vec<&str> = digests
            .iter()
            .filter(|(n, _, _)| *n == 2)
            .map(|(_, _, h)| h.as_str())
            .collect();
        assert_eq!(two, [ACME_PANELS]);
        assert_eq!(digests.len(), 3, "two 1-grams and one 2-gram: {digests:?}");
    }

    #[test]
    fn seeded_denylist_entry_fails() {
        let denylist = parse_denylist(&format!("# header\n\n{ACME_PANELS}\n")).unwrap();
        let files = vec![
            ("docs/clean.md".to_owned(), "Nothing to see here.\n".to_owned()),
            (
                "golden/tables/parties.csv".to_owned(),
                "id,name\n1,<customer>\n2,Acme\nPanels B.V.\n".to_owned(),
            ),
            ("LICENSES/X.txt".to_owned(), "acme panels".to_owned()),
        ];
        let v = check(&files, &denylist);
        // The 2-gram spans a line break; the line of its first word is reported, never the words.
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].starts_with("golden/tables/parties.csv:3: a 2-word sequence"),
            "{v:#?}"
        );
        assert!(!v[0].to_lowercase().contains("acme"));
    }

    /// `check` over a file decoded from raw bytes, as `check-anonymisation` does.
    fn check_bytes(path: &str, bytes: &[u8]) -> Vec<String> {
        let denylist = parse_denylist(ACME_PANELS).unwrap();
        let files: Vec<(String, String)> = decode(bytes).map(|t| (path.to_owned(), t)).into_iter().collect();
        check(&files, &denylist)
    }

    #[test]
    fn utf16_files_are_decoded_and_scanned() {
        let text = "id\nAcme Panels\n";
        let le: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let be: Vec<u8> = [0xfe, 0xff]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        for (name, bytes) in [("le.csv", le), ("be.csv", be)] {
            let v = check_bytes(name, &bytes);
            assert_eq!(v.len(), 1, "{name}: {v:#?}");
            assert!(
                v[0].starts_with(&format!("{name}:2: a 2-word sequence")),
                "{v:#?}"
            );
        }
    }

    #[test]
    fn latin1_files_are_scanned_lossily() {
        // 0xe9 is e-acute in Latin-1 and invalid UTF-8; the ASCII words must still be found.
        let bytes = b"caf\xe9 menu\nAcme Panels\n";
        assert!(String::from_utf8(bytes.to_vec()).is_err());
        let v = check_bytes("notes.txt", bytes);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].starts_with("notes.txt:2:"), "{v:#?}");
    }

    #[test]
    fn binary_files_are_skipped() {
        let bytes = b"\x89PNG\0\0\0Acme Panels";
        assert_eq!(decode(bytes), None);
        assert!(check_bytes("logo.png", bytes).is_empty());
        // A NUL after the probe window does not make a file binary.
        let mut late = b"Acme Panels ".repeat(1000);
        late.push(0);
        assert!(decode(&late).is_some());
    }

    #[test]
    fn empty_denylist_fails_when_entries_are_required() {
        let dir = std::env::temp_dir().join(format!("xtask-anon-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("ci")).unwrap();
        std::fs::write(dir.join("ci/denylist.sha256"), "# no entries\n").unwrap();
        // The strict path returns before the repository is listed, so no git repository is needed.
        let v = crate::check_anonymisation_at(&dir, true).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].contains("--require-entries"), "{v:#?}");
        assert!(
            EMPTY_WARNING.starts_with("check-anonymisation: WARNING: ci/denylist.sha256 has no entries;"),
            "{EMPTY_WARNING}"
        );
    }

    #[test]
    fn malformed_denylist_is_rejected() {
        assert!(parse_denylist("ABCDEF").is_err());
        assert!(parse_denylist(&ACME_PANELS.to_uppercase()).is_err());
        assert!(parse_denylist(&ACME_PANELS[..63]).is_err());
    }

    #[test]
    fn real_repository_passes() {
        let root = crate::workspace_root();
        let violations = crate::check_anonymisation_at(&root, false).unwrap();
        assert_eq!(violations, Vec::<String>::new());
    }
}
