// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The history panel and context restore (doc 16 §8.1, FR-UI-023): the undo steps of the open
//! project with their origins and pages, the shell's marker lines (saves, exports, recoveries) by revision, the row
//! cursor, Show changes, and the editor context each commit was made in.
//!
//! **Rows** ([`HistoryPane::lines`]): `Session::history_view` (one call per revision while the panel is open, kept here
//! with its project session and revision) merged with the markers by revision, newest first; a marker sits after the
//! entry of its revision in time, so above it in the list. An entry's state is `redo` at and above the history's
//! cursor, `now` for the entry right below it and `done` below that. Its origin is the shell's annotation for its
//! revision (`quick fix <rule>`, [`crate::shell::ShellModel::note_history_origin`]) if any, else `Session`'s text.
//!
//! **Context** ([`HistoryContext`]): before every action the shell takes the focused tab's page, its viewport and the
//! window selection; when the action committed, the context is kept under the revision of each new history entry (at
//! most [`MAX_CONTEXTS`], the oldest dropped, cleared with the project). Undo, redo and Undo to here restore the context
//! of the step they crossed last. A step without a kept context (an external change, a recovery) restores nothing.
//! The transaction's own `TxContext` stays unused. Nothing here reads a clock or a path.

use std::collections::BTreeMap;

use ew_api::output::PreviewEntity;
use ew_api::window::{HistoryRow, HistoryView};

use ew_dl::EntityRef;

use crate::canvas::{PageId, Viewport};
use crate::shell::HistoryMarker;

/// The most contexts the shell keeps.
pub const MAX_CONTEXTS: usize = 500;

/// The most records Show changes lists.
pub const MAX_CHANGES: usize = 100;

/// Where a commit was made: the focused tab's page and viewport and the window selection before the gesture.
#[derive(Clone, PartialEq, Debug)]
pub struct HistoryContext {
    /// The page of the focused tab.
    pub page: PageId,
    /// Its viewport.
    pub viewport: Viewport,
    /// The selected entities, in selection order.
    pub selection: Vec<EntityRef>,
}

/// The state of an entry relative to the history's cursor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryState {
    /// Undone: in the redo branch.
    Redo,
    /// The entry right below the cursor: the state the model is in.
    Now,
    /// Applied, below the `now` entry.
    Done,
}

impl EntryState {
    /// The describe word.
    pub fn word(self) -> &'static str {
        match self {
            EntryState::Redo => "redo",
            EntryState::Now => "now",
            EntryState::Done => "done",
        }
    }
}

/// One line of the panel, newest first.
#[derive(Clone, Copy, Debug)]
pub enum HistoryLine<'a> {
    /// An entry of the history.
    Entry {
        /// The row.
        row: &'a HistoryRow,
        /// Its state.
        state: EntryState,
        /// The origin text (the shell's annotation, else the row's).
        origin: &'a str,
    },
    /// A marker line of the shell (view state).
    Marker(&'a HistoryMarker),
}

/// The history panel's state.
#[derive(Default, Debug)]
pub struct HistoryPane {
    /// The view of (project session, revision) while the panel is open.
    pub(crate) view: Option<(u64, u64, HistoryView)>,
    /// The row cursor: an entry index.
    pub(crate) row: Option<usize>,
    /// Show changes: the entry index and its records, while the popup is open.
    pub(crate) changes: Option<(usize, Vec<PreviewEntity>)>,
    /// The shell's origin annotations by revision (`quick fix EW-CORE-023`).
    pub(crate) origins: BTreeMap<u64, String>,
    /// The contexts by revision.
    pub(crate) contexts: BTreeMap<u64, HistoryContext>,
    /// The revision of the step the last undo or redo crossed.
    pub(crate) crossed: Option<u64>,
    /// While Undo to here walks: the steps restore nothing until the last.
    pub(crate) quiet: bool,
    /// While a toast's Undo runs: the toast named the step, so the Undo says nothing more.
    pub(crate) unsaid: bool,
}

