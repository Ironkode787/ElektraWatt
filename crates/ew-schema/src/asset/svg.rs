// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The plain rejecter of SVG assets ([`svg_hazard`], doc 18 §11 TM-18): what [`super::sniff`] refuses in an SVG
//! before any parser or renderer reads it, and why ([`SvgHazard`]).

/// Why an SVG asset is refused before any parser or renderer reads it ([`svg_hazard`];, doc 18 §11 TM-18).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SvgHazard {
    /// The file is not UTF-8 text.
    NotUtf8,
    /// A DOCTYPE with an internal subset (`<!DOCTYPE svg [ ... ]>`): entity expansion, external entities.
    DoctypeSubset,
    /// An entity declaration (`<!ENTITY`).
    Entity,
    /// A script: a `script`, `handler` or `listener` element, or a `javascript:` URL.
    Script,
    /// An event attribute (`onload`, `onclick`, any attribute whose name begins with `on`).
    EventAttribute,
    /// A `foreignObject` element (HTML inside the drawing).
    ForeignObject,
    /// A reference that leaves the document: an `href` that is not a fragment (`#id`) of this document (an external
    /// file, a URL, an embedded `data:` resource), a CSS `url()` or `@import` that is not a fragment, an
    /// `xml-stylesheet` processing instruction, or an `iframe`, `embed`, `object` or XInclude `include` element.
    ExternalReference,
    /// Markup the scanner cannot follow (an unclosed comment, CDATA section, processing instruction, tag or
    /// attribute value, an unknown declaration): refused rather than guessed.
    Malformed,
    /// Elements the parser does not survive: nesting deeper than [`MAX_SVG_DEPTH`], or a chain of references longer
    /// than [`MAX_SVG_REF_DEPTH`] or a cycle of them.
    TooComplex,
    /// A number beyond [`MAX_SVG_NUMBER`] in an attribute or a style sheet.
    HugeNumber,
    /// A construct that panics the parser (`transform-origin`, `feConvolveMatrix`).
    Unsupported,
}

impl SvgHazard {
    /// What the hazard is, in words.
    pub const fn describe(self) -> &'static str {
        match self {
            SvgHazard::NotUtf8 => "it is not UTF-8 text",
            SvgHazard::DoctypeSubset => "it has a DOCTYPE with an internal subset",
            SvgHazard::Entity => "it declares an entity",
            SvgHazard::Script => "it has a script",
            SvgHazard::EventAttribute => "it has an event attribute",
            SvgHazard::ForeignObject => "it has a foreignObject element",
            SvgHazard::ExternalReference => "it references something outside the document",
            SvgHazard::Malformed => "its markup cannot be followed",
            SvgHazard::TooComplex => {
                "its elements nest too deep or reference each other in too long a chain or a circle"
            }
            SvgHazard::HugeNumber => "it has a number too large to draw",
            SvgHazard::Unsupported => "it uses transform-origin or feConvolveMatrix",
        }
    }

    /// How to export the file again so that it passes, in words.
    pub const fn advice(self) -> &'static str {
        match self {
            SvgHazard::NotUtf8 | SvgHazard::Malformed => "export it again as a plain SVG in UTF-8",
            SvgHazard::DoctypeSubset | SvgHazard::Entity => {
                "this is the editing data Illustrator writes with \"Preserve Illustrator Editing Capabilities\": save \
                 the SVG again with that option off (File > Save As, SVG Options), or save it from Inkscape as \
                 \"Plain SVG\""
            }
            SvgHazard::Script | SvgHazard::EventAttribute => {
                "a drawing's image never runs code: remove the scripts and interactivity, then export it again as a \
                 plain SVG"
            }
            SvgHazard::ForeignObject => {
                "it holds HTML: export it again as a plain SVG, in Illustrator with \"Preserve Illustrator Editing \
                 Capabilities\" off, in draw.io after selecting all labels and clearing \"Formatted Text\" and \
                 \"Word Wrap\" (Text tab, Advanced) before File > Export as > SVG"
            }
            SvgHazard::ExternalReference => {
                "remove linked or embedded images and fonts (convert text to outlines) and export it again as a plain \
                 SVG; add a raster picture as its own PNG or JPEG file"
            }
            SvgHazard::TooComplex => {
                "simplify the drawing (fewer nested groups and clones of clones) and export it again as a plain SVG \
                 (Inkscape: \"Plain SVG\")"
            }
            SvgHazard::HugeNumber => {
                "scale the drawing to a normal page size, keep filter values small, and export it again as a plain SVG"
            }
            SvgHazard::Unsupported => {
                "flatten the transforms and remove convolution filters, then export it again as a plain SVG"
            }
        }
    }
}

/// The deepest element nesting an SVG asset may have: the parser recurses once per level, so a deeper document
/// would overflow its stack.
pub const MAX_SVG_DEPTH: usize = 64;
/// The longest chain of references an SVG asset may have (`use` of an element that holds a `use`, a gradient that
/// takes its stops from another, a clip path whose content references a mask, and so on): the parser recurses or
/// walks once per link, and walks a cycle of `href` links that returns to neither end for ever.
pub const MAX_SVG_REF_DEPTH: usize = 32;
/// The largest magnitude of a number in an attribute or a style sheet of an SVG asset (huge arc radii
/// and filter values make the parser allocate without bound or panic).
pub const MAX_SVG_NUMBER: f64 = 1.0e9;

