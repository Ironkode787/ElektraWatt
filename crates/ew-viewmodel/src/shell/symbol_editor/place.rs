// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The drawing of a symbol in Parts (doc 07 §5.6): it opens in the middle of the place, on its Drawing tab, for what
//! it is drawn for ([`Drawing`]): a symbol of his own (the Symbols shelf, New symbol in the place), a piece of the open
//! part (Draw a new symbol…, Change it for this part…), or a page (New symbol on the Draw tab and `Ctrl+I`, the symbol
//! picker's foot, Draw one like it…). `parts.new_symbol`, `parts.draw` and `parts.open {symbol}` open it, ready to draw
//! on: the page's connection grid, What it is as he chose it last, the texts its kind needs where the standard puts
//! them, his drawing grid and Snap. `parts.save` saves the symbol with his parts (My parts, or the library he can write
//! that it came from) and finishes the errand: placed on the page with the Place tool armed, exchanged for the selected
//! symbol, or given to the piece. `parts.back` leaves the drawing, which stays in hand.

use super::sheet::{BOX, what_kinds};
use super::{SymbolEditor, name_words, symbol_error, words};
use crate::shell::parts_place::Origin;
use crate::shell::{Outcome, ShellModel, failed, string_param};
use crate::{key, strings};
use ew_api::output::Uuid;
use ew_api::symbols::{SymbolDraft, schema};
use serde_json::{Map, Value, json};

/// What a drawing in Parts is drawn for: what its strip says, what its next step does.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Drawing {
    /// A symbol of his own, opened from the Symbols shelf or made with New symbol in the place: Save saves it.
    Symbol,
    /// A new symbol for a page: Save and place on it, or Save and use for the symbol `exchange` names.
    Page(PageDrawing),
    /// The symbol of a piece of the open part: Done saves it and gives the piece the symbol.
    Piece(PieceDrawing),
}

/// The piece of the open part a symbol is drawn for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PieceDrawing {
    /// Its row in the part's Drawn as table.
    pub index: usize,
    /// The piece in words: what it is and its pins ("Coil, rectangular A1, A2").
    pub piece: String,
    /// The part's number.
    pub part: String,
}

/// The page a new symbol is drawn for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageDrawing {
    /// The page.
    pub page: Uuid,
    /// Its folio ("102").
    pub folio: String,
    /// The page in words ("102 Control 24 V DC").
    pub words: String,
    /// Its connection grid, nanometres per step as `(num, den)`.
    pub grid: (u64, u64),
    /// The placed symbol a drawing like it is for (Draw one like it…), which Save exchanges: its representation and
    /// its device's tag.
    pub exchange: Option<(Uuid, String)>,
}

impl SymbolEditor {
    /// The strip over the drawing: what is drawn and for what; `None` for a symbol of his own.
    pub fn strip(&self) -> Option<String> {
        if let Some(b) = &self.device_box {
            return Some(strings::format(key!("parts-strip-box"), &[&b.title()]));
        }
        match &self.purpose {
            Drawing::Symbol => None,
            Drawing::Page(p) => {
                let errand = match &p.exchange {
                    Some((_, tag)) => strings::format(key!("parts-strip-symbol-like"), &[tag, &p.words]),
                    None => strings::format(key!("parts-strip-symbol-page"), &[&p.words]),
                };
                // A drawing whose connection grid is no longer the page's says so before it is saved, naming both
                // grids as the status bar does.
                let (num, den) = (self.draft.symbol.grid.num(), self.draft.symbol.grid.den());
                if (num, den) == p.grid {
                    return Some(errand);
                }
                let words = |(n, d): (u64, u64)| {
                    crate::status::grid_words(i64::try_from(n).unwrap_or(0), i64::try_from(d).unwrap_or(1))
                };
                let other = strings::format(
                    key!("parts-strip-other-grid"),
                    &[&words((num, den)), &p.folio, &words(p.grid)],
                );
                Some(format!("{errand} {other}"))
            }
            Drawing::Piece(p) => Some(strings::format(
                key!("parts-strip-symbol-piece"),
                &[&p.piece, &p.part],
            )),
        }
    }

    /// The way back's label: Back without it from a page, To the part from a piece, else Back.
    pub fn back_label(&self) -> &'static str {
        if self.device_box.is_some() {
            return strings::text(key!("parts-to-the-part"));
        }
        match &self.purpose {
            Drawing::Page(_) => strings::text(key!("parts-back-without-it")),
            Drawing::Piece(_) => strings::text(key!("parts-to-the-part")),
            Drawing::Symbol => strings::text(key!("parts-back")),
        }
    }

    /// The next step's label: Save and place on the page, Save and use for the symbol, Done for a piece, else Save.
    pub fn save_label(&self) -> String {
        if self.device_box.is_some() {
            return strings::text(key!("parts-done")).to_owned();
        }
        match &self.purpose {
            Drawing::Page(PageDrawing {
                exchange: Some((_, tag)),
                ..
            }) => strings::format(key!("parts-save-use"), &[tag]),
            Drawing::Page(p) => strings::format(key!("parts-save-place"), &[&p.folio]),
            Drawing::Piece(_) => strings::text(key!("parts-done")).to_owned(),
            Drawing::Symbol => strings::text(key!("parts-save")).to_owned(),
        }
    }

    /// Why the symbol in hand cannot be saved now, in words: it is not his (Make my own from it), or the first thing
    /// To do lists, which Save says above the drawing.
    pub fn save_refusal(&self) -> Option<String> {
        if let Some(why) = &self.locked {
            return Some(why.clone());
        }
        self.outline().todo.into_iter().next().map(|t| t.text)
    }
}

