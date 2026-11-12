// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Title block fields… (`page.title_block`, FT-1686): every field the focused page's title block prints from the
//! project's data, in one form, written where its value comes from in one undo step: the page's title and description,
//! the project's master data, its parties' names, the open revision's label and the names of the parties the current
//! revision names as author and checker (FT-1718). A project without a revision gets one with the label typed
//! (`revision.create`, a second step); an open revision without an author gets one (`revision.set_author`). What a title block prints as fixed text of its form is not
//! a field: an imported QElectroTech template's own texts stay as the form holds them.

use ew_api::AnyCommand;
use serde_json::{Map, Value, json};

use super::{Dialog, FormDialog, FormField, Outcome, ShellModel, string_param};
use crate::key;
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
    /// A date member of the project row (`YYYY-MM-DD`); empty removes it.
    ProjectDate(&'static str),
    /// The name of the first party of a role (its serde word); a party of that role is added for a new name.
    Party(&'static str),
    /// The open revision's label, or a new revision's when the project has none.
    RevisionLabel,
    /// The name of the party the current revision names as its author (FT-1718); an open revision that names none
    /// takes the party of the role `author`, named as typed (`revision.set_author`, a second step).
    RevisionAuthor,
    /// The name of the party the current revision names as its checker; only while it names one (the checker is
    /// set when the revision is approved, Check ▸ Revision ▸ Approve).
    RevisionChecker,
}

/// The party role an open revision without an author takes for the author's name typed in the form.
const AUTHOR_ROLE: &str = "author";

/// The title-block paths the form edits, each with its label and its source (doc 09 §2.6's paths): the one list.
const FIELDS: &[(&str, Key<'static>, Source)] = &[
    ("page.title", key!("field-word-title"), Source::PageText("title")),
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
    (
        "project.edited",
        key!("field-word-edited"),
        Source::ProjectDate("master.edited"),
    ),
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
        key!("choice-author"),
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
        key!("choice-author"),
        Source::RevisionAuthor,
    ),
    (
        "revision.current.checker.name",
        key!("titleblock-checked"),
        Source::RevisionChecker,
    ),
];

/// One edit of `fields.set_many`.
fn edit(kind: &str, target: &str, path: &str, value: Value) -> Value {
    json!({"kind": kind, "targets": [target], "path": path, "value": value})
}

impl ShellModel {
    /// The form's fields for the focused page: the editable paths its title block prints, in its order, with their text;
    /// the revision label only while it can be written (an open revision, or none yet). `None` without a page.
    fn title_block_fields(&self) -> Option<(String, ew_api::output::TitleBlockFacts, Vec<FormField>)> {
        let page = self.focused_page_id()?;
        let facts = self.project.as_ref()?.session.title_block_facts(page)?;
        let fields = facts
            .fields
            .iter()
            .filter_map(|(path, text)| {
                let (name, label_key, source) = FIELDS.iter().find(|(p, _, _)| p == path)?;
                let writable = match source {
                    Source::RevisionLabel => facts.open_revision.is_some() || !facts.has_revisions,
                    // A released revision's author is history: its party's name only, while it names one.
                    Source::RevisionAuthor => {
                        facts.revision_author.is_some() || facts.open_revision.is_some()
                    }
                    Source::RevisionChecker => facts.revision_checker.is_some(),
                    _ => true,
                };
                writable.then(|| FormField {
                    name,
                    label_key: *label_key,
                    choices: &super::choices::NONE,
                    options: Vec::new(),
                    value: text.clone(),
                })
            })
            .collect();
        Some((page.to_string(), facts, fields))
    }

    /// The Title block fields form's line over its fields (FT-1718): that the texts a template writes itself stay as
    /// it holds them.
    pub fn title_block_notes(&self) -> Vec<String> {
        let Some((_, facts, _)) = self.title_block_fields() else {
            return Vec::new();
        };
        let mut notes = Vec::new();
        if facts.fixed_texts {
            notes.push(strings::text(key!("titleblock-note-fixed")).to_owned());
        }
        notes
    }

    /// The line under field `field` of the open form `form`: the form's own ([`FormDialog::note`]), and the Title
    /// block fields form's rule of the revision label before Apply (the persona round of 0.2.0-rc.2: a "B" was refused
    /// only after Apply): the project's form by the next free label, and where the form is set.
    pub fn form_note(&self, form: &FormDialog, field: &str) -> Option<String> {
        if let Some(note) = form.note(field) {
            return Some(note.to_owned());
        }
        if form.action != TITLE_BLOCK || field != "revision.current.label" {
            return None;
        }
        let (_, facts, _) = self.title_block_fields()?;
        let next = facts.revision_next?;
        Some(strings::format(
            key!("titleblock-revision-note"),
            &[
                &next,
                strings::text(key!("action-props-project")),
                strings::text(key!("field-word-revision-label")),
            ],
        ))
    }

    /// The line at the foot of the open form `form`, under its last field: the Title block fields form's word on
    /// Checked where the page's title block prints a Checked cell that the current revision fills from no checker
    /// yet, so the field is not in the form (the persona round of 0.2.0-rc.2: the line stood first, over fields that
    /// had no Checked). `None` for another form.
    pub fn form_foot(&self, form: &FormDialog) -> Option<String> {
        if form.action != TITLE_BLOCK {
            return None;
        }
        let (_, facts, _) = self.title_block_fields()?;
        let prints_checker = facts
            .fields
            .iter()
            .any(|(path, _)| path == "revision.current.checker.name");
        (prints_checker && facts.revision_checker.is_none()).then(|| {
            strings::format(
                key!("titleblock-note-checked"),
                &[
                    strings::text(key!("action-revision-approve")),
                    strings::text(key!("menu-check")),
                ],
            )
        })
    }

    /// `page.title_block {<path>: text, …}` (Page ▸ Title block fields…, FT-1686): without a field the window opens the
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
        let named = fields.iter().any(|f| p.contains_key(f.name));
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
        // The name of the first party of `role`; a party of that role is added for a new name.
        let mut name_party =
            |role: &str, text: String| match parties.iter_mut().find(|party| party["role"] == role) {
                Some(party) => party["name"] = Value::String(text),
                None if text.is_empty() => {}
                None => parties.push(json!({"role": role, "name": text})),
            };
        for f in &fields {
            let text = match string_param(p, f.name) {
                Err(e) => return e,
                Ok(Some(t)) => t.trim().to_owned(),
                Ok(None) if in_form => String::new(),
                Ok(None) => continue,
            };
            if text == f.value {
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
                Source::ProjectDate(member) => {
                    if !text.is_empty() && text.parse::<ew_api::catalog::Date>().is_err() {
                        let message =
                            strings::format(key!("error-titleblock-date"), &[strings::text(f.label_key)]);
                        return self.form_failed(TITLE_BLOCK, message);
                    }
                    edits.push(edit("project", &project, member, or_null(&text)));
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
