// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window's Title block fields form reads of a page: the placeholder paths the page's frame and
//! title-block forms print, in the order of their cells, each with its text now; and where those values live (the
//! project's ID, whether the page overrides the project's description, the open revision, the parties).

use ew_derive::{PageRef, TbValue, TitleBlockValues, current_revision, open_revision};
use ew_ids::{Id, Uuid};
use ew_mltext::MlText;
use ew_schema::{Page, Revision, UserPropScope, UserPropValue};
use serde_json::Value;

use super::view::{PageFrameOf, neutral};
use crate::Session;

/// The facts of a page's title block for its fields form.
#[derive(Clone, PartialEq, Debug)]
pub struct TitleBlockFacts {
    /// The placeholder paths the page's frame and title block print (`project.drawing_id`), in the order of their cells,
    /// each once, with the text the page prints for it now (empty when it has no value; a date in the project's date
    /// format).
    pub fields: Vec<(String, String)>,
    /// The label of the first labelled cell that prints each path (`project.reference` → `Project:`), in the project's
    /// neutral text: where a person finds a field's value on the sheet.
    pub cells: Vec<(String, String)>,
    /// The project record's ID.
    pub project: Uuid,
    /// Whether the page overrides the project's description with its own.
    pub description_overridden: bool,
    /// The open revision, if the current revision is open.
    pub open_revision: Option<Uuid>,
    /// Whether the project has any revision.
    pub has_revisions: bool,
    /// The project's parties, in their JSON form.
    pub parties: Value,
    /// The party roles the current revision names as its author and its checker (their JSON words).
    pub revision_author: Option<String>,
    /// See [`TitleBlockFacts::revision_author`].
    pub revision_checker: Option<String>,
    /// Whether the page's frame or title block writes texts of its own (a cell without a placeholder, as an
    /// imported QElectroTech template does): they are not fields.
    pub fixed_texts: bool,
    /// The next free revision label in the project's label format: after the greatest label of a revision (a
    /// withdrawal record passed over), else the format's first; `None` for a format of a newer build.
    pub revision_next: Option<String>,
    /// The fields of the project's own the page's frame and title block print (doc 09 §2.10), the page's first, then
    /// the project's, each in the order of their cells.
    pub own_fields: Vec<OwnField>,
    /// The page's values of the fields of the project's own, in their JSON form (`fields.set` writes the list whole).
    pub page_props: Value,
    /// The project's values of its fields of its own, in their JSON form.
    pub project_props: Value,
}

/// A field of the project's own that a title block prints (doc 09 §2.10).
#[derive(Clone, PartialEq, Debug)]
pub struct OwnField {
    /// The placeholder path that prints it (`page.prop.customer_sheet`), one of [`TitleBlockFacts::fields`].
    pub path: String,
    /// Its key.
    pub key: String,
    /// Its name as the person typed it.
    pub label: ew_mltext::MlText,
    /// Whether it has one value for each page (else one for the project).
    pub page: bool,
}

impl Session {
    /// The frame form the drawn page `page` draws with while it names none of its own (FR-PAG-018: its page type's
    /// default, else the project's); `None` for a page that is not in the project or a project without a default
    /// frame. Its Form row names it.
    pub fn page_default_frame(&self, page: Uuid) -> Option<Uuid> {
        let q = self.query();
        let record = q.pages().get(Id::from_uuid(page))?;
        ew_derive::frame_form_of(&q, None, &record.kind, true).map(|form| form.as_uuid())
    }

