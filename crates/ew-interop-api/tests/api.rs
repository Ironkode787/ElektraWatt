// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests of the importer API data types.

use ew_ids::Uuid;
use ew_interop_api::{
    Code, DefectClass, ImportError, ImportInput, ImportReport, InputNameError, Limits, RecordKey,
    ReportEntry, Severity, SourceInfo, SourceLocation, check_archive_name, check_file_name,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn code_display_and_parse() {
    assert_eq!(Code::imp(1).to_string(), "EW-IMP-001");
    assert_eq!(Code::BLACK_BOX.to_string(), "EW-IMP-119");
    for n in [1u16, 9, 10, 99, 100, 999] {
        let c = Code::imp(n);
        assert_eq!(c.to_string().parse::<Code>(), Ok(c));
        assert_eq!(c.number(), n);
    }
    for bad in [
        "EW-IMP-000",
        "EW-IMP-01",
        "EW-IMP-0001",
        "ew-imp-001",
        "EW-IMP-1a1",
        " EW-IMP-001",
        "EW-IMP-+01",
        "",
    ] {
        assert!(bad.parse::<Code>().is_err(), "{bad:?} must be rejected");
    }
    assert!(std::panic::catch_unwind(|| Code::imp(0)).is_err());
    assert!(std::panic::catch_unwind(|| Code::imp(1000)).is_err());
    let err = "x".parse::<Code>().unwrap_err();
    assert_eq!(err.to_string(), "not an import code EW-IMP-nnn");
}

#[test]
fn input_accessors_and_file_name_rule() {
    let input = ImportInput::new("x.elmt", b"abc".to_vec()).unwrap();
    assert_eq!(input.name(), "x.elmt");
    assert_eq!(&input.bytes()[..], b"abc");
    assert_eq!(
        hex(&input.sha256()),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(check_file_name(""), Err(InputNameError::Empty));
    for bad in ["a/b", "a\\b", "c:x", "a\0b"] {
        assert_eq!(check_file_name(bad), Err(InputNameError::NotAFileName), "{bad:?}");
    }
    assert_eq!(check_file_name("x.elmt"), Ok(()));
}

#[test]
fn input_new_hashes_and_validates_names() {
    let empty = ImportInput::new("a.elmt", b"".to_vec()).unwrap();
    assert_eq!(
        hex(&empty.sha256()),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let abc = ImportInput::new("a.elmt", b"abc".to_vec()).unwrap();
    assert_eq!(
        hex(&abc.sha256()),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(abc.name(), "a.elmt");
    assert_eq!(&abc.bytes()[..], b"abc");

    assert_eq!(
        ImportInput::new("", Vec::new()).unwrap_err(),
        InputNameError::Empty
    );
    for bad in ["dir/a.elmt", "dir\\a.elmt", "C:a.elmt", "a\0b"] {
        assert_eq!(
            ImportInput::new(bad, Vec::new()).unwrap_err(),
            InputNameError::NotAFileName,
            "{bad:?}"
        );
    }
    assert!(ImportInput::new("a b.elmt", Vec::new()).is_ok());
    assert_eq!(InputNameError::Empty.to_string(), "file name is empty");
    assert_eq!(
        InputNameError::NotAFileName.to_string(),
        "not a file name (contains a path separator, ':' or NUL)"
    );
}

#[test]
fn report_builders() {
    let source = SourceInfo {
        name: "x.elmt".into(),
        ..SourceInfo::default()
    };
    let mut report = ImportReport::new("qet", "0.1.0", source);
    assert_eq!(report.importer, "qet");
    assert_eq!(report.importer_version, "0.1.0");
    assert_eq!(report.source.name, "x.elmt");
    assert!(report.entries.is_empty());

    report.push(ReportEntry::new(Code::BLACK_BOX, Severity::Info, "generic"));
    assert!(!report.has_errors());

    let loc = SourceLocation {
        file: "x.elmt".into(),
        line: Some(3),
        ..SourceLocation::default()
    };
    let entry = ReportEntry::new(Code::MALFORMED_ITEM, Severity::Error, "bad node")
        .with_defect(DefectClass(29))
        .at(loc.clone());
    assert_eq!(entry.defect, Some(DefectClass(29)));
    assert_eq!(entry.location, Some(loc));
    assert_eq!(entry.message, "bad node");
    report.push(entry);
    assert!(report.has_errors());

    assert_eq!(report.with_code(Code::MALFORMED_ITEM).count(), 1);
    assert_eq!(report.with_code(Code::BLACK_BOX).count(), 1);
    assert_eq!(report.with_code(Code::LIMIT).count(), 0);
    let plain = &report.entries[0];
    assert_eq!(plain.defect, None);
    assert_eq!(plain.location, None);
}

#[test]
fn import_error_display() {
    let e = ImportError::LimitExceeded {
        what: "XML depth".into(),
        limit: 256,
    };
    assert_eq!(e.to_string(), "input exceeds the XML depth limit of 256");
    let e = ImportError::Malformed {
        message: "unclosed tag".into(),
        line: Some(7),
    };
    assert_eq!(e.to_string(), "malformed input at line 7: unclosed tag");
    let e = ImportError::Malformed {
        message: "no root".into(),
        line: None,
    };
    assert_eq!(e.to_string(), "malformed input: no root");
    let e = ImportError::Unsupported {
        message: "version 9".into(),
    };
    assert_eq!(e.to_string(), "unsupported input: version 9");
}

#[test]
fn limits_default() {
    let l = Limits::default();
    assert_eq!(l.max_xml_depth, 256);
    assert_eq!(l.max_attribute_bytes, 16 * 1024 * 1024);
    assert_eq!(l.max_input_bytes, 512 * 1024 * 1024);
    assert_eq!(l.max_xml_nodes, 1_000_000);
    assert_eq!(l.max_xml_attributes_per_element, 8_192);
    assert_eq!(l.max_xml_attributes, 1_048_576);
    assert_eq!(l.max_xml_attribute_work, 67_108_864);
    assert_eq!(l.max_text_bytes, 1_048_576);
}

#[test]
fn limits_element_definition_and_stricter() {
    let e = Limits::element_definition();
    assert_eq!(e.max_input_bytes, 16_777_216);
    assert_eq!(e.max_xml_attributes, 131_072);
    assert_eq!(e.max_xml_nodes, 262_144);
    assert_eq!(
        e,
        Limits {
            max_input_bytes: 16_777_216,
            max_xml_attributes: 131_072,
            max_xml_nodes: 262_144,
            ..Limits::default()
        }
    );
    // The stricter of each field, whichever side it comes from.
    let ctx = Limits {
        max_input_bytes: 1000,
        max_xml_nodes: 5_000_000,
        ..Limits::default()
    };
    let m = ctx.stricter(e);
    assert_eq!(m.max_input_bytes, 1000);
    // The preset's node and attribute caps win over the default's.
    assert_eq!(m.max_xml_nodes, 262_144);
    assert_eq!(m.max_xml_attributes, 131_072);
    assert_eq!(Limits::default().stricter(e), e);
}

/// WP-S3.09 D2: the archive, file-set and table members and their defaults (doc 14 §11).
#[test]
fn limits_default_archives_inputs_and_tables() {
    let l = Limits::default();
    assert_eq!(l.max_archive_entries, 100_000);
    assert_eq!(l.max_expanded_bytes, 2 * 1024 * 1024 * 1024);
    assert_eq!(l.max_entry_bytes, 512 * 1024 * 1024);
    assert_eq!(l.max_entry_ratio, 200);
    assert_eq!(l.max_input_files, 64);
    assert_eq!(l.max_table_rows, 1_000_000);
    assert_eq!(l.max_table_columns, 1_024);
}

/// IC-1: `stricter` takes the smaller of every member, whichever side it comes from.
#[test]
fn stricter_covers_every_member() {
    let small = Limits {
        max_input_bytes: 1,
        max_xml_depth: 2,
        max_xml_nodes: 3,
        max_attribute_bytes: 4,
        max_xml_attributes_per_element: 5,
        max_xml_attributes: 6,
        max_xml_attribute_work: 7,
        max_text_bytes: 8,
        max_archive_entries: 9,
        max_expanded_bytes: 10,
        max_entry_bytes: 11,
        max_entry_ratio: 12,
        max_input_files: 13,
        max_table_rows: 14,
        max_table_columns: 15,
    };
    assert_eq!(Limits::default().stricter(small), small);
    assert_eq!(small.stricter(Limits::default()), small);
}

/// IC-2: the archive-name rule, each refusal's whole text.
#[test]
fn archive_names_outside_the_archive_are_refused() {
    let cases = [
        ("", "the archive entry name is empty"),
        ("../x", "the archive entry name leads out of the archive (`..`)"),
        ("a/../b", "the archive entry name leads out of the archive (`..`)"),
        ("/x", "the archive entry name is an absolute path"),
        ("a\\b", "the archive entry name contains a backslash"),
        ("C:x", "the archive entry name contains ':' (a drive path)"),
        (
            "a\0b",
            "the archive entry name contains NUL or a control character",
        ),
        (
            "a\nb",
            "the archive entry name contains NUL or a control character",
        ),
    ];
    for (name, text) in cases {
        let err = check_archive_name(name).unwrap_err();
        assert_eq!(err.to_string(), text, "{name:?}");
    }
    for good in ["xl/workbook.xml", "[Content_Types].xml", "a..b/c", "dir/"] {
        assert_eq!(check_archive_name(good), Ok(()), "{good:?}");
    }
}

/// D1: an entry names the records it is about.
#[test]
fn report_entries_name_their_targets() {
    let id = Uuid::from_u128(7);
    let e = ReportEntry::new(Code::LIMIT, Severity::Info, "x").on(ew_schema::Kind::Device, id);
    assert_eq!(
        e.targets,
        vec![RecordKey {
            kind: ew_schema::Kind::Device,
            id
        }]
    );
    assert!(
        ReportEntry::new(Code::LIMIT, Severity::Info, "x")
            .targets
            .is_empty()
    );
}
