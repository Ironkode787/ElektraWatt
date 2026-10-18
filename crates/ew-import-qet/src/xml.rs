// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hostile-input guards around the XML parser (doc 14 §11, NFR-SEC-02).
//!
//! `roxmltree` parses nested elements recursively, using several kilobytes of stack per level in debug
//! builds, so a deeply nested input would overflow the stack before any depth check on the tree could run.
//! Its duplicate-attribute check compares every attribute of an element with every earlier one, so the
//! parse is quadratic in the attributes of one element. Two guards bound both:
//!
//! 1. [`prescan`] measures the element nesting and counts the attributes with a flat byte scan **before**
//!    parsing, so an input deeper than [`Limits::max_xml_depth`], or with more attributes than
//!    [`Limits::max_xml_attributes_per_element`] or [`Limits::max_xml_attributes`], is refused without
//!    recursion and before `roxmltree` sees it;
//! 2. [`with_parser_stack`] runs the parse (and everything that walks the tree) on a scoped thread whose
//!    stack is sized for the measured depth: 4 MiB plus 64 KiB per level.
//!
//! [`LineIndex`] turns byte offsets into line numbers by bisection, so report locations never rescan the
//! document.

use ew_interop_api::{ImportError, Limits};

/// Line numbers of byte offsets in one text: the offsets of its line feeds, sorted, searched by bisection.
///
/// Lines are counted as `roxmltree::Document::text_pos_at` counts them: 1-based, a new line after each
/// `\n` (so a CR LF pair counts once).
///
/// ```
/// # use ew_import_qet::xml::LineIndex;
/// let lines = LineIndex::new("a\nb\r\nc");
/// assert_eq!((lines.line(0), lines.line(2), lines.line(5), lines.line(99)), (1, 2, 3, 3));
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LineIndex {
    newlines: Vec<usize>,
}

impl LineIndex {
    /// The index of `text`, built in one pass.
    pub fn new(text: &str) -> LineIndex {
        LineIndex {
            newlines: text
                .bytes()
                .enumerate()
                .filter_map(|(i, b)| (b == b'\n').then_some(i))
                .collect(),
        }
    }

    /// The 1-based line of byte offset `pos`; offsets past the end are on the last line.
    pub fn line(&self, pos: usize) -> u32 {
        let before = self.newlines.partition_point(|&nl| nl < pos);
        u32::try_from(before).map_or(u32::MAX, |n| n.saturating_add(1))
    }
}

/// What [`scan`] measured, as far as it got.
struct Scan {
    /// The deepest element nesting seen.
    depth: u32,
    /// The limit that stopped the scan, if any.
    exceeded: Option<ImportError>,
}

impl Scan {
    fn stop(depth: u32, what: &str, limit: u32) -> Scan {
        Scan {
            depth,
            exceeded: Some(ImportError::LimitExceeded {
                what: what.into(),
                limit: u64::from(limit),
            }),
        }
    }
}

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
    let s = scan(xml, limit, u32::MAX, u32::MAX);
    s.exceeded.is_none().then_some(s.depth)
}

/// The flat pre-scan of `xml` against `limits`: the deepest element nesting, unless the scan exceeds
/// [`Limits::max_xml_depth`], [`Limits::max_xml_attributes_per_element`] or [`Limits::max_xml_attributes`]
/// (it stops at the first). Attributes are counted as the `=` signs outside quoted values in start tags, one
/// per attribute in well-formed XML, so the scan is linear in the input. Malformed input gives some result;
/// the parser reports it properly.
///
/// ```
/// # use ew_import_qet::xml::prescan;
/// use ew_interop_api::Limits;
///
/// let limits = Limits { max_xml_attributes_per_element: 2, ..Limits::default() };
/// assert_eq!(prescan("<a x='=' y=\"1\"><b/></a>", &limits).unwrap(), 1);
/// assert!(prescan("<a x='1' y='2' z='3'/>", &limits).is_err());
/// ```
///
/// # Errors
///
/// [`ImportError::LimitExceeded`] naming the limit the scan exceeded: `"XML depth"`, `"XML attributes per
/// element"` or `"XML attribute count"`.
pub fn prescan(xml: &str, limits: &Limits) -> Result<u32, ImportError> {
    let s = scan(
        xml,
        limits.max_xml_depth,
        limits.max_xml_attributes_per_element,
        limits.max_xml_attributes,
    );
    match s.exceeded {
        Some(e) => Err(e),
        None => Ok(s.depth),
    }
}

