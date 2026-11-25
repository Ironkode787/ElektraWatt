// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pins of a drawn box (a box drawn without a part could not get pins). A box whose pins are its own
//! (`TableBox::own_pins`) takes them where it stands: Add pin puts one on the edge nearest the point, its name asked in
//! place ([`PinNameTool`]); a double click on a pin renames it; its right-click removes it while no wire holds it; a wire
//! drawn to the box's edge where no pin is asks for the new pin's name and ends there. A box of a part draws the
//! part's pins: Add pin opens its device box. Each is one undo step.

use ew_api::output::TableRep;
use serde_json::{Map, Value, json};

use super::canvas_actions::point_mm;
use super::{Outcome, ShellModel};
use crate::actions::ActionItem;
use crate::key;
use crate::strings;
use crate::tools::black_box::{BoxCpAt, pin_target, reshape_command};
use crate::tools::box_pin::PinNameTool;
use crate::tools::wire::PageGridMap;
use crate::tools::{Button, CanvasEvent, Mods, tool_named};

/// How near a pin a point is to stand on it (nm on the page): half a grid step of the coarsest grid.
fn on_pin(grid: PageGridMap, pin: (i64, i64), p: (i64, i64)) -> bool {
    let half = grid.step() / 2;
    (pin.0 - p.0).abs() <= half && (pin.1 - p.1).abs() <= half
}

impl ShellModel {
    /// The page point of `x_mm`/`y_mm` (a sheet point, the menu's) and `true`; without them the pointer's, else the
    /// grid cursor's, else the box's middle, and `false` (the pin then follows the pointer until a click puts it).
    fn pin_point(&self, p: &Map<String, Value>, rep: &TableRep) -> Result<((i64, i64), bool), Outcome> {
        let c = self.canvas().ok_or(Outcome::refused(key!("reason-no-page")))?;
        let page = c.page();
        Ok(match point_mm(p)? {
            Some(sheet) => (page.snapper.sheet_to_page(sheet), true),
            None => {
                let at = c
                    .pointer()
                    .map(|q| page.snapper.sheet_to_page(q))
                    .or_else(|| c.cursor().map(|q| page.snapper.snap_grid(q).page_nm))
                    .unwrap_or(((rep.body[0] + rep.body[2]) / 2, rep.body[1]));
                (at, false)
            }
        })
    }

    /// `box.add_cp {x_mm?, y_mm?}` (Add pin, the box's right-click, and the Draw tab's Arrange group): on a box whose
    /// pins are its own, a new pin on the free grid point of its outline nearest the point, its name then asked in
    /// place; without a point the pin follows the pointer along the edge until a click puts it; on a box of a part, its device box opens, where the part's pins are edited.
    pub(crate) fn act_box_add_cp(&mut self, p: &Map<String, Value>) -> Outcome {
        let (rep, b) = match self.selected_box() {
            Ok(one) => one,
            Err(reason) => return Outcome::refused(reason),
        };
        if !b.own_pins {
            let out = self.act_part_box(&Map::new());
            if out == Outcome::Done {
                self.notify(strings::text(key!("toast-box-pin-part")).to_owned(), false);
            }
            return out;
        }
        let (at, given) = match self.pin_point(p, &rep) {
            Ok(at) => at,
            Err(out) => return out,
        };
        let Some(grid) = self.canvas().map(|c| PageGridMap::new(c.page().tables.grid)) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let Some(idx) = (0..=u16::MAX).find(|i| !b.cps.iter().any(|c| c.0 == *i)) else {
            return Outcome::refused(key!("reason-box-pin-no-room"));
        };
        let Some((q, dir)) = pin_target(grid, &b.outline, &b.cps, idx, at) else {
            return Outcome::refused(key!("reason-box-pin-no-room"));
        };
        let (rep, pin) = (ew_dl::EntityRef(rep.id), (idx, q, dir));
        // The menu's point puts the pin at once; the bar's Add pin lets it follow the pointer first.
        let tool = if given {
            PinNameTool::add(rep, pin)
        } else {
            PinNameTool::arm(rep, pin)
        };
        self.activate_tool(Box::new(tool));
        self.announce_tool();
        Outcome::Done
    }