impl HistoryPane {
    /// The view on show, if the panel is open and has fetched one.
    pub fn view(&self) -> Option<&HistoryView> {
        self.view.as_ref().map(|(_, _, v)| v)
    }

    /// The row cursor (an entry index), when one was set.
    pub fn row_cursor(&self) -> Option<usize> {
        self.row
    }

    /// Show changes, while its popup is open: the entry index and its records (at most [`MAX_CHANGES`] are shown).
    pub fn changes(&self) -> Option<(usize, &[PreviewEntity])> {
        self.changes.as_ref().map(|(i, v)| (*i, v.as_slice()))
    }

    /// The panel's header line: "`n` steps · cursor at `c`", "1 step · cursor at `c`" for one; `None` without
    /// a view.
    pub fn header(&self) -> Option<String> {
        let view = self.view()?;
        let key = if view.rows.len() == 1 {
            crate::key!("history-header-one")
        } else {
            crate::key!("history-header")
        };
        Some(crate::strings::format(
            key,
            &[&view.rows.len().to_string(), &view.cursor.to_string()],
        ))
    }

    /// Doc 16a §8: an entry's label in words, its command's title ("Create page" for
    /// `page.create`); a step that moved symbols is "Arrange and route", one that routed wires alone "Route wires"
    /// (docs/design/06a-drafting.md §7.3); a step of the project settings is named by what it sets ([`setting_words`]).
    pub fn label_words(row: &HistoryRow) -> String {
        if row.label == ew_api::window::ARRANGE_STEP {
            crate::strings::text(crate::key!("action-layout-arrange")).to_owned()
        } else if let Some(member) = row.label.strip_prefix(ew_api::window::SETTINGS_STEP) {
            setting_words(member)
        } else {
            crate::strings::command_title(&row.label).to_owned()
        }
    }

    /// An origin text ([`HistoryPane::origin_of`]) in words in the UI language: "by hand" for the window's,
    /// "from `<name>`" for a script, a log or a client, "changed on disk", "import", "merge", "quick fix `<rule>`"; any
    /// other text as it is.
    pub fn origin_words(origin: &str) -> String {
        use crate::strings::{format, text};
        match origin {
            "GUI" => text(crate::key!("history-origin-gui")).to_owned(),
            "external change" => text(crate::key!("history-origin-external")).to_owned(),
            "import" => text(crate::key!("history-origin-import")).to_owned(),
            "merge" => text(crate::key!("history-origin-merge")).to_owned(),
            o => {
                if let Some(rule) = o.strip_prefix("quick fix ") {
                    format(crate::key!("history-origin-fix"), &[rule])
                } else if let Some(name) = o
                    .strip_prefix("script ")
                    .or_else(|| o.split_once(':').map(|(_, name)| name))
                {
                    format(crate::key!("history-origin-from"), &[name])
                } else {
                    o.to_owned()
                }
            }
        }
    }

    /// The origin text of `row`: the shell's annotation for its revision, else its own.
    pub fn origin_of<'a>(&'a self, row: &'a HistoryRow) -> &'a str {
        self.origins
            .get(&row.rev)
            .map_or(row.origin.as_str(), String::as_str)
    }

