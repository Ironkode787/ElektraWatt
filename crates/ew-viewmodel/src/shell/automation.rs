// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's entries of the PLC, bus and fluid commands (WP-S2.13): each handler builds one declared command
//! from the row or selection it is aimed at and commits it as one undo step, saying what it did in words.
//!
//! - **Attach cards without a station** (FT-1672): a CPU row of the PLC navigator attaches the cards that wait for a CPU
//!   (`Session::plc_cards_without_cpu`) with one `assembly.attach`, after the station's last child position.
//! - **The station view** (FR-PLC-028, doc 10 §6): a station's Rack… opens its view as a tab beside the page tabs
//!   (`area::StationTab`), which shows `Session::rack_view`. A part of the view's catalogue (the place catalogue's
//!   parts that fit the station) goes into a free slot with `plc.add_module` (a library part embedded in the same
//!   step); a card dragged to another slot moves there with `fields.set plc_module.slot`, or changes places with the
//!   card it is dropped on. The card under the cursor is the window's selection. EW-PLC-011's prevent half refuses a
//!   slot the part does not allow; the refusal stays at that slot. New station… is the same view on a station that
//!   is made by Create.
//! - **PLC exchange** (doc 10 §4, §6): Export… is a form of format and version, stations and document date that writes
//!   the file whole or not at all (`Session::plc_export`) and records it (`plc.record_exchange`); Import… reads an AML
//!   file or a TIA tag workbook into `Dialog::PlcImport`, the preview by tab (changed, conflicts, removed, parking)
//!   with the choices of its conflicts and parked objects, and Apply commits `plc.apply_exchange` under them.

use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, flag, string_param};
use crate::area::{StationCursor, StationTab};
use crate::key;
use crate::strings;
use crate::strings::Key;

impl ShellModel {
    /// A record named by parameter `name`: its UUID, or a reference (`tag:-201U1`, `id:<uuid>`).
    pub(super) fn record_param(&self, p: &Map<String, Value>, name: &str) -> Result<Uuid, Outcome> {
        let text = match string_param(p, name)? {
            Some(t) => t.trim(),
            None => return Err(failed(key!("error-missing-param"), &[name])),
        };
        if let Ok(id) = text.parse::<Uuid>() {
            return Ok(id);
        }
        let project = self.project.as_ref().ok_or(Outcome::Refused {
            reason_key: key!("reason-no-project"),
        })?;
        project
            .session
            .resolve_reference(text)
            .map_err(|_| failed(key!("error-bad-param"), &[name]))
    }

    /// `plc.attach_cards {station, card?}` (FT-1672): the cards that wait for a station (with `card`, that one of
    /// them) become children of `station`, after its last child position, with one `assembly.attach`; a toast names
    /// them.
    pub(crate) fn act_plc_attach_cards(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.record_param(p, "station"));
        let only = match p.get("card") {
            Some(_) => Some(or_return!(self.record_param(p, "card"))),
            None => None,
        };
        let Some(project) = self.project.as_ref() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        // A card dropped on a station's row (the persona run m3-p4): one that cannot go there says why.
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
                // Only a module goes into a rack (F17 of the rc.1 test: a bus node whose part has no PLC data was
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
}

/// A part the station view's catalogue offers: a row of the place catalogue ([`ew_api::output::CataloguePart`]) that
/// fits the station shown, as the view lists it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StationPart {
    /// The part copy's ID (a library part's copy ID once embedded).
    pub copy: Uuid,
    /// The part number.
    pub number: String,
    /// The part's description in words; empty when it has none.
    pub description: String,
    /// What it gives the PLC (`8 DI`); empty for a valve and a part that says neither channels nor I/O ranges.
    pub gives: String,
    /// The group it is listed under.
    pub group: ew_api::output::PlcGroup,
    /// Why it does not go where the cursor stands (a slot its part does not allow, a valve on a slot); `None` when
    /// it fits there.
    pub unfit: Option<String>,
}

/// What a station's view draws besides its tab ([`ShellModel::station_shown`]).
#[derive(Clone, PartialEq, Debug)]
pub struct StationShown {
    /// The rack: the station's, or the draft's.
    pub rack: ew_api::output::RackView,
    /// The catalogue's parts ([`ShellModel::station_catalogue`]).
    pub catalogue: Vec<StationPart>,
    /// The channels of the module under the cursor ([`ShellModel::station_channels`]).
    pub channels: Vec<ew_api::output::RackChannel>,
    /// What is left to do ([`ShellModel::station_todo`]).
    pub todo: Vec<StationTodo>,
}

/// What [`StationShown`] is made for: the revision, the language generation, the place catalogue's key and the
/// view's own state.
pub(crate) type StationKey = (u64, u64, Option<(u64, u64, String)>, StationTab);

