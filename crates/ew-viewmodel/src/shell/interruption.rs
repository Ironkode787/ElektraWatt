// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's duties of WP-S2.04 part 3 (§9): arming the IP tool and its keys and fields, the Wire tool's end in
//! an interruption point (P while routing), continue and arrange, the black box tool, and the grips of a selected
//! black box with their keyboard forms.
//!
//! **Reads.** The IP symbol and the external arrow are rows of the place catalogue (fetched once per (session,
//! revision, folio)) and their shapes come through the per-revision shape cache (§6.2); a starter symbol the project
//! lacks travels in the command's `copies`, as the picker's placements do (FT-656, FT-673: one undo step). The external
//! references are read from the session's query when the IP tool's
//! field is set. Pointer and cursor moves of the IP and black box tools read nothing (§12 item 4).
//!
//! **Commits** go through the tool's commit path (`execute_with` in the window, `run_script` headless).

use std::cmp::Ordering;
use std::sync::Arc;

use ew_api::output::ew_dl::EntityRef;
use ew_api::output::{PageTables, SymbolShape, TableRep};
use serde_json::{Map, Value, json};

use super::canvas_actions::point_mm;
use super::{Outcome, ShellModel, failed, flag, string_param};
use crate::canvas::CanvasPage;
use crate::strings;
use crate::tools::black_box::{
    nearest_on_outline, on_outline, outward, reshape_by_grip, reshape_command, selected_black_box,
    signed_area2,
};
use crate::tools::ip::{drawing_area, potential_at, potential_of_end, propose, quarter_direction};
use crate::tools::wire::PageGridMap;
use crate::tools::{
    CanvasEvent, IpDir, IpExternal, IpMemory, IpTool, PickerMode, ProjectIps, ToolKey, ToolOutput, WireTool,
    black_box, selection_kinds,
};

