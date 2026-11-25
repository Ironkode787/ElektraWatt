// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Assets: content-addressed image files (`assets/<sha256>.<ext>`, doc 05 §13.3), the embedded sources of
//! underlays (a PDF or a DXF the user embedded) and the project's typeface (a TrueType or OpenType file, doc 09
//! §4.8), which no image gate takes ([`pixel_limit`]).
//!
//! - [`AssetFormat`]: the encodings a project may hold (SVG, PNG, JPEG; PDF and DXF underlay sources; TTF and OTF
//!   typefaces) and their file extensions.
//! - [`AssetRef`]: how a record names an asset, `{sha256, format, size}` (decision log ID-61): readers such as the
//!   forms layer trust it and never sniff the bytes.
//! - [`sniff`]: what the asset store does once, when bytes are imported: it reads the magic bytes and the
//!   intrinsic size (PNG `IHDR`, JPEG `SOF`, SVG `viewBox` or `width`/`height`) and applies the size limits. It
//!   decodes no image and renders nothing. An SVG must also pass [`svg_hazard`].
//! - [`svg_hazard`]: the plain rejecter of SVG assets (doc 18 §11 TM-18): sniffing is not sanitising, so every SVG
//!   the store admits is first read for a DOCTYPE internal subset, entity declarations, scripts and event attributes,
//!   `foreignObject`, references that leave the document, and the nesting, reference chains and numbers the parser
//!   does not survive; any of them refuses the asset before a parser or a renderer sees it.
//! - [`sha256_mentions`]: the asset hashes a text mentions (the asset purge).
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
/// The most pixels a raster asset may declare: 16 Mi, the decode limit of the renderers (`ew_render_cpu`'s
/// `MAX_IMAGE_PIXELS`), each side at most 65535: a larger image would be placed but never drawn.
pub const MAX_RASTER_PIXELS: u64 = 16 * 1024 * 1024;
/// The largest SVG asset: 8 MiB (doc 05 §13.3).
pub const MAX_SVG_BYTES: usize = 8 * 1024 * 1024;
/// The most elements an SVG asset may have (doc 05 §13.3).
pub const MAX_SVG_NODES: usize = 100_000;

/// The largest company logo: 512 KiB (owner ruling). The logo travels as base64 inside a command
/// parameter and a journal line, so it stays well below the command layer's 1 MiB limit of one parameter string.
pub const MAX_LOGO_BYTES: usize = 512 * 1024;

/// The project-row member that holds the company data of a basic project until `Project` types it (decision log
/// ID-40).
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
    /// A PDF, the embedded source of an underlay (never an image).
    Pdf,
    /// A DXF drawing, the embedded source of an underlay (never an image).
    Dxf,
    /// A font of one typeface with TrueType outlines (or an sfnt of version `true`): the project's typeface (doc 09
    /// §4.8; never an image).
    Ttf,
    /// A font of one typeface with PostScript outlines (`OTTO`): the project's typeface (never an image).
    Otf,
}

impl AssetFormat {
    /// The file extension of `assets/<sha256>.<ext>`.
    pub const fn ext(self) -> &'static str {
        match self {
            AssetFormat::Svg => "svg",
            AssetFormat::Png => "png",
            AssetFormat::Jpeg => "jpg",
            AssetFormat::Pdf => "pdf",
            AssetFormat::Dxf => "dxf",
            AssetFormat::Ttf => "ttf",
            AssetFormat::Otf => "otf",
        }
    }

    /// The format of a file extension (`svg`, `png`, `jpg`, `pdf`, `dxf`, `ttf`, `otf`); `None` for any other.
    pub fn from_ext(ext: &str) -> Option<AssetFormat> {
        match ext {
            "svg" => Some(AssetFormat::Svg),
            "png" => Some(AssetFormat::Png),
            "jpg" => Some(AssetFormat::Jpeg),
            "pdf" => Some(AssetFormat::Pdf),
            "dxf" => Some(AssetFormat::Dxf),
            "ttf" => Some(AssetFormat::Ttf),
            "otf" => Some(AssetFormat::Otf),
            _ => None,
        }
    }

    /// Whether the format is a font file (the project's typeface).
    pub const fn is_font(self) -> bool {
        matches!(self, AssetFormat::Ttf | AssetFormat::Otf)
    }

    /// Whether the format is a raster image (it prints rasterised, D-07).
    pub const fn is_raster(self) -> bool {
        matches!(self, AssetFormat::Png | AssetFormat::Jpeg)
    }

    /// Whether the format is an image (SVG, PNG, JPEG): what the image gates take.
    pub const fn is_image(self) -> bool {
        matches!(self, AssetFormat::Svg | AssetFormat::Png | AssetFormat::Jpeg)
    }
}

/// The largest embedded underlay source (a PDF or a DXF): 64 MiB.
pub const MAX_UNDERLAY_BYTES: usize = 64 * 1024 * 1024;