impl StationPart {
    /// The part as one line of a choice: its number, its description and what it gives.
    pub fn words(&self) -> String {
        [&self.number, &self.description, &self.gives]
            .into_iter()
            .filter(|t| !t.is_empty())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// One line of the station view's To do list: a finding or a step that is left, in a sentence, with its fix.
#[derive(Clone, PartialEq, Debug)]
pub struct StationTodo {
    /// The sentence.
    pub text: String,
    /// Whether it is an error (a finding of the slot rule), not a step that is left.
    pub error: bool,
    /// The fix beside it.
    pub fix: Option<TodoFix>,
}

/// The fix beside a [`StationTodo`].
#[derive(Clone, PartialEq, Debug)]
pub enum TodoFix {
    /// A button that runs an action.
    Action {
        /// The button's words.
        label: String,
        /// The action.
        action: &'static str,
        /// Its parameters.
        params: Value,
    },
    /// Choose part…: the parts that fit the card's slot, one of which becomes its part.
    ChoosePart {
        /// The card's rack.
        rack: u16,
        /// The card's slot.
        slot: u32,
    },
    /// Draw on a page…: the schematic pages and New page…; the cards not drawn yet are drawn there.
    DrawOn,
    /// Put it on a bus page: the bus pages when there are several.
    BusPage,
}

impl ShellModel {
    /// The open project's session, for the dialogs' specs and describe lines.
    pub(crate) fn project_session(&self) -> Option<&ew_api::Session> {
        self.project.as_ref().map(|p| &p.session)
    }

    /// The view of `station` (doc 10 §6), as the window draws it: made once per revision and station (FT-1763).
    pub fn rack_view(&self, station: Uuid) -> Option<ew_api::output::RackView> {
        let rev = self.revision()?;
        {
            let mut cache = self.rack_cache.borrow_mut();
            if cache.0 != rev {
                *cache = (rev, Default::default());
            }
            if let Some(view) = cache.1.get(&station) {
                return view.clone();
            }
        }
        self.count(|c| c.rack_view += 1);
        let view = self.project_session()?.rack_view(station);
        self.rack_cache.borrow_mut().1.insert(station, view.clone());
        view
    }

    /// What the station view on show draws: its rack, its catalogue, the channels of the module under its cursor and
    /// what is left to do, made once for the revision, the language and the view's state (the window reads it every
    /// frame).
    pub fn station_shown(&self) -> Option<std::sync::Arc<StationShown>> {
        self.station_shown_of(self.station_tab()?)
    }

    /// [`ShellModel::station_shown`] for the station view of `tab`, which may stand in the group without the focus
    /// (side by side, it shows its lists too). The views of both groups are kept, so that neither is made every frame.
    pub fn station_shown_of(&self, tab: &StationTab) -> Option<std::sync::Arc<StationShown>> {
        let key: StationKey = (
            self.revision()?,
            strings::generation(),
            self.tools.catalogue.as_ref().map(|(k, _)| k.clone()),
            tab.clone(),
        );
        if let Some((_, shown)) = self.station_cache.borrow().iter().find(|(k, _)| *k == key) {
            return Some(std::sync::Arc::clone(shown));
        }
        let shown = std::sync::Arc::new(StationShown {
            rack: self.rack_of(tab)?,
            catalogue: self.catalogue_of(tab),
            channels: self.channels_of(tab),
            todo: self.todo_of(tab),
        });
        let mut cache = self.station_cache.borrow_mut();
        // One view per group of tabs: the oldest goes.
        if cache.len() >= 2 {
            cache.remove(0);
        }
        cache.push((key, std::sync::Arc::clone(&shown)));
        Some(shown)
    }

    /// What kind of valve terminal `device` is, in words (rc.2, F15): `Valve terminal · PLC station on PROFIBUS DP`,
    /// `Valve terminal · PLC station with bus node`, `Valve terminal · multipin, wired`; `None` for any other device.
    /// The terminal's menu and its row's hover in the Devices list say it ([`crate::properties::terminal_kind_text`]).
    pub fn terminal_kind_words(&self, device: Uuid) -> Option<String> {
        Some(crate::properties::terminal_kind_text(
            &self.project_session()?.terminal_kind(device)?,
        ))
    }

    /// The `command.run` parameters that set position `position` of valve module `module` to `occupancy`
    /// (`valve.set_occupancy`), with the copies a library valve part needs and the symbols that draw it in its cell.
    pub fn occupancy_params(&self, module: Uuid, position: u32, occupancy: Value) -> Value {
        let mut params =
            json!({ "module": module.to_string(), "position": position, "occupancy": occupancy });
        let part = params["occupancy"]["valve"]["part"]
            .as_str()
            .and_then(|p| p.parse::<Uuid>().ok());
        if let (Some(part), Some(session)) = (part, self.project_session()) {
            let copies = valve_copies(session, module, part);
            if !copies.is_empty() {
                params["copies"] = Value::Array(copies);
            }
        }
        json!({ "cmd": "valve.set_occupancy", "params": params })
    }

    /// The station view of the focused group's active tab; `None` while that tab shows a page.
    pub fn station_tab(&self) -> Option<&StationTab> {
        self.area.as_ref()?.current().station.as_ref()
    }

    fn station_tab_mut(&mut self) -> Option<&mut StationTab> {
        let area = self.area.as_mut()?;
        let focused = area.focused;
        area.groups[focused].active_tab_mut().station.as_mut()
    }

    /// The rack of a tab's station view: its station's (`rack_view`), or for a station not made yet the draft's
    /// (`Session::draft_rack`).
    pub fn rack_of(&self, tab: &StationTab) -> Option<ew_api::output::RackView> {
        match (&tab.draft, tab.station) {
            (Some(d), _) => Some(self.project_session()?.draft_rack(d.head, &d.modules)),
            (None, Some(s)) => self.rack_view(s),
            (None, None) => None,
        }
    }

    /// The rack the station view on show draws ([`ShellModel::rack_of`]).
    pub fn shown_rack(&self) -> Option<ew_api::output::RackView> {
        self.rack_of(self.station_tab()?)
    }

    /// The project's PLCs (configuration projects) by name, as a new station offers them.
    pub fn plc_config_projects(&self) -> Vec<(Uuid, String)> {
        self.project_session()
            .map(ew_api::Session::plc_config_projects)
            .unwrap_or_default()
    }

    /// Whether `device` is a valve terminal: its view offers the fluid page and the overview page.
    pub fn is_valve_terminal(&self, device: Uuid) -> bool {
        self.project_session()
            .is_some_and(|s| s.fluid_menu(device).terminal)
    }

    /// The station a module was last given to: the station chooser's first button.
    pub fn last_station(&self) -> Option<Uuid> {
        self.last_station
    }

    /// The station not made yet that the view on show builds; `None` for a made station and outside the view.
    pub fn station_draft(&self) -> Option<&StationDraft> {
        self.station_tab()?.draft.as_ref()
    }

    /// The devices already in the project that a new station offers as the station (rc.2 persona run): the bus
    /// devices in no PLC, those that lead a bus for a PLC (CPU), the others for every other kind; by tag.
    pub fn station_candidates(&self) -> Vec<(Uuid, String)> {
        let (Some(s), Some(d)) = (self.project_session(), self.station_draft()) else {
            return Vec::new();
        };
        let cpu = d.kind == StationKind::Cpu;
        s.free_bus_devices()
            .into_iter()
            .filter(|(_, _, leads)| *leads == cpu)
            .map(|(id, tag, _)| (id, tag))
            .collect()
    }

    /// The folio a page the station view makes takes, asking nothing: New page's proposal, else the first number no
    /// page has.
    pub(super) fn free_folio(&self) -> String {
        let (proposal, _) = self.new_page_proposal();
        if !proposal.is_empty() {
            return proposal;
        }
        let labels = self.page_labels();
        (1u64..)
            .map(|n| n.to_string())
            .find(|n| !labels.iter().any(|l| l == n))
            .unwrap_or_default()
    }

    /// The project's schematic pages (multi-line and single-line), which take a station's boxes: folio and title, in
    /// page order (Draw on a page… lists them).
    pub fn schematic_pages(&self) -> Vec<(String, String)> {
        self.page_rows()
            .iter()
            .filter(|r| matches!(r.kind.as_str(), "multi_line" | "single_line"))
            .map(|r| (r.folio.clone(), r.title.clone()))
            .collect()
    }

    /// The project's bus-topology pages: folio and title, in page order (Put it on a bus page asks which when there
    /// are several).
    pub fn bus_pages(&self) -> Vec<(String, String)> {
        self.page_rows()
            .iter()
            .filter(|r| r.kind == "bus_topology")
            .map(|r| (r.folio.clone(), r.title.clone()))
            .collect()
    }

    /// The catalogue of the station view on show: the place catalogue's parts (the device browser's) that fit the
    /// station, under the view's filter (the browser's matching), by group, the parts that fit where the cursor
    /// stands first. A station not made yet without a head part lists the parts that can head a station of its
    /// kind, then the cards; a valve terminal lists the valves for a terminal beside the cards.
    pub fn station_catalogue(&self) -> Vec<StationPart> {
        self.station_tab()
            .map(|tab| self.catalogue_of(tab))
            .unwrap_or_default()
    }

    /// [`ShellModel::station_catalogue`] of the station view of `tab`.
    fn catalogue_of(&self, tab: &StationTab) -> Vec<StationPart> {
        let Some((_, cat)) = self.tools.catalogue.as_ref() else {
            return Vec::new();
        };
        let view = self.rack_of(tab);
        let heads = tab
            .draft
            .as_ref()
            .filter(|d| d.head.is_none() || tab.cursor == Some(StationCursor::Head))
            .map(|d| d.kind.word());
        let terminal = view.as_ref().is_some_and(|v| !v.cells.is_empty());
        // A station whose part takes no modules has no slot, free or taken (a valve terminal of valve positions alone).
        let no_slots = view.as_ref().is_some_and(|v| v.rows.is_empty());
        let tokens = crate::tools::filter_tokens(&tab.filter);
        // A library PLC part that nothing draws is listed too, greyed with what to do (not left out silently).
        let mut out: Vec<(bool, StationPart)> = cat
            .parts
            .iter()
            .map(|p| (p, false))
            .chain(cat.plc_without_functions.iter().map(|p| (p, true)))
            .filter_map(|(p, undrawable)| {
                let (group, gives, allowed) = match (&p.plc, heads) {
                    (Some(plc), Some(kind)) if plc.heads.contains(&kind) => {
                        (plc.group, plc.gives.as_str(), &[][..])
                    }
                    (_, Some(_)) => return None,
                    (Some(plc), None) if plc.heads.is_empty() => {
                        (plc.group, plc.gives.as_str(), plc.allowed.as_slice())
                    }
                    (None, None) if p.terminal_valve && terminal => {
                        (ew_api::output::PlcGroup::Valve, "", &[][..])
                    }
                    _ => return None,
                };
                let mut texts = crate::tools::part_texts(p);
                texts.push(gives);
                if !crate::tools::tokens_hit(&tokens, &texts) {
                    return None;
                }
                let valve = group == ew_api::output::PlcGroup::Valve;
                let unfit = match tab.cursor {
                    _ if undrawable => Some(strings::text(key!("rack-part-no-functions")).to_owned()),
                    _ if valve && !p.cell_symbol => {
                        Some(strings::text(key!("reason-valve-no-cell-symbol")).to_owned())
                    }
                    Some(StationCursor::Slot { .. }) if valve => {
                        Some(strings::text(key!("rack-valve-on-slot")).to_owned())
                    }
                    _ if !valve && heads.is_none() && no_slots => {
                        Some(strings::text(key!("rack-takes-no-modules")).to_owned())
                    }
                    Some(StationCursor::Position { .. }) if !valve && heads.is_none() => {
                        Some(strings::text(key!("rack-module-on-position")).to_owned())
                    }
                    Some(StationCursor::Slot { slot, .. })
                        if !allowed.is_empty() && !allowed.iter().any(|a| u32::from(*a) == slot) =>
                    {
                        let slots: Vec<String> = allowed.iter().map(u16::to_string).collect();
                        Some(strings::format(
                            key!("rack-part-not-in-slot"),
                            &[&slot.to_string(), &slots.join(", ")],
                        ))
                    }
                    _ => None,
                };
                Some((
                    p.library.is_some(),
                    StationPart {
                        copy: p.copy,
                        number: p.part_number.clone(),
                        description: p.description.clone(),
                        gives: gives.to_owned(),
                        group,
                        unfit,
                    },
                ))
            })
            .collect();
        out.sort_by(|(la, a), (lb, b)| {
            (a.group, a.unfit.is_some(), la, &a.number).cmp(&(b.group, b.unfit.is_some(), lb, &b.number))
        });
        out.into_iter().map(|(_, p)| p).collect()
    }

    /// The device under the cursor of the station view on show: the station itself, the card in a slot, the valve
    /// or blanking plate on a position; `None` on a free place, for a station not made yet and without a cursor.
    pub fn station_device(&self) -> Option<Uuid> {
        self.device_of(self.station_tab()?)
    }

    /// [`ShellModel::station_device`] of the station view of `tab`.
    fn device_of(&self, tab: &StationTab) -> Option<Uuid> {
        tab.draft.is_none().then_some(())?;
        let view = self.rack_of(tab)?;
        match tab.cursor? {
            StationCursor::Head => tab.station,
            StationCursor::Slot { rack, slot } => {
                view.rows
                    .iter()
                    .find(|r| r.rack == rack && r.slot == slot)?
                    .module
            }
            StationCursor::Position { module, position } => {
                view.cells
                    .iter()
                    .find(|c| c.module == module && c.position == position)?
                    .at
                    .holder
            }
        }
    }

    /// The channels of the card under the cursor, in channel-number order (`Session::module_channels`).
    pub fn station_channels(&self) -> Vec<ew_api::output::RackChannel> {
        self.station_tab()
            .map(|tab| self.channels_of(tab))
            .unwrap_or_default()
    }

    /// [`ShellModel::station_channels`] of the station view of `tab`.
    fn channels_of(&self, tab: &StationTab) -> Vec<ew_api::output::RackChannel> {
        match (self.project_session(), self.device_of(tab)) {
            (Some(s), Some(card)) => s.module_channels(card),
            _ => Vec::new(),
        }
    }

    /// What is left to do on the station on show, each in a sentence with its fix: the slot rule's findings, a card
    /// without a part, the cards drawn on no page, a station on no bus page, a valve terminal on no fluid page; for a
    /// station not made yet, what Create still needs.
    pub fn station_todo(&self) -> Vec<StationTodo> {
        self.station_tab()
            .map(|tab| self.todo_of(tab))
            .unwrap_or_default()
    }

    /// [`ShellModel::station_todo`] of the station view of `tab`.
    fn todo_of(&self, tab: &StationTab) -> Vec<StationTodo> {
        let Some(session) = self.project_session() else {
            return Vec::new();
        };
        let Some(view) = self.rack_of(tab) else {
            return Vec::new();
        };
        let step = |text: String, fix: Option<TodoFix>| StationTodo {
            text,
            error: false,
            fix,
        };
        let mut out = Vec::new();
        if let Some(d) = &tab.draft {
            let text = match d.head {
                None => strings::text(key!("rack-choose-head")),
                Some(_) => strings::text(key!("rack-todo-create")),
            };
            out.push(step(text.to_owned(), None));
            return out;
        }
        let Some(station) = tab.station else {
            return out;
        };
        let lang = strings::rule_language();
        for f in &view.findings {
            // The finding's card, to go to.
            let card = f
                .targets
                .iter()
                .map(|t| t.id.as_uuid())
                .find(|id| view.rows.iter().any(|r| r.module == Some(*id)));
            out.push(StationTodo {
                text: ew_api::checks::messages::render(f, lang),
                error: true,
                fix: card.map(|c| TodoFix::Action {
                    label: strings::text(key!("rack-todo-show")).to_owned(),
                    action: "plc.rack_select",
                    params: json!({ "card": c.to_string() }),
                }),
            });
        }
        for r in view
            .rows
            .iter()
            .filter(|r| r.module.is_some() && r.part.is_empty())
        {
            out.push(step(
                strings::format(key!("rack-todo-no-part"), &[&r.tag]),
                Some(TodoFix::ChoosePart {
                    rack: r.rack,
                    slot: r.slot,
                }),
            ));
        }
        // A device drawn nowhere has a tag of question marks: its part number names it until it is drawn.
        let name = |r: &ew_api::output::RackRow| {
            let number = r.part.split(" · ").next().unwrap_or_default();
            if (r.tag.contains('?') || r.tag.is_empty()) && !number.is_empty() {
                number.to_owned()
            } else {
                r.tag.clone()
            }
        };
        let undrawn: Vec<String> = std::iter::once(&view.head)
            .chain(&view.rows)
            .filter(|r| r.module.is_some() && !r.drawn)
            .map(name)
            .collect();
        if !undrawn.is_empty() {
            out.push(step(
                strings::format(key!("rack-todo-undrawn"), &[&undrawn.join(", ")]),
                Some(TodoFix::DrawOn),
            ));
        }
        let on_bus_page = self
            .bus_pages()
            .iter()
            .any(|(folio, _)| session.undrawn_interfaces(station, folio).is_empty());
        if session.has_bus_interfaces(station) && !on_bus_page {
            out.push(step(
                strings::format(key!("rack-todo-bus"), &[&name(&view.head)]),
                Some(TodoFix::BusPage),
            ));
        }
        if self.is_valve_terminal(station) {
            let next = |label: Key<'static>, name: &str| TodoFix::Action {
                label: strings::text(label).to_owned(),
                action: "plc.rack_next",
                params: json!({ "step": name }),
            };
            if session
                .undrawn_fluid_terminals()
                .iter()
                .any(|(t, _)| *t == station)
            {
                out.push(step(
                    strings::format(key!("rack-todo-fluid"), &[&name(&view.head)]),
                    Some(next(key!("rack-next-fluid"), "fluid")),
                ));
            }
            // Drawn as its positions, with valves put on them without a symbol (rc.3).
            if session.valves_to_draw(station) {
                out.push(step(
                    strings::format(key!("rack-todo-valves"), &[&name(&view.head)]),
                    Some(TodoFix::Action {
                        label: strings::text(key!("action-fluid-draw-valves")).to_owned(),
                        action: "fluid.draw_valves",
                        params: json!({ "terminal": station.to_string() }),
                    }),
                ));
            }
            if !session.overview_drawn(station) {
                out.push(step(
                    strings::text(key!("rack-todo-overview")).to_owned(),
                    Some(next(key!("rack-next-overview"), "overview")),
                ));
            }
        }
        out
    }

    /// [`ShellModel::station_taken`] for the commands that change a slot (Replace, Remove, Move): a made station's
    /// head stays where it is, which is said in words.
    pub fn station_module(&self) -> Result<(), Key<'static>> {
        self.station_taken()?;
        match self.station_tab() {
            Some(tab) if tab.draft.is_none() && tab.cursor == Some(StationCursor::Head) => {
                Err(key!("reason-rack-head"))
            }
            _ => Ok(()),
        }
    }

