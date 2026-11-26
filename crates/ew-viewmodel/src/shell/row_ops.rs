// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigator row operations: `cable.swap_cores` on the Cables navigator, and the
//! generic half, `navigator.delete` (Delete in a tree) and `navigator.rename` (F2 in a tree).
//!
//! - **Swap cores** (FR-CBL-016): exactly two marked core rows of one cable; one `fields.set_many` through
//!   `Session::swap_cores` that moves the core assignment of their connections, committed directly with the toast.
//! - **Delete**: one `edit.delete` of the acted rows' own records (the marked rows, else the cursor row),
//!   always previewed on the host card, which lists what the cascade removes. A row stands for its own record: a page,
//!   a device (strip, terminal, cable, station, module), a function, a representation, a connection, a conductor (a
//!   core row's second record), a defined potential, a structure node or a report block; a group row, an accessory,
//!   a channel, an undefined potential and the other rows refuse `reason-nav-not-deletable`.
//! - **Rename**: the authored label of a Pages row (the page title) or a Potentials row (a defined
//!   potential's name), through the row's editable route (`fields.set`, the text parsed by `parse_field_input`). Without
//!   `text` it opens the row's inline editor on the current label; with `text` it commits; `cancel` closes the editor.
//!   It also takes a structure node's code (the same `fields.set` route the Properties grid edits it by) and a device's
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
use crate::key;
use crate::navigators::table::row_targets;
use crate::strings;
use crate::strings::Key;

/// A tree row whose label is being renamed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavRename {
    /// The navigator.
    pub kind: NavKind,
    /// The row's key.
    pub key: String,
    /// The label's text when the editor opened (after a refused tag, the text as typed).
    pub text: String,
    /// The row is a device whose tag goes through the Properties Tag field's path; its refusal and proposal
    /// are [`ShellModel::nav_rename_error`].
    pub tag: bool,
}

/// What F2 renames on a row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RenameRoute {
    /// A field through `fields.set`: (record kind, field path).
    Field(&'static str, &'static str),
    /// A device's tag, through the Properties Tag field's path.
    Tag(Uuid),
}

