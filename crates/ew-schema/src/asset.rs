// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Assets: content-addressed image files (`assets/<sha256>.<ext>`, doc 05 §13.3; FT-337).
//!
//! - [`AssetFormat`]: the encodings a project may hold (SVG, PNG, JPEG) and their file extensions.
//! - [`AssetRef`]: how a record names an asset, `{sha256, format, size}` (decision log ID-61): readers such as the
//!   forms layer trust it and never sniff the bytes.
//! - [`sniff`]: what the asset store does once, when bytes are imported: it reads the magic bytes and the
//!   intrinsic size (PNG `IHDR`, JPEG `SOF`, SVG `viewBox` or `width`/`height`) and applies the size limits. It
//!   decodes no image and renders nothing. An SVG must also pass [`svg_hazard`].
//! - [`svg_hazard`]: the conservative pre-parse rejecter of SVG assets (FT-383, doc 18 §11 TM-18): sniffing is not
//!   sanitising, so every SVG the store admits is first scanned for a DOCTYPE internal subset, entity declarations,
//!   scripts and event attributes, `foreignObject`, and references that leave the document; any of them refuses the
//!   asset before a parser or a renderer sees it.
//! - [`sha256_mentions`]: the asset hashes a text mentions (the asset purge, FT-380).
//! - [`base64_encode`] and [`base64_decode`]: the standard alphabet with padding (RFC 4648 §4), the encoding of
//!   asset bytes inside JSON (command parameters, journal lines).
//! - [`company_logo`]: the logo of the provisional company member `Project.extra["ew.company"]` (doc 09 §2.6,
//!   decision log ID-40), in either of its two forms.
//!
//! Hashing is the caller's (the format layer and the command layer own SHA-256).

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::Extra;
use crate::value::Sha256;

/// The largest raster asset (PNG, JPEG): 16 MiB (doc 05 §13.3).
pub const MAX_RASTER_BYTES: usize = 16 * 1024 * 1024;
/// The largest SVG asset: 8 MiB (doc 05 §13.3).
pub const MAX_SVG_BYTES: usize = 8 * 1024 * 1024;
/// The most elements an SVG asset may have (doc 05 §13.3).
pub const MAX_SVG_NODES: usize = 100_000;

/// The largest company logo: 512 KiB (FT-337 fix round, owner ruling). The logo travels as base64 inside a command
/// parameter and a journal line, so it stays well below the command layer's 1 MiB limit of one parameter string.
pub const MAX_LOGO_BYTES: usize = 512 * 1024;

/// The project-row member that holds the company data of a basic project until `Project` types it (decision log
/// ID-40, ID-545).
pub const COMPANY_MEMBER: &str = "ew.company";

/// The encoding of an asset.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetFormat {
    /// SVG, drawn as vectors by EW's own pipeline and never executed.
    Svg,
    /// PNG; prints rasterised (D-07).
    Png,
    /// JPEG; prints rasterised.
    Jpeg,
}

impl AssetFormat {
    /// The file extension of `assets/<sha256>.<ext>`.
    pub const fn ext(self) -> &'static str {
        match self {
            AssetFormat::Svg => "svg",
            AssetFormat::Png => "png",
            AssetFormat::Jpeg => "jpg",
        }
    }

    /// The format of a file extension (`svg`, `png`, `jpg`); `None` for any other.
    pub fn from_ext(ext: &str) -> Option<AssetFormat> {
        match ext {
            "svg" => Some(AssetFormat::Svg),
            "png" => Some(AssetFormat::Png),
            "jpg" => Some(AssetFormat::Jpeg),
            _ => None,
        }
    }

    /// Whether the format is a raster image (it prints rasterised, D-07).
    pub const fn is_raster(self) -> bool {
        !matches!(self, AssetFormat::Svg)
    }
}

/// How a record names an asset: its content hash, its encoding and, when known, its intrinsic size (width and
/// height in pixels or SVG user units; only their ratio is used). Encoded as
/// `{"sha256": "sha256:<64 hex>", "format": "svg", "size": [w, h]}`, `size` absent when unknown.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct AssetRef {
    /// SHA-256 of the file's bytes.
    pub sha256: Sha256,
    /// The encoding.
    pub format: AssetFormat,
    /// The intrinsic width and height, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<(u32, u32)>,
}

impl AssetRef {
    /// The asset's path in a project directory: `assets/<64 hex>.<ext>`.
    pub fn path(&self) -> String {
        let hex = self.sha256.to_string();
        format!("assets/{}.{}", &hex["sha256:".len()..], self.format.ext())
    }
}

/// Why bytes are not an asset.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AssetError {
    /// Not an SVG, PNG or JPEG file.
    UnknownFormat,
    /// Larger than the limit of its format.
    TooLarge {
        /// The format.
        format: AssetFormat,
        /// The length in bytes.
        len: usize,
        /// The limit in bytes.
        limit: usize,
    },
    /// An SVG file with more than [`MAX_SVG_NODES`] elements.
    TooManyNodes,
    /// The header of a PNG or JPEG file is cut short or damaged.
    Damaged(AssetFormat),
    /// Text that is not base64 (standard alphabet, padded).
    Base64,
    /// An SVG file with content that is refused before any parser or renderer reads it ([`svg_hazard`]).
    UnsafeSvg(SvgHazard),
}

