// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel on the navigator framework (WP-S2.09 part 2, §7; D8, D9, D10): the view state that the table of
//! `NavKind::Messages` does not hold, and the pure rules over that table.
//!
//! **Pending** (§7.2, FR-UI-041). The shell keeps the **editing page**: the focused tab's page after every successful
//! commit made while that tab is focused, cleared when the focused tab shows another page, when the project closes and
//! by `messages.check`. A finding row is pending when its node has the facet `kind:completeness`, its `page` facet
//! names the editing page's folio and its state is open. A pending row gets the badge `pending` after its others, is
//! painted muted, and counts in `pending`, not in its severity's count. Changing the editing page re-marks the rows
//! without a `Session` call.
//!
//! **Order.** The provider gives every node its rank as its first natural key, so the framework's default label sort
//! shows Errors, Warnings, Infos, the rules in rule-ID order and the findings in the report's order.

use std::collections::BTreeMap;

use super::{NavNode, NavRole, NavTable};
use crate::canvas::PageId;

/// The column of a finding row's severity word.
const SEVERITY: usize = 0;
/// The column of a finding row's rule ID.
const RULE: usize = 1;
/// The column of a finding row's message.
const MESSAGE: usize = 2;
/// The column of a finding row's quick-fix label.
const FIX: usize = 5;
/// The column of a finding row's state (`open`, `waived`).
const STATE: usize = 6;

/// The message panel's view state in the shell.
#[derive(Clone, Debug, Default)]
pub struct MessagesState {
    /// The page of the focused tab on which the last commit was made (§7.2), with the project session it belongs to.
    pub(crate) editing_page: Option<(u64, PageId)>,
    /// The name of the scheme the table was built with, read with the table (once per revision while the pane is open).
    pub(crate) scheme: Option<&'static str>,
    /// The origin annotations of the commits made through a quick fix, by (project session, revision): `quick fix
    /// <rule ID>` for the history panel (§7.4, §8.4).
    pub(crate) annotations: BTreeMap<(u64, u64), String>,
}

/// The counts of the pane's header line (§7.3): the open findings by severity less the pending ones, the waived
/// findings, and the pending ones.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MessageCounts {
    /// Open, not pending, at error severity.
    pub errors: usize,
    /// Open, not pending, at warning severity.
    pub warnings: usize,
    /// Open, not pending, at info severity.
    pub infos: usize,
    /// Waived, any severity.
    pub waived: usize,
    /// Pending (open completeness findings on the editing page).
    pub pending: usize,
}

/// The finding a detail block shows (§7.3): rule, title, kind, severity, message and the quick fix's label.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FindingDetail {
    /// The rule ID.
    pub rule: String,
    /// The rule's title.
    pub title: String,
    /// The rule kind's word (`completeness`).
    pub kind: String,
    /// The severity word.
    pub severity: String,
    /// The message.
    pub message: String,
    /// The quick fix's label, when the finding has one.
    pub fix: Option<String>,
}

/// The value of a node's facet `name`.
fn facet<'n>(node: &'n NavNode, name: &str) -> Option<&'n str> {
    node.facets
        .iter()
        .find(|(f, _)| f == name)
        .map(|(_, v)| v.as_str())
}

/// Whether a finding row is pending while `editing` (a folio, any case) is the editing page's folio (§7.2).
pub fn is_pending(node: &NavNode, editing: Option<&str>) -> bool {
    let Some(folio) = editing else { return false };
    node.role == NavRole::Finding
        && facet(node, "kind") == Some("completeness")
        && facet(node, "state") == Some("open")
        && facet(node, "page") == Some(folio.to_lowercase().as_str())
}

/// The severity word of a finding row (`error`, `warning`, `info`), `None` for any other row.
pub fn severity(node: &NavNode) -> Option<&'static str> {
    if node.role != NavRole::Finding {
        return None;
    }
    match node.cells.get(SEVERITY).map(String::as_str) {
        Some("error") => Some("error"),
        Some("warning") => Some("warning"),
        Some("info") => Some("info"),
        _ => None,
    }
}

/// The header counts of a Messages table while `editing` is the editing page's folio.
pub fn counts(table: &NavTable, editing: Option<&str>) -> MessageCounts {
    let mut c = MessageCounts::default();
    for n in table.nodes.iter().filter(|n| n.role == NavRole::Finding) {
        if n.cells.get(STATE).map(String::as_str) == Some("waived") {
            c.waived += 1;
        } else if is_pending(n, editing) {
            c.pending += 1;
        } else {
            match severity(n) {
                Some("error") => c.errors += 1,
                Some("warning") => c.warnings += 1,
                Some("info") => c.infos += 1,
                _ => {}
            }
        }
    }
    c
}

