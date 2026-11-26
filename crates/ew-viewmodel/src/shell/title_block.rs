// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Title block fields… (`page.title_block`): every field the focused page's title block prints from the
//! project's data, in one form, written where its value comes from in one undo step: the page's title and description,
//! the project's master data, its parties' names, the open revision's label and the names of the parties the current
//! revision names as author and checker. A project without a revision gets one with the label typed
//! (`revision.create`, a second step); an open revision without an author gets one (`revision.set_author`). What a title block prints as fixed text of its form is not
//! a field: an imported QElectroTech template's own texts stay as the form holds them. Nor is the project's edited
//! date, which the save sets: the form says it in a line over the fields.

use ew_api::AnyCommand;
use serde_json::{Map, Value, json};

use super::{Dialog, FormDialog, FormField, Outcome, ShellModel, string_param};
use crate::key;
use crate::properties::{PropRow, PropertySheet, RowAction, RowValue};
use crate::strings::{self, Key};

/// The form's action.
const TITLE_BLOCK: &str = "page.title_block";

/// Where a title-block path's value is written.
#[derive(Clone, Copy)]
enum Source {
    /// A multi-language member of the page.
    PageText(&'static str),
    /// The page's description: its override when it has one, else the project's description.
    Description,
    /// A multi-language member of the project row.
    ProjectText(&'static str),
    /// An identifier member of the project row; empty removes it.
    ProjectIdent(&'static str),
    /// A date member of the project row, shown and typed in the project's date format; empty removes it.
    ProjectDate(&'static str),
    /// The project's edited date: set when the project is saved with changes, so a line over the fields, not a field.
    Edited,
    /// The name of the first party of a role (its serde word); a party of that role is added for a new name.
    Party(&'static str),
    /// The open revision's label, or a new revision's when the project has none.
    RevisionLabel,
    /// The name of the party the current revision names as its author; an open revision that names none
    /// takes the party of the role `author`, named as typed (`revision.set_author`, a second step).
    RevisionAuthor,
    /// The name of the party the current revision names as its checker; only while it names one (the checker is
    /// set when the revision is approved, Check ▸ Revision ▸ Approve).
    RevisionChecker,
    /// A value the program derives or another place sets (the folio, the supply, the revision's date): a cell shows
    /// it, Title block fields… does not write it.
    Derived,
}

/// The party role an open revision without an author takes for the author's name typed in the form.
const AUTHOR_ROLE: &str = "author";

/// The title-block paths a cell shows (the Shows chooser of Edit frame, doc 09 §3.9) and the form edits, each with its
/// label and its source (doc 09 §2.6's paths): the one list. Its group is its path's scope ([`shows_group`]).
const FIELDS: &[(&str, Key<'static>, Source)] = &[
    ("project.supply", key!("field-word-supply"), Source::Derived),
    ("project.notice", key!("shows-notice"), Source::Derived),
    ("page.folio", key!("field-word-number"), Source::Derived),
    ("page.prev", key!("shows-previous"), Source::Derived),
    ("page.next", key!("shows-next"), Source::Derived),
    ("page.function", key!("shows-function"), Source::Derived),
    ("page.location", key!("shows-location"), Source::Derived),
    ("page.scale", key!("field-word-scale"), Source::Derived),
    ("page.last_changed_in", key!("shows-changed-in"), Source::Derived),
    ("doc.page_index", key!("field-word-position"), Source::Derived),
    ("doc.page_count", key!("shows-page-count"), Source::Derived),
    ("revision.current.date", key!("field-word-date"), Source::Derived),
    ("revision.current.state", key!("shows-state"), Source::Derived),
    ("page.title", key!("column-title"), Source::PageText("title")),
    (
        "page.description",
        key!("field-word-description"),
        Source::Description,
    ),
    (
        "project.description",
        key!("titleblock-project-description"),
        Source::ProjectText("master.description"),
    ),
    (
        "project.long_description",
        key!("field-word-long-description"),
        Source::ProjectText("master.long_description"),
    ),
    (
        "project.document_title",
        key!("field-word-document-title"),
        Source::ProjectText("master.document_title"),
    ),
    (
        "project.reference",
        key!("field-word-project-ref"),
        Source::ProjectIdent("master.project_ref"),
    ),
    (
        "project.drawing_id",
        key!("field-word-drawing-id"),
        Source::ProjectIdent("master.drawing_id"),
    ),
    (
        "project.object",
        key!("field-word-object-id"),
        Source::ProjectIdent("master.object_id"),
    ),
    (
        "project.created",
        key!("field-word-created"),
        Source::ProjectDate("master.created"),
    ),
    ("project.edited", key!("field-word-edited"), Source::Edited),
    (
        "party.customer.name",
        key!("choice-customer"),
        Source::Party("customer"),
    ),
    (
        "party.end_user.name",
        key!("choice-end-user"),
        Source::Party("end_user"),
    ),
    (
        "party.panel_builder.name",
        key!("choice-panel-builder"),
        Source::Party("panel_builder"),
    ),
    (
        "party.author.name",
        key!("props-designer"),
        Source::Party("author"),
    ),
    (
        "party.checker.name",
        key!("choice-checker"),
        Source::Party("checker"),
    ),
    (
        "party.engineer.name",
        key!("choice-engineer"),
        Source::Party("engineer"),
    ),
    (
        "revision.current.label",
        key!("titleblock-revision"),
        Source::RevisionLabel,
    ),
    (
        "revision.current.author.name",
        key!("titleblock-author"),
        Source::RevisionAuthor,
    ),
    (
        "revision.current.checker.name",
        key!("titleblock-checked"),
        Source::RevisionChecker,
    ),
];

/// The group of the Shows chooser a path stands in, by its scope: Project, Page (the page and the document), Revision,
/// People.
pub(crate) fn shows_group(path: &str) -> Key<'static> {
    match path.split('.').next() {
        Some("page" | "doc") => key!("shows-group-page"),
        Some("revision") => key!("shows-group-revision"),
        Some("party") => key!("shows-group-people"),
        _ => key!("shows-group-project"),
    }
}

/// The parts of a person's address a cell shows, after the name: its member and its label.
const ADDRESS: &[(&str, Key<'static>)] = &[
    ("street", key!("field-word-street")),
    ("postcode", key!("field-word-postcode")),
    ("city", key!("field-word-city")),
    ("country", key!("field-word-country")),
];

/// The paths a cell's Shows offers in words, in their groups' order (Project, Page, Revision, People): each path of
/// [`FIELDS`] as "Project · Drawing number", each person's name and address parts as "People · Customer ▸ City".
pub(crate) fn shows_paths() -> Vec<(String, String)> {
    let group = |path: &str| strings::text(shows_group(path));
    let mut out: Vec<(String, String)> = Vec::new();
    for g in ["project", "page", "revision"] {
        for (path, label, source) in FIELDS {
            let scope = path.split('.').next().unwrap_or_default();
            let here = scope == g || (g == "page" && scope == "doc");
            if here && !matches!(source, Source::Party(_)) {
                out.push((
                    (*path).to_owned(),
                    format!("{} \u{b7} {}", group(path), strings::text(*label)),
                ));
            }
        }
    }
    for (role, label) in people() {
        let path = format!("party.{role}.name");
        let who = strings::text(label);
        out.push((
            path.clone(),
            format!(
                "{} \u{b7} {who} \u{25b8} {}",
                group(&path),
                strings::text(key!("shows-name"))
            ),
        ));
        for (part, part_label) in ADDRESS {
            let path = format!("party.{role}.address.{part}");
            out.push((
                path.clone(),
                format!(
                    "{} \u{b7} {who} \u{25b8} {}",
                    group(&path),
                    strings::text(*part_label)
                ),
            ));
        }
    }
    out
}

/// Gives the first party of `role` (its serde word) the name `text`; a party of that role is added for a new name.
fn name_party(parties: &mut Vec<Value>, role: &str, text: String) {
    match parties.iter_mut().find(|party| party["role"] == role) {
        Some(party) => party["name"] = Value::String(text),
        None if text.is_empty() => {}
        None => parties.push(json!({"role": role, "name": text})),
    }
}

/// The people of a project the title block can print, in the order of [`FIELDS`]: each party role (its serde word) with
/// its label, named as the cell that prints it (Author), else by its role (Checker, Customer…).
fn people() -> impl Iterator<Item = (&'static str, Key<'static>)> {
    FIELDS.iter().filter_map(|(_, label, source)| match source {
        Source::Party(role) => Some((*role, *label)),
        _ => None,
    })
}

/// The row key of a person of the project's Properties (`project/people.<role>`).
const PEOPLE: &str = "project/people.";

impl ShellModel {
    /// The project's sheet gets one plain row per person the title block can print (Designer, Checker, Customer…) at
    /// the head of Main, typed as a name; the list of parties, with addresses and personal data, waits under More (no raw JSON box
    /// stands where a person looks for the designer).
    pub(super) fn people_in_rows(sheet: &mut PropertySheet) {
        for section in sheet.sections.iter_mut().filter(|s| s.kind == "project") {
            let Some(list) = section.rows.iter().find(|r| r.path == "parties").cloned() else {
                continue;
            };
            let parties = match &list.value {
                RowValue::One { json, .. } => json.as_array().cloned().unwrap_or_default(),
                _ => Vec::new(),
            };
            let at = section.rows.iter().position(|r| r.path == "parties").unwrap_or(0);
            let rows: Vec<PropRow> = people()
                .map(|(role, _)| {
                    let name = parties
                        .iter()
                        .find(|p| p["role"] == role)
                        .map(|p| match &p["name"] {
                            Value::String(s) => s.clone(),
                            Value::Object(m) => {
                                m.values().find_map(Value::as_str).unwrap_or_default().to_owned()
                            }
                            _ => String::new(),
                        })
                        .unwrap_or_default();
                    PropRow {
                        key: format!("{PEOPLE}{role}"),
                        path: format!("people.{role}"),
                        depth: 0,
                        ty: crate::properties::FieldType::Text,
                        value: RowValue::One {
                            json: Value::String(name.clone()),
                            text: name,
                        },
                        actions: vec![RowAction::Edit],
                        identifier: false,
                        source: None,
                        named_reference: false,
                        ..list.clone()
                    }
                })
                .collect();
            section.rows.splice(at..at, rows);
        }
    }

    /// The label of a person's row ([`ShellModel::people_in_rows`]); `None` for another row.
    pub fn person_label(key: &str) -> Option<&'static str> {
        let role = key.strip_prefix(PEOPLE)?;
        people()
            .find(|(r, _)| *r == role)
            .map(|(_, label)| strings::text(label))
    }

    /// A name typed into a person's row: the party of that role named so, in one `fields.set` of the parties.
    pub(super) fn commit_person(&mut self, key: &str, text: &str) -> Option<Outcome> {
        let role = key.strip_prefix(PEOPLE)?;
        let project = self.project.as_ref()?;
        let q = project.session.query();
        let record = q.project();
        let mut parties = serde_json::to_value(&record.parties)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        let id = record.id.as_uuid().to_string();
        name_party(&mut parties, role, text.trim().to_owned());
        let cmd = AnyCommand {
            name: "fields.set".to_owned(),
            params: json!({"kind": "project", "targets": [id], "path": "parties", "value": parties}),
        };
        let out = self.run_commit(&cmd);
        match &out {
            Outcome::Failed { message } => self.error_under_row(key.to_owned(), message.clone()),
            _ => {
                self.props.edit = None;
                self.props.error = None;
            }
        }
        Some(out)
    }
}

/// One edit of `fields.set_many`.
fn edit(kind: &str, target: &str, path: &str, value: Value) -> Value {
    json!({"kind": kind, "targets": [target], "path": path, "value": value})
}

impl ShellModel {
    /// Whether this session may name a revision's author (`revision.set_author` is a person's step at the window).
    fn may_set_author(&self) -> bool {
        let origin = self.origin();
        self.project
            .as_ref()
            .is_some_and(|p| p.session.may_run("revision.set_author", &origin))
    }

    /// The form's fields for the focused page: the editable paths its title block prints, in its order, with their text;
    /// the revision label only while it can be written (an open revision, or none yet); the fields of the project's own
    /// it prints by their names, the page's first (doc 09 §2.10). `None` without a page.
    fn title_block_fields(&self) -> Option<(String, ew_api::output::TitleBlockFacts, Vec<FormField>)> {
        let page = self.focused_page_id()?;
        let facts = self.project.as_ref()?.session.title_block_facts(page)?;
        let own_field = |path: &str| {
            let own = facts.own_fields.iter().find(|o| o.path == path)?;
            let text = facts
                .fields
                .iter()
                .find(|(p, _)| p == path)
                .map(|(_, t)| t.clone());
            Some((
                own.page,
                FormField {
                    label: Some(strings::render(&ew_api::Text::bare(ew_api::TextArg::Ml(
                        own.label.clone(),
                    )))),
                    name: path.to_owned().into(),
                    label_key: key!("titleblock-own-field"),
                    choices: &super::choices::NONE,
                    options: Vec::new(),
                    value: text.unwrap_or_default(),
                },
            ))
        };
        let own_pages = facts
            .own_fields
            .iter()
            .filter(|o| o.page)
            .filter_map(|o| own_field(&o.path).map(|(_, f)| f));
        let rest = facts.fields.iter().filter_map(|(path, text)| {
            if let Some((page, field)) = own_field(path) {
                return (!page).then_some(field);
            }
            let (name, label_key, source) = FIELDS.iter().find(|(p, _, _)| p == path)?;
            let writable = match source {
                Source::RevisionLabel => facts.open_revision.is_some() || !facts.has_revisions,
                // A released revision's author is history: its party's name only, while it names one. An open
                // revision without one gets it by `revision.set_author`, which only a person at the window runs: a
                // session that may not leaves the field out and says so under the form (`form_foot`), so Apply
                // never applies half.
                Source::RevisionAuthor => {
                    facts.revision_author.is_some()
                        || (facts.open_revision.is_some() && self.may_set_author())
                }
                Source::RevisionChecker => facts.revision_checker.is_some(),
                Source::Edited | Source::Derived => false,
                _ => true,
            };
            writable.then(|| FormField {
                label: None,
                name: (*name).into(),
                label_key: *label_key,
                choices: &super::choices::NONE,
                options: Vec::new(),
                value: text.clone(),
            })
        });
        let fields: Vec<FormField> = own_pages.chain(rest).collect();
        Some((page.to_string(), facts, fields))
    }

    /// The Title block fields form's line over its fields: that the texts a template writes itself stay as
    /// it holds them.
    pub fn title_block_notes(&self) -> Vec<String> {
        let Some((_, facts, _)) = self.title_block_fields() else {
            return Vec::new();
        };
        let mut notes = Vec::new();
        if facts.fixed_texts {
            notes.push(strings::text(key!("titleblock-note-fixed")).to_owned());
        }
        // The edited date the title block prints, which the save sets.
        let edited = FIELDS
            .iter()
            .find(|(_, _, s)| matches!(s, Source::Edited))
            .and_then(|(path, label, _)| {
                facts
                    .fields
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, t)| (label, t))
            });
        if let Some((label, text)) = edited {
            notes.push(match text.as_str() {
                "" => strings::format(key!("titleblock-note-edited-none"), &[strings::text(*label)]),
                date => strings::format(key!("titleblock-note-edited"), &[strings::text(*label), date]),
            });
        }
        notes
    }

    /// The line under field `field` of the open form `form`: the form's own ([`FormDialog::note`]), New page's place
    /// under its folio ([`ShellModel::new_page_place`]), and the Title
    /// block fields form's rule of the revision label before Apply (a "B" must not be refused
    /// only after Apply): the project's form by the next free label, and where the form is set.
    pub fn form_note(&self, form: &FormDialog, field: &str) -> Option<String> {
        if let Some(note) = form.note(field) {
            return Some(note.to_owned());
        }
        if form.action == "page.new" && field == "folio" {
            return self.new_page_place(form);
        }
        if form.action == "report_block.new" && field == "kind" {
            return self.report_had_note(form);
        }
        if form.action != TITLE_BLOCK {
            return None;
        }
        let (_, facts, _) = self.title_block_fields()?;
        // A field of the project's own: whose value it is.
        let own = facts.own_fields.iter().find(|o| o.path == field).map(|o| {
            strings::text(if o.page {
                key!("titleblock-own-page")
            } else {
                key!("titleblock-own-project")
            })
            .to_owned()
        });
        // Which cell of the sheet prints the field (so that a person can tell which field feeds which cell).
        let cell = facts
            .cells
            .iter()
            .find(|(p, _)| p == field)
            .map(|(_, label)| strings::format(key!("titleblock-prints-in"), &[label]));
        let revision = (field == "revision.current.label")
            .then_some(facts.revision_next)
            .flatten()
            .map(|next| {
                strings::format(
                    key!("titleblock-revision-note"),
                    &[
                        &next,
                        strings::text(key!("action-props-project")),
                        strings::text(key!("field-word-revision-label")),
                    ],
                )
            });
        let notes: Vec<String> = own.into_iter().chain(cell).chain(revision).collect();
        (!notes.is_empty()).then(|| notes.join(" "))
    }

    /// The line at the foot of the open form `form`, under its last field: the Title block fields form's word on
    /// Checked where the page's title block prints a Checked cell that the current revision fills from no checker
    /// yet, so the field is not in the form (the line must not stand first, over fields that
    /// have no Checked). `None` for another form.
    pub fn form_foot(&self, form: &FormDialog) -> Option<String> {
        if form.action == "page.fit_grid" {
            return self.grid_fit_foot(form);
        }
        if form.action != TITLE_BLOCK {
            return None;
        }
        let (_, facts, _) = self.title_block_fields()?;
        let prints = |p: &str| facts.fields.iter().any(|(path, _)| path == p);
        let checker =
            (prints("revision.current.checker.name") && facts.revision_checker.is_none()).then(|| {
                strings::format(
                    key!("titleblock-note-checked"),
                    &[
                        strings::text(key!("action-revision-approve")),
                        strings::text(key!("menu-check")),
                    ],
                )
            });
        // The author this session may not set, said before Apply rather than refused after a half-applied one.
        let author = (prints("revision.current.author.name")
            && facts.revision_author.is_none()
            && facts.open_revision.is_some()
            && !self.may_set_author())
        .then(|| strings::text(key!("titleblock-note-author-person")).to_owned());
        let notes: Vec<String> = author.into_iter().chain(checker).collect();
        (!notes.is_empty()).then(|| notes.join(" "))
    }

    /// `page.title_block {<path>: text, …}` (Page ▸ Title block fields…): without a field the window opens the
    /// form of the focused page's title-block fields; its Apply (or the parameters) writes each changed field where its
    /// value comes from in one `fields.set_many`, and a revision label typed in a project without a revision makes
    /// one (`revision.create`). In the form a field left empty clears it; given as parameters, a field not named
    /// keeps its value. A title block that prints no field of the project's data is refused in words.
    pub(crate) fn act_page_title_block(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some((page, facts, fields)) = self.title_block_fields() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        if fields.is_empty() {
            return Outcome::refused(key!("reason-titleblock-fixed"));
        }
        let in_form = self.in_form_of(TITLE_BLOCK);
        let named = fields.iter().any(|f| p.contains_key(f.name.as_ref()));
        if !in_form && !named {
            return self.open_dialog(Dialog::Form(FormDialog {
                action: TITLE_BLOCK,
                title_key: key!("titleblock-title"),
                ok_key: key!("dialog-apply"),
                fields,
                error: None,
                table: None,
            }));
        }
        let project = facts.project.to_string();
        let mut parties = facts.parties.as_array().cloned().unwrap_or_default();
        let mut parties_changed = false;
        let mut edits = Vec::new();
        let mut new_revision = None;
        let mut new_author = None;
        let mut name_party = |role: &str, text: String| name_party(&mut parties, role, text);
        // The values of the fields of the project's own, each list written whole (doc 09 §2.10).
        let props = |v: &Value| v.as_array().cloned().unwrap_or_default();
        let (mut page_props, mut project_props) = (props(&facts.page_props), props(&facts.project_props));
        let (mut page_props_changed, mut project_props_changed) = (false, false);
        for f in &fields {
            let text = match string_param(p, &f.name) {
                Err(e) => return e,
                Ok(Some(t)) => t.trim().to_owned(),
                Ok(None) if in_form => String::new(),
                Ok(None) => continue,
            };
            if text == f.value {
                continue;
            }
            if let Some(own) = facts.own_fields.iter().find(|o| o.path == f.name) {
                let (list, changed) = if own.page {
                    (&mut page_props, &mut page_props_changed)
                } else {
                    (&mut project_props, &mut project_props_changed)
                };
                list.retain(|v| v["key"] != own.key.as_str());
                if !text.is_empty() {
                    list.push(json!({"key": own.key, "value": text}));
                }
                *changed = true;
                continue;
            }
            let Some((_, _, source)) = FIELDS.iter().find(|(p, _, _)| *p == f.name) else {
                continue;
            };
            let or_null = |t: &str| {
                if t.is_empty() {
                    Value::Null
                } else {
                    Value::String(t.to_owned())
                }
            };
            match *source {
                Source::PageText(member) => edits.push(edit("page", &page, member, Value::String(text))),
                Source::Description if facts.description_overridden => {
                    edits.push(edit("page", &page, "description_override", or_null(&text)));
                }
                Source::Description => {
                    edits.push(edit(
                        "project",
                        &project,
                        "master.description",
                        Value::String(text),
                    ));
                }
                Source::ProjectText(member) => {
                    edits.push(edit("project", &project, member, Value::String(text)));
                }
                Source::ProjectIdent(member) => edits.push(edit("project", &project, member, or_null(&text))),
                // Typed in the project's date format, stored ISO.
                Source::ProjectDate(member) if text.is_empty() => {
                    edits.push(edit("project", &project, member, Value::Null));
                }
                Source::ProjectDate(member) => {
                    let format = self.date_format();
                    let Ok(date) = format.parse(&text) else {
                        let message = strings::format(
                            key!("error-titleblock-date"),
                            &[strings::text(f.label_key), &format.example()],
                        );
                        return self.form_failed(TITLE_BLOCK, message);
                    };
                    edits.push(edit("project", &project, member, Value::String(date.to_string())));
                }
                Source::Party(role) => {
                    parties_changed = true;
                    name_party(role, text);
                }
                Source::RevisionAuthor | Source::RevisionChecker => {
                    let named = match *source {
                        Source::RevisionAuthor => facts.revision_author.as_deref(),
                        _ => facts.revision_checker.as_deref(),
                    };
                    parties_changed = true;
                    match (named, facts.open_revision) {
                        (Some(role), _) => name_party(role, text),
                        (None, Some(rev)) if !text.is_empty() => {
                            name_party(AUTHOR_ROLE, text);
                            new_author = Some(rev);
                        }
                        _ => {}
                    }
                }
                // Not a field of the form: the save sets it, or the program derives it.
                Source::Edited | Source::Derived => {}
                Source::RevisionLabel if text.is_empty() => {
                    return self.form_failed(
                        TITLE_BLOCK,
                        strings::text(key!("error-titleblock-revision")).to_owned(),
                    );
                }
                Source::RevisionLabel => match facts.open_revision {
                    Some(rev) => edits.push(edit("revision", &rev.to_string(), "label", Value::String(text))),
                    None => new_revision = Some(text),
                },
            }
        }
        if parties_changed {
            edits.push(edit("project", &project, "parties", Value::Array(parties)));
        }
        if page_props_changed {
            edits.push(edit("page", &page, "user_props", Value::Array(page_props)));
        }
        if project_props_changed {
            edits.push(edit(
                "project",
                &project,
                "user_props",
                Value::Array(project_props),
            ));
        }
        if !edits.is_empty() {
            let cmd = AnyCommand {
                name: "fields.set_many".to_owned(),
                params: json!({ "edits": edits }),
            };
            match self.run_commit(&cmd) {
                Outcome::Done => {}
                Outcome::Failed { message } => return self.form_failed(TITLE_BLOCK, message),
                other => return other,
            }
        }
        let mut steps = Vec::new();
        if let Some(label) = new_revision {
            steps.push(AnyCommand {
                name: "revision.create".to_owned(),
                params: json!({ "label": label }),
            });
        }
        if let Some(rev) = new_author {
            steps.push(AnyCommand {
                name: "revision.set_author".to_owned(),
                params: json!({ "revision": rev.to_string(), "role": AUTHOR_ROLE }),
            });
        }
        for cmd in steps {
            match self.run_commit(&cmd) {
                Outcome::Done => {}
                Outcome::Failed { message } => return self.form_failed(TITLE_BLOCK, message),
                other => return other,
            }
        }
        if in_form {
            self.dialog = None;
        }
        Outcome::Done
    }
}
