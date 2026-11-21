// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page ▸ Copy pages… (`pages.copy`, doc 06 §3.6, "A copy of pages"): the marked pages copied for a second installation
//! by the rule of its form (a folio offset or the page after which the copies go, a structure node of the copy's own,
//! renamed frozen tags and interruption-point names). `Session::copy_pages` composes the one `page.copy`; the shell
//! previews it on the `page-copy` card, which says what the copy leaves out and what continues outside it, and Apply
//! commits it (one undo step) with a toast naming the new folios.

use serde_json::{Map, Value};

use super::forms::{FormDialog, FormField, FormTable};
use super::{Dialog, Mode, Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::properties::{Card, CardOrigin};
use crate::strings;

/// The action of Copy pages… (its form dispatches it).
const ACTION: &str = "pages.copy";

/// The `rename` parameter: an object of old and new texts (`{"6": "7"}`, a script), or the form table's rows
/// (`[{"name": old, "values": {<the column's words>: new}}]`); rows without an old text are skipped.
fn rename_param(p: &Map<String, Value>) -> Result<Vec<(String, String)>, Outcome> {
    let bad = || failed(key!("error-bad-param"), &["rename"]);
    match p.get("rename") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(m)) => m
            .iter()
            .map(|(from, to)| Ok((from.clone(), to.as_str().ok_or_else(bad)?.to_owned())))
            .collect(),
        Some(Value::Array(rows)) => {
            let mut out = Vec::new();
            for row in rows {
                let from = row.get("name").and_then(Value::as_str).ok_or_else(bad)?;
                if from.is_empty() {
                    continue;
                }
                // The table's one column, whatever its words in the window's language.
                let to = row
                    .get("values")
                    .and_then(Value::as_object)
                    .and_then(|m| m.values().next())
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                out.push((from.to_owned(), to.to_owned()));
            }
            Ok(out)
        }
        Some(_) => Err(bad()),
    }
}

impl ShellModel {
    /// The structure nodes of the project as the form offers them: the node's ID and its designation (`+CAB`), in
    /// the order of the designations, after "no node of its own".
    fn copy_node_options(&self) -> Vec<(String, String)> {
        let mut out = vec![(
            String::new(),
            strings::text(key!("form-page-copy-node-none")).to_owned(),
        )];
        let Some(project) = &self.project else { return out };
        let q = project.session.query();
        let mut nodes: Vec<(String, String)> = q
            .structure_nodes()
            .iter()
            .filter_map(|(id, n)| {
                let aspect = serde_json::to_value(&n.aspect).ok()?;
                let sign = super::creation::SIGNS
                    .iter()
                    .find(|(_, a)| Some(*a) == aspect.as_str())?
                    .0;
                Some((id.as_uuid().to_string(), format!("{sign}{}", n.code.as_str())))
            })
            .collect();
        nodes.sort_by(|a, b| super::natural_cmp(&a.1, &b.1).then_with(|| a.cmp(b)));
        out.extend(nodes);
        out
    }

    /// The node a `node` parameter names: its ID (or alias), or its designation with its aspect's sign (`+CAB`).
    fn copy_node(&self, text: &str) -> Result<String, Outcome> {
        let Some((_, aspect)) = super::creation::SIGNS
            .iter()
            .find(|(sign, _)| text.starts_with(sign))
        else {
            return Ok(text.to_owned());
        };
        self.structure_node(aspect, text)
            .map_err(|message| Outcome::Failed { message })
    }

    /// `pages.copy {pages?, offset?, after?, node?, code?, rename?}` (Page ▸ Copy pages…, the page row's right-click
    /// menu): the pages `pages` (folios), else the marked pages (else the Pages cursor's page), copied by the rule:
    /// `offset` added to each folio number, else after the folio `after` (default the last copied page); the
    /// structure node `node` (an ID or a designation such as `+CAB`) replaced in the copy by the node of code `code`
    /// beside it; `rename` the old and new texts of frozen tags and interruption-point names (`{"6": "7"}`). Without
    /// an offset or `after` the window opens the form (offset, after, node, code and the rename table); headless the
    /// copies go after the last copied page. It opens the `page-copy` card; Apply commits one `page.copy`.
    pub(crate) fn act_pages_copy(&mut self, p: &Map<String, Value>) -> Outcome {
        let submitted = self.in_form_of(ACTION);
        let out = self.pages_copy(p);
        // The form that just opened stays; a submitted one closes on its card or keeps its refusal.
        if !submitted && self.in_form_of(ACTION) {
            return out;
        }
        self.form_done(ACTION, out)
    }

