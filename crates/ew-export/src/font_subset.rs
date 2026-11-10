// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The font subsetter of the SVG writer: a TrueType (`glyf`) face cut down to the glyphs a page draws, **with
//! the glyph IDs kept** (doc 09 §4.4; FR-OUT-013).
//!
//! The SVG pages keep their text as `<text>`, so a browser maps the characters through the embedded font's
//! `cmap` and draws the glyphs itself. The subset therefore keeps the `cmap` and every glyph ID (krilla's
//! subsetter serves PDF CID fonts: it renumbers the glyphs and drops the `cmap`).
//!
//! | Table | In the subset |
//! |---|---|
//! | `glyf` | the asked glyphs and the components of composite glyphs (their closure) with their outline bytes as in the face; every other glyph empty |
//! | `loca` | long offsets (`head.indexToLocFormat` 1) |
//! | `head` | as in the face, `checkSumAdjustment` recomputed over the subset |
//! | `post` | version 3.0 (no glyph names), the face's other fields |
//! | `cmap`, `hhea`, `hmtx`, `maxp`, `OS/2`, `name` | as in the face (`OS/2`, `name` and `post` when the face has them) |
//! | `cvt `, `fpgm`, `prep` | as in the face, when the face has them: the kept glyphs keep their instructions |
//! | `GSUB`, `GPOS`, `GDEF`, `kern`, `MATH`, every other table | dropped: the text is shaped already and placed per character |
//!
//! The tables are written in tag order, 4-byte aligned, each with its checksum. The parser uses the standard
//! library only and is bounded: a file over [`MAX_FONT_BYTES`], a table outside the file, a glyph outside `glyf`,
//! a composite glyph nested deeper than [`MAX_COMPOSITE_DEPTH`] or containing itself is a [`SubsetError`], never
//! a panic; every glyph's components are read once, so the time is linear in the face's size.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The largest font file a subset is made from (16 MiB).
pub const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;

/// The deepest nesting of composite glyphs a subset follows: a composite of simple glyphs is 1 level deep.
pub const MAX_COMPOSITE_DEPTH: usize = 16;

/// The `head` checksum constant of the OpenType specification.
const CHECKSUM_MAGIC: u32 = 0xB1B0_AFBA;

/// Why no subset was made of a font.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubsetError {
    /// The file is larger than [`MAX_FONT_BYTES`].
    TooLarge {
        /// Its size in bytes.
        bytes: usize,
    },
    /// The data is not an sfnt font.
    NotAFont,
    /// The data is a font collection.
    Collection,
    /// The table directory runs past the end of the file.
    TruncatedDirectory,
    /// A table lies (partly) outside the file.
    TableOutside {
        /// Its tag.
        tag: [u8; 4],
    },
    /// The font has no `glyf` table (a CFF face): it can only be named.
    NoGlyf,
    /// A table the subset needs is missing.
    MissingTable {
        /// Its tag.
        tag: [u8; 4],
    },
    /// A table is shorter than its fixed part or than its glyph count needs.
    TableTooShort {
        /// Its tag.
        tag: [u8; 4],
    },
    /// `head.indexToLocFormat` is neither 0 nor 1.
    LocaFormat {
        /// The value.
        format: i16,
    },
    /// A glyph's `loca` range is reversed or ends past the `glyf` table.
    GlyphOutside {
        /// The glyph ID.
        glyph: u16,
    },
    /// A glyph ID at or past the font's glyph count.
    GlyphNotInFont {
        /// The glyph ID.
        glyph: u16,
        /// The font's glyph count.
        glyphs: u16,
    },
    /// A composite glyph ends inside a component record.
    CompositeTruncated {
        /// The glyph ID.
        glyph: u16,
    },
    /// A composite glyph nests deeper than [`MAX_COMPOSITE_DEPTH`].
    CompositeTooDeep {
        /// The glyph ID of the outermost composite.
        glyph: u16,
    },
    /// A composite glyph contains itself, directly or through its components.
    CompositeCycle {
        /// The glyph ID.
        glyph: u16,
    },
}

/// A table tag as text: its four bytes, a byte outside printable ASCII as `?`.
fn tag_text(tag: [u8; 4]) -> String {
    tag.iter()
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                char::from(b)
            } else {
                '?'
            }
        })
        .collect()
}