/// Why an SVG asset is refused before any parser or renderer reads it ([`svg_hazard`]; FT-383, doc 18 §11 TM-18).
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
    /// Structure the parser cannot bear (FT-384 audit): nesting deeper than [`MAX_SVG_DEPTH`], a cycle of references,
    /// an expansion through references beyond [`MAX_SVG_EXPANSION`], a marker inside a marker, an over-complex style
    /// sheet, a long run of close-path commands.
    TooComplex,
    /// A number beyond [`MAX_SVG_NUMBER`] in an attribute or a style sheet.
    HugeNumber,
    /// A construct that panics the parser (`transform-origin`, `feConvolveMatrix`; FT-384 audit).
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
            SvgHazard::TooComplex => "its structure is too deep, too complex or cyclic",
            SvgHazard::HugeNumber => "it has a number too large to draw",
            SvgHazard::Unsupported => "it uses transform-origin or feConvolveMatrix",
        }
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::UnknownFormat => f.write_str("not an SVG, PNG or JPEG file"),
            AssetError::TooLarge { format, len, limit } => write!(
                f,
                "the {} file has {len} bytes, more than the limit of {limit}",
                format.ext()
            ),
            AssetError::TooManyNodes => write!(f, "the SVG file has more than {MAX_SVG_NODES} elements"),
            AssetError::Damaged(format) => write!(f, "the {} header is damaged or cut short", format.ext()),
            AssetError::Base64 => f.write_str("not base64 text (standard alphabet, padded)"),
            AssetError::UnsafeSvg(h) => write!(
                f,
                "the SVG file is refused: {} (scripts, entities, foreign content and external references are not \
                 allowed in a drawing's images)",
                h.describe()
            ),
        }
    }
}

impl std::error::Error for AssetError {}

/// The format and intrinsic size of asset bytes, after the size limits (doc 05 §13.3, decision log ID-61).
///
/// # Errors
///
/// [`AssetError`] for bytes of no known format, over a limit, or with a damaged raster header.
pub fn sniff(bytes: &[u8]) -> Result<(AssetFormat, Option<(u32, u32)>), AssetError> {
    let (format, size) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        (AssetFormat::Png, Some(png_size(bytes)?))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        (AssetFormat::Jpeg, Some(jpeg_size(bytes)?))
    } else if let Some(attrs) = svg_root(bytes) {
        (AssetFormat::Svg, svg_size(attrs))
    } else {
        return Err(AssetError::UnknownFormat);
    };
    let limit = if format.is_raster() {
        MAX_RASTER_BYTES
    } else {
        MAX_SVG_BYTES
    };
    if bytes.len() > limit {
        return Err(AssetError::TooLarge {
            format,
            len: bytes.len(),
            limit,
        });
    }
    if format == AssetFormat::Svg && svg_elements(bytes) > MAX_SVG_NODES {
        return Err(AssetError::TooManyNodes);
    }
    if format == AssetFormat::Svg {
        if let Some(hazard) = svg_hazard(bytes) {
            return Err(AssetError::UnsafeSvg(hazard));
        }
    }
    Ok((format, size.filter(|(w, h)| *w > 0 && *h > 0)))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at + 4)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

/// Width and height of the `IHDR` chunk, which must come first.
fn png_size(b: &[u8]) -> Result<(u32, u32), AssetError> {
    let damaged = AssetError::Damaged(AssetFormat::Png);
    if b.get(12..16) != Some(b"IHDR") || be32(b, 8) != Some(13) {
        return Err(damaged);
    }
    match (be32(b, 16), be32(b, 20)) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Ok((w, h)),
        _ => Err(damaged),
    }
}

/// Width and height of the first start-of-frame marker (`SOF0` to `SOF15` without `DHT`, `JPG` and `DAC`).
fn jpeg_size(b: &[u8]) -> Result<(u32, u32), AssetError> {
    let damaged = AssetError::Damaged(AssetFormat::Jpeg);
    let mut at = 2;
    loop {
        // Fill bytes before a marker are allowed.
        while b.get(at) == Some(&0xff) && b.get(at + 1) == Some(&0xff) {
            at += 1;
        }
        if b.get(at) != Some(&0xff) {
            return Err(damaged);
        }
        let marker = *b.get(at + 1).ok_or(damaged.clone())?;
        if (0xd0..=0xd9).contains(&marker) || marker == 0x01 {
            // Markers without a length; the end of the image before a frame is damaged.
            if marker == 0xd9 {
                return Err(damaged);
            }
            at += 2;
            continue;
        }
        let len = usize::from(u16::from_be_bytes([
            *b.get(at + 2).ok_or(damaged.clone())?,
            *b.get(at + 3).ok_or(damaged.clone())?,
        ]));
        if len < 2 {
            return Err(damaged);
        }
        let sof = (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc);
        if sof {
            let h = b.get(at + 5..at + 7).ok_or(damaged.clone())?;
            let w = b.get(at + 7..at + 9).ok_or(damaged.clone())?;
            let (w, h) = (
                u32::from(u16::from_be_bytes([w[0], w[1]])),
                u32::from(u16::from_be_bytes([h[0], h[1]])),
            );
            return if w > 0 && h > 0 { Ok((w, h)) } else { Err(damaged) };
        }
        at += 2 + len;
    }
}

