// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The history panel and context restore (WP-S2.09 part 3, §8.4, §8.5, D13; FR-UI-023): the undo steps of the open
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
//! The transaction's own `TxContext` stays unused (D13). Nothing here reads a clock or a path.

use std::collections::BTreeMap;

use ew_api::output::{HistoryRow, HistoryView, PreviewEntity};

use ew_api::output::ew_dl::EntityRef;

use crate::canvas::{PageId, Viewport};
use crate::shell::HistoryMarker;

/// The most contexts the shell keeps (§8.5).
pub const MAX_CONTEXTS: usize = 500;

/// The most records Show changes lists (§8.4).
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
            merged: 1,
            rev,
            pages: Vec::new(),
        }
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
            page: PageId(ew_api::output::ew_dl::Destination::Page(uuid_of(n))),
            viewport: Viewport {
                centre: (0, 0),
                zoom: 1.0,
                fitted: true,
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
}
