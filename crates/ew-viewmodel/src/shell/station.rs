// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The station view (FR-PLC-028, doc 10 §6): what a station's view shows and the types the window draws it from.
//!
//! A station's Rack… opens its view as a tab beside the page tabs (`area::StationTab`), which shows
//! `Session::rack_view`: the head, the slots, a valve terminal's positions, the view's catalogue (the place catalogue's
//! parts that fit the station), its To do list, and the describe lines of the view. New station… is the same view on
//! a station that is made by Create ([`StationDraft`]).
//!
//! Not here: the view's edits (adding, moving, renaming, the channels, Create) are in `station/rack.rs`; drawing a
//! station on a page (Draw on a page…, Place here, the To do list's next step) in `station/draw.rs`; the fluid
//! commands in `shell/fluid.rs`, the PLC import and export in `shell/plc_exchange.rs`.

use ew_api::output::Uuid;
use serde_json::{Value, json};

use super::ShellModel;
use super::fluid::valve_copies;
use crate::area::{StationCursor, StationTab};
use crate::key;
use crate::strings;
use crate::strings::Key;

mod draw;
mod rack;

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
    /// The device under the cursor ([`ShellModel::station_device`]).
    pub device: Option<Uuid>,
    /// The channels of the card under the cursor, in channel-number order (`Session::module_channels`).
    pub channels: Vec<ew_api::output::RackChannel>,
    /// What is left to do ([`ShellModel::station_todo`]).
    pub todo: Vec<StationTodo>,
    /// For a station not made yet: the project's PLCs by name ([`ShellModel::plc_config_projects`]) and the devices
    /// that can be the station ([`ShellModel::station_candidates`]); empty for a made station.
    pub plcs: Vec<(Uuid, String)>,
    /// See [`StationShown::plcs`].
    pub candidates: Vec<(Uuid, String)>,
}

/// What [`StationShown`] is made for: the revision, the language generation, the place catalogue's key, the view's
/// own state and the page it stands on (its To do draws there).
pub(crate) type StationKey = (u64, u64, Option<(u64, u64, String)>, StationTab, usize);

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
    /// A side of the station drawn on a page of its kind: `plc.rack_next {step, folio}` on the page the view
    /// stands on when it is of that kind, on a new one when the project has none, else on the one the person picks of
    /// `pages`, or a new one.
    OnPage {
        /// `plc.rack_next`'s step: `draw` (an electrical page), `bus` or `fluid`.
        step: &'static str,
        /// The button's words: where it goes, or what it asks.
        label: String,
        /// The folio the button acts on without asking: the page the view stands on, or `new`; `None` when it asks.
        folio: Option<String>,
        /// The pages of the kind, folio and title, that it asks between (New page… after them).
        pages: Vec<(String, String)>,
    },
}

impl ShellModel {
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
    /// (side by side, it shows its lists too). The views of both groups are kept, so that neither is made every frame:
    /// a frame, a pointer move and the view's bar read it, and it is made again only when the project, the language,
    /// the catalogue, the view's state or the page it stands on changes.
    pub fn station_shown_of(&self, tab: &StationTab) -> Option<std::sync::Arc<StationShown>> {
        let key: StationKey = (
            self.revision()?,
            strings::generation(),
            self.tools.catalogue.as_ref().map(|(k, _)| k.clone()),
            tab.clone(),
            self.page_index(),
        );
        if let Some((_, shown)) = self.station_cache.borrow().iter().find(|(k, _)| *k == key) {
            return Some(std::sync::Arc::clone(shown));
        }
        let rack = self.rack_of(tab)?;
        let device = self.device_of(tab, &rack);
        let (plcs, candidates) = match &tab.draft {
            Some(d) => (self.plc_config_projects(), self.candidates_of(d)),
            None => (Vec::new(), Vec::new()),
        };
        let shown = std::sync::Arc::new(StationShown {
            catalogue: self.catalogue_of(tab, &rack),
            channels: match (self.project_session(), device) {
                (Some(s), Some(card)) => s.module_channels(card),
                _ => Vec::new(),
            },
            todo: self.todo_of(tab, &rack),
            device,
            rack,
            plcs,
            candidates,
        });
        let mut cache = self.station_cache.borrow_mut();
        // One view per group of tabs: the oldest goes.
        if cache.len() >= 2 {
            cache.remove(0);
        }
        cache.push((key, std::sync::Arc::clone(&shown)));
        Some(shown)
    }

