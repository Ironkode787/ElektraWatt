// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's duties: arming the Wire tool and the keyboard route editor, the junction
//! sequence editor popup and its commits, the crossings and wire-number settings (Print wire numbers also as a row of
//! the project's Properties), and the Wire tool's live region.
//!
//! **Reads.** The junction editor reads the page tables the canvas already holds and `Session::junction_wires` once
//! per (session, revision, junction) while it is open; the Wire tool and the proposals read nothing.
//!
//! **Commits** go through the tool's commit path (`execute_with` in the window, `run_script` headless), one
//! `junction.edit` per change (D11).

use std::sync::Arc;

use ew_api::output::PageRow;
use ew_api::window::OffCardBind;
use ew_dl::EntityRef;
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, flag, string_param};
use crate::key;
use crate::properties::{PropRow, PropertySheet, RowAction, RowValue};
use crate::strings;
use crate::strings::Key;
use crate::tools::{CanvasEvent, EntityKind, JunctionEditor, Tool, WireTool, selection_kinds};

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The junction sequence editor, while open.
    pub fn junction_editor(&self) -> Option<&JunctionEditor> {
        self.tools.junction_editor.as_ref()
    }

    /// The text of a canvas key the host refused, shown in place of the tool's hint until the next canvas input.
    pub fn tool_notice(&self) -> Option<&str> {
        self.tools.notice.as_deref()
    }

    /// Exactly one junction is selected on the focused page (`junction.open`'s condition).
    pub fn one_junction_selected(&self) -> bool {
        self.canvas().is_some_and(|c| {
            let kinds = selection_kinds(&self.tools.selection, &c.page().tables);
            matches!(kinds.as_slice(), [(_, EntityKind::Junction)])
        })
    }

    /// Exactly one connection is selected on the focused page (`tool.route_edit`'s condition).
    pub fn one_connection_selected(&self) -> bool {
        self.canvas().is_some_and(|c| {
            let kinds = selection_kinds(&self.tools.selection, &c.page().tables);
            matches!(kinds.as_slice(), [(_, EntityKind::Connection)])
        })
    }

    /// Which way the handle under the pointer moves, while Select is idle and a press there would drag a segment or a
    /// bend of the selected line (the window's pointer shape over it); `None` elsewhere.
    pub fn grip_under_pointer(&self) -> Option<crate::tools::route_edit::GripMove> {
        if self.tools.tool.name() != "select" || self.tools.tool.state() != "idle" {
            return None;
        }
        let c = self.canvas()?;
        let at = c.pointer()?;
        let page = c.page();
        let cx = crate::tools::ToolCx {
            page,
            points_per_nm: c.points_per_nm(),
            pane: c.pane(),
            snapper: &page.snapper,
            selection: &self.tools.selection,
            layout: self.tools.layout_options,
            connect_on_move: self.tools.connect_on_move,
        };
        crate::tools::route_edit::press_part(&cx, at)
            .map(|(_, line, part)| crate::tools::route_edit::grip_move(&line, part))
    }

    /// Which way the grip of the selected box under the pointer resizes it, while Select is idle (the window's pointer
    /// shape over it before the press); `None` elsewhere.
    pub fn box_grip_under_pointer(&self) -> Option<crate::tools::black_box::GripResize> {
        if self.tools.tool.name() != "select" || self.tools.tool.state() != "idle" {
            return None;
        }
        let c = self.canvas()?;
        let at = c.pointer()?;
        let page = c.page();
        let cx = crate::tools::ToolCx {
            page,
            points_per_nm: c.points_per_nm(),
            pane: c.pane(),
            snapper: &page.snapper,
            selection: &self.tools.selection,
            layout: self.tools.layout_options,
            connect_on_move: self.tools.connect_on_move,
        };
        let rep = crate::tools::black_box::selected_black_box(&cx)?;
        match crate::tools::black_box::handle_at(&cx, rep, at)? {
            crate::tools::black_box::BoxHandle::Grip(k) => {
                crate::tools::black_box::grip_resize(&rep.black_box.as_ref()?.outline, k)
            }
            crate::tools::black_box::BoxHandle::Pin(_) => None,
        }
    }

    /// Whether a fluid line is selected on the page shown.
    pub fn fluid_line_selected(&self) -> bool {
        self.canvas().is_some_and(|c| {
            let t = &c.page().tables;
            selection_kinds(&self.tools.selection, t)
                .iter()
                .any(|(e, _)| t.connections.iter().any(|c| c.id == e.0 && c.kind == "fluid"))
        })
    }

    /// Whether the project prints every wire's number on the page (`settings.extra.drawing.wire_numbers`, FT-1692).
    pub fn wire_numbers_on(&self) -> bool {
        self.project.as_ref().is_some_and(|p| {
            p.session
                .query()
                .project()
                .settings
                .extra
                .get("drawing")
                .and_then(|d| d.get("wire_numbers"))
                .and_then(Value::as_bool)
                == Some(true)
        })
    }

    /// Whether the project numbers its wires by potential (`settings.extra.drawing.wire_numbering`, doc 08 §5.2).
    pub fn wire_numbering_per_potential(&self) -> bool {
        self.project.as_ref().is_some_and(|p| {
            p.session
                .query()
                .project()
                .settings
                .extra
                .get("drawing")
                .and_then(|d| d.get("wire_numbering"))
                .and_then(Value::as_str)
                == Some("per_potential")
        })
    }

    /// Whether the project draws crossings as hops (`settings.extra.drawing.crossings`).
    pub fn crossings_hops(&self) -> bool {
        self.crossings_word() == Some("hops")
    }

    /// The project's `crossings` setting when it is set: `hops` or `plain` (absent: `None`, drawn plain).
    pub fn crossings_word(&self) -> Option<&'static str> {
        let p = self.project.as_ref()?;
        let q = p.session.query();
        match q
            .project()
            .settings
            .extra
            .get("drawing")
            .and_then(|d| d.get("crossings"))
            .and_then(Value::as_str)
        {
            Some("hops") => Some("hops"),
            Some("plain") => Some("plain"),
            _ => None,
        }
    }

    /// The project's `marker_mode` setting when it is set: `all`, `deviations`,
    /// `explicit` or `none`; `per_wire` for an absent or unknown one.
    pub fn marker_mode_word(&self) -> &'static str {
        let Some(p) = &self.project else {
            return "per_wire";
        };
        let q = p.session.query();
        match q
            .project()
            .settings
            .extra
            .get("drawing")
            .and_then(|d| d.get("marker_mode"))
            .and_then(Value::as_str)
        {
            Some("all") => "all",
            Some("deviations") => "deviations",
            Some("explicit") => "explicit",
            Some("none") => "none",
            _ => "per_wire",
        }
    }

    /// The ghost route of the active Wire tool while it routes (page nm); `None` for another tool.
    pub fn wire_ghost_route(&mut self) -> Option<Vec<(i64, i64)>> {
        self.tools
            .tool
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<WireTool>())
            .and_then(|w| w.ghost_route().map(<[(i64, i64)]>::to_vec))
    }

    /// How many times the active Wire tool ran the router (the exact-count tests); `None` for another
    /// tool.
    pub fn wire_route_calls(&mut self) -> Option<u64> {
        self.tools
            .tool
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<WireTool>())
            .map(|w| w.route_calls())
    }

    /// Whether the focused page's type autoconnects (doc 06 §2.2: multi-line and single-line pages; fluid pages, whose
    /// fluid ports autoconnect with fluid lines, FT-1733).
    pub(super) fn page_autoconnects(&self) -> bool {
        matches!(
            self.page_rows().get(self.page_index()).map(|r| &r.kind),
            Some(ew_api::PageType::MultiLine | ew_api::PageType::SingleLine | ew_api::PageType::Fluid)
        )
    }

    // -----------------------------------------------------------------------------------------------------------
    // The Wire tool and the route editor

    /// The connection kind the Wire tool draws on the focused page (doc 06 §2.2): `Ok(None)` a wire on a multi-line
    /// page, `Ok(Some("overview"))` on a single-line (the MVP kind), overview or assembly-overview page,
    /// `Ok(Some("fluid"))` (a working line, `{"fluid": {}}`) on a fluid page, `Ok(Some("bus"))` (`{"bus": {}}`) on a
    /// bus-topology page (FT-434); refused on every page type whose Wire tool creates nothing.
    pub fn wire_kind(&self) -> Result<Option<&'static str>, Key<'static>> {
        match self.page_rows().get(self.page_index()).map(|r| &r.kind) {
            Some(ew_api::PageType::MultiLine) => Ok(None),
            Some(
                ew_api::PageType::SingleLine
                | ew_api::PageType::Overview
                | ew_api::PageType::AssemblyOverview,
            ) => Ok(Some("overview")),
            Some(ew_api::PageType::Fluid) => Ok(Some("fluid")),
            Some(ew_api::PageType::BusTopology) => Ok(Some("bus")),
            _ => Err(key!("reason-no-wire-kind")),
        }
    }

    /// `tool.wire` (W): arms the Wire tool with the page type's connection kind.
    pub(crate) fn act_tool_wire(&mut self) -> Outcome {
        match self.wire_kind() {
            Ok(kind) => {
                self.activate_tool(Box::new(WireTool::new().with_kind(kind)));
                Outcome::Done
            }
            Err(reason_key) => Outcome::Refused { reason_key },
        }
    }

    /// The live region of the Wire tool while it routes: `Wire: routing from <label>. …`.
    pub(super) fn announce_wire(&mut self) {
        let label = self
            .with_tool_cx(|t, _| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<WireTool>())
                    .and_then(|w| w.start_label().map(str::to_owned))
            })
            .flatten();
        if let Some(label) = label {
            // The keys from the keymap, as the routing hint names them.
            let back = crate::command_bar::key_name(self, crate::tools::BACK);
            let escape = crate::command_bar::key_name(self, crate::tools::ESCAPE);
            self.tools.announcement = Some(strings::format(
                key!("announce-wire-routing"),
                &[&label, &back, &escape],
            ));
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The junction sequence editor

    /// The cell text of a page point of the focused page (`?` outside the frame grid).
    fn cell_of_page_point(&self, p: (i64, i64)) -> String {
        self.canvas()
            .and_then(|c| {
                let page = c.page();
                page.cell_text(page.snapper.page_to_sheet(p))
            })
            .unwrap_or_else(|| "?".to_owned())
    }

    /// Reads the editor of junction `j` from the focused page's tables (the physical wires once per revision).
    fn read_junction_editor(&mut self, j: EntityRef, row: usize) -> Option<JunctionEditor> {
        let rev = (self.project_epoch()?, self.revision()?);
        let tables = Arc::clone(&self.canvas()?.page().tables);
        let wires = match &self.tools.junction_editor {
            Some(e) if e.junction == j && e.rev == rev => e.wires.clone(),
            _ => {
                let w = self.project.as_ref()?.session.junction_wires(j.0);
                self.count(|c| c.junction_wires += 1);
                w
            }
        };
        let cells: Vec<((i64, i64), String)> = tables
            .junctions
            .iter()
            .map(|x| (x.at, self.cell_of_page_point(x.at)))
            .collect();
        let cell_of = |p: (i64, i64)| {
            cells
                .iter()
                .find(|(at, _)| *at == p)
                .map_or_else(|| "?".to_owned(), |(_, c)| c.clone())
        };
        JunctionEditor::read(&tables, j, &cell_of, wires, rev, row)
    }

    /// `junction.open`: the editor of exactly one selected junction (the registry disables it otherwise, naming
    /// what to select). A script selects the junction first (`select.set` with its ID).
    pub(crate) fn act_junction_open(&mut self) -> Outcome {
        let Some(c) = self.canvas() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let tables = Arc::clone(&c.page().tables);
        let js: Vec<EntityRef> = selection_kinds(&self.tools.selection, &tables)
            .into_iter()
            .filter(|(_, k)| *k == EntityKind::Junction)
            .map(|(e, _)| e)
            .collect();
        let [j] = js.as_slice() else {
            return Outcome::refused(key!("reason-one-junction"));
        };
        match self.read_junction_editor(*j, 0) {
            Some(e) => {
                self.palette = None;
                self.tools.picker = None;
                self.tools.junction_editor = Some(e);
                Outcome::Done
            }
            None => Outcome::refused(key!("reason-one-junction")),
        }
    }

    /// Commits `junction.edit` of the open editor with `members`, then reads the editor again with `row` focused.
    fn junction_commit(&mut self, members: Value, row: usize) -> Outcome {
        let Some(editor) = self.tools.junction_editor.clone() else {
            return Outcome::refused(key!("reason-no-junction-editor"));
        };
        let cmd = ew_api::AnyCommand {
            name: "junction.edit".into(),
            params: editor.edit(members),
        };
        let out = self.run_commit(&cmd);
        self.sync_views();
        match self.read_junction_editor(editor.junction, row) {
            Some(e) => self.tools.junction_editor = Some(e),
            None => self.tools.junction_editor = None,
        }
        out
    }

    /// `junction.member {from, to}` (Alt+Up, Alt+Down in the popup): moves a member of the sequence.
    pub(crate) fn act_junction_member(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(editor) = &self.tools.junction_editor else {
            return Outcome::refused(key!("reason-no-junction-editor"));
        };
        let index = |name: &str| {
            p.get(name)
                .and_then(Value::as_u64)
                .and_then(|v| usize::try_from(v).ok())
        };
        let (Some(from), Some(to)) = (index("from"), index("to")) else {
            return failed(key!("error-missing-param"), &["from"]);
        };
        let Some(seq) = editor.reordered(from, to) else {
            return Outcome::refused(key!("reason-junction-member"));
        };
        let ids: Vec<String> = seq.iter().map(|c| c.0.to_string()).collect();
        self.junction_commit(json!({ "sequence": ids }), to)
    }

    /// `junction.style {style, orient?}`: `dot` or `bevel` with its orientation (0 to 7).
    pub(crate) fn act_junction_style(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(editor) = &self.tools.junction_editor else {
            return Outcome::refused(key!("reason-no-junction-editor"));
        };
        let style = match string_param(p, "style") {
            Ok(Some(s)) => s.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["style"]),
            Err(e) => return e,
        };
        let orient = match p.get("orient") {
            None => editor.bevel_orient,
            Some(v) => match v.as_u64().and_then(|o| u8::try_from(o).ok()).filter(|o| *o < 8) {
                Some(o) => o,
                None => return failed(key!("error-bad-param"), &["orient"]),
            },
        };
        let Some(value) = JunctionEditor::style_json(&style, orient) else {
            return failed(key!("error-bad-param"), &["style"]);
        };
        let row = editor.row;
        self.junction_commit(json!({ "style": value }), row)
    }

    /// `junction.wiring {wiring}`: `star`, `chain` or `style_default`.
    pub(crate) fn act_junction_wiring(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(editor) = &self.tools.junction_editor else {
            return Outcome::refused(key!("reason-no-junction-editor"));
        };
        let wiring = match string_param(p, "wiring") {
            Ok(Some(w @ ("star" | "chain" | "style_default"))) => w.to_owned(),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["wiring"]),
            Ok(None) => return failed(key!("error-missing-param"), &["wiring"]),
            Err(e) => return e,
        };
        let row = editor.row;
        self.junction_commit(json!({ "wiring": wiring }), row)
    }

    /// The focused row of the editor (the popup's Up and Down; no command).
    pub fn set_junction_row(&mut self, row: usize) {
        if let Some(e) = &mut self.tools.junction_editor {
            e.row = row.min(e.members.len().saturating_sub(1));
        }
    }

    /// Keeps the open editor on its page: closed when the page changed or the junction is gone, read again after a
    /// new revision (the end of a dispatch).
    pub(super) fn sync_junction_editor(&mut self) {
        let Some(editor) = self.tools.junction_editor.clone() else {
            return;
        };
        let now = self.project_epoch().zip(self.revision());
        if now == Some(editor.rev) {
            return;
        }
        self.tools.junction_editor = self.read_junction_editor(editor.junction, editor.row);
    }

    // -----------------------------------------------------------------------------------------------------------
    // Crossings

    /// `project.crossings {on?}`: commits `project.drawing.set` with `crossings: hops`, or with `null` (the
    /// member removed: plain, as a project without the setting) to turn it off; without `on` it toggles.
    pub(crate) fn act_crossings(&mut self, p: &Map<String, Value>) -> Outcome {
        let on = if p.contains_key("on") {
            flag(p, "on")
        } else {
            !self.crossings_hops()
        };
        let crossings = if on { json!("hops") } else { Value::Null };
        let cmd = ew_api::AnyCommand {
            name: "project.drawing.set".into(),
            params: json!({ "crossings": crossings }),
        };
        self.run_commit(&cmd)
    }

    /// `project.wire_numbers {on?}` (Draw ▸ Connect ▸ Print wire numbers, FT-1692): commits `project.drawing.set`
    /// with `wire_numbers: true`, or with `null` (the member removed: no numbers, as a project without the setting) to
    /// turn it off; without `on` it toggles. The pages, the PDF and print follow the setting; a toast says what it now
    /// is, for the whole project.
    pub(crate) fn act_wire_numbers(&mut self, p: &Map<String, Value>) -> Outcome {
        let on = if p.contains_key("on") {
            flag(p, "on")
        } else {
            !self.wire_numbers_on()
        };
        let cmd = ew_api::AnyCommand {
            name: "project.drawing.set".into(),
            params: json!({ "wire_numbers": if on { json!(true) } else { Value::Null } }),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            let said = if on {
                key!("toast-wire-numbers-on")
            } else {
                key!("toast-wire-numbers-off")
            };
            self.notify(strings::text(said).to_owned(), true);
        }
        out
    }

    /// `project.wire_numbering {on?}` (Draw ▸ Connect ▸ Number wires by potential, doc 08 §5.2): commits
    /// `project.drawing.set` with `wire_numbering: per_potential` (every wire of a potential prints its potential's
    /// code), or with `null` (each wire its own number) to turn it off; without `on` it toggles. A toast says what the
    /// wires now get, and while Print wire numbers is off that the numbers show once it is on.
    pub(crate) fn act_wire_numbering(&mut self, p: &Map<String, Value>) -> Outcome {
        let on = if p.contains_key("on") {
            flag(p, "on")
        } else {
            !self.wire_numbering_per_potential()
        };
        let cmd = ew_api::AnyCommand {
            name: "project.drawing.set".into(),
            params: json!({ "wire_numbering": if on { json!("per_potential") } else { Value::Null } }),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            let said = if on {
                key!("toast-wire-numbering-potential")
            } else {
                key!("toast-wire-numbering-own")
            };
            let mut text = strings::text(said).to_owned();
            if !self.wire_numbers_on() {
                text = format!("{text} {}", strings::text(key!("toast-wire-numbers-hidden")));
            }
            self.notify(text, true);
        }
        out
    }

    /// The row key of Print wire numbers in the project's Properties ([`ShellModel::wire_numbers_in_row`]).
    pub(super) const WIRE_NUMBERS_ROW: &'static str = "project/settings.drawing.wire_numbers";

    /// The project's sheet gets Print wire numbers as a checkbox of Main (the switch three of four persona runs looked
    /// for there and under the filter "wire"); ticking it runs `project.wire_numbers` ([`ShellModel::commit_text`]).
    pub(super) fn wire_numbers_in_row(&self, sheet: &mut PropertySheet) {
        let on = self.wire_numbers_on();
        for section in sheet.sections.iter_mut().filter(|s| s.kind == "project") {
            let Some(model) = section.rows.first().cloned() else {
                continue;
            };
            section.rows.push(PropRow {
                key: Self::WIRE_NUMBERS_ROW.to_owned(),
                path: "settings.drawing.wire_numbers".to_owned(),
                category: crate::properties::Category::Other,
                depth: 0,
                ty: crate::properties::FieldType::Bool,
                value: RowValue::One {
                    json: Value::Bool(on),
                    text: on.to_string(),
                },
                actions: vec![RowAction::Edit],
                route: crate::properties::EditRoute::Command {
                    command: "project.drawing.set".to_owned(),
                },
                identifier: false,
                source: None,
                ..model
            });
        }
    }

    /// Print wire numbers ticked or cleared in the project's Properties: `project.wire_numbers` with `on`; `None` for
    /// another row.
    pub(super) fn commit_wire_numbers_row(&mut self, key: &str, text: &str) -> Option<Outcome> {
        if key != Self::WIRE_NUMBERS_ROW {
            return None;
        }
        // The checkbox sends `yes` or `no`, as for every other yes-or-no row.
        let on = match text.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => true,
            "false" | "no" | "0" => false,
            _ => return Some(failed(key!("error-bad-param"), &["value"])),
        };
        let mut p = Map::new();
        p.insert("on".to_owned(), Value::Bool(on));
        let out = self.act_wire_numbers(&p);
        match &out {
            Outcome::Failed { message } => self.props.set_error(key.to_owned(), message.clone()),
            _ => {
                self.props.edit = None;
                self.props.error = None;
            }
        }
        Some(out)
    }

    /// `project.plc_addresses_at_terminals {on?}` (Project ▸ Channel addresses at terminals, FR-PLC-018, doc 10 §3.5):
    /// commits `project.drawing.set` with `plc_address_at_terminals: true`, or `null` to turn it off; without `on` it
    /// toggles. A terminal on a traced channel's path then prints the channel's address in its `PlcAddress` slot.
    pub(crate) fn act_plc_addresses_at_terminals(&mut self, p: &Map<String, Value>) -> Outcome {
        let on = if p.contains_key("on") {
            flag(p, "on")
        } else {
            !self.plc_addresses_at_terminals_on()
        };
        let cmd = ew_api::AnyCommand {
            name: "project.drawing.set".into(),
            params: json!({ "plc_address_at_terminals": if on { json!(true) } else { Value::Null } }),
        };
        self.run_commit(&cmd)
    }

    /// Whether the project prints Channel addresses at terminals (the drawing setting `plc_address_at_terminals`).
    pub fn plc_addresses_at_terminals_on(&self) -> bool {
        self.project.as_ref().is_some_and(|p| {
            p.session
                .query()
                .project()
                .settings
                .extra
                .get("drawing")
                .and_then(|d| d.get("plc_address_at_terminals"))
                .and_then(Value::as_bool)
                == Some(true)
        })
    }

    /// `navigator.bind {channel, x_mm, y_mm, group?}` (FR-PLC-020): a PLC channel row dropped on the
    /// canvas at the sheet point. The connection point under it, resolved as the Wire tool resolves its targets (the
    /// nearest CP within [`crate::tools::CP_SNAP_PT`] screen points), takes the channel: one `plc.bind_channel` with `endpoint: {cp: {function,
    /// idx}}`, whose own rules decide. A drop with no CP under it is refused with `reason-drop-on-cp`.
    ///
    /// A CP off the channel's card (`Session::off_card_bind`) is reached in the model's terms: a device
    /// **wired** to one channel pin of the card binds the channel to that pin (wired to two or more, the drop is
    /// refused with `reason-bind-wired-pins` and a toast naming the pins, so that the channel is dropped on the pin
    /// meant); a device not wired to the card, when the card is a valve terminal with a free position, is attached
    /// there (`assembly.attach`) and then bound where it was dropped, two commands and two undo steps that the toast
    /// names; **neither** is refused before the commit with `reason-bind-off-card` and a toast that says what to do.
    /// A bind that unbinds another channel (FR-PLC-005) names it in the toast by its address and the pin.
    pub(crate) fn act_navigator_bind(&mut self, p: &Map<String, Value>) -> Outcome {
        let channel = match string_param(p, "channel") {
            Ok(Some(c)) => c.trim().trim_start_matches("id:").to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["channel"]),
            Err(e) => return e,
        };
        let Ok(ch) = channel.parse::<ew_api::output::Uuid>() else {
            return failed(key!("error-bad-param"), &["channel"]);
        };
        let at = match super::canvas_actions::point_mm(p) {
            Ok(Some(at)) => at,
            Ok(None) => return failed(key!("error-missing-param"), &["x_mm"]),
            Err(e) => return e,
        };
        let g = or_return!(self.group_param(p));
        if let Some(a) = &mut self.area
            && a.focused != g
        {
            a.focused = g;
        }
        let Some(c) = self.canvas() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let (page, ppn, pane) = (Arc::clone(c.page()), c.points_per_nm(), c.pane());
        let cx = crate::tools::ToolCx {
            page: &page,
            points_per_nm: ppn,
            pane,
            snapper: &page.snapper,
            selection: &self.tools.selection,
            layout: self.tools.layout_options,
            connect_on_move: self.tools.connect_on_move,
        };
        let crate::tools::wire::WireTarget::Cp { function, idx, .. } =
            crate::tools::wire::resolve_target(&cx, at, false)
        else {
            return Outcome::refused(key!("reason-drop-on-cp"));
        };
        let Some(pr) = self.project.as_ref() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        // (where the channel binds, the device wired to that card pin, the attachment: device, tag, card, card tag,
        // position)
        type Attach = (ew_api::output::Uuid, String, ew_api::output::Uuid, String, u32);
        let (target, wired_to, attach): ((ew_api::output::Uuid, u16), Option<String>, Option<Attach>) =
            match pr.session.off_card_bind(ch, function.0, idx) {
                // A valve's symbol stands for its internal coil (FT-740).
                None => (
                    pr.session
                        .valve_coil_target(ch, function.0, idx)
                        .unwrap_or((function.0, idx)),
                    None,
                    None,
                ),
                Some(OffCardBind::Wired { device, card, pins }) => match pins.as_slice() {
                    [pin] => ((pin.function, pin.idx), Some(device), None),
                    _ => {
                        let list: Vec<String> = pins
                            .iter()
                            .map(|p| crate::describe::pin_in_sentence(&p.label))
                            .collect();
                        let text = strings::format(
                            key!("toast-bind-wired-pins"),
                            &[&device, &card, &pins.len().to_string(), &list.join(", ")],
                        );
                        self.notify_toned(text, crate::notifications::NoteTone::Warning);
                        return Outcome::refused(key!("reason-bind-wired-pins"));
                    }
                },
                Some(OffCardBind::Attach {
                    device,
                    tag,
                    card,
                    card_tag,
                    position,
                }) => (
                    (function.0, idx),
                    None,
                    Some((device, tag, card, card_tag, position)),
                ),
                // Refused here in words, by the rule the command applies (`is_on_module`), not left to the
                // command's error with record IDs; the words say what to do.
                Some(OffCardBind::Neither {
                    device,
                    card,
                    valve_terminal,
                }) => {
                    let text = match (device.is_empty(), valve_terminal) {
                        (true, _) => strings::format(key!("toast-bind-off-card-untagged"), &[&card]),
                        (false, false) => strings::format(key!("toast-bind-off-card"), &[&device, &card]),
                        (false, true) => strings::format(key!("toast-bind-no-position"), &[&device, &card]),
                    };
                    self.notify_toned(text, crate::notifications::NoteTone::Warning);
                    return Outcome::refused(key!("reason-bind-off-card"));
                }
            };
        // The channels the bind unbinds (FR-PLC-005), named in the toast by their addresses.
        let words = pr.session.channel_words(ch).unwrap_or_default();
        let pin = crate::describe::pin_in_sentence(&pr.session.cp_text(target.0, target.1));
        let displaced: Vec<String> = pr
            .session
            .channels_bound_at(target.0, target.1, ch)
            .into_iter()
            .filter_map(|o| pr.session.channel_words(o))
            .collect();
        // A device wired off the card is attached to it by the same command, one undo step (FT-774).
        let mut params = json!({
            "channel": channel,
            "endpoint": {"cp": {"function": target.0.to_string(), "idx": target.1}},
        });
        if let Some((_, _, _, _, position)) = &attach {
            params["attach_at"] = json!(position);
        }
        let cmd = ew_api::AnyCommand {
            name: "plc.bind_channel".into(),
            params,
        };
        let out = self.run_commit(&cmd);
        if out != Outcome::Done {
            return out;
        }
        let mut text = match (&attach, &wired_to) {
            (Some((_, tag, _, card_tag, position)), _) => strings::format(
                key!("toast-bind-attached"),
                &[tag, card_tag, &position.to_string(), &words, &pin],
            ),
            (None, Some(device)) => strings::format(key!("toast-bind-wired"), &[&words, &pin, device]),
            (None, None) => strings::format(key!("toast-bind-done"), &[&words, &pin]),
        };
        // Each displaced channel goes inside the sentence, before its full stop.
        for d in &displaced {
            text = format!(
                "{}{}.",
                strings::without_stop(&text),
                strings::format(key!("toast-bind-displaced"), &[d])
            );
        }
        self.notify(text, true);
        Outcome::Done
    }

    /// The tool context of the focused canvas for a host duty that calls the active tool directly.
    fn with_tool_cx<R>(
        &mut self,
        f: impl FnOnce(&mut dyn Tool, &crate::tools::ToolCx<'_>) -> R,
    ) -> Option<R> {
        let c = self.canvas()?;
        let (page, ppn, pane) = (Arc::clone(c.page()), c.points_per_nm(), c.pane());
        let host = &mut self.tools;
        let cx = crate::tools::ToolCx {
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

    /// Enter with Select idle and exactly one junction selected opens its editor; `None` otherwise.
    pub(super) fn enter_opens_junction(&mut self) -> Option<Outcome> {
        if self.tools.tool.name() != "select" || self.tools.tool.state() != "idle" {
            return None;
        }
        let tables = Arc::clone(&self.canvas()?.page().tables);
        let kinds = selection_kinds(&self.tools.selection, &tables);
        match kinds.as_slice() {
            [(_, EntityKind::Junction)] => Some(self.act_junction_open()),
            _ => None,
        }
    }

    /// Sends a tool key that only some tools take (Tab): the active tool answers.
    /// `tool.wire_dot` (`.`): while a wire is drawn, the joint it makes on a wire is a dot instead of the bevel, or the
    /// bevel again (docs/design/06a-drafting.md §7.6); refused in words while no wire is drawn.
    pub(crate) fn act_wire_dot(&mut self) -> Outcome {
        if self.tools.tool.name() != "wire" || self.tools.tool.state() != "routing" {
            return Outcome::refused(key!("reason-wire-dot"));
        }
        self.send_tool(&CanvasEvent::Key(crate::tools::ToolKey::Dot))
    }

    pub(crate) fn send_proposal_key(&mut self) -> Outcome {
        self.send_tool(&CanvasEvent::Key(crate::tools::ToolKey::NextProposal))
    }

    // -----------------------------------------------------------------------------------------------------------
    // Route (docs/design/06a-drafting.md §7.1, §7.2)

    /// `layout.route` (Route): the plan of the selected wires and every wire with an end on a selected symbol, or with
    /// nothing of the page selected, of the page's wires that are unrouted, broken or routed by the program (doc 06a §3.2).
    /// Nothing to do: a toast says so, and names the number of wires a page-wide Route left as the person's; where
    /// routing those anew would make the page better, its action Route these too… previews that ([`Self::route_his_card`]).
    /// No route of the person's would change: applied at once as one `layout.apply` (one undo step), a toast names the
    /// wires with Undo and the page draws them as selected while it is up; the wires without a way are named with what
    /// is in it. A broken route the person drew would change: a preview card with those wires named, and nothing
    /// changes until Apply.
    pub(crate) fn act_layout_route(&mut self) -> Outcome {
        let ids = self.route_selection_ids();
        // A Route on a selection says so, and how to route the page (a persona run read "Routed 1 wire" and had to
        // guess that the selection was the scope).
        let selected = ids.len();
        let hint = (selected > 0).then(|| strings::text(key!("route-selection-hint")).to_owned());
        let scope = if ids.is_empty() {
            ew_api::routing::RouteScope::Page
        } else {
            ew_api::routing::RouteScope::Selection(ids)
        };
        let Some(folio) = self.current_folio().map(str::to_owned) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let Some(session) = self.project.as_ref().map(|p| &p.session) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(plan) = session.route_plan(&folio, &scope) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let rows = route_rows(session, &folio, &plan);
        let changed = plan.changed();
        let kept = plan.kept.len();
        let Some(params) = plan.params.clone() else {
            // Nothing changed: the wires left as the person's are named by their number, with the offer where routing
            // them anew would make the page better.
            let gain = kept > 0
                && session
                    .route_plan(&folio, &ew_api::routing::RouteScope::Selection(plan.kept.clone()))
                    .is_some_and(|p| p.params.is_some());
            let lead = match (gain, kept) {
                (false, _) if selected > 0 => {
                    strings::format_count(crate::count_key!("toast-route-nothing-selected"), selected)
                }
                (false, _) => strings::text(key!("toast-route-nothing")).to_owned(),
                (true, 1) => strings::text(key!("toast-route-kept-one")).to_owned(),
                (true, n) => strings::format(key!("toast-route-kept"), &[&n.to_string()]),
            };
            let text = std::iter::once(lead)
                .chain(rows)
                .chain(hint)
                .collect::<Vec<_>>()
                .join(" ");
            let page = PageRow::by_folio(self.page_rows(), &folio).map(|r| r.id);
            match page.filter(|_| gain) {
                Some(page) => self.notify_offer(text, crate::notifications::NoteAction::RouteHis, vec![page]),
                None => self.notify(text, false),
            }
            return Outcome::Done;
        };
        let names = wire_names(session, &folio, &changed);
        if !plan.plan.his.is_empty() {
            let page = strings::format(key!("route-folio"), &[&folio]);
            let his = plan.plan.his.len().to_string();
            let lead = strings::format(
                key!("route-preview-lead"),
                &[&page, &changed.len().to_string(), &his],
            );
            return self.preview_host_card_as(
                "layout.apply",
                params,
                key!("reason-route"),
                crate::properties::CardOrigin::Route {
                    lead,
                    rows,
                    page,
                    routed: changed.len(),
                },
            );
        }
        let out = self.run_commit(&ew_api::AnyCommand {
            name: "layout.apply".into(),
            params,
        });
        if out != Outcome::Done {
            return out;
        }
        let n = changed.len().to_string();
        let lead = match (changed.len(), selected > 0) {
            (1, false) => strings::format(key!("toast-route-done-one"), &[&names]),
            (_, false) => strings::format(key!("toast-route-done"), &[&n, &names]),
            (1, true) => strings::format(key!("toast-route-done-selected-one"), &[&names]),
            (_, true) => strings::format(key!("toast-route-done-selected"), &[&n, &names]),
        };
        let kept = match kept {
            0 => None,
            1 => Some(strings::text(key!("route-kept-one")).to_owned()),
            n => Some(strings::format(key!("route-kept"), &[&n.to_string()])),
        };
        let text = std::iter::once(lead)
            .chain(rows)
            .chain(kept)
            .chain(hint)
            .collect::<Vec<_>>()
            .join(" ");
        self.notify(text, true);
        self.tools.last_routed = Some((self.notes.newest_serial(), self.revision(), changed));
        Outcome::Done
    }

    /// Route these too… (the action of Route's toast): the preview card of Route over the wires a page-wide Route
    /// leaves as the person's on the page on show, the card a broken route of his gets; nothing changes until Apply,
    /// one undo step. Where none would run better now, a toast says so.
    pub(super) fn route_his_card(&mut self) -> Outcome {
        let Some(folio) = self.current_folio().map(str::to_owned) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let Some(session) = self.project.as_ref().map(|p| &p.session) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(kept) = session
            .route_plan(&folio, &ew_api::routing::RouteScope::Page)
            .map(|p| p.kept)
        else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let plan = session.route_plan(&folio, &ew_api::routing::RouteScope::Selection(kept));
        let Some((plan, params)) = plan.and_then(|p| p.params.clone().map(|params| (p, params))) else {
            self.notify(strings::text(key!("toast-route-nothing")).to_owned(), false);
            return Outcome::Done;
        };
        let rows = route_rows(session, &folio, &plan);
        let routed = plan.changed().len();
        let page = strings::format(key!("route-folio"), &[&folio]);
        let lead = if routed == 1 {
            strings::format(key!("route-preview-lead-his-one"), &[&page])
        } else {
            strings::format(key!("route-preview-lead-his"), &[&page, &routed.to_string()])
        };
        self.preview_host_card_as(
            "layout.apply",
            params,
            key!("reason-route"),
            crate::properties::CardOrigin::Route {
                lead,
                rows,
                page,
                routed,
            },
        )
    }

    /// The wires the last Route changed, while its toast is on screen and the project is at the revision Route left:
    /// the page draws them as it draws a selected wire, so a person sees which ones moved.
    pub(super) fn last_routed_wires(&self) -> &[ew_api::output::Uuid] {
        match &self.tools.last_routed {
            Some((serial, rev, ids))
                if *rev == self.revision() && self.notes.visible().iter().any(|n| n.serial == *serial) =>
            {
                ids
            }
            _ => &[],
        }
    }

    /// The routes an open Route card plans on the page of canvas `group`, in sheet nm, while the card is current: the
    /// canvas draws them as the preview's ghost until Apply or Cancel.
    pub fn route_preview(&self, group: usize) -> Vec<Vec<(i64, i64)>> {
        let Some(card) = self
            .props
            .card
            .as_ref()
            .filter(|c| matches!(c.origin, crate::properties::CardOrigin::Route { .. }))
            .filter(|c| self.revision() == Some(c.preview.base_rev))
        else {
            return Vec::new();
        };
        let Some(canvas) = self.canvas_of(group) else {
            return Vec::new();
        };
        let page = canvas.page();
        let routes = card.preview.command["routes"]
            .as_array()
            .map_or(&[][..], Vec::as_slice);
        routes
            .iter()
            .filter(|r| {
                r["connection"]
                    .as_str()
                    .is_some_and(|id| page.tables.connections.iter().any(|t| t.id.to_string() == id))
            })
            .map(|r| {
                r["route"]
                    .as_array()
                    .map_or(&[][..], Vec::as_slice)
                    .iter()
                    .filter_map(|p| Some(page.snapper.page_to_sheet((p[0].as_i64()?, p[1].as_i64()?))))
                    .collect()
            })
            .collect()
    }

    /// The wire whose handles the keyboard reaches: with Select at rest, the one connection selected on the focused
    /// page, and its line.
    pub(crate) fn wire_handles(&self) -> Option<(EntityRef, Vec<(i64, i64)>)> {
        if self.tools.tool.name() != "select" || self.tools.tool.state() != "idle" {
            return None;
        }
        let page = self.canvas()?.page();
        let [one] = self.tools.selection.items.as_slice() else {
            return None;
        };
        let c = page.tables.connections.iter().find(|c| c.id == one.0)?;
        Some((*one, crate::tools::autoconnect::dedup(&c.polyline)))
    }

    /// The wire whose handle has the keyboard's focus (`route.handle`), while one has.
    pub(crate) fn handle_focus(&self) -> Option<EntityRef> {
        self.tools.handle.map(|(w, _)| w)
    }

    /// `route.handle {step}` (Tab +1, Shift+Tab −1): the focus goes to the next or the previous handle of the
    /// one selected wire along the wire as a person reads it, from its end that comes first (the upper, else the left
    /// one) to the other, round at its ends; the first
    /// Tab takes the first, the first Shift+Tab the last. Refused without one wire selected.
    pub(crate) fn act_route_handle(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some((wire, line)) = self.wire_handles() else {
            return Outcome::refused(key!("reason-route-handle"));
        };
        let step = p.get("step").and_then(Value::as_i64).unwrap_or(1)
            * if crate::tools::route_edit::reads_backwards(&line) {
                -1
            } else {
                1
            };
        let n = i64::try_from(crate::tools::route_edit::handle_parts(&line).len()).unwrap_or(0);
        if n == 0 {
            return Outcome::refused(key!("reason-route-handle"));
        }
        let next = match self.tools.handle {
            Some((w, k)) if w == wire => i64::try_from(k).unwrap_or(0) + step.signum(),
            _ if step < 0 => n - 1,
            _ => 0,
        };
        self.tools.handle = Some((wire, usize::try_from(next.rem_euclid(n)).unwrap_or(0)));
        Outcome::Done
    }

    /// `route.handle_move {dx, dy}` (the arrow keys while a handle has the focus): the focused handle moved by
    /// grid steps as a drag moves it (a segment across itself, a bend both ways), committed as `connection.route` in
    /// one undo step; the focus stays on the moved handle. Refused without a focused handle.
    pub(crate) fn act_route_handle_move(&mut self, p: &Map<String, Value>) -> Outcome {
        use crate::tools::route_edit::{
            GripMove, grip_move, handle_at, handle_parts, moved_part, route_command,
        };
        use crate::tools::wire::PageGridMap;
        let (dx, dy) = (
            p.get("dx").and_then(Value::as_i64).unwrap_or(0),
            p.get("dy").and_then(Value::as_i64).unwrap_or(0),
        );
        let (Some((wire, line)), Some((focused, k))) = (self.wire_handles(), self.tools.handle) else {
            return Outcome::refused(key!("reason-route-handle-move"));
        };
        let parts = handle_parts(&line);
        let (Some(&part), true) = (parts.get(k), focused == wire) else {
            return Outcome::refused(key!("reason-route-handle-move"));
        };
        let Some(grid) = self.canvas().map(|c| PageGridMap::new(c.page().tables.grid)) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        // A segment moves across itself only: a step along it moves nothing, and says so in the status line (a persona
        // run's arrow key did nothing without a word).
        let steps = match grip_move(&line, part) {
            GripMove::UpDown => (0, dy),
            GripMove::Sideways => (dx, 0),
            GripMove::Free => (dx, dy),
        };
        if steps == (0, 0) {
            let note = match grip_move(&line, part) {
                GripMove::Sideways => key!("status-handle-moves-sideways"),
                _ => key!("status-handle-moves-up-down"),
            };
            self.note_status(strings::text(note).to_owned());
            return Outcome::Done;
        }
        let moved = moved_part(&line, part, steps, grid).filter(|m| *m != line);
        let (Some(moved), Some(at)) = (moved, handle_at(&line, part)) else {
            self.note_status(strings::text(key!("status-handle-cannot-move")).to_owned());
            return Outcome::Done;
        };
        let out = self.commit_from_host(route_command(wire, &moved).as_ref());
        // A route the page refuses (an end off its point, a corner no longer square) moves nothing and says why.
        if let Outcome::Failed { message } = &out {
            let note = strings::format(key!("status-handle-refused"), &[message]);
            self.note_status(note);
            return Outcome::Done;
        }
        if out == Outcome::Done {
            // The focus follows the handle: the one nearest to where it went.
            let want = (at.0 + steps.0 * grid.step(), at.1 + steps.1 * grid.step());
            let line = self.wire_handles().map_or(moved, |(_, l)| l);
            let near = handle_parts(&line)
                .iter()
                .enumerate()
                .filter_map(|(i, &q)| handle_at(&line, q).map(|h| (i, h)))
                .min_by_key(|(_, h)| (h.0 - want.0).abs() + (h.1 - want.1).abs())
                .map(|(i, _)| i);
            self.tools.handle = near.map(|i| (wire, i));
        }
        out
    }

    /// The selected connections and representations of the focused page, which Route routes.
    pub(crate) fn route_selection_ids(&self) -> Vec<ew_api::output::Uuid> {
        self.canvas().map_or_else(Vec::new, |c| {
            selection_kinds(&self.tools.selection, &c.page().tables)
                .into_iter()
                .filter(|(_, k)| matches!(k, EntityKind::Connection | EntityKind::Rep))
                .map(|(e, _)| e.0)
                .collect()
        })
    }

    /// Apply on a Route card: the plan's `layout.apply` commits as a bulk card's command does, and the toast says how
    /// many wires it routed.
    pub(super) fn apply_route_card(&mut self, card: crate::properties::Card) -> Outcome {
        let toast = match &card.origin {
            crate::properties::CardOrigin::Route { page, routed, .. } => strings::format(
                if *routed == 1 {
                    key!("toast-route-applied-one")
                } else {
                    key!("toast-route-applied")
                },
                &[&routed.to_string(), page],
            ),
            _ => String::new(),
        };
        let fresh = self.revision() == Some(card.preview.base_rev) && card.preview.result.is_ok();
        let out = self.apply_bulk_card(card);
        if fresh && out == Outcome::Done {
            self.notify(toast, true);
        }
        out
    }
}

/// How many wires Route names in its toast before it says how many more there are.
const NAMED: usize = 3;

/// The wires `ids` of page `folio` as Route's toast names them: each by its number, else by its ends in words,
/// the first [`NAMED`] of them and then how many more.
fn wire_names(session: &ew_api::Session, folio: &str, ids: &[ew_api::output::Uuid]) -> String {
    let name = |id: &ew_api::output::Uuid| match session.selection_words(*id) {
        Some(ew_api::output::SelectionWords::Wire {
            number: Some(number), ..
        }) => number,
        _ => session
            .connection_words(folio, *id)
            .map(|t| super::tools::rejected_words("", Some(&t)))
            .unwrap_or_default(),
    };
    let mut names: Vec<String> = ids.iter().take(NAMED).map(name).collect();
    if ids.len() > NAMED {
        let more = (ids.len() - NAMED).to_string();
        names.push(strings::format(key!("route-more"), &[&more]));
    }
    super::external::and_list(&names)
}

/// One sentence per wire a plan leaves without a way, with what is in it; per broken wire of the person's it replaces;
/// per net whose wiring order runs against its parts' places; each naming the wire by its ends in words (FT-1600).
pub(super) fn route_rows(
    session: &ew_api::Session,
    folio: &str,
    plan: &ew_api::routing::RoutePlan,
) -> Vec<String> {
    use ew_api::routing::Reason;
    let words = |id: ew_api::output::Uuid| {
        session
            .connection_words(folio, id)
            .map(|t| super::tools::rejected_words("", Some(&t)))
            .unwrap_or_default()
    };
    let p = &plan.plan;
    p.unrouted
        .iter()
        .map(|(id, reason)| {
            let key = match reason {
                Reason::Wires => key!("route-reason-wires"),
                Reason::Parts => key!("route-reason-symbols"),
                Reason::Points => key!("route-reason-points"),
                Reason::FourArms => key!("route-reason-four-way"),
            };
            strings::format(key, &[&words(id.0)])
        })
        .chain(
            p.his
                .iter()
                .map(|id| strings::format(key!("route-his"), &[&words(id.0)])),
        )
        .chain(p.notes.iter().filter_map(|net| {
            let first = net.first()?;
            Some(strings::format(key!("route-against-order"), &[&words(first.0)]))
        }))
        .collect()
}
