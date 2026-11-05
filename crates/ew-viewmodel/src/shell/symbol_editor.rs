// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol editor's independent draft, history and GUI/headless actions.

use super::{Outcome, ShellModel, failed};
use crate::{key, strings};
use ew_api::symbols::{self, SymbolDraft};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// Editable document state. History never shares the project's undo stack.
#[derive(Debug)]
pub struct SymbolEditor {
    /// Replacement generation; the view clears transient tool state when this changes.
    pub generation: u64,
    /// Current portable document.
    pub draft: SymbolDraft,
    /// Last successful export/publication; equality determines unsaved changes.
    saved: Option<SymbolDraft>,
    undo: Vec<SymbolDraft>,
    redo: Vec<SymbolDraft>,
    /// Current orientation of the live preview (A–H).
    pub variant: u8,
    /// Last operation's error, also shown under the editor controls.
    pub error: Option<String>,
    /// Suggested writable library directory.
    pub directory: String,
    /// Other drawings recovered from the same input; selecting one is an explicit replacement.
    pub candidates: Vec<SymbolDraft>,
}

impl SymbolEditor {
    fn new(draft: SymbolDraft, directory: String) -> Self {
        Self {
            generation: 0,
            draft,
            saved: None,
            undo: Vec::new(),
            redo: Vec::new(),
            variant: 0,
            error: None,
            directory,
            candidates: Vec::new(),
        }
    }
    /// Whether the draft differs from its last saved state.
    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.draft)
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
    /// The open symbol editor, independent of the current project.
    pub fn symbol_editor(&self) -> Option<&SymbolEditor> {
        self.symbol_editor.as_ref()
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
        let result = self.symbol_operation(action, p);
        match result {
            Ok(()) => Outcome::Done,
            Err(message) => {
                if let Some(editor) = &mut self.symbol_editor {
                    editor.error = Some(message.clone());
                }
                failed(key!("symbol-operation-failed"), &[&message])
            }
        }
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
            e => e.to_string(),
        };
        if matches!(action, "new" | "open" | "fork" | "close") {
            if self.symbol_editor.as_ref().is_some_and(SymbolEditor::dirty)
                && p.get("discard") != Some(&Value::Bool(true))
            {
                return Err(strings::text(key!("symbol-unsaved")).into());
            }
            if action == "close" {
                self.symbol_editor = None;
                return Ok(());
            }
            let key = if text(p, "key").is_empty() {
                "user.symbol"
            } else {
                text(p, "key")
            };
            let mut candidates = Vec::new();
            let draft = if action == "open" {
                let path = Path::new(text(p, "file"));
                candidates = symbols::open(path, key).map_err(err)?;
                candidates
                    .first()
                    .cloned()
                    .ok_or_else(|| strings::text(key!("eplan-no-symbols")).to_owned())?
            } else if action == "fork" {
                let project = self
                    .project
                    .as_ref()
                    .ok_or_else(|| strings::text(key!("reason-no-project")).to_owned())?;
                project
                    .session
                    .symbol_draft(text(p, "source"), key)
                    .map_err(err)?
            } else {
                let name = if text(p, "name").is_empty() {
                    strings::text(key!("symbol-untitled"))
                } else {
                    text(p, "name")
                };
                SymbolDraft::new(key, name).map_err(err)?
            };
            let dir = self
                .config
                .libraries
                .as_ref()
                .map(|t| t.user.join("symbols.ewlib"))
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut editor = SymbolEditor::new(draft, dir);
            editor.generation = self
                .symbol_editor
                .as_ref()
                .map_or(0, |old| old.generation.wrapping_add(1));
            if action == "open" {
                editor.saved = Some(editor.draft.clone());
            }
            editor.candidates = candidates;
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
            let labels = self.page_labels();
            let mut index = 1;
            while labels.iter().any(|l| l == &index.to_string()) {
                index += 1;
            }
            let params =
                json!({"number":index,"title":strings::text(key!("symbol-title")),"grid":draft.symbol.grid});
            match self.host_create("page.create", params) {
                Outcome::Done => {}
                Outcome::Failed { message } => return Err(message),
                Outcome::Refused { reason_key } => return Err(strings::text(reason_key).into()),
            }
            if let Some(id) = self.tools.last_primary.clone() {
                self.tools.selection.clear();
                self.show_page_by_id(&id);
            }
            return self.symbol_operation("place", json!({"x":40,"y":40}).as_object().ok_or("position")?);
        }
        let editor = self
            .symbol_editor
            .as_mut()
            .ok_or_else(|| strings::text(key!("symbol-not-open")).to_owned())?;
        match action {
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
                        nm(params.as_object().ok_or("point")?, "x")?,
                        nm(params.as_object().ok_or("point")?, "y")?,
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
                    .ok_or("symbol")?
                    .entry("graphics")
                    .or_insert(json!([]))
                    .as_array_mut()
                    .ok_or("graphics")?
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
                    .ok_or("index")?;
                let draft = editor.candidates.get(index).cloned().ok_or("index")?;
                editor.replace(draft.clone());
                editor.saved = Some(draft);
                editor.generation = editor.generation.wrapping_add(1);
            }
            "function" => {
                let fallback;
                let registry = if let Some(project) = &self.project {
                    project.session.libraries()
                } else {
                    fallback = match &self.config.libraries {
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
                            if f.item
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
                draft.function_ref = Some(serde_json::from_value(json!({"library":f.library_id,"item":f.item_id,"version_req":format!("={}",f.version)})).map_err(|e| e.to_string())?);
                draft.function = f.item;
                editor.replace(draft);
            }
            "replace" => {
                let bytes =
                    serde_json::to_vec(p.get("document").ok_or("document")?).map_err(|e| e.to_string())?;
                editor.replace(SymbolDraft::parse(&bytes).map_err(err)?);
            }
            "undo" => {
                if let Some(d) = editor.undo.pop() {
                    editor.redo.push(std::mem::replace(&mut editor.draft, d));
                }
            }
            "redo" => {
                if let Some(d) = editor.redo.pop() {
                    editor.undo.push(std::mem::replace(&mut editor.draft, d));
                }
            }
            "variant" => {
                editor.variant = p
                    .get("variant")
                    .and_then(Value::as_u64)
                    .filter(|v| *v < 8)
                    .ok_or("variant")? as u8;
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
                let object = d["symbol"].as_object_mut().ok_or("symbol")?;
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
                        json!({"arc":{"rect":[[x.min(x2),y.min(y2)],[x.max(x2),y.max(y2)]],"start":0,"sweep":180000,"stroke":stroke}})
                    }
                    "text" => json!({"text":{"pos":[x,y],"text":{"en":text(p,"text")},"align":"top_left"}}),
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
                        .ok_or("graphics")?
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
                    return Err("idx".into());
                }
                let points = d["symbol"]
                    .as_object_mut()
                    .ok_or("symbol")?
                    .entry("cps")
                    .or_insert(json!([]))
                    .as_array_mut()
                    .ok_or("cps")?;
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
                let fn_cp = cp["fn_cp"].as_u64().ok_or("fn_cp")?;
                points.retain(|v| v["idx"] != index);
                points.push(cp);
                points.sort_by_key(|v| v["idx"].as_u64());
                if editor.draft.function_ref.is_none() {
                    let fpoints = d["function"]
                        .as_object_mut()
                        .ok_or("function")?
                        .entry("cps")
                        .or_insert(json!([]))
                        .as_array_mut()
                        .ok_or("cps")?;
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
                    .ok_or("symbol")?
                    .entry("text_slots")
                    .or_insert(json!([]))
                    .as_array_mut()
                    .ok_or("slots")?;
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
                    .ok_or("symbol")?
                    .entry("link_points")
                    .or_insert(json!([]))
                    .as_array_mut()
                    .ok_or("links")?;
                points.push(json!({"name":text(p,"name"),"pos":[nm(p,"x")?,nm(p,"y")?]}));
                editor.replace(
                    SymbolDraft::parse(&serde_json::to_vec(&d).map_err(|e| e.to_string())?).map_err(err)?,
                );
            }
            "export" => {
                let path = Path::new(text(p, "file"));
                symbols::export(&editor.draft, path).map_err(err)?;
                editor.saved = Some(editor.draft.clone());
            }
            "save" => {
                let draft = editor.draft.clone();
                let directory = if text(p, "directory").is_empty() {
                    editor.directory.clone()
                } else {
                    text(p, "directory").to_owned()
                };
                let path = PathBuf::from(&directory);
                if directory.is_empty() {
                    return Err(strings::text(key!("symbol-directory-required")).into());
                }
                let issues = draft.validate();
                if !issues.is_empty() {
                    return Err(err(symbols::Error::Invalid(issues)));
                }
                let mut tiers = self.config.libraries.clone().unwrap_or_else(|| {
                    ew_api::library::LibraryTiers::under(
                        Path::new("."),
                        &path.parent().unwrap_or(Path::new(".")).join(".symbol-index"),
                        Vec::new(),
                        Vec::new(),
                    )
                });
                // Keep every configured tier and add the explicit destination as a writable company path.
                if !tiers.company.contains(&path) {
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
                draft.publish(&mut registry, target).map_err(err)?;
                self.config.libraries = Some(tiers);
                if let Some(project) = &mut self.project {
                    *project.session.libraries_mut() = registry;
                }
                editor.directory = directory;
                editor.saved = Some(draft);
                editor.error = None;
            }
            "place" => {
                let mut draft = editor.draft.clone();
                let variant = editor.variant;
                if let Some(project) = &self.project {
                    draft.check_function(project.session.libraries()).map_err(err)?;
                }
                // Each test placement embeds its own copies, including after a draft edit.
                let serial = self
                    .project
                    .as_ref()
                    .map_or(0, |p| p.session.history().entries().len());
                draft.key = format!(
                    "{}.test.{serial}",
                    draft.key.chars().take(100).collect::<String>()
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
                    .ok_or("symbol")?;
                let page = self
                    .current_page()
                    .ok_or_else(|| strings::text(key!("reason-no-page")).to_owned())?;
                let canvas = self
                    .canvas()
                    .ok_or_else(|| strings::text(key!("reason-no-page")).to_owned())?;
                let snapper = &canvas.page().snapper;
                if snapper.grid() != (draft.symbol.grid.num() as i64, draft.symbol.grid.den() as i64) {
                    return Err(strings::text(key!("symbol-page-grid-mismatch")).into());
                }
                let pos = snapper
                    .snap_grid(snapper.page_to_sheet((nm(p, "x")?, nm(p, "y")?)))
                    .page_nm;
                let orient = ["r0", "r90", "r180", "r270", "m0", "m90", "m180", "m270"][usize::from(variant)];
                let params = json!({"cmd":"device.place","params":{"page":page.uuid_text(),"symbol":symbol,"copies":docs,
                    "pos":[pos.0,pos.1],"orient":orient,"target":{"new_device":{"letter":"U"}}}});
                match self.act_command_run(params.as_object().ok_or("params")?) {
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