impl fmt::Display for SubsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubsetError::TooLarge { bytes } => {
                write!(
                    f,
                    "the font file is {bytes} bytes, more than the 16 MiB a subset is made from"
                )
            }
            SubsetError::NotAFont => write!(f, "the data is not a TrueType or OpenType font"),
            SubsetError::Collection => {
                write!(f, "the file is a font collection; only a single face is subset")
            }
            SubsetError::TruncatedDirectory => {
                write!(f, "the font's table directory runs past the end of the file")
            }
            SubsetError::TableOutside { tag } => {
                write!(f, "the `{}` table lies outside the file", tag_text(*tag))
            }
            SubsetError::NoGlyf => write!(f, "the font has no TrueType outlines (no `glyf` table)"),
            SubsetError::MissingTable { tag } => write!(f, "the font has no `{}` table", tag_text(*tag)),
            SubsetError::TableTooShort { tag } => write!(f, "the `{}` table is too short", tag_text(*tag)),
            SubsetError::LocaFormat { format } => {
                write!(f, "the `head` table names the unknown `loca` format {format}")
            }
            SubsetError::GlyphOutside { glyph } => write!(f, "glyph {glyph} lies outside the `glyf` table"),
            SubsetError::GlyphNotInFont { glyph, glyphs } => {
                write!(f, "glyph {glyph} is not in the font, which has {glyphs} glyphs")
            }
            SubsetError::CompositeTruncated { glyph } => write!(f, "composite glyph {glyph} is truncated"),
            SubsetError::CompositeTooDeep { glyph } => write!(
                f,
                "composite glyph {glyph} nests more than {MAX_COMPOSITE_DEPTH} levels deep"
            ),
            SubsetError::CompositeCycle { glyph } => write!(f, "composite glyph {glyph} contains itself"),
        }
    }
}

impl std::error::Error for SubsetError {}

/// The subset of the TrueType face `font` (a single-face file) that keeps the outlines of `glyphs` and of
/// their components, with the glyph IDs unchanged (see the module documentation).
///
/// # Errors
///
/// [`SubsetError`] when the file is too large, not a single TrueType face, or malformed where the subset
/// reads it, or when a glyph of `glyphs` is not in the font.
pub fn subset_glyf(font: &[u8], glyphs: &BTreeSet<u16>) -> Result<Vec<u8>, SubsetError> {
    Face::parse(font)?.subset(glyphs)
}

/// The glyph IDs the `cmap` of the TrueType face `font` gives `chars` (a browser draws these for the text);
/// characters it does not map, and IDs at or past the glyph count, give none. The Unicode subtable is the
/// face's full-repertoire one (format 12) if it has one, else its BMP one (format 4).
///
/// # Errors
///
/// [`SubsetError`] as for [`subset_glyf`].
pub fn cmap_glyphs(font: &[u8], chars: &BTreeSet<char>) -> Result<BTreeSet<u16>, SubsetError> {
    let face = Face::parse(font)?;
    Ok(face.cmap_glyphs(chars))
}

/// The subset of `font` for a page's text: the glyphs the runs drew and the glyphs the `cmap` gives the
/// characters written, so that a browser that maps the characters itself finds their outlines.
pub(crate) fn subset_for_text(
    font: &[u8],
    glyphs: &BTreeSet<u16>,
    chars: &BTreeSet<char>,
) -> Result<Vec<u8>, SubsetError> {
    let face = Face::parse(font)?;
    let mut all = face.cmap_glyphs(chars);
    all.extend(glyphs.iter().copied());
    face.subset(&all)
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let b = d.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let b = d.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// A parsed face: its tables by tag and the fields the subset reads.
struct Face<'a> {
    tables: BTreeMap<[u8; 4], &'a [u8]>,
    glyf: &'a [u8],
    loca: &'a [u8],
    long_loca: bool,
    num_glyphs: u16,
}

