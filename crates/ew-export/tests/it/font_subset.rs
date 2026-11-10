// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The font subsetter (SV-2): glyph IDs kept, outlines of the used glyphs and their
//! components byte for byte, every other glyph empty, `head`'s checksum adjustment valid, the dropped tables
//! absent; hostile fonts refused with their error, never a panic. The subsets are read back with `skrifa` (D4).

use std::collections::{BTreeMap, BTreeSet};

use ew_export::{MAX_FONT_BYTES, SubsetError, cmap_glyphs, subset_glyf};
use ew_text::bundled;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::types::Tag;
use skrifa::{FontRef, GlyphId, MetadataProvider};

/// The bytes of table `tag` of `font`, read through skrifa's table directory.
pub(crate) fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    FontRef::new(font)
        .ok()?
        .table_data(Tag::new(tag))
        .map(|d| d.as_bytes())
}

/// The outline bytes of every glyph of `font` (from its `loca` and `glyf`), by glyph ID.
pub(crate) fn glyph_bytes(font: &[u8]) -> Vec<&[u8]> {
    let head = table(font, b"head").unwrap();
    let long = i16::from_be_bytes([head[50], head[51]]) == 1;
    let maxp = table(font, b"maxp").unwrap();
    let n = usize::from(u16::from_be_bytes([maxp[4], maxp[5]]));
    let (loca, glyf) = (table(font, b"loca").unwrap(), table(font, b"glyf").unwrap());
    let at = |i: usize| {
        if long {
            u32::from_be_bytes([loca[4 * i], loca[4 * i + 1], loca[4 * i + 2], loca[4 * i + 3]]) as usize
        } else {
            2 * usize::from(u16::from_be_bytes([loca[2 * i], loca[2 * i + 1]]))
        }
    };
    (0..n).map(|i| &glyf[at(i)..at(i + 1)]).collect()
}

/// The glyph of `c` through the `cmap` of `font`, as skrifa reads it.
pub(crate) fn glyph_of(font: &[u8], c: char) -> Option<u16> {
    let g = FontRef::new(font).ok()?.charmap().map(c)?;
    u16::try_from(g.to_u32()).ok()
}

/// Counts the drawing commands of an outline.
struct Count(usize);

impl OutlinePen for Count {
    fn move_to(&mut self, _: f32, _: f32) {
        self.0 += 1;
    }
    fn line_to(&mut self, _: f32, _: f32) {
        self.0 += 1;
    }
    fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {
        self.0 += 1;
    }
    fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
        self.0 += 1;
    }
    fn close(&mut self) {
        self.0 += 1;
    }
}

/// The number of drawing commands skrifa draws for glyph `g` of `font` (0 for an empty glyph).
pub(crate) fn outline_commands(font: &[u8], g: u16) -> usize {
    let font = FontRef::new(font).unwrap();
    let glyph = font
        .outline_glyphs()
        .get(GlyphId::new(u32::from(g)))
        .expect("the glyph is in the font");
    let mut pen = Count(0);
    glyph
        .draw(
            DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
            &mut pen,
        )
        .expect("the glyph draws");
    pen.0
}

/// The sum of the big-endian 32-bit words of `data`, zero-padded.
fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |s, w| {
        let mut b = [0u8; 4];
        b[..w.len()].copy_from_slice(w);
        s.wrapping_add(u32::from_be_bytes(b))
    })
}

/// The table directory of an sfnt: (tag, checksum, offset, length).
fn directory(font: &[u8]) -> Vec<([u8; 4], u32, usize, usize)> {
    let n = usize::from(u16::from_be_bytes([font[4], font[5]]));
    (0..n)
        .map(|i| {
            let r = &font[12 + 16 * i..28 + 16 * i];
            let word = |k: usize| u32::from_be_bytes([r[k], r[k + 1], r[k + 2], r[k + 3]]);
            (
                [r[0], r[1], r[2], r[3]],
                word(4),
                word(8) as usize,
                word(12) as usize,
            )
        })
        .collect()
}

fn faces() -> Vec<(String, &'static [u8])> {
    bundled()
        .faces()
        .map(|f| {
            (
                format!("{} {}", f.family_name, f.weight.value()),
                bundled().face_data(f.key).unwrap(),
            )
        })
        .collect()
}

