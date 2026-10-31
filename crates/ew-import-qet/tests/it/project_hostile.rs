// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hostile `.qet` input through the public project importer: structural checks only (limits refuse the file,
//! item problems become findings, nothing panics). No wall-clock bounds (CLAUDE.md "Testing policy").

use ew_import_qet::QetProjectImporter;
use ew_import_qet::ew_interop_api::{
    Code, ImportBatch, ImportContext, ImportError, ImportInput, Importer, Limits, ReportEntry,
};
use ew_import_qet::project::new_project_context;

/// Imports `bytes` as `x.qet` under `ctx`.
fn import_with(bytes: Vec<u8>, mut ctx: ImportContext) -> Result<ImportBatch, ImportError> {
    let input = ImportInput::new("x.qet", bytes).unwrap();
    QetProjectImporter::new().import(&input, &mut ctx)
}

/// Imports `xml` under the default project context with `limits`.
fn import_limited(xml: &str, limits: Limits) -> Result<ImportBatch, ImportError> {
    let mut ctx = new_project_context();
    ctx.limits = limits;
    import_with(xml.as_bytes().to_vec(), ctx)
}

/// Imports `xml` under the default project context and limits.
fn import(xml: &str) -> Result<ImportBatch, ImportError> {
    import_with(xml.as_bytes().to_vec(), new_project_context())
}

fn with_code(batch: &ImportBatch, code: Code) -> Vec<&ReportEntry> {
    batch.report.entries.iter().filter(|e| e.code == code).collect()
}

#[test]
fn nesting_deeper_than_the_depth_limit_is_refused() {
    let mut xml = String::from("<project><collection>");
    for _ in 0..100 {
        xml.push_str("<category name=\"c\">");
    }
    for _ in 0..100 {
        xml.push_str("</category>");
    }
    xml.push_str("</collection></project>");
    let limits = Limits {
        max_xml_depth: 64,
        ..Limits::default()
    };
    let r = import_limited(&xml, limits);
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

#[test]
fn nesting_within_the_depth_limit_is_read() {
    let mut xml = String::from("<project><collection>");
    for _ in 0..40 {
        xml.push_str("<category name=\"c\">");
    }
    for _ in 0..40 {
        xml.push_str("</category>");
    }
    xml.push_str("</collection></project>");
    let limits = Limits {
        max_xml_depth: 64,
        ..Limits::default()
    };
    assert!(import_limited(&xml, limits).is_ok());
}

#[test]
fn input_larger_than_the_size_limit_is_refused() {
    let mut xml = String::from("<project>");
    xml.push_str(&" ".repeat(2048));
    xml.push_str("</project>");
    let limits = Limits {
        max_input_bytes: 1024,
        ..Limits::default()
    };
    let r = import_limited(&xml, limits);
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

#[test]
fn more_nodes_than_the_node_limit_are_refused() {
    let mut xml = String::from("<project>");
    for _ in 0..2000 {
        xml.push_str("<diagram/>");
    }
    xml.push_str("</project>");
    let limits = Limits {
        max_xml_nodes: 1000,
        ..Limits::default()
    };
    let r = import_limited(&xml, limits);
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

#[test]
fn too_many_attributes_on_one_element_are_refused() {
    let mut xml = String::from("<project><diagram");
    for i in 0..20 {
        xml.push_str(&format!(" a{i}=\"v\""));
    }
    xml.push_str("/></project>");
    let limits = Limits {
        max_xml_attributes_per_element: 10,
        ..Limits::default()
    };
    let r = import_limited(&xml, limits);
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

#[test]
fn a_text_node_over_the_text_limit_is_refused() {
    let xml = format!(
        "<project><properties><property name=\"p\">{}</property></properties></project>",
        "t".repeat(200)
    );
    let limits = Limits {
        max_text_bytes: 100,
        ..Limits::default()
    };
    let r = import_limited(&xml, limits);
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

#[test]
fn a_doctype_with_an_entity_is_malformed() {
    let xml = "<?xml version=\"1.0\"?><!DOCTYPE project [<!ENTITY a \"aaaa\">]><project title=\"&a;\"/>";
    let r = import(xml);
    assert!(matches!(r, Err(ImportError::Malformed { .. })), "{r:?}");
}

#[test]
fn input_that_is_not_utf8_is_unsupported() {
    let mut bytes = b"<project title=\"".to_vec();
    bytes.extend_from_slice(&[0xFF, 0xFE, 0xFD]);
    bytes.extend_from_slice(b"\"/>");
    let r = import_with(bytes, new_project_context());
    assert!(matches!(r, Err(ImportError::Unsupported { .. })), "{r:?}");
}

#[test]
fn a_foreign_root_is_unsupported() {
    let r = import("<definition type=\"element\"/>");
    assert!(matches!(r, Err(ImportError::Unsupported { .. })), "{r:?}");
}

#[test]
fn five_thousand_empty_diagrams_import() {
    let mut xml = String::from("<project>");
    for i in 0..5000 {
        xml.push_str(&format!("<diagram title=\"d{i}\" order=\"{i}\"/>"));
    }
    xml.push_str("</project>");
    let r = import(&xml);
    assert!(r.is_ok(), "{:?}", r.err());
}

#[test]
fn item_garbage_becomes_findings() {
    let xml = r#"<project version="0.100.0">
<diagram folio="1">
<elements>
<element type="embed://x/y.elmt" x="NaN" y="0"/>
<element type="embed://x/y.elmt" x="1" y="2" orientation="9"/>
</elements>
<conductors>
<conductor element1="zz" terminal1="!!" element2="{" terminal2="" num="1">
<segments>
<segment orientation="horizontal" length="abc"/>
</segments>
</conductor>
</conductors>
<unknown_thing/>
</diagram>
</project>"#;
    let batch = import(xml).expect("lenient import");
    let malformed = with_code(&batch, Code::MALFORMED_ITEM);
    // x="NaN" is an error, the orientation and the segment length are warnings.
    assert!(
        malformed.iter().any(|e| e.message == "element skipped: bad x/y"),
        "{malformed:?}"
    );
    assert!(
        malformed
            .iter()
            .any(|e| e.message.starts_with("invalid orientation")),
        "{malformed:?}"
    );
    assert!(
        malformed
            .iter()
            .any(|e| e.message == "segment dropped: bad length"),
        "{malformed:?}"
    );
    let dropped = with_code(&batch, Code::DROPPED_BY_DESIGN);
    assert!(
        dropped
            .iter()
            .any(|e| e.message == "unsupported unknown_thing dropped"),
        "{dropped:?}"
    );
}
