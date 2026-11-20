// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's duties: the picker popup and its choices, the host commits (the
//! starter embedding), arming the Place and change-symbol tools, the transform keys, the keyboard
//! move, and the selection after a commit.
//!
//! Not here: New terminal strip and Terminal… are in `shell/terminals.rs`; Delete and the devices it leaves undrawn
//! in `shell/delete.rs`.
//!
//! **Reads.** The place catalogue is fetched once per (session, revision, folio) while the picker or `place.pick`
//! needs it, a symbol shape once per copy per revision (§6.2); pointer and cursor moves, the picker's filter and its
//! row keys make no `Session` call (§12 item 4) but the shape of a symbol the browser's picture shows first.
//!
//! **Host commits** (§6.4 item 2) go through the tool's commit path: `Session::execute_with` in the window,
//! `Session::run_script` headless. A failed host commit leaves the tool unarmed and shows the message in the banner.

use std::sync::Arc;

use ew_api::output::{CatalogueSymbol, PageTables, PlaceCatalogue, PlacedBy, SymbolShape, Uuid};
use ew_api::{AnyCommand, Session};
use ew_dl::EntityRef;
use serde_json::{Map, Value, json};

use super::terminals::{TERMINAL_DESIGNATION, TERMINAL_ROW_MAX, TERMINAL_SPACING_MAX, terminal_designation};
use super::{Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::strings;
use crate::strings::Key;
use crate::tools::{
    BlackBoxTool, BoxTarget, CanvasEvent, EntityKind, ExchangeTool, Ghost, PendingTerminal, Picker,
    PickerMode, PickerRow, PlaceTarget, PlaceTool, ResultNote, SelectTool, SelectionChange, Tool, ToolCx,
    ToolKey, ToolOutput, entity_kind, selection_kinds, transform,
};

/// What a choice of the picker or a `place.pick` arms.
pub(super) enum Pick {
    /// A symbol row: its index in the catalogue.
    Symbol(usize),
    /// A template function: the part's and the function's indices.
    PartFn(usize, usize),
    /// A part without a function template with a symbol row, or a terminal's part drawn by the kind Terminal… chose:
    /// the part's and the symbol's indices.
    PartSymbol(usize, usize),
    /// Another representation of a function, with a symbol row.
    Function(String, usize),
    /// An existing device (`place.pick device=<tag>`), with a symbol row.
    Join(super::join::JoinPick, usize),
}

/// The browser's picture of a part's function drawn as a box: the box on the 4 mm grid as the part lays it out (else
/// as the black box tool lays one out), each pin with a step of lead; `None` when it does not fit the grid.
fn box_shape(f: &ew_api::output::CatalogueTemplateFn) -> Option<SymbolShape> {
    use ew_api::part_box::{Dir4, EDIT_STEP, steps};
    use ew_api::symbols::units::Grid;
    let pins: Vec<u16> = f.cps.iter().map(|c| c.0).collect();
    let (_, mut corners, cps) =
        steps(&pins, &f.outputs, f.layout.as_ref(), EDIT_STEP).on_page(Grid::MM4, (0, 0))?;
    let (w, h) = corners.get(2).copied()?;
    corners.push((0, 0));
    let mut outline = vec![corners];
    let mut shape_cps = Vec::new();
    for (idx, at, side) in cps {
        let (dir, lead) = match side {
            Dir4::Up => (ew_api::geometry::Dir::Up, (0, -EDIT_STEP)),
            Dir4::Down => (ew_api::geometry::Dir::Down, (0, EDIT_STEP)),
            Dir4::Left => (ew_api::geometry::Dir::Left, (-EDIT_STEP, 0)),
            _ => (ew_api::geometry::Dir::Right, (EDIT_STEP, 0)),
        };
        let end = (at.0 + lead.0, at.1 + lead.1);
        outline.push(vec![at, end]);
        let designation = f
            .cps
            .iter()
            .find(|c| c.0 == idx)
            .map(|c| c.1.clone())
            .unwrap_or_default();
        shape_cps.push(ew_api::output::ShapeCp {
            symbol_cp: idx,
            fn_cp: idx,
            at: end,
            dir,
            designation,
            fluid: false,
            valve_port: None,
        });
    }
    Some(SymbolShape {
        copy: Uuid::nil(),
        key: String::new(),
        cps: shape_cps,
        body: [0, 0, w, h],
        outline,
        rep_type: "multi_line".to_owned(),
        fndef: Uuid::nil(),
        valve: false,
        contact: false,
        fitting: false,
    })
}

/// The toast's sentence about the devices a delete left without a symbol: `-101K1 (now -?K1) has no symbol on
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
            Some(after) if after != *before => {
                strings::format(key!("toast-device-retagged"), &[before, &after])
            }
            _ => before.clone(),
        })
        .collect();
    let part = unplaced.iter().any(|(_, _, p)| *p);
    let key = match (tags.len(), part) {
        (0, _) => return None,
        (1, true) => key!("toast-device-unplaced"),
        (1, false) => key!("toast-device-unplaced-no-part"),
        (_, true) => key!("toast-devices-unplaced"),
        (_, false) => key!("toast-devices-unplaced-no-part"),
    };
    Some(strings::format(key, &[&tags.join(", ")]))
}

/// The sentence and the `connection.draw` of a far autoconnect hit `far` (the ghost CP's point, the target's label,
/// function and index) of the placed representation whose CPs are `cps`: "-1K1:A1 faces -1S1:14, …". Not inlined: the
/// dispatch's frame stays as it was (the main thread's stack is small).
#[inline(never)]
fn far_offer(
    far: &((i64, i64), String, Uuid, u16),
    cps: &[&ew_api::output::TableCp],
    now: &PageTables,
) -> Option<(String, Value)> {
    let (at, label, function, idx) = far;
    let own = cps.iter().find(|c| c.at == *at)?;
    let wire = json!({
        "a": {"cp": {"function": own.function.to_string(), "idx": own.idx}},
        "b": {"cp": {"function": function.to_string(), "idx": idx}},
        "page": now.page.to_string(),
    });
    Some((
        strings::format(key!("toast-connect-offer"), &[&own.label, label]),
        wire,
    ))
}