/// SV-2 on every bundled face: the subset of the glyphs of a short text keeps the glyph count and IDs, the
/// outline bytes of the used glyphs (and of their components), empty glyphs elsewhere, the `cmap`, `hmtx`,
/// `hhea`, `maxp`, `OS/2` and `name` as they are, `post` 3.0, long `loca`; the dropped tables are absent;
/// the table checksums and `head`'s checksum adjustment are valid.
#[test]
fn a_subset_keeps_the_glyph_ids_and_the_used_outlines() {
    for (name, font) in faces() {
        // The bundled faces are TrueType (`glyf`) and carry every table D3 keeps (the brief's stop).
        for tag in [
            b"glyf", b"loca", b"head", b"cmap", b"hhea", b"hmtx", b"maxp", b"OS/2", b"name", b"post",
        ] {
            assert!(table(font, tag).is_some(), "{name} has no {tag:?}");
        }
        assert!(table(font, b"CFF ").is_none(), "{name}");
        let chars: BTreeSet<char> = "Ab9+=∑∞µ".chars().collect();
        let used: BTreeSet<u16> = chars.iter().filter_map(|&c| glyph_of(font, c)).collect();
        assert!(used.len() >= 4, "{name}: {used:?}");
        assert_eq!(
            cmap_glyphs(font, &chars).unwrap(),
            used,
            "{name}: the cmap as skrifa reads it"
        );
        let subset = subset_glyf(font, &used).unwrap();
        assert!(subset.len() < font.len(), "{name}");
        let (before, after) = (glyph_bytes(font), glyph_bytes(&subset));
        assert_eq!(before.len(), after.len(), "{name}: the glyph count");
        for (g, (a, b)) in before.iter().zip(&after).enumerate() {
            let g = g as u16;
            if used.contains(&g) {
                assert_eq!(a, b, "{name}: glyph {g} keeps its outline bytes");
                assert_eq!(
                    outline_commands(&subset, g),
                    outline_commands(font, g),
                    "{name}: glyph {g}"
                );
            } else if !b.is_empty() {
                // A kept glyph that was not asked for is a component of one that was.
                assert_eq!(a, b, "{name}: glyph {g}");
            }
        }
        for c in &chars {
            assert_eq!(
                glyph_of(&subset, *c),
                glyph_of(font, *c),
                "{name}: {c} keeps its glyph ID"
            );
        }
        for tag in [b"cmap", b"hhea", b"hmtx", b"maxp", b"OS/2", b"name"] {
            assert_eq!(table(&subset, tag), table(font, tag), "{name}: {tag:?} is kept");
        }
        for tag in [b"GSUB", b"GPOS", b"GDEF", b"kern", b"MATH"] {
            assert_eq!(table(&subset, tag), None, "{name}: {tag:?} is dropped");
        }
        let post = table(&subset, b"post").unwrap();
        assert_eq!(post.len(), 32, "{name}");
        assert_eq!(post[..4], [0, 3, 0, 0], "{name}: post 3.0");
        assert_eq!(post[4..], table(font, b"post").unwrap()[4..32], "{name}");
        let head = table(&subset, b"head").unwrap();
        assert_eq!(head[50..52], [0, 1], "{name}: long loca");
        assert_eq!(head[..8], table(font, b"head").unwrap()[..8], "{name}");
        // The checksums: each table's (head's with the adjustment zeroed), and the whole file's.
        let dir = directory(&subset);
        let tags: Vec<[u8; 4]> = dir.iter().map(|d| d.0).collect();
        let mut sorted = tags.clone();
        sorted.sort();
        assert_eq!(tags, sorted, "{name}: tables in tag order");
        for (tag, sum, offset, length) in dir {
            assert_eq!(offset % 4, 0, "{name}: {tag:?} is aligned");
            let mut data = subset[offset..offset + length].to_vec();
            if &tag == b"head" {
                data[8..12].fill(0);
            }
            assert_eq!(checksum(&data), sum, "{name}: the checksum of {tag:?}");
        }
        assert_eq!(subset.len() % 4, 0, "{name}");
        assert_eq!(
            checksum(&subset),
            0xB1B0_AFBA,
            "{name}: head's checksum adjustment"
        );
        // Deterministic.
        assert_eq!(subset_glyf(font, &used).unwrap(), subset, "{name}");
    }
}

/// The components of a composite glyph, read from its record.
fn components(record: &[u8]) -> Vec<u16> {
    let mut out = Vec::new();
    let mut at = 10;
    loop {
        let flags = u16::from_be_bytes([record[at], record[at + 1]]);
        out.push(u16::from_be_bytes([record[at + 2], record[at + 3]]));
        at += 4 + if flags & 1 != 0 { 4 } else { 2 };
        at += if flags & 8 != 0 {
            2
        } else if flags & 0x40 != 0 {
            4
        } else if flags & 0x80 != 0 {
            8
        } else {
            0
        };
        if flags & 0x20 == 0 {
            return out;
        }
    }
}

