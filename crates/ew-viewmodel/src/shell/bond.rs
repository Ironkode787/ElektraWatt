// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Draw ▸ Bond (`bond.draw`): the host action that declares a bond with `bond.create`
//! (doc 03 §9.6, doc 08 §3.7) on the page shown, previewed on a card and committed by Apply in one
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
//! Beside it, Draw ▸ Define potential… (`potential.draw`): `potential.define` at the selection's
//! one point, with the name, type and voltage of its parameter form.

use ew_api::AnyCommand;
use ew_api::output::table_edit::potential_input;
use ew_api::output::{InputError, PageTables, TableCp, TableEnd, TableRep, Uuid};
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

/// The one point a potential is defined at on representation `rep`: over the connection points of every
/// function it shows, its one point, or a terminal's lowest; `None` for any other symbol.
fn rep_point(rep: &TableRep, cps: &[TableCp]) -> Option<TableEnd> {
    let mut cps: Vec<(u16, TableEnd)> = cps
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
        [(_, one)] => Some(*one),
        [(_, lowest), ..] if rep.terminal => Some(*lowest),
        _ => None,
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
            return Outcome::refused(key!("reason-no-page"));
        };
        let (end_a, end_b, via) = match (a, b) {
            (Some(a), Some(b)) => {
                let end_a = or_return!(cp_by_label(&tables, &a));
                let end_b = or_return!(cp_by_label(&tables, &b));
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
                    return Outcome::refused(key!("reason-bond-selection"));
                };
                let Some(c) = tables.connections.iter().find(|c| c.id == e.0) else {
                    return Outcome::refused(key!("reason-bond-selection"));
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

    /// The point of the selection a potential is defined at: a selected junction; a selected wire's
    /// `from` end (a connection point or a junction, on the wire's potential as its other end is); a terminal's lowest
    /// connection point, or the one connection point of a symbol that has one. Anything else is refused in words.
    pub(crate) fn potential_point(&self) -> Result<TableEnd, Key<'static>> {
        // While the form a Potentials row opened is open, the row's point.
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
                rep_point(rep, &tables.cps).ok_or(reason)
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
            return Outcome::refused(key!("reason-no-project"));
        };
        let mut params = json!({ "at": end_param(at) });
        for (param, path, text) in texts {
            let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else {
                if path == "name" {
                    return Outcome::refused(key!("table-define-name-first"));
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
                    let why = super::tools::rejected_words(&why.to_string(), Some(&why));
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

    /// The parameter form of `potential.draw` in the palette, its name required.
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
        // The type is a list, the choices Properties offers for a definition's Type (FT-1698), an empty one first
        // (no type); the window words each as Properties does.
        if let Some(project) = &self.project
            && let Some(ty) = form.fields.iter_mut().find(|f| f.name == "type")
        {
            let words: Vec<String> = project
                .session
                .field_choices("potential_def", "ptype", "")
                .into_iter()
                .filter_map(|c| c.value.as_str().map(str::to_owned))
                .collect();
            if !words.is_empty() {
                ty.ty = format!("enum(|{})", words.join("|"));
            }
        }
        self.dialog = None;
        self.palette = Some(PaletteState {
            query: String::new(),
            selected: 0,
            form: Some(form),
        });
        Outcome::Done
    }

    /// Whether Define potential…'s form is open with a Potentials row's point pinned.
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

    /// `potential.draw_row {row}` (a Potentials row's context menu): Define potential…'s form for the row's
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
            return Outcome::refused(key!("reason-potential-row"));
        };
        let out = self.open_potential_form();
        self.potential_at = Some(TableEnd::Cp { function, idx });
        out
    }

    /// The potential reasoned for the one selected wire with the Select tool (FT-1700, `Session::potential_offer`)
    /// and the wire; `None` for any other selection, a wire whose offer was declined, or nothing to offer. Read once
    /// per revision and wire.
    fn offer_for_selection(&self) -> Option<(Uuid, ew_api::output::PotentialOffer)> {
        if self.tools.tool.name() != "select" {
            return None;
        }
        let c = self.canvas()?;
        let kinds = selection_kinds(&self.tools.selection, &c.page().tables);
        let [(e, EntityKind::Connection)] = kinds.as_slice() else {
            return None;
        };
        if self.declined_offers.contains(&e.0) {
            return None;
        }
        let key = (self.revision()?, e.0);
        if let Some((k, offer)) = &*self.offer_cache.borrow()
            && *k == key
        {
            return offer.clone().map(|o| (e.0, o));
        }
        let offer = self.project.as_ref()?.session.potential_offer(e.0);
        *self.offer_cache.borrow_mut() = Some((key, offer.clone()));
        offer.map(|o| (e.0, o))
    }

    /// The reasoned potential the window offers at the selected wire (FT-1700): its title ("Potential 0VDC?") and why,
    /// in words ("-1K1:A2 returns a coil fed from 24VDC"); `None` when there is none to offer or the project is
    /// read-only.
    pub fn potential_offer(&self) -> Option<(String, String)> {
        if self.read_only_reason().is_some() {
            return None;
        }
        let (_, o) = self.offer_for_selection()?;
        // A potential named by a type's word without the type: the type is offered ("Type L1?").
        if o.type_only {
            let ptype = strings::choice_text(&o.ptype);
            return Some((
                strings::format(key!("potential-offer-title-type"), &[&ptype]),
                strings::format(key!("potential-offer-type"), &[&o.name]),
            ));
        }
        let why = match &o.fed_from {
            Some(feed) => strings::format(key!("potential-offer-coil"), &[&o.cp, feed]),
            None => strings::format(
                key!("potential-offer-role"),
                &[&o.cp, &strings::choice_text(&o.ptype)],
            ),
        };
        let title = if o.new {
            key!("potential-offer-title-new")
        } else {
            key!("potential-offer-title")
        };
        Some((strings::format(title, &[&o.name]), why))
    }

    /// `potential.offer {accept}` (the card's Accept and Decline, FT-1700): Accept defines the offered potential at the
    /// selected wire as Define potential… does, in one undo step; Decline keeps it from being offered again for that
    /// wire in this session. Without an offer it is refused in words.
    pub(crate) fn act_potential_offer(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(accept) = p.get("accept").and_then(Value::as_bool) else {
            return failed(key!("error-missing-param"), &["accept"]);
        };
        let Some((wire, offer)) = self.offer_for_selection() else {
            return Outcome::refused(key!("reason-no-potential-offer"));
        };
        if !accept {
            self.declined_offers.insert(wire);
            return Outcome::Done;
        }
        let mut params = Map::new();
        params.insert("name".into(), Value::String(offer.name));
        params.insert("type".into(), Value::String(offer.ptype));
        self.act_potential_draw(&params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A symbol of the page tables showing `functions`, a terminal's or not.
    fn rep(functions: &[u128], terminal: bool) -> TableRep {
        TableRep {
            id: Uuid::from_u128(100),
            functions: functions.iter().map(|f| Uuid::from_u128(*f)).collect(),
            device: None,
            symbol: Uuid::from_u128(200),
            symbol_key: String::new(),
            orient: 0,
            rep_type: "multi_line".to_owned(),
            pos: (0, 0),
            body: [0; 4],
            tag_at: None,
            ip: None,
            black_box: None,
            terminal,
        }
    }

    /// Connection point `idx` of function `function`, drawn by the symbol of [`rep`].
    fn cp(function: u128, idx: u16) -> TableCp {
        TableCp {
            id: Uuid::from_u128(1000 + function * 10 + u128::from(idx)),
            function: Uuid::from_u128(function),
            idx,
            rep: Uuid::from_u128(100),
            label: String::new(),
            at: (0, 0),
            dir: None,
            connections: Vec::new(),
            potential: None,
            side: None,
            plc_address: None,
        }
    }

    /// A symbol showing two functions of one connection point each has two points,
    /// so Define potential takes none of them; one function's one point is the point, a terminal's lowest likewise.
    #[test]
    fn a_symbol_of_several_functions_counts_every_functions_points() {
        let cps = [cp(1, 0), cp(2, 0), cp(3, 1), cp(3, 0)];
        assert_eq!(rep_point(&rep(&[1, 2], false), &cps), None);
        assert_eq!(
            rep_point(&rep(&[1], false), &cps),
            Some(TableEnd::Cp {
                function: Uuid::from_u128(1),
                idx: 0
            })
        );
        assert_eq!(rep_point(&rep(&[3], false), &cps), None);
        assert_eq!(
            rep_point(&rep(&[3], true), &cps),
            Some(TableEnd::Cp {
                function: Uuid::from_u128(3),
                idx: 0
            })
        );
    }
}
