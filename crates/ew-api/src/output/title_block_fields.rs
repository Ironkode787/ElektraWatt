// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window's Title block fields form reads of a page (FT-1686): the placeholder paths the page's frame and
//! title-block forms print, in the order of their cells, each with its text now; and where those values live (the
//! project's ID, whether the page overrides the project's description, the open revision, the parties).

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{PageRef, TbValue, TitleBlockValues, current_revision, open_revision};
use ew_schema::{Page, Revision};
use serde_json::Value;

use super::view::{PageFrameOf, neutral};
use crate::Session;

/// The facts of a page's title block for its fields form.
#[derive(Clone, PartialEq, Debug)]
pub struct TitleBlockFacts {
    /// The placeholder paths the page's frame and title block print (`project.drawing_id`), in the order of their cells,
    /// each once, with the text the page prints for it now (empty when it has no value; a date as `YYYY-MM-DD`).
    pub fields: Vec<(String, String)>,
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
    /// The party roles the current revision names as its author and its checker (their JSON words), FT-1718.
    pub revision_author: Option<String>,
    /// See [`TitleBlockFacts::revision_author`].
    pub revision_checker: Option<String>,
    /// Whether the page's frame or title block writes texts of its own (a cell without a placeholder, as an
    /// imported QElectroTech template does): they are not fields.
    pub fixed_texts: bool,
    /// The next free revision label in the project's label format: after the greatest label of a revision (a
    /// withdrawal record passed over), else the format's first; `None` for a format of a newer build.
    pub revision_next: Option<String>,
}

/// The placeholder paths of a template (`"Drawing: {project.drawing_id | default(-)}"` names `project.drawing_id`):
/// each `{…}` but a doubled brace and a computed cell (`{= …}`), up to its first filter.
fn placeholder_paths(template: &str, out: &mut Vec<String>) {
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        let after = &rest[at + 1..];
        if let Some(skip) = after.strip_prefix('{') {
            rest = skip;
            continue;
        }
        let Some(end) = after.find('}') else {
            return;
        };
        let inner = after[..end].trim();
        let path = inner.split('|').next().unwrap_or("").trim();
        if !inner.starts_with('=') && !path.is_empty() && !out.iter().any(|p| p == path) {
            out.push(path.to_owned());
        }
        rest = &after[end + 1..];
    }
}

impl Session {
    /// The title block's facts of the drawn page `page` (see [`TitleBlockFacts`]); `None` for a page that is not in the
    /// project.
    pub fn title_block_facts(&self, page: Uuid) -> Option<TitleBlockFacts> {
        let q = self.query();
        let id: Id<Page> = Id::from_uuid(page);
        let record = q.pages().get(id)?;
        let key = PageRef::Drawn(id);
        let mut paths = Vec::new();
        let mut fixed_texts = false;
        if let Some(frame) = q.get::<PageFrameOf>(key).as_ref() {
            let forms = std::iter::once(&frame.form).chain(frame.title_block.as_ref());
            for cell in forms.flat_map(|f| f.cells.iter()) {
                if let Some(template) = &cell.value {
                    placeholder_paths(template, &mut paths);
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
        let fields = paths
            .into_iter()
            .map(|path| {
                let text = match values.get(&path) {
                    Some(TbValue::Text(t)) => t.clone(),
                    Some(TbValue::Ml(m)) => neutral(m),
                    Some(TbValue::Date(d)) => d.to_string(),
                    Some(TbValue::Tag { displayed, .. }) => displayed.clone(),
                    Some(TbValue::Asset(_)) | None => String::new(),
                };
                (path, text)
            })
            .collect();
        let project = q.project();
        Some(TitleBlockFacts {
            fields,
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

#[cfg(test)]
mod tests {
    use super::placeholder_paths;

    #[test]
    fn a_template_names_its_paths_once_without_filters_doubled_braces_or_computed_cells() {
        let mut out = Vec::new();
        placeholder_paths("Drawing: {project.drawing_id | default(-)} {{literal}}", &mut out);
        placeholder_paths("{= sum(x)} {page.title}{project.drawing_id}", &mut out);
        placeholder_paths("{unclosed", &mut out);
        assert_eq!(out, ["project.drawing_id", "page.title"]);
    }
}
