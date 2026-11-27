// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The drawing of a symbol in Parts (doc 07 §5.6): its draft, its history and its actions. Where the drawing opens
//! and what its next step does is [`place`]'s.

use super::{Mode, Outcome, Prompt, ShellModel, failed};
use crate::notifications::{Note, NoteTone, back_to_editor};
use crate::{key, strings};
use ew_api::symbols::{self, SymbolDraft};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

mod device_box;
mod place;
mod selection;
mod sheet;
pub use device_box::{DeviceBox, PinLine, SIDES, side_word, side_words};
pub use place::{Drawing, PageDrawing, PieceDrawing};
pub use selection::{Clipboard, Item, bounds, grips, role_words};
pub use sheet::{Outline, OutlineGroup, OutlineRow, RowValue, Sheet, SheetRow, Todo, trim_mm, what_kinds};

/// The editor's drawing tools, in the order of its bar.
pub const TOOLS: [&str; 13] = [
    "select",
    "line",
    "rectangle",
    "circle",
    "ellipse",
    "arc",
    "text",
    "cp",
    "slot",
    "anchor",
    "link",
    "polyline",
    "polygon",
];

/// The drawing grids offered, each as nanometres per steps (0.5, 1, 2 and 4 mm and QElectroTech's 10 px), the first
/// the default. Connection points keep to the symbol's connection grid whatever is chosen.
pub const GRID_STEPS: [(i64, i64); 5] = [
    (500_000, 1),
    (1_000_000, 1),
    (2_000_000, 1),
    (4_000_000, 1),
    (
        ew_units::Grid::QET_10PX.num() as i64,
        ew_units::Grid::QET_10PX.den() as i64,
    ),
];

/// The potential roles a connection point takes, in the role list's order after "None".
pub const ROLES: [&str; 13] = [
    "l1",
    "l2",
    "l3",
    "l",
    "n",
    "pe",
    "pen",
    "fe",
    "l_plus",
    "l_minus",
    "m",
    "signal",
    "undefined",
];

/// The disciplines a symbol is drawn for, in the order the editor's Discipline list offers them.
pub const DISCIPLINES: [&str; 5] = ["electrical", "pneumatic", "vacuum", "hydraulic", "bus"];

/// The function types the editor offers for a symbol of `discipline` (one of [`DISCIPLINES`]), in the order of its list:
/// a valve for a fluid symbol, a contact or a coil for an electrical one, a port for a bus one; a black box for each.
pub fn function_kinds(discipline: &str) -> &'static [&'static str] {
    match discipline {
        "pneumatic" | "vacuum" | "hydraulic" => &["valve", "pole", "black_box"],
        "bus" => &["bus_port", "black_box"],
        _ => &[
            "no",
            "nc",
            "co",
            "coil",
            "pole",
            "terminal",
            "plc_io",
            "black_box",
        ],
    }
}

/// The editor tools that take a binding of the keymap while the editor shows (doc 16a P8): the main window's drawing
/// action whose key chooses the tool. Escape (`ui.dismiss`) returns to Select.
pub const TOOL_KEYS: [(&str, &str); 5] = [
    ("select", "ui.dismiss"),
    ("line", "tool.graphic"),
    ("rectangle", "tool.black_box"),
    ("text", "tool.text"),
    ("cp", "tool.ip"),
];

/// The page's editing keys that act on the editor's selection while the editor shows (what a person learned on
/// a page works here): the main window's action whose key it is, the editor's action, and the direction of an arrow.
pub const EDIT_KEYS: [(&str, &str, (i8, i8)); 10] = [
    ("edit.delete_selection", "symbol.delete", (0, 0)),
    ("edit.cut", "symbol.cut", (0, 0)),
    ("edit.copy", "symbol.copy", (0, 0)),
    ("edit.paste_clip", "symbol.paste", (0, 0)),
    ("edit.duplicate", "symbol.duplicate", (0, 0)),
    ("edit.select_all", "symbol.select", (0, 0)),
    ("cursor.left", "symbol.nudge", (-1, 0)),
    ("cursor.right", "symbol.nudge", (1, 0)),
    ("cursor.up", "symbol.nudge", (0, -1)),
    ("cursor.down", "symbol.nudge", (0, 1)),
];

/// The files the symbol editor opens or imports, as its file picker offers them: the authoring JSON,
/// QElectroTech elements, every EPLAN format and DXF drawings (one draft per block) `symbols::open` reads.
pub const FILE_EXTENSIONS: [&str; 12] = [
    "json", "elmt", "edz", "pxf", "ema", "ems", "esl", "slk", "xml", "zip", "7z", "dxf",
];

/// Parts drawn with a symbol, read for its address and a project revision.
type DrawnParts = ((Option<String>, Option<u64>), Vec<ew_api::piece_symbols::DrawnPart>);

/// Editable document state. History never shares the project's undo stack.
#[derive(Debug)]
pub struct SymbolEditor {
    /// Replacement generation; the view clears transient tool state when this changes.
    pub generation: u64,
    /// Current portable document.
    pub draft: SymbolDraft,
    /// The draft as it was created, imported, exported or published; equality determines unsaved changes.
    saved: Option<SymbolDraft>,
    /// Whether that state was exported or published (an import or a new draft is in no library yet).
    stored: bool,
    /// Whether Parts shows the drawing (its Drawing tab); a drawing left is kept in hand with its history.
    pub(crate) shown: bool,
    /// What the drawing is drawn for: its strip and its next step.
    pub purpose: Drawing,
    /// The library Save saves it back to: his own that it came from; `None`, My parts.
    home: Option<ew_api::output::Uuid>,
    /// Why the drawing is not his to change (a symbol of the starter or of a library he cannot write): it reads as it
    /// is, with Make my own from it; `None` for his own.
    pub locked: Option<String>,
    /// Whether What it is is still the one taken from the symbol he saved last (a new drawing's start): the short
    /// sheet says so under it.
    pub kept_what: bool,
    /// Whether a new drawing has no name of his yet: Name shows empty and To do asks for one first (its key is made
    /// from its name, so no two symbols are called "New symbol" by default).
    pub unnamed: bool,
    /// The library address the drawing was opened from (`library:<library>:<item>`), whose parts its sheet lists.
    pub opened_from: Option<String>,
    /// The card of Use it for every part with this piece…: the parts it would change, before Apply.
    pub use_for_all: Option<Vec<ew_api::piece_symbols::DrawnPart>>,
    /// Parts drawn with it as last read, with the symbol's address and the project revision it was read for: every
    /// frame of the drawing shows it, and reading it scans every part of the libraries.
    drawn_parts: std::cell::RefCell<Option<DrawnParts>>,
    undo: Vec<SymbolDraft>,
    redo: Vec<SymbolDraft>,
    /// Current orientation of the live preview (A–H).
    pub variant: u8,
    /// The drawing tool chosen, one of [`TOOLS`].
    pub tool: &'static str,
    /// Last operation's error, also shown under the editor controls.
    pub error: Option<String>,
    /// Whether that error is the page's other connection grid, which offers Place on a new page with its grid.
    pub offer_new_page: bool,
    /// The draft as last published and the published symbol's picker address (`library:<library>:<item>`).
    published: Option<(SymbolDraft, String)>,
    /// The text style last chosen for a text of the draft, which a new text takes.
    pub text_style: Option<String>,
    /// Other drawings recovered from the same input; selecting one is an explicit replacement.
    pub candidates: Vec<SymbolDraft>,
    /// The discipline the symbol is drawn for while it has no connection point: its page's.
    chosen_discipline: Option<&'static str>,
    /// What is selected in the drawing, in the order it was selected: the outline's rows, the drawing and Properties
    /// show the same things.
    pub selection: Vec<Item>,
    /// The drawing grid lines, shapes, texts and link points snap to: an index into [`GRID_STEPS`]. The person's
    /// choice, kept from one drawing to the next (`Settings::symbol_drawing_grid`).
    pub grid: usize,
    /// Whether the drawing tools snap to the drawing grid (off: to 0.01 mm; `Settings::symbol_snap`).
    pub snap: bool,
    /// Counts the changes of the draft, so the window reads what it measured of the drawing again.
    pub revision: u64,
    /// A part's device box, arranged in the drawing's place; the symbol draft waits under it.
    device_box: Option<DeviceBox>,
}

