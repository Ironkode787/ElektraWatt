// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel in the shell: `NavKind::Messages` in the bottom dock, the
//! editing page and the pending rows, F8 and Shift+F8, F1 on a finding or rule row, the quick fix as a previewed card,
//! and Tools ▸ Run checks.
//!
//! **Calls**. The pane's table is the framework's: one `Session::navigator_table(Messages)` per revision while
//! it is open (one `Session::check` inside), none while it is closed. The editing page changes the pending rows
//! without a call. `messages.check` drops the table, so the end of the dispatch fetches it again; `messages.fix` runs
//! the online check once more to read the finding's quick fix (memoised), then previews the fix.

use ew_api::checks::{CheckMode, FindingState};
use ew_api::output::{NavKind, NavTable};
use ew_api::{AnyCommand, Submit};
use serde_json::{Map, Value};

use super::navigation::bump;
use super::tools::Kept;
use super::{Dialog, FormDialog, FormField, Mode, Outcome, ShellModel, failed, string_param};
use crate::canvas::PageId;
use crate::key;
use crate::layout::PaneKind;
use crate::navigators::messages::{self, FindingDetail, MessageCounts, OnlineFacts};
use crate::navigators::{NavRole, Navigator, Query, visible_rows};
use crate::notifications::NoteTone;
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, PendingEdit};
use crate::strings;
use crate::strings::Key;
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
            .map(strings::scheme_text)
    }

    /// The folio of the editing page, while there is one.
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

    /// The header counts of the open Messages pane.
    pub fn messages_counts(&self) -> Option<MessageCounts> {
        let t = self.navs.get(&NavKind::Messages)?.table()?;
        Some(messages::counts(t, self.editing_folio().as_deref()))
    }

    /// The counts the status bar's findings field shows: the open Messages pane's, else the last Check project's while
    /// the project is at the revision it checked (after a check with the pane closed the field is there, and
    /// its click opens the pane).
    pub fn status_counts(&self) -> Option<MessageCounts> {
        if let Some(c) = self.messages_counts() {
            return Some(c);
        }
        let project = self.project.as_ref()?;
        let at = (project.epoch, project.session.world().rev.0);
        self.messages
            .checked
            .filter(|(when, _, _)| *when == at)
            .map(|(_, _, c)| c)
    }

    /// The Messages tab's count chip (doc 16a §5.7): [`MessageCounts::open`] of the open pane's counts, the
    /// same rule as the status bar's and the check toast's counts; 0 while the pane is closed.
    pub fn messages_badge(&self) -> usize {
        self.messages_counts().map_or(0, |c| c.open())
    }

    /// Whether row `key` of navigator `kind` is a pending finding.
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

    /// The detail block of the Messages cursor's finding, with the waiver's ID while it is waived.
    pub fn messages_detail(&self) -> Option<FindingDetail> {
        let nav = self.navs.get(&NavKind::Messages)?;
        let key = nav.cursor_key()?;
        let mut d = nav.node_of(&key).and_then(|(_, n)| messages::detail(n))?;
        // The message in the UI language, by `report::localise`'s rule
        // (the rule's pattern of that language where it has one, the en-GB message otherwise), for the finding of the
        // online report with this fingerprint.
        let lang = strings::rule_language();
        if lang != "en-GB"
            && let Some(project) = &self.project
        {
            let report = project
                .session
                .check(project.session.online_scheme(), CheckMode::Online);
            if let Some(f) = report
                .findings
                .iter()
                .find(|f| f.fingerprint.to_string() == d.fingerprint)
            {
                d.message = ew_api::checks::messages::render(f, lang);
                // The quick fix's label by its key.
                d.fix = f
                    .fix
                    .as_ref()
                    .map(|q| ew_api::checks::messages::fix_label(q, lang));
            }
        }
        // The message stands on its own line, so it starts with a capital.
        d.message = strings::sentence_start(&d.message).into_owned();
        if d.waived {
            d.waiver = self
                .with_online(|o| o.waivers.get(&d.fingerprint).cloned())
                .flatten();
        }
        Some(d)
    }

    /// The polylines (sheet coordinates) of the wires of canvas group `group`'s page that belong to the potential of
    /// an open EW-POT-001 finding: the window draws them in the conflict colour over the page, a
    /// screen-only overlay like the selection; the page's display list, its print and its export do not change.
    /// A connection point a finding names adds the connections that end at it; a point with none
    /// is a ring, a closed polyline one grid step across, so it is drawn with the overlay's pen and colour.
    pub fn conflict_overlay(&self, group: usize) -> Vec<Vec<(i64, i64)>> {
        let Some(c) = self.canvas_of(group) else {
            return Vec::new();
        };
        // A refused layout drop's (doc 16a §7.3) outline and cross, on the focused group.
        if self.area().is_some_and(|a| a.focused == group) {
            let lines = self.tools.tool.conflict_lines();
            if !lines.is_empty() {
                return lines;
            }
        }
        let Some((members, cps)) = self.with_online(|o| (o.conflict.clone(), o.conflict_cps.clone())) else {
            return Vec::new();
        };
        c.page().conflict_lines(&members, &cps)
    }

    /// The connection IDs of the potentials of the open EW-POT-001 findings of the online report, sorted.
    pub fn conflict_members(&self) -> Vec<ew_api::output::Uuid> {
        self.with_online(|o| o.conflict.clone()).unwrap_or_default()
    }

    /// The open error findings of the online report of the current revision, each as its fingerprint and its message
    /// in the UI language (the check is memoised in the session's store, as [`Self::with_online`]'s is).
    pub(super) fn open_errors(&self) -> Vec<(String, String)> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let lang = strings::rule_language();
        project
            .session
            .check(project.session.online_scheme(), CheckMode::Online)
            .findings
            .iter()
            .filter(|f| {
                matches!(f.state, FindingState::Open) && f.severity == ew_api::checks::Severity::Error
            })
            .map(|f| {
                (
                    f.fingerprint.to_string(),
                    ew_api::checks::messages::render(f, lang),
                )
            })
            .collect()
    }

    /// `f` over the online facts of the current revision: one online check per (project session, revision), read
    /// only when a getter asks (memoised in the session's store, so a revision re-checks what it touched).
    fn with_online<R>(&self, f: impl FnOnce(&OnlineFacts) -> R) -> Option<R> {
        let project = self.project.as_ref()?;
        let at = (project.epoch, project.session.world().rev.0);
        let mut cache = self.messages.online.borrow_mut();
        if cache.as_ref().is_none_or(|c| c.at != at) {
            let report = project
                .session
                .check(project.session.online_scheme(), CheckMode::Online);
            let mut facts = OnlineFacts {
                at,
                ..OnlineFacts::default()
            };
            for finding in &report.findings {
                match &finding.state {
                    FindingState::Waived { waiver } => {
                        facts
                            .waivers
                            .insert(finding.fingerprint.to_string(), waiver.clone());
                    }
                    FindingState::Open if finding.rule == CONFLICT_RULE => {
                        // The finding names the potential's definitions (or its CPs' functions); the members are
                        // those of the definition that is the potential's ID, the others give none.
                        for t in &finding.targets {
                            let target = ew_api::output::HighlightTarget::Potential(t.id.as_uuid());
                            facts.conflict.extend(project.session.highlight_members(&target));
                        }
                        // A potential without a definition is keyed by the member CP its discriminator
                        // names (`attrs@<function>:<idx>`).
                        if let Some(cp) = named_cp(&finding.discriminator) {
                            facts.conflict_cps.push(cp);
                        }
                    }
                    FindingState::Open => {}
                }
            }
            facts.conflict.sort_unstable();
            facts.conflict.dedup();
            facts.conflict_cps.sort_unstable();
            facts.conflict_cps.dedup();
            *cache = Some(facts);
        }
        cache.as_ref().map(f)
    }

    /// The origin annotation the shell keeps for the commit that made revision `rev` of the open project session
    /// (`quick fix <rule ID>`), for the history panel.
    pub fn origin_annotation(&self, rev: u64) -> Option<&str> {
        let epoch = self.project.as_ref()?.epoch;
        self.messages.annotations.get(&(epoch, rev)).map(String::as_str)
    }

    // -----------------------------------------------------------------------------------------------------------
    // The editing page

    /// The project session and revision before an action runs (the dispatch's argument to
    /// [`ShellModel::track_editing_page`]).
    pub(crate) fn epoch_rev(&self) -> Option<(u64, u64)> {
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

    /// The Messages table of `project` now: `Session::navigator_table(Messages)` with the release's housekeeping
    /// warnings of this project session under Warnings ▸ Release.
    pub(super) fn messages_table_of(
        project: &super::Project,
        state: &messages::MessagesState,
    ) -> ew_api::output::NavTable {
        let notes = state
            .release_notes
            .as_ref()
            .filter(|(epoch, _)| *epoch == project.epoch)
            .map_or(&[][..], |(_, notes)| notes.as_slice());
        // The migration report, while it is the pane's choice and the project has an import.
        let mut table = if state.migration && project.session.has_lineage() {
            project.session.migration_table()
        } else {
            project.session.navigator_table(NavKind::Messages)
        };
        super::navigators::table_in_words(&mut table, project);
        messages::with_release_notes(table, notes)
    }

    /// Fetches the Messages table when it is not of the current revision (one counted `navigator_table` call).
    pub(super) fn fetch_messages_table(&mut self) {
        let rev = self.revision();
        let Some(project) = &self.project else { return };
        let Some(n) = self.navs.get(&NavKind::Messages) else {
            return;
        };
        if n.table().is_some_and(|t| Some(t.rev) == rev) {
            return;
        }
        let table = Self::messages_table_of(project, &self.messages);
        if let Some(n) = self.navs.get_mut(&NavKind::Messages) {
            n.table = Some(std::sync::Arc::new(table));
        }
        self.messages.scheme = Some(if self.messages.migration && project.session.has_lineage() {
            strings::text(key!("messages-scheme-migration"))
        } else {
            project.session.online_scheme().name
        });
        bump(&self.calls, |c| c.navigator_table += 1);
    }

    /// The name of the project's own scheme (the online checks'), which the Messages pane shows when it does not show
    /// the migration report.
    pub fn messages_project_scheme(&self) -> Option<&'static str> {
        self.project
            .as_ref()
            .map(|p| strings::scheme_text(p.session.online_scheme().name))
    }

    /// Whether the Messages pane offers the migration report beside the project's scheme (d): the project
    /// holds an import.
    pub fn messages_offers_migration(&self) -> bool {
        self.project.as_ref().is_some_and(|p| p.session.has_lineage())
    }

    /// Whether the Messages pane shows the migration report.
    pub fn messages_shows_migration(&self) -> bool {
        self.messages.migration && self.messages_offers_migration()
    }

    /// `messages.migration {on?}` (d, T11): the Messages pane shows the migration report (a run of the
    /// `migration` scheme, the import runs' findings among the live ones) instead of the project's scheme, or (`on:
    /// false`) the project's scheme again; without `on` it switches. Refused when the project holds no import.
    pub(crate) fn act_messages_migration(&mut self, p: &Map<String, Value>) -> Outcome {
        if !self.messages_offers_migration() {
            return Outcome::refused(key!("reason-no-lineage"));
        }
        let on = or_return!(super::jobs::bool_param(p, "on", !self.messages.migration));
        self.messages.migration = on;
        if let Some(n) = self.navs.get_mut(&NavKind::Messages) {
            n.table = None;
        }
        self.fetch_messages_table();
        Outcome::Done
    }

    /// `messages.show {on?}` (View ▸ Messages): `navigator.show` with the kind `messages` (the pane in the bottom
    /// dock, focused), or `on: false` to close it.
    pub(crate) fn act_messages_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let pane = PaneKind::Navigator(NavKind::Messages);
        // Doc 16a §5.7: the command that opens the pane closes it again; `on` sets it. Only a pane in
        // front closes; one behind another tab of its group (History) is brought to front.
        let in_front = self.layout().shown_in_place_of(pane) == Some(pane);
        let on = or_return!(super::jobs::bool_param(p, "on", !in_front));
        let mut q = Map::new();
        q.insert("kind".into(), Value::from(NavKind::Messages.word()));
        q.insert("on".into(), Value::from(on));
        let out = self.act_navigator_show(&q);
        if on && out == Outcome::Done {
            self.bring_navigator_to_front(NavKind::Messages);
        }
        out
    }

    /// `messages.next` (F8) and `messages.previous` (Shift+F8): the cursor to the next or previous finding row in the
    /// pane's order (its view, sort and filter, the collapsed groups included and then expanded), wrapping, and opens
    /// it; the pane is docked first when it is closed (the focus stays).
    pub(crate) fn act_messages_walk(&mut self, forward: bool) -> Outcome {
        if !self.navs.contains_key(&NavKind::Messages) {
            self.open_messages();
        } else {
            // A pane behind another tab of its group comes to front, so the opened finding is seen.
            self.bring_navigator_to_front(NavKind::Messages);
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
            return Outcome::refused(key!("reason-no-findings"));
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
    pub(crate) fn act_messages_check(&mut self) -> Outcome {
        self.messages.editing_page = None;
        if let Some(n) = self.navs.get_mut(&NavKind::Messages) {
            n.table = None;
        }
        self.fetch_messages_table();
        // The outcome counted, `Check: 1 error, 0 warnings`, from the pane's table or, with the pane closed,
        // from a table fetched for the count alone.
        // With the findings counted, which the next check compares as a set.
        let editing = self.editing_folio();
        let counted = |t: &NavTable| {
            (
                messages::counts(t, editing.as_deref()),
                messages::counted_findings(t, editing.as_deref()),
            )
        };
        let counts = match self.navs.get(&NavKind::Messages).and_then(|n| n.table()) {
            Some(t) => Some(counted(t)),
            None => self
                .project
                .as_ref()
                .map(|project| counted(&Self::messages_table_of(project, &self.messages))),
        };
        if let Some((c, findings)) = counts {
            // The last check of this project, which the toast's changes count from; only a check
            // of the same table, the same scheme and the migration report or not, so a switch reads as no change.
            let table = (
                self.project
                    .as_ref()
                    .map(|p| p.session.online_scheme().name)
                    .unwrap_or_default(),
                self.messages_shows_migration(),
            );
            let epoch = self.project.as_ref().map(|p| p.epoch);
            let before = self
                .messages
                .checked
                .filter(|((e, _), t, _)| Some(*e) == epoch && *t == table)
                .map(|(_, _, before)| {
                    (
                        before,
                        findings.difference(&self.messages.checked_findings).count(),
                    )
                });
            self.messages.checked = self
                .project
                .as_ref()
                .map(|p| ((p.epoch, p.session.world().rev.0), table, c));
            self.messages.checked_findings = findings;
            // A check that found something offers the way to its list (FT-1761).
            let action = (c.errors + c.warnings + c.infos > 0)
                .then(|| self.project.as_ref().map(|p| p.epoch))
                .flatten()
                .map(|epoch| {
                    (
                        crate::notifications::NoteAction::ShowMessages,
                        crate::notifications::Validity::Session {
                            epoch,
                            select: Vec::new(),
                        },
                    )
                });
            self.notes
                .push_toned(check_summary(c, before), "edit", action, Some(check_tone(c)));
            self.messages.check_toast = self
                .messages
                .checked
                .map(|(at, _, _)| (at, self.notes.newest_serial()));
        }
        Outcome::Done
    }

    /// The last check's toast goes once the project leaves the revision it counted (an edit, an undo, another
    /// project), so its counts never disagree with the dock's pending ones; the log keeps it.
    pub(super) fn sync_check_toast(&mut self) {
        let Some((at, serial)) = self.messages.check_toast else {
            return;
        };
        let now = self.project.as_ref().map(|p| (p.epoch, p.session.world().rev.0));
        if now != Some(at) {
            self.notes.dismiss(serial);
            self.messages.check_toast = None;
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Waive… and Unwaive

    /// The finding row `key` of the Messages pane (default the open Waive prompt's row, else the cursor row) when it
    /// is a finding in state `state`: (key, rule, fingerprint, subject as printed); else the refusal.
    fn finding_row(
        &self,
        p: &Map<String, Value>,
        state: &str,
    ) -> Result<(String, String, String, String), Outcome> {
        let key = match string_param(p, "key")? {
            Some(k) => k.to_owned(),
            None => match self.messages.waive.as_ref().filter(|_| self.in_form_of(WAIVE)) {
                Some((k, _)) => k.clone(),
                None => self
                    .nav_cursor(NavKind::Messages)
                    .ok_or(Outcome::refused(key!("reason-no-nav-row")))?,
            },
        };
        let Some(nav) = self.navs.get(&NavKind::Messages) else {
            return Err(failed(key!("error-nav-closed"), &[NavKind::Messages.word()]));
        };
        let Some((_, node)) = nav.node_of(&key) else {
            return Err(failed(key!("error-nav-key"), &[&key, NavKind::Messages.word()]));
        };
        let refusal = Outcome::Refused {
            reason_key: if state == "open" {
                key!("reason-not-open-finding")
            } else {
                key!("reason-not-waived")
            },
        };
        let Some(d) = messages::detail(node) else {
            return Err(refusal);
        };
        if d.waived != (state == "waived") || d.fingerprint.is_empty() {
            return Err(refusal);
        }
        Ok((key, d.rule, d.fingerprint, messages::subject(node)))
    }

    /// `messages.waive {key?, reason?, author?, date?}` (Waive… in the Messages detail): on an open finding
    /// row (default the cursor row) commits one `check.waive` with the rule, the fingerprint, the date and, when
    /// given, the reason and the author (both optional, any length). Without a reason the window opens the Waive
    /// prompt (Reason; Author prefilled from the preference "Author name"), which keeps the row and the date the
    /// window passed (`YYYY-MM-DD`: the window reads the clock, the shell never does); its Waive commits with both
    /// fields empty too. A missing date stays in the prompt and commits nothing; a commit with an author saves it back
    /// to the preference. `check.waive` is `HumanOnly`: headless the action refuses.
    pub(crate) fn act_messages_waive(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.mode == Mode::Headless {
            return Outcome::refused(key!("reason-human-only"));
        }
        let (key, rule, fingerprint, subject) = or_return!(self.finding_row(p, "open"));
        let text = |name: &str| -> Result<String, Outcome> {
            Ok(string_param(p, name)?.unwrap_or_default().trim().to_owned())
        };
        let (reason, author, date) = match (text("reason"), text("author"), text("date")) {
            (Ok(r), Ok(a), Ok(d)) => (r, a, d),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let date = if date.is_empty() {
            self.messages
                .waive
                .as_ref()
                .filter(|(k, _)| self.in_form_of(WAIVE) && *k == key)
                .and_then(|(_, d)| d.clone())
                .unwrap_or_default()
        } else {
            date
        };
        if !self.in_form_of(WAIVE) && reason.is_empty() {
            let author = self.settings.author_name.clone().unwrap_or_default();
            self.messages.waive = Some((key, (!date.is_empty()).then_some(date)));
            return self.open_dialog(Dialog::Form(FormDialog {
                action: WAIVE,
                title_key: key!("waive-title"),
                ok_key: key!("dialog-waive"),
                fields: vec![
                    FormField {
                        name: "reason",
                        label_key: key!("form-reason"),
                        choices: &super::choices::NONE,
                        options: Vec::new(),
                        value: String::new(),
                    },
                    FormField {
                        name: "author",
                        label_key: key!("form-author"),
                        choices: &super::choices::NONE,
                        options: Vec::new(),
                        value: author,
                    },
                ],
                error: None,
                table: None,
            }));
        }
        if date.parse::<ew_api::catalog::Date>().is_err() {
            return self.form_failed(WAIVE, strings::format(key!("error-missing-param"), &["date"]));
        }
        // The reason and the author are optional: an empty one is not sent.
        let mut params = serde_json::json!({ "rule": rule, "fingerprint": fingerprint, "date": date });
        for (name, text) in [("reason", &reason), ("author", &author)] {
            if !text.is_empty() {
                params[name] = Value::String(text.clone());
            }
        }
        // The finding's subject, which the refusal of a finding waived already names.
        if !subject.is_empty() {
            params["subject"] = Value::String(subject);
        }
        let cmd = AnyCommand {
            name: "check.waive".to_owned(),
            params,
        };
        let out = self.run_commit(&cmd);
        match out {
            Outcome::Done => {
                if self.in_form_of(WAIVE) {
                    self.dialog = None;
                }
                self.messages.waive = None;
                if !author.is_empty() && self.settings.author_name.as_deref() != Some(author.as_str()) {
                    self.settings.author_name = Some(author);
                    self.save_settings();
                }
                Outcome::Done
            }
            Outcome::Failed { message } => self.form_failed(WAIVE, message),
            other => other,
        }
    }

    /// `messages.unwaive {key?}` (Unwaive in the Messages detail): on a waived finding row (default the cursor
    /// row) commits one `check.unwaive` of its waiver, after which the finding is open again. Headless it refuses
    /// (`HumanOnly`).
    pub(crate) fn act_messages_unwaive(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.mode == Mode::Headless {
            return Outcome::refused(key!("reason-human-only"));
        }
        let (_, _, fingerprint, _) = or_return!(self.finding_row(p, "waived"));
        let Some(waiver) = self
            .with_online(|o| o.waivers.get(&fingerprint).cloned())
            .flatten()
        else {
            return Outcome::refused(key!("reason-not-waived"));
        };
        let cmd = AnyCommand {
            name: "check.unwaive".to_owned(),
            params: serde_json::json!({ "waiver": waiver }),
        };
        self.run_commit(&cmd)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Quick fixes

    /// `messages.fix {key?}`: previews the quick fix of a finding row (default the cursor row) and opens its card in the
    /// Messages pane, whatever FR-UI-044's policy would say; a finding without one refuses `reason-no-fix`.
    pub(crate) fn act_messages_fix(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k.to_owned(),
            Ok(None) => match self.nav_cursor(NavKind::Messages) {
                Some(k) => k,
                None => {
                    return Outcome::refused(key!("reason-no-nav-row"));
                }
            },
        };
        let Some(nav) = self.navs.get(&NavKind::Messages) else {
            return failed(key!("error-nav-closed"), &[NavKind::Messages.word()]);
        };
        let Some((_, node)) = nav.node_of(&key) else {
            return failed(key!("error-nav-key"), &[&key, NavKind::Messages.word()]);
        };
        if node.role != NavRole::Finding {
            return Outcome::refused(key!("reason-no-fix"));
        }
        let Some(fp) = messages::fingerprint_of(&key).map(str::to_owned) else {
            return Outcome::refused(key!("reason-no-fix"));
        };
        if self.project.is_none() {
            return Outcome::refused(key!("reason-no-project"));
        }
        let Some(finding) = self.finding_of(&fp) else {
            return Outcome::refused(key!("reason-no-fix"));
        };
        let Some(fix) = finding.fix.clone() else {
            return Outcome::refused(key!("reason-no-fix"));
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let rule = finding.rule.to_owned();
        let targets = finding.targets.len();
        let params = fix_params(&project.session, &fix);
        self.preview_fix(fix.command, params, rule, targets)
    }

    /// The finding of fingerprint `fp` in the run the Messages pane lists (the migration report's, or the online
    /// check's).
    fn finding_of(&self, fp: &str) -> Option<ew_api::checks::Finding> {
        let project = self.project.as_ref()?;
        let report = if self.messages_shows_migration() {
            project
                .session
                .check(&ew_api::checks::MIGRATION, CheckMode::Release)
        } else {
            project
                .session
                .check(project.session.online_scheme(), CheckMode::Online)
        };
        report
            .findings
            .iter()
            .find(|f| f.fingerprint.to_string() == fp)
            .cloned()
    }

    /// Opening a finding (`navigator.open` on a Messages row: a double click, Enter) that has no place on a page opens
    /// where it is mended: a text style's finding (EW-CORE-061) the Text styles page at that
    /// style; a finding of the project record Project properties; a finding of other records those records, selected,
    /// in Properties (a device drawn on no page, a cable without a part, a location). A finding of no record at all
    /// says what to do in a banner, its rule's hint.
    pub(super) fn open_finding_elsewhere(&mut self, key: &str) -> Outcome {
        let Some(finding) = messages::fingerprint_of(key).and_then(|fp| self.finding_of(fp)) else {
            return Outcome::Done;
        };
        if finding.rule == ew_api::checks::rules::page::TEXT_BELOW_MINIMUM.id {
            return self.act_text_styles_open(&Map::from_iter([(
                "style".to_owned(),
                Value::String(finding.discriminator.clone()),
            )]));
        }
        let project = self
            .project
            .as_ref()
            .map(|p| p.session.query().project().id.as_uuid());
        if finding.targets.iter().any(|t| Some(t.id.as_uuid()) == project) {
            return self.act_props_project();
        }
        if !finding.targets.is_empty() {
            let ids: Vec<Value> = finding
                .targets
                .iter()
                .map(|t| Value::String(t.id.as_uuid().to_string()))
                .collect();
            if self.act_select_set(&Map::from_iter([("ids".to_owned(), Value::Array(ids))])) == Outcome::Done
            {
                return self.act_props_focus();
            }
        }
        if let Some(hint) = ew_api::checks::messages::hint(finding.rule, strings::rule_language()) {
            self.banner = Some(hint);
        }
        Outcome::Done
    }

    /// The Fix all of the Messages row `key`: a rule group whose findings all have a quick fix of
    /// one kind, or a finding of such a group (Fix all like this): the group's key and its number of findings.
    pub fn fix_all_offer(&self, key: &str) -> Option<(String, usize)> {
        let table = self.navs.get(&NavKind::Messages)?.table()?;
        let (_, node) = table.nodes.iter().enumerate().find(|(_, n)| n.key == key)?;
        let group = match node.role {
            NavRole::Rule => node,
            NavRole::Finding => table.nodes.get(usize::try_from(node.parent?).ok()?)?,
            _ => return None,
        };
        if group.role != NavRole::Rule || !group.badges.iter().any(|b| b == ew_api::output::FIX_ALL) {
            return None;
        }
        let n = group.badges.first()?.parse().ok()?;
        Some((group.key.clone(), n))
    }

    /// `messages.fix_all {key}`: the quick fixes of every finding of the group of row `key` (a rule group, or one of
    /// its findings) as one card: one `check.fix_all`, one Apply, one undo step. A fix that its command refuses now
    /// (what it changes changed since the check) is left out, and a toast says how many.
    pub(crate) fn act_messages_fix_all(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["key"]),
        };
        let Some((group, _)) = self.fix_all_offer(&key) else {
            return Outcome::refused(key!("reason-no-fix"));
        };
        let prints: Vec<String> = self
            .navs
            .get(&NavKind::Messages)
            .and_then(|n| n.table())
            .map(|t| {
                let at = t.nodes.iter().position(|n| n.key == group);
                t.nodes
                    .iter()
                    .filter(|n| {
                        n.role == NavRole::Finding && n.parent.and_then(|p| usize::try_from(p).ok()) == at
                    })
                    .filter_map(|n| messages::fingerprint_of(&n.key).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let migration = self.messages_shows_migration();
        let submit = Submit::from(self.origin());
        let Some(project) = &mut self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let report = if migration {
            project
                .session
                .check(&ew_api::checks::MIGRATION, CheckMode::Release)
        } else {
            project
                .session
                .check(project.session.online_scheme(), CheckMode::Online)
        };
        let found: Vec<&ew_api::checks::Finding> = report
            .findings
            .iter()
            .filter(|f| prints.contains(&f.fingerprint.to_string()))
            .collect();
        let Some(first) = found.first() else {
            return Outcome::refused(key!("reason-no-fix"));
        };
        let rule = first.rule.to_owned();
        let kind = first
            .fix
            .as_ref()
            .map(|x| x.key.map_or_else(|| x.command.clone(), str::to_owned))
            .unwrap_or_default();
        let mut fixes = Vec::new();
        // The findings whose fix can be done: one finding's fix may be several commands (a valve with two open
        // ports), so the count of commands says nothing about how many findings are left.
        let mut n = 0usize;
        for f in &found {
            let Some(fix) = &f.fix else { continue };
            let cmd = AnyCommand {
                name: fix.command.clone(),
                params: fix_params(&project.session, fix),
            };
            if project.session.preview_with(&cmd, submit.clone()).is_ok() {
                n += 1;
                // A fix that is itself several (a valve's open ports plugged) joins as its parts.
                match cmd.params.get("fixes").and_then(Value::as_array) {
                    Some(parts) if fix.command == "check.fix_all" => fixes.extend(parts.iter().cloned()),
                    _ => fixes.push(serde_json::json!({ "cmd": fix.command, "params": cmd.params })),
                }
            }
        }
        let left = found.len() - n;
        let fixes = merge_list_fixes(fixes);
        if fixes.is_empty() {
            return Outcome::refused(key!("reason-no-fix"));
        }
        self.messages.fix_all = Some((kind, n));
        if left > 0 {
            self.notify(
                strings::format(key!("toast-fix-all-left"), &[&left.to_string()]),
                false,
            );
        }
        self.preview_fix(
            "check.fix_all".to_owned(),
            serde_json::json!({ "fixes": fixes }),
            rule,
            n,
        )
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
        let submit = Submit::from(self.origin());
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
            reason: key!("reason-quick-fix"),
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
        let out = self.commit(edit.name.clone(), edit.params.clone(), Kept::Card(preview, None));
        // Fix all says what it did, with Undo: "29 pins marked not used."
        if out == Outcome::Done
            && edit.name == "check.fix_all"
            && let Some((kind, n)) = self.messages.fix_all.take()
        {
            let own = format!("toast-fix-all-{}", kind.replace(['.', '_'], "-"));
            let text = match strings::lookup(&own) {
                Some(_) => strings::format(Key::run_time(&own), &[&n.to_string()]),
                None => strings::format(key!("toast-fix-all"), &[&n.to_string()]),
            };
            self.notify(text, true);
        }
        if out == Outcome::Done
            && let Some((epoch, now)) = self.epoch_rev()
            && now != rev
        {
            let text = format!("quick fix {rule}");
            // The history panel shows the annotation as the entry's origin.
            self.note_history_origin(now, &text);
            self.messages.annotations.insert((epoch, now), text);
        }
        out
    }
}

/// Fix all's fixes with those that set the same list of one record merged: each finding's fix sets the whole list (a
/// function's connection point overrides, each computed alone), so two on one function would undo each other. Their
/// items are merged by `idx`, a later fix's item replacing an earlier one's; every other fix stays as it is.
fn merge_list_fixes(fixes: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for f in fixes {
        let p = &f["params"];
        let same = |o: &Value| {
            o["cmd"] == "fields.set"
                && f["cmd"] == "fields.set"
                && o["params"]["kind"] == p["kind"]
                && o["params"]["targets"] == p["targets"]
                && o["params"]["path"] == p["path"]
                && o["params"]["value"].is_array()
                && p["value"].is_array()
        };
        let Some(earlier) = out.iter_mut().find(|o| same(o)) else {
            out.push(f);
            continue;
        };
        let items = p["value"].as_array().cloned().unwrap_or_default();
        if let Some(list) = earlier["params"]["value"].as_array_mut() {
            for item in items {
                match list
                    .iter_mut()
                    .find(|x| x.get("idx").is_some() && x.get("idx") == item.get("idx"))
                {
                    Some(x) => *x = item,
                    None => list.push(item),
                }
            }
            list.sort_by_key(|x| x.get("idx").and_then(Value::as_u64));
        }
    }
    out
}

/// The action of the Waive prompt.
const WAIVE: &str = "messages.waive";

/// The rule whose open findings colour their potential's wires on the canvas.
const CONFLICT_RULE: &str = "EW-POT-001";

/// The connection point (function, CP index) a finding's discriminator names after its `@` (`attrs@<function>:<idx>`,
/// the member CP that keys a potential without a definition), if it names one.
fn named_cp(discriminator: &str) -> Option<(ew_api::output::Uuid, u16)> {
    let (_, cp) = discriminator.rsplit_once('@')?;
    let (function, idx) = cp.split_once(':')?;
    Some((ew_api::output::Uuid::parse_str(function).ok()?, idx.parse().ok()?))
}

/// The toast of `messages.check`: the errors and warnings always, the infos when there are any; the waived and
/// pending findings, when there are any, qualify the title so the line of counts stays one line:
/// `Check: 1 error, 0 warnings`, `Check · 1 waived: 1 error, 0 warnings`. After an earlier check of
/// the same project (`before`), the line leads with what changed since then, then the project's totals:
/// `Check: −1 error since the last check; project: 0 errors, 2 warnings`. `before` carries how many of
/// this check's findings the earlier one did not count, so that equal counts of other findings read "2 findings
/// changed", not "no change".
pub(crate) fn check_summary(c: MessageCounts, before: Option<(MessageCounts, usize)>) -> String {
    let count = |n: usize, one: Key<'_>, many: Key<'_>| match n {
        1 => strings::text(one).to_owned(),
        n => strings::format(many, &[&n.to_string()]),
    };
    let mut parts = vec![
        count(c.errors, key!("check-error"), key!("check-errors")),
        count(c.warnings, key!("check-warning"), key!("check-warnings")),
    ];
    if c.infos > 0 {
        parts.push(count(c.infos, key!("check-info"), key!("check-infos")));
    }
    if let Some((b, new)) = before {
        let changes: Vec<String> = [
            (b.errors, c.errors, key!("check-error"), key!("check-errors")),
            (
                b.warnings,
                c.warnings,
                key!("check-warning"),
                key!("check-warnings"),
            ),
            (b.infos, c.infos, key!("check-info"), key!("check-infos")),
        ]
        .into_iter()
        .filter(|(was, now, _, _)| was != now)
        .map(|(was, now, one, many)| {
            let sign = if now > was { "+" } else { "\u{2212}" };
            format!("{sign}{}", count(now.abs_diff(was), one, many))
        })
        .collect();
        let changes = if changes.is_empty() && new > 0 {
            count(new, key!("check-changed-one"), key!("check-changed"))
        } else if changes.is_empty() {
            strings::text(key!("check-unchanged")).to_owned()
        } else {
            changes.join(", ")
        };
        parts = vec![strings::format(
            key!("check-since"),
            &[&changes, &parts.join(", ")],
        )];
    }
    // The waived and pending counts qualify the title ("Check · 4 waived · 1 pending"), so the line of
    // severity counts stays one line in the toast.
    let qualifiers: Vec<String> = [
        (c.waived, key!("check-waived-one"), key!("check-waived")),
        (c.pending, key!("check-pending-one"), key!("check-pending")),
    ]
    .into_iter()
    .filter(|(n, _, _)| *n > 0)
    .map(|(n, one, many)| count(n, one, many))
    .collect();
    if qualifiers.is_empty() {
        strings::format(key!("toast-check"), &[&parts.join(", ")])
    } else {
        strings::format(
            key!("toast-check-qualified"),
            &[&parts.join(", "), &qualifiers.join(" · ")],
        )
    }
}
/// The icon of the check's toast (doc 16a §6.10, §6.24): the worst severity present, error before warning before info;
/// a check that finds nothing is a success. Waived and pending findings set no tone.
pub(crate) fn check_tone(c: MessageCounts) -> NoteTone {
    if c.errors > 0 {
        NoteTone::Error
    } else if c.warnings > 0 {
        NoteTone::Warning
    } else if c.infos > 0 {
        NoteTone::Info
    } else {
        NoteTone::Success
    }
}

/// A quick fix's parameters with the library documents its command draws with: EW-CORE-047's Draw its valves in
/// the terminal takes the symbols of the terminal's valves and its supply
/// ([`super::fluid::draw_valves_params`]); any other fix's are the rule's.
fn fix_params(session: &ew_api::Session, fix: &ew_api::checks::QuickFix) -> Value {
    match fix.params["terminal"].as_str().and_then(|t| t.parse().ok()) {
        Some(terminal) if fix.command == "valve_terminal.draw_valves" => {
            super::fluid::draw_valves_params(session, terminal)
        }
        _ => fix.params.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The member CP a CP-keyed EW-POT-001 finding's discriminator names; none for a definition's.
    #[test]
    fn the_discriminator_names_the_keying_connection_point() {
        let f = "0190a5c4-1001-7000-8000-000000000001";
        assert_eq!(
            named_cp(&format!("attrs@{f}:2")),
            Some((ew_api::output::Uuid::parse_str(f).unwrap(), 2))
        );
        assert_eq!(named_cp("attrs"), None);
        assert_eq!(named_cp("attrs@not-an-id:2"), None);
    }

    #[test]
    fn check_tone_follows_the_worst_severity_present() {
        let c = |errors, warnings, infos| MessageCounts {
            errors,
            warnings,
            infos,
            waived: 2,
            pending: 1,
        };
        assert_eq!(check_tone(c(4, 25, 12)), NoteTone::Error);
        assert_eq!(check_tone(c(1, 0, 0)), NoteTone::Error);
        assert_eq!(check_tone(c(0, 25, 12)), NoteTone::Warning);
        assert_eq!(check_tone(c(0, 0, 12)), NoteTone::Info);
        assert_eq!(check_tone(c(0, 0, 0)), NoteTone::Success, "clean");
    }

    #[test]
    fn check_summary_names_errors_and_warnings_always_and_the_rest_when_present() {
        let c = |errors, warnings, infos, waived, pending| MessageCounts {
            errors,
            warnings,
            infos,
            waived,
            pending,
        };
        assert_eq!(
            check_summary(c(1, 0, 0, 0, 0), None),
            "Check: 1 error, 0 warnings."
        );
        assert_eq!(
            check_summary(c(0, 1, 0, 0, 0), None),
            "Check: 0 errors, 1 warning."
        );
        // Waived and pending qualify the title; the line keeps the severities.
        assert_eq!(
            check_summary(c(2, 3, 1, 4, 1), None),
            "Check · 4 waived · 1 pending: 2 errors, 3 warnings, 1 info."
        );
        assert_eq!(
            check_summary(c(0, 25, 0, 1, 0), None),
            "Check · 1 waived: 0 errors, 25 warnings."
        );
        // After an earlier check, what changed leads, then the project's totals.
        assert_eq!(
            check_summary(c(32, 4, 0, 0, 0), Some((c(33, 3, 1, 0, 0), 1))),
            "Check: \u{2212}1 error, +1 warning, \u{2212}1 info since the last check; project: 32 errors, 4 warnings."
        );
        assert_eq!(
            check_summary(c(33, 3, 0, 1, 0), Some((c(33, 3, 0, 0, 0), 0))),
            "Check · 1 waived: no change since the last check; project: 33 errors, 3 warnings."
        );
        // The same counts of other findings are a change, counted by the new findings.
        assert_eq!(
            check_summary(c(33, 3, 0, 0, 0), Some((c(33, 3, 0, 0, 0), 2))),
            "Check: 2 findings changed since the last check; project: 33 errors, 3 warnings."
        );
        assert_eq!(
            check_summary(c(33, 3, 0, 0, 0), Some((c(33, 3, 0, 0, 0), 1))),
            "Check: 1 finding changed since the last check; project: 33 errors, 3 warnings."
        );
        assert!(strings::set_locale("nl"));
        let dutch = check_summary(c(32, 3, 0, 0, 0), Some((c(33, 3, 0, 0, 0), 0)));
        let dutch_changed = check_summary(c(33, 3, 0, 0, 0), Some((c(33, 3, 0, 0, 0), 2)));
        assert!(strings::set_locale("en"));
        assert_eq!(
            dutch_changed,
            "Controle: 2 bevindingen gewijzigd sinds de vorige controle; project: 33 fouten, 3 waarschuwingen."
        );
        assert_eq!(
            dutch,
            "Controle: \u{2212}1 fout sinds de vorige controle; project: 32 fouten, 3 waarschuwingen."
        );
    }

    /// Check project compares the findings themselves (rule, subject, message) as a set: a check whose
    /// counts equal the last one's but whose findings differ reports the change ("1 finding changed"), not "no
    /// change"; the same findings again are no change.
    #[test]
    fn a_check_of_other_findings_with_the_same_counts_reports_the_change() {
        use crate::shell::{Outcome, ShellModel};
        use serde_json::json;
        let mut shell = ShellModel::for_tests();
        let base = crate::shell::fresh_test_dir("ew-check-findings");
        let dir = base.join("g.ew");
        let sample = std::path::Path::new(ew_testdir::SAMPLE);
        crate::shell::properties::tests::copy_dir(sample, &dir);
        let run = |shell: &mut ShellModel, action: &str, p: serde_json::Value| {
            assert_eq!(shell.dispatch(action, &p), Outcome::Done, "{action}");
        };
        let toast = |shell: &ShellModel| shell.toast().expect("the check toast").text.clone();
        run(&mut shell, "file.open", json!({ "dir": dir.to_string_lossy() }));
        run(&mut shell, "messages.check", json!({}));
        let c = shell.status_counts().expect("the checked counts");
        // The last check counted one finding this one has not, in place of one it has: the counts are equal.
        let one = shell
            .messages
            .checked_findings
            .iter()
            .next()
            .cloned()
            .expect("a finding");
        shell.messages.checked_findings.remove(&one);
        shell
            .messages
            .checked_findings
            .insert((one.0.clone(), "-Z99".to_owned(), one.2.clone()));
        run(&mut shell, "messages.check", json!({}));
        assert_eq!(toast(&shell), check_summary(c, Some((c, 1))));
        run(&mut shell, "messages.check", json!({}));
        assert_eq!(toast(&shell), check_summary(c, Some((c, 0))), "the same findings");
        shell.shutdown();
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Check project counts its changes only from a check of the same table. A switch of the
    /// project's scheme between two checks gives no "since the last check": the toast is the plain one of the new
    /// table.
    #[test]
    fn a_scheme_switch_between_two_checks_reports_no_change() {
        use crate::shell::{Outcome, ShellModel};
        use ew_api::{AnyCommand, Origin, Submit};
        use serde_json::json;
        let mut shell = ShellModel::for_tests();
        let base = crate::shell::fresh_test_dir("ew-check-scheme");
        let dir = base.join("g.ew");
        let sample = std::path::Path::new(ew_testdir::SAMPLE);
        crate::shell::properties::tests::copy_dir(sample, &dir);
        let run = |shell: &mut ShellModel, action: &str, p: serde_json::Value| {
            assert_eq!(shell.dispatch(action, &p), Outcome::Done, "{action}");
        };
        run(&mut shell, "file.open", json!({ "dir": dir.to_string_lossy() }));
        run(&mut shell, "messages.check", json!({}));
        run(&mut shell, "messages.check", json!({}));
        let c = shell.status_counts().expect("the checked counts");
        let toast = |shell: &ShellModel| shell.toast().expect("the check toast").text.clone();
        assert_eq!(
            toast(&shell),
            check_summary(c, Some((c, 0))),
            "the same table: no change"
        );
        let cmd = AnyCommand {
            name: "project.checks.set".into(),
            params: json!({"active_scheme": "full"}),
        };
        let session = &mut shell.project.as_mut().unwrap().session;
        session
            .execute_with(&cmd, Submit::from(Origin::Gui))
            .expect("the GUI sets the scheme");
        run(&mut shell, "messages.check", json!({}));
        let c = shell.status_counts().expect("the checked counts");
        assert_eq!(toast(&shell), check_summary(c, None), "another table: no since");
        shell.shutdown();
        let _ = std::fs::remove_dir_all(&base);
    }
}