    /// Whether the cursor of the station view on show stands on something (a module, the station's head, a valve or
    /// a blanking plate), and then whether that is drawn on a page; else why the PLC tab's Module commands have
    /// nothing to act on, in words.
    pub fn station_taken(&self) -> Result<bool, Key<'static>> {
        let tab = self.station_tab().ok_or(key!("reason-rack-closed"))?;
        let view = self.rack_of(tab).ok_or(key!("reason-rack-closed"))?;
        let (taken, drawn) = match tab.cursor {
            Some(StationCursor::Head) => (view.head.module.is_some(), view.head.drawn),
            Some(StationCursor::Slot { rack, slot }) => view
                .rows
                .iter()
                .find(|r| r.rack == rack && r.slot == slot)
                .map_or((false, false), |r| (r.module.is_some(), r.drawn)),
            Some(StationCursor::Position { module, position }) => view
                .cells
                .iter()
                .find(|c| c.module == module && c.position == position)
                .map_or((false, false), |c| (c.at.holder.is_some(), c.at.page.is_some())),
            None => (false, false),
        };
        match (taken, tab.cursor) {
            (true, _) => Ok(drawn),
            // A free slot or an empty position under the cursor: it takes a part from the catalogue.
            (false, Some(_)) => Err(key!("reason-rack-free")),
            (false, None) => Err(key!("reason-rack-no-module")),
        }
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

