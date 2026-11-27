// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The creation actions that open a form or a settings form: the PLC export, a medium, a valve terminal, a station put on a bus page, the fluid and PLC settings, a fluid line's kind, an external reference, a list and a report block.

use super::{
    CHANNEL_SETTINGS, DRAW_PLUG, DRAW_SOLENOID, EXTERNAL_REF_NEW, FLUID_SETTINGS, MEDIUM_NEW, MEDIUM_PRESETS,
    MODULE_SAFETY, PLC_EXPORT, PROCESS_TAG_SCHEME, PUT_STATION, REPORT_NEW, SYMBOL_POLICY, TUBE_MATERIAL,
    VALVE_TERMINAL, medium_preset, report_kind_title, report_kind_words, report_kinds, section_of,
};
use crate::key;
use crate::shell::forms::{FormDialog, Spec, given_flag, given_text, text_param};
use crate::shell::{Dialog, Mode, Outcome, ShellModel, failed};
use crate::strings;
use crate::strings::Key;
use serde_json::{Map, Value, json};

impl ShellModel {
    /// `plc.export {format?, stations?, date?, path?}` (doc 10 §6): without a format the window opens the export form
    /// (its format offered by name); with one the file is written ([`ShellModel::act_plc_export_file`]).
    pub(crate) fn act_plc_export(&mut self, p: &Map<String, Value>) -> Outcome {
        let format = match text_param(p, "format") {
            Ok(f) => f,
            Err(e) => return e,
        };
        if format.trim().is_empty() {
            if self.mode == Mode::Gui && !self.in_form_of(PLC_EXPORT.action) {
                let mut dialog = PLC_EXPORT.dialog(p);
                if let Some(f) = dialog.fields.iter_mut().find(|f| f.name == "format") {
                    f.options = crate::shell::plc_exchange::PLC_FORMATS
                        .iter()
                        .map(|(w, name)| ((*w).to_owned(), (*name).to_owned()))
                        .collect();
                    crate::shell::plc_exchange::PLC_FORMATS[0]
                        .0
                        .clone_into(&mut f.value);
                }
                // The document date starts as today's, which the window read (headless: empty, the model's date).
                if let Some(f) = dialog
                    .fields
                    .iter_mut()
                    .find(|f| f.name == "date" && f.value.is_empty())
                    && let Some(today) = &self.today
                {
                    today.clone_into(&mut f.value);
                }
                dialog.ok_key = key!("dialog-export");
                return self.open_dialog(Dialog::Form(dialog));
            }
            return self.form_missing(&PLC_EXPORT, p, "format");
        }
        let out = self.act_plc_export_file(p, format.trim());
        match out {
            Outcome::Failed { message } => self.form_failed(PLC_EXPORT.action, message),
            other => other,
        }
    }