/// The rename route of a row of navigator `kind`: a page's title or a potential definition's name, a
/// structure node's code (the route the Properties grid edits it by) or a device's tag.
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
    /// The tree row being renamed, if any.
    pub fn nav_rename(&self) -> Option<&NavRename> {
        self.rename.as_ref()
    }

    /// The acted rows of the generic navigator `kind`: the marked rows, else the cursor row; in node order.
    fn acted_rows(&self, kind: NavKind) -> Result<(Arc<NavTable>, Vec<usize>), Outcome> {
        let nav = self
            .generic(kind)
            .ok_or_else(|| failed(key!("error-nav-closed"), &[kind.word()]))?;
        let table = Arc::clone(nav.table().ok_or(Outcome::refused(key!("reason-no-nav-row")))?);
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
            return Err(Outcome::refused(key!("reason-no-nav-row")));
        }
        Ok((table, rows))
    }

    /// The devices the Devices navigator's acted rows stand for: a device row's device, every device row under a group
    /// row (a function's or a location's, the "(none)" group too); in node order, each once.
    pub(crate) fn acted_devices(&self) -> Result<Vec<Uuid>, Outcome> {
        let (table, rows) = self.acted_rows(NavKind::Devices)?;
        let mut out: Vec<Uuid> = Vec::new();
        let mut take = |i: usize| {
            let n = &table.nodes[i];
            if n.role == NavRole::Device
                && let Some(d) = n.records.first()
                && !out.contains(d)
            {
                out.push(*d);
            }
        };
        for i in rows {
            match table.nodes[i].role {
                NavRole::Function | NavRole::Location => {
                    // The device rows of the group: its location groups' children, or its own.
                    let groups: Vec<usize> = std::iter::once(i)
                        .chain(
                            (0..table.nodes.len())
                                .filter(|&j| table.nodes[j].parent == u32::try_from(i).ok()),
                        )
                        .collect();
                    for j in 0..table.nodes.len() {
                        if groups
                            .iter()
                            .any(|g| table.nodes[j].parent == u32::try_from(*g).ok())
                        {
                            take(j);
                        }
                    }
                }
                _ => take(i),
            }
        }
        if out.is_empty() {
            return Err(Outcome::refused(key!("reason-no-device-row")));
        }
        Ok(out)
    }

    /// `devices.set_location {location?}` (Location… on Devices rows): the devices of the acted rows
    /// ([`Self::acted_devices`]) get the location (its ID or `+code`), one `fields.set` of `structure.location`, and a
    /// toast says so. In the window without `location` a dialog asks for it.
    pub(crate) fn act_devices_set_location(&mut self, p: &Map<String, Value>) -> Outcome {
        const ACTION: &str = "devices.set_location";
        let devices = match self.acted_devices() {
            Ok(d) => d,
            Err(e) => return e,
        };
        let locations = self
            .project
            .as_ref()
            .map(|p| p.session.layout_locations())
            .unwrap_or_default();
        let given = match string_param(p, "location") {
            Ok(g) => g.map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned),
            Err(e) => return e,
        };
        let Some(given) = given else {
            if self.mode == Mode::Headless {
                return failed(key!("error-missing-param"), &["location"]);
            }
            if locations.is_empty() {
                return Outcome::refused(key!("reason-no-location"));
            }
            let form = super::FormDialog {
                action: ACTION,
                title_key: key!("devices-location-title"),
                ok_key: key!("devices-location-ok"),
                fields: vec![super::FormField {
                    label: None,
                    name: "location".into(),
                    label_key: key!("form-locate-location"),
                    choices: &super::choices::NONE,
                    options: locations
                        .iter()
                        .map(|(id, code)| (id.to_string(), code.clone()))
                        .collect(),
                    value: locations[0].0.to_string(),
                }],
                error: None,
                table: None,
            };
            return self.open_dialog(super::Dialog::Form(form));
        };
        let Some((location, code)) = locations.iter().find(|(id, code)| {
            id.to_string() == given
                || *code == given
                || code.trim_start_matches('+') == given.trim_start_matches('+')
        }) else {
            return self.form_failed(ACTION, strings::format(key!("error-no-location"), &[&given]));
        };
        let targets: Vec<String> = devices.iter().map(ToString::to_string).collect();
        let out = self.commit_from_host(&AnyCommand {
            name: "fields.set".into(),
            params: json!({
                "kind": "device",
                "targets": targets,
                "path": "structure.location",
                "value": location.to_string(),
            }),
        });
        if out == Outcome::Done {
            if self.in_form_of(ACTION) {
                self.dialog = None;
            }
            let text = strings::format_counted(
                crate::count_key!("toast-devices-location"),
                devices.len(),
                &[code],
            );
            self.notify(text, true);
        }
        out
    }

    /// The context of `cable.swap_cores`: the Cables navigator is open and focused (headless: open).
    pub(crate) fn cables_shown(&self) -> Result<(), Key<'static>> {
        if !self.navigator_open(NavKind::Cables)
            || (self.mode != Mode::Headless && self.focused_navigator() != Some(NavKind::Cables))
        {
            return Err(key!("reason-not-cables"));
        }
        Ok(())
    }

    /// `cable.swap_cores`: the two marked core rows of one cable swap their connections (FR-CBL-016).
    pub(crate) fn act_cable_swap_cores(&mut self) -> Outcome {
        if let Err(reason_key) = self.cables_shown() {
            return Outcome::Refused {
                reason: reason_key.into(),
            };
        }
        let Some(nav) = self.generic(NavKind::Cables) else {
            return failed(key!("error-nav-closed"), &[NavKind::Cables.word()]);
        };
        let Some(table) = nav.table().map(Arc::clone) else {
            return Outcome::refused(key!("reason-swap-two-cores"));
        };
        let rows: Vec<&NavNode> = nav
            .marked()
            .iter()
            .filter_map(|k| nav.node_of(k).map(|(i, _)| &table.nodes[i as usize]))
            .collect();
        let [a, b] = rows.as_slice() else {
            return Outcome::refused(key!("reason-swap-two-cores"));
        };
        let (Some(ca), Some(cb)) = (a.records.get(1), b.records.get(1)) else {
            return Outcome::refused(key!("reason-swap-two-cores"));
        };
        if a.role != NavRole::Conductor || b.role != NavRole::Conductor {
            return Outcome::refused(key!("reason-swap-two-cores"));
        }
        let cable = a
            .parent
            .and_then(|p| table.nodes.get(p as usize))
            .map(|n| n.label.clone())
            .unwrap_or_default();
        let (la, lb) = (a.label.clone(), b.label.clone());
        let project = or_return!(crate::shell::gated(&self.project));
        let cmd = match project.session.swap_cores(*ca, *cb) {
            Ok(cmd) => cmd,
            Err(reason_key) => {
                return Outcome::Refused {
                    reason: Key::run_time(reason_key).into(),
                };
            }
        };
        let outcome = self.commit_from_host(&cmd);
        if matches!(outcome, Outcome::Done) {
            self.notify(
                strings::format(key!("toast-cores-swapped"), &[&la, &lb, &cable]),
                true,
            );
        }
        outcome
    }

    /// `navigator.delete {kind?}`: one `edit.delete` of the acted rows' records, on the host card.
    pub(crate) fn act_navigator_delete(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        if !self.is_writable() {
            return Outcome::refused(key!("reason-read-only"));
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
            let (table, rows) = or_return!(self.acted_rows(kind));
            let mut targets = Vec::new();
            for i in rows {
                match delete_target(&table.nodes[i]) {
                    Some(t) => targets.push(t),
                    None => {
                        return Outcome::refused(key!("reason-nav-not-deletable"));
                    }
                }
            }
            targets
        };
        self.preview_host_card(
            "edit.delete",
            json!({ "targets": targets }),
            key!("reason-nav-delete"),
        )
    }

    /// `navigator.rename {kind?, text?, cancel?}`: opens the cursor row's label editor, commits `text` to the
    /// row's route, or (`cancel`) closes the editor.
    pub(crate) fn act_navigator_rename(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        if flag(p, "cancel") {
            self.rename = None;
            return Outcome::Done;
        }
        let text = match string_param(p, "text") {
            Ok(t) => t.map(str::to_owned),
            Err(e) => return e,
        };
        if !self.is_writable() {
            return Outcome::refused(key!("reason-read-only"));
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
            return Outcome::refused(if key.is_some() {
                key!("reason-nav-not-renamable")
            } else {
                key!("reason-no-nav-row")
            });
        };
        let Some(key) = key else {
            return Outcome::refused(key!("reason-no-nav-row"));
        };
        // A device's tag goes through the Properties Tag field's own path.
        let (route_kind, path) = match route {
            RenameRoute::Field(k, p) => (k, p),
            RenameRoute::Tag(device) => return self.rename_tag(kind, key, device, text),
        };
        let (targets, label) = if legacy_pages {
            let rows = self.page_rows();
            match crate::navigator::PageNavigator::page_of(&key, rows) {
                Some(i) if !rows[i].generated => (vec![rows[i].id], rows[i].title.clone()),
                _ => {
                    return Outcome::refused(key!("reason-nav-not-renamable"));
                }
            }
        } else {
            let Some(node) = node else {
                return Outcome::refused(key!("reason-no-nav-row"));
            };
            let targets = row_targets(&node, route_kind, path);
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
                // The code as the Properties grid shows it (the row's label is the designation).
                (_, "structure_node") => self
                    .field_text(&targets, "structure_node/code")
                    .unwrap_or_else(|| node.label.clone()),
                _ => node.label.clone(),
            };
            (targets, label)
        };
        if targets.is_empty() {
            return Outcome::refused(key!("reason-nav-not-renamable"));
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
            Err(InputError::Invalid(why)) => {
                return Outcome::Failed {
                    message: super::tools::rejected_words(&why.to_string(), Some(&why)),
                };
            }
            Err(InputError::UnknownPart(typed)) => return failed(key!("error-no-part"), &[&typed]),
            Err(InputError::NotOneOf { typed, options }) => {
                return Outcome::Failed {
                    message: crate::properties::not_one_of_text(route_kind, path, &typed, &options),
                };
            }
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

    /// F2 on a device row. Without `text` it opens the inline field on the whole tag (`-K12`, the
    /// Properties grid showing the device); with `text` the typed tag goes through the Properties Tag field's own
    /// path (`ShellModel::tag_rename_commit`): committed, the field closes; refused, it stays open on the typed text
    /// with the grid's refusal and proposal under it ([`ShellModel::nav_rename_error`]).
    fn rename_tag(&mut self, kind: NavKind, key: String, device: Uuid, text: Option<String>) -> Outcome {
        let Some(text) = text else {
            let Some(label) = self.tag_rename_sheet(device) else {
                return Outcome::refused(key!("reason-nav-not-renamable"));
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
