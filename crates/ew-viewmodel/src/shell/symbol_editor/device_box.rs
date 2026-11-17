// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device box, the symbol editor's second kind of thing (rc.3, doc 07 §2.5): a part that is drawn as a box with
//! pins. The editor holds the part's draft: its pins (added, removed, renamed, given a potential) and where each
//! stands on the box (its edge and its place along it), and the box's size. Save saves the part as the part editor
//! does (a new part, a part of the project corrected in place, a part of My parts as its next version, a library's
//! part as the project's own copy), and offers to redraw the boxes of it that are drawn already.

use super::{Outline, OutlineGroup, RowValue, Sheet, SheetRow, SymbolEditor, Todo, role_words, words};
use crate::notifications::{NoteAction, NoteTone, Validity};
use crate::shell::{Dialog, Outcome, ShellModel};
use crate::{key, strings};
use ew_api::part_box::{BoxSteps, Dir4, EDIT_STEP, PinsFrom};
use ew_api::{PartDraft, PartDraftError};
use serde_json::{Map, Value, json};

/// The edges of a box in the order the pin table lists them, each with the word an action names it by.
pub const SIDES: [(&str, Dir4); 4] = [
    ("top", Dir4::Up),
    ("bottom", Dir4::Down),
    ("left", Dir4::Left),
    ("right", Dir4::Right),
];

/// The word of an edge.
pub fn side_word(side: &Dir4) -> &'static str {
    SIDES.iter().find(|(_, d)| d == side).map_or("bottom", |(w, _)| w)
}

fn side_of(word: &str) -> Option<Dir4> {
    SIDES.iter().find(|(w, _)| *w == word).map(|(_, d)| d.clone())
}

fn side_words(side: &Dir4) -> String {
    strings::text(strings::Key::run_time(&format!("box-side-{}", side_word(side)))).to_owned()
}

/// One row of the pin table.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PinLine {
    /// The pin.
    pub idx: u16,
    /// Its name as the device prints it.
    pub name: String,
    /// What it carries, in words; empty for none.
    pub role: String,
    /// The edge it stands on.
    pub side: Dir4,
    /// Its distance along the edge, in grid steps.
    pub at: i64,
}

/// A part's device box being edited.
#[derive(Clone, Debug)]
pub struct DeviceBox {
    /// The part's draft: its identity, its pins and its box.
    pub draft: PartDraft,
    /// The draft as it was opened or last saved; equality decides unsaved changes.
    saved: PartDraft,
    undo: Vec<PartDraft>,
    redo: Vec<PartDraft>,
    /// The selected pins, in the order they were selected.
    pub selection: Vec<u16>,
    /// Counts the changes, so the window reads the box again.
    pub revision: u64,
    /// Why the last action did nothing.
    pub error: Option<String>,
    /// Whether the box was opened from the project (a placed box, the part editor): closing it returns there, not to
    /// the editor's ways in.
    from_project: bool,
}

impl DeviceBox {
    pub(super) fn new(draft: PartDraft) -> DeviceBox {
        DeviceBox {
            saved: draft.clone(),
            draft,
            undo: Vec::new(),
            redo: Vec::new(),
            selection: Vec::new(),
            revision: 0,
            error: None,
            from_project: false,
        }
    }

    /// Whether the box has changes that are not saved.
    pub fn dirty(&self) -> bool {
        self.draft != self.saved
    }

    /// Whether Undo and Redo have a step.
    pub fn can_step(&self, redo: bool) -> bool {
        !(if redo { &self.redo } else { &self.undo }).is_empty()
    }

    /// The part's name as the screen titles it: its part number, else "New part".
    pub fn title(&self) -> String {
        let number = self.draft.text("part_number").trim();
        if number.is_empty() || number == "-" {
            words(key!("box-new-part"))
        } else {
            number.to_owned()
        }
    }

    /// The box as it will be drawn, on the 4 mm grid.
    pub fn steps(&self) -> BoxSteps {
        self.draft.box_steps()
    }

    /// Where the pins come from when they are not edited here, in a sentence; `None` when they are.
    pub fn pins_from(&self) -> Option<String> {
        self.draft.pins_from().map(|from| {
            words(match from {
                PinsFrom::Channels => key!("box-pins-from-channels"),
                PinsFrom::Ports => key!("box-pins-from-ports"),
                PinsFrom::Symbol => key!("box-pins-from-symbol"),
            })
        })
    }