    /// The title block's facts of the drawn page `page` (see [`TitleBlockFacts`]); `None` for a page that is not in the
    /// project.
    pub fn title_block_facts(&self, page: Uuid) -> Option<TitleBlockFacts> {
        let q = self.query();
        let id: Id<Page> = Id::from_uuid(page);
        let record = q.pages().get(id)?;
        let key = PageRef::Drawn(id);
        let mut paths = Vec::new();
        let mut fixed_texts = false;
        let mut cells: Vec<(String, String)> = Vec::new();
        if let Some(frame) = q.get::<PageFrameOf>(key).as_ref() {
            let forms = std::iter::once(&frame.form).chain(frame.title_block.as_ref());
            for cell in forms.flat_map(|f| f.cells.iter()) {
                if let Some(template) = &cell.value {
                    let own = ew_derive::placeholder_paths(template);
                    let label = cell.label.as_ref().map(neutral).unwrap_or_default();
                    let label = label.trim().trim_end_matches(':').trim();
                    for path in own {
                        if !label.is_empty() && !cells.iter().any(|(p, _)| *p == path) {
                            cells.push((path.clone(), label.to_owned()));
                        }
                        if !paths.contains(&path) {
                            paths.push(path);
                        }
                    }
                    fixed_texts |= !template.contains('{') && !template.trim().is_empty();
                }
            }
        }
        let current = current_revision(&q);
        let role = |r: Option<&ew_schema::PartyRole>| {
            r.and_then(|r| serde_json::to_value(r).ok())
                .and_then(|v| v.as_str().map(str::to_owned))
        };
        let values = q.get::<TitleBlockValues>(key);
        let fields: Vec<(String, String)> = paths
            .into_iter()
            .map(|path| {
                let text = match values.get(&path) {
                    Some(TbValue::Text(t)) => t.clone(),
                    Some(TbValue::Ml(m)) => neutral(m),
                    // A date as the project shows it, as the form takes it back.
                    Some(TbValue::Date(d)) => q.project().settings.formatting.date_format().format(*d),
                    Some(TbValue::Tag { displayed, .. }) => displayed.clone(),
                    Some(TbValue::Asset(_)) | None => String::new(),
                };
                (path, text)
            })
            .collect();
        let project = q.project();
        let mut own_fields: Vec<OwnField> = Vec::new();
        for page_scope in [true, false] {
            for (path, _) in &fields {
                let key = if page_scope {
                    path.strip_prefix("page.prop.")
                } else {
                    path.strip_prefix("project.prop.")
                };
                let scope = if page_scope {
                    ew_schema::UserPropScope::Page
                } else {
                    ew_schema::UserPropScope::Project
                };
                let def = key.and_then(|k| {
                    project
                        .user_prop_defs
                        .iter()
                        .find(|d| d.key.as_str() == k && d.scope.contains(&scope))
                });
                if let Some(def) = def {
                    own_fields.push(OwnField {
                        path: path.clone(),
                        key: def.key.as_str().to_owned(),
                        label: def.label.clone(),
                        page: page_scope,
                    });
                }
            }
        }
        Some(TitleBlockFacts {
            own_fields,
            page_props: serde_json::to_value(&record.user_props).unwrap_or(Value::Null),
            project_props: serde_json::to_value(&project.user_props).unwrap_or(Value::Null),
            fields,
            cells,
            project: project.id.as_uuid(),
            description_overridden: record
                .description_override
                .as_ref()
                .is_some_and(|d| !d.is_empty()),
            open_revision: open_revision(&q).map(|r| r.id.as_uuid()),
            has_revisions: q.table::<Revision>().iter().next().is_some(),
            parties: serde_json::to_value(&project.parties).unwrap_or(Value::Null),
            revision_author: role(current.as_ref().and_then(|r| r.author.as_ref())),
            revision_checker: role(current.as_ref().and_then(|r| r.checker.as_ref())),
            fixed_texts,
            revision_next: project
                .settings
                .revision_label_format()
                .next_after(
                    q.table::<Revision>()
                        .iter()
                        .map(|(_, r)| r)
                        .filter(|r| !r.is_withdrawal())
                        .map(|r| r.label.as_str()),
                )
                .map(|l| l.to_string()),
        })
    }
}

/// A field of the project's own with its value on each record of a Properties section (doc 09 §2.10).
#[derive(Clone, PartialEq, Debug)]
pub struct OwnFieldValues {
    /// Its key.
    pub key: String,
    /// Its name as the person typed it.
    pub label: ew_mltext::MlText,
    /// Its value on each record, in their order; an empty text where it has none.
    pub values: Vec<ew_mltext::MlText>,
}

