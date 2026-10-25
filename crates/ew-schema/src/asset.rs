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
    /// Structure the parser cannot bear (FT-384 audit): nesting deeper than [`MAX_SVG_DEPTH`], a cycle of references
    /// (through descendants, through class rules or through a rule with any other selector), an expansion through
    /// references beyond [`MAX_SVG_EXPANSION`], marker content that can carry a marker property (its own attributes,
    /// an ancestor's, a class whose rule names one, or any element when a rule with another selector names one), a
    /// style sheet with more selector groups than [`MAX_SVG_CSS_RULES`] or more cloned declarations than
    /// [`MAX_SVG_CSS_DECLARATIONS`] (upper bounds counted without parsing, so no string, comment or at-rule hides
    /// them), a long run of close-path commands.
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

    /// How to export the file again so that it passes, in words (FT-418).
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
                "simplify the drawing (fewer nested groups, clones, markers and style rules) and export it again as a \
                 plain SVG with presentation attributes rather than a style sheet (Inkscape: \"Plain SVG\"; CorelDRAW: \
                 Styling options \"Presentation attributes\")"
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
            AssetError::UnsafeSvg(h) => {
                write!(f, "the SVG file is refused: {}; {}", h.describe(), h.advice())
            }
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
/// such as `svg:svg`). A doctype's internal subset may hold a `>` (an entity declaration, an attribute list
/// default); the declaration is scanned with its quote and bracket state, so the subset is skipped whole and
/// [`svg_hazard`] can name it. `None` when the bytes are not such a document.
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
            if r.as_bytes()
                .get(..7)
                .is_some_and(|start| start.eq_ignore_ascii_case(b"doctype"))
            {
                let b = r.as_bytes();
                let mut quote = None;
                let mut depth = 0usize;
                let mut j = 7;
                loop {
                    let c = *b.get(j)?;
                    // A comment in the subset may hold a `]`; its content is no bracket or quote.
                    if quote.is_none()
                        && c == b'<'
                        && b.get(j + 1) == Some(&b'!')
                        && b.get(j + 2) == Some(&b'-')
                        && b.get(j + 3) == Some(&b'-')
                    {
                        let end = r[j + 4..].find("-->")?;
                        j = j + 4 + end + 3;
                        continue;
                    }
                    match (quote, c) {
                        (Some(q), c) if c == q => quote = None,
                        (Some(_), _) => {}
                        (None, b'"' | b'\'') => quote = Some(c),
                        (None, b'[') => depth += 1,
                        (None, b']') => depth = depth.saturating_sub(1),
                        (None, b'>') if depth == 0 => break,
                        (None, _) => {}
                    }
                    j += 1;
                }
                rest = &r[j + 1..];
            } else {
                rest = &r[r.find('>')? + 1..];
            }
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
/// The longest chain of references an SVG asset may have (`use` of an element that holds a `use`, a clip path whose
/// content references a mask, and so on): the parser recurses once per link and allows 1024 (FT-384 review fix 6).
pub const MAX_SVG_REF_DEPTH: usize = 32;
/// The largest magnitude of a number in an attribute or a style sheet of an SVG asset (FT-384 audit: huge arc radii
/// and filter values make the parser allocate without bound or panic).
pub const MAX_SVG_NUMBER: f64 = 1.0e9;
/// The most selector groups an SVG asset's style sheets may hold, an upper bound counted without parsing (every
/// group needs a `,` or a `{`): usvg matches every rule against every element (`usvg` 0.48.1
/// `parser/svgtree/parse.rs`), and the CSS parser makes one rule per group (audit of simplecss 0.2.2,
/// `supply-chain/audits.toml`; FT-418). The largest real exporter file seen holds a bound of 82.
pub const MAX_SVG_CSS_RULES: u64 = 4_096;
/// The most declarations an SVG asset's style sheets may hold once every selector group has its own copy of its
/// rule's declarations: for a plain sheet this is the exact count of the copies the CSS parser makes (per rule, its
/// selector groups times its declarations); a sheet that is not plain falls back to the raw group bound times its
/// `:` count (audit of simplecss 0.2.2, `supply-chain/audits.toml`: "selector-group x declaration clone growth";
/// FT-418). The largest real exporter file seen holds a bound of 3,526.
pub const MAX_SVG_CSS_DECLARATIONS: u64 = 262_144;

/// The first reason to refuse an SVG file before any parser or renderer reads it, or `None` (FT-383, doc 05 §13.3,
/// doc 18 §11 TM-18). Sniffing is not sanitising: the sniffer reads only the root element, so this scanner reads the
/// whole document as markup (comments skipped, CDATA read as text) and refuses, conservatively (ASCII case is ignored,
/// so `<SCRIPT` counts):
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
/// - what the parser's audit found it cannot bear (FT-384; the cargo-vet audit of usvg 0.48.1 and its dependencies,
///   2026-10-23): nesting deeper than [`MAX_SVG_DEPTH`]; a cycle of references between elements (`url(#…)` and
///   `href="#…"`, through descendants, through the class rules a class selects and through a rule with any other
///   selector, any length); an expansion through references beyond [`MAX_SVG_EXPANSION`]; marker content that can
///   carry a marker property, from its own attributes, an ancestor's, a class whose rule names one, or any element
///   when a rule with another selector names one; a style sheet with selectors of more than six compound parts, more
///   selector groups than [`MAX_SVG_CSS_RULES`] (an upper bound computed from the raw text without parsing) or more
///   cloned declarations than [`MAX_SVG_CSS_DECLARATIONS`] (exact for a plain sheet, the raw bound otherwise), more
///   than 64 comments or more than 64 KiB; more than 16 `drop-shadow(` functions; a run of more than 16 close-path
///   commands in path data ([`SvgHazard::TooComplex`]); a number beyond [`MAX_SVG_NUMBER`] in an attribute or a style
///   sheet ([`SvgHazard::HugeNumber`]), where a hex colour (`#D63E25`) and a fragment name (`url(#g1e30)`) are not
///   numbers; and the two constructs that panic the parser, `transform-origin` and `feConvolveMatrix`
///   ([`SvgHazard::Unsupported`]).
///
/// A style sheet's references follow the classes and selectors that use them: an element's `class` tokens reach the
/// class rules of those classes, and every element reaches the rule with any other selector (the *wild* rule). A
/// wild rule that references an element with an id other than a gradient is therefore a cycle and is refused; a wild
/// rule may reference gradients, which are not graph nodes.
///
/// A style sheet with escapes, at-rules, braces in comments or strings that can move a rule boundary is read
/// conservatively as one rule that may apply to every element (so it may not reference an element with an id other
/// than a gradient, nor carry a marker property when the document has markers); a quoted string that simplecss reads
/// as a plain value keeps the sheet exact. Real exporters' sheets are plain (Illustrator, Inkscape, CorelDRAW,
/// Visio) or carry no such reference (`@keyframes`, comments).
///
/// Calibrated against real exporters' files (FT-418, `tests/fixtures/svg-exporters/`).
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
                // ASCII lowercasing preserves byte offsets; keep the original case for the ambiguity check.
                value = &text[j + 1..end];
                j = end + 1;
            }
            // XML normalizes literal attribute whitespace before decoding references. A literal TAB becomes a
            // space, while `&#9;` remains a TAB; graph keys and ambiguity comparisons must see the parser's values.
            let Some(value) = decode_attribute_refs(value) else {
                return Some(SvgHazard::Malformed);
            };
            // roxmltree keeps namespace declarations out of `attributes()` and usvg copies only None/SVG/XLink/XML
            // attributes, so `xmlns` and `xmlns:*` are not attributes of the element (review finding 8).
            if attr == "xmlns" || attr.starts_with("xmlns:") {
                continue;
            }
            if let Some(h) = attribute_hazard(attr, &value.to_ascii_lowercase()) {
                return Some(h);
            }
            attrs.push((local_name(attr), value));
        }
        if let Some(h) = scan.open(name, &mut attrs, self_closing) {
            return Some(h);
        }
        i = j;
    }
    scan.finish()
}

