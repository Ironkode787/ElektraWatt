// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The station view's edits: each handler builds one declared command from the place of the rack it is aimed at and
//! commits it as one undo step, saying what it did in words.
//!
//! - **Attach cards without a station**: a CPU row of the PLC navigator attaches the cards that wait for a CPU
//!   (`Session::plc_cards_without_cpu`) with one `assembly.attach`, after the station's last child position.
//! - **The rack**: a part of the view's catalogue goes into a free slot with `plc.add_module` (a library part embedded
//!   in the same step); a card dragged to another slot moves there with `fields.set plc_module.slot`, or changes
//!   places with the card it is dropped on. The card under the cursor is the window's selection. EW-PLC-011's prevent
//!   half refuses a slot the part does not allow; the refusal stays at that slot.
//! - **New station…** and its Create, the tags and the PLC's name changed in place, a channel's symbol and function
//!   text, and a bus device added to a PLC.
//!
//! Not here: what the view shows is in `shell/station.rs`; drawing the station on a page in `station/draw.rs`.

use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use super::{StationDraft, StationKind};
use crate::area::{StationCursor, StationTab};
use crate::key;
use crate::shell::{Outcome, ShellModel, failed, flag, string_param};
use crate::strings;

impl ShellModel {
    /// `plc.attach_cards {station, card?}`: the cards that wait for a station (with `card`, that one of
    /// them) become children of `station`, after its last child position, with one `assembly.attach`; a toast names
    /// them.
    pub(crate) fn act_plc_attach_cards(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.record_param(p, "station"));
        let only = match p.get("card") {
            Some(_) => Some(or_return!(self.record_param(p, "card"))),
            None => None,
        };
        let project = or_return!(crate::shell::gated(&self.project));
        // A card dropped on a station's row: one that cannot go there says why.
        let station_tag = project.session.device_tag(station).unwrap_or_default();
        if !project
            .session
            .plc_stations_for_cards()
            .iter()
            .any(|(s, _)| *s == station)
        {
            return failed(key!("error-station-takes-no-cards"), &[&station_tag]);
        }
        let mut cards = project.session.plc_cards_without_cpu();
        cards.retain(|(card, _)| only.is_none_or(|o| *card == o));
        // A card of another station dropped here moves to this one (`assembly.attach` moves a child); one of this
        // station already, or a device that is no card, says so.
        if let (true, Some(card)) = (cards.is_empty(), only) {
            let tag = project.session.device_tag(card).unwrap_or_default();
            let q = project.session.query();
            let device = q
                .devices()
                .iter()
                .find(|(id, _)| id.as_uuid() == card)
                .map(|(_, d)| d.clone());
            match device {
                Some(d) if d.parent.is_some_and(|p| p.as_uuid() == station) => {
                    return failed(key!("error-card-on-station"), &[&tag, &station_tag]);
                }
                // Only a module goes into a rack (a bus node whose part has no PLC data was
                // attached to a CPU and got eight channels no part gives); a bus device joins a PLC as a station.
                Some(d)
                    if d.aspects.plc_module.as_ref().is_some_and(|m| !m.integrated)
                        && !project.session.not_a_module(card) =>
                {
                    cards.push((card, tag));
                }
                _ if project.session.not_a_module(card) || project.session.bus_device(card) => {
                    return failed(key!("error-not-a-card-bus"), &[&tag]);
                }
                _ => return failed(key!("error-not-a-card"), &[&tag]),
            }
        }
        if cards.is_empty() {
            return Outcome::refused(key!("reason-no-cards-without-cpu"));
        }
        let q = project.session.query();
        let Some(id) = q
            .devices()
            .iter()
            .map(|(id, _)| id)
            .find(|id| id.as_uuid() == station)
        else {
            return failed(key!("error-bad-param"), &["station"]);
        };
        let last = q
            .idx()
            .device_children(id)
            .filter_map(|c| q.devices().get(c).and_then(|d| d.position))
            .max()
            .unwrap_or(0);
        let tag = project.session.device_tag(station).unwrap_or_default();
        let children: Vec<Value> = (cards.iter().zip(last + 1..))
            .map(|((card, _), position)| json!({"device": card.to_string(), "position": position}))
            .collect();
        let tags: Vec<String> = cards.into_iter().map(|(_, t)| t).collect();
        let cmd = ew_api::AnyCommand {
            name: "assembly.attach".into(),
            params: json!({"parent": station.to_string(), "children": children}),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            self.last_station = Some(station);
            let text = strings::format(key!("toast-cards-attached"), &[&tags.join(", "), &tag]);
            self.notify(text, true);
        }
        out
    }

    /// The PLC tab's Module commands (`plc.module_replace {part}`, `plc.module_remove`, `plc.module_draw {folio}`,
    /// `plc.module_left`, `plc.module_right`): `plc.rack_slot`'s `op` on the module under the cursor.
    pub(crate) fn act_plc_module(&mut self, p: &Map<String, Value>, op: &str) -> Outcome {
        let mut params = p.clone();
        params.insert("op".to_owned(), Value::String(op.to_owned()));
        self.act_plc_rack_slot(&params)
    }

    /// `plc.module_goto`: the page the module under the cursor is drawn on, at its drawing (`nav.show`); the
    /// station's tab stays open beside the page.
    pub(crate) fn act_plc_module_goto(&mut self) -> Outcome {
        match self.station_device() {
            Some(device) => self.dispatch("nav.show", &json!({ "record": device.to_string() })),
            None => Outcome::refused(key!("reason-rack-no-module")),
        }
    }

    /// `plc.module_move {folio}` (Move to page): the drawings on electrical pages of the module or the head
    /// under the cursor move to the schematic page `folio` where they stand, with one `rep.move` (one undo step), so the
    /// device keeps its channels, bindings, slot and wires drawn with it; the page is shown. A wire whose other end
    /// stays behind is kept as a pair of interruption points, which a preview card lists first
    /// ([`ShellModel::move_card`]); a module drawn on that page already is refused.
    /// `folio` `new`: a new page after the module's own, made and taken in one step, which the toast names.
    pub(crate) fn act_plc_module_move(&mut self, p: &Map<String, Value>) -> Outcome {
        let cursor = self.station_tab().and_then(|t| t.cursor);
        let Some(device) = self.station_device() else {
            return Outcome::refused(key!("reason-rack-no-module"));
        };
        let folio = match string_param(p, "folio") {
            Ok(Some(f)) => f.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["folio"]),
            Err(e) => return e,
        };
        let drawings = self
            .project_session()
            .map(|s| s.electrical_drawings(device))
            .unwrap_or_default();
        // To a new page: the page after the module's own in its section, as New page proposes it, made and taken in one
        // step (`rep.move_to_new_page`); the toast names it.
        if folio == "new" {
            let before: Vec<Uuid> = self.page_rows().iter().map(|r| r.id).collect();
            let reps: Vec<String> = drawings.iter().map(|(rep, _)| rep.to_string()).collect();
            let title = strings::text(key!("rack-draw-page-title"));
            let mut params = json!({"reps": reps, "title": title});
            // The folio New page proposes after the module's page.
            if let Some((_, on)) = drawings.first()
                && let Ok(number) = self
                    .new_page_proposal(Some(&format!("p:{on}")), Some("multi_line"))
                    .0
                    .parse::<u32>()
            {
                params["number"] = json!(number);
            }
            if let Some(card) = self.move_card("rep.move_to_new_page", &mut params) {
                return card;
            }
            let out = self.rack_commit(cursor, "rep.move_to_new_page", params);
            if out != Outcome::Done {
                return out;
            }
            let made = self
                .page_rows()
                .iter()
                .find(|r| !before.contains(&r.id))
                .map(|r| r.folio.clone());
            if let Some(made) = made {
                let tag = self
                    .project_session()
                    .and_then(|s| s.device_tag(device))
                    .unwrap_or_default();
                let _ = self.dispatch("page.goto", &json!({ "folio": made }));
                self.notify(
                    strings::format(key!("toast-module-moved-new-page"), &[&tag, &made]),
                    true,
                );
            }
            return out;
        }
        let page = self
            .page_rows()
            .iter()
            .find(|r| r.folio == folio && r.kind.is_schematic())
            .map(|r| r.id);
        let Some(page) = page else {
            return failed(key!("error-bad-param"), &["folio"]);
        };
        let reps: Vec<String> = drawings
            .into_iter()
            .filter(|(_, on)| *on != page)
            .map(|(rep, _)| rep.to_string())
            .collect();
        if reps.is_empty() {
            return self.rack_refuse(
                cursor,
                strings::text(key!("reason-rack-move-same-page")).to_owned(),
            );
        }
        let mut params = json!({"reps": reps, "page": page.to_string()});
        if let Some(card) = self.move_card("rep.move", &mut params) {
            return card;
        }
        let out = self.rack_commit(cursor, "rep.move", params);
        if out == Outcome::Done {
            let _ = self.dispatch("page.goto", &json!({ "folio": folio }));
        }
        out
    }

    /// The end of every dispatch for the station views: a tab whose station is gone (deleted, its making undone)
    /// closes.
    pub(in crate::shell) fn drop_gone_stations(&mut self) {
        let Some(session) = self.project.as_ref().map(|p| &p.session) else {
            return;
        };
        let gone = |s: &StationTab| {
            s.draft.is_none() && s.station.is_none_or(|id| session.plc_station_of(id) != Some(id))
        };
        if let Some(a) = &mut self.area {
            a.drop_stations(gone);
        }
    }

    /// Opens the view `tab` beside the page tabs, the canvas area focused; the station's tab when it is open already.
    pub(super) fn open_station(&mut self, tab: StationTab) -> Outcome {
        let Some(area) = &mut self.area else {
            return Outcome::refused(key!("reason-no-pages"));
        };
        let same = |s: &StationTab| match tab.station {
            Some(id) => s.station == Some(id),
            None => s.draft.is_some(),
        };
        let open = area
            .tabs()
            .find(|(_, _, t)| t.station.as_ref().is_some_and(same))
            .map(|(g, i, _)| (g, i));
        let out = match open {
            Some((g, i)) => area.activate(g, i),
            None => area.open_station(tab),
        };
        if let Err(key) = out {
            return Outcome::refused(key);
        }
        self.palette = None;
        self.focus = self.area.as_ref().map(|a| crate::shell::Focus::Canvas(a.focused));
        Outcome::Done
    }

    /// `plc.rack_editor {station}` (FR-PLC-028): opens the view of a station as a tab beside the page tabs.
    pub(crate) fn act_plc_rack_editor(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.record_param(p, "station"));
        if self.rack_view(station).is_none() {
            return failed(key!("error-bad-param"), &["station"]);
        }
        self.last_station = Some(station);
        let out = self.open_station(StationTab {
            station: Some(station),
            ..StationTab::default()
        });
        // The station is selected until a module is: Properties shows it, the cursor stands on its head.
        if out == Outcome::Done {
            let cursor = self.station_tab().and_then(|t| t.cursor);
            self.rack_cursor(cursor.or(Some(StationCursor::Head)));
        }
        out
    }

    /// `plc.new_station {kind?}`: the view of a station not made yet, a PLC (CPU) unless `kind` says another, in the
    /// project's first PLC, else a new one. Nothing is written before Create.
    pub(crate) fn act_plc_new_station(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match string_param(p, "kind") {
            Ok(Some(w)) => match StationKind::from_word(w) {
                Some(k) => Some(k),
                None => return failed(key!("error-bad-param"), &["kind"]),
            },
            Ok(None) => None,
            Err(e) => return e,
        };
        let session = &or_return!(crate::shell::gated(&self.project)).session;
        let config = session.plc_config_projects().first().map(|(c, _)| *c);
        let out = self.open_station(StationTab {
            draft: Some(StationDraft {
                kind: kind.unwrap_or(StationKind::Cpu),
                head: None,
                config,
                modules: Vec::new(),
                tag: None,
            }),
            ..StationTab::default()
        });
        // A draft that was open already takes the kind asked for.
        if let (Some(k), Some(d)) = (kind, self.station_tab_mut().and_then(|t| t.draft.as_mut())) {
            d.kind = k;
        }
        out
    }

    /// The part of the place catalogue that parameter `name` names: its copy ID, or its part number (a script's way).
    fn catalogue_part(
        &mut self,
        p: &Map<String, Value>,
        name: &str,
    ) -> Result<ew_api::output::CataloguePart, Outcome> {
        let text = match string_param(p, name)? {
            Some(t) => t.trim(),
            None => return Err(failed(key!("error-missing-param"), &[name])),
        };
        let id = text.parse::<Uuid>().ok();
        let named = |m: &&ew_api::output::CataloguePart| Some(m.copy) == id || m.part_number == text;
        let Some(cat) = self.catalogue() else {
            return Err(failed(key!("error-bad-param"), &[name]));
        };
        if let Some(m) = cat.parts.iter().find(named) {
            return Ok(m.clone());
        }
        // A part nothing draws is listed greyed: taken, it is refused with what to do.
        if cat.plc_without_functions.iter().any(|m| named(&m)) {
            let cursor = self.station_tab().and_then(|t| t.cursor);
            let why = strings::text(key!("rack-part-no-functions")).to_owned();
            return Err(self.rack_refuse(cursor, why));
        }
        Err(failed(key!("error-bad-param"), &[name]))
    }

    /// `plc.rack_set {kind?, head?, config?, filter?}`: a new station's choices (what it is, its head part, its PLC: an
    /// ID or `new`) and the catalogue's filter text.
    pub(crate) fn act_plc_rack_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match string_param(p, "kind") {
            Ok(Some(w)) => match StationKind::from_word(w) {
                Some(k) => Some(k),
                None => return failed(key!("error-bad-param"), &["kind"]),
            },
            Ok(None) => None,
            Err(e) => return e,
        };
        let head = match p.get("head") {
            Some(_) => {
                let part = or_return!(self.catalogue_part(p, "head"));
                if part.plc.as_ref().is_none_or(|x| x.heads.is_empty()) {
                    return failed(key!("error-bad-param"), &["head"]);
                }
                Some(part.copy)
            }
            None => None,
        };
        let config = match string_param(p, "config") {
            Ok(Some("new")) => Some(None),
            Ok(Some(_)) => {
                let c = or_return!(self.record_param(p, "config"));
                let known = self
                    .project_session()
                    .is_some_and(|s| s.plc_config_projects().iter().any(|(x, _)| *x == c));
                if !known {
                    return failed(key!("error-bad-param"), &["config"]);
                }
                Some(Some(c))
            }
            Ok(None) => None,
            Err(e) => return e,
        };
        let filter = match string_param(p, "filter") {
            Ok(f) => f.map(str::to_owned),
            Err(e) => return e,
        };
        let Some(tab) = self.station_tab_mut() else {
            return Outcome::refused(key!("reason-rack-closed"));
        };
        if let Some(f) = filter {
            tab.filter = f;
        }
        if kind.is_some() || head.is_some() || config.is_some() {
            let Some(d) = &mut tab.draft else {
                return Outcome::refused(key!("reason-rack-made"));
            };
            if let Some(k) = kind
                && k != d.kind
            {
                d.kind = k;
                // Another kind of station has another head.
                d.head = None;
            }
            if let Some(h) = head {
                d.head = Some(h);
            }
            if let Some(c) = config {
                d.config = c;
            }
            tab.error = None;
        }
        Outcome::Done
    }

    /// The places of the rack on show in the order the arrow keys walk them: the head, the slots, the positions.
    fn rack_places(view: &ew_api::output::RackView) -> Vec<StationCursor> {
        std::iter::once(StationCursor::Head)
            .chain(view.rows.iter().map(|r| StationCursor::Slot {
                rack: r.rack,
                slot: r.slot,
            }))
            .chain(view.cells.iter().map(|c| StationCursor::Position {
                module: c.module,
                position: c.position,
            }))
            .collect()
    }

    /// Puts the cursor of the view on show on `place` and selects the device there, as a click on a page selects a
    /// device: Properties shows it and the PLC tab's commands act on it. On a free place the station is selected,
    /// so Properties shows what the view is about.
    fn rack_cursor(&mut self, place: Option<StationCursor>) {
        let Some(tab) = self.station_tab_mut() else {
            return;
        };
        tab.cursor = place;
        // The keys are the rack's again, not the channel table's.
        tab.channel_cursor = None;
        // The view has the keys from here on: the arrows walk its places, Delete empties one.
        self.focus = self.area.as_ref().map(|a| crate::shell::Focus::Canvas(a.focused));
        let station = self
            .station_tab()
            .and_then(|t| t.station.filter(|_| t.draft.is_none()));
        let items = self
            .station_device()
            .or(station)
            .map(ew_dl::EntityRef)
            .into_iter()
            .collect();
        if self
            .tools
            .selection
            .apply(&crate::tools::SelectionChange::Replace(items))
        {
            self.announce_selection();
        }
    }

    /// `plc.rack_select {head?, slot?, rack?, module?, position?, card?, step?}`: the cursor of the station view on
    /// show: the station's head, a slot, a valve position of `module`, the place of device `card`, or `step` places
    /// on from where it stands (the arrow keys).
    pub(crate) fn act_plc_rack_select(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(view) = self.shown_rack() else {
            return Outcome::refused(key!("reason-rack-closed"));
        };
        let places = Self::rack_places(&view);
        let number = |name: &str| p.get(name).and_then(Value::as_u64);
        let place = if flag(p, "head") {
            StationCursor::Head
        } else if let Some(step) = p.get("step").and_then(Value::as_i64) {
            let at = self
                .station_tab()
                .and_then(|t| t.cursor)
                .and_then(|c| places.iter().position(|x| *x == c));
            let last = places.len().saturating_sub(1);
            let to = match at {
                Some(i) => i
                    .saturating_add_signed(isize::try_from(step).unwrap_or(0))
                    .min(last),
                None => 0,
            };
            places[to]
        } else if p.contains_key("card") {
            let card = or_return!(self.record_param(p, "card"));
            let slot = view.rows.iter().find(|r| r.module == Some(card));
            let cell = view.cells.iter().find(|c| c.at.holder == Some(card));
            match (slot, cell) {
                _ if view.head.module == Some(card) => StationCursor::Head,
                (Some(r), _) => StationCursor::Slot {
                    rack: r.rack,
                    slot: r.slot,
                },
                (None, Some(c)) => StationCursor::Position {
                    module: c.module,
                    position: c.position,
                },
                (None, None) => return failed(key!("error-bad-param"), &["card"]),
            }
        } else if let Some(position) = number("position") {
            StationCursor::Position {
                module: or_return!(self.record_param(p, "module")),
                position: u32::try_from(position).unwrap_or(u32::MAX),
            }
        } else if let Some(slot) = number("slot") {
            StationCursor::Slot {
                rack: number("rack").and_then(|r| u16::try_from(r).ok()).unwrap_or(0),
                slot: u32::try_from(slot).unwrap_or(u32::MAX),
            }
        } else {
            return failed(key!("error-missing-param"), &["slot"]);
        };
        if !places.contains(&place) {
            return failed(key!("error-bad-param"), &["slot"]);
        }
        self.rack_cursor(Some(place));
        Outcome::Done
    }

    /// The station of the view on show (not a draft).
    pub(super) fn rack_station(&self) -> Result<Uuid, Outcome> {
        match self.station_tab() {
            Some(StationTab {
                station: Some(station),
                draft: None,
                ..
            }) => Ok(*station),
            Some(_) => Err(Outcome::refused(key!("reason-rack-draft"))),
            None => Err(Outcome::refused(key!("reason-rack-closed"))),
        }
    }

    /// Writes `why` at `place` of the rack on show and fails with it.
    pub(super) fn rack_refuse(&mut self, place: Option<StationCursor>, why: String) -> Outcome {
        if let Some(tab) = self.station_tab_mut() {
            tab.error = Some((place, why.clone()));
            self.said_at(crate::shell::said::Said::Station);
        }
        Outcome::Failed { message: why }
    }

    /// Commits a change of the station on show: a refusal stays at `place` of its rack, a change clears it; a change
    /// that moved addresses says so under the rack (the rack's order is the truth for slots and addresses).
    fn rack_commit(&mut self, place: Option<StationCursor>, name: &str, params: Value) -> Outcome {
        let addresses = |s: &Self| -> Vec<(Option<Uuid>, String)> {
            s.shown_rack()
                .map(|v| v.rows.into_iter().map(|r| (r.module, r.addresses)).collect())
                .unwrap_or_default()
        };
        let before = addresses(self);
        let cmd = ew_api::AnyCommand {
            name: name.to_owned(),
            params,
        };
        let out = self.run_commit(&cmd);
        let error = match &out {
            Outcome::Done => None,
            Outcome::Failed { message } => Some(message.clone()),
            Outcome::Refused { reason: reason_key } => Some(strings::render(reason_key)),
        };
        let after = addresses(self);
        let moved = out == Outcome::Done
            && before
                .iter()
                .any(|(m, a)| m.is_some() && !a.is_empty() && after.iter().any(|(n, b)| n == m && b != a));
        if let Some(tab) = self.station_tab_mut() {
            let said = error.is_some();
            tab.error = error.map(|e| (place, e));
            tab.note = moved.then(|| strings::text(key!("rack-addresses-moved")).to_owned());
            if said {
                self.said_at(crate::shell::said::Said::Station);
            }
        }
        out
    }

    /// `plc.rack_add {part, slot?, rack?, module?, position?}`: a part of the catalogue into the rack on show. A card
    /// goes into the free slot named, else the one the cursor stands on, else the first free one (`plc.add_module`, a
    /// library part embedded with it: as `rack.slot` in a station whose cards have rack positions, else after the
    /// station's cards); a valve into the position named, else the cursor's, else the first empty one
    /// (`valve.set_occupancy`). For a station not made yet a head part becomes its head and a card joins its cards.
    /// The cursor goes to what was added.
    pub(crate) fn act_plc_rack_add(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(view) = self.shown_rack() else {
            return Outcome::refused(key!("reason-rack-closed"));
        };
        let part = or_return!(self.catalogue_part(p, "part"));
        let cursor = self.station_tab().and_then(|t| t.cursor);
        let number = |name: &str| p.get(name).and_then(Value::as_u64);
        // A valve for a terminal: a position's.
        if part.plc.is_none() && part.terminal_valve {
            // A valve without a symbol that fits a cell would stand on its position undrawn.
            if !part.cell_symbol {
                return self.rack_refuse(
                    cursor,
                    strings::text(key!("reason-valve-no-cell-symbol")).to_owned(),
                );
            }
            let station = or_return!(self.rack_station());
            let named = match number("position") {
                Some(n) => Some((
                    or_return!(self.record_param(p, "module")),
                    u32::try_from(n).unwrap_or(u32::MAX),
                )),
                None => None,
            };
            // The position named, else the cursor's while it is empty, else the first empty one.
            let empty = |module: Uuid, position: u32| {
                view.cells
                    .iter()
                    .any(|c| c.module == module && c.position == position && c.at.holder.is_none())
            };
            let at = named
                .or(match cursor {
                    Some(StationCursor::Position { module, position }) if empty(module, position) => {
                        Some((module, position))
                    }
                    _ => None,
                })
                .or_else(|| {
                    view.cells
                        .iter()
                        .find(|c| c.at.holder.is_none())
                        .map(|c| (c.module, c.position))
                });
            let Some((module, position)) = at else {
                let tag = self
                    .project_session()
                    .and_then(|s| s.device_tag(station))
                    .unwrap_or_default();
                return self.rack_refuse(cursor, strings::format(key!("rack-no-free-position"), &[&tag]));
            };
            let place = StationCursor::Position { module, position };
            let occupancy = json!({ "valve": { "state": "used", "part": part.copy.to_string() } });
            let run = self.occupancy_params(module, position, occupancy);
            let out = self.rack_commit(Some(place), "valve.set_occupancy", run["params"].clone());
            if out == Outcome::Done {
                self.rack_cursor(Some(place));
            }
            return out;
        }
        let Some(plc) = &part.plc else {
            return failed(key!("error-bad-param"), &["part"]);
        };
        // A station not made yet: its head, then its cards.
        if let Some(d) = self.station_tab_mut().and_then(|t| t.draft.as_mut()) {
            let place = if plc.heads.is_empty() {
                d.modules.push(part.copy);
                StationCursor::Slot {
                    rack: 0,
                    slot: u32::try_from(d.modules.len()).unwrap_or(u32::MAX),
                }
            } else {
                // With its head chosen the station takes modules: the cursor goes to its first free slot, and the
                // catalogue lists the modules (on the head it lists the heads, to choose another).
                d.head = Some(part.copy);
                StationCursor::Slot {
                    rack: 0,
                    slot: u32::try_from(d.modules.len() + 1).unwrap_or(u32::MAX),
                }
            };
            if let Some(tab) = self.station_tab_mut() {
                tab.error = None;
                tab.cursor = Some(place);
            }
            return Outcome::Done;
        }
        let station = or_return!(self.rack_station());
        if !plc.heads.is_empty() {
            return self.rack_refuse(
                cursor,
                strings::format(key!("rack-part-no-module"), &[&part.part_number]),
            );
        }
        // The free slot named, else the cursor's, else the first: only a free slot the rack offers takes a card.
        let free = |rack: u16, slot: u32| {
            view.rows
                .iter()
                .any(|r| r.module.is_none() && r.rack == rack && r.slot == slot)
        };
        let named = number("slot").map(|s| {
            (
                number("rack").and_then(|r| u16::try_from(r).ok()).unwrap_or(0),
                u32::try_from(s).unwrap_or(u32::MAX),
            )
        });
        let at = named
            .or(match cursor {
                Some(StationCursor::Slot { rack, slot }) if free(rack, slot) => Some((rack, slot)),
                _ => None,
            })
            .or_else(|| {
                view.rows
                    .iter()
                    .find(|r| r.module.is_none())
                    .map(|r| (r.rack, r.slot))
            });
        let Some((rack, slot)) = at else {
            let why = if view.rows.is_empty() {
                key!("rack-takes-no-modules")
            } else {
                key!("rack-full")
            };
            return self.rack_refuse(cursor, strings::text(why).to_owned());
        };
        let place = StationCursor::Slot { rack, slot };
        if !free(rack, slot) {
            let shown = if rack > 0 {
                format!("{rack}.{slot}")
            } else {
                slot.to_string()
            };
            let taken = view
                .rows
                .iter()
                .find(|r| r.module.is_some() && r.rack == rack && r.slot == slot);
            let why = match taken {
                Some(taken) => strings::format(key!("rack-slot-taken"), &[&shown, &taken.tag]),
                None => strings::format(key!("rack-slot-not-free"), &[&shown]),
            };
            return self.rack_refuse(Some(place), why);
        }
        let copies = self
            .project_session()
            .map(|s| s.module_part_copies(part.copy))
            .unwrap_or_default();
        let mut params = json!({
            "station": station.to_string(),
            "part": {"part": {"copy": part.copy.to_string()}, "qty": "1"},
        });
        if view.racked || rack > 0 {
            params["slot"] = json!(slot);
            params["rack"] = json!(rack);
        }
        if !copies.is_empty() {
            params["copies"] = Value::Array(copies);
        }
        let out = self.rack_commit(Some(place), "plc.add_module", params);
        if out == Outcome::Done {
            self.rack_cursor(Some(place));
        }
        out
    }

    /// `plc.rack_move {module, slot}`: a module of the station on show into the slot named (`3`, `<rack>.<slot>`), as
    /// a drag does. On another module's slot the two change places (modules that stand in a row without slot numbers of
    /// their own swap their positions with one `assembly.attach`, others their slots with one `fields.set_many`); a
    /// free slot takes it (`fields.set plc_module.slot`). EW-PLC-011 refuses a slot its part does not allow; the
    /// refusal stays at that slot.
    pub(crate) fn act_plc_rack_move(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.rack_station());
        let module = or_return!(self.record_param(p, "module"));
        let text = match string_param(p, "slot") {
            Ok(Some(t)) => t.trim().to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["slot"]),
            Err(e) => return e,
        };
        let numbers: Vec<Option<u16>> = text.split('.').map(|n| n.trim().parse::<u16>().ok()).collect();
        let (rack, slot) = match numbers.as_slice() {
            [Some(slot)] => (0, u32::from(*slot)),
            [Some(rack), Some(slot)] => (*rack, u32::from(*slot)),
            _ => {
                let cursor = self.station_tab().and_then(|t| t.cursor);
                return self.rack_refuse(cursor, strings::format(key!("rack-bad-slot"), &[&text]));
            }
        };
        let place = StationCursor::Slot { rack, slot };
        let Some(view) = self.rack_view(station) else {
            return failed(key!("error-bad-param"), &["station"]);
        };
        let Some(row) = view.rows.iter().find(|r| r.module == Some(module)) else {
            return failed(key!("error-bad-param"), &["module"]);
        };
        let other = view
            .rows
            .iter()
            .find(|r| r.rack == rack && r.slot == slot && r.module.is_some());
        if other.is_some_and(|o| o.module == Some(module)) {
            return Outcome::Done;
        }
        let set = |m: Uuid, rack: u16, slot: u32| {
            json!({"kind": "device", "targets": [m.to_string()], "path": "plc_module.slot",
                "value": {"rack": {"rack": rack, "slot": slot}}})
        };
        let out = match other {
            Some(o) if row.positional && o.positional => {
                let q = self.project_session().map(ew_api::Session::query);
                let position = |m: Uuid| {
                    q.as_ref().and_then(|q| {
                        q.devices()
                            .get(m.to_string().parse().ok()?)
                            .and_then(|d| d.position)
                    })
                };
                let other = o.module.unwrap_or_default();
                let (Some(a), Some(b)) = (position(module), position(other)) else {
                    return Outcome::Done;
                };
                self.rack_commit(
                    Some(place),
                    "assembly.attach",
                    json!({"parent": station.to_string(), "children": [
                        {"device": module.to_string(), "position": b},
                        {"device": other.to_string(), "position": a}]}),
                )
            }
            Some(o) => self.rack_commit(
                Some(place),
                "fields.set_many",
                json!({"edits": [
                    set(module, rack, slot),
                    set(o.module.unwrap_or_default(), row.rack, row.slot)]}),
            ),
            None => self.rack_commit(Some(place), "fields.set", set(module, rack, slot)),
        };
        if out == Outcome::Done {
            self.rack_cursor_on(module);
        }
        out
    }

    /// Puts the cursor on the place of device `card` in the rack on show, when it has one.
    fn rack_cursor_on(&mut self, card: Uuid) {
        let place = self.shown_rack().and_then(|v| {
            v.rows
                .iter()
                .find(|r| r.module == Some(card))
                .map(|r| StationCursor::Slot {
                    rack: r.rack,
                    slot: r.slot,
                })
        });
        if place.is_some() {
            self.rack_cursor(place);
        }
    }

    /// `plc.rack_slot {op, slot?, rack?, part?, folio?}`: the card in a slot of the rack on show, the one named, else
    /// the cursor's. `up` and `down` move it one slot left or right (it changes places with the card there, or takes
    /// the free slot); `empty` takes it out (a card drawn nowhere is deleted, a drawn one leaves the rack with
    /// `plc.remove_from_rack` and keeps its drawing); `replace` gives it the part named (`fields.set parts`); `draw`
    /// draws its box on the schematic page `folio` (`new`: a new page; absent: the page the view was opened from),
    /// bound to its channels (`plc.draw_station`), and shows that page. With the cursor on a valve position, `empty`
    /// empties it, `blank` puts a blanking plate on it and `replace` a valve of the part (`valve.set_occupancy`). For a
    /// station not made yet the draft changes.
    pub(crate) fn act_plc_rack_slot(&mut self, p: &Map<String, Value>) -> Outcome {
        const OPS: [&str; 6] = ["up", "down", "empty", "replace", "draw", "blank"];
        let op = match string_param(p, "op") {
            Ok(Some(o)) if OPS.contains(&o) => o.to_owned(),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["op"]),
            Ok(None) => return failed(key!("error-missing-param"), &["op"]),
            Err(e) => return e,
        };
        let Some(tab) = self.station_tab() else {
            return Outcome::refused(key!("reason-rack-closed"));
        };
        let cursor = match p.get("slot").and_then(Value::as_u64) {
            Some(slot) => Some(StationCursor::Slot {
                rack: p
                    .get("rack")
                    .and_then(Value::as_u64)
                    .and_then(|r| u16::try_from(r).ok())
                    .unwrap_or(0),
                slot: u32::try_from(slot).unwrap_or(u32::MAX),
            }),
            None => tab.cursor,
        };
        let Some(cursor) = cursor else {
            return Outcome::refused(key!("reason-rack-no-module"));
        };
        let part = if op == "replace" {
            Some(or_return!(self.catalogue_part(p, "part")))
        } else {
            None
        };
        // A station not made yet: its head and its cards are the draft's.
        if let Some(d) = self.station_tab_mut().and_then(|t| t.draft.as_mut()) {
            let StationCursor::Slot { slot, .. } = cursor else {
                match (op.as_str(), part) {
                    ("empty", _) => d.head = None,
                    ("replace", Some(x)) if x.plc.as_ref().is_some_and(|x| !x.heads.is_empty()) => {
                        d.head = Some(x.copy);
                    }
                    _ => return Outcome::refused(key!("reason-rack-draft")),
                }
                return Outcome::Done;
            };
            let i = usize::try_from(slot).unwrap_or(usize::MAX).wrapping_sub(1);
            if i >= d.modules.len() {
                return failed(key!("error-bad-param"), &["slot"]);
            }
            let mut to = i;
            match op.as_str() {
                "up" if i > 0 => {
                    d.modules.swap(i, i - 1);
                    to = i - 1;
                }
                "down" if i + 1 < d.modules.len() => {
                    d.modules.swap(i, i + 1);
                    to = i + 1;
                }
                "empty" => {
                    d.modules.remove(i);
                }
                "replace" => d.modules[i] = part.map_or(d.modules[i], |x| x.copy),
                "draw" | "blank" => return Outcome::refused(key!("reason-rack-draft")),
                _ => {}
            }
            if let Some(tab) = self.station_tab_mut() {
                tab.error = None;
                tab.cursor = Some(StationCursor::Slot {
                    rack: 0,
                    slot: u32::try_from(to + 1).unwrap_or(u32::MAX),
                });
            }
            return Outcome::Done;
        }
        let station = or_return!(self.rack_station());
        let Some(view) = self.rack_view(station) else {
            return failed(key!("error-bad-param"), &["station"]);
        };
        let (rack, slot) = match cursor {
            StationCursor::Slot { rack, slot } => (rack, slot),
            // A valve position: emptied, blanked or given another valve.
            StationCursor::Position { module, position } => {
                let occupancy = match (op.as_str(), &part) {
                    ("empty", _) => json!("empty"),
                    ("blank", _) => json!({ "blank": {} }),
                    ("replace", Some(x)) => {
                        json!({ "valve": { "state": "used", "part": x.copy.to_string() } })
                    }
                    _ => return Outcome::refused(key!("reason-rack-position")),
                };
                let run = self.occupancy_params(module, position, occupancy);
                return self.rack_commit(Some(cursor), "valve.set_occupancy", run["params"].clone());
            }
            StationCursor::Head => {
                return match op.as_str() {
                    "draw" => self.draw_from_rack(station, Some(station), p),
                    _ => Outcome::refused(key!("reason-rack-head")),
                };
            }
        };
        let modules: Vec<&ew_api::output::RackRow> =
            view.rows.iter().filter(|r| r.module.is_some()).collect();
        let Some(row) = modules.iter().find(|r| r.slot == slot && r.rack == rack) else {
            return Outcome::refused(key!("reason-rack-no-module"));
        };
        let module = row.module.unwrap_or_default();
        match op.as_str() {
            "up" | "down" => {
                let target = if op == "up" {
                    slot.checked_sub(1)
                } else {
                    slot.checked_add(1)
                };
                let Some(target) = target.filter(|t| *t > 0) else {
                    return Outcome::Done;
                };
                // A card in a row without slot numbers stays inside the row of cards.
                if row.positional && !modules.iter().any(|r| r.rack == rack && r.slot == target) {
                    return Outcome::Done;
                }
                let to = if rack > 0 {
                    format!("{rack}.{target}")
                } else {
                    target.to_string()
                };
                self.act_plc_rack_move(&Map::from_iter([
                    ("module".to_owned(), Value::String(module.to_string())),
                    ("slot".to_owned(), Value::String(to)),
                ]))
            }
            "empty" if row.drawn => self.rack_commit(
                Some(cursor),
                "plc.remove_from_rack",
                json!({"device": module.to_string()}),
            ),
            "empty" => self.rack_commit(
                Some(cursor),
                "edit.delete",
                json!({"targets": [{"kind": "device", "id": module.to_string()}]}),
            ),
            "replace" => {
                let Some(part) = part else {
                    return Outcome::refused(key!("reason-rack-no-part"));
                };
                let copies = self
                    .project_session()
                    .map(|s| s.module_part_copies(part.copy))
                    .unwrap_or_default();
                self.rack_commit(
                    Some(cursor),
                    "fields.set",
                    json!({"kind": "device", "targets": [module.to_string()], "path": "parts",
                        "value": [{"part": {"copy": part.copy.to_string()}, "qty": "1"}], "copies": copies}),
                )
            }
            "blank" => Outcome::refused(key!("reason-rack-position")),
            _ => self.draw_from_rack(station, Some(module), p),
        }
    }

    /// `plc.rack_create`: Create of a station not made yet: the station and its cards, drawn nowhere yet, with one
    /// `plc.add_station` (library parts embedded with it), one undo step; the view then shows the station's rack, and
    /// its To do list what comes next.
    pub(crate) fn act_plc_rack_create(&mut self) -> Outcome {
        let Some(d) = self.station_draft().cloned() else {
            return Outcome::refused(key!("reason-rack-made"));
        };
        let Some(head) = d.head else {
            return self.rack_refuse(
                Some(StationCursor::Head),
                strings::text(super::StationDraft::NO_HEAD).to_owned(),
            );
        };
        let session = &or_return!(crate::shell::gated(&self.project)).session;
        let mut copies: Vec<Value> = Vec::new();
        for part in std::iter::once(&head).chain(&d.modules) {
            for doc in session.module_part_copies(*part) {
                if !copies.contains(&doc) {
                    copies.push(doc);
                }
            }
        }
        let stations = |s: &ew_api::Session| -> Vec<Uuid> {
            s.query()
                .devices()
                .iter()
                .filter(|(_, x)| x.aspects.plc_station.is_some())
                .map(|(id, _)| id.as_uuid())
                .collect()
        };
        let before = stations(session);
        let use_of = |c: &Uuid| json!({"part": {"copy": c.to_string()}, "qty": "1"});
        let mut params = json!({
            "part": use_of(&head),
            "modules": d.modules.iter().map(|m| json!({"part": use_of(m)})).collect::<Vec<_>>(),
            "copies": copies,
        });
        if let Some(c) = d.config {
            params["config_project"] = json!(c.to_string());
        }
        // The tag typed in the form, frozen (it was read when it was typed).
        if let Some(tag) = &d.tag
            && let Ok(parsed) = session.parse_field_input("device", "tag.frozen", tag)
        {
            params["tag"] = parsed.value;
        }
        let out = self.rack_commit(Some(StationCursor::Head), "plc.add_station", params);
        if out != Outcome::Done {
            return out;
        }
        let made = self
            .project_session()
            .and_then(|s| stations(s).into_iter().find(|x| !before.contains(x)));
        if let Some(tab) = self.station_tab_mut() {
            tab.station = made;
            tab.draft = None;
        }
        self.last_station = made;
        self.rack_cursor(Some(StationCursor::Head));
        out
    }

    /// `plc.rack_rename {device?, plc?, text?, cancel?}`: a tag of the station on show changed in place, as the
    /// Tag row of Properties changes it: the station's in the view's header, a card's in its row (`device`; absent,
    /// the device under the cursor, else the station). Without `text` the field opens on the tag; with `text` the typed
    /// tag goes through the Tag field's own path ([`ShellModel::tag_rename_commit`]): taken, the field closes; refused
    /// (a tag in use), the sentence stands under the rack and the field stays open on the text as typed. `cancel`
    /// closes the field.
    pub(crate) fn act_plc_rack_rename(&mut self, p: &Map<String, Value>) -> Outcome {
        if flag(p, "cancel") {
            if let Some(tab) = self.station_tab_mut() {
                tab.rename = None;
            }
            return Outcome::Done;
        }
        if self.station_draft().is_some() {
            return self.rename_draft(p);
        }
        let station = or_return!(self.rack_station());
        // The PLC's own name, beside the station's tag in the header.
        let plc = self.station_shown().and_then(|s| s.rack.plc_project);
        let renaming_plc = self
            .station_tab()
            .and_then(|t| t.rename.as_ref())
            .is_some_and(|r| Some(r.0) == plc);
        let named = p
            .get("device")
            .and_then(Value::as_str)
            .and_then(|d| d.parse::<Uuid>().ok());
        if flag(p, "plc") || (renaming_plc && named.is_none_or(|d| Some(d) == plc)) {
            return self.rename_station_plc(plc, p);
        }
        let device = match p.get("device") {
            Some(_) => or_return!(self.record_param(p, "device")),
            None => self
                .station_tab()
                .and_then(|t| t.rename.as_ref().map(|r| r.0))
                .or_else(|| self.station_device())
                .unwrap_or(station),
        };
        // The station, a card in its slots, a valve on its positions.
        let of_station = self.station_shown().is_some_and(|s| {
            std::iter::once(&s.rack.head)
                .chain(&s.rack.rows)
                .filter_map(|r| r.module)
                .chain(s.rack.cells.iter().filter_map(|c| c.at.holder))
                .any(|d| d == device)
        });
        if !of_station {
            return failed(key!("error-bad-param"), &["device"]);
        }
        let text = match string_param(p, "text") {
            Ok(t) => t.map(str::to_owned),
            Err(e) => return e,
        };
        let place = if device == station {
            Some(StationCursor::Head)
        } else {
            self.station_tab().and_then(|t| t.cursor)
        };
        let Some(text) = text else {
            // The field opens on the station's head, with the station selected, as its Properties show it.
            if device == station {
                self.rack_cursor(Some(StationCursor::Head));
            }
            let Some(tag) = self.tag_rename_sheet(device) else {
                return Outcome::refused(key!("reason-nav-not-renamable"));
            };
            if let Some(tab) = self.station_tab_mut() {
                tab.rename = Some((device, tag));
                tab.error = None;
            }
            return Outcome::Done;
        };
        // A tag another device has is refused in a sentence, unless the device can join it (the Tag row proposes
        // that): the Counter row takes a card's counter without asking whether another card has it.
        let taken = self.project_session().is_some_and(|s| {
            s.resolve_reference(text.trim()).is_ok_and(|d| d != device)
                && s.join_proposal(device, &text).is_none()
        });
        if taken {
            if let Some(tab) = self.station_tab_mut() {
                tab.rename = Some((device, text.clone()));
            }
            let why = strings::format(key!("rack-rename-taken"), &[text.trim()]);
            return self.rack_refuse(place, why);
        }
        let mut out = self.tag_rename_commit(device, &text);
        // A tag that changes other tags too (a station's cards follow its tag) is previewed in Properties as the Tag
        // row's change is; the Enter in the view takes it at once (Properties and the Preview each offered an
        // Apply for it, and the line that asked for one stayed after it).
        if out == Outcome::Done && self.props.card().is_some() {
            out = self.act_preview_apply();
        }
        if out == Outcome::Done {
            if let Some(tab) = self.station_tab_mut() {
                tab.rename = None;
                tab.error = None;
                tab.note = None;
            }
            return out;
        }
        let why = match (self.props.error(), out) {
            (Some((_, message)), _) => message.to_owned(),
            (None, Outcome::Failed { message }) => message,
            (None, Outcome::Refused { reason: reason_key }) => strings::render(&reason_key),
            (None, Outcome::Done) => String::new(),
        };
        if let Some(tab) = self.station_tab_mut() {
            tab.rename = Some((device, text));
        }
        self.rack_refuse(place, why)
    }

    /// `plc.rack_rename {text?}` on the New station form (it had no tag): the tag the station gets, typed before
    /// Create, which gives it frozen. Without `text` the field opens (the nil ID standing for the station not made
    /// yet); an empty text leaves the tag to its page; a tag another device has, or one a tag cannot be, is refused in
    /// a sentence under the rack, the field staying open on the text as typed.
    fn rename_draft(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match string_param(p, "text") {
            Ok(t) => t.map(str::to_owned),
            Err(e) => return e,
        };
        let open = |s: &mut Self, text: Option<String>| {
            if let Some(tab) = s.station_tab_mut() {
                tab.rename = text.map(|t| (Uuid::nil(), t));
                tab.error = None;
            }
        };
        let Some(text) = text else {
            let typed = self
                .station_draft()
                .and_then(|d| d.tag.clone())
                .unwrap_or_default();
            open(self, Some(typed));
            return Outcome::Done;
        };
        let tag = text.trim().to_owned();
        let refuse = |s: &mut Self, why: String| {
            open(s, Some(text.clone()));
            s.rack_refuse(Some(StationCursor::Head), why)
        };
        if !tag.is_empty() {
            let session = &or_return!(crate::shell::gated(&self.project)).session;
            if session.resolve_reference(&tag).is_ok() {
                return refuse(self, strings::format(key!("rack-rename-taken"), &[&tag]));
            }
            match session.parse_field_input("device", "tag.frozen", &tag) {
                Ok(_) => {}
                Err(ew_api::output::InputError::Invalid(why)) => {
                    return refuse(
                        self,
                        crate::shell::tools::rejected_words(&why.to_string(), Some(&why)),
                    );
                }
                Err(_) => return refuse(self, strings::format(key!("rack-channel-bad"), &[&tag])),
            }
        }
        if let Some(tab) = self.station_tab_mut()
            && let Some(draft) = tab.draft.as_mut()
        {
            draft.tag = (!tag.is_empty()).then_some(tag);
        }
        open(self, None);
        Outcome::Done
    }

    /// `plc.rack_rename {plc: true, text?}`: the name of the PLC the station on show belongs to (its
    /// configuration project, `plc`), changed in place in the view's header as its Properties change it
    /// (`fields.set name`): without `text` the field opens on the name; a name another PLC of the project has, or one a
    /// PLC's name cannot be, is refused in a sentence under the rack, the field staying open on the text as typed.
    fn rename_station_plc(&mut self, plc: Option<Uuid>, p: &Map<String, Value>) -> Outcome {
        let Some(plc) = plc else {
            return Outcome::refused(key!("reason-rack-draft"));
        };
        let text = match string_param(p, "text") {
            Ok(t) => t.map(str::to_owned),
            Err(e) => return e,
        };
        let current = self
            .station_shown()
            .map(|s| s.rack.plc.clone())
            .unwrap_or_default();
        let open = |s: &mut Self, text: Option<String>| {
            if let Some(tab) = s.station_tab_mut() {
                tab.rename = text.map(|t| (plc, t));
            }
        };
        let Some(text) = text else {
            open(self, Some(current));
            if let Some(tab) = self.station_tab_mut() {
                tab.error = None;
            }
            return Outcome::Done;
        };
        let name = text.trim().to_owned();
        if name == current {
            open(self, None);
            return Outcome::Done;
        }
        let refuse = |s: &mut Self, why: String| {
            open(s, Some(text.clone()));
            s.rack_refuse(None, why)
        };
        if self
            .plc_config_projects()
            .iter()
            .any(|(id, n)| *id != plc && *n == name)
        {
            return refuse(self, strings::format(key!("rack-rename-plc-taken"), &[&name]));
        }
        let parsed = match self.project_session() {
            Some(session) => session.parse_field_input("plc_config_project", "name", &name),
            None => return Outcome::refused(key!("reason-no-project")),
        };
        let value = match parsed {
            Ok(parsed) => parsed.value,
            Err(ew_api::output::InputError::Invalid(why)) => {
                return refuse(
                    self,
                    crate::shell::tools::rejected_words(&why.to_string(), Some(&why)),
                );
            }
            Err(_) => return refuse(self, strings::format(key!("rack-rename-plc-bad"), &[&name])),
        };
        let cmd = ew_api::AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "plc_config_project", "targets": [plc.to_string()], "path": "name", "value": value}),
        };
        match self.commit_from_host(&cmd) {
            Outcome::Done => {
                open(self, None);
                if let Some(tab) = self.station_tab_mut() {
                    tab.error = None;
                }
                Outcome::Done
            }
            Outcome::Failed { message } => refuse(self, message),
            Outcome::Refused { reason: reason_key } => refuse(self, strings::render(&reason_key)),
        }
    }

    /// `plc.rack_channel {channel?, column?, text?, next?, cancel?}` (a symbol typed into the channel table did
    /// nothing): a channel's symbol (`column` `symbol`) or function text (`function_text`) of the module on show, or
    /// of any channel of the project, set as Properties set them ([`ShellModel::set_channel_text`]). Without
    /// `text` the field opens on the
    /// cell; with it the cell takes the text, and with `next` the same cell of the next channel opens, so a card's
    /// channels are typed in one go. The function text is the wired device's (the PLC overview prints that): a channel
    /// nothing is wired to has none, said in a sentence. A refusal is said under the rack, the field staying open on
    /// the text as typed. Without `channel` it acts on the field open, else on the cell the keyboard cursor stands on
    /// (F2, `StationTab::channel_cursor`), which a cell of the module on show takes.
    pub(crate) fn act_plc_rack_channel(&mut self, p: &Map<String, Value>) -> Outcome {
        let open = self.station_tab().and_then(|t| t.channel.clone());
        let cursor = self.station_channel_cursor();
        if flag(p, "cancel") {
            if let Some(tab) = self.station_tab_mut() {
                tab.channel = None;
            }
            return Outcome::Done;
        }
        let column = match string_param(p, "column") {
            Ok(Some("symbol")) => "symbol",
            Ok(Some("function_text")) => "function_text",
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["column"]),
            Ok(None) => open
                .as_ref()
                .map(|o| o.1)
                .or(cursor.map(|c| c.1))
                .unwrap_or("symbol"),
            Err(e) => return e,
        };
        let channel = match (p.get("channel"), &open, cursor) {
            (Some(_), _, _) => or_return!(self.record_param(p, "channel")),
            (None, Some(o), _) => o.0,
            (None, None, Some(c)) => c.0,
            (None, None, None) => return failed(key!("error-missing-param"), &["channel"]),
        };
        // The channel of the module on show, else any channel of the project (the PLC grid's cell).
        let rows = self
            .station_shown()
            .map(|s| s.channels.clone())
            .unwrap_or_default();
        let at = rows.iter().position(|c| c.channel == channel);
        let Some(row) = at
            .map(|i| rows[i].clone())
            .or_else(|| self.project_session()?.channel_row(channel))
        else {
            return failed(key!("error-bad-param"), &["channel"]);
        };
        let row = &row;
        let text = match string_param(p, "text") {
            Ok(t) => t.map(str::to_owned),
            Err(e) => return e,
        };
        // The keyboard cursor goes with the cell of the module on show that is acted on, and stays there.
        if at.is_some()
            && let Some(tab) = self.station_tab_mut()
        {
            tab.channel_cursor = Some((channel, column));
        }
        let set = |s: &mut Self, field: Option<(Uuid, &'static str, String)>| {
            if let Some(tab) = s.station_tab_mut() {
                if let Some((channel, column, _)) = &field {
                    tab.channel_cursor = Some((*channel, *column));
                }
                tab.channel = field;
            }
        };
        // The function text is the wired device's.
        if column == "function_text" && row.device.is_none() {
            set(self, None);
            return self.rack_refuse(None, strings::text(key!("rack-channel-no-device")).to_owned());
        }
        let Some(text) = text else {
            let current = if column == "symbol" {
                row.symbol.clone()
            } else {
                row.function_text.clone()
            };
            set(self, Some((channel, column, current)));
            if let Some(tab) = self.station_tab_mut() {
                tab.error = None;
            }
            return Outcome::Done;
        };
        if let Err(why) = self.set_channel_text(row, column, &text) {
            set(self, Some((channel, column, text.clone())));
            return self.rack_refuse(None, why);
        }
        if let Some(tab) = self.station_tab_mut() {
            tab.error = None;
        }
        // Tab: the same cell of the next channel that has it, else the field closes.
        let next = at.filter(|_| flag(p, "next")).and_then(|at| {
            let has = |c: &&ew_api::output::RackChannel| column == "symbol" || c.device.is_some();
            rows.iter().skip(at + 1).find(has).cloned()
        });
        match next {
            Some(n) => {
                let opens = if column == "symbol" {
                    n.symbol
                } else {
                    n.function_text
                };
                set(self, Some((n.channel, column, opens)));
            }
            None => set(self, None),
        }
        Outcome::Done
    }

    /// Sets the symbol (`column` `symbol`) or the function text (`function_text`) of the channel of `row` to `text`, as
    /// the PLC grid and Properties set them (`fields.set`): the symbol on the channel, the function text on the device
    /// wired to it, in the project's language (its other languages stay). The station view's channel table types them
    /// through here; the PLC grid's gestures set the same field (`shell/table.rs`). A text equal to the one shown
    /// commits nothing. The refusal in words.
    pub(in crate::shell) fn set_channel_text(
        &mut self,
        row: &ew_api::output::RackChannel,
        column: &str,
        text: &str,
    ) -> Result<(), String> {
        let target = match (column, row.device) {
            ("symbol", _) => ("channel", row.channel),
            (_, Some(device)) => ("device", device),
            (_, None) => return Err(strings::text(key!("rack-channel-no-device")).to_owned()),
        };
        let current = if column == "symbol" {
            &row.symbol
        } else {
            &row.function_text
        };
        if text.trim() == current.as_str() {
            return Ok(());
        }
        let parsed = match self.project_session() {
            Some(session) => session.parse_field_input(target.0, column, text.trim()),
            None => return Err(strings::text(key!("reason-no-project")).to_owned()),
        };
        let leaf = match parsed {
            Ok(parsed) => parsed.value,
            Err(ew_api::output::InputError::Invalid(why)) => {
                return Err(crate::shell::tools::rejected_words(&why.to_string(), Some(&why)));
            }
            Err(_) => return Err(strings::format(key!("rack-channel-bad"), &[text.trim()])),
        };
        // A function text is one language of the device's: the others stay (as the grid composes it).
        let value = if column == "function_text" {
            let base = self.member_base(target.0, target.1, column);
            ew_api::output::table_edit::compose_leaf(column, column, &base, &leaf, true).unwrap_or(leaf)
        } else {
            leaf
        };
        let cmd = ew_api::AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": target.0, "targets": [target.1.to_string()], "path": column, "value": value}),
        };
        match self.commit_from_host(&cmd) {
            Outcome::Done => Ok(()),
            Outcome::Failed { message } => Err(message),
            Outcome::Refused { reason: reason_key } => Err(strings::render(&reason_key)),
        }
    }

    /// `plc.add_to_plc {device, config?}`: a bus device that belongs to no PLC becomes a station
    /// of the PLC `config` (absent: a new one) with one `plc.add_station`, and its view opens, so its cards are
    /// added there; a network switch has no rack, and a toast says which PLC it belongs to now. A module, a station and
    /// a device without an addressed bus port are refused in words.
    pub(crate) fn act_plc_add_to_plc(&mut self, p: &Map<String, Value>) -> Outcome {
        let device = or_return!(self.record_param(p, "device"));
        let config = match p.get("config") {
            Some(_) => Some(or_return!(self.record_param(p, "config"))),
            None => None,
        };
        let session = &or_return!(crate::shell::gated(&self.project)).session;
        let tag = session.device_tag(device).unwrap_or_default();
        if !session.bus_device(device) {
            return failed(key!("error-not-a-bus-device"), &[&tag]);
        }
        let mut params = json!({ "device": device.to_string() });
        if let Some(c) = config {
            params["config_project"] = json!(c.to_string());
        }
        let out = self.run_commit(&ew_api::AnyCommand {
            name: "plc.add_station".into(),
            params,
        });
        if out != Outcome::Done {
            return out;
        }
        // Chosen in a new station's view, that view is the device's from here on.
        let draft = self.station_draft().is_some();
        // A network switch belongs to the PLC with no rack of its own: a toast says so, with Undo.
        let switch = self.project_session().is_some_and(|s| s.network_switch(device));
        if switch {
            let plc = match self.project_session().map(|s| s.plc_offer(device)) {
                Some(ew_api::output::PlcOffer::Station(name)) => name,
                _ => String::new(),
            };
            self.notify(strings::format(key!("toast-plc-joined"), &[&tag, &plc]), true);
            if draft && let Some(a) = &mut self.area {
                let _ = a.close_tab();
            }
            return Outcome::Done;
        }
        if draft && let Some(tab) = self.station_tab_mut() {
            *tab = StationTab {
                station: Some(device),
                ..StationTab::default()
            };
            return Outcome::Done;
        }
        self.open_station(StationTab {
            station: Some(device),
            ..StationTab::default()
        })
    }
}
