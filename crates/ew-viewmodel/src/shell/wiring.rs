// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's duties of WP-S2.04 part 2 (§8): arming the Wire tool and the keyboard route editor, the junction
//! sequence editor popup and its commits, the crossings setting, and the Wire tool's live region.
//!
//! **Reads.** The junction editor reads the page tables the canvas already holds and `Session::junction_wires` once
//! per (session, revision, junction) while it is open; the Wire tool and the proposals read nothing (§12 item 4).
//!
//! **Commits** go through the tool's commit path (`execute_with` in the window, `run_script` headless), one
//! `junction.edit` per change (D11).

use std::sync::Arc;

use ew_api::output::OffCardBind;
use ew_api::output::ew_dl::EntityRef;
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, flag, string_param};
use crate::strings;
use crate::tools::{CanvasEvent, EntityKind, JunctionEditor, RouteEditTool, Tool, WireTool, selection_kinds};

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The junction sequence editor, while open (WP-S2.04 §8.5).
    pub fn junction_editor(&self) -> Option<&JunctionEditor> {
        self.tools.junction_editor.as_ref()
    }

    /// The text of a canvas key the host refused, shown in place of the tool's hint until the next canvas input.
    pub fn tool_notice(&self) -> Option<&str> {
        self.tools.notice.as_deref()
    }

    /// Exactly one junction is selected on the focused page (`junction.open`'s condition, §8.5).
    pub fn one_junction_selected(&self) -> bool {
        self.canvas().is_some_and(|c| {
            let kinds = selection_kinds(&self.tools.selection, &c.page().tables);
            matches!(kinds.as_slice(), [(_, EntityKind::Junction)])
        })
    }

    /// Exactly one connection is selected on the focused page (`tool.route_edit`'s condition, §8.4).
    pub fn one_connection_selected(&self) -> bool {
        self.canvas().is_some_and(|c| {
            let kinds = selection_kinds(&self.tools.selection, &c.page().tables);
            matches!(kinds.as_slice(), [(_, EntityKind::Connection)])
        })
    }

    /// Whether the project draws crossings as hops (`settings.extra.drawing.crossings`, WP-S2.04 §8.6).
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

    /// The project's `marker_mode` setting when it is set (WP-S2.04 §10.2 item 3): `all`, `deviations`,
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

    /// How many times the active Wire tool ran the router (the exact-count tests, §12 item 4); `None` for another
    /// tool.
    pub fn wire_route_calls(&mut self) -> Option<u64> {
        self.tools
            .tool
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<WireTool>())
            .map(|w| w.route_calls())
    }

    /// Whether the focused page's type autoconnects (doc 06 §2.2: multi-line and single-line pages).
    pub(super) fn page_autoconnects(&self) -> bool {
        matches!(
            self.page_rows().get(self.page_index()).map(|r| r.kind.as_str()),
            Some("multi_line" | "single_line")
        )
    }

    // -----------------------------------------------------------------------------------------------------------
    // The Wire tool and the route editor

    /// The connection kind the Wire tool draws on the focused page (doc 06 §2.2): `Ok(None)` a wire on a multi-line
    /// page, `Ok(Some("overview"))` on a single-line (the MVP kind), overview or assembly-overview page; refused on
    /// a bus-topology or fluid page (their `Bus` and `Fluid` kinds are not drawn by this package) and on every page
    /// type whose Wire tool creates nothing (fix round of WP-S2.04 part 2, item 1).
    pub fn wire_kind(&self) -> Result<Option<&'static str>, &'static str> {
        match self.page_rows().get(self.page_index()).map(|r| r.kind.as_str()) {
            Some("multi_line") => Ok(None),
            Some("single_line" | "overview" | "assembly_overview") => Ok(Some("overview")),
            _ => Err("reason-no-wire-kind"),
        }
    }

    /// `tool.wire` (W, §8.7): arms the Wire tool with the page type's connection kind.
    pub(super) fn act_tool_wire(&mut self) -> Outcome {
        match self.wire_kind() {
            Ok(kind) => {
                self.activate_tool(Box::new(WireTool::new().with_kind(kind)));
                Outcome::Done
            }
            Err(reason_key) => Outcome::Refused { reason_key },
        }
    }

    /// `tool.route_edit` (§8.4): the keyboard route editor of exactly one selected connection.
    pub(super) fn act_route_edit(&mut self) -> Outcome {
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let tables = Arc::clone(&c.page().tables);
        let cursor = c.cursor();
        let connections: Vec<EntityRef> = selection_kinds(&self.tools.selection, &tables)
            .into_iter()
            .filter(|(_, k)| *k == EntityKind::Connection)
            .map(|(e, _)| e)
            .collect();
        let [one] = connections.as_slice() else {
            return Outcome::Refused {
                reason_key: "reason-one-connection",
            };
        };
        let Some(row) = tables.connections.iter().find(|x| x.id == one.0) else {
            return Outcome::Refused {
                reason_key: "reason-one-connection",
            };
        };
        let tool = RouteEditTool::new(row, cursor);
        self.activate_tool(Box::new(tool));
        let overlay = self
            .with_tool_cx(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<RouteEditTool>())
                    .map(|r| r.overlay(cx))
            })
            .flatten();
        if let Some(o) = overlay {
            self.tools.overlay = o;
        }
        Outcome::Done
    }

    /// The live region of the Wire tool while it routes (§8.2): `Wire: routing from <label>. …`.
    pub(super) fn announce_wire(&mut self) {
        let label = self
            .with_tool_cx(|t, _| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<WireTool>())
                    .and_then(|w| w.start_label().map(str::to_owned))
            })
            .flatten();
        if let Some(label) = label {
            self.tools.announcement = Some(strings::format("announce-wire-routing", &[&label]));
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

    /// `junction.open` (§8.5): the editor of exactly one selected junction (the registry disables it otherwise, naming
    /// what to select). A script selects the junction first (`select.set` with its ID).
    pub(super) fn act_junction_open(&mut self) -> Outcome {
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let tables = Arc::clone(&c.page().tables);
        let js: Vec<EntityRef> = selection_kinds(&self.tools.selection, &tables)
            .into_iter()
            .filter(|(_, k)| *k == EntityKind::Junction)
            .map(|(e, _)| e)
            .collect();
        let [j] = js.as_slice() else {
            return Outcome::Refused {
                reason_key: "reason-one-junction",
            };
        };
        match self.read_junction_editor(*j, 0) {
            Some(e) => {
                self.palette = None;
                self.tools.picker = None;
                self.tools.junction_editor = Some(e);
                Outcome::Done
            }
            None => Outcome::Refused {
                reason_key: "reason-one-junction",
            },
        }
    }

    /// Commits `junction.edit` of the open editor with `members`, then reads the editor again with `row` focused.
    fn junction_commit(&mut self, members: Value, row: usize) -> Outcome {
        let Some(editor) = self.tools.junction_editor.clone() else {
            return Outcome::Refused {
                reason_key: "reason-no-junction-editor",
            };
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
    pub(super) fn act_junction_member(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(editor) = &self.tools.junction_editor else {
            return Outcome::Refused {
                reason_key: "reason-no-junction-editor",
            };
        };
        let index = |name: &str| {
            p.get(name)
                .and_then(Value::as_u64)
                .and_then(|v| usize::try_from(v).ok())
        };
        let (Some(from), Some(to)) = (index("from"), index("to")) else {
            return failed("error-missing-param", &["from"]);
        };
        let Some(seq) = editor.reordered(from, to) else {
            return Outcome::Refused {
                reason_key: "reason-junction-member",
            };
        };
        let ids: Vec<String> = seq.iter().map(|c| c.0.to_string()).collect();
        self.junction_commit(json!({ "sequence": ids }), to)
    }

    /// `junction.style {style, orient?}`: `dot` or `bevel` with its orientation (0 to 7).
    pub(super) fn act_junction_style(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(editor) = &self.tools.junction_editor else {
            return Outcome::Refused {
                reason_key: "reason-no-junction-editor",
            };
        };
        let style = match string_param(p, "style") {
            Ok(Some(s)) => s.to_owned(),
            Ok(None) => return failed("error-missing-param", &["style"]),
            Err(e) => return e,
        };
        let orient = match p.get("orient") {
            None => editor.bevel_orient,
            Some(v) => match v.as_u64().and_then(|o| u8::try_from(o).ok()).filter(|o| *o < 8) {
                Some(o) => o,
                None => return failed("error-bad-param", &["orient"]),
            },
        };
        let Some(value) = JunctionEditor::style_json(&style, orient) else {
            return failed("error-bad-param", &["style"]);
        };
        let row = editor.row;
        self.junction_commit(json!({ "style": value }), row)
    }

    /// `junction.wiring {wiring}`: `star`, `chain` or `style_default`.
    pub(super) fn act_junction_wiring(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(editor) = &self.tools.junction_editor else {
            return Outcome::Refused {
                reason_key: "reason-no-junction-editor",
            };
        };
        let wiring = match string_param(p, "wiring") {
            Ok(Some(w @ ("star" | "chain" | "style_default"))) => w.to_owned(),
            Ok(Some(_)) => return failed("error-bad-param", &["wiring"]),
            Ok(None) => return failed("error-missing-param", &["wiring"]),
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

    /// `project.crossings {on?}` (§8.6): commits `project.drawing.set` with `crossings: hops`, or with `null` (the
    /// member removed: plain, as a project without the setting) to turn it off; without `on` it toggles.
    pub(super) fn act_crossings(&mut self, p: &Map<String, Value>) -> Outcome {
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

    /// `navigator.bind {channel, x_mm, y_mm, group?}` (WP-S2.07 §8, FR-PLC-020): a PLC channel row dropped on the
    /// canvas at the sheet point. The connection point under it, resolved as the Wire tool resolves its targets (the
    /// nearest CP within 8 screen points), takes the channel: one `plc.bind_channel` with `endpoint: {cp: {function,
    /// idx}}`, whose own rules decide. A drop with no CP under it is refused with `reason-drop-on-cp`.
    ///
    /// A CP off the channel's card (FT-753, `Session::off_card_bind`) is reached in the model's terms: a device
    /// **wired** to one channel pin of the card binds the channel to that pin (wired to two or more, the drop is
    /// refused with `reason-bind-wired-pins` and a toast naming the pins, so that the channel is dropped on the pin
    /// meant); a device not wired to the card, when the card is a valve terminal with a free position, is attached
    /// there (`assembly.attach`) and then bound where it was dropped, two commands and two undo steps that the toast
    /// names; **neither** is refused before the commit with `reason-bind-off-card` and a toast that says what to do
    /// (FX38d). A bind that unbinds another channel (FR-PLC-005) names it in the toast by its address and the pin
    /// (FT-757).
    pub(super) fn act_navigator_bind(&mut self, p: &Map<String, Value>) -> Outcome {
        let channel = match string_param(p, "channel") {
            Ok(Some(c)) => c.trim().trim_start_matches("id:").to_owned(),
            Ok(None) => return failed("error-missing-param", &["channel"]),
            Err(e) => return e,
        };
        let Ok(ch) = channel.parse::<ew_api::output::Uuid>() else {
            return failed("error-bad-param", &["channel"]);
        };
        let at = match super::canvas_actions::point_mm(p) {
            Ok(Some(at)) => at,
            Ok(None) => return failed("error-missing-param", &["x_mm"]),
            Err(e) => return e,
        };
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        if let Some(a) = &mut self.area
            && a.focused != g
        {
            a.focused = g;
        }
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let (page, ppn, pane) = (Arc::clone(c.page()), c.points_per_nm(), c.pane());
        let cx = crate::tools::ToolCx {
            page: &page,
            points_per_nm: ppn,
            pane,
            snapper: &page.snapper,
            selection: &self.tools.selection,
        };
        let crate::tools::wire::WireTarget::Cp { function, idx, .. } =
            crate::tools::wire::resolve_target(&cx, at, false)
        else {
            return Outcome::Refused {
                reason_key: "reason-drop-on-cp",
            };
        };
        let Some(pr) = self.project.as_ref() else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        // (where the channel binds, the device wired to that card pin, the attachment: device, tag, card, card tag,
        // position)
        type Attach = (ew_api::output::Uuid, String, ew_api::output::Uuid, String, u32);
        let (target, wired_to, attach): ((ew_api::output::Uuid, u16), Option<String>, Option<Attach>) =
            match pr.session.off_card_bind(ch, function.0, idx) {
                None => ((function.0, idx), None, None),
                Some(OffCardBind::Wired { device, card, pins }) => match pins.as_slice() {
                    [pin] => ((pin.function, pin.idx), Some(device), None),
                    _ => {
                        let list: Vec<String> = pins
                            .iter()
                            .map(|p| crate::describe::pin_in_sentence(&p.label))
                            .collect();
                        let text = strings::format(
                            "toast-bind-wired-pins",
                            &[&device, &card, &pins.len().to_string(), &list.join(", ")],
                        );
                        self.notify_toned(text, crate::notifications::NoteTone::Warning);
                        return Outcome::Refused {
                            reason_key: "reason-bind-wired-pins",
                        };
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
                // FX38d: refused here in words, by the rule the command applies (`is_on_module`), not left to the
                // command's error with record IDs; FT-753: the words say what to do.
                Some(OffCardBind::Neither {
                    device,
                    card,
                    valve_terminal,
                }) => {
                    let text = match (device.is_empty(), valve_terminal) {
                        (true, _) => strings::format("toast-bind-off-card-untagged", &[&card]),
                        (false, false) => strings::format("toast-bind-off-card", &[&device, &card]),
                        (false, true) => strings::format("toast-bind-no-position", &[&device, &card]),
                    };
                    self.notify_toned(text, crate::notifications::NoteTone::Warning);
                    return Outcome::Refused {
                        reason_key: "reason-bind-off-card",
                    };
                }
            };
        // FT-757: the channels the bind unbinds (FR-PLC-005), named in the toast by their addresses.
        let words = pr.session.channel_words(ch).unwrap_or_default();
        let pin = crate::describe::pin_in_sentence(&pr.session.cp_text(target.0, target.1));
        let displaced: Vec<String> = pr
            .session
            .channels_bound_at(target.0, target.1, ch)
            .into_iter()
            .filter_map(|o| pr.session.channel_words(o))
            .collect();
        if let Some((device, _, card, _, position)) = &attach {
            let cmd = ew_api::AnyCommand {
                name: "assembly.attach".into(),
                params: json!({
                    "parent": card.to_string(),
                    "children": [{"device": device.to_string(), "position": position}],
                }),
            };
            let out = self.run_commit(&cmd);
            if out != Outcome::Done {
                return out;
            }
        }
        let cmd = ew_api::AnyCommand {
            name: "plc.bind_channel".into(),
            params: json!({
                "channel": channel,
                "endpoint": {"cp": {"function": target.0.to_string(), "idx": target.1}},
            }),
        };
        let out = self.run_commit(&cmd);
        if out != Outcome::Done {
            return out;
        }
        let mut text = match (&attach, &wired_to) {
            (Some((_, tag, _, card_tag, position)), _) => strings::format(
                "toast-bind-attached",
                &[tag, card_tag, &position.to_string(), &words, &pin],
            ),
            (None, Some(device)) => strings::format("toast-bind-wired", &[&words, &pin, device]),
            (None, None) => strings::format("toast-bind-done", &[&words, &pin]),
        };
        for d in &displaced {
            text.push_str(&strings::format("toast-bind-displaced", &[d]));
        }
        // An attachment and a bind are two undo steps: that toast offers no Undo of only the second.
        self.notify(text, attach.is_none());
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
        };
        Some(f(host.tool.as_mut(), &cx))
    }

    /// Enter with Select idle and exactly one junction selected opens its editor (§8.5); `None` otherwise.
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
    pub(super) fn send_proposal_key(&mut self) -> Outcome {
        self.send_tool(&CanvasEvent::Key(crate::tools::ToolKey::NextProposal))
    }
}