/// One element of the document, in document order: its parent's record, whether it carries a marker property
/// attribute, and its class tokens.
struct Record {
    parent: Option<usize>,
    marker_attr: bool,
    classes: Vec<String>,
}

/// A graph node (an element with an id, gradients and stops excepted).
struct Node {
    /// The elements of its subtree, itself included.
    size: u64,
    /// Its references and those of its subtree: target key → count.
    refs: std::collections::BTreeMap<String, u64>,
    /// Its element's record.
    record: usize,
    /// Whether an element of its subtree (itself included) carries a marker property attribute.
    marker_attr: bool,
    /// Whether its element is a `marker`.
    is_marker: bool,
}

/// The state of [`svg_hazard`]'s structural checks: the open elements, the record of every element, the reference
/// graph and the style sheet.
#[derive(Default)]
struct Scan {
    /// The open elements: local name, graph node (an element with an `id`, gradients and stops excepted) and record.
    open: Vec<(String, Option<usize>, usize)>,
    /// The graph node of each id.
    ids: std::collections::BTreeMap<String, usize>,
    /// Every non-empty id seen, gradients and stops included: a repeat is refused.
    seen_ids: std::collections::BTreeSet<String>,
    /// The graph nodes, in document order.
    nodes: Vec<Node>,
    /// Every element, in document order.
    records: Vec<Record>,
    /// References outside any node, and the elements of the document.
    top: (u64, std::collections::BTreeMap<String, u64>),
    /// The ids each class rule references, by class.
    class_targets: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    /// The classes whose rule names a marker property.
    marker_classes: std::collections::BTreeSet<String>,
    /// Whether a rule with a selector other than a plain class exists, and the ids its rules reference.
    css_wild: bool,
    wild_targets: std::collections::BTreeSet<String>,
    /// Whether a wild rule names a marker property.
    css_marker_wild: bool,
    /// The comments in style sheets.
    css_comments: usize,
    /// The upper bound of the selector groups of every style sheet so far.
    css_rules: u64,
    /// The declarations of every style sheet so far: exactly each plain rule's `groups × max(colons, 1)`, the raw
    /// `groups_bound × colons` for a sheet that is not plain.
    css_declarations: u64,
    /// The character data and CDATA text of the open `style` element, as roxmltree merges them.
    style_text: String,
    drop_shadows: usize,
    /// The `href` of each gradient, pattern and filter with an id: the parser follows these chains until a link
    /// returns to the current or the first element, so any other cycle in them never ends (review fix 5).
    chains: std::collections::BTreeMap<String, String>,
}

/// A frame of the reference walk: the node, its edges (target, count) and the next edge to follow.
type Frame = (usize, Vec<(usize, u64)>, usize);

impl Scan {
    fn open(&mut self, name: &str, attrs: &mut [(&str, String)], self_closing: bool) -> Option<SvgHazard> {
        if self.open.len() >= MAX_SVG_DEPTH {
            return Some(SvgHazard::TooComplex);
        }
        // usvg parses each `style` element's own text node; nesting one inside another would split the buffer.
        if name == "style" && self.open.iter().any(|(open, _, _)| open == "style") {
            return Some(SvgHazard::TooComplex);
        }
        // Namespace handling differs between the scanner and usvg. Refuse conflicting local id/class/href
        // values before building records or references, comparing exact XML-normalized decoded values (case kept).
        // Identical values are unambiguous, including a literal paired with its character-reference spelling.
        for local in ["id", "class", "href"] {
            let mut values = attrs
                .iter()
                .filter(|(attr, _)| *attr == local)
                .map(|(_, value)| value);
            if let Some(first) = values.next() {
                if values.any(|value| value != first) {
                    return Some(SvgHazard::TooComplex);
                }
            }
        }
        // All following hazard/graph logic retains the scanner's conservative ASCII-insensitive behaviour.
        for (_, value) in attrs.iter_mut() {
            value.make_ascii_lowercase();
        }
        let id = attrs
            .iter()
            .find(|(a, _)| *a == "id")
            .map(|(_, v)| v.trim().to_owned());
        if let Some(id) = &id {
            // usvg resolves `<use>` to the first element with an id and every `url(#…)` to the last; duplicate ids
            // are invalid SVG that no exporter writes, so the scanner refuses them rather than follow either.
            if !id.is_empty() && !self.seen_ids.insert(id.clone()) {
                return Some(SvgHazard::TooComplex);
            }
        }
        if let (Some(id), true) = (
            &id,
            matches!(name, "lineargradient" | "radialgradient" | "pattern" | "filter"),
        ) {
            let href = attrs
                .iter()
                .find(|(a, _)| *a == "href")
                .and_then(|(_, v)| v.trim().strip_prefix('#'));
            if let Some(target) = href {
                // The first chain of an id wins, as the parser's first element with the id does.
                self.chains
                    .entry(id.clone())
                    .or_insert_with(|| target.trim().to_owned());
            }
        }
        let record = self.records.len();
        self.records.push(Record {
            parent: self.open.last().map(|(_, _, record)| *record),
            marker_attr: false,
            classes: Vec::new(),
        });
        let node = match id {
            Some(id) if !matches!(name, "lineargradient" | "radialgradient" | "stop") && !id.is_empty() => {
                let k = self.nodes.len();
                self.nodes.push(Node {
                    size: 0,
                    refs: std::collections::BTreeMap::new(),
                    record,
                    marker_attr: false,
                    is_marker: name == "marker",
                });
                self.ids.entry(id).or_insert(k);
                Some(k)
            }
            _ => None,
        };
        self.open.push((name.to_owned(), node, record));
        self.count_element();
        for (attr, value) in attrs {
            let value = value.as_str();
            if *attr == "transform-origin" {
                return Some(SvgHazard::Unsupported);
            }
            if let (false, Some(h)) = (matches!(*attr, "id" | "class" | "href"), numbers(value)) {
                return Some(h);
            }
            if *attr == "d" && close_run(value) > 16 {
                return Some(SvgHazard::TooComplex);
            }
            if *attr == "style" && value.contains("transform-origin") {
                return Some(SvgHazard::Unsupported);
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
                let classes: Vec<String> = value.split_ascii_whitespace().map(str::to_owned).collect();
                for class in &classes {
                    self.reference(&format!("\u{0}class:{class}"));
                }
                self.records[record].classes = classes;
            }
            if matches!(*attr, "marker-start" | "marker-mid" | "marker-end" | "marker")
                || (*attr == "style" && value.contains("marker"))
            {
                self.records[record].marker_attr = true;
                // The element is already on the open stack: itself and every open node learn of the attribute.
                for (_, node, _) in &self.open {
                    if let Some(k) = node {
                        self.nodes[*k].marker_attr = true;
                    }
                }
            }
        }
        if self_closing {
            if let Some(h) = self.close() {
                return Some(h);
            }
        }
        None
    }

    /// Pop the innermost element; a closed `style` element's whole text is analysed once.
    fn close(&mut self) -> Option<SvgHazard> {
        if let Some((name, _, _)) = self.open.pop() {
            if name == "style" {
                let text = std::mem::take(&mut self.style_text);
                return self.style_sheet(&text);
            }
        }
        None
    }

    /// One more element in every open node's subtree and in the document.
    fn count_element(&mut self) {
        self.top.0 += 1;
        for (_, node, _) in &self.open {
            if let Some(k) = node {
                self.nodes[*k].size += 1;
            }
        }
    }

    /// A reference to `target` from the innermost element: an edge from every open node, or from the document.
    fn reference(&mut self, target: &str) {
        let mut any = false;
        for (_, node, _) in &self.open {
            if let Some(k) = node {
                *self.nodes[*k].refs.entry(target.to_owned()).or_insert(0) += 1;
                any = true;
            }
        }
        if !any {
            *self.top.1.entry(target.to_owned()).or_insert(0) += 1;
        }
    }

