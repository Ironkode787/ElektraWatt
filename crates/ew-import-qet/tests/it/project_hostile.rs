// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hostile `.qet` input through the public project importer: structural checks only (limits refuse the file,
//! item problems become findings, nothing panics). No wall-clock bounds (CLAUDE.md "Testing policy").

use ew_import_qet::QetProjectImporter;
use ew_import_qet::ew_interop_api::{
    Code, ImportBatch, ImportContext, ImportError, ImportInput, Limits, ReportEntry,
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
fn input_over_a_limit_is_refused() {
    let nested = |n: usize| {
        let mut xml = String::from("<project><collection>");
        for _ in 0..n {
            xml.push_str("<category name=\"c\">");
        }
        for _ in 0..n {
            xml.push_str("</category>");
        }
        xml.push_str("</collection></project>");
        xml
    };
    let diagrams = format!("<project>{}</project>", "<diagram/>".repeat(2000));
    let mut attributes = String::from("<project><diagram");
    for i in 0..20 {
        attributes.push_str(&format!(" a{i}=\"v\""));
    }
    attributes.push_str("/></project>");
    let cases = [
        (
            "depth",
            nested(100),
            Limits {
                max_xml_depth: 64,
                ..Limits::default()
            },
        ),
        (
            "input size",
            format!("<project>{}</project>", " ".repeat(2048)),
            Limits {
                max_input_bytes: 1024,
                ..Limits::default()
            },
        ),
        (
            "nodes",
            diagrams,
            Limits {
                max_xml_nodes: 1000,
                ..Limits::default()
            },
        ),
        (
            "attributes per element",
            attributes,
            Limits {
                max_xml_attributes_per_element: 10,
                ..Limits::default()
            },
        ),
        (
            "text node",
            format!(
                "<project><properties><property name=\"p\">{}</property></properties></project>",
                "t".repeat(200)
            ),
            Limits {
                max_text_bytes: 100,
                ..Limits::default()
            },
        ),
    ];
    for (name, xml, limits) in cases {
        let r = import_limited(&xml, limits);
        assert!(
            matches!(r, Err(ImportError::LimitExceeded { .. })),
            "{name}: {r:?}"
        );
    }
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

/// WP-S3.09 IC-5 (D5 c): a `.qet` that declares an external entity is refused before any byte of it is resolved.
#[test]
fn an_external_entity_is_refused() {
    let xml = "<?xml version=\"1.0\"?><!DOCTYPE project [<!ENTITY e SYSTEM \"file:///etc/passwd\">]>\
               <project title=\"&e;\"/>";
    let err = import(xml).unwrap_err();
    assert_eq!(
        err.to_string(),
        "malformed input at line 1: a DOCTYPE is not accepted"
    );
}

/// WP-S3.09 IC-5 (D5 c): an `xi:include` is never followed: it is unknown content, reported and dropped, and the
/// rest of the folio imports.
#[test]
fn an_xinclude_is_reported_and_never_followed() {
    let xml = r#"<project xmlns:xi="http://www.w3.org/2001/XInclude">
<diagram folio="1" title="One">
<xi:include href="file:///etc/passwd" parse="text"/>
</diagram>
</project>"#;
    let batch = import(xml).unwrap();
    let dropped: Vec<&str> = with_code(&batch, Code::DROPPED_BY_DESIGN)
        .iter()
        .map(|e| e.message.as_str())
        .filter(|m| m.contains("include"))
        .collect();
    assert_eq!(dropped, ["unsupported include dropped"]);
    assert!(
        batch
            .records
            .iter()
            .any(|r| matches!(r, ew_import_qet::ew_interop_api::ew_schema::Record::Page(_)))
    );
}

/// A placed thumbnail element is dropped by design with its reason in words, no plan reference.
#[test]
fn a_thumbnail_element_is_dropped_in_words() {
    let xml = r#"<project version="0.100.0">
<collection><category name="import"><element name="t.elmt">
<definition type="element" version="0.5" hotspot_x="30" hotspot_y="20" width="60" height="40" link_type="thumbnail">
<names><name lang="en">Thumbnail</name></names>
<description><line x1="-20" y1="0" x2="20" y2="0" style="line-style:normal;line-weight:normal;filling:none;color:black"/></description>
</definition>
</element></category></collection>
<diagram folio="1">
<elements>
<element type="embed://import/t.elmt" x="100" y="100" uuid="{00000000-0000-4000-8000-000000000001}"/>
</elements>
</diagram>
</project>"#;
    let batch = import(xml).expect("lenient import");
    let dropped: Vec<&str> = with_code(&batch, Code::DROPPED_BY_DESIGN)
        .iter()
        .map(|e| e.message.as_str())
        .filter(|m| m.contains("thumbnail"))
        .collect();
    assert_eq!(
        dropped,
        ["thumbnail element is a graphic; not imported (no page-item records yet)"]
    );
}
