// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The identity of an element definition: the SHA-256 of its canonical XML (doc 14 §2.3 "QET element
//! definition: content hash of the canonical definition XML", §4.3).
//!
//! A definition is identified by its content, never by its `embed://` path: the same element embedded
//! under two paths, re-indented by another QET version or with its attributes in another order is one
//! definition; any change of content is another.
//!
//! # Canonicalisation
//!
//! The `<definition>` element and its subtree, re-serialised from the parsed tree (so entity and character
//! references are already expanded and attribute values normalised by the XML parser):
//!
//! 1. **Elements** as `<name attrs>children</name>`, always with an explicit end tag; the name is the local
//!    name (QET uses no namespaces; a prefix is dropped).
//! 2. **Attributes** sorted by local name (byte-wise), each as ` name="value"`; in values `&`, `<`, `"`, tab,
//!    LF and CR are written as `&amp;`, `&lt;`, `&quot;`, `&#9;`, `&#10;`, `&#13;`.
//! 3. **Text** with leading and trailing XML white space (space, tab, LF, CR) removed and every inner run
//!    of it collapsed to one space; a text that becomes empty is dropped; `&`, `<`, `>` escaped. Adjacent
//!    text nodes (text around a dropped comment) are joined before trimming.
//! 4. **Dropped:** comments, processing instructions, the XML declaration, the DOCTYPE (refused anyway) and
//!    everything outside the `<definition>` element.
//!
//! The result is UTF-8 without a trailing newline. The digest is SHA-256 over those bytes, and the identity
//! key is `qet:eldef:<64 lower-case hex digits>` ([`identity_key`]).

use std::fmt::Write as _;

use roxmltree::Node;
use sha2::{Digest, Sha256};

/// The canonical XML of `element` and its subtree, by the rules of the module documentation.
///
/// ```
/// let a = roxmltree::Document::parse("<d b=\"2\"  a='1'>\n  <x/> <!-- c --> t \n u <y></y></d>").unwrap();
/// let b = roxmltree::Document::parse("<d a=\"1\" b=\"2\"><x></x>t u<y/></d>").unwrap();
/// let ca = ew_import_qet::canon::canonical_xml(a.root_element());
/// assert_eq!(ca, "<d a=\"1\" b=\"2\"><x></x>t u<y></y></d>");
/// assert_eq!(ca, ew_import_qet::canon::canonical_xml(b.root_element()));
/// ```
pub fn canonical_xml(element: Node<'_, '_>) -> String {
    let mut out = String::new();
    write_element(element, &mut out);
    out
}

/// SHA-256 of [`canonical_xml`].
pub fn identity_sha256(element: Node<'_, '_>) -> [u8; 32] {
    Sha256::digest(canonical_xml(element).as_bytes()).into()
}

/// The identity key `qet:eldef:<64 lower-case hex digits>` of a definition digest.
pub fn identity_key(digest: &[u8; 32]) -> String {
    let mut key = String::from("qet:eldef:");
    for b in digest {
        let _ = write!(key, "{b:02x}");
    }
    key
}

fn is_xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

fn write_element(node: Node<'_, '_>, out: &mut String) {
    let name = node.tag_name().name();
    out.push('<');
    out.push_str(name);
    let mut attrs: Vec<(&str, &str)> = node.attributes().map(|a| (a.name(), a.value())).collect();
    attrs.sort_unstable();
    for (k, v) in attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        for c in v.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '"' => out.push_str("&quot;"),
                '\t' => out.push_str("&#9;"),
                '\n' => out.push_str("&#10;"),
                '\r' => out.push_str("&#13;"),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    out.push('>');
    let mut text = String::new();
    for child in node.children() {
        if child.is_element() {
            flush_text(&mut text, out);
            write_element(child, out);
        } else if child.is_text() {
            text.push_str(child.text().unwrap_or(""));
        }
        // Comments and processing instructions are dropped; text around them joins.
    }
    flush_text(&mut text, out);
    out.push_str("</");
    out.push_str(name);
    out.push('>');
}

/// Writes the pending text, trimmed and with white-space runs collapsed, and clears it.
fn flush_text(text: &mut String, out: &mut String) {
    let mut first = true;
    for word in text.split(is_xml_space).filter(|w| !w.is_empty()) {
        if !first {
            out.push(' ');
        }
        first = false;
        for c in word.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                c => out.push(c),
            }
        }
    }
    text.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(xml: &str) -> String {
        canonical_xml(roxmltree::Document::parse(xml).unwrap().root_element())
    }

    #[test]
    fn escapes_and_entities() {
        assert_eq!(
            canon("<a v=\"x&amp;&lt;&quot;&#9;&#10;y\">1 &lt; 2 &amp;&gt; 3</a>"),
            "<a v=\"x&amp;&lt;&quot;&#9;&#10;y\">1 &lt; 2 &amp;&gt; 3</a>"
        );
        // A literal newline in an attribute is normalised to a space by the XML parser.
        assert_eq!(canon("<a v=\"x\ny\"/>"), "<a v=\"x y\"></a>");
    }

    #[test]
    fn insensitive_to_layout_and_order() {
        let a = "<?xml version=\"1.0\"?>\n<!-- licence -->\n<definition type=\"element\" version=\"0.80\">\n  <description>\n    <line x1=\"0\" y1=\"0\"/>\n  </description>\n</definition>\n";
        let b = "<definition version=\"0.80\" type=\"element\"><description><line y1=\"0\" x1=\"0\"></line></description></definition>";
        assert_eq!(canon(a), canon(b));
        let da = identity_sha256(roxmltree::Document::parse(a).unwrap().root_element());
        let db = identity_sha256(roxmltree::Document::parse(b).unwrap().root_element());
        assert_eq!(da, db);
        // A changed value is another definition.
        let c = b.replace("y1=\"0\"", "y1=\"10\"");
        let dc = identity_sha256(roxmltree::Document::parse(&c).unwrap().root_element());
        assert_ne!(da, dc);
    }

    #[test]
    fn key_format() {
        let key = identity_key(&[0xab; 32]);
        assert_eq!(key.len(), "qet:eldef:".len() + 64);
        assert!(key.starts_with("qet:eldef:abab"));
    }

    /// Fixed vector: the canonical bytes and digest of a small definition never change (they are the
    /// identity of imported symbols). Digest computed independently with Python's hashlib over the bytes.
    #[test]
    fn fixed_vector() {
        let xml = "<definition type=\"element\"><description><terminal x=\"0\" y=\"-20\" orientation=\"n\"/></description></definition>";
        let doc = roxmltree::Document::parse(xml).unwrap();
        assert_eq!(
            canonical_xml(doc.root_element()),
            "<definition type=\"element\"><description><terminal orientation=\"n\" x=\"0\" y=\"-20\"></terminal></description></definition>"
        );
        assert_eq!(identity_key(&identity_sha256(doc.root_element())), FIXED_KEY);
    }

    const FIXED_KEY: &str = "qet:eldef:d91b900237f15a540e37fe312aa95bac3e770abc55712316b09ff9ca1463bd19";
}