/// The first reason to refuse an SVG file before any parser or renderer reads it, or `None` (doc 05 §13.3,
/// doc 18 §11 TM-18): the plain rejecter. Sniffing is not sanitising: the sniffer reads only the root element and
/// [`sniff`](super::sniff) caps the size ([`MAX_SVG_BYTES`](super::MAX_SVG_BYTES)) and the elements
/// ([`MAX_SVG_NODES`](super::MAX_SVG_NODES)), so this reader reads the whole
/// document as markup (comments skipped, CDATA read as text, character references decoded as the parser decodes
/// them) and refuses, conservatively (ASCII case is ignored, so `<SCRIPT` counts):
///
/// - a DOCTYPE with an internal subset, and any entity declaration ([`SvgHazard::DoctypeSubset`],
///   [`SvgHazard::Entity`]); a DOCTYPE with only an external identifier (`PUBLIC "-//W3C//DTD SVG 1.1//EN" "…"`, as
///   drawing programs write it) passes, because no reader of EW fetches it;
/// - `script`, `handler` and `listener` elements and `javascript:` text ([`SvgHazard::Script`]);
/// - attributes whose local name begins with `on` ([`SvgHazard::EventAttribute`]); namespace declarations
///   (`xmlns`, `xmlns:*`) are not attributes of the element and are skipped;
/// - `foreignObject` elements ([`SvgHazard::ForeignObject`]);
/// - every `href` (any prefix) that is not a fragment `#…` of the document, CSS `url(…)` that is not a fragment and
///   `@import`, in attributes and in text (style sheets), `xml-stylesheet` processing instructions, and `iframe`,
///   `embed`, `object` and `include` elements ([`SvgHazard::ExternalReference`]); an embedded `data:` image counts
///   as such a reference (EW draws an SVG asset's paths, not images inside it);
/// - text that is not UTF-8, and markup it cannot follow ([`SvgHazard::NotUtf8`], [`SvgHazard::Malformed`]);
/// - the elements beyond what the parser survives ([`SvgHazard::TooComplex`]): nesting deeper than [`MAX_SVG_DEPTH`],
///   and a chain of references longer than [`MAX_SVG_REF_DEPTH`] or a cycle of them, the references being the
///   `href="#…"` and `url(#…)` of an element's attributes, each counted from the element and from every ancestor
///   with an id;
/// - a number beyond [`MAX_SVG_NUMBER`] in an attribute or a style sheet ([`SvgHazard::HugeNumber`]), where a hex
///   colour (`#D63E25`) and a fragment name (`url(#g1e30)`) are not numbers;
/// - `transform-origin` and `feConvolveMatrix`, which panic the parser ([`SvgHazard::Unsupported`]).
///
/// What it lets through costs time and memory only while the drawing is made, with the parse inside `catch_unwind`:
/// the parser stops expanding references at a million nodes, and what a style sheet, markers, a reference made in a
/// style sheet or long path data cost is not bounded beyond the caps (doc 18 §11 TM-18).
///
/// Linear in the length of the file.
pub fn svg_hazard(bytes: &[u8]) -> Option<SvgHazard> {
    let mut scan = Plain::default();
    read_markup(bytes, &mut scan).or_else(|| scan.finish())
}

