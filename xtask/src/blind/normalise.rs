// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The normalisation of product output (WP-S6.13 §5.10, D14, decision log ID-630) and the guards that go with it.
//!
//! This is the only rewriting of product output, applied alike to live runs, replays and committed recordings: the run
//! folder's absolute path, in any spelling the operating system gives it, becomes `«run»`, and the transcript
//! header's version `«version»`. A record ID is never rewritten: the product prints the alias `@n` itself (FT-422), so
//! a raw UUID in a transcript line is a product defect and is reported with its line.

use std::path::Path;

/// The first hyphenated UUID of `text` (8-4-4-4-12 hexadecimal digits), if any.
pub fn first_uuid(text: &str) -> Option<&str> {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let b = text.as_bytes();
    'start: for i in 0..b.len() {
        let mut j = i;
        for (g, n) in GROUPS.iter().enumerate() {
            if g > 0 {
                if b.get(j) != Some(&b'-') {
                    continue 'start;
                }
                j += 1;
            }
            for _ in 0..*n {
                if !b.get(j).is_some_and(u8::is_ascii_hexdigit) {
                    continue 'start;
                }
                j += 1;
            }
        }
        // ASCII from `i` to `j`, so both are character boundaries.
        return Some(&text[i..j]);
    }
    None
}

/// The first line of `lines` (1-based) that holds a UUID, with the UUID.
pub fn find_uuid_line<'a>(lines: impl IntoIterator<Item = &'a str>) -> Option<(usize, String)> {
    lines
        .into_iter()
        .enumerate()
        .find_map(|(i, l)| first_uuid(l).map(|u| (i + 1, u.to_owned())))
}

/// Whether `text` holds an absolute path of a machine: a drive path (`C:\`, `C:/`), a verbatim path (`\\?\`), a UNC
/// path, or a Unix path under a home, temporary or system folder. Used on what is committed.
pub fn absolute_path_in(text: &str) -> Option<String> {
    let b = text.as_bytes();
    for i in 0..b.len() {
        let drive = b[i].is_ascii_alphabetic()
            && b.get(i + 1) == Some(&b':')
            && matches!(b.get(i + 2), Some(b'\\' | b'/'))
            && (i == 0 || !b[i - 1].is_ascii_alphanumeric());
        if drive {
            return Some(text[i..].chars().take(24).collect());
        }
    }
    for needle in [
        "\\\\?\\",
        "\\\\\\\\?\\\\",
        "/Users/",
        "/home/",
        "/tmp/",
        "/var/",
        "/private/",
        "/mnt/",
    ] {
        if let Some(at) = text.find(needle) {
            return Some(text[at..].chars().take(24).collect());
        }
    }
    None
}

/// Replaces a run folder's path and a transcript's version.
#[derive(Clone, Debug)]
pub struct Normaliser {
    /// The spellings of the run folder, longest first.
    spellings: Vec<String>,
}

impl Normaliser {
    /// A normaliser for the run folder `dir` (the folder as given and as the system names it).
    pub fn new(dir: &Path) -> Normaliser {
        let mut bases: Vec<String> = Vec::new();
        let mut add = |p: &Path| {
            let s = p.to_string_lossy().into_owned();
            let plain = s.strip_prefix("\\\\?\\").unwrap_or(&s).to_owned();
            for t in [s.clone(), plain] {
                if !t.is_empty() && !bases.contains(&t) {
                    bases.push(t);
                }
            }
        };
        add(dir);
        if let Ok(canonical) = dir.canonicalize() {
            add(&canonical);
        }
        let mut spellings: Vec<String> = Vec::new();
        for base in bases {
            let slash = base.replace('\\', "/");
            let variants = [
                base.clone(),
                slash.clone(),
                base.replace('\\', "\\\\"),
                format!("\\\\?\\{base}"),
                format!("\\\\\\\\?\\\\{}", base.replace('\\', "\\\\")),
            ];
            for v in variants {
                // A Windows drive letter may be written in either case.
                let mut cased = vec![v.clone()];
                if let Some(first) = v.chars().next()
                    && v[first.len_utf8()..].starts_with(':')
                {
                    let flipped: String = if first.is_ascii_uppercase() {
                        first.to_ascii_lowercase()
                    } else {
                        first.to_ascii_uppercase()
                    }
                    .to_string()
                        + &v[first.len_utf8()..];
                    cased.push(flipped);
                }
                for c in cased {
                    if !spellings.contains(&c) {
                        spellings.push(c);
                    }
                }
            }
        }
        spellings.sort_by_key(|s| std::cmp::Reverse(s.len()));
        Normaliser { spellings }
    }