    /// `fluid.medium_new {preset?, name, p_nom, code?}` (FR-FLU-008): one `fluid.medium.edit` that makes a medium of
    /// that name, nominal pressure (bar) and ISO 1219-2 letter; a usual medium (`preset`, [`MEDIUM_PRESETS`]) gives the
    /// name and the pressure left out. In the window a missing name or pressure opens the form, whose first field
    /// offers the usual media.
    pub(crate) fn act_fluid_medium_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for name in ["preset", "name", "p_nom", "code"] {
            match text_param(p, name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [preset, mut name, mut p_nom, code] = <[String; 4]>::try_from(v).expect("four parameters");
        if !preset.is_empty() {
            let Some((words, pressure)) = medium_preset(&preset) else {
                return failed(key!("error-bad-param"), &["preset"]);
            };
            if name.is_empty() {
                name = words;
            }
            if p_nom.is_empty() {
                pressure.clone_into(&mut p_nom);
            }
        }
        for (field, value) in [("name", &name), ("p_nom", &p_nom)] {
            if value.trim().is_empty() {
                // A form opened afresh sets the medium on no row (the Medium row's New medium… names its row after).
                if p.is_empty() {
                    self.props.medium_for = None;
                }
                let out = self.form_missing(&MEDIUM_NEW, p, field);
                // The first field offers the usual media, in the UI language.
                if let Some(Dialog::Form(form)) = &mut self.dialog
                    && form.action == MEDIUM_NEW.action
                    && let Some(f) = form.fields.iter_mut().find(|f| f.name == "preset")
                {
                    f.options = MEDIUM_PRESETS
                        .iter()
                        .filter_map(|(w, _)| medium_preset(w).map(|(words, _)| ((*w).to_owned(), words)))
                        .collect();
                }
                return out;
            }
        }
        let pressure = p_nom.trim().replace(',', ".");
        if pressure.parse::<f64>().is_err() {
            return self.form_failed(
                MEDIUM_NEW.action,
                strings::format(key!("error-medium-pressure"), &[p_nom.trim()]),
            );
        }
        let mut set = json!({"name": name.trim(), "p_nom": pressure});
        if !code.trim().is_empty() {
            set["code"] = Value::String(code.trim().to_owned());
        }
        let before: Vec<_> = self
            .project_session()
            .map(|s| s.media().into_iter().map(|(id, _)| id).collect())
            .unwrap_or_default();
        let out = self.host_create("fluid.medium.edit", json!({ "set": set }));
        if out == Outcome::Done {
            self.notify(strings::format(key!("toast-medium-new"), &[name.trim()]), true);
        }
        let out = self.form_done(MEDIUM_NEW.action, out);
        // Made from a tube's Medium row (its New medium…): the tubes take the new medium.
        let made = self
            .project_session()
            .and_then(|s| s.media().into_iter().find(|(id, _)| !before.contains(id)));
        if let (Some(row), Some((id, name)), Outcome::Done) = (self.props.medium_for.take(), made, &out)
            && self.props_find(&row).is_some()
        {
            return self.commit_value(&row, Value::String(id.to_string()), name, Vec::new());
        }
        out
    }

    /// `fluid.valve_terminal {terminal?}` (Fluid ▸ Valve terminal…): on a fluid page a valve terminal of the project
    /// that is drawn on no fluid page yet (made as a station first) comes first: the form offers "Draw -104A1 here" for
    /// each, then "A new terminal…". `terminal` a terminal draws it here (`fluid.draw_terminal`); `new`, or no such
    /// terminal, opens the device browser at the valve terminals. From another page it works on the nearest fluid page,
    /// else on one made then ([`ShellModel::fluid_page_here`]).
    pub(crate) fn act_fluid_valve_terminal(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = match text_param(p, "terminal") {
            Ok(t) => t,
            Err(e) => return e,
        };
        let here = self.fluid_page_here();
        if here != Outcome::Done {
            return here;
        }
        let undrawn = self
            .project_session()
            .map(ew_api::Session::undrawn_fluid_terminals)
            .unwrap_or_default();
        if terminal.is_empty() && !undrawn.is_empty() && self.mode == Mode::Gui {
            let out = self.form_missing(&VALVE_TERMINAL, p, "terminal");
            if let Some(Dialog::Form(form)) = &mut self.dialog
                && form.action == VALVE_TERMINAL.action
                && let Some(f) = form.fields.iter_mut().find(|f| f.name == "terminal")
            {
                f.options = undrawn
                    .iter()
                    .map(|(id, tag)| {
                        (
                            id.to_string(),
                            strings::format(key!("valve-terminal-draw-here"), &[tag]),
                        )
                    })
                    .chain([(
                        "new".to_owned(),
                        strings::text(key!("valve-terminal-new")).to_owned(),
                    )])
                    .collect();
                f.value = undrawn[0].0.to_string();
            }
            return out;
        }
        // The form, if open, gives way to what was chosen.
        let _ = self.form_done(VALVE_TERMINAL.action, Outcome::Done);
        if terminal.is_empty() || terminal == "new" {
            return self.act_discipline_picker(crate::tools::PickerMode::Parts, "ValveTerminalNode");
        }
        self.dispatch("fluid.draw_terminal", &json!({ "terminal": terminal }))
    }

    /// `bus.put_station {station?, x_mm?, y_mm?, group?}` (Put a station on this page…): on a bus-topology page,
    /// every interface of the station or bus device `station` (its tag or ID) that the page does not draw yet, a box
    /// each, as the rack's Put it on a bus page puts it: on the pointer, or at the sheet point `x_mm`, `y_mm` (a row
    /// dropped on the page). `new` places what the asking placement waited with (a part from the device browser), else
    /// opens the black-box picker. Without `station` the window asks which.
    pub(crate) fn act_bus_put_station(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = match text_param(p, "station") {
            Ok(t) => t,
            Err(e) => return e,
        };
        // In words, as a failure: a row dropped on another page says why (the window shows a failure in its banner).
        if self.page_rows().get(self.page_index()).map(|r| &r.kind) != Some(&ew_api::PageType::BusTopology) {
            return Outcome::Failed {
                message: strings::text(key!("reason-put-station-bus-page")).to_owned(),
            };
        }
        if station.is_empty() {
            if self.stations_off_page(None).is_empty() {
                return Outcome::refused(key!("reason-no-station-off-page"));
            }
            if self.mode == Mode::Headless {
                return failed(key!("error-missing-param"), &["station"]);
            }
            return self.ask_put_station(None);
        }
        let _ = self.form_done(PUT_STATION.action, Outcome::Done);
        let pending = self.tools.put_station_part.take();
        if station == "new" {
            let Some((part, letter)) = pending else {
                return self.act_open_picker(crate::tools::PickerMode::BlackBox);
            };
            let Some(cat) = self.catalogue() else {
                return Outcome::refused(key!("reason-no-catalogue"));
            };
            let Some(pi) = cat.parts.iter().position(|r| r.copy == part) else {
                return Outcome::refused(key!("reason-unknown-symbol"));
            };
            return self.arm_part_ports(&cat, pi, letter);
        }
        let device = match self.join_device_ref(&station) {
            Ok(id) => id,
            Err(reason) => return failed(key!("error-join-device"), &[&station, &reason]),
        };
        let out = self.put_station_here(device);
        // A drop: the boxes' first corner at the drop point and the opposite one a little below to the right (the boxes
        // grow to hold their sockets).
        let (Some(x), Some(y)) = (
            p.get("x_mm").and_then(Value::as_f64),
            p.get("y_mm").and_then(Value::as_f64),
        ) else {
            return out;
        };
        if out != Outcome::Done {
            return out;
        }
        for (dx, dy) in [(0.0, 0.0), (40.0, 24.0)] {
            let mut corner = p.clone();
            corner.remove("station");
            corner.insert("x_mm".into(), json!(x + dx));
            corner.insert("y_mm".into(), json!(y + dy));
            for release in [false, true] {
                let out = self.act_press(&corner, release);
                if out != Outcome::Done {
                    return out;
                }
            }
        }
        Outcome::Done
    }

    /// Arms the black box tool with every interface of `device` that the bus-topology page in front does not draw yet,
    /// a box each; a device without a bus interface, or with all of them drawn there, is refused in words.
    pub(in crate::shell) fn put_station_here(&mut self, device: ew_api::output::Uuid) -> Outcome {
        let Some(folio) = self.current_folio().map(str::to_owned) else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let Some((tag, bus, interfaces)) = self.project_session().map(|s| {
            (
                s.device_tag(device).unwrap_or_default(),
                s.has_bus_interfaces(device),
                s.undrawn_interfaces(device, &folio),
            )
        }) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        if !bus {
            return Outcome::Failed {
                message: strings::format(key!("rack-no-bus-interface"), &[&tag]),
            };
        }
        if interfaces.is_empty() {
            return Outcome::Failed {
                message: strings::format(key!("rack-bus-drawn"), &[&tag, &folio]),
            };
        }
        let Some(cat) = self.catalogue() else {
            return Outcome::refused(key!("reason-no-catalogue"));
        };
        let labels: Vec<&str> = interfaces.iter().map(|i| i.label.as_str()).collect();
        let label = format!("{tag} {}", labels.join(", "));
        let target = json!({"device": {"device": device.to_string()}});
        self.arm_ports(&cat, target, label, &interfaces, None)
    }

    /// The form Put a station on this page: "Put -1A1 here" for each station or bus device the bus page does not draw
    /// yet (with `pending`, each PLC station of its part copy), the first chosen, then "A new station". `pending` is the
    /// placement that asked (the part and the letter given), which "A new station" goes on with.
    pub(in crate::shell) fn ask_put_station(
        &mut self,
        pending: Option<(ew_api::output::Uuid, Option<String>)>,
    ) -> Outcome {
        let offered = self.stations_off_page(pending.as_ref().map(|(part, _)| *part));
        self.tools.put_station_part = pending;
        let out = self.open_dialog(Dialog::Form(PUT_STATION.dialog(&Map::new())));
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && form.action == PUT_STATION.action
        {
            form.ok_key = key!("dialog-put-station");
            if let Some(f) = form.fields.iter_mut().find(|f| f.name == "station") {
                f.options = offered
                    .iter()
                    .map(|(id, tag)| (id.to_string(), strings::format(key!("put-station-here"), &[tag])))
                    .chain([(
                        "new".to_owned(),
                        strings::text(key!("put-station-new")).to_owned(),
                    )])
                    .collect();
                f.value = offered
                    .first()
                    .map_or_else(|| "new".to_owned(), |(id, _)| id.to_string());
            }
        }
        out
    }

    /// Brings the electrical page a multipin terminal's plug (`solenoid` false) or a valve's solenoid is drawn on in
    /// front (Draw plug went to page 1 without a word): the page `page` names (the folio of a
    /// multi-line page, or `new`: a new one); without, the page in front when it is one; else in the window the form asks
    /// which, the electrical pages and New page… (`target`, its words, stands in it as what is drawn), the next one after
    /// the page in front chosen; headless the nearest one (`none` without any). `Ok(true)`: that page is in front;
    /// `Ok(false)`: the form asks.
    pub(in crate::shell) fn electrical_page_for(
        &mut self,
        solenoid: bool,
        p: &Map<String, Value>,
        target: (ew_api::output::Uuid, String),
        none: Key<'static>,
    ) -> Result<bool, Outcome> {
        let spec = if solenoid { &DRAW_SOLENOID } else { &DRAW_PLUG };
        let page = text_param(p, "page")?;
        // The form, if open, gives way to what was chosen.
        let _ = self.form_done(spec.action, Outcome::Done);
        let pages: Vec<(usize, String, String)> = self
            .page_rows()
            .iter()
            .enumerate()
            .filter(|(_, r)| r.kind == ew_api::PageType::MultiLine)
            .map(|(i, r)| (i, r.folio.clone(), r.title.clone()))
            .collect();
        let folio = match page.as_str() {
            "new" => {
                let folio = self.free_folio("multi_line");
                let title = strings::text(key!("draw-page-new-title"));
                let made = self.dispatch(
                    "page.new",
                    &json!({ "folio": folio, "type": "multi_line", "title": title }),
                );
                if made != Outcome::Done {
                    return Err(made);
                }
                folio
            }
            "" if pages.iter().any(|(i, _, _)| *i == self.page_index()) => return Ok(true),
            "" if self.mode == Mode::Gui => {
                let here = self.page_index();
                let next = pages.iter().find(|(i, _, _)| *i > here).or(pages.last());
                let mut dialog = spec.dialog(p);
                dialog.ok_key = key!("dialog-draw");
                for f in &mut dialog.fields {
                    if f.name == "page" {
                        f.options = pages
                            .iter()
                            .map(|(_, folio, title)| {
                                let words = strings::format(key!("rack-page-choice"), &[folio, title]);
                                (folio.clone(), words.trim().to_owned())
                            })
                            .chain([(
                                "new".to_owned(),
                                strings::text(key!("rack-draw-new-page")).to_owned(),
                            )])
                            .collect();
                        f.value = next.map_or_else(|| "new".to_owned(), |(_, folio, _)| folio.clone());
                    } else {
                        f.options = vec![(target.0.to_string(), target.1.clone())];
                        f.value = target.0.to_string();
                    }
                }
                self.open_dialog(Dialog::Form(dialog));
                return Ok(false);
            }
            "" => {
                let out = self.show_page_of_kind(&ew_api::PageType::MultiLine, none);
                return if out == Outcome::Done { Ok(true) } else { Err(out) };
            }
            f if pages.iter().any(|(_, folio, _)| folio == f) => f.to_owned(),
            _ => return Err(failed(key!("error-bad-param"), &["page"])),
        };
        let out = self.dispatch("page.goto", &json!({ "folio": folio }));
        if out != Outcome::Done {
            return Err(out);
        }
        // The page change ends the tool now, not after a tool is armed on the new page.
        self.sync_tool_context();
        Ok(true)
    }

    /// The project settings member `member` as JSON (`Value::Null` when absent).
    pub(super) fn settings_member(&self, member: &str) -> Value {
        self.project_session()
            .and_then(|s| serde_json::to_value(&s.query().project().settings).ok())
            .and_then(|v| v.get(member).cloned())
            .unwrap_or(Value::Null)
    }

    /// Opens the settings form `spec` with `prefill` (the project's values), its button Apply; headless a settings
    /// action without its parameters fails.
    pub(super) fn open_settings_form(
        &mut self,
        spec: &'static Spec,
        prefill: Map<String, Value>,
        first: &str,
    ) -> Outcome {
        if self.mode == Mode::Headless {
            return failed(key!("error-missing-param"), &[first]);
        }
        let mut dialog = spec.dialog(&prefill);
        dialog.ok_key = key!("dialog-apply");
        if spec.action == FLUID_SETTINGS.action
            && let Some(f) = dialog.fields.iter_mut().find(|f| f.name == "material")
        {
            f.options = self.enum_options(TUBE_MATERIAL.0, TUBE_MATERIAL.1, &f.value);
        }
        self.open_dialog(Dialog::Form(dialog))
    }

    /// `plc.process_tag_scheme {scheme}` (Project ▸ Process-tag scheme…, FR-PLC-049): the project's process-tag scheme,
    /// one `project.settings.set`; an empty scheme removes it. Without `scheme` the window opens the form with the
    /// scheme the project has.
    pub(crate) fn act_process_tag_scheme(&mut self, p: &Map<String, Value>) -> Outcome {
        if !p.contains_key("scheme") && !self.in_form_of(PROCESS_TAG_SCHEME.action) {
            let mut prefill = Map::new();
            if let Value::String(s) = self.settings_member("process_tag_scheme") {
                prefill.insert("scheme".into(), Value::String(s));
            }
            return self.open_settings_form(&PROCESS_TAG_SCHEME, prefill, "scheme");
        }
        let scheme = match text_param(p, "scheme") {
            Ok(s) => s,
            Err(e) => return e,
        };
        let value = if scheme.is_empty() {
            Value::Null
        } else {
            Value::String(scheme)
        };
        let out = self.host_create(
            "project.settings.set",
            json!({"member": "process_tag_scheme", "value": value}),
        );
        self.form_done(PROCESS_TAG_SCHEME.action, out)
    }

    /// `fluid.settings {od, id, material, line_tags}` (Project ▸ Fluid settings…, doc 11 §2.4, §5.4): the default tube's
    /// outer and inner diameter (mm; empty removes it) and material, and whether fluid lines are tagged by page and
    /// counter, as one `project.settings.set` of the member `fluid` (its other values kept). Without parameters the
    /// window opens the form with the project's values.
    pub(crate) fn act_fluid_settings(&mut self, p: &Map<String, Value>) -> Outcome {
        let current = self.settings_member("fluid");
        if p.is_empty() && !self.in_form_of(FLUID_SETTINGS.action) {
            let tube = &current["default_tube"];
            let mm = |v: &Value| {
                v.as_i64()
                    .map(|nm| ew_api::output::mm_words(i128::from(nm), false))
                    .map(Value::String)
            };
            let mut prefill = Map::new();
            for (name, value) in [
                ("od", mm(&tube["od"])),
                ("id", mm(&tube["id"])),
                (
                    "material",
                    tube["material"].as_str().map(|m| Value::String(m.to_owned())),
                ),
                (
                    "line_tags",
                    Some(json!(current.get("line_tags").is_some().to_string())),
                ),
            ] {
                if let Some(v) = value {
                    prefill.insert(name.into(), v);
                }
            }
            return self.open_settings_form(&FLUID_SETTINGS, prefill, "od");
        }
        let mut fluid = current.as_object().cloned().unwrap_or_default();
        let mut tube = fluid
            .get("default_tube")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for name in ["od", "id"] {
            let text = match given_text(p, name) {
                Ok(Some(t)) => t,
                Ok(None) => continue,
                Err(e) => return e,
            };
            if text.is_empty() {
                tube.remove(name);
                continue;
            }
            match ew_api::output::parse_mm(&text).filter(|nm| *nm > 0) {
                Some(nm) => {
                    tube.insert(name.into(), json!(nm));
                }
                None => {
                    return self.form_failed(
                        FLUID_SETTINGS.action,
                        strings::format(key!("error-tube-size"), &[&text]),
                    );
                }
            }
        }
        // A material this version does not offer (one of a newer format) passes through unchanged.
        let stored = tube.get("material").and_then(Value::as_str).map(str::to_owned);
        match given_text(p, "material") {
            Ok(None) => {}
            Ok(Some(m)) if m.is_empty() => {
                tube.remove("material");
            }
            Ok(Some(m))
                if self
                    .enum_options(
                        TUBE_MATERIAL.0,
                        TUBE_MATERIAL.1,
                        stored.as_deref().unwrap_or_default(),
                    )
                    .iter()
                    .any(|(w, _)| *w == m) =>
            {
                tube.insert("material".into(), Value::String(m));
            }
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["material"]),
            Err(e) => return e,
        }
        if tube.is_empty() {
            fluid.remove("default_tube");
        } else {
            fluid.insert("default_tube".into(), Value::Object(tube.clone()));
        }
        match given_flag(p, "line_tags") {
            Some(true) => {
                fluid.insert("line_tags".into(), json!("page_counter"));
            }
            Some(false) => {
                fluid.remove("line_tags");
            }
            None => {}
        }
        // Apply shows its change: the toast says what the default tube is now and that the
        // tubes drawn already take it; without a material it says the tubes still lack one (EW-FL-007).
        let size = match (
            tube.get("od").and_then(Value::as_i64),
            tube.get("id").and_then(Value::as_i64),
        ) {
            (Some(od), Some(id)) => Some(format!(
                "{} × {} mm",
                ew_api::output::mm_words(i128::from(od), false),
                ew_api::output::mm_words(i128::from(id), false)
            )),
            _ => None,
        };
        let material = tube
            .get("material")
            .and_then(Value::as_str)
            .map(strings::choice_text);
        let toast = match (size, material) {
            (Some(size), Some(material)) => {
                strings::format(key!("toast-fluid-settings"), &[&format!("{size} {material}")])
            }
            (Some(size), None) => strings::format(key!("toast-fluid-settings-no-material"), &[&size]),
            _ => strings::text(key!("toast-fluid-settings-no-size")).to_owned(),
        };
        let value = if fluid.is_empty() {
            Value::Null
        } else {
            Value::Object(fluid)
        };
        let out = self.host_create("project.settings.set", json!({"member": "fluid", "value": value}));
        if out == Outcome::Done {
            self.notify(toast, true);
        }
        self.form_done(FLUID_SETTINGS.action, out)
    }

