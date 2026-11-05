// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A cable with its cores from the window (FT-1371, ID-1362): Draw ▸ Connect ▸ New cable… (`cable.new`) and Connect
//! cores… (`cable.connect_cores`), the existing model commands behind a form, as the creation dialogs are.
//!
//! - **New cable…** creates a cable device from a cable part with one conductor per core of the part
//!   (`Session::create_cable`: one `cable.create` that embeds the part first when it comes from an installed library,
//!   FR-CBL-004, one undo step). Its form offers the cable parts of the project, then of the installed libraries.
//! - **Connect cores…** makes the selected wires of the page the cable's cores, by order (doc 08 §7.2): the wires left
//!   to right, then top to bottom, take the cable's free cores in its part's order (`Session::connect_cores`, one
//!   `fields.set_many`, one undo step). Its form offers the project's cables by tag, two cables with one tag told
//!   apart by their part and position (`Session::cables`).
//!
//! In the window without its parameter each action opens its form; headless the parameter is required. A refusal stays
//! under the fields, a success closes the form with a toast.

use ew_api::output::Uuid;
use serde_json::{Map, Value};

use super::{Dialog, FormDialog, FormField, Mode, Outcome, ShellModel, failed};
use crate::key;
use crate::strings;
use crate::strings::Key;
use crate::tools::{EntityKind, selection_kinds};

/// The action of New cable….
const NEW: &str = "cable.new";
/// The action of Connect cores….
const CONNECT: &str = "cable.connect_cores";

