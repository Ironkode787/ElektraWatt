// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigator row operations of WP-S2.07 part 2 (§7): `cable.swap_cores` on the Cables navigator, and FT-490's
//! generic half, `navigator.delete` (Delete in a tree) and `navigator.rename` (F2 in a tree).
//!
//! - **Swap cores** (FR-CBL-016): exactly two marked core rows of one cable; one `fields.set_many` through
//!   `Session::swap_cores` that moves the core assignment of their connections, committed directly with the toast.
//! - **Delete** (FT-490): one `edit.delete` of the acted rows' own records (the marked rows, else the cursor row),
//!   always previewed on the host card, which lists what the cascade removes. A row stands for its own record: a page,
//!   a device (strip, terminal, cable, station, module), a function, a representation, a connection, a conductor (a
//!   core row's second record), a defined potential, a structure node or a report block; a group row, an accessory,
//!   a channel, an undefined potential and the other rows refuse `reason-nav-not-deletable`.
//! - **Rename** (FT-490): the authored label of a Pages row (the page title) or a Potentials row (a defined
//!   potential's name), through the row's editable route (`fields.set`, the text parsed by `parse_field_input`). Without
//!   `text` it opens the row's inline editor on the current label; with `text` it commits; `cancel` closes the editor.
//!   Other rows refuse `reason-nav-not-renamable`.
//!
//! In tree mode the navigator's Delete and F2 (bound to `table.clear` and `table.edit`, which act in table mode) run
//! these actions ([`ShellModel::shortcut_action`]).

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::output::{InputError, NavKind, NavNode, NavRole, NavTable};
use serde_json::{Map, Value, json};

use super::{Mode, Outcome, ShellModel, failed, flag, string_param};
use crate::navigators::table::row_targets;
use crate::strings;

/// A tree row whose label is being renamed (FT-490).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavRename {
    /// The navigator.
    pub kind: NavKind,
    /// The row's key.
    pub key: String,
    /// The label's text when the editor opened.
    pub text: String,
}

/// The rename route of navigator `kind` (FT-490): (record kind, field path).
fn rename_route(kind: NavKind) -> Option<(&'static str, &'static str)> {
    match kind {
        NavKind::Pages => Some(("page", "title")),
        NavKind::Potentials => Some(("potential_def", "name")),
        _ => None,
    }
}

/// The record a row stands for when it is deleted, as `edit.delete`'s target; `None` for a row that stands for none.
fn delete_target(node: &NavNode) -> Option<Value> {
    let (kind, record) = match node.role {
        NavRole::Page => ("page", node.records.first()),
        NavRole::Device
        | NavRole::Strip
        | NavRole::Terminal
        | NavRole::Cable
        | NavRole::Station
        | NavRole::Module => ("device", node.records.first()),
        NavRole::Function => ("function", node.records.first()),
        NavRole::Representation => ("representation", node.records.first()),
        NavRole::Connection => ("connection", node.records.first()),
        NavRole::Conductor => ("conductor", node.records.get(1)),
        NavRole::Potential => {
            let defined = node.kinds.first().is_some_and(|k| k == "potential_def");
            ("potential_def", node.records.first().filter(|_| defined))
        }
        NavRole::StructureNode => ("structure_node", node.records.first()),
        NavRole::ReportBlock => ("report_block", node.records.first()),
        _ => return None,
    };
    record.map(|r| json!({"kind": kind, "id": r.to_string()}))
}

impl ShellModel {
    /// The tree row being renamed, if any (FT-490).
    pub fn nav_rename(&self) -> Option<&NavRename> {
        self.rename.as_ref()
    }

    /// The acted rows of the generic navigator `kind`: the marked rows, else the cursor row; in node order.
    fn acted_rows(&self, kind: NavKind) -> Result<(Arc<NavTable>, Vec<usize>), Outcome> {
        let nav = self
            .generic(kind)
            .ok_or_else(|| failed("error-nav-closed", &[kind.word()]))?;
        let table = Arc::clone(nav.table().ok_or(Outcome::Refused {
            reason_key: "reason-no-nav-row",
        })?);
        let mut rows: Vec<usize> = nav
            .marked()
            .iter()
            .filter_map(|k| nav.node_of(k).map(|(i, _)| i as usize))
            .collect();
        if rows.is_empty()
            && let Some((i, _)) = nav.cursor_key().and_then(|k| nav.node_of(&k))
        {
            rows.push(i as usize);
        }
        rows.sort_unstable();
        if rows.is_empty() {
            return Err(Outcome::Refused {
                reason_key: "reason-no-nav-row",
            });
        }
        Ok((table, rows))
    }

