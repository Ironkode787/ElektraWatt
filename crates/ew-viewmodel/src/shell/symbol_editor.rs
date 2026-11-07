// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol editor's independent draft, history and GUI/headless actions.

use super::{Mode, Outcome, Prompt, ShellModel, failed};
use crate::notifications::{NoteAction, Validity};
use crate::{key, strings};
use ew_api::symbols::{self, SymbolDraft};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

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

/// The editor tools that take a binding of the keymap while the editor shows (doc 16a P8): the main window's drawing
/// action whose key chooses the tool. Escape (`ui.dismiss`) returns to Select.
pub const TOOL_KEYS: [(&str, &str); 5] = [
    ("select", "ui.dismiss"),
    ("line", "tool.graphic"),
    ("rectangle", "tool.black_box"),
    ("text", "tool.text"),
    ("cp", "tool.ip"),
];

/// The files the symbol editor opens or imports, as its file picker offers them (M25-p5#5): the authoring JSON,
/// QElectroTech elements and every EPLAN format `symbols::open` reads.
pub const FILE_EXTENSIONS: [&str; 11] = [
    "json", "elmt", "edz", "pxf", "ema", "ems", "esl", "slk", "xml", "zip", "7z",
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
    /// Whether the editor's view shows; a draft left with Close is kept with its history (ID-1463).
    pub(crate) shown: bool,
    undo: Vec<SymbolDraft>,
    redo: Vec<SymbolDraft>,
    /// Current orientation of the live preview (A–H).
    pub variant: u8,
    /// The drawing tool chosen, one of [`TOOLS`].
    pub tool: &'static str,
    /// Last operation's error, also shown under the editor controls.
    pub error: Option<String>,
    /// Whether that error is the page's other connection grid, which offers Place on a new page with its grid
    /// (ID-1477 (1)).
    pub offer_new_page: bool,
    /// The text style last chosen for a text of the draft, which a new text takes (ID-1477 (2)).
    pub text_style: Option<String>,
    /// Suggested writable library directory.
    pub directory: String,
    /// Other drawings recovered from the same input; selecting one is an explicit replacement.
    pub candidates: Vec<SymbolDraft>,
    /// Available embedded and installed symbols, read when the editor opens.
    pub sources: Vec<SymbolSource>,
    /// Installed function definitions, read when the editor opens.
    pub functions: Vec<SymbolFunctionChoice>,
}

impl SymbolEditor {
    fn new(draft: SymbolDraft, directory: String) -> Self {
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
            text_style: None,
            directory,
            candidates: Vec::new(),
            sources: Vec::new(),
            functions: Vec::new(),
        }
    }
    /// Whether the draft differs from its last saved state: a new, forked or imported draft is not, until edited.
    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.draft)
    }
    /// Whether the draft, unchanged, was exported or published; an unedited new or imported draft is in no library.
    pub fn stored(&self) -> bool {
        self.stored && !self.dirty()
    }
    /// The key of the words naming the draft's state: unsaved changes, saved, or not saved yet.
    pub fn state_key(&self) -> strings::Key<'static> {
        if self.dirty() {
            key!("symbol-dirty")
        } else if self.stored {
            key!("symbol-saved")
        } else {
            key!("symbol-not-saved")
        }
    }
    /// Whether editor undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    /// Whether editor redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    fn replace(&mut self, draft: SymbolDraft) {
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
    }
}

/// The style a new free text takes when none was chosen in the draft: the small style pin designations use
/// (ID-1477 (2); `ew_scene`'s default for a connection point's designation).
pub const NEW_TEXT_STYLE: &str = "cp.small";

/// The style of a free text of `after` that differs from the same text of `before`, or is new with a style: the style
/// a person chose last (ID-1477 (2)).
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

/// A text style's name in the window's words (ID-1477 (2)): `text-style-<key>`, else the key itself (a project's own
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

impl ShellModel {
    /// The symbol editor while its view shows, independent of the current project.
    pub fn symbol_editor(&self) -> Option<&SymbolEditor> {
        self.symbol_editor.as_ref().filter(|e| e.shown)
    }

