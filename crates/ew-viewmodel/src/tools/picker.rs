// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The picker (WP-S2.04 §7.1): a popup of the tool host listing what can be placed on the focused page
//! (`Session::place_catalogue`, fetched once per (revision, folio) while it is open). Its rows are the symbol rows
//! (`<key>` with the badge `embedded` or `starter`), then one row per function of each part's template
//! (`<part number> <manufacturer>`, `<n> of <count> <fndef> [main]`). The filter matches case-folded substrings of
//! the key, the part number and the FnDef key; Up, Down, Home and End move the row; Enter chooses; Escape closes.
//!
//! A row is disabled with a reason when the page's type does not allow the symbol's representation type or when a
//! template function has no symbol on the page. In exchange mode (§7.5) only the allowed symbol rows are listed. A
//! black-box row (a black-box symbol, or a template function that is a black box) arms the black box tool instead of
//! Place (WP-S2.04 §9.5); Insert ▸ Black box opens the picker on those rows alone ([`PickerMode::BlackBox`]).
//!
//! **The strip chooser** (FT-421). Choosing a terminal symbol without a strip turns the picker into its strip mode:
//! one row per strip of the project (the catalogue's `strips`, by tag), each with the next free designation the
//! placement proposes, then the strip's ID, which the printer names `@n` (`row strip "-X1031" terminal "21" @7`;
//! FT-485 item 9); the filter matches the tag; Enter arms the Place tool with that strip.

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
    /// The strip chooser of a terminal placement (FT-421): the project's strips.
    Strip,
    /// Insert ▸ Black box (WP-S2.04 §9.4): the black-box symbols and the template functions that are black boxes.
    BlackBox,
}

impl PickerMode {
    /// The transcript's word: `place` (also for the parts filter) or `exchange`.
    pub fn word(self) -> &'static str {
        match self {
            PickerMode::Place | PickerMode::Parts => "place",
            PickerMode::Exchange => "exchange",
            PickerMode::Strip => "strip",
            PickerMode::BlackBox => "box",
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
    /// A strip of the strip chooser: an index into the catalogue's strips.
    Strip(usize),
}

/// The terminal placement a strip chooser was opened for: the terminal symbol and the parameters of the choice that
/// opened it (`letter`, `designation`).
#[derive(Clone, PartialEq, Debug)]
pub struct PendingTerminal {
    /// The terminal symbol's key (its row is looked up again in the catalogue the choice is made with).
    pub symbol: String,
    /// The parameters of the choice (an explicit `designation` wins over the strip's next one).
    pub params: serde_json::Map<String, serde_json::Value>,
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
    /// The terminal placement of the strip chooser.
    pub pending: Option<PendingTerminal>,
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
            pending: None,
        };
        p.refilter();
        p
    }

    /// The strip chooser for the terminal placement `pending` (FT-421).
    pub fn strips(folio: &str, catalogue: Arc<PlaceCatalogue>, pending: PendingTerminal) -> Picker {
        let mut p = Picker::new(PickerMode::Strip, folio, catalogue);
        p.pending = Some(pending);
        p
    }

    /// Recomputes the rows after a filter change; the current row goes back to the first.
    pub fn refilter(&mut self) {
        let f = self.filter.to_lowercase();
        let hit = |s: &str| f.is_empty() || s.to_lowercase().contains(&f);
        let mut rows = Vec::new();
        if self.mode == PickerMode::Strip {
            for (i, strip) in self.catalogue.strips.iter().enumerate() {
                if hit(&strip.tag) {
                    rows.push(PickerRow::Strip(i));
                }
            }
            self.rows = rows;
            self.row = 0;
            return;
        }
        if self.mode != PickerMode::Parts {
            for (i, s) in self.catalogue.symbols.iter().enumerate() {
                if self.mode == PickerMode::Exchange && !s.allowed {
                    continue;
                }
                if self.mode == PickerMode::BlackBox && !s.black_box {
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
                    if self.mode == PickerMode::BlackBox && !func.black_box {
                        continue;
                    }
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
                // From WP-S2.04 part 3 a black-box row arms the black box tool (§9.5).
                if !s.allowed {
                    Some("reason-rep-type-not-allowed")
                } else {
                    None
                }
            }
            PickerRow::PartFn(p, k) => {
                let f = self.catalogue.parts.get(p)?.functions.get(k)?;
                if f.symbol.is_none() {
                    Some("reason-no-symbol-for-function")
                } else {
                    None
                }
            }
            PickerRow::Strip(_) => None,
        }
    }

    /// The describe line of a row (without its indentation): `row symbol "<key>" embedded`,
    /// `row part "<number>" function <n> of <count> "<fndef>" [main]` or `row strip "<tag>" terminal "<next>" <id>`,
    /// then ` disabled: <reason>`.
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
            PickerRow::Strip(i) => {
                let strip = &self.catalogue.strips[i];
                // The strip's ID ends the line, which the printer names `@n` (FT-485 item 9): a strip without
                // terminals prints `-X??` like every other, and this is how it is selected (and deleted).
                format!(
                    "row strip \"{}\" terminal \"{}\" {}",
                    strip.tag, strip.next_designation, strip.id
                )
            }
        };
        if let Some(reason) = self.disabled(row) {
            line.push_str(&format!(" disabled: {}", crate::strings::text(reason)));
        }
        line
    }
}
