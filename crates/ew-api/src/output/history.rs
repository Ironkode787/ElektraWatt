// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The history panel's rows (WP-S2.09 part 3, §4.2, §8.4): every entry of the undo stack with its label, its origin as
//! the panel prints it, whether it is an undo step or a marker line, its merged count, its revision and the folios of
//! the pages its changes touch in the current model.
//!
//! **Pages.** A page record names itself; a representation, connection, page item or structure box its page; a device
//! or a function the pages of its representations; any other record none. The folios are in page order, at most
//! 8. **Origin.** `GUI`, `script <base name>` for a replayed command log, `external change`,
//! `import`, else `Origin::label`. The shell's own annotations (`quick fix <rule>`) are view state and stay in the
//! shell.

use std::collections::BTreeSet;

use ew_commands::{History, ModelDiff, Origin};
use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{
    Connection, Device, Function, Kind, Page, PageItem, Representation, StructureBox,
};
use ew_derive::{PageOrder, PageRef, Q};

use crate::Session;

/// The most folios a history row lists.
const MAX_HISTORY_PAGES: usize = 8;

/// One entry of the history (§4.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HistoryRow {
    /// The entry's index in the stack (0 the oldest kept).
    pub index: usize,
    /// The label (the command's).
    pub label: String,
    /// The origin as the panel prints it.
    pub origin: String,
    /// Whether it is an undo step.
    pub step: bool,
    /// Whether it is a marker line that changed the model (undo cannot pass it).
    pub barrier: bool,
    /// How many transactions the step holds: 1, or more when later gestures were merged into it.
    pub merged: u32,
    /// The revision that committed it.
    pub rev: u64,
    /// The folios of the pages its changes touch, in page order, at most 8.
    pub pages: Vec<String>,
}

/// The history as the panel shows it (§4.2).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct HistoryView {
    /// Every entry, oldest first.
    pub rows: Vec<HistoryRow>,
    /// The number of applied entries: the entries at and above it are the redo branch.
    pub cursor: usize,
    /// The estimated size of what the history keeps.
    pub bytes: usize,
    /// Its bound in bytes.
    pub max_bytes: usize,
    /// Its bound in steps.
    pub max_steps: usize,
}

/// The origin text of the panel (§8.4): `GUI`, `script <base name>`, `external change`, `import`, else the label.
fn origin_text(origin: &Origin) -> String {
    match origin {
        Origin::Gui => "GUI".to_owned(),
        Origin::CmdLog { file } => {
            let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
            format!("script {base}")
        }
        Origin::External => "external change".to_owned(),
        Origin::Import { .. } => "import".to_owned(),
        other => other.label(),
    }
}

/// The pages of `diff`'s records in the model of `q` (see the module doc).
fn pages_of(q: &Q, diff: &ModelDiff) -> BTreeSet<Id<Page>> {
    let mut pages = BTreeSet::new();
    let reps_of = |f: Id<Function>, out: &mut BTreeSet<Id<Page>>| {
        for r in q.idx().reps_of_function(f) {
            if let Some(rep) = q.reps().get(r) {
                out.insert(rep.page);
            }
        }
    };
    for change in &diff.changes {
        let (kind, id) = change.key();
        match kind {
            Kind::Page => {
                let page: Id<Page> = id.cast();
                if q.pages().get(page).is_some() {
                    pages.insert(page);
                }
            }
            Kind::Representation => {
                if let Some(rep) = q.reps().get(id.cast::<Representation>()) {
                    pages.insert(rep.page);
                }
            }
            Kind::Connection => {
                if let Some(page) = q.connections().get(id.cast::<Connection>()).and_then(|c| c.page) {
                    pages.insert(page);
                }
            }
            Kind::PageItem => {
                if let Some(item) = q.page_items().get(id.cast::<PageItem>()) {
                    pages.insert(item.page);
                }
            }
            Kind::StructureBox => {
                if let Some(b) = q.structure_boxes().get(id.cast::<StructureBox>()) {
                    pages.insert(b.page);
                }
            }
            Kind::Function => reps_of(id.cast::<Function>(), &mut pages),
            Kind::Device => {
                let functions: Vec<Id<Function>> = q.idx().functions_of_device(id.cast::<Device>()).collect();
                for f in functions {
                    reps_of(f, &mut pages);
                }
            }
            _ => {}
        }
    }
    pages
}

/// The folios of `pages` in page order, at most `MAX_HISTORY_PAGES`.
fn folios(q: &Q, pages: &BTreeSet<Id<Page>>) -> Vec<String> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .filter(|e| matches!(e.page, PageRef::Drawn(p) if pages.contains(&p)))
        .take(MAX_HISTORY_PAGES)
        .map(|e| e.folio.clone())
        .collect()
}

/// The rows of `history` over the current model `q`.
pub(super) fn view_of(history: &History, q: &Q) -> HistoryView {
    let limits = history.limits();
    let rows = history
        .entries()
        .iter()
        .enumerate()
        .map(|(index, e)| HistoryRow {
            index,
            label: e.label().to_owned(),
            origin: origin_text(e.origin()),
            step: e.is_step(),
            barrier: e.is_barrier(),
            merged: e.merged().saturating_add(1),
            rev: e.rev().0,
            pages: e
                .changes()
                .map(|diff| folios(q, &pages_of(q, &diff)))
                .unwrap_or_default(),
        })
        .collect();
    HistoryView {
        rows,
        cursor: history.cursor(),
        bytes: history.bytes(),
        max_bytes: limits.max_bytes,
        max_steps: limits.max_steps,
    }
}

impl Session {
    /// The history panel's view of the undo stack (WP-S2.09 §4.2): every entry, oldest first, with its label, its
    /// origin text, its step and barrier flags, the number of transactions merged into it, its revision and the folios of the pages its changes
    /// touch in the current model; the cursor and the bounds. One call per revision while the panel is open (§9
    /// item 5).
    pub fn history_view(&self) -> HistoryView {
        view_of(self.history(), &self.query())
    }

    /// The records the history entry `index` changes, labelled as [`Session::preview_entities`] labels them, from the
    /// current model (Show changes, §8.4); empty for an index out of range.
    pub fn history_entities(&self, index: usize) -> Vec<super::PreviewEntity> {
        let Some(entry) = self.history().entry(index) else {
            return Vec::new();
        };
        let Ok(diff) = entry.changes() else {
            return Vec::new();
        };
        super::preview::entities_of(&diff, &self.query(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_texts() {
        assert_eq!(origin_text(&Origin::Gui), "GUI");
        assert_eq!(
            origin_text(&Origin::CmdLog {
                file: "scripts/b2.ewui".into()
            }),
            "script b2.ewui"
        );
        assert_eq!(origin_text(&Origin::External), "external change");
        assert_eq!(origin_text(&Origin::Import { lineage: "x".into() }), "import");
        assert_eq!(origin_text(&Origin::Merge), "merge");
    }
}