    /// The end of every dispatch for the station views: a tab whose station is gone (deleted, its making undone)
    /// closes.
    pub(super) fn drop_gone_stations(&mut self) {
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
    fn open_station(&mut self, tab: StationTab) -> Outcome {
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
        self.focus = self.area.as_ref().map(|a| super::Focus::Canvas(a.focused));
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
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let config = session.plc_config_projects().first().map(|(c, _)| *c);
        let out = self.open_station(StationTab {
            draft: Some(StationDraft {
                kind: kind.unwrap_or(StationKind::Cpu),
                head: None,
                config,
                modules: Vec::new(),
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
        // The view has the keys from here on: the arrows walk its places, Delete empties one.
        self.focus = self.area.as_ref().map(|a| super::Focus::Canvas(a.focused));
        let station = self
            .station_tab()
            .and_then(|t| t.station.filter(|_| t.draft.is_none()));
        let items = self
            .station_device()
            .or(station)
            .map(ew_api::output::ew_dl::EntityRef)
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
    fn rack_station(&self) -> Result<Uuid, Outcome> {
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
    fn rack_refuse(&mut self, place: Option<StationCursor>, why: String) -> Outcome {
        if let Some(tab) = self.station_tab_mut() {
            tab.error = Some((place, why.clone()));
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
            Outcome::Refused { reason_key } => Some(strings::text(*reason_key).to_owned()),
        };
        let after = addresses(self);
        let moved = out == Outcome::Done
            && before
                .iter()
                .any(|(m, a)| m.is_some() && !a.is_empty() && after.iter().any(|(n, b)| n == m && b != a));
        if let Some(tab) = self.station_tab_mut() {
            tab.error = error.map(|e| (place, e));
            tab.note = moved.then(|| strings::text(key!("rack-addresses-moved")).to_owned());
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
            // A valve without a symbol that fits a cell would stand on its position undrawn (rc.3).
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

    /// `plc.draw_station`'s parameters for `station` on `page` (`Session::station_draw_params`): each device not drawn
    /// yet a box 39.7 × 31.75 mm (wider where its pins need it), with its part's connection points laid out as the black box
    /// tool lays them (`tools::black_box::layout`), beside the one before it from the next free place, a new row under
    /// the first where the next box would pass the drawing area's right. A device whose part has no black-box entry is
    /// left out. `None` without the page shown, its frame or the catalogue.
    fn station_boxes(&mut self, station: Uuid, page: Uuid) -> Option<Value> {
        const SIZE: (i64, i64, i64) = (39_687_500, 31_750_000, 7_937_500);
        let mut params = self.project_session()?.station_draw_params(station, page)?;
        let grid = crate::tools::wire::PageGridMap::new(self.canvas()?.page().tables.grid);
        let cat = self.catalogue()?;
        let snap = |v: i64| grid.coord(grid.index(v));
        let at = |i: usize| params["at"][i].as_i64().unwrap_or(0);
        let (x0, y0, right) = (
            snap(at(0)),
            snap(at(1)),
            params["right"].as_i64().unwrap_or(i64::MAX),
        );
        let (mut x, mut y) = (x0, y0);
        let mut boxes = Vec::new();
        for d in params["devices"].as_array().cloned().unwrap_or_default() {
            let (Some(device), Some(part)) =
                (d[0].as_str(), d[1].as_str().and_then(|p| p.parse::<Uuid>().ok()))
            else {
                continue;
            };
            let Some(entry) = cat
                .parts
                .iter()
                .find(|r| r.copy == part)
                .and_then(|r| r.functions.iter().find(|f| f.black_box))
            else {
                continue;
            };
            if x > x0 && x + SIZE.0 > right {
                x = x0;
                y += SIZE.1 + 2 * SIZE.2;
            }
            let Some((pos, outline, cps)) = crate::tools::black_box::layout(
                grid,
                (x, y),
                (snap(x + SIZE.0), snap(y + SIZE.1)),
                &entry.cps,
                &[],
            ) else {
                continue;
            };
            let width = outline.iter().map(|p| p.0).max().unwrap_or(SIZE.0);
            boxes.push(json!({
                "device": device,
                "pos": [pos.0, pos.1],
                "box": {
                    "outline": outline.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
                    "cps": cps.iter().map(|(i, p, dir)| json!({"idx": i, "pos": [p.0, p.1], "dir": dir.word()}))
                        .collect::<Vec<_>>(),
                },
            }));
            x = snap(x + width + SIZE.2);
        }
        if let Some(m) = params.as_object_mut() {
            for k in ["at", "right", "devices"] {
                m.remove(k);
            }
            m.insert("boxes".into(), Value::Array(boxes));
        }
        Some(params)
    }

    /// Draws devices of `station` not drawn yet on a schematic page with one `plc.draw_station`: all of them (`only`
    /// absent: Draw on a page…), or the one device `only` (a card's Draw on a page…), each bound to its channels as a
    /// box drawn by hand is (normaliser N3). The page is `folio` of `p` (`new`: a new page, asking nothing), else the
    /// page the view was opened from; the page is shown with what was drawn. A refusal brings the view back and
    /// stays in it.
    fn draw_from_rack(&mut self, station: Uuid, only: Option<Uuid>, p: &Map<String, Value>) -> Outcome {
        let cursor = self.station_tab().and_then(|t| t.cursor);
        let folio = match string_param(p, "folio") {
            Ok(Some("new")) => {
                let folio = self.free_folio();
                let title = strings::text(key!("rack-draw-page-title"));
                let made = self.dispatch(
                    "page.new",
                    &json!({ "folio": folio, "type": "multi_line", "title": title }),
                );
                if made != Outcome::Done {
                    return made;
                }
                folio
            }
            Ok(Some(f)) if self.schematic_pages().iter().any(|(x, _)| x == f) => f.to_owned(),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["folio"]),
            Ok(None) => {
                let shown = self
                    .page_rows()
                    .get(self.page_index())
                    .filter(|r| matches!(r.kind.as_str(), "multi_line" | "single_line"))
                    .map(|r| r.folio.clone());
                match shown {
                    Some(folio) => folio,
                    None => {
                        return self
                            .rack_refuse(cursor, strings::text(key!("reason-rack-draw-page")).to_owned());
                    }
                }
            }
            Err(e) => return e,
        };
        let opened = self.dispatch("page.goto", &json!({ "folio": folio }));
        if opened != Outcome::Done {
            return opened;
        }
        // Back in the station's view, where a refusal is said.
        let back = |s: &mut Self, why: String| {
            let _ = s.open_station(StationTab {
                station: Some(station),
                ..StationTab::default()
            });
            s.rack_refuse(cursor, why)
        };
        let page = self.page_rows().get(self.page_index()).map(|r| r.id);
        let Some(mut params) = page.and_then(|page| self.station_boxes(station, page)) else {
            return back(self, strings::text(key!("reason-rack-no-frame")).to_owned());
        };
        if let (Some(module), Some(boxes)) = (only, params["boxes"].as_array_mut()) {
            boxes.retain(|b| b["device"].as_str() == Some(module.to_string().as_str()));
        }
        let out = self.run_commit(&ew_api::AnyCommand {
            name: "plc.draw_station".into(),
            params,
        });
        match out {
            Outcome::Done => Outcome::Done,
            Outcome::Failed { message } => back(self, message),
            Outcome::Refused { reason_key } => back(self, strings::text(reason_key).to_owned()),
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
                strings::text(key!("rack-choose-head")).to_owned(),
            );
        };
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
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

    /// `plc.rack_next {step, folio?}`: what is left to do for the station on show, from its To do list: `draw` draws
    /// the station and its cards not drawn yet on a schematic page ([`ShellModel::draw_from_rack`]); `fluid` draws a
    /// valve terminal on a fluid page (`fluid.draw_terminal`); `overview` makes a valve terminal's overview pages
    /// (`fluid.generate_overview`, previewed); `bus` opens a bus-topology page and puts the station's interfaces not
    /// drawn there on the pointer.
    pub(crate) fn act_plc_rack_next(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.rack_station());
        let cursor = self.station_tab().and_then(|t| t.cursor);
        match string_param(p, "step") {
            // A valve terminal on a fluid page: drawn as its positions there (rc.2).
            Ok(Some("fluid")) => {
                self.dispatch("fluid.draw_terminal", &json!({ "terminal": station.to_string() }))
            }
            // The station and its cards not drawn yet, in a row in slot order, one undo step (rc.2).
            Ok(Some("draw")) => self.draw_from_rack(station, None, p),
            Ok(Some("overview")) => self.act_fluid_generate_overview(&Map::from_iter([(
                "station".to_owned(),
                Value::String(station.to_string()),
            )])),
            Ok(Some("bus")) => {
                // The bus page named, else the only one; with none one is made, asking nothing; with several the
                // window asks which (rc.2 persona run: the button ended in "make one with New page first").
                // A station whose part has no bus interface cannot stand on a bus page: said in the view.
                let (tag, bus) = self.project_session().map_or((String::new(), false), |s| {
                    (
                        s.device_tag(station).unwrap_or_default(),
                        s.has_bus_interfaces(station),
                    )
                });
                if !bus {
                    return self.rack_refuse(cursor, strings::format(key!("rack-no-bus-interface"), &[&tag]));
                }
                let pages = self.bus_pages();
                let folio = match string_param(p, "folio") {
                    Ok(Some(f)) if pages.iter().any(|(x, _)| x == f) => f.to_owned(),
                    Ok(Some(_)) => return failed(key!("error-bad-param"), &["folio"]),
                    Err(e) => return e,
                    Ok(None) => match pages.as_slice() {
                        [(only, _)] => only.clone(),
                        [] => {
                            let folio = self.free_folio();
                            let title = strings::text(key!("rack-bus-page-title"));
                            let made = self.dispatch(
                                "page.new",
                                &json!({ "folio": folio, "type": "bus_topology", "title": title }),
                            );
                            if made != Outcome::Done {
                                return self.rack_refuse(
                                    cursor,
                                    strings::text(key!("reason-rack-no-bus-page")).to_owned(),
                                );
                            }
                            folio
                        }
                        _ => {
                            return self.rack_refuse(
                                cursor,
                                strings::text(key!("reason-rack-which-bus-page")).to_owned(),
                            );
                        }
                    },
                };
                // Every interface not drawn there, a box each (rc.2 persona run: the CPU stood there with its DP alone).
                let interfaces = self
                    .project_session()
                    .map(|s| s.undrawn_interfaces(station, &folio))
                    .unwrap_or_default();
                if interfaces.is_empty() {
                    return self
                        .rack_refuse(cursor, strings::format(key!("rack-bus-drawn"), &[&tag, &folio]));
                }
                let opened = self.dispatch("page.goto", &json!({ "folio": folio }));
                if opened != Outcome::Done {
                    return opened;
                }
                self.put_station_here(station)
            }
            Ok(_) => failed(key!("error-bad-param"), &["step"]),
            Err(e) => e,
        }
    }

    /// `plc.add_to_plc {device, config?}` (F19 of the rc.1 test): a bus device that belongs to no PLC becomes a station
    /// of the PLC `config` (absent: a new one) with one `plc.add_station`, and its view opens, so its cards are
    /// added there; a network switch has no rack, and a toast says which PLC it belongs to now. A module, a station and
    /// a device without an addressed bus port are refused in words.
    pub(crate) fn act_plc_add_to_plc(&mut self, p: &Map<String, Value>) -> Outcome {
        let device = or_return!(self.record_param(p, "device"));
        let config = match p.get("config") {
            Some(_) => Some(or_return!(self.record_param(p, "config"))),
            None => None,
        };
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
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
        // A network switch belongs to the PLC with no rack of its own: a toast says so, with Undo (rc.2 persona run).
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

    /// The describe lines of the station view on show: a draft's choices, the head, the slots, a valve terminal's
    /// positions, the cursor, the catalogue's parts, what is left to do, the note and the last refusal.
    pub(crate) fn station_lines(&self) -> Vec<String> {
        let (Some(tab), Some(v)) = (self.station_tab(), self.shown_rack()) else {
            return Vec::new();
        };
        let mut lines = vec![format!("station \"{}\" \"{}\"", v.tag, v.part)];
        if let Some(d) = &tab.draft {
            lines.push(format!("  kind {}", d.kind.word()));
            let config = d
                .config
                .and_then(|c| {
                    self.project_session()?
                        .plc_config_projects()
                        .into_iter()
                        .find(|(x, _)| *x == c)
                        .map(|(_, n)| n)
                })
                .unwrap_or_else(|| strings::text(key!("rack-new-plc")).to_owned());
            lines.push(format!("  plc \"{config}\""));
        } else {
            lines.push(format!("  plc \"{}\"", v.plc));
        }
        for (label, code) in &v.order_codes {
            lines.push(format!("  order_code \"{label}\" \"{code}\""));
        }
        let place = |c: &StationCursor| match c {
            StationCursor::Head => "head".to_owned(),
            StationCursor::Slot { rack: 0, slot } => format!("slot {slot}"),
            StationCursor::Slot { rack, slot } => format!("slot {rack}.{slot}"),
            StationCursor::Position { position, .. } => format!("position {position}"),
        };
        for r in &v.rows {
            let mut line = format!("  slot {}", rack_slot_text(r));
            if r.module.is_some() {
                line.push_str(&format!(" \"{}\" \"{}\"", r.tag, r.part));
                if !r.gives.is_empty() {
                    line.push_str(&format!(" gives \"{}\"", r.gives));
                }
                if !r.addresses.is_empty() {
                    line.push_str(&format!(" at \"{}\"", r.addresses));
                }
                if let Some(page) = &r.page {
                    line.push_str(&format!(" page \"{page}\""));
                }
                if !r.allowed.is_empty() {
                    let allowed: Vec<String> = r.allowed.iter().map(u16::to_string).collect();
                    line.push_str(&format!(" allowed {}", allowed.join(",")));
                }
            } else {
                line.push_str(" free");
            }
            lines.push(line);
        }
        for c in &v.cells {
            lines.push(format!(
                "  position {} \"{}\" \"{}\"",
                c.position,
                rack_position_text(&c.at),
                c.at.part
            ));
        }
        if let Some(c) = &tab.cursor {
            lines.push(format!("  cursor {}", place(c)));
        }
        for p in self.station_catalogue() {
            let unfit = p.unfit.map(|u| format!(" unfit \"{u}\"")).unwrap_or_default();
            lines.push(format!("  part \"{}\" \"{}\"{unfit}", p.number, p.gives));
        }
        for t in self.station_todo() {
            let kind = if t.error { "finding" } else { "todo" };
            lines.push(format!("  {kind} \"{}\"", t.text));
        }
        if let Some(n) = &tab.note {
            lines.push(format!("  note \"{n}\""));
        }
        if let Some((at, e)) = &tab.error {
            let at = at
                .as_ref()
                .map(|c| format!(" at {}", place(c)))
                .unwrap_or_default();
            lines.push(format!("  error{at} \"{e}\""));
        }
        lines
    }
}

impl ShellModel {
    /// `plc.preview {cmd, params}` (FR-PLC-014, FR-PLC-015): the previewed PLC addressing command `cmd` (`plc.readdress`,
    /// `plc.convert_manual_to_auto`, `plc.freeze`) as a card in the Preview tab, whose list of changes is the command's
    /// notices (every channel whose address moves, old → new); Apply commits it as one undo step.
    pub(crate) fn act_plc_preview(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "cmd") {
            Ok(Some(n)) if ["plc.readdress", "plc.convert_manual_to_auto", "plc.freeze"].contains(&n) => {
                n.to_owned()
            }
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["cmd"]),
            Ok(None) => return failed(key!("error-missing-param"), &["cmd"]),
            Err(e) => return e,
        };
        let params = p.get("params").cloned().unwrap_or_else(|| json!({}));
        self.preview_bulk(&name, params)
    }

    /// `plc.instrument_list {config, path?}` (FR-PLC-050, doc 10 §4.7): a configuration project row's Import instrument
    /// list…: the instrument list (a CSV or the first sheet of a workbook) as `plc.apply_instrument_list`, previewed in
    /// the Preview tab; Apply commits it as one undo step. In the window a missing path asks for the table.
    pub(crate) fn act_plc_instrument_list(&mut self, p: &Map<String, Value>) -> Outcome {
        let config = or_return!(self.record_param(p, "config"));
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => {
                return self.need_path("plc.instrument_list", crate::shell::Prompt::OpenTable, "path", p);
            }
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = or_return!(read_exchange_file(&path, &file));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let config = config.to_string();
        let cmd = match session.instrument_list(Some(&config), &bytes) {
            Ok(cmd) => cmd,
            Err(e) => return failed(key!("error-plc-import-file"), &[&file, &e.to_string()]),
        };
        self.preview_bulk(&cmd.name, cmd.params)
    }

    /// `fluid.generate_overview {station}` (FR-FLU-033, doc 11 §5.5): `valve_terminal.generate_overview` of a valve
    /// terminal as a card in the Preview tab (`Session::overview_params`: the band of the page's frame, the starter
    /// overview module for the members not drawn yet), whose changes list each page's members, the pages it creates
    /// and a page it empties; the derived changes show the tags a new page break changes. Apply commits it as one
    /// undo step.
    pub(crate) fn act_fluid_generate_overview(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = if p.contains_key("station") {
            or_return!(self.record_param(p, "station"))
        } else {
            match self.selected_valve_terminal() {
                Ok(s) => s,
                Err(reason) => return Outcome::refused(reason),
            }
        };
        let Some(mut params) = self.project_session().and_then(|s| s.overview_params(station)) else {
            return Outcome::refused(key!("reason-overview-no-page"));
        };
        // A new overview page names the terminal it shows (the persona round: page 202 had no title).
        if let Some(tag) = self.project_session().and_then(|s| s.device_tag(station)) {
            params["title"] = Value::String(strings::format(key!("overview-page-title"), &[&tag]));
        }
        self.preview_bulk("valve_terminal.generate_overview", params)
    }

    /// The valve terminal the selection is (one device whose `fluid_menu` says so); else the reason in words: select
    /// one, or the selected device is none.
    pub(crate) fn selected_valve_terminal(&self) -> Result<Uuid, Key<'static>> {
        let session = self.project_session().ok_or(key!("reason-no-project"))?;
        let selected = self.selected_devices();
        let [device] = selected.as_slice() else {
            return Err(key!("reason-overview-select"));
        };
        let device: Uuid = device.parse().map_err(|_| key!("reason-overview-select"))?;
        if session.fluid_menu(device).terminal {
            Ok(device)
        } else {
            Err(key!("reason-overview-not-terminal"))
        }
    }

    /// `fluid.valve_add {module, position}` (doc 11 §7, the persona round): a valve terminal's Add valve at position
    /// n: one `valve.set_occupancy` with the part of its other valves, else the libraries' valve for its positions
    /// (embedded with it), then the new valve's symbol on the pointer when the page shown takes it, so that a click
    /// places it; a toast names the valve and its position.
    pub(crate) fn act_fluid_valve_add(&mut self, p: &Map<String, Value>) -> Outcome {
        let module = or_return!(self.record_param(p, "module"));
        let Some(position) = p.get("position").and_then(Value::as_u64) else {
            return failed(key!("error-missing-param"), &["position"]);
        };
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(part) = session.fluid_menu(module).valve_part else {
            return Outcome::refused(key!("reason-valve-no-part"));
        };
        let mut params = json!({
            "module": module.to_string(), "position": position,
            "occupancy": { "valve": { "state": "used", "part": part.to_string() } },
        });
        let copies = valve_copies(session, module, part);
        if !copies.is_empty() {
            params["copies"] = Value::Array(copies);
        }
        let out = self.run_logged("valve.set_occupancy".to_owned(), params);
        if out != Outcome::Done {
            return out;
        }
        let Some(session) = self.project_session() else {
            return out;
        };
        let valve = session.valve_at(module, position);
        let tag = valve.and_then(|v| session.device_tag(v)).unwrap_or_default();
        let module_tag = session.device_tag(module).unwrap_or_default();
        // On a terminal drawn as its positions the valve stands in its cell already (rc.3).
        let seated = session.drawn_as_positions(module);
        self.notify(
            strings::format(
                key!("toast-valve-added"),
                &[&tag, &position.to_string(), &module_tag],
            ),
            true,
        );
        if let Some(valve) = valve.filter(|_| self.canvas().is_some() && !seated) {
            let mut pick = Map::new();
            pick.insert("function".into(), Value::String(valve.to_string()));
            let _ = self.act_place_pick(&pick);
        }
        Outcome::Done
    }

    /// `fluid.go_output {channel?}` (rc.2, F15): a terminal valve's Go to output: the PLC list shown with the channel
    /// that drives the solenoid marked; refused in words while no output drives it.
    pub(crate) fn act_fluid_go_output(&mut self, p: &Map<String, Value>) -> Outcome {
        if p.get("channel").is_none_or(Value::is_null) {
            return Outcome::refused(key!("reason-solenoid-no-output"));
        }
        let channel = or_return!(self.record_param(p, "channel"));
        let shown = self.dispatch("navigator.show", &json!({ "kind": "plc" }));
        if shown != Outcome::Done {
            return shown;
        }
        self.dispatch(
            "navigator.select",
            &json!({ "kind": "plc", "keys": [format!("channel:{channel}")] }),
        )
    }

    /// `fluid.draw_slots {rep}` (rc.2, F15): Draw as slots on a valve terminal drawn before as a free box: the preview
    /// of `valve_terminal.draw_slots`, committed from it as one undo step.
    pub(crate) fn act_fluid_draw_slots(&mut self, p: &Map<String, Value>) -> Outcome {
        let rep = or_return!(self.record_param(p, "rep"));
        let mut params = json!({ "rep": rep.to_string() });
        // Its valves and its supply drawn on no page are drawn in it, with their symbols (rc.3).
        if let (Some(session), Some(c)) = (self.project_session(), self.canvas()) {
            let tables = &c.page().tables;
            let terminal = tables.reps.iter().find(|r| r.id == rep).and_then(|r| r.device);
            let copies = session.cell_copies(terminal, &[], Some(tables.page));
            if !copies.is_empty() {
                params["copies"] = Value::Array(copies);
            }
        }
        self.preview_bulk("valve_terminal.draw_slots", params)
    }

    /// `fluid.draw_valves {terminal}` (rc.3): Draw its valves in the terminal: `valve_terminal.draw_valves` with the
    /// symbols it needs, one undo step: the valves put on its positions without a symbol stand in its cells, and its
    /// supply ports 1, 3 and 5 at its left end; a toast says so.
    pub(crate) fn act_fluid_draw_valves(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = or_return!(self.record_param(p, "terminal"));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let tag = session.device_tag(terminal).unwrap_or_default();
        let params = draw_valves_params(session, terminal);
        let out = self.run_logged("valve_terminal.draw_valves".to_owned(), params);
        if out == Outcome::Done {
            self.notify(
                strings::format(key!("toast-terminal-valves-drawn"), &[&tag]),
                true,
            );
        }
        out
    }

    /// `fluid.cell_fill {terminal, module, position, replace?, part?}` (rc.2, F15): Fill position n… or Replace… on a
    /// cell of a valve terminal drawn as its positions on the page shown: with `part`, that valve is placed in the cell
    /// ([`Self::fill_cell`]); without, the device browser opens narrowed to valves for a terminal, and its choice does
    /// it. Refused in words when the terminal is not drawn as its positions on the page shown.
    pub(crate) fn act_fluid_cell_fill(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = or_return!(self.record_param(p, "terminal"));
        let module = or_return!(self.record_param(p, "module"));
        let Some(position) = p.get("position").and_then(Value::as_u64) else {
            return failed(key!("error-missing-param"), &["position"]);
        };
        let replace = p.get("replace").and_then(Value::as_bool).unwrap_or(false);
        let at = self.canvas().and_then(|c| {
            let t = &c.page().tables;
            let grid = crate::tools::wire::PageGridMap::new(t.grid);
            let cell = t
                .reps
                .iter()
                .filter(|r| r.device == Some(terminal))
                .filter_map(|r| r.black_box.as_ref())
                .flat_map(|b| b.cells.iter())
                .find(|c| c.module == module && u64::from(c.position) == position)?;
            Some((
                grid.coord(grid.index(cell.rect[0]) + 1),
                grid.coord(grid.index(cell.rect[1]) + 2),
            ))
        });
        let Some(at) = at else {
            return Outcome::refused(key!("reason-cell-not-shown"));
        };
        let Ok(position) = u32::try_from(position) else {
            return Outcome::refused(key!("reason-cell-not-shown"));
        };
        let fill = crate::tools::CellFill {
            terminal,
            module,
            position,
            replace,
            at,
        };
        if p.contains_key("part") {
            let part = or_return!(self.record_param(p, "part"));
            return self.fill_cell(fill, part);
        }
        let out = self.act_open_picker(crate::tools::PickerMode::Parts);
        if out == Outcome::Done
            && let Some(picker) = &mut self.tools.picker
        {
            picker.cell = Some(fill);
            picker.refilter();
        }
        out
    }

    /// Places valve part `part` in the cell `fill` names: one `device.place` of the part's main function with its
    /// symbol at the cell's point, into the terminal (the cell is its position; `replace` removes the cell's valve or
    /// blanking plate), one undo step; the toast says where it sits.
    pub(crate) fn fill_cell(&mut self, fill: crate::tools::CellFill, part: Uuid) -> Outcome {
        self.tools.picker = None;
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let Some(row) = cat.parts.iter().find(|r| r.copy == part) else {
            return Outcome::refused(key!("reason-valve-no-part"));
        };
        let Some(f) = row.functions.iter().find(|f| f.main).or(row.functions.first()) else {
            return Outcome::refused(key!("reason-no-symbol-for-function"));
        };
        let Some(key) = f.symbol.clone() else {
            return Outcome::refused(key!("reason-no-symbol-for-function"));
        };
        let (Some(project), Some(c)) = (self.project.as_ref(), self.canvas()) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let symbol = cat
            .symbols
            .iter()
            .find(|s| s.key == key)
            .and_then(|s| s.copy)
            .or_else(|| project.session.starter_symbol_copy(&key));
        let Some(symbol) = symbol else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let mut params = json!({
            "page": c.page().tables.page.to_string(),
            "symbol": symbol.to_string(),
            "pos": [fill.at.0, fill.at.1],
            "target": {"from_part": {"part": part.to_string(), "template": f.index,
                "letter": row.letter.clone().unwrap_or_else(|| "Q".to_owned())}},
            "into": fill.terminal.to_string(),
        });
        if fill.replace {
            params["replace"] = Value::Bool(true);
        }
        match project.session.starter_embed_part(part, &key) {
            Ok(Some(embed)) => params["copies"] = embed["copies"].clone(),
            Ok(None) => {}
            Err(keys) => return failed(key!("error-part-fndef-missing"), &[&keys.join(", ")]),
        }
        self.run_logged("device.place".to_owned(), params)
    }

    /// `fluid.draw_solenoid {function, page?}` (rc.2, F11): a valve's Draw solenoid 14 on this page… or on an
    /// electrical page…: on the electrical page `page` names or the one in front, else the window asks which
    /// ([`Self::electrical_page_for`]), the solenoid's coil symbol on the pointer, so that a click places it as that
    /// solenoid of the valve; a toast says which and where. Refused in words without an electrical page (headless).
    pub(crate) fn act_fluid_draw_solenoid(&mut self, p: &Map<String, Value>) -> Outcome {
        let function = or_return!(self.record_param(p, "function"));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some((tag, name, _)) = session.solenoid_of(function) else {
            return Outcome::refused(key!("reason-not-solenoid"));
        };
        let what = strings::format(key!("props-solenoid-of"), &[&name, &tag]);
        let out =
            self.arm_on_electrical_page(p, (function, function), what, key!("reason-no-electrical-page"));
        if out != Outcome::Done || self.tool().name() != "place" {
            return out;
        }
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.notify(
            strings::format(key!("toast-solenoid-draw"), &[&name, &tag, &folio]),
            false,
        );
        Outcome::Done
    }

    /// `fluid.draw_plug {terminal?, page?}` (rc.2): a multipin valve terminal's Draw the plug on an electrical page…,
    /// from any page, as a valve's Draw solenoid: the plug's box on the pointer on the electrical page `page` names or
    /// the one in front, else the window asks which, so that a click places it; a toast says whose and where. Without
    /// `terminal`, of the selected multipin terminal. Refused in words for another device or without an electrical page
    /// (headless).
    pub(crate) fn act_fluid_draw_plug(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = if p.contains_key("terminal") {
            or_return!(self.record_param(p, "terminal"))
        } else {
            match self.selected_multipin_terminal() {
                Ok(t) => t,
                Err(reason) => return Outcome::refused(reason),
            }
        };
        let Some(plug) = self.project_session().and_then(|s| s.terminal_plug(terminal)) else {
            return Outcome::refused(key!("reason-not-multipin"));
        };
        let what = strings::format(key!("draw-plug-of"), &[&plug.tag]);
        let out = self.arm_on_electrical_page(
            p,
            (terminal, plug.function),
            what,
            key!("reason-no-electrical-page-plug"),
        );
        if out != Outcome::Done || self.tool().name() != "black_box" {
            return out;
        }
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.notify(
            strings::format(key!("toast-plug-draw"), &[&plug.tag, &folio]),
            false,
        );
        Outcome::Done
    }

    /// The multipin valve terminal the selection is; else the reason in words: select one, or the selected device is
    /// none.
    pub(crate) fn selected_multipin_terminal(&self) -> Result<Uuid, Key<'static>> {
        let session = self.project_session().ok_or(key!("reason-no-project"))?;
        let selected = self.selected_devices();
        let [device] = selected.as_slice() else {
            return Err(key!("reason-plug-select"));
        };
        let device: Uuid = device.parse().map_err(|_| key!("reason-plug-select"))?;
        match session.terminal_kind(device) {
            Some(ew_api::output::TerminalKind::Multipin) => Ok(device),
            _ => Err(key!("reason-not-multipin")),
        }
    }

    /// `fluid.draw_terminal {terminal}` (rc.2): a valve terminal made as a station (its valves in its rack) drawn on a
    /// fluid page: the fluid page shown, else the nearest one, with its box of positions on the pointer, its valves in
    /// their cells, so that a click places it; a toast says which and where. Refused in words for a device that is no
    /// valve terminal or without a fluid page.
    pub(crate) fn act_fluid_draw_terminal(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = or_return!(self.record_param(p, "terminal"));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        if session.terminal_kind(terminal).is_none() {
            return Outcome::refused(key!("reason-not-valve-terminal"));
        }
        let main = session.main_function(terminal).map(|(f, _)| f);
        let tag = session.device_tag(terminal).unwrap_or_default();
        let out = match main {
            Some(main) => self.arm_on_page(main, "fluid", key!("reason-no-fluid-page-terminal")),
            // Made in the New station dialog, it has no main function yet: its box gives it one.
            None => match self.show_page_of_kind("fluid", key!("reason-no-fluid-page-terminal")) {
                Outcome::Done => self.arm_terminal_box(terminal),
                other => other,
            },
        };
        if out != Outcome::Done {
            return out;
        }
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.notify(
            strings::format(key!("toast-terminal-draw"), &[&tag, &folio]),
            false,
        );
        Outcome::Done
    }

    /// Arms the placement of `function` on the page shown when it is of kind `kind` (`multi_line`, `fluid`), else on the
    /// nearest such page before it (a project's electrical pages usually come before its fluid pages), else after it;
    /// refused with `none` without one.
    fn arm_on_page(&mut self, function: Uuid, kind: &str, none: Key<'static>) -> Outcome {
        let out = self.show_page_of_kind(kind, none);
        if out != Outcome::Done {
            return out;
        }
        let mut pick = Map::new();
        pick.insert("function".into(), Value::String(function.to_string()));
        self.act_place_pick(&pick)
    }

    /// [`Self::electrical_page_for`] for `target` (a solenoid's coil, or a multipin terminal: what the form names in
    /// `what`), then `function` (the coil itself, or the terminal's plug) on the pointer there; `Done` with no tool armed
    /// while the window asks which page.
    fn arm_on_electrical_page(
        &mut self,
        p: &Map<String, Value>,
        (target, function): (Uuid, Uuid),
        what: String,
        none: Key<'static>,
    ) -> Outcome {
        let solenoid = target == function;
        match self.electrical_page_for(solenoid, p, (target, what), none) {
            Ok(true) => {
                let mut pick = Map::new();
                pick.insert("function".into(), Value::String(function.to_string()));
                self.act_place_pick(&pick)
            }
            Ok(false) => Outcome::Done,
            Err(out) => out,
        }
    }

    /// Shows the page shown when it is of kind `kind`, else the nearest such page before it, else after it; refused
    /// with `none` without one.
    pub(super) fn show_page_of_kind(&mut self, kind: &str, none: Key<'static>) -> Outcome {
        let of_kind = |i: &usize| self.page_rows().get(*i).is_some_and(|r| r.kind == kind);
        let here = self.page_index();
        let Some(page) = (here..=here)
            .chain((0..here).rev())
            .chain(here + 1..self.page_count())
            .find(of_kind)
        else {
            return Outcome::refused(none);
        };
        // From a station's view the page is shown in a page tab, also when it is the page the view was opened from.
        if page != here || self.station_tab().is_some() {
            let out = self.walk(page);
            if out != Outcome::Done {
                return out;
            }
            // The page change ends the tool now, not after a tool is armed on the new page.
            self.sync_tool_context();
        }
        Outcome::Done
    }

    /// `fluid.set_medium {line, medium}` (FR-FLU-009): the medium the fluid line's net carries, defined at the line's
    /// end with one `fluid.medium.define`; a toast names the medium and the line.
    pub(crate) fn act_fluid_set_medium(&mut self, p: &Map<String, Value>) -> Outcome {
        let line = or_return!(self.record_param(p, "line"));
        let medium = or_return!(self.record_param(p, "medium"));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some((function, idx)) = session.line_end_cp(line) else {
            return Outcome::refused(key!("reason-line-no-port"));
        };
        let name = session
            .media()
            .into_iter()
            .find(|(m, _)| *m == medium)
            .map(|(_, n)| n)
            .unwrap_or_default();
        let cmd = ew_api::AnyCommand {
            name: "fluid.medium.define".into(),
            params: json!({"function": function.to_string(), "idx": idx, "medium": medium.to_string()}),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            self.notify(strings::format(key!("toast-line-medium"), &[&name]), true);
        }
        out
    }
}

/// A slot of the rack and slot editor as the window and the describe text print it: `3`, or `<rack>.<slot>`.
pub fn rack_slot_text(r: &ew_api::output::RackRow) -> String {
    if r.rack == 0 {
        r.slot.to_string()
    } else {
        format!("{}.{}", r.rack, r.slot)
    }
}

/// What one valve position holds in words: the valve's tag, `blanking plate` or `empty`.
pub fn rack_position_text(p: &ew_api::output::RackPosition) -> String {
    match (&p.valve, p.blank) {
        (Some(tag), _) => tag.clone(),
        (None, true) => strings::text(key!("rack-blank")).to_owned(),
        (None, false) => strings::text(key!("rack-empty")).to_owned(),
    }
}

/// A valve terminal module's positions in words: `1 -201U3-Q1 · 2 blank · 3 empty`.
pub fn rack_positions_text(r: &ew_api::output::RackRow) -> String {
    r.positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let what = rack_position_text(p);
            format!("{} {what}", i + 1)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// What a new station is (F18 of the rc.1 test), as the new-station dialog asks it; it narrows the head parts offered
/// (`Session::station_parts`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StationKind {
    /// A PLC: a CPU.
    Cpu,
    /// A remote I/O station: an interface module on a bus.
    Remote,
    /// A valve terminal with a bus node.
    ValveTerminal,
    /// A compact device with channels of its own.
    Compact,
}

impl StationKind {
    /// Every kind, in the dialog's order.
    pub const ALL: [StationKind; 4] = [
        StationKind::Cpu,
        StationKind::Remote,
        StationKind::ValveTerminal,
        StationKind::Compact,
    ];

    /// The word of `plc.new_station`'s and `plc.rack_set`'s `kind`.
    pub fn word(self) -> &'static str {
        match self {
            StationKind::Cpu => "cpu",
            StationKind::Remote => "remote",
            StationKind::ValveTerminal => "valve_terminal",
            StationKind::Compact => "compact",
        }
    }

    /// The kind of a word.
    pub fn from_word(word: &str) -> Option<StationKind> {
        StationKind::ALL.into_iter().find(|k| k.word() == word)
    }

    /// The kind in words, as the dialog offers it.
    pub fn text_key(self) -> Key<'static> {
        match self {
            StationKind::Cpu => key!("rack-kind-cpu"),
            StationKind::Remote => key!("rack-kind-remote"),
            StationKind::ValveTerminal => key!("rack-kind-valve-terminal"),
            StationKind::Compact => key!("rack-kind-compact"),
        }
    }

    /// The kind in full words, the hover text of its short choice.
    pub fn tip_key(self) -> Key<'static> {
        match self {
            StationKind::Cpu => key!("rack-kind-cpu-tip"),
            StationKind::Remote => key!("rack-kind-remote-tip"),
            StationKind::ValveTerminal => key!("rack-kind-valve-terminal-tip"),
            StationKind::Compact => key!("rack-kind-compact-tip"),
        }
    }
}

/// A station not made yet: the new-station dialog's choices (F18 of the rc.1 test).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StationDraft {
    /// What it is.
    pub kind: StationKind,
    /// Its head part (a project or library part copy); `None` before one is chosen.
    pub head: Option<Uuid>,
    /// The PLC (configuration project) it belongs to; `None`: a new one.
    pub config: Option<Uuid>,
    /// Its module parts in slot order.
    pub modules: Vec<Uuid>,
}

