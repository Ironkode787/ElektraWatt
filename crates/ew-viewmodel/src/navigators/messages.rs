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
    /// The finding row and the date the open Waive prompt commits for (FT-674): the row's key and the date the
    /// window passed, as `YYYY-MM-DD`.
    pub(crate) waive: Option<(String, Option<String>)>,
    /// What the shell reads from the online report, once per (project session, revision) and only when asked.
    pub(crate) online: std::cell::RefCell<Option<OnlineFacts>>,
    /// The counts of the last Check project, by (project session, revision): the status bar's findings field while
    /// the Messages pane is closed and the revision is the checked one (FX38d: its click opens the pane).
    pub(crate) checked: Option<((u64, u64), MessageCounts)>,
    /// The serial of the last Check project's toast and the (project session, revision) it counted: the toast goes
    /// once the revision leaves that one, so its counts never disagree with the dock's pending ones (FT-759).
    pub(crate) check_toast: Option<((u64, u64), u64)>,
    /// FT-842: the release's housekeeping warnings in words (the file name only) that the Messages table shows under
    /// Warnings ▸ Release, with the project session they belong to; drained once from the session, kept until the
    /// project closes.
    pub(crate) release_notes: Option<(u64, Vec<String>)>,
}

/// What the shell reads from the project's online check report (FT-674, FT-675), for one (project session,
/// revision).
#[derive(Clone, Debug, Default)]
pub(crate) struct OnlineFacts {
    /// The (project session, revision) it was read at.
    pub(crate) at: (u64, u64),
    /// The waiver record ID of each waived finding, by the finding's fingerprint.
    pub(crate) waivers: BTreeMap<String, String>,
    /// The connections of the potentials of the open EW-POT-001 findings, sorted (the canvas's conflict overlay).
    pub(crate) conflict: Vec<ew_api::output::Uuid>,
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

impl MessageCounts {
    /// The open findings that count (FT-710): errors, warnings and infos of the project, without the pending and the
    /// waived ones. The check's toast, the status bar and the Messages tab's count chip all read this rule.
    pub fn open(&self) -> usize {
        self.errors + self.warnings + self.infos
    }
}

/// The finding a detail block shows (§7.3): rule, title, kind, severity, message and the quick fix's label; and what
/// Waive… and Unwaive need (FT-674): the fingerprint, whether it is waived and the waiver's ID.
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
    /// The finding's fingerprint (32 hex digits), what `check.waive` names it by.
    pub fingerprint: String,
    /// Whether the finding is waived (its state cell).
    pub waived: bool,
    /// The waiver's record ID while it is waived, what `check.unwaive` takes; the shell fills it from the online
    /// report ([`detail`] leaves it `None`).
    pub waiver: Option<String>,
}

/// The value of a node's facet `name`.
fn facet<'n>(node: &'n NavNode, name: &str) -> Option<&'n str> {
    node.facets
        .iter()
        .find(|(f, _)| f == name)
        .map(|(_, v)| v.as_str())
}

/// The one pending rule (§7.2): an open completeness finding whose page (`page`, lower case) is the editing page
/// (`editing`, a folio, any case). The Messages rows ([`is_pending`]) and the Properties header's chips
/// ([`chip_pending`]) both read it (M39: the chip counts what the pane counts).
pub fn pending_on(completeness_open: bool, page: Option<&str>, editing: Option<&str>) -> bool {
    match (page, editing) {
        (Some(page), Some(folio)) => completeness_open && page == folio.to_lowercase(),
        _ => false,
    }
}

/// Whether a finding row is pending while `editing` (a folio, any case) is the editing page's folio (§7.2).
pub fn is_pending(node: &NavNode, editing: Option<&str>) -> bool {
    node.role == NavRole::Finding
        && pending_on(
            facet(node, "kind") == Some("completeness") && facet(node, "state") == Some("open"),
            facet(node, "page"),
            editing,
        )
}