/// The natural order of two names: runs of decimal digits compare by their value, other runs by their text.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn runs(s: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let mut start = 0;
        let bytes = s.as_bytes();
        for i in 1..=bytes.len() {
            if i == bytes.len() || bytes[i].is_ascii_digit() != bytes[start].is_ascii_digit() {
                out.push((bytes[start].is_ascii_digit(), &s[start..i]));
                start = i;
            }
        }
        out
    }
    if a.is_empty() || b.is_empty() {
        return a.cmp(b);
    }
    for (x, y) in runs(a).into_iter().zip(runs(b)) {
        let o = match (x, y) {
            ((true, x), (true, y)) => {
                let (tx, ty) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                tx.len().cmp(&ty.len()).then_with(|| tx.cmp(ty))
            }
            ((_, x), (_, y)) => x.cmp(y),
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

impl IpDir {
    /// The direction of its word.
    fn of_word(w: &str) -> IpDir {
        match w {
            "target" => IpDir::Target,
            "undirected" => IpDir::Undirected,
            _ => IpDir::Source,
        }
    }
}

/// The sort key of an arranged IP: its potential name, its name, its function ID and its position (y, x).
type ArrangeKey = (Option<String>, String, String, (i64, i64));

/// The interruption points of a page that a selection holds: their table rows, in selection order.
fn selected_ips<'t>(model: &ShellModel, tables: &'t PageTables) -> Vec<&'t TableRep> {
    selection_kinds(model.selection(), tables)
        .into_iter()
        .filter_map(|(e, _)| tables.reps.iter().find(|r| r.id == e.0 && r.ip.is_some()))
        .collect()
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// How many interruption points of the focused page are selected (`ip.continue_on` wants one, `ip.arrange_selected`
    /// two or more).
    pub fn selected_ip_count(&self) -> usize {
        self.canvas()
            .map_or(0, |c| selected_ips(self, &c.page().tables).len())
    }

    /// Exactly one black box of the focused page is selected (`box.grip`, `box.add_cp`).
    pub fn one_black_box_selected(&self) -> bool {
        self.canvas().is_some_and(|c| {
            let t = &c.page().tables;
            matches!(self.selection().items.as_slice(), [one]
                if t.reps.iter().any(|r| r.id == one.0 && r.black_box.is_some()))
        })
    }

    /// The project's external references as the IP tool's field offers them: (ID text, printed text
    /// `<device>:<connection>`), in printed-text order.
    pub fn external_choices(&self) -> Vec<(String, String)> {
        let Some(p) = &self.project else {
            return Vec::new();
        };
        let q = p.session.query();
        let mut out: Vec<(String, String)> = q
            .external_refs()
            .iter()
            .map(|(id, x)| {
                let label = match &x.connection {
                    Some(c) => format!("{}:{}", x.device.as_str(), c.as_str()),
                    None => x.device.as_str().to_owned(),
                };
                (id.to_string(), label)
            })
            .collect();
        out.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        out
    }

    // -----------------------------------------------------------------------------------------------------------
    // Reads

    /// The shape of the catalogue symbol whose key is `base` in the page's grid family (`ew.gen.ip`,
    /// `ew.gen.ip@qet10`), and the library documents a command drawing it carries (a starter symbol the project lacks,
    /// with its FnDef; FT-673).
    fn family_shape(&mut self, base: &str) -> Result<(Arc<SymbolShape>, Vec<Value>), Outcome> {
        let none = Outcome::Refused {
            reason_key: "reason-no-ip-symbol",
        };
        let cat = self.catalogue().ok_or(Outcome::Refused {
            reason_key: "reason-no-catalogue",
        })?;
        let key = cat
            .symbols
            .iter()
            .find(|s| s.key.split('@').next() == Some(base))
            .map(|s| s.key.clone())
            .ok_or(none.clone())?;
        let (row, copies) = self.starter_row(&key)?;
        let shape = self.row_shape(&row).ok_or(none)?;
        Ok((shape, copies))
    }

    /// The drawing area of the focused page (page nm), or its sheet.
    fn focused_area(&self) -> Option<[i64; 4]> {
        let page = self.canvas()?.page();
        drawing_area(page).or_else(|| {
            let a = page.snapper.sheet_to_page((0, 0));
            let b = page.snapper.sheet_to_page(page.sheet);
            Some([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)])
        })
    }

    /// The project's source points (FT-822): every interruption point of direction `source` that a representation
    /// draws, with its page; external connection points are left out (they never pair).
    fn project_ips(&self) -> ProjectIps {
        let mut out = ProjectIps::default();
        let Some(p) = &self.project else {
            return out;
        };
        let q = p.session.query();
        let idx = q.idx();
        for (id, f) in q.functions().iter() {
            // The function kind is read through its JSON form (`{"ip": {"name", "direction", …}}`): the shell names no
            // schema type of its own.
            let Ok(kind) = serde_json::to_value(&f.kind) else {
                continue;
            };
            let Some(ip) = kind.get("ip") else {
                continue;
            };
            if ip.get("direction").and_then(Value::as_str) != Some("source") || ip.get("external").is_some() {
                continue;
            }
            let Some(name) = ip.get("name").and_then(Value::as_str) else {
                continue;
            };
            for r in idx.reps_of_function(id) {
                if let Some(rep) = q.reps().get(r) {
                    out.add_source(name, rep.page.as_uuid());
                }
            }
        }
        out
    }

    /// Gives the armed IP tool the project's source points again when the model changed since it read them (FT-822).
    pub(super) fn sync_ip_project(&mut self) {
        let rev = self.revision();
        let stale = self.ip_tool().is_some_and(|t| t.project_rev() != rev);
        if stale {
            let ips = self.project_ips();
            if let Some(t) = self.ip_tool() {
                t.set_project(ips, rev);
            }
        }
    }

    /// The IP tool, when it is the active tool.
    fn ip_tool(&mut self) -> Option<&mut IpTool> {
        self.tools
            .tool
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<IpTool>())
    }

    // -----------------------------------------------------------------------------------------------------------
    // The IP tool (§9.1) and P while routing (§9.2)

    /// `tool.ip` (P): while the Wire tool routes, the wire ends in an interruption point (§9.2); otherwise the IP tool
    /// is armed with the IP symbol of the page's grid family.
    pub(super) fn act_tool_ip(&mut self) -> Outcome {
        if self.tools.tool.name() == "wire" && self.tools.tool.state() == "routing" {
            return self.wire_end_in_ip();
        }
        let (shape, copies) = match self.family_shape("ew.gen.ip") {
            Ok(s) => s,
            Err(e) => return e,
        };
        let area = self.focused_area();
        let mut tool = IpTool::new(shape, self.tools.ip_memory.clone(), area).with_copies(copies);
        tool.set_project(self.project_ips(), self.revision());
        self.activate_tool(Box::new(tool));
        Outcome::Done
    }

    /// P while the Wire tool routes: `connection.draw_to_ip` at the routed end (one transaction, one undo step).
    fn wire_end_in_ip(&mut self) -> Outcome {
        let (shape, copies) = match self.family_shape("ew.gen.ip") {
            Ok(s) => s,
            Err(e) => return e,
        };
        let memory = self.tools.ip_memory.clone();
        let area = self.focused_area();
        let project = self.project_ips();
        let result = self
            .with_tool(|t, cx| {
                let w = t.as_any_mut()?.downcast_mut::<WireTool>()?;
                Some(w.end_in_ip(cx, &shape, |start, pos| {
                    let tables = &cx.page.tables;
                    let direction = area
                        .and_then(|a| quarter_direction(pos, a))
                        .unwrap_or(memory.last_direction);
                    let base = potential_of_end(tables, start)
                        .or_else(|| memory.last_name.clone())
                        .unwrap_or_else(|| "IP".to_owned());
                    // FT-822: the project's sources decide the role and the series, as the IP tool's do.
                    let (name, direction) = propose(&base, false, direction, false, false, &project, tables);
                    (name, direction.word())
                }))
            })
            .flatten();
        match result {
            Some(Ok((out, name, direction))) => {
                // FT-673: the IP symbol the project lacks travels in the wire's command (one undo step).
                let out = match out {
                    ToolOutput::Commit(cmd) if !copies.is_empty() => match cmd.to_json() {
                        Ok(mut params) => {
                            params["copies"] = Value::Array(copies);
                            ToolOutput::Commit(Box::new(ew_api::AnyCommand {
                                name: cmd.name().to_owned(),
                                params,
                            }))
                        }
                        Err(_) => ToolOutput::Commit(cmd),
                    },
                    other => other,
                };
                let outcome = self.apply_tool_output(out);
                if outcome == Outcome::Done {
                    self.tools.ip_memory = IpMemory {
                        last_name: Some(name),
                        last_direction: IpDir::of_word(direction),
                    };
                }
                outcome
            }
            Some(Err(reason_key)) => Outcome::Refused { reason_key },
            None => Outcome::Refused {
                reason_key: "reason-ip-end-grid",
            },
        }
    }

    /// `tool.ip_direction` (D) and `tool.star_source`: the IP tool's direction and star toggles; the star of an
    /// external connection point is refused (it never pairs).
    pub(super) fn act_ip_key(&mut self, key: ToolKey) -> Outcome {
        let external = match self.ip_tool() {
            Some(t) => t.external().is_some(),
            None => {
                return Outcome::Refused {
                    reason_key: "reason-ip-tool-only",
                };
            }
        };
        if key == ToolKey::ToggleStar && external {
            self.tools.notice = Some(strings::text("reason-external-never-pairs").to_owned());
            self.tools.notice_sticky = false;
            return Outcome::Refused {
                reason_key: "reason-external-never-pairs",
            };
        }
        self.send_tool(&CanvasEvent::Key(key))
    }

    /// `tool.ip_direction {direction?}`: D cycles the direction; the headless `direction` (`source`, `target`,
    /// `undirected`) sets it, so a script needs no knowledge of the default. While the Graphic tool draws an arc, D
    /// toggles the arc's sweep direction (WP-S2.04 §10.5) and a `direction` is refused.
    pub(super) fn act_ip_direction(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.tools.tool.name() == "graphic" {
            if p.contains_key("direction") {
                return failed("error-bad-param", &["direction"]);
            }
            return self.send_tool(&CanvasEvent::Key(ToolKey::ToggleDirection));
        }
        let direction = match string_param(p, "direction") {
            Ok(None) => return self.act_ip_key(ToolKey::ToggleDirection),
            Ok(Some("source")) => IpDir::Source,
            Ok(Some("target")) => IpDir::Target,
            Ok(Some("undirected")) => IpDir::Undirected,
            Ok(Some(_)) => return failed("error-bad-param", &["direction"]),
            Err(e) => return e,
        };
        if self.ip_tool().is_none() {
            return Outcome::Refused {
                reason_key: "reason-ip-tool-only",
            };
        }
        let overlay = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<IpTool>())
                    .map(|ip| ip.choose_direction(direction, cx))
            })
            .flatten();
        if let Some(o) = overlay {
            self.tools.overlay = o;
        }
        Outcome::Done
    }

    /// `ip.name {text}`: the IP tool's name field.
    pub(super) fn act_ip_name(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.ip_tool().is_none() {
            return Outcome::Refused {
                reason_key: "reason-ip-tool-only",
            };
        }
        match string_param(p, "text") {
            Ok(Some(t)) if !t.trim().is_empty() => self.send_tool(&CanvasEvent::Text(t.to_owned())),
            Ok(_) => failed("error-missing-param", &["text"]),
            Err(e) => e,
        }
    }

    /// Resolves an external reference by its ID (or alias, `id:<uuid>`), its label (`JE-N1000-03:L`, the device and
    /// the connection) or its label after the location the page prints (`+J-E-206 / JE-N1000-03:L`, the cross-reference
    /// text of the external point followed by the connection; FT-485 item 8): by ID first; by label only when exactly
    /// one reference (at that location, when one is named) has it (fix round, item 7). `Err` carries the outcome: a
    /// text no reference has fails with a message that lists the labels the project has (FT-485 item 8); a label two
    /// references share is refused with `reason-external-ambiguous`.
    pub(super) fn resolve_external(&self, s: &str) -> Result<IpExternal, Outcome> {
        let s = s.strip_prefix("id:").unwrap_or(s).trim();
        let choices = self.external_choices();
        if let Some((id, label)) = choices.iter().find(|(id, _)| id.eq_ignore_ascii_case(s)) {
            return Ok(IpExternal {
                id: id.clone(),
                label: label.clone(),
            });
        }
        // `<location> / <label>`: the location narrows the references to those at its node.
        let (node, label) = match s.rsplit_once(" / ") {
            Some((location, label)) => (
                Some(self.structure_node("location", location.trim()).ok()),
                label.trim(),
            ),
            None => (None, s),
        };
        let locations = self.external_locations();
        let by_text: Vec<&(String, String)> = choices
            .iter()
            .filter(|(id, l)| {
                l == label
                    && match &node {
                        None => true,
                        Some(None) => false,
                        Some(Some(n)) => locations.get(id).is_some_and(|at| at.eq_ignore_ascii_case(n)),
                    }
            })
            .collect();
        match by_text.as_slice() {
            [(id, label)] => Ok(IpExternal {
                id: id.clone(),
                label: label.clone(),
            }),
            [] => {
                let mut labels: Vec<&str> = choices.iter().map(|(_, l)| l.as_str()).collect();
                labels.dedup();
                let known = if labels.is_empty() {
                    strings::text("external-none").to_owned()
                } else {
                    labels.join(", ")
                };
                Err(failed("error-no-external", &[s, &known]))
            }
            _ => Err(Outcome::Refused {
                reason_key: "reason-external-ambiguous",
            }),
        }
    }

    /// The location node (its ID text) of each external reference, by the reference's ID text.
    fn external_locations(&self) -> std::collections::BTreeMap<String, String> {
        let Some(p) = &self.project else {
            return std::collections::BTreeMap::new();
        };
        let q = p.session.query();
        q.external_refs()
            .iter()
            .map(|(id, x)| (id.to_string(), x.location.as_uuid().to_string()))
            .collect()
    }

    /// `ip.external {external}`: the IP tool's external-reference field: an external reference by its ID (or alias,
    /// `id:<uuid>`) or its printed text (`JE-N1000-03:L`); an empty text clears it (the parameter is `external:text`;
    /// `null` is accepted too). While set, the ghost and the commit use the external arrow of the page's grid family.
    pub(super) fn act_ip_external(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.ip_tool().is_none() {
            return Outcome::Refused {
                reason_key: "reason-ip-tool-only",
            };
        }
        let external = match p.get("external") {
            None => return failed("error-missing-param", &["external"]),
            Some(Value::Null) => None,
            Some(Value::String(s)) if s.is_empty() => None,
            Some(Value::String(s)) => match self.resolve_external(s) {
                Ok(e) => Some(e),
                Err(outcome) => return outcome,
            },
            Some(_) => return failed("error-bad-param", &["external"]),
        };
        let base = if external.is_some() {
            "ew.gen.external-arrow"
        } else {
            "ew.gen.ip"
        };
        let (shape, copies) = match self.family_shape(base) {
            Ok(s) => s,
            Err(e) => return e,
        };
        let overlay = self
            .with_tool(|t, cx| {
                t.as_any_mut().and_then(|a| a.downcast_mut::<IpTool>()).map(|ip| {
                    ip.set_copies(copies);
                    ip.set_external(external, shape, cx)
                })
            })
            .flatten();
        if let Some(o) = overlay {
            self.tools.overlay = o;
        }
        self.announce_tool();
        Outcome::Done
    }

    /// After a commit of the IP tool: the host remembers its name and direction, and the ghost proposes the next name
    /// on the new revision.
    pub(super) fn after_ip_commit(&mut self) {
        if self.tools.tool.name() != "ip" {
            return;
        }
        let refreshed = self
            .with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<IpTool>())
                    .map(|ip| (ip.memory().clone(), ip.refresh(cx)))
            })
            .flatten();
        if let Some((memory, overlay)) = refreshed {
            self.tools.ip_memory = memory;
            if !overlay.items.is_empty() {
                self.tools.overlay = overlay;
            }
        }
    }

    /// FT-822: the IP tool's commit that the model refused (a prevent rule: EW-CORE-052 for a second source of a name)
    /// is a refusal of the tool's click, not a failure: the tool stays armed with its ghost where it was (the second
    /// member: keep the overlay), the reason stands in words in the HUD until the next canvas input and in a warning
    /// toast, and the window shows no banner (a banner is for a state of the whole project, doc 16a §6.11). Every
    /// other outcome passes unchanged.
    pub(super) fn ip_commit_refused(&mut self, out: Outcome) -> (Outcome, bool) {
        let Outcome::Failed { message } = &out else {
            return (out, false);
        };
        if self.tools.tool.name() != "ip" {
            return (out, false);
        }
        // C40 (H40 item 6): a second source says what is in the way and the way out, not the rule's ID tie-break.
        let text = self
            .second_source_words(message)
            .unwrap_or_else(|| strings::format("notice-ip-not-placed", &[message]));
        self.tools.notice = Some(text.clone());
        self.tools.notice_sticky = true;
        self.notify_toned(text, crate::notifications::NoteTone::Warning);
        (
            Outcome::Refused {
                reason_key: "reason-ip-not-placed",
            },
            true,
        )
    }

    /// The words of an EW-CORE-052 refusal of the IP tool's point (H40 item 6): "Not placed: 0V already has a source
    /// on page 101. Press D for a target, or mark one source as the star.", with the key of `tool.ip_direction` as the
    /// window names it ([`second_source_text`]) and the first page in page order that
    /// holds a source of the ghost's name, by its folio as the Pages navigator shows it; `None` for another refusal.
    fn second_source_words(&self, message: &str) -> Option<String> {
        if !message.contains("EW-CORE-052:") {
            return None;
        }
        let Some(crate::tools::Ghost::Ip { name, .. }) = self.tools.tool.ghost() else {
            return None;
        };
        let pages = self.project_ips().source_pages(&name);
        let folio = self
            .page_rows()
            .iter()
            .find(|r| pages.contains(&r.id))
            .map(|r| r.folio.clone())?;
        let key = crate::command_bar::key_of(self, "tool.ip_direction");
        Some(second_source_text(&name, &folio, &key))
    }

    /// Whether the kept preview failed with the refusal the HUD already carries (H40 item 6): the canvas shows no
    /// "Preview of … failed" chip for it, the HUD line and the toast say it once.
    pub fn preview_refusal_in_hud(&self) -> bool {
        self.tools.notice_sticky
            && self.tools.notice.is_some()
            && self.tools.preview.as_ref().is_some_and(|p| p.result.is_err())
    }

    // -----------------------------------------------------------------------------------------------------------
    // Continue and arrange (§9.3)

    /// `ip.continue_on {folio, x_mm?, y_mm?}` (Edit ▸ Continue on page…): `ip.continue` of exactly one selected
    /// interruption point onto the page `folio`. Without a point: the same y (on the target's grid), and x the leftmost
    /// grid point inside the target's drawing area when the original lies in the right half of its page's drawing
    /// area, else the rightmost. With `x_mm`, `y_mm` (sheet mm of the target page): that point, snapped to its grid.
    pub(super) fn act_ip_continue_on(&mut self, p: &Map<String, Value>) -> Outcome {
        let folio = match string_param(p, "folio") {
            Ok(Some(f)) => f.to_owned(),
            // Without a folio (the menu entry in the window): the next drawn page in page order.
            Ok(None) => {
                let rows = self.page_rows();
                match rows
                    .iter()
                    .skip(self.page_index() + 1)
                    .find(|r| !r.generated)
                    .map(|r| r.folio.clone())
                {
                    Some(f) => f,
                    None => {
                        return Outcome::Refused {
                            reason_key: "reason-no-next-page",
                        };
                    }
                }
            }
            Err(e) => return e,
        };
        let point = match point_mm(p) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let tables = Arc::clone(&c.page().tables);
        let [ip] = selected_ips(self, &tables)[..] else {
            return Outcome::Refused {
                reason_key: "reason-select-one-ip",
            };
        };
        let (function, original) = (ip.functions.first().copied(), ip.pos);
        let Some(function) = function else {
            return Outcome::Refused {
                reason_key: "reason-select-one-ip",
            };
        };
        let here = self.focused_area();
        let target: Arc<CanvasPage> = match self.page_for(&folio) {
            Ok(page) => page,
            Err(_) => return failed("error-bad-param", &["folio"]),
        };
        if target.tables.page.is_nil() {
            return Outcome::Refused {
                reason_key: "reason-not-editable",
            };
        }
        let grid = PageGridMap::new(target.tables.grid);
        let pos = match point {
            Some(at) => target.snapper.snap_grid(at).page_nm,
            None => {
                let area = drawing_area(&target).unwrap_or_else(|| {
                    let a = target.snapper.sheet_to_page((0, 0));
                    let b = target.snapper.sheet_to_page(target.sheet);
                    [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]
                });
                let right = here.is_some_and(|h| {
                    2 * (i128::from(original.0) - i128::from(h[0])) > i128::from(h[2]) - i128::from(h[0])
                });
                let x = if right {
                    let k = grid.index(area[0]);
                    if grid.coord(k) < area[0] {
                        grid.coord(k + 1)
                    } else {
                        grid.coord(k)
                    }
                } else {
                    let k = grid.index(area[2]);
                    if grid.coord(k) > area[2] {
                        grid.coord(k - 1)
                    } else {
                        grid.coord(k)
                    }
                };
                (x, grid.coord(grid.index(original.1)))
            }
        };
        let cmd = ew_api::AnyCommand {
            name: "ip.continue".into(),
            params: json!({
                "ip": function.to_string(),
                "page": target.tables.page.to_string(),
                "pos": [pos.0, pos.1],
            }),
        };
        self.commit_from_host(&cmd)
    }

    /// `ip.arrange_selected {by?, chain_order?}` (Edit ▸ Arrange interruption points): the selected interruption points
    /// of the page (at least two) sorted by the natural order of their names (`by: name`, the default) or of their
    /// derived potential names, then their names (`by: potential`; one without a potential last), ties by position (y, x), then ID,
    /// committed as `ip.arrange {ips, set_sort: chain_order}` (D12).
    pub(super) fn act_ip_arrange(&mut self, p: &Map<String, Value>) -> Outcome {
        let by_potential = match string_param(p, "by") {
            Ok(None | Some("name")) => false,
            Ok(Some("potential")) => true,
            Ok(Some(_)) => return failed("error-bad-param", &["by"]),
            Err(e) => return e,
        };
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let tables = Arc::clone(&c.page().tables);
        let ips = selected_ips(self, &tables);
        if ips.len() < 2 {
            return Outcome::Refused {
                reason_key: "reason-select-ips",
            };
        }
        let mut keyed: Vec<ArrangeKey> = ips
            .iter()
            .filter_map(|r| {
                let f = r.functions.first()?;
                let name = r.ip.as_ref()?.name.clone();
                let potential = if by_potential {
                    tables
                        .cps
                        .iter()
                        .filter(|c| c.rep == r.id)
                        .find_map(|c| c.potential.clone())
                        .or_else(|| {
                            tables
                                .cps
                                .iter()
                                .find(|c| c.rep == r.id)
                                .and_then(|c| potential_at(&tables, c.at))
                        })
                } else {
                    None
                };
                Some((potential, name, f.to_string(), (r.pos.1, r.pos.0)))
            })
            .collect();
        keyed.sort_by(|a, b| {
            let pot = match (&a.0, &b.0) {
                (Some(x), Some(y)) => natural_cmp(x, y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            pot.then_with(|| natural_cmp(&a.1, &b.1))
                // Ties by the current position (y, x), then by ID (FT-439: no UUID-keyed order first).
                .then_with(|| a.3.cmp(&b.3))
                .then_with(|| a.2.cmp(&b.2))
        });
        let mut params = json!({ "ips": keyed.iter().map(|k| k.2.clone()).collect::<Vec<_>>() });
        if flag(p, "chain_order") {
            params["set_sort"] = Value::Bool(true);
        }
        let cmd = ew_api::AnyCommand {
            name: "ip.arrange".into(),
            params,
        };
        self.commit_from_host(&cmd)
    }

    // -----------------------------------------------------------------------------------------------------------
    // The black box tool (§9.4, §9.5)

    /// `tool.black_box` (B): without parameters the picker on the black-box rows; with `place.pick`'s parameters the
    /// black box tool is armed at once (a row that is no black box is refused).
    pub(super) fn act_tool_black_box(&mut self, p: &Map<String, Value>) -> Outcome {
        if !p.contains_key("symbol") && !p.contains_key("part") {
            return self.act_open_picker(PickerMode::BlackBox);
        }
        self.tools.black_box_pick = true;
        let out = self.act_place_pick(p);
        self.tools.black_box_pick = false;
        out
    }

    /// The one selected black box and the grid cursor's page point.
    fn box_and_cursor(&self) -> Result<(TableRep, (i64, i64)), Outcome> {
        let c = self.canvas().ok_or(Outcome::Refused {
            reason_key: "reason-no-page",
        })?;
        let t = &c.page().tables;
        let rep = match self.selection().items.as_slice() {
            [one] => t.reps.iter().find(|r| r.id == one.0 && r.black_box.is_some()),
            _ => None,
        }
        .cloned()
        .ok_or(Outcome::Refused {
            reason_key: "reason-one-black-box",
        })?;
        let cursor = c.cursor().ok_or(Outcome::Refused {
            reason_key: "reason-no-cursor",
        })?;
        Ok((rep, c.page().snapper.snap_grid(cursor).page_nm))
    }

    /// `box.grip {grip}`: the keyboard form of a grip drag: grip `grip` of the one selected black box goes to the grid
    /// cursor, committed as `box.reshape` without `cps` (the CPs re-projected).
    pub(super) fn act_box_grip(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(grip) = p
            .get("grip")
            .and_then(Value::as_u64)
            .and_then(|g| usize::try_from(g).ok())
        else {
            return failed("error-missing-param", &["grip"]);
        };
        let (rep, to) = match self.box_and_cursor() {
            Ok(v) => v,
            Err(e) => return e,
        };
        let Some(b) = &rep.black_box else {
            return Outcome::Refused {
                reason_key: "reason-one-black-box",
            };
        };
        match reshape_by_grip(&b.outline, grip, to) {
            Ok(outline) => {
                if outline == b.outline {
                    return Outcome::Done;
                }
                let cmd = reshape_command(&rep, &outline, None);
                self.commit_from_host(cmd.as_ref())
            }
            Err(reason_key) => Outcome::Refused { reason_key },
        }
    }

    /// `box.add_cp` (Edit ▸ Add connection point): a CP of the one selected black box at the grid cursor, which must lie
    /// on its outline, with the smallest function CP index the box does not use, leaving along the edge's outward
    /// normal; committed as `box.reshape` with `cps`.
    pub(super) fn act_box_add_cp(&mut self) -> Outcome {
        let (rep, at) = match self.box_and_cursor() {
            Ok(v) => v,
            Err(e) => return e,
        };
        let Some(b) = &rep.black_box else {
            return Outcome::Refused {
                reason_key: "reason-one-black-box",
            };
        };
        if !on_outline(&b.outline, at) {
            return Outcome::Refused {
                reason_key: "reason-cp-off-outline",
            };
        }
        let Some(idx) = (0..=u16::MAX).find(|i| !b.cps.iter().any(|(j, ..)| j == i)) else {
            return Outcome::Refused {
                reason_key: "reason-cp-off-outline",
            };
        };
        let n = b.outline.len();
        let area = signed_area2(&b.outline);
        let dir = (0..n)
            .find_map(|i| {
                let (a, c) = (b.outline[i], b.outline[(i + 1) % n]);
                on_outline(&[a, c], at).then(|| outward(a, c, area)).flatten()
            })
            .unwrap_or(ew_api::geometry::Dir::Up);
        let mut cps = b.cps.clone();
        cps.push((idx, at, dir));
        let cmd = reshape_command(&rep, &b.outline, Some(&cps));
        self.commit_from_host(cmd.as_ref())
    }

    /// The grips of a selected black box (§9.4): while Select is idle and exactly one black box is selected, the overlay
    /// shows its grips and CPs; once that no longer holds, a grips overlay the host set is cleared (the end of every
    /// dispatch).
    pub(super) fn sync_box_grips(&mut self) {
        let idle = self.tools.tool.name() == "select" && self.tools.tool.state() == "idle";
        let want = if idle {
            self.with_tool(|_, cx| selected_black_box(cx).map(|r| black_box::grips_overlay(cx, r)))
                .flatten()
        } else {
            None
        };
        match want {
            Some(o) => {
                if self.tools.overlay.items.is_empty()
                    || self.tools.grips.as_ref() == Some(&self.tools.overlay)
                {
                    self.tools.overlay = o.clone();
                    self.tools.grips = Some(o);
                }
            }
            None => {
                if let Some(g) = self.tools.grips.take()
                    && self.tools.overlay == g
                {
                    self.tools.overlay = crate::tools::Overlay::default();
                }
            }
        }
    }

    /// The nearest grid point on the outline of the one selected black box to the grid cursor (tests and the window's
    /// keyboard CP move).
    pub fn box_outline_point(&self) -> Option<(i64, i64)> {
        let (rep, at) = self.box_and_cursor().ok()?;
        let b = rep.black_box.as_ref()?;
        let grid = PageGridMap::new(self.canvas()?.page().tables.grid);
        nearest_on_outline(grid, &b.outline, at).map(|(p, _)| p)
    }

    /// The selection's black box entity (tests).
    pub fn selected_black_box_id(&self) -> Option<EntityRef> {
        let c = self.canvas()?;
        match self.selection().items.as_slice() {
            [one]
                if c.page()
                    .tables
                    .reps
                    .iter()
                    .any(|r| r.id == one.0 && r.black_box.is_some()) =>
            {
                Some(*one)
            }
            _ => None,
        }
    }
}

/// The EW-CORE-052 notice of the IP tool for the point `name` whose existing source is on page `folio`: the way out
/// names `key`, the first binding of `tool.ip_direction` (the user may rebind it), or the action by its title when it
/// has none (cycle-40 review).
fn second_source_text(name: &str, folio: &str, key: &str) -> String {
    if key.is_empty() {
        let title = crate::actions::find("tool.ip_direction")
            .map_or("tool.ip_direction", |m| strings::text(m.label_key));
        strings::format("notice-ip-second-source-unbound", &[name, folio, title])
    } else {
        strings::format("notice-ip-second-source", &[name, folio, key])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cycle-40 review: the EW-CORE-052 notice names the key the user has for `tool.ip_direction`, or the action by its
    /// title when it has none.
    #[test]
    fn the_second_source_notice_names_the_key_or_the_title() {
        assert_eq!(
            second_source_text("0V", "101", "D"),
            "Not placed: 0V already has a source on page 101. Press D for a target, or mark one source as the star."
        );
        assert_eq!(
            second_source_text("0V", "101", ""),
            "Not placed: 0V already has a source on page 101. Use Interruption point direction for a target, or mark \
             one source as the star."
        );
    }

    #[test]
    fn the_natural_order_reads_numbers_by_value() {
        let mut v = vec!["L-10", "N-1", "L-2", "L-1", "PE"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["L-1", "L-2", "L-10", "N-1", "PE"]);
        assert_eq!(natural_cmp("24VDC-01", "24VDC-1"), Ordering::Greater);
        assert_eq!(natural_cmp("", "A"), Ordering::Less);
    }
}