impl<'a> Face<'a> {
    fn parse(font: &'a [u8]) -> Result<Face<'a>, SubsetError> {
        if font.len() > MAX_FONT_BYTES {
            return Err(SubsetError::TooLarge { bytes: font.len() });
        }
        let version = u32_at(font, 0).ok_or(SubsetError::NotAFont)?;
        match &version.to_be_bytes() {
            b"ttcf" => return Err(SubsetError::Collection),
            [0, 1, 0, 0] | b"true" | b"OTTO" => {}
            _ => return Err(SubsetError::NotAFont),
        }
        let count = usize::from(u16_at(font, 4).ok_or(SubsetError::TruncatedDirectory)?);
        if font.len() < 12 + 16 * count {
            return Err(SubsetError::TruncatedDirectory);
        }
        let mut tables = BTreeMap::new();
        for i in 0..count {
            let at = 12 + 16 * i;
            let tag = [font[at], font[at + 1], font[at + 2], font[at + 3]];
            let (Some(offset), Some(length)) = (u32_at(font, at + 8), u32_at(font, at + 12)) else {
                return Err(SubsetError::TruncatedDirectory);
            };
            let start = offset as usize;
            let data = start
                .checked_add(length as usize)
                .and_then(|end| font.get(start..end))
                .ok_or(SubsetError::TableOutside { tag })?;
            tables.entry(tag).or_insert(data);
        }
        let glyf = *tables.get(b"glyf").ok_or(SubsetError::NoGlyf)?;
        for tag in [b"head", b"maxp", b"loca", b"cmap", b"hhea", b"hmtx"] {
            if !tables.contains_key(tag) {
                return Err(SubsetError::MissingTable { tag: *tag });
            }
        }
        let head = tables[b"head"];
        if head.len() < 54 {
            return Err(SubsetError::TableTooShort { tag: *b"head" });
        }
        let format = i16::from_be_bytes([head[50], head[51]]);
        let long_loca = match format {
            0 => false,
            1 => true,
            format => return Err(SubsetError::LocaFormat { format }),
        };
        let num_glyphs = u16_at(tables[b"maxp"], 4).ok_or(SubsetError::TableTooShort { tag: *b"maxp" })?;
        let loca = tables[b"loca"];
        let entry = if long_loca { 4 } else { 2 };
        if loca.len() < (usize::from(num_glyphs) + 1) * entry {
            return Err(SubsetError::TableTooShort { tag: *b"loca" });
        }
        if tables.get(b"post").is_some_and(|p| p.len() < 32) {
            return Err(SubsetError::TableTooShort { tag: *b"post" });
        }
        Ok(Face {
            tables,
            glyf,
            loca,
            long_loca,
            num_glyphs,
        })
    }

    /// The byte range of glyph `g` in `glyf` (validated: in order and inside the table).
    fn range(&self, g: u16) -> Result<(usize, usize), SubsetError> {
        let i = usize::from(g);
        let (start, end) = if self.long_loca {
            (
                u32_at(self.loca, 4 * i).map(|v| v as usize),
                u32_at(self.loca, 4 * i + 4).map(|v| v as usize),
            )
        } else {
            (
                u16_at(self.loca, 2 * i).map(|v| 2 * usize::from(v)),
                u16_at(self.loca, 2 * i + 2).map(|v| 2 * usize::from(v)),
            )
        };
        match (start, end) {
            (Some(s), Some(e)) if s <= e && e <= self.glyf.len() => Ok((s, e)),
            _ => Err(SubsetError::GlyphOutside { glyph: g }),
        }
    }

    /// The component glyph IDs of glyph `g` (none for a simple or empty glyph).
    fn components(&self, g: u16) -> Result<Vec<u16>, SubsetError> {
        const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
        const WE_HAVE_A_SCALE: u16 = 0x0008;
        const MORE_COMPONENTS: u16 = 0x0020;
        const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
        const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;
        let (s, e) = self.range(g)?;
        let data = &self.glyf[s..e];
        let truncated = SubsetError::CompositeTruncated { glyph: g };
        match data.get(..2) {
            Some(c) if i16::from_be_bytes([c[0], c[1]]) < 0 => {}
            _ => return Ok(Vec::new()),
        }
        let mut out = Vec::new();
        let mut at = 10;
        loop {
            let flags = u16_at(data, at).ok_or(truncated)?;
            let component = u16_at(data, at + 2).ok_or(truncated)?;
            at += 4;
            at += if flags & ARG_1_AND_2_ARE_WORDS != 0 { 4 } else { 2 };
            if flags & WE_HAVE_A_SCALE != 0 {
                at += 2;
            } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
                at += 4;
            } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
                at += 8;
            }
            if at > data.len() {
                return Err(truncated);
            }
            out.push(component);
            if flags & MORE_COMPONENTS == 0 {
                return Ok(out);
            }
        }
    }

    /// The composite nesting depth of glyph `g` (0 for a simple glyph), recording every glyph of its closure
    /// in `depths`; `path` holds the composites being expanded, outermost first.
    fn depth(
        &self,
        g: u16,
        path: &mut Vec<u16>,
        depths: &mut BTreeMap<u16, usize>,
    ) -> Result<usize, SubsetError> {
        if g >= self.num_glyphs {
            return Err(SubsetError::GlyphNotInFont {
                glyph: g,
                glyphs: self.num_glyphs,
            });
        }
        let depth = match depths.get(&g) {
            Some(&d) => d,
            None => {
                if path.contains(&g) {
                    return Err(SubsetError::CompositeCycle { glyph: g });
                }
                let components = self.components(g)?;
                if components.is_empty() {
                    0
                } else {
                    if path.len() >= MAX_COMPOSITE_DEPTH {
                        return Err(SubsetError::CompositeTooDeep {
                            glyph: path.first().copied().unwrap_or(g),
                        });
                    }
                    path.push(g);
                    let mut deepest = 0;
                    for c in components {
                        deepest = deepest.max(self.depth(c, path, depths)?);
                    }
                    path.pop();
                    deepest + 1
                }
            }
        };
        if path.len() + depth > MAX_COMPOSITE_DEPTH {
            return Err(SubsetError::CompositeTooDeep {
                glyph: path.first().copied().unwrap_or(g),
            });
        }
        depths.insert(g, depth);
        Ok(depth)
    }

    fn subset(&self, glyphs: &BTreeSet<u16>) -> Result<Vec<u8>, SubsetError> {
        let mut closure = BTreeMap::new();
        let mut path = Vec::new();
        for &g in glyphs {
            self.depth(g, &mut path, &mut closure)?;
        }
        let mut glyf = Vec::new();
        let mut loca = Vec::with_capacity((usize::from(self.num_glyphs) + 1) * 4);
        for g in 0..self.num_glyphs {
            // The offsets fit: the kept bytes are a part of a `glyf` table of at most MAX_FONT_BYTES.
            loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
            if closure.contains_key(&g) {
                let (s, e) = self.range(g)?;
                glyf.extend_from_slice(&self.glyf[s..e]);
            }
        }
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        let mut head = self.tables[b"head"].to_vec();
        head[8..12].fill(0);
        head[50..52].copy_from_slice(&1i16.to_be_bytes());
        let mut out: BTreeMap<[u8; 4], Vec<u8>> = BTreeMap::new();
        // The hinting tables stay byte for byte when the face has them: the kept glyphs keep their instructions in
        // `glyf`, which call into `fpgm` and read `cvt `.
        for tag in [
            b"cmap", b"hhea", b"hmtx", b"maxp", b"OS/2", b"name", b"cvt ", b"fpgm", b"prep",
        ] {
            if let Some(t) = self.tables.get(tag) {
                out.insert(*tag, t.to_vec());
            }
        }
        if let Some(post) = self.tables.get(b"post") {
            let mut post = post[..32].to_vec();
            post[..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
            out.insert(*b"post", post);
        }
        out.insert(*b"glyf", glyf);
        out.insert(*b"loca", loca);
        out.insert(*b"head", head);
        Ok(assemble(&out))
    }

    /// The glyph IDs of `chars` through the best Unicode `cmap` subtable.
    fn cmap_glyphs(&self, chars: &BTreeSet<char>) -> BTreeSet<u16> {
        let cmap = self.tables[b"cmap"];
        let mut format4 = None;
        let mut format12 = None;
        let count = usize::from(u16_at(cmap, 2).unwrap_or(0));
        for i in 0..count {
            let (Some(platform), Some(encoding), Some(offset)) = (
                u16_at(cmap, 4 + 8 * i),
                u16_at(cmap, 6 + 8 * i),
                u32_at(cmap, 8 + 8 * i),
            ) else {
                break;
            };
            let unicode = platform == 0 || (platform == 3 && matches!(encoding, 1 | 10));
            let Some(sub) = cmap.get(offset as usize..).filter(|_| unicode) else {
                continue;
            };
            match u16_at(sub, 0) {
                Some(4) if format4.is_none() => format4 = Some(sub),
                Some(12) if format12.is_none() => format12 = Some(sub),
                _ => {}
            }
        }
        let map = |c: char| -> Option<u16> {
            match (format12, format4) {
                (Some(sub), _) => format12_glyph(sub, u32::from(c)),
                (None, Some(sub)) => format4_glyph(sub, u32::from(c)),
                (None, None) => None,
            }
        };
        chars
            .iter()
            .filter_map(|&c| map(c))
            .filter(|&g| g != 0 && g < self.num_glyphs)
            .collect()
    }
}