impl ShellModel {
    /// After a commit that drew PLC devices for the first time (the rack comes first): a station
    /// placed directly opens its view in a tab beside the page, so it is filled there (a further symbol of a station
    /// drawn before, a station the project had put on a bus page, or one whose view is open already, does not; Escape
    /// goes back to the page); a module that joined a station says which slot it got, with Rack… as the toast's action; a module that
    /// waits asks which station takes it when the project has several, and offers New station… when it has none (the
    /// persona run m3-p4 found the PLC panel by trying every icon).
    fn say_card_waits(&mut self, before: &PageTables, now: &PageTables) {
        let existing = std::mem::take(&mut self.tools.drew_existing);
        let Some(session) = self.project_session() else {
            return;
        };
        let q = session.query();
        // The devices this commit drew on the page that no page drew before it.
        let mut first: Vec<Uuid> = Vec::new();
        for r in now
            .reps
            .iter()
            .filter(|r| !before.reps.iter().any(|b| b.id == r.id))
        {
            let Some(d) = r.device else { continue };
            let Ok(id) = d.to_string().parse() else {
                continue;
            };
            let all = q
                .idx()
                .functions_of_device(id)
                .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
                .count();
            let fresh = now
                .reps
                .iter()
                .filter(|n| n.device == Some(d) && !before.reps.iter().any(|b| b.id == n.id))
                .count();
            if all == fresh && !first.contains(&d) {
                first.push(d);
            }
        }
        drop(q);
        if first.is_empty() {
            return;
        }
        let waiting = session.plc_cards_without_cpu();
        let stations = session.plc_stations_for_cards();
        let new_station = first
            .iter()
            .copied()
            .find(|d| matches!(session.plc_offer(*d), ew_api::output::PlcOffer::Station(_)));
        // The modules among them (a valve joining its terminal is no module) that sit in a station's rack.
        let q = session.query();
        let module = |d: &Uuid| {
            d.to_string()
                .parse()
                .ok()
                .and_then(|id| q.devices().get(id).cloned())
                .and_then(|x| x.aspects.plc_module.clone())
                .is_some_and(|m| !m.integrated)
        };
        let joined: Vec<(Uuid, Uuid)> = first
            .iter()
            .filter(|d| module(d))
            .filter_map(|d| {
                let station = session.plc_station_of(*d)?;
                (station != *d).then_some((*d, station))
            })
            .collect();
        drop(q);
        let waits: Vec<(Uuid, String)> = first
            .iter()
            .filter_map(|d| waiting.iter().find(|(c, _)| c == d).cloned())
            .collect();
        if let Some(station) = new_station {
            // A station whose view is open in a tab already (drawn from that view) stays on the page it was drawn on.
            let open = self.area.as_ref().is_some_and(|a| {
                a.tabs()
                    .any(|(_, _, t)| t.station.as_ref().is_some_and(|s| s.station == Some(station)))
            });
            if !existing && !open {
                let _ = self.act_plc_rack_editor(&Map::from_iter([(
                    "station".to_owned(),
                    Value::String(station.to_string()),
                )]));
            }
            return;
        }
        if let Some((card, station)) = joined.first().copied() {
            let tag = session.device_tag(card).unwrap_or_default();
            let at = session.device_tag(station).unwrap_or_default();
            let slot = session
                .rack_view(station)
                .and_then(|v| v.rows.into_iter().find(|r| r.module == Some(card)))
                .map(|r| super::station::rack_slot_text(&r))
                .unwrap_or_default();
            let text = strings::format(key!("toast-card-joined"), &[&tag, &at, &slot]);
            self.last_station = Some(station);
            self.notify_rack(text, station);
            return;
        }
        let Some((card, tag)) = waits.into_iter().next() else {
            return;
        };
        match stations.len() {
            0 => {
                let text = strings::format(key!("toast-card-waits-cpu"), &[&tag]);
                self.notify_new_station(text);
            }
            1 => {
                let text = strings::format(key!("toast-card-waits"), &[&tag]);
                self.notify(text, false);
            }
            _ => {
                self.open_dialog(super::Dialog::ChooseStation { card, tag, stations });
            }
        }
    }

    /// Arms the black box tool with the box of positions of valve terminal `terminal`, a PLC station whose main
    /// function is no box (made in the New station dialog): one click draws it as its positions, its main function
    /// the box's, or another function beside its bus port when that is put on a bus page first.
    pub(super) fn arm_terminal_box(&mut self, terminal: Uuid) -> Outcome {
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let Some(row) = cat.symbols.iter().find(|s| s.black_box && s.allowed) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(copy) = row.copy.or_else(|| session.starter_symbol_copy(&row.key)) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let mut copies = match session.symbol_embed(&row.key) {
            Ok(params) => params.map(copies_of).unwrap_or_default(),
            Err(error) => {
                return Outcome::Failed {
                    message: error.to_string(),
                };
            }
        };
        // Its valves and its supply stand in it with their symbols.
        if let Some(c) = self.canvas() {
            let cells = session.cell_copies(Some(terminal), &[], Some(c.page().tables.page));
            super::fluid::extend_copies(&mut copies, cells);
        }
        let count = u16::try_from(session.terminal_positions(terminal).len()).ok();
        let target = BoxTarget::Device {
            device: terminal.to_string(),
            main: session.main_function(terminal).is_none(),
        };
        self.tools.picker = None;
        self.activate_tool(Box::new(
            BlackBoxTool::new(copy.to_string(), target)
                .with_copies(copies)
                .with_slots(count),
        ));
        Outcome::Done
    }

