// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The box of a part drawn as a box with pins, arranged in Parts (doc 07 §2.5, §5.6: Arrange…): the drawing shows the
//! box as a device of the part is drawn; a pin is dragged along its edge or onto another edge, an edge to size the
//! box. There is one list of pins, the open sheet's: the box works on a copy of the sheet, which Done and To the part
//! give back to it, so a pin dragged is its row changed; nothing is saved until the part is (its Save's card names
//! the boxes it draws anew).

use super::{Outline, OutlineGroup, RowValue, Sheet, SheetRow, SymbolEditor, Todo, role_words, words};
use crate::shell::{Outcome, ShellModel};
use crate::{key, strings};
use ew_api::part_box::{BoxSteps, Dir4, EDIT_STEP, PinsFrom};
use ew_api::{PartDraftError, PartSheet};
use serde_json::{Map, Value, json};

/// The edges of a box in the order the pin table lists them, each with the word an action names it by.
pub const SIDES: [(&str, Dir4, strings::Key<'static>); 4] = [
    ("top", Dir4::Up, key!("box-side-top")),
    ("bottom", Dir4::Down, key!("box-side-bottom")),
    ("left", Dir4::Left, key!("box-side-left")),
    ("right", Dir4::Right, key!("box-side-right")),
];

/// The word of an edge.
pub fn side_word(side: &Dir4) -> &'static str {
    SIDES
        .iter()
        .find(|(_, d, _)| d == side)
        .map_or("bottom", |(w, ..)| w)
}

fn side_of(word: &str) -> Option<Dir4> {
    SIDES.iter().find(|(w, ..)| *w == word).map(|(_, d, _)| d.clone())
}

/// The words of an edge (Top, Bottom, Left, Right).
pub fn side_words(side: &Dir4) -> String {
    let key = SIDES
        .iter()
        .find(|(_, d, _)| d == side)
        .map_or(key!("box-side-bottom"), |(.., k)| *k);
    strings::text(key).to_owned()
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
    pub draft: PartSheet,
    /// The draft as it was opened or last saved; equality decides unsaved changes.
    saved: PartSheet,
    undo: Vec<PartSheet>,
    redo: Vec<PartSheet>,
    /// The selected pins, in the order they were selected.
    pub selection: Vec<u16>,
    /// Counts the changes, so the window reads the box again.
    pub revision: u64,
    /// Why the last action did nothing.
    pub error: Option<String>,
    /// The pin table's order held while the same pins are edited (a pin given another edge jumped to that
    /// edge's rows under the pointer): the selection it was taken for and the pins in that order.
    held: Option<(Vec<u16>, Vec<u16>)>,
}

