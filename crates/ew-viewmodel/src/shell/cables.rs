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
//! - **Cable…** on selected bus lines (`bus.cable`, doc 10 §2.7) opens the bus line's form instead: the cable that
//!   carries them (its tag, its part, those made for the bus first, its length) and, for one line, the plug at each end
//!   with its termination (`Session::bus_cable_form`, one `bus.cable.set`).
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

/// The value of New cable type…, the last entry of a Cable form's list of cable types (rc.3): it opens the small form of
/// a new cable part, never names a part (a part number has no angle brackets).
pub const NEW_CABLE_TYPE: &str = "<new cable type>";

/// Whether `form` is a Cable form whose list of cable types ends in New cable type… (Cable… from wires, New cable…).
pub(super) fn chooses_cable_type(form: &FormDialog) -> bool {
    [NEW, FROM_WIRES].contains(&form.action) && form.fields.iter().any(|f| f.name == "part")
}

/// The list entry of New cable type….
fn new_type_entry() -> (String, String) {
    (
        NEW_CABLE_TYPE.to_owned(),
        strings::text(key!("form-cable-type-new")).to_owned(),
    )
}

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
                    let options = new_cable_options(&project.session);
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

/// The rows' cores of Cable…'s table after its type changed: a wire that is a cable's core already keeps it where
/// the new type has that core, and every other wire takes a core as the form proposes it for the new type
/// (`Session::assign_cores`, by the potential of the row's wire, `wires` in the rows' order), so a core typed by hand
/// for a wire not yet in a cable is proposed again; a text that is no type yet changes nothing.
pub(super) fn retype(session: Option<&ew_api::Session>, wires: &[Uuid], form: &mut super::FormDialog) {
    let Some(session) = session else {
        return;
    };
    let Some((cores, _)) = form
        .fields
        .iter()
        .find(|f| f.name == "type")
        .and_then(|f| session.cable_type(&f.value))
    else {
        return;
    };
    let Some(table) = &mut form.table else {
        return;
    };
    let rows: Vec<(Uuid, String)> = table
        .rows
        .iter()
        .zip(wires)
        .map(|(r, w)| {
            // A wire that is a cable's core already keeps it; a wire not yet in a cable is proposed afresh for the
            // new type, or the neutral would stay on the brown core that a type without a blue one gave it.
            let core = if session.cable_of_wire(*w).is_some() {
                r.cells.first().cloned().unwrap_or_default()
            } else {
                String::new()
            };
            (*w, core)
        })
        .collect();
    for (row, (_, core)) in table.rows.iter_mut().zip(session.assign_cores(&cores, &rows)) {
        if let Some(cell) = row.cells.first_mut() {
            *cell = core;
        }
    }
    // A new cable (its form asks for a part) says at once which wire the type leaves without a core, and why (rc.3:
    // three wires listed, the cable made from two, nothing said).
    if form.fields.iter().any(|f| f.name == "part") {
        let rows: Vec<(String, String)> = table
            .rows
            .iter()
            .map(|r| (r.key.clone(), r.cells.first().cloned().unwrap_or_default()))
            .collect();
        if let Some(message) = coreless(&cores, &rows) {
            form.error = Some(message);
        }
    }
}

/// Why the first of `rows` (a wire in words, its core) has no core of the type `cores`, in a sentence: the type's
/// only free cores are green-yellow, kept for a protective conductor, or it has none free; `None` when every row has
/// a core.
fn coreless(cores: &[ew_api::window::TypeCore], rows: &[(String, String)]) -> Option<String> {
    let (wire, _) = rows.iter().find(|(_, core)| core.trim().is_empty())?;
    let free_green_yellow = cores
        .iter()
        .filter(|c| !rows.iter().any(|(_, core)| core.trim() == c.core))
        .any(|c| c.colour.iter().map(String::as_str).eq(["GN", "YE"]));
    Some(if free_green_yellow {
        strings::format(key!("error-cable-wire-no-core-pe"), &[wire])
    } else {
        strings::format(
            key!("error-cable-wire-no-core"),
            &[wire, &cores.len().to_string()],
        )
    })
}

