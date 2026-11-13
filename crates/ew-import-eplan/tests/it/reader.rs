// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Import boundaries and manufacturer compatibility evidence.

use ew_import_eplan::read;

fn zip(name: &str, data: &[u8]) -> Vec<u8> {
    zip_of(&[Entry::stored(name, data)], None)
}

#[test]
fn ordinary_zip_crc_paths_and_declared_bombs_are_checked() {
    let xml =
        br#"<partsmanagement><part><functiontemplate connectionDesignation="1"/></part></partsmanagement>"#;
    assert!(read(&zip("parts.xml", xml), "sample.edz").is_ok());
    assert_eq!(
        read(&zip("../parts.xml", xml), "sample.edz").unwrap_err(),
        "eplan-archive-path"
    );
    assert_eq!(
        read(&zip("C:/parts.xml", xml), "sample.edz").unwrap_err(),
        "eplan-archive-path"
    );
    let mut bad = zip("parts.xml", xml);
    bad[39] ^= 1;
    assert_eq!(read(&bad, "sample.edz").unwrap_err(), "eplan-archive-invalid");
    let mut bomb = zip("parts.xml", xml);
    let centre = 39 + xml.len();
    bomb[centre + 24..centre + 28].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(read(&bomb, "sample.edz").unwrap_err(), "eplan-input-limit");
}

/// One ZIP entry: its name, method, flags, the stored bytes, the declared size and CRC.
pub(crate) struct Entry<'a> {
    name: &'a str,
    method: u16,
    flags: u16,
    data: Vec<u8>,
    size: u32,
    crc: u32,
}

impl<'a> Entry<'a> {
    pub(crate) fn stored(name: &'a str, data: &[u8]) -> Self {
        Entry {
            name,
            method: 0,
            flags: 0,
            data: data.to_vec(),
            size: data.len() as u32,
            crc: crc32fast::hash(data),
        }
    }
}

