// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The generator of the hostile corpus's binary files and of the files too large to commit (WP-S6.04 D10): each file by
//! its corpus name. [`committed`] are the small binary files, committed under `hostile/` and compared with what this
//! module makes (`EW_BLESS=1` writes them); [`large`] are made here only, in memory, at each run.

use ew_api::zip::{Method, crc32, write};

/// The small binary files of the corpus that this module makes, by their path below `hostile/`.
pub fn committed() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("eplan/entry-parent.edz", eplan_entry_parent()),
        ("eplan/ratio-over-200.edz", eplan_ratio_over_200()),
        (
            "eplan/seven-zip-dictionary-4-gib.edz",
            seven_zip_dictionary_4_gib(),
        ),
        (
            "eplan/seven-zip-entries-over-the-limit.edz",
            seven_zip_entries_over_the_limit(),
        ),
        ("ewz/parent-entry.ewz", ewz_parent_entry()),
        ("raster/png-2-31-pixels.png", png_2_31_pixels()),
        ("xlsx/billion-laughs.xlsx", xlsx_with_rels(&billion_laughs_rels())),
        ("xlsx/entry-absolute.xlsx", xlsx_entry_named("/x")),
        ("xlsx/entry-drive.xlsx", xlsx_entry_named("C:x")),
        ("xlsx/entry-parent.xlsx", xlsx_entry_named("../../x")),
        ("xlsx/external-entity.xlsx", xlsx_with_rels(EXTERNAL_ENTITY_RELS)),
        ("xlsx/ratio-over-200.xlsx", xlsx_ratio_over_200()),
        ("xlsx/zip-bomb.xlsx", xlsx_zip_bomb()),
    ]
}

/// The files made at each run only, by their path below `hostile/` (none of them is committed).
pub fn large() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("eplan/depth-10000.xml", eplan_depth_10000()),
        (
            "eplan/element-of-200000-attributes.xml",
            eplan_element_of_200000_attributes(),
        ),
        (
            "eplan/parts-pins-over-the-budget.xml",
            eplan_parts_pins_over_the_budget(),
        ),
        ("ewz/65536-entries.ewz", ewz_65536_entries()),
        ("jsonl/line-over-16-mib.jsonl", jsonl_line_over_16_mib()),
        (
            "qet/attributes-over-the-limit.qet",
            qet_attributes_over_the_limit(),
        ),
    ]
}

/// The package relationships of a workbook: `xl/workbook.xml` is its document.
const RELS: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>
"#;

/// The package relationships behind an external entity that names a file of the reading host.
const EXTERNAL_ENTITY_RELS: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE Relationships [<!ENTITY e SYSTEM "file:///etc/passwd">]>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="&e;"/></Relationships>
"#;

