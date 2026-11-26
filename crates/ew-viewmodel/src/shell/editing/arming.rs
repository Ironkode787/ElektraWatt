// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arming the canvas tools: a catalogue row, a strip, a part or a black box the next click places.

use super::{Pick, embedding, text_with_key};
use crate::key;
use crate::shell::terminals::{
    TERMINAL_DESIGNATION, TERMINAL_ROW_MAX, TERMINAL_SPACING_MAX, terminal_designation,
};
use crate::shell::{Outcome, ShellModel, failed, string_param};
use crate::strings;
use crate::strings::Key;
use crate::tools::{
    BlackBoxTool, BoxTarget, ExchangeTool, PendingTerminal, Picker, PickerMode, PlaceTarget, PlaceTool,
};
use ew_api::output::{CatalogueSymbol, PlaceCatalogue, SymbolShape, Uuid};
use serde_json::{Map, Value, json};
use std::sync::Arc;

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Arming

    /// The catalogue strip named by `param`: its ID, or its tag as the page prints it.
    pub(in crate::shell) fn catalogue_strip(cat: &PlaceCatalogue, param: &str) -> Option<usize> {
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
    pub(in crate::shell) fn tag_example(&self, aspect: &str, letter: &str) -> Option<String> {
        self.tag_example_except(aspect, letter, None)
    }

    /// [`Self::tag_example`] with the device `except` left out: a cable the same step removes (a bus line's
    /// cable replaced), whose tag the numbering gives again.
    pub(in crate::shell) fn tag_example_except(
        &self,
        aspect: &str,
        letter: &str,
        except: Option<Uuid>,
    ) -> Option<String> {
        let row = self.page_rows().get(self.page_index())?;
        let numbered_here = match row.kind {
            ew_api::PageType::MultiLine | ew_api::PageType::SingleLine => true,
            ew_api::PageType::BusTopology => aspect == "cable",
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

    /// A refusal whose text names action `action`'s first key: the first of `keys` formatted with it, or the second
    /// when the action has no key.
    pub(super) fn refused_with_key(&self, keys: (Key<'_>, Key<'_>), action: &str) -> Outcome {
        Outcome::Failed {
            message: text_with_key(keys, &crate::command_bar::key_of(self, action)),
        }
    }

    /// Arms the black box tool with the interfaces of a device on a bus-topology page, a box each (`bus.place_ports`,
    /// doc 10 §2.7): `target` the command's target, `label` what the tool's label names, `part` the part copy of a new
    /// device (whose closure travels in the placement).
    pub(in crate::shell) fn arm_ports(
        &mut self,
        cat: &PlaceCatalogue,
        target: Value,
        label: String,
        interfaces: &[ew_api::output::CatalogueInterface],
        part: Option<ew_api::output::Uuid>,
    ) -> Outcome {
        let project = or_return!(crate::shell::gated(&self.project));
        let Some(row) = cat.symbols.iter().find(|r| r.black_box && r.allowed) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let Some(copy) = row.copy.or_else(|| project.session.starter_symbol_copy(&row.key)) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let mut copies = match embedding(&project.session, &row.key, part) {
            Ok(copies) => copies,
            Err(out) => return out,
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
    pub(in crate::shell) fn arm_part_ports(
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

    pub(in crate::shell) fn arm(
        &mut self,
        cat: Arc<PlaceCatalogue>,
        pick: Pick,
        p: &Map<String, Value>,
    ) -> Outcome {
        let letter_param = match string_param(p, "letter") {
            Ok(v) => v.map(str::to_owned),
            Err(e) => return e,
        };
        // On a bus-topology page a part with bus interfaces is placed as its interfaces, a box each, with every port
        // (doc 10 §2.7): a station as a switch or a drive, its supply pins left to its place on a schematic page.
        if let Pick::PartFn(pi, _) | Pick::PartSymbol(pi, _) = &pick
            && !cat.parts[*pi].interfaces.is_empty()
        {
            // A station of this part that the project has and this page does not show comes first (Device ▸ Place must not
            // make a second, empty CPU beside the one drawn with its cards): Put -1A1 here, or a new one.
            let copy = cat.parts[*pi].copy;
            if self.mode == crate::shell::Mode::Gui && !self.stations_off_page(Some(copy)).is_empty() {
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
                let Some(key) = &f.symbol else {
                    return Outcome::refused(key!("reason-no-symbol-for-function"));
                };
                let Some(i) = cat.symbols.iter().position(|s| s.key == *key) else {
                    return Outcome::refused(key!("reason-no-symbol-for-function"));
                };
                // A black-box entry is a box drawn by the frame (`PlaceCatalogue::drawn_as_box`).
                if cat.drawn_as_box(*pi, *fi) {
                    box_cps = Some(f.cps.clone());
                }
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
        // The refusals name the way out with its key from the keymap, or without the parenthesis when unbound.
        if self.tools.black_box_pick && !is_box {
            return self.refused_with_key(super::NOT_BLACK_BOX, "tool.place");
        }
        // An unplaced black-box function (a station placed first on a bus-topology page keeps its supply pins for a
        // schematic page) is drawn by the black box tool as another representation of it.
        if let (true, Pick::Function(function, _)) = (is_box, &pick)
            && self.project.is_some()
            && let Ok(id) = function.parse::<Uuid>()
        {
            return self.arm_unplaced_box(&row, function, id);
        }
        if is_box && matches!(pick, Pick::Function(..) | Pick::Join(..)) {
            return self.refused_with_key(super::BLACK_BOX_TOOL, "tool.black_box");
        }
        // What the project lacks travels in the placement itself, so that placing from the picker is one undo
        // step and the recorded command replays without a library: a starter symbol with its FnDef and, for a part's
        // template function (part-first), every FnDef the part's
        // template needs and a library part's closure. The tool names a starter symbol by the copy ID its embedding
        // gives it.
        let copies: Vec<Value> = match &self.project {
            None => Vec::new(),
            Some(project) => match embedding(&project.session, &row.key, part_copy) {
                Ok(copies) => copies,
                Err(out) => return out,
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
            return self.arm_black_box(
                &row,
                part,
                letter_param,
                part_letter,
                part_copy,
                part_cps,
                part_outputs,
                part_layout,
                copies,
            );
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
            (_, part) if terminal => match self.terminal_target(cat, &row, part, p) {
                Ok(target) => target,
                Err(out) => return out,
            },
            _ => {
                let Some(letter) = letter_param.or(row.letter.clone()) else {
                    return Outcome::refused(key!("reason-letter-needed"));
                };
                PlaceTarget::NewDevice { letter }
            }
        };
        self.arm_place_tool(shape, target, p, copies)
    }

    /// Arms the Place tool with `shape` for `target`: its autoconnect hints, the Join chooser where the symbol's FnDef
    /// can be an auxiliary function, a Terminal… row's count and spacing; a part's function that a device of the part
    /// has unplaced fills that device.
    fn arm_place_tool(
        &mut self,
        shape: Arc<SymbolShape>,
        target: PlaceTarget,
        p: &Map<String, Value>,
        copies: Vec<Value>,
    ) -> Outcome {
        self.tools.picker = None;
        let autoconnect = self.page_autoconnects();
        // J offers the Join chooser while the symbol's FnDef can be an auxiliary function.
        let joinable = self
            .project
            .as_ref()
            .is_some_and(|p| p.session.auxiliary_fndef(shape.fndef));
        // A part's function that a device of the part has unplaced fills that device.
        let slot = match &target {
            PlaceTarget::Part { part, template, .. } => part.parse::<Uuid>().ok().map(|p| (p, *template)),
            _ => None,
        };
        // A terminal row of Terminal… (`count`): one click lays them all.
        let row = match &target {
            PlaceTarget::Terminal { .. } => p.get("count").and_then(Value::as_u64).unwrap_or(1),
            _ => 1,
        };
        let row = u32::try_from(row).unwrap_or(1).clamp(1, TERMINAL_ROW_MAX);
        // The spacing Terminal… gave; else a strip drawn on the page keeps its pitch; else a pick's step.
        let pitch = match &target {
            PlaceTarget::Terminal { strip, .. } => self.strip_pitch(strip),
            _ => None,
        };
        let spacing = p
            .get("spacing")
            .and_then(Value::as_u64)
            .and_then(|s| u32::try_from(s).ok())
            .or(pitch)
            .unwrap_or(crate::shell::TERMINAL_PICK_STEPS)
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

    /// Arms the black box tool for an unplaced black-box function (a station placed first on a bus-topology page keeps
    /// its supply pins for a schematic page): another representation of it, a valve terminal's positions on a fluid
    /// page, a plug's pins down its two sides; placed by Place, it says that it is drawn as a box.
    fn arm_unplaced_box(&mut self, row: &CatalogueSymbol, function: &str, id: Uuid) -> Outcome {
        let project = or_return!(crate::shell::gated(&self.project));
        let cps = project.session.function_cp_list(id);
        let Some(copy) = row.copy.or_else(|| project.session.starter_symbol_copy(&row.key)) else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        let target = BoxTarget::Function {
            function: function.to_owned(),
            cps,
        };
        // A valve terminal made as a station, drawn on a fluid page: as its positions, one click.
        let fluid = self
            .page_rows()
            .get(self.page_index())
            .is_some_and(|r| r.kind == ew_api::PageType::Fluid);
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
        // Placed by Place, not by the Black box tool, it says at once that it is drawn as a box and how.
        let drawn_as_box = !self.tools.black_box_pick
            && self
                .tools
                .picker
                .as_ref()
                .is_none_or(|picker| picker.mode != PickerMode::BlackBox);
        let tag = {
            let q = project.session.query();
            function
                .parse()
                .ok()
                .and_then(|f| q.functions().get(f))
                .and_then(|f| f.device)
                .and_then(|d| project.session.device_tag(d.as_uuid()))
        };
        self.tools.picker = None;
        self.activate_tool(Box::new(
            BlackBoxTool::new(copy.to_string(), target)
                .with_copies(copies)
                .with_slots(slots)
                .with_pins(pins)
                .with_drawn_as_box(drawn_as_box),
        ));
        if drawn_as_box
            && slots.is_none()
            && !pins
            && let Some(tag) = tag
        {
            self.notify(strings::format(key!("toast-drawn-as-box"), &[&tag]), false);
        }
        Outcome::Done
    }

    /// Arms the black box tool for a black-box row: a part's template function as the part's box, else a new device's
    /// box; on a fluid page a valve terminal as its positions; a part drawn as a box says so.
    #[allow(clippy::too_many_arguments)]
    fn arm_black_box(
        &mut self,
        row: &CatalogueSymbol,
        part: Option<(String, u16, String)>,
        letter_param: Option<String>,
        part_letter: Option<String>,
        part_copy: Option<Uuid>,
        part_cps: Vec<(u16, String)>,
        part_outputs: Vec<u16>,
        part_layout: Option<ew_api::part_box::PartBox>,
        copies: Vec<Value>,
    ) -> Outcome {
        let Some(copy) = row.copy else {
            return Outcome::refused(key!("reason-unknown-symbol"));
        };
        // A part without a symbol of its own that the Place tool places, not the Black box tool, is drawn as a
        // box: the hint says so.
        let drawn_as_box = part.is_some()
            && !self.tools.black_box_pick
            && self
                .tools
                .picker
                .as_ref()
                .is_none_or(|picker| picker.mode != PickerMode::BlackBox);
        let named = part.as_ref().map(|(_, _, number)| number.clone());
        let target = match part {
            Some((part, template, number)) => {
                let Some(letter) = letter_param.or(part_letter).or(row.letter.clone()) else {
                    return Outcome::refused(key!("reason-letter-needed"));
                };
                // A PLC card without a device box of its own is laid out as a card (doc 10 §3.6).
                let card = part_copy
                    .zip(self.project_session())
                    .map(|(copy, s)| s.part_card_pins(copy, &part_cps));
                BoxTarget::Part {
                    part,
                    template,
                    letter,
                    number,
                    cps: part_cps,
                    outputs: part_outputs,
                    layout: part_layout,
                    card,
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
            .is_some_and(|r| r.kind == ew_api::PageType::Fluid);
        let slots = part_copy
            .filter(|_| fluid)
            .and_then(|p| self.project.as_ref()?.session.part_positions(p));
        // Drawn as its positions, its supply ports stand in its left end.
        let mut copies = copies;
        if let (Some(_), Some(project), Some(c)) = (slots, &self.project, self.canvas()) {
            let cells = project.session.cell_copies(None, &[], Some(c.page().tables.page));
            crate::shell::fluid::extend_copies(&mut copies, cells);
        }
        self.activate_tool(Box::new(
            BlackBoxTool::new(copy.to_string(), target)
                .with_copies(copies)
                .with_rep_type(rep_type)
                .with_slots(slots)
                .with_drawn_as_box(drawn_as_box),
        ));
        // The hint alone went unread: the first click came before it.
        if drawn_as_box
            && slots.is_none()
            && let Some(number) = named
        {
            self.notify(strings::format(key!("toast-drawn-as-box"), &[&number]), false);
        }
        Outcome::Done
    }

    /// The target of a terminal placement: its strip (a new one when the project has none and the picker chose it, or
    /// the picker's strip chooser when none is named) and its designation; `Err` the outcome the placement ends with
    /// instead (a refusal, or the strip chooser opened).
    fn terminal_target(
        &mut self,
        cat: Arc<PlaceCatalogue>,
        row: &CatalogueSymbol,
        part: Option<(String, u16, String)>,
        p: &Map<String, Value>,
    ) -> Result<PlaceTarget, Outcome> {
        let mut strip = string_param(p, "strip")?.map(str::to_owned);
        // Without a strip in the project a terminal has nowhere to go: a direct `place.pick` refuses. Chosen
        // in the picker (the window's path) it goes on a new strip, made as Terminal…'s new strip is (no
        // empty strip chooser stands between Place and the page).
        if cat.strips.is_empty() && (strip.is_some() || self.tools.picker.is_none()) {
            return Err(Outcome::refused(key!("reason-no-strip")));
        }
        let mut cat = cat;
        if cat.strips.is_empty() {
            let made = self.act_strip_new(&Map::new());
            if made != Outcome::Done {
                return Err(made);
            }
            strip.clone_from(&self.tools.last_primary);
            // Its toast says what happens now, not the Terminal… way strip.new's names.
            self.notify(
                strings::text(key!("toast-strip-new-for-terminal")).to_owned(),
                false,
            );
            match self.catalogue() {
                Some(fresh) => cat = fresh,
                None => return Err(Outcome::refused(key!("reason-no-catalogue"))),
            }
        }
        let strip = strip.as_deref();
        // Without a strip the picker asks for one: its strip chooser.
        let Some(strip) = strip else {
            let Some(folio) = self.current_folio().map(str::to_owned) else {
                return Err(Outcome::refused(key!("reason-no-page")));
            };
            let pending = PendingTerminal {
                symbol: row.key.clone(),
                params: p.clone(),
                part: part.map(|(copy, template, _)| (copy, template)),
            };
            self.palette = None;
            self.tools.picker = Some(Picker::strips(&folio, Arc::clone(&cat), pending));
            return Err(Outcome::Done);
        };
        // An unknown strip name says which strips there are (G-PP finding 9): a new strip's
        // `-X??` takes its counter with its first terminal, so the name the toast gave may have changed.
        let Some(i) = Self::catalogue_strip(&cat, strip) else {
            let tags: Vec<&str> = cat.strips.iter().map(|st| st.tag.as_str()).collect();
            return Err(failed(key!("error-unknown-strip"), &[strip, &tags.join(", ")]));
        };
        let st = &cat.strips[i];
        let designation = match terminal_designation(p) {
            Ok(Some(d)) => d.to_owned(),
            Ok(None) => st.next_designation.clone(),
            Err(e) => return Err(e),
        };
        Ok(PlaceTarget::Terminal {
            strip: st.id.to_string(),
            strip_tag: st.tag.clone(),
            designation,
            part: part.map(|(copy, _, _)| copy),
        })
    }

    /// Arms the change-symbol tool for the representation the picker was opened for and symbol row `i`.
    pub(super) fn arm_exchange(&mut self, cat: Arc<PlaceCatalogue>, i: usize) -> Outcome {
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
}