/// The command a preview was made of, printed by its name only (its parameters hold the whole file read).
#[derive(Clone, PartialEq, Eq)]
pub struct PreviewedCommand(pub std::sync::Arc<ew_api::AnyCommand>);

impl std::fmt::Debug for PreviewedCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.name)
    }
}

/// The tabs of the PLC import's preview (doc 10 §6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportTab {
    /// The differing fields the merge applies, keeps, rejects or leaves pending.
    Changed,
    /// The fields both sides changed: each takes the file's value or keeps the project's.
    Conflicts,
    /// The objects of the last exchange the file no longer holds.
    Removed,
    /// The file's objects without a counterpart: a creatable one can be created unplaced.
    Parking,
}

impl ImportTab {
    /// The tabs in order.
    pub const ALL: [ImportTab; 4] = [
        ImportTab::Changed,
        ImportTab::Conflicts,
        ImportTab::Removed,
        ImportTab::Parking,
    ];

    /// The tab's word (`plc.import_tab`'s `tab`).
    pub fn word(self) -> &'static str {
        match self {
            ImportTab::Changed => "changed",
            ImportTab::Conflicts => "conflicts",
            ImportTab::Removed => "removed",
            ImportTab::Parking => "parking",
        }
    }
}

/// One differing field of the import's preview.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportChange {
    /// Its key in the resolutions.
    pub id: String,
    /// The object and the field in words (`channel Q0.1 · symbol`).
    pub text: String,
    /// The project's value, and the file's.
    pub ours: String,
    /// The file's value.
    pub theirs: String,
    /// What the merge makes of it: `apply`, `rejected`, `kept`, `pending` or `conflict`.
    pub state: &'static str,
    /// The side Apply takes without a choice: `ours` or `theirs`.
    pub default: &'static str,
    /// Why the file's value cannot be taken, in words; `None` when it can.
    pub refused: Option<String>,
}

