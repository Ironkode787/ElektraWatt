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

use ew_api::output::ew_dl::EntityRef;
use serde_json::{Map, Value, json};

use super::canvas_actions::point_mm;
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

    /// `junction.open` (§8.5): the editor of exactly one selected junction, or (headless, `x_mm` and `y_mm`) of the
    /// junction within 8 points of that sheet point, the selection unchanged.
    pub(super) fn act_junction_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let tables = Arc::clone(&c.page().tables);
        let junction = match point_mm(p) {
            Err(e) => return e,
            Ok(Some(at)) => {
                let page = c.page();
                let reach = c.points_per_nm();
                let r = if reach > 0.0 {
                    (crate::tools::CP_SNAP_PT / reach).floor().min(9.0e15) as i128
                } else {
                    0
                };
                tables
                    .junctions
                    .iter()
                    .map(|j| (crate::tools::dist2(page.snapper.page_to_sheet(j.at), at), j.id))
                    .filter(|(d, _)| *d <= r * r)
                    .min()
                    .map(|(_, id)| EntityRef(id))
            }
            Ok(None) => {
                let js: Vec<EntityRef> = selection_kinds(&self.tools.selection, &tables)
                    .into_iter()
                    .filter(|(_, k)| *k == EntityKind::Junction)
                    .map(|(e, _)| e)
                    .collect();
                match js.as_slice() {
                    [one] => Some(*one),
                    _ => None,
                }
            }
        };
        let Some(j) = junction else {
            return Outcome::Refused {
                reason_key: "reason-one-junction",
            };
        };
        match self.read_junction_editor(j, 0) {
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
            [(_, EntityKind::Junction)] => Some(self.act_junction_open(&Map::new())),
            _ => None,
        }
    }

    /// Sends a tool key that only some tools take (Tab): the active tool answers.
    pub(super) fn send_proposal_key(&mut self) -> Outcome {
        self.send_tool(&CanvasEvent::Key(crate::tools::ToolKey::NextProposal))
    }
}