    /// `box.remove_pin {idx}` (a pin's right-click, Remove pin): the pin of the one selected box whose pins are its
    /// own, while no wire holds it, as one `box.reshape` without it.
    pub(crate) fn act_box_remove_pin(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(idx) = p
            .get("idx")
            .and_then(Value::as_u64)
            .and_then(|i| u16::try_from(i).ok())
        else {
            return super::failed(key!("error-missing-param"), &["idx"]);
        };
        let (rep, b) = match self.selected_box() {
            Ok(one) => one,
            Err(reason) => return Outcome::refused(reason),
        };
        if !b.own_pins {
            return Outcome::refused(key!("reason-box-pin-of-part"));
        }
        if self.pin_wired(&rep, idx) {
            return Outcome::refused(key!("reason-box-pin-wired"));
        }
        let cps: Vec<BoxCpAt> = b.cps.iter().copied().filter(|c| c.0 != idx).collect();
        let cmd = reshape_command(&rep, &b.outline, Some(&cps));
        self.commit_from_host(cmd.as_ref())
    }

    /// Whether a connection on the page ends at pin `idx` of the box `rep`.
    fn pin_wired(&self, rep: &TableRep, idx: u16) -> bool {
        let Some(c) = self.canvas() else { return false };
        c.page()
            .tables
            .cps
            .iter()
            .any(|cp| cp.rep == rep.id && cp.idx == idx && !cp.connections.is_empty())
    }

    /// The pin of the selected box whose pins are its own at the pointer (a double click's): its index, point,
    /// direction and name.
    fn pin_under_pointer(&self) -> Option<(TableRep, BoxCpAt, String)> {
        let (rep, b) = self.selected_box().ok().filter(|(_, b)| b.own_pins)?;
        let c = self.canvas()?;
        let p = c.page().snapper.sheet_to_page(c.pointer()?);
        let grid = PageGridMap::new(c.page().tables.grid);
        let pin = *b.cps.iter().find(|cp| on_pin(grid, cp.1, p))?;
        let name = b
            .names
            .iter()
            .find(|(i, _)| *i == pin.0)
            .map(|(_, n)| n.clone())
            .unwrap_or_default();
        Some((rep, pin, name))
    }

    /// A double click on a pin of a selected box whose pins are its own: its name asked in place (rename); `false`
    /// when the double click is on no such pin.
    pub(super) fn rename_pin_at_pointer(&mut self) -> bool {
        let Some((rep, pin, name)) = self.pin_under_pointer() else {
            return false;
        };
        self.activate_tool(Box::new(PinNameTool::rename(ew_dl::EntityRef(rep.id), pin, name)));
        self.announce_tool();
        true
    }

    /// The box entries of the canvas menu for the black box `rep`, opened at the sheet point `at`: on a pin of a box
    /// whose pins are its own Remove pin (greyed while a wire holds it), else Add pin aimed at the point; Add pin on a
    /// box of a part says it opens the device box.
    pub(super) fn box_pin_entries(&self, rep: &TableRep, at: Option<(i64, i64)>, out: &mut Vec<ActionItem>) {
        let Some(b) = &rep.black_box else { return };
        // A valve terminal's box has the pins of its positions.
        if b.valve_terminal {
            return;
        }
        let mm = |v: i64| v as f64 / 1e6;
        let point = at.map_or_else(|| json!({}), |a| json!({"x_mm": mm(a.0), "y_mm": mm(a.1)}));
        if !b.own_pins {
            let label = strings::text(key!("context-box-pin-part")).to_owned();
            out.extend(self.aimed("box.add_cp", point, label));
            return;
        }
        let pin = self.canvas().zip(at).and_then(|(c, at)| {
            let p = c.page().snapper.sheet_to_page(at);
            let grid = PageGridMap::new(c.page().tables.grid);
            b.cps.iter().find(|cp| on_pin(grid, cp.1, p)).copied()
        });
        match pin {
            Some((idx, ..)) => {
                let name = b
                    .names
                    .iter()
                    .find(|(i, _)| *i == idx)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_default();
                let label = strings::format(key!("context-box-pin-remove"), &[&name]);
                let item = self
                    .aimed("box.remove_pin", json!({ "idx": idx }), label)
                    .map(|mut i| {
                        if self.pin_wired(rep, idx) {
                            i.enabled = false;
                            i.reason = Some(strings::text(key!("reason-box-pin-wired")).to_owned());
                        }
                        i
                    });
                out.extend(item);
            }
            None => {
                let label = strings::text(key!("action-box-add-cp")).to_owned();
                out.extend(self.aimed("box.add_cp", point, label));
            }
        }
    }