    /// The pin table: the pins in the order they stand on the box, the edges top, bottom, left, right, each from its
    /// start.
    pub fn pin_table(&self) -> Vec<PinLine> {
        self.steps()
            .pins
            .into_iter()
            .filter_map(|(idx, side, at)| {
                let pin = self.draft.pins.iter().find(|p| p.idx == idx)?;
                Some(PinLine {
                    idx,
                    name: pin.name.clone(),
                    role: if pin.role.is_empty() {
                        String::new()
                    } else {
                        role_words(&pin.role)
                    },
                    side,
                    at,
                })
            })
            .collect()
    }

    /// Takes `change` of the draft as one undo step; a change that changes nothing is none.
    fn edit<T>(&mut self, change: impl FnOnce(&mut PartDraft) -> T) -> T {
        let before = self.draft.clone();
        let out = change(&mut self.draft);
        if self.draft != before {
            self.undo.push(before);
            if self.undo.len() > 32 {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.revision = self.revision.wrapping_add(1);
            self.error = None;
        }
        self.selection
            .retain(|i| self.draft.pins.iter().any(|p| p.idx == *i));
        out
    }

    pub(super) fn step(&mut self, redo: bool) {
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        if let Some(d) = from.pop() {
            to.push(std::mem::replace(&mut self.draft, d));
            self.revision = self.revision.wrapping_add(1);
            self.error = None;
            self.selection
                .retain(|i| self.draft.pins.iter().any(|p| p.idx == *i));
        }
    }

    /// What keeps the part from being saved.
    fn todo(&self) -> Vec<Todo> {
        let mut out = Vec::new();
        let d = &self.draft;
        if d.new {
            let empty = |f: &str| {
                let t = d.text(f).trim();
                t.is_empty() || t == "-"
            };
            if empty("part_number") {
                out.push(words(key!("box-todo-number")));
            }
            if empty("manufacturer") {
                out.push(words(key!("box-todo-manufacturer")));
            }
        }
        out.extend(d.messages.iter().map(|m| {
            let args: Vec<&str> = m.args.iter().map(String::as_str).collect();
            strings::format(strings::Key::run_time(m.key), &args)
        }));
        out.into_iter().map(|text| Todo { text, item: None }).collect()
    }

    /// The box as the left panel lists it: the part, and what is left to do; the pins are the pin table's.
    pub fn outline(&self) -> Outline {
        Outline {
            title: self.title(),
            what: strings::format(key!("box-what-line"), &[self.draft.text("manufacturer").trim()])
                .trim_end_matches([' ', '·'])
                .to_owned(),
            groups: Vec::<OutlineGroup>::new(),
            todo: self.todo(),
            candidates: Vec::new(),
            import_notes: Vec::new(),
            import_reviewed: false,
        }
    }

    /// The rows of the selected pins, or of the part and its box with none selected.
    pub fn sheet(&self) -> Sheet {
        let text = |key: &'static str, label: Key, value: String| SheetRow {
            key,
            label: words(label),
            value: RowValue::Text(Some(value)),
            more: false,
            help: None,
            locked: None,
        };
        let locked = self.pins_from();
        let b = self.steps();
        let mm = |steps: i64| RowValue::Mm(Some((steps * EDIT_STEP) as f64 / 1e6));
        if self.selection.is_empty() {
            let d = &self.draft;
            let identity = (!d.new).then(|| words(key!("box-identity-in-part-editor")));
            let mut rows: Vec<SheetRow> = [
                ("part_number", key!("part-field-part_number")),
                ("manufacturer", key!("part-field-manufacturer")),
                ("description", key!("part-field-description")),
            ]
            .into_iter()
            .map(|(field, label)| SheetRow {
                locked: identity.clone(),
                ..text(field, label, d.text(field).to_owned())
            })
            .collect();
            for (key, label, steps) in [
                ("width", key!("symbol-width-mm"), b.w),
                ("height", key!("symbol-height-mm"), b.h),
            ] {
                rows.push(SheetRow {
                    value: mm(steps),
                    locked: locked.clone(),
                    ..text(key, label, String::new())
                });
            }
            return Sheet {
                title: self.title(),
                line: words(key!("box-line")),
                rows,
            };
        }
        let table = self.pin_table();
        let chosen: Vec<&PinLine> = table.iter().filter(|p| self.selection.contains(&p.idx)).collect();
        let same = |f: &dyn Fn(&PinLine) -> String| {
            let first = chosen.first().map(|p| f(p));
            first.filter(|v| chosen.iter().all(|p| f(p) == *v))
        };
        let role_of = |p: &PinLine| {
            self.draft
                .pins
                .iter()
                .find(|d| d.idx == p.idx)
                .map(|d| {
                    if d.role.is_empty() {
                        "none".to_owned()
                    } else {
                        d.role.clone()
                    }
                })
                .unwrap_or_default()
        };
        let mut roles = vec![("none".to_owned(), role_words(""))];
        roles.extend(ew_api::part_box::pin_roles().into_iter().map(|r| {
            let text = role_words(&r);
            (r, text)
        }));
        let sides = SIDES
            .iter()
            .map(|(w, d)| ((*w).to_owned(), side_words(d)))
            .collect();
        let rows = vec![
            SheetRow {
                value: RowValue::Text(same(&|p| p.name.clone())),
                locked: locked.clone(),
                ..text("name", key!("box-pin-name"), String::new())
            },
            SheetRow {
                value: RowValue::Choice {
                    value: same(&role_of),
                    options: roles,
                },
                help: Some(words(key!("symbol-potential-role-help"))),
                locked: locked.clone(),
                ..text("role", key!("symbol-potential-role"), String::new())
            },
            SheetRow {
                value: RowValue::Choice {
                    value: same(&|p| side_word(&p.side).to_owned()),
                    options: sides,
                },
                locked: locked.clone(),
                ..text("side", key!("box-pin-side"), String::new())
            },
            SheetRow {
                value: RowValue::Mm(
                    same(&|p| p.at.to_string())
                        .and_then(|k| k.parse::<i64>().ok())
                        .map(|k| (k * EDIT_STEP) as f64 / 1e6),
                ),
                help: Some(words(key!("box-pin-at-help"))),
                locked,
                ..text("at", key!("box-pin-at"), String::new())
            },
        ];
        let (title, line) = match chosen[..] {
            [pin] => (
                strings::format(key!("box-pin-title"), &[&pin.name]),
                words(key!("box-pin-line")),
            ),
            _ => (
                strings::format(key!("box-pins-selected"), &[&chosen.len().to_string()]),
                words(key!("symbol-things-shared")),
            ),
        };
        Sheet { title, line, rows }
    }
}

