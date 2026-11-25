// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Assets: content-addressed image files (`assets/<sha256>.<ext>`, doc 05 §13.3), the embedded sources of
//! underlays (a PDF or a DXF the user embedded) and the project's typeface (a TrueType or OpenType file, doc 09
//! §4.8), which no image gate takes ([`pixel_limit`]).
//!
//! - [`AssetFormat`]: the encodings a project may hold (SVG, PNG, JPEG; PDF and DXF underlay sources; TTF and OTF
//!   typefaces) and their file extensions.
//! - [`AssetRef`]: how a record names an asset, `{sha256, format, size}`, so that readers such as the
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
//!   until `Project` types it), in either of its two forms.
//!
//! Hashing is the caller's (the format layer and the command layer own SHA-256).

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::Extra;
use crate::value::Sha256;

mod svg;

pub use svg::{MAX_SVG_DEPTH, MAX_SVG_NUMBER, MAX_SVG_REF_DEPTH, SvgHazard, svg_hazard};

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

/// The project-row member that holds the company data of a basic project until `Project` types it.
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
            // The words and the advice are the application's (`ew_commands::asset_error_text`); a log names the
            // hazard.
            AssetError::UnsafeSvg(h) => write!(f, "the SVG file is refused ({h:?})"),
        }
    }
}

impl std::error::Error for AssetError {}

/// The format and intrinsic size of asset bytes, after the size limits (doc 05 §13.3). The pixel
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