/// The glyph of `c` in a format 4 subtable (segments searched by halving; `None` when unmapped or malformed).
/// The arrays are read up to the end of the `cmap` table, not the subtable's own length field, which some faces
/// write modulo 65536; every read is bounded.
fn format4_glyph(sub: &[u8], c: u32) -> Option<u16> {
    let c = u16::try_from(c).ok()?;
    let segments = usize::from(u16_at(sub, 6)? / 2);
    let ends = 14;
    let starts = ends + 2 * segments + 2;
    let deltas = starts + 2 * segments;
    let range_offsets = deltas + 2 * segments;
    if range_offsets + 2 * segments > sub.len() {
        return None;
    }
    // The first segment whose end is at or after `c`.
    let (mut lo, mut hi) = (0, segments);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if u16_at(sub, ends + 2 * mid)? < c {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let i = lo;
    if i == segments || u16_at(sub, starts + 2 * i)? > c {
        return None;
    }
    let start = u16_at(sub, starts + 2 * i)?;
    let delta = u16_at(sub, deltas + 2 * i)?;
    let range = u16_at(sub, range_offsets + 2 * i)?;
    if range == 0 {
        return Some(c.wrapping_add(delta));
    }
    let at = range_offsets + 2 * i + usize::from(range) + 2 * usize::from(c - start);
    match u16_at(sub, at)? {
        0 => None,
        g => Some(g.wrapping_add(delta)),
    }
}

/// The glyph of `c` in a format 12 subtable (groups searched by halving; `None` when unmapped or malformed).
fn format12_glyph(sub: &[u8], c: u32) -> Option<u16> {
    let groups = (u32_at(sub, 12)? as usize).min(sub.len().saturating_sub(16) / 12);
    let (mut lo, mut hi) = (0, groups);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let at = 16 + 12 * mid;
        let (start, end) = (u32_at(sub, at)?, u32_at(sub, at + 4)?);
        if end < c {
            lo = mid + 1;
        } else if start > c {
            hi = mid;
        } else {
            let g = u32_at(sub, at + 8)?.checked_add(c - start)?;
            return u16::try_from(g).ok();
        }
    }
    None
}