/// The plain rejecter's markup reader: the first refusal while reading, with the document's references left in
/// `scan`.
fn read_markup(bytes: &[u8], scan: &mut Plain) -> Option<SvgHazard> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Some(SvgHazard::NotUtf8);
    };
    // ASCII lower case keeps every byte offset.
    let lower = text.to_ascii_lowercase();
    if lower.contains("<!entity") {
        return Some(SvgHazard::Entity);
    }
    let b = lower.as_bytes();
    let find = |from: usize, needle: &str| lower[from..].find(needle).map(|k| from + k);
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            let end = find(i, "<").unwrap_or(b.len());
            // The parser decodes character references in character data: check what it will read.
            let Some(text) = decode_refs(&lower[i..end]) else {
                return Some(SvgHazard::Malformed);
            };
            if let Some(h) = scan.text(&text) {
                return Some(h);
            }
            i = end;
            continue;
        }
        let rest = &lower[i..];
        if rest.starts_with("<!--") {
            let Some(end) = find(i + 4, "-->") else {
                return Some(SvgHazard::Malformed);
            };
            i = end + 3;
            continue;
        }
        if rest.starts_with("<![cdata[") {
            let Some(end) = find(i + 9, "]]>") else {
                return Some(SvgHazard::Malformed);
            };
            if let Some(h) = scan.text(&lower[i + 9..end]) {
                return Some(h);
            }
            i = end + 3;
            continue;
        }
        if rest.starts_with("<!doctype") {
            let mut j = i + 9;
            let mut quote = None;
            loop {
                let Some(&c) = b.get(j) else {
                    return Some(SvgHazard::Malformed);
                };
                match (quote, c) {
                    (Some(q), c) if c == q => quote = None,
                    (Some(_), _) => {}
                    (None, b'"' | b'\'') => quote = Some(c),
                    (None, b'[') => return Some(SvgHazard::DoctypeSubset),
                    (None, b'>') => break,
                    (None, _) => {}
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        if rest.starts_with("<!") {
            return Some(SvgHazard::Malformed);
        }
        if rest.starts_with("<?") {
            let Some(end) = find(i + 2, "?>") else {
                return Some(SvgHazard::Malformed);
            };
            let target = lower[i + 2..end]
                .split(|c: char| c.is_whitespace())
                .next()
                .unwrap_or("");
            if target == "xml-stylesheet" {
                return Some(SvgHazard::ExternalReference);
            }
            i = end + 2;
            continue;
        }
        if rest.starts_with("</") {
            let Some(end) = find(i, ">") else {
                return Some(SvgHazard::Malformed);
            };
            if let Some(h) = scan.close() {
                return Some(h);
            }
            i = end + 1;
            continue;
        }
        // A start tag: its name, then its attributes up to `>`.
        let mut j = i + 1;
        while j < b.len() && !(b[j].is_ascii_whitespace() || b[j] == b'>' || b[j] == b'/') {
            j += 1;
        }
        let name = local_name(&lower[i + 1..j]);
        match name {
            "script" | "handler" | "listener" => return Some(SvgHazard::Script),
            "foreignobject" => return Some(SvgHazard::ForeignObject),
            "iframe" | "embed" | "object" | "include" => return Some(SvgHazard::ExternalReference),
            "feconvolvematrix" => return Some(SvgHazard::Unsupported),
            _ => {}
        }
        let mut attrs: Vec<(&str, String)> = Vec::new();
        let mut self_closing = false;
        loop {
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            match b.get(j) {
                None => return Some(SvgHazard::Malformed),
                Some(b'>') => {
                    j += 1;
                    break;
                }
                Some(b'/') => {
                    self_closing = true;
                    j += 1;
                    continue;
                }
                Some(_) => self_closing = false,
            }
            let start = j;
            while j < b.len() && !(b[j].is_ascii_whitespace() || matches!(b[j], b'=' | b'>' | b'/')) {
                j += 1;
            }
            let attr = &lower[start..j];
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let mut value = "";
            if b.get(j) == Some(&b'=') {
                j += 1;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                let Some(&q) = b.get(j).filter(|q| matches!(q, b'"' | b'\'')) else {
                    return Some(SvgHazard::Malformed);
                };
                let Some(end) = lower[j + 1..].find(char::from(q)).map(|k| j + 1 + k) else {
                    return Some(SvgHazard::Malformed);
                };
                value = &text[j + 1..end];
                j = end + 1;
            }
            // XML normalizes literal attribute whitespace before decoding references, as the parser reads the value.
            let Some(value) = decode_attribute_refs(value) else {
                return Some(SvgHazard::Malformed);
            };
            // roxmltree keeps namespace declarations out of `attributes()` and usvg copies only None/SVG/XLink/XML
            // attributes, so `xmlns` and `xmlns:*` are not attributes of the element.
            if attr == "xmlns" || attr.starts_with("xmlns:") {
                continue;
            }
            let value = value.to_ascii_lowercase();
            if let Some(h) = attribute_hazard(attr, &value) {
                return Some(h);
            }
            attrs.push((local_name(attr), value));
        }
        if let Some(h) = scan.open(name, &attrs, self_closing) {
            return Some(h);
        }
        i = j;
    }
    None
}

/// What the plain rejecter keeps of a document while it reads it: the open elements, the text of an open `style`
/// element and the references between the elements with an id.
#[derive(Default)]
struct Plain {
    /// The open elements, innermost last: the local name and the node of an element with an id.
    open: Vec<(String, Option<usize>)>,
    /// The text of the open `style` element (roxmltree merges its chunks into one text node).
    style: String,
    /// The elements with an id or a reference, in document order: each with its id and the ids it references, itself
    /// or (for an element with an id) a descendant.
    nodes: Vec<(Option<String>, Vec<String>)>,
}

impl Plain {
    /// A start tag `name` with its attributes (lower case).
    fn open(&mut self, name: &str, attrs: &[(&str, String)], self_closing: bool) -> Option<SvgHazard> {
        if self.open.len() >= MAX_SVG_DEPTH {
            return Some(SvgHazard::TooComplex);
        }
        for (attr, value) in attrs {
            if *attr == "transform-origin" || (*attr == "style" && value.contains("transform-origin")) {
                return Some(SvgHazard::Unsupported);
            }
            if let (false, Some(h)) = (matches!(*attr, "id" | "class" | "href"), numbers(value)) {
                return Some(h);
            }
        }
        let id = attrs
            .iter()
            .find(|(a, _)| *a == "id")
            .map(|(_, id)| id.trim().to_owned())
            .filter(|id| !id.is_empty());
        let mut targets: Vec<String> = Vec::new();
        for (attr, value) in attrs {
            if *attr == "href" {
                if let Some(target) = value.trim().strip_prefix('#') {
                    targets.push(target.trim().to_owned());
                }
            }
            targets.extend(url_targets(value));
        }
        // A reference made inside an element is made by every ancestor with an id too: the parser converts the
        // referenced content as part of each of them.
        for k in self.open.iter().filter_map(|(_, n)| *n) {
            self.nodes[k].1.extend(targets.iter().cloned());
        }
        let has_id = id.is_some();
        let node = (has_id || !targets.is_empty()).then(|| {
            self.nodes.push((id, targets));
            self.nodes.len() - 1
        });
        if !self_closing {
            self.open.push((name.to_owned(), node.filter(|_| has_id)));
        }
        None
    }

    /// An end tag: the innermost element closes; a closed `style` element's whole text is checked once.
    fn close(&mut self) -> Option<SvgHazard> {
        match self.open.pop() {
            Some((name, _)) if name == "style" => {
                let text = std::mem::take(&mut self.style);
                text_hazard(&text)
                    .or_else(|| {
                        text.contains("transform-origin")
                            .then_some(SvgHazard::Unsupported)
                    })
                    .or_else(|| numbers(&text))
            }
            _ => None,
        }
    }

    /// Character data: inside `style` it is kept for [`Plain::close`]; elsewhere the text checks.
    fn text(&mut self, text: &str) -> Option<SvgHazard> {
        if self.open.last().is_some_and(|(name, _)| name == "style") {
            self.style.push_str(text);
            return None;
        }
        text_hazard(text)
    }

    /// The checks that need the whole document: no chain of references longer than [`MAX_SVG_REF_DEPTH`] links and no
    /// cycle of them, from any element. Depth-first with the longest chain from each node kept, so linear
    /// in the references; it never descends deeper than the bound.
    fn finish(self) -> Option<SvgHazard> {
        // The parser resolves a reference to an element with the id; with several, any may be the one.
        let mut by_id: std::collections::BTreeMap<&str, Vec<usize>> = std::collections::BTreeMap::new();
        for (k, (id, _)) in self.nodes.iter().enumerate() {
            if let Some(id) = id {
                by_id.entry(id.as_str()).or_default().push(k);
            }
        }
        // The longest chain from each node; `None` while the node is on the walk's path.
        let mut longest: Vec<Option<Option<usize>>> = vec![None; self.nodes.len()];
        fn walk(
            k: usize,
            depth: usize,
            nodes: &[(Option<String>, Vec<String>)],
            by_id: &std::collections::BTreeMap<&str, Vec<usize>>,
            longest: &mut [Option<Option<usize>>],
        ) -> Option<usize> {
            match longest[k] {
                Some(Some(n)) => return Some(n),
                Some(None) => return None,
                None => {}
            }
            if depth > MAX_SVG_REF_DEPTH {
                return None;
            }
            longest[k] = Some(None);
            let mut best = 0;
            for target in &nodes[k].1 {
                for &next in by_id.get(target.as_str()).into_iter().flatten() {
                    best = best.max(walk(next, depth + 1, nodes, by_id, longest)? + 1);
                }
            }
            longest[k] = Some(Some(best));
            Some(best)
        }
        for k in 0..self.nodes.len() {
            match walk(k, 0, &self.nodes, &by_id, &mut longest) {
                Some(n) if n <= MAX_SVG_REF_DEPTH => {}
                _ => return Some(SvgHazard::TooComplex),
            }
        }
        None
    }
}

/// `text` with its character references (`&#48;`, `&#x30;`) and the five predefined entities (`&lt;`, `&gt;`,
/// `&amp;`, `&quot;`, `&apos;`) decoded, in ASCII lower case, as the parser will read it; `None` for any other
/// reference or one that names no character (the parser refuses those).
fn decode_refs(text: &str) -> Option<String> {
    let mut value = decode_refs_exact(text)?;
    value.make_ascii_lowercase();
    Some(value)
}

/// XML attribute-value normalization: CRLF is one space and literal CR/LF/TAB become spaces, before reference
/// decoding. Numeric references preserve their characters; stylesheet character data uses `decode_refs` instead.
fn decode_attribute_refs(text: &str) -> Option<String> {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                normalized.push(' ');
            }
            '\n' | '\t' => normalized.push(' '),
            _ => normalized.push(c),
        }
    }
    decode_refs_exact(&normalized)
}

