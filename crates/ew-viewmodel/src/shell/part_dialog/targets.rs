// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the part dialog gives a part to: the terminals of a strip and the cells of a grid, and the dialog's refresh.

use super::{LAYOUT_KIND_WORDS, Target, message_text, parse_items, purposes};
use crate::key;
use crate::navigators::table::{accessory_index, strip_route};
use crate::shell::{Dialog, Outcome, ShellModel, failed};
use crate::strings;
use crate::strings::Key;
use ew_api::output::{NavKind, NavRole, Uuid, accessory_part_kinds};
use ew_api::{AssignMode, PartOffer, PartTarget};
use serde_json::Value;

impl ShellModel {
    /// The strip editor's target of `part.choose` (`bridge_part`, `accessory_part`, or the Part cell of marked
    /// terminal rows for `parts`), from its cursor and marks; `None` when the strip editor does not offer it.
    pub(super) fn strip_part_target(&self, want: Option<&str>) -> Option<Target> {
        self.strip_editor_shown().ok()?;
        let nav = self.generic(NavKind::Strips)?;
        let table = nav.table()?;
        let (ci, cursor) = nav.cursor_key().and_then(|k| nav.node_of(&k))?;
        let column = nav
            .table_mode
            .as_ref()
            .and_then(|m| m.column.clone())
            .and_then(|c| table.columns.iter().position(|x| x.key == c));
        let route = column.and_then(|c| strip_route(table, cursor, c)).map(|r| r.1);
        let strip_node = |i: u32| table.nodes.get(i as usize);
        let want = want.or(match route.as_deref() {
            Some(p) if p.starts_with("bridge_part:") => Some("bridge_part"),
            Some(p) if p.starts_with("accessory_part:") => Some("accessory_part"),
            _ => None,
        })?;
        let mut terminals: Vec<Uuid> = nav
            .marked()
            .iter()
            .filter_map(|k| nav.node_of(k))
            .filter(|(_, n)| n.role == NavRole::Terminal)
            .filter_map(|(_, n)| n.records.first().copied())
            .collect();
        if terminals.is_empty() && cursor.role == NavRole::Terminal {
            terminals.extend(cursor.records.first().copied());
        }
        let strip_label = cursor
            .parent
            .and_then(strip_node)
            .map_or_else(String::new, |s| crate::navigators::strip_label(&s.label));
        let _ = ci;
        match want {
            "bridge_part" => {
                if terminals.is_empty() {
                    return None;
                }
                let bridge = table
                    .columns
                    .iter()
                    .position(|c| c.key == "bridge")
                    .and_then(|i| cursor.cells.get(i).cloned())
                    .unwrap_or_default();
                Some(Target {
                    target: PartTarget::BridgePart { terminals },
                    text: strings::format(
                        key!("part-choose-target-bridge"),
                        &[&format!("{strip_label} {bridge}")],
                    ),
                    tag: None,
                    function: None,
                    kinds: accessory_part_kinds("bridge")
                        .iter()
                        .map(|k| (*k).to_owned())
                        .collect(),
                })
            }
            "accessory_part" => {
                if cursor.role != NavRole::Accessory {
                    return None;
                }
                let index = accessory_index(cursor)?;
                let strip = cursor.parent.and_then(strip_node)?.records.first().copied()?;
                let kind = ew_api::output::ACCESSORY_KINDS
                    .iter()
                    .find(|k| strings::render(&ew_api::output::accessory_kind_word(k)) == cursor.label)
                    .copied()
                    .unwrap_or("partition");
                let pos = table
                    .columns
                    .iter()
                    .position(|c| c.key == "pos")
                    .and_then(|i| cursor.cells.get(i).cloned())
                    .unwrap_or_default();
                Some(Target {
                    target: PartTarget::AccessoryPart { strip, index },
                    text: strings::format(
                        key!("part-choose-target-accessory"),
                        &[&cursor.label.to_lowercase(), &format!("{strip_label} {pos}")],
                    ),
                    tag: None,
                    function: None,
                    kinds: accessory_part_kinds(kind)
                        .iter()
                        .map(|k| (*k).to_owned())
                        .collect(),
                })
            }
            _ => {
                if terminals.is_empty() {
                    return None;
                }
                let function = self
                    .project
                    .as_ref()
                    .and_then(|p| p.session.main_function(terminals[0]))
                    .map(|f| f.0);
                let tag = self.tag_of(terminals[0]);
                Some(Target {
                    text: if terminals.len() == 1 {
                        tag.clone()
                    } else {
                        strings::format(
                            key!("part-choose-target-devices"),
                            &[&terminals.len().to_string()],
                        )
                    },
                    target: PartTarget::Devices {
                        devices: terminals,
                        mode: AssignMode::ReplaceAll,
                    },
                    tag: Some(tag),
                    function,
                    kinds: Vec::new(),
                })
            }
        }
    }

