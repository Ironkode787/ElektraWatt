// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What is selected in the symbol editor's drawing, and what a person does with it as on a page: select (one, more,
//! all), move by the pointer or the arrow keys, delete, copy, cut, paste, duplicate, line up, and drag a shape's
//! handle. Each change of the draft is one step of the editor's undo history.

use super::{SymbolEditor, damaged, edit_error, nm, text, words};
use crate::{key, strings};
use ew_api::symbols::SymbolDraft;
use serde_json::{Map, Value, json};

/// A thing of the drawing a person can select: by the list of the symbol that holds it and its place there.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Item {
    /// A connection point.
    Cp(usize),
    /// A text field (the tag, a pin's number): a text the page writes.
    Slot(usize),
    /// A shape or a text of the drawing.
    Graphic(usize),
    /// A link point.
    Link(usize),
    /// The cross-reference point of its own.
    Anchor,
}

impl Item {
    /// The member of the symbol that lists the item and the item's place in it; `None` for the cross-reference point.
    pub fn place(self) -> Option<(&'static str, usize)> {
        match self {
            Item::Cp(i) => Some(("cps", i)),
            Item::Slot(i) => Some(("text_slots", i)),
            Item::Graphic(i) => Some(("graphics", i)),
            Item::Link(i) => Some(("link_points", i)),
            Item::Anchor => None,
        }
    }

    /// The item as an action names it: `cp:0`, `slot:1`, `graphic:2`, `link:0`, `anchor`.
    pub fn word(self) -> String {
        match self {
            Item::Cp(i) => format!("cp:{i}"),
            Item::Slot(i) => format!("slot:{i}"),
            Item::Graphic(i) => format!("graphic:{i}"),
            Item::Link(i) => format!("link:{i}"),
            Item::Anchor => "anchor".to_owned(),
        }
    }

    /// The item [`Item::word`] names; `None` for any other text.
    pub fn parse(word: &str) -> Option<Item> {
        if word == "anchor" {
            return Some(Item::Anchor);
        }
        let (kind, index) = word.split_once(':')?;
        let i = index.parse().ok()?;
        match kind {
            "cp" => Some(Item::Cp(i)),
            "slot" => Some(Item::Slot(i)),
            "graphic" => Some(Item::Graphic(i)),
            "link" => Some(Item::Link(i)),
            _ => None,
        }
    }

    /// The item an issue's JSON pointer names (`/symbol/cps/0`); `None` for an issue of the symbol itself.
    pub fn of_pointer(pointer: &str) -> Option<Item> {
        let mut parts = pointer.strip_prefix("/symbol/")?.split('/');
        let (list, index) = (parts.next()?, parts.next());
        if list == "anchor" {
            return Some(Item::Anchor);
        }
        let i = index?.parse().ok()?;
        match list {
            "cps" => Some(Item::Cp(i)),
            "text_slots" => Some(Item::Slot(i)),
            "graphics" => Some(Item::Graphic(i)),
            "link_points" => Some(Item::Link(i)),
            _ => None,
        }
    }
}

/// The value of `item` in the draft's document `doc`.
pub(super) fn value_of(doc: &Value, item: Item) -> Option<&Value> {
    match item.place() {
        Some((list, i)) => doc["symbol"][list].get(i),
        None => doc["symbol"]["anchor"].get("named"),
    }
}

/// The value of `item` in `doc`, to change it.
pub(super) fn value_mut(doc: &mut Value, item: Item) -> Option<&mut Value> {
    match item.place() {
        Some((list, i)) => doc["symbol"][list].get_mut(i),
        None => doc["symbol"]["anchor"].get_mut("named"),
    }
}

/// The members of a shape, a point or a text that hold points.
const POINTS: [&str; 6] = ["from", "to", "centre", "pos", "rect", "points"];

/// Every point of `v` (a pair of whole numbers under one of [`POINTS`], or the pair itself), visited with `f`.
fn each_point(v: &mut Value, f: &mut dyn FnMut(&mut i64, &mut i64)) {
    fn pair(v: &mut Value, f: &mut dyn FnMut(&mut i64, &mut i64)) {
        let Some(a) = v.as_array_mut() else { return };
        if a.len() == 2 && a.iter().all(Value::is_i64) {
            let (mut x, mut y) = (a[0].as_i64().unwrap_or(0), a[1].as_i64().unwrap_or(0));
            f(&mut x, &mut y);
            a[0] = json!(x);
            a[1] = json!(y);
        } else {
            for v in a {
                pair(v, f);
            }
        }
    }
    match v {
        Value::Array(_) => pair(v, f),
        Value::Object(map) => {
            for (k, v) in map {
                // A primitive's tag (`rect` of a rectangle) holds an object; only an array is points.
                if v.is_array() && POINTS.contains(&k.as_str()) {
                    pair(v, f);
                } else if v.is_object() {
                    each_point(v, f);
                }
            }
        }
        _ => {}
    }
}