/// The trimmed, non-empty text parameter `name`.
fn given(p: &Map<String, Value>, name: &str) -> Option<String> {
    p.get(name)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

impl ShellModel {
    /// Whether the open dialog is the form of `action`.
    fn in_cable_form(&self, action: &str) -> bool {
        matches!(&self.dialog, Some(Dialog::Form(f)) if f.action == action)
    }

    /// A refusal: it stays under the fields when the form of `action` is open.
    fn cable_form_failed(&mut self, action: &str, message: String) -> Outcome {
        if let Some(Dialog::Form(f)) = &mut self.dialog
            && f.action == action
        {
            f.error = Some(message.clone());
        }
        Outcome::Failed { message }
    }

    /// The form of `action` with its button `ok_key` and its one choice field `name` (its label `label`, the
    /// options, the first chosen).
    fn cable_form(
        action: &'static str,
        title_key: Key<'static>,
        ok_key: Key<'static>,
        name: &'static str,
        label_key: Key<'static>,
        options: Vec<(String, String)>,
    ) -> FormDialog {
        let value = options.first().map(|o| o.0.clone()).unwrap_or_default();
        FormDialog {
            action,
            title_key,
            ok_key,
            fields: vec![FormField {
                name,
                label_key,
                choices: &[],
                options,
                value,
            }],
            error: None,
            table: None,
        }
    }

    /// `cable.new {part}` (Draw ▸ Connect ▸ New cable…, FT-1371): a cable from the cable part `part` (its number or its
    /// ID), with its cores, then the toast naming the new cable. A project with no cable part, here or in an installed
    /// library, is refused in words.
    pub(crate) fn act_cable_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let offers = project.session.layout_part_offers("cable");
        if offers.is_empty() {
            return Outcome::Refused {
                reason_key: key!("reason-no-cable-part"),
            };
        }
        let Some(part) = given(p, "part") else {
            return match self.mode {
                Mode::Headless => failed(key!("error-missing-param"), &["part"]),
                Mode::Gui if self.in_cable_form(NEW) => {
                    let message = self.missing_text("part");
                    self.cable_form_failed(NEW, message)
                }
                Mode::Gui => {
                    let options = offers
                        .iter()
                        .map(|o| {
                            let words = if o.description.is_empty() {
                                o.number.clone()
                            } else {
                                format!("{} \u{b7} {}", o.number, o.description)
                            };
                            (o.part.to_string(), words)
                        })
                        .collect();
                    let form = Self::cable_form(
                        NEW,
                        key!("cable-new-title"),
                        key!("dialog-create"),
                        "part",
                        key!("form-cable-part"),
                        options,
                    );
                    self.open_dialog(Dialog::Form(form))
                }
            };
        };
        let Some(offer) = offers
            .iter()
            .find(|o| o.part.to_string() == part || o.number == part)
        else {
            let message = strings::format(key!("error-no-cable-part"), &[&part]);
            return self.cable_form_failed(NEW, message);
        };
        let Some(cmd) = project.session.create_cable(offer.part) else {
            let message = strings::format(key!("error-no-cable-part"), &[&part]);
            return self.cable_form_failed(NEW, message);
        };
        let cores = offer_cores(&cmd);
        match self.host_create(&cmd.name, cmd.params) {
            Outcome::Done => {
                let tag = self
                    .tools
                    .last_primary
                    .as_deref()
                    .and_then(|id| Uuid::parse_str(id).ok())
                    .and_then(|id| self.project.as_ref()?.session.device_tag(id))
                    .unwrap_or_default();
                self.notify(
                    strings::format(
                        key!("toast-cable-new"),
                        &[
                            &tag,
                            &strings::format_count(crate::count_key!("toast-cable-new-cores"), cores),
                        ],
                    ),
                    true,
                );
                if self.in_cable_form(NEW) {
                    self.dialog = None;
                }
                Outcome::Done
            }
            Outcome::Failed { message } => self.cable_form_failed(NEW, message),
            other => other,
        }
    }

    /// The connections selected on the page shown, left to right, then top to bottom (by the leftmost, then the
    /// topmost point of each drawn path): the order a cable's line would cross them (doc 08 §7.2).
    fn selected_wires(&self) -> Vec<Uuid> {
        let Some(c) = self.canvas() else {
            return Vec::new();
        };
        let tables = &c.page().tables;
        let mut wires: Vec<((i64, i64), Uuid)> = selection_kinds(&self.tools.selection, tables)
            .into_iter()
            .filter(|(_, k)| *k == EntityKind::Connection)
            .filter_map(|(e, _)| tables.connections.iter().find(|c| c.id == e.0))
            .map(|c| {
                let x = c.polyline.iter().map(|p| p.0).min().unwrap_or(0);
                let y = c.polyline.iter().map(|p| p.1).min().unwrap_or(0);
                ((x, y), c.id)
            })
            .collect();
        wires.sort();
        wires.into_iter().map(|(_, id)| id).collect()
    }

    /// `cable.connect_cores {cable}` (Draw ▸ Connect ▸ Connect cores…, FT-1371): the selected wires become cores of
    /// the cable `cable` (its ID, its tag, or its words in the form's list), by order, in one undo step, then the toast.
    /// No wire selected, no cable in the project, a tag that several cables print, a cable with too few free cores:
    /// refused in words.
    pub(crate) fn act_cable_connect_cores(&mut self, p: &Map<String, Value>) -> Outcome {
        let wires = self.selected_wires();
        if wires.is_empty() {
            return Outcome::Refused {
                reason_key: key!("reason-cores-no-wires"),
            };
        }
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let cables = project.session.cables();
        if cables.is_empty() {
            return Outcome::Refused {
                reason_key: key!("reason-cores-no-cable"),
            };
        }
        let Some(cable) = given(p, "cable") else {
            return match self.mode {
                Mode::Headless => failed(key!("error-missing-param"), &["cable"]),
                Mode::Gui if self.in_cable_form(CONNECT) => {
                    let message = self.missing_text("cable");
                    self.cable_form_failed(CONNECT, message)
                }
                Mode::Gui => {
                    let options = cables
                        .iter()
                        .map(|(id, _, words)| (id.to_string(), words.clone()))
                        .collect();
                    let form = Self::cable_form(
                        CONNECT,
                        key!("connect-cores-title"),
                        key!("dialog-connect"),
                        "cable",
                        key!("form-cable"),
                        options,
                    );
                    self.open_dialog(Dialog::Form(form))
                }
            };
        };
        // A tag that several cables print names none of them: refused in words, never the first (decision log ID-1376).
        let shared = cables.iter().filter(|(_, tag, _)| *tag == cable).count();
        if shared > 1 {
            let message = strings::format(key!("error-cable-ambiguous"), &[&cable, &shared.to_string()]);
            return self.cable_form_failed(CONNECT, message);
        }
        let Some((id, _, tag)) = cables
            .iter()
            .find(|(id, tag, words)| id.to_string() == cable || *tag == cable || *words == cable)
            .cloned()
        else {
            let message = strings::format(key!("error-no-cable"), &[&cable]);
            return self.cable_form_failed(CONNECT, message);
        };
        let cmd = match project.session.connect_cores(id, &wires) {
            Ok(cmd) => cmd,
            Err(refused) if refused.args.is_empty() => {
                return Outcome::Refused {
                    reason_key: Key::run_time(refused.key),
                };
            }
            Err(refused) => {
                let args: Vec<&str> = refused.args.iter().map(String::as_str).collect();
                let message = strings::format(Key::run_time(refused.key), &args);
                return self.cable_form_failed(CONNECT, message);
            }
        };
        let out = self.commit_from_host(&cmd);
        match out {
            Outcome::Done => {
                // FT-1406: the cable is placed through its first core now, so the toast names it as it reads after
                // the step.
                let tag = self
                    .project
                    .as_ref()
                    .and_then(|p| p.session.cables().into_iter().find(|(i, _, _)| *i == id))
                    .map_or(tag, |(_, _, words)| words);
                self.notify(
                    strings::format(key!("toast-cores-connected"), &[&wires.len().to_string(), &tag]),
                    true,
                );
                if self.in_cable_form(CONNECT) {
                    self.dialog = None;
                }
                Outcome::Done
            }
            Outcome::Failed { message } => self.cable_form_failed(CONNECT, message),
            other => other,
        }
    }
}

/// The number of conductors the command of New cable… creates.
fn offer_cores(command: &ew_api::AnyCommand) -> usize {
    command
        .params
        .get("conductors")
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}