    /// The property grid's devices as `part.choose`'s target.
    pub(super) fn grid_part_target(&self, use_: Option<usize>, add: bool) -> Result<Option<Target>, Outcome> {
        let Some(sheet) = self.props.sheet() else {
            return Ok(None);
        };
        let Some(section) = sheet
            .sections
            .iter()
            .find(|s| s.kind == "device" && !s.targets.is_empty())
        else {
            return Ok(None);
        };
        let uses = section
            .rows
            .iter()
            .find(|r| r.ty.is_part_uses())
            .map(|r| &r.value);
        let copy_at = |i: usize| -> Option<Option<Uuid>> {
            match uses {
                Some(ew_api::output::RowValue::One { json, .. }) => Some(
                    json.get(i)
                        .and_then(|u| u.get("part"))
                        .and_then(|p| p.get("copy"))
                        .and_then(Value::as_str)
                        .and_then(|s| s.parse().ok()),
                ),
                Some(ew_api::output::RowValue::Varies { .. }) => None,
                _ => Some(None),
            }
        };
        let mode = if add {
            AssignMode::Append
        } else {
            match (use_, copy_at(use_.unwrap_or(0))) {
                (_, None) => AssignMode::ReplaceAll,
                (Some(_), Some(None)) => return Err(failed(key!("error-bad-param"), &["use"])),
                (None, Some(None)) => AssignMode::Append,
                (_, Some(Some(from))) => AssignMode::Replace { from },
            }
        };
        // A placeholder is replaced on every device that has it (doc 07 §3.3).
        let devices = match mode {
            AssignMode::Replace { from } => self
                .project
                .as_ref()
                .and_then(|p| p.session.placeholder_users(from))
                .filter(|users| !users.is_empty())
                .unwrap_or_else(|| section.targets.clone()),
            _ => section.targets.clone(),
        };
        let tag = self.tag_of(devices[0]);
        let function = self
            .project
            .as_ref()
            .and_then(|p| p.session.main_function(devices[0]))
            .map(|f| f.0);
        // A cable is offered cable parts only (every part was offered).
        let cables = self
            .project
            .as_ref()
            .map(|p| p.session.cables())
            .unwrap_or_default();
        let kinds = if devices.iter().all(|d| cables.iter().any(|(c, _, _)| c == d)) {
            vec!["cable".to_owned()]
        } else {
            Vec::new()
        };
        Ok(Some(Target {
            text: if devices.len() == 1 {
                tag.clone()
            } else {
                strings::format(key!("part-choose-target-devices"), &[&devices.len().to_string()])
            },
            target: PartTarget::Devices { devices, mode },
            tag: Some(tag),
            function,
            kinds,
        }))
    }

    /// Lists the offers of the open dialog's query again (favourites and recent parts from the settings).
    pub(super) fn part_refresh(&mut self) {
        let favourites = parse_items(&self.settings.parts_favourites);
        let recent = parse_items(&self.settings.parts_recent);
        let Some(project) = &self.project else {
            return;
        };
        if let Some(Dialog::PartChoose(c)) = &mut self.dialog {
            c.query.favourites = favourites;
            c.query.recent = recent;
            (c.query.purpose_categories, c.query.purpose_kinds) = purposes(&c.query.text);
            // A part's description in the window's language, else English, else its first.
            c.query.lang = Some(strings::rule_language().to_owned());
            c.offers = project.session.part_offers(&c.query);
            // A layout kind is offered to a device only where panel layout
            // attaches it (`chooser_layout_kinds`: an enclosure or a plate to a device without a main function, never an
            // end bracket, a rail or a duct).
            if let PartTarget::Devices { devices, .. } = &c.target {
                let allowed = project.session.chooser_layout_kinds(devices);
                c.offers
                    .retain(|o| !LAYOUT_KIND_WORDS.contains(&o.kind.as_str()) || allowed.contains(&o.kind));
            }
            // A cable's parts that do not fit it come after those that do, each with the reason (greyed, not
            // hidden).
            let cable = match &c.target {
                PartTarget::Devices { devices, .. } if devices.len() == 1 => devices
                    .first()
                    .copied()
                    .filter(|d| project.session.cables().iter().any(|(c, _, _)| c == d)),
                _ => None,
            };
            let mut rows: Vec<(Option<String>, PartOffer)> = std::mem::take(&mut c.offers)
                .into_iter()
                .map(|o| {
                    let why = cable
                        .and_then(|cable| project.session.cable_part_misfit(cable, &o.pick))
                        .map(|r| message_text(Key::run_time(r.key), &r.args));
                    (why, o)
                })
                .collect();
            rows.sort_by_key(|(why, _)| why.is_some());
            (c.misfits, c.offers) = rows.into_iter().unzip();
            c.cursor = c.cursor.filter(|i| *i < c.offers.len());
        }
    }

    /// The terminals of strip `strip` without a part, in their order on the strip.
    pub(crate) fn strip_terminals_without_part(&self, strip: Uuid) -> Vec<Uuid> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let mut terminals: Vec<(u32, Uuid)> = q
            .devices()
            .iter()
            .filter(|(_, d)| d.parts.is_empty())
            .filter_map(|(id, d)| {
                let t = d
                    .aspects
                    .terminal
                    .as_ref()
                    .filter(|t| t.strip.as_uuid() == strip)?;
                Some((t.seq, id.as_uuid()))
            })
            .collect();
        terminals.sort_unstable();
        terminals.into_iter().map(|(_, id)| id).collect()
    }

    /// The terminals of strip `strip` (its ID) without a part as `part.choose`'s target (a strip's row).
    pub(super) fn strip_terminals_target(&self, strip: &str) -> Result<Target, Outcome> {
        let terminals = strip
            .parse()
            .map(|s| self.strip_terminals_without_part(s))
            .map_err(|_| failed(key!("error-bad-param"), &["strip"]))?;
        let Some(&first) = terminals.first() else {
            return Err(Outcome::refused(key!("reason-strip-all-parts")));
        };
        let tag = self.tag_of(first);
        Ok(Target {
            text: if terminals.len() == 1 {
                tag.clone()
            } else {
                strings::format(
                    key!("part-choose-target-devices"),
                    &[&terminals.len().to_string()],
                )
            },
            function: self
                .project
                .as_ref()
                .and_then(|p| p.session.main_function(first))
                .map(|f| f.0),
            target: PartTarget::Devices {
                devices: terminals,
                mode: AssignMode::ReplaceAll,
            },
            tag: Some(tag),
            kinds: Vec::new(),
        })
    }
}
