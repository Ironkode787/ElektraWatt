// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded EPLAN EDZ (7-Zip and ZIP), PXF macro XML and parts XML symbol import.

#![forbid(unsafe_code)]

mod archive;
mod pxf;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Maximum compressed archive input.
pub const MAX_INPUT: u64 = 32 * 1024 * 1024;
/// Maximum individual archive entry or standalone XML.
pub const MAX_FILE: u64 = 4 * 1024 * 1024;
/// Maximum total unpacked archive bytes.
pub const MAX_TOTAL: u64 = 64 * 1024 * 1024;

/// One editable drawing recovered from a source file. Geometry is in EW nanometres, y down.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedSymbol {
    /// Source member name, never a filesystem path.
    pub source: String,
    /// Human name of the symbol, part or macro variant.
    pub name: String,
    /// Native SymbolDef primitive JSON.
    pub graphics: Vec<Value>,
    /// Symbol CP geometry and FnDef designation pairs.
    pub pins: Vec<Pin>,
    /// Stable diagnostic keys. An incomplete import requires explicit review in the editor.
    pub notes: Vec<String>,
}

/// A connection recovered from XML. Native-library references may need manual verification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pin {
    /// Printed designation.
    pub designation: String,
    /// Position in nanometres relative to insertion origin.
    pub position: [i64; 2],
    /// Outward direction when known; `None` requires author input.
    pub direction: Option<String>,
}

/// Import EDZ (7z/ZIP), PXF XML macros (.ema/.ems/.esl), or parts XML.
/// Binary symbol databases (.slk) and unknown PXF revisions are explicitly refused.
/// No filesystem extraction, linked-file loading or code execution occurs.
///
/// # Errors
/// Stable diagnostic key for unsupported, unsafe, malformed or oversized input.
pub fn read(bytes: &[u8], name: &str) -> Result<Vec<ImportedSymbol>, String> {
    if bytes.len() as u64 > MAX_INPUT {
        return Err("eplan-input-limit".into());
    }
    let is_archive = bytes.starts_with(b"7z\xbc\xaf\x27\x1c") || bytes.starts_with(b"PK");
    let files = if is_archive {
        archive::read(bytes)?
    } else {
        vec![(archive::safe_name(name)?, bytes.to_vec())]
    };
    let mut result = Vec::new();
    for (name, data) in files {
        let lower = name.to_ascii_lowercase();
        if ![".xml", ".ema", ".ems", ".esl", ".slk"]
            .iter()
            .any(|e| lower.ends_with(e))
        {
            continue;
        }
        if lower.ends_with("manifest.xml") || lower.ends_with("manufacturer.xml") {
            continue;
        }
        if data.len() as u64 > MAX_FILE {
            return Err("eplan-input-limit".into());
        }
        let text = std::str::from_utf8(&data).map_err(|_| "eplan-binary-unsupported")?;
        if !text.trim_start_matches('\u{feff}').trim_start().starts_with('<') {
            return Err("eplan-binary-unsupported".into());
        }
        let doc = pxf::xml(text)?;
        match doc.root_element().tag_name().name() {
            "EplanPxfRoot" => result.extend(pxf::read(&doc, &name)?),
            "partsmanagement" => result.extend(parts(&doc, &name)?),
            _ if !is_archive => return Err("eplan-format-unsupported".into()),
            _ => {}
        }
        if result.len() > 512 {
            return Err("eplan-input-limit".into());
        }
    }
    if result.is_empty() {
        return Err("eplan-no-symbols".into());
    }
    Ok(result)
}

fn parts(doc: &roxmltree::Document<'_>, source: &str) -> Result<Vec<ImportedSymbol>, String> {
    let mut out = Vec::new();
    for part in doc.descendants().filter(|n| n.has_tag_name("part")) {
        let name = part
            .attribute("P_ARTICLE_PARTNR")
            .unwrap_or("Imported part")
            .to_owned();
        let mut pins = Vec::new();
        for template in part.descendants().filter(|n| n.has_tag_name("functiontemplate")) {
            for designation in template
                .attribute("connectionDesignation")
                .unwrap_or("")
                .split(['\n', '\r', '¶'])
            {
                let designation = designation.trim();
                if designation.is_empty() {
                    continue;
                }
                if pins.len() >= 4096 {
                    return Err("eplan-input-limit".into());
                }
                pins.push(Pin {
                    designation: designation.into(),
                    position: [0, (pins.len() as i64 + 1) * 4_000_000],
                    direction: Some("left".into()),
                });
            }
        }
        if pins.is_empty() {
            continue;
        }
        let height = (pins.len() as i64 + 1) * 4_000_000;
        let mut graphics =
            vec![json!({"rect":{"rect":[[4_000_000,0],[24_000_000,height]],"stroke":{"pen":"p026"}}})];
        for pin in &pins {
            graphics.push(json!({"line":{"from":pin.position,"to":[4_000_000,pin.position[1]],"stroke":{"pen":"p026"}}}));
        }
        out.push(ImportedSymbol {
            source: source.into(),
            name,
            graphics,
            pins,
            notes: vec!["eplan-generated-body".into(), "eplan-function-review".into()],
        });
    }
    Ok(out)
}