    /// The options of a settings form's choice of the enum field `path` of `kind`: none ("—"), then the format's values
    /// in words, read from the field's own type (one source); `current`, when this version does not offer it (a value of
    /// a newer format), as it is written, so that it passes through unchanged.
    pub(super) fn enum_options(&self, kind: &str, path: &str, current: &str) -> Vec<(String, String)> {
        let mut options = vec![(String::new(), "—".to_owned())];
        let words = self
            .project_session()
            .map(|s| s.field_choices(kind, path, ""))
            .unwrap_or_default();
        options.extend(words.into_iter().filter_map(|c| {
            let word = c.value.as_str()?.to_owned();
            Some((word.clone(), strings::choice_text(&word)))
        }));
        if !options.iter().any(|(w, _)| w == current) {
            options.push((current.to_owned(), current.to_owned()));
        }
        options
    }

    /// The record a row's settings form is for: parameter `name`, else the open form's own record. In the window, an
    /// action given the record alone opens its form with the record's values (`Session::plc_settings`) and `None` is
    /// returned with the outcome.
    pub(super) fn row_form(
        &mut self,
        spec: &'static Spec,
        p: &Map<String, Value>,
        name: &str,
        prefill: impl Fn(&Value) -> Map<String, Value>,
    ) -> Result<(ew_api::output::Uuid, Value), Outcome> {
        let open = self.in_form_of(spec.action);
        let id = match (p.contains_key(name), open) {
            (true, _) => self.record_param(p, name)?,
            (false, true) => self
                .form_target
                .ok_or_else(|| failed(key!("error-missing-param"), &[name]))?,
            (false, false) => return Err(failed(key!("error-missing-param"), &[name])),
        };
        let Some(current) = self.project_session().and_then(|s| s.plc_settings(id)) else {
            return Err(failed(key!("error-bad-param"), &[name]));
        };
        if self.mode == Mode::Gui && !open && p.len() == 1 {
            let mut dialog = spec.dialog(&prefill(&current));
            dialog.ok_key = key!("dialog-apply");
            for f in &mut dialog.fields {
                let path = match &*f.name {
                    "mode" => "mode",
                    "evaluation" => "safety.evaluation",
                    _ => continue,
                };
                f.options = self.enum_options("channel", path, &f.value);
            }
            self.form_target = Some(id);
            return Err(self.open_dialog(Dialog::Form(dialog)));
        }
        Ok((id, current))
    }