    /// What kind of valve terminal `device` is, in words: `Valve terminal · PLC station on PROFIBUS DP`,
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
        self.count(|c| c.station_shown += 1);
        match (&tab.draft, tab.station) {
            (Some(d), _) => Some(self.project_session()?.draft_rack(d.head, &d.modules)),
            (None, Some(s)) => self.rack_view(s),
            (None, None) => None,
        }
    }

    /// The rack the station view on show draws ([`ShellModel::rack_of`], read from [`ShellModel::station_shown`]).
    pub fn shown_rack(&self) -> Option<ew_api::output::RackView> {
        Some(self.station_shown()?.rack.clone())
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

    /// The devices already in the project that a new station offers as the station: the bus
    /// devices in no PLC, those that lead a bus for a PLC (CPU), the others for every other kind; by tag.
    pub fn station_candidates(&self) -> Vec<(Uuid, String)> {
        self.station_shown()
            .map(|s| s.candidates.clone())
            .unwrap_or_default()
    }

    /// [`ShellModel::station_candidates`] of the station not made yet `d`.
    fn candidates_of(&self, d: &StationDraft) -> Vec<(Uuid, String)> {
        let Some(s) = self.project_session() else {
            return Vec::new();
        };
        let cpu = d.kind == StationKind::Cpu;
        s.free_bus_devices()
            .into_iter()
            .filter(|(_, _, leads)| *leads == cpu)
            .map(|(id, tag, _)| (id, tag))
            .collect()
    }

    /// The folio a page of `kind` the program makes takes, asking nothing: New page's proposal for the kind (the
    /// section made for it), else the first number no page has.
    pub(super) fn free_folio(&self, kind: &str) -> String {
        let (proposal, _) = self.new_page_proposal(None, Some(kind));
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
        self.pages_of_kind("bus_topology")
    }

    /// The project's fluid pages: folio and title, in page order (a valve terminal's Draw on a fluid page… asks which
    /// when there are several).
    pub fn fluid_pages(&self) -> Vec<(String, String)> {
        self.pages_of_kind("fluid")
    }

    fn pages_of_kind(&self, kind: &str) -> Vec<(String, String)> {
        self.page_rows()
            .iter()
            .filter(|r| r.kind == kind)
            .map(|r| (r.folio.clone(), r.title.clone()))
            .collect()
    }

    /// The catalogue of the station view on show: the place catalogue's parts (the device browser's) that fit the
    /// station, under the view's filter (the browser's matching), by group, the parts that fit where the cursor
    /// stands first. A station not made yet without a head part lists the parts that can head a station of its
    /// kind, then the cards; a valve terminal lists the valves for a terminal beside the cards.
    pub fn station_catalogue(&self) -> Vec<StationPart> {
        self.station_shown()
            .map(|s| s.catalogue.clone())
            .unwrap_or_default()
    }

    /// [`ShellModel::station_catalogue`] of the station view of `tab`, whose rack is `view`.
    fn catalogue_of(&self, tab: &StationTab, view: &ew_api::output::RackView) -> Vec<StationPart> {
        let Some((_, cat)) = self.tools.catalogue.as_ref() else {
            return Vec::new();
        };
        self.count(|c| c.station_shown += 1);
        let heads = tab
            .draft
            .as_ref()
            .filter(|d| d.head.is_none() || tab.cursor == Some(StationCursor::Head))
            .map(|d| d.kind.word());
        let terminal = !view.cells.is_empty();
        // A station whose part takes no modules has no slot, free or taken (a valve terminal of valve positions alone).
        let no_slots = view.rows.is_empty();
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
        self.station_shown()?.device
    }

    /// [`ShellModel::station_device`] of the station view of `tab`, whose rack is `view`.
    fn device_of(&self, tab: &StationTab, view: &ew_api::output::RackView) -> Option<Uuid> {
        tab.draft.is_none().then_some(())?;
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

    /// What is left to do on the station on show, each in a sentence with its fix: the slot rule's findings, a card
    /// without a part, each side of the station on no page of its kind (its boxes on no electrical page, its bus
    /// ports on no bus page, a valve terminal's valves on no fluid page), its overview; for a station not made yet,
    /// what Create still needs.
    pub fn station_todo(&self) -> Vec<StationTodo> {
        self.station_shown().map(|s| s.todo.clone()).unwrap_or_default()
    }

    /// [`ShellModel::station_todo`] of the station view of `tab`, whose rack is `view`.
    fn todo_of(&self, tab: &StationTab, view: &ew_api::output::RackView) -> Vec<StationTodo> {
        let Some(session) = self.project_session() else {
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
        let terminal = self.is_valve_terminal(station);
        // Each side of the station is drawn on a page of its kind: its boxes on an electrical page, its bus
        // ports on a bus page, a valve terminal's valves on their positions on a fluid page. A drawing of one side
        // does not stand for another.
        let open = self.page_rows().get(self.page_index());
        let on_page = |step: &'static str, kinds: &[&str], pages: Vec<(String, String)>| {
            let here = open.filter(|r| kinds.contains(&r.kind.as_str()));
            let (label, folio) = match (here, pages.is_empty()) {
                (Some(r), _) => {
                    let at = if step == "bus" {
                        key!("rack-put-here")
                    } else {
                        key!("rack-draw-here")
                    };
                    (strings::format(at, &[&r.folio]), Some(r.folio.clone()))
                }
                (None, true) => {
                    let new = match step {
                        "bus" => key!("rack-put-new-bus"),
                        "fluid" => key!("rack-draw-new-fluid"),
                        _ => key!("rack-draw-new-electrical"),
                    };
                    (strings::text(new).to_owned(), Some("new".to_owned()))
                }
                (None, false) => {
                    let ask = match step {
                        "bus" => key!("rack-next-bus"),
                        "fluid" => key!("rack-next-fluid"),
                        _ => key!("rack-next-electrical"),
                    };
                    (strings::text(ask).to_owned(), None)
                }
            };
            Some(TodoFix::OnPage {
                step,
                label,
                folio,
                pages,
            })
        };
        let rows: Vec<&ew_api::output::RackRow> = std::iter::once(&view.head).chain(&view.rows).collect();
        let off: Vec<String> = session
            .off_electrical_pages(station)
            .into_iter()
            .filter_map(|d| rows.iter().find(|r| r.module == Some(d)).map(|r| name(r)))
            .collect();
        if !off.is_empty() {
            let text = if terminal && off.len() == 1 {
                strings::format(key!("rack-todo-electrical-side"), &[&off[0]])
            } else {
                strings::format(key!("rack-todo-undrawn"), &[&off.join(", ")])
            };
            out.push(step(
                text,
                on_page("draw", &["multi_line", "single_line"], self.schematic_pages()),
            ));
        }
        // A card wider than the drawing area, drawn one box at a time (docs/design/06a-drafting.md §7.5): the rest goes
        // onto a page the person chooses, another than the one its first box stands on.
        for card in session.cards_drawn_in_part(station) {
            let Some(row) = rows.iter().find(|r| r.module == Some(card)) else {
                continue;
            };
            let pages: Vec<(String, String)> = self
                .schematic_pages()
                .into_iter()
                .filter(|(folio, _)| row.page.as_ref() != Some(folio))
                .collect();
            out.push(step(
                strings::format(key!("rack-todo-card-rest"), &[&name(row)]),
                Some(TodoFix::OnPage {
                    step: "draw",
                    label: strings::text(key!("rack-next-electrical")).to_owned(),
                    folio: None,
                    pages,
                }),
            ));
        }
        let on_bus_page = self
            .bus_pages()
            .iter()
            .any(|(folio, _)| session.undrawn_interfaces(station, folio).is_empty());
        if session.has_bus_interfaces(station) && !on_bus_page {
            out.push(step(
                strings::format(key!("rack-todo-bus"), &[&name(&view.head)]),
                on_page("bus", &["bus_topology"], self.bus_pages()),
            ));
        }
        if terminal {
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
                    on_page("fluid", &["fluid"], self.fluid_pages()),
                ));
            }
            // Drawn as its positions, with valves put on them without a symbol.
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
        let shown = self.station_shown().ok_or(key!("reason-rack-closed"))?;
        let view = &shown.rack;
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

/// What a new station is, as the new-station dialog asks it; it narrows the head parts offered
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

/// A station not made yet: the new-station dialog's choices.
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
    /// The tag typed for it, which Create gives it frozen; `None`: the tag its page gives it when it is drawn.
    pub tag: Option<String>,
}
