// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's entries of the fluid commands (doc 11): a valve terminal's overview, its valves, slots,
//! solenoids, plugs and multi-pin terminal drawn, a cell filled, a line's medium set. Each handler builds one declared
//! command from the selection it is aimed at and commits it as one undo step, saying what it did in words.
//!
//! Not here: the fluid forms (New medium…, Valve terminal…, the fluid settings) are in `shell/creation.rs`; a valve
//! terminal's rack in the station view in `shell/station.rs`.

use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed};
use crate::key;
use crate::strings;
use crate::strings::Key;

impl ShellModel {
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
        // A new overview page names the terminal it shows.
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

    /// `fluid.valve_add {module, position}` (doc 11 §7): a valve terminal's Add valve at position
    /// n: one `valve.set_occupancy` with the part of its other valves, else the libraries' valve for its positions
    /// (embedded with it), then the new valve's symbol on the pointer when the page shown takes it, so that a click
    /// places it; a toast names the valve and its position.
    pub(crate) fn act_fluid_valve_add(&mut self, p: &Map<String, Value>) -> Outcome {
        let module = or_return!(self.record_param(p, "module"));
        let Some(position) = p.get("position").and_then(Value::as_u64) else {
            return failed(key!("error-missing-param"), &["position"]);
        };
        let session = &or_return!(crate::shell::gated(&self.project)).session;
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
        // On a terminal drawn as its positions the valve stands in its cell already.
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

    /// `fluid.go_output {channel?}`: a terminal valve's Go to output: the PLC list shown with the channel
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

    /// `fluid.draw_slots {rep}`: Draw as slots on a valve terminal drawn before as a free box: the preview
    /// of `valve_terminal.draw_slots`, committed from it as one undo step.
    pub(crate) fn act_fluid_draw_slots(&mut self, p: &Map<String, Value>) -> Outcome {
        let rep = or_return!(self.record_param(p, "rep"));
        let mut params = json!({ "rep": rep.to_string() });
        // Its valves and its supply drawn on no page are drawn in it, with their symbols.
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

    /// `fluid.draw_valves {terminal}`: Draw its valves in the terminal: `valve_terminal.draw_valves` with the
    /// symbols it needs, one undo step: the valves put on its positions without a symbol stand in its cells, and its
    /// supply ports 1, 3 and 5 at its left end; a toast says so.
    pub(crate) fn act_fluid_draw_valves(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = or_return!(self.record_param(p, "terminal"));
        let session = &or_return!(crate::shell::gated(&self.project)).session;
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

    /// `fluid.cell_fill {terminal, module, position, replace?, part?}`: Fill position n… or Replace… on a
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

    /// `fluid.draw_solenoid {function, page?}`: a valve's Draw solenoid 14 on this page… or on an
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

    /// `fluid.draw_plug {terminal?, page?}`: a multipin valve terminal's Draw the plug on an electrical page…,
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

    /// `fluid.plug_ports` (Fluid ▸ Plug ports: no need to right-click one port at a
    /// time): the open working ports of the selected valves plugged, one undo step; a toast says whose.
    pub(crate) fn act_fluid_plug_ports(&mut self) -> Outcome {
        let (fixes, tags) = match self.selected_open_ports() {
            Ok(found) => found,
            Err(reason) => return failed(reason, &[]),
        };
        let cmd = ew_api::AnyCommand {
            name: "check.fix_all".into(),
            params: json!({ "fixes": fixes }),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            self.notify(
                strings::format(key!("toast-ports-plugged"), &[&tags.join(", ")]),
                true,
            );
        }
        out
    }

    /// The open working ports of the selected valves as plugs ([`plug_fix`]) and the valves' tags; else why not, in
    /// words.
    pub(crate) fn selected_open_ports(&self) -> Result<(Vec<Value>, Vec<String>), Key<'static>> {
        let session = self.project_session().ok_or(key!("reason-no-project"))?;
        let (mut plugs, mut tags) = (Vec::new(), Vec::new());
        for device in self.selected_devices() {
            let Ok(id) = device.parse::<Uuid>() else { continue };
            let open = session.fluid_menu(id).open_ports;
            if open.is_empty() {
                continue;
            }
            tags.push(session.device_tag(id).unwrap_or_default());
            plugs.extend(open.iter().map(|(f, idx, _)| plug_fix(*f, *idx)));
        }
        if plugs.is_empty() {
            return Err(key!("reason-plug-ports-select"));
        }
        Ok((plugs, tags))
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

    /// `fluid.draw_terminal {terminal}`: a valve terminal made as a station (its valves in its rack) drawn on a
    /// fluid page: the fluid page shown, else the nearest one, else one made there and then ([`Self::fluid_page_here`]),
    /// with its box of positions on the pointer, its valves in their cells, so that a click places it; a toast says which
    /// and where. A failure is said in words, also from a button that bypasses the bar's greying (the station's
    /// To do and the Valve terminal dialog's Create did nothing and said nothing).
    pub(crate) fn act_fluid_draw_terminal(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = or_return!(self.record_param(p, "terminal"));
        let Some(session) = self.project_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        if session.terminal_kind(terminal).is_none() {
            return failed(key!("reason-not-valve-terminal"), &[]);
        }
        // Its box drawn elsewhere (a schematic page) is drawn again; a terminal made in the New station dialog has no
        // main function yet, and one put on a bus page first has its bus port as that: a new box is drawn then.
        let main = session
            .main_function(terminal)
            .map(|(f, _)| f)
            .filter(|f| session.is_black_box_function(*f));
        let tag = session.device_tag(terminal).unwrap_or_default();
        let out = match self.fluid_page_here() {
            Outcome::Done => match main {
                Some(main) => {
                    let mut pick = Map::new();
                    pick.insert("function".into(), Value::String(main.to_string()));
                    self.act_place_pick(&pick)
                }
                None => self.arm_terminal_box(terminal),
            },
            other => other,
        };
        match out {
            Outcome::Done => {}
            Outcome::Refused { reason: reason_key } => {
                return Outcome::Failed {
                    message: strings::render(&reason_key),
                };
            }
            failure @ Outcome::Failed { .. } => return failure,
        }
        let folio = self.current_folio().unwrap_or_default().to_owned();
        self.notify(
            strings::format(key!("toast-terminal-draw"), &[&tag, &folio]),
            false,
        );
        Outcome::Done
    }

    /// The fluid page a fluid command works on: the page shown when it is one, else the nearest
    /// ([`Self::show_page_of_kind`]), else a new one made there and then, asking nothing, as the station's Put it on a
    /// bus page makes its page; a toast names the page made.
    pub(crate) fn fluid_page_here(&mut self) -> Outcome {
        if self.page_rows().iter().any(|r| r.kind == ew_api::PageType::Fluid) {
            return self.show_page_of_kind(&ew_api::PageType::Fluid, key!("reason-no-fluid-page-terminal"));
        }
        let folio = self.new_fluid_page();
        if folio.is_empty() {
            return failed(key!("reason-fluid-page-not-made"), &[]);
        }
        let out = self.dispatch("page.goto", &json!({ "folio": folio }));
        if out == Outcome::Done {
            self.sync_tool_context();
            self.notify(strings::format(key!("toast-fluid-page-made"), &[&folio]), false);
        }
        out
    }

    /// Makes a fluid page with the next free folio, asking nothing; its folio, empty when it could not be made.
    pub(super) fn new_fluid_page(&mut self) -> String {
        let folio = self.free_folio("fluid");
        let title = strings::text(key!("fluid-page-title"));
        let made = self.dispatch(
            "page.new",
            &json!({ "folio": folio, "type": "fluid", "title": title }),
        );
        if made == Outcome::Done {
            folio
        } else {
            String::new()
        }
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
    pub(super) fn show_page_of_kind(&mut self, kind: &ew_api::PageType, none: Key<'static>) -> Outcome {
        let of_kind = |i: &usize| self.page_rows().get(*i).is_some_and(|r| r.kind == *kind);
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
        let session = &or_return!(crate::shell::gated(&self.project)).session;
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

/// The copies a valve of part `part` about to take a position of module `module` needs: the library part's
/// closure ([`ew_api::Session::module_part_copies`]) and the symbols that draw it, and the terminal's supply, in its cell
/// ([`ew_api::Session::cell_copies`]).
pub(super) fn valve_copies(session: &ew_api::Session, module: Uuid, part: Uuid) -> Vec<Value> {
    let mut copies = session.module_part_copies(part);
    extend_copies(&mut copies, session.cell_copies(Some(module), &[part], None));
    copies
}

/// The plug of port `idx` of fluid function `function`: a `fluid.port.close` as `command.run` and `check.fix_all` take it.
pub(crate) fn plug_fix(function: Uuid, idx: u16) -> Value {
    json!({ "cmd": "fluid.port.close", "params": {
        "function": function.to_string(), "idx": idx, "closure": { "kind": "plug" } } })
}

/// `copies` with the documents of `more` it does not hold yet (by ID): a command refuses a document named twice.
pub(crate) fn extend_copies(copies: &mut Vec<Value>, more: Vec<Value>) {
    for c in more {
        if !copies.iter().any(|x| x["id"] == c["id"]) {
            copies.push(c);
        }
    }
}

/// The parameters of `valve_terminal.draw_valves` for `terminal`, with the symbols it draws with: its menu's
/// Draw its valves in the terminal and EW-CORE-047's fix run it.
pub(crate) fn draw_valves_params(session: &ew_api::Session, terminal: Uuid) -> Value {
    let mut params = json!({ "terminal": terminal.to_string() });
    let copies = session.cell_copies(Some(terminal), &[], None);
    if !copies.is_empty() {
        params["copies"] = Value::Array(copies);
    }
    params
}