    /// `plc.symbol_policy {config, template?, tag_table?, strict_iec?}` (a configuration project row's Symbol policy…,
    /// FR-PLC-047): the configuration project's symbol policy, one `fields.set` of `symbols` (its other members kept).
    pub(crate) fn act_plc_symbol_policy(&mut self, p: &Map<String, Value>) -> Outcome {
        let (id, current) = match self.row_form(&SYMBOL_POLICY, p, "config", |c| {
            let mut m = Map::new();
            for name in ["template", "tag_table"] {
                if let Some(v) = c.get(name).filter(|v| v.is_string()) {
                    m.insert(name.into(), v.clone());
                }
            }
            let strict = c.get("strict_iec").and_then(Value::as_bool).unwrap_or(true);
            m.insert("strict_iec".into(), json!(strict.to_string()));
            m
        }) {
            Ok(x) => x,
            Err(out) => return out,
        };
        let mut value = current.as_object().cloned().unwrap_or_default();
        for name in ["template", "tag_table"] {
            match given_text(p, name) {
                Ok(None) => {}
                Ok(Some(t)) if t.is_empty() => {
                    value.remove(name);
                }
                Ok(Some(t)) => {
                    value.insert(name.into(), Value::String(t));
                }
                Err(e) => return e,
            }
        }
        if let Some(strict) = given_flag(p, "strict_iec") {
            value.insert("strict_iec".into(), json!(strict));
        }
        let out = self.host_create(
            "fields.set",
            json!({"kind": "plc_config_project", "targets": [id.to_string()], "path": "symbols",
                "value": Value::Object(value)}),
        );
        self.form_done(SYMBOL_POLICY.action, out)
    }

