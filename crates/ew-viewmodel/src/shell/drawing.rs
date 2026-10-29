// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's duties of WP-S2.04 part 4b (§10.5, §10.7): arming the Text, Graphic, Wire marker and Structure box
//! tools (and, from part 5, the Image tool with its asset, §11), the Text tool's binding, text edit (F2, and the window's double click on a selected free text), the text
//! popup's content, the tools' strip fields, the marker mode action and the return to Select when an edit finishes.
//!
//! **Reads.** Arming and the pointer and cursor moves of the Text, Graphic, Wire marker and Structure box tools read
//! nothing (§12 item 4); text edit resolves the selection through the page tables the canvas has already fetched. A
//! structure box's location node and external reference are resolved through the session's query when its field is
//! set.
//!
//! **Commits** go through the tool's commit path (`execute_with` in the window, `run_script` headless).

use std::io::Read as _;

use ew_api::AnyCommand;
use serde_json::{Map, Value, json};

use super::{Mode, Outcome, Prompt, ShellModel, failed, string_param};
use crate::strings;
use crate::tools::{
    BINDABLE, EntityKind, Ghost, GraphicShape, GraphicTool, ImageAsset, ImageTool, MarkerTool,
    StructureBoxTool, TextEditTool, TextTool, selection_kinds, tool_named,
};

/// The text popup of the window (WP-S2.04 §10.5): the title's label key and the field's initial text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextPopup {
    /// The key of the window's title (`tool-text` or `tool-text_edit`).
    pub title_key: &'static str,
    /// The text the field starts with (the ghost's).
    pub text: String,
}

impl ShellModel {
    /// The Text tool's current binding (the tool strip's choice), while the tool is active.
    pub fn text_bind(&self) -> Option<String> {
        match self.tools.tool.ghost() {
            Some(Ghost::Text { bind, .. }) => bind,
            _ => None,
        }
    }

    /// The text popup, while the Text tool types or Text edit is open: its title and its field's initial text.
    pub fn text_popup(&self) -> Option<TextPopup> {
        let title_key = match (self.tools.tool.name(), self.tools.tool.state()) {
            ("text", "typing") => "tool-text",
            ("text_edit", _) => "tool-text_edit",
            _ => return None,
        };
        let text = match self.tools.tool.ghost() {
            Some(Ghost::Text { text, .. }) => text,
            _ => String::new(),
        };
        Some(TextPopup { title_key, text })
    }