impl DeviceBox {
    pub(super) fn new(draft: PartSheet) -> DeviceBox {
        DeviceBox {
            saved: draft.clone(),
            draft,
            undo: Vec::new(),
            redo: Vec::new(),
            selection: Vec::new(),
            revision: 0,
            error: None,
            held: None,
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

    /// Where the pins come from when they are not added, named and removed here, in a sentence; `None` when they are.
    pub fn pins_from(&self) -> Option<String> {
        self.draft.pins_from().map(from_words)
    }

    /// The box's size in millimetres and in steps of the 4 mm grid a page draws it on: `(width, height)` each.
    pub fn size(&self) -> ((f64, f64), (i64, i64)) {
        let b = self.steps();
        let mm = |steps: i64| (steps * EDIT_STEP) as f64 / 1e6;
        ((mm(b.w), mm(b.h)), (b.w, b.h))
    }

    /// Whether the pins' places on the box and the box's size are edited here: also on a PLC card's or a bus
    /// device's box, whose pins' names and potentials are the part's.
    pub fn places_pins(&self) -> bool {
        self.draft.places_pins()
    }

    /// The pin table: the pins in the order they stand on the box, the edges top, bottom, left, right, each from its
    /// start; while the same pins stay selected and are edited, in the order they had when the editing began, so a row
    /// keeps its place under the pointer, and a new selection sorts them again.
    pub fn pin_table(&self) -> Vec<PinLine> {
        let mut lines = self.placed_pins();
        if let Some((selection, order)) = &self.held
            && *selection == self.selection
        {
            lines.sort_by_key(|l| order.iter().position(|i| *i == l.idx).unwrap_or(usize::MAX));
        }
        lines
    }

    /// The pins in the order they stand on the box.
    fn placed_pins(&self) -> Vec<PinLine> {
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
    fn edit<T>(&mut self, change: impl FnOnce(&mut PartSheet) -> T) -> T {
        // The table's order is held from the first edit of these pins on.
        if self
            .held
            .as_ref()
            .is_none_or(|(selection, _)| *selection != self.selection)
        {
            let order = self.placed_pins().iter().map(|l| l.idx).collect();
            self.held = Some((self.selection.clone(), order));
        }
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

    /// What a pin's text that cannot be read says, under the box.
    fn todo(&self) -> Vec<Todo> {
        let d = &self.draft;
        d.messages
            .iter()
            .map(|m| {
                let args: Vec<&str> = m.args.iter().map(String::as_str).collect();
                let text = strings::format(strings::Key::run_time(m.key), &args);
                Todo { text, item: None }
            })
            .collect()
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
        // A card's or a bus device's pins keep their names and potentials, which are the part's; their places and
        // the box's size are the box's.
        let locked = self.pins_from();
        let placed = locked.clone().filter(|_| !self.places_pins());
        let b = self.steps();
        let mm = |steps: i64| RowValue::Mm(Some((steps * EDIT_STEP) as f64 / 1e6));
        if self.selection.is_empty() {
            // The part's identity is its sheet's; here the box's size.
            let mut rows: Vec<SheetRow> = Vec::new();
            for (key, label, steps) in [
                ("width", key!("symbol-width-mm"), b.w),
                ("height", key!("symbol-height-mm"), b.h),
            ] {
                rows.push(SheetRow {
                    value: mm(steps),
                    locked: placed.clone(),
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
            .map(|(w, d, _)| ((*w).to_owned(), side_words(d)))
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
                ..text("role", key!("box-pin-carries"), String::new())
            },
            SheetRow {
                value: RowValue::Choice {
                    value: same(&|p| side_word(&p.side).to_owned()),
                    options: sides,
                },
                locked: placed.clone(),
                ..text("side", key!("box-pin-side"), String::new())
            },
            SheetRow {
                value: RowValue::Mm(
                    same(&|p| p.at.to_string())
                        .and_then(|k| k.parse::<i64>().ok())
                        .map(|k| (k * EDIT_STEP) as f64 / 1e6),
                ),
                help: Some(words(key!("box-pin-at-help"))),
                locked: placed,
                ..text("at", key!("box-pin-at"), String::new())
            },
        ];
        let (title, line) = match chosen[..] {
            [pin] => (
                strings::format(key!("box-pin-title"), &[&pin.name]),
                strings::format(key!("box-pin-line"), &[&self.title()]),
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

/// Where a part's pins come from, in a sentence that says what is changed where.
fn from_words(from: PinsFrom) -> String {
    words(match from {
        PinsFrom::Channels => key!("box-pins-from-channels"),
        PinsFrom::Ports => key!("box-pins-from-ports"),
        PinsFrom::Symbol => key!("box-pins-from-symbol"),
    })
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
        Outcome::Refused { reason: reason_key } => Err(strings::render(&reason_key)),
    }
}

impl ShellModel {
    /// `parts.draw {what: box, part?}` (Arrange… under the part's pins in Parts and in the device browser, which names
    /// it by `part`; Arrange its box… and Add pin on a placed box, which name it by the device in Properties): the
    /// part's box as the drawing of the place, its pins the sheet's. A part that is not his opens in Parts as it is,
    /// and a line says that Make my own from it is the way to its pins; on the open sheet it is refused in words.
    pub(crate) fn arrange_box(&mut self, part: Option<&str>) -> Outcome {
        let open_here = part.is_some() || self.parts.as_deref().and_then(|p| p.sheet.as_ref()).is_none();
        if open_here {
            let named = part.map_or_else(|| json!({}), |part| json!({ "part": part }));
            let opened = self.dispatch("parts.open", &named);
            if opened != Outcome::Done {
                return opened;
            }
        }
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        if !open.editable() {
            if open_here {
                self.notify(strings::text(key!("toast-box-pin-not-yours")).to_owned(), false);
                return Outcome::Done;
            }
            return Outcome::refused(key!("reason-part-not-yours"));
        }
        let draft = open.sheet.clone();
        if self.symbol_editor.is_none() {
            // A drawing holds the box; nothing of a symbol is in hand under it.
            let Ok(blank) = ew_api::symbols::SymbolDraft::new("", strings::text(key!("symbol-untitled")))
            else {
                return Outcome::refused(key!("reason-no-part-open"));
            };
            self.symbol_editor = Some(SymbolEditor::new(blank));
        }
        self.symbol_editor_generation = self.symbol_editor_generation.wrapping_add(1);
        let generation = self.symbol_editor_generation;
        if let Some(editor) = &mut self.symbol_editor {
            editor.shown = true;
            editor.generation = generation;
            editor.device_box = Some(DeviceBox::new(draft));
        }
        self.enter_place();
        Outcome::Done
    }

    /// Done and To the part on the box: the open sheet takes the box's pins and their places, and the place shows the
    /// part again; a drawing of a symbol in hand under the box stays in hand.
    pub(crate) fn close_box(&mut self) -> Outcome {
        let Some(b) = self.symbol_editor.as_mut().and_then(|e| e.device_box.take()) else {
            return Outcome::refused(key!("reason-no-device-box"));
        };
        let changed = b.dirty();
        let mut draft = b.draft;
        // A box arranged keeps its layout as the screen shows it, as 0.3.5's box Save did: a part with pins of its own
        // that had none takes the one drawn here. A card's or a bus device's box takes one only from a pin placed or
        // a size given (`relayout`). A box nobody arranged keeps none, and its part's Save draws no placed box anew.
        if changed && draft.box_.is_none() && draft.pins_from().is_none() && !draft.pins.is_empty() {
            draft.box_ = Some(draft.box_steps().layout(EDIT_STEP));
        }
        if let Some(editor) = &mut self.symbol_editor {
            editor.shown = false;
            if !editor.in_hand() {
                self.symbol_editor = None;
            }
        }
        if changed && let Some(open) = self.parts.as_deref_mut().and_then(|p| p.sheet.as_mut()) {
            open.sheet = draft;
            self.sheet_changed();
        }
        Outcome::Done
    }

    /// The draft of the part the pin actions change: the box's while it is arranged, else the open sheet of Parts.
    fn pin_draft(&mut self) -> Option<&mut PartSheet> {
        if let Some(b) = self.symbol_editor.as_mut().and_then(|e| e.device_box.as_mut()) {
            return Some(&mut b.draft);
        }
        self.parts
            .as_deref_mut()
            .and_then(|p| p.sheet.as_mut())
            .map(|o| &mut o.sheet)
    }

    /// `part.pin {add?, name?, remove?, pin?, role?, side?, at?, before?}`: the pins of the part being edited. `add`
    /// adds a pin (named `name`, else the next free number); `remove` removes pin `remove`; with `pin`: `name` and
    /// `role` name it and say what it carries, `side` with `at` (millimetres along the edge) puts it there, and
    /// `side` alone moves it in the pin table's order onto that edge, before pin `before` or last. Refused in words
    /// for a part whose pins come from its channels, its ports or its symbol, except that a card's or a bus device's
    /// pin is put on its edge and moved along it (its name and what it carries are the part's).
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
            return Outcome::refused(key!("reason-no-part-open"));
        };
        if let Some(from) = draft.pins_from() {
            let names = p.get("add") == Some(&Value::Bool(true))
                || p.contains_key("remove")
                || text("name").is_some()
                || text("role").is_some();
            if names || !draft.places_pins() {
                return Outcome::Failed {
                    message: from_words(from),
                };
            }
        }
        // A place typed between two steps of the pin grid, or on a corner, and where the pin stands instead.
        let taken = std::cell::Cell::new(None);
        let place = |d: &mut PartSheet, idx: u16, side: Dir4, mm: f64| -> bool {
            let at = (mm * 1e6).round() as i64;
            if !d.place_pin(idx, side, at) {
                return false;
            }
            let stands = d
                .box_steps()
                .pins
                .into_iter()
                .find(|pin| pin.0 == idx)
                .map_or(at, |pin| pin.2 * EDIT_STEP);
            if stands != at {
                taken.set(Some((mm, stands)));
            }
            true
        };
        let change = |d: &mut PartSheet| -> Result<Option<u16>, String> {
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
                    Some(mm) => place(d, idx, side, mm),
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
                place(d, idx, side, mm);
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
        if let (Ok(_), Some((typed, stands))) = (&result, taken.get()) {
            self.notes.push(crate::notifications::Note::new(
                "part",
                strings::format(
                    key!("box-pin-on-step"),
                    &[
                        &super::sheet::trim_mm(stands as f64 / 1e6),
                        &super::sheet::trim_mm(EDIT_STEP as f64 / 1e6),
                        &super::sheet::trim_mm(typed),
                    ],
                ),
            ));
        }
        match result {
            Ok(_) => Outcome::Done,
            Err(message) => Outcome::Failed { message },
        }
    }

    /// `part.box_size {width, height, from_left?, from_top?}` in millimetres: the box's size, no smaller than its pins
    /// need; with `from_left` the width changes at the left edge and with `from_top` the height at the top, the pins on
    /// the other edges staying where they stand.
    pub(crate) fn act_part_box_size(&mut self, p: &Map<String, Value>) -> Outcome {
        let from = |name: &str| p.get(name) == Some(&Value::Bool(true));
        let side = |name: &str| p.get(name).and_then(Value::as_f64).filter(|v| *v > 0.0);
        let Some(b) = self.symbol_editor.as_mut().and_then(|e| e.device_box.as_mut()) else {
            return Outcome::refused(key!("reason-no-device-box"));
        };
        if !b.places_pins() {
            return Outcome::Failed {
                message: b.pins_from().unwrap_or_default(),
            };
        }
        let now = b.steps();
        let nm = |v: Option<f64>, steps: i64| v.map_or(steps * EDIT_STEP, |mm| (mm * 1e6).round() as i64);
        let (w, h) = (nm(side("width"), now.w), nm(side("height"), now.h));
        let (left, top) = (from("from_left"), from("from_top"));
        b.edit(|d| d.size_box(w, h, left, top));
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

    /// The drawing's own actions while a box is arranged: the keys a person learned on a page act on the pins (Delete
    /// removes the selected ones, an arrow moves them a grid step along their edge, Select all), Undo and Redo step
    /// the box's history, and Test on a page waits for the part's Save. `None` for an action that is the symbol's
    /// alone.
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
                if b.places_pins() {
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
            // A device of the part is placed from the device browser once the part is saved.
            "scratch" => Err(words(key!("box-test-save-first"))),
            // The clipboard holds pins: their names, what they carry and their edges; a pasted pin stands last on its
            // edge, and the pasted pins are selected.
            "copy" | "cut" | "duplicate" | "paste" if b.draft.pins_from().is_some() => {
                Err(b.pins_from().unwrap_or_default())
            }
            "copy" | "cut" | "duplicate" | "paste" => {
                let pins: Vec<(String, String, Dir4)> = b
                    .pin_table()
                    .into_iter()
                    .filter(|l| b.selection.contains(&l.idx))
                    .map(|l| {
                        let role = b
                            .draft
                            .pins
                            .iter()
                            .find(|p| p.idx == l.idx)
                            .map(|p| p.role.clone());
                        (l.name, role.unwrap_or_default(), l.side)
                    })
                    .collect();
                let paste = match action {
                    "paste" => self.symbol_clipboard.1.clone(),
                    "duplicate" => pins.clone(),
                    _ => Vec::new(),
                };
                if action == "paste" && paste.is_empty() {
                    Err(words(key!("symbol-nothing-copied")))
                } else if action != "paste" && pins.is_empty() {
                    Err(words(key!("reason-nothing-selected")))
                } else {
                    if matches!(action, "copy" | "cut") {
                        self.symbol_clipboard = super::Clipboard::of_pins(pins);
                        let gui = self.mode == crate::shell::Mode::Gui;
                        self.clip.set(self.symbol_clipboard.text(), None, 0, gui);
                    }
                    let gone = if action == "cut" {
                        b.selection.clone()
                    } else {
                        Vec::new()
                    };
                    let added = b.edit(|d| {
                        for idx in gone {
                            d.remove_pin(idx);
                        }
                        paste
                            .iter()
                            .map(|(name, role, side)| {
                                let idx = d.add_pin(Some(name));
                                let _ = d.set_pin(idx, None, Some(role));
                                d.move_pin(idx, side.clone(), None);
                                idx
                            })
                            .collect::<Vec<u16>>()
                    });
                    if !added.is_empty() {
                        b.selection = added;
                    }
                    Ok(())
                }
            }
            // The drawing tools are the symbol's: on a box they do nothing.
            "tool" | "grid" | "variant" | "align" | "grip" | "move" => Ok(()),
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
}