    fn say_result(&mut self, before: &PageTables, now: &PageTables) {
        let Some(note) = self.tools.result_note.take() else {
            return;
        };
        match note {
            ResultNote::Placed {
                pos,
                wires,
                moved,
                into,
                far,
            } => {
                // The representation the placement added at its insertion point (the lowest ID if several), else the one
                // it added elsewhere (a valve placed in a terminal's cell stands at the cell's point).
                let added = |at: bool| {
                    now.reps
                        .iter()
                        .filter(|r| (!at || r.pos == pos) && !before.reps.iter().any(|b| b.id == r.id))
                        .min_by_key(|r| r.id)
                };
                let Some(rep) = added(true).or_else(|| added(false)) else {
                    return;
                };
                let cps: Vec<&ew_api::output::TableCp> = now.cps.iter().filter(|c| c.rep == rep.id).collect();
                // A black box (a valve terminal drawn as its positions) prints no pin label: its device's tag.
                let tag = crate::describe::rep_tag(rep, &cps)
                    .or_else(|| rep.device.and_then(|d| self.project_session()?.device_tag(d)))
                    .unwrap_or_else(|| rep.symbol_key.clone());
                // A sentence names a terminal "-X1011 terminal 3", never "-X1011:3".
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
                    Some(cell) => strings::format(key!("toast-placed"), &[&tag, &cell]),
                    None => strings::format(key!("toast-placed-no-cell"), &[&tag]),
                };
                // A symbol dropped on a wire is split into it (normaliser 7) and drew no wire of its own: the
                // toast says that it is in the circuit, where a connection of the page now ends on it.
                let ends_on_rep = |e: &ew_api::output::TableEnd| matches!(e, ew_api::output::TableEnd::Cp { function, .. } if rep.functions.contains(function));
                let in_wire = now
                    .connections
                    .iter()
                    .any(|c| ends_on_rep(&c.a) || ends_on_rep(&c.b));
                let text = match wires {
                    // The way to place without the wires takes its key from the keymap, as the
                    // interruption point's toast does.
                    Some((count, list)) => strings::format(
                        key!("toast-autoconnect"),
                        &[
                            strings::without_stop(&placed),
                            &count,
                            &list,
                            &crate::command_bar::key_name(self, "canvas.activate_alt"),
                        ],
                    ),
                    None if in_wire => {
                        strings::format(key!("toast-placed-in-wire"), &[strings::without_stop(&placed)])
                    }
                    None => placed,
                };
                // A terminal that left the click point says where and why, after the result.
                let text = match moved {
                    Some(moved) => format!("{}, {moved}", strings::without_stop(&text)),
                    None => text,
                };
                // A valve dropped into a valve terminal's box says where it sits now.
                let text = match rep.device.filter(|_| into).and_then(|d| self.seat_words(d)) {
                    Some(seat) => format!("{text} {seat}"),
                    None => text,
                };
                let text = match rep.black_box.as_ref().filter(|b| !b.cells.is_empty()) {
                    Some(b) => self.terminal_placed_words(text, rep.device, &b.cells),
                    None => text,
                };
                // A far autoconnect hit is offered, not drawn, with Connect.
                match far.first().and_then(|f| far_offer(f, &cps, now)) {
                    Some((question, wire)) => self.notify_connect_offer(format!("{text} {question}"), wire),
                    None => self.notify(text, true),
                }
            }
            // An interruption point dropped on a pin says where it stands, what it connects to and the way
            // to place it free (the key from the keymap).
            ResultNote::IpConnected {
                pos,
                name,
                pin,
                external,
                taken,
                placed,
            } => {
                let cell = self.canvas().and_then(|c| {
                    let page = c.page();
                    page.cell_text(page.snapper.page_to_sheet(pos))
                });
                let key = crate::command_bar::key_name(self, "canvas.activate_alt");
                // An external point is named so.
                let text = match (cell, external) {
                    (Some(cell), false) => {
                        strings::format(key!("toast-ip-connected"), &[&name, &cell, &pin, &key])
                    }
                    (None, false) => {
                        strings::format(key!("toast-ip-connected-no-cell"), &[&name, "", &pin, &key])
                    }
                    (Some(cell), true) => {
                        strings::format(key!("toast-ip-external-connected"), &[&name, &cell, &pin, &key])
                    }
                    (None, true) => strings::format(
                        key!("toast-ip-external-connected-no-cell"),
                        &[&name, "", &pin, &key],
                    ),
                };
                // A renamed point says why first; where its partner is, last.
                let text = match taken {
                    Some(taken) => format!("{taken} {text}"),
                    None => text,
                };
                let text = match placed {
                    Some(placed) => format!("{text} {placed}"),
                    None => text,
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
                        (1, 1) => key!("toast-ip-deleted-wire"),
                        (1, _) => key!("toast-ip-deleted-wires"),
                        (_, 1) => key!("toast-ips-deleted-wire"),
                        _ => key!("toast-ips-deleted-wires"),
                    };
                    strings::format(key, &[&names.join(", "), &gone.to_string()])
                });
                // The devices left without a symbol stay in the project; the toast says where.
                let unplaced = self
                    .project
                    .as_ref()
                    .and_then(|p| unplaced_text(&unplaced, |d| p.session.device_tag(d.0)));
                let text: Vec<String> = ips.into_iter().chain(unplaced).collect();
                if text.is_empty() {
                    return;
                }
                // Each sentence ends with its own full stop.
                self.notify(text.join(" "), true);
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The picker popup, while open.
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

    /// What the active tool floats or drags.
    pub fn tool_ghost(&self) -> Option<Ghost> {
        self.tools.tool.ghost()
    }

    // -----------------------------------------------------------------------------------------------------------
    // Reads

    /// The (session, revision, folio) of the focused canvas.
    fn canvas_key(&self) -> Option<(u64, u64, String)> {
        // A station's view shows no page: its catalogue is that of the page it was opened from.
        let folio = match self.canvas() {
            Some(c) => c.page().folio.clone(),
            None => self.station_tab().and(self.current_folio())?.to_owned(),
        };
        Some((self.project_epoch()?, self.revision()?, folio))
    }

