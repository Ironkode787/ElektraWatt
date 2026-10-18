// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hostile-input guards around the XML parser (doc 14 §11, NFR-SEC-02).
//!
//! `roxmltree` parses nested elements recursively, using several kilobytes of stack per level in debug
//! builds, so a deeply nested input would overflow the stack before any depth check on the tree could run.
//! Two guards prevent that:
//!
//! 1. [`max_depth`] measures the element nesting with a flat byte scan **before** parsing, so an input
//!    deeper than [`Limits::max_xml_depth`](ew_interop_api::Limits) is refused without recursion;
//! 2. [`with_parser_stack`] runs the parse (and everything that walks the tree) on a scoped thread whose
//!    stack is sized for the measured depth: 4 MiB plus 64 KiB per level.

use ew_interop_api::ImportError;

/// The deepest element nesting of `xml` by a flat scan, or `None` if it exceeds `limit` (the scan stops
/// there). Comments, CDATA sections, processing instructions and declarations are skipped; quoted attribute
/// values may contain `>` and `/`. Malformed input gives some number; the parser reports it properly.
///
/// ```
/// # use ew_import_qet::xml::max_depth;
/// assert_eq!(max_depth("<a><b/><c x='/>'><!-- <d> --></c></a>", 10), Some(2));
/// assert_eq!(max_depth("<a><b><c/></b></a>", 1), None);
/// ```
pub fn max_depth(xml: &str, limit: u32) -> Option<u32> {
    let b = xml.as_bytes();
    let (mut i, mut depth, mut max) = (0usize, 0u32, 0u32);
    let skip_to = |from: usize, end: &[u8]| -> usize {
        b[from..]
            .windows(end.len())
            .position(|w| w == end)
            .map_or(b.len(), |p| from + p + end.len())
    };
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &b[i..];
        if rest.starts_with(b"<!--") {
            i = skip_to(i + 4, b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            i = skip_to(i + 9, b"]]>");
        } else if rest.starts_with(b"<?") {
            i = skip_to(i + 2, b"?>");
        } else if rest.starts_with(b"<!") {
            i = skip_to(i + 2, b">");
        } else if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            i = skip_to(i + 2, b">");
        } else {
            // A start tag: find its end outside quotes.
            let mut j = i + 1;
            let mut quote: Option<u8> = None;
            while j < b.len() {
                match (quote, b[j]) {
                    (Some(q), c) if c == q => quote = None,
                    (None, b'"' | b'\'') => quote = Some(b[j]),
                    (None, b'>') => break,
                    _ => {}
                }
                j += 1;
            }
            let self_closing = j > i + 1 && b[j - 1] == b'/' && quote.is_none();
            if !self_closing {
                depth += 1;
                max = max.max(depth);
                if depth > limit {
                    return None;
                }
            }
            i = j + 1;
        }
    }
    Some(max)
}

/// Runs `f` on a scoped thread with a stack for XML of nesting `depth` (4 MiB + 64 KiB per level).
///
/// # Errors
///
/// [`ImportError::Unsupported`] if the thread cannot be started; otherwise whatever `f` returns. A panic in
/// `f` is resumed on the caller's thread.
pub fn with_parser_stack<T: Send>(
    depth: u32,
    f: impl FnOnce() -> Result<T, ImportError> + Send,
) -> Result<T, ImportError> {
    let stack = (4 << 20) + (depth as usize) * (64 << 10);
    std::thread::scope(|s| {
        let handle = std::thread::Builder::new()
            .name("ew-import-qet-xml".into())
            .stack_size(stack)
            .spawn_scoped(s, f)
            .map_err(|e| ImportError::Unsupported {
                message: format!("cannot start the XML parser thread: {e}"),
            })?;
        handle.join().unwrap_or_else(|p| std::panic::resume_unwind(p))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depths() {
        assert_eq!(max_depth("", 5), Some(0));
        assert_eq!(max_depth("<?xml version=\"1.0\"?><a/>", 5), Some(0));
        assert_eq!(max_depth("<a></a>", 5), Some(1));
        assert_eq!(max_depth("<a><![CDATA[<b><c>]]><b>t</b></a>", 5), Some(2));
        assert_eq!(max_depth("<a x=\"1>2\" y='/'><b/></a>", 5), Some(1));
        let deep = format!("{}{}", "<a>".repeat(100_000), "</a>".repeat(100_000));
        assert_eq!(max_depth(&deep, 256), None);
        assert_eq!(max_depth(&deep, 100_000), Some(100_000));
    }

    #[test]
    fn deep_parse_on_the_big_stack() {
        let d = 256;
        let deep = format!("<r>{}{}</r>", "<a>".repeat(d - 1), "</a>".repeat(d - 1));
        let depth = max_depth(&deep, 256).unwrap();
        assert_eq!(depth, 256);
        let n = with_parser_stack(depth, || {
            let doc = roxmltree::Document::parse(&deep).map_err(|e| ImportError::Malformed {
                message: e.to_string(),
                line: None,
            })?;
            Ok(doc.descendants().count())
        })
        .unwrap();
        assert_eq!(n, 257); // the document root plus 256 elements
    }
}
