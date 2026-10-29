// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's duty of FT-411 (WP-S2.04 §10.7), widened by cycle 33 (G-PP ruling #4): Edit ▸ Bridge terminals,
//! the host command on the selection that draws page-less bridges between two or more selected terminals of one
//! strip, consecutive in the strip's terminal order (BR1 over :1 to :4 is 1-2, 2-3, 3-4), in one undo step.
//!
//! **Ends.** A terminal's bridge end is its part's bridge shaft (the lowest CP of the first part use whose copy has
//! a shaft-1 entry); a terminal without one (no part chosen yet, or a part without a shaft) bridges at the lowest
//! side-1 CP of its function definition, so a bridge can be drawn before the parts are chosen.
//!
//! **Reads.** The focused page's tables resolve the selection; the terminal aspects, the part copies, the function
//! definitions and the connections come from the project's base tables through the session's query when the action
//! runs (and when its menu item is evaluated). No memoised query is read, and a variant the shell cannot name (the
//! connection kind, an endpoint, a clamp key, a CP side) is inspected through its record's JSON form.
//!
//! **Commit.** The parameters go through the tool's commit path (`execute_with` in the window, `run_script`
//! headless): one bridge is one `connection.draw`, a chain of two or more one `connection.draw_many`; either is one
//! undo step.

use ew_api::AnyCommand;
use serde_json::{Value, json};

use super::interruption::natural_cmp;
use super::{Outcome, ShellModel};
use crate::strings;
use crate::tools::{EntityKind, selection_kinds};

/// A connection point: the function's ID text and the CP index.
type CpKey = (String, u16);

/// One selected terminal as the bridge rule needs it: its function, its strip, its sequence and designation, and
/// (after the end rule) its bridge end's CP index.
struct Terminal {
    function: String,
    seq: u32,
    designation: String,
    strip: String,
    end: u16,
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
    /// Whether `strip.bridge` can act on the selection: `Ok(the command)` or `Err(reason key)`.
    ///
    /// `pub(crate)` because the action's enabled predicate in `actions.rs` calls it: the Draw menu evaluates it. Its
    /// cost is linear in the project's connections plus the selected terminals' keyed lookups.
    pub(crate) fn bridge_command(&self) -> Result<AnyCommand, &'static str> {
        let selection: &'static str = "reason-bridge-selection";
        // 1. The focused page's tables; two or more selected representations, each with one function and a
        // terminal device. Two representations of one terminal count once.
        let tables = self
            .canvas()
            .map(|c| std::sync::Arc::clone(&c.page().tables))
            .ok_or(selection)?;
        let kinds = selection_kinds(&self.tools.selection, &tables);
        if kinds.len() < 2 || kinds.iter().any(|(_, kind)| *kind != EntityKind::Rep) {
            return Err(selection);
        }
        let project = self.project.as_ref().ok_or(selection)?;
        let q = project.session.query();

        let mut terminals: Vec<Terminal> = Vec::with_capacity(kinds.len());
        let mut devices = Vec::with_capacity(kinds.len());
        for (entity, _) in &kinds {
            let Some(rep) = tables.reps.iter().find(|r| r.id == entity.0) else {
                return Err(selection);
            };
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
            let function_text = function.id.as_uuid().to_string();
            if terminals.iter().any(|t| t.function == function_text) {
                continue;
            }
            terminals.push(Terminal {
                function: function_text,
                seq: terminal.seq,
                designation: terminal.designation.as_str().to_owned(),
                strip: terminal.strip.as_uuid().to_string(),
                end: 0,
            });
            devices.push((device_id, function.def));
        }
        if terminals.len() < 2 {
            return Err(selection);
        }

        // 2. Every terminal belongs to the same strip.
        if terminals.iter().any(|t| t.strip != terminals[0].strip) {
            return Err("reason-bridge-strips");
        }