impl ShellModel {
    /// The wires of Cable…'s rows, in the rows' order: the form's rows of the selected wires.
    pub(super) fn cable_row_wires(&self) -> Vec<Uuid> {
        self.project.as_ref().map_or_else(Vec::new, |p| {
            let form = p.session.cable_form(&self.selected_wires());
            form.rows.into_iter().map(|(w, _)| w).collect()
        })
    }

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
    /// already belongs to, else the tag a new cable would take), its type (that cable's, else `<n>X<cross-section>`
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
        // A bus line takes a bus cable and its plugs: its own form, never a refusal of a wire's cores.
        if self.only_bus_lines_selected() {
            return self.act_bus_cable(&Map::new());
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
            None => self.tag_example("cable", "W"),
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
                    // A new cable asks for its type first: the cable parts that fit the wires, best first, or none
                    // and the size typed (rc.3). Choosing a part writes its number as the type.
                    let mut fields = Vec::new();
                    let mut type_text = form.type_text.clone();
                    if form.cable.is_none() {
                        let options = from_wires_options(&project.session, &wires, &form.type_text);
                        // The best part that fits, else no part (never New cable type…, the list's last entry).
                        let value = options
                            .get(1)
                            .map(|o| o.0.clone())
                            .filter(|v| v != NEW_CABLE_TYPE)
                            .unwrap_or_default();
                        if !value.is_empty() {
                            type_text.clone_from(&value);
                        }
                        fields.push(FormField {
                            name: "part",
                            label_key: key!("form-cable-kind"),
                            choices: &super::choices::NONE,
                            options,
                            value,
                        });
                    }
                    fields.push(text("tag", key!("form-cable-tag"), proposed.unwrap_or_default()));
                    fields.push(text("type", key!("form-cable-type"), type_text));
                    let mut dialog = FormDialog {
                        action: FROM_WIRES,
                        title_key: key!("bar-cable"),
                        ok_key: key!("dialog-apply"),
                        fields,
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
                    let row_wires: Vec<Uuid> = form.rows.iter().map(|r| r.0).collect();
                    retype(Some(&project.session), &row_wires, &mut dialog);
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
                project.session.assign_cores(&cores, &rows)
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
        // Every selected wire becomes a core of a new cable, or nothing is made and the form says which wire cannot.
        if cable.is_none() {
            let cores = project
                .session
                .cable_type(&type_text)
                .map(|(c, _)| c)
                .unwrap_or_default();
            let words: Vec<(String, String)> = rows
                .iter()
                .map(|(w, core)| (self.wire_words(*w), core.clone()))
                .collect();
            if let Some(message) = coreless(&cores, &words).filter(|_| !cores.is_empty()) {
                return self.form_failed(FROM_WIRES, message);
            }
        }
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
        let wires = strings::format_count(crate::count_key!("cable-wires-count"), cored);
        let said = if cable.is_some() {
            key!("toast-cable-wires-changed")
        } else {
            key!("toast-cable-wires")
        };
        self.notify(strings::format(said, &[&tag, &wires]), true);
        if self.in_form_of(FROM_WIRES) {
            self.dialog = None;
        }
        Outcome::Done
    }
}

/// New cable…'s list: the cable parts by their number and description, then New cable type….
fn new_cable_options(session: &ew_api::Session) -> Vec<(String, String)> {
    session
        .layout_part_offers("cable")
        .iter()
        .map(|o| {
            let words = if o.description.is_empty() {
                o.number.clone()
            } else {
                format!("{} \u{b7} {}", o.number, o.description)
            };
            (o.part.to_string(), words)
        })
        .chain([new_type_entry()])
        .collect()
}

/// Cable…'s list of cable types for the wires `wires` and the size `proposed`: no part (the size typed), the cable
/// parts that fit, best first, then New cable type….
fn from_wires_options(session: &ew_api::Session, wires: &[Uuid], proposed: &str) -> Vec<(String, String)> {
    std::iter::once((
        String::new(),
        strings::text(key!("form-cable-no-part")).to_owned(),
    ))
    .chain(session.cable_parts_for(wires, proposed))
    .chain([new_type_entry()])
    .collect()
}

impl ShellModel {
    /// Back in the Cable form `form` after New cable type… saved the cable part `number` (rc.3): the form's list again,
    /// the new type in it and chosen (in Cable… from wires also as the type, its cores proposed again).
    pub(super) fn cable_type_made(&mut self, mut form: FormDialog, number: &str) {
        let Some(project) = &self.project else {
            return;
        };
        let (mut options, value) = if form.action == NEW {
            let offers = project.session.layout_part_offers("cable");
            let value = offers
                .iter()
                .find(|o| o.number == number)
                .map(|o| o.part.to_string());
            (new_cable_options(&project.session), value)
        } else {
            let wires = self.selected_wires();
            let size = form
                .fields
                .iter()
                .find(|f| f.name == "type")
                .map(|f| f.value.clone())
                .unwrap_or_default();
            (
                from_wires_options(&project.session, &wires, &size),
                Some(number.to_owned()),
            )
        };
        let Some(value) = value else {
            self.dialog = Some(Dialog::Form(form));
            return;
        };
        // A type that does not fit the wires is listed all the same: it is the one just made.
        if !options.iter().any(|(v, _)| *v == value) {
            options.insert(options.len() - 1, (value.clone(), number.to_owned()));
        }
        if let Some(f) = form.fields.iter_mut().find(|f| f.name == "part") {
            f.options = options;
        }
        self.dialog = Some(Dialog::Form(form));
        self.set_form_field("part", &value);
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

/// The action of a bus line's Cable form.
pub(super) const BUS_CABLE: &str = "bus.cable";

/// The value of New bus cable… and New plug…, the last entry of a bus line's Cable form's lists of cables and plugs
/// (rc.5): it opens the small form of a new bus part, never names a part.
pub const NEW_BUS_PART: &str = "<new bus part>";

/// The value of A new cable, the first entry of a bus line's Cable form's choice of cable (rc.5).
const NEW_CABLE: &str = "<new cable>";

impl ShellModel {
    /// The selected connections that are bus lines, in the order of [`Self::selected_wires`].
    pub(super) fn selected_bus_lines(&self) -> Vec<Uuid> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        self.selected_wires()
            .into_iter()
            .filter(|w| project.session.is_bus_line(*w))
            .collect()
    }

    /// Whether every selected connection is a bus line (Cable… then opens the bus line's form).
    fn only_bus_lines_selected(&self) -> bool {
        let wires = self.selected_wires();
        !wires.is_empty() && self.selected_bus_lines().len() == wires.len()
    }

    /// `bus.cable {part?, tag?, length?, plug_a?, termination_a?, plug_b?, termination_b?}` (Cable… on a selected bus
    /// line, doc 10 §2.7): the cable of the selected bus lines and, for one line, the plug at each end, in one form and
    /// one undo step. Without `part` the window opens the form: the cable's tag (the lines' cable's, else the tag a new
    /// cable would take), its part (the cable parts made for the line's bus first, the others saying they are not), its
    /// length in metres, and for one line the plug at each end with its termination. With `part` it applies: the lines'
    /// cable keeps its part; a typed tag of another cable links the lines to that cable; else a new cable of the part,
    /// a typed tag other than the proposal frozen. No bus line selected, a part, plug, length or tag the form does not
    /// take: refused in words.
    pub(crate) fn act_bus_cable(&mut self, p: &Map<String, Value>) -> Outcome {
        let lines = self.selected_bus_lines();
        if lines.is_empty() {
            return Outcome::refused(key!("reason-bus-no-lines"));
        }
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let form = project.session.bus_cable_form(&lines);
        if form.cables.is_empty() {
            return Outcome::refused(key!("reason-no-cable-part"));
        }
        let proposed = match form.cable {
            Some(_) => Some(form.tag.clone()),
            None => self.tag_example("cable", "W"),
        };
        let Some(part) = given(p, "part") else {
            return match self.mode {
                Mode::Headless => failed(key!("error-missing-param"), &["part"]),
                Mode::Gui if self.in_form_of(BUS_CABLE) => {
                    let message = self.missing_text("part");
                    self.form_failed(BUS_CABLE, message)
                }
                Mode::Gui => {
                    let dialog = self.bus_cable_dialog(&form, proposed);
                    self.open_dialog(Dialog::Form(dialog))
                }
            };
        };
        let Some(part) = form
            .cables
            .iter()
            .find(|o| o.part.to_string() == part || o.words == part)
            .map(|o| o.part)
        else {
            let message = strings::format(key!("error-no-cable-part"), &[&part]);
            return self.form_failed(BUS_CABLE, message);
        };
        // The cable: one of the project's bus cables chosen (a line that runs on, rc.5), or a new one chosen; else the
        // lines' cable while its tag stands, else an existing one the typed tag names, else a new one.
        let typed = given(p, "tag");
        let named = typed.as_ref().and_then(|t| {
            project
                .session
                .cables()
                .into_iter()
                .find(|(_, tag, _)| tag == t)
                .map(|(id, _, _)| id)
        });
        let chosen = match p.get("cable").and_then(Value::as_str).map(str::trim) {
            None => None,
            Some("" | NEW_CABLE) => Some(None),
            Some(text) => match form.existing.iter().find(|(id, _)| id.to_string() == text) {
                Some((id, _)) => Some(Some(*id)),
                None => {
                    let message = strings::format(key!("error-no-cable"), &[text]);
                    return self.form_failed(BUS_CABLE, message);
                }
            },
        };
        let cable = match (chosen, &typed, form.cable) {
            (Some(Some(c)), _, _) => Some(c),
            // A new cable chosen is new even where its tag is the one the lines' cable gives up.
            (Some(None), _, _) => named.filter(|n| Some(*n) != form.cable),
            (None, None, Some(c)) => Some(c),
            (None, Some(t), Some(c)) if *t == form.tag => Some(c),
            _ => named,
        };
        // A new cable's tag is frozen only when typed other than the next tag the numbering gives.
        let example = self.tag_example_except("cable", "W", form.cable.filter(|_| form.left_unused));
        let frozen = match typed.filter(|t| cable.is_none() && Some(t) != example.as_ref()) {
            None => None,
            Some(tag) => match project.session.parse_field_input("device", "tag.frozen", &tag) {
                Ok(parsed) if !parsed.value.is_null() => Some(parsed.value),
                _ => {
                    let such_as = proposed.unwrap_or_else(|| "-W1".to_owned());
                    let message = strings::format(key!("error-strip-tag"), &[&tag, &such_as]);
                    return self.form_failed(BUS_CABLE, message);
                }
            },
        };
        let length = match given(p, "length") {
            None => None,
            Some(text) => {
                let nm = project
                    .session
                    .parse_field_input("device", "cable.length", &text.replace(',', "."))
                    .ok()
                    .and_then(|parsed| parsed.value.get("manual").and_then(Value::as_i64));
                let Some(nm) = nm else {
                    let message = strings::format(key!("error-bus-cable-length"), &[&text]);
                    return self.form_failed(BUS_CABLE, message);
                };
                Some(nm)
            }
        };
        let mut plugs: Vec<ew_api::window::PlugChoice> = Vec::new();
        for (plug, termination) in [("plug_a", "termination_a"), ("plug_b", "termination_b")]
            .into_iter()
            .take(form.ends.len())
        {
            let on = or_return!(super::bulk::bool_text_param(p, termination, false));
            let chosen = match given(p, plug) {
                None => None,
                Some(text) => match form.plugs.iter().find(|o| o.part.to_string() == text) {
                    Some(o) => Some(o.part),
                    None => {
                        let message = strings::format(key!("error-bus-plug"), &[&text, &form.bus]);
                        return self.form_failed(BUS_CABLE, message);
                    }
                },
            };
            plugs.push((chosen, on));
        }
        let new_part = cable.is_none().then_some(part);
        let Some(cmd) = project
            .session
            .bus_cable_apply(&lines, cable, new_part, frozen, length, &plugs)
        else {
            let message = strings::format(key!("error-no-cable-part"), &[&part.to_string()]);
            return self.form_failed(BUS_CABLE, message);
        };
        match self.commit_from_host(&cmd) {
            Outcome::Done => {}
            Outcome::Failed { message } => return self.form_failed(BUS_CABLE, message),
            other => return other,
        }
        let tag = self
            .project
            .as_ref()
            .and_then(|p| {
                let cable = p.session.bus_cable_form(&lines).cable?;
                p.session.device_tag(cable)
            })
            .unwrap_or_default();
        let count = strings::format_count(crate::count_key!("bus-cable-lines-count"), lines.len());
        self.notify(strings::format(key!("toast-bus-cable"), &[&tag, &count]), true);
        if self.in_form_of(BUS_CABLE) {
            self.dialog = None;
        }
        Outcome::Done
    }

    /// Back in the bus line's Cable form `form` after New bus cable… or New plug… saved the part `number` (rc.5): the
    /// form's lists again with the new part, chosen in its field `field`; every other field keeps what it held.
    pub(super) fn bus_part_made(&mut self, form: FormDialog, number: &str, field: &'static str) {
        let lines = self.selected_bus_lines();
        let Some(project) = &self.project else {
            return;
        };
        let now = project.session.bus_cable_form(&lines);
        let offers = if field == "part" { &now.cables } else { &now.plugs };
        let made = offers
            .iter()
            .find(|o| o.words == number || o.words.starts_with(&format!("{number} \u{b7}")))
            .map(|o| o.part.to_string());
        let mut fresh = self.bus_cable_dialog(&now, None);
        for f in &mut fresh.fields {
            if let Some(old) = form.fields.iter().find(|o| o.name == f.name) {
                f.value.clone_from(&old.value);
            }
        }
        self.dialog = Some(Dialog::Form(fresh));
        if let Some(value) = made {
            // A new cable type makes a new cable for the line, under the next tag.
            if field == "part" {
                self.set_form_field("cable", NEW_CABLE);
            }
            self.set_form_field(field, &value);
        }
    }

    /// After the bus line's Cable form's choice of cable changed (rc.5): the tag field shows the chosen cable's tag, or
    /// the next tag a new cable takes.
    pub(super) fn bus_cable_chosen(&mut self, value: &str) {
        let tag = match value.parse::<Uuid>() {
            Ok(c) => self.project.as_ref().and_then(|p| p.session.device_tag(c)),
            Err(_) => {
                let replaced = self.replaced_bus_cable();
                self.tag_example_except("cable", "W", replaced)
            }
        };
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && let Some(f) = form.fields.iter_mut().find(|f| f.name == "tag")
        {
            f.value = tag.unwrap_or_default();
        }
    }

    /// The selected lines' cable when a new cable for them leaves it unused, which the step then removes
    /// (`BusCableForm::left_unused`): its tag is free for the new one.
    fn replaced_bus_cable(&self) -> Option<Uuid> {
        let form = self
            .project
            .as_ref()?
            .session
            .bus_cable_form(&self.selected_bus_lines());
        form.cable.filter(|_| form.left_unused)
    }

    /// The form of [`Self::act_bus_cable`] for `form`, the tag `proposed` for a new cable.
    fn bus_cable_dialog(&self, form: &ew_api::window::BusCableForm, proposed: Option<String>) -> FormDialog {
        let field = |name, label_key, options: Vec<(String, String)>, value: String| FormField {
            name,
            label_key,
            choices: &super::choices::NONE,
            options,
            value,
        };
        let cables = form
            .cables
            .iter()
            .map(|o| {
                let words = if o.made_for || form.bus.is_empty() {
                    o.words.clone()
                } else {
                    strings::format(key!("form-bus-cable-not-for"), &[&o.words, &form.bus])
                };
                (o.part.to_string(), words)
            })
            .chain([(
                NEW_BUS_PART.to_owned(),
                strings::text(key!("form-bus-cable-new-part")).to_owned(),
            )])
            .collect();
        // A new cable, or one of the project's bus cables the line runs on in (rc.5).
        let which = std::iter::once((
            NEW_CABLE.to_owned(),
            strings::text(key!("form-bus-cable-new")).to_owned(),
        ))
        .chain(form.existing.iter().map(|(id, w)| (id.to_string(), w.clone())))
        .collect();
        let mut fields = vec![
            field(
                "cable",
                key!("form-bus-cable-which"),
                which,
                form.cable.map_or_else(|| NEW_CABLE.to_owned(), |c| c.to_string()),
            ),
            field(
                "tag",
                key!("form-cable-tag"),
                Vec::new(),
                proposed.unwrap_or_default(),
            ),
            field(
                "part",
                key!("form-cable-part"),
                cables,
                form.part.map(|u| u.to_string()).unwrap_or_default(),
            ),
            field(
                "length",
                key!("form-bus-cable-length"),
                Vec::new(),
                form.length.clone(),
            ),
        ];
        let ends = [
            (
                "plug_a",
                "termination_a",
                key!("form-bus-plug-a"),
                key!("form-bus-termination-a"),
            ),
            (
                "plug_b",
                "termination_b",
                key!("form-bus-plug-b"),
                key!("form-bus-termination-b"),
            ),
        ];
        for ((plug, termination, plug_key, termination_key), end) in ends.into_iter().zip(&form.ends) {
            // Every choice names its end, which the label cannot (it is a text of the form).
            let options = std::iter::once((
                String::new(),
                strings::format(key!("form-bus-plug-none"), &[&end.words]),
            ))
            .chain(
                form.plugs
                    .iter()
                    .map(|o| (o.part.to_string(), format!("{}: {}", end.words, o.words))),
            )
            .chain([(
                NEW_BUS_PART.to_owned(),
                strings::format(key!("form-bus-plug-new"), &[&end.words]),
            )])
            .collect();
            let value = end.plug.map(|u| u.to_string()).unwrap_or_default();
            fields.push(field(plug, plug_key, options, value));
            fields.push(FormField {
                name: termination,
                label_key: termination_key,
                choices: &super::choices::BOOL,
                options: Vec::new(),
                value: (end.termination == Some(true)).to_string(),
            });
        }
        FormDialog {
            action: BUS_CABLE,
            title_key: key!("bar-cable"),
            ok_key: key!("dialog-apply"),
            fields,
            error: None,
            table: None,
        }
    }
}