/// The attribute text of the root `<svg …>` element: after an optional BOM, white space, an XML declaration,
/// comments, processing instructions and a doctype, the first element must be `svg` (with or without a prefix
/// such as `svg:svg`). `None` when the bytes are not such a document.
fn svg_root(bytes: &[u8]) -> Option<&str> {
    let head = &bytes[..bytes.len().min(64 * 1024)];
    // The root tag lies within the head; a cut in the middle of a UTF-8 sequence after it does not matter.
    let text = match std::str::from_utf8(head) {
        Ok(t) => t,
        Err(e) => std::str::from_utf8(&head[..e.valid_up_to()]).ok()?,
    };
    let mut rest = text.strip_prefix('\u{feff}').unwrap_or(text);
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix("<?") {
            rest = &r[r.find("?>")? + 2..];
        } else if let Some(r) = rest.strip_prefix("<!--") {
            rest = &r[r.find("-->")? + 3..];
        } else if let Some(r) = rest.strip_prefix("<!") {
            rest = &r[r.find('>')? + 1..];
        } else {
            break;
        }
    }
    let tag = rest.strip_prefix('<')?;
    let name_end = tag
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(tag.len());
    let name = &tag[..name_end];
    let local = name.rsplit(':').next().unwrap_or(name);
    if local != "svg" {
        return None;
    }
    let end = tag.find('>')?;
    Some(&tag[name_end..end])
}

/// The value of attribute `name` in an attribute text (`name="v"` or `name='v'`).
fn attr<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = attrs;
    while let Some(i) = rest.find(name) {
        let before = rest[..i].chars().next_back();
        let after = rest[i + name.len()..].trim_start();
        rest = &rest[i + name.len()..];
        if before.is_some_and(|c| !c.is_whitespace()) {
            continue;
        }
        let Some(after) = after.strip_prefix('=') else {
            continue;
        };
        let after = after.trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let value = &after[1..];
        return value.find(quote).map(|end| &value[..end]);
    }
    None
}

/// A positive length as a whole number, rounded (the unit, if any, is dropped: only the ratio is used).
fn length(text: &str) -> Option<u32> {
    let text = text.trim();
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '+' || c == '-' || c == 'e' || c == 'E'))
        .unwrap_or(text.len());
    if text[end..].trim_start().starts_with('%') {
        return None;
    }
    let v: f64 = text[..end].parse().ok()?;
    if !(v.is_finite() && v > 0.0 && v < 1.0e9) {
        return None;
    }
    // Rounded to whole units, at least 1.
    Some((v.round() as u32).max(1))
}

/// The intrinsic size of an SVG root: its `viewBox` width and height, else its `width` and `height`.
fn svg_size(attrs: &str) -> Option<(u32, u32)> {
    if let Some(vb) = attr(attrs, "viewBox") {
        let parts: Vec<&str> = vb
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() == 4 {
            if let (Some(w), Some(h)) = (length(parts[2]), length(parts[3])) {
                return Some((w, h));
            }
        }
    }
    match (
        attr(attrs, "width").and_then(length),
        attr(attrs, "height").and_then(length),
    ) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    }
}

/// The number of element start tags (`<` followed by a name character), an upper bound of the node count that
/// needs no parser.
fn svg_elements(bytes: &[u8]) -> usize {
    bytes
        .windows(2)
        .filter(|w| w[0] == b'<' && (w[1].is_ascii_alphabetic() || w[1] == b'_'))
        .count()
}

/// The deepest element nesting an SVG asset may have (the parser recurses per level; FT-384 audit).
pub const MAX_SVG_DEPTH: usize = 64;
/// The most elements an SVG asset may expand to through its references (`use`, clip paths, masks, patterns,
/// markers, filters), counted with multiplicity (FT-384 audit: usvg converts a referenced subtree once per reference).
pub const MAX_SVG_EXPANSION: u64 = 200_000;
/// The largest magnitude of a number in an attribute or a style sheet of an SVG asset (FT-384 audit: huge arc radii
/// and filter values make the parser allocate without bound or panic).
pub const MAX_SVG_NUMBER: f64 = 1.0e9;

