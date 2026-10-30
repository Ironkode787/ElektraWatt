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
use ew_api::output::{CatalogueSymbol, PageTables, PlaceCatalogue, SymbolShape, Uuid};
use ew_api::{AnyCommand, Session};
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, string_param};
use crate::strings;
use crate::tools::{
    BlackBoxTool, BoxTarget, CanvasEvent, EntityKind, ExchangeTool, Ghost, PendingTerminal, Picker,
    PickerMode, PickerRow, PlaceTarget, PlaceTool, ResultNote, SelectTool, SelectionChange, Tool, ToolCx,
    ToolKey, ToolOutput, entity_kind, selection_kinds, transform,
};

/// What a choice of the picker or a `place.pick` arms.
enum Pick {
    /// A symbol row: its index in the catalogue.
    Symbol(usize),
    /// A template function: the part's and the function's indices.
    PartFn(usize, usize),
    /// Another representation of a function, with a symbol row.
    Function(String, usize),
    /// An existing device (WP-S2.18 D1, D4: `place.pick device=<tag>`), with a symbol row.
    Join(super::join::JoinPick, usize),
}

/// The names of a terminal's designation in `place.pick` and `picker.choose`: `terminal_designation`, and
/// `designation`, its old name (FT-543: the old name invited a device tag).
const TERMINAL_DESIGNATION: [&str; 2] = ["terminal_designation", "designation"];

/// The terminal designation of `p`: `terminal_designation`, else its old name `designation`.
fn terminal_designation(p: &Map<String, Value>) -> Result<Option<&str>, Outcome> {
    match string_param(p, TERMINAL_DESIGNATION[0])? {
        Some(d) => Ok(Some(d)),
        None => string_param(p, TERMINAL_DESIGNATION[1]),
    }
}

/// The result note of a delete of the representations `reps` (FT-547, FT-709): the names of the interruption points
/// among them, in page-table order, and the connections at their connection points that `drawn` does not name; and
/// `unplaced`, the devices the delete leaves without a symbol; `None` when neither applies.
fn delete_note(
    reps: &[EntityRef],
    drawn: &crate::tools::Selection,
    t: &PageTables,
    unplaced: Vec<(EntityRef, String, bool)>,
) -> Option<ResultNote> {
    let ips: Vec<&ew_api::output::TableRep> = t
        .reps
        .iter()
        .filter(|r| r.ip.is_some() && reps.contains(&EntityRef(r.id)))
        .collect();
    if ips.is_empty() && unplaced.is_empty() {
        return None;
    }
    let mut wires: Vec<EntityRef> = Vec::new();
    for r in &ips {
        for c in t.cps.iter().filter(|c| c.rep == r.id) {
            for w in &c.connections {
                let w = EntityRef(*w);
                if !drawn.items.contains(&w) && !wires.contains(&w) {
                    wires.push(w);
                }
            }
        }
    }
    Some(ResultNote::Deleted {
        names: ips
            .iter()
            .filter_map(|r| r.ip.as_ref().map(|ip| ip.name.clone()))
            .collect(),
        wires,
        unplaced,
    })
}

/// The toast's sentence about the devices a delete left without a symbol (FT-709): `-101K1 (now -?K1) has no symbol on
/// a page now; it stays in the Devices navigator and the parts list`. Each device is named by its tag before the
/// delete, with the tag the navigator shows now when that differs (`now` gives it); the parts list is named when one
/// of them has a part.
fn unplaced_text(
    unplaced: &[(EntityRef, String, bool)],
    now: impl Fn(EntityRef) -> Option<String>,
) -> Option<String> {
    let tags: Vec<String> = unplaced
        .iter()
        .map(|(d, before, _)| match now(*d) {
            Some(after) if after != *before => strings::format("toast-device-retagged", &[before, &after]),
            _ => before.clone(),
        })
        .collect();
    let part = unplaced.iter().any(|(_, _, p)| *p);
    let key = match (tags.len(), part) {
        (0, _) => return None,
        (1, true) => "toast-device-unplaced",
        (1, false) => "toast-device-unplaced-no-part",
        (_, true) => "toast-devices-unplaced",
        (_, false) => "toast-devices-unplaced-no-part",
    };
    Some(strings::format(key, &[&tags.join(", ")]))
}

