// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device and symbol browser (the picker): one overlay of the tool host for what can be placed on the focused page
//! (`Session::place_catalogue`, fetched once per (revision, folio) while it is open). Its tree ([`Picker::nodes`]) is
//! All, In this project, then one node per function category with words ([`category_words`]; the first level of a
//! symbol's FnDef category, of a part's own category or its main function's), then Other, each with the count of its
//! hits under the filter. Its rows ([`Picker::rows`]) are those of the chosen node: the symbol rows (the plain name,
//! [`symbol_name`], with the badge `embedded` or `starter`, then the `<key>`), then one row per part
//! ([`PickerRow::Part`]); a part with a function template opens ([`Picker::set_open`]) to one row per function
//! (`<n> of <count> <fndef> [main]`), and choosing the part itself places its main function. A function that a device
//! of the part has unplaced fills that device ([`Picker::joins`]; its row says so) instead of making a new one. The
//! filter matches case-folded substrings of the key, the part number, the manufacturer, the FnDef keys, a symbol's name and keywords
//! and a part's description and category, every typed token somewhere in the row, across the categories. Up, Down,
//! Home and End move the row; Enter chooses; Escape or a click beside the browser closes it.
//!
//! **Device from part** lists the project's own part copies first, then the libraries' parts; a
//! held copy without a function template is one row, whose choice lists the allowed symbols
//! that are no black box, one of which places the part (`device.place` `from_part`, entry 0). Its empty state under
//! a filter names Edit ▸ Choose part… as the next step.
//!
//! A row is disabled with a reason when the page's type does not allow the symbol's representation type or when a
//! template function has no symbol on the page. In exchange mode (§7.5) only the allowed symbol rows are listed. A
//! black-box row (a black-box symbol, or a template function that is a black box) arms the black box tool instead of
//! Place; Insert ▸ Black box opens the picker on those rows alone ([`PickerMode::BlackBox`]).
//!
//! **The strip chooser**. Choosing a terminal symbol without a strip turns the picker into its strip mode:
//! one row per strip of the project (the catalogue's `strips`, by tag), each with the next free designation the
//! placement proposes, then the strip's ID, which the printer names `@n` (`row strip "-X1031" terminal "21" @7`); the
//! filter matches the tag; Enter arms the Place tool with that strip. The strip chooser and a part's symbol choice have
//! no tree.

use crate::key;
use crate::strings::Key;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_api::output::{CatalogueSymbol, PlaceCatalogue, PlacedBy, SymbolShape};

/// A symbol's plain name, as the picker shows it: the key's last segment without its grid
/// variant (`ew.iec.mcb-1p@qet10` → `mcb-1p`).
pub fn symbol_name(key: &str) -> &str {
    let base = key.split('@').next().unwrap_or(key);
    base.rsplit('.').next().unwrap_or(base)
}

/// What the picker was opened for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickerMode {
    /// Insert ▸ Symbol: symbols and parts.
    Place,
    /// Insert ▸ Device from part: parts only.
    Parts,
    /// Edit ▸ Change symbol: allowed symbols only.
    Exchange,
    /// The strip chooser of a terminal placement: the project's strips.
    Strip,
    /// Insert ▸ Black box: the black-box symbols and the template functions that are black boxes.
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
    /// A template function of an opened part: the part's and the function's indices in the catalogue.
    PartFn(usize, usize),
    /// A strip of the strip chooser: an index into the catalogue's strips.
    Strip(usize),
    /// A part: an index into the catalogue's parts. Choosing a part with a function template places its main
    /// function ([`Picker::part_main`]); choosing a part copy without one lists the symbols to place it with
    /// ([`Picker::part`]).
    Part(usize),
}

/// A node of the browser's tree.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum Node {
    /// Every row.
    #[default]
    All,
    /// The project's own symbols and part copies.
    Project,
    /// The rows of one function category with words, by its key (`ProtectionOvercurrent`).
    Category(String),
    /// The rows whose category has no words, or that have none.
    Other,
}

impl Node {
    /// The node's word in `picker.category {to}`: `all`, `project`, `other` or the category key.
    pub fn word(&self) -> &str {
        match self {
            Node::All => "all",
            Node::Project => "project",
            Node::Category(key) => key,
            Node::Other => "other",
        }
    }

    /// The node of word `word` ([`Node::word`]); `None` for a category without words.
    pub fn of_word(word: &str) -> Option<Node> {
        match word {
            "all" => Some(Node::All),
            "project" => Some(Node::Project),
            "other" => Some(Node::Other),
            key => category_words(key).map(|_| Node::Category(key.to_owned())),
        }
    }

