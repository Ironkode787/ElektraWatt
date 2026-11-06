// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol editor's readers (WP-S2.10; M25-p1#8, M25-p2#6):
//! - [`authoring`]: the bytes as a portable `ew.symbol/1` draft (`SymbolDraft::parse`), and as an EPLAN input named
//!   `.edz`, `.ems` and `.xml` through `symbols::import_eplan`, the front door under `Limits::element_definition()`;
//! - [`slk`]: the bytes as an SLK 1.2 descriptor and its eight `.sdb` tables (nine sections of a 2-byte little-endian
//!   length and the bytes, the last cut short by the input's end), through `slk::directory` and `slk::read`.
//!
//! Properties, whatever the bytes: no reader panics (the front door's caught panic, `symbol-import-stopped`, fails the
//! target); an accepted draft validates, previews (every variant for a parsed draft), and a draft with no validation
//! issue yields its library documents; an SLK read holds the read budget: at most 4,096 graphics and 4,096
//! connection points per symbol, 65,536 of both together, 512 symbols.

use std::collections::BTreeMap;

use ew_api::symbols::{self, Error, SymbolDraft};

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    /// A draft was parsed from the bytes.
    pub parsed: bool,
    /// The drafts the readers made of it.
    pub drafts: usize,
}

/// The front door's refusal of a panic is a finding.
fn no_panic(e: &Error) {
    assert!(
        !matches!(e, Error::Input(key) if key == "symbol-import-stopped"),
        "the reader panicked: {e}"
    );
}

/// A draft validates, previews, and yields its documents when it has no issue.
fn exercise(draft: &SymbolDraft, variants: u8) {
    let issues = draft.validate();
    for variant in 0..variants {
        let _ = draft.preview(variant);
    }
    if issues.is_empty() {
        let _ = draft.documents(symbols::DRAFT_LIBRARY);
    }
}

/// `symbol_authoring`: the bytes as a draft, then as an EPLAN input under three names.
pub fn authoring(data: &[u8]) -> Outcome {
    let mut out = Outcome::default();
    if let Ok(draft) = SymbolDraft::parse(data) {
        out.parsed = true;
        out.drafts += 1;
        exercise(&draft, 8);
    }
    for name in ["sample.edz", "sample.ems", "sample.xml"] {
        match symbols::import_eplan(data, name, "fuzz.symbol") {
            Ok(drafts) => {
                out.drafts += drafts.len();
                for draft in &drafts {
                    exercise(draft, 1);
                }
            }
            Err(e) => no_panic(&e),
        }
    }
    out
}

/// What [`slk`] did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SlkOutcome {
    /// The descriptor named a `.sdb` folder.
    pub descriptor: bool,
    /// The symbols read from the tables.
    pub symbols: usize,
}

/// The nine sections of an input: the descriptor, then the tables of `slk::FILES` in order.
pub fn sections(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut rest = data;
    for _ in 0..9 {
        let len = match rest {
            [a, b, tail @ ..] => {
                rest = tail;
                usize::from(u16::from_le_bytes([*a, *b]))
            }
            _ => 0,
        };
        let (section, tail) = rest.split_at(len.min(rest.len()));
        out.push(section);
        rest = tail;
    }
    out
}

/// `symbol_slk`: the descriptor through `slk::directory`, the tables through `slk::read`.
pub fn slk(data: &[u8]) -> SlkOutcome {
    use ew_import_eplan::slk::{FILES, directory, read};
    let sections = sections(data);
    let mut out = SlkOutcome {
        descriptor: directory(sections[0]).is_ok(),
        symbols: 0,
    };
    let files: BTreeMap<String, Vec<u8>> = FILES
        .iter()
        .zip(&sections[1..])
        .map(|(name, bytes)| ((*name).to_owned(), bytes.to_vec()))
        .collect();
    if let Ok(symbols) = read(&files) {
        let mut items = 0;
        for s in &symbols {
            assert!(
                s.graphics.len() <= 4096 && s.pins.len() <= 4096,
                "a symbol over 4,096 items"
            );
            items += s.graphics.len() + s.pins.len();
        }
        assert!(items <= 65_536, "{items} items in one read");
        assert!(
            !symbols.is_empty() && symbols.len() <= 512,
            "{} symbols",
            symbols.len()
        );
        out.symbols = symbols.len();
    }
    out
}
