// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The creation actions of pages, sections and structure nodes: New page, New section, Edit section and New structure node.

use super::{
    PAGE_GRIDS, PAGE_NEW, PAGE_TYPES, SECTION_EDIT, SECTION_NEW, STRUCTURE_KINDS, STRUCTURE_NEW,
    page_grid_value, page_grid_word, parse_folio, section_of, strip_sign,
};
use crate::key;
use crate::shell::forms::{FormDialog, FormField, text_param};
use crate::shell::{Dialog, Mode, Outcome, ShellModel, failed};
use crate::strings;
use serde_json::{Map, Value, json};

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // The actions

    /// `page.new {folio, type?, title?, section?, grid?, at?}`: one `page.create` with the explicit folio and the
    /// connection grid given, else the current page's, then the new page is shown. Without a folio the dialog opens
    /// with the folio proposed after the Pages row `at` the menu was opened on, else after the open page
    /// ([`ShellModel::new_page_proposal`]).
    pub(crate) fn act_page_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in PAGE_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [folio, ty, title, section, place, grid] = <[String; 6]>::try_from(v).expect("six fields");
        // The grid of the page in front; with a station's view in front (no canvas), the page tab beside it.
        let current = self.canvas().map(|c| c.page().snapper.grid()).or_else(|| {
            let page = self.current_page()?;
            let rows = self.page_rows();
            let row = rows.iter().find(|r| crate::canvas::PageId::of(r) == page)?;
            self.project_session()?
                .page_grid(&row.folio)
                .map(|g| (g.num_nm, g.den))
        });
        let current_grid = current
            .and_then(|(num, den)| page_grid_word(num, den))
            .unwrap_or_default();
        if folio.is_empty() {
            // The dialog opens with the proposed folio and section filled in.
            if !self.in_form_of(PAGE_NEW.action) {
                // The Pages row the menu was opened on, when it was.
                let at = p.get("at").and_then(Value::as_str);
                let kind = if ty.is_empty() { "multi_line" } else { ty.as_str() };
                let (mut proposal, proposed_section) = self.new_page_proposal(at, Some(kind));
                if place == "end"
                    && let Some(end) = self.section_free_folio(&proposed_section)
                {
                    proposal = end.to_string();
                }
                self.tools.page_proposal = Some((proposal.clone(), at.map(str::to_owned)));
                let mut filled = p.clone();
                if !proposal.is_empty() {
                    filled.insert("folio".into(), Value::String(proposal));
                }
                if section.is_empty() && !proposed_section.is_empty() {
                    filled.insert("section".into(), Value::String(proposed_section));
                }
                if grid.is_empty() && !current_grid.is_empty() {
                    filled.insert("grid".into(), Value::String(current_grid.to_owned()));
                }
                if self.mode == Mode::Gui {
                    let out = self.open_dialog(Dialog::Form(PAGE_NEW.dialog(&filled)));
                    self.page_place_options();
                    return out;
                }
            }
            return self.form_missing(&PAGE_NEW, p, "folio");
        }
        let (number, label) = match parse_folio(&folio) {
            Ok(f) => f,
            Err(message) => return self.form_failed(PAGE_NEW.action, message),
        };
        let mut params = json!({ "number": number });
        if let Some(label) = label {
            params["label"] = Value::String(label);
        }
        // A page the project's frame defaults leave without a frame (an imported QElectroTech project) takes the
        // frame and title block of the page it is made from.
        let kind = if ty.is_empty() { "multi_line" } else { ty.as_str() };
        if let Some(form) = self
            .focused_page_id()
            .zip(self.project.as_ref())
            .and_then(|(page, project)| project.session.new_page_form(page, kind))
        {
            params["form"] = Value::String(form.to_string());
        }
        if !ty.is_empty() {
            if !PAGE_TYPES.contains(&ty.as_str()) {
                return self.form_failed(
                    PAGE_NEW.action,
                    strings::format(key!("error-page-type"), &[&ty, &PAGE_TYPES.join(", ")]),
                );
            }
            params["kind"] = Value::String(ty);
        }
        if !title.is_empty() {
            params["title"] = Value::String(title);
        }
        let grid = if grid.is_empty() {
            current_grid
        } else {
            grid.as_str()
        };
        if !grid.is_empty() {
            let Some(value) = page_grid_value(grid) else {
                return self.form_failed(
                    PAGE_NEW.action,
                    strings::format(key!("error-page-grid"), &[grid, &PAGE_GRIDS.join(", ")]),
                );
            };
            params["grid"] = value;
        } else if let Some((num, den)) = current {
            // A current page on a grid neither choice names (an imported one) still gives its grid; the
            // dialog's field is then empty.
            params["grid"] = json!({"num": num, "den": den});
        }
        if !section.is_empty() {
            let Some(project) = &self.project else {
                return Outcome::refused(key!("reason-no-project"));
            };
            match project.session.resolve_reference(&format!("section:{section}")) {
                Ok(id) => params["section"] = Value::String(id.to_string()),
                Err(why) => {
                    return self.form_failed(
                        PAGE_NEW.action,
                        crate::shell::tools::rejected_words("", Some(&why)),
                    );
                }
            }
        } else if let Some(id) = section_of(self, number) {
            // A new page belongs to the section whose folio range holds its number (window check, item 7): a sub-page
            // of 102 in the range 100–199 sits beside 102 in the navigator, not in the section-less group.
            params["section"] = Value::String(id);
        }
        let out = self.host_create("page.create", params);
        let out = self.form_done(PAGE_NEW.action, out);
        if out == Outcome::Done
            && let Some(id) = self.tools.last_primary.clone()
        {
            // The new page holds none of the old page's selection (item 4: here, not in `show`).
            self.tools.selection.clear();
            self.show_page_by_id(&id);
        }
        out
    }

    /// What New section proposes: the number after the highest section's, and the hundred of folios after the last
    /// folio any section holds (the sections of a drawing set are hundreds by convention: 100 to 199, 200 to 299).
    pub(super) fn new_section_proposal(&self) -> (u32, u32, u32) {
        let (mut number, mut end) = (0u32, 0u32);
        if let Some(project) = &self.project {
            for (_, s) in project.session.query().sections().iter() {
                number = number.max(u32::from(s.number));
                end = end.max(s.folio_range.1);
            }
        }
        let first = (end / 100 + 1) * 100;
        (number + 1, first, first + 99)
    }

    /// `section.new {title, first, last, number?}`: one `section.create` of the folio range `first` to `last`.
    /// Without a range the window opens the form with the number and the range proposed.
    pub(crate) fn act_section_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in SECTION_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [title, first, last, number] = <[String; 4]>::try_from(v).expect("four fields");
        let (next, from, to) = self.new_section_proposal();
        if first.is_empty() || last.is_empty() {
            if self.mode == Mode::Gui && !self.in_form_of(SECTION_NEW.action) {
                let mut filled = p.clone();
                for (name, typed, proposal) in [
                    ("first", &first, from),
                    ("last", &last, to),
                    ("number", &number, next),
                ] {
                    if typed.is_empty() {
                        filled.insert(name.into(), Value::String(proposal.to_string()));
                    }
                }
                return self.open_dialog(Dialog::Form(SECTION_NEW.dialog(&filled)));
            }
            let missing = if first.is_empty() { "first" } else { "last" };
            return self.form_missing(&SECTION_NEW, p, missing);
        }
        let whole = |name: &str, text: &str| {
            text.parse::<u32>()
                .map_err(|_| strings::format(key!("error-section-whole-number"), &[name, text]))
        };
        let parsed = whole("first", &first)
            .and_then(|a| Ok((a, whole("last", &last)?)))
            .and_then(|(a, b)| {
                let n = if number.is_empty() {
                    next
                } else {
                    whole("number", &number)?
                };
                let n = u16::try_from(n)
                    .map_err(|_| strings::format(key!("error-section-whole-number"), &["number", &number]))?;
                Ok((a, b, n))
            });
        let (first, last, number) = match parsed {
            Ok(v) => v,
            Err(message) => return self.form_failed(SECTION_NEW.action, message),
        };
        let mut params = json!({ "number": number, "folio_range": [first, last] });
        if !title.is_empty() {
            params["title"] = Value::String(title.clone());
        }
        let out = self.host_create("section.create", params);
        if out == Outcome::Done {
            self.notify(
                strings::format(
                    key!("toast-section-new"),
                    &[&number.to_string(), &title, &first.to_string(), &last.to_string()],
                ),
                true,
            );
        }
        self.form_done(SECTION_NEW.action, out)
    }

    /// The project's sections by number with their titles, for the links of the project's Properties.
    pub fn project_sections(&self) -> &[(u16, String)] {
        self.project.as_ref().map_or(&[], |p| p.sections.as_slice())
    }

    /// `section.edit {number, title?, first?, last?}` (Edit section… on a section of the Pages list, a section's link in
    /// Project properties): the section's title and folio range in one step, so a section of a running project is
    /// renamed and given its range there, not only in the template. Without a title or a range the window opens the
    /// form with the section's own; a range is whole numbers, the first not after the last.
    pub(crate) fn act_section_edit(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for name in ["number", "title", "first", "last"] {
            match text_param(p, name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [number, title, first, last] = <[String; 4]>::try_from(v).expect("four fields");
        let project = or_return!(crate::shell::gated(&self.project));
        let q = project.session.query();
        let found = number.parse::<u16>().ok().and_then(|n| {
            q.sections()
                .iter()
                .find(|(_, s)| s.number == n)
                .map(|(id, s)| (id.as_uuid().to_string(), s.folio_range))
        });
        let shown = project
            .sections
            .iter()
            .find(|(n, _)| n.to_string() == number)
            .map(|(_, t)| t.clone())
            .unwrap_or_default();
        let Some((id, (from, to))) = found else {
            let message = strings::format(key!("error-section-missing"), &[&number]);
            return self.form_failed(SECTION_EDIT, message);
        };
        let in_form = self.in_form_of(SECTION_EDIT);
        if !in_form && title.is_empty() && first.is_empty() && last.is_empty() {
            if self.mode != Mode::Gui {
                return failed(key!("error-missing-param"), &["title"]);
            }
            let field = |name: &'static str, label_key, value: String| FormField {
                name,
                label_key,
                choices: &crate::shell::choices::NONE,
                options: Vec::new(),
                value,
            };
            return self.open_dialog(Dialog::Form(FormDialog {
                action: SECTION_EDIT,
                title_key: key!("edit-section-title"),
                ok_key: key!("dialog-apply"),
                fields: vec![
                    field("title", key!("form-title"), shown),
                    field("first", key!("form-section-first"), from.to_string()),
                    field("last", key!("form-section-last"), to.to_string()),
                    field("number", key!("form-section"), number),
                ],
                error: None,
                table: None,
            }));
        }
        let whole = |name: &str, text: &str, kept: u32| {
            if text.is_empty() {
                return Ok(kept);
            }
            text.parse::<u32>()
                .map_err(|_| strings::format(key!("error-section-whole-number"), &[name, text]))
        };
        let range = whole("first", &first, from).and_then(|a| Ok((a, whole("last", &last, to)?)));
        let (a, b) = match range {
            Ok((a, b)) if a <= b => (a, b),
            Ok((a, b)) => {
                let message = strings::format(key!("error-section-range"), &[&a.to_string(), &b.to_string()]);
                return self.form_failed(SECTION_EDIT, message);
            }
            Err(message) => return self.form_failed(SECTION_EDIT, message),
        };
        let edit = |path: &str, value: Value| json!({"kind": "section", "targets": [id], "path": path, "value": value});
        let mut edits = vec![edit("folio_range", json!([a, b]))];
        if !title.is_empty() || in_form {
            edits.push(edit("title", Value::String(title)));
        }
        let cmd = ew_api::AnyCommand {
            name: "fields.set_many".to_owned(),
            params: json!({ "edits": edits }),
        };
        let out = self.run_commit(&cmd);
        self.form_done(SECTION_EDIT, out)
    }

    /// The parent the New structure node dialog proposes for a node of aspect `kind`: the
    /// first marked node of the Structure identifiers navigator (else its cursor row) when that node's aspect is `kind`,
    /// by its designation when the designation names it, else by its ID; empty otherwise.
    pub fn new_structure_parent(&self, kind: &str) -> String {
        let Some(nav) = self.nav(crate::navigators::NavKind::Structure) else {
            return String::new();
        };
        let prefix = format!("aspect:{kind}/");
        let pick = nav
            .marked()
            .iter()
            .find(|k| k.starts_with(&prefix))
            .cloned()
            .or_else(|| nav.cursor_key().filter(|k| k.starts_with(&prefix)));
        let Some((_, node)) = pick.as_deref().and_then(|k| nav.node_of(k)) else {
            return String::new();
        };
        if node.role != crate::navigators::NavRole::StructureNode {
            return String::new();
        }
        let Some(id) = node.records.first().map(ToString::to_string) else {
            return String::new();
        };
        match self.structure_node(kind, &node.label) {
            Ok(found) if found == id => node.label.clone(),
            _ => id,
        }
    }

    /// `structure.new {kind, designation, parent?}`: one `structure.create_node`.
    pub(crate) fn act_structure_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in STRUCTURE_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [kind, designation, parent] = <[String; 3]>::try_from(v).expect("three fields");
        for (name, value) in [("kind", &kind), ("designation", &designation)] {
            if value.is_empty() {
                // The dialog opens with the marked structure node as the parent
                // when its aspect is the chosen kind's.
                if self.mode == Mode::Gui && !self.in_form_of(STRUCTURE_NEW.action) && parent.is_empty() {
                    let chosen = if kind.is_empty() {
                        STRUCTURE_NEW.fields[0].default
                    } else {
                        kind.as_str()
                    };
                    let proposal = self.new_structure_parent(chosen);
                    if !proposal.is_empty() {
                        let mut filled = p.clone();
                        filled.insert("parent".into(), Value::String(proposal));
                        return self.open_dialog(Dialog::Form(STRUCTURE_NEW.dialog(&filled)));
                    }
                }
                return self.form_missing(&STRUCTURE_NEW, p, name);
            }
        }
        if !STRUCTURE_KINDS.contains(&kind.as_str()) {
            return self.form_failed(
                STRUCTURE_NEW.action,
                strings::format(
                    key!("error-structure-kind"),
                    &[&kind, &STRUCTURE_KINDS.join(", ")],
                ),
            );
        }
        let designation = match strip_sign(&kind, &designation) {
            Ok(d) if !d.is_empty() => d,
            Ok(_) => return self.form_missing(&STRUCTURE_NEW, p, "designation"),
            Err(message) => return self.form_failed(STRUCTURE_NEW.action, message),
        };
        let mut params = json!({ "aspect": kind, "code": designation });
        if !parent.is_empty() {
            match self.structure_node(&kind, &parent) {
                Ok(id) => params["parent"] = Value::String(id),
                Err(message) => return self.form_failed(STRUCTURE_NEW.action, message),
            }
        }
        let out = self.host_create("structure.create_node", params);
        if out == Outcome::Done {
            self.notify(
                strings::format(key!("toast-structure-new"), &[&kind, &designation]),
                true,
            );
        }
        self.form_done(STRUCTURE_NEW.action, out)
    }
}