/// The largest font file a project takes as its typeface: 2.5 MiB (doc 09 §4.8). The text faces of one typeface weigh
/// up to about 1.8 MiB (Calibri 1.6 MiB, Segoe UI Variable 1.8 MiB); only emoji, symbol and East Asian faces and
/// faces for every script weigh more (2.4 to 22 MiB). Its base64 fits one command's parameters (4 MiB) in parts of
/// one parameter string each (1 MiB).
pub const MAX_TYPEFACE_BYTES: usize = 2_560 * 1024;

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
    /// A PNG or JPEG file that declares more than [`MAX_RASTER_PIXELS`] pixels, or a side over 65535.
    TooManyPixels {
        /// The format.
        format: AssetFormat,
        /// The declared width in pixels.
        width: u32,
        /// The declared height in pixels.
        height: u32,
    },
    /// The header of a PNG or JPEG file is cut short or damaged.
    Damaged(AssetFormat),
    /// Text that is not base64 (standard alphabet, padded).
    Base64,
    /// An SVG file with content that is refused before any parser or renderer reads it ([`svg_hazard`]).
    UnsafeSvg(SvgHazard),
}

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
            AssetError::TooManyPixels {
                format,
                width,
                height,
            } => write!(
                f,
                "the {} image is {width} × {height} pixels, more than a drawing can show (at most \
                 {MAX_RASTER_PIXELS} pixels and 65535 on a side); scale it down and add it again",
                format.ext()
            ),
            AssetError::Damaged(format) => write!(f, "the {} header is damaged or cut short", format.ext()),
            AssetError::Base64 => f.write_str("not base64 text (standard alphabet, padded)"),
            AssetError::UnsafeSvg(h) => {
                write!(f, "the SVG file is refused: {}; {}", h.describe(), h.advice())
            }
        }
    }
}

impl std::error::Error for AssetError {}