impl Session {
    /// The fields of the project's own of one scope with their values: with `page`, each field of each page on the pages
    /// `targets`; else each field of the whole project, the project's value (doc 09 §2.10). A target that is no page is
    /// passed over.
    pub fn own_field_values(&self, page: bool, targets: &[Uuid]) -> Vec<OwnFieldValues> {
        let q = self.query();
        let project = q.project();
        let scope = if page {
            UserPropScope::Page
        } else {
            UserPropScope::Project
        };
        let lists: Vec<Vec<UserPropValue>> = if page {
            targets
                .iter()
                .filter_map(|t| q.pages().get(Id::<Page>::from_uuid(*t)))
                .map(|p| p.user_props.clone())
                .collect()
        } else {
            vec![project.user_props.clone()]
        };
        project
            .user_prop_defs
            .iter()
            .filter(|d| d.scope.contains(&scope))
            .map(|d| OwnFieldValues {
                key: d.key.as_str().to_owned(),
                label: d.label.clone(),
                values: lists
                    .iter()
                    .map(|l| {
                        l.iter()
                            .find(|v| v.key == d.key)
                            .map(|v| v.value.clone())
                            .unwrap_or_default()
                    })
                    .collect(),
            })
            .collect()
    }

    /// The edits of `fields.set_many` that give the field `key` the value `text` typed in `language` (the window's,
    /// `nl-NL`; empty: none) on each of `targets` (with `page`, pages; else the project's one ID), each record's value
    /// list written whole by [`typed_own_values`].
    pub fn own_field_edits(
        &self,
        page: bool,
        targets: &[Uuid],
        key: &str,
        text: &str,
        language: Option<&str>,
    ) -> Vec<Value> {
        let q = self.query();
        let project = q.project();
        let lists: Vec<(Uuid, Vec<UserPropValue>)> = if page {
            targets
                .iter()
                .filter_map(|t| q.pages().get(Id::<Page>::from_uuid(*t)))
                .map(|p| (p.id.as_uuid(), p.user_props.clone()))
                .collect()
        } else {
            vec![(project.id.as_uuid(), project.user_props.clone())]
        };
        let kind = if page { "page" } else { "project" };
        lists
            .into_iter()
            .map(|(target, values)| {
                let values: Vec<Value> = values.iter().filter_map(|v| serde_json::to_value(v).ok()).collect();
                let list = typed_own_values(&values, key, text, language);
                serde_json::json!({"kind": kind, "targets": [target.to_string()], "path": "user_props", "value": list})
            })
            .collect()
    }
}

/// A record's values of the fields of the project's own (`user_props`, in their JSON form) with the field `key` given
/// the text `text` typed in `language` (the window's, `nl-NL`) (doc 09 §2.10): a value of several languages keeps the
/// others and takes `text` as that language's, an empty text removing that language; any other value becomes the
/// neutral text `text`; a value left empty goes. The one rule of a field's row in Properties and of Title block
/// fields….
pub fn typed_own_values(values: &[Value], key: &str, text: &str, language: Option<&str>) -> Vec<Value> {
    let text = text.trim();
    let at = values.iter().position(|v| v["key"] == key);
    let old: Option<MlText> = at.and_then(|i| serde_json::from_value(values[i]["value"].clone()).ok());
    let tag = language.and_then(|l| l.parse::<ew_mltext::LangTag>().ok());
    let new = match (old, tag) {
        (Some(mut m), Some(tag)) if !m.is_neutral() => {
            m.set_lang(tag, text);
            m
        }
        _ => MlText::neutral(text),
    };
    let mut out = values.to_vec();
    let entry = serde_json::json!({"key": key, "value": new});
    match (at, new.is_empty()) {
        (Some(i), true) => {
            out.remove(i);
        }
        (Some(i), false) => {
            // The record's other members of the value (a newer format's) stay.
            out[i]["value"] = entry["value"].clone();
        }
        (None, true) => {}
        (None, false) => out.push(entry),
    }
    out
}