/// The first reason to refuse an SVG file before any parser or renderer reads it, or `None` (FT-383, doc 05 §13.3,
/// doc 18 §11 TM-18). Sniffing is not sanitising: the sniffer reads only the root element, so this scanner reads the
/// whole document as markup (comments skipped, CDATA read as text) and refuses, conservatively (ASCII case is ignored,
/// so `<SCRIPT` counts):
///
/// - a DOCTYPE with an internal subset, and any entity declaration ([`SvgHazard::DoctypeSubset`],
///   [`SvgHazard::Entity`]); a DOCTYPE with only an external identifier (`PUBLIC "-//W3C//DTD SVG 1.1//EN" "…"`, as
///   drawing programs write it) passes, because no reader of EW fetches it;
/// - `script`, `handler` and `listener` elements and `javascript:` text ([`SvgHazard::Script`]);
/// - attributes whose local name begins with `on` ([`SvgHazard::EventAttribute`]);
/// - `foreignObject` elements ([`SvgHazard::ForeignObject`]);
/// - every `href` (any prefix) that is not a fragment `#…` of the document, CSS `url(…)` that is not a fragment and
///   `@import`, in attributes and in text (style sheets), `xml-stylesheet` processing instructions, and `iframe`,
///   `embed`, `object` and `include` elements ([`SvgHazard::ExternalReference`]); an embedded `data:` image counts
///   as such a reference (EW draws an SVG asset's paths, not images inside it);
/// - text that is not UTF-8, and markup it cannot follow ([`SvgHazard::NotUtf8`], [`SvgHazard::Malformed`]);
/// - what the parser's audit found it cannot bear (FT-384; the cargo-vet audit of usvg 0.48.1 and its dependencies,
///   2026-10-23): nesting deeper than [`MAX_SVG_DEPTH`]; a cycle of references between elements (`url(#…)` and
///   `href="#…"`, through descendants and through style-sheet rules, any length); an expansion through references
///   beyond [`MAX_SVG_EXPANSION`]; a marker reference inside a marker; a style sheet with selectors of more than six
///   compound parts or 32 groups, more than 64 comments or more than 64 KiB; more than 16 `drop-shadow(` functions;
///   a run of more than 16 close-path commands in path data ([`SvgHazard::TooComplex`]); a number beyond
///   [`MAX_SVG_NUMBER`] in an attribute or a style sheet ([`SvgHazard::HugeNumber`]); and the two constructs that
///   panic the parser, `transform-origin` and `feConvolveMatrix` ([`SvgHazard::Unsupported`]).
///
/// Linear in the length of the file (the reference graph is walked once). The drawing pipeline parses an admitted SVG
/// with external references disabled and inside `catch_unwind` besides (FT-384), so this is the first of two guards,
/// not the only one.
pub fn svg_hazard(bytes: &[u8]) -> Option<SvgHazard> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Some(SvgHazard::NotUtf8);
    };
    // ASCII lower case keeps every byte offset.
    let lower = text.to_ascii_lowercase();
    if lower.contains("<!entity") {
        return Some(SvgHazard::Entity);
    }
    let mut scan = Scan::default();
    let b = lower.as_bytes();
    let find = |from: usize, needle: &str| lower[from..].find(needle).map(|k| from + k);
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            let end = find(i, "<").unwrap_or(b.len());
            if let Some(h) = scan.text(&lower[i..end]) {
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
            scan.close();
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
        let mut attrs: Vec<(&str, &str)> = Vec::new();
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
                value = &lower[j + 1..end];
                j = end + 1;
            }
            if let Some(h) = attribute_hazard(attr, value) {
                return Some(h);
            }
            attrs.push((local_name(attr), value));
        }
        if let Some(h) = scan.open(name, &attrs, self_closing) {
            return Some(h);
        }
        i = j;
    }
    scan.finish()
}

/// The state of [`svg_hazard`]'s structural checks: the open elements, the reference graph and the style sheet.
#[derive(Default)]
struct Scan {
    /// The open elements: local name and graph node (an element with an `id`, gradients and stops excepted).
    open: Vec<(String, Option<usize>)>,
    /// The graph node of each id.
    ids: std::collections::BTreeMap<String, usize>,
    /// Per node: the elements of its subtree, and its references (target id, count).
    nodes: Vec<(u64, std::collections::BTreeMap<String, u64>)>,
    /// References outside any node, and the elements of the document.
    top: (u64, std::collections::BTreeMap<String, u64>),
    /// The ids the style sheets reference (applied, conservatively, from every element with a `class`).
    css_targets: std::collections::BTreeSet<String>,
    /// Whether a style sheet names a marker property, and whether the document has a `marker` element.
    css_marker: bool,
    markers: bool,
    /// The comments in style sheets.
    css_comments: usize,
    drop_shadows: usize,
}

/// A frame of the reference walk: the node, its edges (target, count) and the next edge to follow.
type Frame = (usize, Vec<(usize, u64)>, usize);

/// The node standing for "the rules of the style sheets", referenced by every element with a `class`.
const CSS_NODE: &str = "\u{0}css";

impl Scan {
    fn open(&mut self, name: &str, attrs: &[(&str, &str)], self_closing: bool) -> Option<SvgHazard> {
        if self.open.len() >= MAX_SVG_DEPTH {
            return Some(SvgHazard::TooComplex);
        }
        let in_marker = self.open.iter().any(|(n, _)| n == "marker");
        if name == "marker" {
            self.markers = true;
        }
        let id = attrs
            .iter()
            .find(|(a, _)| *a == "id")
            .map(|(_, v)| v.trim().to_owned());
        let node = match id {
            Some(id) if !matches!(name, "lineargradient" | "radialgradient" | "stop") && !id.is_empty() => {
                let k = self.nodes.len();
                self.nodes.push((0, std::collections::BTreeMap::new()));
                self.ids.insert(id, k);
                Some(k)
            }
            _ => None,
        };
        self.open.push((name.to_owned(), node));
        self.count_element();
        for (attr, value) in attrs {
            if *attr == "transform-origin" {
                return Some(SvgHazard::Unsupported);
            }
            if let Some(h) = numbers(value) {
                return Some(h);
            }
            if *attr == "d" && close_run(value) > 16 {
                return Some(SvgHazard::TooComplex);
            }
            if *attr == "style" && value.contains("transform-origin") {
                return Some(SvgHazard::Unsupported);
            }
            if in_marker && (attr.starts_with("marker") || (*attr == "style" && value.contains("marker"))) {
                return Some(SvgHazard::TooComplex);
            }
            self.drop_shadows += value.matches("drop-shadow(").count();
            if *attr == "href" {
                if let Some(target) = value.trim().strip_prefix('#') {
                    self.reference(target.trim());
                }
            }
            for target in url_targets(value) {
                self.reference(&target);
            }
            if *attr == "class" {
                self.reference(CSS_NODE);
            }
        }
        if self_closing {
            self.close();
        }
        None
    }