/// The key a draft takes from its name while the user has given it none: `user.` and the name's letters and digits
/// in lower case, every other run of characters one hyphen (`New symbol` gives `user.new-symbol`).
fn key_of_name(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.chars().take(100).collect();
    let slug = slug.trim_end_matches('-');
    format!("user.{}", if slug.is_empty() { "symbol" } else { slug })
}

/// Whether the draft's key is the one its name gives, in a draft that no library holds yet: such a key follows the
/// name and gives way to a free one when the library holds it for another symbol.
fn key_from_name(draft: &SymbolDraft) -> bool {
    draft.identity.is_none() && draft.key == key_of_name(&name_words(&draft.symbol.name))
}

impl SymbolEditor {
    fn new(mut draft: SymbolDraft) -> Self {
        // A person names a symbol; its key is made from the name, never asked for.
        if draft.key.is_empty() && draft.identity.is_none() {
            draft.key = key_of_name(&name_words(&draft.symbol.name));
        }
        Self {
            generation: 0,
            draft: draft.clone(),
            saved: Some(draft.clone()),
            stored: false,
            shown: true,
            purpose: Drawing::Symbol,
            home: None,
            locked: None,
            kept_what: false,
            unnamed: false,
            opened_from: None,
            use_for_all: None,
            drawn_parts: std::cell::RefCell::new(None),
            undo: Vec::new(),
            redo: Vec::new(),
            variant: 0,
            tool: TOOLS[0],
            error: None,
            offer_new_page: false,
            published: None,
            text_style: None,
            candidates: Vec::new(),
            chosen_discipline: None,
            selection: Vec::new(),
            grid: 0,
            snap: true,
            revision: 0,
            device_box: None,
        }
    }

    /// Whether the editor holds work: a draft with something drawn, with unsaved changes, or one of a library.
    pub fn in_hand(&self) -> bool {
        self.dirty()
            || self.stored
            || !self.draft.symbol.graphics.is_empty()
            || !self.draft.symbol.cps.is_empty()
    }

    /// The discipline the symbol is drawn for (one of [`DISCIPLINES`]): the one most of its function's pins have (the
    /// first met among equals), else the one chosen last, else electrical.
    pub fn discipline(&self) -> &'static str {
        let mut counts = [0_usize; DISCIPLINES.len()];
        for pin in self.draft.function.cps.iter() {
            let word = serde_json::to_value(&pin.discipline).ok();
            if let Some(i) = DISCIPLINES
                .iter()
                .position(|d| word.as_ref().and_then(Value::as_str) == Some(*d))
            {
                counts[i] += 1;
            }
        }
        let most = counts.iter().copied().max().unwrap_or(0);
        if most == 0 {
            return self.chosen_discipline.unwrap_or(DISCIPLINES[0]);
        }
        let first = self.draft.function.cps.iter().find_map(|pin| {
            let word = serde_json::to_value(&pin.discipline).ok()?;
            let i = DISCIPLINES.iter().position(|d| word.as_str() == Some(*d))?;
            (counts[i] == most).then_some(DISCIPLINES[i])
        });
        first.unwrap_or(DISCIPLINES[0])
    }
    /// Whether the draft differs from its last saved state: a new, forked or imported draft is not, until edited.
    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.draft) || self.device_box.as_ref().is_some_and(DeviceBox::dirty)
    }
    /// Whether the draft, unchanged, was exported or published; an unedited new or imported draft is in no library.
    pub fn stored(&self) -> bool {
        self.stored && !self.dirty()
    }
    /// The key of the words naming the draft's state: unsaved changes, saved, or not saved yet.
    pub fn state_key(&self) -> strings::Key<'static> {
        if let Some(b) = &self.device_box {
            return if b.dirty() {
                key!("symbol-dirty")
            } else if b.draft.new {
                key!("symbol-not-saved")
            } else {
                key!("symbol-saved")
            };
        }
        if self.dirty() {
            key!("symbol-dirty")
        } else if self.stored {
            key!("symbol-saved")
        } else {
            key!("symbol-not-saved")
        }
    }
    /// The picker address of the published symbol when the draft is saved to a library and unchanged since:
    /// Place on a new page with its grid places it; otherwise the offer is Test on a new page.
    pub fn published(&self) -> Option<&str> {
        self.published
            .as_ref()
            .filter(|(draft, _)| *draft == self.draft)
            .map(|(_, address)| address.as_str())
    }
    /// Whether editor undo is available.
    pub fn can_undo(&self) -> bool {
        match &self.device_box {
            Some(b) => b.can_step(false),
            None => !self.undo.is_empty(),
        }
    }
    /// Whether editor redo is available.
    pub fn can_redo(&self) -> bool {
        match &self.device_box {
            Some(b) => b.can_step(true),
            None => !self.redo.is_empty(),
        }
    }
    fn replace(&mut self, mut draft: SymbolDraft) {
        // A key made from the name follows a new name; a key left empty is made from the name again.
        if draft.identity.is_none()
            && (draft.key.is_empty() || (draft.key == self.draft.key && key_from_name(&self.draft)))
        {
            draft.key = key_of_name(&name_words(&draft.symbol.name));
        }
        if self.draft == draft {
            return;
        }
        self.undo.push(std::mem::replace(&mut self.draft, draft));
        // Bounded document history; a document is at most 4 MiB.
        if self.undo.len() > 16 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.error = None;
        self.revision = self.revision.wrapping_add(1);
        self.prune_selection();
    }
}

/// The direction a wire leaves a connection point at `pos` (nm) in: the way the line or path that ends nearest it,
/// within half a connection-grid step, points out of its other end (`up`, `right`, `down`, `left`); `None` where no
/// lead ends there.
fn lead_direction(draft: &SymbolDraft, [x, y]: [i64; 2]) -> Option<&'static str> {
    use symbols::schema::Primitive;
    let reach = draft.symbol.grid.at(1).map_or(2_000_000, |n| n.0 / 2);
    let xy = |p: &symbols::units::Pt2| (p.x.0, p.y.0);
    let ends = draft.symbol.graphics.iter().flat_map(|g| match g {
        Primitive::Line(l) => vec![(xy(&l.to), xy(&l.from)), (xy(&l.from), xy(&l.to))],
        Primitive::Polyline(l) if l.points.len() >= 2 => {
            let n = l.points.len();
            vec![
                (xy(&l.points[n - 1]), xy(&l.points[n - 2])),
                (xy(&l.points[0]), xy(&l.points[1])),
            ]
        }
        _ => Vec::new(),
    });
    let (end, other) = ends
        .map(|(end, other)| ((end.0 - x).abs().max((end.1 - y).abs()), end, other))
        .filter(|(d, ..)| *d <= reach)
        .min_by_key(|(d, ..)| *d)
        .map(|(_, end, other)| (end, other))?;
    let (dx, dy) = (end.0 - other.0, end.1 - other.1);
    Some(if dx.abs() >= dy.abs() {
        if dx >= 0 { "right" } else { "left" }
    } else if dy > 0 {
        "down"
    } else {
        "up"
    })
}

/// The name of `draft`'s symbol in the window's language.
pub fn display_name(draft: &SymbolDraft) -> String {
    name_words(&draft.symbol.name)
}

/// The style a new free text takes when none was chosen in the draft: the small style pin designations use
/// (`ew_scene`'s default for a connection point's designation).
pub const NEW_TEXT_STYLE: &str = "cp.small";

