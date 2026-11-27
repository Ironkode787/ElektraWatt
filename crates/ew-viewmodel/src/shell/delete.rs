// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Delete on a page (`edit.delete_selection`) and the devices it leaves undrawn: a device that loses its last
//! representation goes with it in the same command unless it holds more than the drawing, a device no page draws is
//! deleted itself, one drawn only on other pages is refused, and the toast names the devices that stay without a
//! symbol.
//!
//! Not here: Cut (`shell/clipboard.rs`); the toast's sentence about the devices left without a symbol is made after
//! the commit in `shell/editing.rs`.

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::output::PageTables;
use ew_dl::EntityRef;
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel};
use crate::key;
use crate::tools::{CanvasEvent, EntityKind, ResultNote, selection_kinds, transform};

/// The wires on the page tables `t` drawn to a connection point of the representations `reps` (whose drawing symbol
/// is one of them) that the selection `drawn` does not name, in page-table order: once the symbols go such a wire would
/// be drawn on no page, so the delete takes it with it.
fn wires_at(t: &PageTables, reps: &[EntityRef], drawn: &crate::tools::Selection) -> Vec<EntityRef> {
    let mut wires: Vec<EntityRef> = Vec::new();
    for c in t.cps.iter().filter(|c| reps.contains(&EntityRef(c.rep))) {
        for w in &c.connections {
            let w = EntityRef(*w);
            if !drawn.items.contains(&w) && !wires.contains(&w) {
                wires.push(w);
            }
        }
    }
    wires
}

/// The result note of a delete of the representations `reps`: the names of the interruption points among them, in
/// page-table order, the wires `wires` it takes with them, `unplaced`, the devices it leaves without a symbol, and
/// `undrawn`, the terminals it leaves drawn nowhere; `None` when none applies.
fn delete_note(
    reps: &[EntityRef],
    t: &PageTables,
    wires: Vec<EntityRef>,
    unplaced: Vec<(EntityRef, String, bool)>,
    undrawn: Vec<EntityRef>,
) -> Option<ResultNote> {
    let ips: Vec<&ew_api::output::TableRep> = t
        .reps
        .iter()
        .filter(|r| r.ip.is_some() && reps.contains(&EntityRef(r.id)))
        .collect();
    if ips.is_empty() && unplaced.is_empty() && wires.is_empty() && undrawn.is_empty() {
        return None;
    }
    // An interruption point's wires are its own sentence; the others are counted apart.
    let at_ips: Vec<EntityRef> = ips.iter().map(|r| EntityRef(r.id)).collect();
    let ip_wires: Vec<EntityRef> = wires
        .iter()
        .copied()
        .filter(|w| {
            t.cps
                .iter()
                .any(|c| at_ips.contains(&EntityRef(c.rep)) && c.connections.contains(&w.0))
        })
        .collect();
    let others: Vec<EntityRef> = wires.into_iter().filter(|w| !ip_wires.contains(w)).collect();
    Some(ResultNote::Deleted {
        names: ips
            .iter()
            .filter_map(|r| r.ip.as_ref().map(|ip| ip.name.clone()))
            .collect(),
        wires: ip_wires,
        others,
        unplaced,
        undrawn,
    })
}

impl ShellModel {
    /// Whether Delete on the schematic page shown has something of the selection to act on, as
    /// [`Self::act_delete_selection`] resolves it: what the page draws of it, a device no page draws (deleted itself),
    /// or a device or a function another page draws (refused with its own reason). A page chosen in the page list has
    /// none, so Edit ▸ Delete is not offered for it.
    pub(crate) fn delete_has_target(&self) -> bool {
        if self.selection_drawn_here() {
            return true;
        }
        let Some(page) = self.canvas().map(|c| Arc::clone(c.page())) else {
            return false;
        };
        let (unused, elsewhere) = self.undrawn_devices(&self.copy_targets(), &page.tables);
        !unused.is_empty() || elsewhere
    }

    /// `edit.delete_selection` (Delete): `edit.delete` with every entity of the selection the page tables
    /// resolve.
    pub(crate) fn act_delete_selection(&mut self) -> Outcome {
        self.delete_selection(false)
    }