/// The format and intrinsic size of asset bytes, after the size limits (doc 05 §13.3, decision log ID-61). The pixel
/// limit of a raster image is [`pixel_limit`]'s, which only the gates that bring an image in apply.
///
/// # Errors
///
/// [`AssetError`] for bytes of no known format, over a limit, or with a damaged raster header.
pub fn sniff(bytes: &[u8]) -> Result<(AssetFormat, Option<(u32, u32)>), AssetError> {
    let (format, size) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        (AssetFormat::Png, Some(png_size(bytes)?))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        (AssetFormat::Jpeg, Some(jpeg_size(bytes)?))
    } else if bytes.starts_with(b"%PDF-") {
        (AssetFormat::Pdf, None)
    } else if is_dxf(bytes) {
        (AssetFormat::Dxf, None)
    } else if bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"true") {
        (AssetFormat::Ttf, None)
    } else if bytes.starts_with(b"OTTO") {
        (AssetFormat::Otf, None)
    } else if let Some(attrs) = svg_root(bytes) {
        (AssetFormat::Svg, svg_size(attrs))
    } else {
        return Err(AssetError::UnknownFormat);
    };
    let limit = match format {
        AssetFormat::Png | AssetFormat::Jpeg => MAX_RASTER_BYTES,
        AssetFormat::Svg => MAX_SVG_BYTES,
        AssetFormat::Pdf | AssetFormat::Dxf => MAX_UNDERLAY_BYTES,
        AssetFormat::Ttf | AssetFormat::Otf => MAX_TYPEFACE_BYTES,
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

/// The pixel limit of a raster image a project takes in: a PNG or JPEG of `size` that
/// declares more than [`MAX_RASTER_PIXELS`] pixels, or a side over 65535, is refused. The gates that bring an image in
/// apply it after [`sniff`]; an open does not, so an image an earlier build took in is kept (and not drawn).
///
/// # Errors
///
/// [`AssetError::TooManyPixels`] for a raster image over the limit; [`AssetError::UnknownFormat`] for an underlay
/// source (a PDF or a DXF), which is no image.
pub fn pixel_limit(format: AssetFormat, size: Option<(u32, u32)>) -> Result<(), AssetError> {
    if !format.is_image() {
        return Err(AssetError::UnknownFormat);
    }
    match size {
        Some((width, height))
            if format.is_raster()
                && (u64::from(width) * u64::from(height) > MAX_RASTER_PIXELS
                    || width > 65535
                    || height > 65535) =>
        {
            Err(AssetError::TooManyPixels {
                format,
                width,
                height,
            })
        }
        _ => Ok(()),
    }
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

/// Whether bytes are a DXF drawing: the binary sentinel, or (after an optional BOM, white space and `999` comment
/// pairs) the ASCII pair `0` / `SECTION` at the start.
fn is_dxf(bytes: &[u8]) -> bool {
    if bytes.starts_with(b"AutoCAD Binary DXF\r\n\x1a\0") {
        return true;
    }
    let head = &bytes[..bytes.len().min(4096)];
    let head = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    let Ok(text) = std::str::from_utf8(head).or_else(|e| std::str::from_utf8(&head[..e.valid_up_to()]))
    else {
        return false;
    };
    let mut lines = text.lines().map(str::trim);
    loop {
        match (lines.next(), lines.next()) {
            (Some("999"), Some(_)) => {}
            (Some("0"), Some(v)) => return v == "SECTION",
            _ => return false,
        }
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
/// [`sniff`] caps the size ([`MAX_SVG_BYTES`]) and the elements ([`MAX_SVG_NODES`]), so this reader reads the whole
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
/// object. A project of a release before the asset store holds the earlier form, a bare `"sha256:<64 hex>"` text that
/// names no format: it is read here and nowhere else, as the reference `held` gives the held asset of that hash, else
/// as a PNG of unknown size (what the title block assumed before the asset store existed), so that such a project
/// opens unchanged. `None` when absent or not readable (each member is read on its own).
pub fn company_logo(extra: &Extra, held: impl FnOnce(&Sha256) -> Option<AssetRef>) -> Option<AssetRef> {
    let logo = extra.get(COMPANY_MEMBER)?.get("logo")?;
    match logo {
        Value::String(text) => {
            let sha256 = text.parse::<Sha256>().ok()?;
            Some(held(&sha256).unwrap_or(AssetRef {
                sha256,
                format: AssetFormat::Png,
                size: None,
            }))
        }
        Value::Object(_) => serde_json::from_value(logo.clone()).ok(),
        _ => None,
    }
}

/// Every SHA-256 that `bytes` mention as text: each run of 64 consecutive hexadecimal digits (either case) inside a
/// run of ASCII hex digits, so a run of 70 digits mentions its seven 64-digit windows. This is the conservative
/// "appears in a file" test of the asset purge (doc 05 §13.3): any form a writer may use to name an asset
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

    /// A raster image that declares more pixels than a drawing shows, or a side over 65535, is refused at the
    /// gate in words; the decode limit itself is taken. The sniff alone (an open's) takes it.
    #[test]
    fn a_raster_image_over_the_decode_limit_is_refused() {
        let gate = |bytes: &[u8]| sniff(bytes).and_then(|(f, size)| pixel_limit(f, size).map(|()| (f, size)));
        // The sniff of an open takes it; the gate refuses it.
        assert_eq!(
            sniff(&png(65536, 32768)),
            Ok((AssetFormat::Png, Some((65536, 32768))))
        );
        let refused = gate(&png(65536, 32768)).unwrap_err();
        assert_eq!(
            refused,
            AssetError::TooManyPixels {
                format: AssetFormat::Png,
                width: 65536,
                height: 32768
            }
        );
        assert_eq!(
            refused.to_string(),
            "the png image is 65536 × 32768 pixels, more than a drawing can show (at most 16777216 pixels and 65535 \
             on a side); scale it down and add it again"
        );
        assert_eq!(gate(&png(4096, 4096)), Ok((AssetFormat::Png, Some((4096, 4096)))));
        assert!(matches!(
            gate(&png(4097, 4096)),
            Err(AssetError::TooManyPixels { .. })
        ));
        assert!(matches!(
            gate(&png(65536, 1)),
            Err(AssetError::TooManyPixels { .. })
        ));
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

    /// The earlier bare-hash form takes the held asset's reference, else reads as a PNG of unknown size; the object
    /// form is read as it is.
    #[test]
    fn the_company_logo_reads_both_forms() {
        let hash = format!("sha256:{}", "cd".repeat(32));
        let png = AssetRef {
            sha256: hash.parse().unwrap(),
            format: AssetFormat::Png,
            size: None,
        };
        let svg = AssetRef {
            format: AssetFormat::Svg,
            size: Some((40, 10)),
            ..png
        };
        let mut extra = Extra::new();
        assert_eq!(company_logo(&extra, |_| None), None);
        extra.insert(COMPANY_MEMBER.into(), serde_json::json!({"logo": hash}));
        assert_eq!(company_logo(&extra, |_| None), Some(png));
        assert_eq!(
            company_logo(&extra, |sha| (*sha == svg.sha256).then_some(svg)),
            Some(svg)
        );
        extra.insert(
            COMPANY_MEMBER.into(),
            serde_json::json!({"logo": {"sha256": hash, "format": "svg", "size": [3, 1]}}),
        );
        assert_eq!(
            company_logo(&extra, |_| Some(png)).map(|r| r.size),
            Some(Some((3, 1)))
        );
        extra.insert(COMPANY_MEMBER.into(), serde_json::json!({"logo": 5}));
        assert_eq!(company_logo(&extra, |_| None), None);
    }
}