/// Moves the value of an item by (`dx`, `dy`) nanometres.
pub(super) fn translate(v: &mut Value, dx: i64, dy: i64) {
    each_point(v, &mut |x, y| {
        *x = x.saturating_add(dx);
        *y = y.saturating_add(dy);
    });
}

/// The bounds of an item's geometry in nanometres, `[left, top, right, bottom]`: its points, a circle with its
/// radius; a text by the point it stands at. `None` for a value without a point.
pub fn bounds(v: &Value) -> Option<[i64; 4]> {
    let mut b: Option<[i64; 4]> = None;
    let mut v = v.clone();
    each_point(&mut v, &mut |x, y| {
        let (x, y) = (*x, *y);
        b = Some(b.map_or([x, y, x, y], |b| {
            [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
        }));
    });
    let r = v.get("circle").and_then(|c| c["r"].as_i64()).unwrap_or(0);
    b.map(|b| [b[0] - r, b[1] - r, b[2] + r, b[3] + r])
}

/// The handles of a shape in nanometres, in the order `symbol.grip` numbers them: a line's two ends; the four corners
/// of a rectangle, an ellipse, an arc and an image (top left, then clockwise); a circle's four points (right, bottom,
/// left, top); every corner of a polyline or a polygon. A text and a point have none: they are moved.
pub fn grips(v: &Value) -> Vec<[i64; 2]> {
    let point = |v: &Value| Some([v.get(0)?.as_i64()?, v.get(1)?.as_i64()?]);
    let Some((kind, shape)) = v.as_object().and_then(|m| m.iter().next()) else {
        return Vec::new();
    };
    let found = match kind.as_str() {
        "line" => point(&shape["from"])
            .zip(point(&shape["to"]))
            .map(|(a, b)| vec![a, b]),
        "rect" | "ellipse" | "arc" | "image" => point(&shape["rect"][0])
            .zip(point(&shape["rect"][1]))
            .map(|(a, b)| vec![a, [b[0], a[1]], b, [a[0], b[1]]]),
        "circle" => point(&shape["centre"]).zip(shape["r"].as_i64()).map(|(c, r)| {
            vec![
                [c[0] + r, c[1]],
                [c[0], c[1] + r],
                [c[0] - r, c[1]],
                [c[0], c[1] - r],
            ]
        }),
        "polyline" | "polygon" => shape["points"]
            .as_array()
            .map(|a| a.iter().filter_map(point).collect()),
        _ => None,
    };
    found.unwrap_or_default()
}

/// Puts handle `grip` of the shape `v` (as [`grips`] numbers them) at `to`; whether the shape has that handle.
fn move_grip(v: &mut Value, grip: usize, to: [i64; 2]) -> bool {
    let before = grips(v);
    if grip >= before.len() {
        return false;
    }
    let Some((kind, shape)) = v.as_object_mut().and_then(|m| m.iter_mut().next()) else {
        return false;
    };
    match kind.as_str() {
        "line" => shape[if grip == 0 { "from" } else { "to" }] = json!(to),
        // The corner opposite the handle stays; the rectangle is kept in order.
        "rect" | "ellipse" | "arc" | "image" => {
            let fixed = before[(grip + 2) % 4];
            shape["rect"] = json!([
                [fixed[0].min(to[0]), fixed[1].min(to[1])],
                [fixed[0].max(to[0]), fixed[1].max(to[1])]
            ]);
        }
        "circle" => {
            let centre = [
                (before[0][0] + before[2][0]) / 2,
                (before[1][1] + before[3][1]) / 2,
            ];
            let r = ((to[0] - centre[0]) as f64).hypot((to[1] - centre[1]) as f64);
            shape["r"] = json!(r.round() as i64);
        }
        _ => shape["points"][grip] = json!(to),
    }
    true
}

/// What Copy and Cut hold: each item's value with its kind, and for a connection point the function pin it draws.
#[derive(Clone, Debug, Default)]
pub struct Clipboard(Vec<(Item, Value, Option<Value>)>);

impl Clipboard {
    /// Whether nothing was copied yet.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl SymbolEditor {
    /// Keeps of the selection what the draft still holds, after a change of the draft.
    pub(super) fn prune_selection(&mut self) {
        let doc = serde_json::to_value(&self.draft).unwrap_or_default();
        self.selection.retain(|i| value_of(&doc, *i).is_some());
    }

    /// Selects the thing a tool just made, the last of its list (`kind` names the list; its place is not read), so
    /// its rows show in Properties.
    pub(super) fn select_last(&mut self, kind: Item) {
        let s = &self.draft.symbol;
        let last = |n: usize| n.checked_sub(1);
        let item = match kind {
            Item::Cp(_) => last(s.cps.len()).map(Item::Cp),
            Item::Slot(_) => last(s.text_slots.len()).map(Item::Slot),
            Item::Graphic(_) => last(s.graphics.len()).map(Item::Graphic),
            Item::Link(_) => last(s.link_points.len()).map(Item::Link),
            Item::Anchor => Some(Item::Anchor),
        };
        self.selection = item.into_iter().collect();
    }

    /// Every thing of the drawing, in the order Select all takes them.
    pub fn items(&self) -> Vec<Item> {
        let s = &self.draft.symbol;
        let mut out: Vec<Item> = (0..s.cps.len()).map(Item::Cp).collect();
        out.extend((0..s.text_slots.len()).map(Item::Slot));
        out.extend((0..s.graphics.len()).map(Item::Graphic));
        out.extend((0..s.link_points.len()).map(Item::Link));
        if matches!(s.anchor, ew_api::symbols::schema::AnchorPolicy::Named(_)) {
            out.push(Item::Anchor);
        }
        out
    }

    /// The step a move of the selection keeps to, in nanometres: the connection grid's while a connection point is
    /// selected (it must stay on it), else the drawing grid's.
    pub fn move_step(&self) -> i64 {
        if self.selection.iter().any(|i| matches!(i, Item::Cp(_))) {
            self.draft.symbol.grid.at(1).map_or(4_000_000, |n| n.0)
        } else {
            self.grid_step()
        }
    }

    /// The drawing grid's step in nanometres.
    pub fn grid_step(&self) -> i64 {
        let (num, den) = super::GRID_STEPS[self.grid.min(super::GRID_STEPS.len() - 1)];
        (num / den).max(1)
    }

    /// The draft as its document, to change it; the editor takes the changed one with [`SymbolEditor::take`].
    fn document(&self) -> Result<Value, String> {
        serde_json::to_value(&self.draft).map_err(|e| e.to_string())
    }

    /// Takes the changed document `doc` as the draft, one undo step.
    pub(super) fn take(&mut self, doc: &Value) -> Result<(), String> {
        let draft =
            SymbolDraft::parse(&serde_json::to_vec(doc).map_err(|e| e.to_string())?).map_err(edit_error)?;
        self.replace(draft);
        Ok(())
    }

    /// `symbol.select {items, mode?}`: `items` a list of item words or `all`; `mode` `replace` (the default), `add`
    /// (Shift) or `toggle` (Ctrl).
    pub(super) fn select(&mut self, p: &Map<String, Value>) -> Result<(), String> {
        let all = self.items();
        let named: Vec<Item> = match p.get("items") {
            Some(Value::String(s)) if s == "all" => all.clone(),
            Some(Value::Array(list)) => list
                .iter()
                .map(|v| v.as_str().and_then(Item::parse).filter(|i| all.contains(i)))
                .collect::<Option<_>>()
                .ok_or_else(|| words(key!("symbol-select-invalid")))?,
            _ => return Err(words(key!("symbol-select-invalid"))),
        };
        match text(p, "mode") {
            "add" => {
                for item in named {
                    self.selection.retain(|i| *i != item);
                    self.selection.push(item);
                }
            }
            "toggle" => {
                for item in named {
                    if self.selection.contains(&item) {
                        self.selection.retain(|i| *i != item);
                    } else {
                        self.selection.push(item);
                    }
                }
            }
            _ => self.selection = named,
        }
        Ok(())
    }

    fn selected(&self) -> Result<Vec<Item>, String> {
        if self.selection.is_empty() {
            return Err(words(key!("symbol-nothing-selected")));
        }
        Ok(self.selection.clone())
    }

    /// `symbol.move {dx, dy}` in millimetres, and `symbol.nudge {dx, dy}` in steps of [`SymbolEditor::move_step`] (the
    /// arrow keys): the selection moved, one undo step.
    pub(super) fn move_selection(&mut self, p: &Map<String, Value>, steps: bool) -> Result<(), String> {
        let items = self.selected()?;
        let (dx, dy) = if steps {
            let count = |name: &str| p.get(name).and_then(Value::as_i64).unwrap_or(0);
            let step = self.move_step();
            (count("dx").saturating_mul(step), count("dy").saturating_mul(step))
        } else {
            let part = |name: &str| if p.contains_key(name) { nm(p, name) } else { Ok(0) };
            (part("dx")?, part("dy")?)
        };
        if (dx, dy) == (0, 0) {
            return Ok(());
        }
        let mut doc = self.document()?;
        for item in items {
            if let Some(v) = value_mut(&mut doc, item) {
                translate(v, dx, dy);
            }
        }
        self.take(&doc)
    }

    /// Removes `items` from `doc`: a connection point with the texts that write its number and, in a draft that owns
    /// its function, the function pin no other point draws; the cross-reference point returns to the origin.
    fn remove(&self, doc: &mut Value, items: &[Item]) {
        let mut items = items.to_vec();
        items.sort();
        items.dedup();
        let mut gone_cps = Vec::new();
        for item in items.into_iter().rev() {
            match item.place() {
                None => doc["symbol"]["anchor"] = json!("origin"),
                Some((list, i)) => {
                    if let Some(a) = doc["symbol"][list].as_array_mut()
                        && i < a.len()
                    {
                        let v = a.remove(i);
                        if list == "cps" {
                            gone_cps.push(v);
                        }
                    }
                }
            }
        }
        for cp in gone_cps {
            if let Some(slots) = doc["symbol"]["text_slots"].as_array_mut() {
                slots.retain(|s| s["kind"].get("cp_designation") != Some(&cp["idx"]));
            }
            let still_drawn = doc["symbol"]["cps"]
                .as_array()
                .is_some_and(|a| a.iter().any(|c| c["fn_cp"] == cp["fn_cp"]));
            if self.draft.function_ref.is_none()
                && !still_drawn
                && let Some(pins) = doc["function"]["cps"].as_array_mut()
            {
                pins.retain(|pin| pin["idx"] != cp["fn_cp"]);
            }
        }
    }

    /// `symbol.delete`: the selection removed, one undo step.
    pub(super) fn delete_selection(&mut self) -> Result<(), String> {
        let items = self.selected()?;
        let mut doc = self.document()?;
        self.remove(&mut doc, &items);
        self.selection.clear();
        self.take(&doc)
    }

    /// What Copy holds of the selection.
    pub(super) fn copied(&self) -> Result<Clipboard, String> {
        let doc = self.document()?;
        let mut items = self.selected()?;
        items.sort();
        Ok(Clipboard(
            items
                .into_iter()
                .filter_map(|item| {
                    let v = value_of(&doc, item)?.clone();
                    let pin = matches!(item, Item::Cp(_))
                        .then(|| {
                            doc["function"]["cps"]
                                .as_array()?
                                .iter()
                                .find(|pin| pin["idx"] == v["fn_cp"])
                                .cloned()
                        })
                        .flatten();
                    Some((item, v, pin))
                })
                .collect(),
        ))
    }

    /// `symbol.paste` and `symbol.duplicate`: the things of `clip` added one connection-grid step right and down of
    /// where they were copied (so a connection point stays on its grid), and selected. A connection point takes the
    /// next free number and, in a draft that owns its function, a function pin of its own with the copied one's name.
    pub(super) fn paste(&mut self, clip: &Clipboard) -> Result<(), String> {
        if clip.is_empty() {
            return Err(words(key!("symbol-nothing-copied")));
        }
        let step = self.draft.symbol.grid.at(1).map_or(4_000_000, |n| n.0);
        let mut doc = self.document()?;
        let own_function = self.draft.function_ref.is_none();
        let mut pasted = Vec::new();
        let next = |list: &Value, member: &str| {
            list.as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v[member].as_u64())
                .max()
                .map_or(1, |n| n + 1)
        };
        for (item, value, pin) in &clip.0 {
            let mut v = value.clone();
            translate(&mut v, step, step);
            let Some((list, _)) = item.place() else {
                doc["symbol"]["anchor"] = json!({ "named": v });
                pasted.push(Item::Anchor);
                continue;
            };
            if list == "cps" {
                let idx = next(&doc["symbol"]["cps"], "idx");
                if idx > u64::from(u16::MAX) {
                    return Err(words(key!("symbol-cp-index-invalid")));
                }
                v["idx"] = json!(idx);
                if own_function && let Some(pin) = pin {
                    let fn_idx = next(&doc["function"]["cps"], "idx");
                    let mut pin = pin.clone();
                    pin["idx"] = json!(fn_idx);
                    v["fn_cp"] = json!(fn_idx);
                    doc["function"]["cps"]
                        .as_array_mut()
                        .ok_or_else(damaged)?
                        .push(pin);
                }
            }
            let target = doc["symbol"]
                .as_object_mut()
                .ok_or_else(damaged)?
                .entry(list)
                .or_insert(json!([]))
                .as_array_mut()
                .ok_or_else(damaged)?;
            target.push(v);
            let i = target.len() - 1;
            pasted.push(match item {
                Item::Cp(_) => Item::Cp(i),
                Item::Slot(_) => Item::Slot(i),
                Item::Graphic(_) => Item::Graphic(i),
                Item::Link(_) | Item::Anchor => Item::Link(i),
            });
        }
        self.take(&doc)?;
        self.selection = pasted;
        self.prune_selection();
        Ok(())
    }

    /// `symbol.align {to}`: the selected things lined up on the left, centre or right, or the top, middle or bottom,
    /// of what they span together; one undo step. Refused with fewer than two things.
    pub(super) fn align(&mut self, p: &Map<String, Value>) -> Result<(), String> {
        let items = self.selected()?;
        let mut doc = self.document()?;
        let each: Vec<(Item, [i64; 4])> = items
            .iter()
            .filter_map(|i| Some((*i, bounds(value_of(&doc, *i)?)?)))
            .collect();
        if each.len() < 2 {
            return Err(words(key!("symbol-align-two")));
        }
        let all = each.iter().fold(each[0].1, |a, (_, b)| {
            [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
        });
        for (item, b) in each {
            let (dx, dy) = match text(p, "to") {
                "left" => (all[0] - b[0], 0),
                "centre" => ((all[0] + all[2] - b[0] - b[2]) / 2, 0),
                "right" => (all[2] - b[2], 0),
                "top" => (0, all[1] - b[1]),
                "middle" => (0, (all[1] + all[3] - b[1] - b[3]) / 2),
                "bottom" => (0, all[3] - b[3]),
                _ => return Err(words(key!("symbol-align-invalid"))),
            };
            if let Some(v) = value_mut(&mut doc, item) {
                translate(v, dx, dy);
            }
        }
        self.take(&doc)
    }

    /// `symbol.grip {grip, x, y}`: handle `grip` of the one selected shape put at `x`/`y` (millimetres), one undo step.
    pub(super) fn grip(&mut self, p: &Map<String, Value>) -> Result<(), String> {
        let [item @ Item::Graphic(_)] = self.selection[..] else {
            return Err(words(key!("symbol-grip-one")));
        };
        let grip = p
            .get("grip")
            .and_then(Value::as_u64)
            .and_then(|g| usize::try_from(g).ok())
            .ok_or_else(|| words(key!("symbol-grip-one")))?;
        let to = [nm(p, "x")?, nm(p, "y")?];
        let mut doc = self.document()?;
        let moved = value_mut(&mut doc, item).is_some_and(|v| move_grip(v, grip, to));
        if !moved {
            return Err(words(key!("symbol-grip-one")));
        }
        self.take(&doc)
    }

    /// `symbol.grid {step?, snap?}`: the drawing grid's step, the offered one nearest to `step` millimetres, and
    /// whether the drawing tools snap to it.
    pub(super) fn set_grid(&mut self, p: &Map<String, Value>) -> Result<(), String> {
        if p.contains_key("step") {
            let want = nm(p, "step")?;
            self.grid = (0..super::GRID_STEPS.len())
                .min_by_key(|i| {
                    let (num, den) = super::GRID_STEPS[*i];
                    (num / den - want).abs()
                })
                .unwrap_or(0);
        }
        if let Some(snap) = p.get("snap").and_then(Value::as_bool) {
            self.snap = snap;
        }
        Ok(())
    }
}

/// The words of a potential role as a role list shows them: codes as printed, Signal and Undefined in words.
pub fn role_words(role: &str) -> String {
    match role {
        "" | "none" => strings::text(key!("symbol-role-none")).to_owned(),
        "signal" | "undefined" => {
            strings::text(strings::Key::run_time(&format!("symbol-role-{role}"))).to_owned()
        }
        _ => role
            .to_uppercase()
            .replace("L_PLUS", "L+")
            .replace("L_MINUS", "L-"),
    }
}