    /// Delete's `edit.delete` of the selection; for a cut (`cut`) its toast says what the cut took instead, without
    /// the sentences of the devices and terminals it leaves undrawn (its paste brings them back).
    pub(super) fn delete_selection(&mut self, cut: bool) -> Outcome {
        // In a station's view Delete takes the card under the cursor out of its slot (a valve off its position).
        if self.station_tab().is_some() {
            let empty = Map::from_iter([("op".to_owned(), Value::String("empty".to_owned()))]);
            return self.act_plc_rack_slot(&empty);
        }
        self.cancel_pending_move();
        // A device or a function selected by reference (`select.set` with `tag:…`) is deleted as
        // the representations the page shown draws of it, as Cut takes it; a device that no page draws (an empty strip,
        // a device never placed) is deleted itself. A device drawn only on other pages is refused, never deleted there.
        let targets = self.copy_targets();
        let Some(page) = self.canvas().map(|c| Arc::clone(c.page())) else {
            return Outcome::refused(key!("reason-nothing-selected"));
        };
        // On a panel page the selected placements, carriers, legends and holes by hand, without the undrawn-device
        // step (a placement's device is never deleted by it).
        if page.panel.is_some() {
            let list: Vec<Value> = crate::tools::page_selection_kinds(&self.tools.selection, &page)
                .into_iter()
                .filter(|(_, k)| {
                    matches!(
                        k,
                        EntityKind::Placement | EntityKind::Carrier | EntityKind::Block | EntityKind::Feature
                    )
                })
                .map(|(e, k)| json!({"kind": k.kind_name(), "id": e.0.to_string()}))
                .collect();
            if list.is_empty() {
                return Outcome::refused(key!("reason-nothing-selected"));
            }
            let cmd = AnyCommand {
                name: "edit.delete".into(),
                params: json!({ "targets": list }),
            };
            let out = self.commit_from_host(&cmd);
            if out == Outcome::Done {
                self.tools.selection.clear();
            }
            return out;
        }
        let mut drawn = crate::tools::Selection::default();
        drawn.apply(&crate::tools::SelectionChange::Replace(
            targets
                .iter()
                .copied()
                .filter(|e| crate::tools::entity_kind(&page.tables, *e).is_some())
                .collect(),
        ));
        let (mut unused, elsewhere) = self.undrawn_devices(&targets, &page.tables);
        // G-PP finding 5: a device that loses its last representation with this delete goes with it in the same
        // command (one undo step), unless it has another function or a terminal role; the run left a phantom `-?G1`
        // with its part in the parts list.
        let reps: Vec<EntityRef> = selection_kinds(&drawn, &page.tables)
            .into_iter()
            .filter(|(_, k)| *k == EntityKind::Rep)
            .map(|(e, _)| e)
            .collect();
        for d in self.devices_left_undrawn(&reps) {
            if !unused.contains(&d) {
                unused.push(d);
            }
        }
        // The wires drawn to the symbols' pins go with them, so no wire stays drawn on no page.
        let wires = wires_at(&page.tables, &reps, &drawn);
        let mut cmd = transform::delete_command(&drawn, &page);
        if !unused.is_empty() || !wires.is_empty() {
            let mut list: Vec<Value> = cmd
                .as_ref()
                .and_then(|c| c.params["targets"].as_array().cloned())
                .unwrap_or_default();
            list.extend(
                unused
                    .iter()
                    .map(|e| json!({"kind": "device", "id": e.0.to_string()})),
            );
            list.extend(
                wires
                    .iter()
                    .map(|w| json!({"kind": "connection", "id": w.0.to_string()})),
            );
            cmd = Some(Box::new(AnyCommand {
                name: "edit.delete".into(),
                params: json!({ "targets": list }),
            }));
        }
        let Some(cmd) = cmd else {
            return Outcome::refused(if elsewhere {
                key!("reason-not-on-page")
            } else {
                key!("reason-nothing-selected")
            });
        };
        // The toast counts the wires the delete takes with the symbols. A device that keeps no symbol but stays in the
        // project (EW-CORE-047) is named, with where it stays; terminals left drawn nowhere are counted, with a way to
        // remove them from the project too.
        let note = if cut {
            (!reps.is_empty()).then(|| ResultNote::Cut {
                symbols: reps.len(),
                terminals: page
                    .tables
                    .reps
                    .iter()
                    .filter(|r| reps.contains(&EntityRef(r.id)))
                    .all(|r| r.terminal),
                wires,
            })
        } else {
            let unplaced = self.devices_kept_unplaced(&reps, &unused);
            let undrawn = self.terminals_left_undrawn(&reps);
            delete_note(&reps, &page.tables, wires, unplaced, undrawn)
        };
        let out = self.commit_from_host(cmd.as_ref());
        if out == Outcome::Done {
            self.tools.result_note = note;
        }
        out
    }