impl ShellModel {
    /// The toast of the result note the dispatch's commit left (FT-546, FT-547), read on the page tables after it.
    fn say_result(&mut self, before: &PageTables, now: &PageTables) {
        let Some(note) = self.tools.result_note.take() else {
            return;
        };
        match note {
            ResultNote::Placed { pos, wires } => {
                // The representation the placement added at its insertion point (the lowest ID if several).
                let Some(rep) = now
                    .reps
                    .iter()
                    .filter(|r| r.pos == pos && !before.reps.iter().any(|b| b.id == r.id))
                    .min_by_key(|r| r.id)
                else {
                    return;
                };
                let cps: Vec<&ew_api::output::TableCp> = now.cps.iter().filter(|c| c.rep == rep.id).collect();
                let tag = crate::describe::rep_tag(rep, &cps).unwrap_or_else(|| rep.symbol_key.clone());
                // A sentence names a terminal "-X1011 terminal 3", never "-X1011:3" (FT-716).
                let tag = if rep.terminal {
                    crate::describe::terminal_in_sentence(&tag)
                } else {
                    tag
                };
                let cell = self.canvas().and_then(|c| {
                    let page = c.page();
                    page.cell_text(page.snapper.page_to_sheet(pos))
                });
                let placed = match cell {
                    Some(cell) => strings::format("toast-placed", &[&tag, &cell]),
                    None => strings::format("toast-placed-no-cell", &[&tag]),
                };
                // FT-759: a symbol dropped on a wire is split into it (normaliser 7) and drew no wire of its own: the
                // toast says that it is in the circuit, where a connection of the page now ends on it.
                let ends_on_rep = |e: &ew_api::output::TableEnd| matches!(e, ew_api::output::TableEnd::Cp { function, .. } if rep.functions.contains(function));
                let in_wire = now
                    .connections
                    .iter()
                    .any(|c| ends_on_rep(&c.a) || ends_on_rep(&c.b));
                let text = match wires {
                    Some((count, list)) => strings::format("toast-autoconnect", &[&placed, &count, &list]),
                    None if in_wire => strings::format("toast-placed-in-wire", &[&placed]),
                    None => placed,
                };
                self.notify(text, true);
            }
            ResultNote::Deleted {
                names,
                wires,
                unplaced,
            } => {
                let gone = wires
                    .iter()
                    .filter(|w| !now.connections.iter().any(|c| c.id == w.0))
                    .count();
                let ips = (!names.is_empty() && gone > 0).then(|| {
                    let key = match (names.len(), gone) {
                        (1, 1) => "toast-ip-deleted-wire",
                        (1, _) => "toast-ip-deleted-wires",
                        (_, 1) => "toast-ips-deleted-wire",
                        _ => "toast-ips-deleted-wires",
                    };
                    strings::format(key, &[&names.join(", "), &gone.to_string()])
                });
                // FT-709: the devices left without a symbol stay in the project; the toast says where.
                let unplaced = self
                    .project
                    .as_ref()
                    .and_then(|p| unplaced_text(&unplaced, |d| p.session.device_tag(d.0)));
                let text: Vec<String> = ips.into_iter().chain(unplaced).collect();
                if text.is_empty() {
                    return;
                }
                self.notify(text.join(". "), true);
            }
        }
    }

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