/// The rule ID of a finding or rule row (the finding's `rule` cell, the rule group's key segment).
pub fn rule_of(node: &NavNode) -> Option<String> {
    match node.role {
        NavRole::Finding => node.cells.get(RULE).cloned(),
        NavRole::Rule => node
            .key
            .rsplit('/')
            .next()
            .and_then(|s| s.strip_prefix("rule:"))
            .map(str::to_owned),
        _ => None,
    }
}

/// The fingerprint text of a finding row's key (`…/finding:<fingerprint>`).
pub fn fingerprint_of(key: &str) -> Option<&str> {
    key.rsplit('/').next()?.strip_prefix("finding:")
}

/// The detail block of a finding row (§7.3); `None` for any other row.
pub fn detail(node: &NavNode) -> Option<FindingDetail> {
    let rule = rule_of(node).filter(|_| node.role == NavRole::Finding)?;
    let title = ew_api::checks::rule(&rule)
        .map(|r| r.meta().title.to_owned())
        .unwrap_or_default();
    let cell = |i: usize| node.cells.get(i).cloned().unwrap_or_default();
    let fix = cell(FIX);
    Some(FindingDetail {
        title,
        kind: facet(node, "kind").unwrap_or_default().to_owned(),
        severity: cell(SEVERITY),
        message: cell(MESSAGE),
        fix: (!fix.is_empty()).then_some(fix),
        rule,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(kind: &str, page: &str, state: &str, severity: &str) -> NavNode {
        NavNode {
            key: "severity:error/rule:EW-CORE-033/finding:00".to_owned(),
            parent: Some(1),
            role: NavRole::Finding,
            label: "-3W1: cable without length".to_owned(),
            badges: vec![severity.to_owned()],
            records: Vec::new(),
            drawables: Vec::new(),
            open: None,
            cells: vec![
                severity.to_owned(),
                "EW-CORE-033".to_owned(),
                "cable without length".to_owned(),
                "-3W1".to_owned(),
                format!("{page}-B2"),
                String::new(),
                state.to_owned(),
            ],
            table_row: true,
            tokens: Vec::new(),
            facets: vec![
                ("kind".to_owned(), kind.to_owned()),
                ("page".to_owned(), page.to_lowercase()),
                ("state".to_owned(), state.to_owned()),
            ],
            natural: Vec::new(),
        }
    }

    #[test]
    fn pending_is_an_open_completeness_finding_on_the_editing_page() {
        let n = finding("completeness", "101", "open", "error");
        assert!(is_pending(&n, Some("101")));
        assert!(!is_pending(&n, Some("102")), "another page");
        assert!(!is_pending(&n, None), "no editing page");
        assert!(!is_pending(
            &finding("conflict", "101", "open", "error"),
            Some("101")
        ));
        assert!(!is_pending(
            &finding("completeness", "101", "waived", "error"),
            Some("101")
        ));
        assert!(
            is_pending(&finding("completeness", "A1", "open", "info"), Some("a1")),
            "folios fold case"
        );
    }

    #[test]
    fn counts_move_pending_out_of_their_severity() {
        let table = NavTable {
            kind: super::super::NavKind::Messages,
            rev: 1,
            columns: Vec::new(),
            nodes: vec![
                finding("completeness", "101", "open", "error"),
                finding("conflict", "101", "open", "error"),
                finding("completeness", "102", "open", "warning"),
                finding("hygiene", "101", "waived", "info"),
                finding("hygiene", "101", "open", "info"),
            ],
            index: BTreeMap::new(),
        };
        let none = counts(&table, None);
        assert_eq!(
            (none.errors, none.warnings, none.infos, none.waived, none.pending),
            (2, 1, 1, 1, 0)
        );
        let on_101 = counts(&table, Some("101"));
        assert_eq!(
            (
                on_101.errors,
                on_101.warnings,
                on_101.infos,
                on_101.waived,
                on_101.pending
            ),
            (1, 1, 1, 1, 1)
        );
    }

    #[test]
    fn the_detail_of_a_finding() {
        let d = detail(&finding("completeness", "101", "open", "error")).unwrap();
        assert_eq!(d.rule, "EW-CORE-033");
        assert_eq!(d.kind, "completeness");
        assert_eq!(d.severity, "error");
        assert_eq!(d.message, "cable without length");
        assert_eq!(d.fix, None);
        assert!(!d.title.is_empty(), "the shipped rule's title");
        assert_eq!(fingerprint_of("severity:error/rule:X/finding:ab12"), Some("ab12"));
        assert_eq!(fingerprint_of("severity:error/rule:X"), None);
    }
}