type Key = strings::Key<'static>;

impl SymbolEditor {
    /// The device box being edited, in the drawing's place.
    pub fn device_box(&self) -> Option<&DeviceBox> {
        self.device_box.as_ref()
    }
}

/// A draft's refusal in the window's words.
fn draft_words(e: PartDraftError) -> String {
    match e {
        PartDraftError::Refused(key) => strings::text(strings::Key::run_time(key)).to_owned(),
        PartDraftError::Field(m) => {
            let args: Vec<&str> = m.args.iter().map(String::as_str).collect();
            strings::format(strings::Key::run_time(m.key), &args)
        }
    }
}

fn outcome_words(out: Outcome) -> Result<(), String> {
    match out {
        Outcome::Done => Ok(()),
        Outcome::Failed { message } => Err(message),
        Outcome::Refused { reason_key } => Err(strings::text(reason_key).to_owned()),
    }
}

impl ShellModel {
    /// The draft of the part the pin actions change: the device box editor's, else the part editor dialog's.
    fn pin_draft(&mut self) -> Option<&mut PartDraft> {
        if let Some(b) = self.symbol_editor.as_mut().and_then(|e| e.device_box.as_mut()) {
            return Some(&mut b.draft);
        }
        match &mut self.dialog {
            Some(Dialog::PartEdit(e)) => Some(&mut e.draft),
            _ => None,
        }
    }

