// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A cable with its cores from the window: Draw ▸ Connect ▸ New cable… (`cable.new`) and Connect
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
//! - **Cable…** (`cable.from_wires`, FT-1685) is the one flow from the Draw tab: the selected wires become the cores
//!   of a new cable, or of the cable one of them already belongs to, in one form (`Session::cable_form`, its tag, its
//!   type as written, `3G1.5`, and the wires with their cores) and one apply (`Session::cable_apply`).
//!
//! In the window without its parameter each action opens its form; headless the parameter is required. A refusal stays
//! under the fields, a success closes the form with a toast.

use ew_api::output::Uuid;
use ew_api::output::ew_dl::{Destination, EntityRef};
use serde_json::{Map, Value};

use super::{Dialog, FormDialog, FormField, Mode, Outcome, ShellModel, failed};
use crate::canvas::PageId;
use crate::key;
use crate::strings;
use crate::strings::Key;
use crate::tools::{EntityKind, SelectionChange, selection_kinds};

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
                choices: &super::choices::NONE,
                options,
                value,
            }],
            error: None,
            table: None,
        }
    }

    /// `cable.new {part}` (Draw ▸ Connect ▸ New cable…): a cable from the cable part `part` (its number or its
    /// ID), with its cores, then the toast naming the new cable. A project with no cable part, here or in an installed
    /// library, is refused in words.
    pub(crate) fn act_cable_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let offers = project.session.layout_part_offers("cable");
        if offers.is_empty() {
            return Outcome::refused(key!("reason-no-cable-part"));
        }
        let Some(part) = given(p, "part") else {
            return match self.mode {
                Mode::Headless => failed(key!("error-missing-param"), &["part"]),
                Mode::Gui if self.in_form_of(NEW) => {
                    let message = self.missing_text("part");
                    self.form_failed(NEW, message)
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
            return self.form_failed(NEW, message);
        };
        let Some(cmd) = project.session.create_cable(offer.part) else {
            let message = strings::format(key!("error-no-cable-part"), &[&part]);
            return self.form_failed(NEW, message);
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
                if self.in_form_of(NEW) {
                    self.dialog = None;
                }
                Outcome::Done
            }
            Outcome::Failed { message } => self.form_failed(NEW, message),
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

    /// Shows the page of `cable`'s first wire in the focused tab, as a jump, with the cable's wires there selected
    /// (Cable… from a Cables row). A cable without a wire is refused in words.
    fn show_cable_wires(&mut self, cable: Uuid) -> Result<(), Outcome> {
        let Some(project) = &self.project else {
            return Err(Outcome::refused(key!("reason-no-project")));
        };
        let wires = project.session.cable_wires(cable);
        let Some(page) = wires.first().and_then(|(w, _)| project.session.wire_page(*w)) else {
            return Err(Outcome::refused(key!("reason-cable-no-wire")));
        };
        let on_page: Vec<EntityRef> = wires
            .iter()
            .filter(|(w, _)| project.session.wire_page(*w) == Some(page))
            .map(|(w, _)| EntityRef(*w))
            .collect();
        let page = PageId(Destination::Page(page));
        if let Some(g) = self.area.as_ref().map(|a| a.focused)
            && self.current_page() != Some(page)
        {
            self.jump_to(g, page);
        }
        if self.tools.selection.apply(&SelectionChange::Replace(on_page)) {
            self.announce_selection();
        }
        Ok(())
    }

    /// `cable.connect_cores {cable}` (Draw ▸ Connect ▸ Connect cores…): the selected wires become cores of
    /// the cable `cable` (its ID, its tag, or its words in the form's list), by order, in one undo step, then the toast.
    /// No wire selected, no cable in the project, a tag that several cables print, a cable with too few free cores:
    /// refused in words.
    pub(crate) fn act_cable_connect_cores(&mut self, p: &Map<String, Value>) -> Outcome {
        let wires = self.selected_wires();
        if wires.is_empty() {
            return Outcome::refused(key!("reason-cores-no-wires"));
        }
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let cables = project.session.cables();
        if cables.is_empty() {
            return Outcome::refused(key!("reason-cores-no-cable"));
        }
        let Some(cable) = given(p, "cable") else {
            return match self.mode {
                Mode::Headless => failed(key!("error-missing-param"), &["cable"]),
                Mode::Gui if self.in_form_of(CONNECT) => {
                    let message = self.missing_text("cable");
                    self.form_failed(CONNECT, message)
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
        // A tag that several cables print names none of them: refused in words, never the first.
        let shared = cables.iter().filter(|(_, tag, _)| *tag == cable).count();
        if shared > 1 {
            let message = strings::format(key!("error-cable-ambiguous"), &[&cable, &shared.to_string()]);
            return self.form_failed(CONNECT, message);
        }
        let Some((id, _, tag)) = cables
            .iter()
            .find(|(id, tag, words)| id.to_string() == cable || *tag == cable || *words == cable)
            .cloned()
        else {
            let message = strings::format(key!("error-no-cable"), &[&cable]);
            return self.form_failed(CONNECT, message);
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
                return self.form_failed(CONNECT, message);
            }
        };
        let out = self.commit_from_host(&cmd);
        match out {
            Outcome::Done => {
                // The cable is placed through its first core now, so the toast names it as it reads after
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
                if self.in_form_of(CONNECT) {
                    self.dialog = None;
                }
                Outcome::Done
            }
            Outcome::Failed { message } => self.form_failed(CONNECT, message),
            other => other,
        }
    }
}

/// The action of Cable….
const FROM_WIRES: &str = "cable.from_wires";

/// The rows' cores of Cable…'s table after its type changed to `type_text`: a core the new type has stays, the
/// others take its free cores in order (`assign_cores`); a text that is no type yet changes nothing.
pub(super) fn retype(session: Option<&ew_api::Session>, form: &mut super::FormDialog) {
    let Some((cores, _)) = session.and_then(|s| {
        let text = form.fields.iter().find(|f| f.name == "type")?.value.clone();
        s.cable_type(&text)
    }) else {
        return;
    };
    let Some(table) = &mut form.table else {
        return;
    };
    let rows: Vec<(Uuid, String)> = table
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            (
                Uuid::from_u128(i as u128),
                r.cells.first().cloned().unwrap_or_default(),
            )
        })
        .collect();
    for (row, (_, core)) in table
        .rows
        .iter_mut()
        .zip(ew_api::window::assign_cores(&cores, &rows))
    {
        if let Some(cell) = row.cells.first_mut() {
            *cell = core;
        }
    }
}

impl ShellModel {
    /// A wire in words, by its ends as the page prints them (`-1K1:A1 → L1`); its ID without a page.
    fn wire_words(&self, wire: Uuid) -> String {
        let Some(project) = &self.project else {
            return wire.to_string();
        };
        project
            .session
            .wire_page(wire)
            .and_then(|p| self.page_rows().iter().find(|r| r.id == p))
            .and_then(|r| project.session.connection_words(&r.folio, wire))
            .map_or_else(
                || wire.to_string(),
                |t| super::tools::rejected_words("", Some(&t)),
            )
    }

    /// `cable.from_wires {tag?, type?, cores?}` (Draw ▸ Connect ▸ Cable…, FT-1685): the selected wires as the cores of
    /// a cable, in one form. Without `type` the window opens the form: the cable's tag (the cable one of the wires
    /// already belongs to, else the tag a new cable would take), its type (that cable's, else `<n>x<cross-section>`
    /// of the first wire), and the wires with the core each gets, which the person can change. With `type` it applies:
    /// a typed tag of another existing cable puts the wires in that cable; a tag other than the proposal is frozen.
    /// `cores` is the form's table, or a text of cores in the wires' order (`BU BN`); absent, each wire keeps its core
    /// or takes the next free one. A new cable is one undo step; an existing cable that gains cores is two. No wire
    /// selected, a type, a core or a tag the form does not take: refused in words. With `cable` (its ID, a Cables row's
    /// Cable…, FT-1693) the page of its first wire shows first, the cable's wires there selected.
    pub(crate) fn act_cable_from_wires(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(cable) = given(p, "cable") {
            let Ok(cable) = cable.parse::<Uuid>() else {
                return failed(key!("error-bad-param"), &["cable"]);
            };
            if let Err(out) = self.show_cable_wires(cable) {
                return out;
            }
        }
        let wires = self.selected_wires();
        if wires.is_empty() {
            return Outcome::refused(key!("reason-cores-no-wires"));
        }
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let form = project.session.cable_form(&wires);
        let proposed = match form.cable {
            Some(_) => Some(form.tag.clone()),
            None => self.strip_tag_example("W"),
        };
        let Some(type_text) = given(p, "type") else {
            return match self.mode {
                Mode::Headless => failed(key!("error-missing-param"), &["type"]),
                Mode::Gui if self.in_form_of(FROM_WIRES) => {
                    let message = self.missing_text("type");
                    self.form_failed(FROM_WIRES, message)
                }
                Mode::Gui => {
                    let text = |name, label_key, value: String| FormField {
                        name,
                        label_key,
                        choices: &super::choices::NONE,
                        options: Vec::new(),
                        value,
                    };
                    let rows = form
                        .rows
                        .iter()
                        .map(|(w, core)| super::FormRow {
                            key: self.wire_words(*w),
                            cells: vec![core.clone()],
                        })
                        .collect();
                    let dialog = FormDialog {
                        action: FROM_WIRES,
                        title_key: key!("bar-cable"),
                        ok_key: key!("dialog-apply"),
                        fields: vec![
                            text("tag", key!("form-cable-tag"), proposed.unwrap_or_default()),
                            text("type", key!("form-cable-type"), form.type_text.clone()),
                        ],
                        error: None,
                        table: Some(super::FormTable {
                            name: "cores",
                            heading_key: key!("form-cable-cores"),
                            fixed: true,
                            key_label: key!("form-cable-wire"),
                            columns: vec![(strings::text(key!("form-cable-core")).to_owned(), String::new())],
                            rows,
                        }),
                    };
                    self.open_dialog(Dialog::Form(dialog))
                }
            };
        };
        // The cable: the one the wires belong to, else an existing one the typed tag names, else a new one.
        let typed_tag = given(p, "tag");
        let cables = project.session.cables();
        let named = typed_tag
            .as_ref()
            .and_then(|t| cables.iter().find(|(_, tag, _)| tag == t).map(|(id, _, _)| *id));
        let cable = form.cable.or(named);
        let rows: Vec<(Uuid, String)> = if form.cable.is_none() && named.is_some() {
            wires.iter().map(|w| (*w, String::new())).collect()
        } else {
            form.rows.clone()
        };
        // The cores given: the form's table by row, or a text in the wires' order.
        let given_cores: Option<Vec<String>> = match p.get("cores") {
            Some(Value::Array(table)) => Some(
                table
                    .iter()
                    .map(|r| {
                        r.get("values")
                            .and_then(Value::as_object)
                            .and_then(|v| v.values().next())
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .trim()
                            .to_owned()
                    })
                    .collect(),
            ),
            Some(Value::String(s)) => Some(
                s.split([',', ' '])
                    .filter(|c| !c.is_empty())
                    .map(str::to_owned)
                    .collect(),
            ),
            _ => None,
        };
        let rows: Vec<(Uuid, String)> = match given_cores {
            Some(cores) => rows
                .iter()
                .enumerate()
                .map(|(i, (w, _))| (*w, cores.get(i).cloned().unwrap_or_default()))
                .collect(),
            None => {
                let cores = project
                    .session
                    .cable_type(&type_text)
                    .map(|(c, _)| c)
                    .unwrap_or_default();
                ew_api::window::assign_cores(&cores, &rows)
            }
        };
        // A typed tag other than the cable's own, or the proposal, is frozen.
        let current = match cable {
            Some(c) => project.session.device_tag(c),
            None => proposed.clone(),
        };
        let frozen = match typed_tag.filter(|t| Some(t) != current.as_ref() && named.is_none()) {
            None => None,
            Some(tag) => match project.session.parse_field_input("device", "tag.frozen", &tag) {
                Ok(parsed) if !parsed.value.is_null() => Some(parsed.value),
                _ => {
                    let such_as = proposed.unwrap_or_else(|| "-W1".to_owned());
                    let message = strings::format(key!("error-strip-tag"), &[&tag, &such_as]);
                    return self.form_failed(FROM_WIRES, message);
                }
            },
        };
        let commands = match project.session.cable_apply(cable, frozen, &type_text, &rows) {
            Ok(commands) => commands,
            Err(refused) => {
                let args: Vec<&str> = refused.args.iter().map(String::as_str).collect();
                let message = strings::format(Key::run_time(refused.key), &args);
                return self.form_failed(FROM_WIRES, message);
            }
        };
        for cmd in &commands {
            match self.commit_from_host(cmd) {
                Outcome::Done => {}
                Outcome::Failed { message } => return self.form_failed(FROM_WIRES, message),
                other => return other,
            }
        }
        // The cable as it reads after the step (a new one takes its page number from its first core).
        let tag = self
            .project
            .as_ref()
            .and_then(|p| {
                let c = cable.or_else(|| {
                    rows.iter()
                        .find(|(_, c)| !c.is_empty())
                        .and_then(|(w, _)| p.session.cable_of_wire(*w))
                })?;
                p.session.device_tag(c)
            })
            .unwrap_or_default();
        let cored = rows.iter().filter(|(_, c)| !c.is_empty()).count();
        self.notify(
            strings::format(key!("toast-cable-wires"), &[&tag, &cored.to_string()]),
            true,
        );
        if self.in_form_of(FROM_WIRES) {
            self.dialog = None;
        }
        Outcome::Done
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
