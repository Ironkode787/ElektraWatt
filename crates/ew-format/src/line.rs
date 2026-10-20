// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Canonical lines and library documents (doc 05 §3.1, §3.4) and the JSON Lines file form.

use ew_schema::{Kind, LibDocument, Record};
use serde::Serialize;

use crate::error::FormatError;
use crate::order::LineKey;

/// `$k` first, then the record's own members in declaration order: the envelope of `ew-schema`'s
/// `Record` and `LibDocument`, rebuilt here so that a row held behind an `Arc` serialises without being
/// cloned into a `Record`. The bytes are the same (tested over the golden project).
#[derive(Serialize)]
struct Tagged<'a, T> {
    #[serde(rename = "$k")]
    k: &'static str,
    #[serde(flatten)]
    inner: &'a T,
}

fn compact<T: Serialize>(value: &T, what: impl Fn() -> String) -> Result<String, FormatError> {
    let text = serde_json::to_string(value).map_err(|e| FormatError::Serialize {
        record: what(),
        message: e.to_string(),
    })?;
    // Compact serde_json output escapes every control character, so a line never breaks.
    debug_assert!(!text.contains('\n'));
    Ok(text)
}

/// The canonical line of a record of kind `kind` whose struct (without `$k`) is `inner` (doc 05 §3.1): the
/// compact `serde_json` form. Same bytes as [`record_line`] of the corresponding [`Record`].
///
/// # Errors
///
/// [`FormatError::Serialize`] for a record that has no JSON form (ADR-0005: a coordinate beyond
/// ±(2⁵³ − 1)).
pub fn tagged_line<T: Serialize>(kind: Kind, inner: &T) -> Result<String, FormatError> {
    compact(
        &Tagged {
            k: kind.name(),
            inner,
        },
        || kind.name().to_owned(),
    )
}

/// The canonical line of a record (doc 05 §3.1). A record of an unknown kind re-emits its members in
/// byte-wise key order; the writer prefers its original line where it has one (doc 05 §10.3).
///
/// # Errors
///
/// [`FormatError::Serialize`] for a record that has no JSON form.
pub fn record_line(record: &Record) -> Result<String, FormatError> {
    compact(record, || {
        let id = record
            .id()
            .map(|u| u.hyphenated().to_string())
            .unwrap_or_default();
        format!("{}:{id}", record.kind_name().unwrap_or("?"))
    })
}

/// The text of a library document (doc 05 §3.4): pretty JSON with 2-space indentation, every array or
/// object whose compact form is at most [`INLINE_LIMIT`] bytes written inline with `": "` and `", "`
/// separators, LF line ends and a final LF.
///
/// # Errors
///
/// [`FormatError::Serialize`] for a document that has no JSON form.
pub fn lib_document_text(doc: &LibDocument) -> Result<String, FormatError> {
    let line = compact(doc, || format!("{:?}", doc.kind()))?;
    Ok(pretty(&line))
}

/// [`lib_document_text`] of a library copy of kind `kind` held behind an `Arc`.
///
/// # Errors
///
/// [`FormatError::Serialize`] for a document that has no JSON form.
pub fn tagged_document<T: Serialize>(kind: Kind, inner: &T) -> Result<String, FormatError> {
    Ok(pretty(&tagged_line(kind, inner)?))
}

/// The compact length up to which an array or object of a library document stays on one line.
pub const INLINE_LIMIT: usize = 100;

/// One JSON value of a compact text, by byte range.
enum Node {
    Scalar(usize, usize),
    Array(usize, usize, Vec<Node>),
    Object(usize, usize, Vec<(usize, usize, Node)>),
}

/// A recursive-descent reader of compact JSON as `serde_json` writes it (no whitespace outside strings).
struct Spans<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Spans<'_> {
    fn string_end(&self, start: usize) -> usize {
        let mut i = start + 1;
        while i < self.b.len() {
            match self.b[i] {
                b'\\' => i += 2,
                b'"' => return i + 1,
                _ => i += 1,
            }
        }
        self.b.len()
    }

    fn value(&mut self) -> Node {
        let start = self.pos;
        match self.b.get(start) {
            Some(b'"') => {
                self.pos = self.string_end(start);
                Node::Scalar(start, self.pos)
            }
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                while self.b.get(self.pos).is_some_and(|&c| c != b']') {
                    items.push(self.value());
                    if self.b.get(self.pos) == Some(&b',') {
                        self.pos += 1;
                    }
                }
                self.pos += 1;
                Node::Array(start, self.pos, items)
            }
            Some(b'{') => {
                self.pos += 1;
                let mut members = Vec::new();
                while self.b.get(self.pos).is_some_and(|&c| c != b'}') {
                    let key_start = self.pos;
                    let key_end = self.string_end(key_start);
                    self.pos = key_end + 1; // the `:`
                    members.push((key_start, key_end, self.value()));
                    if self.b.get(self.pos) == Some(&b',') {
                        self.pos += 1;
                    }
                }
                self.pos += 1;
                Node::Object(start, self.pos, members)
            }
            _ => {
                while self
                    .b
                    .get(self.pos)
                    .is_some_and(|&c| !matches!(c, b',' | b']' | b'}'))
                {
                    self.pos += 1;
                }
                Node::Scalar(start, self.pos)
            }
        }
    }
}