/// Decode references without normalising case or literal whitespace.
fn decode_refs_exact(text: &str) -> Option<String> {
    if !text.contains('&') {
        return Some(text.to_owned());
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(k) = rest.find('&') {
        out.push_str(&rest[..k]);
        rest = &rest[k + 1..];
        let end = rest.find(';').filter(|e| *e <= 12)?;
        let name = &rest[..end];
        let c = match name {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = match name.strip_prefix("#x") {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => name.strip_prefix('#')?.parse::<u32>().ok()?,
                };
                char::from_u32(code)?
            }
        };
        out.push(c);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// The target of the funcIRI whose `url(` ends at `start` and the index to continue a scan from, read as
/// svgtypes' `parse_func_iri` reads it: white space is skipped, then a quoted target runs to the next same quote
/// (or to the end of the text) and keeps its inner white space, while an unquoted target runs to the next `)` (or
/// to the end); either is trimmed of ASCII white space. The index is past the closing quote, `)` or the whole text,
/// so a scan that takes it advances monotonically.
fn func_iri_target(text: &str, start: usize) -> (&str, usize) {
    let b = text.as_bytes();
    let mut i = start;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    let end = if matches!(b.get(i), Some(b'"' | b'\'')) {
        let quote = b[i];
        i += 1;
        text[i..].find(char::from(quote)).map(|k| i + k)
    } else {
        text[i..].find(')').map(|k| i + k)
    };
    let target = &text[i..end.unwrap_or(text.len())];
    (target.trim_ascii(), end.map_or(text.len(), |end| end + 1))
}

/// The ids of the `url(#id)` references in `text` (lower case): a quoted id is kept whole, inner white space
/// included, as svgtypes reads it.
fn url_targets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(k) = text[at..].find("url(") {
        let (target, next) = func_iri_target(text, at + k + 4);
        if let Some(id) = target.strip_prefix('#') {
            out.push(id.to_owned());
        }
        at = next;
    }
    out
}

/// [`SvgHazard::HugeNumber`] when `text` has a number beyond [`MAX_SVG_NUMBER`] (or one that is not finite): a
/// number begins with a digit or a `.` that does not follow a digit, a `.`, `_` or `#` (a colour). A letter before it
/// does not hide it: path data writes a command letter right before its number (`A1e30`); the attributes that hold
/// names (`id`, `class`, `href`) are not read as numbers. A `#` token (a hex colour or a fragment name) is skipped
/// whole first, so its hex digits (`#D63E25`) are not read as an exponent and a `.` right after it (`#000.5e30`)
/// still starts a number. A number is read with the SVG grammar (digits, at most one `.` with digits, an optional
/// exponent), and a digit or a `.` right after it starts the next number, so the compressed `0.5.5e39` is `0.5`
/// and `.5e39` (a `.` after a number is a boundary like the position after a `#` token).
fn numbers(text: &str) -> Option<SvgHazard> {
    let b = text.as_bytes();
    let mut i = 0;
    let mut boundary = usize::MAX;
    while i < b.len() {
        if b[i] == b'#' {
            let mut j = i + 1;
            while j < b.len()
                && (b[j].is_ascii_alphabetic() || b[j].is_ascii_digit() || matches!(b[j], b'_' | b'-'))
            {
                j += 1;
            }
            i = j;
            boundary = j;
            continue;
        }
        let c = b[i];
        let starts = (c.is_ascii_digit() || c == b'.')
            && (i == 0
                || i == boundary
                || !(b[i - 1].is_ascii_digit() || matches!(b[i - 1], b'_' | b'#' | b'.')));
        if !starts {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j < b.len() && b[j] == b'.' {
            j += 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
        }
        if j < b.len() && b[j] == b'e' {
            let mut k = j + 1;
            if k < b.len() && matches!(b[k], b'+' | b'-') {
                k += 1;
            }
            if k < b.len() && b[k].is_ascii_digit() {
                while k < b.len() && b[k].is_ascii_digit() {
                    k += 1;
                }
                j = k;
            }
        }
        if let Ok(v) = text[i..j].parse::<f64>() {
            if !v.is_finite() || v.abs() > MAX_SVG_NUMBER {
                return Some(SvgHazard::HugeNumber);
            }
        }
        // The position after a number is a boundary: a `.` or a digit there starts the next number.
        boundary = j;
        i = j.max(i + 1);
    }
    None
}

/// The local part of an XML name (`xlink:href` gives `href`).
fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// The hazard of one attribute (lower-case name and value), if any.
fn attribute_hazard(name: &str, value: &str) -> Option<SvgHazard> {
    let local = local_name(name);
    if local.starts_with("on") {
        return Some(SvgHazard::EventAttribute);
    }
    if local == "href" && !value.trim_start().starts_with('#') {
        return Some(SvgHazard::ExternalReference);
    }
    text_hazard(value)
}

/// The hazard of text (lower case): character data, a CDATA section or an attribute value, read as CSS too.
fn text_hazard(text: &str) -> Option<SvgHazard> {
    if text.contains("javascript:") {
        return Some(SvgHazard::Script);
    }
    if text.contains("@import") {
        return Some(SvgHazard::ExternalReference);
    }
    let mut at = 0;
    while let Some(k) = text[at..].find("url(") {
        let (target, next) = func_iri_target(text, at + k + 4);
        if !target.starts_with('#') {
            return Some(SvgHazard::ExternalReference);
        }
        at = next;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{AssetError, AssetFormat, sniff};

    /// hostile SVG documents (synthetic, no artwork) are refused by the store's one entry, each for its reason.
    #[test]
    fn hostile_svgs_are_refused_before_any_reader() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#;
        let cases: Vec<(&str, String, SvgHazard)> = vec![
            (
                "external entity (XXE) whose subset hides a root",
                format!(
                    r#"<!DOCTYPE svg [<!ENTITY x SYSTEM "file:///etc/hostname"><svg {NS} viewBox="0 0 4 4">]><svg {NS}>&x;</svg>"#
                ),
                SvgHazard::Entity,
            ),
            (
                "billion laughs",
                format!(
                    r#"<?xml version="1.0"?><!DOCTYPE lolz [<!ENTITY lol "lol"><!ENTITY lol2 "&lol;&lol;">]><svg {NS}>&lol2;</svg>"#
                ),
                SvgHazard::Entity,
            ),
            (
                "an empty internal subset",
                format!(r#"<!DOCTYPE svg [ ]><svg {NS} viewBox="0 0 4 4"/>"#),
                SvgHazard::DoctypeSubset,
            ),
            (
                "a script element",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><script>alert(1)</script></svg>"#),
                SvgHazard::Script,
            ),
            (
                "a prefixed script element in CDATA",
                format!(
                    r#"<svg {NS} xmlns:s="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><s:SCRIPT><![CDATA[x()]]></s:SCRIPT></svg>"#
                ),
                SvgHazard::Script,
            ),
            (
                "an onload attribute on the root",
                format!(r#"<svg {NS} viewBox="0 0 4 4" onload="alert(1)"/>"#),
                SvgHazard::EventAttribute,
            ),
            (
                "an event attribute without quotes around its spacing",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><rect width="1" height="1" onClick = 'x()'/></svg>"#),
                SvgHazard::EventAttribute,
            ),
            (
                "a javascript: link",
                format!(
                    r##"<svg {NS} viewBox="0 0 4 4"><a xlink:href="#x" title="javascript:x()"><rect width="1" height="1"/></a></svg>"##
                ),
                SvgHazard::Script,
            ),
            (
                "foreignObject",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><foreignObject width="4" height="4"><p>x</p></foreignObject></svg>"#
                ),
                SvgHazard::ForeignObject,
            ),
            (
                "an external image",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><image href="https://example.invalid/a.png" width="4" height="4"/></svg>"#
                ),
                SvgHazard::ExternalReference,
            ),
            (
                "a use of another file",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><use xlink:href="other.svg#a"/></svg>"#),
                SvgHazard::ExternalReference,
            ),
            (
                "an embedded data: image",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><image href="data:image/png;base64,AAAA" width="4" height="4"/></svg>"#
                ),
                SvgHazard::ExternalReference,
            ),
            (
                "an xml-stylesheet instruction",
                format!(r#"<?xml-stylesheet type="text/css" href="evil.css"?><svg {NS} viewBox="0 0 4 4"/>"#),
                SvgHazard::ExternalReference,
            ),
            (
                "a style sheet import",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><style>@import url(evil.css);</style></svg>"#),
                SvgHazard::ExternalReference,
            ),
            (
                "an external paint server",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><rect width="4" height="4" fill="url( 'https://example.invalid/p.svg#g')"/></svg>"#
                ),
                SvgHazard::ExternalReference,
            ),
            (
                "an unclosed comment",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><!-- <script>"#),
                SvgHazard::Malformed,
            ),
            (
                "an unclosed attribute value",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><rect width="4/></svg>"#),
                SvgHazard::Malformed,
            ),
        ];
        for (what, text, hazard) in &cases {
            assert_eq!(svg_hazard(text.as_bytes()), Some(*hazard), "{what}");
            match sniff(text.as_bytes()) {
                Err(AssetError::UnsafeSvg(h)) => assert_eq!(h, *hazard, "{what}"),
                other => panic!("{what}: the store admits it: {other:?}"),
            }
        }
        // Not UTF-8 after an SVG root.
        let mut bytes = format!(r#"<svg {NS} viewBox="0 0 4 4"><text>"#).into_bytes();
        bytes.extend_from_slice(&[0xff, 0xfe]);
        bytes.extend_from_slice(b"</text></svg>");
        assert_eq!(sniff(&bytes), Err(AssetError::UnsafeSvg(SvgHazard::NotUtf8)));
        assert!(
            AssetError::UnsafeSvg(SvgHazard::Script)
                .to_string()
                .contains("it has a script")
        );
    }

    /// A hex colour or a fragment name is skipped whole, so its digits are not an exponent; a `.` right
    /// after the `#` token still starts a number.
    #[test]
    fn hex_colours_and_fragment_names_are_not_numbers() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4""#;
        let accepted = [
            format!(r##"<svg {NS}><rect width="1" height="1" fill="#D63E25"/></svg>"##),
            format!(r#"<svg {NS}><style>.a{{fill:#E8E238}}</style></svg>"#),
            format!(r##"<svg {NS}><rect width="1" height="1" fill="#F2E12C"/></svg>"##),
            format!(r#"<svg {NS}><rect width="1" height="1" fill="url(#g1e30)"/></svg>"#),
        ];
        for text in &accepted {
            assert_eq!(svg_hazard(text.as_bytes()), None, "{text}");
        }
        let refused = [
            format!(
                r#"<svg {NS}><rect width="1" height="1" style="filter:drop-shadow(#000.5e30px 0 0)"/></svg>"#
            ),
            format!(r#"<svg {NS}><rect width="1" height="1" stroke-width="1e30"/></svg>"#),
            format!(r#"<svg {NS}><path d="M0 0 A1e30 1 0 0 1 2 2"/></svg>"#),
        ];
        for text in &refused {
            assert_eq!(svg_hazard(text.as_bytes()), Some(SvgHazard::HugeNumber), "{text}");
        }
    }

    /// A number is read with the SVG grammar, so a compressed run (`0.5.5e39`) is checked as
    /// its several numbers and a huge finite one cannot hide in the dot-joined token.
    #[test]
    fn compressed_numbers_are_read_one_by_one() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4""#;
        let morphology = format!(
            r#"<svg {NS}><filter id="f"><feMorphology radius="0.5.5e39"/></filter><rect width="4" height="4" filter="url(#f)"/></svg>"#
        );
        let arc = format!(r#"<svg {NS}><path d="M0 0 A0.5.5e40 0 0 1 1 1"/></svg>"#);
        for text in [morphology, arc] {
            assert_eq!(svg_hazard(text.as_bytes()), Some(SvgHazard::HugeNumber), "{text}");
        }
        let accepted = [
            format!(r#"<svg {NS}><polyline points="0.5.5 1.5.5"/></svg>"#),
            format!(r#"<svg {NS} inkscape:version="1.2.3 (b0a8486541, 2022-12-01)"/>"#),
            format!(r#"<svg {NS}><rect width="1" height="1" stroke-width="1.5"/></svg>"#),
        ];
        for text in &accepted {
            assert_eq!(svg_hazard(text.as_bytes()), None, "{text}");
        }
    }

    /// roxmltree does not list namespace declarations as attributes and usvg copies none of
    /// them, so `xmlns` and `xmlns:*` are skipped; a real event attribute is still refused.
    #[test]
    fn namespace_declarations_are_not_attributes() {
        let declarations = [
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:href="urn:x" viewBox="0 0 4 4"/>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:onload="urn:x" viewBox="0 0 4 4"/>"#,
        ];
        for text in declarations {
            assert_eq!(svg_hazard(text.as_bytes()), None, "{text}");
        }
        let event = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4" onload="x()"/>"#;
        assert_eq!(svg_hazard(event.as_bytes()), Some(SvgHazard::EventAttribute));
    }

    #[test]
    fn ordinary_exporter_namespace_attributes_still_pass() {
        let text = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:serif="http://www.serif.com/" inkscape:version="1.2.3" serif:version="2.0" viewBox="0 0 4 4"><g id="layer" inkscape:label="Layer 1" serif:id="layer"/></svg>"#;
        assert_eq!(svg_hazard(text.as_bytes()), None);
        assert_eq!(sniff(text.as_bytes()), Ok((AssetFormat::Svg, Some((4, 4)))));
    }

    /// A DOCTYPE whose internal subset holds a `>` (an entity, an attribute list) is scanned with its
    /// quote and bracket state, so the root is still found and the subset is named rather than "not an SVG".
    #[test]
    fn a_doctype_subset_is_named_not_unknown() {
        let entity = r#"<?xml version="1.0"?><!DOCTYPE svg [ <!ENTITY ns_svg "http://www.w3.org/2000/svg"> ]><svg xmlns="&ns_svg;" viewBox="0 0 4 4"/>"#;
        assert_eq!(
            sniff(entity.as_bytes()),
            Err(AssetError::UnsafeSvg(SvgHazard::Entity))
        );
        let attlist = r#"<!DOCTYPE svg [ <!ATTLIST svg a CDATA "x>y"> ]><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"/>"#;
        assert_eq!(
            sniff(attlist.as_bytes()),
            Err(AssetError::UnsafeSvg(SvgHazard::DoctypeSubset))
        );
        let unclosed = r#"<!DOCTYPE svg [ <svg xmlns="http://www.w3.org/2000/svg"/>"#;
        assert_eq!(sniff(unclosed.as_bytes()), Err(AssetError::UnknownFormat));
        let external = r#"<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"/>"#;
        assert_eq!(sniff(external.as_bytes()), Ok((AssetFormat::Svg, Some((4, 4)))));
        let comment = r#"<!DOCTYPE svg [ <!-- ] --> <!ENTITY x "y"> ]><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"/>"#;
        assert_eq!(
            sniff(comment.as_bytes()),
            Err(AssetError::UnsafeSvg(SvgHazard::Entity))
        );
    }

    /// Every refusal says what the hazard is and how to export the file again.
    #[test]
    fn every_refusal_says_how_to_export_again() {
        let hazards = [
            SvgHazard::NotUtf8,
            SvgHazard::DoctypeSubset,
            SvgHazard::Entity,
            SvgHazard::Script,
            SvgHazard::EventAttribute,
            SvgHazard::ForeignObject,
            SvgHazard::ExternalReference,
            SvgHazard::Malformed,
            SvgHazard::TooComplex,
            SvgHazard::HugeNumber,
            SvgHazard::Unsupported,
        ];
        for h in hazards {
            assert_eq!(
                AssetError::UnsafeSvg(h).to_string(),
                format!("the SVG file is refused: {}; {}", h.describe(), h.advice()),
                "{h:?}"
            );
            assert!(h.advice().contains("SVG"), "{h:?}");
        }
    }

    /// What drawing programs write passes: an external DOCTYPE identifier, comments (also ones that mention
    /// a script), local fragment references, paint servers and styles.
    #[test]
    fn ordinary_svgs_pass_the_rejecter() {
        let text = r##"<?xml version="1.0" encoding="UTF-8"?>
<!-- Generator: a drawing program; <script> in a comment is no script -->
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1" viewBox="0 0 80 20">
  <defs>
    <linearGradient id="g"><stop offset="0" stop-color="#123456"/></linearGradient>
    <style type="text/css"><![CDATA[ .a { fill: url(#g); opacity: 0.5 } ]]></style>
    <path id="r" d="M0 0 H10 V10 Z"/>
  </defs>
  <rect class="a" width="80" height="20" fill="url(#g)" clip-path="url( '#c' )"/>
  <use xlink:href="#r" x="5"/>
  <a href="#r"><text x="1" y="15">Example &amp; Co</text></a>
</svg>
"##;
        assert_eq!(svg_hazard(text.as_bytes()), None);
        assert_eq!(sniff(text.as_bytes()), Ok((AssetFormat::Svg, Some((80, 20)))));
    }

    /// What the parser does not survive (a stack overflow, a walk without end, an allocation without bound, a panic)
    /// is refused before it runs (synthetic documents).
    #[test]
    fn svgs_the_parser_cannot_bear_are_refused() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#;
        let deep = format!(
            r#"<svg {NS} viewBox="0 0 4 4">{}{}</svg>"#,
            "<g>".repeat(MAX_SVG_DEPTH),
            "</g>".repeat(MAX_SVG_DEPTH)
        );
        let cases: Vec<(&str, String, SvgHazard)> = vec![
            ("nesting beyond the limit", deep, SvgHazard::TooComplex),
            (
                "a three-element clip-path cycle",
                format!(
                    r##"<svg {NS} viewBox="0 0 4 4"><clipPath id="a" clip-path="url(#b)"><rect width="1" height="1"/></clipPath><clipPath id="b" clip-path="url(#c)"><rect width="1" height="1"/></clipPath><clipPath id="c"><rect width="1" height="1" clip-path="url(#a)"/></clipPath></svg>"##
                ),
                SvgHazard::TooComplex,
            ),
            (
                "a huge arc radius",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><path d="M0 0 A1e30 1e30 0 1 1 1 0"/></svg>"#),
                SvgHazard::HugeNumber,
            ),
            (
                "a filter value beyond f32",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><filter id="f"><feMorphology radius="1e39"/></filter></svg>"#
                ),
                SvgHazard::HugeNumber,
            ),
            (
                "transform-origin",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><rect width="1" height="1" transform-origin="-"/></svg>"#
                ),
                SvgHazard::Unsupported,
            ),
            (
                "feConvolveMatrix",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><filter id="f"><feConvolveMatrix order="65536 65536"/></filter></svg>"#
                ),
                SvgHazard::Unsupported,
            ),
        ];
        for (what, text, hazard) in &cases {
            assert_eq!(svg_hazard(text.as_bytes()), Some(*hazard), "{what}");
        }
    }

    /// What only costs time and memory while the drawing is made passes the plain rejecter: an expansion through
    /// references (the parser stops at a million nodes), markers inside markers, a long selector, a long run of
    /// close-path commands and a reference made in a style sheet.
    #[test]
    fn what_only_costs_drawing_time_passes_the_plain_rejecter() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#;
        let mut bomb = format!(r#"<svg {NS} viewBox="0 0 4 4"><defs><path id="l0" d="M0 0 H1"/>"#);
        for level in 1..8 {
            bomb.push_str(&format!(r#"<g id="l{level}">"#));
            for _ in 0..10 {
                bomb.push_str(&format!(r##"<use href="#l{}"/>"##, level - 1));
            }
            bomb.push_str("</g>");
        }
        bomb.push_str(r##"</defs><use href="#l7"/></svg>"##);
        let passes = [
            bomb,
            format!(
                r##"<svg {NS} viewBox="0 0 4 4"><marker id="m"><path d="M0 0 L1 1 L2 0" marker-mid="url(#n)"/></marker><marker id="n"><path d="M0 0 L1 1"/></marker></svg>"##
            ),
            format!(
                r#"<svg {NS} viewBox="0 0 4 4"><style>g {} {{ fill: red }}</style></svg>"#,
                "* ".repeat(19)
            ),
            format!(
                r#"<svg {NS} viewBox="0 0 4 4"><path d="M0 0 H1 {}"/></svg>"#,
                "Z".repeat(40)
            ),
            format!(
                r##"<svg {NS} viewBox="0 0 4 4"><style>.k {{ clip-path: url(#c) }}</style><clipPath id="c"><rect class="k" width="1" height="1"/></clipPath></svg>"##
            ),
        ];
        for text in &passes {
            assert_eq!(svg_hazard(text.as_bytes()), None, "{text}");
        }
    }

    /// A drawing-program logo with an `id` on the root, classes, clip paths through `use`, gradients and
    /// colours whose hex digits look like exponents passes the structural checks.
    #[test]
    fn a_drawing_program_logo_passes_the_structural_checks() {
        let text = r##"<?xml version="1.0" encoding="utf-8"?>
<svg version="1.1" id="Layer_1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" x="0px" y="0px" viewBox="0 0 120 30" style="enable-background:new 0 0 120 30;" xml:space="preserve">
<style type="text/css">
	.st0{clip-path:url(#SVGID_2_);fill:url(#SVGID_3_);}
	.st1{fill:#1E9E3F;}
</style>
<g>
	<defs><rect id="SVGID_1_" width="120" height="30"/></defs>
	<clipPath id="SVGID_2_"><use xlink:href="#SVGID_1_" style="overflow:visible;"/></clipPath>
	<linearGradient id="SVGID_3_" gradientUnits="userSpaceOnUse" x1="0" y1="15" x2="120" y2="15">
		<stop offset="0" style="stop-color:#0B5E2E"/><stop offset="1" style="stop-color:#1E9E3F"/>
	</linearGradient>
	<rect class="st0" width="120" height="30"/>
	<path class="st1" d="M10.5,5.2c1.2-0.4,2.5,0.1,3,1.3l4.2,9.8c0.5,1.2-0.1,2.5-1.3,3L6.6,23.5Z M30 5 h10 v20 z"/>
</g>
</svg>
"##;
        assert_eq!(svg_hazard(text.as_bytes()), None);
    }

    /// Review fixes 4 to 6: character references decoded before the checks (the parser decodes them), cycles in the
    /// `href` chains of gradients (the parser's chain walk never ends on them), and long reference chains (the parser
    /// recurses once per link).
    #[test]
    fn decoded_references_gradient_chains_and_long_chains_are_refused() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#;
        let zeros = "&#48;".repeat(10);
        let mut chain = format!(r#"<svg {NS} viewBox="0 0 4 4"><defs><path id="u0" d="M0 0 H1"/>"#);
        for k in 1..1000 {
            chain.push_str(&format!(r##"<use id="u{k}" href="#u{}"/>"##, k - 1));
        }
        chain.push_str(r##"</defs><use href="#u999"/></svg>"##);
        let cases: Vec<(&str, String, SvgHazard)> = vec![
            (
                "a huge arc radius spelled with character references",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><path d="M0 0 A1{zeros} 1{zeros} 0 1 1 1 0"/></svg>"#),
                SvgHazard::HugeNumber,
            ),
            (
                "a huge number in hexadecimal references",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><path d="M0 0 A&#x31;e30 1 0 1 1 1 0"/></svg>"#),
                SvgHazard::HugeNumber,
            ),
            (
                "transform-origin spelled with a reference in a style attribute",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><rect width="1" height="1" style="transform&#45;origin: -"/></svg>"#
                ),
                SvgHazard::Unsupported,
            ),
            (
                "transform-origin spelled with a reference in a style sheet",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><style>rect {{ transform&#x2d;origin: - }}</style></svg>"#
                ),
                SvgHazard::Unsupported,
            ),
            (
                "an unknown entity reference",
                format!(r#"<svg {NS} viewBox="0 0 4 4"><rect width="&bogus;"/></svg>"#),
                SvgHazard::Malformed,
            ),
            (
                "a gradient href cycle that returns to neither end",
                format!(
                    r##"<svg {NS} viewBox="0 0 4 4"><linearGradient id="g1" href="#g2"/><linearGradient id="g2" xlink:href="#g3"/><radialGradient id="g3" href="#g2"/><rect width="4" height="4" fill="url(#g1)"/></svg>"##
                ),
                SvgHazard::TooComplex,
            ),
            ("a chain of a thousand use links", chain, SvgHazard::TooComplex),
        ];
        for (what, text, hazard) in &cases {
            assert_eq!(svg_hazard(text.as_bytes()), Some(*hazard), "{what}");
        }
        // What stays allowed: an ordinary gradient chain, a short use chain, references in text.
        let mut short = format!(r#"<svg {NS} viewBox="0 0 4 4"><defs><path id="u0" d="M0 0 H1"/>"#);
        for k in 1..MAX_SVG_REF_DEPTH - 2 {
            short.push_str(&format!(r##"<use id="u{k}" href="#u{}"/>"##, k - 1));
        }
        short.push_str(&format!(
            r##"</defs><use href="#u{}"/></svg>"##,
            MAX_SVG_REF_DEPTH - 3
        ));
        let fine = [
            format!(
                r##"<svg {NS} viewBox="0 0 4 4"><linearGradient id="a"><stop offset="0"/></linearGradient><linearGradient id="b" href="#a"/><linearGradient id="c" href="#b"/><rect width="4" height="4" fill="url(#c)"/></svg>"##
            ),
            short,
            format!(r#"<svg {NS} viewBox="0 0 4 4"><text>A &amp; B &#169; 2026</text></svg>"#),
        ];
        for text in &fine {
            assert_eq!(svg_hazard(text.as_bytes()), None, "{text}");
        }
    }

    /// A cycle-free gradient chain longer than `MAX_SVG_REF_DEPTH` is refused (usvg walks it once per
    /// attribute of each gradient, so its work grows with the square of the chain); a shorter one passes.
    #[test]
    fn long_gradient_chains_are_refused() {
        let chain = |n: usize| {
            let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4">"#);
            svg.push_str(r#"<linearGradient id="g0"><stop offset="0"/></linearGradient>"#);
            for k in 1..n {
                svg.push_str(&format!(r##"<linearGradient id="g{k}" href="#g{}"/>"##, k - 1));
            }
            svg.push_str(&format!(
                r##"<rect width="4" height="4" fill="url(#g{})"/></svg>"##,
                n - 1
            ));
            svg
        };
        assert_eq!(MAX_SVG_REF_DEPTH, 32);
        assert_eq!(
            svg_hazard(chain(33).as_bytes()),
            Some(SvgHazard::TooComplex),
            "33 gradients"
        );
        assert_eq!(svg_hazard(chain(31).as_bytes()), None, "31 gradients");
        assert_eq!(svg_hazard(chain(32).as_bytes()), None, "32 gradients, the bound");
    }
}