    /// The symbol editor's draft and history, also while the view is left (Close keeps them, ID-1463).
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
    /// resolves against the default user library's folder, never the working directory (ID-1463).
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
        // ID-1463: in the window a missing path opens the house file or folder picker.
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
        let result = self.symbol_operation(action, p);
        match result {
            Ok(()) => {
                if matches!(action, "place" | "scratch") {
                    self.after_test_placement(action == "scratch");
                }
                Outcome::Done
            }
            Err(message) => {
                if let Some(editor) = &mut self.symbol_editor {
                    editor.error = Some(message.clone());
                }
                failed(key!("symbol-operation-failed"), &[&message])
            }
        }
    }

    /// The window's way to an editor action that drops the draft (New symbol, Open or import, Fork, a candidate,
    /// Discard draft): with unsaved changes it asks Save symbol, Discard or Cancel in the house dialog first, the
    /// action then running with `discard`; otherwise the action runs at once (ID-1472).
    pub fn ask_symbol_discard(&mut self, action: &'static str, params: Value) -> Outcome {
        if self.symbol_editor.as_ref().is_some_and(SymbolEditor::dirty) {
            return self.open_dialog(super::Dialog::ConfirmDiscard {
                pending: super::Pending { action, params },
                symbol: true,
            });
        }
        self.dispatch(action, &params)
    }

    /// After a test placement the editor's view closes and the page shows with the symbol selected; the toast offers
    /// the way back to the kept draft (ID-1463).
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
        self.notes.push(
            text,
            "symbol",
            Some((NoteAction::BackToEditor, Validity::SymbolDraft)),
        );
    }

    /// A new page with the connection grid `grid` (`{num, den}`), at the first free whole folio, titled `title`, then
    /// shown: Test on a new page's and Place on a new page with its grid's page (ID-1477 (1)).
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
        let err = |e: symbols::Error| match e {
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
        };
        if matches!(action, "new" | "open" | "fork" | "close") {
            let discard = p.get("discard") == Some(&Value::Bool(true));
            // ID-1463: Close leaves the view and keeps the draft and its history; Discard drops them, explicitly.
            if action == "close" {
                if discard {
                    self.symbol_editor = None;
                } else if let Some(editor) = &mut self.symbol_editor {
                    editor.shown = false;
                }
                return Ok(());
            }
            // Reopening the editor without naming a new symbol returns to the kept draft; `discard` starts a fresh one
            // (ID-1472).
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
            // No shared default key (ID-1463): a new draft starts without one, an import takes its file's name.
            let key = text(p, "key");
            if action == "fork" && key.is_empty() {
                return Err(strings::text(key!("symbol-key-required")).into());
            }
            let mut candidates = Vec::new();
            let draft = if action == "open" {
                let path = self.symbol_path(text(p, "file"), key!("symbol-file-required"))?;
                let key = if key.is_empty() {
                    symbols::file_key(&path)
                } else {
                    key.to_owned()
                };
                candidates = symbols::open(&path, &key).map_err(err)?;
                candidates
                    .first()
                    .cloned()
                    .ok_or_else(|| strings::text(key!("eplan-no-symbols")).to_owned())?
            } else if action == "fork" {
                if let Some(project) = &self.project {
                    project
                        .session
                        .symbol_draft(text(p, "source"), key)
                        .map_err(err)?
                } else {
                    let registry = match &self.configured_libraries() {
                        Some(tiers) => ew_api::library::Registry::open(tiers).map_err(|e| e.to_string())?,
                        None => ew_api::library::Registry::starter_only(),
                    };
                    SymbolDraft::from_library(&registry, text(p, "source"), key).map_err(err)?
                }
            } else {
                let name = if text(p, "name").is_empty() {
                    strings::text(key!("symbol-untitled"))
                } else {
                    text(p, "name")
                };
                SymbolDraft::new(key, name).map_err(err)?
            };
            // ID-1477 (5): without a folder given, Save to library saves to the default user library, which the field
            // shows, so "Ready to save" holds.
            let dir = self
                .symbol_tiers()
                .map(|t| t.user.join("symbols.ewlib"))
                .map(|p| std::path::absolute(&p).unwrap_or(p))
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut editor = SymbolEditor::new(draft, dir);
            self.symbol_editor_generation = self.symbol_editor_generation.wrapping_add(1);
            editor.generation = self.symbol_editor_generation;
            editor.candidates = candidates;
            let fallback;
            let registry = if let Some(project) = &self.project {
                for (id, s) in project.session.query().symbols().iter() {
                    editor.sources.push(SymbolSource {
                        source: id.to_string(),
                        library: None,
                        key: s.item.meta.key.as_ref().map(ToString::to_string),
                        name: s.item.name.clone(),
                        grid: s.item.grid,
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
            self.symbol_editor = Some(editor);
            return Ok(());
        }
        if action == "scratch" {
            let draft = &self
                .symbol_editor
                .as_ref()
                .ok_or_else(|| strings::text(key!("symbol-not-open")).to_owned())?
                .draft;
            draft.documents(symbols::DRAFT_LIBRARY).map_err(err)?;
            let project = self
                .project
                .as_ref()
                .ok_or_else(|| strings::text(key!("reason-no-project")).to_owned())?;
            draft.check_function(project.session.libraries()).map_err(err)?;
            let title = strings::format(key!("symbol-scratch-title"), &[&name_words(&draft.symbol.name)]);
            let grid = json!(draft.symbol.grid);
            self.new_page_with_grid(grid, Some(title))?;
            return self.symbol_operation("place", json!({"x":40,"y":40}).as_object().ok_or_else(damaged)?);
        }
        let configured_libraries = self.configured_libraries();
        // The paths are resolved, and an empty one refused, before the draft is borrowed (ID-1463).
        let file_path = matches!(action, "export")
            .then(|| self.symbol_path(text(p, "file"), key!("symbol-file-required")))
            .transpose()?;
        let directory_path = match action {
            // A Library folder typed empty is refused; Save without one takes the editor's (ID-1477 (5)).
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
            "map_cp" => {
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
            }
            // The potential role of the function pin a connection point draws; `none` clears it (part 3's nit, D6).
            "role" => {
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
                    SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(err)?,
                );
            }
            "path" => {
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
                    .push(
                        json!({kind:{"points":converted,"closed":kind=="polygon","stroke":{"pen":"p026"}}}),
                    );
                editor.replace(
                    SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(err)?,
                );
            }
            "candidate" => {
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
                self.symbol_editor_generation = self.symbol_editor_generation.wrapping_add(1);
                editor.generation = self.symbol_editor_generation;
            }
            "function" => {
                let fallback;
                let registry = if let Some(project) = &self.project {
                    project.session.libraries()
                } else {
                    fallback = match &configured_libraries {
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
                draft.function_ref = Some(serde_json::from_value(json!({"library":f.library_id,"item":f.item_id,"version_req":format!("^{}",f.version)})).map_err(|e| e.to_string())?);
                draft.function = f.item;
                editor.replace(draft);
            }
            "replace" => {
                let bytes = serde_json::to_vec(
                    p.get("document")
                        .ok_or_else(|| words(key!("symbol-document-required")))?,
                )
                .map_err(|e| e.to_string())?;
                let draft = SymbolDraft::parse(&bytes).map_err(err)?;
                if let Some(style) = chosen_text_style(&editor.draft, &draft) {
                    editor.text_style = Some(style);
                }
                editor.replace(draft);
            }
            "undo" => {
                if let Some(d) = editor.undo.pop() {
                    editor.redo.push(std::mem::replace(&mut editor.draft, d));
                    editor.error = None;
                }
            }
            "redo" => {
                if let Some(d) = editor.redo.pop() {
                    editor.undo.push(std::mem::replace(&mut editor.draft, d));
                    editor.error = None;
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
                    return Err(err(symbols::Error::Invalid(issues)));
                }
                editor.error = None;
            }
            "draw" => {
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
                    // ID-1477 (2): centred on its click, in the style last chosen, else the pin designations' small one.
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
                    SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(err)?,
                );
            }
            "cp" => {
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
                let mut cp = points
                    .iter()
                    .find(|v| v["idx"] == index)
                    .cloned()
                    .unwrap_or(json!({"idx":index,"fn_cp":index,"dir":"left"}));
                if let Some(mapping) = p.get("fn_cp") {
                    cp["fn_cp"] = mapping.clone();
                }
                if !text(p, "direction").is_empty() {
                    cp["dir"] = json!(text(p, "direction"));
                }
                cp["pos"] = json!([nm(p, "x")?, nm(p, "y")?]);
                let fn_cp = cp["fn_cp"]
                    .as_u64()
                    .ok_or_else(|| words(key!("symbol-function-pin-invalid")))?;
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
                        .unwrap_or(json!({"idx":fn_cp,"designation":index.to_string()}));
                    if !text(p, "designation").is_empty() {
                        fcp["designation"] = json!(text(p, "designation"));
                    }
                    fpoints.retain(|v| v["idx"] != fn_cp);
                    fpoints.push(fcp);
                    fpoints.sort_by_key(|v| v["idx"].as_u64());
                }
                editor.replace(
                    SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(err)?,
                );
            }
            "slot" => {
                let mut d = serde_json::to_value(&editor.draft).map_err(|e| e.to_string())?;
                let slots = d["symbol"]
                    .as_object_mut()
                    .ok_or_else(damaged)?
                    .entry("text_slots")
                    .or_insert(json!([]))
                    .as_array_mut()
                    .ok_or_else(damaged)?;
                let kind = p.get("kind").cloned().unwrap_or(json!("tag"));
                slots.push(
                    json!({"kind":kind,"pos":[nm(p,"x")?,nm(p,"y")?],"align":"top_left","visible":true}),
                );
                editor.replace(
                    SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(err)?,
                );
            }
            "link" => {
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
                    SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(err)?,
                );
            }
            "export" => {
                let path = file_path.ok_or_else(damaged)?;
                symbols::export(&editor.draft, &path).map_err(err)?;
                editor.saved = Some(editor.draft.clone());
                editor.stored = true;
            }
            "directory" => {
                let path = directory_path.ok_or_else(damaged)?;
                editor.directory = path.to_string_lossy().into_owned();
            }
            "save" => {
                let mut draft = editor.draft.clone();
                let path = directory_path.ok_or_else(damaged)?;
                let directory = path.to_string_lossy().into_owned();
                let issues = draft.validate();
                if !issues.is_empty() {
                    return Err(err(symbols::Error::Invalid(issues)));
                }
                let mut tiers =
                    tiers.ok_or_else(|| strings::text(key!("symbol-directory-required")).to_owned())?;
                // A library of the user tier opens from it; any other destination is a writable company path.
                let user_tier = path.extension().is_some_and(|e| e == "ewlib")
                    && path.parent() == std::path::absolute(&tiers.user).ok().as_deref();
                if !user_tier && !tiers.company.contains(&path) {
                    tiers.company.push(path.clone());
                }
                let mut registry = ew_api::library::Registry::open(&tiers).map_err(|e| e.to_string())?;
                draft.check_function(&registry).map_err(err)?;
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
                // M25-p4#6: the writer's version refusals in words, the item named by the symbol, without its UUID.
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
                draft.publish(&mut registry, target).map_err(|e| match e {
                    symbols::Error::Library(ew_api::library::LibraryError::Immutable {
                        item: id,
                        version,
                    }) => strings::format(key!("symbol-version-taken"), &[&item(id), &version]),
                    symbols::Error::Library(ew_api::library::LibraryError::Downgrade {
                        item: id,
                        version,
                        held,
                    }) => strings::format(key!("symbol-version-older"), &[&item(id), &version, &held]),
                    e => err(e),
                })?;
                // A key other than the one the library holds for the draft's items published new ones (ID-1474).
                let note = (before.is_some() && before != draft.identity)
                    .then(|| strings::format(key!("symbol-saved-new"), &[&draft.key]));
                self.config.libraries = Some(tiers);
                if let Some(project) = &mut self.project {
                    *project.session.libraries_mut() = registry;
                }
                // The draft's history publishes the same items from now on.
                let history = editor.undo.iter_mut().chain(editor.redo.iter_mut());
                for d in history.chain([&mut editor.draft]) {
                    if d.key == draft.key {
                        d.identity.clone_from(&draft.identity);
                    }
                }
                editor.directory = directory;
                editor.saved = Some(draft);
                editor.stored = true;
                editor.error = None;
                if !user_tier {
                    self.settings
                        .remember_library(&path, self.config.config_dir.as_deref());
                }
                self.save_settings();
                self.libraries_changed();
                if let Some(note) = note {
                    self.notify(note, false);
                }
            }
            "place" => {
                let mut draft = editor.draft.clone();
                let variant = editor.variant;
                if let Some(project) = &self.project {
                    draft.check_function(project.session.libraries()).map_err(err)?;
                }
                // Each content placed embeds its own copies, after a draft edit, a reopen or a trimmed history too: the
                // suffix is derived from the content (M25-p3#6).
                let suffix = content_suffix(&draft);
                draft.key = format!(
                    "{}.test.{suffix}",
                    draft.key.chars().take(100).collect::<String>()
                );
                // The same content placed again is the same copy (re-review part 4): its item IDs come from its key,
                // which carries the content's suffix, never a new UUIDv7 per placement.
                let item = |kind: &str| {
                    ew_api::output::Uuid::new_v5(
                        &symbols::DRAFT_LIBRARY,
                        format!("{kind}|{}", draft.key).as_bytes(),
                    )
                    .to_string()
                };
                let fndef = draft.function_ref.is_none().then(|| item("fndef"));
                draft.identity = Some(
                    serde_json::from_value(json!({"symbol": item("symbol"), "fndef": fndef}))
                        .map_err(|e| e.to_string())?,
                );
                let mut docs = draft.documents(symbols::DRAFT_LIBRARY).map_err(err)?;
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
                let page = self
                    .current_page()
                    .ok_or_else(|| strings::text(key!("reason-no-page")).to_owned())?;
                let canvas = self
                    .canvas()
                    .ok_or_else(|| strings::text(key!("reason-no-page")).to_owned())?;
                let snapper = &canvas.page().snapper;
                let (num, den) = snapper.grid();
                if (num, den) != (draft.symbol.grid.num() as i64, draft.symbol.grid.den() as i64) {
                    // ID-1477 (1): the picker's words, both grids named; the editor then offers Place on a new page
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
            }
            _ => return Err(strings::text(key!("symbol-tool-invalid")).into()),
        }
        Ok(())
    }
}