    /// Character data: outside `style` the text checks; inside it every chunk is kept for [`Scan::style_sheet`],
    /// which runs once when the element closes (roxmltree merges the chunks into one text node).
    fn text(&mut self, text: &str) -> Option<SvgHazard> {
        if self.open.last().is_some_and(|(name, _, _)| name == "style") {
            self.style_text.push_str(text);
            return None;
        }
        text_hazard(text)
    }

    /// The checks of one `<style>` element's whole text: the text hazards and per-sheet checks, the document-wide
    /// group and declaration bounds, and the references of the sheet (exactly when it is plain, as one wild rule
    /// otherwise).
    fn style_sheet(&mut self, text: &str) -> Option<SvgHazard> {
        if let Some(h) = text_hazard(text) {
            return Some(h);
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
        // The group bound without parsing: every selector group needs a `,` or a `{`. The declaration bound is
        // exact for a plain sheet (below); a sheet that is not plain keeps the raw product of the group bound and
        // the `:` count, a declaration needing a `:`.
        let groups_bound = text.matches(',').count() as u64 + text.matches('{').count() as u64;
        self.css_rules = self.css_rules.saturating_add(groups_bound);
        if self.css_rules > MAX_SVG_CSS_RULES {
            return Some(SvgHazard::TooComplex);
        }
        let Some((clean, plain)) = plain_style_sheet(text) else {
            return Some(SvgHazard::Malformed);
        };
        if !plain {
            // A string that can move a rule boundary, an escape, an at-rule or braces in comments can hide a rule
            // from the split, so the six-part check runs on the raw preludes (the text before each `{` back to the
            // previous `{` or `}`) and the whole sheet is one rule that may apply to every element.
            let mut at = 0;
            while let Some(k) = clean[at..].find(['{', '}']) {
                let delimiter = at + k;
                if clean.as_bytes()[delimiter] == b'{' && !prelude_within_part_cap(&clean[at..delimiter]) {
                    return Some(SvgHazard::TooComplex);
                }
                at = delimiter + 1;
            }
            self.css_declarations = self
                .css_declarations
                .saturating_add(groups_bound.saturating_mul(text.matches(':').count() as u64));
            if self.css_declarations > MAX_SVG_CSS_DECLARATIONS {
                return Some(SvgHazard::TooComplex);
            }
            self.css_wild = true;
            self.wild_targets.extend(url_targets(text));
            if text.contains("marker") {
                self.css_marker_wild = true;
            }
            return None;
        }
        // Rule preludes: the text before each `{` back to the previous `}`; its body up to the next `}`. The loop
        // records into locals first: text outside the bodies that names a reference or a marker makes the whole
        // sheet wild, and the class rules recorded before that decision stay (more edges are conservative).
        let mut class_targets: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
            std::collections::BTreeMap::new();
        let mut marker_classes = std::collections::BTreeSet::new();
        let mut wild = false;
        let mut wild_targets = std::collections::BTreeSet::new();
        let mut marker_wild = false;
        // The declarations simplecss clones into the rule groups of this sheet, exactly.
        let mut declarations = 0u64;
        let mut outside = String::new();
        let mut rest = clean.as_str();
        while let Some(open) = rest.find('{') {
            let prelude = rest[..open].rsplit('}').next().unwrap_or("");
            outside.push_str(prelude);
            if !prelude_within_part_cap(prelude) {
                return Some(SvgHazard::TooComplex);
            }
            let groups: Vec<&str> = prelude.split(',').collect();
            let after = &rest[open + 1..];
            let (body, next) = match after.find('}') {
                Some(close) => (&after[..close], Some(&after[close + 1..])),
                None => (after, None),
            };
            declarations = declarations.saturating_add(
                (groups.len() as u64).saturating_mul((body.matches(':').count() as u64).max(1)),
            );
            let targets = url_targets(body);
            let carries_marker = body.contains("marker");
            // A class rule selects only plain class names; every other selector is a wild rule.
            let class_rule = groups.iter().all(|g| {
                g.trim().strip_prefix('.').is_some_and(|class| {
                    !class.is_empty()
                        && class
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
                })
            });
            if class_rule {
                for g in &groups {
                    if let Some(class) = g.trim().strip_prefix('.') {
                        class_targets
                            .entry(class.to_owned())
                            .or_default()
                            .extend(targets.iter().cloned());
                        if carries_marker {
                            marker_classes.insert(class.to_owned());
                        }
                    }
                }
            } else {
                wild = true;
                wild_targets.extend(targets);
                if carries_marker {
                    marker_wild = true;
                }
            }
            match next {
                Some(next) => rest = next,
                None => {
                    rest = "";
                    break;
                }
            }
        }
        self.css_declarations = self.css_declarations.saturating_add(declarations);
        if self.css_declarations > MAX_SVG_CSS_DECLARATIONS {
            return Some(SvgHazard::TooComplex);
        }
        outside.push_str(rest);
        if outside.contains("url(") || outside.contains("marker") {
            wild = true;
            wild_targets.extend(url_targets(text));
            marker_wild |= text.contains("marker");
        }
        for (class, targets) in class_targets {
            self.class_targets.entry(class).or_default().extend(targets);
        }
        self.marker_classes.extend(marker_classes);
        self.css_wild |= wild;
        self.wild_targets.extend(wild_targets);
        self.css_marker_wild |= marker_wild;
        None
    }

    /// The checks that need the whole document: style-sheet totals, marker rules, cycles and the expansion.
    fn finish(mut self) -> Option<SvgHazard> {
        // A `style` element still open at the end of the file is analysed too.
        if self.open.iter().any(|(name, _, _)| name == "style") {
            let text = std::mem::take(&mut self.style_text);
            if let Some(h) = self.style_sheet(&text) {
                return Some(h);
            }
        }
        if self.css_comments > 64 || self.drop_shadows > 16 {
            return Some(SvgHazard::TooComplex);
        }
        if chain_hazard(&self.chains) {
            return Some(SvgHazard::TooComplex);
        }
        // The graph: node k, then one class node per class rule (in the map's order), then the wild node.
        let n = self.nodes.len();
        let class_keys: Vec<&str> = self.class_targets.keys().map(String::as_str).collect();
        let class_index: std::collections::BTreeMap<&str, usize> = class_keys
            .iter()
            .enumerate()
            .map(|(position, class)| (*class, n + position))
            .collect();
        let wild = n + class_keys.len();
        let total_nodes = wild + usize::from(self.css_wild);
        let index = |key: &str| -> Option<usize> {
            if let Some(class) = key.strip_prefix("\u{0}class:") {
                class_index.get(class).copied()
            } else {
                self.ids.get(key).copied()
            }
        };
        let edges = |k: usize| -> Vec<(usize, u64)> {
            if k < n {
                let mut out: Vec<(usize, u64)> = self.nodes[k]
                    .refs
                    .iter()
                    .filter_map(|(target, count)| index(target).map(|j| (j, *count)))
                    .collect();
                if self.css_wild {
                    out.push((wild, 1));
                }
                out
            } else if let Some(class) = class_keys.get(k - n) {
                self.class_targets
                    .get(*class)
                    .map(|targets| {
                        targets
                            .iter()
                            .filter_map(|target| index(target).map(|j| (j, 1)))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                self.wild_targets
                    .iter()
                    .filter_map(|target| index(target).map(|j| (j, 1)))
                    .collect()
            }
        };
        let size = |k: usize| -> u64 { if k < n { self.nodes[k].size } else { 0 } };
        // Iterative depth-first walk: a grey node met again is a cycle; a black node's cost is final.
        let mut cost: Vec<Option<u64>> = vec![None; total_nodes];
        let mut depth = vec![0usize; total_nodes];
        let mut grey = vec![false; total_nodes];
        for root in 0..total_nodes {
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
                // The longest chain of references from this node (class and wild nodes are no link of their own).
                let below = out.iter().map(|(j, _)| depth[*j]).max().unwrap_or(0);
                depth[k] = below + usize::from(k < n);
                if depth[k] > MAX_SVG_REF_DEPTH {
                    return Some(SvgHazard::TooComplex);
                }
                cost[k] = Some(total);
                grey[k] = false;
                stack.pop();
            }
        }
        // Marker content may not itself carry a marker property, from any source: a marker whose subtree holds
        // such an element, or a class or wild rule that names one, can nest marker expansion without bound (usvg
        // inherits the marker properties; only the same marker re-entering itself is stopped).
        if self.nodes.iter().any(|node| node.is_marker) {
            if self.css_marker_wild {
                return Some(SvgHazard::TooComplex);
            }
            let mut chain_bad = vec![false; self.records.len()];
            for (r, record) in self.records.iter().enumerate() {
                chain_bad[r] = record.marker_attr
                    || record.classes.iter().any(|c| self.marker_classes.contains(c))
                    || record.parent.is_some_and(|parent| chain_bad[parent]);
            }
            let bad = |k: usize| -> bool {
                if k < n {
                    chain_bad[self.nodes[k].record] || self.nodes[k].marker_attr
                } else if let Some(class) = class_keys.get(k - n) {
                    self.marker_classes.contains(*class)
                } else {
                    self.css_marker_wild
                }
            };
            // One breadth-first search from every marker node at once, over the graph above.
            let mut seen = vec![false; total_nodes];
            let mut queue = std::collections::VecDeque::new();
            for (k, node) in self.nodes.iter().enumerate() {
                if node.is_marker {
                    seen[k] = true;
                    queue.push_back(k);
                }
            }
            while let Some(k) = queue.pop_front() {
                if bad(k) {
                    return Some(SvgHazard::TooComplex);
                }
                for (j, _) in edges(k) {
                    if !seen[j] {
                        seen[j] = true;
                        queue.push_back(j);
                    }
                }
            }
        }
        let mut total = self.top.1.iter().fold(self.top.0, |acc, (t, c)| {
            let k = index(t);
            acc.saturating_add(c.saturating_mul(k.and_then(|k| cost[k]).unwrap_or(0)))
        });
        if self.css_wild {
            total = total.saturating_add(cost[wild].unwrap_or(0));
        }
        (total > MAX_SVG_EXPANSION).then_some(SvgHazard::TooComplex)
    }
}

/// Whether every selector group of a `,`-separated prelude has at most six compound parts (white space, `>`, `+`
/// and `~` separate parts): usvg matches every rule against every element, and a deeper descendant selector costs
/// combinations of ancestor chains.
fn prelude_within_part_cap(prelude: &str) -> bool {
    prelude.split(',').all(|group| {
        group
            .split(|c: char| c.is_whitespace() || matches!(c, '>' | '+' | '~'))
            .filter(|part| !part.is_empty())
            .count()
            <= 6
    })
}

/// `text` with every comment removed (replaced by nothing), and whether the sheet is plain for the exact rule model:
/// outside comments it has no `\` and no `@`, no unclosed string and no string whose content holds `{`, `}`, `;`,
/// `(`, `)`, `\`, `,`, `/*` or `*/`, no comment holds a `{` or a `}`, and its braces alternate
/// `{`, `}`, `{`, `}`, … starting with `{` (a final `{` without its `}` is allowed). `None` for an unclosed comment.
fn plain_style_sheet(text: &str) -> Option<(String, bool)> {
    let b = text.as_bytes();
    let mut clean = Vec::with_capacity(b.len());
    let mut plain = true;
    // The index of the `)` whose span was last checked: a `(` below it lies inside that span.
    let mut checked_until = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            let start = i + 2;
            let end = text[start..].find("*/").map(|end| start + end)?;
            if b[start..end].iter().any(|c| matches!(c, b'{' | b'}')) {
                plain = false;
            }
            i = end + 2;
            continue;
        }
        match b[i] {
            b'"' | b'\'' => {
                // simplecss reads a quoted string as one value token. A string whose content holds a brace, a
                // semicolon or parenthesis, a comment delimiter, a backslash or a comma (the group separator), or
                // that is not closed, can move a rule boundary and makes the sheet not plain; any other string keeps
                // it plain.
                let quote = b[i];
                let start = i + 1;
                clean.push(quote);
                i += 1;
                let mut closed = false;
                while i < b.len() {
                    if b[i] == b'\\' {
                        clean.push(b[i]);
                        i += 1;
                        if i < b.len() {
                            clean.push(b[i]);
                            i += 1;
                        }
                        continue;
                    }
                    clean.push(b[i]);
                    i += 1;
                    if b[i - 1] == quote {
                        closed = true;
                        break;
                    }
                }
                let content = &text[start..i - usize::from(closed)];
                if !closed
                    || content
                        .bytes()
                        .any(|c| matches!(c, b'{' | b'}' | b';' | b'(' | b')' | b'\\' | b','))
                    || content.contains("/*")
                    || content.contains("*/")
                {
                    plain = false;
                }
            }
            b'(' => {
                // simplecss skips a function value to the next `)`: a brace or a `;` inside one would make the
                // scanner's body split disagree with the parser. A `(` below the end of the span already checked
                // lies inside the same `)`, so its span is checked once.
                if i >= checked_until {
                    if let Some(end) = text[i + 1..].find(')') {
                        let end = i + 1 + end;
                        let inside = &text[i + 1..end];
                        if inside.bytes().any(|c| matches!(c, b'{' | b'}' | b';')) || inside.contains("/*") {
                            plain = false;
                        }
                        checked_until = end;
                    } else {
                        plain = false;
                        checked_until = b.len();
                    }
                }
                clean.push(b[i]);
                i += 1;
            }
            b'\\' => {
                plain = false;
                clean.push(b[i]);
                i += 1;
            }
            b'@' => {
                plain = false;
                clean.push(b[i]);
                i += 1;
            }
            _ => {
                clean.push(b[i]);
                i += 1;
            }
        }
    }
    let clean = String::from_utf8(clean).ok()?;
    if plain {
        let mut open = false;
        for c in clean.bytes() {
            match c {
                b'{' if open => {
                    plain = false;
                    break;
                }
                b'{' => open = true,
                b'}' if !open => {
                    plain = false;
                    break;
                }
                b'}' => open = false,
                _ => {}
            }
        }
    }
    Some((clean, plain))
}

