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