/// The reference to the library symbol at `address` (`library:<library>:<item>`), any version.
fn address_ref(address: &str) -> Option<schema::LibRef> {
    let (library, item) = address.strip_prefix("library:")?.split_once(':')?;
    let (library, item) = (library.parse::<Uuid>().ok()?, item.parse::<Uuid>().ok()?);
    serde_json::from_value(json!({"library": library, "item": item, "version_req": "*"})).ok()
}

/// A piece in words: what it is, as the pieces table names it, and its pins ("Coil, rectangular A1, A2").
fn piece_words(row: &ew_api::part_pieces::PieceRow) -> String {
    let what = ew_api::part_pieces::piece_kinds()
        .into_iter()
        .find(|k| k.key == row.what)
        .map_or_else(|| row.what.clone(), |k| strings::render(&k.words));
    format!("{what} {}", row.pins.trim()).trim_end().to_owned()
}

/// What it is, by its [`what_kinds`] word, for a drawing of `discipline`: the word given (the symbol drawn last),
/// else the guess for a first symbol (doc 07 §5.6): a normally open contact on an electrical page, else a device box.
fn what_for(word: Option<&str>, discipline: &str) -> String {
    match word.filter(|w| what_kinds().iter().any(|(k, _)| k == w)) {
        Some(w) => w.to_owned(),
        None if discipline == "electrical" => "ew.fn.contact-no".to_owned(),
        None => BOX.to_owned(),
    }
}

