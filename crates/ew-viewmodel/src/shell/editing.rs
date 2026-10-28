// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's duties of WP-S2.04 part 1 (§6.4, §7): the picker popup and its choices, the host commits (the
//! starter embedding and `strip.new`), arming the Place and change-symbol tools, the transform keys, the keyboard
//! move, Delete, and the selection after a commit.
//!
//! **Reads.** The place catalogue is fetched once per (session, revision, folio) while the picker or `place.pick`
//! needs it, a symbol shape once per copy per revision (§6.2); pointer and cursor moves, the picker's filter and its
//! row keys make no `Session` call (§12 item 4).
//!
//! **Host commits** (§6.4 item 2) go through the tool's commit path: `Session::execute_with` in the window,
//! `Session::run_script` headless. A failed host commit leaves the tool unarmed and shows the message in the banner.

use std::sync::Arc;

use ew_api::output::ew_dl::EntityRef;
use ew_api::output::{CatalogueSymbol, PlaceCatalogue, SymbolShape};
use ew_api::{AnyCommand, Session};
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, string_param};
use crate::strings;
use crate::tools::{
    BlackBoxTool, BoxTarget, CanvasEvent, EntityKind, ExchangeTool, Ghost, PendingTerminal, Picker,
    PickerMode, PickerRow, PlaceTarget, PlaceTool, SelectTool, SelectionChange, Tool, ToolCx, ToolKey,
    ToolOutput, entity_kind, selection_kinds, transform,
};

/// What a choice of the picker or a `place.pick` arms.
enum Pick {
    /// A symbol row: its index in the catalogue.
    Symbol(usize),
    /// A template function: the part's and the function's indices.
    PartFn(usize, usize),
    /// Another representation of a function, with a symbol row.
    Function(String, usize),
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The picker popup, while open (WP-S2.04 §7.1).
    pub fn picker(&self) -> Option<&Picker> {
        self.tools.picker.as_ref()
    }