/// One parked object of the import's preview.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportParked {
    /// Its ID in the file: its key in the resolutions.
    pub id: String,
    /// Its kind and name in words.
    pub text: String,
    /// Whether several objects of the project match it (else none does).
    pub ambiguous: bool,
    /// Whether "create unplaced" applies.
    pub creatable: bool,
}

/// The PLC import's preview (`Dialog::PlcImport`): the file, its tab on show, the plan in words, and the choices.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlcImportCard {
    /// The file's name.
    pub file: String,
    /// The file's path, which the preview is made of again when "the file is complete" changes.
    pub path: String,
    /// Whether the file is complete (doc 10 §4.4): an object of the last exchange it lacks is removed.
    pub complete: bool,
    /// Why Apply would be refused (its prevent rules, tried on the preview), in words; `None` when it commits.
    pub refusal: Option<String>,
    /// The `plc.apply_exchange` the preview was made of, with every option it was made with (`complete`); Apply
    /// commits it under the choices, and the file is neither read nor merged again here.
    pub apply: PreviewedCommand,
    /// The tab on show.
    pub tab: ImportTab,
    /// The differing fields (not conflicts).
    pub changes: Vec<ImportChange>,
    /// The conflicts.
    pub conflicts: Vec<ImportChange>,
    /// The removed objects in words.
    pub removed: Vec<String>,
    /// The parked objects.
    pub parking: Vec<ImportParked>,
    /// The choices: a change's key or a parked object's ID to `ours`, `theirs` or `create`.
    pub choices: std::collections::BTreeMap<String, String>,
}

