// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol editor's independent draft, history and GUI/headless actions.

use super::{Mode, Outcome, Prompt, ShellModel, failed};
use crate::notifications::{Note, NoteAction, NoteTone, Offer, Validity};
use crate::{key, strings};
use ew_api::symbols::{self, SymbolDraft};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

mod device_box;
mod selection;
mod sheet;
pub use device_box::{DeviceBox, PinLine, SIDES, side_word};
pub use selection::{Clipboard, Item, bounds, grips, role_words};
pub use sheet::{Outline, OutlineGroup, OutlineRow, RowValue, Sheet, SheetRow, Todo, WHAT, trim_mm};

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
/// the default (FT-1690). Connection points keep to the symbol's connection grid whatever is chosen.
pub const GRID_STEPS: [(i64, i64); 5] = [
    (500_000, 1),
    (1_000_000, 1),
    (2_000_000, 1),
    (4_000_000, 1),
    (7_937_500, 3),
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

/// A named symbol that can be forked without changing its source.
#[derive(Debug)]
pub struct SymbolSource {
    /// Source key, or embedded copy identity; passed to `symbol.fork`.
    pub source: String,
    /// Source library name; an embedded project copy has no library here.
    pub library: Option<String>,
    /// Optional stable key, retained for searching and automation.
    pub key: Option<String>,
    /// Human-readable symbol name.
    pub name: symbols::MlText,
    /// Connection grid, so native and imported versions can be distinguished.
    pub grid: symbols::units::Grid,
    /// The first level of its function's category (`Relay`), which the symbol browser files it under; empty when
    /// its function does not resolve.
    pub category: String,
    /// The symbol copy whose shape the start card's preview draws (`Session::symbol_shape`).
    pub copy: Option<ew_api::output::Uuid>,
}

/// A function definition offered by the editor's library chooser.
#[derive(Debug)]
pub struct SymbolFunctionChoice {
    /// Library/item address passed to `symbol.function`.
    pub source: String,
    /// Human-readable source library name.
    pub library: String,
    /// Stable key, retained for display and legacy automation.
    pub key: String,
    /// Electrical function type.
    pub kind: symbols::schema::ContactKind,
    /// Source category path, distinguishing functions with the same kind and pins.
    pub category: String,
    /// Printed connection designations, in function-pin order.
    pub pins: Vec<String>,
}

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
    /// Whether the editor's view shows; a draft left with Close is kept with its history.
    pub(crate) shown: bool,
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
    /// Suggested writable library directory.
    pub directory: String,
    /// Other drawings recovered from the same input; selecting one is an explicit replacement.
    pub candidates: Vec<SymbolDraft>,
    /// Available embedded and installed symbols, read when the editor opens.
    pub sources: Vec<SymbolSource>,
    /// Installed function definitions, read when the editor opens.
    pub functions: Vec<SymbolFunctionChoice>,
    /// The parts drawn as a box, read with the symbols ([`ShellModel::box_parts`]).
    box_sources: Vec<(String, String, String)>,
    /// The discipline chosen last, which a symbol without connection points is drawn for.
    chosen_discipline: Option<&'static str>,
    /// What is selected in the drawing, in the order it was selected: the outline's rows, the drawing and Properties
    /// show the same things.
    pub selection: Vec<Item>,
    /// The drawing grid lines, shapes, texts and link points snap to: an index into [`GRID_STEPS`]. The person's
    /// choice for the session, kept from one draft to the next.
    pub grid: usize,
    /// Whether the drawing tools snap to the drawing grid (off: to 0.01 mm).
    pub snap: bool,
    /// Counts the changes of the draft, so the window reads what it measured of the drawing again.
    pub revision: u64,
    /// Whether the start card shows in the drawing's place: the ways in (a new symbol, a copy of a library symbol, a
    /// device box, an import) and the symbols edited lately.
    pub start: bool,
    /// A part's device box, edited in the drawing's place; the symbol draft waits under it.
    device_box: Option<DeviceBox>,
}

