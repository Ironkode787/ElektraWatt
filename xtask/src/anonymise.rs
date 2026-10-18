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

use std::collections::HashSet;

use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

/// A SHA-256 digest.
pub type Hash = [u8; 32];

/// The longest n-gram hashed.
pub const MAX_N: usize = 3;

/// Parses the denylist: one lower-case hex SHA-256 digest per line; `#` comments and blank lines are ignored.
pub fn parse_denylist(text: &str) -> Result<HashSet<Hash>> {
    let mut set = HashSet::new();
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
pub fn scan(text: &str, denylist: &HashSet<Hash>) -> Vec<(usize, usize)> {
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

/// Checks `files` (path, content) against `denylist`. Files under `LICENSES/` are skipped.
pub fn check(files: &[(String, String)], denylist: &HashSet<Hash>) -> Vec<String> {
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

    #[test]
    fn malformed_denylist_is_rejected() {
        assert!(parse_denylist("ABCDEF").is_err());
        assert!(parse_denylist(&ACME_PANELS.to_uppercase()).is_err());
        assert!(parse_denylist(&ACME_PANELS[..63]).is_err());
    }

    #[test]
    fn real_repository_passes() {
        let root = crate::workspace_root();
        let violations = crate::check_anonymisation_at(&root).unwrap();
        assert_eq!(violations, Vec::<String>::new());
    }
}