    /// The name in words of symbol `key` from the cached place catalogue (M39: the Place HUD names the symbol, not
    /// its key); `None` when the catalogue is not cached, lacks the symbol, or the symbol has no name.
    pub fn symbol_title(&self, key: &str) -> Option<String> {
        let (_, cat) = self.tools.catalogue.as_ref()?;
        cat.symbols
            .iter()
            .find(|s| s.key == key)
            .map(|s| s.name.clone())
            .filter(|n| !n.is_empty())
    }

    /// The place catalogue of the focused page, through the cache.
    pub(super) fn catalogue(&mut self) -> Option<Arc<PlaceCatalogue>> {
        let key = self.canvas_key()?;
        if let Some((k, cat)) = &self.tools.catalogue
            && *k == key
        {
            return Some(Arc::clone(cat));
        }
        let mut cat = self.project.as_ref()?.session.place_catalogue(&key.2)?;
        self.count(|c| c.place_catalogue += 1);
        // A symbol's name in the window's language (FT-1530).
        for s in &mut cat.symbols {
            if !s.name_text.is_empty() {
                s.name = super::symbol_editor::name_words(&s.name_text);
            }
        }
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

    /// The picker's picture follows its current row: the shape of the symbol the row places, through the shape
    /// cache (one `Session` read per symbol and revision); none for a strip or a part without a symbol.
    pub(super) fn refresh_picker_preview(&mut self) {
        let Some(picker) = &self.tools.picker else {
            return;
        };
        let symbol = picker
            .rows
            .get(picker.row)
            .and_then(|r| picker.row_symbol(*r))
            .map(|s| (s.key.clone(), s.copy));
        // The current row's part, for its card (F22).
        let part = match picker.rows.get(picker.row) {
            Some(PickerRow::Part(p) | PickerRow::PartFn(p, _)) => {
                picker.catalogue.parts.get(*p).map(|x| x.copy)
            }
            _ => None,
        };
        // A part drawn as a box shows its box with its pins as it lays them out, not the frame every box shares.
        let boxed = match picker.rows.get(picker.row) {
            Some(PickerRow::Part(p)) => picker.part_main(*p).map(|k| (*p, k)),
            Some(PickerRow::PartFn(p, k)) => Some((*p, *k)),
            _ => None,
        }
        .and_then(|(p, k)| picker.catalogue.parts.get(p)?.functions.get(k))
        .filter(|f| f.black_box || f.layout.is_some())
        .and_then(box_shape)
        .map(Arc::new);
        let shape = boxed.or_else(|| {
            symbol.and_then(|(key, copy)| {
                let copy = copy.or_else(|| self.project.as_ref()?.session.starter_symbol_copy(&key))?;
                self.cached_shape(copy.to_string(), |s| s.symbol_shape(copy))
            })
        });
        let card = match (part, &self.tools.picker) {
            (Some(copy), Some(p)) if p.card.as_ref().is_some_and(|c| c.card.part == copy) => p.card.clone(),
            (Some(copy), _) => self.part_card(copy).map(Arc::new),
            (None, _) => None,
        };
        if let Some(picker) = &mut self.tools.picker {
            picker.preview = shape;
            picker.card = card;
        }
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
            layout: host.layout_options,
            connect_on_move: host.connect_on_move,
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
            self.banner = Some(strings::format(key!("banner-host-commit"), &[name, message]));
        }
        out
    }

