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
//! needs it, a symbol shape once per copy per revision; pointer and cursor moves, the picker's filter and its
//! row keys make no `Session` call but the shape of a symbol the browser's picture shows first.
//!
//! **Host commits** go through the tool's commit path: `Session::execute_with` in the window,
//! `Session::run_script` headless. A failed host commit leaves the tool unarmed and shows the message in the banner.

use std::sync::Arc;

use ew_api::output::{CatalogueSymbol, PageTables, PlaceCatalogue, SymbolShape, Uuid};
use ew_api::{AnyCommand, Session};
use ew_dl::EntityRef;
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed};
use crate::key;
use crate::notifications::{Note, Offer};
use crate::strings;
use crate::strings::Key;
use crate::tools::{BlackBoxTool, BoxTarget, Ghost, Picker, PickerRow, PlaceTool, ResultNote, Tool, ToolCx};

mod arming;

mod actions;

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

/// The browser's picture of a part's function drawn as a box: the box on the 4 mm grid as the part lays it out, else a
/// PLC card's (`card`, its pins by kind) as the black box tool places it (doc 10 §3.6), else as that tool lays a plain
/// box out, each pin with a step of lead; `None` when it does not fit the grid.
fn box_shape(
    f: &ew_api::output::CatalogueTemplateFn,
    card: Option<ew_api::output::CardPins>,
) -> Option<SymbolShape> {
    use ew_api::part_box::{Dir4, EDIT_STEP, steps};
    use ew_api::symbols::units::Grid;
    let pins: Vec<u16> = f.cps.iter().map(|c| c.0).collect();
    let laid = match card.filter(|c| f.layout.is_none() && c.is_card()) {
        Some(card) => card.boxes(i64::MAX / 4).into_iter().next()?,
        None => steps(&pins, &f.outputs, f.layout.as_ref(), EDIT_STEP),
    };
    let (_, mut corners, cps) = laid.on_page(Grid::MM4, (0, 0))?;
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
    /// waits asks which station takes it when the project has several, and offers New station… when it has none.
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
            let rack = self.rack_offer(station);
            self.notify_offer(text, rack);
            return;
        }
        let Some((card, tag)) = waits.into_iter().next() else {
            return;
        };
        match stations.len() {
            0 => {
                let text = strings::format(key!("toast-card-waits-cpu"), &[&tag]);
                let new = self.session_offer(
                    "new_station",
                    key!("action-plc-new-station"),
                    vec![("plc.new_station", json!({}))],
                );
                self.notify_offer(text, new);
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
        let session = &or_return!(crate::shell::gated(&self.project)).session;
        let Some(copy) = row.copy.or_else(|| session.starter_symbol_copy(&row.key)) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let mut copies = match embedding(session, &row.key, None) {
            Ok(copies) => copies,
            Err(out) => return out,
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
            } => self.say_placed(before, now, pos, wires, moved, into, far),
            // An interruption point dropped on a pin says where it stands, what it connects to and the way
            // to place it free (the key from the keymap).
            ResultNote::IpConnected {
                name,
                pin,
                external,
                taken,
                placed,
            } => self.say_ip_connected(name, pin, external, taken, placed),
            ResultNote::Deleted {
                names,
                wires,
                others,
                unplaced,
                undrawn,
            } => self.say_deleted(now, names, wires, others, unplaced, undrawn),
            // A cut says what it took and the wires it removed with it; the paste brings the cut items back.
            ResultNote::Cut {
                symbols,
                terminals,
                wires,
            } => self.say_cut(now, symbols, terminals, wires),
        }
    }

    /// A placement's result line: the tag and the cell of what it placed, the wires it drew, where a terminal moved
    /// to, where a valve sits, and a far autoconnect hit offered with Connect.
    #[allow(clippy::too_many_arguments)]
    fn say_placed(
        &mut self,
        before: &PageTables,
        now: &PageTables,
        pos: (i64, i64),
        wires: Option<(bool, Vec<(String, String)>)>,
        moved: Option<String>,
        into: bool,
        far: Vec<((i64, i64), String, Uuid, u16)>,
    ) {
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
        // A row's terminal that made a wire is named on the placed terminal's strip (its ghost named a strip
        // without a tag `-X??`): "-X1501 terminal 1".
        let strip = rep
            .terminal
            .then(|| tag.rsplit_once(':').map(|(s, _)| s.to_owned()))
            .flatten();
        let wires = wires.map(|(tubes, list)| {
            let own = |o: String| match (&strip, o.rsplit_once(':')) {
                (Some(strip), Some((_, number))) => {
                    crate::describe::terminal_in_sentence(&format!("{strip}:{number}"))
                }
                _ => o,
            };
            (
                tubes,
                list.into_iter().map(|(o, to)| (own(o), to)).collect::<Vec<_>>(),
            )
        });
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
        // The wires it made, after what it placed: the one by its two pins, two or more by their count and the
        // first; then the way to place without them, its key from the keymap as the interruption point's
        // toast takes it.
        let wires = wires.filter(|(_, list)| !list.is_empty());
        let without = wires.is_some().then(|| {
            strings::format(
                key!("toast-place-without-wires"),
                &[&crate::command_bar::key_name(self, "canvas.activate_alt")],
            )
        });
        let text = match wires {
            Some((tubes, list)) => {
                let placed = strings::without_stop(&placed);
                let (own, to) = &list[0];
                let n = list.len();
                let count = strings::format(
                    if tubes {
                        key!("count-tubes")
                    } else {
                        key!("count-wires")
                    },
                    &[&n.to_string()],
                );
                let more = (n - 1).to_string();
                strings::format_counted(
                    crate::count_key!("toast-placed-connected"),
                    n,
                    &[placed, &count, own, to, &more],
                )
            }
            None if in_wire => {
                strings::format(key!("toast-placed-in-wire"), &[strings::without_stop(&placed)])
            }
            None => placed,
        };
        // A terminal that left the click point says where and why, in the order of the language's pattern.
        let text = match moved {
            Some(moved) => strings::format(
                key!("toast-placed-and-moved"),
                &[strings::without_stop(&text), strings::without_stop(&moved)],
            ),
            None => text,
        };
        // A valve dropped into a valve terminal's box says where it sits now.
        let mut sentences: Vec<String> = std::iter::once(text)
            .chain(rep.device.filter(|_| into).and_then(|d| self.seat_words(d)))
            .collect();
        if let Some(b) = rep.black_box.as_ref().filter(|b| !b.cells.is_empty()) {
            sentences = self.terminal_placed_words(sentences.join(" "), rep.device, &b.cells);
        }
        // A far autoconnect hit is offered, not drawn, with Connect: its Undo and Connect are valid while the
        // placement is the next undo step.
        match (far.first().and_then(|f| far_offer(f, &cps, now)), without) {
            (Some((question, wire)), without) => {
                sentences.extend(without);
                sentences.push(question);
                let connect = self.undo_validity().map(|v| {
                    Offer::steps(
                        "connect",
                        key!("note-action-connect"),
                        v,
                        vec![crate::notifications::command_step("connection.draw", wire)],
                    )
                    .once()
                });
                let undo = self.undo_offer();
                self.notes
                    .push(Note::of("edit", sentences).offering([undo, connect]));
            }
            (None, Some(without)) => self.notify_wired(sentences, without),
            (None, None) => {
                let undo = self.undo_offer();
                self.notes.push(Note::of("edit", sentences).offering([undo]));
            }
        }
    }

    /// An interruption point dropped on a pin says the pin it stands on, which says where, and the way to place it free;
    /// its sentences fit a toast at 1920 × 1080 whole.
    fn say_ip_connected(
        &mut self,
        name: String,
        pin: String,
        external: bool,
        taken: Option<String>,
        placed: Option<String>,
    ) {
        let key = crate::command_bar::key_name(self, "canvas.activate_alt");
        // An external point is named so.
        let words = if external {
            key!("toast-ip-external-connected")
        } else {
            key!("toast-ip-connected")
        };
        let text = strings::format(words, &[&name, &pin, &key]);
        // A renamed point says why first; where its partner is, last.
        let sentences = taken.into_iter().chain([text]).chain(placed).collect();
        let undo = self.undo_offer();
        self.notes.push(Note::of("edit", sentences).offering([undo]));
    }

    /// A delete says the interruption points and the wires it took, the devices it left without a symbol and the
    /// terminals it left drawn nowhere, with Remove them from the project (it, for one) beside its Undo.
    fn say_deleted(
        &mut self,
        now: &PageTables,
        names: Vec<String>,
        wires: Vec<EntityRef>,
        others: Vec<EntityRef>,
        unplaced: Vec<(EntityRef, String, bool)>,
        undrawn: Vec<EntityRef>,
    ) {
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
        // The wires drawn to the deleted symbols went with them (none stays drawn on no page).
        let others = others
            .iter()
            .filter(|w| !now.connections.iter().any(|c| c.id == w.0))
            .count();
        let others =
            (others > 0).then(|| strings::format_count(crate::count_key!("toast-deleted-wires"), others));
        // The devices left without a symbol stay in the project; the toast says where.
        let unplaced = self
            .project
            .as_ref()
            .and_then(|p| unplaced_text(&unplaced, |d| p.session.device_tag(d.0)));
        // Terminals drawn nowhere stay in their strip and the lists, unless removed from the project too.
        let terminals = (!undrawn.is_empty())
            .then(|| strings::format_count(crate::count_key!("toast-terminals-undrawn"), undrawn.len()));
        let sentences: Vec<String> = ips
            .into_iter()
            .chain(others)
            .chain(unplaced)
            .chain(terminals)
            .collect();
        if sentences.is_empty() {
            return;
        }
        // Each sentence ends with its own full stop. Remove it (them) from the project, beside the delete's Undo,
        // removes the terminals the delete left drawn nowhere (and a strip that keeps no terminal and draws
        // nothing); the notifications list keeps the offer while it can run, after the toast has gone.
        let undo = self.undo_offer();
        if undrawn.is_empty() {
            self.notes.push(Note::of("edit", sentences).offering([undo]));
        } else {
            let terminals: Vec<ew_api::output::Uuid> = undrawn.iter().map(|e| e.0).collect();
            let label =
                strings::format_count(crate::count_key!("note-action-remove-undrawn"), terminals.len());
            let removal = self.removal_command(&terminals);
            let remove = self.undo_validity().zip(removal).map(|(v, params)| {
                Offer::steps(
                    "remove_undrawn",
                    key!("note-action-remove-undrawn"),
                    v,
                    vec![crate::notifications::command_step("edit.delete", params)],
                )
                .once()
                .labelled(label)
            });
            self.notes
                .push(Note::of("edit", sentences).offering([undo, remove]));
        }
    }

    /// A cut says what it took and the wires it removed with it.
    fn say_cut(&mut self, now: &PageTables, symbols: usize, terminals: bool, wires: Vec<EntityRef>) {
        let what = if terminals {
            crate::count_key!("toast-cut-terminals")
        } else {
            crate::count_key!("toast-cut-symbols")
        };
        let gone = wires
            .iter()
            .filter(|w| !now.connections.iter().any(|c| c.id == w.0))
            .count();
        let sentences = std::iter::once(strings::format_count(what, symbols))
            .chain((gone > 0).then(|| strings::format_count(crate::count_key!("toast-cut-wires"), gone)))
            .collect();
        let undo = self.undo_offer();
        self.notes.push(Note::of("edit", sentences).offering([undo]));
    }

    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The picker popup, while open.
    pub fn picker(&self) -> Option<&Picker> {
        self.tools.picker.as_ref()
    }

    /// Whether the active place tool waits for the next terminal designation.
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
            None => self.station_page_folio()?.to_owned(),
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
        // A symbol's name in the window's language.
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
        .and_then(|(p, k)| Some((picker.catalogue.parts.get(p)?, k)))
        .and_then(|(part, k)| Some((part.copy, part.functions.get(k)?)))
        // Drawn as a box: its function a black box, or (not in the project yet) its symbol the frame's.
        .filter(|(_, f)| {
            let framed = (f.symbol.as_ref())
                .and_then(|k| picker.catalogue.symbols.iter().find(|s| s.key == *k))
                .is_some_and(|s| s.black_box);
            f.black_box || framed || f.layout.is_some()
        })
        .and_then(|(copy, f)| {
            let card = self.project_session().map(|s| s.part_card_pins(copy, &f.cps));
            box_shape(f, card)
        })
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

    /// A host commit: the tool's commit path; a failure shows its message in the banner.
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
    /// carries, so that the embedding is part of that command's transaction (one undo step, and the
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
        let copies = embedding(&project.session, key, None)?;
        if row.copy.is_none() {
            row.copy = project.session.starter_symbol_copy(key);
        }
        if row.copy.is_none() {
            return Err(no_row);
        }
        Ok((row, copies))
    }
}

