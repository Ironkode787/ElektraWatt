// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The editing actions of the canvas: the picker, placing, the place tool's options and its announcements.

use super::Pick;
use crate::key;
use crate::notifications::{Note, NoteAction};
use crate::shell::terminals::TERMINAL_DESIGNATION;
use crate::shell::{Outcome, ShellModel, failed, string_param};
use crate::strings;
use crate::tools::{
    CanvasEvent, EntityKind, Ghost, Picker, PickerMode, PickerRow, PlaceTool, SelectTool, SelectionChange,
    ToolKey, ToolOutput, entity_kind, selection_kinds, transform,
};
use ew_api::output::{PlacedBy, Uuid};
use ew_dl::EntityRef;
use serde_json::{Map, Value, json};
use std::sync::Arc;

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Actions

    /// `tool.place` and `tool.place_part`: opens the picker of the focused page.
    pub(crate) fn act_open_picker(&mut self, mode: PickerMode) -> Outcome {
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.palette = None;
        self.tools.junction_editor = None;
        let slots = self.template_slots();
        let mut picker = Picker::new(mode, &folio, cat);
        for (p, part) in picker.catalogue.parts.iter().enumerate() {
            for (k, f) in part.functions.iter().enumerate() {
                if let Some((_, tag, _)) = slots.get(&(part.copy, f.index)) {
                    picker.joins.insert((p, k), tag.clone());
                }
            }
        }
        self.tools.picker = Some(picker);
        Outcome::Done
    }

    /// `picker.filter {text}`.
    pub(crate) fn act_picker_filter(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match string_param(p, "text") {
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["text"]),
            Err(e) => return e,
        };
        if let Some(picker) = &mut self.tools.picker {
            picker.set_filter(&text);
        }
        Outcome::Done
    }

    /// `picker.row {step | to}`: Up, Down (`step` −1, 1), Home (`to` `first` or 0), End (`to` `last`, −1 or a large
    /// number). Either parameter alone is enough (the signature lists both as optional).
    pub(crate) fn act_picker_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let step = p.get("step").and_then(Value::as_i64);
        let to = match p.get("to") {
            None => None,
            Some(Value::String(word)) => match word.as_str() {
                "first" => Some(0),
                "last" => Some(-1),
                _ => return failed(key!("error-bad-param"), &["to"]),
            },
            Some(v) => match v.as_i64() {
                Some(t) => Some(t),
                None => return failed(key!("error-bad-param"), &["to"]),
            },
        };
        if step.is_none() && to.is_none() {
            return failed(key!("error-missing-param"), &["step"]);
        }
        if let Some(picker) = &mut self.tools.picker {
            let to = to.map(|t| if t < 0 { picker.rows.len() as i64 - 1 } else { t });
            picker.move_row(step, to);
        }
        Outcome::Done
    }

    /// `picker.category {to | step}`: the browser's tree node by its word (`all`, `project`, `other` or a category
    /// key, [`Node::word`](crate::tools::Node::word)), or `step` nodes down (up when negative, clamped).
    pub(crate) fn act_picker_category(&mut self, p: &Map<String, Value>) -> Outcome {
        let to = match string_param(p, "to") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        let step = p.get("step").and_then(Value::as_i64);
        let Some(picker) = &mut self.tools.picker else {
            return Outcome::refused(key!("reason-no-picker"));
        };
        let node = match (to, step) {
            (Some(word), _) => match crate::tools::Node::of_word(&word) {
                Some(node) => node,
                None => return failed(key!("error-bad-param"), &["to"]),
            },
            (None, Some(step)) => {
                let Some(at) = picker.nodes.iter().position(|(n, _)| *n == picker.node) else {
                    return Outcome::Done;
                };
                let last = picker.nodes.len().saturating_sub(1) as i64;
                let at = (at as i64).saturating_add(step).clamp(0, last) as usize;
                picker.nodes[at].0.clone()
            }
            (None, None) => return failed(key!("error-missing-param"), &["to"]),
        };
        picker.set_node(node);
        Outcome::Done
    }

    /// `picker.open {open?}`: opens the part of the current row to its functions (`open` true, the default) or closes
    /// it, its part's row current then.
    pub(crate) fn act_picker_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let open = match p.get("open") {
            None => true,
            Some(Value::Bool(b)) => *b,
            Some(_) => return failed(key!("error-bad-param"), &["open"]),
        };
        let Some(picker) = &mut self.tools.picker else {
            return Outcome::refused(key!("reason-no-picker"));
        };
        picker.set_open(open);
        Outcome::Done
    }

    /// `picker.new_page`: the current row, a symbol on another connection grid than the page's, placed on
    /// a new page with its grid and selected; the picker closes.
    pub(crate) fn act_picker_new_page(&mut self) -> Outcome {
        let Some(picker) = &self.tools.picker else {
            return Outcome::refused(key!("reason-no-picker"));
        };
        let symbol = match picker.rows.get(picker.row) {
            Some(PickerRow::Symbol(i)) => picker.catalogue.symbols.get(*i),
            _ => None,
        };
        let Some((key, letter, [(num, den), _])) =
            symbol.and_then(|s| Some((s.key.clone(), s.letter.clone(), s.grid_mismatch?)))
        else {
            return Outcome::refused(key!("reason-no-grid-mismatch"));
        };
        self.tools.picker = None;
        let back = self.current_folio().map(str::to_owned);
        if let Err(message) = self.new_page_with_grid(json!({"num": num, "den": den}), None) {
            return Outcome::Failed { message };
        }
        // The canvas and the tool's context follow the new page now, so the placement's tool is not escaped by it.
        self.sync_views();
        self.sync_tool_context();
        let letter = letter.unwrap_or_else(|| "U".to_owned());
        for (action, params) in [
            ("place.pick", json!({"symbol": key, "letter": letter})),
            ("cursor.set", json!({"x_mm": 40, "y_mm": 40})),
            ("canvas.activate", json!({})),
        ] {
            let out = self.dispatch(action, &params);
            if out != Outcome::Done {
                self.take_back_new_page(back.as_deref());
                return out;
            }
        }
        let placed = self.tools.last_primary.clone();
        let _ = self.dispatch("ui.dismiss", &json!({}));
        if let Some(id) = placed {
            return self.dispatch("select.set", &json!({ "ids": [id] }));
        }
        Outcome::Done
    }

    /// Takes back the page that Place on a new page with its grid made when the placement on it is refused (a
    /// terminal in a project without a strip): the page was made for the symbol alone, and an empty page left behind
    /// is a surprise. The page that was on show before, `back`, shows again.
    pub(crate) fn take_back_new_page(&mut self, back: Option<&str>) {
        let _ = self.dispatch("ui.dismiss", &json!({}));
        let _ = self.dispatch("edit.undo", &json!({}));
        if let Some(folio) = back {
            let _ = self.dispatch("page.goto", &json!({ "folio": folio }));
        }
    }

    /// `picker.choose`: the current row.
    pub(crate) fn act_picker_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(picker) = &self.tools.picker else {
            return Outcome::refused(key!("reason-no-picker"));
        };
        let Some(&row) = picker.rows.get(picker.row) else {
            return Outcome::refused(key!("reason-no-picker-row"));
        };
        if let Some(reason_key) = picker.disabled(row) {
            // SM25b m3: the grids' reason names both grids.
            if reason_key == key!("reason-symbol-grid") {
                return Outcome::Failed {
                    message: picker.disabled_words(row).unwrap_or_default(),
                };
            }
            return Outcome::Refused {
                reason: reason_key.into(),
            };
        }
        let (mode, cat, pending) = (picker.mode, Arc::clone(&picker.catalogue), picker.pending.clone());
        // A valve terminal's position being filled: the valve chosen goes in that cell.
        if let (Some(fill), PickerRow::Part(pi) | PickerRow::PartFn(pi, _)) = (picker.cell, row) {
            return self.fill_cell(fill, cat.parts[pi].copy);
        }
        // Its Blanking plate: the position blanked, as the cell's menu does.
        if let (Some(fill), PickerRow::Blank) = (picker.cell, row) {
            self.tools.picker = None;
            let params = json!({ "cmd": "valve.set_occupancy", "params": { "module": fill.module.to_string(),
                "position": fill.position, "occupancy": { "blank": {} } } });
            return self.dispatch("command.run", &params);
        }
        // A part without a function template asks for its symbol, then places with it.
        if let (PickerRow::Symbol(i), Some(pi)) = (row, picker.part) {
            return self.arm(cat, Pick::PartSymbol(pi, i), p);
        }
        match (mode, row) {
            (PickerMode::Exchange, PickerRow::Symbol(i)) => self.arm_exchange(cat, i),
            // A symbol with a tool of its own arms that tool: a wire marker is drawn across its wires, an
            // interruption point is placed with its name and direction.
            (_, PickerRow::Symbol(i)) if cat.symbols[i].placed_by == PlacedBy::MarkerTool => {
                self.tools.picker = None;
                self.dispatch("tool.marker", &json!({}))
            }
            (_, PickerRow::Symbol(i)) if cat.symbols[i].placed_by == PlacedBy::InterruptionTool => {
                self.tools.picker = None;
                let incoming =
                    cat.symbols[i].key.split('@').next() == Some(ew_api::output::starter::GEN_IP_TARGET);
                let direction = if incoming { "target" } else { "source" };
                match self.dispatch("tool.ip", &json!({})) {
                    Outcome::Done => self.dispatch("tool.ip_direction", &json!({ "direction": direction })),
                    other => other,
                }
            }
            // A part with a function template places its main function, as its first function row did.
            (_, PickerRow::Part(pi)) => match self.tools.picker.as_ref().and_then(|p| p.part_main(pi)) {
                Some(k) => self.arm(cat, Pick::PartFn(pi, k), p),
                None => {
                    if let Some(picker) = &mut self.tools.picker {
                        picker.choose_part(pi);
                    }
                    Outcome::Done
                }
            },
            (_, PickerRow::Symbol(i)) => self.arm(cat, Pick::Symbol(i), p),
            (_, PickerRow::PartFn(a, b)) => self.arm(cat, Pick::PartFn(a, b), p),
            // The strip chooser: the pending terminal placement with the chosen strip.
            (_, PickerRow::Strip(i)) => {
                let Some(pending) = pending else {
                    return Outcome::refused(key!("reason-no-picker-row"));
                };
                let Some(symbol) = cat.symbols.iter().position(|r| r.key == pending.symbol) else {
                    return Outcome::refused(key!("reason-unknown-symbol"));
                };
                // The parameters of this choice (`designation`, `letter`, `strip`) win over those of the choice that
                // opened the chooser; the strip is the current row's unless this choice names one.
                let mut params = pending.params.clone();
                // A designation of this choice replaces the earlier one under either of its names.
                if TERMINAL_DESIGNATION.iter().any(|k| p.contains_key(*k)) {
                    for k in TERMINAL_DESIGNATION {
                        params.remove(k);
                    }
                }
                params.extend(p.iter().map(|(k, v)| (k.clone(), v.clone())));
                if params.get("strip").is_none_or(Value::is_null) {
                    params.insert("strip".into(), Value::from(cat.strips[i].id.to_string()));
                }
                // A part-first terminal picks its part again (G-PP finding 2).
                let pick = pending
                    .part
                    .as_ref()
                    .and_then(|(copy, template)| {
                        let pi = cat.parts.iter().position(|r| r.copy.to_string() == *copy)?;
                        let fi = cat.parts[pi]
                            .functions
                            .iter()
                            .position(|f| f.index == *template)?;
                        Some(Pick::PartFn(pi, fi))
                    })
                    .unwrap_or(Pick::Symbol(symbol));
                self.arm(cat, pick, &params)
            }
            // Only a position being filled lists it (above).
            (_, PickerRow::Blank) => Outcome::refused(key!("reason-no-picker-row")),
        }
    }

    /// `place.pick` (headless): one of `symbol` (key or copy ID), `part` (part number or copy ID) with
    /// `template` (0-based; a part without a function template with `symbol` instead), `function` (ID, with `symbol`); optional `letter`; for a terminal `strip` (tag or ID)
    /// and `designation`. `device=<tag>` (with `symbol`) joins an existing device, and `function` also
    /// takes `<tag>:<slot>` or `<tag>` (with `symbol`): see [`crate::shell::join`].
    pub(crate) fn act_place_pick(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let unknown = Outcome::refused(key!("reason-unknown-symbol"));
        let symbol = match string_param(p, "symbol") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        // A library symbol's address names it before and after its first placement, and so does its key: an
        // installed library's symbol not yet embedded is listed by its address.
        let find = |s: &str| {
            let held = ew_api::output::address_copy(s);
            let by_key = |s: &str| {
                cat.symbols
                    .iter()
                    .position(|r| r.key == s || r.copy.is_some_and(|c| c.to_string() == s || held == Some(c)))
            };
            by_key(s).or_else(|| {
                let address = self.project.as_ref()?.session.library_symbol_address(s)?;
                cat.symbols.iter().position(|r| r.key == address)
            })
        };
        // A symbol named but not offered on this page is unknown, not missing.
        let named = |s: Option<&str>| match s {
            None => Err(failed(key!("error-missing-param"), &["symbol"])),
            Some(s) => find(s).ok_or_else(|| Outcome::refused(key!("reason-unknown-symbol"))),
        };
        let device = match string_param(p, "device") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        let port = match string_param(p, "port") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        // A port of a device (its tag or ID) on a bus-topology page: the box of that interface with its sockets (doc
        // 10 §2.7); `port` names the interface as its row does ("X1", "X1 P1R").
        if let (Some(device), Some(port)) = (&device, &port)
            && self.page_rows().get(self.page_index()).map(|r| &r.kind)
                == Some(&ew_api::PageType::BusTopology)
        {
            let id = match self.join_device_ref(device) {
                Ok(id) => id,
                Err(reason) => return failed(key!("error-join-device"), &[device, &reason]),
            };
            let folio = self.current_folio().unwrap_or_default().to_owned();
            let project = or_return!(crate::shell::gated(&self.project));
            let interfaces: Vec<ew_api::output::CatalogueInterface> = project
                .session
                .undrawn_interfaces(id, &folio)
                .into_iter()
                .filter(|i| i.label == *port)
                .collect();
            if interfaces.is_empty() {
                return Outcome::refused(key!("reason-port-drawn"));
            }
            let tag = project.session.device_tag(id).unwrap_or_default();
            let label = format!("{tag} {port}");
            let target = json!({"device": {"device": id.to_string()}});
            return self.arm_ports(&cat, target, label, &interfaces, None);
        }
        // A device by its printed tag (or its ID) joins it.
        if let Some(device) = device {
            let i = match named(symbol.as_deref()) {
                Ok(i) => i,
                Err(e) => return e,
            };
            return match self.pick_join_device(&cat, i, &device) {
                Ok((cat, i, j)) => {
                    self.tools.picker = None;
                    self.arm(cat, Pick::Join(j, i), p)
                }
                Err(e) => e,
            };
        }
        let pick = match (string_param(p, "part"), string_param(p, "function")) {
            (Err(e), _) | (_, Err(e)) => return e,
            // A function by its device's printed tag (`-101K1`, `-101K1:13-14`); an ID keeps the path below.
            (_, Ok(Some(function))) if function.trim().parse::<ew_api::output::Uuid>().is_err() => {
                let i = match named(symbol.as_deref()) {
                    Ok(i) => i,
                    Err(e) => return e,
                };
                return match self.pick_function_tag(&cat, i, function) {
                    Ok((cat, i, f)) => {
                        self.tools.picker = None;
                        self.arm(cat, Pick::Function(f, i), p)
                    }
                    Err(e) => e,
                };
            }
            (Ok(Some(part)), _) => {
                let template = p.get("template").and_then(Value::as_u64).unwrap_or(0);
                let Some(pi) = cat
                    .parts
                    .iter()
                    .position(|r| r.part_number == part || r.copy.to_string() == part)
                else {
                    return unknown;
                };
                // A part without a function template is placed with the symbol named beside it.
                if cat.parts[pi].functions.is_empty() {
                    let i = match named(symbol.as_deref()) {
                        Ok(i) => i,
                        Err(e) => return e,
                    };
                    Pick::PartSymbol(pi, i)
                } else {
                    let Some(fi) = cat.parts[pi]
                        .functions
                        .iter()
                        .position(|f| u64::from(f.index) == template)
                    else {
                        return unknown;
                    };
                    Pick::PartFn(pi, fi)
                }
            }
            (_, Ok(Some(function))) => {
                // An unplaced terminal row names its terminal device, which places the device's main
                // function.
                let main = Uuid::parse_str(function)
                    .ok()
                    .and_then(|u| self.project.as_ref()?.session.main_function(u))
                    .map(|(f, key)| (f.to_string(), key));
                let function = main.as_ref().map_or(function, |(f, _)| f.as_str());
                // Without `symbol`, the drop of an unplaced function row takes the first
                // catalogue symbol that draws the function's FnDef (the Devices navigator's function row names it).
                let symbol = symbol.or_else(|| {
                    let fndef = match &main {
                        Some((_, key)) => key.clone(),
                        None => self.function_fndef_key(function)?,
                    };
                    cat.symbols
                        .iter()
                        .find(|s| s.fndef_key == fndef && s.allowed && !s.black_box)
                        // A black-box function (a station's supply pins) is drawn by the black-box frame.
                        .or_else(|| {
                            cat.symbols
                                .iter()
                                .find(|s| s.fndef_key == fndef && s.allowed && s.black_box)
                        })
                        // A black-box function on another FnDef (a multipin terminal's plug) by the frame too.
                        .or_else(|| {
                            let black_box = self.project.as_ref().is_some_and(|p| {
                                function.parse().is_ok_and(|f| p.session.is_black_box_function(f))
                            });
                            cat.symbols.iter().find(|s| black_box && s.allowed && s.black_box)
                        })
                        .map(|s| s.key.clone())
                });
                let i = match named(symbol.as_deref()) {
                    Ok(i) => i,
                    Err(e) => return e,
                };
                Pick::Function(function.to_owned(), i)
            }
            _ => match named(symbol.as_deref()) {
                Ok(i) => Pick::Symbol(i),
                Err(e) => return e,
            },
        };
        self.tools.picker = None;
        self.arm(cat, pick, p)
    }

    /// The transform keys: `Key(k)` to the active tool; with Select, V on several representations is
    /// refused `reason-one-rep` and Shift+V `reason-place-only`; with Place, Shift+V is the host's (the next
    /// catalogue symbol drawing the same function definition).
    pub(crate) fn act_tool_key(&mut self, key: ToolKey) -> Outcome {
        if matches!(key, ToolKey::NextProposal | ToolKey::ConnectOnMove) {
            return self.send_tool(&CanvasEvent::Key(key));
        }
        if self.tools.tool.name() == "select" {
            let count_reps = |s: &Self| {
                s.canvas().map_or(0, |c| {
                    selection_kinds(&s.tools.selection, &c.page().tables)
                        .iter()
                        .filter(|(_, k)| *k == EntityKind::Rep)
                        .count()
                })
            };
            let mut reps = count_reps(self);
            // A device or a function selected by reference (`select.set` with a tag, `-X1011:2`) is transformed as
            // the representations the page shown draws of it, as Delete and Cut take it (items 9 and 10):
            // the selection becomes those representations, but only once the key is sent: a refused key
            // leaves the selection as it was.
            let mut replace_with = None;
            if reps == 0 {
                let drawn = self.drawn_reps_of_selection();
                if !drawn.is_empty() {
                    let mut probe = crate::tools::Selection::default();
                    probe.apply(&crate::tools::SelectionChange::Replace(drawn.clone()));
                    reps = probe.items.len();
                    replace_with = Some(drawn);
                }
            }
            match key {
                ToolKey::NextRepType => {
                    return Outcome::refused(key!("reason-place-only"));
                }
                ToolKey::NextVariant if reps > 1 => {
                    return Outcome::refused(key!("reason-one-rep"));
                }
                _ if reps == 0 => {
                    return Outcome::refused(key!("reason-nothing-selected"));
                }
                _ => {}
            }
            if let Some(drawn) = replace_with {
                self.tools
                    .selection
                    .apply(&crate::tools::SelectionChange::Replace(drawn));
            }
        }
        if key == ToolKey::NextRepType && self.tools.tool.name() == "place" {
            return self.next_rep_type();
        }
        self.send_tool(&CanvasEvent::Key(key))
    }

    /// Shift+V with Place: the next catalogue symbol (key order, wrapping) that draws the same function
    /// definition and the page allows; an embedded one only (a starter symbol is chosen through the picker).
    pub(super) fn next_rep_type(&mut self) -> Outcome {
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let current = self
            .with_tool(|t, _| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<PlaceTool>())
                    .map(|p| p.shape().key.clone())
            })
            .flatten();
        let Some(current) = current else {
            return Outcome::Done;
        };
        let Some(at) = cat.symbols.iter().position(|s| s.key == current) else {
            return Outcome::refused(key!("reason-no-next-rep-type"));
        };
        let fndef = cat.symbols[at].fndef_key.clone();
        let n = cat.symbols.len();
        let next = (1..n)
            .map(|k| &cat.symbols[(at + k) % n])
            .find(|s| s.fndef_key == fndef && s.allowed && s.copy.is_some())
            .cloned();
        let Some(row) = next else {
            return Outcome::refused(key!("reason-no-next-rep-type"));
        };
        let Some(shape) = self.row_shape(&row) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let out = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<PlaceTool>())
                    .map(|p| p.set_symbol(shape, cx))
            })
            .flatten();
        if let Some(ToolOutput::Transient(o)) = out {
            self.tools.overlay = o;
        }
        Outcome::Done
    }

    /// `tool.move` (M): the keyboard move of the selection's movable entities (representations, page items
    /// and structure boxes); the grid cursor goes to the primary entity's anchor when it is not shown, else the first
    /// movable entity's (a representation's insertion point, an item's top-left bounds corner, a structure box's
    /// first outline vertex).
    pub(crate) fn act_tool_move(&mut self) -> Outcome {
        if self.tools.tool.name() != "select"
            && let Some(select) = crate::tools::tool_named("select")
        {
            self.activate_tool(select);
        }
        let Some(c) = self.canvas() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let tables = Arc::clone(&c.page().tables);
        let mut movable = transform::selected_movable(&self.tools.selection, c.page());
        // A device or a function selected by reference (`select.set` with its tag) is moved as the
        // representations the page shown draws of it, as R and X take it: the selection becomes those symbols, but
        // only once the move starts: a refused move puts the selection back as it was.
        let mut replaced = None;
        if movable.is_empty() {
            let drawn = self.drawn_reps_of_selection();
            if !drawn.is_empty() {
                let before = self.tools.selection.clone();
                let changed = self
                    .tools
                    .selection
                    .apply(&crate::tools::SelectionChange::Replace(drawn.clone()));
                replaced = Some((before, changed));
                movable = drawn;
            }
        }
        let Some(c) = self.canvas() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        if movable.is_empty() {
            return Outcome::refused(key!("reason-nothing-to-move"));
        }
        if c.cursor().is_none() {
            let primary = self
                .tools
                .selection
                .primary
                .filter(|e| movable.iter().any(|m| m == e))
                .unwrap_or(movable[0]);
            let anchor = if let Some(rep) = tables.reps.iter().find(|r| r.id == primary.0) {
                // A symbol fitted to the grid (its position between grid points) is carried by a pin on the grid, so
                // the first arrow key moves it a whole step.
                let g = crate::tools::wire::PageGridMap::new(tables.grid);
                let on = |p: (i64, i64)| g.coord(g.index(p.0)) == p.0 && g.coord(g.index(p.1)) == p.1;
                Some(if on(rep.pos) {
                    rep.pos
                } else {
                    tables
                        .cps
                        .iter()
                        .filter(|c| c.rep == rep.id && on(c.at))
                        .min_by_key(|c| c.idx)
                        .map_or(rep.pos, |c| c.at)
                })
            } else if let Some(item) = tables.items.iter().find(|i| i.id == primary.0) {
                Some((item.bounds[0], item.bounds[1]))
            } else {
                tables
                    .structure_boxes
                    .iter()
                    .find(|b| b.id == primary.0)
                    .and_then(|b| b.outline.first().copied())
            };
            if let Some(anchor) = anchor {
                let at = c.page().snapper.page_to_sheet(anchor);
                let g = self.area.as_ref().map_or(0, |a| a.focused);
                if let Some(c) = self.canvas_mut(g) {
                    c.put_cursor(Some(at));
                }
                self.cursor_moved(g);
            }
        }
        let out = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<SelectTool>())
                    .and_then(|s| s.start_move(cx))
            })
            .flatten();
        match out {
            Some(ToolOutput::Transient(o)) => {
                if replaced.is_some_and(|(_, changed)| changed) {
                    self.announce_selection();
                }
                self.tools.overlay = o;
                self.announce_tool();
                Outcome::Done
            }
            _ => {
                if let Some((before, _)) = replaced {
                    self.tools.selection = before;
                }
                Outcome::refused(key!("reason-no-cursor"))
            }
        }
    }

    /// The representations of the page shown that the selection names by reference: for a selected device or function
    /// the page tables do not list, the representations drawing it; empty when nothing of the selection is drawn here.
    pub(super) fn drawn_reps_of_selection(&self) -> Vec<EntityRef> {
        let Some(page) = self.canvas().map(|c| Arc::clone(c.page())) else {
            return Vec::new();
        };
        self.copy_targets()
            .into_iter()
            .filter(|e| crate::tools::entity_kind(&page.tables, *e) == Some(EntityKind::Rep))
            .collect()
    }

    /// `rep.exchange {symbol?}`: on exactly one selected representation that draws no interruption point;
    /// without `symbol` the picker opens in exchange mode, with one the tool is armed at once.
    pub(crate) fn act_rep_exchange(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(c) = self.canvas() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let tables = Arc::clone(&c.page().tables);
        let reps: Vec<EntityRef> = selection_kinds(&self.tools.selection, &tables)
            .into_iter()
            .filter(|(_, k)| *k == EntityKind::Rep)
            .map(|(e, _)| e)
            .collect();
        let rep = match reps.as_slice() {
            [] => {
                return Outcome::refused(key!("reason-nothing-selected"));
            }
            [one] => *one,
            _ => {
                return Outcome::refused(key!("reason-one-rep"));
            }
        };
        if tables.reps.iter().any(|r| r.id == rep.0 && r.ip.is_some()) {
            return Outcome::refused(key!("reason-ip-exchange"));
        }
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        self.tools.exchange_rep = Some(rep);
        match string_param(p, "symbol") {
            Err(e) => e,
            Ok(Some(s)) => {
                let Some(i) = cat
                    .symbols
                    .iter()
                    .position(|r| r.allowed && (r.key == s || r.copy.is_some_and(|c| c.to_string() == s)))
                else {
                    return Outcome::refused(key!("reason-unknown-symbol"));
                };
                self.arm_exchange(cat, i)
            }
            Ok(None) => {
                let folio = self.current_folio().unwrap_or_default().to_owned();
                self.tools.picker = Some(Picker::new(PickerMode::Exchange, &folio, cat));
                Outcome::Done
            }
        }
    }

    /// A command of the host on the active selection (Delete): the tool's commit path with the after-commit duties.
    pub(in crate::shell) fn commit_from_host(&mut self, cmd: &dyn ew_api::DynCommand) -> Outcome {
        self.tools.before_commit = self.canvas().map(|c| Arc::clone(&c.page().tables));
        let out = self.run_commit(cmd);
        self.tools.overlay = crate::tools::Overlay::default();
        self.tools.preview = None;
        out
    }

    /// The after-commit duties, at the end of a dispatch: a committed record the page tables no longer
    /// hold leaves the selection; the place tool's shape is read again for a new revision.
    pub(in crate::shell) fn after_commit(&mut self) {
        // A box armed with records this dispatch took away (a Delete in a navigator, as an Undo would) ends.
        self.end_tool_without_records();
        self.after_layout_commit();
        self.after_paste();
        self.after_place();
        self.refresh_place_shape();
        self.refresh_strip_tag();
        self.place_terminal_row();
        // A sticky join target fills the next slot, then makes new auxiliary functions.
        self.refresh_join_target();
        self.cancel_stale_move();
        let Some(before) = self.tools.before_commit.take() else {
            self.tools.result_note = None;
            return;
        };
        let Some(now) = self.canvas().map(|c| Arc::clone(&c.page().tables)) else {
            self.tools.result_note = None;
            return;
        };
        self.say_result(&before, &now);
        self.say_card_waits(&before, &now);
        // A bus line just drawn without a cable asks for one.
        let drawn = now
            .connections
            .iter()
            .filter(|c| c.kind == "bus" && !before.connections.iter().any(|b| b.id == c.id))
            .map(|c| c.id)
            .find(|id| {
                self.project
                    .as_ref()
                    .is_some_and(|p| p.session.bus_cable_form(&[*id]).cable.is_none())
            });
        // A bus line drawn without a cable says so, with Choose cable… opening its Cable form (no cable was asked
        // for, and the person looked for it in Properties). A toast, not a form, so drawing several lines goes on.
        if let Some(line) = drawn {
            let choose = self.session_offer(NoteAction::ChooseCable, vec![line]);
            let text = strings::text(key!("toast-bus-line-no-cable")).to_owned();
            self.notes.push(Note::new("bus_line", text).offering([choose]));
        }
        // Supply marks placed: their Potential panel (H9).
        self.after_supply_marks(&before, &now);
        // The IP tool's ghost proposes its next name on the page as the commit left it (its kept direction may meet
        // the name it just placed).
        self.after_ip_commit();
        let kept: Vec<EntityRef> = self
            .tools
            .selection
            .items
            .iter()
            .copied()
            .filter(|e| entity_kind(&before, *e).is_none() || entity_kind(&now, *e).is_some())
            .collect();
        if kept != self.tools.selection.items && self.tools.selection.apply(&SelectionChange::Replace(kept)) {
            self.announce_selection();
        }
    }

    /// A pending drag or keyboard move whose selection changed or whose representations are gone is escaped at the end
    /// of the dispatch that changed them: its ghost, readout and preview go.
    pub(super) fn cancel_stale_move(&mut self) {
        let stale = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<SelectTool>())
                    .is_some_and(|s| s.pending_move_stale(cx))
            })
            .unwrap_or(false);
        if stale {
            // The tool's answer is `Cancel`; the host's part of it follows.
            let _ = self.with_tool(|t, cx| t.on_event(&CanvasEvent::Escape, cx));
            self.tools.overlay = crate::tools::Overlay::default();
            self.tools.preview = None;
            self.announce_tool();
        }
    }

    /// The place tool's shape after a new revision: read once again through the per-revision cache, so
    /// a library update reaches the floating symbol; nothing else of the tool changes.
    pub(super) fn refresh_place_shape(&mut self) {
        if self.tools.tool.name() != "place" {
            return;
        }
        let Some(rev) = self.project_epoch().zip(self.revision()) else {
            return;
        };
        if self.tools.shapes.as_ref().is_some_and(|(r, _)| *r == rev) {
            return;
        }
        let copy = self
            .with_tool(|t, _| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<PlaceTool>())
                    .map(|p| p.shape().copy)
            })
            .flatten();
        let Some(copy) = copy else { return };
        if let Some(shape) = self.cached_shape(copy.to_string(), |s| s.symbol_shape(copy)) {
            let _ = self.with_tool(|t, _| {
                if let Some(p) = t.as_any_mut().and_then(|a| a.downcast_mut::<PlaceTool>()) {
                    p.replace_shape(shape);
                }
            });
        }
    }

    /// The live-region text of the place tool after a change: `Place <key>: <target>, variant <letter>, at
    /// <cell>`.
    pub(in crate::shell) fn announce_place(&mut self) {
        let Some(Ghost::Place {
            symbol,
            variant,
            at_nm,
            ..
        }) = self.tools.tool.ghost()
        else {
            return;
        };
        let Some(c) = self.canvas() else { return };
        let v = variant.to_string();
        // The label without its variant part (`Variant A · `): the announcement names the variant itself.
        let prefix = strings::format(key!("place-variant"), &[&v, ""]);
        let label = self
            .tools
            .overlay
            .labels()
            .next()
            .map(|l| l.strip_prefix(prefix.as_str()).unwrap_or(l).to_owned())
            .unwrap_or_default();
        let cell = at_nm
            .map(|p| c.page().snapper.page_to_sheet(p))
            .and_then(|s| c.page().cell_text(s));
        self.tools.announcement = Some(match cell {
            Some(cell) => strings::format(key!("announce-place"), &[&symbol, &label, &v, &cell]),
            None => strings::format(key!("announce-place-no-cell"), &[&symbol, &label, &v]),
        });
    }
}
