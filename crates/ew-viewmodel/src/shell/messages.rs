// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel in the shell (WP-S2.09 part 2, §7.2 to §7.6): `NavKind::Messages` in the bottom dock, the
//! editing page and the pending rows, F8 and Shift+F8, F1 on a finding or rule row, the quick fix as a previewed card,
//! and Tools ▸ Run checks.
//!
//! **Calls** (§7.5). The pane's table is the framework's: one `Session::navigator_table(Messages)` per revision while
//! it is open (one `Session::check` inside), none while it is closed. The editing page changes the pending rows
//! without a call. `messages.check` drops the table, so the end of the dispatch fetches it again; `messages.fix` runs
//! the online check once more to read the finding's quick fix (memoised), then previews the fix.

use ew_api::checks::CheckMode;
use ew_api::output::NavKind;
use ew_api::{AnyCommand, Origin, Submit};
use serde_json::{Map, Value};

use super::navigation::{Focus, bump};
use super::{Mode, Outcome, ShellModel, failed, string_param};
use crate::canvas::PageId;
use crate::layout::PaneKind;
use crate::navigators::messages::{self, FindingDetail, MessageCounts};
use crate::navigators::{NavRole, Navigator, Query, visible_rows};
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, PendingEdit};
use crate::strings;
use crate::tools::PreviewState;

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The scheme the Messages table was built with, while the pane is open.
    pub fn messages_scheme(&self) -> Option<&'static str> {
        self.navs
            .contains_key(&NavKind::Messages)
            .then_some(self.messages.scheme)
            .flatten()
    }

    /// The folio of the editing page (§7.2), while there is one.
    pub fn editing_folio(&self) -> Option<String> {
        let (epoch, page) = self.messages.editing_page?;
        if self.project.as_ref().map(|p| p.epoch) != Some(epoch) {
            return None;
        }
        self.page_rows()
            .iter()
            .find(|r| PageId::of(r) == page)
            .map(|r| r.folio.clone())
    }

    /// The header counts of the open Messages pane (§7.3).
    pub fn messages_counts(&self) -> Option<MessageCounts> {
        let t = self.navs.get(&NavKind::Messages)?.table()?;
        Some(messages::counts(t, self.editing_folio().as_deref()))
    }

    /// Whether row `key` of navigator `kind` is a pending finding (§7.2).
    pub fn row_pending(&self, kind: NavKind, key: &str) -> bool {
        if kind != NavKind::Messages {
            return false;
        }
        let editing = self.editing_folio();
        self.navs
            .get(&kind)
            .and_then(|n| n.node_of(key))
            .is_some_and(|(_, n)| messages::is_pending(n, editing.as_deref()))
    }

    /// The detail block of the Messages cursor's finding (§7.3).
    pub fn messages_detail(&self) -> Option<FindingDetail> {
        let nav = self.navs.get(&NavKind::Messages)?;
        let key = nav.cursor_key()?;
        nav.node_of(&key).and_then(|(_, n)| messages::detail(n))
    }

    /// The origin annotation the shell keeps for the commit that made revision `rev` of the open project session
    /// (`quick fix <rule ID>`, §7.4), for the history panel (§8.4).
    pub fn origin_annotation(&self, rev: u64) -> Option<&str> {
        let epoch = self.project.as_ref()?.epoch;
        self.messages.annotations.get(&(epoch, rev)).map(String::as_str)
    }

    // -----------------------------------------------------------------------------------------------------------
    // The editing page (§7.2)

    /// The project session and revision before an action runs (the dispatch's argument to
    /// [`ShellModel::track_editing_page`]).
    pub(super) fn epoch_rev(&self) -> Option<(u64, u64)> {
        self.project.as_ref().map(|p| (p.epoch, p.session.world().rev.0))
    }

    /// After an action: a successful commit (the revision moved and the entry below the history's cursor is of the new
    /// revision) makes the focused tab's page the editing page; a project change or another page in the focused tab
    /// clears it.
    pub(super) fn track_editing_page(&mut self, before: Option<(u64, u64)>) {
        let now = self.epoch_rev();
        if let (Some((e0, r0)), Some((e1, r1))) = (before, now)
            && e0 == e1
            && r0 != r1
            && let Some(p) = &self.project
        {
            let h = p.session.history();
            let committed = h
                .cursor()
                .checked_sub(1)
                .and_then(|i| h.entries().get(i))
                .is_some_and(|e| e.rev().0 == r1);
            if committed && let Some(page) = self.current_page() {
                self.messages.editing_page = Some((e1, page));
            }
        }
        if let Some((epoch, page)) = self.messages.editing_page
            && (now.map(|n| n.0) != Some(epoch) || self.current_page() != Some(page))
        {
            self.messages.editing_page = None;
        }
        let epoch = now.map(|n| n.0);
        self.messages.annotations.retain(|(e, _), _| Some(*e) == epoch);
    }

    // -----------------------------------------------------------------------------------------------------------
    // The pane

    /// Docks the Messages pane in the bottom dock (D14) when it is closed, and fetches its table now (the end of the
    /// dispatch fetches none twice: the table is of the current revision).
    fn open_messages(&mut self) {
        let pane = PaneKind::Navigator(NavKind::Messages);
        let layout = self.layout().with_bottom(pane);
        self.replace_layout(layout);
        self.navs
            .entry(NavKind::Messages)
            .or_insert_with(|| Navigator::new(NavKind::Messages));
        self.fetch_messages_table();
    }

    /// Fetches the Messages table when it is not of the current revision (one counted `navigator_table` call).
    pub(super) fn fetch_messages_table(&mut self) {
        let rev = self.revision();
        let Some(project) = &self.project else { return };
        let Some(n) = self.navs.get_mut(&NavKind::Messages) else {
            return;
        };
        if n.table().is_some_and(|t| Some(t.rev) == rev) {
            return;
        }
        n.table = Some(std::sync::Arc::new(
            project.session.navigator_table(NavKind::Messages),
        ));
        self.messages.scheme = Some(project.session.online_scheme().name);
        bump(&self.calls, |c| c.navigator_table += 1);
    }

    /// `messages.show {on?}` (View ▸ Messages): `navigator.show` with the kind `messages` (the pane in the bottom
    /// dock, focused), or `on: false` to close it.
    pub(super) fn act_messages_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut q = Map::new();
        q.insert("kind".into(), Value::from(NavKind::Messages.word()));
        if let Some(on) = p.get("on") {
            q.insert("on".into(), on.clone());
        }
        self.act_navigator_show(&q)
    }

    /// `messages.next` (F8) and `messages.previous` (Shift+F8): the cursor to the next or previous finding row in the
    /// pane's order (its view, sort and filter, the collapsed groups included and then expanded), wrapping, and opens
    /// it; the pane is docked first when it is closed (the focus stays).
    pub(super) fn act_messages_walk(&mut self, forward: bool) -> Outcome {
        if !self.navs.contains_key(&NavKind::Messages) {
            self.open_messages();
        }
        let Some(nav) = self.navs.get(&NavKind::Messages) else {
            return Outcome::Done;
        };
        let Some(table) = nav.table().cloned() else {
            return Outcome::Done;
        };
        let rows = visible_rows(
            &table,
            nav.view,
            &nav.sort,
            nav.descending,
            &Query::parse(&nav.filter),
            &std::collections::BTreeSet::new(),
        );
        let keys: Vec<&str> = rows
            .iter()
            .map(|r| table.nodes[r.node as usize].key.as_str())
            .collect();
        let is_finding = |i: usize| table.nodes[rows[i].node as usize].role == NavRole::Finding;
        let at = nav.cursor_key().and_then(|c| keys.iter().position(|k| *k == c));
        let n = rows.len();
        let pick = if forward {
            let start = at.map_or(0, |a| a + 1);
            (0..n).map(|i| (start + i) % n).find(|&i| is_finding(i))
        } else {
            let start = at.unwrap_or(0) + n;
            (1..=n).map(|i| (start - i) % n).find(|&i| is_finding(i))
        };
        let Some(i) = pick else {
            return Outcome::Refused {
                reason_key: "reason-no-findings",
            };
        };
        let key = keys[i].to_owned();
        let node = rows[i].node;
        if let Some(nav) = self.navs.get_mut(&NavKind::Messages) {
            nav.reveal(node);
        }
        let mut q = Map::new();
        q.insert("kind".into(), Value::from(NavKind::Messages.word()));
        q.insert("key".into(), Value::String(key));
        self.act_navigator_open(&q)
    }

    /// `messages.check` (Tools ▸ Run checks): ends the pending state and fetches the pane's table again.
    pub(super) fn act_messages_check(&mut self) -> Outcome {
        self.messages.editing_page = None;
        if let Some(n) = self.navs.get_mut(&NavKind::Messages) {
            n.table = None;
        }
        self.fetch_messages_table();
        // FT-549: the outcome counted, `Check: 1 error, 0 warnings`, from the pane's table or, with the pane closed,
        // from a table fetched for the count alone.
        let counts = self.messages_counts().or_else(|| {
            let project = self.project.as_ref()?;
            let table = project.session.navigator_table(NavKind::Messages);
            Some(messages::counts(&table, self.editing_folio().as_deref()))
        });
        if let Some(c) = counts {
            self.notify(check_summary(c), false);
        }
        Outcome::Done
    }

    /// F1 in the Messages pane with the cursor on a finding or rule row: the rule's page of the book (§7.3).
    pub(super) fn messages_help_target(&self) -> Option<String> {
        if self.focus != Some(Focus::Navigator(NavKind::Messages)) {
            return None;
        }
        let nav = self.navs.get(&NavKind::Messages)?;
        let key = nav.cursor_key()?;
        let (_, node) = nav.node_of(&key)?;
        crate::help::rule_target(&messages::rule_of(node)?)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Quick fixes (§7.4, D10)

    /// `messages.fix {key?}`: previews the quick fix of a finding row (default the cursor row) and opens its card in the
    /// Messages pane, whatever FR-UI-044's policy would say; a finding without one refuses `reason-no-fix`.
    pub(super) fn act_messages_fix(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k.to_owned(),
            Ok(None) => match self.nav_cursor(NavKind::Messages) {
                Some(k) => k,
                None => {
                    return Outcome::Refused {
                        reason_key: "reason-no-nav-row",
                    };
                }
            },
        };
        let Some(nav) = self.navs.get(&NavKind::Messages) else {
            return failed("error-nav-closed", &[NavKind::Messages.word()]);
        };
        let Some((_, node)) = nav.node_of(&key) else {
            return failed("error-nav-key", &[&key, NavKind::Messages.word()]);
        };
        if node.role != NavRole::Finding {
            return Outcome::Refused {
                reason_key: "reason-no-fix",
            };
        }
        let Some(fp) = messages::fingerprint_of(&key).map(str::to_owned) else {
            return Outcome::Refused {
                reason_key: "reason-no-fix",
            };
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let report = project
            .session
            .check(project.session.online_scheme(), CheckMode::Online);
        let Some(finding) = report.findings.iter().find(|f| f.fingerprint.to_string() == fp) else {
            return Outcome::Refused {
                reason_key: "reason-no-fix",
            };
        };
        let Some(fix) = finding.fix.clone() else {
            return Outcome::Refused {
                reason_key: "reason-no-fix",
            };
        };
        let rule = finding.rule.to_owned();
        let targets = finding.targets.len();
        self.preview_fix(fix.command, fix.params, rule, targets)
    }

    /// Previews a quick fix's command and shows its card (`CardOrigin::QuickFix`).
    fn preview_fix(&mut self, name: String, params: Value, rule: String, targets: usize) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let cmd = AnyCommand {
            name: name.clone(),
            params: params.clone(),
        };
        let submit = match self.mode {
            Mode::Gui => Submit::from(Origin::Gui),
            Mode::Headless => Submit::from(Origin::CmdLog {
                file: self.origin_file.clone(),
            }),
        };
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        bump(calls, |c| c.preview_with += 1);
        let result = project.session.preview_with(&cmd, submit);
        let derived = match &result {
            Ok(p) => project.session.derived_diff(p),
            Err(_) => Default::default(),
        };
        let changes = result.as_ref().map_or(0, |p| p.diff().len());
        let edit = PendingEdit {
            row: String::new(),
            kind: String::new(),
            route: EditRoute::ReadOnly(FieldReadOnly::NoCommand),
            path: String::new(),
            targets: Vec::new(),
            leaf: None,
            base: None,
            name: name.clone(),
            params: params.clone(),
            typed: String::new(),
            label: String::new(),
        };
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: params,
                name,
                result: result.map_err(|e| e.to_string()),
                derived,
            },
            edit,
            targets,
            changes,
            reason: "reason-quick-fix",
            origin: CardOrigin::QuickFix { rule },
        });
        Outcome::Done
    }

    /// Apply on a quick-fix card: a fresh card commits one transaction and records the origin annotation `quick fix
    /// <rule ID>`; a stale one previews the same command again and shows a fresh card (a second Apply).
    pub(super) fn apply_fix_card(&mut self, card: Card) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let CardOrigin::QuickFix { rule } = card.origin.clone() else {
            return Outcome::Done;
        };
        if let Err(message) = &card.preview.result {
            return Outcome::Failed {
                message: message.clone(),
            };
        }
        if card.preview.base_rev != rev {
            return self.preview_fix(card.edit.name, card.edit.params, rule, card.targets);
        }
        let Card { preview, edit, .. } = card;
        let out = self.commit_kept(preview, &edit.name, &edit.params, None);
        if out == Outcome::Done
            && let Some((epoch, now)) = self.epoch_rev()
            && now != rev
        {
            let text = format!("quick fix {rule}");
            // WP-S2.09 part 3 (§8.4): the history panel shows the annotation as the entry's origin.
            self.note_history_origin(now, &text);
            self.messages.annotations.insert((epoch, now), text);
        }
        out
    }
}

