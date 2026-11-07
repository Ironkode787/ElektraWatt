// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EPLAN PXF 1.23 macro drawing subset, verified against manufacturer exports.
//! Unknown drawable records are reported, never interpreted as successful geometry.

use crate::{Budget, ImportedSymbol, Pin};
use roxmltree::{Document, Node, ParsingOptions};
use serde_json::json;

/// The deepest element nesting read: the tree check below allows 65 ancestors with the document root.
const MAX_DEPTH: usize = 64;
/// The most attributes of one element, counted before parsing as well as after it: the parser's duplicate-attribute
/// check is quadratic in the attributes of one element (M25-p4#3).
const MAX_ATTRIBUTES: usize = 256;

/// The index just past the first `pattern` at or after `from`, else the end of `b`.
fn after(b: &[u8], from: usize, pattern: &[u8]) -> usize {
    b.get(from..)
        .and_then(|rest| rest.windows(pattern.len()).position(|w| w == pattern))
        .map_or(b.len(), |at| from + at + pattern.len())
}

/// Whether the element nesting of `text` exceeds `limit`, or one element has more than `attributes` attributes (an
/// `=` outside its quoted values), by a flat scan before the parser reads it (SM25#1, M25-p4#3): comments, CDATA
/// sections, processing instructions and declarations are skipped, and a quoted attribute value may hold `>`, `/` and
/// `=`. A malformed input gives some answer; the parser reports it.
fn over_limits(text: &str, limit: usize, attributes: usize) -> bool {
    let b = text.as_bytes();
    let (mut at, mut depth) = (0, 0usize);
    while let Some(open) = b.get(at..).and_then(|rest| rest.iter().position(|&c| c == b'<')) {
        at += open + 1;
        let rest = &b[at..];
        if rest.starts_with(b"!--") {
            at = after(b, at, b"-->");
        } else if rest.starts_with(b"![CDATA[") {
            at = after(b, at, b"]]>");
        } else if rest.starts_with(b"?") {
            at = after(b, at, b"?>");
        } else if rest.starts_with(b"!") {
            at = after(b, at, b">");
        } else if rest.starts_with(b"/") {
            depth = depth.saturating_sub(1);
            at = after(b, at, b">");
        } else {
            let mut quote = None;
            let mut end = at;
            let mut held = 0usize;
            while let Some(&c) = b.get(end) {
                match quote {
                    Some(q) if c == q => quote = None,
                    Some(_) => {}
                    None if c == b'"' || c == b'\'' => quote = Some(c),
                    None if c == b'>' => break,
                    None if c == b'=' => held += 1,
                    None => {}
                }
                end += 1;
            }
            if held > attributes {
                return true;
            }
            if !(end > at && b[end - 1] == b'/' && b.get(end) == Some(&b'>')) {
                depth += 1;
                if depth > limit {
                    return true;
                }
            }
            at = end + 1;
        }
    }
    false
}

pub(crate) fn xml(text: &str) -> Result<Document<'_>, String> {
    if text.len() as u64 > crate::MAX_FILE
        || text.bytes().filter(|b| *b == b'<').count() > 100_000
        || text.bytes().filter(|b| *b == b'=').count() > 200_000
        || over_limits(text, MAX_DEPTH, MAX_ATTRIBUTES)
    {
        return Err("eplan-input-limit".into());
    }
    let doc = Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(|_| "eplan-xml-invalid")?;
    for n in doc.descendants() {
        if n.ancestors().take(66).count() > 65
            || n.attributes().len() > MAX_ATTRIBUTES
            || n.attributes().any(|a| a.value().len() > 65536)
        {
            return Err("eplan-input-limit".into());
        }
    }
    Ok(doc)
}

fn number(s: &str) -> Result<f64, String> {
    let n = s.parse::<f64>().map_err(|_| "eplan-geometry-invalid")?;
    if !n.is_finite() || n.abs() > 1_000_000.0 {
        return Err("eplan-geometry-invalid".into());
    }
    Ok(n)
}
fn pair(s: &str) -> Result<[f64; 2], String> {
    let mut p = s.split('/');
    Ok([
        number(p.next().ok_or("eplan-geometry-invalid")?)?,
        number(p.next().ok_or("eplan-geometry-invalid")?)?,
    ])
}
fn point(n: Node<'_, '_>, a: &str, origin: [f64; 2]) -> Result<[i64; 2], String> {
    let p = pair(n.attribute(a).ok_or("eplan-geometry-invalid")?)?;
    Ok([
        ((p[0] - origin[0]) * 1_000_000.0).round() as i64,
        ((origin[1] - p[1]) * 1_000_000.0).round() as i64,
    ])
}
fn plain(s: &str) -> String {
    let choices: Vec<_> = s.split(';').filter(|p| !p.is_empty()).collect();
    choices
        .iter()
        .find(|p| p.starts_with("en_"))
        .or(choices.first())
        .map(|p| p.split_once('@').map_or(*p, |(_, v)| v).to_owned())
        .unwrap_or_default()
}