    /// `part.box {part?, new?}` (Pins and box… in the part editor, Edit this device's box… on a placed box, Device
    /// box for a part… on the start card): the device box editor on the part editor's draft when it is open, on the
    /// part named (`id:<copy>`, or a part number as `part.edit` reads it), on a new part with `new`, else on the part
    /// of the selected device. A box with unsaved changes is refused unless `discard` (the window asks first).
    pub(crate) fn act_part_box(&mut self, p: &Map<String, Value>) -> Outcome {
        let discard = p.get("discard") == Some(&Value::Bool(true));
        let open = self
            .symbol_editor
            .as_ref()
            .and_then(|e| e.device_box.as_ref())
            .is_some_and(DeviceBox::dirty);
        if open && !discard {
            return Outcome::Failed {
                message: words(key!("box-unsaved")),
            };
        }
        let draft = if let Some(Dialog::PartEdit(editor)) = &self.dialog {
            let draft = editor.draft.clone();
            self.dialog = None;
            draft
        } else {
            let Some(project) = &self.project else {
                return Outcome::refused(key!("reason-no-project"));
            };
            let made = if p.get("new") == Some(&Value::Bool(true)) {
                project.session.new_part_draft()
            } else {
                let copy = match self.part_edit_copy(p) {
                    Ok(Some(c)) => c,
                    Ok(None) => return Outcome::refused(key!("reason-part-edit-target")),
                    Err(e) => return e,
                };
                project
                    .session
                    .part_draft(copy)
                    .map_err(|_| PartDraftError::Refused("reason-part-edit-target"))
            };
            match made {
                Ok(d) => d,
                Err(e) => {
                    return Outcome::Failed {
                        message: draft_words(e),
                    };
                }
            }
        };
        // The editor's screen, with the box in the drawing's place; a symbol draft in hand waits under it.
        let from_project = self.symbol_editor().is_none();
        if self.symbol_editor.is_none() {
            let made = self.symbol_operation("new", &Map::new());
            if let Err(message) = made {
                return Outcome::Failed { message };
            }
        }
        self.tools.picker = None;
        if let Some(editor) = &mut self.symbol_editor {
            editor.shown = true;
            editor.start = false;
            editor.device_box = Some(DeviceBox {
                from_project,
                ..DeviceBox::new(draft)
            });
        }
        Outcome::Done
    }