    /// `plc.module_safety {module, f_source?, f_dest?, f_wd_ms?}` (a module row's Safety data…, FR-PLC-041): the
    /// module's F-addresses and watchdog, one `fields.set` of `plc_module.safety` (its range kept); all three empty
    /// make the module a standard one again.
    pub(crate) fn act_plc_module_safety(&mut self, p: &Map<String, Value>) -> Outcome {
        let (id, current) = match self.row_form(&MODULE_SAFETY, p, "module", |c| {
            let mut m = Map::new();
            for name in ["f_source", "f_dest", "f_wd_ms"] {
                if let Some(n) = c.get(name).and_then(Value::as_u64) {
                    m.insert(name.into(), Value::String(n.to_string()));
                }
            }
            m
        }) {
            Ok(x) => x,
            Err(out) => return out,
        };
        let mut value = current.as_object().cloned().unwrap_or_default();
        for name in ["f_source", "f_dest", "f_wd_ms"] {
            let text = match given_text(p, name) {
                Ok(Some(t)) => t,
                Ok(None) => continue,
                Err(e) => return e,
            };
            if text.is_empty() {
                value.remove(name);
                continue;
            }
            match text.parse::<u16>() {
                Ok(n) => {
                    value.insert(name.into(), json!(n));
                }
                Err(_) => {
                    return self.form_failed(
                        MODULE_SAFETY.action,
                        strings::format(key!("error-safety-number"), &[&text]),
                    );
                }
            }
        }
        let value = if ["f_source", "f_dest", "f_wd_ms"]
            .iter()
            .any(|n| value.contains_key(*n))
        {
            Value::Object(value)
        } else {
            Value::Null
        };
        let out = self.host_create(
            "fields.set",
            json!({"kind": "device", "targets": [id.to_string()], "path": "plc_module.safety", "value": value}),
        );
        self.form_done(MODULE_SAFETY.action, out)
    }