    /// The context of `cable.swap_cores`: the Cables navigator is open and focused (headless: open).
    pub(crate) fn cables_shown(&self) -> Result<(), &'static str> {
        if !self.navigator_open(NavKind::Cables)
            || (self.mode != Mode::Headless && self.focused_navigator() != Some(NavKind::Cables))
        {
            return Err("reason-not-cables");
        }
        Ok(())
    }

    /// `cable.swap_cores`: the two marked core rows of one cable swap their connections (§7, FR-CBL-016).
    pub(super) fn act_cable_swap_cores(&mut self) -> Outcome {
        if let Err(reason_key) = self.cables_shown() {
            return Outcome::Refused { reason_key };
        }
        let Some(nav) = self.generic(NavKind::Cables) else {
            return failed("error-nav-closed", &[NavKind::Cables.word()]);
        };
        let Some(table) = nav.table().map(Arc::clone) else {
            return Outcome::Refused {
                reason_key: "reason-swap-two-cores",
            };
        };
        let rows: Vec<&NavNode> = nav
            .marked()
            .iter()
            .filter_map(|k| nav.node_of(k).map(|(i, _)| &table.nodes[i as usize]))
            .collect();
        let [a, b] = rows.as_slice() else {
            return Outcome::Refused {
                reason_key: "reason-swap-two-cores",
            };
        };
        let (Some(ca), Some(cb)) = (a.records.get(1), b.records.get(1)) else {
            return Outcome::Refused {
                reason_key: "reason-swap-two-cores",
            };
        };
        if a.role != NavRole::Conductor || b.role != NavRole::Conductor {
            return Outcome::Refused {
                reason_key: "reason-swap-two-cores",
            };
        }
        let cable = a
            .parent
            .and_then(|p| table.nodes.get(p as usize))
            .map(|n| n.label.clone())
            .unwrap_or_default();
        let (la, lb) = (a.label.clone(), b.label.clone());
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let cmd = match project.session.swap_cores(*ca, *cb) {
            Ok(cmd) => cmd,
            Err(reason_key) => return Outcome::Refused { reason_key },
        };
        let outcome = self.commit_from_host(&cmd);
        if matches!(outcome, Outcome::Done) {
            self.notify(strings::format("toast-cores-swapped", &[&la, &lb, &cable]), true);
        }
        outcome
    }

    /// `navigator.delete {kind?}` (FT-490): one `edit.delete` of the acted rows' records, on the host card.
    pub(super) fn act_navigator_delete(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        if !self.is_writable() {
            return Outcome::Refused {
                reason_key: "reason-read-only",
            };
        }
        let targets: Vec<Value> = if kind == NavKind::Pages && self.generic(NavKind::Pages).is_none() {
            match self.marked_pages() {
                Ok(rows) => rows
                    .iter()
                    .map(|r| json!({"kind": "page", "id": r.id.to_string()}))
                    .collect(),
                Err(e) => return e,
            }
        } else {
            let (table, rows) = match self.acted_rows(kind) {
                Ok(r) => r,
                Err(e) => return e,
            };
            let mut targets = Vec::new();
            for i in rows {
                match delete_target(&table.nodes[i]) {
                    Some(t) => targets.push(t),
                    None => {
                        return Outcome::Refused {
                            reason_key: "reason-nav-not-deletable",
                        };
                    }
                }
            }
            targets
        };
        self.preview_host_card("edit.delete", json!({ "targets": targets }), "reason-nav-delete")
    }

    /// `navigator.rename {kind?, text?, cancel?}` (FT-490): opens the cursor row's label editor, commits `text` to the
    /// row's route, or (`cancel`) closes the editor.
    pub(super) fn act_navigator_rename(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.open_kind(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        if flag(p, "cancel") {
            self.rename = None;
            return Outcome::Done;
        }
        let text = match string_param(p, "text") {
            Ok(t) => t.map(str::to_owned),
            Err(e) => return e,
        };
        if !self.is_writable() {
            return Outcome::Refused {
                reason_key: "reason-read-only",
            };
        }
        let Some((route_kind, path)) = rename_route(kind) else {
            return Outcome::Refused {
                reason_key: "reason-nav-not-renamable",
            };
        };
        // The row: the open editor's, else the cursor row; its targets and current label.
        let key = match self.rename.as_ref().filter(|r| r.kind == kind) {
            Some(r) => Some(r.key.clone()),
            None if kind == NavKind::Pages && self.generic(NavKind::Pages).is_none() => {
                self.navigator_row_key()
            }
            None => self
                .generic(kind)
                .and_then(crate::navigators::Navigator::cursor_key),
        };
        let Some(key) = key else {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        };
        let (targets, label) = if kind == NavKind::Pages && self.generic(NavKind::Pages).is_none() {
            let rows = self.page_rows();
            match crate::navigator::PageNavigator::page_of(&key, rows) {
                Some(i) if !rows[i].generated => (vec![rows[i].id], rows[i].title.clone()),
                _ => {
                    return Outcome::Refused {
                        reason_key: "reason-nav-not-renamable",
                    };
                }
            }
        } else {
            let node = self
                .generic(kind)
                .and_then(|n| n.node_of(&key).map(|(_, node)| node.clone()));
            let Some(node) = node else {
                return Outcome::Refused {
                    reason_key: "reason-no-nav-row",
                };
            };
            let targets = row_targets(&node, route_kind);
            let label = if kind == NavKind::Pages {
                // The Pages table's title column.
                let t = self.generic(kind).and_then(|n| n.table().map(Arc::clone));
                t.and_then(|t| {
                    let c = t.columns.iter().position(|c| c.key == "title")?;
                    node.cells.get(c).cloned()
                })
                .unwrap_or_default()
            } else {
                node.label.clone()
            };
            (targets, label)
        };
        if targets.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-nav-not-renamable",
            };
        }
        let Some(text) = text else {
            self.rename = Some(NavRename {
                kind,
                key,
                text: label,
            });
            return Outcome::Done;
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let value = match project.session.parse_field_input(route_kind, path, &text) {
            Ok(parsed) => parsed.value,
            Err(InputError::Invalid(message)) => return Outcome::Failed { message },
            Err(InputError::UnknownPart(typed)) => return failed("error-no-part", &[&typed]),
            Err(InputError::Ambiguous { choices }) => {
                let words: Vec<&str> = choices.iter().map(|(t, _)| t.as_str()).collect();
                return Outcome::Failed {
                    message: words.join(", "),
                };
            }
        };
        let targets: Vec<String> = targets.iter().map(ToString::to_string).collect();
        let cmd = AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": route_kind, "targets": targets, "path": path, "value": value}),
        };
        let outcome = self.commit_from_host(&cmd);
        if matches!(outcome, Outcome::Done) {
            self.rename = None;
        }
        outcome
    }
}