    fn close(&mut self) {
        self.open.pop();
    }

    /// One more element in every open node's subtree and in the document.
    fn count_element(&mut self) {
        self.top.0 += 1;
        for (_, node) in &self.open {
            if let Some(k) = node {
                self.nodes[*k].0 += 1;
            }
        }
    }

    /// A reference to `target` from the innermost element: an edge from every open node, or from the document.
    fn reference(&mut self, target: &str) {
        let mut any = false;
        for (_, node) in &self.open {
            if let Some(k) = node {
                *self.nodes[*k].1.entry(target.to_owned()).or_insert(0) += 1;
                any = true;
            }
        }
        if !any {
            *self.top.1.entry(target.to_owned()).or_insert(0) += 1;
        }
    }

    /// Character data: the text checks, and inside `style` the style-sheet checks.
    fn text(&mut self, text: &str) -> Option<SvgHazard> {
        if let Some(h) = text_hazard(text) {
            return Some(h);
        }
        if self.open.last().is_none_or(|(n, _)| n != "style") {
            return None;
        }
        if text.len() > 64 * 1024 {
            return Some(SvgHazard::TooComplex);
        }
        if text.contains("transform-origin") {
            return Some(SvgHazard::Unsupported);
        }
        if let Some(h) = numbers(text) {
            return Some(h);
        }
        self.css_comments += text.matches("/*").count();
        self.drop_shadows += text.matches("drop-shadow(").count();
        if text.contains("marker") {
            self.css_marker = true;
        }
        self.css_targets.extend(url_targets(text));
        // Rule preludes: the text before each `{` back to the previous `}`.
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            let prelude = rest[..open].rsplit('}').next().unwrap_or("");
            let groups: Vec<&str> = prelude.split(',').collect();
            if groups.len() > 32 {
                return Some(SvgHazard::TooComplex);
            }
            for g in groups {
                let parts = g
                    .split(|c: char| c.is_whitespace() || matches!(c, '>' | '+' | '~'))
                    .filter(|p| !p.is_empty())
                    .count();
                if parts > 6 {
                    return Some(SvgHazard::TooComplex);
                }
            }
            rest = &rest[open + 1..];
            match rest.find('}') {
                Some(close) => rest = &rest[close + 1..],
                None => break,
            }
        }
        None
    }

    /// The checks that need the whole document: style-sheet totals, marker rules, cycles and the expansion.
    fn finish(self) -> Option<SvgHazard> {
        if self.css_comments > 64 || self.drop_shadows > 16 || (self.css_marker && self.markers) {
            return Some(SvgHazard::TooComplex);
        }
        // The graph: node k, plus the style-sheet node (index n) whose edges go to every style-sheet target.
        let n = self.nodes.len();
        let index = |id: &str| -> Option<usize> {
            if id == CSS_NODE {
                Some(n)
            } else {
                self.ids.get(id).copied()
            }
        };
        let css_edges: Vec<(usize, u64)> = self
            .css_targets
            .iter()
            .filter_map(|t| index(t))
            .map(|k| (k, 1))
            .collect();
        let edges = |k: usize| -> Vec<(usize, u64)> {
            if k == n {
                return css_edges.clone();
            }
            self.nodes[k]
                .1
                .iter()
                .filter_map(|(t, c)| index(t).map(|j| (j, *c)))
                .collect()
        };
        let size = |k: usize| -> u64 { if k == n { 0 } else { self.nodes[k].0 } };
        // Iterative depth-first walk: a grey node met again is a cycle; a black node's cost is final.
        let mut cost: Vec<Option<u64>> = vec![None; n + 1];
        let mut grey = vec![false; n + 1];
        for root in 0..=n {
            if cost[root].is_some() {
                continue;
            }
            let mut stack: Vec<Frame> = vec![(root, edges(root), 0)];
            grey[root] = true;
            while let Some((k, out, at)) = stack.last_mut() {
                if let Some(&(j, _)) = out.get(*at) {
                    *at += 1;
                    if grey[j] {
                        return Some(SvgHazard::TooComplex);
                    }
                    if cost[j].is_none() {
                        grey[j] = true;
                        let e = edges(j);
                        stack.push((j, e, 0));
                    }
                    continue;
                }
                let k = *k;
                let total = out.iter().fold(size(k), |acc, (j, c)| {
                    acc.saturating_add(c.saturating_mul(cost[*j].unwrap_or(0)))
                });
                if total > MAX_SVG_EXPANSION {
                    return Some(SvgHazard::TooComplex);
                }
                cost[k] = Some(total);
                grey[k] = false;
                stack.pop();
            }
        }
        let total = self.top.1.iter().fold(self.top.0, |acc, (t, c)| {
            let k = index(t);
            acc.saturating_add(c.saturating_mul(k.and_then(|k| cost[k]).unwrap_or(0)))
        });
        (total > MAX_SVG_EXPANSION).then_some(SvgHazard::TooComplex)
    }
}