    /// `part.box_details` (Part data… in the device box editor): the part editor on the box's draft, with its pins
    /// and box as they are now; its Save saves the part, and Pins and box… returns here.
    pub(crate) fn act_part_box_details(&mut self) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(b) = self.symbol_editor.as_mut().and_then(|e| e.device_box.take()) else {
            return Outcome::refused(key!("reason-no-device-box"));
        };
        let manufacturers = project.session.part_manufacturers();
        let name = if b.draft.new {
            String::new()
        } else {
            format!(
                "{} ({})",
                b.draft.text("part_number"),
                b.draft.text("manufacturer")
            )
        };
        self.leave_box(b.from_project);
        self.open_dialog(Dialog::PartEdit(Box::new(
            crate::shell::part_dialog::PartEditor {
                draft: b.draft,
                name,
                manufacturers,
                error: None,
                back: None,
            },
        )))
    }

    /// After the box is closed: back to the project when it was opened from there, else to what the editor held (a
    /// symbol draft, or the ways in).
    fn leave_box(&mut self, from_project: bool) {
        let Some(editor) = &mut self.symbol_editor else {
            return;
        };
        editor.start = !editor.in_hand();
        if from_project {
            editor.shown = false;
            if !editor.in_hand() {
                self.symbol_editor = None;
            }
        }
    }

    /// The project's parts that are drawn as a box with pins of their own, for the start card's Device box for a
    /// part…: each as (what `part.box` takes, its part number, its manufacturer and description).
    pub fn box_parts(&self) -> Vec<(String, String, String)> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let session = &project.session;
        let mut out: Vec<(String, String, String)> = session
            .held_part_copies()
            .into_iter()
            .filter_map(|copy| {
                let d = session.part_draft(copy).ok()?;
                (d.pins_from().is_none() && (d.box_part || !d.pins.is_empty())).then(|| {
                    let detail = [d.text("manufacturer"), d.text("description")]
                        .iter()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" \u{b7} ");
                    (format!("id:{copy}"), d.text("part_number").to_owned(), detail)
                })
            })
            .collect();
        out.sort_by_key(|a| a.1.to_lowercase());
        out
    }

    /// `part.pin {add?, name?, remove?, pin?, role?, side?, at?, before?}`: the pins of the part being edited. `add`
    /// adds a pin (named `name`, else the next free number); `remove` removes pin `remove`; with `pin`: `name` and
    /// `role` name it and say what it carries, `side` with `at` (millimetres along the edge) puts it there, and
    /// `side` alone moves it in the pin table's order onto that edge, before pin `before` or last. Refused in words
    /// for a part whose pins come from its channels, its ports or its symbol.
    pub(crate) fn act_part_pin(&mut self, p: &Map<String, Value>) -> Outcome {
        let index = |name: &str| {
            p.get(name)
                .and_then(Value::as_u64)
                .and_then(|n| u16::try_from(n).ok())
        };
        let text = |name: &str| p.get(name).and_then(Value::as_str);
        let in_box = self
            .symbol_editor
            .as_ref()
            .is_some_and(|e| e.device_box.is_some());
        let Some(draft) = self.pin_draft() else {
            return Outcome::refused(key!("reason-no-part-editor"));
        };
        if let Some(from) = draft.pins_from() {
            return Outcome::Failed {
                message: words(match from {
                    PinsFrom::Channels => key!("box-pins-from-channels"),
                    PinsFrom::Ports => key!("box-pins-from-ports"),
                    PinsFrom::Symbol => key!("box-pins-from-symbol"),
                }),
            };
        }
        let change = |d: &mut PartDraft| -> Result<Option<u16>, String> {
            if p.get("add") == Some(&Value::Bool(true)) {
                return Ok(Some(d.add_pin(text("name"))));
            }
            if p.contains_key("remove") {
                let idx = index("remove").ok_or_else(|| words(key!("box-pin-unknown")))?;
                return if d.remove_pin(idx) {
                    Ok(None)
                } else {
                    Err(words(key!("box-pin-unknown")))
                };
            }
            let idx = index("pin").ok_or_else(|| words(key!("box-pin-unknown")))?;
            if text("name").is_some() || text("role").is_some() {
                let role = text("role").map(|r| if r == "none" { "" } else { r });
                d.set_pin(idx, text("name"), role).map_err(draft_words)?;
            }
            if let Some(word) = text("side") {
                let side = side_of(word).ok_or_else(|| words(key!("box-side-invalid")))?;
                let done = match p.get("at").and_then(Value::as_f64) {
                    Some(mm) => d.place_pin(idx, side, (mm * 1e6).round() as i64),
                    None => d.move_pin(idx, side, index("before")),
                };
                if !done {
                    return Err(words(key!("box-pin-unknown")));
                }
            } else if let Some(mm) = p.get("at").and_then(Value::as_f64) {
                let side = d
                    .box_steps()
                    .pins
                    .into_iter()
                    .find(|pin| pin.0 == idx)
                    .map(|pin| pin.1)
                    .ok_or_else(|| words(key!("box-pin-unknown")))?;
                d.place_pin(idx, side, (mm * 1e6).round() as i64);
            }
            Ok(None)
        };
        let result = if in_box {
            let Some(b) = self.symbol_editor.as_mut().and_then(|e| e.device_box.as_mut()) else {
                return Outcome::Done;
            };
            let result = b.edit(change);
            // A new pin is the one selected: its name is typed next.
            if let Ok(Some(added)) = &result {
                b.selection = vec![*added];
            }
            if let Err(message) = &result {
                b.error = Some(message.clone());
            }
            result
        } else {
            change(draft)
        };
        match result {
            Ok(_) => Outcome::Done,
            Err(message) => Outcome::Failed { message },
        }
    }

    /// `part.box_size {width, height}` in millimetres: the box's size, no smaller than its pins need.
    pub(crate) fn act_part_box_size(&mut self, p: &Map<String, Value>) -> Outcome {
        let side = |name: &str| p.get(name).and_then(Value::as_f64).filter(|v| *v > 0.0);
        let Some(b) = self.symbol_editor.as_mut().and_then(|e| e.device_box.as_mut()) else {
            return Outcome::refused(key!("reason-no-device-box"));
        };
        if b.draft.pins_from().is_some() {
            return Outcome::Failed {
                message: b.pins_from().unwrap_or_default(),
            };
        }
        let now = b.steps();
        let nm = |v: Option<f64>, steps: i64| v.map_or(steps * EDIT_STEP, |mm| (mm * 1e6).round() as i64);
        let (w, h) = (nm(side("width"), now.w), nm(side("height"), now.h));
        b.edit(|d| d.size_box(w, h));
        Outcome::Done
    }

    /// `part.box_select {pins, mode?}`: the selected pins of the device box, `pins` a list of pin numbers or `all`;
    /// `mode` `replace` (the default), `add` or `toggle`.
    pub(crate) fn act_part_box_select(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(b) = self.symbol_editor.as_mut().and_then(|e| e.device_box.as_mut()) else {
            return Outcome::refused(key!("reason-no-device-box"));
        };
        let all: Vec<u16> = b.pin_table().iter().map(|l| l.idx).collect();
        let named: Vec<u16> = match p.get("pins") {
            Some(Value::String(s)) if s == "all" => all,
            Some(Value::Array(list)) => list
                .iter()
                .filter_map(|v| v.as_u64().and_then(|n| u16::try_from(n).ok()))
                .filter(|i| all.contains(i))
                .collect(),
            _ => return crate::shell::failed(key!("error-bad-param"), &["pins"]),
        };
        match p.get("mode").and_then(Value::as_str) {
            Some("add") => {
                for i in named {
                    b.selection.retain(|s| *s != i);
                    b.selection.push(i);
                }
            }
            Some("toggle") => {
                for i in named {
                    if b.selection.contains(&i) {
                        b.selection.retain(|s| *s != i);
                    } else {
                        b.selection.push(i);
                    }
                }
            }
            _ => b.selection = named,
        }
        Outcome::Done
    }

    /// The editor's own actions while a device box is edited: the keys a person learned on a page act on the pins
    /// (Delete removes the selected ones, an arrow moves them a grid step along their edge, Select all), Undo and
    /// Redo step the box's history, Save saves the part, Test on a page places a device of it, and Close returns to
    /// what the editor held. `None` for an action that is the symbol's alone.
    pub(super) fn box_operation(
        &mut self,
        action: &str,
        p: &Map<String, Value>,
    ) -> Option<Result<(), String>> {
        let b = self.symbol_editor.as_mut()?.device_box.as_mut()?;
        let done = match action {
            "undo" | "redo" => {
                b.step(action == "redo");
                Ok(())
            }
            "delete" => {
                let gone = b.selection.clone();
                if b.draft.pins_from().is_some() {
                    Err(b.pins_from().unwrap_or_default())
                } else {
                    b.edit(|d| {
                        for idx in gone {
                            d.remove_pin(idx);
                        }
                    });
                    Ok(())
                }
            }
            "nudge" => {
                let step = |name: &str| p.get(name).and_then(Value::as_i64).unwrap_or(0);
                let (dx, dy) = (step("dx"), step("dy"));
                let moved: Vec<(u16, Dir4, i64)> = b
                    .steps()
                    .pins
                    .into_iter()
                    .filter(|pin| b.selection.contains(&pin.0))
                    .collect();
                if b.draft.pins_from().is_none() {
                    b.edit(|d| {
                        for (idx, side, at) in moved {
                            let along = if matches!(side, Dir4::Up | Dir4::Down) {
                                dx
                            } else {
                                dy
                            };
                            if along != 0 {
                                d.place_pin(idx, side, (at + along) * EDIT_STEP);
                            }
                        }
                    });
                }
                Ok(())
            }
            "select" => {
                let all = p.get("items") == Some(&Value::String("all".to_owned()));
                b.selection = if all {
                    b.pin_table().iter().map(|l| l.idx).collect()
                } else {
                    Vec::new()
                };
                Ok(())
            }
            "prop" => return Some(self.box_prop(p)),
            "save" => return Some(self.box_save()),
            "scratch" => return Some(self.box_test()),
            "close" => {
                let discard = p.get("discard") == Some(&Value::Bool(true));
                if b.dirty() && !discard {
                    Err(words(key!("box-unsaved")))
                } else {
                    let from_project = b.from_project;
                    self.symbol_editor.as_mut()?.device_box = None;
                    self.leave_box(from_project);
                    Ok(())
                }
            }
            "start" => {
                if b.dirty() && p.get("discard") != Some(&Value::Bool(true)) {
                    Err(words(key!("box-unsaved")))
                } else {
                    let editor = self.symbol_editor.as_mut()?;
                    editor.device_box = None;
                    editor.start = true;
                    Ok(())
                }
            }
            // The clipboard and the drawing tools are the symbol's: on a box they do nothing.
            "copy" | "cut" | "paste" | "duplicate" | "tool" | "grid" | "variant" | "align" | "grip"
            | "move" => Ok(()),
            _ => return None,
        };
        if let (Err(message), Some(b)) = (
            &done,
            self.symbol_editor.as_mut().and_then(|e| e.device_box.as_mut()),
        ) {
            b.error = Some(message.clone());
        }
        Some(done)
    }

    /// A row of the box's Properties: of the selected pins, or of the part and its box.
    fn box_prop(&mut self, p: &Map<String, Value>) -> Result<(), String> {
        let key = super::text(p, "key");
        let value = p.get("value").cloned().unwrap_or(Value::Null);
        let invalid = || words(key!("symbol-prop-invalid"));
        let selection = self
            .symbol_editor
            .as_ref()
            .and_then(|e| e.device_box.as_ref())
            .map(|b| b.selection.clone())
            .unwrap_or_default();
        if selection.is_empty() {
            return match key {
                "width" | "height" => outcome_words(
                    self.act_part_box_size(&json!({ key: value }).as_object().cloned().unwrap_or_default()),
                ),
                "part_number" | "manufacturer" | "description" => {
                    let text = value.as_str().ok_or_else(invalid)?.to_owned();
                    let project = self
                        .project
                        .as_ref()
                        .ok_or_else(|| words(key!("reason-no-project")))?;
                    let b = self
                        .symbol_editor
                        .as_mut()
                        .and_then(|e| e.device_box.as_mut())
                        .ok_or_else(invalid)?;
                    if !b.draft.new {
                        return Err(words(key!("box-identity-in-part-editor")));
                    }
                    let session = &project.session;
                    b.edit(|d| {
                        // The text is kept either way; a text that does not fit shows as what is left to do.
                        let _ = session.set_draft_field(d, key, &text);
                    });
                    Ok(())
                }
                _ => Err(invalid()),
            };
        }
        let mut params = Map::new();
        match key {
            "name" | "role" | "side" => {
                params.insert(key.to_owned(), json!(value.as_str().ok_or_else(invalid)?));
            }
            "at" => {
                params.insert("at".to_owned(), json!(value.as_f64().ok_or_else(invalid)?));
            }
            _ => return Err(invalid()),
        }
        // Several pins changed are one step of the box's history.
        let before = self
            .symbol_editor
            .as_ref()
            .and_then(|e| e.device_box.as_ref())
            .map(|b| (b.draft.clone(), b.undo.clone()));
        let mut result = Ok(());
        for idx in &selection {
            params.insert("pin".to_owned(), json!(idx));
            result = outcome_words(self.act_part_pin(&params));
            if result.is_err() {
                break;
            }
        }
        if let (Some((draft, undo)), Some(b)) = (
            before,
            self.symbol_editor.as_mut().and_then(|e| e.device_box.as_mut()),
        ) {
            b.undo = undo;
            if b.draft != draft {
                b.undo.push(draft);
            }
            b.selection = selection;
        }
        result
    }

    /// Save: the part saved as the part editor saves it, the editor then on the saved part; where boxes of it are
    /// drawn already and its pins or layout changed, a message offers to redraw them and says how many.
    fn box_save(&mut self) -> Result<(), String> {
        let b = self
            .symbol_editor
            .as_ref()
            .and_then(|e| e.device_box.as_ref())
            .ok_or_else(|| words(key!("symbol-not-open")))?;
        let mut draft = b.draft.clone();
        if let Some(todo) = b.todo().first() {
            return Err(todo.text.clone());
        }
        // The box is saved as the screen shows it: a part that had no layout takes the one drawn here.
        if draft.box_.is_none() && draft.pins_from().is_none() && !draft.pins.is_empty() {
            draft.box_ = Some(draft.box_steps().layout(EDIT_STEP));
        }
        let from_project = b.from_project;
        let layout_changed = (&draft.pins, &draft.box_) != (&b.saved.pins, &b.saved.box_);
        let project = self
            .project
            .as_mut()
            .ok_or_else(|| words(key!("reason-no-project")))?;
        let command = if draft.new {
            project.session.new_part_command(&draft).map(|made| made.command)
        } else if draft.own && draft.fork {
            project.session.own_part_update(&draft).map(|plan| plan.command)
        } else {
            project.session.part_edit_command(&draft)
        }
        .map_err(draft_words)?;
        outcome_words(self.run_commit(&command))?;
        // The saved part is the project's copy of the draft's item: the part itself, or the fork that Save made.
        let project = self
            .project
            .as_ref()
            .ok_or_else(|| words(key!("reason-no-project")))?;
        let session = &project.session;
        let number = draft.text("part_number").trim().to_owned();
        let fork = command
            .params
            .get("fork")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok());
        let copy = match (fork, draft.new) {
            (Some(item), _) => session.part_copy_of_item(item),
            (None, true) => session.part_copy_of_item(draft.copy),
            (None, false) => Some(draft.copy),
        }
        .ok_or_else(|| words(key!("reason-part-edit-target")))?;
        let saved = session
            .part_draft(copy)
            .map_err(|_| words(key!("reason-part-edit-target")))?;
        let drawn = if layout_changed && saved.box_.is_some() {
            session.boxes_of_part(copy).len()
        } else {
            0
        };
        let epoch = project.epoch;
        if let Some(b) = self.symbol_editor.as_mut().and_then(|e| e.device_box.as_mut()) {
            *b = DeviceBox {
                from_project,
                ..DeviceBox::new(saved)
            };
        }
        self.refresh_rows();
        if drawn > 0 {
            self.notes.push(
                strings::format(key!("box-saved-redraw"), &[&number, &drawn.to_string()]),
                "part",
                Some((
                    NoteAction::RedrawBoxes,
                    Validity::Session {
                        epoch,
                        select: vec![copy],
                    },
                )),
            );
        } else {
            self.notify_toned(strings::format(key!("box-saved"), &[&number]), NoteTone::Success);
        }
        Ok(())
    }

    /// `part.redraw_boxes {part}`: every box drawn of a device of the part redrawn as the part's layout says, where
    /// it stands; its wires follow their pins. One undo step per box.
    pub(crate) fn act_part_redraw_boxes(&mut self, p: &Map<String, Value>) -> Outcome {
        let copy = match self.part_edit_copy(p) {
            Ok(Some(c)) => c,
            Ok(None) => return Outcome::refused(key!("reason-part-edit-target")),
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let commands: Vec<_> = project
            .session
            .boxes_of_part(copy)
            .into_iter()
            .filter_map(|rep| project.session.redraw_box_command(copy, rep))
            .collect();
        let count = commands.len();
        for command in commands {
            let out = self.run_commit(&command);
            if out != Outcome::Done {
                return out;
            }
        }
        self.notify_toned(
            strings::format(key!("box-redrawn"), &[&count.to_string()]),
            NoteTone::Success,
        );
        Outcome::Done
    }

    /// Test on a page: the box saved, then the Device tool armed with the part on the page in front, the editor
    /// stepping aside as after a symbol's test placement: a click places the box as it is laid out.
    fn box_test(&mut self) -> Result<(), String> {
        let dirty = self
            .symbol_editor
            .as_ref()
            .and_then(|e| e.device_box.as_ref())
            .is_some_and(|b| b.dirty() || b.draft.new);
        if dirty {
            self.box_save()?;
        }
        let (number, copy, template) = self
            .symbol_editor
            .as_ref()
            .and_then(|e| e.device_box.as_ref())
            .map(|b| {
                (
                    b.draft.text("part_number").trim().to_owned(),
                    b.draft.copy,
                    b.draft.main_entry(),
                )
            })
            .unwrap_or_default();
        if let Some(editor) = &mut self.symbol_editor {
            editor.shown = false;
        }
        self.sync_views();
        self.sync_tool_context();
        let placed = outcome_words(self.dispatch(
            "place.pick",
            &json!({ "part": copy.to_string(), "template": template }),
        ));
        if placed.is_err()
            && let Some(editor) = &mut self.symbol_editor
        {
            editor.shown = true;
        }
        placed?;
        self.notes.push(
            strings::format(key!("box-test-armed"), &[&number]),
            "symbol",
            Some((NoteAction::BackToEditor, Validity::SymbolDraft)),
        );
        Ok(())
    }
}
