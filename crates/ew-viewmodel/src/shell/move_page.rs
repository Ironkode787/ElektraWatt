// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Move to page… (doc 06 §3.4): the selected symbols onto another schematic page, and the preview card a move opens
//! first, which a PLC card's Move to page shares when it keeps a wire to what stays behind as a pair of interruption
//! points or leaves selected texts and boxes on the page.

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
    /// The folio `rep.move_to_new_page` gives the page it makes after the page `after` (`params`' `number`), where
    /// a made page of that page's kind goes ([`ShellModel::new_page_proposal`]), with the section it makes for it where
    /// no section is for the kind (`new_section`).
    pub(super) fn new_page_folio(&self, after: Uuid, params: &mut Value) {
        let kind = self
            .page_rows()
            .iter()
            .find(|r| r.id == after)
            .map(|r| ew_api::output::page_kind_name(&r.kind));
        let (folio, _) = self.new_page_proposal(Some(&format!("p:{after}")), kind.as_deref());
        let Ok(number) = folio.parse::<u32>() else {
            return;
        };
        params["number"] = json!(number);
        if let Some((_, section)) = kind.and_then(|k| self.new_section_for(&k, number)) {
            params["new_section"] = section;
        }
    }

    /// A cross-page move (`rep.move` or `rep.move_to_new_page` with `params`) with `keep`, the interruption-point
    /// forms of the page shown: when the move keeps a wire to what stays behind as a pair of interruption points, or
    /// leaves selected texts and boxes on the page (`stays`, the line that says so), or `always`, the preview card that
    /// lists the pairs and that line under its lead, which says how many symbols move to which page (Apply moves);
    /// `None` otherwise, the caller then commits the move as it is, `keep` in it.
    pub(super) fn move_card(
        &mut self,
        name: &str,
        params: &mut Value,
        stays: Option<String>,
        always: bool,
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
        let notices = self
            .project
            .as_mut()
            .and_then(|p| p.session.preview(&cmd).ok())
            .map(|preview| preview.outcome().notices.clone())
            .unwrap_or_default();
        let pairs = notices.iter().any(|n| n.key() == PAIR);
        let reason = match (pairs, &stays, always) {
            (true, _, _) => key!("reason-move-keeps-wires"),
            (false, Some(_), _) => key!("reason-move-leaves-items"),
            (false, None, true) => key!("reason-move-preview"),
            (false, None, false) => return None,
        };
        // The section made for the new page is among the command's notices, which the card shows.
        let rows = stays.into_iter().collect();
        // What moves where: the card's summary leads with it and says what its count of items is made of.
        let symbols = params["reps"].as_array().map_or(0, Vec::len);
        let lead = match (name, params["number"].as_u64(), params["page"].as_str()) {
            ("rep.move_to_new_page", number, _) => strings::format_counted(
                crate::count_key!("preview-move-lead-new"),
                symbols,
                &[&number.map(|n| n.to_string()).unwrap_or_default()],
            ),
            (_, _, Some(page)) => {
                let folio = self
                    .page_rows()
                    .iter()
                    .find(|r| r.id.to_string() == page)
                    .map(|r| r.folio.clone())
                    .unwrap_or_default();
                strings::format_counted(crate::count_key!("preview-move-lead"), symbols, &[&folio])
            }
            _ => String::new(),
        };
        let origin = CardOrigin::Bulk {
            rows,
            lead: Some(lead).filter(|l| !l.is_empty()),
        };
        Some(self.preview_host_card_as(name, params.clone(), reason, origin))
    }

    /// `edit.move_to_page {page}` (Page ▸ Move to page…, which asks for the page when it is left out): the selected
    /// symbols of the page shown onto the schematic page of folio `page` where they stand (`new`: a new page where a made
    /// page of this one's kind goes, its folio as New page proposes it, [`ShellModel::new_page_folio`]), one undo step.
    /// Its preview card comes first ([`ShellModel::move_card`]): what moves where, the pairs of interruption points
    /// that keep a wire to what stays behind, the selected texts and boxes that stay on the page; Apply moves and shows
    /// the page ([`ShellModel::moved_to`]).
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
        let (name, mut params) = if folio == "new" {
            let mut params = json!({ "reps": reps });
            self.new_page_folio(here, &mut params);
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
        // Its question says that a preview follows (the action `previews`): every move shows its card first.
        self.move_card(name, &mut params, stays, true)
            .unwrap_or(Outcome::Done)
    }

    /// The page a move's card leaves on show after its Apply (`rep.move`'s page, the page `rep.move_to_new_page`
    /// made), as a move made at once shows it; `None` for another card.
    pub(super) fn moved_to(&self, card: &crate::properties::Card) -> Option<String> {
        match card.edit.name.as_str() {
            "rep.move" => {
                let page = card.edit.params.get("page")?.as_str()?;
                self.page_rows()
                    .iter()
                    .find(|r| r.id.to_string() == page)
                    .map(|r| r.folio.clone())
            }
            "rep.move_to_new_page" => {
                let preview = card.preview.result.as_ref().ok()?;
                let rows = self.page_rows();
                self.project_session()?
                    .preview_folios(preview)
                    .into_iter()
                    .find(|(id, _)| !rows.iter().any(|r| r.id == *id))
                    .map(|(_, folio)| folio)
            }
            _ => None,
        }
    }
}
