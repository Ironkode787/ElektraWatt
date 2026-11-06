// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded EPLAN EDZ, PXF/parts XML and native SLK companion-table symbol import.

#![forbid(unsafe_code)]

mod archive;
mod pxf;
mod seven;
pub mod slk;

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
/// Native SLK databases use [`slk::directory`] and [`slk::read`] with bounded companion tables.
/// This byte-only entry point refuses binary input and unknown PXF revisions.
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
    let mut budget = Budget::default();
    // An archive member that is not read, or an XML of another kind, is reported on every symbol (spec §1).
    let mut skipped = false;
    for (name, data) in files {
        let lower = name.to_ascii_lowercase();
        if ![".xml", ".pxf", ".ema", ".ems", ".esl", ".slk"]
            .iter()
            .any(|e| lower.ends_with(e))
        {
            skipped = true;
            continue;
        }
        if lower.ends_with("manifest.xml") || lower.ends_with("manufacturer.xml") {
            continue;
        }
        if data.len() as u64 > MAX_FILE {
            return Err("eplan-input-limit".into());
        }
        let text = text(&data)?;
        if !text.trim_start_matches('\u{feff}').trim_start().starts_with('<') {
            return Err("eplan-binary-unsupported".into());
        }
        let doc = pxf::xml(&text)?;
        match doc.root_element().tag_name().name() {
            "EplanPxfRoot" => result.extend(pxf::read(&doc, &name, &mut budget)?),
            "partsmanagement" => result.extend(parts(&doc, &name, &mut budget)?),
            _ if !is_archive => return Err("eplan-format-unsupported".into()),
            _ => skipped = true,
        }
    }
    if result.is_empty() {
        return Err("eplan-no-symbols".into());
    }
    if skipped {
        for symbol in &mut result {
            symbol.notes.push("eplan-archive-skipped".into());
        }
    }
    Ok(result)
}

/// The text of an XML member: UTF-8, or UTF-16 after its byte-order mark.
fn text(data: &[u8]) -> Result<std::borrow::Cow<'_, str>, String> {
    let utf16 = |rest: &[u8], unit: fn([u8; 2]) -> u16| {
        if !rest.len().is_multiple_of(2) {
            return Err("eplan-binary-unsupported".to_owned());
        }
        let units: Vec<u16> = rest.chunks_exact(2).map(|c| unit([c[0], c[1]])).collect();
        String::from_utf16(&units)
            .map(std::borrow::Cow::Owned)
            .map_err(|_| "eplan-binary-unsupported".to_owned())
    };
    match data {
        [0xff, 0xfe, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xfe, 0xff, rest @ ..] => utf16(rest, u16::from_be_bytes),
        _ => std::str::from_utf8(data)
            .map(std::borrow::Cow::Borrowed)
            .map_err(|_| "eplan-binary-unsupported".to_owned()),
    }
}

/// The most graphics, and the most connection points, of one imported symbol.
const SYMBOL_ITEMS: usize = 4096;
/// The most graphics and connection points of one read, all its symbols together (M25-p2#2), as many as the SLK
/// reader's graphics.
const READ_ITEMS: usize = 65_536;
/// The most symbols of one read.
const SYMBOLS: usize = 512;

/// One budget across a read and every document in it: each symbol, graphic and connection point is counted
/// before it is added, so a read never holds more than the limits above.
#[derive(Default)]
pub(crate) struct Budget {
    items: usize,
    symbols: usize,
}

impl Budget {
    /// One more graphic or connection point, for a symbol that holds `held` of its kind already.
    pub(crate) fn item(&mut self, held: usize) -> Result<(), String> {
        if held >= SYMBOL_ITEMS || self.items >= READ_ITEMS {
            return Err("eplan-input-limit".into());
        }
        self.items += 1;
        Ok(())
    }

    /// One more symbol.
    pub(crate) fn symbol(&mut self) -> Result<(), String> {
        if self.symbols >= SYMBOLS {
            return Err("eplan-input-limit".into());
        }
        self.symbols += 1;
        Ok(())
    }
}

fn parts(
    doc: &roxmltree::Document<'_>,
    source: &str,
    budget: &mut Budget,
) -> Result<Vec<ImportedSymbol>, String> {
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
                budget.item(pins.len())?;
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
        budget.symbol()?;
        budget.item(0)?;
        let height = (pins.len() as i64 + 1) * 4_000_000;
        let mut graphics =
            vec![json!({"rect":{"rect":[[4_000_000,0],[24_000_000,height]],"stroke":{"pen":"p026"}}})];
        for pin in &pins {
            budget.item(graphics.len())?;
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