/// The checksum of a table: the sum of its big-endian 32-bit words, the last one padded with zeros.
fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, w| {
        let mut b = [0u8; 4];
        b[..w.len()].copy_from_slice(w);
        sum.wrapping_add(u32::from_be_bytes(b))
    })
}

/// An sfnt file of `tables` (TrueType outlines): the directory in tag order, each table 4-byte aligned with its
/// checksum, and `head.checkSumAdjustment` set so that the whole file sums to the magic constant.
fn assemble(tables: &BTreeMap<[u8; 4], Vec<u8>>) -> Vec<u8> {
    let count = tables.len();
    let selector = if count == 0 { 0 } else { count.ilog2() as usize };
    let search = (1usize << selector) * 16;
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    for v in [count, search, selector, count * 16 - search.min(count * 16)] {
        out.extend_from_slice(&(v as u16).to_be_bytes());
    }
    let mut offset = 12 + 16 * count;
    let mut head_at = None;
    for (tag, data) in tables {
        if tag == b"head" {
            head_at = Some(offset);
        }
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(data).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        offset += data.len().div_ceil(4) * 4;
    }
    for data in tables.values() {
        out.extend_from_slice(data);
        out.resize(out.len().div_ceil(4) * 4, 0);
    }
    if let Some(at) = head_at {
        let adjustment = CHECKSUM_MAGIC.wrapping_sub(checksum(&out));
        out[at + 8..at + 12].copy_from_slice(&adjustment.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_pad_the_last_word() {
        assert_eq!(checksum(&[]), 0);
        assert_eq!(checksum(&[0, 0, 0, 1, 0, 0, 0, 2]), 3);
        assert_eq!(checksum(&[1]), 0x0100_0000);
        assert_eq!(checksum(&[0xff, 0xff, 0xff, 0xff, 0, 0, 0, 2]), 1);
    }

    #[test]
    fn tags_print_their_bytes() {
        assert_eq!(tag_text(*b"OS/2"), "OS/2");
        assert_eq!(tag_text([b'a', 0, 0xff, b' ']), "a?? ");
    }
}