    /// The Wire tool's click on the edge of a box whose pins are its own where no pin is: the pin's name is asked
    /// in place; once it is named the wire ends at it, one undo step with the pin. The Wire tool waits meanwhile, its
    /// wire's ghost shown until the pin is named or the naming cancelled.
    pub(super) fn pin_for_wire(&mut self, rep: ew_api::output::Uuid, pin: BoxCpAt) -> Outcome {
        let tool = PinNameTool::add(ew_dl::EntityRef(rep), pin);
        let wire = std::mem::replace(&mut self.tools.tool, Box::new(tool));
        let ghost = self.tools.overlay.clone();
        if let Some(t) = self
            .tools
            .tool
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<PinNameTool>())
        {
            t.resume = Some(wire);
            t.under = ghost;
        }
        self.tool_epoch = self.tool_epoch.wrapping_add(1);
        self.tools.preview = None;
        self.announce_tool();
        Outcome::Done
    }

    /// The end of every dispatch: a named pin (or a cancelled naming) returns to Select, or hands the waiting Wire tool
    /// back, which then ends its wire at the new pin; the pin and the wire are one undo step. After Tab, the next pin of
    /// the side is asked for.
    pub(super) fn after_pin_named(&mut self) {
        fn pin_tool(s: &mut ShellModel) -> Option<&mut PinNameTool> {
            s.tools
                .tool
                .as_any_mut()
                .and_then(|a| a.downcast_mut::<PinNameTool>())
        }
        let Some(tool) = pin_tool(self) else { return };
        let (rep, next) = (tool.rep(), tool.wants_next());
        // After Tab: the next point of the side, read from the box as the commit left it.
        if next {
            let b = self.canvas().and_then(|c| {
                let t = &c.page().tables;
                Some((
                    PageGridMap::new(t.grid),
                    t.reps.iter().find(|r| r.id == rep.0)?.black_box.clone()?,
                ))
            });
            let moved = match (b, pin_tool(self)) {
                (Some((grid, b)), Some(t)) => t.go_next(grid, &b.outline, &b.cps),
                _ => false,
            };
            if !moved {
                self.notify(strings::text(key!("toast-box-pin-side-full")).to_owned(), false);
            }
        }
        if !pin_tool(self).is_some_and(|t| t.finished()) {
            return;
        }
        let Some(tool) = self
            .tools
            .tool
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<PinNameTool>())
        else {
            return;
        };
        let (resume, at, named) = (tool.resume.take(), tool.at(), tool.named());
        match resume {
            Some(wire) => {
                self.tools.tool = wire;
                self.tool_epoch = self.tool_epoch.wrapping_add(1);
                if named {
                    let rev = self.revision();
                    if let Some(sheet) = self.canvas().map(|c| c.page().snapper.page_to_sheet(at)) {
                        let _ = self.send_tool(&CanvasEvent::Press {
                            at: sheet,
                            button: Button::Primary,
                            mods: Mods::default(),
                        });
                    }
                    if self.revision() != rev
                        && let Some(p) = &mut self.project
                    {
                        p.session.fold_last_step();
                    }
                    self.sync_views();
                }
            }
            None => {
                if let Some(select) = tool_named("select") {
                    self.activate_tool(select);
                }
            }
        }
    }
}