/// The toast of `messages.check` (FT-549): the errors and warnings always, the infos, waived and pending findings when
/// there are any: `Check: 1 error, 0 warnings`.
pub(crate) fn check_summary(c: MessageCounts) -> String {
    let count = |n: usize, one: &str, many: &str| match n {
        1 => strings::text(one).to_owned(),
        n => strings::format(many, &[&n.to_string()]),
    };
    let mut parts = vec![
        count(c.errors, "check-error", "check-errors"),
        count(c.warnings, "check-warning", "check-warnings"),
    ];
    for (n, one, many) in [
        (c.infos, "check-info", "check-infos"),
        (c.waived, "check-waived-one", "check-waived"),
        (c.pending, "check-pending-one", "check-pending"),
    ] {
        if n > 0 {
            parts.push(count(n, one, many));
        }
    }
    strings::format("toast-check", &[&parts.join(", ")])
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_summary_names_errors_and_warnings_always_and_the_rest_when_present() {
        let c = |errors, warnings, infos, waived, pending| MessageCounts {
            errors,
            warnings,
            infos,
            waived,
            pending,
        };
        assert_eq!(check_summary(c(1, 0, 0, 0, 0)), "Check: 1 error, 0 warnings");
        assert_eq!(check_summary(c(0, 1, 0, 0, 0)), "Check: 0 errors, 1 warning");
        assert_eq!(
            check_summary(c(2, 3, 1, 4, 1)),
            "Check: 2 errors, 3 warnings, 1 info, 4 waived, 1 pending"
        );
    }
}
