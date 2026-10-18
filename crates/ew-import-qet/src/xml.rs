// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hostile-input guards around the XML parser (doc 14 §11, NFR-SEC-02).
//!
//! `roxmltree` parses nested elements recursively, using several kilobytes of stack per level in debug
//! builds, so a deeply nested input would overflow the stack before any depth check on the tree could run.
//! Its duplicate-attribute check compares every attribute of an element with every earlier one, so the
//! parse is quadratic in the attributes of one element, and its cost over a document is the sum of the
//! squares of the per-element attribute counts. Two guards bound both:
//!
//! 1. [`prescan`] measures the element nesting, counts the attributes and elements and sums the attribute
//!    work with a flat byte scan **before** parsing, so an input deeper than [`Limits::max_xml_depth`],
//!    with more attributes than [`Limits::max_xml_attributes_per_element`] or [`Limits::max_xml_attributes`],
//!    with more attribute work than [`Limits::max_xml_attribute_work`], or with more elements and comments
//!    than [`Limits::max_xml_nodes`] allows, is refused without recursion and before `roxmltree` sees it.
//!    It also bounds the memory `roxmltree` reserves before it parses: the parser sizes its node and
//!    attribute vectors from the number of `<` and `=` bytes in the whole input (about 72 bytes each in
//!    0.21.1, so 16 million `=` in one text reserve 1.1 GB), whatever its node limit, so an input with more
//!    of them than [`markup_char_limit`] is refused;
//! 2. [`with_parser_stack`] runs the parse (and everything that walks the tree) on a scoped thread whose
//!    stack is sized for the measured depth: 4 MiB plus 64 KiB per level.
//!
//! [`LineIndex`] turns byte offsets into line numbers from a sparse checkpoint index, so report locations
//! never rescan the document and the index stays small however many lines it has.

use ew_interop_api::{ImportError, Limits};

/// Bytes between two checkpoints of a [`LineIndex`].
const LINE_STRIDE: usize = 64 * 1024;

/// Line numbers of byte offsets in one text, from a sparse index: the number of line feeds before every
/// 64 KiB boundary of the text.
///
/// The index holds one `u32` per 64 KiB of text (32 KiB for a 512 MiB input), so it stays small however many
/// lines the text has. [`LineIndex::line`] starts at the checkpoint below the offset and counts the line
/// feeds of at most 64 KiB, so a lookup costs at most 64 KiB of scanning; with at most 1,000 findings per
/// definition that is at most about 64 MB per import (doc 14 §11).
///
/// Lines are counted as `roxmltree::Document::text_pos_at` counts them: 1-based, a new line after each
/// `\n` (so a CR LF pair counts once).
///
/// ```
/// # use ew_import_qet::xml::LineIndex;
/// let text = "a\nb\r\nc";
/// let lines = LineIndex::new(text);
/// assert_eq!(
///     (lines.line(text, 0), lines.line(text, 2), lines.line(text, 5), lines.line(text, 99)),
///     (1, 2, 3, 3)
/// );
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LineIndex {
    /// Entry `k`: the line feeds in the first `k × 64 KiB` bytes of the text, saturated at `u32::MAX`.
    checkpoints: Vec<u32>,
    /// The length of the indexed text, in bytes.
    len: usize,
}

impl LineIndex {
    /// The index of `text`, built in one pass.
    pub fn new(text: &str) -> LineIndex {
        let mut checkpoints = Vec::with_capacity(text.len() / LINE_STRIDE + 1);
        checkpoints.push(0u32);
        let mut feeds = 0u32;
        for chunk in text.as_bytes().chunks(LINE_STRIDE) {
            feeds = feeds.saturating_add(newlines(chunk));
            if chunk.len() == LINE_STRIDE {
                checkpoints.push(feeds);
            }
        }
        LineIndex {
            checkpoints,
            len: text.len(),
        }
    }