/// SV-2: a composite glyph brings its components, which keep their outlines although nothing asked for them,
/// and the composite draws in the subset as in the face.
#[test]
fn a_composite_glyph_brings_its_components() {
    let font = bundled()
        .face_data(bundled().faces().next().unwrap().key)
        .unwrap();
    let glyphs = glyph_bytes(font);
    let (c, g) = "ÄÅÉÖÜäåéöü"
        .chars()
        .filter_map(|c| glyph_of(font, c).map(|g| (c, g)))
        .find(|&(_, g)| glyphs[usize::from(g)].first().is_some_and(|b| *b & 0x80 != 0))
        .expect("an accented letter is a composite glyph");
    let parts = components(glyphs[usize::from(g)]);
    assert!(!parts.is_empty() && !parts.contains(&g), "{c}");
    let subset = subset_glyf(font, &BTreeSet::from([g])).unwrap();
    let kept = glyph_bytes(&subset);
    for p in &parts {
        assert!(!kept[usize::from(*p)].is_empty(), "{c}: component {p} is kept");
        assert_eq!(
            kept[usize::from(*p)],
            glyphs[usize::from(*p)],
            "{c}: component {p}"
        );
    }
    assert!(outline_commands(&subset, g) > 0, "{c} draws");
    assert_eq!(outline_commands(&subset, g), outline_commands(font, g), "{c}");
    let other = glyph_of(font, 'Z').unwrap();
    assert!(kept[usize::from(other)].is_empty(), "an unused glyph is empty");
    assert!(!glyphs[usize::from(other)].is_empty());
}

// --- Hostile fonts, built in memory (D3's bounds) ---

/// A simple glyph: one contour of one on-curve point.
fn simple() -> Vec<u8> {
    let mut g = vec![0, 1, 0, 0, 0, 0, 0, 10, 0, 10];
    g.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0, 0]);
    g.push(0);
    g
}

/// A composite glyph of `parts` (offsets as bytes, no scale).
fn composite(parts: &[u16]) -> Vec<u8> {
    let mut g = vec![0xff, 0xff, 0, 0, 0, 0, 0, 10, 0, 10];
    for (i, p) in parts.iter().enumerate() {
        let more: u16 = if i + 1 < parts.len() { 0x20 } else { 0 };
        g.extend_from_slice(&(0x0002 | more).to_be_bytes());
        g.extend_from_slice(&p.to_be_bytes());
        g.extend_from_slice(&[0, 0]);
    }
    g
}

/// The tables of a minimal TrueType font of `glyphs` (long `loca`).
fn tables(glyphs: &[Vec<u8>]) -> BTreeMap<[u8; 4], Vec<u8>> {
    let n = glyphs.len() as u16;
    let mut head = vec![0u8; 54];
    head[..4].copy_from_slice(&[0, 1, 0, 0]);
    head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    head[50..52].copy_from_slice(&1i16.to_be_bytes());
    let mut maxp = vec![0, 0, 0x50, 0];
    maxp.extend_from_slice(&n.to_be_bytes());
    let mut hhea = vec![0u8; 36];
    hhea[..4].copy_from_slice(&[0, 1, 0, 0]);
    hhea[34..36].copy_from_slice(&n.to_be_bytes());
    let hmtx = vec![0u8; 4 * glyphs.len()];
    // One format 4 subtable with the final segment only (nothing mapped).
    let mut cmap = vec![0, 0, 0, 1, 0, 3, 0, 1, 0, 0, 0, 12];
    cmap.extend_from_slice(&[0, 4, 0, 24, 0, 0, 0, 2, 0, 2, 0, 0, 0, 0]);
    cmap.extend_from_slice(&[0xff, 0xff, 0, 0, 0xff, 0xff, 0, 1, 0, 0]);
    let mut glyf = Vec::new();
    let mut loca = Vec::new();
    for g in glyphs {
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        glyf.extend_from_slice(g);
    }
    loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
    BTreeMap::from([
        (*b"head", head),
        (*b"maxp", maxp),
        (*b"hhea", hhea),
        (*b"hmtx", hmtx),
        (*b"cmap", cmap),
        (*b"glyf", glyf),
        (*b"loca", loca),
    ])
}

