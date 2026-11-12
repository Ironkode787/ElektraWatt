// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Joining an existing device from the window (FR-DEV-038, FR-DEV-005, FR-UI-017).
//!
//! **One command (D1).** A join is always `device.place`: with `existing_function` while the device has an unplaced
//! function of the symbol's FnDef (a part's template slot), else with `join_device {device, role: aux}`. The Place
//! tool's [`PlaceTarget::Join`] carries the device and its unplaced slots; the host refreshes them after every commit,
//! so the target stays sticky: the next placement fills the next slot, then makes new auxiliary functions.
//!
//! **Before placing (D2).** J (`place.join`) opens the chooser list of doc 16a §6.22 while the armed symbol's FnDef
//! can be an auxiliary function: "New device" first, then [`ew_api::Session::join_candidates`]' rows (the slots,
//! "slot 13-14", then the devices that take auxiliary functions). Enter (`chooser.choose`) arms the row's target, the
//! ghost label reads `join -K1`; "New device" restores the target before the join.
//!
//! **After placing (D3).** J (and Join device… in the page's right-click menu) with the Select tool and one placed
//! symbol selected opens the chooser of the devices it can join, its own left out; Enter is `rep.join_device`.
//! **From a part.** A part's function that a device of the part has unplaced arms the join of that device
//! ([`ShellModel::template_slots`]); "New device" is the part's new device, which also comes back once the device has
//! no more of that function.
//!
//! **The text interface (D4).** `place.pick device=<tag>` arms the join; `function=<tag>:<slot>` or
//! `function=<tag>` arms `existing_function` (a tag with one unplaced function of the armed FnDef resolves to it,
//! several are refused with their slots listed). A tag is read by the reference grammar (`Session::resolve_reference`: a
//! bare tag, `tag:`, `id:`); a UUID still works.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_api::output::{PlaceCatalogue, Uuid};
use serde_json::{Map, Value};

use super::{Outcome, ShellModel, failed};
use crate::key;
use crate::strings;
use crate::tools::{Chooser, ChooserRow, ChooserValue, PlaceTarget, PlaceTool, ToolOutput};

/// What `place.pick` arms for a join (D4).
pub(super) struct JoinPick {
    /// The device's ID text.
    pub device: String,
    /// Its tag as printed.
    pub tag: String,
    /// Its unplaced functions of the symbol's FnDef, in slot order.
    pub slots: Vec<String>,
}

impl ShellModel {
    /// The Join chooser, while open.
    pub fn chooser(&self) -> Option<&Chooser> {
        self.tools.chooser.as_ref()
    }

    /// The canvas object of the chooser's active row, which shows the hover outline while the row is active (doc 16a
    /// §6.22, §7.1): the row's device's first representation on the focused page (in the page table's order); none
    /// for "New device" or a device not drawn on this page.
    pub fn chooser_object(&self) -> Option<ew_api::output::ew_dl::EntityRef> {
        let (ChooserValue::Join { device, .. } | ChooserValue::JoinRep { device, .. }) =
            &self.chooser()?.current()?.value
        else {
            return None;
        };
        let device = Uuid::parse_str(device).ok()?;
        let page = self.canvas()?.page();
        page.tables
            .reps
            .iter()
            .find(|r| r.device == Some(device))
            .map(|r| ew_api::output::ew_dl::EntityRef(r.id))
    }

