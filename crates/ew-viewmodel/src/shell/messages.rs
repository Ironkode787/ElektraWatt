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

use ew_api::checks::{CheckMode, FindingState};
use ew_api::output::NavKind;
use ew_api::{AnyCommand, Submit};
use serde_json::{Map, Value};

use super::navigation::{Focus, bump};
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

    /// The counts the status bar's findings field shows: the open Messages pane's, else the last Check project's while
    /// the project is at the revision it checked (FX38d: after a check with the pane closed the field is there, and
    /// its click opens the pane).
    pub fn status_counts(&self) -> Option<MessageCounts> {
        if let Some(c) = self.messages_counts() {
            return Some(c);
        }
        let project = self.project.as_ref()?;
        let at = (project.epoch, project.session.world().rev.0);
        self.messages
            .checked
            .filter(|(when, _)| *when == at)
            .map(|(_, c)| c)
    }

    /// The Messages tab's count chip (doc 16a §5.7, FT-710): [`MessageCounts::open`] of the open pane's counts, the
    /// same rule as the status bar's and the check toast's counts; 0 while the pane is closed.
    pub fn messages_badge(&self) -> usize {
        self.messages_counts().map_or(0, |c| c.open())
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

    /// The detail block of the Messages cursor's finding (§7.3), with the waiver's ID while it is waived (FT-674).
    pub fn messages_detail(&self) -> Option<FindingDetail> {
        let nav = self.navs.get(&NavKind::Messages)?;
        let key = nav.cursor_key()?;
        let mut d = nav.node_of(&key).and_then(|(_, n)| messages::detail(n))?;
        // WP-S2.08 part 3 (D10, ID-1219 ruling 6): the message in the UI language, by `report::localise`'s rule
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
            }
        }
        // FT-1140 (ID-1229 item 9): the message stands on its own line, so it starts with a capital (ID-1182).
        d.message = strings::sentence_start(&d.message).into_owned();
        if d.waived {
            d.waiver = self
                .with_online(|o| o.waivers.get(&d.fingerprint).cloned())
                .flatten();
        }
        Some(d)
    }

    /// The polylines (sheet coordinates) of the wires of canvas group `group`'s page that belong to the potential of
    /// an open EW-POT-001 finding (FT-675, ID-828): the window draws them in the conflict colour over the page, a
    /// screen-only overlay like the selection; the page's display list, its print and its export do not change.
    /// FT-696 (WP-S2.11 D9): a connection point a finding names adds the connections that end at it; a point with none
    /// is a ring, a closed polyline one grid step across, so it is drawn with the overlay's pen and colour.
    pub fn conflict_overlay(&self, group: usize) -> Vec<Vec<(i64, i64)>> {
        let Some(c) = self.canvas_of(group) else {
            return Vec::new();
        };
        // WP-S4.02 part 3 (doc 16a §7.3): a refused layout drop's outline and cross, on the focused group.
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

    /// The connection IDs of the potentials of the open EW-POT-001 findings of the online report, sorted (FT-675).
    pub fn conflict_members(&self) -> Vec<ew_api::output::Uuid> {
        self.with_online(|o| o.conflict.clone()).unwrap_or_default()
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
                        // FT-696: a potential without a definition is keyed by the member CP its discriminator
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

    /// The Messages table of `project` now: `Session::navigator_table(Messages)` with the release's housekeeping
    /// warnings of this project session under Warnings ▸ Release (FT-842).
    pub(super) fn messages_table_of(
        project: &super::Project,
        state: &messages::MessagesState,
    ) -> ew_api::output::NavTable {
        let notes = state
            .release_notes
            .as_ref()
            .filter(|(epoch, _)| *epoch == project.epoch)
            .map_or(&[][..], |(_, notes)| notes.as_slice());
        // WP-S3.09 D9 d: the migration report, while it is the pane's choice and the project has an import.
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

    /// Whether the Messages pane offers the migration report beside the project's scheme (WP-S3.09 D9 d): the project
    /// holds an import.
    pub fn messages_offers_migration(&self) -> bool {
        self.project.as_ref().is_some_and(|p| p.session.has_lineage())
    }

    /// Whether the Messages pane shows the migration report.
    pub fn messages_shows_migration(&self) -> bool {
        self.messages.migration && self.messages_offers_migration()
    }

    /// `messages.migration {on?}` (WP-S3.09 D9 d, T11): the Messages pane shows the migration report (a run of the
    /// `migration` scheme, the import runs' findings among the live ones) instead of the project's scheme, or (`on:
    /// false`) the project's scheme again; without `on` it switches. Refused when the project holds no import.
    pub(crate) fn act_messages_migration(&mut self, p: &Map<String, Value>) -> Outcome {
        if !self.messages_offers_migration() {
            return Outcome::Refused {
                reason_key: key!("reason-no-lineage"),
            };
        }
        let on = match super::jobs::bool_param(p, "on", !self.messages.migration) {
            Ok(on) => on,
            Err(e) => return e,
        };
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
        // FX38d (doc 16a §5.7): the command that opens the pane closes it again; `on` sets it. FT-916: only a pane in
        // front closes; one behind another tab of its group (History) is brought to front.
        let in_front = self.layout().shown_in_place_of(pane) == Some(pane);
        let on = match super::jobs::bool_param(p, "on", !in_front) {
            Ok(on) => on,
            Err(e) => return e,
        };
        let mut q = Map::new();
        q.insert("kind".into(), Value::from(NavKind::Messages.word()));
        q.insert("on".into(), Value::from(on));
        let out = self.act_navigator_show(&q);
        if on && out == Outcome::Done {
            self.bring_messages_to_front();
        }
        out
    }

    /// FT-916: the Messages pane, when it is docked behind another tab of its group (History), becomes the shown tab.
    fn bring_messages_to_front(&mut self) {
        let layout = self.layout().showing(PaneKind::Navigator(NavKind::Messages));
        if layout != *self.layout() {
            self.replace_layout(layout);
        }
    }

    /// `messages.next` (F8) and `messages.previous` (Shift+F8): the cursor to the next or previous finding row in the
    /// pane's order (its view, sort and filter, the collapsed groups included and then expanded), wrapping, and opens
    /// it; the pane is docked first when it is closed (the focus stays).
    pub(crate) fn act_messages_walk(&mut self, forward: bool) -> Outcome {
        if !self.navs.contains_key(&NavKind::Messages) {
            self.open_messages();
        } else {
            // FT-916: a pane behind another tab of its group comes to front, so the opened finding is seen.
            self.bring_messages_to_front();
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
                reason_key: key!("reason-no-findings"),
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
    pub(crate) fn act_messages_check(&mut self) -> Outcome {
        self.messages.editing_page = None;
        if let Some(n) = self.navs.get_mut(&NavKind::Messages) {
            n.table = None;
        }
        self.fetch_messages_table();
        // FT-549: the outcome counted, `Check: 1 error, 0 warnings`, from the pane's table or, with the pane closed,
        // from a table fetched for the count alone.
        let counts = self.messages_counts().or_else(|| {
            let project = self.project.as_ref()?;
            let table = Self::messages_table_of(project, &self.messages);
            Some(messages::counts(&table, self.editing_folio().as_deref()))
        });
        if let Some(c) = counts {
            self.messages.checked = self
                .project
                .as_ref()
                .map(|p| ((p.epoch, p.session.world().rev.0), c));
            self.notify_toned(check_summary(c), check_tone(c));
            self.messages.check_toast = self
                .messages
                .checked
                .map(|(at, _)| (at, self.notes.newest_serial()));
        }
        Outcome::Done
    }

    /// FT-759: the last check's toast goes once the project leaves the revision it counted (an edit, an undo, another
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
    // Waive… and Unwaive (FT-674; WP-S2.09, WP-S1.16 D7)

    /// The finding row `key` of the Messages pane (default the open Waive prompt's row, else the cursor row) when it
    /// is a finding in state `state`: (key, rule, fingerprint); else the refusal.
    fn finding_row(&self, p: &Map<String, Value>, state: &str) -> Result<(String, String, String), Outcome> {
        let key = match string_param(p, "key")? {
            Some(k) => k.to_owned(),
            None => match self.messages.waive.as_ref().filter(|_| self.in_waive_form()) {
                Some((k, _)) => k.clone(),
                None => self.nav_cursor(NavKind::Messages).ok_or(Outcome::Refused {
                    reason_key: key!("reason-no-nav-row"),
                })?,
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
        Ok((key, d.rule, d.fingerprint))
    }

    /// Whether the open dialog is the Waive prompt.
    fn in_waive_form(&self) -> bool {
        matches!(&self.dialog, Some(Dialog::Form(f)) if f.action == "messages.waive")
    }

    /// A refusal of the Waive prompt's fields: it stays in the prompt when the prompt is open.
    fn waive_failed(&mut self, message: String) -> Outcome {
        if self.in_waive_form()
            && let Some(Dialog::Form(f)) = &mut self.dialog
        {
            f.error = Some(message.clone());
        }
        Outcome::Failed { message }
    }

    /// `messages.waive {key?, reason?, author?, date?}` (Waive… in the Messages detail, FT-674): on an open finding
    /// row (default the cursor row) commits one `check.waive` with the rule, the fingerprint, the reason (at least
    /// ten characters once trimmed, as the command checks), the author and the date. Without a reason the window
    /// opens the Waive prompt (Reason; Author prefilled from the preference "Author name"), which keeps the row and
    /// the date the window passed (`YYYY-MM-DD`: the window reads the clock, the shell never does). A short reason,
    /// an empty author or a missing date stays in the prompt and commits nothing; a commit saves the author back to
    /// the preference. `check.waive` is `HumanOnly`: headless the action refuses.
    pub(crate) fn act_messages_waive(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.mode == Mode::Headless {
            return Outcome::Refused {
                reason_key: key!("reason-human-only"),
            };
        }
        let (key, rule, fingerprint) = match self.finding_row(p, "open") {
            Ok(r) => r,
            Err(e) => return e,
        };
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
                .filter(|(k, _)| self.in_waive_form() && *k == key)
                .and_then(|(_, d)| d.clone())
                .unwrap_or_default()
        } else {
            date
        };
        if !self.in_waive_form() && reason.is_empty() {
            let author = self.settings.author_name.clone().unwrap_or_default();
            self.messages.waive = Some((key, (!date.is_empty()).then_some(date)));
            return self.open_dialog(Dialog::Form(FormDialog {
                action: "messages.waive",
                title_key: key!("waive-title"),
                ok_key: key!("dialog-waive"),
                fields: vec![
                    FormField {
                        name: "reason",
                        label_key: key!("form-reason"),
                        choices: &[],
                        options: Vec::new(),
                        value: String::new(),
                    },
                    FormField {
                        name: "author",
                        label_key: key!("form-author"),
                        choices: &[],
                        options: Vec::new(),
                        value: author,
                    },
                ],
                error: None,
                table: None,
            }));
        }
        let min = WAIVER_REASON_MIN_CHARS;
        if reason.chars().count() < min {
            return self.waive_failed(strings::format(key!("error-waive-reason"), &[&min.to_string()]));
        }
        if author.is_empty() {
            return self.waive_failed(strings::text(key!("error-waive-author")).to_owned());
        }
        if date.parse::<ew_api::catalog::Date>().is_err() {
            return self.waive_failed(strings::format(key!("error-missing-param"), &["date"]));
        }
        let cmd = AnyCommand {
            name: "check.waive".to_owned(),
            params: serde_json::json!({
                "rule": rule, "fingerprint": fingerprint, "reason": reason, "author": author, "date": date,
            }),
        };
        let out = self.run_commit(&cmd);
        match out {
            Outcome::Done => {
                if self.in_waive_form() {
                    self.dialog = None;
                }
                self.messages.waive = None;
                if self.settings.author_name.as_deref() != Some(author.as_str()) {
                    self.settings.author_name = Some(author);
                    self.save_settings();
                }
                Outcome::Done
            }
            Outcome::Failed { message } => self.waive_failed(message),
            other => other,
        }
    }

    /// `messages.unwaive {key?}` (Unwaive in the Messages detail, FT-674): on a waived finding row (default the cursor
    /// row) commits one `check.unwaive` of its waiver, after which the finding is open again. Headless it refuses
    /// (`HumanOnly`).
    pub(crate) fn act_messages_unwaive(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.mode == Mode::Headless {
            return Outcome::Refused {
                reason_key: key!("reason-human-only"),
            };
        }
        let (_, _, fingerprint) = match self.finding_row(p, "waived") {
            Ok(r) => r,
            Err(e) => return e,
        };
        let Some(waiver) = self
            .with_online(|o| o.waivers.get(&fingerprint).cloned())
            .flatten()
        else {
            return Outcome::Refused {
                reason_key: key!("reason-not-waived"),
            };
        };
        let cmd = AnyCommand {
            name: "check.unwaive".to_owned(),
            params: serde_json::json!({ "waiver": waiver }),
        };
        self.run_commit(&cmd)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Quick fixes (§7.4, D10)

    /// `messages.fix {key?}`: previews the quick fix of a finding row (default the cursor row) and opens its card in the
    /// Messages pane, whatever FR-UI-044's policy would say; a finding without one refuses `reason-no-fix`.
    pub(crate) fn act_messages_fix(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k.to_owned(),
            Ok(None) => match self.nav_cursor(NavKind::Messages) {
                Some(k) => k,
                None => {
                    return Outcome::Refused {
                        reason_key: key!("reason-no-nav-row"),
                    };
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
            return Outcome::Refused {
                reason_key: key!("reason-no-fix"),
            };
        }
        let Some(fp) = messages::fingerprint_of(&key).map(str::to_owned) else {
            return Outcome::Refused {
                reason_key: key!("reason-no-fix"),
            };
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let report = project
            .session
            .check(project.session.online_scheme(), CheckMode::Online);
        let Some(finding) = report.findings.iter().find(|f| f.fingerprint.to_string() == fp) else {
            return Outcome::Refused {
                reason_key: key!("reason-no-fix"),
            };
        };
        let Some(fix) = finding.fix.clone() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-fix"),
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

/// The shortest reason of a waiver in characters, trimmed: `check.waive`'s own minimum (doc 15 §2.5), checked here so
/// the prompt keeps a short reason without a command round trip; the command checks it again.
const WAIVER_REASON_MIN_CHARS: usize = 10;

/// The rule whose open findings colour their potential's wires on the canvas (FT-675).
const CONFLICT_RULE: &str = "EW-POT-001";

/// The connection point (function, CP index) a finding's discriminator names after its `@` (`attrs@<function>:<idx>`,
/// the member CP that keys a potential without a definition), if it names one.
fn named_cp(discriminator: &str) -> Option<(ew_api::output::Uuid, u16)> {
    let (_, cp) = discriminator.rsplit_once('@')?;
    let (function, idx) = cp.split_once(':')?;
    Some((ew_api::output::Uuid::parse_str(function).ok()?, idx.parse().ok()?))
}

/// The toast of `messages.check` (FT-549): the errors and warnings always, the infos when there are any; the waived and
/// pending findings, when there are any, qualify the title so the line of counts stays one line (FT-664):
/// `Check: 1 error, 0 warnings`, `Check · 1 waived: 1 error, 0 warnings`.
pub(crate) fn check_summary(c: MessageCounts) -> String {
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
    // FT-664: the waived and pending counts qualify the title ("Check · 4 waived · 1 pending"), so the line of
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

#[cfg(test)]
mod tests {
    use super::*;

    /// FT-696: the member CP a CP-keyed EW-POT-001 finding's discriminator names; none for a definition's.
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
        assert_eq!(check_summary(c(1, 0, 0, 0, 0)), "Check: 1 error, 0 warnings");
        assert_eq!(check_summary(c(0, 1, 0, 0, 0)), "Check: 0 errors, 1 warning");
        // FT-664: waived and pending qualify the title; the line keeps the severities.
        assert_eq!(
            check_summary(c(2, 3, 1, 4, 1)),
            "Check · 4 waived · 1 pending: 2 errors, 3 warnings, 1 info"
        );
        assert_eq!(
            check_summary(c(0, 25, 0, 1, 0)),
            "Check · 1 waived: 0 errors, 25 warnings"
        );
    }
}