    fn pages_copy(&mut self, p: &Map<String, Value>) -> Outcome {
        let pages = or_return!(self.macro_pages(p));
        let offset = or_return!(super::bulk::uint_param(p, "offset"));
        let text = |name: &str| -> Result<String, Outcome> {
            Ok(string_param(p, name)?
                .map(|s| s.trim().to_owned())
                .unwrap_or_default())
        };
        let (after, node, code) = match (text("after"), text("node"), text("code")) {
            (Ok(a), Ok(n), Ok(c)) => (a, n, c),
            (Err(e), ..) | (_, Err(e), _) | (.., Err(e)) => return e,
        };
        let rename = or_return!(rename_param(p));
        let last = self
            .page_rows()
            .iter()
            .find(|r| pages.last() == Some(&r.id))
            .map(|r| r.folio.clone())
            .unwrap_or_default();
        let rule = offset.is_some() || !after.is_empty() || p.contains_key("pages");
        if !rule && self.mode == Mode::Gui && !self.in_form_of(ACTION) {
            let field = |name: &'static str, label_key, value: String| FormField {
                name,
                label_key,
                choices: &super::choices::NONE,
                options: Vec::new(),
                value,
            };
            let form = FormDialog {
                action: ACTION,
                title_key: key!("page-copy-title"),
                ok_key: key!("dialog-page-copy"),
                fields: vec![
                    field("offset", key!("form-page-copy-offset"), String::new()),
                    field("after", key!("form-page-copy-after"), last),
                    FormField {
                        options: self.copy_node_options(),
                        ..field("node", key!("form-page-copy-node"), String::new())
                    },
                    field("code", key!("form-page-copy-code"), String::new()),
                ],
                error: None,
                table: Some(FormTable {
                    name: "rename",
                    heading_key: key!("form-page-copy-rename"),
                    fixed: false,
                    key_label: key!("form-page-copy-rename-old"),
                    add_key: key!("form-page-copy-rename-add"),
                    columns: vec![(
                        strings::text(key!("form-page-copy-rename-new")).to_owned(),
                        String::new(),
                    )],
                    rows: Vec::new(),
                }),
            };
            return self.open_dialog(Dialog::Form(form));
        }
        let folios = match offset {
            Some(n) => ew_api::CopyFolios::Offset(n),
            None => {
                let folio = if after.is_empty() { last } else { after };
                match ew_api::output::PageRow::by_folio(self.page_rows(), &folio) {
                    Some(r) if !r.generated => ew_api::CopyFolios::After(r.id),
                    Some(_) => return failed(key!("error-generated-page-order"), &[]),
                    None => return failed(key!("error-no-folio"), &[&folio]),
                }
            }
        };
        let structure = if node.is_empty() {
            if !code.is_empty() {
                return failed(key!("error-missing-param"), &["node"]);
            }
            Vec::new()
        } else {
            let id = or_return!(self.copy_node(&node));
            let Ok(id) = id.parse() else {
                return failed(key!("error-bad-param"), &["node"]);
            };
            if code.is_empty() {
                return failed(key!("error-missing-param"), &["code"]);
            }
            vec![(id, code)]
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let spec = ew_api::PageCopySpec {
            pages: pages.clone(),
            folios,
            structure,
            rename,
        };
        let plan = match project.session.copy_pages(&spec) {
            Ok(plan) => plan,
            Err(e) => return super::tools::command_failed(&e, &Value::Null),
        };
        let mut rows = Vec::new();
        if !plan.drawn_elsewhere.is_empty() {
            rows.push(strings::format(
                key!("page-copy-drawn-elsewhere"),
                &[&plan.drawn_elsewhere.join(", ")],
            ));
        }
        if !plan.cleared_frozen.is_empty() {
            rows.push(strings::format(
                key!("page-copy-frozen-cleared"),
                &[&plan.cleared_frozen.join(", ")],
            ));
        }
        if !plan.continued.is_empty() {
            rows.push(strings::format(
                key!("page-copy-continued"),
                &[&plan.continued.join(", ")],
            ));
        }
        let folios: Vec<String> = pages
            .iter()
            .filter_map(|id| self.page_rows().iter().find(|r| r.id == *id))
            .map(|r| r.folio.clone())
            .collect();
        let lead = match folios.as_slice() {
            [one] => strings::format(key!("page-copy-lead-one"), &[one]),
            _ => strings::format(
                key!("page-copy-lead"),
                &[&folios.len().to_string(), &folios.join(", ")],
            ),
        };
        self.preview_host_card_as(
            &plan.command.name,
            plan.command.params,
            key!("reason-bulk"),
            CardOrigin::PageCopy { lead, rows },
        )
    }

    /// The toast of a copy of pages' Apply, read from its card before the commit: the new pages' folios; `None` for
    /// another card.
    pub(super) fn page_copy_toast(&self, card: &Card) -> Option<String> {
        if !matches!(card.origin, CardOrigin::PageCopy { .. }) {
            return None;
        }
        let preview = card.preview.result.as_ref().ok()?;
        let rows = self.page_rows();
        let new: Vec<String> = self
            .project_session()?
            .preview_folios(preview)
            .into_iter()
            .filter(|(id, _)| !rows.iter().any(|r| r.id == *id))
            .map(|(_, folio)| folio)
            .collect();
        Some(match new.as_slice() {
            [one] => strings::format(key!("toast-pages-copied-one"), &[one]),
            _ => strings::format(
                key!("toast-pages-copied"),
                &[&new.len().to_string(), &new.join(", ")],
            ),
        })
    }
}