    /// `plc.channel_settings {channel, mode?, evaluation?, requirement?}` (a channel row's Mode and safety…): a
    /// multi-function port's mode and the channel's safety evaluation and requirement, one `fields.set_many` of `mode`
    /// and `safety` (its pair and test-pulse pin kept); no evaluation makes it a standard channel.
    pub(crate) fn act_plc_channel_settings(&mut self, p: &Map<String, Value>) -> Outcome {
        let (id, current) = match self.row_form(&CHANNEL_SETTINGS, p, "channel", |c| {
            let mut m = Map::new();
            for (name, value) in [
                ("mode", &c["mode"]),
                ("evaluation", &c["safety"]["evaluation"]),
                ("requirement", &c["safety"]["requirement"]),
            ] {
                if let Some(s) = value.as_str() {
                    m.insert(name.into(), Value::String(s.to_owned()));
                }
            }
            m
        }) {
            Ok(x) => x,
            Err(out) => return out,
        };
        let text = |name: &str| given_text(p, name);
        let (mode, evaluation, requirement) = match (text("mode"), text("evaluation"), text("requirement")) {
            (Ok(m), Ok(e), Ok(r)) => (m, e, r),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        // A value left out keeps the stored one.
        let stored = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let evaluation = evaluation.unwrap_or_else(|| stored(&current["safety"]["evaluation"]));
        let requirement = requirement.unwrap_or_else(|| stored(&current["safety"]["requirement"]));
        let mode = mode.unwrap_or_else(|| stored(&current["mode"]));
        let safety = if evaluation.is_empty() {
            Value::Null
        } else {
            let mut s = current["safety"].as_object().cloned().unwrap_or_default();
            s.insert("evaluation".into(), Value::String(evaluation));
            if requirement.is_empty() {
                s.remove("requirement");
            } else {
                s.insert("requirement".into(), Value::String(requirement));
            }
            Value::Object(s)
        };
        let target = json!([id.to_string()]);
        let mode = if mode.is_empty() {
            Value::Null
        } else {
            Value::String(mode)
        };
        let out = self.host_create(
            "fields.set_many",
            json!({"edits": [
                {"kind": "channel", "targets": target, "path": "mode", "value": mode},
                {"kind": "channel", "targets": target, "path": "safety", "value": safety},
            ]}),
        );
        self.form_done(CHANNEL_SETTINGS.action, out)
    }

    /// `tool.fluid_line {line_kind}` (Draw ▸ Fluid line…, doc 11 §7): the Wire tool drawing fluid lines of a
    /// line kind on a fluid page. Without `line_kind` the window opens the line-kind picker (the kinds in words, working
    /// first).
    pub(crate) fn act_tool_fluid_line(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.wire_kind() != Ok(Some("fluid")) {
            return Outcome::refused(key!("reason-not-fluid-page"));
        }
        let kind = match text_param(p, "line_kind") {
            Ok(k) => k,
            Err(e) => return e,
        };
        // Without a kind the tool starts at once with the one used last (a dialog first caught the clicks meant
        // for the ports); Line kind in the HUD changes it.
        let kind = if kind.is_empty() {
            self.tools.tube_kind.unwrap_or("working")
        } else {
            match crate::strings::LINE_KINDS.iter().copied().find(|w| *w == kind) {
                Some(k) => k,
                None => return failed(key!("error-bad-param"), &["line_kind"]),
            }
        };
        self.tools.tube_kind = Some(kind);
        self.activate_tool(Box::new(
            crate::tools::WireTool::new()
                .with_kind(Some("fluid"))
                .with_line_kind(kind),
        ));
        // The first tube of a project without a default tube says where its size is set, once.
        if self.default_tube_mm().is_none() && !std::mem::replace(&mut self.tools.tube_size_said, true) {
            self.notify(strings::text(key!("toast-tube-no-size")).to_owned(), false);
        }
        Outcome::Done
    }

    /// `fluid.tube_kind` (the Tube tool's Line kind in the HUD): the next line kind, remembered; the Tube
    /// tool, while it draws no tube, takes it at once.
    pub(crate) fn act_fluid_tube_kind(&mut self, _p: &Map<String, Value>) -> Outcome {
        let kinds = crate::strings::LINE_KINDS;
        let now = self.tools.tube_kind.unwrap_or("working");
        let next = kinds
            .iter()
            .position(|w| *w == now)
            .map_or(kinds[0], |i| kinds[(i + 1) % kinds.len()]);
        self.tools.tube_kind = Some(next);
        if self.tools.tool.name() == "wire" && self.tools.tool.state() != "routing" {
            self.activate_tool(Box::new(
                crate::tools::WireTool::new()
                    .with_kind(Some("fluid"))
                    .with_line_kind(next),
            ));
        }
        Outcome::Done
    }

    /// The project's default tube as `outer/inner` millimetres (`6/4`); `None` without one.
    pub(super) fn default_tube_mm(&self) -> Option<String> {
        let tube = self.settings_member("fluid")["default_tube"].clone();
        let mm = |v: &Value| {
            v.as_i64()
                .map(|nm| ew_api::output::mm_words(i128::from(nm), false))
        };
        Some(format!("{}/{}", mm(&tube["od"])?, mm(&tube["id"])?))
    }

    /// What the Tube tool draws, as the HUD says it: its line kind and the project's default tube ("Working
    /// line · tube 6/4 mm"), or that the project has none and where it is set.
    pub(crate) fn tube_words(&self) -> String {
        let kind = self.tools.tube_kind.unwrap_or("working");
        let kind = strings::field_choice_text("connection", "line_kind", kind);
        match self.default_tube_mm() {
            Some(size) => strings::format(key!("hud-tube"), &[&kind, &size]),
            None => strings::format(key!("hud-tube-no-size"), &[&kind]),
        }
    }

    /// `external_ref.new {location, device, connection?, project?}`: one `external_ref.create` at a location
    /// node.
    pub(crate) fn act_external_ref_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in EXTERNAL_REF_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [location, device, connection, project] = <[String; 4]>::try_from(v).expect("four fields");
        for (name, value) in [("location", &location), ("device", &device)] {
            if value.is_empty() {
                return self.form_missing(&EXTERNAL_REF_NEW, p, name);
            }
        }
        let location = match self.structure_node("location", &location) {
            Ok(id) => id,
            Err(message) => return self.form_failed(EXTERNAL_REF_NEW.action, message),
        };
        let mut params = json!({ "location": location, "device": device });
        if !connection.is_empty() {
            params["connection"] = Value::String(connection.clone());
        }
        if !project.is_empty() {
            params["project"] = Value::String(project);
        }
        let out = self.host_create("external_ref.create", params);
        if out == Outcome::Done {
            // The label `ip.external` takes (item 8).
            let label = if connection.is_empty() {
                device.clone()
            } else {
                format!("{device}:{connection}")
            };
            self.notify(strings::format(key!("toast-external-ref-new"), &[&label]), true);
        }
        self.form_done(EXTERNAL_REF_NEW.action, out)
    }