/// The package relationships behind nine levels of entities, each ten of the one before (10^9 bytes expanded).
fn billion_laughs_rels() -> Vec<u8> {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE Relationships [\n");
    out.push_str(" <!ENTITY a \"aaaaaaaaaa\">\n");
    for (before, name) in "abcdefgh".chars().zip("bcdefghi".chars()) {
        out.push_str(&format!(
            " <!ENTITY {name} \"{}\">\n",
            format!("&{before};").repeat(10)
        ));
    }
    out.push_str(
        "]>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship \
         Id=\"rId1\" Type=\"&i;\" Target=\"xl/workbook.xml\"/></Relationships>\n",
    );
    out.into_bytes()
}

/// A workbook of one entry, `_rels/.rels` with `rels`.
fn xlsx_with_rels(rels: &[u8]) -> Vec<u8> {
    write(&[("_rels/.rels", Method::Deflated, rels)]).expect("a workbook")
}

/// The archive `entries` with the name `placeholder` of one of them replaced by `name` of the same length in its local
/// and central headers (the writer refuses such a name itself).
fn renamed(entries: &[(&str, Method, &[u8])], placeholder: &str, name: &str) -> Vec<u8> {
    assert_eq!(placeholder.len(), name.len());
    let mut bytes = write(entries).expect("an archive");
    let mut at = 0;
    let mut replaced = 0;
    while let Some(i) = find(&bytes[at..], placeholder.as_bytes()) {
        let i = at + i;
        bytes[i..i + name.len()].copy_from_slice(name.as_bytes());
        at = i + name.len();
        replaced += 1;
    }
    assert_eq!(replaced, 2, "the name stands in one local and one central header");
    bytes
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// A workbook with its relationships and one entry named `name`.
fn xlsx_entry_named(name: &str) -> Vec<u8> {
    let placeholder = "a".repeat(name.len());
    renamed(
        &[
            ("_rels/.rels", Method::Deflated, RELS),
            (&placeholder, Method::Deflated, b"x"),
        ],
        &placeholder,
        name,
    )
}

/// A workbook whose relationships are 1 MiB of spaces that pack to about a thousandth: honestly declared, above the
/// 200 : 1 ratio.
fn xlsx_ratio_over_200() -> Vec<u8> {
    let mut rels = RELS.to_vec();
    rels.resize(1 << 20, b' ');
    xlsx_with_rels(&rels)
}

/// A compression bomb: the relationships declared as 4,096 bytes whose deflate stream unpacks to 10 MiB.
fn xlsx_zip_bomb() -> Vec<u8> {
    let mut rels = RELS.to_vec();
    rels.resize(10 << 20, b' ');
    let mut bytes = xlsx_with_rels(&rels);
    let name = b"_rels/.rels";
    let local = find(&bytes, name).expect("the local header") - 30;
    let after = local + 30 + name.len();
    let central = after + find(&bytes[after..], name).expect("the central header") - 46;
    let declared = 4096u32.to_le_bytes();
    bytes[local + 22..local + 26].copy_from_slice(&declared);
    bytes[central + 24..central + 28].copy_from_slice(&declared);
    bytes
}

/// An `.ewz` whose third entry is named `../x`.
fn ewz_parent_entry() -> Vec<u8> {
    renamed(
        &[
            ("mimetype", Method::Stored, ew_api::ewz::MIMETYPE.as_bytes()),
            ("manifest.json", Method::Deflated, b"{}"),
            ("aa/x", Method::Deflated, b"x"),
        ],
        "aa/x",
        "../x",
    )
}

/// An `.ewz` of 65,536 empty stored entries in the writer's fixed fields, one more than its 16-bit counts hold: the end
/// record says 65,535 (`0xffff`, what a writer without ZIP64 leaves there).
fn ewz_65536_entries() -> Vec<u8> {
    fn put16(out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    fn put32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for i in 0..65_536u32 {
        let name = if i == 0 {
            "mimetype".to_owned()
        } else {
            format!("e/{i:05}")
        };
        let data: &[u8] = if i == 0 {
            ew_api::ewz::MIMETYPE.as_bytes()
        } else {
            b""
        };
        let len = u32::try_from(data.len()).expect("a short entry");
        let offset = u32::try_from(out.len()).expect("under 4 GiB");
        let name_len = u16::try_from(name.len()).expect("a short name");
        put32(&mut out, 0x0403_4b50);
        for v in [20, 0x0800, 0, 0, 0x0021] {
            put16(&mut out, v);
        }
        put32(&mut out, crc32(data));
        put32(&mut out, len);
        put32(&mut out, len);
        put16(&mut out, name_len);
        put16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        put32(&mut central, 0x0201_4b50);
        for v in [0x0314, 20, 0x0800, 0, 0, 0x0021] {
            put16(&mut central, v);
        }
        put32(&mut central, crc32(data));
        put32(&mut central, len);
        put32(&mut central, len);
        for v in [name_len, 0, 0, 0, 0] {
            put16(&mut central, v);
        }
        put32(&mut central, 0o100_644 << 16);
        put32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = u32::try_from(out.len()).expect("under 4 GiB");
    let cd_size = u32::try_from(central.len()).expect("under 4 GiB");
    out.extend_from_slice(&central);
    put32(&mut out, 0x0605_4b50);
    for v in [0, 0, 0xffff, 0xffff] {
        put16(&mut out, v);
    }
    put32(&mut out, cd_size);
    put32(&mut out, cd_offset);
    put16(&mut out, 0);
    out
}

/// A PNG whose header declares 65,536 × 32,768 pixels (2^31), with an empty image stream.
fn png_2_31_pixels() -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&u32::try_from(data.len()).expect("a short chunk").to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&65_536u32.to_be_bytes());
    ihdr.extend_from_slice(&32_768u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    // A zlib stream of no bytes: the header, one empty final stored block, the Adler-32 of nothing.
    chunk(
        &mut out,
        b"IDAT",
        &[0x78, 0x01, 0x01, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01],
    );
    chunk(&mut out, b"IEND", &[]);
    out
}

/// A page line one byte over the loader's 16 MiB line limit.
fn jsonl_line_over_16_mib() -> Vec<u8> {
    let head = br#"{"$k":"page","x":""#;
    let tail = b"\"}\n";
    let mut out = head.to_vec();
    out.resize((16 << 20) + 1 - tail.len() + 1, b'a');
    out.extend_from_slice(tail);
    out
}

/// An EPLAN parts document of one part with one connection.
const EPLAN_PARTS: &[u8] =
    br#"<partsmanagement><part><functiontemplate connectionDesignation="1"/></part></partsmanagement>"#;

/// An EDZ whose parts document is named `../x.xml`.
fn eplan_entry_parent() -> Vec<u8> {
    renamed(
        &[("aaaaaaaa", Method::Deflated, EPLAN_PARTS)],
        "aaaaaaaa",
        "../x.xml",
    )
}

/// An EDZ whose parts document carries 1 MiB of spaces that pack to about a thousandth: honestly declared, above the
/// 200 : 1 ratio.
fn eplan_ratio_over_200() -> Vec<u8> {
    let mut parts = EPLAN_PARTS.to_vec();
    parts.resize(1 << 20, b' ');
    write(&[("parts.xml", Method::Deflated, &parts)]).expect("an archive")
}

/// A 7z archive of the packed bytes `packed` and the plain header `header` behind them, its start header's CRC right.
fn seven_zip(packed: &[u8], header: &[u8]) -> Vec<u8> {
    let mut start = Vec::new();
    start.extend((packed.len() as u64).to_le_bytes());
    start.extend((header.len() as u64).to_le_bytes());
    start.extend(crc32(header).to_le_bytes());
    let mut out = b"7z\xbc\xaf\x27\x1c\x00\x04".to_vec();
    out.extend(crc32(&start).to_le_bytes());
    out.extend(start);
    out.extend(packed);
    out.extend(header);
    out
}

/// A 7z archive of one LZMA2 folder over 16 bytes that declares a 4 GiB dictionary (property 40): the decoder would
/// allocate it.
fn seven_zip_dictionary_4_gib() -> Vec<u8> {
    let header = [
        0x01, 0x04, 0x06, 0x00, 0x01, 0x09, 0x10, 0x00, 0x07, 0x0b, 0x01, 0x00, 0x01, 0x21, 0x21, 0x01, 40,
        0x0c, 0x10, 0x00, 0x00, 0x00,
    ];
    seven_zip(&[0; 16], &header)
}

/// A 7z archive whose files declare 5,000 entries (the NUMBER `93 88`), over the 4,096 of the reader.
fn seven_zip_entries_over_the_limit() -> Vec<u8> {
    seven_zip(&[], &[0x01, 0x05, 0x93, 0x88, 0x00, 0x00])
}

/// An EPLAN XML nested 10,000 deep (SM25#1: it overflowed the stack before the depth was checked).
fn eplan_depth_10000() -> Vec<u8> {
    format!("{}{}", "<a>".repeat(10_000), "</a>".repeat(10_000)).into_bytes()
}

/// An EPLAN XML whose one element carries 200,000 attributes, within the document's 200,000 `=`, the last repeating
/// the first (M25-p4#3): the parser's duplicate check is quadratic in an element's attributes, so it worked through
/// all of them before it refused the repeat.
fn eplan_element_of_200000_attributes() -> Vec<u8> {
    let mut out = String::from("<partsmanagement");
    for i in 0..199_999 {
        out.push_str(&format!(" a{i}=''"));
    }
    out.push_str(" a0=''/>");
    out.into_bytes()
}

/// A parts document of 17 parts of 4,000 connections each (`a¶`, 3 bytes): every part within the 4,096 of one
/// symbol, all of them together over the 65,536 graphics and connection points of one read (M25-p2#2).
fn eplan_parts_pins_over_the_budget() -> Vec<u8> {
    let part = format!(
        "<part><functiontemplate connectionDesignation=\"{}\"/></part>",
        "a\u{b6}".repeat(4_000)
    );
    format!("<partsmanagement>{}</partsmanagement>", part.repeat(17)).into_bytes()
}

/// A project whose first diagram carries 8,193 attributes, one over the per-element limit.
fn qet_attributes_over_the_limit() -> Vec<u8> {
    let mut out = String::from("<project version=\"0.100\" title=\"T\"><diagram");
    for i in 0..8_193 {
        out.push_str(&format!(" a{i}=\"v\""));
    }
    out.push_str("/></project>\n");
    out.into_bytes()
}
