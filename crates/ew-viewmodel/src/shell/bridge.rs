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
//! **Selection by records (WP-S2.07 D9, FT-548).** The window selection's items of any kind count: a representation
//! with one function, a function, or a device; each resolves to the terminal function of a terminal device (a device:
//! its function). No canvas and no page is needed.
//!
//! **Terminals, not CPs (D10, FT-585).** Two terminals are bridged when a bridge connection joins any CP of one to any
//! CP of the other; such a pair is kept, and the command is refused when every pair is; the group rule's "touches"
//! reads terminals. The stored CPs stay what was drawn.
//!
//! **Remove bridge (D11).** `strip.unbridge` deletes the bridge connections with both ends on the selected terminals in
//! one `edit.delete`, and refuses when that would leave a group in two pieces.
//!
//! **Reads.** The terminal aspects, the part copies, the function definitions and the connections come from the
//! project's base tables through the session's query when the action runs (and when its menu item is evaluated). No
//! memoised query is read, and a variant the shell cannot name (the connection kind, an endpoint, a clamp key, a CP
//! side) is inspected through its record's JSON form.
//!
//! **Commit.** The parameters go through the tool's commit path (`execute_with` in the window, `run_script`
//! headless): one bridge is one `connection.draw`, a chain of two or more one `connection.draw_many`; either is one
//! undo step.

use ew_api::AnyCommand;
use ew_api::output::Uuid;
use serde_json::{Value, json};

use super::interruption::natural_cmp;
use super::{Outcome, ShellModel};
use crate::strings;

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

/// A bridge connection as the group rule reads it: its ID, its group, its two CP ends (a junction end is none) and the
/// strip the `a` end belongs to, when that end is a terminal CP.
struct Bridge {
    id: String,
    group: u16,
    a: Option<CpKey>,
    b: Option<CpKey>,
    strip: Option<String>,
}

impl Bridge {
    /// The function of each end (D10: a bridge joins terminals).
    fn ends(&self) -> (Option<&str>, Option<&str>) {
        (
            self.a.as_ref().map(|(f, _)| f.as_str()),
            self.b.as_ref().map(|(f, _)| f.as_str()),
        )
    }

    /// Whether it joins the terminals of functions `x` and `y` (either order).
    fn joins(&self, x: &str, y: &str) -> bool {
        matches!(self.ends(), (Some(a), Some(b)) if (a == x && b == y) || (a == y && b == x))
    }
}