    /// The catalogue row of symbol `key` with the copy ID it has, or, for a starter symbol the project does not
    /// embed yet, the one its embedding gives it; and the library documents (`copies`) that the command using it
    /// carries, so that the embedding is part of that command's transaction (FT-656: one undo step, and the
    /// recorded command replays without a library). Nothing is committed here.
    pub(super) fn starter_row(&mut self, key: &str) -> Result<(CatalogueSymbol, Vec<Value>), Outcome> {
        let no_row = Outcome::refused(key!("reason-unknown-symbol"));
        let cat = self
            .catalogue()
            .ok_or(Outcome::refused(key!("reason-no-catalogue")))?;
        let mut row = cat
            .symbols
            .iter()
            .find(|s| s.key == key)
            .cloned()
            .ok_or(no_row.clone())?;
        let project = self.project.as_ref().ok_or(no_row.clone())?;
        let copies = project
            .session
            .symbol_embed(key)
            .map_err(|e| Outcome::Failed {
                message: e.to_string(),
            })?
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

    /// The catalogue strip named by `param`: its ID, or its tag as the page prints it.
    pub(super) fn catalogue_strip(cat: &PlaceCatalogue, param: &str) -> Option<usize> {
        cat.strips
            .iter()
            .position(|st| st.id.to_string() == param)
            .or_else(|| cat.strips.iter().position(|st| st.tag == param))
    }

    /// Arms the Place tool (or, in exchange mode, the change-symbol tool) for `pick` of `cat`; `p` holds the
    /// optional `letter`, `strip` and `designation`. A part's template function arms **part-first**:
    /// the starter symbol and every FnDef the part's template needs are
    /// embedded in one host commit before the tool arms; an FnDef neither the project nor the starter library holds
    /// fails the pick by name and arms nothing.
    /// The tag a new strip (`aspect` `strip`) or cable (`cable`) with letter `letter` would take from its first terminal,
    /// core or bus line on the page on show: its numbering format (`Session::numbering_format`) with the lowest counter
    /// no device of that letter prints on the page (FR-NUM-007 fills gaps). `None` off a schematic page (a cable: and a
    /// bus layout), or on a folio that is not a whole number, where the scheme's `{page}` is not the folio as
    /// written.
    pub(super) fn tag_example(&self, aspect: &str, letter: &str) -> Option<String> {
        self.tag_example_except(aspect, letter, None)
    }

    /// [`Self::tag_example`] with the device `except` left out: a cable the same step removes (a bus line's
    /// cable replaced), whose tag the numbering gives again.
    pub(super) fn tag_example_except(
        &self,
        aspect: &str,
        letter: &str,
        except: Option<Uuid>,
    ) -> Option<String> {
        let row = self.page_rows().get(self.page_index())?;
        let numbered_here = match row.kind.as_str() {
            "multi_line" | "single_line" => true,
            "bus_topology" => aspect == "cable",
            _ => false,
        };
        if !numbered_here || row.folio.is_empty() || !row.folio.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let session = &self.project.as_ref()?.session;
        // The format the scheme gives it (`-101X1` under `{page}{letter}{counter}`, a strip's `-X1011`
        // under the strip rule `{letter}{page}{counter}`); a format with another token gives no example.
        let format = session
            .numbering_format(aspect, letter)
            .replace("{letter}", letter)
            .replace("{page}", &row.folio);
        if format.replace("{counter}", "").contains('{') {
            return None;
        }
        let q = session.query();
        let taken: std::collections::BTreeSet<String> = q
            .devices()
            .iter()
            .filter(|(id, d)| d.tag.letter.as_str() == letter && Some(id.as_uuid()) != except)
            .filter_map(|(id, _)| session.device_tag(id.as_uuid()))
            .collect();
        // Bounded: at most one candidate per printed tag is taken.
        (1..=taken.len() + 1)
            .map(|c| format!("-{}", format.replace("{counter}", &c.to_string())))
            .find(|t| !taken.contains(t))
    }

    /// A refusal whose text names action `action`'s first key: `<key>` formatted with it, or `<key>-unbound` when the
    /// action has no key (FT-896).
    fn refused_with_key(&self, key: Key<'_>, action: &str) -> Outcome {
        Outcome::Failed {
            message: text_with_key(key, &crate::command_bar::key_of(self, action)),
        }
    }

    /// Arms the black box tool with the interfaces of a device on a bus-topology page, a box each (`bus.place_ports`,
    /// doc 10 §2.7): `target` the command's target, `label` what the tool's label names, `part` the part copy of a new
    /// device (whose closure travels in the placement).
    pub(super) fn arm_ports(
        &mut self,
        cat: &PlaceCatalogue,
        target: Value,
        label: String,
        interfaces: &[ew_api::output::CatalogueInterface],
        part: Option<ew_api::output::Uuid>,
    ) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(row) = cat.symbols.iter().find(|r| r.black_box && r.allowed) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let Some(copy) = row.copy.or_else(|| project.session.starter_symbol_copy(&row.key)) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let mut copies: Vec<Value> = match part {
            Some(part) => match project.session.starter_embed_part(part, &row.key) {
                Err(keys) => return failed(key!("error-part-fndef-missing"), &[&keys.join(", ")]),
                Ok(params) => params.map(copies_of).unwrap_or_default(),
            },
            None => match project.session.symbol_embed(&row.key) {
                Ok(params) => params.map(copies_of).unwrap_or_default(),
                Err(error) => {
                    return Outcome::Failed {
                        message: error.to_string(),
                    };
                }
            },
        };
        let fndefs: Vec<ew_api::output::Uuid> = interfaces.iter().map(|i| i.fndef).collect();
        for doc in project.session.port_fndef_copies(&fndefs) {
            if !copies.iter().any(|c| c["id"] == doc["id"]) {
                copies.push(doc);
            }
        }
        let interfaces = interfaces
            .iter()
            .map(|i| crate::tools::PortTarget {
                interface: i.interface.clone(),
                port_label: i.port_label.clone(),
                fndef: i.fndef.to_string(),
                sockets: i.sockets.clone(),
            })
            .collect();
        self.tools.picker = None;
        self.activate_tool(Box::new(
            BlackBoxTool::new(
                copy.to_string(),
                BoxTarget::Ports {
                    target,
                    label,
                    interfaces,
                },
            )
            .with_copies(copies),
        ));
        Outcome::Done
    }

    /// Arms the black box tool with every interface of part `pi` of `cat`, a box each, for a new device of it with the
    /// letter `letter` (else the part's, else the black-box symbol's).
    pub(super) fn arm_part_ports(
        &mut self,
        cat: &PlaceCatalogue,
        pi: usize,
        letter: Option<String>,
    ) -> Outcome {
        let part = &cat.parts[pi];
        let Some(letter) = letter.or(part.letter.clone()).or_else(|| {
            cat.symbols
                .iter()
                .find(|r| r.black_box)
                .and_then(|r| r.letter.clone())
        }) else {
            return Outcome::refused(key!("reason-letter-needed"));
        };
        let target = json!({"from_part": {"part": part.copy.to_string(), "letter": letter}});
        let label = strings::format(key!("place-target-part"), &[&part.part_number]);
        let interfaces = part.interfaces.clone();
        let copy = part.copy;
        self.arm_ports(cat, target, label, &interfaces, Some(copy))
    }

