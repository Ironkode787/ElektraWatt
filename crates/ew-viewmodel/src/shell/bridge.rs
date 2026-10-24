// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's duty of FT-411 (WP-S2.04 §10.7): Edit ▸ Bridge terminals, the host command on the selection
//! that draws the page-less `connection.draw` between two selected terminals' bridge shafts.
//!
//! **Reads.** The focused page's tables resolve the selection; the terminal aspects, the part copies and the
//! connections come from the project's base tables through the session's query when the action runs (and when its
//! menu item is evaluated). No memoised query is read, and a variant the shell cannot name (the connection kind,
//! an endpoint, a clamp key) is inspected through its record's JSON form.
//!
//! **Commit.** The parameters go through the tool's commit path (`execute_with` in the window, `run_script`
//! headless), one undo step per bridge.

use ew_api::AnyCommand;
use serde_json::{Value, json};

use super::interruption::natural_cmp;
use super::{Outcome, ShellModel};
use crate::tools::{EntityKind, selection_kinds};

/// A connection point: the function's ID text and the CP index.
type CpKey = (String, u16);

/// One selected terminal as the bridge rule needs it: its function, its strip, its sequence and designation, and
/// (after the shaft rule) its bridge-shaft CP index.
struct Terminal {
    function: String,
    seq: u32,
    designation: String,
    strip: String,
    shaft: Option<u16>,
}

/// A bridge connection as the group rule reads it: its group, its two CP ends (a junction end is none) and the
/// strip the `a` end belongs to, when that end is a terminal CP.
struct Bridge {
    group: u16,
    a: Option<CpKey>,
    b: Option<CpKey>,
    strip: Option<String>,
}

impl ShellModel {
    /// Whether `strip.bridge` can act on the selection (FT-411): `Ok(command parameters)` or `Err(reason key)`.
    ///
    /// `pub(crate)` because the action's enabled predicate in `actions.rs` calls it: the Edit menu evaluates it. Its
    /// cost is linear in the project's connections plus the two selected terminals' keyed lookups.
    pub(crate) fn bridge_params(&self) -> Result<Value, &'static str> {
        let selection: &'static str = "reason-bridge-selection";
        // 1. The focused page's tables; exactly two selected representations, each with one function and a
        // terminal device.
        let tables = self
            .canvas()
            .map(|c| std::sync::Arc::clone(&c.page().tables))
            .ok_or(selection)?;
        let kinds = selection_kinds(&self.tools.selection, &tables);
        let [(a_entity, EntityKind::Rep), (b_entity, EntityKind::Rep)] = kinds.as_slice() else {
            return Err(selection);
        };
        let Some(a_rep) = tables.reps.iter().find(|r| r.id == a_entity.0) else {
            return Err(selection);
        };
        let Some(b_rep) = tables.reps.iter().find(|r| r.id == b_entity.0) else {
            return Err(selection);
        };
        let project = self.project.as_ref().ok_or(selection)?;
        let q = project.session.query();

        let mut terminals = Vec::with_capacity(2);
        let mut devices = Vec::with_capacity(2);
        for rep in [a_rep, b_rep] {
            // The page tables and the selection carry UUIDs; the typed row ID comes from the record's canonical
            // text, which `q.reps().get` fixes by inference (the shell names no schema type).
            let Ok(rep_id) = rep.id.to_string().parse() else {
                return Err(selection);
            };
            let Some(record) = q.reps().get(rep_id) else {
                return Err(selection);
            };
            let [function] = record.functions.as_slice() else {
                return Err(selection);
            };
            let Some(function) = q.functions().get(*function) else {
                return Err(selection);
            };
            let Some(device_id) = function.device else {
                return Err(selection);
            };
            let Some(device) = q.devices().get(device_id) else {
                return Err(selection);
            };
            let Some(terminal) = device.aspects.terminal.as_ref() else {
                return Err(selection);
            };
            terminals.push(Terminal {
                function: function.id.as_uuid().to_string(),
                seq: terminal.seq,
                designation: terminal.designation.as_str().to_owned(),
                strip: terminal.strip.as_uuid().to_string(),
                shaft: None,
            });
            devices.push(device_id);
        }

        // 2. Both terminals belong to the same strip.
        if terminals[0].strip != terminals[1].strip {
            return Err(selection);
        }

        // 3. Each terminal's bridge shaft: the lowest CP of the first part use whose copy has a shaft-1 entry.
        for (terminal, device_id) in terminals.iter_mut().zip(&devices) {
            let Some(device) = q.devices().get(*device_id) else {
                return Err("reason-bridge-no-shaft");
            };
            let mut shaft = None;
            for use_ in &device.parts {
                // A part use whose copy the project does not hold fails closed: the terminal has no usable shaft.
                let Some(part) = q.parts().get(use_.part.copy) else {
                    return Err("reason-bridge-no-shaft");
                };
                let mut lowest: Option<u16> = None;
                for cp in part.item.cp_pattern.iter() {
                    let Ok(clamp) = serde_json::to_value(&cp.clamp) else {
                        continue;
                    };
                    if clamp.get("shaft").and_then(Value::as_u64) == Some(1) {
                        lowest = Some(lowest.map_or(cp.idx, |m| m.min(cp.idx)));
                    }
                }
                if lowest.is_some() {
                    shaft = lowest;
                    break;
                }
            }
            terminal.shaft = Some(shaft.ok_or("reason-bridge-no-shaft")?);
        }