/// A ZIP of `entries`; `local` overrides the local header offset of the central record at that index.
pub(crate) fn zip_of(entries: &[Entry<'_>], local: Option<(usize, u32)>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut offsets = Vec::new();
    for e in entries {
        offsets.push(out.len() as u32);
        out.extend(b"PK\x03\x04");
        for n in [20u16, e.flags, e.method, 0, 0] {
            out.extend(n.to_le_bytes());
        }
        for n in [e.crc, e.data.len() as u32, e.size] {
            out.extend(n.to_le_bytes());
        }
        out.extend((e.name.len() as u16).to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(e.name.as_bytes());
        out.extend(&e.data);
    }
    let start = out.len() as u32;
    for (i, e) in entries.iter().enumerate() {
        out.extend(b"PK\x01\x02");
        for n in [20u16, 20, e.flags, e.method, 0, 0] {
            out.extend(n.to_le_bytes());
        }
        for n in [e.crc, e.data.len() as u32, e.size] {
            out.extend(n.to_le_bytes());
        }
        for n in [e.name.len() as u16, 0, 0, 0, 0] {
            out.extend(n.to_le_bytes());
        }
        let at = match local {
            Some((j, at)) if j == i => at,
            _ => offsets[i],
        };
        for n in [0u32, at] {
            out.extend(n.to_le_bytes());
        }
        out.extend(e.name.as_bytes());
    }
    let size = out.len() as u32 - start;
    out.extend(b"PK\x05\x06");
    let count = entries.len() as u16;
    for n in [0u16, 0, count, count] {
        out.extend(n.to_le_bytes());
    }
    out.extend(size.to_le_bytes());
    out.extend(start.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

const PARTS: &[u8] =
    br#"<partsmanagement><part><functiontemplate connectionDesignation="1"/></part></partsmanagement>"#;

/// M25-p2#4: a ratio bomb with truthful sizes, overlapping entries, the encryption flag and names that differ in
/// case only are refused, each with its key.
#[test]
fn zip_ratio_overlap_encryption_and_case_duplicates_are_refused() {
    let spaces = vec![b' '; 4 * 1024 * 1024];
    let deflated = miniz_oxide::deflate::compress_to_vec(&spaces, 6);
    let bomb = Entry {
        name: "parts.xml",
        method: 8,
        flags: 0,
        size: spaces.len() as u32,
        crc: crc32fast::hash(&spaces),
        data: deflated,
    };
    assert_eq!(
        read(&zip_of(&[bomb], None), "a.edz").unwrap_err(),
        "eplan-input-limit"
    );
    let encrypted = Entry {
        flags: 1,
        ..Entry::stored("parts.xml", PARTS)
    };
    assert_eq!(
        read(&zip_of(&[encrypted], None), "a.edz").unwrap_err(),
        "eplan-archive-method"
    );
    let twins = [
        Entry::stored("Parts.xml", PARTS),
        Entry::stored("parts.XML", PARTS),
    ];
    assert_eq!(
        read(&zip_of(&twins, None), "a.edz").unwrap_err(),
        "eplan-archive-path"
    );
    // The second entry's local header lies inside the first entry's data.
    let inner = zip_of(&[Entry::stored("b.xml", PARTS)], None);
    let local_len = 30 + "b.xml".len() + PARTS.len();
    let outer = [
        Entry::stored("a.xml", &inner[..local_len]),
        Entry::stored("b.xml", PARTS),
    ];
    let overlap = zip_of(&outer, Some((1, 30 + "a.xml".len() as u32)));
    assert_eq!(read(&overlap, "a.edz").unwrap_err(), "eplan-archive-invalid");
}

/// A 7z NUMBER.
fn number(v: u64) -> Vec<u8> {
    if v < 0x80 {
        vec![v as u8]
    } else if v < 0x4000 {
        vec![0x80 | (v >> 8) as u8, v as u8]
    } else {
        let mut out = vec![0xff];
        out.extend(v.to_le_bytes());
        out
    }
}

/// A 7z archive of `packed` streams and the next header `next` behind them.
fn seven_of(packed: &[u8], next: &[u8]) -> Vec<u8> {
    let mut start = Vec::new();
    start.extend((packed.len() as u64).to_le_bytes());
    start.extend((next.len() as u64).to_le_bytes());
    start.extend(crc32fast::hash(next).to_le_bytes());
    let mut out = b"7z\xbc\xaf\x27\x1c\x00\x04".to_vec();
    out.extend(crc32fast::hash(&start).to_le_bytes());
    out.extend(start);
    out.extend(packed);
    out.extend(next);
    out
}

/// A non-solid 7z archive of copy-coded members, one folder each; `encoded` stores its header as a copy-coded
/// stream behind them (an encoded header).
pub(crate) fn seven(files: &[(&str, &[u8])], encoded: bool) -> Vec<u8> {
    let mut packed = Vec::new();
    let mut h = vec![0x01, 0x04, 0x06, 0x00];
    h.extend(number(files.len() as u64));
    h.push(0x09);
    for (_, d) in files {
        packed.extend(*d);
        h.extend(number(d.len() as u64));
    }
    h.extend([0x00, 0x07, 0x0b]);
    h.extend(number(files.len() as u64));
    h.push(0x00);
    for _ in files {
        h.extend([0x01, 0x01, 0x00]);
    }
    h.push(0x0c);
    for (_, d) in files {
        h.extend(number(d.len() as u64));
    }
    h.extend([0x0a, 0x01]);
    for (_, d) in files {
        h.extend(crc32fast::hash(d).to_le_bytes());
    }
    h.extend([0x00, 0x08, 0x00, 0x00, 0x05]);
    h.extend(number(files.len() as u64));
    let names: Vec<u8> = files
        .iter()
        .flat_map(|(n, _)| n.encode_utf16().chain([0]).flat_map(u16::to_le_bytes))
        .collect();
    h.push(0x11);
    h.extend(number(names.len() as u64 + 1));
    h.push(0x00);
    h.extend(names);
    h.extend([0x00, 0x00]);
    if !encoded {
        return seven_of(&packed, &h);
    }
    let mut e = vec![0x17, 0x06];
    e.extend(number(packed.len() as u64));
    e.extend([0x01, 0x09]);
    e.extend(number(h.len() as u64));
    e.extend([0x00, 0x07, 0x0b, 0x01, 0x00, 0x01, 0x01, 0x00, 0x0c]);
    e.extend(number(h.len() as u64));
    e.extend([0x0a, 0x01]);
    e.extend(crc32fast::hash(&h).to_le_bytes());
    e.extend([0x00, 0x00]);
    packed.extend(&h);
    seven_of(&packed, &e)
}

/// The main streams of a header with one folder of `coder` (flags, method and properties) over `pack` packed
/// bytes that unpack to `unpack`.
fn one_folder(coder: &[u8], pack: u64, unpack: u64) -> Vec<u8> {
    let mut h = vec![0x01, 0x04, 0x06, 0x00, 0x01, 0x09];
    h.extend(number(pack));
    h.extend([0x00, 0x07, 0x0b, 0x01, 0x00, 0x01]);
    h.extend(coder);
    h.push(0x0c);
    h.extend(number(unpack));
    h.extend([0x00, 0x00, 0x00]);
    h
}

/// M25-p2#1 and #4: the 7z branch reads copy-coded members under a plain and an encoded header, and EW's own
/// header walk refuses what `sevenz-rust2` would allocate for or decode: a damaged start header, too many
/// entries, an oversized dictionary, a method other than copy, LZMA and LZMA2, an oversized encoded header and a
/// declared ratio past 200 : 1.
#[test]
fn seven_zip_members_read_and_its_header_is_bounded_before_decoding() {
    for encoded in [false, true] {
        let archive = seven(
            &[("EDZ/parts.xml", PARTS), ("EDZ/pictures/a.jpg", b"\xff\xd8")],
            encoded,
        );
        let symbols = read(&archive, "sample.edz").unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].source, "EDZ/parts.xml");
        assert_eq!(
            symbols[0].notes,
            [
                "eplan-generated-body",
                "eplan-function-review",
                "eplan-archive-skipped"
            ]
        );
    }
    let mut damaged = seven(&[("parts.xml", PARTS)], false);
    damaged[12] ^= 1;
    assert_eq!(read(&damaged, "a.edz").unwrap_err(), "eplan-archive-invalid");
    let mut files = vec![0x01, 0x05];
    files.extend(number(5000));
    files.extend([0x00, 0x00]);
    assert_eq!(
        read(&seven_of(&[], &files), "a.edz").unwrap_err(),
        "eplan-input-limit"
    );
    // LZMA2 with its 4 GiB dictionary (property 40), and the AES method.
    let lzma2 = one_folder(&[0x21, 0x21, 0x01, 40], 16, 16);
    assert_eq!(
        read(&seven_of(&[0; 16], &lzma2), "a.edz").unwrap_err(),
        "eplan-input-limit"
    );
    let aes = one_folder(&[0x04, 0x06, 0xf1, 0x07, 0x01], 16, 16);
    assert_eq!(
        read(&seven_of(&[0; 16], &aes), "a.edz").unwrap_err(),
        "eplan-archive-method"
    );
    let ratio = one_folder(&[0x01, 0x00], 16, 1024 * 1024);
    assert_eq!(
        read(&seven_of(&[0; 16], &ratio), "a.edz").unwrap_err(),
        "eplan-input-limit"
    );
    let mut big = vec![0x17];
    big.extend(&one_folder(&[0x01, 0x00], 16, 1 << 30)[2..]);
    assert_eq!(
        read(&seven_of(&[0; 16], &big), "a.edz").unwrap_err(),
        "eplan-input-limit"
    );
}

/// M25-p2#2: one budget across the documents of a read: parts within the per-symbol cap but past the total of
/// 65,536 graphics and connection points are refused before they are made.
#[test]
fn the_items_of_a_read_share_one_budget() {
    let pins = vec!["a"; 4000].join("&#10;");
    let part = format!(r#"<part><functiontemplate connectionDesignation="{pins}"/></part>"#);
    let doc = |n: usize| format!("<partsmanagement>{}</partsmanagement>", part.repeat(n));
    assert_eq!(read(doc(8).as_bytes(), "parts.xml").unwrap().len(), 8);
    assert_eq!(
        read(doc(9).as_bytes(), "parts.xml").unwrap_err(),
        "eplan-input-limit"
    );
    let a = doc(4);
    let b = doc(5);
    let archive = zip_of(
        &[
            Entry::stored("a.xml", a.as_bytes()),
            Entry::stored("b.xml", b.as_bytes()),
        ],
        None,
    );
    assert_eq!(read(&archive, "a.edz").unwrap_err(), "eplan-input-limit");
}

/// SM25#1: a 10,000-level XML is refused before the parser recurses into it, and 64 levels still parse.
#[test]
fn deep_xml_is_refused_before_parsing() {
    let deep = |n: usize| format!("{}{}", "<a x='>/'>".repeat(n), "</a>".repeat(n));
    for n in [65, 1000, 10_000] {
        assert_eq!(
            read(deep(n).as_bytes(), "deep.xml").unwrap_err(),
            "eplan-input-limit"
        );
    }
    assert_eq!(
        read(deep(64).as_bytes(), "deep.xml").unwrap_err(),
        "eplan-format-unsupported"
    );
    let commented = format!(
        "<partsmanagement><!--{}-->{}</partsmanagement>",
        "<a>".repeat(100),
        "<b/>".repeat(100)
    );
    assert_eq!(
        read(commented.as_bytes(), "parts.xml").unwrap_err(),
        "eplan-no-symbols"
    );
}

/// M25-p4#3: the attributes of one element are counted before parsing, because the parser's duplicate check is
/// quadratic in them. 257 are the limit's refusal even where the parser would refuse the element for its own reason
/// (here the last attribute repeats the first); 256, each with an `=` in its quoted value, still read.
#[test]
fn an_elements_attributes_are_counted_before_parsing() {
    let attributes = |n: usize| (0..n).map(|i| format!(" a{i}='x=y'")).collect::<String>();
    let parts = |attributes: &str| {
        format!(
            r#"<partsmanagement{attributes}><part><functiontemplate connectionDesignation="1"/></part></partsmanagement>"#
        )
    };
    assert_eq!(
        read(parts(&attributes(256)).as_bytes(), "parts.xml")
            .unwrap()
            .len(),
        1
    );
    let repeated = format!("{} a0='x'", attributes(256));
    assert_eq!(
        read(parts(&repeated).as_bytes(), "parts.xml").unwrap_err(),
        "eplan-input-limit"
    );
}

/// The range encoder of the LZMA SDK, as far as a stream of literals and one long match needs it (M25-p4#4): the LZMA
/// and LZMA2 fixtures are made here, with no external tool and no third-party content.
struct RangeEncoder {
    low: u64,
    range: u32,
    cache: u8,
    pending: u64,
    out: Vec<u8>,
}

impl RangeEncoder {
    fn new() -> Self {
        RangeEncoder {
            low: 0,
            range: u32::MAX,
            cache: 0,
            pending: 1,
            out: Vec::new(),
        }
    }

    fn shift_low(&mut self) {
        let carry = (self.low >> 32) as u8;
        if carry != 0 || self.low < 0xff00_0000 {
            let mut byte = self.cache;
            while self.pending > 0 {
                self.out.push(byte.wrapping_add(carry));
                byte = 0xff;
                self.pending -= 1;
            }
            self.cache = (self.low >> 24) as u8;
        }
        self.pending += 1;
        self.low = (self.low & 0x00ff_ffff) << 8;
    }

    fn normalize(&mut self) {
        if self.range < 1 << 24 {
            self.range <<= 8;
            self.shift_low();
        }
    }

    /// One bit under the adaptive probability `prob` (11 bits, moved by 5).
    fn bit(&mut self, prob: &mut u16, bit: u32) {
        let bound = (self.range >> 11) * u32::from(*prob);
        if bit == 0 {
            self.range = bound;
            *prob += (2048 - *prob) >> 5;
        } else {
            self.low += u64::from(bound);
            self.range -= bound;
            *prob -= *prob >> 5;
        }
        self.normalize();
    }

    /// The `bits` low bits of `symbol` through the bit tree `probs`, the highest bit first.
    fn tree(&mut self, probs: &mut [u16], bits: u32, symbol: u32) {
        let mut at = 1;
        for i in (0..bits).rev() {
            let bit = (symbol >> i) & 1;
            self.bit(&mut probs[at], bit);
            at = (at << 1) | bit as usize;
        }
    }

    /// The `bits` low bits of `symbol` through the bit tree `probs`, the lowest bit first.
    fn reverse(&mut self, probs: &mut [u16], bits: u32, symbol: u32) {
        let mut at = 1;
        for i in 0..bits {
            let bit = (symbol >> i) & 1;
            self.bit(&mut probs[at], bit);
            at = (at << 1) | bit as usize;
        }
    }

    /// The `bits` low bits of `value` at probability one half, the highest first.
    fn direct(&mut self, value: u32, bits: u32) {
        for i in (0..bits).rev() {
            self.range >>= 1;
            if (value >> i) & 1 == 1 {
                self.low += u64::from(self.range);
            }
            self.normalize();
        }
    }

    fn finish(mut self) -> Vec<u8> {
        for _ in 0..5 {
            self.shift_low();
        }
        self.out
    }
}

/// `data` as a raw LZMA stream (lc 3, lp 0, pb 2, no end marker) of literals and, with `repeat = (at, distance)`,
/// one match: the bytes from `at` repeat those `distance` bytes back, as many as `distance` (at most 273). A
/// distance over 128 makes the decoder read direct bits, the assembly loop of lzma-rust2's range decoder.
fn lzma(data: &[u8], repeat: Option<(usize, usize)>) -> Vec<u8> {
    let mut rc = RangeEncoder::new();
    let mut is_match = [[1024u16; 4]; 12];
    let mut is_rep = [1024u16; 12];
    let mut literal = vec![[1024u16; 0x300]; 8];
    let (mut choice, mut low, mut mid, mut high) =
        ([1024u16; 2], [[1024u16; 8]; 4], [[1024u16; 8]; 4], [1024u16; 256]);
    let mut slots = [[1024u16; 64]; 4];
    let mut align = [1024u16; 16];
    let (mut state, mut pos, mut rep) = (0usize, 0usize, 0usize);
    while pos < data.len() {
        let pos_state = pos & 3;
        if let Some((at, distance)) = repeat.filter(|&(at, _)| at == pos) {
            let len = distance;
            assert!((129..=273).contains(&len) && at >= distance && at + len <= data.len());
            assert_eq!(data[at..at + len], data[at - distance..at - distance + len]);
            rc.bit(&mut is_match[state][pos_state], 1);
            rc.bit(&mut is_rep[state], 0);
            state = if state < 7 { 7 } else { 10 };
            let l = (len - 2) as u32;
            if l < 8 {
                rc.bit(&mut choice[0], 0);
                rc.tree(&mut low[pos_state], 3, l);
            } else if l < 16 {
                rc.bit(&mut choice[0], 1);
                rc.bit(&mut choice[1], 0);
                rc.tree(&mut mid[pos_state], 3, l - 8);
            } else {
                rc.bit(&mut choice[0], 1);
                rc.bit(&mut choice[1], 1);
                rc.tree(&mut high, 8, l - 16);
            }
            let d = (distance - 1) as u32;
            let top = 31 - d.leading_zeros();
            let slot = (top << 1) | ((d >> (top - 1)) & 1);
            assert!(slot >= 14, "a slot of direct bits");
            rc.tree(&mut slots[(len - 2).min(3)], 6, slot);
            let footer = (slot >> 1) - 1;
            let reduced = d - ((2 | (slot & 1)) << footer);
            rc.direct(reduced >> 4, footer - 4);
            rc.reverse(&mut align, 4, reduced & 15);
            (pos, rep) = (pos + len, distance);
            continue;
        }
        let byte = u32::from(data[pos]);
        rc.bit(&mut is_match[state][pos_state], 0);
        let previous = if pos == 0 { 0 } else { data[pos - 1] };
        let probs = &mut literal[usize::from(previous >> 5)];
        if state < 7 {
            rc.tree(probs, 8, byte);
        } else {
            // The literal after a match is coded against the byte at the match's distance.
            let mut matched = u32::from(data[pos - rep]);
            let (mut offset, mut symbol) = (0x100u32, 1u32);
            for i in (0..8).rev() {
                matched <<= 1;
                let match_bit = matched & offset;
                let bit = (byte >> i) & 1;
                rc.bit(&mut probs[(offset + match_bit + symbol) as usize], bit);
                symbol = (symbol << 1) | bit;
                offset &= 0u32.wrapping_sub(bit) ^ !match_bit;
            }
        }
        state = match state {
            0..=3 => 0,
            4..=9 => state - 3,
            _ => state - 6,
        };
        pos += 1;
    }
    rc.finish()
}

/// One parts XML member `EDZ/parts.xml`, LZMA2-coded (one LZMA chunk, a 64 KiB dictionary), under an LZMA-encoded
/// header, as 7-Zip writes an EDZ: its two parts are alike, so the member's stream holds a match of a distance over
/// 128 (M25-p4#4). Returns the archive and the length of the member's packed stream, which comes first.
fn lzma_seven() -> (Vec<u8>, usize) {
    let part = r#"<part P_ARTICLE_PARTNR="An LZMA2 seed of the 7z reader's own test"><functiontemplate connectionDesignation="1&#10;2&#10;FE"/></part>"#;
    let data = format!("<partsmanagement>{part}{part}</partsmanagement>");
    let raw = lzma(data.as_bytes(), Some((17 + part.len(), part.len())));
    let (u, c) = (data.len() - 1, raw.len() - 1);
    let mut packed = vec![
        0xe0 | (u >> 16) as u8,
        (u >> 8) as u8,
        u as u8,
        (c >> 8) as u8,
        c as u8,
        0x5d,
    ];
    packed.extend(&raw);
    packed.push(0x00);
    let member = packed.len();
    // The plain header: one LZMA2 folder (dictionary property 8) of the one file.
    let mut h = vec![0x01, 0x04, 0x06, 0x00, 0x01, 0x09];
    h.extend(number(member as u64));
    h.extend([0x00, 0x07, 0x0b, 0x01, 0x00, 0x01, 0x21, 0x21, 0x01, 0x08, 0x0c]);
    h.extend(number(data.len() as u64));
    h.extend([0x0a, 0x01]);
    h.extend(crc32fast::hash(data.as_bytes()).to_le_bytes());
    h.extend([0x00, 0x08, 0x00, 0x00, 0x05, 0x01, 0x11]);
    let names: Vec<u8> = "EDZ/parts.xml"
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect();
    h.extend(number(names.len() as u64 + 1));
    h.push(0x00);
    h.extend(names);
    h.extend([0x00, 0x00]);
    // The header as one LZMA folder (a 64 KiB dictionary) behind the member.
    let header = lzma(&h, None);
    let mut e = vec![0x17, 0x06];
    e.extend(number(member as u64));
    e.extend([0x01, 0x09]);
    e.extend(number(header.len() as u64));
    e.extend([
        0x00, 0x07, 0x0b, 0x01, 0x00, 0x01, 0x23, 0x03, 0x01, 0x01, 0x05, 0x5d,
    ]);
    e.extend((1u32 << 16).to_le_bytes());
    e.push(0x0c);
    e.extend(number(h.len() as u64));
    e.extend([0x0a, 0x01]);
    e.extend(crc32fast::hash(&h).to_le_bytes());
    e.extend([0x00, 0x00]);
    packed.extend(header);
    (seven_of(&packed, &e), member)
}

// Allowed: the program boundary of a test, read once; the value never reaches a verdict.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// M25-p4#4: an LZMA2 member under an LZMA-encoded header reads through both decoders (EW's header walk decodes the
/// header, sevenz-rust2 the header again and the member), and a damaged byte in either stream is refused after its
/// decoder ran. The archive is the `symbol_authoring` fuzz seed `lzma2-7z.edz` (`EW_BLESS=1` writes it).
#[test]
fn seven_zip_lzma2_members_read_under_an_lzma_encoded_header() {
    let (archive, member) = lzma_seven();
    let symbols = read(&archive, "sample.edz").unwrap();
    let read_back: Vec<(&str, &str, Vec<&str>)> = symbols
        .iter()
        .map(|s| {
            let pins = s.pins.iter().map(|p| p.designation.as_str()).collect();
            (s.source.as_str(), s.name.as_str(), pins)
        })
        .collect();
    let part = (
        "EDZ/parts.xml",
        "An LZMA2 seed of the 7z reader's own test",
        vec!["1", "2", "FE"],
    );
    assert_eq!(read_back, [part.clone(), part]);
    for at in [32 + 6 + 2, 32 + member + 2] {
        let mut damaged = archive.clone();
        damaged[at] ^= 0x40;
        assert_eq!(
            read(&damaged, "sample.edz").unwrap_err(),
            "eplan-archive-invalid",
            "byte {at}"
        );
    }
    let seed = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lzma2-7z.edz");
    if bless() {
        std::fs::write(&seed, &archive).unwrap();
    }
    let held =
        std::fs::read(&seed).unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 writes it)", seed.display()));
    assert!(
        held == archive,
        "the fuzz seed is not this archive (EW_BLESS=1 writes it)"
    );
}

/// M25-p2#8 and SM25#4: a UTF-16 XML reads as XML, and a `.pxf` member or file is read as PXF.
#[test]
fn utf16_xml_and_pxf_names_are_read() {
    let mut utf16 = vec![0xff, 0xfe];
    utf16.extend(
        std::str::from_utf8(PARTS)
            .unwrap()
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    assert_eq!(read(&utf16, "parts.xml").unwrap()[0].pins[0].designation, "1");
    let pxf = br#"<EplanPxfRoot PxfVersion="1.23" Name="M"><MacroVariant><O31 A531="0/0" A532="4/0"/></MacroVariant></EplanPxfRoot>"#;
    assert_eq!(read(pxf, "macro.pxf").unwrap()[0].name, "M · 0");
    let archive = zip_of(&[Entry::stored("macro.pxf", pxf)], None);
    assert_eq!(read(&archive, "a.edz").unwrap()[0].notes, ["eplan-styles-review"]);
}

/// M25-p2#8: an XML of another kind in an archive is reported on the symbols read beside it.
#[test]
fn an_unread_xml_in_an_archive_is_reported() {
    let archive = zip_of(
        &[
            Entry::stored("parts.xml", PARTS),
            Entry::stored("other.xml", b"<other/>"),
        ],
        None,
    );
    assert_eq!(
        read(&archive, "a.edz").unwrap()[0].notes,
        [
            "eplan-generated-body",
            "eplan-function-review",
            "eplan-archive-skipped"
        ]
    );
}

#[test]
fn pxf_origin_y_axis_and_unresolved_function_are_explicit() {
    let source = br#"<EplanPxfRoot PxfVersion="1.23" Name="Example"><MacroVariant ReferencePoint="8/16/0" VariantId="0"><O31 A531="8/16" A532="12/20"/><O89 A1651="12/20" A1652="16/24"/><O17 A185="FE"><S40x1201 A762="8/16"/></O17><O999/></MacroVariant></EplanPxfRoot>"#;
    let symbols = read(source, "example.ems").unwrap();
    let s = &symbols[0];
    assert_eq!(s.graphics[0]["line"]["from"], serde_json::json!([0, 0]));
    assert_eq!(
        s.graphics[0]["line"]["to"],
        serde_json::json!([4_000_000, -4_000_000])
    );
    assert_eq!(s.pins[0].designation, "FE");
    assert_eq!(s.pins[0].direction, None);
    assert_eq!(s.notes, vec!["eplan-macro-incomplete", "eplan-styles-review"]);
}

#[test]
fn part_connections_generate_a_labelled_fallback() {
    let data=br#"<partsmanagement><part P_ARTICLE_PARTNR="Example"><functiontemplate connectionDesignation="1&#10;2&#10;FE"/></part></partsmanagement>"#;
    let symbols = read(data, "parts.xml").unwrap();
    let s = &symbols[0];
    assert_eq!(
        s.pins.iter().map(|p| p.designation.as_str()).collect::<Vec<_>>(),
        ["1", "2", "FE"]
    );
    assert_eq!(s.pins[2].position, [0, 12_000_000]);
    assert_eq!(s.notes, ["eplan-generated-body", "eplan-function-review"]);
}

#[test]
fn malformed_and_unsupported_inputs_refuse_without_panics() {
    for (data, name, error) in [
        (&b"\xff\0"[..], "library.slk", "eplan-binary-unsupported"),
        (&b"PK\x03\x04"[..], "archive.edz", "eplan-archive-invalid"),
        (
            &b"<EplanPxfRoot PxfVersion=\"2\"/>"[..],
            "macro.ema",
            "eplan-pxf-version",
        ),
        (
            &b"<!DOCTYPE a [<!ENTITY x 'a'>]><a>&x;</a>"[..],
            "parts.xml",
            "eplan-xml-invalid",
        ),
        (&b"<partsmanagement/>"[..], "../escape.xml", "eplan-archive-path"),
        (&b"<other/>"[..], "parts.xml", "eplan-format-unsupported"),
    ] {
        assert_eq!(read(data, name).unwrap_err(), error);
    }
}

/// External manufacturer files are opt-in local evidence and never repository fixtures.
#[test]
// The environment only selects a private test fixture; it never enters persisted or exported output.
#[allow(clippy::disallowed_methods)]
fn manufacturer_edz_when_supplied() {
    let Some(path) = std::env::var_os("EW_EPLAN_SAMPLE") else {
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    let symbols = read(&bytes, "manufacturer.edz").unwrap();
    assert!(symbols.iter().any(|s| !s.graphics.is_empty()));
    assert!(symbols.iter().any(|s| !s.pins.is_empty()));
    assert!(symbols.iter().all(|s| !s.notes.is_empty()));
}