    pub(super) fn arm(&mut self, cat: Arc<PlaceCatalogue>, pick: Pick, p: &Map<String, Value>) -> Outcome {
        let letter_param = match string_param(p, "letter") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        // On a bus-topology page a part with bus interfaces is placed as its interfaces, a box each, with every port
        // (doc 10 §2.7): a station as a switch or a drive, its supply pins left to its place on a schematic page.
        if let Pick::PartFn(pi, _) | Pick::PartSymbol(pi, _) = &pick
            && !cat.parts[*pi].interfaces.is_empty()
        {
            // A station of this part that the project has and this page does not show comes first (a persona run:
            // Device ▸ Place made a second, empty CPU beside the one drawn with its cards): Put -1A1 here, or a new one.
            let copy = cat.parts[*pi].copy;
            if self.mode == super::Mode::Gui && !self.stations_off_page(Some(copy)).is_empty() {
                self.tools.picker = None;
                return self.ask_put_station(Some((copy, letter_param)));
            }
            return self.arm_part_ports(&cat, *pi, letter_param);
        }
        let mut box_cps: Option<Vec<(u16, String)>> = None;
        let mut part_copy = None;
        // The letter of the part's own function category, which a black box placed from it takes.
        let mut part_letter = None;
        let mut part_cps: Vec<(u16, String)> = Vec::new();
        let mut part_outputs: Vec<u16> = Vec::new();
        let mut part_layout = None;
        let (row_index, part) = match &pick {
            Pick::Symbol(i) | Pick::Function(_, i) | Pick::Join(_, i) => (*i, None),
            Pick::PartFn(pi, fi) => {
                let part = &cat.parts[*pi];
                let f = &part.functions[*fi];
                part_copy = Some(part.copy);
                part_letter.clone_from(&part.letter);
                // The CPs come from the part's `cp_pattern`, which the catalogue lists whether or not the FnDef is
                // embedded yet; `f.black_box` is false until it is (the first black box of a project arms through
                // the starter symbol row instead), so the CPs are kept apart from that test (row 4).
                part_cps = f.cps.clone();
                part_outputs = f.outputs.clone();
                part_layout.clone_from(&f.layout);
                if f.black_box {
                    box_cps = Some(f.cps.clone());
                }
                let Some(key) = &f.symbol else {
                    return Outcome::refused(key!("reason-no-symbol-for-function"));
                };
                let Some(i) = cat.symbols.iter().position(|s| s.key == *key) else {
                    return Outcome::refused(key!("reason-no-symbol-for-function"));
                };
                (
                    i,
                    Some((part.copy.to_string(), f.index, part.part_number.clone())),
                )
            }
            // The device carries the part; its one function is the symbol's (`from_part` entry 0).
            Pick::PartSymbol(pi, i) => {
                let part = &cat.parts[*pi];
                part_copy = Some(part.copy);
                part_letter.clone_from(&part.letter);
                (*i, Some((part.copy.to_string(), 0, part.part_number.clone())))
            }
        };
        let row = cat.symbols[row_index].clone();
        if !row.allowed {
            return Outcome::refused(key!("reason-rep-type-not-allowed"));
        }
        // A symbol on another connection grid is placed here as its copy drawn on the page's grid, which the
        // project keeps; a symbol a part draws keeps the refusal with both grids named.
        if let Some(grids) = row.grid_mismatch {
            if let Pick::Symbol(_) = &pick {
                return match self.symbol_on_page_grid(&row.key, grids[1]) {
                    Ok(copy) => {
                        let mut again = p.clone();
                        again.insert("symbol".to_owned(), json!(copy));
                        self.act_place_pick(&again)
                    }
                    Err(message) => Outcome::Failed { message },
                };
            }
            return Outcome::Failed {
                message: crate::tools::grid_words(grids),
            };
        }
        // A black box (a black-box symbol, or a template function that is one) arms the black box tool;
        // `tool.black_box` arms nothing else.
        let is_box = row.black_box || box_cps.is_some();
        // FT-896: the refusals name the way out with its key from the keymap, or without the parenthesis when unbound.
        if self.tools.black_box_pick && !is_box {
            return self.refused_with_key(key!("reason-not-black-box"), "tool.place");
        }
        // An unplaced black-box function (a station placed first on a bus-topology page keeps its supply pins for a
        // schematic page) is drawn by the black box tool as another representation of it.
        if let (true, Pick::Function(function, _)) = (is_box, &pick)
            && let Some(project) = &self.project
            && let Ok(id) = function.parse::<Uuid>()
        {
            let cps = project.session.function_cp_list(id);
            let Some(copy) = row.copy.or_else(|| project.session.starter_symbol_copy(&row.key)) else {
                return Outcome::refused(key!("reason-unknown-symbol"));
            };
            let target = BoxTarget::Function {
                function: function.clone(),
                cps,
            };
            // A valve terminal made as a station, drawn on a fluid page: as its positions, one click.
            let fluid = self
                .page_rows()
                .get(self.page_index())
                .is_some_and(|r| r.kind == "fluid");
            let terminal = project.session.terminal_of_main(id);
            let slots = terminal
                .filter(|_| fluid)
                .and_then(|t| u16::try_from(project.session.terminal_positions(t).len()).ok());
            // A multipin terminal's main function elsewhere is its plug: its pins down its two sides, one click.
            let pins = !fluid && terminal.is_some_and(|t| project.session.terminal_plug(t).is_some());
            // Drawn as its positions, its valves and its supply stand in it with their symbols.
            let copies = match (slots, self.canvas()) {
                (Some(_), Some(c)) => project
                    .session
                    .cell_copies(terminal, &[], Some(c.page().tables.page)),
                _ => Vec::new(),
            };
            self.tools.picker = None;
            self.activate_tool(Box::new(
                BlackBoxTool::new(copy.to_string(), target)
                    .with_copies(copies)
                    .with_slots(slots)
                    .with_pins(pins),
            ));
            return Outcome::Done;
        }
        if is_box && matches!(pick, Pick::Function(..) | Pick::Join(..)) {
            return self.refused_with_key(key!("reason-black-box-tool"), "tool.black_box");
        }
        // FT-656: what the project lacks travels in the placement itself, so that placing from the picker is one undo
        // step and the recorded command replays without a library: a starter symbol with its FnDef and, for a part's
        // template function (part-first), every FnDef the part's
        // template needs and a library part's closure. The tool names a starter symbol by the copy ID its embedding
        // gives it.
        let copies: Vec<Value> = match (&self.project, part_copy) {
            (None, _) => Vec::new(),
            (Some(project), Some(part_copy)) => match project.session.starter_embed_part(part_copy, &row.key)
            {
                Err(keys) => return failed(key!("error-part-fndef-missing"), &[&keys.join(", ")]),
                Ok(params) => params.map(copies_of).unwrap_or_default(),
            },
            (Some(project), None) => match project.session.symbol_embed(&row.key) {
                Ok(params) => params.map(copies_of).unwrap_or_default(),
                Err(error) => {
                    return Outcome::Failed {
                        message: error.to_string(),
                    };
                }
            },
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
                return Outcome::refused(key!("reason-unknown-symbol"));
            };
            let target = match part {
                Some((part, template, number)) => {
                    let Some(letter) = letter_param.or(part_letter).or(row.letter.clone()) else {
                        return Outcome::refused(key!("reason-letter-needed"));
                    };
                    BoxTarget::Part {
                        part,
                        template,
                        letter,
                        number,
                        cps: part_cps,
                        outputs: part_outputs,
                        layout: part_layout,
                    }
                }
                None => {
                    let Some(letter) = letter_param.or(row.letter.clone()) else {
                        return Outcome::refused(key!("reason-letter-needed"));
                    };
                    BoxTarget::NewDevice { letter }
                }
            };
            self.tools.picker = None;
            // On a bus-topology page the frame draws a topology box (a device with its sockets, doc 10 §2.7).
            let rep_type = (row.rep_type == "topology").then(|| row.rep_type.clone());
            // On a fluid page a valve terminal is drawn as its positions, one click.
            let fluid = self
                .page_rows()
                .get(self.page_index())
                .is_some_and(|r| r.kind == "fluid");
            let slots = part_copy
                .filter(|_| fluid)
                .and_then(|p| self.project.as_ref()?.session.part_positions(p));
            // Drawn as its positions, its supply ports stand in its left end.
            let mut copies = copies;
            if let (Some(_), Some(project), Some(c)) = (slots, &self.project, self.canvas()) {
                let cells = project.session.cell_copies(None, &[], Some(c.page().tables.page));
                super::fluid::extend_copies(&mut copies, cells);
            }
            self.activate_tool(Box::new(
                BlackBoxTool::new(copy.to_string(), target)
                    .with_copies(copies)
                    .with_rep_type(rep_type)
                    .with_slots(slots),
            ));
            return Outcome::Done;
        }
        let Some(shape) = self.row_shape(&row) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        // A terminal: a terminal symbol (a PE terminal's category `TerminalEarth` counts too), or any symbol or
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
                    return Outcome::refused(key!("reason-letter-needed"));
                };
                PlaceTarget::Part {
                    part,
                    template,
                    letter,
                    number,
                }
            }
            (_, part) if terminal => {
                let mut strip = or_return!(string_param(p, "strip")).map(str::to_owned);
                // Without a strip in the project a terminal has nowhere to go: a direct `place.pick` refuses. Chosen
                // in the picker (the window's path) it goes on a new strip, made as Terminal…'s new strip is (a
                // persona run: the empty strip chooser that opened instead stood between Place and the page).
                if cat.strips.is_empty() && (strip.is_some() || self.tools.picker.is_none()) {
                    return Outcome::refused(key!("reason-no-strip"));
                }
                let mut cat = cat;
                if cat.strips.is_empty() {
                    let made = self.act_strip_new(&Map::new());
                    if made != Outcome::Done {
                        return made;
                    }
                    strip.clone_from(&self.tools.last_primary);
                    // Its toast says what happens now, not the Terminal… way strip.new's names.
                    self.notify(
                        strings::text(key!("toast-strip-new-for-terminal")).to_owned(),
                        false,
                    );
                    match self.catalogue() {
                        Some(fresh) => cat = fresh,
                        None => return Outcome::refused(key!("reason-no-catalogue")),
                    }
                }
                let strip = strip.as_deref();
                // Without a strip the picker asks for one: its strip chooser.
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
                // An unknown strip name says which strips there are (G-PP finding 9): a new strip's
                // `-X??` takes its counter with its first terminal, so the name the toast gave may have changed.
                let Some(i) = Self::catalogue_strip(&cat, strip) else {
                    let tags: Vec<&str> = cat.strips.iter().map(|st| st.tag.as_str()).collect();
                    return failed(key!("error-unknown-strip"), &[strip, &tags.join(", ")]);
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
                    return Outcome::refused(key!("reason-letter-needed"));
                };
                PlaceTarget::NewDevice { letter }
            }
        };
        self.tools.picker = None;
        let autoconnect = self.page_autoconnects();
        // J offers the Join chooser while the symbol's FnDef can be an auxiliary function.
        let joinable = self
            .project
            .as_ref()
            .is_some_and(|p| p.session.auxiliary_fndef(shape.fndef));
        // A part's function that a device of the part has unplaced fills that device (FT-1658).
        let slot = match &target {
            PlaceTarget::Part { part, template, .. } => part.parse::<Uuid>().ok().map(|p| (p, *template)),
            _ => None,
        };
        // A terminal row of Terminal… (`count`): one click lays them all (FT-1695).
        let row = match &target {
            PlaceTarget::Terminal { .. } => p.get("count").and_then(Value::as_u64).unwrap_or(1),
            _ => 1,
        };
        let row = u32::try_from(row).unwrap_or(1).clamp(1, TERMINAL_ROW_MAX);
        let spacing = p
            .get("spacing")
            .and_then(Value::as_u64)
            .and_then(|s| u32::try_from(s).ok())
            .unwrap_or(super::TERMINAL_SPACING_DEFAULT)
            .clamp(1, TERMINAL_SPACING_MAX);
        self.activate_tool(Box::new(
            PlaceTool::new(shape, target)
                .with_autoconnect(autoconnect)
                .with_joinable(joinable)
                .with_copies(copies)
                .with_row(row)
                .with_spacing(spacing),
        ));
        if let Some((part, template)) = slot {
            self.join_template_slot(part, template);
        }
        Outcome::Done
    }

    /// Arms the change-symbol tool for the representation the picker was opened for and symbol row `i`.
    fn arm_exchange(&mut self, cat: Arc<PlaceCatalogue>, i: usize) -> Outcome {
        let Some(rep) = self.tools.exchange_rep else {
            return Outcome::refused(key!("reason-one-rep"));
        };
        let key = cat.symbols[i].key.clone();
        // A starter symbol the project lacks travels in the exchange itself (one undo step).
        let (row, copies) = or_return!(self.starter_row(&key));
        let Some(shape) = self.row_shape(&row) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        // The FnDef the representation's current symbol draws.
        let old = self
            .canvas()
            .and_then(|c| c.page().tables.reps.iter().find(|r| r.id == rep.0).cloned());
        let Some(old) = old else {
            return Outcome::refused(key!("reason-one-rep"));
        };
        // The shape of the representation's current symbol: the function CPs it draws and their designations.
        let old_symbol = old.symbol;
        let current = self.cached_shape(old_symbol.to_string(), |s| s.symbol_shape(old_symbol));
        self.tools.picker = None;
        self.tools.exchange_rep = None;
        let Some(tool) = self.with_tool(|_, cx| {
            ExchangeTool::new(rep, Arc::clone(&shape), current.as_deref(), cx).with_copies(copies)
        }) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let overlay = tool.overlay();
        self.activate_tool(Box::new(tool));
        self.tools.overlay = overlay;
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // Actions

    /// `tool.place` and `tool.place_part` (§7.6): opens the picker of the focused page.
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

    /// `picker.choose`: the current row (§7.1).
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
            return Outcome::Refused { reason_key };
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
                let incoming = cat.symbols[i].key.split('@').next() == Some("ew.gen.ip-target");
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

    /// `place.pick` (§7.6, headless): one of `symbol` (key or copy ID), `part` (part number or copy ID) with
    /// `template` (0-based; a part without a function template with `symbol` instead), `function` (ID, with `symbol`); optional `letter`; for a terminal `strip` (tag or ID)
    /// and `designation`. `device=<tag>` (with `symbol`) joins an existing device, and `function` also
    /// takes `<tag>:<slot>` or `<tag>` (with `symbol`): see [`super::join`].
    pub(crate) fn act_place_pick(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let unknown = Outcome::refused(key!("reason-unknown-symbol"));
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
        let port = match string_param(p, "port") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        // A port of a device (its tag or ID) on a bus-topology page: the box of that interface with its sockets (doc
        // 10 §2.7); `port` names the interface as its row does ("X1", "X1 P1R").
        if let (Some(device), Some(port)) = (&device, &port)
            && self.page_rows().get(self.page_index()).map(|r| r.kind.as_str()) == Some("bus_topology")
        {
            let id = match self.join_device_ref(device) {
                Ok(id) => id,
                Err(reason) => return failed(key!("error-join-device"), &[device, &reason]),
            };
            let folio = self.current_folio().unwrap_or_default().to_owned();
            let Some(project) = &self.project else {
                return Outcome::refused(key!("reason-no-project"));
            };
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
            let Some(i) = symbol.as_deref().and_then(find) else {
                return failed(key!("error-missing-param"), &["symbol"]);
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
                let Some(i) = symbol.as_deref().and_then(find) else {
                    return failed(key!("error-missing-param"), &["symbol"]);
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
                    let Some(i) = symbol.as_deref().and_then(find) else {
                        return failed(key!("error-missing-param"), &["symbol"]);
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
                let Some(i) = symbol.as_deref().and_then(find) else {
                    return failed(key!("error-missing-param"), &["symbol"]);
                };
                Pick::Function(function.to_owned(), i)
            }
            _ => {
                let Some(s) = symbol else {
                    return failed(key!("error-missing-param"), &["symbol"]);
                };
                let Some(i) = find(&s) else { return unknown };
                Pick::Symbol(i)
            }
        };
        self.tools.picker = None;
        self.arm(cat, pick, p)
    }

    /// The transform keys (§7.4, §7.2): `Key(k)` to the active tool; with Select, V on several representations is
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

    /// Shift+V with Place (§7.2): the next catalogue symbol (key order, wrapping) that draws the same function
    /// definition and the page allows; an embedded one only (a starter symbol is chosen through the picker).
    fn next_rep_type(&mut self) -> Outcome {
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

    /// `tool.move` (M, §7.3): the keyboard move of the selection's movable entities (representations, page items
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
    fn drawn_reps_of_selection(&self) -> Vec<EntityRef> {
        let Some(page) = self.canvas().map(|c| Arc::clone(c.page())) else {
            return Vec::new();
        };
        self.copy_targets()
            .into_iter()
            .filter(|e| crate::tools::entity_kind(&page.tables, *e) == Some(EntityKind::Rep))
            .collect()
    }

    /// `rep.exchange {symbol?}` (§7.5): on exactly one selected representation that draws no interruption point;
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
        // A box armed with records this dispatch took away (a Delete in a navigator, as an Undo would) ends.
        self.end_tool_without_records();
        self.after_layout_commit();
        self.after_paste();
        // FT-900
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
        if let Some(line) = drawn {
            self.notify_bus_line(line);
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

/// The text of `key` naming the key chord `chord` (FT-896): `key` formatted with it, or `<key>-unbound` (the sentence
/// without the parenthesis) when the action has no key.
fn text_with_key(key: Key<'_>, chord: &str) -> String {
    if chord.is_empty() {
        strings::text(Key::run_time(&format!("{}-unbound", key.as_str()))).to_owned()
    } else {
        strings::format(key, &[chord])
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

#[cfg(test)]
mod tests {
    use super::text_with_key;
    use crate::key;

    /// FT-896: the two black-box refusals name the key from the keymap, or leave the parenthesis out when the action
    /// has no key.
    #[test]
    fn the_black_box_refusals_name_the_key_or_none() {
        assert_eq!(
            text_with_key(key!("reason-black-box-tool"), "B"),
            "This part is placed as a black box: use the Black box tool (B)"
        );
        assert_eq!(
            text_with_key(key!("reason-black-box-tool"), ""),
            "This part is placed as a black box: use the Black box tool"
        );
        assert_eq!(
            text_with_key(key!("reason-not-black-box"), "I"),
            "This is not a black box: place it with Draw \u{25b8} Symbol\u{2026} (I)"
        );
        assert_eq!(
            text_with_key(key!("reason-not-black-box"), ""),
            "This is not a black box: place it with Draw \u{25b8} Symbol\u{2026}"
        );
    }
}