        // The strip's bridges: every bridge connection whose `a` end is a CP of one of the strip's terminal
        // functions; with their two CP ends and the group. Only the `a` end's function, device and terminal aspect
        // are read, through the keyed tables, so the menu's enabled predicate stays linear in the bridges.
        let mut bridges: Vec<Bridge> = Vec::new();
        for (_, connection) in q.connections().iter() {
            let Ok(kind) = serde_json::to_value(&connection.kind) else {
                continue;
            };
            let Some(spec) = kind.get("bridge") else {
                continue;
            };
            let Some(group) = spec
                .get("group")
                .and_then(Value::as_u64)
                .and_then(|g| u16::try_from(g).ok())
            else {
                continue;
            };
            let end = |endpoint: &Value| -> Option<CpKey> {
                let cp = endpoint.get("cp")?;
                let function = cp.get(0)?.as_str()?.to_owned();
                let idx = u16::try_from(cp.get(1)?.as_u64()?).ok()?;
                Some((function, idx))
            };
            let a = serde_json::to_value(&connection.a).ok().and_then(|e| end(&e));
            let b = serde_json::to_value(&connection.b).ok().and_then(|e| end(&e));
            let strip = connection.a.function().and_then(|function| {
                let function = q.functions().get(function)?;
                let device = q.devices().get(function.device?)?;
                Some(device.aspects.terminal.as_ref()?.strip.as_uuid().to_string())
            });
            bridges.push(Bridge { group, a, b, strip });
        }

        // The terminals in the rule's order: lower sequence first, then the natural order of the designation,
        // then the function ID.
        terminals.sort_by(|x, y| {
            x.seq
                .cmp(&y.seq)
                .then_with(|| natural_cmp(&x.designation, &y.designation))
                .then_with(|| x.function.cmp(&y.function))
        });
        let (a, b) = (&terminals[0], &terminals[1]);
        let a_cp = (a.function.clone(), a.shaft.unwrap_or_default());
        let b_cp = (b.function.clone(), b.shaft.unwrap_or_default());

        // 4. An existing bridge between exactly these two CPs (either order).
        let same_ends = |bridge: &Bridge| {
            (bridge.a.as_ref() == Some(&a_cp) && bridge.b.as_ref() == Some(&b_cp))
                || (bridge.a.as_ref() == Some(&b_cp) && bridge.b.as_ref() == Some(&a_cp))
        };
        if bridges.iter().any(same_ends) {
            return Err("reason-bridge-exists");
        }

        // 5. The group: the smallest group of the strip's bridges that ends at one of the two shafts, else the
        // largest group of the strip's bridges plus one, else 1.
        let strip = a.strip.as_str();
        let strip_bridges: Vec<&Bridge> = bridges
            .iter()
            .filter(|bridge| bridge.strip.as_deref() == Some(strip))
            .collect();
        let touches = |bridge: &Bridge| {
            bridge.a.as_ref().is_some_and(|cp| cp == &a_cp || cp == &b_cp)
                || bridge.b.as_ref().is_some_and(|cp| cp == &a_cp || cp == &b_cp)
        };
        let group: u64 = if let Some(group) = strip_bridges
            .iter()
            .filter(|bridge| touches(bridge))
            .map(|bridge| bridge.group)
            .min()
        {
            u64::from(group)
        } else if let Some(group) = strip_bridges.iter().map(|bridge| bridge.group).max() {
            u64::from(group) + 1
        } else {
            1
        };

        // 6. The command's parameters: no page and no route (a page-less bridge).
        Ok(json!({
            "a": {"cp": {"function": a.function, "idx": a.shaft.unwrap_or_default()}},
            "b": {"cp": {"function": b.function, "idx": b.shaft.unwrap_or_default()}},
            "kind": {"bridge": {"kind": "plug_in", "group": group, "shaft": 1}},
        }))
    }

    /// `strip.bridge` (Edit ▸ Bridge terminals, FT-411): the page-less `connection.draw` between the two selected
    /// terminals' bridge shafts, one undo step.
    pub(super) fn act_strip_bridge(&mut self) -> Outcome {
        match self.bridge_params() {
            Ok(params) => self.commit_from_host(&AnyCommand {
                name: "connection.draw".into(),
                params,
            }),
            Err(reason_key) => Outcome::Refused { reason_key },
        }
    }
}
