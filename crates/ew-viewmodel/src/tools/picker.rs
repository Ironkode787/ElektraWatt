// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The picker (WP-S2.04 §7.1): a popup of the tool host listing what can be placed on the focused page
//! (`Session::place_catalogue`, fetched once per (revision, folio) while it is open). Its rows are the symbol rows
//! (`<key>` with the badge `embedded` or `starter`), then one row per function of each part's template
//! (`<part number> <manufacturer>`, `<n> of <count> <fndef> [main]`). The filter matches case-folded substrings of
//! the key, the part number and the FnDef key; Up, Down, Home and End move the row; Enter chooses; Escape closes.
//!
//! A row is disabled with a reason when the page's type does not allow the symbol's representation type, when the
//! symbol draws a black box (placed by part 3's black box tool) or when a template function has no symbol on the
//! page. In exchange mode (§7.5) only the allowed symbol rows are listed.

use std::sync::Arc;

use ew_api::output::{CatalogueSymbol, PlaceCatalogue};

/// What the picker was opened for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickerMode {
    /// Insert ▸ Symbol: symbols and parts.
    Place,
    /// Insert ▸ Device from part: parts only.
    Parts,
    /// Edit ▸ Change symbol: allowed symbols only.
    Exchange,
}

impl PickerMode {
    /// The transcript's word: `place` (also for the parts filter) or `exchange`.
    pub fn word(self) -> &'static str {
        match self {
            PickerMode::Place | PickerMode::Parts => "place",
            PickerMode::Exchange => "exchange",
        }
    }
}

/// One row of the picker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickerRow {
    /// A symbol row: an index into the catalogue's symbols.
    Symbol(usize),
    /// A template function: the part's and the function's indices in the catalogue.
    PartFn(usize, usize),
}

/// The picker's state.
#[derive(Clone, Debug)]
pub struct Picker {
    /// Why it was opened.
    pub mode: PickerMode,
    /// The filter text.
    pub filter: String,
    /// The current row, an index into [`Picker::rows`].
    pub row: usize,
    /// The catalogue of the page.
    pub catalogue: Arc<PlaceCatalogue>,
    /// The page's folio.
    pub folio: String,
    /// The visible rows under the filter.
    pub rows: Vec<PickerRow>,
}

impl Picker {
    /// A picker over `catalogue` in `mode`, the filter empty, the first row current.
    pub fn new(mode: PickerMode, folio: &str, catalogue: Arc<PlaceCatalogue>) -> Picker {
        let mut p = Picker {
            mode,
            filter: String::new(),
            row: 0,
            catalogue,
            folio: folio.to_owned(),
            rows: Vec::new(),
        };
        p.refilter();
        p
    }

    /// Recomputes the rows after a filter change; the current row goes back to the first.
    pub fn refilter(&mut self) {
        let f = self.filter.to_lowercase();
        let hit = |s: &str| f.is_empty() || s.to_lowercase().contains(&f);
        let mut rows = Vec::new();
        if self.mode != PickerMode::Parts {
            for (i, s) in self.catalogue.symbols.iter().enumerate() {
                if self.mode == PickerMode::Exchange && !s.allowed {
                    continue;
                }
                if hit(&s.key) || hit(&s.fndef_key) {
                    rows.push(PickerRow::Symbol(i));
                }
            }
        }
        if self.mode != PickerMode::Exchange {
            for (p, part) in self.catalogue.parts.iter().enumerate() {
                for (k, func) in part.functions.iter().enumerate() {
                    if hit(&part.part_number) || hit(&func.fndef_key) {
                        rows.push(PickerRow::PartFn(p, k));
                    }
                }
            }
        }
        self.rows = rows;
        self.row = 0;
    }

    /// Sets the filter.
    pub fn set_filter(&mut self, text: &str) {
        self.filter = text.to_owned();
        self.refilter();
    }

    /// Moves the current row by `step` (clamped) or to `to` (clamped); without rows it stays 0.
    pub fn move_row(&mut self, step: Option<i64>, to: Option<i64>) {
        let n = self.rows.len() as i64;
        if n == 0 {
            self.row = 0;
            return;
        }
        let target = match (step, to) {
            (_, Some(t)) => t,
            (Some(s), None) => self.row as i64 + s,
            (None, None) => self.row as i64,
        };
        self.row = target.clamp(0, n - 1) as usize;
    }

    /// The symbol row `i` of the catalogue.
    pub fn symbol(&self, i: usize) -> Option<&CatalogueSymbol> {
        self.catalogue.symbols.get(i)
    }

    /// The key of the reason a row is disabled; `None` for an enabled row.
    pub fn disabled(&self, row: PickerRow) -> Option<&'static str> {
        match row {
            PickerRow::Symbol(i) => {
                let s = self.catalogue.symbols.get(i)?;
                if !s.allowed {
                    Some("reason-rep-type-not-allowed")
                } else if s.black_box && self.mode != PickerMode::Exchange {
                    Some("reason-black-box-tool")
                } else {
                    None
                }
            }
            PickerRow::PartFn(p, k) => {
                let f = self.catalogue.parts.get(p)?.functions.get(k)?;
                if f.black_box {
                    Some("reason-black-box-tool")
                } else if f.symbol.is_none() {
                    Some("reason-no-symbol-for-function")
                } else {
                    None
                }
            }
        }
    }

    /// The describe line of a row (without its indentation): `row symbol "<key>" embedded` or
    /// `row part "<number>" function <n> of <count> "<fndef>" [main]`, then ` disabled: <reason>`.
    pub fn row_line(&self, row: PickerRow) -> String {
        let mut line = match row {
            PickerRow::Symbol(i) => {
                let s = &self.catalogue.symbols[i];
                format!(
                    "row symbol \"{}\" {}",
                    s.key,
                    if s.copy.is_some() { "embedded" } else { "starter" }
                )
            }
            PickerRow::PartFn(p, k) => {
                let part = &self.catalogue.parts[p];
                let f = &part.functions[k];
                format!(
                    "row part \"{}\" function {} of {} \"{}\"{}",
                    part.part_number,
                    k + 1,
                    part.functions.len(),
                    f.fndef_key,
                    if f.main { " main" } else { "" }
                )
            }
        };
        if let Some(reason) = self.disabled(row) {
            line.push_str(&format!(" disabled: {}", crate::strings::text(reason)));
        }
        line
    }
}