/// A text style's name in the window's words, as the Text styles page names it (`ew_derive::text::text_style_name`); a
/// project's own style by its key, so that two of them stay apart in the choice.
pub fn text_style_words(key: &str) -> String {
    let name = |key: &str| ew_api::checks::messages::text_style_name(key, strings::rule_language());
    let words = name(key);
    if words == name("") && !key.starts_with("qet.") {
        key.to_owned()
    } else {
        words
    }
}

/// A suffix that only the same content repeats: FNV-1a over the draft's document, without its key and identity.
fn content_suffix(draft: &SymbolDraft) -> String {
    let mut d = draft.clone();
    d.key.clear();
    d.identity = None;
    let bytes = serde_json::to_vec(&d).unwrap_or_default();
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}")
}

/// The words of a symbol's name in the window's language (the dialog's and the scratch page's titles), else its
/// English text, else its first.
pub(crate) fn name_words(name: &symbols::MlText) -> String {
    let language = strings::locale().split('-').next().unwrap_or("en");
    match name.view() {
        symbols::MlTextView::Neutral(s) => s.to_owned(),
        symbols::MlTextView::Langs(map) => map
            .iter()
            .find(|(k, _)| k.as_str() == language)
            .or_else(|| map.iter().find(|(k, _)| k.as_str() == "en"))
            .or_else(|| map.iter().next())
            .map(|(_, v)| v.to_string())
            .unwrap_or_default(),
    }
}

fn words(key: strings::Key<'static>) -> String {
    strings::text(key).to_owned()
}

/// A draft whose own document lacks a member it always has: in words, never the member's name.
fn damaged() -> String {
    words(key!("symbol-draft-damaged"))
}

fn text<'a>(p: &'a Map<String, Value>, key: &str) -> &'a str {
    p.get(key).and_then(Value::as_str).unwrap_or("")
}
/// The coordinate `name` of `p` in nanometres: a number of millimetres, or a length typed as every typed length is
/// read (`parse_mm`), within a kilometre of the origin.
fn nm(p: &Map<String, Value>, name: &str) -> Result<i64, String> {
    const LIMIT_NM: f64 = 1_000_000.0 * 1_000_000.0;
    let value = p
        .get(name)
        .and_then(|v| {
            v.as_f64()
                .map(|mm| mm * 1_000_000.0)
                .or_else(|| ew_api::output::parse_mm(v.as_str()?).map(|nm| nm as f64))
        })
        .ok_or_else(|| strings::text(key!("symbol-coordinate-required")).to_owned())?;
    if !value.is_finite() || value.abs() > LIMIT_NM {
        return Err(strings::text(key!("symbol-geometry-limit")).into());
    }
    Ok(value.round() as i64)
}

/// A symbol error in the window's words: an invalid draft's issues one per line, a refusal of the reader in its
/// words, a taken key named, anything else as it reads.
fn symbol_error(e: symbols::Error) -> String {
    match e {
        symbols::Error::Invalid(issues) => issues
            .iter()
            .map(|i| strings::format(strings::Key::run_time(i.code), &[&i.detail]))
            .collect::<Vec<_>>()
            .join("\n"),
        // A DXF reader's refusal reads in English here, as it always has: the open gives it its own words
        // (`file_error`).
        symbols::Error::Refused(t) if !t.key().starts_with("dxf-") => super::text_words(&t),
        symbols::Error::KeyTaken { key, function } => strings::format(
            if function {
                key!("symbol-function-key-taken")
            } else {
                key!("symbol-key-taken")
            },
            &[&key],
        ),
        e => e.to_string(),
    }
}

/// The error of reading or writing a symbol file in the window's words: the reader's or the system's own message
/// (a parser's line and column, an operating system's error text) gives way to `words`, which say what a person
/// can do; a refusal that is already in words stays.
fn file_error(e: symbols::Error, words: strings::Key<'static>) -> String {
    match e {
        symbols::Error::Input(_) => strings::text(words).to_owned(),
        symbols::Error::Refused(t) if t.key().starts_with("dxf-") => strings::text(words).to_owned(),
        e => symbol_error(e),
    }
}

/// The error of an edit to the draft in the window's words.
// An edit the reader refuses is refused in words, never in the parser's (a text with no
// characters, say).
fn edit_error(e: symbols::Error) -> String {
    match e {
        symbols::Error::Input(_) => words(key!("symbol-change-invalid")),
        symbols::Error::Refused(t) if !t.key().starts_with("symbol-") => words(key!("symbol-change-invalid")),
        e => symbol_error(e),
    }
}

impl ShellModel {
    /// The drawing while Parts shows it, on its Drawing tab.
    pub fn symbol_editor(&self) -> Option<&SymbolEditor> {
        self.symbol_editor
            .as_ref()
            .filter(|e| e.shown && self.parts.as_deref().is_some_and(|p| !p.aside))
    }

    /// The drawing in hand with its history, also while Parts does not show it.
    pub fn kept_symbol_editor(&self) -> Option<&SymbolEditor> {
        self.symbol_editor.as_ref()
    }

    /// The tiers a symbol is saved through: the configured ones, else the settings folder's, else the user's.
    fn symbol_tiers(&self) -> Option<ew_api::library::LibraryTiers> {
        // Without configured tiers, the settings folder's, as a remembered company path gives; never tiers or an
        // index in the working directory or beside the library.
        let config_dir = self.config.config_dir.as_deref();
        self.configured_libraries()
            .or_else(|| {
                config_dir.map(|d| {
                    ew_api::library::LibraryTiers::under(d, &d.join("library-index"), Vec::new(), Vec::new())
                })
            })
            .or_else(|| ew_api::library::LibraryTiers::for_user(Vec::new(), Vec::new()))
    }