/// A new drawing on connection grid `grid`, as what `what` is: its function set as what it is, and the texts its kind
/// needs already where the standard puts them (doc 07a): the device tag at the upper left, and under it the
/// cross-reference of a coil or a contact. Its points take the discipline of its page as they are drawn.
fn new_draft(grid: (u64, u64), what: &str) -> Result<SymbolDraft, String> {
    let draft = SymbolDraft::new("", strings::text(key!("symbol-untitled"))).map_err(symbol_error)?;
    let mut d = serde_json::to_value(&draft).map_err(|e| e.to_string())?;
    d["symbol"]["grid"] = json!({"num": grid.0, "den": grid.1});
    // A starter function: the symbol draws it; a box: a function of its own, its pins as drawn.
    match (
        ew_api::symbols::starter_function(what),
        ew_api::symbols::piece_function(what),
    ) {
        (Some(def), Some(r)) => {
            d["function"] = serde_json::to_value(def).map_err(|e| e.to_string())?;
            d["function_ref"] = serde_json::to_value(r).map_err(|e| e.to_string())?;
        }
        _ => {
            d["function"]["contact_kind"] = json!("black_box");
            d["function"]["category"] = json!(["Assembly", "Generic", "Generic"]);
        }
    }
    let kind = d["function"]["contact_kind"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    // One text line under another: 6 mm, 4 mm on the QET grid (doc 07a).
    let line: i64 = if grid == (ew_units::Grid::QET_10PX.num(), ew_units::Grid::QET_10PX.den()) {
        4_000_000
    } else {
        6_000_000
    };
    let mut slots =
        vec![json!({"kind": "tag", "pos": [-1_000_000, 0], "align": "bottom_right", "visible": true})];
    if matches!(kind.as_str(), "no" | "nc" | "co" | "coil") {
        slots.push(json!({"kind": "xref", "pos": [-1_000_000, line], "align": "top_right", "visible": true}));
    }
    d["symbol"]["text_slots"] = json!(slots);
    SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(symbol_error)
}

impl ShellModel {
    /// The tag a page would show for a device of what the drawing is, as its sample (doc 07 §5.6): the letter a new
    /// device of its function's category takes, unnumbered (`-S?`); for a piece of a part the part's first device
    /// (`-K1`).
    pub fn drawing_sample_tag(&self) -> String {
        let Some(editor) = self.symbol_editor() else {
            return String::new();
        };
        let letter = self
            .parts_session()
            .and_then(|s| s.device_letter(&editor.draft.function.category))
            .unwrap_or_default();
        match editor.purpose {
            Drawing::Piece(_) => format!("-{letter}1"),
            _ => format!("-{letter}?"),
        }
    }

    /// The page shown, as a new symbol is drawn for it: a drawn page with its grid and its words.
    fn page_drawing(&self) -> Option<PageDrawing> {
        let id = self.current_page()?;
        let row = self
            .page_rows()
            .iter()
            .find(|r| crate::canvas::PageId::of(r) == id)?;
        let (num, den) = self.canvas()?.page().snapper.grid();
        Some(PageDrawing {
            page: row.id,
            folio: row.folio.clone(),
            words: self.page_words(row.id)?,
            grid: (num.unsigned_abs(), den.unsigned_abs()),
            exchange: None,
        })
    }

    /// The discipline a symbol for page `page` is drawn for: pneumatic on a fluid page, else electrical.
    fn page_discipline(&self, page: Uuid) -> &'static str {
        let fluid = self
            .page_rows()
            .iter()
            .find(|r| r.id == page)
            .is_some_and(|r| r.kind == ew_api::PageType::Fluid);
        if fluid { "pneumatic" } else { "electrical" }
    }

    /// Opens `draft` as the drawing of the place, for `purpose`: the place shows over the project (entered when it was
    /// not), on its Drawing tab, with the drawing grid, Snap and the tool he left the last time; `home` is the library
    /// it is saved back to (his own, writable; `None`: My parts), `locked` why it is not his to change.
    pub(crate) fn open_drawing(
        &mut self,
        draft: SymbolDraft,
        purpose: Drawing,
        home: Option<Uuid>,
        locked: Option<String>,
    ) {
        let mut editor = SymbolEditor::new(draft);
        let (grid, snap) = (
            self.settings.symbol_drawing_grid.unwrap_or(0),
            self.settings.symbol_snap.unwrap_or(true),
        );
        editor.grid = grid.min(super::GRID_STEPS.len() - 1);
        editor.snap = snap;
        if let Some(before) = &self.symbol_editor {
            editor.tool = before.tool;
        }
        editor.purpose = purpose;
        editor.home = home;
        editor.locked = locked;
        self.symbol_editor_generation = self.symbol_editor_generation.wrapping_add(1);
        editor.generation = self.symbol_editor_generation;
        self.symbol_editor = Some(editor);
        self.tools.picker = None;
        self.enter_place();
        if let Some(place) = self.parts.as_deref_mut() {
            place.aside = false;
        }
    }

    /// `parts.new_symbol {like?}` (New symbol: the Draw tab and `Ctrl+I`, the symbol picker's foot, the Part tab of
    /// Parts): a new drawing at once, with no question first. From a page it is drawn for that page, on its connection
    /// grid, and its next step places it there; in Parts it is a symbol of his own. `like`: `last`, a copy of the symbol
    /// he saved last, its name ready to type over; `selected` (Draw one like it…), a copy of the symbol selected on the
    /// page, which Save then exchanges; `hand`, the drawing in hand again.
    pub(crate) fn act_parts_new_symbol(&mut self, p: &Map<String, Value>) -> Outcome {
        let like = match string_param(p, "like") {
            Ok(l) => l.map(str::to_owned),
            Err(e) => return e,
        };
        if like.as_deref() == Some("hand") {
            return match &mut self.symbol_editor {
                Some(editor) => {
                    editor.shown = true;
                    self.enter_place();
                    if let Some(place) = self.parts.as_deref_mut() {
                        place.aside = false;
                    }
                    Outcome::Done
                }
                None => Outcome::refused(key!("reason-no-drawing-in-hand")),
            };
        }
        if let Some(out) = self.in_hand_asks("parts.new_symbol", p) {
            return out;
        }
        // From a page shown in the project's window; in Parts a symbol of his own.
        let page = (self.parts.is_none() || self.parts.as_deref().is_some_and(|p| p.aside))
            .then(|| self.page_drawing())
            .flatten();
        let made = match like.as_deref() {
            None => {
                let (grid, discipline) = match &page {
                    Some(p) => (p.grid, self.page_discipline(p.page)),
                    None => ((4_000_000, 1), "electrical"),
                };
                let what = what_for(self.settings.symbol_last_kind.as_deref(), discipline);
                new_draft(grid, &what).map(|d| (d, None))
            }
            Some("last") => match self.settings.symbols_recent.first().cloned() {
                Some(address) => self.copy_draft(&address).map(|d| (d, None)),
                None => return Outcome::refused(key!("reason-no-symbol-saved")),
            },
            Some("selected") => match self.selected_symbol() {
                Some((rep, copy, tag)) => self.copy_draft(&copy.to_string()).map(|d| (d, Some((rep, tag)))),
                None => return Outcome::refused(key!("reason-no-symbol-selected")),
            },
            Some(_) => return failed(key!("error-bad-param"), &["like"]),
        };
        let (draft, exchange) = match made {
            Ok(made) => made,
            Err(message) => return Outcome::Failed { message },
        };
        let purpose = match page {
            Some(mut p) => {
                p.exchange = exchange;
                Drawing::Page(p)
            }
            None => Drawing::Symbol,
        };
        let discipline = match &purpose {
            Drawing::Page(p) => self.page_discipline(p.page),
            _ => "electrical",
        };
        let kept_what = like.is_none() && self.settings.symbol_last_kind.is_some();
        self.open_drawing(draft, purpose, None, None);
        if let Some(editor) = &mut self.symbol_editor {
            editor.chosen_discipline = Some(discipline);
            editor.kept_what = kept_what;
            // A new drawing has no name of his yet; a copy has its source's, ready to type over.
            editor.unnamed = like.is_none();
        }
        Outcome::Done
    }

    /// New symbol's menu (doc 07 §5.6): Like the last one (the symbol he saved last, by name), Like the selected one
    /// (Draw one like it…), and Go on with the drawing in hand; each as its `like`, its words and why it cannot be
    /// chosen now.
    pub fn new_symbol_starts(&self) -> Vec<(&'static str, String, Option<String>)> {
        let last = self.settings.symbols_recent.first().and_then(|address| {
            let registry = self.symbol_registry().ok()?;
            let draft = SymbolDraft::from_library(&registry, address, "user.symbol").ok()?;
            Some(name_words(&draft.symbol.name))
        });
        let hand = self
            .symbol_editor
            .as_ref()
            .filter(|e| e.in_hand())
            .map(|e| name_words(&e.draft.symbol.name));
        let selected = self.selected_symbol().map(|(_, _, tag)| tag);
        let with = |key, what: &Option<String>| match what {
            Some(w) => strings::format(key, &[w]),
            None => strings::format(key, &[""])
                .trim_end_matches([' ', '·'])
                .to_owned(),
        };
        vec![
            (
                "last",
                with(key!("new-symbol-like-last"), &last),
                last.is_none().then(|| words(key!("reason-no-symbol-saved"))),
            ),
            (
                "selected",
                with(key!("new-symbol-like-selected"), &selected),
                selected
                    .is_none()
                    .then(|| words(key!("reason-no-symbol-selected"))),
            ),
            (
                "hand",
                with(key!("new-symbol-in-hand"), &hand),
                hand.is_none().then(|| words(key!("reason-no-drawing-in-hand"))),
            ),
        ]
    }

    /// The symbol selected on the page: its representation, its symbol copy and its device's tag.
    fn selected_symbol(&self) -> Option<(Uuid, Uuid, String)> {
        let session = self.project_session()?;
        let q = session.query();
        self.tools.selection.items.iter().find_map(|e| {
            let rep = q.reps().get(e.0.to_string().parse().ok()?)?;
            let device = rep
                .functions
                .first()
                .and_then(|f| q.functions().get(*f))
                .and_then(|f| f.device)?;
            let tag = session.device_tag(device.as_uuid())?;
            Some((e.0, rep.symbol.as_uuid(), tag))
        })
    }

    /// A copy of the symbol `source` (a library address, a starter key or a copy the project holds) as a new symbol
    /// of his own: a key from its name, no identity of its source, its name ready to type over.
    fn copy_draft(&self, source: &str) -> Result<SymbolDraft, String> {
        let key = "user.symbol";
        let mut draft = match self.parts_session() {
            Some(session) => session.symbol_draft(source, key),
            None => SymbolDraft::from_library(&self.symbol_registry()?, source, key),
        }
        .map_err(symbol_error)?;
        draft.identity = None;
        draft.key = super::key_of_name(&name_words(&draft.symbol.name));
        Ok(draft)
    }

    /// The libraries a drawing reads and saves through: the configured tiers', else the settings folder's.
    fn symbol_registry(&self) -> Result<ew_api::library::Registry, String> {
        match self.symbol_tiers() {
            Some(tiers) => ew_api::library::Registry::open(&tiers).map_err(|e| e.to_string()),
            None => Ok(ew_api::library::Registry::starter_only()),
        }
    }

    /// `parts.draw {what?, part?, piece?, from?, done?}` (Draw a new symbol… and Change it for this part… of a piece's
    /// Symbol cell, Arrange… of a box, the place's Drawing tab): `what` `box` arranges the open part's box (`part` opens
    /// Parts on that part first); else the drawing of piece `piece` of the open part, `from` `new` (an empty drawing of
    /// the piece's function, its pins standing on it already) or `change` (a copy of the symbol that draws the piece
    /// now); without a piece, the drawing in hand on the Drawing tab again. `done` leaves the drawing for the Part tab,
    /// the drawing in hand kept.
    pub(crate) fn act_parts_draw(&mut self, p: &Map<String, Value>) -> Outcome {
        if crate::shell::flag(p, "done") {
            if let Some(editor) = &mut self.symbol_editor {
                editor.shown = false;
            }
            return Outcome::Done;
        }
        match string_param(p, "what") {
            Ok(Some("box")) => {
                let part = string_param(p, "part").ok().flatten().map(str::to_owned);
                return self.arrange_box(part.as_deref());
            }
            Ok(Some("symbol") | None) => {}
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["what"]),
            Err(e) => return e,
        }
        let Some(piece) = p.get("piece").and_then(Value::as_u64) else {
            return match &mut self.symbol_editor {
                Some(editor) if self.parts.is_some() => {
                    editor.shown = true;
                    Outcome::Done
                }
                _ => Outcome::refused(key!("reason-no-drawing-in-hand")),
            };
        };
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        if !open.editable() {
            return Outcome::refused(key!("reason-part-not-yours"));
        }
        let i = usize::try_from(piece).unwrap_or(usize::MAX);
        let Some(row) = open.sheet.pieces.get(i).cloned() else {
            return failed(key!("error-bad-param"), &["piece"]);
        };
        let of = PieceDrawing {
            index: i,
            piece: piece_words(&row),
            part: open.title(),
        };
        if let Some(out) = self.in_hand_asks("parts.draw", p) {
            return out;
        }
        let Some(session) = self.parts_session() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        let made = match string_param(p, "from") {
            Ok(Some("change")) => {
                let source = if row.symbol.is_empty() {
                    match ew_api::output::starter_piece_symbol(&row.what) {
                        Some(key) => key,
                        None => return Outcome::refused(key!("reason-piece-no-symbol")),
                    }
                } else {
                    row.symbol.clone()
                };
                // His copy draws the piece's function, so that the piece's Symbol cell offers it.
                self.copy_draft(&source).map(|mut d| {
                    d.function_ref = ew_api::symbols::piece_function(&row.what);
                    d
                })
            }
            Ok(Some("new") | None) => session.piece_drawing(&row, &of.piece).map_err(symbol_error),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["from"]),
            Err(e) => return e,
        };
        match made {
            Ok(mut draft) => {
                // Its name follows from the piece and the part ("Coil, rectangular A1, A2 of MY-CON-9"), its key from
                // the name: neither is asked.
                let name = strings::format(key!("parts-piece-symbol-name"), &[&of.piece, &of.part]);
                if let Ok(named) = SymbolDraft::new("", &name) {
                    draft.symbol.name = named.symbol.name;
                }
                draft.key = super::key_of_name(&name);
                self.open_drawing(draft, Drawing::Piece(of), None, None);
                Outcome::Done
            }
            Err(message) => Outcome::Failed { message },
        }
    }

    /// `parts.open {symbol}`: the symbol at `address` (a library address, `id:<copy>` of the project's, or a key) in the
    /// drawing of the place, as a symbol of his own: his own (My parts, or another library he can write) is changed
    /// where it lives; another reads as it is, with Make my own from it. A symbol a library holds opens as that library
    /// holds it now, whatever version the project embeds; the project's copy only where no library has it.
    pub(crate) fn open_symbol(&mut self, address: &str) -> Outcome {
        let registry = match self.symbol_registry() {
            Ok(r) => r,
            Err(message) => return Outcome::Failed { message },
        };
        let current = self.library_symbol_address(address, &registry);
        let address = current.as_deref().unwrap_or(address);
        let source = address.strip_prefix("id:").unwrap_or(address);
        // The library it lives in, named by its address; none for a copy only the project holds.
        let home = address
            .strip_prefix("library:")
            .and_then(|r| r.split_once(':'))
            .and_then(|(l, _)| l.parse::<Uuid>().ok());
        let found = home.and_then(|l| registry.libraries().into_iter().find(|i| i.id == l));
        // It keeps its own key: the library's document's, the project's copy's, else the key it was named by.
        let key_of = |s: &schema::SymbolDef| s.meta.key.as_ref().map(|k| k.as_str().to_owned());
        let own_key = address
            .strip_prefix("library:")
            .and_then(|r| r.split_once(':'))
            .and_then(|(l, i)| {
                let (l, i) = (l.parse::<Uuid>().ok()?, i.parse::<Uuid>().ok()?);
                registry
                    .documents(l, schema::Kind::Symbol)
                    .into_iter()
                    .find_map(|d| match d {
                        schema::LibDocument::Symbol(s) if s.item_id.as_uuid() == i => key_of(&s.item),
                        _ => None,
                    })
            })
            .or_else(|| {
                let session = self.parts_session()?;
                let q = session.query();
                let held = q
                    .symbols()
                    .iter()
                    .find(|(id, _)| id.as_uuid().to_string() == source)?;
                key_of(&held.1.item)
            })
            .unwrap_or_else(|| source.to_owned());
        let draft = match self.parts_session() {
            Some(session) => session.symbol_draft(source, &own_key),
            None => SymbolDraft::from_library(&registry, source, &own_key),
        };
        let mut draft = match draft {
            Ok(d) => d,
            Err(e) => {
                return Outcome::Failed {
                    message: symbol_error(e),
                };
            }
        };
        let (home, locked) = match found {
            Some(l) if l.writable => (Some(l.id), None),
            Some(l) => (
                None,
                Some(if l.id == schema::starter::LIBRARY {
                    words(key!("parts-symbol-lock-starter"))
                } else {
                    strings::format(key!("parts-symbol-lock-library"), &[&l.name])
                }),
            ),
            None => (None, Some(words(key!("parts-symbol-lock-project")))),
        };
        if home.is_none() {
            draft.identity = None;
        }
        self.open_drawing(draft, Drawing::Symbol, home, locked);
        if let Some(editor) = &mut self.symbol_editor {
            editor.stored = home.is_some();
            editor.opened_from = Some(address.to_owned());
        }
        Outcome::Done
    }

    /// The library address (`library:<library>:<item>`) of the symbol `address` names where an installed library holds
    /// it: the item a project's copy (`id:<copy>`) was taken from, or the symbol of a key; `None` for a library address
    /// already, and for a symbol no library holds.
    fn library_symbol_address(&self, address: &str, registry: &ew_api::library::Registry) -> Option<String> {
        if address.starts_with("library:") {
            return None;
        }
        let held = address.strip_prefix("id:").and_then(|copy| {
            let session = self.parts_session()?;
            let q = session.query();
            let (_, s) = q
                .symbols()
                .iter()
                .find(|(id, _)| id.as_uuid().to_string() == copy)?;
            Some((s.library_id.as_uuid(), s.item_id.as_uuid()))
        });
        registry.libraries().into_iter().find_map(|l| {
            registry
                .documents(l.id, schema::Kind::Symbol)
                .into_iter()
                .find_map(|d| match d {
                    schema::LibDocument::Symbol(s) => {
                        let is = match held {
                            Some((library, item)) => {
                                s.library_id.as_uuid() == library && s.item_id.as_uuid() == item
                            }
                            None => s.item.meta.key.as_ref().is_some_and(|k| k.as_str() == address),
                        };
                        is.then(|| ew_api::part_pieces::library_address(&s))
                    }
                    _ => None,
                })
        })
    }

    /// The parts a page draws with the drawing's symbol (its sheet's Parts drawn with it, doc 07 §5.6): a symbol a
    /// library holds, saved or opened from there; none for a drawing no library holds yet.
    pub fn drawing_parts(&self) -> Vec<ew_api::piece_symbols::DrawnPart> {
        let Some(editor) = self.symbol_editor() else {
            return Vec::new();
        };
        let address = editor
            .published()
            .map(str::to_owned)
            .or_else(|| editor.opened_from.clone());
        let Some(session) = self.parts_session() else {
            return Vec::new();
        };
        // The symbol by its library address where it has one (a starter's key names none: its function finds them).
        let symbol = address.as_deref().and_then(address_ref);
        let fndef = match symbol.as_ref().map(|s| session.libraries().resolve(s)) {
            Some(Ok(schema::LibDocument::Symbol(s))) => s.item.fndef,
            _ => editor.draft.symbol.fndef.clone(),
        };
        session.parts_drawn_with(symbol.as_ref(), &fndef)
    }

    /// `parts.use_for_all {apply?, cancel?}` (Use it for every part with this piece…, on the strip of a piece's
    /// drawing): the symbol saved, its card names his own parts with a piece of that function and that many pins
    /// ([`ew_api::Session::parts_with_piece`]); `apply` writes them with the symbol and brings the project's copies to
    /// them in one undo step; `cancel` closes the card.
    pub(crate) fn act_parts_use_for_all(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(Drawing::Piece(of)) = self.symbol_editor().map(|e| e.purpose.clone()) else {
            return Outcome::refused(key!("reason-no-piece-drawing"));
        };
        if crate::shell::flag(p, "cancel") {
            if let Some(editor) = &mut self.symbol_editor {
                editor.use_for_all = None;
            }
            return Outcome::Done;
        }
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        let Some(row) = open.sheet.pieces.get(of.index).cloned() else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        let except = open.sheet.source.map(|(_, item)| item);
        // The symbol is his and saved first: the parts name it by its address.
        let address = match self
            .symbol_editor()
            .and_then(|e| e.published().map(str::to_owned))
        {
            Some(a) => a,
            None => match self.symbol_save_checked() {
                Ok(a) => a,
                Err(message) => return Outcome::Failed { message },
            },
        };
        let (Some(symbol), Some(fndef)) = (address_ref(&address), ew_api::symbols::piece_function(&row.what))
        else {
            return failed(key!("error-bad-param"), &["piece"]);
        };
        let pins = row.pins.split(',').filter(|s| !s.trim().is_empty()).count();
        if !crate::shell::flag(p, "apply") {
            let Some(session) = self.parts_session() else {
                return Outcome::refused(key!("reason-no-parts-place"));
            };
            let parts = session.parts_with_piece(&fndef, pins, &symbol, except);
            if let Some(editor) = &mut self.symbol_editor {
                editor.use_for_all = Some(parts);
            }
            return Outcome::Done;
        }
        let Some(session) = self.parts_session_mut().0 else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        let count = session.parts_with_piece(&fndef, pins, &symbol, except).len();
        let plan = match session.use_symbol_for_pieces(&fndef, pins, &symbol, except) {
            Ok(plan) => plan,
            Err(message) => return Outcome::Failed { message },
        };
        if let Some(editor) = &mut self.symbol_editor {
            editor.use_for_all = None;
        }
        self.libraries_changed();
        if let Some(plan) = plan {
            let out = self.run_commit(&plan.command);
            if out != Outcome::Done {
                return out;
            }
        }
        self.notify(
            strings::format(key!("toast-symbol-used-for"), &[&count.to_string()]),
            false,
        );
        Outcome::Done
    }

    /// Make my own from it, on a drawing that is not his: the same drawing as a new symbol of his own, a key from its
    /// name, saved into My parts.
    pub(crate) fn own_drawing(&mut self) -> Outcome {
        let Some(editor) = &mut self.symbol_editor else {
            return Outcome::refused(key!("reason-no-drawing-in-hand"));
        };
        if editor.locked.take().is_none() {
            return Outcome::refused(key!("reason-symbol-is-yours"));
        }
        editor.draft.identity = None;
        editor.draft.key = super::key_of_name(&name_words(&editor.draft.symbol.name));
        editor.home = None;
        editor.stored = false;
        editor.saved = None;
        Outcome::Done
    }

    /// Discard on the drawing: its changes since it was opened or saved go.
    pub(crate) fn discard_drawing(&mut self) -> Outcome {
        let Some(editor) = &mut self.symbol_editor else {
            return Outcome::refused(key!("reason-no-drawing-in-hand"));
        };
        let Some(saved) = editor.saved.clone().filter(|s| *s != editor.draft) else {
            return Outcome::refused(key!("reason-part-not-changed"));
        };
        editor.replace(saved);
        Outcome::Done
    }

    /// Before a way in replaces the drawing in hand that has changes not saved: the window asks Save symbol, Discard
    /// or Cancel (the action then runs with `discard`), a script is refused in words; `None` where the action goes on.
    pub(crate) fn in_hand_asks(&mut self, action: &'static str, p: &Map<String, Value>) -> Option<Outcome> {
        let unsaved = self.symbol_editor.as_ref().is_some_and(SymbolEditor::dirty);
        if !unsaved || crate::shell::flag(p, "discard") {
            return None;
        }
        Some(match self.mode {
            crate::shell::Mode::Headless => failed(key!("symbol-unsaved"), &[]),
            crate::shell::Mode::Gui => self.open_dialog(crate::shell::Dialog::ConfirmDiscard {
                pending: crate::shell::Pending {
                    action,
                    params: Value::Object(p.clone()),
                    cancelled: None,
                },
                symbol: true,
            }),
        })
    }

    /// `parts.save` on the drawing: the symbol saved with his parts, then its errand finished: placed on the page it
    /// was drawn for with the Place tool armed (`Save and place on 102`), exchanged for the symbol it was drawn like
    /// (`Save and use for -102S1`), given to the piece it was drawn for (`Done`); a symbol of his own stays open.
    pub(crate) fn save_drawing(&mut self) -> Outcome {
        let Some(editor) = &self.symbol_editor else {
            return Outcome::refused(key!("reason-no-drawing-in-hand"));
        };
        if editor.device_box.is_some() {
            return self.close_box();
        }
        let purpose = editor.purpose.clone();
        let address = match self.symbol_save_checked() {
            Ok(a) => a,
            Err(message) => return Outcome::Failed { message },
        };
        // What it is, kept for the next symbol he draws.
        if let Some(what) = self.symbol_editor.as_ref().and_then(SymbolEditor::what) {
            self.settings.symbol_last_kind = Some(what);
            self.save_settings();
        }
        match purpose {
            Drawing::Symbol => Outcome::Done,
            Drawing::Piece(p) => {
                if let Some(editor) = &mut self.symbol_editor {
                    editor.shown = false;
                }
                self.dispatch(
                    "parts.field",
                    &json!({"field": format!("pieces[{}].symbol", p.index), "value": address}),
                )
            }
            Drawing::Page(page) => self.place_saved(&address, &page),
        }
    }

    /// The symbol in hand saved with his parts, as every Save of it does (the strip's, Ctrl+S and File ▸ Save through
    /// [`ShellModel::save_drawing`], a question's Save, Use it for every part): refused in words while it is not his
    /// (Make my own from it) or To do lists a step, the refusal shown on the drawing. Its address.
    pub(crate) fn symbol_save_checked(&mut self) -> Result<String, String> {
        let editor = self
            .symbol_editor
            .as_ref()
            .ok_or_else(|| words(key!("reason-no-drawing-in-hand")))?;
        let saved = match editor.save_refusal() {
            Some(why) => Err(why),
            None => self.symbol_save(),
        };
        if let Err(message) = &saved {
            if let Some(editor) = &mut self.symbol_editor {
                editor.error = Some(message.clone());
            }
            self.said_at(crate::shell::said::Said::Editor);
        }
        saved
    }

    /// The saved symbol at `address` on the page it was drawn for: the place gives way to the page, and the Place tool
    /// is armed with the symbol (on the page's grid, as the picker places one of another grid); drawn like a placed
    /// symbol, that symbol is exchanged for it instead, its wires kept.
    fn place_saved(&mut self, address: &str, page: &PageDrawing) -> Outcome {
        let leaves = self.drawing_leaves_parts();
        if let Some(editor) = &mut self.symbol_editor {
            editor.shown = false;
        }
        if leaves {
            self.parts = None;
            self.parts_alone = None;
        } else {
            // A part open in Parts waits aside, its typing kept, as after a test placement.
            self.step_aside();
        }
        self.show_page_by_id(&page.page.to_string());
        self.sync_views();
        self.sync_tool_context();
        if let Some((rep, _)) = &page.exchange {
            return self.exchange_saved(address, *rep);
        }
        self.dispatch("place.pick", &json!({ "symbol": address }))
    }

    /// Exchanges representation `rep` for the saved symbol at `address` (`rep.exchange_symbol` with the symbol and its
    /// function embedded), its wires kept by the exchange's rule.
    fn exchange_saved(&mut self, address: &str, rep: Uuid) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(reference) = address_ref(address) else {
            return failed(key!("error-bad-param"), &["symbol"]);
        };
        let (library, item) = (reference.library.as_uuid(), reference.item.as_uuid());
        let docs = match project.session.libraries().closure(&reference) {
            Ok(d) => d,
            Err(e) => {
                return Outcome::Failed {
                    message: e.to_string(),
                };
            }
        };
        let symbol = ew_api::output::v5::lib_copy_id::<schema::SymbolDef>(library, item);
        let q = project.session.query();
        let copies: Vec<Value> = docs
            .iter()
            .filter(|d| match d {
                schema::LibDocument::Symbol(s) => !q.symbols().contains(s.id),
                schema::LibDocument::Fndef(f) => q.fndefs().get(f.id).is_none(),
                _ => false,
            })
            .filter_map(|d| serde_json::to_value(d).ok())
            .collect();
        drop(q);
        let command = ew_api::AnyCommand {
            name: "rep.exchange_symbol".to_owned(),
            params: json!({"rep": rep.to_string(), "symbol": symbol.as_uuid().to_string(), "copies": copies}),
        };
        self.run_commit(&command)
    }

    /// The Symbols shelf (doc 07 §5.3): the symbols of the installed libraries and those only in the project, whose
    /// name or what it is holds every word of `query`, in `scope` (`project`: those the project holds; `mine`: his
    /// own); those he drew lately first, the rest by name.
    pub(crate) fn symbols_found(
        &self,
        query: &str,
        scope: ew_api::parts::PartScope,
    ) -> Vec<super::super::parts_place::SymbolFound> {
        use super::super::parts_place::SymbolFound;
        use ew_api::parts::PartScope;
        use schema::{Kind, LibDocument};
        let Some(session) = self.parts_session() else {
            return Vec::new();
        };
        let registry = session.libraries();
        let q = session.query();
        let what = |fndef: &schema::LibRef| {
            let category = match registry.resolve(fndef) {
                Ok(LibDocument::Fndef(f)) => f.item.category.0.as_str().to_owned(),
                _ => String::new(),
            };
            crate::tools::category_words(&category)
                .unwrap_or_default()
                .to_owned()
        };
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let hits = |name: &str, what: &str| {
            let text = format!("{name} {what}").to_lowercase();
            terms.iter().all(|w| text.contains(w))
        };
        let recent = &self.settings.symbols_recent;
        let mut out: Vec<SymbolFound> = Vec::new();
        for library in registry.libraries() {
            if scope == PartScope::Mine && !library.writable {
                continue;
            }
            for doc in registry.documents(library.id, Kind::Symbol) {
                let LibDocument::Symbol(s) = doc else { continue };
                let address = ew_api::part_pieces::library_address(&s);
                let copy = ew_api::output::v5::lib_copy_id::<schema::SymbolDef>(
                    s.library_id.as_uuid(),
                    s.item_id.as_uuid(),
                );
                if scope == PartScope::Project && !q.symbols().contains(copy) {
                    continue;
                }
                let name = name_words(&s.item.name);
                let what = what(&s.item.fndef);
                if !hits(&name, &what) {
                    continue;
                }
                let lives = if library.writable {
                    words(key!("parts-lives-mine"))
                } else if library.id == schema::starter::LIBRARY {
                    words(key!("parts-lives-starter"))
                } else {
                    library.name.clone()
                };
                out.push(SymbolFound {
                    recent: recent.contains(&address),
                    address,
                    name,
                    what,
                    lives,
                    mine: library.writable,
                });
            }
        }
        // The project's own symbols that no installed library holds (an import's, a colleague's).
        if scope != PartScope::Mine {
            for (id, s) in q.symbols().iter() {
                let held = registry
                    .libraries()
                    .iter()
                    .any(|l| l.id == s.library_id.as_uuid());
                let name = name_words(&s.item.name);
                let what = what(&s.item.fndef);
                if held || !hits(&name, &what) {
                    continue;
                }
                out.push(SymbolFound {
                    address: format!("id:{}", id.as_uuid()),
                    name,
                    what,
                    lives: words(key!("parts-lives-here")),
                    mine: false,
                    recent: false,
                });
            }
        }
        out.sort_by_cached_key(|s| {
            let at = recent.iter().position(|a| *a == s.address).unwrap_or(usize::MAX);
            (at, s.name.to_lowercase())
        });
        out
    }

    /// `parts.back` on the drawing: from a page, back to it without the symbol, the drawing kept in hand (New symbol
    /// offers to go on with it); from a piece, back to the part; a symbol of his own, back to the list.
    pub(crate) fn back_from_drawing(&mut self) -> Outcome {
        if self
            .symbol_editor
            .as_ref()
            .is_some_and(|e| e.device_box.is_some())
        {
            return self.close_box();
        }
        let leaves = self.drawing_leaves_parts();
        let Some(editor) = &mut self.symbol_editor else {
            return Outcome::refused(key!("reason-no-drawing-in-hand"));
        };
        editor.shown = false;
        if leaves {
            self.parts = None;
            self.parts_alone = None;
        }
        Outcome::Done
    }

    /// Whether leaving the drawing (Back, Save and place) leaves Parts too: it is a page's drawing, Parts was opened
    /// for it from the project, and no part is open in Parts; else Parts stays, a part open in it with its typing.
    fn drawing_leaves_parts(&self) -> bool {
        self.symbol_editor
            .as_ref()
            .is_some_and(|e| matches!(e.purpose, Drawing::Page(_)))
            && (self.parts.as_deref()).is_some_and(|p| p.origin == Origin::Project && p.sheet.is_none())
    }
}