    /// The 1-based line of byte offset `pos` in `text`, which must be the text the index was built from;
    /// offsets past the end are on the last line.
    pub fn line(&self, text: &str, pos: usize) -> u32 {
        debug_assert_eq!(text.len(), self.len, "LineIndex used with another text");
        let pos = pos.min(text.len()).min(self.len);
        let k = (pos / LINE_STRIDE).min(self.checkpoints.len().saturating_sub(1));
        let start = k * LINE_STRIDE;
        let before = self.checkpoints.get(k).copied().unwrap_or(0);
        let tail = text.as_bytes().get(start..pos).map_or(0, newlines);
        before.saturating_add(tail).saturating_add(1)
    }

    /// The number of checkpoints: one per 64 KiB of text, plus one.
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// The heap memory the index holds, in bytes.
    pub fn heap_bytes(&self) -> usize {
        self.checkpoints.capacity() * std::mem::size_of::<u32>()
    }
}

/// The line feeds in `bytes`, saturated at `u32::MAX`.
fn newlines(bytes: &[u8]) -> u32 {
    u32::try_from(bytes.iter().filter(|&&b| b == b'\n').count()).unwrap_or(u32::MAX)
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
        Scan::stop_u64(depth, what, u64::from(limit))
    }

    fn stop_u64(depth: u32, what: &str, limit: u64) -> Scan {
        Scan {
            depth,
            exceeded: Some(ImportError::LimitExceeded {
                what: what.into(),
                limit,
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
    let limits = Limits {
        max_xml_depth: limit,
        max_xml_nodes: u32::MAX,
        max_xml_attributes_per_element: u32::MAX,
        max_xml_attributes: u32::MAX,
        max_xml_attribute_work: u64::MAX,
        ..Limits::default()
    };
    let s = scan(xml, &limits);
    s.exceeded.is_none().then_some(s.depth)
}

/// The flat pre-scan of `xml` against `limits`: the deepest element nesting, unless the scan exceeds
/// [`Limits::max_xml_depth`], [`Limits::max_xml_attributes_per_element`], [`Limits::max_xml_attributes`],
/// [`Limits::max_xml_attribute_work`] or [`Limits::max_xml_nodes`], or has more `<` and `=` bytes than
/// [`markup_char_limit`] (it stops at the first). Attributes are
/// counted as the `=` signs outside quoted values in start tags, one per attribute in well-formed XML; the
/// attribute work is the sum over elements of the square of that count. The node count is a lower bound of
/// what `roxmltree` counts: the document root, the elements and the comments (texts and processing
/// instructions are left to the parser's own limit), so an input refused here would be refused by the parser
/// too, only later and after allocating its tree. The scan is linear in the input. Malformed input gives
/// some result; the parser reports it properly.
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
/// element"`, `"XML attribute count"`, `"XML attribute work"`, `"XML node count"` or `"XML markup
/// characters"`.
pub fn prescan(xml: &str, limits: &Limits) -> Result<u32, ImportError> {
    let s = scan(xml, limits);
    match s.exceeded {
        Some(e) => Err(e),
        None => Ok(s.depth),
    }
}

/// The most `<` and `=` bytes the pre-scan accepts in one input: two per node (a start and an end tag) plus
/// one per attribute, `2 × max_xml_nodes + max_xml_attributes` (3,048,576 by default).
///
/// `roxmltree` reserves one node per `<` and one attribute per `=` of the whole input before parsing (about
/// 72 bytes each in 0.21.1), including those in comments, texts and attribute values, and before its node
/// limit applies. A document within the node and attribute limits needs no more, and at the default the
/// reservation stays near 210 MB, within the 256 MB of doc 14 §11.
///
/// ```
/// # use ew_import_qet::xml::markup_char_limit;
/// assert_eq!(markup_char_limit(&ew_interop_api::Limits::default()), 3_048_576);
/// ```
pub fn markup_char_limit(limits: &Limits) -> u64 {
    u64::from(limits.max_xml_nodes)
        .saturating_mul(2)
        .saturating_add(u64::from(limits.max_xml_attributes))
}

/// The scanner behind [`max_depth`] and [`prescan`]; stops at the first exceeded limit.
fn scan(xml: &str, limits: &Limits) -> Scan {
    let max_depth = limits.max_xml_depth;
    let max_attrs_per_element = limits.max_xml_attributes_per_element;
    let b = xml.as_bytes();
    let markup_limit = markup_char_limit(limits);
    let markup = b.iter().filter(|&&c| c == b'<' || c == b'=').count() as u64;
    if markup > markup_limit {
        return Scan::stop_u64(0, "XML markup characters", markup_limit);
    }
    let (mut i, mut depth, mut max) = (0usize, 0u32, 0u32);
    let mut total_attrs: u64 = 0;
    let mut work: u64 = 0;
    // The document root is a node.
    let mut nodes: u64 = 1;
    let mut count_node = |max: u32| -> Option<Scan> {
        nodes += 1;
        (nodes > u64::from(limits.max_xml_nodes))
            .then(|| Scan::stop(max, "XML node count", limits.max_xml_nodes))
    };
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
            if let Some(stop) = count_node(max) {
                return stop;
            }
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
            if total_attrs > u64::from(limits.max_xml_attributes) {
                return Scan::stop(max, "XML attribute count", limits.max_xml_attributes);
            }
            work = work.saturating_add(u64::from(attrs) * u64::from(attrs));
            if work > limits.max_xml_attribute_work {
                return Scan::stop_u64(max, "XML attribute work", limits.max_xml_attribute_work);
            }
            if let Some(stop) = count_node(max) {
                return stop;
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
            assert_eq!(lines.line(text, pos), doc.text_pos_at(pos).row, "{node:?}");
        }
        assert_eq!(LineIndex::new("").line("", 0), 1);
    }

    #[test]
    fn sparse_line_index_matches_a_full_scan_across_checkpoints() {
        // Line feeds just before, on and after the 64 KiB boundaries, a CR LF across one, and long runs
        // without any.
        let mut text = String::new();
        let mut i = 0usize;
        while text.len() < 3 * LINE_STRIDE + 1_000 {
            let near = text.len() % LINE_STRIDE;
            text.push(if near == 0 || near == LINE_STRIDE - 1 || i % 977 == 0 {
                '\n'
            } else if near == LINE_STRIDE - 2 {
                '\r'
            } else {
                'x'
            });
            i += 1;
        }
        let lines = LineIndex::new(&text);
        assert_eq!(lines.checkpoint_count(), text.len() / LINE_STRIDE + 1);
        // The reference: the line of every offset by a full scan.
        let mut reference = vec![1u32];
        for &b in text.as_bytes() {
            let last = *reference.last().unwrap();
            reference.push(last + u32::from(b == b'\n'));
        }
        let full = |pos: usize| reference[pos.min(text.len())];
        let probes = (0..text.len() + 3)
            .step_by(97)
            .chain((1..=3).flat_map(|k| (k * LINE_STRIDE - 3)..=(k * LINE_STRIDE + 3)));
        for pos in probes.chain([text.len(), text.len() + 10]) {
            assert_eq!(lines.line(&text, pos), full(pos), "offset {pos}");
        }
        // A text of exactly one stride has a checkpoint at its end.
        let exact = "\n".repeat(LINE_STRIDE);
        let lines = LineIndex::new(&exact);
        assert_eq!(lines.checkpoint_count(), 2);
        assert_eq!(
            lines.line(&exact, LINE_STRIDE),
            u32::try_from(LINE_STRIDE).unwrap() + 1
        );
        assert_eq!(
            lines.line(&exact, LINE_STRIDE - 1),
            u32::try_from(LINE_STRIDE).unwrap()
        );
    }

    #[test]
    fn line_index_is_sparse() {
        // 8 MiB of line feeds: one checkpoint per 64 KiB, not one entry per line.
        let text = "\n".repeat(8 << 20);
        let lines = LineIndex::new(&text);
        assert_eq!(lines.checkpoint_count(), 129);
        assert!(lines.heap_bytes() <= 129 * 4 + 64, "{}", lines.heap_bytes());
        assert_eq!(lines.line(&text, text.len()), (8 << 20) + 1);
    }

    #[test]
    fn attribute_work_is_the_sum_of_squares() {
        let limits = Limits {
            max_xml_attribute_work: 3 * 3 + 2 * 2,
            ..Limits::default()
        };
        // 3² + 2² = 13 passes; one more attribute anywhere exceeds it.
        assert!(prescan("<a x='1' y='2' z='3'><b u='1' v='2'/></a>", &limits).is_ok());
        assert_eq!(
            prescan("<a x='1' y='2' z='3'><b u='1' v='2'/><c w='1'/></a>", &limits),
            Err(ImportError::LimitExceeded {
                what: "XML attribute work".into(),
                limit: 13
            })
        );
        assert!(prescan("<a x='1' y='2' z='3' w='4'/>", &limits).is_err());
        // One element at the default per-element cap is exactly the default work.
        let d = Limits::default();
        assert_eq!(
            u64::from(d.max_xml_attributes_per_element).pow(2),
            d.max_xml_attribute_work
        );
    }

    #[test]
    fn node_count_is_a_lower_bound_of_the_parser_count() {
        let limits = Limits {
            max_xml_nodes: 5,
            ..Limits::default()
        };
        let what = |r: Result<u32, ImportError>| match r {
            Err(ImportError::LimitExceeded { what, limit }) => (what, limit),
            other => panic!("{other:?}"),
        };
        let parse = |xml: &str| {
            roxmltree::Document::parse_with_options(
                xml,
                roxmltree::ParsingOptions {
                    nodes_limit: limits.max_xml_nodes,
                    ..roxmltree::ParsingOptions::default()
                },
            )
            .map(|_| ())
        };
        // The root, three elements and a comment: five nodes, accepted by both.
        let at = "<?xml version='1.0'?><a><b/><!-- c --><c></c></a>";
        assert!(prescan(at, &limits).is_ok());
        assert!(parse(at).is_ok());
        // One more element or comment: refused by both, by the scan first.
        for over in [
            "<a><b/><!-- c --><c></c><d/></a>",
            "<a><b/><!-- c --><c></c><!--d--></a>",
        ] {
            assert_eq!(what(prescan(over, &limits)), ("XML node count".to_owned(), 5));
            assert!(matches!(parse(over), Err(roxmltree::Error::NodesLimitReached)));
        }
        // Texts are left to the parser: the scan passes, the parser refuses.
        let texts = "<a>t<b/>u<c/>v</a>";
        assert!(prescan(texts, &limits).is_ok());
        assert!(matches!(parse(texts), Err(roxmltree::Error::NodesLimitReached)));
    }

    #[test]
    fn markup_characters_that_the_parser_reserves_for_are_bounded() {
        let limits = Limits {
            max_xml_nodes: 4,
            max_xml_attributes: 2,
            ..Limits::default()
        };
        assert_eq!(markup_char_limit(&limits), 10);
        // Ten `<` and `=` in all pass, wherever they are (seven `<`, two attribute `=` and one in a value);
        // one more in a comment, a text or an attribute value is refused before the structure is looked at.
        let at = "<a x='1' y='='><b/><!--<<<--></a>";
        assert_eq!(prescan(at, &limits), Ok(1));
        for over in [
            "<a x='1' y='='><b/><!--<<<<--></a>",
            "<a x='1' y='='><b/><!--<<<-->=</a>",
            "<a x='1' y='=='><b/><!--<<<--></a>",
        ] {
            assert_eq!(
                prescan(over, &limits),
                Err(ImportError::LimitExceeded {
                    what: "XML markup characters".into(),
                    limit: 10
                }),
                "{over}"
            );
        }
        // `max_depth` sets no such bound.
        assert_eq!(max_depth(&format!("<a>{}</a>", "=".repeat(100)), 5), Some(1));
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
