// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Import boundaries and manufacturer compatibility evidence.

use ew_import_eplan::read;

fn zip(name: &str, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let crc = crc32fast::hash(data);
    out.extend(b"PK\x03\x04");
    for n in [20u16, 0, 0, 0, 0] {
        out.extend(n.to_le_bytes());
    }
    for n in [crc, data.len() as u32, data.len() as u32] {
        out.extend(n.to_le_bytes());
    }
    out.extend((name.len() as u16).to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend(name.as_bytes());
    out.extend(data);
    let start = out.len() as u32;
    out.extend(b"PK\x01\x02");
    for n in [20u16, 20, 0, 0, 0, 0] {
        out.extend(n.to_le_bytes());
    }
    for n in [crc, data.len() as u32, data.len() as u32] {
        out.extend(n.to_le_bytes());
    }
    for n in [name.len() as u16, 0, 0, 0, 0] {
        out.extend(n.to_le_bytes());
    }
    for n in [0u32, 0] {
        out.extend(n.to_le_bytes());
    }
    out.extend(name.as_bytes());
    let size = out.len() as u32 - start;
    out.extend(b"PK\x05\x06");
    for n in [0u16, 0, 1, 1] {
        out.extend(n.to_le_bytes());
    }
    out.extend(size.to_le_bytes());
    out.extend(start.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
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
struct Entry<'a> {
    name: &'a str,
    method: u16,
    flags: u16,
    data: Vec<u8>,
    size: u32,
    crc: u32,
}

impl<'a> Entry<'a> {
    fn stored(name: &'a str, data: &[u8]) -> Self {
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
fn zip_of(entries: &[Entry<'_>], local: Option<(usize, u32)>) -> Vec<u8> {
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
fn seven(files: &[(&str, &[u8])], encoded: bool) -> Vec<u8> {
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
