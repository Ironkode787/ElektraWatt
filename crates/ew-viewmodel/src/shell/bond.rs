// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Draw ▸ Bond (`bond.draw`, FT row 7 of the G-PP triage): the host action that declares a bond with `bond.create`
//! (FT-164's command; doc 03 §9.6, doc 08 §3.7) on the page shown, previewed on a card and committed by Apply in one
//! undo step.
//!
//! **What it bonds.** Either the one selected connection, whose two ends become the bond's ends and which becomes
//! its `via` (the bond cut: the wire no longer merges the two potentials, so 0 V keeps its role and colour and PE
//! stays PE), or the two connection points `a` and `b` named as the page text prints them (`-102G1:−#1`); then the
//! one drawn connection of the page that runs exactly between them, if there is one, is the `via`. The kind is
//! `pelv_0v_pe` unless `kind` names another of the command's kinds.
//!
//! **Reads.** The focused page's tables only; no memoised query.
//!
//! **Commit.** The card is the host card of the renumber dialogs (`CardOrigin::Bulk`, reason `reason-bond`): Apply
//! commits the kept preview (`ShellModel::commit_kept`; headless one command-log line).

use ew_api::output::{PageTables, TableEnd};
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, string_param};
use crate::tools::{EntityKind, selection_kinds};

/// The bond kinds `bond.create` takes, in the order the Bond submenu lists them; the first is the default.
pub const BOND_KINDS: [&str; 4] = ["pelv_0v_pe", "fe_pe", "equipotential", "shield"];

/// The command parameters of an endpoint of the page tables.
fn end_param(e: TableEnd) -> Value {
    match e {
        TableEnd::Cp { function, idx } => json!({"cp": {"function": function.to_string(), "idx": idx}}),
        TableEnd::Junction { id } => json!({"junction": {"junction": id.to_string()}}),
    }
}

/// The connection point of the page that the page text prints as `label` (quotes optional).
fn cp_by_label(t: &PageTables, label: &str) -> Result<TableEnd, Outcome> {
    let label = label.trim().trim_matches('"');
    let hits: Vec<TableEnd> = t
        .cps
        .iter()
        .filter(|c| c.label == label)
        .map(|c| TableEnd::Cp {
            function: c.function,
            idx: c.idx,
        })
        .collect();
    match hits.as_slice() {
        [one] => Ok(*one),
        [] => Err(failed("error-bond-end", &[label])),
        _ => Err(failed("error-bond-end-ambiguous", &[label])),
    }
}

impl ShellModel {
    /// `bond.draw` (Draw ▸ Bond): previews `bond.create` on the selected connection or on the CPs `a` and `b`, with
    /// `kind` (default `pelv_0v_pe`), and shows its card.
    pub(super) fn act_bond_draw(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match string_param(p, "kind") {
            Err(e) => return e,
            Ok(None) => BOND_KINDS[0],
            Ok(Some(k)) => match BOND_KINDS.iter().find(|x| **x == k) {
                Some(k) => k,
                None => return failed("error-bad-param", &["kind"]),
            },
        };
        let (a, b) = match (string_param(p, "a"), string_param(p, "b")) {
            (Err(e), _) | (_, Err(e)) => return e,
            (Ok(a), Ok(b)) => (a.map(str::to_owned), b.map(str::to_owned)),
        };
        let Some(tables) = self.canvas().map(|c| std::sync::Arc::clone(&c.page().tables)) else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let (end_a, end_b, via) = match (a, b) {
            (Some(a), Some(b)) => {
                let end_a = match cp_by_label(&tables, &a) {
                    Ok(e) => e,
                    Err(out) => return out,
                };
                let end_b = match cp_by_label(&tables, &b) {
                    Ok(e) => e,
                    Err(out) => return out,
                };
                if end_a == end_b {
                    return failed("error-bond-same-end", &[&a]);
                }
                let between: Vec<_> = tables
                    .connections
                    .iter()
                    .filter(|c| (c.a == end_a && c.b == end_b) || (c.a == end_b && c.b == end_a))
                    .map(|c| c.id)
                    .collect();
                let via = match between.as_slice() {
                    [one] => Some(*one),
                    _ => None,
                };
                (end_a, end_b, via)
            }
            (None, None) => {
                let kinds = selection_kinds(&self.tools.selection, &tables);
                let [(e, EntityKind::Connection)] = kinds.as_slice() else {
                    return Outcome::Refused {
                        reason_key: "reason-bond-selection",
                    };
                };
                let Some(c) = tables.connections.iter().find(|c| c.id == e.0) else {
                    return Outcome::Refused {
                        reason_key: "reason-bond-selection",
                    };
                };
                (c.a, c.b, Some(c.id))
            }
            (Some(_), None) => return failed("error-missing-param", &["b"]),
            (None, Some(_)) => return failed("error-missing-param", &["a"]),
        };
        let mut params = json!({"a": end_param(end_a), "b": end_param(end_b), "kind": kind});
        if let Some(via) = via {
            params["via"] = Value::String(via.to_string());
        }
        self.preview_host_card("bond.create", params, "reason-bond")
    }
}