impl PlcImportCard {
    /// The rows of tab `tab`.
    pub fn count(&self, tab: ImportTab) -> usize {
        match tab {
            ImportTab::Changed => self.changes.len(),
            ImportTab::Conflicts => self.conflicts.len(),
            ImportTab::Removed => self.removed.len(),
            ImportTab::Parking => self.parking.len(),
        }
    }
}

/// The bytes of the exchange file or instrument list at `path` (named `file`), read through the importers' input limit:
/// a file larger than the limit, or one that cannot be read, is refused in words.
fn read_exchange_file(path: &std::path::Path, file: &str) -> Result<Vec<u8>, Outcome> {
    match super::drawing_import::read_limited(path) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => {
            let mib = (ew_api::plc_exchange::max_file_bytes() >> 20).to_string();
            Err(failed(key!("error-plc-import-too-large"), &[file, &mib]))
        }
        Err(e) => Err(failed(
            key!("error-plc-import-file"),
            &[file, &crate::jobs::io_cause(&e)],
        )),
    }
}

/// The PLC import's card of the preview `import` of the file `file`: its rows in the window's words.
fn plc_import_card(
    file: String,
    import: ew_api::plc_exchange::PlcImport,
    refusal: Option<&(ew_api::CmdError, Vec<String>)>,
) -> PlcImportCard {
    let words = |v: &Option<String>| v.clone().unwrap_or_default();
    // Apply's refusal in the window's words; the rows of the changes it concerns say so.
    let why = refusal.map(|(e, _)| match super::tools::command_failed(e, &Value::Null) {
        Outcome::Failed { message } => message,
        _ => e.to_string(),
    });
    let named: &[String] = refusal.map_or(&[], |(_, changes)| changes.as_slice());
    let (conflicts, changes): (Vec<_>, Vec<_>) = import
        .plan
        .changes
        .iter()
        .map(|c| {
            let field = plc_field_text(&c.object, &c.field);
            let refused = (!c.acceptable).then(|| {
                // A field of a group the file cannot take is a comment of the other form (the command's reason).
                let key = if c.group.is_some() {
                    key!("refusal-plc-exchange-language")
                } else {
                    key!("refusal-plc-exchange-not-acceptable")
                };
                strings::sentence_start(&strings::format(key, &[&field, &c.subject])).into_owned()
            });
            let refused = refused.or_else(|| why.clone().filter(|_| named.contains(&c.id)));
            ImportChange {
                id: c.id.clone(),
                text: strings::format(key!("plc-import-row"), &[&c.subject, &field]),
                ours: words(&c.ours),
                theirs: words(&c.theirs),
                state: c.state.word(),
                default: match c.default {
                    ew_api::plc_exchange::Choice::Ours => "ours",
                    ew_api::plc_exchange::Choice::Theirs => "theirs",
                },
                refused,
            }
        })
        .partition(|c| c.state == "conflict");
    let object = |o: &ew_api::plc_exchange::ExchangeObject, name: &str| {
        strings::format(key!("plc-import-object"), &[&plc_object_text(o), name])
    };
    PlcImportCard {
        file,
        path: String::new(),
        complete: false,
        refusal: why,
        apply: PreviewedCommand(std::sync::Arc::new(import.apply)),
        tab: ImportTab::Changed,
        changes,
        conflicts,
        removed: import
            .plan
            .removed
            .iter()
            .map(|(o, _, name)| object(o, name))
            .collect(),
        parking: import
            .plan
            .parking
            .iter()
            .map(|p| ImportParked {
                id: p.id.clone(),
                text: object(&p.object, &p.name),
                ambiguous: p.ambiguous,
                creatable: p.creatable,
            })
            .collect(),
        choices: std::collections::BTreeMap::new(),
    }
}

/// The kind of an exchanged object in the window's words: its `plc-object-<kind>` text, `plc-object-other` for a kind
/// of a newer format.
pub fn plc_object_text(object: &ew_api::plc_exchange::ExchangeObject) -> String {
    strings::lookup(&format!("plc-object-{}", object.name()))
        .filter(|_| !object.name().is_empty())
        .unwrap_or_else(|| strings::text(key!("plc-object-other")))
        .to_owned()
}