    /// The form a new list prints on and the library copies to embed with it (`form_copies`, absent where the project
    /// holds the form): the set's own frame when it has one (`Session::new_list_form`, doc 09 §3.8), else the shipped
    /// frame without the grid with its title block. `None` without a project.
    pub(in crate::shell) fn list_form(&self) -> Option<(Value, Option<Value>)> {
        let project = self.project.as_ref()?;
        let own = project.session.new_list_form();
        let form = own
            .map(|id| Value::String(id.to_string()))
            .or_else(|| crate::wizard::shipped_form_id(crate::wizard_data::NOGRID_FRAME))?;
        let held = own.is_some()
            || form.as_str().is_some_and(|id| {
                project
                    .session
                    .query()
                    .forms()
                    .iter()
                    .any(|(f, _)| f.as_uuid().to_string() == id)
            });
        let keys = [crate::wizard_data::NOGRID_FRAME, crate::wizard_data::TITLE_BLOCK];
        let copies =
            (!held).then(|| Value::Array(crate::wizard::shipped_forms_lacking(&project.session, &keys)));
        Some((form, copies))
    }

    /// The first pages of the lists of report kind `kind` that print on pages of their own, each its folio and its
    /// page, in page order.
    pub(in crate::shell) fn list_pages_of(&self, kind: &str) -> Vec<(String, ew_api::output::Uuid)> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let rows = self.page_rows();
        let mut out: Vec<(usize, String, ew_api::output::Uuid)> = q
            .report_blocks()
            .iter()
            .filter(|(_, b)| b.embedded.is_none() && ew_api::output::block_kind(b) == kind)
            .filter_map(|(id, _)| {
                let first = ew_api::output::report_folios(&q, &id.to_string())
                    .ok()?
                    .into_iter()
                    .next()?;
                let at = rows.iter().position(|r| r.folio == first)?;
                Some((at, first, rows[at].id))
            })
            .collect();
        out.sort();
        out.into_iter().map(|(_, folio, page)| (folio, page)).collect()
    }

    /// The Report form's line under its Report field while a list of the kind chosen is already a page of the set:
    /// where, and that Create adds another; `None` otherwise.
    pub(in crate::shell) fn report_had_note(&self, form: &FormDialog) -> Option<String> {
        let kind = form.fields.iter().find(|f| f.name == "kind")?.value.clone();
        let (folio, _) = self.list_pages_of(&kind).into_iter().next()?;
        Some(strings::format(
            key!("form-report-had"),
            &[&report_kind_words(&kind), &folio],
        ))
    }

    /// `report_block.new {kind, folio, title?}` (Output ▸ Reports ▸ Report block…): one `report_block.create`
    /// of the report kind `kind` (its key, `tube_list`, or its name, `Tube list`) from folio `folio` on, titled `title` (else
    /// the report's name in every shipped language), on the set's own frame (`Session::new_list_form`, doc 09 §3.8), else
    /// on the shipped frame without the grid (embedded
    /// with its title block where the project does not hold it, as the company-standard wizard's blocks are), in the
    /// section whose folio range holds the folio. In the window without a kind or a folio its form opens, the first
    /// folio after the last page proposed.
    pub(crate) fn act_report_block_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in REPORT_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [kind, folio, title] = <[String; 3]>::try_from(v).expect("three fields");
        for (name, value) in [("kind", &kind), ("folio", &folio)] {
            if value.is_empty() {
                if self.mode == Mode::Gui && !self.in_form_of(REPORT_NEW.action) {
                    let next = self
                        .page_labels()
                        .iter()
                        .filter_map(|l| l.split('.').next()?.parse::<u32>().ok())
                        .max()
                        .map_or(1, |n| n.saturating_add(1));
                    let mut filled = p.clone();
                    if folio.is_empty() {
                        filled.insert("folio".into(), Value::String(next.to_string()));
                    }
                    let mut dialog = REPORT_NEW.dialog(&filled);
                    if let Some(f) = dialog.fields.iter_mut().find(|f| f.name == "kind") {
                        let kinds = report_kinds();
                        f.options = kinds
                            .iter()
                            .map(|k| ((*k).to_owned(), report_kind_words(k)))
                            .collect();
                        if f.value.is_empty()
                            && let Some(first) = kinds.first()
                        {
                            (*first).clone_into(&mut f.value);
                        }
                    }
                    return self.open_dialog(Dialog::Form(dialog));
                }
                return self.form_missing(&REPORT_NEW, p, name);
            }
        }
        // The kind by its key (`tube_list`) or by its name as the form and the refusal list it (`Tube list`).
        let kinds = report_kinds();
        let named = kinds
            .iter()
            .find(|k| **k == kind || report_kind_words(k).to_lowercase() == kind.trim().to_lowercase());
        let Some(kind) = named.map(|k| (*k).to_owned()) else {
            let words: Vec<String> = kinds.iter().map(|k| report_kind_words(k)).collect();
            return self.form_failed(
                REPORT_NEW.action,
                strings::format(key!("error-report-kind"), &[&kind, &words.join(", ")]),
            );
        };
        let Ok(number) = folio.parse::<u32>() else {
            return self.form_failed(
                REPORT_NEW.action,
                strings::format(key!("error-first-folio"), &[&folio]),
            );
        };
        let Some((form, copies)) = self.list_form() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let mut params = json!({
            "source": { "kind": kind },
            "form": form,
            "first_folio": number,
        });
        if let Some(copies) = copies {
            params["form_copies"] = copies;
        }
        // Without a title of its own the block is titled by its report's name, so its pages read "912 Tube list" in
        // the page tree, not a bare folio: in every language the program ships, so that its pages
        // print it in the project's language (doc 09 §3.8). A title typed stays as typed.
        params["title"] = if title.is_empty() {
            report_kind_title(&kind)
        } else {
            Value::String(title)
        };
        if let Some(id) = section_of(self, number) {
            params["section"] = Value::String(id);
        }
        let had = self.list_pages_of(&kind);
        let out = self.host_create("report_block.create", params);
        // A list of that kind already a page of the set:
        // the toast says where, with Go to page beside Undo.
        if out == Outcome::Done
            && let Some((folio_had, page)) = had.first()
        {
            let text = strings::format(
                key!("toast-report-block-had"),
                &[&report_kind_words(&kind), &folio, folio_had],
            );
            self.notify_undo_or_go_to(text, *page);
            return self.form_done(REPORT_NEW.action, out);
        }
        if out == Outcome::Done {
            // A block with nothing to list makes no page: the toast says so and what gives it rows.
            let empty = self.project_session().is_some_and(|s| {
                matches!(
                    ew_api::output::report_folios(&s.query(), &number.to_string()),
                    Err(ew_api::output::RowsError::NoPages(_))
                )
            });
            let words = report_kind_words(&kind);
            let text = if empty {
                let rows =
                    strings::lookup(&format!("report-rows-{}", kind.replace('_', "-"))).unwrap_or_default();
                strings::format(key!("toast-report-block-empty"), &[&words, &folio, rows])
            } else {
                strings::format(key!("toast-report-block-new"), &[&words, &folio])
            };
            self.notify(text, true);
            if matches!(kind.as_str(), "parts_list" | "parts_summary") {
                self.notify_unlisted();
            }
        }
        self.form_done(REPORT_NEW.action, out)
    }
}