/// An sfnt file of `tables` (checksums left zero: the subsetter does not read them).
fn sfnt(tables: &BTreeMap<[u8; 4], Vec<u8>>) -> Vec<u8> {
    let mut out = vec![0, 1, 0, 0];
    out.extend_from_slice(&(tables.len() as u16).to_be_bytes());
    out.extend_from_slice(&[0; 6]);
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        offset += data.len().div_ceil(4) * 4;
    }
    for data in tables.values() {
        out.extend_from_slice(data);
        out.resize(out.len().div_ceil(4) * 4, 0);
    }
    out
}

fn refusal(font: &[u8], glyphs: &[u16]) -> String {
    subset_glyf(font, &glyphs.iter().copied().collect())
        .unwrap_err()
        .to_string()
}

/// The synthetic fonts are well formed where they are not made hostile: they subset, and skrifa reads them.
#[test]
fn the_synthetic_font_subsets() {
    let font = sfnt(&tables(&[simple(), composite(&[2]), simple(), simple()]));
    let subset = subset_glyf(&font, &BTreeSet::from([1])).unwrap();
    assert!(FontRef::new(&subset).is_ok());
    let kept = glyph_bytes(&subset);
    assert_eq!(kept.len(), 4);
    assert_eq!(
        kept.iter().map(|g| !g.is_empty()).collect::<Vec<_>>(),
        [false, true, true, false]
    );
    assert_eq!(checksum(&subset), 0xB1B0_AFBA);
}

/// "the subset keeps `cvt `, `fpgm` and `prep` byte for byte when the face has them". A hinted
/// face's kept glyphs keep their instructions, so the subset keeps the tables they call into; a face without the
/// three gains none of them.
#[test]
fn a_hinted_face_keeps_its_hinting_tables_byte_for_byte() {
    let mut t = tables(&[simple(), simple(), simple()]);
    let plain = subset_glyf(&sfnt(&t), &BTreeSet::from([1])).unwrap();
    for tag in [b"cvt ", b"fpgm", b"prep"] {
        assert_eq!(table(&plain, tag), None, "{}", String::from_utf8_lossy(tag));
    }
    let hinting = [
        (*b"cvt ", vec![0, 10, 0, 20, 0xff, 0xf6]),
        (*b"fpgm", vec![0xb0, 0x00, 0x2c, 0x01, 0x2d]),
        (*b"prep", vec![0xb8, 0x01, 0xff, 0x85, 0x00]),
    ];
    t.extend(hinting.clone());
    let font = sfnt(&t);
    let subset = subset_glyf(&font, &BTreeSet::from([1])).unwrap();
    assert!(FontRef::new(&subset).is_ok());
    for (tag, bytes) in &hinting {
        assert_eq!(
            table(&subset, tag),
            Some(&bytes[..]),
            "{}",
            String::from_utf8_lossy(tag)
        );
    }
}

/// SV-2: a `loca` that points past the end of `glyf` is refused.
#[test]
fn a_loca_past_glyf_is_refused() {
    let mut t = tables(&[simple(), simple(), simple()]);
    let end = t[b"glyf"].len() as u32;
    t.get_mut(b"loca").unwrap()[8..12].copy_from_slice(&(end + 40).to_be_bytes());
    let font = sfnt(&t);
    assert_eq!(refusal(&font, &[1]), "glyph 1 lies outside the `glyf` table");
    // A reversed range is refused as well: glyph 1 (20 bytes) now starts at 50 and ends at 40.
    let mut t = tables(&[simple(), simple(), simple()]);
    t.get_mut(b"loca").unwrap()[4..8].copy_from_slice(&50u32.to_be_bytes());
    assert_eq!(refusal(&sfnt(&t), &[1]), "glyph 1 lies outside the `glyf` table");
}

/// SV-2: a composite glyph that contains itself, directly or through another, is refused.
#[test]
fn a_composite_cycle_is_refused() {
    let font = sfnt(&tables(&[simple(), composite(&[2]), composite(&[0, 1])]));
    assert_eq!(refusal(&font, &[1]), "composite glyph 1 contains itself");
    let font = sfnt(&tables(&[simple(), composite(&[1])]));
    assert_eq!(refusal(&font, &[1]), "composite glyph 1 contains itself");
}

/// A composite chain of `levels` composites over one simple glyph: glyph `i` (1 … levels) holds glyph `i + 1`.
fn chain(levels: u16) -> Vec<u8> {
    let mut glyphs = vec![simple()];
    for i in 1..=levels {
        glyphs.push(composite(&[i + 1]));
    }
    glyphs.push(simple());
    sfnt(&tables(&glyphs))
}