/// The text of `key` naming the key chord `chord`: `key` formatted with it, or `unbound` (the sentence without the
/// parenthesis) when the action has no key.
fn text_with_key((key, unbound): (Key<'_>, Key<'_>), chord: &str) -> String {
    if chord.is_empty() {
        strings::text(unbound).to_owned()
    } else {
        strings::format(key, &[chord])
    }
}

/// A part placed as a black box: use its tool, with the tool's key and without one.
const BLACK_BOX_TOOL: (Key<'static>, Key<'static>) = (
    key!("reason-black-box-tool"),
    key!("reason-black-box-tool-unbound"),
);

/// A symbol that is no black box: place it with Draw ▸ Symbol…, with its key and without one.
const NOT_BLACK_BOX: (Key<'static>, Key<'static>) =
    (key!("reason-not-black-box"), key!("reason-not-black-box-unbound"));

/// The library documents of a `{"copies": [...]}` parameter object.
/// The library documents a placement of symbol `key` carries (the embedding travels in the placement itself,
/// one undo step, and the recorded command replays without a library): with the part copy `part`, every FnDef its
/// template needs and a library part's closure, else the starter symbol with its FnDef. A part whose FnDefs the
/// project lacks is refused in words; a symbol that cannot be embedded fails with its message.
fn embedding(session: &Session, key: &str, part: Option<Uuid>) -> Result<Vec<Value>, Outcome> {
    match part {
        Some(part) => match session.starter_embed_part(part, key) {
            Err(keys) => Err(failed(key!("error-part-fndef-missing"), &[&keys.join(", ")])),
            Ok(params) => Ok(params.map(copies_of).unwrap_or_default()),
        },
        None => match session.symbol_embed(key) {
            Ok(params) => Ok(params.map(copies_of).unwrap_or_default()),
            Err(error) => Err(Outcome::Failed {
                message: error.to_string(),
            }),
        },
    }
}

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
    use super::{BLACK_BOX_TOOL, NOT_BLACK_BOX, text_with_key};

    /// The two black-box refusals name the key from the keymap, or leave the parenthesis out when the action
    /// has no key.
    #[test]
    fn the_black_box_refusals_name_the_key_or_none() {
        assert_eq!(
            text_with_key(BLACK_BOX_TOOL, "B"),
            "This part is placed as a black box: use the Black box tool (B)"
        );
        assert_eq!(
            text_with_key(BLACK_BOX_TOOL, ""),
            "This part is placed as a black box: use the Black box tool"
        );
        assert_eq!(
            text_with_key(NOT_BLACK_BOX, "I"),
            "This is not a black box: place it with Draw \u{25b8} Symbol\u{2026} (I)"
        );
        assert_eq!(
            text_with_key(NOT_BLACK_BOX, ""),
            "This is not a black box: place it with Draw \u{25b8} Symbol\u{2026}"
        );
    }
}