    /// `text` with the run folder's path as `«run»` and, in a transcript header, the version as `«version»`.
    pub fn text(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for s in &self.spellings {
            if out.contains(s.as_str()) {
                out = out.replace(s.as_str(), "«run»");
            }
        }
        const HEADER: &str = "{\"$schema\":\"ew.uitranscript/1\",\"version\":\"";
        if let Some(rest) = out.strip_prefix(HEADER)
            && let Some(end) = rest.find('"')
        {
            out = format!("{HEADER}«version»{}", &rest[end..]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_uuid_is_found_and_an_alias_is_not_one() {
        let uuid = "0197a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b";
        assert_eq!(first_uuid(&format!("select @1 and {uuid}.")), Some(uuid));
        assert_eq!(first_uuid("ew.iec.mcb-1pn@qet10 id:@3 @12"), None);
        assert_eq!(
            first_uuid("0197a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2"),
            None,
            "one digit short"
        );
        assert_eq!(first_uuid("-X1031:11/a 2026-10-24"), None);
        // A UUID in the middle of a word still counts: the transcript prints none.
        assert_eq!(first_uuid(&format!("x{uuid}y")), Some(uuid));
        assert_eq!(
            find_uuid_line(["a", "b", &format!("c {uuid}")]),
            Some((3, uuid.to_owned()))
        );
        assert_eq!(find_uuid_line(["a", "@1"]), None);
    }

    #[test]
    fn an_absolute_path_is_found_in_every_spelling() {
        for text in [
            "C:\\Users\\x\\ew-blind\\r1",
            "see c:/work/x",
            "\\\\?\\C:\\a",
            "/home/me/run",
            "/tmp/ew-blind/r1/work",
            "/Users/me/x",
            "{\"p\":\"D:\\\\x\"}",
        ] {
            assert!(absolute_path_in(text).is_some(), "{text}");
        }
        for text in [
            "project",
            "out/project-t1.pdf",
            "a:b",
            "-X1031:11/a",
            "ew.iec.mcb-1pn@qet10",
            "page 101-B2",
        ] {
            assert_eq!(absolute_path_in(text), None, "{text}");
        }
    }

    #[test]
    fn the_run_folder_and_the_version_are_replaced_in_every_spelling() {
        let dir = PathBuf::from(if cfg!(windows) {
            "C:\\Temp\\ew-blind\\r1"
        } else {
            "/tmp/ew-blind/r1"
        });
        let n = Normaliser::new(&dir);
        let plain = dir.to_string_lossy().into_owned();
        let slash = plain.replace('\\', "/");
        let escaped = plain.replace('\\', "\\\\");
        for spelling in [plain.clone(), slash, escaped, format!("\\\\?\\{plain}")] {
            let text = format!("{{\"message\":\"wrote {spelling}\\\\out.pdf\"}}");
            let out = n.text(&text);
            assert!(
                out.contains("«run»") && !out.contains("ew-blind"),
                "{spelling}: {out}"
            );
        }
        assert_eq!(n.text("project"), "project");
        let header = "{\"$schema\":\"ew.uitranscript/1\",\"version\":\"0.0.0\"}";
        assert_eq!(
            n.text(header),
            "{\"$schema\":\"ew.uitranscript/1\",\"version\":\"«version»\"}"
        );
        // Only the header's version: a version-looking value elsewhere stays.
        let step = "{\"step\":1,\"version\":\"0.0.0\"}";
        assert_eq!(n.text(step), step);
        // The normaliser is deterministic.
        assert_eq!(n.text(header), n.text(header));
    }
}