/// A field of an exchanged object in the window's words: the object's own `plc-field-<kind>-<field>` text (a tag's
/// `name` is its symbol), else `plc-field-<field>`, a comment in a language with the language; a field this version
/// does not know as it is written.
pub fn plc_field_text(object: &ew_api::plc_exchange::ExchangeObject, field: &str) -> String {
    if let Some(lang) = field.strip_prefix("comment.") {
        return strings::format(key!("plc-field-comment-in"), &[lang]);
    }
    let key = field.replace(['.', '_'], "-");
    strings::lookup(&format!("plc-field-{}-{key}", object.name()))
        .or_else(|| strings::lookup(&format!("plc-field-{key}")))
        .map_or_else(|| field.to_owned(), str::to_owned)
}

/// The PLC export formats PLC exchange… offers: (word, the format, its name); product names, the same in every
/// language.
pub const PLC_FORMATS: [(&str, &str); 4] = [
    ("aml-1.4.0", "AutomationML AR APC 1.4.0"),
    ("aml-1.2.0", "AutomationML AR APC 1.2.0"),
    ("tia-xlsx", "TIA Portal PLC tags (XLSX)"),
    ("plcopen", "PLCopen XML (CODESYS)"),
];

/// The exchange format of a word of [`PLC_FORMATS`].
pub(super) fn plc_format(word: &str) -> Option<ew_api::plc_exchange::PlcFormat> {
    use ew_api::plc_exchange::{ApcVersion, PlcFormat};
    match word {
        "aml-1.4.0" => Some(PlcFormat::Aml(ApcVersion::V1_4_0)),
        "aml-1.2.0" => Some(PlcFormat::Aml(ApcVersion::V1_2_0)),
        "tia-xlsx" => Some(PlcFormat::TiaXlsx),
        "plcopen" => Some(PlcFormat::Plcopen),
        _ => None,
    }
}

impl ShellModel {
    /// `plc.import {path?, complete?}` (doc 10 §4.4): reads an AML file or a TIA tag workbook and opens its preview
    /// against the project's configuration project; nothing changes until Apply. In the window a missing path asks for
    /// the file.
    pub(crate) fn act_plc_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => {
                return self.need_path("plc.import", crate::shell::Prompt::OpenPlcExchange, "path", p);
            }
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = or_return!(read_exchange_file(&path, &file));
        let complete = p.get("complete").and_then(Value::as_bool).unwrap_or(false);
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let options = ew_api::plc_exchange::PlcOptions {
            complete,
            ..Default::default()
        };
        let import = match session.plc_import(&bytes, &options) {
            Ok(i) => i,
            Err(e) => return failed(key!("error-plc-import"), &[&e.to_string()]),
        };
        // Apply tried on the preview (its prevent rules run), so a start the file gives that overlaps another is
        // refused here already, its row marked, rather than at Apply.
        let refusal = self
            .project
            .as_mut()
            .and_then(|project| project.session.plc_import_refusal(&import));
        let mut card = plc_import_card(file, import, refusal.as_ref());
        card.path = path.to_string_lossy().into_owned();
        card.complete = complete;
        if card.changes.is_empty() && !card.conflicts.is_empty() {
            card.tab = ImportTab::Conflicts;
        }
        self.open_dialog(crate::shell::Dialog::PlcImport(Box::new(card)))
    }

    /// `plc.import_tab {tab}`: the tab the import's preview shows.
    pub(crate) fn act_plc_import_tab(&mut self, p: &Map<String, Value>) -> Outcome {
        let tab = match string_param(p, "tab") {
            Ok(Some(w)) => match ImportTab::ALL.into_iter().find(|t| t.word() == w) {
                Some(t) => t,
                None => return failed(key!("error-bad-param"), &["tab"]),
            },
            Ok(None) => return failed(key!("error-missing-param"), &["tab"]),
            Err(e) => return e,
        };
        match &mut self.dialog {
            Some(crate::shell::Dialog::PlcImport(card)) => {
                card.tab = tab;
                Outcome::Done
            }
            _ => Outcome::refused(key!("reason-plc-import-closed")),
        }
    }

    /// `plc.import_choose {id, choice?}`: a conflict's or a parked object's choice (`ours`, `theirs`, `create`; absent:
    /// the default again).
    pub(crate) fn act_plc_import_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        let id = match string_param(p, "id") {
            Ok(Some(i)) => i.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["id"]),
            Err(e) => return e,
        };
        let choice = match string_param(p, "choice") {
            Ok(Some(c)) if ["ours", "theirs", "create"].contains(&c) => Some(c.to_owned()),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["choice"]),
            Ok(None) => None,
            Err(e) => return e,
        };
        match &mut self.dialog {
            Some(crate::shell::Dialog::PlcImport(card)) => {
                match choice {
                    Some(c) => card.choices.insert(id, c),
                    None => card.choices.remove(&id),
                };
                Outcome::Done
            }
            _ => Outcome::refused(key!("reason-plc-import-closed")),
        }
    }

    /// `plc.import_apply`: the import's `plc.apply_exchange` under its choices, one undo step; the preview closes and a
    /// toast says what changed.
    pub(crate) fn act_plc_import_apply(&mut self) -> Outcome {
        let Some(crate::shell::Dialog::PlcImport(card)) = self.dialog.clone() else {
            return Outcome::refused(key!("reason-plc-import-closed"));
        };
        let mut apply = (*card.apply.0).clone();
        apply.params["resolutions"] = json!(card.choices);
        let out = self.run_commit(&apply);
        if out == Outcome::Done {
            self.dialog = None;
            self.notify(strings::format(key!("toast-plc-import"), &[&card.file]), true);
        }
        out
    }

    /// `plc.export {format?, stations?, date?, path?, replace?}` (doc 10 §4.3, §4.5, §4.6): the configuration project
    /// exported in the format (`aml-1.4.0`, `aml-1.2.0`, `tia-xlsx`, `plcopen`) for the stations (tags, comma
    /// separated; empty: every station) at the document date (`YYYY-MM-DD`; empty: the model's), written whole or
    /// not at all, then recorded with `plc.record_exchange`. In the window a missing format opens the form and a missing
    /// path asks for the file.
    pub(crate) fn act_plc_export_file(&mut self, p: &Map<String, Value>, format: &str) -> Outcome {
        let Some(fmt) = plc_format(format) else {
            return failed(key!("error-bad-param"), &["format"]);
        };
        let stations: Vec<String> = string_param(p, "stations")
            .ok()
            .flatten()
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .map(|s| if s.contains(':') { s } else { format!("tag:{s}") })
            .collect();
        let date = match string_param(p, "date")
            .ok()
            .flatten()
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            None => None,
            Some(d) => match d.parse::<ew_api::plc_exchange::Date>() {
                Ok(d) => Some(d),
                Err(_) => return failed(key!("error-plc-export-date"), &[d]),
            },
        };
        // The form's values are checked before the file is asked for, so a value refused keeps the form.
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => super::jobs::absolute_target(std::path::PathBuf::from(path)),
            Ok(None) => {
                let mut rest = p.clone();
                rest.insert("replace".to_owned(), Value::Bool(true));
                self.dialog = None;
                let prompt = crate::shell::Prompt::SavePlcExchange(fmt.extension());
                return self.need_path("plc.export", prompt, "path", &rest);
            }
        };
        // An existing file is replaced only on request (the save dialog asks itself, and says so).
        if path.exists() && !or_return!(super::jobs::bool_param(p, "replace", false)) {
            return Outcome::refused(key!("reason-file-exists"));
        }
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let options = ew_api::plc_exchange::PlcOptions {
            stations,
            date,
            ..Default::default()
        };
        let export = match session.plc_export(fmt, &options) {
            Ok(e) => e,
            Err(e) => return failed(key!("error-plc-export"), &[&e.to_string()]),
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Err(e) = super::external::write_replacing(&path, &export.bytes) {
            return failed(key!("error-plc-export"), &[&e.to_string()]);
        }
        let out = export
            .record
            .as_ref()
            .map_or(Outcome::Done, |record| self.run_commit(record));
        if out == Outcome::Done {
            let [stations, modules, channels, tags] = export.counts;
            let [stations, modules, channels, tags] = [
                strings::format_count(crate::count_key!("plc-count-stations"), stations),
                strings::format_count(crate::count_key!("plc-count-modules"), modules),
                strings::format_count(crate::count_key!("plc-count-channels"), channels),
                strings::format_count(crate::count_key!("plc-count-symbols"), tags),
            ];
            let mut text = strings::format(
                key!("toast-plc-export"),
                &[&name, &stations, &modules, &channels, &tags],
            );
            for w in &export.warnings {
                text = format!("{} {w}.", strings::without_stop(&text));
            }
            self.notify(text, false);
        }
        out
    }
}

/// The copies a valve of part `part` about to take a position of module `module` needs (rc.3): the library part's
/// closure ([`ew_api::Session::module_part_copies`]) and the symbols that draw it, and the terminal's supply, in its cell
/// ([`ew_api::Session::cell_copies`]).
fn valve_copies(session: &ew_api::Session, module: Uuid, part: Uuid) -> Vec<Value> {
    let mut copies = session.module_part_copies(part);
    extend_copies(&mut copies, session.cell_copies(Some(module), &[part], None));
    copies
}

/// `copies` with the documents of `more` it does not hold yet (by ID): a command refuses a document named twice.
pub(crate) fn extend_copies(copies: &mut Vec<Value>, more: Vec<Value>) {
    for c in more {
        if !copies.iter().any(|x| x["id"] == c["id"]) {
            copies.push(c);
        }
    }
}

/// The parameters of `valve_terminal.draw_valves` for `terminal`, with the symbols it draws with (rc.3): its menu's
/// Draw its valves in the terminal and EW-CORE-047's fix run it.
pub(crate) fn draw_valves_params(session: &ew_api::Session, terminal: Uuid) -> Value {
    let mut params = json!({ "terminal": terminal.to_string() });
    let copies = session.cell_copies(Some(terminal), &[], None);
    if !copies.is_empty() {
        params["copies"] = Value::Array(copies);
    }
    params
}
