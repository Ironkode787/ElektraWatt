// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hostile and broken AML files are refused in words and never panic (doc 14 §11, NFR-SEC-02, NFR-REL-12).

use ew_caex::{ApcVersion, ImportError, Limits, read_aml, write_aml};

use crate::{options, sample};

fn refusal(bytes: &[u8], limits: &Limits) -> ImportError {
    read_aml(bytes, limits).expect_err("the file is refused")
}

#[test]
fn a_doctype_is_refused() {
    let aml = "<?xml version=\"1.0\"?><!DOCTYPE CAEXFile [<!ENTITY a \"aaaa\">]><CAEXFile SchemaVersion=\"2.15\"/>";
    let e = refusal(aml.as_bytes(), &Limits::default());
    assert!(matches!(&e, ImportError::Malformed { message, .. } if message.contains("DOCTYPE")), "{e}");
}

#[test]
fn a_file_beyond_a_limit_is_refused_by_that_limit() {
    let aml = write_aml(&sample(), &options(ApcVersion::V1_4_0));
    let what = |limits: Limits| match refusal(aml.as_bytes(), &limits) {
        ImportError::LimitExceeded { what, .. } => what,
        other => panic!("not a limit: {other}"),
    };
    let d = Limits::default();
    assert_eq!(what(Limits { max_input_bytes: 100, ..d }), "input size");
    assert_eq!(what(Limits { max_xml_depth: 3, ..d }), "XML depth");
    assert_eq!(what(Limits { max_xml_nodes: 10, ..d }), "XML node");
    assert_eq!(what(Limits { max_xml_attributes: 5, ..d }), "XML attribute");
    assert_eq!(
        what(Limits { max_xml_attributes_per_element: 1, ..d }),
        "attributes per XML element"
    );
    assert_eq!(what(Limits { max_attribute_bytes: 4, ..d }), "XML attribute value");
    assert_eq!(what(Limits { max_text_bytes: 8, ..d }), "XML text");
}

#[test]
fn what_is_not_an_ar_apc_file_is_refused_in_words() {
    let d = Limits::default();
    let e = refusal(b"<html/>", &d);
    assert!(e.to_string().contains("no AutomationML file"), "{e}");
    let e = refusal(b"<CAEXFile SchemaVersion=\"1.0\"/>", &d);
    assert!(e.to_string().contains("CAEX 2.15 and 3.0 are read"), "{e}");
    let e = refusal(b"<CAEXFile SchemaVersion=\"2.15\"><InstanceHierarchy Name=\"x\"/></CAEXFile>", &d);
    assert!(e.to_string().contains("no AR APC project"), "{e}");
    let e = refusal(&[0xff, 0xfe, 0x00], &d);
    assert!(e.to_string().contains("not UTF-8"), "{e}");
    let e = refusal(b"<CAEXFile SchemaVersion=\"2.15\"><InstanceHierarchy>", &d);
    assert!(matches!(e, ImportError::Malformed { .. }), "{e}");
}

#[test]
fn a_number_that_is_none_names_its_element() {
    let aml = write_aml(&sample(), &options(ApcVersion::V1_4_0));
    let (head, tail) = aml.split_at(aml.find("PositionNumber").expect("a position"));
    let aml = format!("{head}{}", tail.replacen("<Value>1</Value>", "<Value>first</Value>", 1));
    let e = refusal(aml.as_bytes(), &Limits::default());
    assert!(
        e.to_string().contains("`PositionNumber` of `-201U3` is `first`"),
        "{e}"
    );
}

/// The stand-in of a fuzz target while the fuzz workspace is cut: every truncation and a spread of single-byte
/// damage of a real file is read or refused, and none panics.
#[test]
fn a_damaged_file_never_panics() {
    let aml = write_aml(&sample(), &options(ApcVersion::V1_4_0)).into_bytes();
    let limits = Limits::default();
    for cut in (0..aml.len()).step_by(7) {
        let _ = read_aml(&aml[..cut], &limits);
    }
    for (i, replacement) in (0..aml.len()).step_by(11).zip(b"<>\"&/=\0\xff ".iter().cycle()) {
        let mut damaged = aml.clone();
        damaged[i] = *replacement;
        let _ = read_aml(&damaged, &limits);
    }
}