    /// Whether the active place tool waits for the next terminal designation (§7.2).
    pub(super) fn place_needs_designation(&mut self) -> bool {
        self.tools
            .tool
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<PlaceTool>())
            .is_some_and(|p| p.needs_designation())
    }

    /// What the active tool floats or drags (WP-S2.04 §7.7).
    pub fn tool_ghost(&self) -> Option<Ghost> {
        self.tools.tool.ghost()
    }

    // -----------------------------------------------------------------------------------------------------------
    // Reads

    /// The (session, revision, folio) of the focused canvas.
    fn canvas_key(&self) -> Option<(u64, u64, String)> {
        let c = self.canvas()?;
        Some((self.project_epoch()?, self.revision()?, c.page().folio.clone()))
    }

    /// The place catalogue of the focused page, through the cache.
    pub(super) fn catalogue(&mut self) -> Option<Arc<PlaceCatalogue>> {
        let key = self.canvas_key()?;
        if let Some((k, cat)) = &self.tools.catalogue
            && *k == key
        {
            return Some(Arc::clone(cat));
        }
        let cat = self.project.as_ref()?.session.place_catalogue(&key.2)?;
        self.count(|c| c.place_catalogue += 1);
        let cat = Arc::new(cat);
        self.tools.catalogue = Some((key, Arc::clone(&cat)));
        Some(cat)
    }

    /// The shape of symbol copy `key` (its ID text), through the per-revision cache; `fetch` reads it.
    pub(super) fn cached_shape(
        &mut self,
        key: String,
        fetch: impl FnOnce(&Session) -> Option<SymbolShape>,
    ) -> Option<Arc<SymbolShape>> {
        let rev = (self.project_epoch()?, self.revision()?);
        if self.tools.shapes.as_ref().is_none_or(|(r, _)| *r != rev) {
            self.tools.shapes = Some((rev, Default::default()));
        }
        if let Some((_, m)) = &self.tools.shapes
            && let Some(s) = m.get(&key)
        {
            return Some(Arc::clone(s));
        }
        let shape = Arc::new(fetch(&self.project.as_ref()?.session)?);
        self.count(|c| c.symbol_shape += 1);
        if let Some((_, m)) = &mut self.tools.shapes {
            m.insert(key, Arc::clone(&shape));
        }
        Some(shape)
    }

    /// The shape of an embedded catalogue symbol.
    pub(super) fn row_shape(&mut self, row: &CatalogueSymbol) -> Option<Arc<SymbolShape>> {
        let copy = row.copy?;
        self.cached_shape(copy.to_string(), |s| s.symbol_shape(copy))
    }

    /// The tool context of the focused canvas, for host duties that call a tool directly.
    pub(super) fn with_tool<R>(&mut self, f: impl FnOnce(&mut dyn Tool, &ToolCx<'_>) -> R) -> Option<R> {
        let c = self.canvas()?;
        let (page, ppn, pane) = (Arc::clone(c.page()), c.points_per_nm(), c.pane());
        let host = &mut self.tools;
        let cx = ToolCx {
            page: &page,
            points_per_nm: ppn,
            pane,
            snapper: &page.snapper,
            selection: &host.selection,
        };
        Some(f(host.tool.as_mut(), &cx))
    }

    // -----------------------------------------------------------------------------------------------------------
    // Host commits

    /// A host commit (§6.4 item 2): the tool's commit path; a failure shows its message in the banner.
    pub(super) fn host_commit(&mut self, name: &str, params: Value) -> Outcome {
        let cmd = AnyCommand {
            name: name.into(),
            params,
        };
        let out = self.run_commit(&cmd);
        if let Outcome::Failed { message } = &out {
            self.banner = Some(strings::format("banner-host-commit", &[name, message]));
        }
        out
    }

    /// Embeds starter symbol `key` (one `library.embed`, one undo step) when the project has not; then the row of
    /// the refreshed catalogue.
    pub(super) fn embedded_row(&mut self, key: &str) -> Result<(Arc<PlaceCatalogue>, usize), Outcome> {
        let no_row = Outcome::Refused {
            reason_key: "reason-unknown-symbol",
        };
        let cat = self.catalogue().ok_or(Outcome::Refused {
            reason_key: "reason-no-catalogue",
        })?;
        let i = cat
            .symbols
            .iter()
            .position(|s| s.key == key)
            .ok_or(no_row.clone())?;
        if cat.symbols[i].copy.is_some() {
            return Ok((cat, i));
        }
        let params = self
            .project
            .as_ref()
            .and_then(|p| p.session.starter_embed(key))
            .ok_or(no_row.clone())?;
        match self.host_commit("library.embed", params) {
            Outcome::Done => {}
            other => return Err(other),
        }
        self.sync_views();
        let cat = self.catalogue().ok_or(no_row.clone())?;
        let i = cat
            .symbols
            .iter()
            .position(|s| s.key == key && s.copy.is_some())
            .ok_or(no_row)?;
        Ok((cat, i))
    }

    // -----------------------------------------------------------------------------------------------------------
    // Arming

    /// The catalogue strip named by `param`: its ID, or its tag as the page prints it (FT-421).
    fn catalogue_strip(cat: &PlaceCatalogue, param: &str) -> Option<usize> {
        cat.strips
            .iter()
            .position(|st| st.id.to_string() == param)
            .or_else(|| cat.strips.iter().position(|st| st.tag == param))
    }

    /// Arms the Place tool (or, in exchange mode, the change-symbol tool) for `pick` of `cat`; `p` holds the
    /// optional `letter`, `strip` and `designation`. A part's template function arms **part-first** (WP-S2.04 §7.1
    /// as amended by the cycle-27 follow-up): the starter symbol and every FnDef the part's template needs are
    /// embedded in one host commit before the tool arms; an FnDef neither the project nor the starter library holds
    /// fails the pick by name and arms nothing.
    fn arm(&mut self, cat: Arc<PlaceCatalogue>, pick: Pick, p: &Map<String, Value>) -> Outcome {
        let letter_param = match string_param(p, "letter") {
            Ok(v) => v.map(str::to_owned).or_else(|| self.tools.pick_letter.clone()),
            Err(e) => return e,
        };
        let mut box_cps: Option<Vec<(u16, String)>> = None;
        let mut part_copy = None;
        let (row_index, part) = match &pick {
            Pick::Symbol(i) | Pick::Function(_, i) => (*i, None),
            Pick::PartFn(pi, fi) => {
                let part = &cat.parts[*pi];
                let f = &part.functions[*fi];
                part_copy = Some(part.copy);
                if f.black_box {
                    box_cps = Some(f.cps.clone());
                }
                let Some(key) = &f.symbol else {
                    return Outcome::Refused {
                        reason_key: "reason-no-symbol-for-function",
                    };
                };
                let Some(i) = cat.symbols.iter().position(|s| s.key == *key) else {
                    return Outcome::Refused {
                        reason_key: "reason-no-symbol-for-function",
                    };
                };
                (
                    i,
                    Some((part.copy.to_string(), f.index, part.part_number.clone())),
                )
            }
        };
        let row = cat.symbols[row_index].clone();
        if !row.allowed {
            return Outcome::Refused {
                reason_key: "reason-rep-type-not-allowed",
            };
        }
        // A black box (a black-box symbol, or a template function that is one) arms the black box tool (WP-S2.04 §9.5);
        // `tool.black_box` arms nothing else.
        let is_box = row.black_box || box_cps.is_some();
        if self.tools.black_box_pick && !is_box {
            return Outcome::Refused {
                reason_key: "reason-not-black-box",
            };
        }
        if is_box && matches!(pick, Pick::Function(..)) {
            return Outcome::Refused {
                reason_key: "reason-black-box-tool",
            };
        }
        // A starter symbol is embedded first (its own undo step). For a part's template function the arm is
        // part-first (WP-S2.04 §7.1 as amended by the cycle-27 follow-up): the symbol and every FnDef the part's
        // template needs go in one commit, so that the functions of the `from_part` target all have their
        // definition.
        let (cat, row_index) = if let Some(part_copy) = part_copy {
            match self
                .project
                .as_ref()
                .map(|p| p.session.starter_embed_part(part_copy, &row.key))
            {
                Some(Err(keys)) => return failed("error-part-fndef-missing", &[&keys.join(", ")]),
                Some(Ok(Some(params))) => match self.host_commit("library.embed", params) {
                    Outcome::Done => self.sync_views(),
                    other => return other,
                },
                _ => {}
            }
            match self.embedded_row(&row.key) {
                Ok(v) => v,
                Err(e) => return e,
            }
        } else {
            match self.embedded_row(&row.key) {
                Ok(v) => v,
                Err(e) => return e,
            }
        };
        let row = cat.symbols[row_index].clone();
        if is_box {
            let Some(copy) = row.copy else {
                return Outcome::Refused {
                    reason_key: "reason-unknown-symbol",
                };
            };
            let target = match part {
                Some((part, template, number)) => {
                    let Some(letter) = letter_param.or(row.letter.clone()) else {
                        return Outcome::Refused {
                            reason_key: "reason-letter-needed",
                        };
                    };
                    BoxTarget::Part {
                        part,
                        template,
                        letter,
                        number,
                        cps: box_cps.unwrap_or_default(),
                    }
                }
                None => {
                    let Some(letter) = letter_param.or(row.letter.clone()) else {
                        return Outcome::Refused {
                            reason_key: "reason-letter-needed",
                        };
                    };
                    BoxTarget::NewDevice { letter }
                }
            };
            self.tools.picker = None;
            self.activate_tool(Box::new(BlackBoxTool::new(copy.to_string(), target)));
            return Outcome::Done;
        }
        let Some(shape) = self.row_shape(&row) else {
            return Outcome::Refused {
                reason_key: "reason-unknown-symbol",
            };
        };
        let target = match (pick, part) {
            (Pick::Function(function, _), _) => PlaceTarget::Function { function },
            (_, Some((part, template, number))) => {
                let Some(letter) = letter_param.or(row.letter.clone()) else {
                    return Outcome::Refused {
                        reason_key: "reason-letter-needed",
                    };
                };
                PlaceTarget::Part {
                    part,
                    template,
                    letter,
                    number,
                }
            }
            // A terminal symbol, or any symbol given a strip or a designation (a PE terminal's category is
            // `TerminalEarth`, not `Terminal`).
            _ if row.terminal || p.contains_key("strip") || p.contains_key("designation") => {
                let strip = match string_param(p, "strip") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if cat.strips.is_empty() {
                    return Outcome::Refused {
                        reason_key: "reason-no-strip",
                    };
                }
                // Without a strip the picker asks for one (FT-421): its strip chooser.
                let Some(strip) = strip else {
                    let folio = self.current_folio().unwrap_or_default().to_owned();
                    let pending = PendingTerminal {
                        symbol: row.key.clone(),
                        params: p.clone(),
                    };
                    self.palette = None;
                    self.tools.picker = Some(Picker::strips(&folio, Arc::clone(&cat), pending));
                    return Outcome::Done;
                };
                let Some(i) = Self::catalogue_strip(&cat, strip) else {
                    return Outcome::Refused {
                        reason_key: "reason-no-strip",
                    };
                };
                let st = &cat.strips[i];
                let designation = match string_param(p, "designation") {
                    Ok(Some(d)) => d.to_owned(),
                    Ok(None) => st.next_designation.clone(),
                    Err(e) => return e,
                };
                PlaceTarget::Terminal {
                    strip: st.id.to_string(),
                    strip_tag: st.tag.clone(),
                    designation,
                }
            }
            _ => {
                let Some(letter) = letter_param.or(row.letter.clone()) else {
                    return Outcome::Refused {
                        reason_key: "reason-letter-needed",
                    };
                };
                PlaceTarget::NewDevice { letter }
            }
        };
        self.tools.picker = None;
        let autoconnect = self.page_autoconnects();
        self.activate_tool(Box::new(
            PlaceTool::new(shape, target).with_autoconnect(autoconnect),
        ));
        Outcome::Done
    }

    /// Arms the change-symbol tool for the representation the picker was opened for and symbol row `i`.
    fn arm_exchange(&mut self, cat: Arc<PlaceCatalogue>, i: usize) -> Outcome {
        let Some(rep) = self.tools.exchange_rep else {
            return Outcome::Refused {
                reason_key: "reason-one-rep",
            };
        };
        let key = cat.symbols[i].key.clone();
        let (cat, i) = match self.embedded_row(&key) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let row = cat.symbols[i].clone();
        let Some(shape) = self.row_shape(&row) else {
            return Outcome::Refused {
                reason_key: "reason-unknown-symbol",
            };
        };
        // The FnDef the representation's current symbol draws.
        let old = self
            .canvas()
            .and_then(|c| c.page().tables.reps.iter().find(|r| r.id == rep.0).cloned());
        let Some(old) = old else {
            return Outcome::Refused {
                reason_key: "reason-one-rep",
            };
        };
        // The shape of the representation's current symbol: the function CPs it draws and their designations.
        let old_symbol = old.symbol;
        let current = self.cached_shape(old_symbol.to_string(), |s| s.symbol_shape(old_symbol));
        self.tools.picker = None;
        self.tools.exchange_rep = None;
        let Some(tool) =
            self.with_tool(|_, cx| ExchangeTool::new(rep, Arc::clone(&shape), current.as_deref(), cx))
        else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let overlay = tool.overlay();
        self.activate_tool(Box::new(tool));
        self.tools.overlay = overlay;
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // Actions

    /// `tool.place` and `tool.place_part` (§7.6): opens the picker of the focused page.
    pub(super) fn act_open_picker(&mut self, mode: PickerMode) -> Outcome {
        let Some(cat) = self.catalogue() else {
            return Outcome::Refused {
                reason_key: "reason-no-catalogue",
            };
        };
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.palette = None;
        self.tools.junction_editor = None;
        self.tools.picker = Some(Picker::new(mode, &folio, cat));
        Outcome::Done
    }

    /// `picker.filter {text}`.
    pub(super) fn act_picker_filter(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match string_param(p, "text") {
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => return failed("error-missing-param", &["text"]),
            Err(e) => return e,
        };
        if let Some(picker) = &mut self.tools.picker {
            picker.set_filter(&text);
        }
        Outcome::Done
    }

    /// `picker.row {step | to}`: Up, Down (`step` −1, 1), Home (`to` 0), End (`to` −1 or a large number).
    pub(super) fn act_picker_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let step = p.get("step").and_then(Value::as_i64);
        let to = p.get("to").and_then(Value::as_i64);
        if step.is_none() && to.is_none() {
            return failed("error-missing-param", &["step"]);
        }
        if let Some(picker) = &mut self.tools.picker {
            let to = to.map(|t| if t < 0 { picker.rows.len() as i64 - 1 } else { t });
            picker.move_row(step, to);
        }
        Outcome::Done
    }

    /// `picker.choose`: the current row (§7.1).
    pub(super) fn act_picker_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(picker) = &self.tools.picker else {
            return Outcome::Refused {
                reason_key: "reason-no-picker",
            };
        };
        let Some(&row) = picker.rows.get(picker.row) else {
            return Outcome::Refused {
                reason_key: "reason-no-picker-row",
            };
        };
        if let Some(reason_key) = picker.disabled(row) {
            return Outcome::Refused { reason_key };
        }
        let (mode, cat, pending) = (picker.mode, Arc::clone(&picker.catalogue), picker.pending.clone());
        match (mode, row) {
            (PickerMode::Exchange, PickerRow::Symbol(i)) => self.arm_exchange(cat, i),
            (_, PickerRow::Symbol(i)) => self.arm(cat, Pick::Symbol(i), p),
            (_, PickerRow::PartFn(a, b)) => self.arm(cat, Pick::PartFn(a, b), p),
            // The strip chooser (FT-421): the pending terminal placement with the chosen strip.
            (_, PickerRow::Strip(i)) => {
                let Some(pending) = pending else {
                    return Outcome::Refused {
                        reason_key: "reason-no-picker-row",
                    };
                };
                let Some(symbol) = cat.symbols.iter().position(|r| r.key == pending.symbol) else {
                    return Outcome::Refused {
                        reason_key: "reason-unknown-symbol",
                    };
                };
                let mut params = pending.params.clone();
                params.insert("strip".into(), Value::from(cat.strips[i].id.to_string()));
                self.arm(cat, Pick::Symbol(symbol), &params)
            }
        }
    }

    /// `place.pick` (§7.6, headless): one of `symbol` (key or copy ID), `part` (part number or copy ID) with
    /// `template` (0-based), `function` (ID, with `symbol`); optional `letter`; for a terminal `strip` (tag or ID)
    /// and `designation`.
    pub(super) fn act_place_pick(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(cat) = self.catalogue() else {
            return Outcome::Refused {
                reason_key: "reason-no-catalogue",
            };
        };
        let unknown = Outcome::Refused {
            reason_key: "reason-unknown-symbol",
        };
        let symbol = match string_param(p, "symbol") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        let find = |s: &str| {
            cat.symbols
                .iter()
                .position(|r| r.key == s || r.copy.is_some_and(|c| c.to_string() == s))
        };
        let pick = match (string_param(p, "part"), string_param(p, "function")) {
            (Err(e), _) | (_, Err(e)) => return e,
            (Ok(Some(part)), _) => {
                let template = p.get("template").and_then(Value::as_u64).unwrap_or(0);
                let Some(pi) = cat
                    .parts
                    .iter()
                    .position(|r| r.part_number == part || r.copy.to_string() == part)
                else {
                    return unknown;
                };
                let Some(fi) = cat.parts[pi]
                    .functions
                    .iter()
                    .position(|f| u64::from(f.index) == template)
                else {
                    return unknown;
                };
                Pick::PartFn(pi, fi)
            }
            (_, Ok(Some(function))) => {
                // WP-S2.06 part 1 (§6.7): without `symbol`, the drop of an unplaced function row takes the first
                // catalogue symbol that draws the function's FnDef (the Devices navigator's function row names it).
                let symbol = symbol.or_else(|| {
                    let fndef = self.function_fndef_key(function)?;
                    cat.symbols
                        .iter()
                        .find(|s| s.fndef_key == fndef && s.allowed && !s.black_box)
                        .map(|s| s.key.clone())
                });
                let Some(i) = symbol.as_deref().and_then(find) else {
                    return failed("error-missing-param", &["symbol"]);
                };
                Pick::Function(function.to_owned(), i)
            }
            _ => {
                let Some(s) = symbol else {
                    return failed("error-missing-param", &["symbol"]);
                };
                let Some(i) = find(&s) else { return unknown };
                Pick::Symbol(i)
            }
        };
        self.tools.picker = None;
        self.arm(cat, pick, p)
    }

    /// `strip.new {letter}` (§7.6): a host commit of `device.create` with the strip aspect. While the picker's strip
    /// chooser is open it lists the new strip, which becomes its current row (FT-421).
    pub(super) fn act_strip_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let letter = match string_param(p, "letter") {
            Ok(v) => v.unwrap_or("X").to_owned(),
            Err(e) => return e,
        };
        let out = self.host_commit("device.create", json!({"letter": letter, "strip": {}}));
        if out != Outcome::Done {
            return out;
        }
        // FT-485 item 1: the strip draws nothing until a terminal is placed, so the toast names it and says where it
        // shows (a strip's tag counts once it has a terminal on a page: `-X??` until then).
        self.sync_views();
        let tag = self
            .catalogue()
            .and_then(|cat| {
                let new = self.tools.last_primary.clone()?;
                cat.strips
                    .iter()
                    .find(|s| s.id.to_string() == new)
                    .map(|s| s.tag.clone())
            })
            .unwrap_or_else(|| format!("-{letter}"));
        self.notify(strings::format("toast-strip-new", &[&tag]), true);
        let pending = self
            .tools
            .picker
            .as_ref()
            .filter(|p| p.mode == PickerMode::Strip)
            .and_then(|p| p.pending.clone());
        if let Some(pending) = pending {
            self.sync_views();
            if let Some(cat) = self.catalogue() {
                let folio = self.current_folio().unwrap_or_default().to_owned();
                let mut picker = Picker::strips(&folio, Arc::clone(&cat), pending);
                let new = self.tools.last_primary.clone();
                if let Some(i) = picker.rows.iter().position(
                    |r| matches!(r, PickerRow::Strip(k) if Some(cat.strips[*k].id.to_string()) == new),
                ) {
                    picker.row = i;
                }
                self.tools.picker = Some(picker);
            }
        }
        out
    }

    /// The transform keys (§7.4, §7.2): `Key(k)` to the active tool; with Select, V on several representations is
    /// refused `reason-one-rep` and Shift+V `reason-place-only`; with Place, Shift+V is the host's (the next
    /// catalogue symbol drawing the same function definition).
    pub(super) fn act_tool_key(&mut self, key: ToolKey) -> Outcome {
        if matches!(key, ToolKey::NextProposal | ToolKey::ConnectOnMove) {
            return self.send_tool(&CanvasEvent::Key(key));
        }
        if self.tools.tool.name() == "select" {
            let reps = self.canvas().map_or(0, |c| {
                selection_kinds(&self.tools.selection, &c.page().tables)
                    .iter()
                    .filter(|(_, k)| *k == EntityKind::Rep)
                    .count()
            });
            match key {
                ToolKey::NextRepType => {
                    return Outcome::Refused {
                        reason_key: "reason-place-only",
                    };
                }
                ToolKey::NextVariant if reps > 1 => {
                    return Outcome::Refused {
                        reason_key: "reason-one-rep",
                    };
                }
                _ if reps == 0 => {
                    return Outcome::Refused {
                        reason_key: "reason-nothing-selected",
                    };
                }
                _ => {}
            }
        }
        if key == ToolKey::NextRepType && self.tools.tool.name() == "place" {
            return self.next_rep_type();
        }
        self.send_tool(&CanvasEvent::Key(key))
    }

    /// Shift+V with Place (§7.2): the next catalogue symbol (key order, wrapping) that draws the same function
    /// definition and the page allows; an embedded one only (a starter symbol is chosen through the picker).
    fn next_rep_type(&mut self) -> Outcome {
        let Some(cat) = self.catalogue() else {
            return Outcome::Refused {
                reason_key: "reason-no-catalogue",
            };
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
            return Outcome::Refused {
                reason_key: "reason-no-next-rep-type",
            };
        };
        let fndef = cat.symbols[at].fndef_key.clone();
        let n = cat.symbols.len();
        let next = (1..n)
            .map(|k| &cat.symbols[(at + k) % n])
            .find(|s| s.fndef_key == fndef && s.allowed && s.copy.is_some())
            .cloned();
        let Some(row) = next else {
            return Outcome::Refused {
                reason_key: "reason-no-next-rep-type",
            };
        };
        let Some(shape) = self.row_shape(&row) else {
            return Outcome::Refused {
                reason_key: "reason-unknown-symbol",
            };
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

    /// `tool.move` (M, §7.3): the keyboard move of the selection's movable entities (representations, page items
    /// and structure boxes); the grid cursor goes to the primary entity's anchor when it is not shown, else the first
    /// movable entity's (a representation's insertion point, an item's top-left bounds corner, a structure box's
    /// first outline vertex).
    pub(super) fn act_tool_move(&mut self) -> Outcome {
        if self.tools.tool.name() != "select"
            && let Some(select) = crate::tools::tool_named("select")
        {
            self.activate_tool(select);
        }
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let tables = Arc::clone(&c.page().tables);
        let movable = transform::selected_movable(&self.tools.selection, c.page());
        if movable.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-nothing-to-move",
            };
        }
        if c.cursor().is_none() {
            let primary = self
                .tools
                .selection
                .primary
                .filter(|e| movable.iter().any(|m| m == e))
                .unwrap_or(movable[0]);
            let anchor = if let Some(rep) = tables.reps.iter().find(|r| r.id == primary.0) {
                Some(rep.pos)
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
                self.tools.overlay = o;
                self.announce_tool();
                Outcome::Done
            }
            _ => Outcome::Refused {
                reason_key: "reason-no-cursor",
            },
        }
    }

    /// `edit.delete_selection` (Delete, §7.4): `edit.delete` with every entity of the selection the page tables
    /// resolve.
    pub(super) fn act_delete_selection(&mut self) -> Outcome {
        self.cancel_pending_move();
        // FT-485 items 9 and 10: a device or a function selected by reference (`select.set` with `tag:…`) is deleted as
        // the representations the page shown draws of it, as Cut takes it; a device that no page draws (an empty strip,
        // a device never placed) is deleted itself. A device drawn only on other pages is refused, never deleted there.
        let targets = self.copy_targets();
        let Some(page) = self.canvas().map(|c| Arc::clone(c.page())) else {
            return Outcome::Refused {
                reason_key: "reason-nothing-selected",
            };
        };
        let mut drawn = crate::tools::Selection::default();
        drawn.apply(&crate::tools::SelectionChange::Replace(
            targets
                .iter()
                .copied()
                .filter(|e| crate::tools::entity_kind(&page.tables, *e).is_some())
                .collect(),
        ));
        let (unused, elsewhere) = self.undrawn_devices(&targets, &page.tables);
        let mut cmd = transform::delete_command(&drawn, &page);
        if !unused.is_empty() {
            let mut list: Vec<Value> = cmd
                .as_ref()
                .and_then(|c| c.params["targets"].as_array().cloned())
                .unwrap_or_default();
            list.extend(
                unused
                    .iter()
                    .map(|e| json!({"kind": "device", "id": e.0.to_string()})),
            );
            cmd = Some(Box::new(AnyCommand {
                name: "edit.delete".into(),
                params: json!({ "targets": list }),
            }));
        }
        let Some(cmd) = cmd else {
            return Outcome::Refused {
                reason_key: if elsewhere {
                    "reason-not-on-page"
                } else {
                    "reason-nothing-selected"
                },
            };
        };
        self.commit_from_host(cmd.as_ref())
    }

    /// Of `targets`, the entities the page tables do not hold: the devices no representation of the project draws
    /// (and no terminal names as its strip), in target order; and whether any other target is a device or a function
    /// that some other page draws.
    fn undrawn_devices(
        &self,
        targets: &[EntityRef],
        tables: &ew_api::output::PageTables,
    ) -> (Vec<EntityRef>, bool) {
        let mut unused = Vec::new();
        let mut elsewhere = false;
        let Some(project) = &self.project else {
            return (unused, elsewhere);
        };
        let q = project.session.query();
        for e in targets {
            if crate::tools::entity_kind(tables, *e).is_some() {
                continue;
            }
            let Ok(device_id) = e.0.to_string().parse() else {
                continue;
            };
            let Some(device) = q.devices().get(device_id) else {
                // A function drawn on another page (a device's own functions are handled through the device).
                elsewhere |= q
                    .reps()
                    .iter()
                    .any(|(_, r)| r.functions.iter().any(|f| f.as_uuid() == e.0));
                continue;
            };
            let functions: Vec<_> = q
                .functions()
                .iter()
                .filter(|(_, f)| f.device == Some(device.id))
                .map(|(_, f)| f.id)
                .collect();
            let drawn = q
                .reps()
                .iter()
                .any(|(_, r)| r.functions.iter().any(|f| functions.contains(f)));
            if drawn {
                elsewhere = true;
            } else if !q
                .devices()
                .iter()
                .any(|(_, d)| d.aspects.terminal.as_ref().is_some_and(|t| t.strip == device.id))
            {
                unused.push(*e);
            }
        }
        (unused, elsewhere)
    }

    /// `rep.exchange {symbol?}` (§7.5): on exactly one selected representation that draws no interruption point;
    /// without `symbol` the picker opens in exchange mode, with one the tool is armed at once.
    pub(super) fn act_rep_exchange(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let tables = Arc::clone(&c.page().tables);
        let reps: Vec<EntityRef> = selection_kinds(&self.tools.selection, &tables)
            .into_iter()
            .filter(|(_, k)| *k == EntityKind::Rep)
            .map(|(e, _)| e)
            .collect();
        let rep = match reps.as_slice() {
            [] => {
                return Outcome::Refused {
                    reason_key: "reason-nothing-selected",
                };
            }
            [one] => *one,
            _ => {
                return Outcome::Refused {
                    reason_key: "reason-one-rep",
                };
            }
        };
        if tables.reps.iter().any(|r| r.id == rep.0 && r.ip.is_some()) {
            return Outcome::Refused {
                reason_key: "reason-ip-exchange",
            };
        }
        let Some(cat) = self.catalogue() else {
            return Outcome::Refused {
                reason_key: "reason-no-catalogue",
            };
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
                    return Outcome::Refused {
                        reason_key: "reason-unknown-symbol",
                    };
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
    pub(super) fn commit_from_host(&mut self, cmd: &dyn ew_api::DynCommand) -> Outcome {
        self.tools.before_commit = self.canvas().map(|c| Arc::clone(&c.page().tables));
        let out = self.run_commit(cmd);
        self.tools.overlay = crate::tools::Overlay::default();
        self.tools.preview = None;
        out
    }

    /// The after-commit duties (§6.4 item 3), at the end of a dispatch: a committed record the page tables no longer
    /// hold leaves the selection; the place tool's shape is read again for a new revision.
    pub(super) fn after_commit(&mut self) {
        // WP-S2.05 part 3
        self.after_paste();
        self.refresh_place_shape();
        self.refresh_strip_tag();
        self.cancel_stale_move();
        let Some(before) = self.tools.before_commit.take() else {
            return;
        };
        let Some(now) = self.canvas().map(|c| Arc::clone(&c.page().tables)) else {
            return;
        };
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
    /// of the dispatch that changed them (fix round, item 11): its ghost, readout and preview go.
    fn cancel_stale_move(&mut self) {
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

    /// Cancels a pending keyboard move or drag before Delete acts on the selection (§7.4: Delete deletes; the move is
    /// a gesture not yet committed, so it goes first).
    fn cancel_pending_move(&mut self) {
        if matches!(self.tools.tool.state(), "moving" | "dragging") {
            let _ = self.with_tool(|t, cx| t.on_event(&CanvasEvent::Escape, cx));
            self.tools.overlay = crate::tools::Overlay::default();
            self.tools.preview = None;
        }
    }

    /// The strip tag of a terminal placement after a new revision (FT-421): read from the place catalogue (cached per
    /// revision, so a dispatch that changes nothing reads nothing), because a new strip prints its tag only once its
    /// first terminal numbers it.
    fn refresh_strip_tag(&mut self) {
        let strip = self
            .with_tool(|t, _| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<PlaceTool>())
                    .and_then(|p| match p.target() {
                        PlaceTarget::Terminal { strip, .. } => Some(strip.clone()),
                        _ => None,
                    })
            })
            .flatten();
        let Some(strip) = strip else { return };
        let Some(cat) = self.catalogue() else { return };
        let Some(tag) = cat
            .strips
            .iter()
            .find(|st| st.id.to_string() == strip)
            .map(|st| st.tag.clone())
        else {
            return;
        };
        let _ = self.with_tool(|t, _| {
            if let Some(p) = t.as_any_mut().and_then(|a| a.downcast_mut::<PlaceTool>()) {
                p.set_strip_tag(tag);
            }
        });
    }

    /// The place tool's shape after a new revision (§12 item 4): read once again through the per-revision cache, so
    /// a library update reaches the floating symbol; nothing else of the tool changes.
    fn refresh_place_shape(&mut self) {
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

    /// The live-region text of the place tool after a change (§7.2): `Place <key>: <target>, variant <letter>, at
    /// <cell>`.
    pub(super) fn announce_place(&mut self) {
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
        let label = self
            .tools
            .overlay
            .labels()
            .next()
            .map(|l| l.split_once(' ').map_or(l, |(_, rest)| rest).to_owned())
            .unwrap_or_default();
        let cell = at_nm
            .map(|p| c.page().snapper.page_to_sheet(p))
            .and_then(|s| c.page().cell_text(s));
        let v = variant.to_string();
        self.tools.announcement = Some(match cell {
            Some(cell) => strings::format("announce-place", &[&symbol, &label, &v, &cell]),
            None => strings::format("announce-place-no-cell", &[&symbol, &label, &v]),
        });
    }
}