/// Whether the `href` chains of gradients, patterns and filters (id to target) hold a cycle or a chain of more than
/// [`MAX_SVG_REF_DEPTH`] elements: the parser walks a chain once per attribute of each element on it (without
/// recursion), so a long cycle-free chain costs the square of its length (review round 2), and a cycle that returns
/// to neither end never ends (review fix 5). Each id has at most one link: one walk per id, each id finished once
/// with the length of its chain, so the whole check is linear in the number of ids.
fn chain_hazard(chains: &std::collections::BTreeMap<String, String>) -> bool {
    // The length (elements, itself included) of the chain from each finished id; `None` while on the current walk.
    let mut length: std::collections::BTreeMap<&str, Option<usize>> = std::collections::BTreeMap::new();
    for start in chains.keys() {
        let mut walk: Vec<&str> = Vec::new();
        let mut at = start.as_str();
        // The length of the chain after the walk's last element.
        let tail = loop {
            match length.get(at) {
                Some(None) => return true,
                Some(Some(n)) => break *n,
                None => {}
            }
            length.insert(at, None);
            walk.push(at);
            match chains.get(at) {
                Some(next) => at = next.as_str(),
                // The last element of the chain (its target, or an id without a link): one more element.
                None => break 0,
            }
        };
        let mut n = tail;
        for id in walk.into_iter().rev() {
            n += 1;
            if n > MAX_SVG_REF_DEPTH {
                return true;
            }
            length.insert(id, Some(n));
        }
    }
    false
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

    /// FT-418: a hex colour or a fragment name is skipped whole, so its digits are not an exponent; a `.` right
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

    /// FT-418 final review: a number is read with the SVG grammar, so a compressed run (`0.5.5e39`) is checked as
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

    /// FT-418 final review: roxmltree does not list namespace declarations as attributes and usvg copies none of
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
    fn namespace_shadowed_ids_do_not_hide_reference_cycles() {
        let shadowed = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:f="urn:ignored" viewBox="0 0 4 4"><linearGradient f:id="d1" id="g1" href="#g2"/><linearGradient f:id="d2" id="g2" href="#g3"/><radialGradient f:id="d3" id="g3" href="#g2"/><rect width="4" height="4" fill="url(#g1)"/></svg>"##;
        let decoy_after = shadowed
            .replace(r#"f:id="d1" id="g1""#, r#"id="g1" f:id="d1""#)
            .replace(r#"f:id="d2" id="g2""#, r#"id="g2" f:id="d2""#)
            .replace(r#"f:id="d3" id="g3""#, r#"id="g3" f:id="d3""#);
        let ordinary = shadowed
            .replace(r#"f:id="d1" "#, "")
            .replace(r#"f:id="d2" "#, "")
            .replace(r#"f:id="d3" "#, "");
        // Scanner and sniff only: this audited hostile cycle must never reach usvg.
        for text in [ordinary.as_str(), shadowed, decoy_after.as_str()] {
            assert_eq!(svg_hazard(text.as_bytes()), Some(SvgHazard::TooComplex), "{text}");
            assert_eq!(
                sniff(text.as_bytes()),
                Err(AssetError::UnsafeSvg(SvgHazard::TooComplex)),
                "{text}"
            );
        }
    }

    #[test]
    fn conflicting_local_id_class_and_href_attributes_are_refused() {
        for namespace in [
            "urn:ignored",
            "http://www.w3.org/2000/svg",
            "http://www.w3.org/1999/xlink",
            "http://www.w3.org/XML/1998/namespace",
        ] {
            let prefix = if namespace == "http://www.w3.org/XML/1998/namespace" {
                "xml"
            } else {
                "f"
            };
            for attr in ["id", "class", "href"] {
                // The leading # keeps every href local; case and whitespace differences must remain conflicts.
                for (first, second, hazard) in [
                    ("#first", "#second", Some(SvgHazard::TooComplex)),
                    ("#First", "#first", Some(SvgHazard::TooComplex)),
                    ("#first", "#first ", Some(SvgHazard::TooComplex)),
                    ("#First", "#First", None),
                    ("#First", "#&#70;irst", None),
                    ("#first\tsecond", "#first second", None),
                    ("#first\rsecond", "#first second", None),
                    ("#first\nsecond", "#first second", None),
                    ("#first\r\nsecond", "#first second", None),
                    ("#first\tsecond", "#first&#9;second", Some(SvgHazard::TooComplex)),
                    ("#first\rsecond", "#first&#13;second", Some(SvgHazard::TooComplex)),
                    ("#first\nsecond", "#first&#10;second", Some(SvgHazard::TooComplex)),
                    (
                        "#first\r\nsecond",
                        "#first&#13;&#10;second",
                        Some(SvgHazard::TooComplex),
                    ),
                ] {
                    for prefixed_first in [false, true] {
                        let plain = format!(r#"{attr}="{first}""#);
                        let prefixed = format!(r#"{prefix}:{attr}="{second}""#);
                        let attributes = if prefixed_first {
                            format!("{prefixed} {plain}")
                        } else {
                            format!("{plain} {prefixed}")
                        };
                        let text = format!(
                            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:{prefix}="{namespace}" viewBox="0 0 4 4"><g {attributes}/></svg>"#
                        );
                        assert_eq!(svg_hazard(text.as_bytes()), hazard, "{text}");
                        assert_eq!(
                            sniff(text.as_bytes()),
                            hazard.map_or(Ok((AssetFormat::Svg, Some((4, 4)))), |h| Err(
                                AssetError::UnsafeSvg(h)
                            )),
                            "{text}"
                        );
                    }
                }
            }
        }
        let declarations = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:id="urn:id" xmlns:class="urn:class" xmlns:href="urn:href" id="drawing" class="drawing" viewBox="0 0 4 4"/>"#;
        assert_eq!(svg_hazard(declarations.as_bytes()), None);
        assert_eq!(
            sniff(declarations.as_bytes()),
            Ok((AssetFormat::Svg, Some((4, 4))))
        );
    }

    #[test]
    fn normalized_attribute_ids_do_not_hide_reference_cycles() {
        // Scanner/sniff only: literal whitespace becomes a space before decoding character references.
        for (literal, reference) in [
            ("\t", "&#9;"),
            ("\r", "&#13;"),
            ("\n", "&#10;"),
            ("\r\n", "&#13;&#10;"),
        ] {
            for decoys in [None, Some(false), Some(true)] {
                let attributes = |n| {
                    let id = format!("id=\"g{literal}{n}\"");
                    let decoy = format!("f:id=\"g{reference}{n}\"");
                    match decoys {
                        None => id,
                        Some(false) => format!("{decoy} {id}"),
                        Some(true) => format!("{id} {decoy}"),
                    }
                };
                let text = format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:f="urn:ignored" viewBox="0 0 4 4"><linearGradient {} href="#g 2"/><linearGradient {} href="#g 3"/><radialGradient {} href="#g 2"/><rect width="4" height="4" fill="url('#g 1')"/></svg>"##,
                    attributes(1),
                    attributes(2),
                    attributes(3)
                );
                assert_eq!(
                    svg_hazard(text.as_bytes()),
                    Some(SvgHazard::TooComplex),
                    "{text:?}"
                );
                assert_eq!(
                    sniff(text.as_bytes()),
                    Err(AssetError::UnsafeSvg(SvgHazard::TooComplex)),
                    "{text:?}"
                );
            }
        }
        // A referenced TAB remains a TAB in both the id and quoted funcIRI; its self-reference is a cycle.
        let numeric_cycle = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><g id="g&#9;1" fill="url('#g&#9;1')"/></svg>"#;
        assert_eq!(svg_hazard(numeric_cycle.as_bytes()), Some(SvgHazard::TooComplex));
        assert_eq!(
            sniff(numeric_cycle.as_bytes()),
            Err(AssetError::UnsafeSvg(SvgHazard::TooComplex))
        );
        // Literal TAB normalizes to a space, so it differs from the referenced TAB in the id and is safe.
        let safe = numeric_cycle.replace("url('#g&#9;1')", "url('#g\t1')");
        assert_eq!(svg_hazard(safe.as_bytes()), None);
        assert_eq!(sniff(safe.as_bytes()), Ok((AssetFormat::Svg, Some((4, 4)))));
    }

    #[test]
    fn ordinary_exporter_namespace_attributes_still_pass() {
        let text = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:serif="http://www.serif.com/" inkscape:version="1.2.3" serif:version="2.0" viewBox="0 0 4 4"><g id="layer" inkscape:label="Layer 1" serif:id="layer"/></svg>"#;
        assert_eq!(svg_hazard(text.as_bytes()), None);
        assert_eq!(sniff(text.as_bytes()), Ok((AssetFormat::Svg, Some((4, 4)))));
    }

    /// FT-418: a DOCTYPE whose internal subset holds a `>` (an entity, an attribute list) is scanned with its
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

    /// FT-418: a style sheet's references follow the classes and selectors that use them; the group and declaration
    /// caps are upper bounds counted without parsing, and a rule with any other selector is followed from every
    /// element.
    #[test]
    fn style_sheet_references_follow_the_classes_and_selectors_that_use_them() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4""#;
        let cases: Vec<(&str, String, Option<SvgHazard>)> = vec![
            (
                "a pattern through its class",
                format!(
                    r#"<svg {NS}><style>.p {{ fill: url(#pat) }}</style><pattern id="pat" width="1" height="1"><rect class="p" width="1" height="1"/></pattern></svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "a clip path through a wild rule",
                format!(
                    r#"<svg {NS}><style>rect {{ clip-path: url(#c) }}</style><clipPath id="c"><rect width="1" height="1"/></clipPath></svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "two classes, one without references",
                format!(
                    r#"<svg {NS}><style>.a {{ fill: url(#p) }} .b {{ fill: #fff }}</style><pattern id="p" width="1" height="1"><rect class="b" width="1" height="1"/></pattern><rect class="a" width="4" height="4"/></svg>"#
                ),
                None,
            ),
            (
                "a wild rule that references a gradient",
                format!(
                    r#"<svg {NS}><style>rect {{ fill: url(#g) }}</style><linearGradient id="g"><stop offset="0"/></linearGradient><rect width="1" height="1"/></svg>"#
                ),
                None,
            ),
            (
                "fifty-five selector groups",
                format!(
                    r#"<svg {NS}><style>{}</style></svg>"#,
                    (0..55).map(|k| format!(".c{k}")).collect::<Vec<_>>().join(",") + " { fill: red }"
                ),
                None,
            ),
            (
                "five thousand selector groups",
                format!(
                    r#"<svg {NS}><style>{}</style></svg>"#,
                    (0..5_000).map(|k| format!(".c{k}")).collect::<Vec<_>>().join(",") + " { fill: red }"
                ),
                Some(SvgHazard::TooComplex),
            ),
        ];
        for (what, text, expected) in &cases {
            assert_eq!(svg_hazard(text.as_bytes()), *expected, "{what}");
        }
    }

    /// FT-418: comments are removed without a space before the rules are read, so a comment is no selector part; an
    /// unclosed comment is markup the scanner cannot follow, and a brace in a comment makes the sheet wild.
    #[test]
    fn a_comment_is_not_a_selector_part() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4""#;
        let cases: Vec<(&str, String, Option<SvgHazard>)> = vec![
            (
                "a comment removed without a space",
                format!(
                    r#"<svg {NS}><style>.a/**/b {{ fill: url(#p) }}</style><pattern id="p" width="1" height="1"><rect class="a" width="1" height="1"/></pattern><rect class="a" width="4" height="4"/></svg>"#
                ),
                None,
            ),
            (
                "a long comment before a class rule",
                format!(
                    r#"<svg {NS}><style>/* a, b c d e f g h */ .a {{ fill: url(#p) }}</style><pattern id="p" width="1" height="1"><rect class="b" width="1" height="1"/></pattern><rect class="a" width="4" height="4"/></svg>"#
                ),
                None,
            ),
            (
                "an unclosed comment",
                format!(r#"<svg {NS}><style>/* unclosed .a {{ fill: red }}</style></svg>"#),
                Some(SvgHazard::Malformed),
            ),
            (
                "a brace in a comment",
                format!(
                    r#"<svg {NS}><style>/* {{ */ .k {{ marker-end: url(#n) }}</style><marker id="m"><path d="M0 0 L1 1"/></marker><marker id="n"><path d="M0 0 L1 1"/></marker></svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
        ];
        for (what, text, expected) in &cases {
            assert_eq!(svg_hazard(text.as_bytes()), *expected, "{what}");
        }
    }

    /// FT-418 review: roxmltree merges adjacent character data and CDATA sections into one text node, and usvg
    /// parses that merged text; the scanner analyses the whole `<style>` element once.
    #[test]
    fn style_text_is_read_whole_across_cdata_sections() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 4 4""#;
        let split_marker = format!(
            r#"<svg {NS}><style><![CDATA[.k {{ ]]><![CDATA[marker-end: url(#n) }}]]></style><marker id="m"><path class="k" d="M0 0 L1 1 L2 0"/></marker><marker id="n"><path class="k" d="M0 0 L1 1 L2 0"/></marker><path d="M0 0 L4 4" marker-end="url(#m)"/></svg>"#
        );
        assert_eq!(svg_hazard(split_marker.as_bytes()), Some(SvgHazard::TooComplex));
        let split_transform = format!(
            r#"<svg {NS}><style><![CDATA[rect {{ transform-]]><![CDATA[origin: - }}]]></style></svg>"#
        );
        assert_eq!(
            svg_hazard(split_transform.as_bytes()),
            Some(SvgHazard::Unsupported)
        );
        let mut chain = format!(r#"<svg {NS}>"#);
        for i in 1..=30 {
            let next = i + 1;
            chain.push_str(&format!(
                r#"<style><![CDATA[.k{i} {{ ]]><![CDATA[marker-end: url(#m{next}) }}]]></style>"#
            ));
            chain.push_str(&format!(
                r#"<marker id="m{i}"><path class="k{i}" d="M0 0 L1 1"/><path class="k{i}" d="M0 0 L1 1"/></marker>"#
            ));
        }
        chain.push_str(r#"<path d="M0 0 L4 4" style="marker-end: url(#m1)"/></svg>"#);
        assert_eq!(svg_hazard(chain.as_bytes()), Some(SvgHazard::TooComplex));
    }

    /// FT-418 review: simplecss honours quoted strings; a sheet with a string is read conservatively as one wild
    /// rule, so a hidden marker or reference is still refused.
    #[test]
    fn css_strings_make_the_sheet_wild() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 4 4""#;
        let cases: Vec<(&str, String, Option<SvgHazard>)> = vec![
            (
                "a quoted brace hides the marker declaration",
                format!(
                    r#"<svg {NS}><style>.k {{ font-family: "}}"; marker-end: url(#n) }}</style><marker id="m"><path class="k" d="M0 0 L1 1 L2 0"/></marker><marker id="n"><path d="M0 0 L1 1"/></marker><path d="M0 0 L4 4" marker-end="url(#m)"/></svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "a quoted comment start hides the marker rule",
                format!(
                    r#"<svg {NS}><style>.a {{ font-family: "/*"; }} .k {{ marker-end: url(#n) }} /* */</style><marker id="m"><path class="k" d="M0 0 L1 1 L2 0"/></marker><marker id="n"><path d="M0 0 L1 1"/></marker><path d="M0 0 L4 4" marker-end="url(#m)"/></svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "three patterns behind a quoted brace",
                format!(
                    r#"<svg {NS}><style>.p1 {{ font-family: "}}"; fill: url(#pat2) }} .p2 {{ font-family: "}}"; fill: url(#pat3) }} .p3 {{ font-family: "}}"; fill: url(#pat1) }}</style><pattern id="pat1" width="1" height="1"><rect class="p1" width="1" height="1"/></pattern><pattern id="pat2" width="1" height="1"><rect class="p2" width="1" height="1"/></pattern><pattern id="pat3" width="1" height="1"><rect class="p3" width="1" height="1"/></pattern><rect width="1" height="1" fill="url(#pat1)"/></svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "a quoted comment start without a target",
                format!(r#"<svg {NS}><style>.a {{ font-family: "/*"; fill: red }}</style></svg>"#),
                None,
            ),
        ];
        for (what, text, expected) in &cases {
            assert_eq!(svg_hazard(text.as_bytes()), *expected, "{what}");
        }
    }

    /// FT-418 final review 1 and 5: a funcIRI target is read as svgtypes reads it — a quoted id runs to its closing
    /// quote and keeps its inner white space, and the scan continues past the target — so a quoted reference cycle
    /// is seen and the scan is linear. The 8,192-deep `url(#` attribute only calls the scanner: this guards against
    /// the quadratic scan; neither its result nor the time is asserted.
    #[test]
    fn url_targets_read_quoted_ids_whole_and_in_linear_time() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4""#;
        let pattern_cycle = format!(
            r#"<svg {NS}><defs><pattern id="pat a" width="1" height="1"><rect width="1" height="1" fill="url('#pat b')"/></pattern><pattern id="pat b" width="1" height="1"><rect width="1" height="1" fill="url('#pat c')"/></pattern><pattern id="pat c" width="1" height="1"><rect width="1" height="1" fill="url('#pat a')"/></pattern></defs><rect width="4" height="4" fill="url('#pat a')"/></svg>"#
        );
        assert_eq!(
            svg_hazard(pattern_cycle.as_bytes()),
            Some(SvgHazard::TooComplex),
            "the quoted pattern cycle"
        );
        let clip_cycle = format!(
            r#"<svg {NS}><clipPath id="clip a"><rect width="1" height="1" clip-path="url('#clip b')"/></clipPath><clipPath id="clip b"><rect width="1" height="1" clip-path="url('#clip c')"/></clipPath><clipPath id="clip c"><rect width="1" height="1" clip-path="url('#clip a')"/></clipPath><rect width="4" height="4" clip-path="url('#clip a')"/></svg>"#
        );
        assert_eq!(
            svg_hazard(clip_cycle.as_bytes()),
            Some(SvgHazard::TooComplex),
            "the quoted clip-path cycle"
        );
        let gradient = format!(
            r#"<svg {NS}><linearGradient id="g"><stop offset="0"/></linearGradient><rect width="1" height="1" fill="url( '#g' )"/></svg>"#
        );
        assert_eq!(svg_hazard(gradient.as_bytes()), None, "a spaced quoted gradient");
        let quadratic = format!(
            r#"<svg {NS}><rect width="1" height="1" fill="url(#{})"/></svg>"#,
            "url(#".repeat(8_191)
        );
        let _ = svg_hazard(quadratic.as_bytes());
    }

    /// FT-418 final review 6: a string that holds nothing that can move a rule boundary keeps a style sheet plain;
    /// a string with a brace, semicolon, parenthesis, comment delimiter or backslash makes it wild as before. The
    /// 20,000-`(` sheet (the lead's quadratic path finding) only calls the scanner; no time is asserted.
    #[test]
    fn harmless_strings_keep_a_sheet_plain() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4""#;
        let quoted_font = format!(
            r##"<svg {NS}><style>.txt {{ font-family: "Arial" }} .st0 {{ fill: url(#pat) }}</style><pattern id="pat" width="1" height="1"><rect width="1" height="1" fill="#fff"/></pattern><rect class="st0" width="1" height="1"/></svg>"##
        );
        assert_eq!(svg_hazard(quoted_font.as_bytes()), None, "a harmless string");
        let brace_font = format!(
            r##"<svg {NS}><style>.txt {{ font-family: "a}}b" }} .st0 {{ fill: url(#pat) }}</style><pattern id="pat" width="1" height="1"><rect width="1" height="1" fill="#fff"/></pattern><rect class="st0" width="1" height="1"/></svg>"##
        );
        assert_eq!(
            svg_hazard(brace_font.as_bytes()),
            Some(SvgHazard::TooComplex),
            "a string that moves a rule boundary"
        );
        let marker_class = format!(
            r#"<svg {NS}><style>.k {{ font-family: "x" ; marker-end: url(#n) }}</style><marker id="m"><path class="k" d="M0 0 L1 1"/></marker><marker id="n"><path d="M0 0 L1 1"/></marker></svg>"#
        );
        assert_eq!(
            svg_hazard(marker_class.as_bytes()),
            Some(SvgHazard::TooComplex),
            "the marker class is still seen"
        );
        let parentheses = format!(r#"<svg {NS}><style>{}</style></svg>"#, "(".repeat(20_000) + ")");
        assert_eq!(
            svg_hazard(parentheses.as_bytes()),
            None,
            "20,000 open parentheses"
        );
    }

    /// FT-418 re-review: simplecss skips a function value to the next `)` and parses text the scanner's body split
    /// does not attribute to a rule; both make the sheet wild.
    #[test]
    fn plain_sheets_hide_nothing_in_parentheses_or_outside_bodies() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 4 4""#;
        let marker = format!(
            r#"<svg {NS}><style>.b {{ fill: var(x }} ) ; marker-end: url(#n)</style><marker id="m"><path class="b" d="M0 0 L1 1 L2 0"/></marker><marker id="n"><path d="M0 0 L4 4"/></marker><path d="M0 0 L1 1 L2 0" marker-mid="url(#m)"/></svg>"#
        );
        assert_eq!(svg_hazard(marker.as_bytes()), Some(SvgHazard::TooComplex));
        let cycle = format!(
            r#"<svg {NS}><style>.b {{ fill: var(x }} ) ; clip-path: url(#a)</style><clipPath id="a"><rect width="1" height="1" clip-path="url(#b)"/></clipPath><clipPath id="b"><rect width="1" height="1" clip-path="url(#c)"/></clipPath><clipPath id="c"><rect class="b" width="1" height="1"/></clipPath><rect width="1" height="1" clip-path="url(#a)"/></svg>"#
        );
        assert_eq!(svg_hazard(cycle.as_bytes()), Some(SvgHazard::TooComplex));
        let rgb = format!(
            r#"<svg {NS}><style>.a {{ fill: rgb(1, 2, 3) }}</style><rect class="a" width="1" height="1"/></svg>"#
        );
        assert_eq!(svg_hazard(rgb.as_bytes()), None);
    }

    /// FT-418 re-review: a `style` element inside a `style` element would split the gathered text; it is refused.
    #[test]
    fn a_style_inside_a_style_is_refused() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 4 4""#;
        let nested = format!(
            r#"<svg {NS}><style>.a {{ fill: red }} /*<style>*/ .b {{ marker-end: url(#n) }}</style><marker id="m"><path d="M0 0 L1 1 L2 0"/></marker><marker id="n"><path d="M0 0 L4 4"/></marker><path d="M0 0 L1 1 L2 0" marker-mid="url(#m)"/></svg>"#
        );
        assert_eq!(svg_hazard(nested.as_bytes()), Some(SvgHazard::TooComplex));
    }

    /// FT-418 review: the selector-group and declaration bounds are counted without parsing, so quoted braces and
    /// hidden declarations cannot shrink them.
    #[test]
    fn css_bounds_need_no_parsing() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4""#;
        let groups = |prefix: &str, n: usize| {
            (0..n)
                .map(|k| format!("{prefix}{k}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut hidden = groups("g", 4_096);
        hidden.push_str(" { font-family: \"}\";");
        hidden.push_str(&"a:1;".repeat(10_000));
        hidden.push('}');
        let mut over = groups(".c", 2_048);
        over.push_str(" {");
        over.push_str(&"a:1;".repeat(129));
        over.push('}');
        let mut under = groups(".c", 2_048);
        under.push_str(" {");
        under.push_str(&"a:1;".repeat(127));
        under.push('}');
        let cases: Vec<(&str, String, Option<SvgHazard>)> = vec![
            (
                "a quoted brace hiding ten thousand declarations",
                format!(r#"<svg {NS}><style>{hidden}</style></svg>"#),
                Some(SvgHazard::TooComplex),
            ),
            (
                "4,096 groups, at the group bound",
                format!(
                    r#"<svg {NS}><style>{} {{ fill: red }}</style></svg>"#,
                    groups(".c", 4_096)
                ),
                None,
            ),
            (
                "4,097 groups, over the group bound",
                format!(
                    r#"<svg {NS}><style>{} {{ fill: red }}</style></svg>"#,
                    groups(".c", 4_097)
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "the declaration bound alone, over",
                format!(r#"<svg {NS}><style>{over}</style></svg>"#),
                Some(SvgHazard::TooComplex),
            ),
            (
                "the declaration bound alone, under",
                format!(r#"<svg {NS}><style>{under}</style></svg>"#),
                None,
            ),
        ];
        for (what, text, expected) in &cases {
            assert_eq!(svg_hazard(text.as_bytes()), *expected, "{what}");
        }
    }

    /// FT-418 final review 7: a plain sheet's declaration bound is the exact count of the copies simplecss makes
    /// (each rule's selector groups times its declarations), not the raw group bound times the `:` count.
    #[test]
    fn plain_sheets_count_their_declarations_exactly() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4""#;
        let rules = |n: usize, declarations: usize| {
            (0..n)
                .map(|k| format!(".st{k} {{ {} }}", "a:1;".repeat(declarations)))
                .collect::<String>()
        };
        let review = format!(r#"<svg {NS}><style>{}</style></svg>"#, rules(101, 26));
        assert_eq!(svg_hazard(review.as_bytes()), None, "101 rules, 26 declarations");
        let many = format!(r#"<svg {NS}><style>{}</style></svg>"#, rules(200, 10));
        assert_eq!(svg_hazard(many.as_bytes()), None, "200 rules, 10 declarations");
        let over = format!(
            r#"<svg {NS}><style>{} {{ {} }}</style></svg>"#,
            (0..2_048).map(|k| format!(".c{k}")).collect::<Vec<_>>().join(","),
            "a:1;".repeat(129)
        );
        assert_eq!(
            svg_hazard(over.as_bytes()),
            Some(SvgHazard::TooComplex),
            "2,048 groups, 129 declarations"
        );
    }

    /// FT-418 final review (regression of fix round 2): the six-part selector cap applies to every raw prelude of a
    /// sheet that is not plain too, back to the previous `{` or `}`.
    #[test]
    fn the_part_cap_applies_to_sheets_that_are_not_plain() {
        let hidden = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><style>.q { font-family: "a}b" } g * * * * * * * * { fill: red }</style></svg>"#;
        assert_eq!(svg_hazard(hidden.as_bytes()), Some(SvgHazard::TooComplex));
        let short = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><style>.q { font-family: "a}b" } .a .b { fill: red }</style></svg>"#;
        assert_eq!(svg_hazard(short.as_bytes()), None);
    }

    /// FT-418 review: an at-rule is skipped by the CSS parser but is read conservatively as one wild rule here; a
    /// sheet with an at-rule but no reference still passes.
    #[test]
    fn at_rules_make_the_sheet_wild() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4""#;
        let media = format!(
            r#"<svg {NS}><style>@media print {{ rect {{ clip-path: url(#c) }} }}</style><clipPath id="c"><rect width="1" height="1"/></clipPath><rect width="1" height="1"/></svg>"#
        );
        assert_eq!(svg_hazard(media.as_bytes()), Some(SvgHazard::TooComplex));
        let keyframes = format!(
            r#"<svg {NS}><style>.blink {{ animation: blinking 1s infinite; }} @keyframes blinking {{ 50% {{ opacity: 0.0; }} }}</style></svg>"#
        );
        assert_eq!(svg_hazard(keyframes.as_bytes()), None);
    }

    /// FT-418 review: duplicate ids are refused (usvg resolves `<use>` first-wins and `url(#…)` last-wins, so the
    /// scanner cannot follow both), as are two different `href`s on one element (a deliberate stricter-than-usvg
    /// rule).
    #[test]
    fn repeated_ids_and_double_hrefs_are_refused() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 4 4""#;
        let repeated = format!(
            r#"<svg {NS}><pattern id="p" width="1" height="1"><rect width="1" height="1" fill="url(#q)"/></pattern><pattern id="q" width="1" height="1"><rect width="1" height="1" fill="url(#p)"/></pattern><g id="p"/></svg>"#
        );
        assert_eq!(svg_hazard(repeated.as_bytes()), Some(SvgHazard::TooComplex));
        let clip_paths = format!(
            r#"<svg {NS}><clipPath id="a"><rect width="1" height="1"/></clipPath><clipPath id="a"><rect width="1" height="1" clip-path="url(#b)"/></clipPath><clipPath id="b"><rect width="1" height="1" clip-path="url(#c)"/></clipPath><clipPath id="c"><rect width="1" height="1" clip-path="url(#a)"/></clipPath><rect width="1" height="1" clip-path="url(#a)"/></svg>"#
        );
        assert_eq!(svg_hazard(clip_paths.as_bytes()), Some(SvgHazard::TooComplex));
        let gradients = format!(r#"<svg {NS}><linearGradient id="g"/><linearGradient id="g"/></svg>"#);
        assert_eq!(svg_hazard(gradients.as_bytes()), Some(SvgHazard::TooComplex));
        let different = format!(r##"<svg {NS}><linearGradient id="a" href="#b" xlink:href="#c"/></svg>"##);
        assert_eq!(svg_hazard(different.as_bytes()), Some(SvgHazard::TooComplex));
        let same = format!(r##"<svg {NS}><linearGradient id="a" href="#b" xlink:href="#b"/></svg>"##);
        assert_eq!(svg_hazard(same.as_bytes()), None);
    }

    /// FT-418: marker content that can carry a marker property is refused from any source, and what real drawing
    /// programs write (a marker used by a class, presentation attributes, a plain rule) still passes.
    #[test]
    fn marker_content_that_can_carry_a_marker_is_refused() {
        const NS: &str = r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 4 4""#;
        let n = r#"<marker id="n"><path d="M0 0 L1 1"/></marker>"#;
        let p = r#"<path d="M0 0 L1 1 L2 0"/>"#;
        let cases: Vec<(&str, String, Option<SvgHazard>)> = vec![
            (
                "(a)",
                format!(r#"<svg {NS}><g marker-mid="url(#n)"><marker id="m">{p}</marker></g>{n}</svg>"#),
                Some(SvgHazard::TooComplex),
            ),
            (
                "(b)",
                format!(r#"<svg {NS}><marker id="m" marker-mid="url(#n)">{p}</marker>{n}</svg>"#),
                Some(SvgHazard::TooComplex),
            ),
            (
                "(c)",
                format!(
                    r#"<svg {NS}><style>.k {{ marker-mid: url(#n) }}</style><marker id="m"><path class="k" d="M0 0 L1 1 L2 0"/></marker>{n}</svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "(d)",
                format!(
                    r#"<svg {NS}><style>.k {{ marker-mid: url(#n) }}</style><g class="k"><marker id="m">{p}</marker></g>{n}</svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "(e)",
                format!(
                    r#"<svg {NS}><style>path {{ marker-mid: url(#n) }}</style><marker id="m">{p}</marker>{n}</svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "(e2)",
                format!(
                    r#"<svg {NS}><style>path {{ marker-mid: url(#g) }}</style><linearGradient id="g"><stop offset="0"/></linearGradient><marker id="m">{p}</marker></svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "(f)",
                format!(
                    r##"<svg {NS}><defs><g id="arrow"><path d="M0 0 L1 1 L2 0" marker-mid="url(#n)"/></g></defs><marker id="m"><use href="#arrow"/></marker>{n}</svg>"##
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "(g)",
                format!(
                    r#"<svg {NS}><marker id="m"><path class="k" d="M0 0 L1 1 L2 0"/></marker>{n}<style>.k {{ marker-mid: url(#n) }}</style></svg>"#
                ),
                Some(SvgHazard::TooComplex),
            ),
            (
                "(i)",
                format!(
                    r##"<svg {NS}><style>.st2 {{ marker-end: url(#m) }} .st3 {{ fill: #00f }}</style><defs><g id="lend"><path d="M1 -1 L0 0 L1 1"/></g><marker id="m" class="st3"><use xlink:href="#lend"/></marker></defs><path class="st2" d="M0 0 L10 0"/></svg>"##
                ),
                None,
            ),
            (
                "(j)",
                format!(
                    r#"<svg {NS}><defs><marker id="a" markerWidth="5" markerHeight="6" style="overflow:visible"><path d="M0 0 L5 3 L0 6 Z" style="fill:#000"/></marker></defs><path d="M0 0 L10 0" style="marker-end:url(#a)"/></svg>"#
                ),
                None,
            ),
            (
                "(k)",
                format!(
                    r#"<svg {NS}><style>path {{ stroke: #000 }}</style><marker id="m">{p}</marker><path d="M0 0 L4 4" marker-end="url(#m)"/></svg>"#
                ),
                None,
            ),
        ];
        for (letter, text, expected) in &cases {
            assert_eq!(svg_hazard(text.as_bytes()), *expected, "{letter}");
        }
    }

    /// FT-418: every refusal says what the hazard is and how to export the file again.
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
                "more cloned declarations than the bound",
                format!(
                    r#"<svg {NS} viewBox="0 0 4 4"><style>{} {{ fill: red }}</style></svg>"#,
                    (0..5_000).map(|k| format!(".c{k}")).collect::<Vec<_>>().join(",")
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

    /// Review round 2: a cycle-free gradient chain longer than `MAX_SVG_REF_DEPTH` is refused (usvg walks it once per
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
