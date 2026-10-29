// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The chunk structure of PNG bytes, checked before the `png` crate decodes them (fuzz finding of CI run 40897313649:
//! `png` 0.18.1 indexes past a `PLTE` whose length is not a multiple of 3 and panics while it builds its palette).
//!
//! The rules are those of the PNG specification (third edition, §11.2.3 `PLTE`, §11.3.1.1 `tRNS`, §5.6 chunk
//! ordering) that bound the palette and transparency data the decoder indexes:
//!
//! - the first chunk is `IHDR` of 13 bytes;
//! - `PLTE` holds 1 to 256 entries of 3 bytes (a length of 3 to 768 that is a multiple of 3), no more than
//!   2^bit depth entries for colour type 3, and appears only for colour types 2, 3 and 6;
//! - `tRNS` holds 2 bytes for colour type 0, 6 bytes for colour type 2, no more entries than the `PLTE` before it for
//!   colour type 3, and does not appear for colour types 4 and 6;
//! - colour type 3 has a `PLTE` before the first `IDAT`, and the bytes reach an `IDAT` (both chunks must precede it).
//!
//! Only the chunks up to the first `IDAT` are read; CRCs are left to the decoder. The check is a pure function of the
//! bytes.

/// The PNG signature.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Whether the chunks of PNG `bytes` up to the first `IDAT` keep the rules of the module documentation. `false` refuses
/// the image before it reaches the decoder.
pub(crate) fn chunks_are_sound(bytes: &[u8]) -> bool {
    let Some(mut rest) = bytes.strip_prefix(&SIGNATURE) else {
        return false;
    };
    // (bit depth, colour type) of `IHDR`, and the entry count of `PLTE`.
    let mut header: Option<(u8, u8)> = None;
    let mut palette: Option<usize> = None;
    loop {
        let Some((kind, data, tail)) = next_chunk(rest) else {
            return false;
        };
        rest = tail;
        let Some((depth, colour)) = header else {
            if &kind == b"IHDR" && data.len() == 13 {
                header = Some((data[8], data[9]));
                continue;
            }
            return false;
        };
        match &kind {
            b"PLTE" => {
                if !plte_is_sound(data.len(), depth, colour) {
                    return false;
                }
                palette = Some(data.len() / 3);
            }
            b"tRNS" if !trns_is_sound(data.len(), colour, palette) => return false,
            b"IDAT" => return colour != 3 || palette.is_some(),
            b"IEND" => return false,
            _ => {}
        }
    }
}

/// The next chunk of `bytes`: its type, its data and the bytes after its CRC; `None` when it does not fit.
fn next_chunk(bytes: &[u8]) -> Option<([u8; 4], &[u8], &[u8])> {
    let (head, rest) = bytes.split_first_chunk::<8>()?;
    let len = usize::try_from(u32::from_be_bytes([head[0], head[1], head[2], head[3]])).ok()?;
    let kind = [head[4], head[5], head[6], head[7]];
    let data = rest.get(..len)?;
    let tail = rest.get(len.checked_add(4)?..)?;
    Some((kind, data, tail))
}

/// A `PLTE` of `len` bytes in an image of bit depth `depth` and colour type `colour`.
fn plte_is_sound(len: usize, depth: u8, colour: u8) -> bool {
    let entries = len / 3;
    let shape = len.is_multiple_of(3) && (1..=256).contains(&entries);
    match colour {
        // 2^depth entries at most; a depth of 8 or more already allows all 256.
        3 => shape && (depth >= 8 || entries <= 1 << depth),
        2 | 6 => shape,
        _ => false,
    }
}

