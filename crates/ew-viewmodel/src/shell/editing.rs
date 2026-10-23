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
    CanvasEvent, EntityKind, ExchangeTool, Ghost, Picker, PickerMode, PickerRow, PlaceTarget, PlaceTool,
    SelectTool, SelectionChange, Tool, ToolCx, ToolKey, ToolOutput, entity_kind, selection_kinds, transform,
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
    fn catalogue(&mut self) -> Option<Arc<PlaceCatalogue>> {
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
    fn cached_shape(
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
    fn row_shape(&mut self, row: &CatalogueSymbol) -> Option<Arc<SymbolShape>> {
        let copy = row.copy?;
        self.cached_shape(copy.to_string(), |s| s.symbol_shape(copy))
    }

    /// The tool context of the focused canvas, for host duties that call a tool directly.
    fn with_tool<R>(&mut self, f: impl FnOnce(&mut dyn Tool, &ToolCx<'_>) -> R) -> Option<R> {
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
    fn host_commit(&mut self, name: &str, params: Value) -> Outcome {
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
    fn embedded_row(&mut self, key: &str) -> Result<(Arc<PlaceCatalogue>, usize), Outcome> {
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

    /// The strip of a terminal placement: `param` (an ID, or a tag of a strip whose segment the page draws), else
    /// the strip the last `strip.new` created, else the first strip whose segment the page draws. `(ID, tag)`.
    fn resolve_strip(&self, param: Option<&str>) -> Option<(String, String)> {
        let c = self.canvas()?;
        let tables = &c.page().tables;
        // The strips whose segments the page draws, with the tag their CPs print.
        let strips: Vec<(String, String)> = tables
            .reps
            .iter()
            .filter(|r| r.rep_type == "strip_segment")
            .filter_map(|r| {
                let device = r.device?;
                let tag = tables
                    .cps
                    .iter()
                    .find(|cp| cp.rep == r.id)
                    .and_then(|cp| cp.label.split_once(':').map(|(t, _)| t.to_owned()))?;
                Some((device.to_string(), tag))
            })
            .collect();
        match param {
            Some(p) if p.len() == 36 && p.chars().filter(|c| *c == '-').count() == 4 => {
                let tag = strips
                    .iter()
                    .find(|(id, _)| id == p)
                    .map_or_else(|| p.to_owned(), |(_, t)| t.clone());
                Some((p.to_owned(), tag))
            }
            Some(tag) => strips.into_iter().find(|(_, t)| t == tag),
            None => match &self.tools.last_strip {
                Some(id) => Some((id.clone(), "X".to_owned())),
                None => strips.into_iter().next(),
            },
        }
    }

    /// The next free decimal designation of the terminals of the strip with `tag` the page draws (`1` without one).
    fn next_free_designation(&self, tag: &str) -> String {
        let Some(c) = self.canvas() else {
            return "1".to_owned();
        };
        let prefix = format!("{tag}:");
        let max = c
            .page()
            .tables
            .cps
            .iter()
            .filter_map(|cp| cp.label.strip_prefix(&prefix))
            .filter_map(|rest| rest.split('/').next()?.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        (max + 1).to_string()
    }

    /// Arms the Place tool (or, in exchange mode, the change-symbol tool) for `pick` of `cat`; `p` holds the
    /// optional `letter`, `strip` and `designation`.
    fn arm(&mut self, cat: Arc<PlaceCatalogue>, pick: Pick, p: &Map<String, Value>) -> Outcome {
        let letter_param = match string_param(p, "letter") {
            Ok(v) => v.map(str::to_owned).or_else(|| self.tools.pick_letter.clone()),
            Err(e) => return e,
        };
        let (row_index, part) = match &pick {
            Pick::Symbol(i) | Pick::Function(_, i) => (*i, None),
            Pick::PartFn(pi, fi) => {
                let part = &cat.parts[*pi];
                let f = &part.functions[*fi];
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
        if row.black_box {
            return Outcome::Refused {
                reason_key: "reason-black-box-tool",
            };
        }
        // A starter symbol is embedded first (its own undo step).
        let (cat, row_index) = match self.embedded_row(&row.key) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let row = cat.symbols[row_index].clone();
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
                let Some((strip, strip_tag)) = self.resolve_strip(strip) else {
                    return Outcome::Refused {
                        reason_key: "reason-no-strip",
                    };
                };
                let designation = match string_param(p, "designation") {
                    Ok(Some(d)) => d.to_owned(),
                    Ok(None) => self.next_free_designation(&strip_tag),
                    Err(e) => return e,
                };
                PlaceTarget::Terminal {
                    strip,
                    strip_tag,
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
        self.activate_tool(Box::new(PlaceTool::new(shape, target)));
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
        let old_symbol = old.symbol;
        let old_fndef = self
            .cached_shape(old_symbol.to_string(), |s| s.symbol_shape(old_symbol))
            .map(|s| s.fndef);
        let same_def = old_fndef == Some(shape.fndef);
        self.tools.picker = None;
        self.tools.exchange_rep = None;
        let Some(tool) = self.with_tool(|_, cx| ExchangeTool::new(rep, Arc::clone(&shape), same_def, cx))
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
                reason_key: "reason-no-row",
            };
        };
        if let Some(reason_key) = picker.disabled(row) {
            return Outcome::Refused { reason_key };
        }
        let (mode, cat) = (picker.mode, Arc::clone(&picker.catalogue));
        match (mode, row) {
            (PickerMode::Exchange, PickerRow::Symbol(i)) => self.arm_exchange(cat, i),
            (_, PickerRow::Symbol(i)) => self.arm(cat, Pick::Symbol(i), p),
            (_, PickerRow::PartFn(a, b)) => self.arm(cat, Pick::PartFn(a, b), p),
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

    /// `strip.new {letter}` (§7.6): a host commit of `device.create` with the strip aspect; the new strip becomes the
    /// default strip of terminal placements.
    pub(super) fn act_strip_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let letter = match string_param(p, "letter") {
            Ok(v) => v.unwrap_or("X").to_owned(),
            Err(e) => return e,
        };
        let out = self.host_commit("device.create", json!({"letter": letter, "strip": {}}));
        if out == Outcome::Done {
            self.tools.last_strip = self.tools.last_primary.clone();
        }
        out
    }

    /// The transform keys (§7.4, §7.2): `Key(k)` to the active tool; with Select, V on several representations is
    /// refused `reason-one-rep` and Shift+V `reason-place-only`; with Place, Shift+V is the host's (the next
    /// catalogue symbol drawing the same function definition).
    pub(super) fn act_tool_key(&mut self, key: ToolKey) -> Outcome {
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

    /// `tool.move` (M, §7.3): the keyboard move of the selection's representations; the grid cursor goes to the
    /// primary entity's insertion point when it is not shown.
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
        let reps: Vec<EntityRef> = selection_kinds(&self.tools.selection, &tables)
            .into_iter()
            .filter(|(_, k)| *k == EntityKind::Rep)
            .map(|(e, _)| e)
            .collect();
        if reps.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-nothing-to-move",
            };
        }
        if c.cursor().is_none() {
            let primary = self
                .tools
                .selection
                .primary
                .filter(|e| entity_kind(&tables, *e) == Some(EntityKind::Rep))
                .unwrap_or(reps[0]);
            if let Some(rep) = tables.reps.iter().find(|r| r.id == primary.0) {
                let at = c.page().snapper.page_to_sheet(rep.pos);
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
        let cmd = self
            .canvas()
            .and_then(|c| transform::delete_command(&self.tools.selection, c.page()));
        let Some(cmd) = cmd else {
            return Outcome::Refused {
                reason_key: "reason-nothing-selected",
            };
        };
        self.commit_from_host(cmd.as_ref())
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
    fn commit_from_host(&mut self, cmd: &dyn ew_api::DynCommand) -> Outcome {
        self.tools.before_commit = self.canvas().map(|c| Arc::clone(&c.page().tables));
        let out = self.run_commit(cmd);
        self.tools.overlay = crate::tools::Overlay::default();
        self.tools.preview = None;
        out
    }

    /// The after-commit duties (§6.4 item 3), at the end of a dispatch: a committed record the page tables no longer
    /// hold leaves the selection; the place tool's shape is read again for a new revision.
    pub(super) fn after_commit(&mut self) {
        self.refresh_place_shape();
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