/// A library or project symbol that Edit symbol or Copy and edit opens (FT-1694).
struct EditTarget {
    /// What `symbol.fork` reads it from: its library address, or the project copy's ID.
    source: String,
    /// Its own key (empty for a project copy without one).
    key: String,
    /// Its name in words.
    name: String,
    /// The folder of the writable library that holds it, else why it can only be copied.
    folder: Result<PathBuf, String>,
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

/// The version after `version` by its last number (`1.0.0` gives `1.0.1`); `None` for a version that does not end
/// in a number.
fn next_patch(version: &str) -> Option<String> {
    let (head, last) = version.rsplit_once('.')?;
    let next = last.parse::<u32>().ok()?.checked_add(1)?;
    Some(format!("{head}.{next}"))
}

/// Whether the draft's key is the one its name gives, in a draft that no library holds yet: such a key follows the
/// name and gives way to a free one when the library holds it for another symbol.
fn key_from_name(draft: &SymbolDraft) -> bool {
    draft.identity.is_none() && draft.key == key_of_name(&name_words(&draft.symbol.name))
}

impl SymbolEditor {
    fn new(mut draft: SymbolDraft, directory: String) -> Self {
        // A person names a symbol; its key is made from the name until they type one (the Library tab).
        if draft.key.is_empty() && draft.identity.is_none() {
            draft.key = key_of_name(&name_words(&draft.symbol.name));
        }
        Self {
            generation: 0,
            draft: draft.clone(),
            saved: Some(draft.clone()),
            stored: false,
            shown: true,
            undo: Vec::new(),
            redo: Vec::new(),
            variant: 0,
            tool: TOOLS[0],
            error: None,
            offer_new_page: false,
            published: None,
            text_style: None,
            directory,
            candidates: Vec::new(),
            sources: Vec::new(),
            functions: Vec::new(),
            box_sources: Vec::new(),
            chosen_discipline: None,
            selection: Vec::new(),
            grid: 0,
            snap: true,
            revision: 0,
            start: false,
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
        // A change of the draft is work in hand: the start card gives way to it.
        self.start = false;
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

impl SymbolSource {
    /// The symbol as a list names it: its name with its connection grid ("Lamp (4 mm grid)"), and where it is from
    /// (its library, or "Project copy").
    pub fn words(&self) -> (String, String) {
        let from = self
            .library
            .clone()
            .unwrap_or_else(|| words(key!("symbol-project-copy")));
        // The name says its grid: the starter library holds most symbols on both (two rows read "Lamp").
        let grid = crate::status::grid_words(self.grid.num() as i64, self.grid.den() as i64);
        let name = strings::format(key!("symbol-source-grid"), &[&name_words(&self.name), &grid]);
        (name, from)
    }
}

/// The style a new free text takes when none was chosen in the draft: the small style pin designations use
/// (`ew_scene`'s default for a connection point's designation).
pub const NEW_TEXT_STYLE: &str = "cp.small";

/// The style of a free text of `after` that differs from the same text of `before`, or is new with a style: the style
/// a person chose last.
fn chosen_text_style(before: &SymbolDraft, after: &SymbolDraft) -> Option<String> {
    let styles = |d: &SymbolDraft| -> Vec<Option<String>> {
        serde_json::to_value(&d.symbol.graphics)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .map(|g| g.get("text")?.get("style")?.as_str().map(str::to_owned))
            .collect()
    };
    let (old, new) = (styles(before), styles(after));
    new.iter()
        .enumerate()
        .find(|(i, s)| s.is_some() && old.get(*i) != Some(s))
        .and_then(|(_, s)| s.clone())
}

/// A text style's name in the window's words: `text-style-<key>`, else the key itself (a project's own
/// style).
pub fn text_style_words(key: &str) -> String {
    strings::lookup(&format!("text-style-{}", key.replace(['.', '_'], "-")))
        .map_or_else(|| key.to_owned(), str::to_owned)
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

/// A refusal in the window's words (part 3's nit: never a raw member name).
/// The first level of the category of the FnDef `fndef` names, as the library resolves it; empty when it does not.
fn category_of(registry: &ew_api::library::Registry, fndef: &symbols::schema::LibRef) -> String {
    match registry.resolve(fndef) {
        Ok(symbols::schema::LibDocument::Fndef(f)) => f.item.category.0.as_str().to_owned(),
        _ => String::new(),
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
fn nm(p: &Map<String, Value>, name: &str) -> Result<i64, String> {
    let value = p
        .get(name)
        .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
        .ok_or_else(|| strings::text(key!("symbol-coordinate-required")).to_owned())?;
    if !value.is_finite() || value.abs() > 1_000_000.0 {
        return Err(strings::text(key!("symbol-geometry-limit")).into());
    }
    Ok((value * 1_000_000.0).round() as i64)
}

/// A symbol error in the window's words: an invalid draft's issues one per line, a refusal key of the reader in its
/// text, a taken key named, anything else as it reads.
fn symbol_error(e: symbols::Error) -> String {
    match e {
        symbols::Error::Invalid(issues) => issues
            .iter()
            .map(|i| strings::format(strings::Key::run_time(i.code), &[&i.detail]))
            .collect::<Vec<_>>()
            .join("\n"),
        symbols::Error::Input(s)
            if (s.starts_with("symbol-") || s.starts_with("eplan-")) && !s.contains(' ') =>
        {
            strings::text(strings::Key::run_time(&s)).to_owned()
        }
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
        symbols::Error::Input(s)
            if !(s.starts_with("symbol-") || s.starts_with("eplan-")) || s.contains(' ') =>
        {
            strings::text(words).to_owned()
        }
        e => symbol_error(e),
    }
}

/// The error of an edit to the draft in the window's words.
// An edit the reader refuses is refused in words, never in the parser's (a text with no
// characters, say).
fn edit_error(e: symbols::Error) -> String {
    match e {
        symbols::Error::Input(s) if !s.starts_with("symbol-") || s.contains(' ') => {
            words(key!("symbol-change-invalid"))
        }
        e => symbol_error(e),
    }
}

impl ShellModel {
    /// The symbol editor while its view shows, independent of the current project.
    pub fn symbol_editor(&self) -> Option<&SymbolEditor> {
        self.symbol_editor.as_ref().filter(|e| e.shown)
    }

    /// The symbol editor's draft and history, also while the view is left (Close keeps them).
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

    /// New/import/replace/edit/history/save/place share this action implementation.
    pub(crate) fn act_symbol(&mut self, action: &str, p: &Map<String, Value>) -> Outcome {
        // In the window a missing path opens the house file or folder picker.
        let prompt = match action {
            "open" => Some(("symbol.open", Prompt::OpenSymbol, "file")),
            "export" => Some(("symbol.export", Prompt::SaveSymbol, "file")),
            "directory" => Some(("symbol.directory", Prompt::LibraryFolder, "directory")),
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
                        None => editor.error = Some(message.clone()),
                    }
                }
                failed(key!("symbol-operation-failed"), &[&message])
            }
        }
    }

    /// The symbol `symbol.edit` and `symbol.copy_edit` act on (FT-1694): the one named by `symbol` (its key, copy ID or
    /// `library:<library>:<item>` address), else the one the browser's current row places.
    fn edit_target(&self, p: &Map<String, Value>) -> Result<EditTarget, String> {
        let named = text(p, "symbol");
        let given = if named.is_empty() {
            let picker = self.tools.picker.as_ref();
            let row = picker.and_then(|k| Some((k, *k.rows.get(k.row)?)));
            let symbol = row.and_then(|(k, r)| k.row_symbol(r));
            match symbol {
                Some(s) => s.copy.map_or_else(|| s.key.clone(), |c| c.to_string()),
                None => return Err(strings::text(key!("reason-no-picker-row")).to_owned()),
            }
        } else {
            named.to_owned()
        };
        let fallback;
        let registry = match &self.project {
            Some(project) => project.session.libraries(),
            None => {
                fallback = match &self.configured_libraries() {
                    Some(tiers) => ew_api::library::Registry::open(tiers).map_err(|e| e.to_string())?,
                    None => ew_api::library::Registry::starter_only(),
                };
                &fallback
            }
        };
        // A copy the project embeds names the library item it was copied from by its key.
        let project_copy = self.project.as_ref().and_then(|project| {
            let q = project.session.query();
            q.symbols()
                .iter()
                .find(|(id, _)| id.to_string() == given)
                .map(|(_, s)| s.clone())
        });
        let wanted = project_copy
            .as_ref()
            .and_then(|s| s.item.meta.key.as_ref().map(ToString::to_string))
            .unwrap_or_else(|| given.clone());
        for library in registry.libraries() {
            for doc in registry.documents(library.id, symbols::schema::Kind::Symbol) {
                let symbols::schema::LibDocument::Symbol(s) = doc else {
                    continue;
                };
                let address = format!("library:{}:{}", s.library_id, s.item_id);
                let key = s
                    .item
                    .meta
                    .key
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                if address != wanted && key != wanted && s.id.to_string() != wanted {
                    continue;
                }
                let folder = match &library.path {
                    Some(path) if library.writable => Ok(path.clone()),
                    _ => Err(strings::format(key!("symbol-edit-read-only"), &[&library.name])),
                };
                return Ok(EditTarget {
                    source: address,
                    key,
                    name: name_words(&s.item.name),
                    folder,
                });
            }
        }
        // A symbol the project holds alone (an import's) is copied from the project.
        let Some(copy) = project_copy else {
            return Err(strings::text(key!("reason-unknown-symbol")).to_owned());
        };
        Ok(EditTarget {
            source: given,
            key: copy
                .item
                .meta
                .key
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            name: name_words(&copy.item.name),
            folder: Err(strings::text(key!("symbol-edit-project-only")).to_owned()),
        })
    }

    /// Why the browser's current row offers Copy and edit alone (FT-1694): its library is read-only, or the project
    /// alone holds it; `None` when Edit symbol is offered too, or the row places no symbol.
    pub fn symbol_edit_refusal(&self) -> Option<String> {
        self.edit_target(&Map::new()).ok()?.folder.err()
    }

    /// Why Edit symbol cannot open the symbol `p` names (`symbol`: its key, copy ID or library address): its library
    /// is read-only, the project alone holds it, or it is not found; `None` when it can (the canvas menu, FT-1693).
    pub(super) fn symbol_edit_reason(&self, p: &Map<String, Value>) -> Option<String> {
        match self.edit_target(p) {
            Ok(t) => t.folder.err(),
            Err(why) => Some(why),
        }
    }

    /// `symbol.edit` and, with `copy`, `symbol.copy_edit` (FT-1694): the symbol editor on the symbol of the browser's
    /// current row (or the one `symbol` names), the browser closed. Edit symbol opens the symbol itself, under its own
    /// key, saving to its library (a new version there); a symbol of a read-only library, or one the project alone
    /// holds, is refused in words. Copy and edit opens a copy under a key made from its name, saving to the user's
    /// library under a free key. Unsaved changes of the kept draft are refused unless `discard` (the window asks first).
    pub(crate) fn act_symbol_edit(&mut self, p: &Map<String, Value>, copy: bool) -> Outcome {
        let target = match self.edit_target(p) {
            Ok(t) => t,
            Err(message) => return Outcome::Failed { message },
        };
        let folder = match (&target.folder, copy) {
            (Err(why), false) => return Outcome::Failed { message: why.clone() },
            (Ok(folder), false) => Some(folder.clone()),
            (_, true) => None,
        };
        let key = if copy {
            key_of_name(&target.name)
        } else {
            target.key.clone()
        };
        if key.is_empty() {
            return failed(key!("symbol-key-required"), &[]);
        }
        let mut params = Map::new();
        params.insert("source".into(), Value::String(target.source));
        params.insert("key".into(), Value::String(key));
        if let Some(discard) = p.get("discard") {
            params.insert("discard".into(), discard.clone());
        }
        let out = self.act_symbol("fork", &params);
        if out != Outcome::Done {
            return out;
        }
        self.tools.picker = None;
        if let Some(editor) = &mut self.symbol_editor {
            match folder {
                // The library that holds the symbol is where Save writes its new version; unchanged, the draft is
                // that library's symbol.
                Some(folder) => {
                    editor.directory = folder.to_string_lossy().into_owned();
                    editor.stored = true;
                    self.settings.record_symbol_edit(text(&params, "source"));
                }
                // A copy is a new item: no identity of its source, so Save gives it a free key of its name.
                None => {
                    editor.draft.identity = None;
                    editor.saved = Some(editor.draft.clone());
                }
            }
        }
        Outcome::Done
    }

    /// The window's way to an editor action that drops the draft (New symbol, Open or import, Fork, a candidate,
    /// Discard draft): with unsaved changes it asks Save symbol, Discard or Cancel in the house dialog first, the
    /// action then running with `discard`; otherwise the action runs at once.
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

    /// After a test placement the editor's view closes and the page shows with the symbol selected; the toast offers
    /// the way back to the kept draft.
    fn after_test_placement(&mut self, scratch: bool) {
        if let Some(editor) = &mut self.symbol_editor {
            editor.shown = false;
        }
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
        let back = Offer::new(NoteAction::BackToEditor, Validity::SymbolDraft);
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
        // The editor steps aside first, as after a test placement: the Place tool works on the project's page.
        if let Some(editor) = &mut self.symbol_editor {
            editor.shown = false;
        }
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
                Outcome::Refused { reason_key } => strings::text(reason_key).to_owned(),
            };
            // The page was made for this symbol alone: a refused placement takes it back.
            self.take_back_new_page(back.as_deref());
            if let Some(editor) = &mut self.symbol_editor {
                editor.shown = true;
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
            Outcome::Refused { reason_key } => return Err(strings::text(reason_key).into()),
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
        if matches!(action, "new" | "open" | "fork" | "close") {
            return self.symbol_open_close(action, p);
        }
        if action == "start" {
            return self.symbol_start(p);
        }
        if action == "scratch" {
            return self.symbol_scratch();
        }
        if action == "prop" {
            return self.symbol_prop(p);
        }
        let configured_libraries = self.configured_libraries();
        // The paths are resolved, and an empty one refused, before the draft is borrowed.
        let file_path = matches!(action, "export")
            .then(|| self.symbol_path(text(p, "file"), key!("symbol-file-required")))
            .transpose()?;
        let directory_path = match action {
            // A Library folder typed empty is refused; Save without one takes the editor's.
            "save" | "directory" => Some(self.symbol_path(
                if action == "save" && text(p, "directory").is_empty() {
                    self.symbol_editor.as_ref().map_or("", |e| e.directory.as_str())
                } else {
                    text(p, "directory")
                },
                key!("symbol-directory-required"),
            )?),
            _ => None,
        };
        let tiers = matches!(action, "save").then(|| self.symbol_tiers()).flatten();
        let editor = self
            .symbol_editor
            .as_mut()
            .ok_or_else(|| strings::text(key!("symbol-not-open")).to_owned())?;
        match action {
            "map_cp" => Self::symbol_map_cp(editor, p)?,
            "discipline" => Self::symbol_discipline(editor, p)?,
            // The potential role of the function pin a connection point draws; `none` clears it (part 3's nit, D6).
            "role" => Self::symbol_role(editor, p)?,
            "path" => Self::symbol_add_path(editor, p)?,
            "candidate" => Self::symbol_candidate(editor, &mut self.symbol_editor_generation, p)?,
            "function" => {
                Self::symbol_function(editor, self.project.as_ref(), configured_libraries.as_ref(), p)?;
            }
            "replace" => Self::symbol_replace(editor, p)?,
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
            "grid" => editor.set_grid(p)?,
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
            "directory" => {
                let path = directory_path.ok_or_else(damaged)?;
                editor.directory = path.to_string_lossy().into_owned();
            }
            "save" => self.symbol_save(directory_path, tiers)?,
            "place" => {
                let draft = editor.draft.clone();
                let variant = editor.variant;
                self.symbol_place(draft, variant, p)?;
            }
            _ => return Err(strings::text(key!("symbol-tool-invalid")).into()),
        }
        Ok(())
    }

    /// New, Open or import, Fork and Close: a new editor with a fresh draft, the kept draft shown again, or the
    /// editor's view left (its draft dropped with `discard`).
    fn symbol_open_close(&mut self, action: &str, p: &Map<String, Value>) -> Result<(), String> {
        let discard = p.get("discard") == Some(&Value::Bool(true));
        // Close leaves the view and keeps the draft and its history; Discard drops them, explicitly.
        if action == "close" {
            if discard {
                self.symbol_editor = None;
            } else if let Some(editor) = &mut self.symbol_editor {
                editor.shown = false;
            }
            return Ok(());
        }
        // Reopening the editor without naming a new symbol returns to the kept draft; `discard` starts a fresh one.
        if action == "new"
            && !discard
            && text(p, "key").is_empty()
            && text(p, "name").is_empty()
            && let Some(editor) = &mut self.symbol_editor
        {
            editor.shown = true;
            return Ok(());
        }
        if self.symbol_editor.as_ref().is_some_and(SymbolEditor::dirty) && !discard {
            return Err(strings::text(key!("symbol-unsaved")).into());
        }
        // No shared default key: a new draft starts without one, an import takes its file's name.
        let key = text(p, "key");
        if action == "fork" && key.is_empty() {
            return Err(strings::text(key!("symbol-key-required")).into());
        }
        let (draft, candidates) = self.symbol_start_draft(action, key, p)?;
        // Without a folder given, Save to library saves to the default user library, which the field
        // shows, so "Ready to save" holds.
        let dir = self
            .symbol_tiers()
            .map(|t| t.user.join("symbols.ewlib"))
            .map(|p| std::path::absolute(&p).unwrap_or(p))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut editor = SymbolEditor::new(draft, dir);
        if let Some(before) = &self.symbol_editor {
            (editor.grid, editor.snap) = (before.grid, before.snap);
        }
        self.symbol_editor_generation = self.symbol_editor_generation.wrapping_add(1);
        editor.generation = self.symbol_editor_generation;
        editor.candidates = candidates;
        // Opened with nothing in hand and nothing named, the editor shows the ways in first.
        editor.start = action == "new" && !discard && key.is_empty() && text(p, "name").is_empty();
        self.symbol_editor_sources(&mut editor)?;
        self.symbol_editor = Some(editor);
        Ok(())
    }

    /// `symbol.start {leave?}`: shows the start card in the drawing's place, the editor opened when it was not; with
    /// `leave`, back to the draft in hand, and with nothing in hand the editor closes.
    fn symbol_start(&mut self, p: &Map<String, Value>) -> Result<(), String> {
        let leave = p.get("leave") == Some(&Value::Bool(true));
        match &mut self.symbol_editor {
            Some(editor) if leave && editor.in_hand() => editor.start = false,
            Some(_) if leave => self.symbol_editor = None,
            Some(editor) => {
                editor.shown = true;
                editor.start = true;
                // The card lists what the libraries hold now: a symbol saved since the editor opened too.
                self.refresh_symbol_sources()?;
            }
            None if leave => {}
            None => return self.symbol_open_close("new", &Map::new()),
        }
        Ok(())
    }

    /// Reads the editor's choices (the symbols to copy, the ready-made functions) again.
    fn refresh_symbol_sources(&mut self) -> Result<(), String> {
        let Some(mut editor) = self.symbol_editor.take() else {
            return Ok(());
        };
        editor.sources.clear();
        editor.functions.clear();
        let read = self.symbol_editor_sources(&mut editor);
        self.symbol_editor = Some(editor);
        read
    }

    /// The symbols edited lately that a library still holds, most recent first: each as (its address for
    /// `symbol.edit`, its name, its library).
    pub fn symbol_recents(&self) -> Vec<(String, String, String)> {
        let Some(editor) = self.symbol_editor() else {
            return Vec::new();
        };
        self.settings
            .symbols_recent
            .iter()
            .filter_map(|address| {
                let source = editor.sources.iter().find(|s| s.source == *address)?;
                let (name, library) = source.words();
                Some((address.clone(), name, library))
            })
            .collect()
    }

    /// The draft a new editor starts with, and the other drawings of an imported file: the file's first drawing
    /// (Open), a copy of a project or library symbol under `key` (Fork), else a new empty draft.
    fn symbol_start_draft(
        &self,
        action: &str,
        key: &str,
        p: &Map<String, Value>,
    ) -> Result<(SymbolDraft, Vec<SymbolDraft>), String> {
        let mut candidates = Vec::new();
        let draft = if action == "open" {
            let path = self.symbol_path(text(p, "file"), key!("symbol-file-required"))?;
            let key = if key.is_empty() {
                symbols::file_key(&path)
            } else {
                key.to_owned()
            };
            candidates =
                symbols::open(&path, &key).map_err(|e| file_error(e, key!("symbol-file-unreadable")))?;
            candidates
                .first()
                .cloned()
                .ok_or_else(|| strings::text(key!("eplan-no-symbols")).to_owned())?
        } else if action == "fork" {
            if let Some(project) = &self.project {
                project
                    .session
                    .symbol_draft(text(p, "source"), key)
                    .map_err(symbol_error)?
            } else {
                let registry = match &self.configured_libraries() {
                    Some(tiers) => ew_api::library::Registry::open(tiers).map_err(|e| e.to_string())?,
                    None => ew_api::library::Registry::starter_only(),
                };
                SymbolDraft::from_library(&registry, text(p, "source"), key).map_err(symbol_error)?
            }
        } else {
            let name = if text(p, "name").is_empty() {
                strings::text(key!("symbol-untitled"))
            } else {
                text(p, "name")
            };
            SymbolDraft::new(key, name).map_err(symbol_error)?
        };
        Ok((draft, candidates))
    }

    /// A new editor's choices: the project's embedded symbols, every library's symbols, and the libraries' function
    /// definitions sorted by key.
    fn symbol_editor_sources(&self, editor: &mut SymbolEditor) -> Result<(), String> {
        let fallback;
        editor.box_sources = self.read_box_parts();
        let registry = if let Some(project) = &self.project {
            let libraries = project.session.libraries();
            for (id, s) in project.session.query().symbols().iter() {
                editor.sources.push(SymbolSource {
                    source: id.to_string(),
                    library: None,
                    key: s.item.meta.key.as_ref().map(ToString::to_string),
                    name: s.item.name.clone(),
                    grid: s.item.grid,
                    category: category_of(libraries, &s.item.fndef),
                    copy: Some(id.as_uuid()),
                });
            }
            project.session.libraries()
        } else {
            fallback = match &self.configured_libraries() {
                Some(tiers) => ew_api::library::Registry::open(tiers).map_err(|e| e.to_string())?,
                None => ew_api::library::Registry::starter_only(),
            };
            &fallback
        };
        for library in registry.libraries() {
            for doc in registry.documents(library.id, symbols::schema::Kind::Symbol) {
                if let symbols::schema::LibDocument::Symbol(s) = doc {
                    editor.sources.push(SymbolSource {
                        source: format!("library:{}:{}", s.library_id, s.item_id),
                        library: Some(library.name.clone()),
                        key: s.item.meta.key.as_ref().map(ToString::to_string),
                        category: category_of(registry, &s.item.fndef),
                        copy: Some(s.id.as_uuid()),
                        name: s.item.name,
                        grid: s.item.grid,
                    });
                }
            }
            for doc in registry.documents(library.id, symbols::schema::Kind::Fndef) {
                if let symbols::schema::LibDocument::Fndef(f) = doc
                    && let Some(key) = &f.item.meta.key
                {
                    editor.functions.push(SymbolFunctionChoice {
                        source: format!("library:{}:{}", f.library_id, f.item_id),
                        library: library.name.clone(),
                        key: key.to_string(),
                        kind: f.item.contact_kind,
                        category: format!(
                            "{} / {} / {}",
                            f.item.category.0, f.item.category.1, f.item.category.2
                        ),
                        pins: f.item.cps.iter().map(|p| p.designation.to_string()).collect(),
                    });
                }
            }
        }
        editor.functions.sort_by(|a, b| a.key.cmp(&b.key));
        editor.functions.dedup_by(|a, b| a.source == b.source);
        // In the symbol browser's order (FT-1810: Copy a library symbol was one long list without order): by the
        // words of the function category the browser files a symbol under, Other last, then by name.
        editor.sources.sort_by_cached_key(|s| {
            let filed = crate::tools::category_words(&s.category);
            (
                filed.is_none(),
                filed.unwrap_or_default().to_lowercase(),
                s.words().0.to_lowercase(),
            )
        });
        Ok(())
    }

    /// The shape of the start card's symbol `source` for its preview (FT-1810): the copy's outline and connection
    /// points as the symbol browser draws them; `None` without a project or for a symbol no copy is read for.
    pub fn symbol_source_shape(&self, source: &SymbolSource) -> Option<ew_api::output::SymbolShape> {
        self.project.as_ref()?.session.symbol_shape(source.copy?)
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

    /// Sets the potential role of the function pin connection point `cp` draws, or clears it with `none`; refused
    /// while the draft uses a library's function definition.
    fn symbol_role(editor: &mut SymbolEditor, p: &Map<String, Value>) -> Result<(), String> {
        let cp = p.get("cp").and_then(Value::as_u64);
        let fn_cp = editor
            .draft
            .symbol
            .cps
            .iter()
            .find(|c| Some(u64::from(c.idx)) == cp)
            .map(|c| u64::from(c.fn_cp))
            .ok_or_else(|| {
                strings::format(key!("symbol-cp-unknown"), &[&cp.unwrap_or_default().to_string()])
            })?;
        if editor.draft.function_ref.is_some() {
            return Err(words(key!("symbol-role-assigned")));
        }
        let mut d = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
        let pin = d["function"]["cps"]
            .as_array_mut()
            .and_then(|pins| pins.iter_mut().find(|v| v["idx"] == fn_cp))
            .and_then(Value::as_object_mut)
            .ok_or_else(damaged)?;
        match text(p, "role") {
            "none" => {
                pin.remove("potential_role");
            }
            role => {
                pin.insert("potential_role".into(), json!(role));
            }
        }
        editor.replace(
            SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(edit_error)?,
        );
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

    /// Gives the draft the library function definition `key` (its library address or its own key), referenced and
    /// copied into the draft. An empty `key` takes an assigned definition off (Take off, F24): the draft keeps its
    /// copy, whose function type and connection points can be changed again.
    fn symbol_function(
        editor: &mut SymbolEditor,
        project: Option<&super::Project>,
        configured_libraries: Option<&ew_api::library::LibraryTiers>,
        p: &Map<String, Value>,
    ) -> Result<(), String> {
        if text(p, "key").is_empty() && editor.draft.function_ref.is_some() {
            let mut draft = editor.draft.clone();
            draft.function_ref = None;
            editor.replace(draft);
            return Ok(());
        }
        let fallback;
        let registry = if let Some(project) = project {
            project.session.libraries()
        } else {
            fallback = match configured_libraries {
                Some(tiers) => ew_api::library::Registry::open(tiers).map_err(|e| e.to_string())?,
                None => ew_api::library::Registry::starter_only(),
            };
            &fallback
        };
        let f = registry
            .libraries()
            .iter()
            .flat_map(|l| registry.documents(l.id, symbols::schema::Kind::Fndef))
            .find_map(|d| match d {
                symbols::schema::LibDocument::Fndef(f)
                    if format!("library:{}:{}", f.library_id, f.item_id) == text(p, "key")
                        || f.item
                            .meta
                            .key
                            .as_ref()
                            .is_some_and(|k| k.as_str() == text(p, "key")) =>
                {
                    Some(f)
                }
                _ => None,
            })
            .ok_or_else(|| strings::text(key!("symbol-function-required")).to_owned())?;
        let mut draft = editor.draft.clone();
        draft.function_ref = Some(
            serde_json::from_value(
                json!({"library":f.library_id,"item":f.item_id,"version_req":format!("^{}",f.version)}),
            )
            .map_err(|e| e.to_string())?,
        );
        draft.function = f.item;
        editor.replace(draft);
        Ok(())
    }

    /// Replaces the draft with the whole `document`, keeping the text style it chose last.
    fn symbol_replace(editor: &mut SymbolEditor, p: &Map<String, Value>) -> Result<(), String> {
        let bytes = serde_json::to_vec(
            p.get("document")
                .ok_or_else(|| words(key!("symbol-document-required")))?,
        )
        .map_err(|e| e.to_string())?;
        let draft = SymbolDraft::parse(&bytes).map_err(edit_error)?;
        if let Some(style) = chosen_text_style(&editor.draft, &draft) {
            editor.text_style = Some(style);
        }
        editor.replace(draft);
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
        let mut cp = old.unwrap_or(json!({"idx":index,"fn_cp":index,"dir":"left","discipline":discipline}));
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

    /// Save to library: the valid draft published to the library at `directory_path` (made when missing), the tiers
    /// and settings updated, and the draft and its history marked as saved there.
    fn symbol_save(
        &mut self,
        directory_path: Option<PathBuf>,
        tiers: Option<ew_api::library::LibraryTiers>,
    ) -> Result<(), String> {
        let editor = self
            .symbol_editor
            .as_mut()
            .ok_or_else(|| strings::text(key!("symbol-not-open")).to_owned())?;
        let mut draft = editor.draft.clone();
        let path = directory_path.ok_or_else(damaged)?;
        let directory = path.to_string_lossy().into_owned();
        let issues = draft.validate();
        if !issues.is_empty() {
            return Err(symbol_error(symbols::Error::Invalid(issues)));
        }
        let mut tiers = tiers.ok_or_else(|| strings::text(key!("symbol-directory-required")).to_owned())?;
        // A library of the user tier opens from it; any other destination is a writable company path.
        let user_tier = path.extension().is_some_and(|e| e == "ewlib")
            && path.parent() == std::path::absolute(&tiers.user).ok().as_deref();
        if !user_tier && !tiers.company.contains(&path) {
            tiers.company.push(path.clone());
        }
        let mut registry = ew_api::library::Registry::open(&tiers).map_err(|e| e.to_string())?;
        draft.check_function(&registry).map_err(symbol_error)?;
        let target = match registry
            .libraries()
            .into_iter()
            .find(|l| l.path.as_ref() == Some(&path))
        {
            Some(l) => l.id,
            None => registry
                .create(
                    &path,
                    "user-symbols",
                    strings::text(key!("symbol-user-library")),
                    draft.symbol.meta.licence.as_deref().unwrap_or("CC-BY-SA-4.0"),
                )
                .map_err(|e| e.to_string())?,
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
                    if tries < 1000 && next_patch(&draft.version).is_some() =>
                {
                    draft.version = next_patch(&draft.version).unwrap_or_default();
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
        if let Some(project) = &mut self.project {
            *project.session.libraries_mut() = registry;
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
        editor.directory = directory;
        // The published symbol as the Insert symbol picker addresses it, while the draft is this one.
        editor.published = draft
            .identity
            .as_ref()
            .map(|i| (draft.clone(), format!("library:{target}:{}", i.symbol)));
        editor.saved = Some(draft);
        editor.stored = true;
        editor.error = None;
        if let Some((_, address)) = &editor.published {
            self.settings.record_symbol_edit(address);
        }
        if !user_tier {
            self.settings
                .remember_library(&path, self.config.config_dir.as_deref());
        }
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
        Ok(())
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
            Outcome::Refused { reason_key } => return Err(strings::text(reason_key).into()),
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
                Outcome::Refused { reason_key } => return Err(strings::text(reason_key).into()),
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