fn render(text: &str, node: &Node, indent: usize, inline: bool, out: &mut String) {
    match node {
        Node::Scalar(s, e) => out.push_str(&text[*s..*e]),
        Node::Array(s, e, items) => {
            if items.is_empty() {
                out.push_str("[]");
            } else if inline || e - s <= INLINE_LIMIT {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    render(text, item, indent, true, out);
                }
                out.push(']');
            } else {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    push_indent(out, indent + 2);
                    render(text, item, indent + 2, false, out);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                push_indent(out, indent);
                out.push(']');
            }
        }
        Node::Object(s, e, members) => {
            if members.is_empty() {
                out.push_str("{}");
            } else if inline || e - s <= INLINE_LIMIT {
                out.push('{');
                for (i, (ks, ke, value)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&text[*ks..*ke]);
                    out.push_str(": ");
                    render(text, value, indent, true, out);
                }
                out.push('}');
            } else {
                out.push_str("{\n");
                for (i, (ks, ke, value)) in members.iter().enumerate() {
                    push_indent(out, indent + 2);
                    out.push_str(&text[*ks..*ke]);
                    out.push_str(": ");
                    render(text, value, indent + 2, false, out);
                    out.push_str(if i + 1 < members.len() { ",\n" } else { "\n" });
                }
                push_indent(out, indent);
                out.push('}');
            }
        }
    }
}

fn push_indent(out: &mut String, n: usize) {
    out.extend(std::iter::repeat_n(' ', n));
}

/// Pretty-prints compact canonical JSON (the output of `serde_json::to_string`) in the library-document
/// form of doc 05 §3.4, keeping the member order. A final LF is appended.
///
/// ```
/// let compact = r#"{"$k":"part","meta":{"licence":"CC0-1.0","sources":["golden tables"]},"cp_pattern":[]}"#;
/// assert_eq!(
///     ew_format::pretty(compact),
///     "{\"$k\": \"part\", \"meta\": {\"licence\": \"CC0-1.0\", \"sources\": [\"golden tables\"]}, \"cp_pattern\": []}\n"
/// );
/// ```
pub fn pretty(compact: &str) -> String {
    let mut spans = Spans {
        b: compact.as_bytes(),
        pos: 0,
    };
    let root = spans.value();
    let mut out = String::with_capacity(compact.len() * 5 / 4 + 16);
    render(compact, &root, 0, false, &mut out);
    out.push('\n');
    out
}

/// The bytes of a JSON Lines file: the lines sorted by key (doc 05 §3.3), each followed by LF (doc 05
/// §3.1). No lines give no bytes: a file that would be empty is not written.
pub fn jsonl_file(mut lines: Vec<(LineKey, String)>) -> Vec<u8> {
    lines.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let len = lines.iter().map(|(_, l)| l.len() + 1).sum();
    let mut out = Vec::with_capacity(len);
    for (_, line) in lines {
        out.extend_from_slice(line.as_bytes());
        out.push(b'\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_containers_break_short_ones_stay_inline() {
        let long: String = (0..60).map(|i| format!("{i}")).collect::<Vec<_>>().join(",");
        let compact = format!(r#"{{"a":[{long}],"b":{{"c":"d"}},"e":[[1,2],[3,4]]}}"#);
        let text = pretty(&compact);
        let expected = format!(
            "{{\n  \"a\": [\n{}\n  ],\n  \"b\": {{\"c\": \"d\"}},\n  \"e\": [[1, 2], [3, 4]]\n}}\n",
            (0..60)
                .map(|i| format!("    {i}"))
                .collect::<Vec<_>>()
                .join(",\n")
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn strings_with_structural_characters_are_kept() {
        let compact = r#"{"a":"x,\"]}y","b":["{",":"]}"#;
        assert_eq!(pretty(compact), "{\"a\": \"x,\\\"]}y\", \"b\": [\"{\", \":\"]}\n");
    }

    #[test]
    fn jsonl_files_sort_and_end_with_lf() {
        let a = LineKey::new(
            "device",
            Some("00000000-0000-7000-8000-000000000002".parse().unwrap()),
        );
        let b = LineKey::new(
            "page",
            Some("00000000-0000-7000-8000-000000000001".parse().unwrap()),
        );
        let bytes = jsonl_file(vec![(a, "{\"d\":1}".into()), (b, "{\"p\":1}".into())]);
        assert_eq!(bytes, b"{\"p\":1}\n{\"d\":1}\n");
        assert!(jsonl_file(Vec::new()).is_empty());
    }
}