    /// The panel's lines, newest first: the entries of the view and `markers`, a marker after the entry of its
    /// revision (above it in this order). Empty without a view.
    pub fn lines<'a>(&'a self, markers: &'a [HistoryMarker]) -> Vec<HistoryLine<'a>> {
        let Some(view) = self.view() else {
            return Vec::new();
        };
        // (revision, 0 entry or 1 marker, position) ascending, then reversed: newest first.
        let mut keyed: Vec<((u64, u8, usize), HistoryLine<'a>)> = Vec::new();
        for row in &view.rows {
            let state = if row.index >= view.cursor {
                EntryState::Redo
            } else if row.index + 1 == view.cursor {
                EntryState::Now
            } else {
                EntryState::Done
            };
            keyed.push((
                (row.rev, 0, row.index),
                HistoryLine::Entry {
                    row,
                    state,
                    origin: self.origin_of(row),
                },
            ));
        }
        for (i, m) in markers.iter().enumerate() {
            keyed.push(((m.rev, 1, i), HistoryLine::Marker(m)));
        }
        keyed.sort_by_key(|(k, _)| *k);
        keyed.into_iter().rev().map(|(_, l)| l).collect()
    }

    /// Keeps `context` under `rev`, dropping the oldest beyond [`MAX_CONTEXTS`]; an existing context stays (a nested
    /// action kept the nearer one).
    pub(crate) fn keep_context(&mut self, rev: u64, context: HistoryContext) {
        self.contexts.entry(rev).or_insert(context);
        while self.contexts.len() > MAX_CONTEXTS {
            self.contexts.pop_first();
        }
    }

    /// Forgets the project's history state (the project closed): the view, the cursor, the popup, the annotations and
    /// the contexts.
    pub(crate) fn clear(&mut self) {
        *self = HistoryPane::default();
    }
}

/// A step of `project.settings.set` in words, by the member it sets (`ew_api::window::SETTINGS_STEP`): a drawing setting
/// changed alone by its setting ("Print wire numbers"), the drawing settings, the frame defaults, the languages and
/// the supply by their own words ("Set frames"), a member of the settable table by the command's title ("Set project
/// settings"). The one place that words a member.
fn setting_words(member: &str) -> String {
    use crate::strings::text;
    if let Some(setting) = member.strip_prefix("drawing.") {
        return crate::strings::field_label("project", &format!("settings.drawing.{setting}"));
    }
    match member {
        "drawing" => text(crate::key!("step-settings-drawing")).to_owned(),
        "frames" => text(crate::key!("step-settings-frames")).to_owned(),
        "languages" => text(crate::key!("step-settings-languages")).to_owned(),
        "supply" => text(crate::key!("step-settings-supply")).to_owned(),
        _ => crate::strings::command_title("project.settings.set").to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: usize, rev: u64) -> HistoryRow {
        HistoryRow {
            index,
            label: format!("step {index}"),
            origin: "GUI".into(),
            step: true,
            barrier: false,
            rev,
            pages: Vec::new(),
        }
    }

    /// One row reads "1 step", two "2 steps"; no view, no header.
    #[test]
    fn the_header_counts_one_step_in_the_singular() {
        let pane = |rows: Vec<HistoryRow>, cursor: usize| HistoryPane {
            view: Some((
                1,
                9,
                HistoryView {
                    rows,
                    cursor,
                    ..HistoryView::default()
                },
            )),
            ..HistoryPane::default()
        };
        assert_eq!(
            pane(vec![row(0, 2)], 1).header().as_deref(),
            Some("1 step \u{b7} cursor at 1")
        );
        assert_eq!(
            pane(vec![row(0, 2), row(1, 4)], 1).header().as_deref(),
            Some("2 steps \u{b7} cursor at 1")
        );
        assert_eq!(HistoryPane::default().header(), None);
    }

    #[test]
    fn lines_are_newest_first_with_markers_after_their_revision() {
        let mut pane = HistoryPane {
            view: Some((
                1,
                9,
                HistoryView {
                    rows: vec![row(0, 2), row(1, 4), row(2, 6)],
                    cursor: 2,
                    ..HistoryView::default()
                },
            )),
            ..HistoryPane::default()
        };
        pane.origins.insert(4, "quick fix EW-CORE-023".into());
        let markers = [
            HistoryMarker {
                rev: 4,
                text: "Exported PDF to a.pdf".into(),
            },
            HistoryMarker {
                rev: 0,
                text: "Saved".into(),
            },
        ];
        let got: Vec<String> = pane
            .lines(&markers)
            .iter()
            .map(|l| match l {
                HistoryLine::Entry { row, state, origin } => {
                    format!("{} {} {origin}", row.index, state.word())
                }
                HistoryLine::Marker(m) => m.text.clone(),
            })
            .collect();
        assert_eq!(
            got,
            [
                "2 redo GUI",
                "Exported PDF to a.pdf",
                "1 now quick fix EW-CORE-023",
                "0 done GUI",
                "Saved"
            ]
        );
    }

    #[test]
    fn contexts_keep_the_first_and_drop_the_oldest() {
        let mut pane = HistoryPane::default();
        let ctx = |n: u128| HistoryContext {
            page: PageId(ew_dl::Destination::Page(uuid_of(n))),
            viewport: Viewport {
                centre: (0, 0),
                zoom: 1.0,
                fitted: true,
                first: true,
            },
            selection: Vec::new(),
        };
        pane.keep_context(1, ctx(1));
        pane.keep_context(1, ctx(2));
        assert_eq!(pane.contexts[&1], ctx(1), "the nearer context stays");
        for rev in 2..=(MAX_CONTEXTS as u64 + 1) {
            pane.keep_context(rev, ctx(3));
        }
        assert_eq!(pane.contexts.len(), MAX_CONTEXTS);
        assert!(!pane.contexts.contains_key(&1), "the oldest went");
    }

    fn uuid_of(n: u128) -> ew_api::output::Uuid {
        ew_api::output::Uuid::from_u128(n)
    }

    /// A step of `project.settings.set` reads what it set, in both languages: a drawing setting alone, the drawing
    /// settings, the frame defaults, the languages and a member of the supply, and an earlier log's alias as the
    /// command with that member.
    #[test]
    fn a_settings_step_is_named_by_what_it_sets() {
        let project = serde_json::from_str(
            r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1","settings":{"frames":{"per_page_type":[]}}}"#,
        )
        .expect("a project");
        let mut s = ew_api::Session::from_records(project, [], [], ew_api::SessionOptions::default())
            .expect("a session");
        let log = ew_api::CmdLog::parse(concat!(
            r#"{"$schema":"ew.cmdlog/1"}"#,
            "\n",
            r#"{"cmd":"project.settings.set","params":{"member":"drawing","value":{"wire_numbers":true}}}"#,
            "\n",
            r#"{"cmd":"project.settings.set","params":{"member":"drawing","value":{"crossings":"hops","marker_mode":"all"}}}"#,
            "\n",
            r#"{"cmd":"project.settings.set","params":{"member":"frames","value":{}}}"#,
            "\n",
            r#"{"cmd":"project.settings.set","params":{"member":"languages","value":{"source":"nl-NL"}}}"#,
            "\n",
            r#"{"cmd":"project.settings.set","params":{"member":"supply.u_nom","value":"400"}}"#,
            "\n",
            r#"{"cmd":"project.drawing.set","params":{"crossings":"plain","marker_mode":"none"}}"#,
            "\n",
            r#"{"cmd":"project.languages.set","params":{"source":"en"}}"#,
            "\n",
            r#"{"cmd":"project.supply.set","params":{"path":"phases","value":3}}"#,
            "\n",
        ))
        .expect("a log");
        s.run_script(&log, "steps.ewcmd").expect("the log runs");
        let view = s.history_view();
        let words = || -> Vec<String> { view.rows.iter().map(HistoryPane::label_words).collect() };
        assert_eq!(
            words(),
            [
                "Print wire numbers",
                "Set drawing data",
                "Set frames",
                "Set languages",
                "Set supply",
                "Set drawing data",
                "Set languages",
                "Set supply",
            ]
        );
        assert!(crate::strings::set_locale("nl"));
        let dutch = words();
        assert!(crate::strings::set_locale("en"));
        assert_eq!(
            dutch,
            [
                "Draadnummers afdrukken",
                "Tekeninggegevens instellen",
                "Kaders instellen",
                "Talen instellen",
                "Voeding instellen",
                "Tekeninggegevens instellen",
                "Talen instellen",
                "Voeding instellen",
            ]
        );
    }
}