    /// The free text `text.edit` would open: its UUID text, its label point (the bounds' top-left corner, page nm) and
    /// its text. Errors: `reason-text-bound` for a bound text, `reason-select-one-text` for anything else.
    pub(crate) fn text_edit_target(&self) -> Result<(String, (i64, i64), String), &'static str> {
        let Some(c) = self.canvas() else {
            return Err("reason-select-one-text");
        };
        let tables = &c.page().tables;
        let kinds = selection_kinds(self.selection(), tables);
        let [(e, EntityKind::Item)] = kinds.as_slice() else {
            return Err("reason-select-one-text");
        };
        let Some(item) = tables.items.iter().find(|i| i.id == e.0 && i.kind == "free_text") else {
            return Err("reason-select-one-text");
        };
        if item.bind.is_some() {
            return Err("reason-text-bound");
        }
        Ok((
            item.id.to_string(),
            (item.bounds[0], item.bounds[1]),
            item.text.clone().unwrap_or_default(),
        ))
    }

    // -----------------------------------------------------------------------------------------------------------
    // The Text tool and text edit (§10.5)

    /// `tool.text` (T): the Text tool, armed.
    pub(super) fn act_tool_text(&mut self) -> Outcome {
        self.activate_tool(Box::new(TextTool::new()));
        Outcome::Done
    }

    /// `text.bind {bind}`: the Text tool's binding; `""` or `null` clears it, a name of [`BINDABLE`] sets it.
    pub(super) fn act_text_bind(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.tools.tool.name() != "text" {
            return Outcome::Refused {
                reason_key: "reason-text-tool-only",
            };
        }
        let bind = match p.get("bind") {
            None => return failed("error-missing-param", &["bind"]),
            Some(Value::Null) => None,
            Some(Value::String(s)) if s.is_empty() => None,
            Some(Value::String(s)) if BINDABLE.contains(&s.as_str()) => Some(s.clone()),
            Some(_) => return failed("error-bad-param", &["bind"]),
        };
        let overlay = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<TextTool>())
                    .map(|tool| {
                        tool.set_bind(bind);
                        tool.refresh(cx)
                    })
            })
            .flatten();
        // The binding changes no point: without one the overlay is left as it is.
        if let Some(o) = overlay
            && !o.items.is_empty()
        {
            self.tools.overlay = o;
        }
        self.announce_tool();
        Outcome::Done
    }

    /// `text.edit` (F2): text edit on exactly one selected unbound free text.
    pub(super) fn act_text_edit(&mut self) -> Outcome {
        // The same guard as `text.edit`'s enabled predicate (`one_free_text`): the double-click path reaches here
        // without it, and a read-only project or a page without symbols has no text to edit.
        if let Err(reason_key) = crate::actions::editable_page(self) {
            return Outcome::Refused { reason_key };
        }
        match self.text_edit_target() {
            Ok((item, at, text)) => {
                self.activate_tool(Box::new(TextEditTool::new(item, at, text)));
                Outcome::Done
            }
            Err(reason_key) => Outcome::Refused { reason_key },
        }
    }

    /// The end of every dispatch that calls `after_commit`: a finished text edit returns to Select, the selection kept.
    pub(super) fn after_text_edit(&mut self) {
        let finished = self
            .with_tool(|t, _| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<TextEditTool>())
                    .is_some_and(|e| e.finished())
            })
            .unwrap_or(false);
        if finished && let Some(select) = tool_named("select") {
            self.activate_tool(select);
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The Graphic tool (§10.5)

    /// `tool.graphic {shape?}` (L, Insert ▸ Graphic): the Graphic tool, `line` without a shape.
    pub(super) fn act_tool_graphic(&mut self, p: &Map<String, Value>) -> Outcome {
        let shape = match string_param(p, "shape") {
            Ok(None) => GraphicShape::Line,
            Ok(Some(w)) => match GraphicShape::of_word(w) {
                Some(s) => s,
                None => return failed("error-bad-param", &["shape"]),
            },
            Err(e) => return e,
        };
        self.activate_tool(Box::new(GraphicTool::new(shape)));
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The Wire marker tool (§10.5, §10.7)

    /// `tool.marker` (Shift+W): the Wire marker tool, armed.
    pub(super) fn act_tool_marker(&mut self) -> Outcome {
        self.activate_tool(Box::new(MarkerTool::new()));
        Outcome::Done
    }

    /// `marker.fields {csa?, colours?}`: the Wire marker tool's fields; `""` clears one. `colours` is split on ASCII
    /// whitespace into one entry per connection and each entry on `+` into one or two codes.
    pub(super) fn act_marker_fields(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.tools.tool.name() != "marker" {
            return Outcome::Refused {
                reason_key: "reason-marker-tool-only",
            };
        }
        if !p.contains_key("csa") && !p.contains_key("colours") {
            return failed("error-missing-param", &["csa"]);
        }
        let csa = match p.get("csa") {
            None => None,
            Some(Value::String(s)) => {
                let text = s.trim();
                Some((!text.is_empty()).then(|| text.to_owned()))
            }
            Some(_) => return failed("error-bad-param", &["csa"]),
        };
        let colours = match p.get("colours") {
            None => None,
            Some(Value::String(s)) => {
                let mut out: Vec<Vec<String>> = Vec::new();
                for entry in s.split_ascii_whitespace() {
                    let codes: Vec<String> = entry.split('+').map(str::to_owned).collect();
                    if codes.len() > 2 || codes.iter().any(String::is_empty) {
                        return failed("error-bad-param", &["colours"]);
                    }
                    out.push(codes);
                }
                Some(out)
            }
            Some(_) => return failed("error-bad-param", &["colours"]),
        };
        let overlay = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<MarkerTool>())
                    .map(|tool| {
                        tool.set_fields(csa, colours);
                        tool.refresh(cx)
                    })
            })
            .flatten();
        // The fields change no point: without one the overlay is left as it is.
        if let Some(o) = overlay
            && !o.items.is_empty()
        {
            self.tools.overlay = o;
        }
        self.announce_tool();
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The Structure box tool (§10.5, §10.7)

    /// `tool.structure_box` (Shift+B): the Structure box tool, armed.
    pub(super) fn act_tool_structure_box(&mut self) -> Outcome {
        self.activate_tool(Box::new(StructureBoxTool::new()));
        Outcome::Done
    }

    /// `structure_box.fields {style?, caption?, location?, external?}`: the Structure box tool's fields; `""` clears
    /// a text, `null` or `""` clears the external reference. Every given member is checked before any is applied.
    pub(super) fn act_structure_box_fields(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.tools.tool.name() != "structure_box" {
            return Outcome::Refused {
                reason_key: "reason-structure-box-tool-only",
            };
        }
        if !["style", "caption", "location", "external"]
            .iter()
            .any(|name| p.contains_key(*name))
        {
            return failed("error-missing-param", &["style"]);
        }
        let style: Option<&'static str> = match p.get("style") {
            None => None,
            Some(Value::String(s)) if s == "dotted" => Some("dotted"),
            Some(Value::String(s)) if s == "dash_dot" => Some("dash_dot"),
            Some(_) => return failed("error-bad-param", &["style"]),
        };
        let caption: Option<Option<String>> = match p.get("caption") {
            None => None,
            Some(Value::String(s)) => Some((!s.is_empty()).then(|| s.clone())),
            Some(_) => return failed("error-bad-param", &["caption"]),
        };
        let location: Option<Option<(String, String)>> = match p.get("location") {
            None => None,
            Some(Value::String(s)) if s.is_empty() => Some(None),
            Some(Value::String(s)) => match self.structure_node("location", s) {
                Ok(id) => Some(Some((id, s.clone()))),
                Err(message) => return Outcome::Failed { message },
            },
            Some(_) => return failed("error-bad-param", &["location"]),
        };
        let external: Option<Option<(String, String)>> = match p.get("external") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(Value::String(s)) if s.is_empty() => Some(None),
            Some(Value::String(s)) => match self.resolve_external(s) {
                Ok(e) => Some(Some((e.id, e.label))),
                Err(outcome) => return outcome,
            },
            Some(_) => return failed("error-bad-param", &["external"]),
        };
        let overlay = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<StructureBoxTool>())
                    .map(|tool| {
                        if let Some(style) = style {
                            tool.set_style(style);
                        }
                        if let Some(caption) = caption {
                            tool.set_caption(caption);
                        }
                        if let Some(location) = location {
                            tool.set_location(location);
                        }
                        if let Some(external) = external {
                            tool.set_external(external);
                        }
                        tool.refresh(cx)
                    })
            })
            .flatten();
        // The fields change no point: without one the overlay is left as it is.
        if let Some(o) = overlay
            && !o.items.is_empty()
        {
            self.tools.overlay = o;
        }
        self.announce_tool();
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The Image tool (WP-S2.04 §11)

    /// `tool.image` (Insert ▸ Image…): the Image tool, without an asset until `image.pick`.
    ///
    /// In the window it also raises the native file dialog (`Prompt::OpenImage`), whose answer is `image.file`;
    /// headless the tool waits for `image.pick`.
    pub(super) fn act_tool_image(&mut self) -> Outcome {
        self.activate_tool(Box::new(ImageTool::new()));
        match self.mode {
            Mode::Gui => self.need_path("image.file", Prompt::OpenImage, "path", &Map::new()),
            Mode::Headless => Outcome::Done,
        }
    }

    /// `image.file {path}`: the Image tool's asset from a file (the window's dialog answer, or a file dropped on the
    /// canvas; any tool may be active, the Image tool is armed when it is not). The file is read (at most
    /// [`ew_api::MAX_LOGO_BYTES`]), sniffed as the asset store sniffs ([`ew_api::asset_ref_of`]: format, intrinsic
    /// size, the limits, the SVG rejecter) and hashed. A project that holds the hash arms the tool with it, as
    /// `image.pick` does; otherwise the tool keeps the bytes as pending `data` for the commit. A file the store does
    /// not accept fails with the store's text and changes nothing.
    pub(super) fn act_image_file(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => return self.need_path("image.file", Prompt::OpenImage, "path", p),
        };
        let name = path.display().to_string();
        let limit = ew_api::MAX_LOGO_BYTES;
        let too_large = || failed("error-image-too-large", &[&name, &(limit / 1024).to_string()]);
        // Only a regular file is read: a character device or a FIFO reports length 0, and reading it to the end would
        // not finish. The read is bounded as well (the file may grow after the metadata).
        match std::fs::metadata(&path) {
            Ok(m) if !m.is_file() => return failed("error-image-file", &[&name, "not a regular file"]),
            Ok(m) if m.len() > limit as u64 => return too_large(),
            Ok(_) => {}
            Err(e) => return failed("error-image-file", &[&name, &e.to_string()]),
        }
        let mut bytes = Vec::new();
        if let Err(e) =
            std::fs::File::open(&path).and_then(|f| f.take(limit as u64 + 1).read_to_end(&mut bytes))
        {
            return failed("error-image-file", &[&name, &e.to_string()]);
        }
        if bytes.len() > limit {
            return too_large();
        }
        let reference = match ew_api::asset_ref_of(&bytes) {
            Ok(r) => r,
            Err(e) => return failed("error-image-refused", &[&name, &e.to_string()]),
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let held = project.session.asset(&reference.sha256).is_some();
        let asset = ImageAsset {
            hash: reference.sha256.to_string(),
            size: reference.size,
            data: (!held).then(|| ew_api::base64_encode(&bytes)),
        };
        if self.tools.tool.name() != "image" {
            self.activate_tool(Box::new(ImageTool::new()));
        }
        let overlay = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<ImageTool>())
                    .map(|tool| {
                        tool.set_asset(asset);
                        tool.refresh(cx)
                    })
            })
            .flatten();
        if let Some(o) = overlay
            && !o.items.is_empty()
        {
            self.tools.overlay = o;
        }
        self.announce_tool();
        Outcome::Done
    }

    /// `image.pick {asset}`: the Image tool's asset, a hash (`sha256:<64 hex>`) the project already holds; the tool
    /// is armed with it and its intrinsic size. A hash the project does not hold fails and changes nothing.
    pub(super) fn act_image_pick(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.tools.tool.name() != "image" {
            return Outcome::Refused {
                reason_key: "reason-image-tool-only",
            };
        }
        let text = match string_param(p, "asset") {
            Ok(Some(t)) => t,
            Ok(None) => return failed("error-missing-param", &["asset"]),
            Err(e) => return e,
        };
        let Ok(sha256) = text.parse::<ew_api::Sha256>() else {
            return failed("error-bad-param", &["asset"]);
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let Some((reference, _)) = project.session.asset(&sha256) else {
            return failed("error-image-asset-missing", &[text]);
        };
        let asset = ImageAsset {
            hash: sha256.to_string(),
            size: reference.size,
            data: None,
        };
        let overlay = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<ImageTool>())
                    .map(|tool| {
                        tool.set_asset(asset);
                        tool.refresh(cx)
                    })
            })
            .flatten();
        // The asset changes no point: without one the overlay is left as it is.
        if let Some(o) = overlay
            && !o.items.is_empty()
        {
            self.tools.overlay = o;
        }
        self.announce_tool();
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The marker mode action (§10.6)

    /// `project.marker_mode {mode}` (Tools ▸ Wire markers): commits `project.drawing.set` with the mode, or `null`
    /// for `per_wire`; a mode the project already has commits nothing.
    pub(super) fn act_marker_mode(&mut self, p: &Map<String, Value>) -> Outcome {
        let mode = match p.get("mode") {
            None => return failed("error-missing-param", &["mode"]),
            Some(Value::String(s))
                if matches!(
                    s.as_str(),
                    "per_wire" | "all" | "deviations" | "explicit" | "none"
                ) =>
            {
                s.as_str()
            }
            Some(_) => return failed("error-bad-param", &["mode"]),
        };
        if mode == self.marker_mode_word() {
            self.notify_marker_count(false);
            return Outcome::Done;
        }
        let value = if mode == "per_wire" {
            Value::Null
        } else {
            json!(mode)
        };
        let cmd = AnyCommand {
            name: "project.drawing.set".into(),
            params: json!({ "marker_mode": value }),
        };
        let outcome = self.run_commit(&cmd);
        if matches!(outcome, Outcome::Done) {
            self.notify_marker_count(true);
        }
        outcome
    }

    /// FT-554: the toast of a marker-mode switch, how many wires carry a marker now (`0 wires carry a marker`), so a
    /// mode that selects no wire on this project does not look like a mode that did nothing.
    fn notify_marker_count(&mut self, undo: bool) {
        let Some(project) = &self.project else { return };
        let n = project.session.marker_wire_count();
        self.notify(strings::format_count("toast-marker-count", n), undo);
    }
}
