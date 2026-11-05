// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EPLAN PXF 1.23 macro drawing subset, verified against manufacturer exports.
//! Unknown drawable records are reported, never interpreted as successful geometry.

use crate::{ImportedSymbol, Pin};
use roxmltree::{Document, Node, ParsingOptions};
use serde_json::json;

pub(crate) fn xml(text: &str) -> Result<Document<'_>, String> {
    if text.len() as u64 > crate::MAX_FILE
        || text.bytes().filter(|b| *b == b'<').count() > 100_000
        || text.bytes().filter(|b| *b == b'=').count() > 200_000
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
            || n.attributes().len() > 256
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

pub(crate) fn read(doc: &Document<'_>, source: &str) -> Result<Vec<ImportedSymbol>, String> {
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
            if symbol.graphics.len() > 4096 || symbol.pins.len() > 4096 {
                return Err("eplan-input-limit".into());
            }
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
                symbol.graphics.push(g);
            }
        }
        if incomplete {
            symbol.notes.push("eplan-macro-incomplete".into());
        }
        symbol.notes.push("eplan-styles-review".into());
        if !symbol.graphics.is_empty() || !symbol.pins.is_empty() {
            out.push(symbol);
        }
    }
    if out.is_empty() {
        return Err("eplan-no-symbols".into());
    }
    Ok(out)
}
