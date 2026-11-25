// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Move to page… (doc 06 §3.4): the selected symbols onto another schematic page, and the preview card a move opens
//! when it keeps a wire to what stays behind as a pair of interruption points, which a PLC card's Move to page shares,
//! or leaves selected texts and boxes on the page.

use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use crate::key;
use crate::properties::CardOrigin;
use crate::shell::{Outcome, ShellModel, failed, string_param};
use crate::strings;
use crate::tools::{EntityKind, selection_kinds};

/// The key of the notice by which a move names a pair of interruption points it makes.
const PAIR: &str = "notice-rep-move-keep-pair";

impl ShellModel {
    /// A cross-page move (`rep.move` or `rep.move_to_new_page` with `params`) with `keep`, the interruption-point
    /// forms of the page shown: when the move keeps a wire to what stays behind as a pair of interruption points, or
    /// leaves selected texts and boxes on the page (`stays`, the line that says so), the preview card that lists the
    /// pairs and that line (Apply moves); `None` otherwise, the caller then commits the move as it is, `keep` in it.
    pub(super) fn move_card(
        &mut self,
        name: &str,
        params: &mut Value,
        stays: Option<String>,
    ) -> Option<Outcome> {
        if let Ok((source, target, copies)) = self.ip_forms() {
            let target = target.as_ref().unwrap_or(&source);
            params["keep"] = json!({
                "source": source.copy.to_string(),
                "target": target.copy.to_string(),
                "copies": copies,
            });
        }
        let cmd = ew_api::AnyCommand {
            name: name.to_owned(),
            params: params.clone(),
        };
        let pairs = self
            .project
            .as_mut()
            .and_then(|p| p.session.preview(&cmd).ok())
            .is_some_and(|preview| preview.outcome().notices.iter().any(|n| n.key() == PAIR));
        let reason = match (pairs, &stays) {
            (true, _) => key!("reason-move-keeps-wires"),
            (false, Some(_)) => key!("reason-move-leaves-items"),
            (false, None) => return None,
        };
        let rows = stays.into_iter().collect();
        Some(self.preview_host_card_as(name, params.clone(), reason, CardOrigin::Bulk { rows }))
    }

    /// `edit.move_to_page {page}` (Page ▸ Move to page…, which asks for the page when it is left out): the selected
    /// symbols of the page shown onto the schematic page of folio `page` where they stand (`new`: a new page after this one in
    /// its section, its folio as New page proposes it), one undo step. A wire to what stays behind is kept as a pair of interruption points, which a preview card
    /// lists first ([`ShellModel::move_card`]), as it says the selected texts and boxes that stay on the page; a move
    /// without either is made at once and its page shown.
    pub(crate) fn act_move_to_page(&mut self, p: &Map<String, Value>) -> Outcome {
        let folio = match string_param(p, "page") {
            Ok(Some(f)) => f.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["page"]),
            Err(e) => return e,
        };
        let Some(page) = self.canvas().map(|c| c.page()) else {
            return Outcome::refused(key!("reason-no-pages"));
        };
        let here = page.tables.page;
        let reps: Vec<String> = crate::tools::transform::selected_reps(&self.tools.selection, page)
            .into_iter()
            .map(|e| e.0.to_string())
            .collect();
        if reps.is_empty() {
            return Outcome::refused(key!("reason-move-select-symbols"));
        }
        // The selected texts and boxes stay where they are: the card says so before anything moves.
        let left = selection_kinds(&self.tools.selection, &page.tables)
            .iter()
            .filter(|(_, k)| matches!(k, EntityKind::Item | EntityKind::StructureBox))
            .count();
        let stays = (left > 0)
            .then(|| strings::format_counted(crate::count_key!("preview-move-stays"), left, &[&page.folio]));
        let before: Vec<Uuid> = self.page_rows().iter().map(|r| r.id).collect();
        let (name, mut params) = if folio == "new" {
            let mut params = json!({ "reps": reps });
            if let Ok(number) = self
                .new_page_proposal(Some(&format!("p:{here}")), Some("multi_line"))
                .0
                .parse::<u32>()
            {
                params["number"] = json!(number);
            }
            ("rep.move_to_new_page", params)
        } else {
            let Some(to) = self
                .page_rows()
                .iter()
                .find(|r| r.folio == folio && r.kind.is_schematic() && r.id != here)
                .map(|r| r.id)
            else {
                return failed(key!("error-bad-param"), &["page"]);
            };
            ("rep.move", json!({ "reps": reps, "page": to.to_string() }))
        };
        if let Some(card) = self.move_card(name, &mut params, stays) {
            return card;
        }
        let out = self.run_commit(&ew_api::AnyCommand {
            name: name.to_owned(),
            params,
        });
        if out != Outcome::Done {
            return out;
        }
        let shown = if folio == "new" {
            self.page_rows()
                .iter()
                .find(|r| !before.contains(&r.id))
                .map(|r| r.folio.clone())
        } else {
            Some(folio)
        };
        if let Some(folio) = shown {
            let _ = self.dispatch("page.goto", &json!({ "folio": folio }));
        }
        Outcome::Done
    }
}
