// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests of the importer API data types.

use ew_ids::{Id, Uuid};
use ew_interop_api::{
    Code, DefectClass, Detection, ImportBatch, ImportContext, ImportError, ImportInput, ImportReport,
    Importer, InputNameError, Limits, ReportEntry, Severity, SourceInfo, SourceLocation, check_file_name,
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
fn defect_class_display() {
    assert_eq!(DefectClass(1).to_string(), "D-01");
    assert_eq!(DefectClass(29).to_string(), "D-29");
    assert_eq!(DefectClass(100).to_string(), "D-100");
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

#[test]
fn context_names_the_project() {
    let project = Id::from_uuid(Uuid::from_u128(0x0192_f3a4_5b6c_7d8e_8f01_2345_6789_ab00));
    let ctx = ImportContext::new(project);
    assert_eq!(ctx.project, project);
    assert_eq!(
        (ctx.lineage, ctx.as_of, ctx.limits),
        (None, None, Limits::default())
    );
}

struct Tiny;

impl Importer for Tiny {
    fn id(&self) -> &'static str {
        "tiny"
    }

    fn version(&self) -> &'static str {
        "1"
    }

    fn detect(&self, _head: &[u8], name: &str) -> Detection {
        if name.ends_with(".x") {
            Detection::Maybe(10)
        } else {
            Detection::No
        }
    }

    fn import(&self, input: &ImportInput, _ctx: &mut ImportContext) -> Result<ImportBatch, ImportError> {
        let source = SourceInfo {
            name: input.name().to_owned(),
            ..SourceInfo::default()
        };
        Ok(ImportBatch {
            records: vec![],
            lib: vec![],
            report: ImportReport::new("tiny", "1", source),
        })
    }
}

#[test]
fn importer_is_object_safe() {
    let importer: &dyn Importer = &Tiny;
    assert_eq!(importer.id(), "tiny");
    assert_eq!(importer.detect(b"", "a.x"), Detection::Maybe(10));
    assert_eq!(importer.detect(b"", "a.y"), Detection::No);
    let input = ImportInput::new("a.x", b"data".to_vec()).unwrap();
    let batch = importer
        .import(&input, &mut ImportContext::new(Id::from_uuid(Uuid::nil())))
        .unwrap();
    assert!(batch.records.is_empty() && batch.lib.is_empty());
    assert_eq!(batch.report.importer, "tiny");
    assert!(Detection::No < Detection::Maybe(0));
    assert!(Detection::Maybe(0) < Detection::Maybe(5));
    assert!(Detection::Maybe(5) < Detection::Certain);
}