/// A `tRNS` of `len` bytes in an image of colour type `colour` whose `PLTE` so far has `palette` entries.
fn trns_is_sound(len: usize, colour: u8, palette: Option<usize>) -> bool {
    match colour {
        0 => len == 2,
        2 => len == 6,
        3 => palette.is_some_and(|entries| len <= entries),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::decode_png;

    /// The CRC-32 of a chunk (ISO 3309, as the PNG specification uses it), so that the hand-built PNGs pass the
    /// decoder's CRC check and reach its palette code.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff_u32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    /// A PNG of `chunks` (type and data), with lengths and CRCs filled in.
    fn png_of(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut out = SIGNATURE.to_vec();
        for (kind, data) in chunks {
            out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
            let start = out.len();
            out.extend_from_slice(*kind);
            out.extend_from_slice(data);
            let crc = crc32(&out[start..]);
            out.extend_from_slice(&crc.to_be_bytes());
        }
        out
    }

    /// `IHDR` data of a 2 × 2 image.
    fn ihdr(depth: u8, colour: u8) -> Vec<u8> {
        vec![0, 0, 0, 2, 0, 0, 0, 2, depth, colour, 0, 0, 0]
    }

    /// The zlib stream of two 2-pixel rows of index 0 at bit depth 8 (filter byte 0, stored block).
    const IDAT_INDEXED_2X2: [u8; 17] = [
        0x78, 0x01, 0x01, 0x06, 0x00, 0xf9, 0xff, 0, 0, 0, 0, 0, 0, 0x00, 0x06, 0x00, 0x01,
    ];

    /// A 2 × 2 palette image (colour type 3, depth 8) with the `PLTE` and `tRNS` data given.
    fn indexed(plte: Option<&[u8]>, trns: Option<&[u8]>) -> Vec<u8> {
        let ihdr = ihdr(8, 3);
        let mut chunks: Vec<(&[u8; 4], &[u8])> = vec![(b"IHDR", &ihdr)];
        if let Some(plte) = plte {
            chunks.push((b"PLTE", plte));
        }
        if let Some(trns) = trns {
            chunks.push((b"tRNS", trns));
        }
        chunks.push((b"IDAT", &IDAT_INDEXED_2X2));
        chunks.push((b"IEND", &[]));
        png_of(&chunks)
    }

    /// The control: a well-formed palette image with `tRNS` passes the check and decodes.
    #[test]
    fn a_sound_palette_image_passes_and_decodes() {
        let png = indexed(Some(&[255, 0, 0, 0, 0, 255]), Some(&[128]));
        assert!(chunks_are_sound(&png));
        let img = decode_png(&png).expect("a sound palette image decodes");
        assert_eq!(img.rgba, [255, 0, 0, 128].repeat(4));
    }

    /// The fuzz finding, rebuilt with valid CRCs so that it reaches `png`'s palette code in an ordinary build: a `PLTE`
    /// of 14 bytes followed by `tRNS` made `png` 0.18.1 panic (`range end index 3 out of range for slice of length 2`).
    /// `decode_png` is called without `decode_image`'s `catch_unwind`: it must refuse, not panic.
    #[test]
    fn a_plte_length_that_is_not_a_multiple_of_3_is_refused_without_a_panic() {
        let plte = [255, 0, 0, 0, 255, 0, 0, 0, 255, 9, 9, 9, 7, 7];
        for png in [
            indexed(Some(&plte), Some(&[128])),
            indexed(Some(&plte[..13]), None),
        ] {
            assert!(!chunks_are_sound(&png));
            assert_eq!(decode_png(&png), None);
        }
    }

    #[test]
    fn a_plte_outside_1_to_256_entries_is_refused() {
        assert!(!chunks_are_sound(&indexed(Some(&[]), None)), "no entries");
        assert!(!chunks_are_sound(&indexed(Some(&[0; 771]), None)), "257 entries");
        assert!(chunks_are_sound(&indexed(Some(&[0; 768]), None)), "256 entries");
    }

    #[test]
    fn a_plte_with_more_entries_than_the_bit_depth_allows_is_refused() {
        let at_depth = |depth: u8, entries: usize| {
            let ihdr = ihdr(depth, 3);
            let plte = vec![0; entries * 3];
            png_of(&[(b"IHDR", &ihdr), (b"PLTE", &plte), (b"IDAT", &[]), (b"IEND", &[])])
        };
        assert!(chunks_are_sound(&at_depth(1, 2)));
        assert!(!chunks_are_sound(&at_depth(1, 3)), "3 entries at depth 1");
        assert!(chunks_are_sound(&at_depth(4, 16)));
        assert!(!chunks_are_sound(&at_depth(4, 17)), "17 entries at depth 4");
    }

    #[test]
    fn a_palette_image_without_plte_is_refused() {
        let png = indexed(None, None);
        assert!(!chunks_are_sound(&png));
        assert_eq!(decode_png(&png), None);
    }

    #[test]
    fn a_trns_with_more_entries_than_the_palette_is_refused() {
        let plte = [255, 0, 0, 0, 0, 255];
        assert!(chunks_are_sound(&indexed(Some(&plte), Some(&[1, 2]))), "2 of 2");
        let png = indexed(Some(&plte), Some(&[1, 2, 3]));
        assert!(!chunks_are_sound(&png), "3 alpha values for 2 entries");
        assert_eq!(decode_png(&png), None);
        // tRNS before PLTE has no palette to count against.
        let ihdr = ihdr(8, 3);
        let early = png_of(&[
            (b"IHDR", &ihdr),
            (b"tRNS", &[1]),
            (b"PLTE", &plte),
            (b"IDAT", &IDAT_INDEXED_2X2),
            (b"IEND", &[]),
        ]);
        assert!(!chunks_are_sound(&early), "tRNS before PLTE");
    }

    #[test]
    fn a_trns_of_the_wrong_length_for_the_colour_type_is_refused() {
        let with_trns = |colour: u8, trns: &[u8]| {
            let ihdr = ihdr(8, colour);
            png_of(&[(b"IHDR", &ihdr), (b"tRNS", trns), (b"IDAT", &[]), (b"IEND", &[])])
        };
        assert!(chunks_are_sound(&with_trns(0, &[0, 1])), "grey: 2 bytes");
        assert!(!chunks_are_sound(&with_trns(0, &[0, 1, 2])), "grey: 3 bytes");
        assert!(chunks_are_sound(&with_trns(2, &[0; 6])), "RGB: 6 bytes");
        assert!(!chunks_are_sound(&with_trns(2, &[0; 2])), "RGB: 2 bytes");
        assert!(
            !chunks_are_sound(&with_trns(4, &[0; 2])),
            "grey and alpha: none allowed"
        );
        assert!(!chunks_are_sound(&with_trns(6, &[0; 6])), "RGBA: none allowed");
    }

    #[test]
    fn a_plte_in_a_grey_image_is_refused() {
        for colour in [0, 4] {
            let ihdr = ihdr(8, colour);
            let png = png_of(&[
                (b"IHDR", &ihdr),
                (b"PLTE", &[0; 3]),
                (b"IDAT", &[]),
                (b"IEND", &[]),
            ]);
            assert!(!chunks_are_sound(&png), "colour type {colour}");
        }
    }

    #[test]
    fn a_broken_chunk_structure_is_refused() {
        let png = indexed(Some(&[255, 0, 0]), None);
        assert!(chunks_are_sound(&png));
        assert!(!chunks_are_sound(&png[..8]), "the signature alone");
        assert!(!chunks_are_sound(&png[..30]), "cut inside IHDR's CRC");
        assert!(!chunks_are_sound(&png[1..]), "no signature");
        let no_ihdr = png_of(&[(b"PLTE", &[0; 3]), (b"IDAT", &[]), (b"IEND", &[])]);
        assert!(!chunks_are_sound(&no_ihdr), "PLTE first");
        let short_ihdr = png_of(&[(b"IHDR", &[0; 12]), (b"IDAT", &[]), (b"IEND", &[])]);
        assert!(!chunks_are_sound(&short_ihdr), "IHDR of 12 bytes");
        let ihdr = ihdr(8, 6);
        let no_idat = png_of(&[(b"IHDR", &ihdr), (b"IEND", &[])]);
        assert!(!chunks_are_sound(&no_idat), "IEND before any IDAT");
    }

    /// The 93-byte input of the fuzz finding itself (CI run 40897313649, also in
    /// `fuzz/regressions/png_pdf/plte-length-not-a-multiple-of-3`): refused by the check. Its CRCs are zero (libFuzzer
    /// builds `png` with `cfg(fuzzing)`, which skips the CRC check), so an ordinary build's decoder refuses it as well.
    #[test]
    fn the_fuzz_finding_is_refused() {
        let bytes = FUZZ_FINDING;
        assert_eq!(bytes.len(), 93);
        assert!(!chunks_are_sound(&bytes));
        assert_eq!(decode_png(&bytes), None);
    }

    const FUZZ_FINDING: [u8; 93] = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00,
        0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x03, 0x00, 0x00, 0x00, 0x45, 0x68, 0xfd, 0x16, 0x00,
        0x00, 0x00, 0x0e, 0x50, 0x4c, 0x54, 0x45, 0xff, 0x74, 0x52, 0x4e, 0x53, 0x80, 0xad, 0x5e, 0x5b, 0x46,
        0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x49, 0x44, 0x41, 0x54, 0x78,
        0xda, 0x63, 0x60, 0x64, 0x64, 0x60, 0x60, 0x04, 0x00, 0x00, 0x0b, 0x00, 0x03, 0x4f, 0x41, 0x36, 0xf9,
        0xb2, 0xba, 0xb1, 0xbb, 0xae, 0x42, 0x60, 0x82,
    ];
}