        // 3. Each terminal's bridge end: the lowest CP of the first part use whose copy has a shaft-1 entry, else
        // the lowest side-1 CP of the function's definition.
        for (terminal, (device_id, def)) in terminals.iter_mut().zip(&devices) {
            let Some(device) = q.devices().get(*device_id) else {
                return Err("reason-bridge-no-shaft");
            };
            let mut shaft = None;
            for use_ in &device.parts {
                // A part use whose copy the project does not hold fails closed: the terminal has no usable shaft.
                let Some(part) = q.parts().get(use_.part.copy) else {
                    return Err("reason-bridge-no-shaft");
                };
                let lowest = part
                    .item
                    .cp_pattern
                    .iter()
                    .filter(|cp| {
                        serde_json::to_value(&cp.clamp)
                            .is_ok_and(|clamp| clamp.get("shaft").and_then(Value::as_u64) == Some(1))
                    })
                    .map(|cp| cp.idx)
                    .min();
                if lowest.is_some() {
                    shaft = lowest;
                    break;
                }
            }
            let end = shaft.or_else(|| {
                let fndef = q.fndefs().get(*def)?;
                fndef
                    .item
                    .cps
                    .iter()
                    .filter(|cp| serde_json::to_value(&cp.side).is_ok_and(|side| side == "side1"))
                    .map(|cp| cp.idx)
                    .min()
            });
            terminal.end = end.ok_or("reason-bridge-no-shaft")?;
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

        // The terminals in the strip's order: lower sequence first, then the natural order of the designation,
        // then the function ID. Consecutive terminals of that order are the chain's pairs.
        terminals.sort_by(|x, y| {
            x.seq
                .cmp(&y.seq)
                .then_with(|| natural_cmp(&x.designation, &y.designation))
                .then_with(|| x.function.cmp(&y.function))
        });
        let ends: Vec<CpKey> = terminals.iter().map(|t| (t.function.clone(), t.end)).collect();

        // 4. A pair already bridged between exactly these two CPs (either order) is kept; when every pair is, the
        // command is refused.
        let same_ends = |bridge: &Bridge, a: &CpKey, b: &CpKey| {
            (bridge.a.as_ref() == Some(a) && bridge.b.as_ref() == Some(b))
                || (bridge.a.as_ref() == Some(b) && bridge.b.as_ref() == Some(a))
        };
        let pairs: Vec<(&CpKey, &CpKey)> = ends
            .windows(2)
            .map(|w| (&w[0], &w[1]))
            .filter(|(a, b)| !bridges.iter().any(|bridge| same_ends(bridge, a, b)))
            .collect();
        if pairs.is_empty() {
            return Err("reason-bridge-exists");
        }

        // 5. The group: the smallest group of the strip's bridges that ends at one of the chain's ends, else the
        // largest group of the strip's bridges plus one, else 1.
        let strip = terminals[0].strip.as_str();
        let strip_bridges: Vec<&Bridge> = bridges
            .iter()
            .filter(|bridge| bridge.strip.as_deref() == Some(strip))
            .collect();
        let touches = |bridge: &Bridge| {
            bridge.a.as_ref().is_some_and(|cp| ends.contains(cp))
                || bridge.b.as_ref().is_some_and(|cp| ends.contains(cp))
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

        // 6. The command: no page and no route (page-less bridges).
        let connection = |(a, b): &(&CpKey, &CpKey)| {
            json!({
                "a": {"cp": {"function": a.0, "idx": a.1}},
                "b": {"cp": {"function": b.0, "idx": b.1}},
                "kind": {"bridge": {"kind": "plug_in", "group": group, "shaft": 1}},
            })
        };
        Ok(match pairs.as_slice() {
            [one] => AnyCommand {
                name: "connection.draw".into(),
                params: connection(one),
            },
            many => AnyCommand {
                name: "connection.draw_many".into(),
                params: json!({"connections": many.iter().map(connection).collect::<Vec<_>>()}),
            },
        })
    }

    /// `strip.bridge` (Edit ▸ Bridge terminals, FT-411, cycle 33): the page-less bridges between the selected
    /// terminals, consecutive in the strip's order, one undo step.
    pub(super) fn act_strip_bridge(&mut self) -> Outcome {
        match self.bridge_command() {
            Ok(cmd) => {
                // FT-549: the bridges and the terminals they join, counted from the command.
                let connections: Vec<&Value> = match cmd.params.get("connections") {
                    Some(Value::Array(many)) => many.iter().collect(),
                    _ => vec![&cmd.params],
                };
                let terminals: std::collections::BTreeSet<String> = connections
                    .iter()
                    .flat_map(|c| [&c["a"]["cp"]["function"], &c["b"]["cp"]["function"]])
                    .map(Value::to_string)
                    .collect();
                let outcome = self.commit_from_host(&cmd);
                if matches!(outcome, Outcome::Done) {
                    let bridges = connections.len();
                    let key = if bridges == 1 {
                        "toast-bridged-one"
                    } else {
                        "toast-bridged"
                    };
                    let text = strings::format(key, &[&bridges.to_string(), &terminals.len().to_string()]);
                    self.notify(text, true);
                }
                outcome
            }
            Err(reason_key) => Outcome::Refused { reason_key },
        }
    }
}