    /// The active Place tool, if any.
    fn place_tool(&mut self) -> Option<&mut PlaceTool> {
        self.tools
            .tool
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<PlaceTool>())
    }

    /// Whether the active Place tool offers the Join chooser (its symbol's FnDef can be an auxiliary function).
    pub(crate) fn place_joinable(&self) -> bool {
        self.tools.tool.name() == "place" && self.tool_ghost().is_some_and(|g| g.joinable())
    }

    /// The placed symbol J offers to join another device (FT-1658: J on a placed contact was refused as "not a
    /// contact"): with the Select tool active, the one representation selected on the page shown, which draws one
    /// function of a device; with that device and the function's FnDef.
    pub(crate) fn join_rep(&self) -> Option<(Uuid, Uuid, Uuid)> {
        if self.tools.tool.name() != "select" {
            return None;
        }
        let page = self.canvas()?.page();
        let mut reps = page.tables.reps.iter().filter(|r| {
            self.tools
                .selection
                .items
                .contains(&ew_api::output::ew_dl::EntityRef(r.id))
        });
        let (Some(rep), None) = (reps.next(), reps.next()) else {
            return None;
        };
        let device = rep.device?;
        let [function] = rep.functions.as_slice() else {
            return None;
        };
        let q = self.project.as_ref()?.session.query();
        let fndef = q
            .functions()
            .get(function.to_string().parse().ok()?)?
            .def
            .as_uuid();
        Some((rep.id, device, fndef))
    }

    /// The device a join names: a device's UUID, or a reference (a bare tag such as `-101K1`,
    /// `tag:`, `id:`) that resolves to a device.
    fn join_device_ref(&self, text: &str) -> Result<Uuid, String> {
        let Some(project) = &self.project else {
            return Err(strings::text(key!("reason-no-project")).to_owned());
        };
        let id = match text.trim().parse::<Uuid>() {
            Ok(id) => id,
            Err(_) => project
                .session
                .resolve_reference(text.trim())
                .map_err(|why| super::tools::rejected_words("", Some(&why)))?,
        };
        if project.session.device_tag(id).is_none() {
            return Err(strings::format(key!("error-join-not-device"), &[text]));
        }
        Ok(id)
    }

    /// The FnDef copy the catalogue's symbol row `i` draws; a starter symbol the project lacks is not embedded here: the
    /// placement carries it (FT-656), so `place.pick device=` is one undo step.
    fn row_fndef(
        &mut self,
        cat: &PlaceCatalogue,
        i: usize,
    ) -> Result<(Arc<PlaceCatalogue>, usize, Uuid), Outcome> {
        let (row, _) = self.starter_row(&cat.symbols[i].key)?;
        let shape = self
            .row_shape(&row)
            .ok_or(Outcome::refused(key!("reason-unknown-symbol")))?;
        let cat = self
            .catalogue()
            .ok_or(Outcome::refused(key!("reason-no-catalogue")))?;
        Ok((cat, i, shape.fndef))
    }

    /// `place.pick device=<tag>` (D4): the join target on the device for symbol row `i`. A device with no unplaced
    /// function of the symbol's FnDef takes it as a new auxiliary function, which only a contact can be.
    pub(super) fn pick_join_device(
        &mut self,
        cat: &PlaceCatalogue,
        i: usize,
        device: &str,
    ) -> Result<(Arc<PlaceCatalogue>, usize, JoinPick), Outcome> {
        let id = self
            .join_device_ref(device)
            .map_err(|reason| failed(key!("error-join-device"), &[device, &reason]))?;
        let (cat, i, fndef) = self.row_fndef(cat, i)?;
        let Some(project) = &self.project else {
            return Err(Outcome::refused(key!("reason-no-project")));
        };
        let slots: Vec<String> = project
            .session
            .unplaced_slots(id, fndef)
            .into_iter()
            .map(|(f, _)| f.to_string())
            .collect();
        if slots.is_empty() && !project.session.auxiliary_fndef(fndef) {
            return Err(Outcome::refused(key!("reason-join-not-auxiliary")));
        }
        let tag = project.session.device_tag(id).unwrap_or_default();
        Ok((
            cat,
            i,
            JoinPick {
                device: id.to_string(),
                tag,
                slots,
            },
        ))
    }

    /// `place.pick function=<tag>[:<slot>]` (D4): the unplaced function of the symbol's FnDef that the text names. The
    /// whole text is read as a device first (a terminal's tag holds a `:`), else the part after the last `:` is the
    /// slot (the function's CP designations joined by `-`, `13-14`).
    pub(super) fn pick_function_tag(
        &mut self,
        cat: &PlaceCatalogue,
        i: usize,
        text: &str,
    ) -> Result<(Arc<PlaceCatalogue>, usize, String), Outcome> {
        let (device, slot) = match self.join_device_ref(text) {
            Ok(id) => (id, None),
            Err(whole) => match text.rsplit_once(':') {
                Some((tag, slot)) if !tag.is_empty() && !slot.is_empty() => match self.join_device_ref(tag) {
                    Ok(id) => (id, Some(slot.trim().to_owned())),
                    Err(_) => return Err(failed(key!("error-join-device"), &[text, &whole])),
                },
                _ => return Err(failed(key!("error-join-device"), &[text, &whole])),
            },
        };
        let (cat, i, fndef) = self.row_fndef(cat, i)?;
        let Some(project) = &self.project else {
            return Err(Outcome::refused(key!("reason-no-project")));
        };
        let tag = project.session.device_tag(device).unwrap_or_default();
        let slots = project.session.unplaced_slots(device, fndef);
        let function_key = cat.symbols[i].fndef_key.clone();
        let listed = || {
            slots
                .iter()
                .map(|(_, s)| format!("{tag}:{s}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let function = match (slot, slots.as_slice()) {
            (Some(slot), _) => match slots.iter().find(|(_, s)| *s == slot) {
                Some((f, _)) => *f,
                None => {
                    let list = if slots.is_empty() {
                        strings::text(key!("error-join-none")).to_owned()
                    } else {
                        listed()
                    };
                    return Err(failed(
                        key!("error-join-unknown-slot"),
                        &[&tag, &slot, &function_key, &list],
                    ));
                }
            },
            (None, []) => return Err(failed(key!("error-join-no-slot"), &[&tag, &function_key])),
            (None, [(f, _)]) => *f,
            (None, many) => {
                return Err(failed(
                    key!("error-join-slots"),
                    &[&tag, &many.len().to_string(), &function_key, &listed()],
                ));
            }
        };
        Ok((cat, i, function.to_string()))
    }

    // -----------------------------------------------------------------------------------------------------------
    // The Join chooser (D2)

    /// `place.join` (J): opens the Join chooser of the active Place tool: "New device", then the devices the symbol
    /// can join. The active row is the current target's.
    pub(crate) fn act_place_join(&mut self) -> Outcome {
        if let Some((rep, own, fndef)) = self.join_rep() {
            return self.open_rep_join(rep, own, fndef);
        }
        if !self.place_joinable() {
            return Outcome::refused(key!("reason-join-not-auxiliary"));
        }
        let Some((fndef, current)) = self.place_tool().map(|p| {
            let current = match p.target() {
                PlaceTarget::Join { device, slots, .. } => Some((device.clone(), slots.first().cloned())),
                _ => None,
            };
            (p.shape().fndef, current)
        }) else {
            return Outcome::refused(key!("reason-join-not-auxiliary"));
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let candidates = project.session.join_candidates(fndef);
        let mut rows = vec![ChooserRow {
            name: strings::text(key!("chooser-new-device")).to_owned(),
            kind: String::new(),
            value: ChooserValue::NewDevice,
        }];
        rows.extend(candidates.rows.into_iter().map(|c| ChooserRow {
            name: c.tag.clone(),
            kind: if c.function.is_some() {
                strings::format(key!("chooser-kind-slot"), &[&c.slot])
            } else {
                strings::text(key!("chooser-kind-auxiliary")).to_owned()
            },
            value: ChooserValue::Join {
                device: c.device.to_string(),
                tag: c.tag,
                function: c.function.map(|f| f.to_string()),
            },
        }));
        // The current target's row: its slot's row, else the device's first row.
        let row = current
            .and_then(|(device, slot)| {
                let on = |r: &ChooserRow, want: Option<&String>| {
                    matches!(&r.value, ChooserValue::Join { device: d, function, .. }
                        if *d == device && (want.is_none() || function.as_ref() == want))
                };
                rows.iter()
                    .position(|r| on(r, slot.as_ref()))
                    .or_else(|| rows.iter().position(|r| on(r, None)))
            })
            .unwrap_or(0);
        self.palette = None;
        self.tools.picker = None;
        self.tools.chooser = Some(Chooser::new(
            strings::text(key!("chooser-join-question")).to_owned(),
            rows,
            row,
        ));
        Outcome::Done
    }

    /// The devices the placed symbol `rep` (of device `own`, drawing a function of FnDef `fndef`) can join, one row per
    /// device (its first slot, else auxiliary), its own device left out: the rows of [`Self::open_rep_join`]'s chooser,
    /// and whether the canvas menu offers Join device… (FT-1693).
    pub(crate) fn rep_join_rows(&self, rep: Uuid, own: Uuid, fndef: Uuid) -> Vec<ChooserRow> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let mut rows: Vec<ChooserRow> = Vec::new();
        for c in project.session.join_candidates(fndef).rows {
            let device = c.device.to_string();
            let listed = rows
                .iter()
                .any(|r| matches!(&r.value, ChooserValue::JoinRep { device: d, .. } if *d == device));
            if c.device == own || listed {
                continue;
            }
            rows.push(ChooserRow {
                name: c.tag,
                kind: if c.function.is_some() {
                    strings::format(key!("chooser-kind-slot"), &[&c.slot])
                } else {
                    strings::text(key!("chooser-kind-auxiliary")).to_owned()
                },
                value: ChooserValue::JoinRep {
                    rep: rep.to_string(),
                    device,
                },
            });
        }
        rows
    }

    /// J with one placed symbol selected: the Join chooser of the devices it can join ([`Self::rep_join_rows`]);
    /// Enter joins it with `rep.join_device`, its wires kept.
    fn open_rep_join(&mut self, rep: Uuid, own: Uuid, fndef: Uuid) -> Outcome {
        if self.project.is_none() {
            return Outcome::refused(key!("reason-no-project"));
        }
        let rows = self.rep_join_rows(rep, own, fndef);
        if rows.is_empty() {
            return Outcome::refused(key!("reason-join-no-device"));
        }
        self.palette = None;
        self.tools.picker = None;
        self.tools.chooser = Some(Chooser::new(
            strings::text(key!("chooser-join-question")).to_owned(),
            rows,
            0,
        ));
        Outcome::Done
    }

    /// `chooser.row {step | to}`: Up and Down (`step` −1, 1, wrapping), Home (`to` 0), End (`to` −1).
    pub(crate) fn act_chooser_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let step = p.get("step").and_then(Value::as_i64);
        let to = p.get("to").and_then(Value::as_i64);
        if step.is_none() && to.is_none() {
            return failed(key!("error-missing-param"), &["step"]);
        }
        let Some(c) = &mut self.tools.chooser else {
            return Outcome::refused(key!("reason-no-chooser"));
        };
        c.move_row(step, to);
        Outcome::Done
    }

    /// `chooser.type {text}`: type-ahead on the rows' names (doc 16a §6.22).
    pub(crate) fn act_chooser_type(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match super::string_param(p, "text") {
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["text"]),
            Err(e) => return e,
        };
        let Some(c) = &mut self.tools.chooser else {
            return Outcome::refused(key!("reason-no-chooser"));
        };
        c.type_ahead(&text);
        Outcome::Done
    }

    /// `chooser.choose {row?}`: picks the active row (or row `row`) and closes the chooser. A device row arms the join
    /// (D2): the ghost label reads `join -K1`; "New device" restores the target before the join.
    pub(crate) fn act_chooser_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(c) = &self.tools.chooser else {
            return Outcome::refused(key!("reason-no-chooser"));
        };
        let index = match p.get("row") {
            None => c.row,
            Some(v) => match v.as_u64().and_then(|i| usize::try_from(i).ok()) {
                Some(i) if i < c.rows.len() => i,
                _ => return failed(key!("error-bad-param"), &["row"]),
            },
        };
        let Some(row) = c.rows.get(index).cloned() else {
            return Outcome::refused(key!("reason-no-chooser"));
        };
        self.tools.chooser = None;
        let out = match row.value {
            ChooserValue::JoinRep { rep, device } => {
                let params = serde_json::json!({ "rep": rep, "device": device });
                let (outcome, notices) = self.run_logged_or_commit("rep.join_device", &params, None);
                if outcome == Outcome::Done
                    && let Some(toast) = rep.parse().ok().and_then(|r| self.joined_toast(r, &notices))
                {
                    self.notify(toast, true);
                }
                return outcome;
            }
            ChooserValue::NewDevice => self.with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<PlaceTool>())
                    .map(|p| p.unjoin(cx))
            }),
            ChooserValue::Join {
                device,
                tag,
                function,
            } => {
                let fndef = self.place_tool().map(|p| p.shape().fndef);
                let mut slots: Vec<String> = match (fndef, device.parse::<Uuid>(), &self.project) {
                    (Some(fndef), Ok(id), Some(project)) => project
                        .session
                        .unplaced_slots(id, fndef)
                        .into_iter()
                        .map(|(f, _)| f.to_string())
                        .collect(),
                    _ => Vec::new(),
                };
                // The chosen slot is filled first.
                if let Some(f) = function
                    && let Some(at) = slots.iter().position(|s| *s == f)
                {
                    let f = slots.remove(at);
                    slots.insert(0, f);
                }
                let target = PlaceTarget::Join { device, tag, slots };
                self.with_tool(|t, cx| {
                    t.as_any_mut()
                        .and_then(|a| a.downcast_mut::<PlaceTool>())
                        .map(|p| p.join(target, cx))
                })
            }
        };
        if let Some(Some(ToolOutput::Transient(o))) = out {
            self.tools.overlay = o;
        }
        self.tools.preview = None;
        self.announce_place();
        Outcome::Done
    }

    /// The unplaced template functions of the project's devices, by (part copy, template entry): the device a
    /// placement of that entry of that part fills, as its ID text, its tag and that function's ID text; of several
    /// devices the first in natural tag order. A contact of -102K1's part picked in the device browser fills -102K1's
    /// free contact instead of making a second relay (FT-1658).
    pub(crate) fn template_slots(&self) -> BTreeMap<(Uuid, u16), (String, String, String)> {
        let mut out = BTreeMap::new();
        let Some(project) = &self.project else {
            return out;
        };
        let session = &project.session;
        let q = session.query();
        let idx = q.idx();
        let mut found: Vec<((Uuid, u16), String, String, String)> = Vec::new();
        for (id, f) in q.functions().iter() {
            let (Some(device), Some(entry)) = (f.device, f.template_entry) else {
                continue;
            };
            if idx.reps_of_function(id).next().is_some() {
                continue;
            }
            let Some(part) = q.devices().get(device).and_then(|d| d.parts.first()) else {
                continue;
            };
            let (device, function) = (device.as_uuid(), id.as_uuid());
            // A slot as `device.place`'s `existing_function` takes it: a normal function (no black box).
            if !session
                .unplaced_slots(device, f.def.as_uuid())
                .iter()
                .any(|(s, _)| *s == function)
            {
                continue;
            }
            let tag = session.device_tag(device).unwrap_or_default();
            found.push((
                (part.part.copy.as_uuid(), entry),
                tag,
                device.to_string(),
                function.to_string(),
            ));
        }
        found.sort_by(|a, b| super::interruption::natural_cmp(&a.1, &b.1).then_with(|| a.2.cmp(&b.2)));
        for (key, tag, device, function) in found {
            out.entry(key).or_insert((device, tag, function));
        }
        out
    }

    /// A join target after a commit (D1, sticky): its slots and tag as the project has them now. The slots it had
    /// that are still unplaced keep their order (a chosen slot stays first until it is filled); new ones follow. A
    /// join a part's placement made goes back to placing a new device of the part once the part's slots are filled:
    /// the part has no more of that function.
    pub(super) fn refresh_join_target(&mut self) {
        let Some((device, fndef, before)) = self.place_tool().and_then(|p| match p.target() {
            PlaceTarget::Join { device, slots, .. } => Some((device.clone(), p.shape().fndef, slots.clone())),
            _ => None,
        }) else {
            return;
        };
        let Ok(id) = device.parse::<Uuid>() else { return };
        let Some(project) = &self.project else { return };
        let now: Vec<String> = project
            .session
            .unplaced_slots(id, fndef)
            .into_iter()
            .map(|(f, _)| f.to_string())
            .collect();
        let filled = !before.is_empty() && now.is_empty();
        let mut slots: Vec<String> = before.into_iter().filter(|f| now.contains(f)).collect();
        slots.extend(now.into_iter().filter(|f| !slots.contains(f)).collect::<Vec<_>>());
        let tag = project.session.device_tag(id);
        if filled
            && self
                .place_tool()
                .is_some_and(|p| matches!(p.base(), Some(PlaceTarget::Part { .. })))
        {
            let out = self.with_tool(|t, cx| {
                t.as_any_mut()
                    .and_then(|a| a.downcast_mut::<PlaceTool>())
                    .map(|p| p.unjoin(cx))
            });
            if let Some(Some(ToolOutput::Transient(o))) = out {
                self.tools.overlay = o;
            }
            return;
        }
        if let Some(p) = self.place_tool() {
            p.set_join_state(slots, tag);
        }
    }

    /// Arms the active Place tool, just armed for a part's template entry `template` of part copy `part`, with the
    /// join of the device that has that entry unplaced ([`Self::template_slots`]); the part's placement stays its
    /// "New device" (J). Nothing changes when no device has it.
    pub(super) fn join_template_slot(&mut self, part: Uuid, template: u16) {
        let Some((device, tag, function)) = self.template_slots().remove(&(part, template)) else {
            return;
        };
        let fndef = self.place_tool().map(|p| p.shape().fndef);
        let mut slots: Vec<String> = match (fndef, device.parse::<Uuid>(), &self.project) {
            (Some(fndef), Ok(id), Some(project)) => project
                .session
                .unplaced_slots(id, fndef)
                .into_iter()
                .map(|(f, _)| f.to_string())
                .filter(|f| *f != function)
                .collect(),
            _ => Vec::new(),
        };
        slots.insert(0, function);
        let target = PlaceTarget::Join { device, tag, slots };
        let out = self.with_tool(|t, cx| {
            t.as_any_mut()
                .and_then(|a| a.downcast_mut::<PlaceTool>())
                .map(|p| p.join(target, cx))
        });
        if let Some(Some(ToolOutput::Transient(o))) = out {
            self.tools.overlay = o;
        }
    }
}