    /// A path the user typed or picked: refused in words when empty, before any file system call; a relative path
    /// resolves against the default user library's folder, never the working directory.
    fn symbol_path(&self, text: &str, required: strings::Key<'static>) -> Result<PathBuf, String> {
        if text.trim().is_empty() {
            return Err(strings::text(required).into());
        }
        let path = Path::new(text);
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }
        let base = self
            .symbol_tiers()
            .ok_or_else(|| strings::text(required).to_owned())?
            .user;
        std::path::absolute(base.join(path)).map_err(|e| e.to_string())
    }

    /// Text styles used by both the project and the editor preview.
    pub fn symbol_text_styles(&self) -> symbols::TextStyleTable {
        self.project
            .as_ref()
            .map_or_else(ew_api::output::new_project_text_style_table, |p| {
                ew_api::output::text_style_table(&p.session.query())
            })
    }

    /// The drawing's actions: an import, its history, its tools and selection, a test placement.
    pub(crate) fn act_symbol(&mut self, action: &str, p: &Map<String, Value>) -> Outcome {
        // In the window a missing path opens the house file picker.
        let prompt = match action {
            "open" => Some(("symbol.open", Prompt::OpenSymbol, "file")),
            "export" => Some(("symbol.export", Prompt::SaveSymbol, "file")),
            _ => None,
        };
        if let Some((name, prompt, param)) = prompt
            && self.mode == Mode::Gui
            && !p.contains_key(param)
        {
            return self.need_path(name, prompt, param, p);
        }
        if let Some(editor) = &mut self.symbol_editor {
            editor.offer_new_page = false;
        }
        let result = if action == "new_page" {
            self.place_published_on_new_page()
        } else {
            self.symbol_operation(action, p)
        };
        match result {
            Ok(()) => {
                // A device box's test arms the Device tool instead (its own message says so).
                let boxed = self
                    .kept_symbol_editor()
                    .is_some_and(|e| e.device_box().is_some());
                if matches!(action, "place" | "scratch") && !boxed {
                    self.after_test_placement(action == "scratch");
                }
                Outcome::Done
            }
            Err(message) => {
                if let Some(editor) = &mut self.symbol_editor {
                    match &mut editor.device_box {
                        Some(b) => b.error = Some(message.clone()),
                        None => {
                            editor.error = Some(message.clone());
                            self.said_at(super::said::Said::Editor);
                        }
                    }
                }
                failed(key!("symbol-operation-failed"), &[&message])
            }
        }
    }

    /// The window's way to an action that drops the drawing in hand (New symbol, Open or import, a candidate): with
    /// unsaved changes it asks Save symbol, Discard or Cancel in the house dialog first, the action then running with
    /// `discard`; otherwise the action runs at once.
    pub fn ask_symbol_discard(&mut self, action: &'static str, params: Value) -> Outcome {
        // While a device box is edited the action is the box's: only its own changes are asked about (a symbol draft
        // under it stays in hand).
        let unsaved = self.symbol_editor.as_ref().is_some_and(|e| match e.device_box() {
            Some(b) => b.dirty(),
            None => e.dirty(),
        });
        if unsaved {
            return self.open_dialog(super::Dialog::ConfirmDiscard {
                pending: super::Pending {
                    action,
                    params,
                    cancelled: None,
                },
                symbol: true,
            });
        }
        self.dispatch(action, &params)
    }

    /// After a test placement Parts steps aside and the page shows with the symbol selected; the toast offers the way
    /// back to the drawing in hand.
    fn after_test_placement(&mut self, scratch: bool) {
        self.step_aside();
        if let Some(id) = self.tools.last_primary.clone() {
            let _ = self.dispatch("select.set", &json!({ "ids": [id] }));
        }
        let text = if scratch {
            let page = self
                .page_labels()
                .get(self.page_index())
                .cloned()
                .unwrap_or_default();
            strings::format(key!("symbol-scratch-created"), &[&page])
        } else {
            strings::text(key!("symbol-placed")).to_owned()
        };
        let back = back_to_editor();
        self.notes.push(Note::new("symbol", text).offering([Some(back)]));
    }

    /// `symbol.new_page`: the published symbol of a draft saved to a library and unchanged
    /// since, placed on a new page with its grid at 40/40 mm (Test on a new page's point) and selected, as the Insert
    /// symbol picker's offer places it; the editor steps aside as after a test placement. Any other draft is refused:
    /// its offer is Test on a new page.
    fn place_published_on_new_page(&mut self) -> Result<(), String> {
        let editor = self
            .symbol_editor
            .as_ref()
            .ok_or_else(|| strings::text(key!("symbol-not-open")).to_owned())?;
        let address = editor
            .published()
            .ok_or_else(|| strings::text(key!("symbol-not-published")).to_owned())?
            .to_owned();
        let grid = json!(editor.draft.symbol.grid);
        let back = self.current_folio().map(str::to_owned);
        self.new_page_with_grid(grid, None)?;
        let page = self.tools.last_primary.clone();
        // Parts steps aside first, as after a test placement: the Place tool works on the project's page.
        self.step_aside();
        // The canvas and the tool's context follow the new page, so the placement's tool is not escaped by it.
        self.sync_views();
        self.sync_tool_context();
        for (action, params) in [
            ("place.pick", json!({ "symbol": address })),
            ("cursor.set", json!({"x_mm": 40, "y_mm": 40})),
            ("canvas.activate", json!({})),
        ] {
            let refusal = match self.dispatch(action, &params) {
                Outcome::Done => continue,
                Outcome::Failed { message } => message,
                Outcome::Refused { reason: reason_key } => strings::render(&reason_key),
            };
            // The page was made for this symbol alone: a refused placement takes it back.
            self.take_back_new_page(back.as_deref());
            if let Some(place) = self.parts.as_deref_mut() {
                place.aside = false;
            }
            return Err(refusal);
        }
        let placed = self
            .tools
            .last_primary
            .clone()
            .filter(|id| Some(id) != page.as_ref());
        let _ = self.dispatch("ui.dismiss", &json!({}));
        // The Place tool's own toast says what was placed where, with Undo; the Draw tab's Symbols returns to the draft.
        if let Some(id) = placed {
            let _ = self.dispatch("select.set", &json!({ "ids": [id] }));
        }
        Ok(())
    }

    /// A new page with the connection grid `grid` (`{num, den}`), at the first free whole folio, titled `title`, then
    /// shown: Test on a new page's and Place on a new page with its grid's page.
    pub(crate) fn new_page_with_grid(&mut self, grid: Value, title: Option<String>) -> Result<(), String> {
        let labels = self.page_labels();
        let mut index = 1;
        while labels.iter().any(|l| l == &index.to_string()) {
            index += 1;
        }
        let mut params = json!({"number":index,"grid":grid});
        if let Some(title) = title {
            params["title"] = json!(title);
        }
        match self.host_create("page.create", params) {
            Outcome::Done => {}
            Outcome::Failed { message } => return Err(message),
            Outcome::Refused { reason: reason_key } => return Err(strings::render(&reason_key)),
        }
        if let Some(id) = self.tools.last_primary.clone() {
            self.tools.selection.clear();
            self.show_page_by_id(&id);
        }
        Ok(())
    }

    fn symbol_operation(&mut self, action: &str, p: &Map<String, Value>) -> Result<(), String> {
        // While a device box is edited, the editor's actions are the box's.
        if let Some(done) = self.box_operation(action, p) {
            return done;
        }
        if action == "open" {
            return self.symbol_import(p);
        }
        if action == "scratch" {
            return self.symbol_scratch();
        }
        // A drawing that is not his reads as it is: what changes it is refused with why.
        let changes = !matches!(
            action,
            "tool" | "variant" | "validate" | "select" | "copy" | "export" | "grid"
        );
        if changes && let Some(why) = self.symbol_editor.as_ref().and_then(|e| e.locked.clone()) {
            return Err(why);
        }
        if action == "prop" {
            return self.symbol_prop(p);
        }
        // The path is resolved, and an empty one refused, before the draft is borrowed.
        let file_path = matches!(action, "export")
            .then(|| self.symbol_path(text(p, "file"), key!("symbol-file-required")))
            .transpose()?;
        let editor = self
            .symbol_editor
            .as_mut()
            .ok_or_else(|| strings::text(key!("symbol-not-open")).to_owned())?;
        match action {
            // A connection point's function pin and discipline, which its Properties rows set.
            "map_cp" => Self::symbol_map_cp(editor, p)?,
            "discipline" => Self::symbol_discipline(editor, p)?,
            "path" => Self::symbol_add_path(editor, p)?,
            "candidate" => Self::symbol_candidate(editor, &mut self.symbol_editor_generation, p)?,
            "undo" => {
                if let Some(d) = editor.undo.pop() {
                    editor.redo.push(std::mem::replace(&mut editor.draft, d));
                    editor.error = None;
                    editor.revision = editor.revision.wrapping_add(1);
                    editor.prune_selection();
                }
            }
            "redo" => {
                if let Some(d) = editor.redo.pop() {
                    editor.undo.push(std::mem::replace(&mut editor.draft, d));
                    editor.error = None;
                    editor.revision = editor.revision.wrapping_add(1);
                    editor.prune_selection();
                }
            }
            "tool" => {
                editor.tool = TOOLS
                    .into_iter()
                    .find(|t| *t == text(p, "tool"))
                    .ok_or_else(|| strings::text(key!("symbol-tool-invalid")).to_owned())?;
            }
            "variant" => {
                editor.variant =
                    p.get("variant")
                        .and_then(Value::as_u64)
                        .filter(|v| *v < 8)
                        .ok_or_else(|| words(key!("symbol-variant-invalid")))? as u8;
            }
            "validate" => {
                let issues = editor.draft.validate();
                if !issues.is_empty() {
                    return Err(symbol_error(symbols::Error::Invalid(issues)));
                }
                editor.error = None;
            }
            "select" => editor.select(p)?,
            "move" => editor.move_selection(p, false)?,
            "nudge" => editor.move_selection(p, true)?,
            "delete" => editor.delete_selection()?,
            "copy" | "cut" => {
                self.symbol_clipboard = editor.copied()?;
                if action == "cut" {
                    editor.delete_selection()?;
                }
                // On the system clipboard too: the platform turns Ctrl+V into a paste only when it holds a text.
                let gui = self.mode == Mode::Gui;
                self.clip.set(self.symbol_clipboard.text(), None, 0, gui);
            }
            // What was pasted is what the next paste copies, so each paste lands a step further.
            "paste" => {
                editor.paste(&self.symbol_clipboard)?;
                self.symbol_clipboard = editor.copied()?;
            }
            "duplicate" => {
                let clip = editor.copied()?;
                editor.paste(&clip)?;
            }
            "align" => editor.align(p)?,
            "grip" => editor.grip(p)?,
            "grid" => {
                editor.set_grid(p)?;
                // His drawing grid and Snap are the next drawing's.
                self.settings.symbol_drawing_grid = Some(editor.grid);
                self.settings.symbol_snap = Some(editor.snap);
                self.save_settings();
            }
            "draw" => Self::symbol_draw(editor, p)?,
            "cp" => Self::symbol_cp(editor, p)?,
            "slot" => Self::symbol_slot(editor, p)?,
            "link" => Self::symbol_link(editor, p)?,
            "export" => {
                let path = file_path.ok_or_else(damaged)?;
                symbols::export(&editor.draft, &path)
                    .map_err(|e| file_error(e, key!("symbol-export-failed")))?;
                editor.saved = Some(editor.draft.clone());
                editor.stored = true;
            }
            "place" => {
                let draft = editor.draft.clone();
                let variant = editor.variant;
                self.symbol_place(draft, variant, p)?;
            }
            _ => return Err(strings::text(key!("symbol-tool-invalid")).into()),
        }
        Ok(())
    }

    /// `symbol.open {file, key?}` (a symbol file: a QElectroTech element, an EPLAN symbol file, a DXF drawing): its
    /// first drawing as the drawing in Parts, a symbol of his own, the file's other drawings beside it. The drawing in
    /// hand with unsaved changes is kept unless `discard` (the window asks first).
    fn symbol_import(&mut self, p: &Map<String, Value>) -> Result<(), String> {
        let discard = p.get("discard") == Some(&Value::Bool(true));
        if self.symbol_editor.as_ref().is_some_and(SymbolEditor::dirty) && !discard {
            return Err(strings::text(key!("symbol-unsaved")).into());
        }
        let path = self.symbol_path(text(p, "file"), key!("symbol-file-required"))?;
        let key = match text(p, "key") {
            "" => symbols::file_key(&path),
            key => key.to_owned(),
        };
        let candidates =
            symbols::open(&path, &key).map_err(|e| file_error(e, key!("symbol-file-unreadable")))?;
        let draft = candidates
            .first()
            .cloned()
            .ok_or_else(|| strings::text(key!("eplan-no-symbols")).to_owned())?;
        // An import opened on a drawing for a page or a piece is drawn for the same errand.
        let purpose = (self
            .symbol_editor
            .as_ref()
            .filter(|e| e.shown && e.device_box.is_none()))
        .map_or(Drawing::Symbol, |e| e.purpose.clone());
        self.open_drawing(draft, purpose, None, None);
        if let Some(editor) = &mut self.symbol_editor {
            editor.candidates = candidates;
        }
        Ok(())
    }

    /// Test on a new page: the draft checked, a page with its grid and the draft's name made, and the draft placed
    /// on it at 40/40 mm.
    fn symbol_scratch(&mut self) -> Result<(), String> {
        let draft = &self
            .symbol_editor
            .as_ref()
            .ok_or_else(|| strings::text(key!("symbol-not-open")).to_owned())?
            .draft;
        draft.documents(symbols::DRAFT_LIBRARY).map_err(symbol_error)?;
        let project = self
            .project
            .as_ref()
            .ok_or_else(|| strings::text(key!("reason-no-project")).to_owned())?;
        draft
            .check_function(project.session.libraries())
            .map_err(symbol_error)?;
        let title = strings::format(key!("symbol-scratch-title"), &[&name_words(&draft.symbol.name)]);
        let grid = json!(draft.symbol.grid);
        self.new_page_with_grid(grid, Some(title))?;
        self.symbol_operation("place", json!({"x":40,"y":40}).as_object().ok_or_else(damaged)?)
    }

    /// Maps connection point `cp` to function pin `fn_cp`, swapping with the one other point that held the pin;
    /// both pins must be physical and of the point's discipline.
    fn symbol_map_cp(editor: &mut SymbolEditor, p: &Map<String, Value>) -> Result<(), String> {
        let index = |key: &str| {
            p.get(key)
                .and_then(Value::as_u64)
                .and_then(|i| u16::try_from(i).ok())
        };
        let (Some(cp), Some(fn_cp)) = (index("cp"), index("fn_cp")) else {
            return Err(strings::text(key!("symbol-mapping-incompatible")).to_owned());
        };
        let mut draft = editor.draft.clone();
        let compatible = |c: &symbols::schema::SymbolCp, pin| {
            draft
                .function
                .cps
                .get(&pin)
                .is_some_and(|f| f.physical && f.discipline == c.discipline)
        };
        let mut target = draft
            .symbol
            .cps
            .get(&cp)
            .cloned()
            .ok_or_else(|| strings::text(key!("symbol-mapping-incompatible")).to_owned())?;
        if !compatible(&target, fn_cp) {
            return Err(strings::text(key!("symbol-mapping-incompatible")).to_owned());
        }
        let occupied: Vec<_> = draft
            .symbol
            .cps
            .iter()
            .filter(|c| c.idx != cp && c.fn_cp == fn_cp)
            .cloned()
            .collect();
        if occupied.len() > 1 {
            return Err(strings::text(key!("symbol-mapping-incompatible")).to_owned());
        }
        if let Some(mut other) = occupied.into_iter().next() {
            if !compatible(&other, target.fn_cp) {
                return Err(strings::text(key!("symbol-mapping-incompatible")).to_owned());
            }
            other.fn_cp = target.fn_cp;
            draft.symbol.cps.insert(other);
        }
        target.fn_cp = fn_cp;
        draft.symbol.cps.insert(target);
        editor.replace(draft);
        Ok(())
    }

    /// Sets the discipline the symbol is drawn for: every connection point and function pin takes it, and a
    /// function type it does not offer ([`function_kinds`]) becomes a black box; with `cp`, that one point and the pin it
    /// draws only (a point that differs). Refused while the draft uses a library's function definition.
    fn symbol_discipline(editor: &mut SymbolEditor, p: &Map<String, Value>) -> Result<(), String> {
        let Some(&word) = DISCIPLINES.iter().find(|d| **d == text(p, "discipline")) else {
            return Err(words(key!("symbol-discipline-invalid")));
        };
        if editor.draft.function_ref.is_some() {
            return Err(words(key!("symbol-function-locked")));
        }
        let one = p.get("cp").and_then(Value::as_u64);
        let before = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
        let mut d = before.clone();
        let mut pins = Vec::new();
        for point in d
            .pointer_mut("/symbol/cps")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if one.is_none_or(|i| point["idx"] == i) {
                point["discipline"] = json!(word);
                pins.push(point["fn_cp"].clone());
            }
        }
        for pin in d
            .pointer_mut("/function/cps")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if one.is_none() || pins.contains(&pin["idx"]) {
                pin["discipline"] = json!(word);
            }
        }
        if one.is_none() {
            editor.chosen_discipline = Some(word);
            let kind = d["function"]["contact_kind"].as_str();
            if kind.is_some_and(|k| !function_kinds(word).contains(&k)) {
                d["function"]["contact_kind"] = json!("black_box");
            }
        }
        // A symbol with nothing to change (no point yet) keeps its draft: the choice waits for its first point.
        if d != before {
            editor.replace(
                SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?)
                    .map_err(edit_error)?,
            );
        }
        Ok(())
    }

    /// Adds a polyline, or a polygon when `closed`, through `points`.
    fn symbol_add_path(editor: &mut SymbolEditor, p: &Map<String, Value>) -> Result<(), String> {
        let points = p
            .get("points")
            .and_then(Value::as_array)
            .ok_or_else(|| strings::text(key!("symbol-graphic-invalid")).to_owned())?;
        if points.len() > symbols::MAX_ITEMS {
            return Err(strings::text(key!("symbol-input-limit")).into());
        }
        let mut converted = Vec::new();
        for point in points {
            let params = json!({"x":point.get(0),"y":point.get(1)});
            converted.push([
                nm(params.as_object().ok_or_else(damaged)?, "x")?,
                nm(params.as_object().ok_or_else(damaged)?, "y")?,
            ]);
        }
        let kind = if p.get("closed") == Some(&Value::Bool(true)) {
            "polygon"
        } else {
            "polyline"
        };
        let mut d = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
        d["symbol"]
            .as_object_mut()
            .ok_or_else(damaged)?
            .entry("graphics")
            .or_insert(json!([]))
            .as_array_mut()
            .ok_or_else(damaged)?
            .push(json!({kind:{"points":converted,"closed":kind=="polygon","stroke":{"pen":"p026"}}}));
        editor.replace(
            SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(edit_error)?,
        );
        editor.select_last(Item::Graphic(0));
        Ok(())
    }

    /// Replaces the draft with another drawing of the same import, as a fresh start: it becomes the saved state and
    /// the view clears its tool state.
    fn symbol_candidate(
        editor: &mut SymbolEditor,
        generation: &mut u64,
        p: &Map<String, Value>,
    ) -> Result<(), String> {
        if editor.dirty() && p.get("discard") != Some(&Value::Bool(true)) {
            return Err(strings::text(key!("symbol-unsaved")).into());
        }
        let index = p
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|i| usize::try_from(i).ok())
            .ok_or_else(|| words(key!("symbol-candidate-invalid")))?;
        let draft = editor
            .candidates
            .get(index)
            .cloned()
            .ok_or_else(|| words(key!("symbol-candidate-invalid")))?;
        editor.replace(draft.clone());
        editor.saved = Some(draft);
        editor.stored = false;
        *generation = generation.wrapping_add(1);
        editor.generation = *generation;
        Ok(())
    }

    /// Draws a graphic with drawing tool `tool` from `x`/`y` to `x2`/`y2`, a text or the anchor at `x`/`y`.
    fn symbol_draw(editor: &mut SymbolEditor, p: &Map<String, Value>) -> Result<(), String> {
        let tool = text(p, "tool");
        let (x, y) = (nm(p, "x")?, nm(p, "y")?);
        let (x2, y2) = if matches!(tool, "text" | "anchor") {
            (x, y)
        } else {
            (nm(p, "x2")?, nm(p, "y2")?)
        };
        let mut d = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
        let object = d["symbol"].as_object_mut().ok_or_else(damaged)?;
        let stroke = json!({"pen":"p026"});
        let value = match tool {
            "line" => json!({"line":{"from":[x,y],"to":[x2,y2],"stroke":stroke}}),
            "rectangle" => {
                json!({"rect":{"rect":[[x.min(x2),y.min(y2)],[x.max(x2),y.max(y2)]],"stroke":stroke}})
            }
            "circle" => {
                json!({"circle":{"centre":[x,y],"r": (((x2-x) as f64).hypot((y2-y) as f64)).round() as i64,"stroke":stroke}})
            }
            "ellipse" => {
                json!({"ellipse":{"rect":[[x.min(x2),y.min(y2)],[x.max(x2),y.max(y2)]],"stroke":stroke}})
            }
            "arc" => {
                let angle = |key: &str, default: i64| -> Result<i32, String> {
                    let degrees = match p.get(key) {
                        Some(v) => v.as_i64().ok_or_else(|| words(key!("symbol-angle-invalid")))?,
                        None => default,
                    };
                    if !(-360..=360).contains(&degrees) {
                        return Err(words(key!("symbol-angle-invalid")));
                    }
                    Ok((degrees * 1000) as i32)
                };
                json!({"arc":{"rect":[[x.min(x2),y.min(y2)],[x.max(x2),y.max(y2)]],"start":angle("start",0)?,"sweep":angle("sweep",180)?,"stroke":stroke}})
            }
            // Centred on its click, in the style last chosen, else the pin designations' small one.
            "text" => {
                let style = editor.text_style.as_deref().unwrap_or(NEW_TEXT_STYLE);
                json!({"text":{"pos":[x,y],"text":{"en":text(p,"text")},"align":"centre","style":style}})
            }
            // `centre`: at the centre of the drawing, wherever its graphics go.
            "anchor" if p.get("centre").and_then(Value::as_bool) == Some(true) => {
                object.insert("anchor".into(), json!("graphic_centre"));
                Value::Null
            }
            "anchor" => {
                object.insert("anchor".into(), json!({"named":[x,y]}));
                Value::Null
            }
            _ => return Err(strings::text(key!("symbol-tool-invalid")).into()),
        };
        if !value.is_null() {
            object
                .entry("graphics")
                .or_insert(json!([]))
                .as_array_mut()
                .ok_or_else(damaged)?
                .push(value);
        }
        editor.replace(
            SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(edit_error)?,
        );
        editor.select_last(if tool == "anchor" {
            Item::Anchor
        } else {
            Item::Graphic(0)
        });
        Ok(())
    }

    /// Adds or moves connection point `idx` (else the next free one) to `x`/`y`, with its function pin, direction
    /// and, while the draft owns its function, the pin's designation. A new point and pin take the symbol's discipline.
    fn symbol_cp(editor: &mut SymbolEditor, p: &Map<String, Value>) -> Result<(), String> {
        let discipline = editor.discipline();
        let mut d = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
        let index = p.get("idx").and_then(Value::as_u64).unwrap_or_else(|| {
            editor
                .draft
                .symbol
                .cps
                .iter()
                .map(|p| u64::from(p.idx) + 1)
                .max()
                .unwrap_or(1)
        });
        if index > u64::from(u16::MAX) {
            return Err(words(key!("symbol-cp-index-invalid")));
        }
        let points = d["symbol"]
            .as_object_mut()
            .ok_or_else(damaged)?
            .entry("cps")
            .or_insert(json!([]))
            .as_array_mut()
            .ok_or_else(damaged)?;
        let old = points.iter().find(|v| v["idx"] == index).cloned();
        let new = old.is_none();
        // A new point of a drawing of a starter function (What it is) stands for the function's first pin no point
        // stands for yet; of a function of its own, for a pin of its own number.
        let free = (editor.draft.function_ref.is_some())
            .then(|| {
                let drawn: Vec<u16> = editor.draft.symbol.cps.iter().map(|c| c.fn_cp).collect();
                editor
                    .draft
                    .function
                    .cps
                    .iter()
                    .map(|c| c.idx)
                    .find(|i| !drawn.contains(i))
            })
            .flatten()
            .map_or(index, u64::from);
        let mut cp = old.unwrap_or(json!({"idx":index,"fn_cp":free,"dir":"left","discipline":discipline}));
        if let Some(mapping) = p.get("fn_cp") {
            cp["fn_cp"] = mapping.clone();
        }
        let pos = [nm(p, "x")?, nm(p, "y")?];
        match text(p, "direction") {
            "" => {}
            // `lead`: the way the lead it ends points, so a wire carries on from it; left where it ends none.
            "lead" => {
                if let Some(dir) = lead_direction(&editor.draft, pos) {
                    cp["dir"] = json!(dir);
                }
            }
            word => cp["dir"] = json!(word),
        }
        cp["pos"] = json!(pos);
        let fn_cp = cp["fn_cp"]
            .as_u64()
            .ok_or_else(|| words(key!("symbol-function-pin-invalid")))?;
        let dir = cp["dir"].clone();
        points.retain(|v| v["idx"] != index);
        points.push(cp);
        points.sort_by_key(|v| v["idx"].as_u64());
        if editor.draft.function_ref.is_none() {
            let fpoints = d["function"]
                .as_object_mut()
                .ok_or_else(damaged)?
                .entry("cps")
                .or_insert(json!([]))
                .as_array_mut()
                .ok_or_else(damaged)?;
            let mut fcp = fpoints
                .iter()
                .find(|v| v["idx"] == fn_cp)
                .cloned()
                .unwrap_or(json!({"idx":fn_cp,"designation":index.to_string(),"discipline":discipline}));
            if !text(p, "designation").is_empty() {
                fcp["designation"] = json!(text(p, "designation"));
            }
            fpoints.retain(|v| v["idx"] != fn_cp);
            fpoints.push(fcp);
            fpoints.sort_by_key(|v| v["idx"].as_u64());
        }
        // A new point's number is written by a text of its own, inside the body at the point, turned by the edge it
        // stands on: where a device box prints a pin's name (doc 07 §2.5), so the drawing shows what the page prints.
        if new {
            let name = d["function"]["cps"]
                .as_array()
                .and_then(|a| a.iter().find(|v| v["idx"] == fn_cp))
                .and_then(|v| v["designation"].as_str())
                .unwrap_or_default()
                .to_owned();
            let dir: symbols::schema::Dir4 = serde_json::from_value(dir).map_err(|e| e.to_string())?;
            if let Some((off, rot, align)) = symbols::pin_text_place(&dir, &name) {
                let slot = json!({"kind":{"cp_designation":index},"pos":[pos[0] + off.x.0, pos[1] + off.y.0],
                    "rot":rot,"align":align,"visible":true});
                d["symbol"]
                    .as_object_mut()
                    .ok_or_else(damaged)?
                    .entry("text_slots")
                    .or_insert(json!([]))
                    .as_array_mut()
                    .ok_or_else(damaged)?
                    .push(slot);
            }
        }
        editor.replace(
            SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(edit_error)?,
        );
        if let Some(at) = editor
            .draft
            .symbol
            .cps
            .iter()
            .position(|c| u64::from(c.idx) == index)
        {
            editor.selection = vec![Item::Cp(at)];
        }
        Ok(())
    }

    /// Adds a text slot of `kind` (a tag by default) at `x`/`y`.
    fn symbol_slot(editor: &mut SymbolEditor, p: &Map<String, Value>) -> Result<(), String> {
        let mut d = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
        let slots = d["symbol"]
            .as_object_mut()
            .ok_or_else(damaged)?
            .entry("text_slots")
            .or_insert(json!([]))
            .as_array_mut()
            .ok_or_else(damaged)?;
        let kind = p.get("kind").cloned().unwrap_or(json!("tag"));
        slots.push(json!({"kind":kind,"pos":[nm(p,"x")?,nm(p,"y")?],"align":"top_left","visible":true}));
        editor.replace(
            SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(edit_error)?,
        );
        editor.select_last(Item::Slot(0));
        Ok(())
    }

    /// Adds a link point named `name` at `x`/`y`.
    fn symbol_link(editor: &mut SymbolEditor, p: &Map<String, Value>) -> Result<(), String> {
        let mut d = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
        let points = d["symbol"]
            .as_object_mut()
            .ok_or_else(damaged)?
            .entry("link_points")
            .or_insert(json!([]))
            .as_array_mut()
            .ok_or_else(damaged)?;
        points.push(json!({"name":text(p,"name"),"pos":[nm(p,"x")?,nm(p,"y")?]}));
        editor.replace(
            SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(edit_error)?,
        );
        editor.select_last(Item::Link(0));
        Ok(())
    }

    /// Saves the drawing with his parts (doc 07 §5.6): the valid draft published to the library it came from where
    /// he can write it, else to My parts (made when missing); the draft and its history marked as saved there. Its
    /// address (`library:<library>:<item>`), which the Insert symbol picker and a piece name it by.
    pub(crate) fn symbol_save(&mut self) -> Result<String, String> {
        let tiers = self
            .symbol_tiers()
            .ok_or_else(|| strings::text(key!("error-part-no-my-parts")).to_owned())?;
        let editor = self
            .symbol_editor
            .as_mut()
            .ok_or_else(|| strings::text(key!("symbol-not-open")).to_owned())?;
        let mut draft = editor.draft.clone();
        let issues = draft.validate();
        if !issues.is_empty() {
            return Err(symbol_error(symbols::Error::Invalid(issues)));
        }
        let mut registry = ew_api::library::Registry::open(&tiers).map_err(|e| e.to_string())?;
        draft.check_function(&registry).map_err(symbol_error)?;
        let home = editor
            .home
            .filter(|h| registry.libraries().iter().any(|l| l.id == *h && l.writable));
        let target = match home {
            Some(h) => h,
            None => ew_api::eplan::user_parts_library(&mut registry)
                .map_err(|_| strings::text(key!("error-part-no-my-parts")).to_owned())?,
        };
        let before = draft.identity.clone();
        // The writer's version refusals in words, the item named by the symbol, without its UUID.
        let fndef_item = draft.identity.as_ref().and_then(|i| i.fndef).map(|f| f.as_uuid());
        let name = name_words(&draft.symbol.name);
        let item = |id| {
            let kind = if Some(id) == fndef_item {
                key!("symbol-item-function")
            } else {
                key!("symbol-item-symbol")
            };
            strings::format(kind, &[&name])
        };
        // Save saves. A key made from the name that the library holds for another symbol takes the next free number,
        // and a changed symbol whose version the library already holds takes the next patch version: neither is a
        // reason to send a person to the Library tab.
        let (from_name, base) = (key_from_name(&draft), draft.key.clone());
        let version = draft.version.clone();
        let (mut number, mut tries) = (1_u32, 0_u32);
        let published = loop {
            tries += 1;
            match draft.publish(&mut registry, target) {
                Err(symbols::Error::KeyTaken { .. }) if from_name && tries < 1000 => {
                    number += 1;
                    draft.key = format!("{base}-{number}");
                }
                Err(symbols::Error::Library(ew_api::library::LibraryError::Immutable { .. }))
                    if tries < 1000 && ew_api::next_by_last_number(&draft.version).is_some() =>
                {
                    draft.version = ew_api::next_by_last_number(&draft.version).unwrap_or_default();
                }
                other => break other,
            }
        };
        published.map_err(|e| match e {
            symbols::Error::Library(ew_api::library::LibraryError::Immutable { item: id, version }) => {
                strings::format(key!("symbol-version-taken"), &[&item(id), &version])
            }
            symbols::Error::Library(ew_api::library::LibraryError::Downgrade {
                item: id,
                version,
                held,
            }) => strings::format(key!("symbol-version-older"), &[&item(id), &version, &held]),
            e => symbol_error(e),
        })?;
        // A key other than the one the library holds for the draft's items published new ones.
        // Only when the symbol's own identity changed, not when only an owned function
        // definition took one.
        let symbol_of = |i: &Option<symbols::DraftIdentity>| i.as_ref().map(|i| i.symbol);
        let note = (before.is_some() && symbol_of(&before) != symbol_of(&draft.identity))
            .then(|| strings::format(key!("symbol-saved-new"), &[&draft.key]));
        let library = registry
            .libraries()
            .into_iter()
            .find(|l| l.id == target)
            .map(|l| l.name)
            .unwrap_or_default();
        self.config.libraries = Some(tiers);
        // The libraries the place reads, the project's or (no project) its own over the libraries alone, are those it
        // was saved into: a piece takes it, the Symbols shelf lists it.
        if let Some(project) = &mut self.project {
            *project.session.libraries_mut() = registry;
        } else if let Some(alone) = &mut self.parts_alone {
            *alone.libraries_mut() = registry;
        }
        // The draft's history publishes the same items from now on, under the key the library took.
        let history = editor.undo.iter_mut().chain(editor.redo.iter_mut());
        for d in history.chain([&mut editor.draft]) {
            if d.key == draft.key || d.key == base {
                d.key.clone_from(&draft.key);
                d.identity.clone_from(&draft.identity);
            }
        }
        editor.draft.version.clone_from(&draft.version);
        let saved_version = (draft.version != version).then(|| draft.version.clone());
        editor.home = Some(target);
        // The published symbol as the Insert symbol picker addresses it, while the draft is this one.
        editor.published = draft
            .identity
            .as_ref()
            .map(|i| (draft.clone(), format!("library:{target}:{}", i.symbol)));
        editor.saved = Some(draft);
        editor.stored = true;
        editor.error = None;
        let address = editor
            .published
            .as_ref()
            .map(|(_, a)| a.clone())
            .ok_or_else(damaged)?;
        self.settings.record_symbol_edit(&address);
        self.save_settings();
        self.libraries_changed();
        // What was saved where, and how it gets onto a page: the click's answer.
        self.notify_toned(
            note.unwrap_or_else(|| match &saved_version {
                Some(v) => strings::format(key!("symbol-saved-version"), &[&name, &library, v]),
                None => strings::format(key!("symbol-saved-to"), &[&name, &library]),
            }),
            NoteTone::Success,
        );
        Ok(address)
    }

    /// Test placement: a copy of `draft` under a key its content derives, placed at `x`/`y` on the current page in
    /// orientation `variant`; refused when the page's connection grid differs.
    fn symbol_place(
        &mut self,
        mut draft: SymbolDraft,
        variant: u8,
        p: &Map<String, Value>,
    ) -> Result<(), String> {
        if let Some(project) = &self.project {
            draft
                .check_function(project.session.libraries())
                .map_err(symbol_error)?;
        }
        let (docs, symbol) = self.placed_copies(&mut draft, "test")?;
        let page = self
            .current_page()
            .ok_or_else(|| strings::text(key!("reason-no-page")).to_owned())?;
        let canvas = self
            .canvas()
            .ok_or_else(|| strings::text(key!("reason-no-page")).to_owned())?;
        let snapper = &canvas.page().snapper;
        let (num, den) = snapper.grid();
        if (num, den) != (draft.symbol.grid.num() as i64, draft.symbol.grid.den() as i64) {
            // The picker's words, both grids named; the editor then offers Place on a new page
            // with its grid.
            let words = crate::tools::grid_words([
                (draft.symbol.grid.num(), draft.symbol.grid.den()),
                (num.unsigned_abs(), den.unsigned_abs()),
            ]);
            if let Some(editor) = &mut self.symbol_editor {
                editor.offer_new_page = true;
            }
            return Err(words);
        }
        let pos = snapper
            .snap_grid(snapper.page_to_sheet((nm(p, "x")?, nm(p, "y")?)))
            .page_nm;
        let orient = ["r0", "r90", "r180", "r270", "m0", "m90", "m180", "m270"][usize::from(variant)];
        let params = json!({"cmd":"device.place","params":{"page":page.uuid_text(),"symbol":symbol,"copies":docs,
            "pos":[pos.0,pos.1],"orient":orient,"target":{"new_device":{"letter":"U"}}}});
        match self.act_command_run(params.as_object().ok_or_else(damaged)?) {
            Outcome::Done => {}
            Outcome::Failed { message } => return Err(message),
            Outcome::Refused { reason: reason_key } => return Err(strings::render(&reason_key)),
        }
        Ok(())
    }

    /// The symbol of a catalogue row (`source`: a starter key, a library address or a project copy) on another
    /// connection grid than the page's, as its copy drawn on the page's grid `grid` ([`SymbolDraft::on_grid`]), which
    /// the project then holds: its copy ID, for `place.pick`. The same symbol on the same grid is the same copy.
    pub(super) fn symbol_on_page_grid(
        &mut self,
        source: &str,
        (num, den): (u64, u64),
    ) -> Result<String, String> {
        let project = self
            .project
            .as_ref()
            .ok_or_else(|| strings::text(key!("reason-no-project")).to_owned())?;
        let grid = symbols::units::Grid::new(num, den).map_err(|e| e.to_string())?;
        let held = project
            .session
            .symbol_draft(source, "user.symbol")
            .map_err(symbol_error)?;
        let mut draft = held.on_grid(grid).map_err(symbol_error)?;
        draft.key = key_of_name(&name_words(&draft.symbol.name));
        let (docs, symbol) = self.placed_copies(&mut draft, "grid")?;
        let project = self
            .project
            .as_ref()
            .ok_or_else(|| strings::text(key!("reason-no-project")).to_owned())?;
        let q = project.session.query();
        let lacking: Vec<Value> = docs
            .iter()
            .filter(|d| match d {
                symbols::schema::LibDocument::Symbol(s) => !q.symbols().contains(s.id),
                symbols::schema::LibDocument::Fndef(f) => q.fndefs().get(f.id).is_none(),
                _ => true,
            })
            .filter_map(|d| serde_json::to_value(d).ok())
            .collect();
        drop(q);
        if !lacking.is_empty() {
            let command = ew_api::AnyCommand {
                name: "library.embed".to_owned(),
                params: json!({ "copies": lacking }),
            };
            match self.run_commit(&command) {
                Outcome::Done => {}
                Outcome::Failed { message } => return Err(message),
                Outcome::Refused { reason: reason_key } => return Err(strings::render(&reason_key)),
            }
        }
        Ok(symbol.to_string())
    }

    /// The library documents a placement of `draft` embeds and its symbol's copy ID: the draft under a key its
    /// content derives (`<key>.<tag>.<suffix>`), with item IDs from that key, and the function it names.
    fn placed_copies(
        &self,
        draft: &mut SymbolDraft,
        tag: &str,
    ) -> Result<(Vec<symbols::schema::LibDocument>, ew_api::output::Uuid), String> {
        // Each content placed embeds its own copies, after a draft edit, a reopen or a trimmed history too: the
        // suffix is derived from the content.
        let suffix = content_suffix(draft);
        draft.key = format!(
            "{}.{tag}.{suffix}",
            draft.key.chars().take(100).collect::<String>()
        );
        // The same content placed again is the same copy (re-review part 4): its item IDs come from its key,
        // which carries the content's suffix, never a new UUIDv7 per placement. The v5 name's parts
        // joined with U+001F (ADR-0004 amendment 4), as projects store these IDs.
        let item = |kind: &str| {
            ew_api::output::v5::kind_item_id(symbols::DRAFT_LIBRARY, kind, &draft.key).to_string()
        };
        let fndef = draft.function_ref.is_none().then(|| item("fndef"));
        draft.identity = Some(
            serde_json::from_value(json!({"symbol": item("symbol"), "fndef": fndef}))
                .map_err(|e| e.to_string())?,
        );
        let mut docs = draft.documents(symbols::DRAFT_LIBRARY).map_err(symbol_error)?;
        if let Some(r) = &draft.function_ref {
            let project = self
                .project
                .as_ref()
                .ok_or_else(|| strings::text(key!("reason-no-project")).to_owned())?;
            docs.extend(
                project
                    .session
                    .libraries()
                    .closure(r)
                    .map_err(|e| e.to_string())?,
            );
        }
        let symbol = docs
            .iter()
            .find_map(|d| match d {
                symbols::schema::LibDocument::Symbol(s) => Some(s.id),
                _ => None,
            })
            .ok_or_else(damaged)?;
        Ok((docs, symbol.as_uuid()))
    }
}