/// Composites nest at most 16 levels deep; the 17th level is refused, also when the deep part was reached
/// first from a shallower glyph.
#[test]
fn a_composite_deeper_than_16_levels_is_refused() {
    assert!(subset_glyf(&chain(16), &BTreeSet::from([1])).is_ok());
    assert_eq!(
        refusal(&chain(17), &[1]),
        "composite glyph 1 nests more than 16 levels deep"
    );
    // Glyph 1 heads a chain of 16 levels (accepted, and remembered); glyph 18 holds glyph 1, so it is 17 deep.
    let mut glyphs = vec![simple()];
    for i in 1..=16 {
        glyphs.push(composite(&[i + 1]));
    }
    glyphs.push(simple());
    glyphs.push(composite(&[1]));
    let font = sfnt(&tables(&glyphs));
    assert!(subset_glyf(&font, &BTreeSet::from([1])).is_ok());
    assert_eq!(
        refusal(&font, &[1, 18]),
        "composite glyph 18 nests more than 16 levels deep"
    );
}

/// SV-2: a table directory that runs past the end of the file is refused, and so is a table outside it.
#[test]
fn a_truncated_table_directory_and_a_table_outside_the_file_are_refused() {
    let font = sfnt(&tables(&[simple(), simple()]));
    assert_eq!(
        refusal(&font[..12 + 16 * 3], &[1]),
        "the font's table directory runs past the end of the file"
    );
    assert_eq!(
        refusal(&font[..8], &[1]),
        "the font's table directory runs past the end of the file"
    );
    // The last table (`maxp` in tag order) now ends 4 bytes past the file.
    let short = &font[..font.len() - 4];
    let last = directory(&font).into_iter().max_by_key(|d| d.2).unwrap();
    assert_eq!(&last.0, b"maxp");
    assert_eq!(refusal(short, &[1]), "the `maxp` table lies outside the file");
}

/// A face over 16 MiB is refused before anything is read.
#[test]
fn a_face_over_16_mib_is_refused() {
    let big = vec![0u8; MAX_FONT_BYTES + 1];
    assert_eq!(
        refusal(&big, &[1]),
        "the font file is 16777217 bytes, more than the 16 MiB a subset is made from"
    );
    assert_eq!(
        cmap_glyphs(&big, &BTreeSet::from(['a'])).unwrap_err(),
        SubsetError::TooLarge { bytes: 16_777_217 }
    );
}

/// The other refusals, each with its text: not a font, a collection, a CFF face, a missing table, a short table,
/// an unknown `loca` format, a glyph past the glyph count, a truncated composite.
#[test]
fn malformed_fonts_are_refused_in_words() {
    assert_eq!(
        refusal(b"not a font", &[1]),
        "the data is not a TrueType or OpenType font"
    );
    assert_eq!(
        refusal(b"ttcf\0\x01\0\0\0\0\0\0", &[1]),
        "the file is a font collection; only a single face is subset"
    );
    let mut t = tables(&[simple(), simple()]);
    let glyf = t.remove(b"glyf").unwrap();
    t.insert(*b"CFF ", glyf);
    assert_eq!(
        refusal(&sfnt(&t), &[1]),
        "the font has no TrueType outlines (no `glyf` table)"
    );
    let mut t = tables(&[simple(), simple()]);
    t.remove(b"hmtx");
    assert_eq!(refusal(&sfnt(&t), &[1]), "the font has no `hmtx` table");
    let mut t = tables(&[simple(), simple()]);
    t.get_mut(b"head").unwrap().truncate(50);
    assert_eq!(refusal(&sfnt(&t), &[1]), "the `head` table is too short");
    let mut t = tables(&[simple(), simple()]);
    t.get_mut(b"loca").unwrap().truncate(8);
    assert_eq!(refusal(&sfnt(&t), &[1]), "the `loca` table is too short");
    let mut t = tables(&[simple(), simple()]);
    t.get_mut(b"head").unwrap()[50..52].copy_from_slice(&7i16.to_be_bytes());
    assert_eq!(
        refusal(&sfnt(&t), &[1]),
        "the `head` table names the unknown `loca` format 7"
    );
    let font = sfnt(&tables(&[simple(), simple()]));
    assert_eq!(
        refusal(&font, &[2]),
        "glyph 2 is not in the font, which has 2 glyphs"
    );
    let mut cut = composite(&[0]);
    cut.truncate(13);
    let font = sfnt(&tables(&[simple(), cut]));
    assert_eq!(refusal(&font, &[1]), "composite glyph 1 is truncated");
    let font = sfnt(&tables(&[simple(), composite(&[5])]));
    assert_eq!(
        refusal(&font, &[1]),
        "glyph 5 is not in the font, which has 2 glyphs"
    );
}