/// Whether a Properties header chip's finding is pending while `editing` is the editing page's folio: its
/// `pending_on` is the page of an open completeness finding (the header lists open findings only).
pub fn chip_pending(chip: &ew_api::output::FindingChip, editing: Option<&str>) -> bool {
    pending_on(chip.pending_on.is_some(), chip.pending_on.as_deref(), editing)
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

/// The key of the rule group of the release's housekeeping warnings (FT-842) under Warnings.
pub const RELEASE_KEY: &str = "severity:warning/rule:release";

/// Whether a row is one of the release's housekeeping warnings (FT-842) or their group.
pub fn is_release_note(node: &NavNode) -> bool {
    node.key == RELEASE_KEY || node.key.starts_with(&format!("{RELEASE_KEY}/"))
}

/// FT-842 (cycle 41, ruling 2: "each once as a Messages entry in words, with the file name only. The entry has
/// severity Warning, and its rule column names the source in a word ("Release"), not an ID"): `table` with the
/// release's housekeeping warnings `notes` under Warnings ▸ Release, after the rules of the check (the Warnings group
/// is added when the check has none). The rows are finding rows (severity `warning`, state `open`) without a target,
/// a page, a quick fix or a fingerprint: Enter opens nothing and the detail has no Waive. The top-level groups are
/// ranked Errors, Warnings, Infos again. No notes: the table as it is.
pub fn with_release_notes(mut table: NavTable, notes: &[String]) -> NavTable {
    use ew_api::output::{nav_natural_key, nav_tokens};
    if notes.is_empty() {
        return table;
    }
    let natural = |label: &str, cells: &[String]| -> Vec<ew_api::output::NaturalKey> {
        std::iter::once(label)
            .chain(cells.iter().map(String::as_str))
            .map(nav_natural_key)
            .collect()
    };
    let tokens = |label: &str, cells: &[String]| -> Vec<String> {
        let mut t: Vec<String> = std::iter::once(label)
            .chain(cells.iter().map(String::as_str))
            .flat_map(nav_tokens)
            .collect();
        t.sort();
        t.dedup();
        t
    };
    let node = |key: String,
                parent: Option<u32>,
                role: NavRole,
                label: String,
                badges: Vec<String>,
                cells: Vec<String>,
                facets: Vec<(String, String)>| NavNode {
        key,
        parent,
        role,
        tokens: tokens(&label, &cells),
        natural: natural(&label, &cells),
        label,
        badges,
        records: Vec::new(),
        kinds: Vec::new(),
        define_at: None,
        drawables: Vec::new(),
        open: None,
        table_row: !cells.is_empty(),
        cells,
        facets,
    };
    let index = |nodes: &[NavNode]| u32::try_from(nodes.len()).unwrap_or(u32::MAX);
    let ranked = |mut n: NavNode, at: u32| {
        if let Some(first) = n.natural.first_mut() {
            *first = nav_natural_key(&at.to_string());
        }
        n
    };
    let warnings = match table
        .nodes
        .iter()
        .position(|n| n.parent.is_none() && n.key == "severity:warning")
    {
        Some(at) => u32::try_from(at).unwrap_or(u32::MAX),
        None => {
            let at = index(&table.nodes);
            table.nodes.push(node(
                "severity:warning".to_owned(),
                None,
                NavRole::Severity,
                "Warnings".to_owned(),
                Vec::new(),
                Vec::new(),
                vec![("kind".to_owned(), NavRole::Severity.word().to_owned())],
            ));
            at
        }
    };
    let source = crate::strings::text("release-source").to_owned();
    let rule = index(&table.nodes);
    let group = node(
        RELEASE_KEY.to_owned(),
        Some(warnings),
        NavRole::Rule,
        source.clone(),
        vec![notes.len().to_string()],
        Vec::new(),
        vec![("kind".to_owned(), NavRole::Rule.word().to_owned())],
    );
    table.nodes.push(ranked(group, rule));
    for (i, text) in notes.iter().enumerate() {
        let at = index(&table.nodes);
        let cells = vec![
            "warning".to_owned(),
            source.clone(),
            text.clone(),
            String::new(),
            String::new(),
            String::new(),
            "open".to_owned(),
        ];
        let row = node(
            format!("{RELEASE_KEY}/trouble:{i}"),
            Some(rule),
            NavRole::Finding,
            text.clone(),
            vec!["warning".to_owned()],
            cells,
            vec![("state".to_owned(), "open".to_owned())],
        );
        table.nodes.push(ranked(row, at));
    }
    // The top-level groups in the order Errors, Warnings, Infos (siblings sort by their rank).
    for n in table.nodes.iter_mut().filter(|n| n.parent.is_none()) {
        let order = match n.key.as_str() {
            "severity:error" => 0u32,
            "severity:warning" => 1,
            _ => 2,
        };
        if let Some(first) = n.natural.first_mut() {
            *first = nav_natural_key(&order.to_string());
        }
    }
    table
}

/// The detail block of a finding row (§7.3); `None` for any other row and for the release's housekeeping warnings
/// (FT-842: nothing to waive).
pub fn detail(node: &NavNode) -> Option<FindingDetail> {
    if is_release_note(node) {
        return None;
    }
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
        fingerprint: fingerprint_of(&node.key).unwrap_or_default().to_owned(),
        waived: cell(STATE) == "waived",
        waiver: None,
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
            kinds: Vec::new(),
            define_at: None,
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
        // FT-710: the status bar's field and the tab's chip read the same counts; the pending one is its own part.
        assert_eq!(none.open(), 4);
        assert_eq!(on_101.open(), 3);
        let text = |c| {
            crate::status::findings_text(c)
                .map(|(t, _)| t)
                .unwrap_or_default()
        };
        assert_eq!(text(none), "2 errors · 1 warning · 1 info");
        assert_eq!(
            text(on_101),
            "1 error · 1 warning · 1 info · 1 pending on this page"
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
        assert_eq!(d.fingerprint, "00");
        assert!(!d.waived);
        assert_eq!(d.waiver, None);
        assert!(
            detail(&finding("completeness", "101", "waived", "error"))
                .unwrap()
                .waived
        );
        assert!(!d.title.is_empty(), "the shipped rule's title");
        assert_eq!(fingerprint_of("severity:error/rule:X/finding:ab12"), Some("ab12"));
        assert_eq!(fingerprint_of("severity:error/rule:X"), None);
    }

    /// A severity group as the provider builds it, ranked `rank`.
    fn group(key: &str, label: &str, rank: &str) -> NavNode {
        NavNode {
            key: key.to_owned(),
            parent: None,
            role: NavRole::Severity,
            label: label.to_owned(),
            badges: Vec::new(),
            records: Vec::new(),
            kinds: Vec::new(),
            define_at: None,
            drawables: Vec::new(),
            open: None,
            cells: Vec::new(),
            table_row: false,
            tokens: Vec::new(),
            facets: Vec::new(),
            natural: vec![ew_api::output::nav_natural_key(rank)],
        }
    }

    /// FT-842 (cycle 41, ruling 2): the release's warnings go under a Warnings group between Errors and Infos, as one
    /// Release rule with a finding row each (severity warning, rule column "Release", the text, state open), shown
    /// in that order by the default sort; their detail is `None` (nothing to waive); no notes leave the table as is.
    #[test]
    fn release_notes_sit_under_warnings_release_between_errors_and_infos() {
        let mut errors = finding("conflict", "101", "open", "error");
        errors.parent = Some(0);
        errors.natural = vec![ew_api::output::nav_natural_key("1")];
        let base = NavTable {
            kind: super::super::NavKind::Messages,
            rev: 1,
            columns: Vec::new(),
            nodes: vec![
                group("severity:error", "Errors", "0"),
                errors,
                group("severity:info", "Infos", "2"),
            ],
            index: BTreeMap::new(),
        };
        assert_eq!(with_release_notes(base.clone(), &[]), base);
        let notes = vec!["The release record a.json could not be removed: denied".to_owned()];
        let t = with_release_notes(base, &notes);
        let keys: Vec<&str> = t.nodes.iter().map(|n| n.key.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "severity:error",
                "severity:error/rule:EW-CORE-033/finding:00",
                "severity:info",
                "severity:warning",
                RELEASE_KEY,
                "severity:warning/rule:release/trouble:0",
            ]
        );
        let row = &t.nodes[5];
        assert_eq!(row.parent, Some(4));
        assert_eq!(t.nodes[4].parent, Some(3));
        assert_eq!(
            row.cells,
            vec!["warning", "Release", notes[0].as_str(), "", "", "", "open"]
        );
        assert!(detail(row).is_none(), "nothing to waive");
        assert_eq!(counts(&t, None).warnings, 1);
        let rows = super::super::visible_rows(
            &t,
            super::super::NavView::Tree,
            &super::super::SortBy::Label,
            false,
            &super::super::Query::parse(""),
            &std::collections::BTreeSet::new(),
        );
        let shown: Vec<&str> = rows
            .iter()
            .map(|r| t.nodes[r.node as usize].key.as_str())
            .collect();
        assert_eq!(
            shown,
            vec![
                "severity:error",
                "severity:error/rule:EW-CORE-033/finding:00",
                "severity:warning",
                RELEASE_KEY,
                "severity:warning/rule:release/trouble:0",
                "severity:info",
            ]
        );
    }
}