pub(crate) fn read(
    doc: &Document<'_>,
    source: &str,
    budget: &mut Budget,
) -> Result<Vec<ImportedSymbol>, String> {
    let root = doc.root_element();
    if root.attribute("PxfVersion") != Some("1.23") {
        return Err("eplan-pxf-version".into());
    }
    let mut out = Vec::new();
    for variant in root.children().filter(|n| n.has_tag_name("MacroVariant")) {
        let origin = pair(variant.attribute("ReferencePoint").unwrap_or("0/0"))?;
        let mut symbol = ImportedSymbol {
            source: source.into(),
            name: format!(
                "{} · {}",
                root.attribute("Name").unwrap_or("Macro"),
                variant.attribute("VariantId").unwrap_or("0")
            ),
            graphics: Vec::new(),
            pins: Vec::new(),
            notes: Vec::new(),
        };
        let stroke = json!({"pen":"p026"});
        let mut incomplete = false;
        for n in variant.descendants().filter(Node::is_element) {
            let graphic = match n.tag_name().name() {
                "O31" => Some(
                    json!({"line":{"from":point(n,"A531",origin)?,"to":point(n,"A532",origin)?,"stroke":stroke}}),
                ),
                "O89" => {
                    let a = point(n, "A1651", origin)?;
                    let b = point(n, "A1652", origin)?;
                    Some(
                        json!({"rect":{"rect":[[a[0].min(b[0]),a[1].min(b[1])],[a[0].max(b[0]),a[1].max(b[1])]],"stroke":stroke}}),
                    )
                }
                "O34" => {
                    let mut points = Vec::new();
                    for p in n.attribute("A621").ok_or("eplan-geometry-invalid")?.split(';') {
                        let p = pair(p)?;
                        points.push([
                            ((p[0] - origin[0]) * 1_000_000.0).round() as i64,
                            ((origin[1] - p[1]) * 1_000_000.0).round() as i64,
                        ]);
                    }
                    Some(json!({"polyline":{"points":points,"stroke":stroke}}))
                }
                "O32" => {
                    let c = point(n, "A555", origin)?;
                    let radii = pair(n.attribute("A556").ok_or("eplan-geometry-invalid")?)?;
                    let r = (radii[0].hypot(radii[1]) * 1_000_000.0).round() as i64;
                    let start = number(n.attribute("A553").unwrap_or("0"))?;
                    let end = number(n.attribute("A554").unwrap_or("0"))?;
                    if (end - start - std::f64::consts::TAU).abs() < 0.000001 {
                        Some(json!({"circle":{"centre":c,"r":r,"stroke":stroke}}))
                    } else {
                        incomplete = true;
                        None
                    }
                }
                "O30" => Some(
                    json!({"text":{"pos":point(n,"A501",origin)?,"text":{"en":plain(n.attribute("A511").unwrap_or(""))},"align":"top_left"}}),
                ),
                "O17" => {
                    if let Some(position) = n.children().find(|n| n.has_tag_name("S40x1201")) {
                        let name = n.attribute("A185").unwrap_or("");
                        if !name.is_empty() {
                            budget.item(symbol.pins.len())?;
                            symbol.pins.push(Pin {
                                designation: name.into(),
                                position: point(position, "A762", origin)?,
                                direction: None,
                            });
                        }
                    }
                    incomplete = true;
                    None
                }
                "O91" => {
                    incomplete = true;
                    None
                }
                name if name.starts_with('O') => {
                    incomplete = true;
                    None
                }
                _ => None,
            };
            if let Some(g) = graphic {
                budget.item(symbol.graphics.len())?;
                symbol.graphics.push(g);
            }
        }
        if incomplete {
            symbol.notes.push("eplan-macro-incomplete".into());
        }
        symbol.notes.push("eplan-styles-review".into());
        if !symbol.graphics.is_empty() || !symbol.pins.is_empty() {
            budget.symbol()?;
            out.push(symbol);
        }
    }
    if out.is_empty() {
        return Err("eplan-no-symbols".into());
    }
    Ok(out)
}
