// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Assets: content-addressed image files (`assets/<sha256>.<ext>`, doc 05 §13.3; FT-337).
//!
//! - [`AssetFormat`]: the encodings a project may hold (SVG, PNG, JPEG) and their file extensions.
//! - [`AssetRef`]: how a record names an asset, `{sha256, format, size}` (decision log ID-61): readers such as the
//!   forms layer trust it and never sniff the bytes.
//! - [`sniff`]: what the asset store does once, when bytes are imported: it reads the magic bytes and the
//!   intrinsic size (PNG `IHDR`, JPEG `SOF`, SVG `viewBox` or `width`/`height`) and applies the size limits. It
//!   decodes no image and renders nothing.
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

#[cfg(test)]
mod tests {
    use super::*;

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