    /// The node's name in words.
    pub fn words(&self) -> String {
        match self {
            Node::All => crate::strings::text(key!("choice-all")).to_owned(),
            Node::Project => crate::strings::text(key!("picker-group-project")).to_owned(),
            Node::Category(key) => category_words(key).unwrap_or_default().to_owned(),
            Node::Other => crate::strings::text(key!("category-other")).to_owned(),
        }
    }

    /// The node a symbol or part of function category `category` stands under besides All.
    fn of_category(category: &str) -> Node {
        if category_words(category).is_some() {
            Node::Category(category.to_owned())
        } else {
            Node::Other
        }
    }
}

/// The words of the function category `key` (the first level of an FnDef category, `ProtectionOvercurrent`) in the UI
/// language, its `function-category-<key>` text; `None` for a category without words (the browser lists it under
/// Other).
pub fn category_words(key: &str) -> Option<&'static str> {
    if key.is_empty() {
        return None;
    }
    crate::strings::lookup(&format!("function-category-{key}"))
}

/// The terminal placement a strip chooser was opened for: the terminal symbol and the parameters of the choice that
/// opened it (`letter`, `designation`).
#[derive(Clone, PartialEq, Debug)]
pub struct PendingTerminal {
    /// The terminal symbol's key (its row is looked up again in the catalogue the choice is made with).
    pub symbol: String,
    /// The parameters of the choice (an explicit `designation` wins over the strip's next one).
    pub params: serde_json::Map<String, serde_json::Value>,
    /// The part copy and template entry of a part-first terminal (G-PP finding 2): the chosen strip's terminal
    /// carries the part.
    pub part: Option<(String, u16)>,
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
    /// The visible rows: those of the chosen node under the filter, an opened part followed by its functions.
    pub rows: Vec<PickerRow>,
    /// The terminal placement of the strip chooser.
    pub pending: Option<PendingTerminal>,
    /// The chosen part without a function template: an index into the catalogue's parts; while set, the
    /// rows are the allowed symbols that are no black box, one of which places the part.
    pub part: Option<usize>,
    /// The chosen node of the tree.
    pub node: Node,
    /// The tree under the filter, each node with its count of hits: All, In this project, the categories by their
    /// words, Other, each with hits (All and the chosen node always); empty for the strip chooser and a part's symbol
    /// choice.
    pub nodes: Vec<(Node, usize)>,
    /// The parts opened to their functions, as indices into the catalogue's parts.
    pub open: BTreeSet<usize>,
    /// The shape of what the current row places, for the browser's picture; the host sets it after each change.
    pub preview: Option<Arc<SymbolShape>>,
    /// The tag of the device each function of a part fills, by (part, function) indices into the catalogue: a device
    /// of the part that has the function unplaced (the host's `template_slots`, set when the browser opens).
    pub joins: BTreeMap<(usize, usize), String>,
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
            part: None,
            node: Node::All,
            nodes: Vec::new(),
            open: BTreeSet::new(),
            preview: None,
            joins: BTreeMap::new(),
        };
        p.refilter();
        p
    }

    /// The strip chooser for the terminal placement `pending`.
    pub fn strips(folio: &str, catalogue: Arc<PlaceCatalogue>, pending: PendingTerminal) -> Picker {
        let mut p = Picker::new(PickerMode::Strip, folio, catalogue);
        p.pending = Some(pending);
        p
    }

    /// The functions of part `p` the mode lists, as indices into its template: every one, or in
    /// [`PickerMode::BlackBox`] the black boxes.
    fn listed_functions(&self, p: usize) -> Vec<usize> {
        let Some(part) = self.catalogue.parts.get(p) else {
            return Vec::new();
        };
        (0..part.functions.len())
            .filter(|k| self.mode != PickerMode::BlackBox || part.functions[*k].black_box)
            .collect()
    }

    /// The function that choosing part `p` places: its main function among those listed, else the first listed;
    /// `None` for a part without one.
    pub fn part_main(&self, p: usize) -> Option<usize> {
        let part = self.catalogue.parts.get(p)?;
        let listed = self.listed_functions(p);
        listed
            .iter()
            .copied()
            .find(|k| part.functions[*k].main)
            .or_else(|| listed.first().copied())
    }

    /// Whether part `p` opens to its functions: a part with more than one function the mode lists.
    pub fn opens(&self, p: usize) -> bool {
        self.listed_functions(p).len() > 1
    }

    /// Recomputes the tree and the rows after a change of the filter, the node or an opened part; the current row
    /// goes back to the first.
    pub fn refilter(&mut self) {
        // Every typed token is a case-folded substring of one of the row's texts ("relay" found the generic
        // relay only by its key).
        let f = self.filter.to_lowercase();
        let tokens: Vec<&str> = f.split_whitespace().collect();
        let hits = |texts: &[&str]| {
            let texts: Vec<String> = texts.iter().map(|t| t.to_lowercase()).collect();
            tokens.iter().all(|tok| texts.iter().any(|t| t.contains(tok)))
        };
        let mut rows = Vec::new();
        self.nodes.clear();
        if self.mode == PickerMode::Strip {
            for (i, strip) in self.catalogue.strips.iter().enumerate() {
                if hits(&[&strip.tag]) {
                    rows.push(PickerRow::Strip(i));
                }
            }
            self.rows = rows;
            self.row = 0;
            return;
        }
        // The symbols a part without a function template can be placed with.
        if self.part.is_some() {
            for (i, s) in self.catalogue.symbols.iter().enumerate() {
                let mut texts = vec![s.key.as_str(), s.fndef_key.as_str()];
                texts.extend(s.words.iter().map(String::as_str));
                if s.allowed && !s.black_box && hits(&texts) {
                    rows.push(PickerRow::Symbol(i));
                }
            }
            self.rows = rows;
            self.row = 0;
            return;
        }
        // Every hit under the filter across the categories, with its node and whether the project holds it.
        let mut found: Vec<(PickerRow, Node, bool)> = Vec::new();
        if self.mode != PickerMode::Parts {
            for (i, s) in self.catalogue.symbols.iter().enumerate() {
                if self.mode == PickerMode::Exchange && !s.allowed {
                    continue;
                }
                if self.mode == PickerMode::BlackBox && !s.black_box {
                    continue;
                }
                // What the program draws with what it belongs to (a junction's dot, a cable's marker, the arrow
                // of an external reference) is no row to place: chosen, it could only be refused.
                if self.mode != PickerMode::Exchange && s.placed_by == PlacedBy::Program {
                    continue;
                }
                let mut texts = vec![s.key.as_str(), s.fndef_key.as_str()];
                texts.extend(s.words.iter().map(String::as_str));
                if hits(&texts) {
                    found.push((
                        PickerRow::Symbol(i),
                        Node::of_category(&s.category),
                        s.copy.is_some(),
                    ));
                }
            }
        }
        if self.mode != PickerMode::Exchange {
            // One order, (manufacturer, part number), the catalogue's order within a tie; the project's own
            // copies before the libraries' parts.
            let mut parts: Vec<usize> = (0..self.catalogue.parts.len()).collect();
            parts.sort_by(|a, b| {
                let (a, b) = (&self.catalogue.parts[*a], &self.catalogue.parts[*b]);
                (a.library.is_some(), &a.manufacturer, &a.part_number).cmp(&(
                    b.library.is_some(),
                    &b.manufacturer,
                    &b.part_number,
                ))
            });
            for p in parts {
                let part = &self.catalogue.parts[p];
                let mut texts = vec![part.part_number.as_str(), part.manufacturer.as_str()];
                texts.extend(part.words.iter().map(String::as_str));
                if part.functions.is_empty() {
                    // Device from part lists a held copy without a function template as one row.
                    if self.mode != PickerMode::Parts {
                        continue;
                    }
                } else {
                    let listed = self.listed_functions(p);
                    if listed.is_empty() {
                        continue;
                    }
                    texts.extend(listed.iter().map(|k| part.functions[*k].fndef_key.as_str()));
                }
                if hits(&texts) {
                    found.push((
                        PickerRow::Part(p),
                        Node::of_category(&part.category),
                        part.library.is_none(),
                    ));
                }
            }
        }
        // The tree: All, In this project, the categories by their words, Other.
        let mut categories: Vec<Node> = found
            .iter()
            .filter(|(_, n, _)| matches!(n, Node::Category(_)))
            .map(|(_, n, _)| n.clone())
            .collect();
        categories.sort_by_cached_key(|n| (n.words().to_lowercase(), n.word().to_owned()));
        categories.dedup();
        let mut tree = vec![Node::All, Node::Project];
        tree.extend(categories);
        tree.push(Node::Other);
        let count = |node: &Node| match node {
            Node::All => found.len(),
            Node::Project => found.iter().filter(|(_, _, held)| *held).count(),
            _ => found.iter().filter(|(_, n, _)| n == node).count(),
        };
        let nodes: Vec<(Node, usize)> = tree
            .into_iter()
            .map(|n| {
                let c = count(&n);
                (n, c)
            })
            .filter(|(n, c)| *c > 0 || *n == Node::All || *n == self.node)
            .collect();
        for (row, node, held) in found {
            let shown = match &self.node {
                Node::All => true,
                Node::Project => held,
                chosen => *chosen == node,
            };
            if !shown {
                continue;
            }
            rows.push(row);
            if let PickerRow::Part(p) = row
                && self.open.contains(&p)
                && self.opens(p)
            {
                rows.extend(
                    self.listed_functions(p)
                        .into_iter()
                        .map(|k| PickerRow::PartFn(p, k)),
                );
            }
        }
        self.nodes = nodes;
        self.rows = rows;
        self.row = 0;
    }

    /// Chooses the tree's node `node`; the current row goes back to the first.
    pub fn set_node(&mut self, node: Node) {
        self.node = node;
        self.refilter();
    }

    /// Opens (`true`) or closes the part of the current row, a part row or one of its function rows; its part's
    /// row is current then. A row of a part that does not open stays as it is.
    pub fn set_open(&mut self, open: bool) {
        let p = match self.rows.get(self.row) {
            Some(PickerRow::Part(p) | PickerRow::PartFn(p, _)) => *p,
            _ => return,
        };
        if !self.opens(p) {
            return;
        }
        if open {
            self.open.insert(p);
        } else {
            self.open.remove(&p);
        }
        self.refilter();
        self.row = self
            .rows
            .iter()
            .position(|r| *r == PickerRow::Part(p))
            .unwrap_or(0);
    }

    /// The string key of the text the picker shows instead of rows; `None` while it has rows. With a filter typed it
    /// is `picker-empty` ("Nothing matches the filter"); with the filter empty the list itself is empty, and the text
    /// says what is missing for the mode (a fresh project has no part copies, so Device lists nothing).
    /// Device from part's empty state names Choose part… as the next step (`picker-empty-parts-filter`).
    pub fn empty_text(&self) -> Option<Key<'static>> {
        if !self.rows.is_empty() {
            return None;
        }
        if !self.filter.is_empty() {
            if self.mode == PickerMode::Parts && self.part.is_none() {
                return Some(key!("picker-empty-parts-filter"));
            }
            return Some(key!("picker-empty"));
        }
        Some(match self.mode {
            PickerMode::Parts => key!("picker-empty-parts"),
            PickerMode::Strip => key!("picker-empty-strips"),
            PickerMode::BlackBox => key!("picker-empty-black-box"),
            PickerMode::Place | PickerMode::Exchange => key!("picker-empty-page"),
        })
    }

    /// Sets the filter.
    pub fn set_filter(&mut self, text: &str) {
        self.filter = text.to_owned();
        self.refilter();
    }

    /// Lists the symbols to place part `p` (a part without a function template) with, the filter empty.
    pub fn choose_part(&mut self, p: usize) {
        self.part = Some(p);
        self.filter.clear();
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

    /// The catalogue symbol row `row` places: the symbol's own, a function's, a part's main function's; `None` for
    /// a strip, a part without a function template and a function without a symbol on the page.
    pub fn row_symbol(&self, row: PickerRow) -> Option<&CatalogueSymbol> {
        match row {
            PickerRow::Symbol(i) => self.symbol(i),
            PickerRow::PartFn(p, k) => {
                let key = self.catalogue.parts.get(p)?.functions.get(k)?.symbol.as_deref()?;
                self.catalogue.symbols.iter().find(|s| s.key == key)
            }
            PickerRow::Part(p) => self.row_symbol(PickerRow::PartFn(p, self.part_main(p)?)),
            PickerRow::Strip(_) => None,
        }
    }

    /// The key of the reason a row is disabled; `None` for an enabled row.
    pub fn disabled(&self, row: PickerRow) -> Option<Key<'static>> {
        match row {
            PickerRow::Symbol(i) => {
                let s = self.catalogue.symbols.get(i)?;
                // A black-box row arms the black box tool.
                if s.grid_mismatch.is_some() {
                    Some(key!("reason-symbol-grid"))
                } else if !s.allowed {
                    Some(key!("reason-rep-type-not-allowed"))
                } else if self.mode != PickerMode::Exchange
                    && s.placed_by == PlacedBy::Device
                    && s.letter.is_none()
                    && !s.terminal
                {
                    // A new device needs a letter code, and the symbol's category maps to none.
                    Some(key!("reason-letter-needed"))
                } else {
                    None
                }
            }
            PickerRow::PartFn(p, k) => {
                let f = self.catalogue.parts.get(p)?.functions.get(k)?;
                if f.symbol.is_none() {
                    Some(key!("reason-no-symbol-for-function"))
                } else {
                    None
                }
            }
            // A part is disabled as the function choosing it places.
            PickerRow::Part(p) => self.disabled(PickerRow::PartFn(p, self.part_main(p)?)),
            PickerRow::Strip(_) => None,
        }
    }

    /// The words of the reason a row is disabled; `None` for an enabled row. A symbol on another connection grid
    /// names both grids as the status bar does.
    pub fn disabled_words(&self, row: PickerRow) -> Option<String> {
        let reason = self.disabled(row)?;
        if let PickerRow::Symbol(i) = row
            && let Some(grids) = self.catalogue.symbols.get(i).and_then(|s| s.grid_mismatch)
        {
            return Some(grid_words(grids));
        }
        // A symbol the page's type does not take names the page types that do, as New page lists them.
        if reason == key!("reason-rep-type-not-allowed")
            && let PickerRow::Symbol(i) = row
            && let Some(s) = self.catalogue.symbols.get(i)
        {
            let types: Vec<String> = crate::shell::PAGE_TYPES
                .iter()
                .filter(|t| ew_api::output::page_type_takes(t, &s.rep_type))
                .map(|t| crate::strings::choice_text(t))
                .collect();
            if !types.is_empty() {
                return Some(crate::strings::format(
                    key!("reason-rep-type-pages"),
                    &[&types.join(", ")],
                ));
            }
        }
        Some(crate::strings::text(reason).to_owned())
    }
}

