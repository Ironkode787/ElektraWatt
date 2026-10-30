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
//!   FT-633 adds a structure node's code (the same `fields.set` route the Properties grid edits it by) and a device's
//!   tag, whose typed text goes through the Properties Tag field's own path (the Join proposal, the Letter or Counter
//!   row with FR-NUM-029's refusal and proposal, else the frozen tag), its refusal shown under the inline field.
//!   Other rows refuse `reason-nav-not-renamable`.
//!
//! In tree mode the navigator's Delete and F2 (bound to `table.clear` and `table.edit`, which act in table mode) run
//! these actions ([`ShellModel::shortcut_action`]).

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::output::{InputError, NavKind, NavNode, NavRole, NavTable, Uuid};
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
    /// The label's text when the editor opened (after a refused tag, the text as typed).
    pub text: String,
    /// FT-633: the row is a device whose tag goes through the Properties Tag field's path; its refusal and proposal
    /// are [`ShellModel::nav_rename_error`].
    pub tag: bool,
}

/// What F2 renames on a row (FT-490, FT-633).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RenameRoute {
    /// A field through `fields.set`: (record kind, field path).
    Field(&'static str, &'static str),
    /// A device's tag, through the Properties Tag field's path.
    Tag(Uuid),
}

/// The rename route of a row of navigator `kind`: a page's title or a potential definition's name (FT-490), a
/// structure node's code (FT-633(a), the route the Properties grid edits it by) or a device's tag (FT-633(b)).
fn rename_route(kind: NavKind, node: Option<&NavNode>) -> Option<RenameRoute> {
    match (kind, node.map(|n| n.role)) {
        (NavKind::Pages, _) => Some(RenameRoute::Field("page", "title")),
        (NavKind::Potentials, _) => Some(RenameRoute::Field("potential_def", "name")),
        (_, Some(NavRole::StructureNode)) => Some(RenameRoute::Field("structure_node", "code")),
        (_, Some(NavRole::Device | NavRole::Strip | NavRole::Cable | NavRole::Station | NavRole::Module)) => {
            node.and_then(|n| n.records.first()).map(|d| RenameRoute::Tag(*d))
        }
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
        let legacy_pages = kind == NavKind::Pages && self.generic(NavKind::Pages).is_none();
        // The row: the open editor's, else the cursor row; its route, targets and current label.
        let key = match self.rename.as_ref().filter(|r| r.kind == kind) {
            Some(r) => Some(r.key.clone()),
            None if legacy_pages => self.navigator_row_key(),
            None => self
                .generic(kind)
                .and_then(crate::navigators::Navigator::cursor_key),
        };
        let node = key.as_ref().and_then(|key| {
            self.generic(kind)
                .and_then(|n| n.node_of(key).map(|(_, node)| node.clone()))
        });
        let Some(route) = rename_route(kind, node.as_ref()) else {
            return Outcome::Refused {
                reason_key: if key.is_some() {
                    "reason-nav-not-renamable"
                } else {
                    "reason-no-nav-row"
                },
            };
        };
        let Some(key) = key else {
            return Outcome::Refused {
                reason_key: "reason-no-nav-row",
            };
        };
        // FT-633(b): a device's tag goes through the Properties Tag field's own path.
        let (route_kind, path) = match route {
            RenameRoute::Field(k, p) => (k, p),
            RenameRoute::Tag(device) => return self.rename_tag(kind, key, device, text),
        };
        let (targets, label) = if legacy_pages {
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
            let Some(node) = node else {
                return Outcome::Refused {
                    reason_key: "reason-no-nav-row",
                };
            };
            let targets = row_targets(&node, route_kind);
            let label = match (kind, route_kind) {
                // The Pages table's title column.
                (NavKind::Pages, _) => {
                    let t = self.generic(kind).and_then(|n| n.table().map(Arc::clone));
                    t.and_then(|t| {
                        let c = t.columns.iter().position(|c| c.key == "title")?;
                        node.cells.get(c).cloned()
                    })
                    .unwrap_or_default()
                }
                // FT-633(a): the code as the Properties grid shows it (the row's label is the designation).
                (_, "structure_node") => self
                    .field_text(&targets, "structure_node/code")
                    .unwrap_or_else(|| node.label.clone()),
                _ => node.label.clone(),
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
                tag: false,
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

    /// The display text of the sheet row `row` of `records` (one `Session::property_sheet` call).
    fn field_text(&self, records: &[Uuid], row: &str) -> Option<String> {
        let sheet = self.project.as_ref()?.session.property_sheet(records);
        match &crate::properties::find_row(&sheet, row)?.1.value {
            ew_api::output::RowValue::One { text, .. } => Some(text.clone()),
            _ => None,
        }
    }

    /// FT-633(b): F2 on a device row. Without `text` it opens the inline field on the whole tag (`-K12`, the
    /// Properties grid showing the device); with `text` the typed tag goes through the Properties Tag field's own
    /// path (`ShellModel::tag_rename_commit`): committed, the field closes; refused, it stays open on the typed text
    /// with the grid's refusal and proposal under it ([`ShellModel::nav_rename_error`]).
    fn rename_tag(&mut self, kind: NavKind, key: String, device: Uuid, text: Option<String>) -> Outcome {
        let Some(text) = text else {
            let Some(label) = self.tag_rename_sheet(device) else {
                return Outcome::Refused {
                    reason_key: "reason-nav-not-renamable",
                };
            };
            self.rename = Some(NavRename {
                kind,
                key,
                text: label,
                tag: true,
            });
            return Outcome::Done;
        };
        let outcome = self.tag_rename_commit(device, &text);
        self.rename = match &outcome {
            Outcome::Done => None,
            _ => Some(NavRename {
                kind,
                key,
                text,
                tag: true,
            }),
        };
        outcome
    }
}