    /// The devices that deleting the representations `reps` leaves without any representation, in ID order: every
    /// function of the device is drawn only by `reps` or by nothing. A device is kept (its functions stay unplaced, as
    /// before, and EW-CORE-047 reports it) when another representation draws one of its functions, when it has a
    /// terminal or strip role, child devices, or a cable, assembly or PLC aspect, or when one of its functions
    /// carries intent beyond the deleted drawing: a
    /// potential definition or a bond at one of its CPs, or a connection that is page-less or drawn on a page other
    /// than the deleted representations' pages. Deleting the device would drop or cascade that intent unseen.
    fn devices_left_undrawn(&self, reps: &[EntityRef]) -> Vec<EntityRef> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let idx = q.idx();
        let deleted: std::collections::BTreeSet<_> =
            reps.iter().filter_map(|e| e.0.to_string().parse().ok()).collect();
        let mut functions = std::collections::BTreeSet::new();
        for r in &deleted {
            if let Some(rep) = q.reps().get(*r) {
                functions.extend(rep.functions.iter().copied());
            }
        }
        let devices: std::collections::BTreeSet<_> = functions
            .iter()
            .filter_map(|f| q.functions().get(*f).and_then(|f| f.device))
            .collect();
        let pages: std::collections::BTreeSet<_> = deleted
            .iter()
            .filter_map(|r| q.reps().get(*r).map(|r| r.page))
            .collect();
        let carries_intent = |f| {
            q.potential_defs().iter().any(|(_, p)| p.at.function() == Some(f))
                || q.bonds()
                    .iter()
                    .any(|(_, b)| b.a.function() == Some(f) || b.b.function() == Some(f))
                || idx.connected_cps_of_function(f).iter().any(|(_, cs)| {
                    cs.iter().any(|c| {
                        q.connections()
                            .get(*c)
                            .is_some_and(|c| c.page.is_none_or(|p| !pages.contains(&p)))
                    })
                })
        };
        let mut out = Vec::new();
        for id in devices {
            let Some(d) = q.devices().get(id) else {
                continue;
            };
            let a = &d.aspects;
            // A PLC module whose aspect holds no setting (what normaliser N0 gives a card's device) is no other
            // role: the card goes with its last symbol, its channels with it (the model's delete policy), so a card
            // deleted to be drawn again leaves no `-?A1` behind. A station, or a module with a slot, a start, a part
            // use or an integrated I/O, stays.
            let set_module = a
                .plc_module
                .as_ref()
                .is_some_and(|m| serde_json::to_value(m).map_or(true, |v| v != json!({})));
            if a.terminal.is_some()
                || a.strip.is_some()
                || a.cable.is_some()
                || a.assembly.is_some()
                || a.plc_station.is_some()
                || set_module
                || q.devices().iter().any(|(_, c)| c.parent == Some(id))
            {
                continue;
            }
            // A function no representation draws (a part's other contacts) is no other role (a relay
            // placed by one contact and deleted left `-?K2` with its four unplaced contacts).
            let all_gone = idx
                .functions_of_device(id)
                .all(|f| idx.reps_of_function(f).all(|r| deleted.contains(&r)) && !carries_intent(f));
            if all_gone {
                out.push(EntityRef(id.as_uuid()));
            }
        }
        out
    }

    /// The devices that deleting the representations `reps` leaves without any representation while the delete keeps
    /// them (not in `deleted`, the devices the same command deletes), in ID order: each as the device, its tag and whether
    /// it has a part. Terminals and strips are left out (a terminal stays in its strip).
    fn devices_kept_unplaced(
        &self,
        reps: &[EntityRef],
        deleted: &[EntityRef],
    ) -> Vec<(EntityRef, String, bool)> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let idx = q.idx();
        let gone: std::collections::BTreeSet<_> =
            reps.iter().filter_map(|e| e.0.to_string().parse().ok()).collect();
        let devices: std::collections::BTreeSet<_> = gone
            .iter()
            .filter_map(|r| q.reps().get(*r))
            .flat_map(|r| r.functions.iter().copied())
            .filter_map(|f| q.functions().get(f).and_then(|f| f.device))
            .collect();
        devices
            .into_iter()
            .filter(|d| !deleted.contains(&EntityRef(d.as_uuid())))
            .filter_map(|d| {
                let dev = q.devices().get(d)?;
                if dev.aspects.terminal.is_some() || dev.aspects.strip.is_some() {
                    return None;
                }
                let left = idx
                    .functions_of_device(d)
                    .any(|f| idx.reps_of_function(f).any(|r| !gone.contains(&r)));
                if left {
                    return None;
                }
                let tag = project.session.device_tag(d.as_uuid())?;
                Some((EntityRef(d.as_uuid()), tag, !dev.parts.is_empty()))
            })
            .collect()
    }

    /// The terminals that deleting the representations `reps` leaves drawn by no symbol of the project, in ID order:
    /// they stay in their strip and in the lists (doc 06 §3.4), and the toast offers to remove them from the project.
    fn terminals_left_undrawn(&self, reps: &[EntityRef]) -> Vec<EntityRef> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let idx = q.idx();
        let gone: std::collections::BTreeSet<_> =
            reps.iter().filter_map(|e| e.0.to_string().parse().ok()).collect();
        let devices: std::collections::BTreeSet<_> = gone
            .iter()
            .filter_map(|r| q.reps().get(*r))
            .flat_map(|r| r.functions.iter().copied())
            .filter_map(|f| q.functions().get(f).and_then(|f| f.device))
            .filter(|d| q.devices().get(*d).is_some_and(|d| d.aspects.terminal.is_some()))
            .collect();
        devices
            .into_iter()
            .filter(|d| {
                idx.functions_of_device(*d)
                    .all(|f| idx.reps_of_function(f).all(|r| gone.contains(&r)))
            })
            .map(|d| EntityRef(d.as_uuid()))
            .collect()
    }

    /// Of `targets`, the entities the page tables do not hold: the devices no representation of the project draws
    /// (and no terminal names as its strip), in target order; and whether any other target is a device or a function
    /// that some other page draws.
    fn undrawn_devices(
        &self,
        targets: &[EntityRef],
        tables: &ew_api::output::PageTables,
    ) -> (Vec<EntityRef>, bool) {
        let mut unused = Vec::new();
        let mut elsewhere = false;
        let Some(project) = &self.project else {
            return (unused, elsewhere);
        };
        let q = project.session.query();
        for e in targets {
            if crate::tools::entity_kind(tables, *e).is_some() {
                continue;
            }
            let Ok(device_id) = e.0.to_string().parse() else {
                continue;
            };
            let Some(device) = q.devices().get(device_id) else {
                // A function drawn on another page (a device's own functions are handled through the device).
                elsewhere |= q
                    .reps()
                    .iter()
                    .any(|(_, r)| r.functions.iter().any(|f| f.as_uuid() == e.0));
                continue;
            };
            let functions: Vec<_> = q
                .functions()
                .iter()
                .filter(|(_, f)| f.device == Some(device.id))
                .map(|(_, f)| f.id)
                .collect();
            let drawn = q
                .reps()
                .iter()
                .any(|(_, r)| r.functions.iter().any(|f| functions.contains(f)));
            if drawn {
                elsewhere = true;
            } else if !q
                .devices()
                .iter()
                .any(|(_, d)| d.aspects.terminal.as_ref().is_some_and(|t| t.strip == device.id))
            {
                unused.push(*e);
            }
        }
        (unused, elsewhere)
    }

    /// Cancels a pending keyboard move or drag before Delete acts on the selection (Delete deletes; the move is
    /// a gesture not yet committed, so it goes first).
    fn cancel_pending_move(&mut self) {
        if matches!(self.tools.tool.state(), "moving" | "dragging") {
            let _ = self.with_tool(|t, cx| t.on_event(&CanvasEvent::Escape, cx));
            self.tools.overlay = crate::tools::Overlay::default();
            self.tools.preview = None;
        }
    }
}