impl ShellModel {
    /// Whether `strip.bridge` can act on the selection: `Ok(the command)` or `Err(reason key)`.
    ///
    /// `pub(crate)` because the action's enabled predicate in `actions.rs` calls it: the Draw menu evaluates it. Its
    /// cost is linear in the project's connections plus the selected terminals' keyed lookups.
    pub(crate) fn bridge_command(&self) -> Result<AnyCommand, &'static str> {
        let selection: &'static str = "reason-bridge-selection";
        // 1. The selection's terminals (D9): each item a representation with one function, a function or a device,
        // resolving to a terminal device's function; one terminal counts once.
        let project = self.project.as_ref().ok_or(selection)?;
        let q = project.session.query();
        let found = self.selected_terminals().ok_or(selection)?;
        let mut terminals: Vec<Terminal> = Vec::with_capacity(found.len());
        let mut devices = Vec::with_capacity(found.len());
        for (function, device) in found {
            let (Ok(function_id), Ok(device_id)) = (function.to_string().parse(), device.to_string().parse())
            else {
                return Err(selection);
            };
            let Some(device) = q.devices().get(device_id) else {
                return Err(selection);
            };
            let Some(terminal) = device.aspects.terminal.as_ref() else {
                return Err(selection);
            };
            let def = q.functions().get(function_id).map(|f| f.def).ok_or(selection)?;
            terminals.push(Terminal {
                function: function.to_string(),
                seq: terminal.seq,
                designation: terminal.designation.as_str().to_owned(),
                strip: terminal.strip.as_uuid().to_string(),
                end: 0,
            });
            devices.push((device_id, def));
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

        let bridges = self.project_bridges();

        // The terminals in the strip's order: lower sequence first, then the natural order of the designation,
        // then the function ID. Consecutive terminals of that order are the chain's pairs.
        terminals.sort_by(|x, y| {
            x.seq
                .cmp(&y.seq)
                .then_with(|| natural_cmp(&x.designation, &y.designation))
                .then_with(|| x.function.cmp(&y.function))
        });
        let ends: Vec<CpKey> = terminals.iter().map(|t| (t.function.clone(), t.end)).collect();

        // 4. A pair of terminals already joined by a bridge (any CP of one to any CP of the other, D10) is kept; when
        // every pair is, the command is refused.
        let pairs: Vec<(&CpKey, &CpKey)> = ends
            .windows(2)
            .map(|w| (&w[0], &w[1]))
            .filter(|(a, b)| !bridges.iter().any(|bridge| bridge.joins(&a.0, &b.0)))
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
        // D10: a bridge touches the chain when it ends on one of the chain's terminals (any CP of it).
        let functions: Vec<&str> = ends.iter().map(|(f, _)| f.as_str()).collect();
        let touches = |bridge: &Bridge| {
            let (a, b) = bridge.ends();
            a.is_some_and(|f| functions.contains(&f)) || b.is_some_and(|f| functions.contains(&f))
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

impl ShellModel {
    /// The terminals of the window selection (D9): (terminal function, terminal device) for each item — a
    /// representation with one function, a function, or a device (its function) — each terminal once, in selection
    /// order; `None` when an item is none of these or not a terminal.
    pub(super) fn selected_terminals(&self) -> Option<Vec<(Uuid, Uuid)>> {
        let project = self.project.as_ref()?;
        let q = project.session.query();
        let mut out: Vec<(Uuid, Uuid)> = Vec::new();
        for item in &self.tools.selection.items {
            let text = item.0.to_string();
            let function = if let Some(rep) = text.parse().ok().and_then(|id| q.reps().get(id)) {
                let [function] = rep.functions.as_slice() else {
                    return None;
                };
                *function
            } else if let Some(id) = text.parse().ok().filter(|id| q.functions().get(*id).is_some()) {
                id
            } else {
                // A device: its function (a terminal has one; the lowest ID when it has several).
                let device = text.parse().ok().filter(|id| q.devices().get(*id).is_some())?;
                q.functions()
                    .iter()
                    .filter(|(_, f)| f.device == Some(device))
                    .map(|(id, _)| id)
                    .min()?
            };
            let f = q.functions().get(function)?;
            let device = f.device?;
            q.devices().get(device)?.aspects.terminal.as_ref()?;
            let pair = (function.as_uuid(), device.as_uuid());
            if !out.iter().any(|(x, _)| *x == pair.0) {
                out.push(pair);
            }
        }
        Some(out)
    }

    /// `strip.unbridge` (Remove bridge, D11): the bridge connections with both ends on the selected terminals, in one
    /// `edit.delete`; refused when none joins them (`reason-unbridge-none`) or when the deletion would leave a group in
    /// two pieces (`reason-unbridge-split`).
    pub(crate) fn unbridge_command(&self) -> Result<AnyCommand, &'static str> {
        let terminals = self.selected_terminals().ok_or("reason-bridge-selection")?;
        let functions: Vec<String> = terminals.iter().map(|(f, _)| f.to_string()).collect();
        let bridges = self.project_bridges();
        let on = |b: &Bridge| matches!(b.ends(), (Some(a), Some(z)) if functions.iter().any(|f| f == a) && functions.iter().any(|f| f == z));
        let removed: Vec<&Bridge> = bridges.iter().filter(|b| on(b)).collect();
        if removed.is_empty() {
            return Err("reason-unbridge-none");
        }
        // Each touched group: what stays must be one piece (connected over its terminals).
        let groups: std::collections::BTreeSet<(Option<&str>, u16)> =
            removed.iter().map(|b| (b.strip.as_deref(), b.group)).collect();
        for (strip, group) in groups {
            let kept: Vec<(&str, &str)> = bridges
                .iter()
                .filter(|b| b.strip.as_deref() == strip && b.group == group && !on(b))
                .filter_map(|b| match b.ends() {
                    (Some(a), Some(z)) => Some((a, z)),
                    _ => None,
                })
                .collect();
            if pieces(&kept) > 1 {
                return Err("reason-unbridge-split");
            }
        }
        let targets: Vec<Value> = removed
            .iter()
            .map(|b| json!({"kind": "connection", "id": b.id}))
            .collect();
        Ok(AnyCommand {
            name: "edit.delete".into(),
            params: json!({ "targets": targets }),
        })
    }

    /// `strip.unbridge` (the strip editor's row menu, D11): one `edit.delete`, the toast.
    pub(super) fn act_strip_unbridge(&mut self) -> Outcome {
        match self.unbridge_command() {
            Ok(cmd) => {
                let n = cmd.params["targets"].as_array().map_or(0, Vec::len);
                let outcome = self.commit_from_host(&cmd);
                if outcome == Outcome::Done {
                    let text = if n == 1 {
                        strings::format("toast-unbridged-one", &["1"])
                    } else {
                        strings::format("toast-unbridged", &[&n.to_string()])
                    };
                    self.notify(text, true);
                }
                outcome
            }
            Err(reason_key) => Outcome::Refused { reason_key },
        }
    }

    /// Every bridge connection of the project with its ID, group, two CP ends and the strip of its `a` end (the
    /// strip's bridges are those whose `a` end is a CP of one of its terminal functions). Only the `a` end's function,
    /// device and terminal aspect are read, through the keyed tables, so the menu's enabled predicate stays linear in
    /// the bridges.
    fn project_bridges(&self) -> Vec<Bridge> {
        let Some(project) = self.project.as_ref() else {
            return Vec::new();
        };
        let q = project.session.query();
        let mut bridges: Vec<Bridge> = Vec::new();
        for (id, connection) in q.connections().iter() {
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
            bridges.push(Bridge {
                id: id.as_uuid().to_string(),
                group,
                a,
                b,
                strip,
            });
        }
        bridges
    }
}

/// The number of connected pieces of the terminals joined by `links` (0 without links).
fn pieces(links: &[(&str, &str)]) -> usize {
    let mut nodes: Vec<&str> = links.iter().flat_map(|(a, b)| [*a, *b]).collect();
    nodes.sort_unstable();
    nodes.dedup();
    let mut parent: Vec<usize> = (0..nodes.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (a, b) in links {
        let (Ok(x), Ok(y)) = (nodes.binary_search(a), nodes.binary_search(b)) else {
            continue;
        };
        let (rx, ry) = (root(&mut parent, x), root(&mut parent, y));
        if rx != ry {
            parent[rx] = ry;
        }
    }
    (0..nodes.len())
        .filter(|i| root(&mut parent.clone(), *i) == *i)
        .count()
}