/// Why a symbol on another connection grid cannot be placed on the page, both grids named as the status bar names
/// them ("4 mm", "QET 10 px"), the words New page and Properties offer.
pub(crate) fn grid_words([symbol, page]: [(u64, u64); 2]) -> String {
    let words = |(num, den): (u64, u64)| crate::status::grid_words(num as i64, den as i64);
    crate::strings::format(key!("reason-symbol-grid"), &[&words(symbol), &words(page)])
}

impl Picker {
    /// The describe line of a row (without its indentation): `row symbol "<key>" embedded`,
    /// `row part "<number>" function <n> of <count> "<fndef>" [main]` (a part row as the function it places, then
    /// ` open` or ` closed` when it opens), `row part "<number>" symbol` or `row strip "<tag>" terminal "<next>" <id>`,
    /// then ` disabled: <reason>`.
    pub fn row_line(&self, row: PickerRow) -> String {
        let mut line = match row {
            PickerRow::Symbol(i) => {
                // The plain name first, as the window shows it, then the badge, then the key (the window's
                // tooltip), which `place` and the scripts name the symbol by.
                let s = &self.catalogue.symbols[i];
                format!(
                    "row symbol \"{}\" {} \"{}\"",
                    symbol_name(&s.key),
                    // An installed or user library's symbol is never "starter".
                    if s.copy.is_some() {
                        "embedded"
                    } else if s.key.starts_with("library:") {
                        "library"
                    } else {
                        "starter"
                    },
                    s.key
                )
            }
            PickerRow::PartFn(p, k) => {
                let part = &self.catalogue.parts[p];
                let f = &part.functions[k];
                let mut line = format!(
                    "row part \"{}\" function {} of {} \"{}\"{}",
                    part.part_number,
                    k + 1,
                    part.functions.len(),
                    f.fndef_key,
                    if f.main { " main" } else { "" }
                );
                if let Some(tag) = self.joins.get(&(p, k)) {
                    line.push_str(&format!(" joins \"{tag}\""));
                }
                line
            }
            // A part with a function template reads as the function choosing it places, then `open` or `closed`
            // when it opens to its functions.
            PickerRow::Part(p) => match self.part_main(p) {
                Some(k) => {
                    let mut line = self.row_line(PickerRow::PartFn(p, k));
                    if let Some(at) = line.find(" disabled: ") {
                        line.truncate(at);
                    }
                    if self.opens(p) {
                        line.push_str(if self.open.contains(&p) {
                            " open"
                        } else {
                            " closed"
                        });
                    }
                    line
                }
                None => format!("row part \"{}\" symbol", self.catalogue.parts[p].part_number),
            },
            PickerRow::Strip(i) => {
                let strip = &self.catalogue.strips[i];
                // The strip's ID ends the line, which the printer names `@n` (item 9): a strip without
                // terminals prints `-X??` like every other, and this is how it is selected (and deleted).
                format!(
                    "row strip \"{}\" terminal \"{}\" {}",
                    strip.tag, strip.next_designation, strip.id
                )
            }
        };
        if let Some(reason) = self.disabled_words(row) {
            line.push_str(&format!(" disabled: {reason}"));
        }
        line
    }
}