/// The ids of the `url(#id)` references in `text` (lower case).
fn url_targets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(k) = rest.find("url(") {
        rest = &rest[k + 4..];
        let target = rest.trim_start().trim_start_matches(['"', '\'']).trim_start();
        if let Some(id) = target.strip_prefix('#') {
            let end = id
                .find(|c: char| matches!(c, ')' | '"' | '\'') || c.is_whitespace())
                .unwrap_or(id.len());
            out.push(id[..end].to_owned());
        }
    }
    out
}

/// [`SvgHazard::HugeNumber`] when `text` has a number beyond [`MAX_SVG_NUMBER`] (or one that is not finite): a
/// number begins with a digit or a `.` that does not follow a letter, a digit, `_`, `-` of a name, or `#` (a colour).
fn numbers(text: &str) -> Option<SvgHazard> {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let starts = (c.is_ascii_digit() || c == b'.')
            && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || matches!(b[i - 1], b'_' | b'#' | b'.')));
        if !starts {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
            j += 1;
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
        i = j.max(i + 1);
    }
    None
}

/// The longest run of close-path commands (`z`, separated by white space or commas) in path data.
fn close_run(d: &str) -> usize {
    let (mut best, mut run) = (0, 0);
    for c in d.chars() {
        match c {
            'z' => {
                run += 1;
                best = best.max(run);
            }
            c if c.is_whitespace() || c == ',' => {}
            _ => run = 0,
        }
    }
    best
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
    let mut rest = text;
    while let Some(k) = rest.find("url(") {
        rest = &rest[k + 4..];
        let target = rest.trim_start().trim_start_matches(['"', '\'']).trim_start();
        if !target.starts_with('#') {
            return Some(SvgHazard::ExternalReference);
        }
    }
    None
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 of `bytes`: the standard alphabet with padding, no line breaks (RFC 4648 §4).
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The bytes of base64 text (standard alphabet, padded, no white space; the canonical form only: the unused bits
/// of the last group must be zero, so that one byte sequence has one text).
///
/// # Errors
///
/// [`AssetError::Base64`] for any other text.
pub fn base64_decode(text: &str) -> Result<Vec<u8>, AssetError> {
    let b = text.as_bytes();
    if b.len() % 4 != 0 {
        return Err(AssetError::Base64);
    }
    let value = |c: u8| -> Option<u32> {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        Some(u32::from(v))
    };
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    let groups = b.len() / 4;
    for (g, chunk) in b.chunks(4).enumerate() {
        let pad = chunk.iter().rev().take_while(|c| **c == b'=').count();
        if pad > 2 || (pad > 0 && g + 1 != groups) {
            return Err(AssetError::Base64);
        }
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            let v = if i >= 4 - pad {
                0
            } else {
                value(*c).ok_or(AssetError::Base64)?
            };
            n = (n << 6) | v;
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        let keep = 3 - pad;
        // Canonical: the bits a padded group does not carry are zero.
        if bytes[keep..].iter().any(|b| *b != 0) {
            return Err(AssetError::Base64);
        }
        out.extend_from_slice(&bytes[..keep]);
    }
    Ok(out)
}

/// The company logo of a project row's `extra` (the member [`COMPANY_MEMBER`], its `logo`): an [`AssetRef`]
/// object, or the earlier form, a bare `"sha256:<64 hex>"` text, read as a PNG of unknown size (what the title
/// block assumed before the asset store existed). `None` when absent or not readable (each member is read on its
/// own).
pub fn company_logo(extra: &Extra) -> Option<AssetRef> {
    let logo = extra.get(COMPANY_MEMBER)?.get("logo")?;
    match logo {
        Value::String(text) => text.parse::<Sha256>().ok().map(|sha256| AssetRef {
            sha256,
            format: AssetFormat::Png,
            size: None,
        }),
        Value::Object(_) => serde_json::from_value(logo.clone()).ok(),
        _ => None,
    }
}

/// Every SHA-256 that `bytes` mention as text: each run of 64 consecutive hexadecimal digits (either case) inside a
/// run of ASCII hex digits, so a run of 70 digits mentions its seven 64-digit windows. This is the conservative
/// "appears in a file" test of the asset purge (doc 05 §13.3, FT-380): any form a writer may use to name an asset
/// (`sha256:<hex>`, a bare hash, `assets/<hex>.svg`, upper-case hex of a newer writer) contains the 64 digits, so an
/// asset whose hash is not in the returned set is named by no such text. It over-approximates (a random hex run in a
/// file keeps an asset), never under-approximates.
pub fn sha256_mentions(bytes: &[u8], out: &mut std::collections::BTreeSet<Sha256>) {
    const DIGITS: usize = 64;
    let nibble = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    };
    let mut start = 0;
    while start < bytes.len() {
        if nibble(bytes[start]).is_none() {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < bytes.len() && nibble(bytes[end]).is_some() {
            end += 1;
        }
        if end - start >= DIGITS {
            for at in start..=end - DIGITS {
                let mut hash = [0u8; 32];
                for (i, b) in hash.iter_mut().enumerate() {
                    let hi = nibble(bytes[at + 2 * i]).unwrap_or(0);
                    let lo = nibble(bytes[at + 2 * i + 1]).unwrap_or(0);
                    *b = (hi << 4) | lo;
                }
                out.insert(Sha256(hash));
            }
        }
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FT-383: hostile SVG documents (synthetic, no artwork) are refused by the store's one entry, each for its reason.
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
                // The sniffer already refuses a document whose root it cannot find (a DOCTYPE subset).
                Err(AssetError::UnknownFormat) => {}
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

    /// FT-383: what drawing programs write passes: an external DOCTYPE identifier, comments (also ones that mention
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

    /// FT-384: what the cargo-vet audit of usvg 0.48.1 and its dependencies found the parser cannot bear (stack
    /// overflow, exponential expansion, panics) is refused before the parser runs (synthetic documents).
    #[test]
    fn svgs_the_parser_cannot_bear_are_refused() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#;
        let deep = format!(
            r#"<svg {NS} viewBox="0 0 4 4">{}{}</svg>"#,
            "<g>".repeat(MAX_SVG_DEPTH),
            "</g>".repeat(MAX_SVG_DEPTH)
        );
        let mut bomb = format!(r#"<svg {NS} viewBox="0 0 4 4"><defs><path id="l0" d="M0 0 H1"/>"#);
        for level in 1..8 {
            bomb.push_str(&format!(r#"<g id="l{level}">"#));
            for _ in 0..10 {
                bomb.push_str(&format!(r##"<use href="#l{}"/>"##, level - 1));
            }
            bomb.push_str("</g>");
        }
        bomb.push_str(r##"</defs><use href="#l7"/></svg>"##);
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
                "a pattern cycle through a style-sheet class",
                format!(
                    r##"<svg {NS} viewBox="0 0 4 4"><style>.p {{ fill: url(#pat) }}</style><pattern id="pat" width="1" height="1"><rect class="p" width="1" height="1"/></pattern></svg>"##
                ),
                SvgHazard::TooComplex,
            ),
            ("a use expansion bomb", bomb, SvgHazard::TooComplex),
            (
                "a marker inside a marker",
                format!(
                    r##"<svg {NS} viewBox="0 0 4 4"><marker id="m"><path d="M0 0 L1 1 L2 0" marker-mid="url(#n)"/></marker><marker id="n"><path d="M0 0 L1 1"/></marker></svg>"##
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
            (
                "a descendant selector of twenty parts",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><style>g {} {{ fill: red }}</style></svg>"#,
                    "* ".repeat(19)
                ),
                SvgHazard::TooComplex,
            ),
            (
                "a selector group of forty",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><style>{} {{ fill: red }}</style></svg>"#,
                    vec!["a"; 40].join(",")
                ),
                SvgHazard::TooComplex,
            ),
            (
                "a run of close-path commands",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><path d="M0 0 H1 {}"/></svg>"#,
                    "Z".repeat(40)
                ),
                SvgHazard::TooComplex,
            ),
        ];
        for (what, text, hazard) in &cases {
            assert_eq!(svg_hazard(text.as_bytes()), Some(*hazard), "{what}");
        }
    }

    /// FT-384: a drawing-program logo with an `id` on the root, classes, clip paths through `use`, gradients and
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

    #[test]
    fn mentions_find_every_64_digit_window_in_either_case() {
        let mut out = std::collections::BTreeSet::new();
        let hex = "ab".repeat(32);
        let text = format!("x sha256:{hex} y assets/{}.svg z", hex.to_uppercase());
        sha256_mentions(text.as_bytes(), &mut out);
        assert_eq!(out.into_iter().collect::<Vec<_>>(), [Sha256([0xab; 32])]);
        // 63 digits mention nothing; 66 digits mention three windows.
        let mut out = std::collections::BTreeSet::new();
        sha256_mentions("1".repeat(63).as_bytes(), &mut out);
        assert!(out.is_empty());
        sha256_mentions(format!("01{}", "2".repeat(64)).as_bytes(), &mut out);
        assert_eq!(out.len(), 3);
        // Binary bytes around a hash do not hide it.
        let mut out = std::collections::BTreeSet::new();
        let mut bytes = vec![0u8, 0xff, b'g'];
        bytes.extend_from_slice("cd".repeat(32).as_bytes());
        bytes.push(0x80);
        sha256_mentions(&bytes, &mut out);
        assert!(out.contains(&Sha256([0xcd; 32])));
    }

    /// A PNG signature and `IHDR` of `w × h` (the rest of the file does not matter to the sniffer).
    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        b
    }

    #[test]
    fn png_and_jpeg_sizes_come_from_their_headers() {
        assert_eq!(sniff(&png(120, 40)), Ok((AssetFormat::Png, Some((120, 40)))));
        assert_eq!(sniff(&png(0, 40)), Err(AssetError::Damaged(AssetFormat::Png)));
        assert_eq!(
            sniff(&png(1, 1)[..20]),
            Err(AssetError::Damaged(AssetFormat::Png))
        );
        // SOI, an APP0 segment of 16 bytes, then SOF0 with height 30 and width 90.
        let mut jpg = vec![0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10];
        jpg.extend_from_slice(&[0; 14]);
        jpg.extend_from_slice(&[0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 30, 0x00, 90, 0x03]);
        assert_eq!(sniff(&jpg), Ok((AssetFormat::Jpeg, Some((90, 30)))));
        assert_eq!(
            sniff(&[0xff, 0xd8, 0xff, 0xd9]),
            Err(AssetError::Damaged(AssetFormat::Jpeg))
        );
    }

    #[test]
    fn svg_size_comes_from_the_view_box_else_width_and_height() {
        let s = br#"<?xml version="1.0"?><!-- logo --><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 20.4"><rect width="80" height="20"/></svg>"#;
        assert_eq!(sniff(s), Ok((AssetFormat::Svg, Some((80, 20)))));
        let s = b"\xef\xbb\xbf\n<svg width='40mm' height=\"10mm\"></svg>";
        assert_eq!(sniff(s), Ok((AssetFormat::Svg, Some((40, 10)))));
        let s = br#"<svg:svg xmlns:svg="http://www.w3.org/2000/svg" width="100%" height="50"/>"#;
        assert_eq!(sniff(s), Ok((AssetFormat::Svg, None)));
        // A `data-viewBox` attribute is not the view box.
        let s = br#"<svg data-viewBox="0 0 1 1" width="3" height="2"/>"#;
        assert_eq!(sniff(s), Ok((AssetFormat::Svg, Some((3, 2)))));
        assert_eq!(sniff(b"<html><svg/></html>"), Err(AssetError::UnknownFormat));
        assert_eq!(sniff(b"GIF89a"), Err(AssetError::UnknownFormat));
        assert_eq!(sniff(b""), Err(AssetError::UnknownFormat));
    }

    #[test]
    fn limits_apply_per_format() {
        let mut big = b"<svg width='1' height='1'>".to_vec();
        big.resize(MAX_SVG_BYTES + 1, b' ');
        assert!(matches!(
            sniff(&big),
            Err(AssetError::TooLarge {
                format: AssetFormat::Svg,
                ..
            })
        ));
        let mut many = b"<svg>".to_vec();
        for _ in 0..MAX_SVG_NODES {
            many.extend_from_slice(b"<g/>");
        }
        assert_eq!(sniff(&many), Err(AssetError::TooManyNodes));
        let mut raster = png(2, 2);
        raster.resize(MAX_RASTER_BYTES + 1, 0);
        assert!(matches!(
            sniff(&raster),
            Err(AssetError::TooLarge {
                format: AssetFormat::Png,
                ..
            })
        ));
    }

    #[test]
    fn base64_round_trips_and_is_canonical() {
        for (bytes, text) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"\xff\xfe\x00", "//4A"),
        ] {
            assert_eq!(base64_encode(bytes), text);
            assert_eq!(base64_decode(text).as_deref(), Ok(bytes));
        }
        for bad in ["Zg=", "Zh==", "Z===", "Zg==Zg==", "Zm9v\n", "Zm-v", "===="] {
            assert_eq!(base64_decode(bad), Err(AssetError::Base64), "{bad:?}");
        }
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(base64_decode(&base64_encode(&all)), Ok(all));
    }

    #[test]
    fn asset_refs_encode_and_name_their_file() {
        let r = AssetRef {
            sha256: Sha256([0xab; 32]),
            format: AssetFormat::Svg,
            size: Some((80, 20)),
        };
        let v = serde_json::to_value(r).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"sha256": format!("sha256:{}", "ab".repeat(32)), "format": "svg", "size": [80, 20]})
        );
        assert_eq!(serde_json::from_value::<AssetRef>(v).unwrap(), r);
        assert_eq!(r.path(), format!("assets/{}.svg", "ab".repeat(32)));
        assert_eq!(AssetFormat::from_ext("jpg"), Some(AssetFormat::Jpeg));
        assert_eq!(AssetFormat::from_ext("gif"), None);
    }

    #[test]
    fn the_company_logo_reads_both_forms() {
        let hash = format!("sha256:{}", "cd".repeat(32));
        let mut extra = Extra::new();
        assert_eq!(company_logo(&extra), None);
        extra.insert(COMPANY_MEMBER.into(), serde_json::json!({"logo": hash}));
        assert_eq!(
            company_logo(&extra),
            Some(AssetRef {
                sha256: hash.parse().unwrap(),
                format: AssetFormat::Png,
                size: None
            })
        );
        extra.insert(
            COMPANY_MEMBER.into(),
            serde_json::json!({"logo": {"sha256": hash, "format": "svg", "size": [3, 1]}}),
        );
        assert_eq!(company_logo(&extra).map(|r| r.size), Some(Some((3, 1))));
        extra.insert(COMPANY_MEMBER.into(), serde_json::json!({"logo": 5}));
        assert_eq!(company_logo(&extra), None);
    }
}