/// The scanner behind [`max_depth`] and [`prescan`]; stops at the first exceeded limit.
fn scan(xml: &str, max_depth: u32, max_attrs_per_element: u32, max_attrs: u32) -> Scan {
    let b = xml.as_bytes();
    let (mut i, mut depth, mut max) = (0usize, 0u32, 0u32);
    let mut total_attrs: u64 = 0;
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
            // A start tag: find its end outside quotes, counting the attributes on the way.
            let mut j = i + 1;
            let mut quote: Option<u8> = None;
            let mut attrs: u32 = 0;
            while j < b.len() {
                match (quote, b[j]) {
                    (Some(q), c) if c == q => quote = None,
                    (None, b'"' | b'\'') => quote = Some(b[j]),
                    (None, b'>') => break,
                    (None, b'=') => {
                        attrs = attrs.saturating_add(1);
                        if attrs > max_attrs_per_element {
                            return Scan::stop(max, "XML attributes per element", max_attrs_per_element);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            total_attrs += u64::from(attrs);
            if total_attrs > u64::from(max_attrs) {
                return Scan::stop(max, "XML attribute count", max_attrs);
            }
            let self_closing = j > i + 1 && b[j - 1] == b'/' && quote.is_none();
            if !self_closing {
                depth += 1;
                max = max.max(depth);
                if depth > max_depth {
                    return Scan::stop(max, "XML depth", max_depth);
                }
            }
            i = j + 1;
        }
    }
    Scan {
        depth: max,
        exceeded: None,
    }
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
    fn attribute_limits_per_element_and_document() {
        let limits = Limits {
            max_xml_attributes_per_element: 3,
            max_xml_attributes: 5,
            ..Limits::default()
        };
        let what = |r: Result<u32, ImportError>| match r {
            Err(ImportError::LimitExceeded { what, limit }) => (what, limit),
            other => panic!("{other:?}"),
        };
        // Three per element and five in all pass; '=' inside quotes or outside tags does not count.
        assert_eq!(
            prescan(
                "<a x='a=b' y=\"=\" z='1'>=<b u='1' v='2'/><!-- p='1' q='2' --></a>",
                &limits
            )
            .unwrap(),
            1
        );
        assert_eq!(
            what(prescan("<a w='1' x='1' y='1' z='1'/>", &limits)),
            ("XML attributes per element".to_owned(), 3)
        );
        assert_eq!(
            what(prescan(
                "<a x='1' y='1'><b x='1' y='1'/><c x='1' y='1'/></a>",
                &limits
            )),
            ("XML attribute count".to_owned(), 5)
        );
        let shallow = Limits {
            max_xml_depth: 1,
            ..Limits::default()
        };
        assert_eq!(
            what(prescan("<a><b/><c></c></a>", &shallow)),
            ("XML depth".to_owned(), 1)
        );
        assert_eq!(prescan("<a><b/></a>", &shallow).unwrap(), 1);
    }

    #[test]
    fn line_index_matches_roxmltree() {
        let text = "<a>\n  <b/>\r\n<c>\n\n</c>\n<d\nx='1'/></a>";
        let doc = roxmltree::Document::parse(text).unwrap();
        let lines = LineIndex::new(text);
        for node in doc.descendants() {
            let pos = node.range().start;
            assert_eq!(lines.line(pos), doc.text_pos_at(pos).row, "{node:?}");
        }
        assert_eq!(LineIndex::new("").line(0), 1);
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
