// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window's Title block fields form reads of a page (FT-1686): the placeholder paths the page's frame and
//! title-block forms print, in the order of their cells, each with its text now; and where those values live (the
//! project's ID, whether the page overrides the project's description, the open revision, the parties).

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{PageRef, TbValue, TitleBlockValues, open_revision};
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
        if let Some(frame) = q.get::<PageFrameOf>(key).as_ref() {
            let forms = std::iter::once(&frame.form).chain(frame.title_block.as_ref());
            for cell in forms.flat_map(|f| f.cells.iter()) {
                if let Some(template) = &cell.value {
                    placeholder_paths(template, &mut paths);
                }
            }
        }
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