    /// The catalogue row of symbol `key` with the copy ID it has, or, for a starter symbol the project does not
    /// embed yet, the one its embedding gives it; and the library documents (`copies`) that the command using it
    /// carries, so that the embedding is part of that command's transaction (FT-656, FT-673: one undo step, and the
    /// recorded command replays without a library). Nothing is committed here.
    pub(super) fn starter_row(&mut self, key: &str) -> Result<(CatalogueSymbol, Vec<Value>), Outcome> {
        let no_row = Outcome::Refused {
            reason_key: "reason-unknown-symbol",
        };
        let cat = self.catalogue().ok_or(Outcome::Refused {
            reason_key: "reason-no-catalogue",
        })?;
        let mut row = cat
            .symbols
            .iter()
            .find(|s| s.key == key)
            .cloned()
            .ok_or(no_row.clone())?;
        let project = self.project.as_ref().ok_or(no_row.clone())?;
        let copies = project
            .session
            .starter_embed(key)
            .map(copies_of)
            .unwrap_or_default();
        if row.copy.is_none() {
            row.copy = project.session.starter_symbol_copy(key);
        }
        if row.copy.is_none() {
            return Err(no_row);
        }
        Ok((row, copies))
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
        let mut part_cps: Vec<(u16, String)> = Vec::new();
        let (row_index, part) = match &pick {
            Pick::Symbol(i) | Pick::Function(_, i) | Pick::Join(_, i) => (*i, None),
            Pick::PartFn(pi, fi) => {
                let part = &cat.parts[*pi];
                let f = &part.functions[*fi];
                part_copy = Some(part.copy);
                // The CPs come from the part's `cp_pattern`, which the catalogue lists whether or not the FnDef is
                // embedded yet; `f.black_box` is false until it is (the first black box of a project arms through
                // the starter symbol row instead), so the CPs are kept apart from that test (cycle 33, row 4).
                part_cps = f.cps.clone();
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
        if is_box && matches!(pick, Pick::Function(..) | Pick::Join(..)) {
            return Outcome::Refused {
                reason_key: "reason-black-box-tool",
            };
        }
        // FT-656: what the project lacks travels in the placement itself, so that placing from the picker is one undo
        // step and the recorded command replays without a library: a starter symbol with its FnDef and, for a part's
        // template function (part-first, WP-S2.04 §7.1 as amended by the cycle-27 follow-up), every FnDef the part's
        // template needs and a library part's closure. The tool names a starter symbol by the copy ID its embedding
        // gives it.
        let copies: Vec<Value> = match (&self.project, part_copy) {
            (None, _) => Vec::new(),
            (Some(project), Some(part_copy)) => match project.session.starter_embed_part(part_copy, &row.key)
            {
                Err(keys) => return failed("error-part-fndef-missing", &[&keys.join(", ")]),
                Ok(params) => params.map(copies_of).unwrap_or_default(),
            },
            (Some(project), None) => project
                .session
                .starter_embed(&row.key)
                .map(copies_of)
                .unwrap_or_default(),
        };
        let mut row = row;
        if row.copy.is_none() {
            row.copy = self
                .project
                .as_ref()
                .and_then(|p| p.session.starter_symbol_copy(&row.key));
        }
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
                        cps: part_cps,
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
            self.activate_tool(Box::new(
                BlackBoxTool::new(copy.to_string(), target).with_copies(copies),
            ));
            return Outcome::Done;
        }
        let Some(shape) = self.row_shape(&row) else {
            return Outcome::Refused {
                reason_key: "reason-unknown-symbol",
            };
        };
        // A terminal: a terminal symbol (a PE terminal's category `TerminalEarth` counts too, FT-618), or any symbol or
        // part given a strip or a designation. A part-first terminal joins the strip and carries its part (G-PP
        // finding 2: the part route ignored `strip` and `designation` and made a free device).
        let terminal = row.terminal
            || p.contains_key("strip")
            || TERMINAL_DESIGNATION.iter().any(|k| p.contains_key(*k));
        let target = match (pick, part) {
            (Pick::Function(function, _), _) => PlaceTarget::Function { function },
            (Pick::Join(j, _), _) => PlaceTarget::Join {
                device: j.device,
                tag: j.tag,
                slots: j.slots,
            },
            (_, Some((part, template, number))) if !terminal => {
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
            (_, part) if terminal => {
                let strip = match string_param(p, "strip") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                // Without a strip in the project a terminal has nowhere to go. Chosen in the picker (the window's
                // path) the strip chooser opens on its empty state, which says why and offers New terminal strip
                // (FT-605); `strip.new` then lists the new strip as the current row. A direct `place.pick` refuses.
                if cat.strips.is_empty() && (strip.is_some() || self.tools.picker.is_none()) {
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
                        part: part.map(|(copy, template, _)| (copy, template)),
                    };
                    self.palette = None;
                    self.tools.picker = Some(Picker::strips(&folio, Arc::clone(&cat), pending));
                    return Outcome::Done;
                };
                // An unknown strip name says which strips there are (cycle 33, G-PP finding 9): a new strip's
                // `-X??` takes its counter with its first terminal, so the name the toast gave may have changed.
                let Some(i) = Self::catalogue_strip(&cat, strip) else {
                    let tags: Vec<&str> = cat.strips.iter().map(|st| st.tag.as_str()).collect();
                    return failed("error-unknown-strip", &[strip, &tags.join(", ")]);
                };
                let st = &cat.strips[i];
                let designation = match terminal_designation(p) {
                    Ok(Some(d)) => d.to_owned(),
                    Ok(None) => st.next_designation.clone(),
                    Err(e) => return e,
                };
                PlaceTarget::Terminal {
                    strip: st.id.to_string(),
                    strip_tag: st.tag.clone(),
                    designation,
                    part: part.map(|(copy, _, _)| copy),
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
        // WP-S2.18 D2: J offers the Join chooser while the symbol's FnDef can be an auxiliary function.
        let joinable = self
            .project
            .as_ref()
            .is_some_and(|p| p.session.auxiliary_fndef(shape.fndef));
        self.activate_tool(Box::new(
            PlaceTool::new(shape, target)
                .with_autoconnect(autoconnect)
                .with_joinable(joinable)
                .with_copies(copies),
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
        // FT-673: a starter symbol the project lacks travels in the exchange itself (one undo step).
        let (row, copies) = match self.starter_row(&key) {
            Ok(v) => v,
            Err(e) => return e,
        };
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
        let Some(tool) = self.with_tool(|_, cx| {
            ExchangeTool::new(rep, Arc::clone(&shape), current.as_deref(), cx).with_copies(copies)
        }) else {
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

    /// `picker.row {step | to}`: Up, Down (`step` −1, 1), Home (`to` `first` or 0), End (`to` `last`, −1 or a large
    /// number). Either parameter alone is enough (the signature lists both as optional).
    pub(super) fn act_picker_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let step = p.get("step").and_then(Value::as_i64);
        let to = match p.get("to") {
            None => None,
            Some(Value::String(word)) => match word.as_str() {
                "first" => Some(0),
                "last" => Some(-1),
                _ => return failed("error-bad-param", &["to"]),
            },
            Some(v) => match v.as_i64() {
                Some(t) => Some(t),
                None => return failed("error-bad-param", &["to"]),
            },
        };
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
                // The parameters of this choice (`designation`, `letter`, `strip`) win over those of the choice that
                // opened the chooser; the strip is the current row's unless this choice names one.
                let mut params = pending.params.clone();
                // A designation of this choice replaces the earlier one under either of its names (FT-543).
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
        }
    }

    /// `place.pick` (§7.6, headless): one of `symbol` (key or copy ID), `part` (part number or copy ID) with
    /// `template` (0-based), `function` (ID, with `symbol`); optional `letter`; for a terminal `strip` (tag or ID)
    /// and `designation`. WP-S2.18 D4: `device=<tag>` (with `symbol`) joins an existing device, and `function` also
    /// takes `<tag>:<slot>` or `<tag>` (with `symbol`): see [`super::join`].
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
        let device = match string_param(p, "device") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        // WP-S2.18 D4: a device by its printed tag (or its ID) joins it.
        if let Some(device) = device {
            let Some(i) = symbol.as_deref().and_then(find) else {
                return failed("error-missing-param", &["symbol"]);
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
            // WP-S2.18 D4: a function by its device's printed tag (`-101K1`, `-101K1:13-14`); an ID keeps the path below.
            (_, Ok(Some(function))) if function.trim().parse::<ew_api::output::Uuid>().is_err() => {
                let Some(i) = symbol.as_deref().and_then(find) else {
                    return failed("error-missing-param", &["symbol"]);
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
                // WP-S2.07 §6.8: an unplaced terminal row names its terminal device, which places the device's main
                // function.
                let main = Uuid::parse_str(function)
                    .ok()
                    .and_then(|u| self.project.as_ref()?.session.main_function(u))
                    .map(|(f, key)| (f.to_string(), key));
                let function = main.as_ref().map_or(function, |(f, _)| f.as_str());
                // WP-S2.06 part 1 (§6.7): without `symbol`, the drop of an unplaced function row takes the first
                // catalogue symbol that draws the function's FnDef (the Devices navigator's function row names it).
                let symbol = symbol.or_else(|| {
                    let fndef = match &main {
                        Some((_, key)) => key.clone(),
                        None => self.function_fndef_key(function)?,
                    };
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
            // the representations the page shown draws of it, as Delete and Cut take it (FT-485 items 9 and 10):
            // the selection becomes those representations, but only once the key is sent (FT-626): a refused key
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

    /// The representations of the page shown that the selection names by reference: for a selected device or function
    /// the page tables do not list, the representations drawing it; empty when nothing of the selection is drawn here.
    fn drawn_reps_of_selection(&self) -> Vec<EntityRef> {
        let Some(page) = self.canvas().map(|c| Arc::clone(c.page())) else {
            return Vec::new();
        };
        self.copy_targets()
            .into_iter()
            .filter(|e| crate::tools::entity_kind(&page.tables, *e) == Some(EntityKind::Rep))
            .collect()
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
        let (mut unused, elsewhere) = self.undrawn_devices(&targets, &page.tables);
        // G-PP finding 5: a device that loses its last representation with this delete goes with it in the same
        // command (one undo step), unless it has another function or a terminal role; the run left a phantom `-?G1`
        // with its part in the parts list.
        let reps: Vec<EntityRef> = selection_kinds(&drawn, &page.tables)
            .into_iter()
            .filter(|(_, k)| *k == EntityKind::Rep)
            .map(|(e, _)| e)
            .collect();
        for d in self.devices_left_undrawn(&reps) {
            if !unused.contains(&d) {
                unused.push(d);
            }
        }
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
        // FT-547: an interruption point takes the wires at its connection point with it; the toast says so. FT-709: a
        // device that keeps no symbol but stays in the project (EW-CORE-047) is named, with where it stays.
        let unplaced = self.devices_kept_unplaced(&reps, &unused);
        let note = delete_note(&reps, &drawn, &page.tables, unplaced);
        let out = self.commit_from_host(cmd.as_ref());
        if out == Outcome::Done {
            self.tools.result_note = note;
        }
        out
    }

    /// The devices that deleting the representations `reps` leaves without any representation, in ID order: every
    /// function of the device is drawn only by `reps`. A device is kept (its functions stay unplaced, as before, and
    /// EW-CORE-047 reports it) when it has a function `reps` do not draw, a terminal or strip role, child devices, or
    /// a cable, assembly or PLC aspect, or when one of its functions carries intent beyond the deleted drawing: a
    /// potential definition or a bond at one of its CPs, or a connection that is page-less or drawn on a page other
    /// than the deleted representations' pages. Deleting the device would drop or cascade that intent unseen.
    fn devices_left_undrawn(&self, reps: &[EntityRef]) -> Vec<EntityRef> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let idx = q.idx();
        let deleted: std::collections::BTreeSet<_> =
            reps.iter().filter_map(|e| e.0.to_string().parse().ok()).collect();
        let mut functions = std::collections::BTreeSet::new();
        for r in &deleted {
            if let Some(rep) = q.reps().get(*r) {
                functions.extend(rep.functions.iter().copied());
            }
        }
        let devices: std::collections::BTreeSet<_> = functions
            .iter()
            .filter_map(|f| q.functions().get(*f).and_then(|f| f.device))
            .collect();
        let pages: std::collections::BTreeSet<_> = deleted
            .iter()
            .filter_map(|r| q.reps().get(*r).map(|r| r.page))
            .collect();
        let carries_intent = |f| {
            q.potential_defs().iter().any(|(_, p)| p.at.function() == Some(f))
                || q.bonds()
                    .iter()
                    .any(|(_, b)| b.a.function() == Some(f) || b.b.function() == Some(f))
                || idx.connected_cps_of_function(f).iter().any(|(_, cs)| {
                    cs.iter().any(|c| {
                        q.connections()
                            .get(*c)
                            .is_some_and(|c| c.page.is_none_or(|p| !pages.contains(&p)))
                    })
                })
        };
        let mut out = Vec::new();
        for id in devices {
            let Some(d) = q.devices().get(id) else {
                continue;
            };
            let a = &d.aspects;
            if a.terminal.is_some()
                || a.strip.is_some()
                || a.cable.is_some()
                || a.assembly.is_some()
                || a.plc_station.is_some()
                || a.plc_module.is_some()
                || q.devices().iter().any(|(_, c)| c.parent == Some(id))
            {
                continue;
            }
            let all_gone = idx.functions_of_device(id).all(|f| {
                functions.contains(&f)
                    && idx.reps_of_function(f).all(|r| deleted.contains(&r))
                    && !carries_intent(f)
            });
            if all_gone {
                out.push(EntityRef(id.as_uuid()));
            }
        }
        out
    }

    /// The devices that deleting the representations `reps` leaves without any representation while the delete keeps
    /// them (not in `deleted`, the devices the same command deletes), in ID order (FT-709): each as the device, its tag and whether
    /// it has a part. Terminals and strips are left out (a terminal stays in its strip).
    fn devices_kept_unplaced(
        &self,
        reps: &[EntityRef],
        deleted: &[EntityRef],
    ) -> Vec<(EntityRef, String, bool)> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let idx = q.idx();
        let gone: std::collections::BTreeSet<_> =
            reps.iter().filter_map(|e| e.0.to_string().parse().ok()).collect();
        let devices: std::collections::BTreeSet<_> = gone
            .iter()
            .filter_map(|r| q.reps().get(*r))
            .flat_map(|r| r.functions.iter().copied())
            .filter_map(|f| q.functions().get(f).and_then(|f| f.device))
            .collect();
        devices
            .into_iter()
            .filter(|d| !deleted.contains(&EntityRef(d.as_uuid())))
            .filter_map(|d| {
                let dev = q.devices().get(d)?;
                if dev.aspects.terminal.is_some() || dev.aspects.strip.is_some() {
                    return None;
                }
                let left = idx
                    .functions_of_device(d)
                    .any(|f| idx.reps_of_function(f).any(|r| !gone.contains(&r)));
                if left {
                    return None;
                }
                let tag = project.session.device_tag(d.as_uuid())?;
                Some((EntityRef(d.as_uuid()), tag, !dev.parts.is_empty()))
            })
            .collect()
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
        // WP-S2.18 D1: a sticky join target fills the next slot, then makes new auxiliary functions.
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
        // The IP tool's ghost proposes its next name on the page as the commit left it (its kept direction may meet
        // the name it just placed; FT-547).
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

/// The library documents of a `{"copies": [...]}` parameter object (FT-656).
fn copies_of(params: Value) -> Vec<Value> {
    match params {
        Value::Object(mut m) => match m.remove("copies") {
            Some(Value::Array(copies)) => copies,
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}
