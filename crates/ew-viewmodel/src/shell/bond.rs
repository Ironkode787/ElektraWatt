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
//!
//! Beside it, Draw ▸ Define potential… (`potential.draw`, FT-1202, ID-1270): `potential.define` at the selection's
//! one point, with the name, type and voltage of its parameter form (WP-S2.08 D5 (a)).

use ew_api::AnyCommand;
use ew_api::output::table_edit::potential_input;
use ew_api::output::{InputError, PageTables, TableEnd};
use serde_json::{Map, Value, json};

use super::{Outcome, PaletteState, ShellModel, failed, string_param};
use crate::key;
use crate::strings;
use crate::strings::Key;
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
        [] => Err(failed(key!("error-bond-end"), &[label])),
        _ => Err(failed(key!("error-bond-end-ambiguous"), &[label])),
    }
}

impl ShellModel {
    /// `bond.draw` (Draw ▸ Bond): previews `bond.create` on the selected connection or on the CPs `a` and `b`, with
    /// `kind` (default `pelv_0v_pe`), and shows its card.
    pub(crate) fn act_bond_draw(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match string_param(p, "kind") {
            Err(e) => return e,
            Ok(None) => BOND_KINDS[0],
            Ok(Some(k)) => match BOND_KINDS.iter().find(|x| **x == k) {
                Some(k) => k,
                None => return failed(key!("error-bad-param"), &["kind"]),
            },
        };
        let (a, b) = match (string_param(p, "a"), string_param(p, "b")) {
            (Err(e), _) | (_, Err(e)) => return e,
            (Ok(a), Ok(b)) => (a.map(str::to_owned), b.map(str::to_owned)),
        };
        let Some(tables) = self.canvas().map(|c| std::sync::Arc::clone(&c.page().tables)) else {
            return Outcome::Refused {
                reason_key: key!("reason-no-page"),
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
                    return failed(key!("error-bond-same-end"), &[&a]);
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
                        reason_key: key!("reason-bond-selection"),
                    };
                };
                let Some(c) = tables.connections.iter().find(|c| c.id == e.0) else {
                    return Outcome::Refused {
                        reason_key: key!("reason-bond-selection"),
                    };
                };
                (c.a, c.b, Some(c.id))
            }
            (Some(_), None) => return failed(key!("error-missing-param"), &["b"]),
            (None, Some(_)) => return failed(key!("error-missing-param"), &["a"]),
        };
        let mut params = json!({"a": end_param(end_a), "b": end_param(end_b), "kind": kind});
        if let Some(via) = via {
            params["via"] = Value::String(via.to_string());
        }
        self.preview_host_card("bond.create", params, key!("reason-bond"))
    }

    /// The point of the selection a potential is defined at (FT-1202, ID-1270): a selected junction; a selected wire's
    /// `from` end (a connection point or a junction, on the wire's potential as its other end is); a terminal's lowest
    /// connection point, or the one connection point of a symbol that has one. Anything else is refused in words.
    pub(crate) fn potential_point(&self) -> Result<TableEnd, Key<'static>> {
        // FT-1239: while the form a Potentials row opened is open, the row's point.
        if let Some(at) = self.potential_at.filter(|_| self.potential_form_open()) {
            return Ok(at);
        }
        let reason = key!("reason-potential-point");
        let Some(c) = self.canvas() else {
            return Err(reason);
        };
        let tables = &c.page().tables;
        let kinds = selection_kinds(&self.tools.selection, tables);
        let [(e, kind)] = kinds.as_slice() else {
            return Err(reason);
        };
        match kind {
            EntityKind::Junction => Ok(TableEnd::Junction { id: e.0 }),
            EntityKind::Connection => tables
                .connections
                .iter()
                .find(|c| c.id == e.0)
                .map(|c| c.a)
                .ok_or(reason),
            EntityKind::Rep => {
                let rep = tables.reps.iter().find(|r| r.id == e.0).ok_or(reason)?;
                let mut cps: Vec<(u16, TableEnd)> = tables
                    .cps
                    .iter()
                    .filter(|c| rep.functions.contains(&c.function))
                    .map(|c| {
                        let end = TableEnd::Cp {
                            function: c.function,
                            idx: c.idx,
                        };
                        (c.idx, end)
                    })
                    .collect();
                cps.sort_by_key(|(idx, _)| *idx);
                match cps.as_slice() {
                    [(_, one)] => Ok(*one),
                    [(_, lowest), ..] if rep.terminal => Ok(*lowest),
                    _ => Err(reason),
                }
            }
            _ => Err(reason),
        }
    }

    /// `potential.draw {name?, type?, voltage?}` (Draw ▸ Define potential…): without `name` it opens its parameter form
    /// (the name required, the type and the voltage as printed: `L+`, `24 V DC`); with it, commits `potential.define`
    /// at [`ShellModel::potential_point`] in one undo step. A text the field does not take fails with the field's
    /// words, as the form labels it.
    pub(crate) fn act_potential_draw(&mut self, p: &Map<String, Value>) -> Outcome {
        let texts = match (
            string_param(p, "name"),
            string_param(p, "type"),
            string_param(p, "voltage"),
        ) {
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
            (Ok(name), Ok(ptype), Ok(voltage)) => [
                ("name", "name", name),
                ("type", "ptype", ptype),
                ("voltage", "voltage", voltage),
            ],
        };
        if texts[0].2.is_none() {
            return self.open_potential_form();
        }
        let at = match self.potential_point() {
            Ok(at) => at,
            Err(reason_key) => return Outcome::Refused { reason_key },
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let mut params = json!({ "at": end_param(at) });
        for (param, path, text) in texts {
            let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else {
                if path == "name" {
                    return Outcome::Refused {
                        reason_key: key!("table-define-name-first"),
                    };
                }
                continue;
            };
            let field = crate::palette::PaletteField {
                name: param.to_owned(),
                ty: "text".to_owned(),
                optional: true,
                value: String::new(),
            }
            .label();
            let (input, extra) = potential_input("potential_def", path, text);
            params[path] = match project.session.parse_field_input("potential_def", path, &input) {
                Ok(parsed) => parsed.value,
                // The form takes the printed forms: its refusal names them, not the stored words.
                Err(InputError::Invalid(_) | InputError::NotOneOf { .. }) if path == "ptype" => {
                    return failed(key!("error-potential-type"), &[text]);
                }
                Err(InputError::Invalid(_)) if path == "voltage" => {
                    return failed(key!("error-potential-voltage"), &[text]);
                }
                Err(InputError::Invalid(why)) => {
                    let title = strings::text(key!("command-potential-define"));
                    return Outcome::Failed {
                        message: strings::format(key!("error-command-field"), &[title, &field, &why]),
                    };
                }
                Err(InputError::NotOneOf { typed, options }) => {
                    let why = crate::properties::not_one_of_text("potential_def", path, &typed, &options);
                    let title = strings::text(key!("command-potential-define"));
                    return Outcome::Failed {
                        message: strings::format(key!("error-command-field"), &[title, &field, &why]),
                    };
                }
                Err(InputError::UnknownPart(_) | InputError::Ambiguous { .. }) => {
                    return failed(key!("error-bad-param"), &[&field]);
                }
            };
            if let Some((_, ac_dc)) = extra {
                params["env"] = json!({ "ac_dc": ac_dc });
            }
        }
        let cmd = AnyCommand {
            name: "potential.define".into(),
            params,
        };
        self.commit_from_host(&cmd)
    }

    /// The parameter form of `potential.draw` in the palette (WP-S2.08 D5 (a)), its name required.
    fn open_potential_form(&mut self) -> Outcome {
        self.potential_at = None;
        let Some(meta) = crate::actions::find("potential.draw") else {
            return Outcome::Done;
        };
        let Ok(specs) = crate::actions::parse_signature(meta.params) else {
            return Outcome::Done;
        };
        let mut form = crate::palette::PaletteForm::new(meta.id, &specs);
        if let Some(name) = form.fields.first_mut() {
            name.optional = false;
        }
        self.dialog = None;
        self.palette = Some(PaletteState {
            query: String::new(),
            selected: 0,
            form: Some(form),
        });
        Outcome::Done
    }

    /// Whether Define potential…'s form is open with a Potentials row's point pinned (FT-1239).
    pub(crate) fn potential_pinned(&self) -> bool {
        self.potential_at.is_some() && self.potential_form_open()
    }

    /// Whether the palette holds Define potential…'s form.
    fn potential_form_open(&self) -> bool {
        self.palette
            .as_ref()
            .and_then(|p| p.form.as_ref())
            .is_some_and(|f| f.action == "potential.draw")
    }

    /// `potential.draw_row {row}` (a Potentials row's context menu, FT-1239): Define potential…'s form for the row's
    /// potential at its definition point (`NavNode::define_at`, the row's lowest member connection point), whatever
    /// the selection or the page on show; a row without one is refused in words.
    pub(crate) fn act_potential_draw_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let row = match string_param(p, "row") {
            Err(e) => return e,
            Ok(Some(row)) => row.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["row"]),
        };
        let at = self
            .nav(crate::navigators::NavKind::Potentials)
            .and_then(|n| n.table())
            .and_then(|t| t.nodes.iter().find(|n| n.key == row))
            .and_then(|n| n.define_at);
        let Some((function, idx)) = at else {
            return Outcome::Refused {
                reason_key: key!("reason-potential-row"),
            };
        };
        let out = self.open_potential_form();
        self.potential_at = Some(TableEnd::Cp { function, idx });
        out
    }
}
