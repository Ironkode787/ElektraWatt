// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 6 (D16, FR-UI-045): the glossary pane. Help ▸ Glossary (`help.glossary`) opens the pane "Glossary"
//! in the bottom dock: one row per term of `glossary/glossary.toml` ([`crate::glossary::terms`], the source of the
//! palette's aliases and of the user guide's page) with the EW term in the UI language, the EPLAN and the QET terms
//! and the definition, and a row's actions as buttons.
//!
//! **The search** (`glossary.filter`) covers every column in the UI language and in English, by the navigator
//! filter's rule (§6.3's plain text): the text split into tokens as a navigator row's cells are
//! (`ew_api::output::nav_tokens`), a row matching when every typed token is a prefix of one of its tokens, or when
//! the typed text, case-folded, lies in one of its texts. An EPLAN or QET cell the file leaves empty stays empty.

use ew_api::output::nav_tokens;
use serde_json::{Map, Value};

use super::{Outcome, ShellModel, string_param};
use crate::glossary::{Term, terms};
use crate::layout::PaneKind;

/// One row of the glossary pane.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GlossaryRow {
    /// The EW term in the UI language.
    pub term: String,
    /// The EPLAN terms, joined by ", ".
    pub eplan: String,
    /// The QET terms, joined by ", ".
    pub qet: String,
    /// The definition in the UI language.
    pub definition: String,
    /// The action IDs the row offers, in file order.
    pub actions: Vec<String>,
}

/// The glossary's locale of the UI language (`nl` reads `nl-NL`; every other language `en-GB`).
fn glossary_locale() -> &'static str {
    if crate::strings::locale() == "nl" {
        "nl-NL"
    } else {
        "en-GB"
    }
}

/// Whether `term` matches the typed `filter` in the UI language `locale` or in English.
fn matches(term: &Term, locale: &str, filter: &str) -> bool {
    let texts: Vec<&str> = [
        term.label(locale),
        term.label("en-GB"),
        term.ew.as_str(),
        term.definition(locale),
        term.definition("en-GB"),
    ]
    .into_iter()
    .chain(term.eplan.iter().map(String::as_str))
    .chain(term.qet.iter().map(String::as_str))
    .collect();
    let folded = filter.trim().to_lowercase();
    if folded.is_empty() || texts.iter().any(|t| t.to_lowercase().contains(&folded)) {
        return true;
    }
    let tokens: Vec<String> = texts.iter().flat_map(|t| nav_tokens(t)).collect();
    nav_tokens(filter)
        .iter()
        .all(|q| tokens.iter().any(|t| t.starts_with(q.as_str())))
}

impl ShellModel {
    /// Whether the glossary pane is in the layout.
    pub fn glossary_shown(&self) -> bool {
        self.layout.panes().contains(&PaneKind::Glossary)
    }

    /// The glossary pane's search text.
    pub fn glossary_filter(&self) -> &str {
        &self.glossary_filter
    }

    /// The glossary rows the search text matches, in file order.
    pub fn glossary_rows(&self) -> Vec<GlossaryRow> {
        let locale = glossary_locale();
        terms()
            .iter()
            .filter(|t| matches(t, locale, &self.glossary_filter))
            .map(|t| GlossaryRow {
                term: t.label(locale).to_owned(),
                eplan: t.eplan.join(", "),
                qet: t.qet.join(", "),
                definition: t.definition(locale).to_owned(),
                actions: t.actions.clone(),
            })
            .collect()
    }

    /// `help.glossary {on?}` (Help ▸ Glossary): opens or closes the pane in the bottom dock.
    pub(crate) fn act_help_glossary(&mut self, p: &Map<String, Value>) -> Outcome {
        let shown = self.glossary_shown();
        let want = match super::jobs::bool_param(p, "on", !shown) {
            Ok(w) => w,
            Err(e) => return e,
        };
        let layout = if want {
            self.layout
                .with_bottom(PaneKind::Glossary)
                .showing(PaneKind::Glossary)
        } else {
            self.layout.without(PaneKind::Glossary)
        };
        self.replace_layout(layout);
        Outcome::Done
    }

    /// `glossary.filter {text}`: the pane's search text.
    pub(crate) fn act_glossary_filter(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "text") {
            Err(e) => e,
            Ok(text) => {
                text.unwrap_or_default().clone_into(&mut self.glossary_filter);
                self.glossary_row = None;
                Outcome::Done
            }
        }
    }

    /// The row cursor of the glossary pane (FT-1281): the index of its row among [`ShellModel::glossary_rows`], none
    /// before the first move or a new search.
    pub fn glossary_row(&self) -> Option<usize> {
        let n = self.glossary_rows().len();
        self.glossary_row.filter(|r| *r < n)
    }

    /// `glossary.row {step?, to?}` (Up and Down in the pane's field, FT-1281): moves the row cursor by `step` rows
    /// (from before the first row when there is none) or to row `to` (1-based), kept within the rows; nothing to move
    /// over no row.
    pub(crate) fn act_glossary_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let n = self.glossary_rows().len();
        if n == 0 {
            return Outcome::Done;
        }
        let at = if let Some(step) = p.get("step") {
            let Some(step) = step.as_i64() else {
                return super::failed(crate::key!("error-bad-param"), &["step"]);
            };
            let from = match self.glossary_row() {
                Some(r) => r as i64,
                None if step > 0 => -1,
                None => 0,
            };
            (from + step).clamp(0, n as i64 - 1)
        } else {
            match p.get("to").and_then(Value::as_u64) {
                Some(to) if to >= 1 && (to as usize) <= n => to as i64 - 1,
                Some(_) | None if p.contains_key("to") => return super::failed(crate::key!("error-bad-param"), &["to"]),
                _ => return super::failed(crate::key!("error-missing-param"), &["step"]),
            }
        };
        self.glossary_row = Some(at as usize);
        Outcome::Done
    }
}
