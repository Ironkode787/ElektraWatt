// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 6 (D17, FR-UI-048): the three one-time tips for a user who comes from EPLAN or QET, each a toast with
//! one action of the registry.
//!
//! 1. A Line segment of the Graphic tool ending on a connection point: "Lines do not connect. Draw a wire instead."
//!    with **Convert to connection** (`edit.undo` of the line, one undo step, then `tool.wire`).
//! 2. An edit refused on a derived tag (the property grid's Tag row, the Devices table's tag column): "Tags are derived
//!    from the structure and the counter." with **Renumber devices…** (`devices.renumber`).
//! 3. An edit refused on a generated page (a row of a report block's generated page in the property grid, a generated
//!    page's row in the Pages table): "This page is generated from a report block." with **Show report block**
//!    (`select.set` with the block, then `props.focus`).
//!
//! A tip is shown only while `tips` is on and first run's answer `comes_from` is `eplan` or `qet`, and at most once:
//! its word enters `tips_seen` in `settings.toml` when it is shown.

use ew_api::output::{NavKind, NavTable, PageTables, Uuid};
use serde_json::{Value, json};

use super::{Outcome, ShellModel};
use crate::key;
use crate::notifications::{NoteAction, NoteTone, Notification, Validity};
use crate::properties::{PropRow, Provenance, SheetSection};
use crate::strings::{self, Key};

/// One of the three tips (D17).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tip {
    /// A Line segment ending on a connection point.
    LineOnCp,
    /// An edit refused on a derived tag.
    DerivedTag,
    /// An edit refused on a generated page.
    GeneratedPage,
}

impl Tip {
    /// The word `tips_seen` remembers it by.
    pub fn word(self) -> &'static str {
        match self {
            Tip::LineOnCp => "line_on_cp",
            Tip::DerivedTag => "derived_tag",
            Tip::GeneratedPage => "generated_page",
        }
    }

    /// The key of its text.
    fn text_key(self) -> Key<'static> {
        match self {
            Tip::LineOnCp => key!("tip-line-on-cp"),
            Tip::DerivedTag => key!("tip-derived-tag"),
            Tip::GeneratedPage => key!("tip-generated-page"),
        }
    }
}

/// Whether `cmd` (an `item.place` of the Graphic tool) places a line or a polyline with a point on a connection point
/// of `tables` (the page before the commit): tip 1's habit.
pub(super) fn line_ends_on_cp(cmd: &Value, tables: Option<&PageTables>) -> bool {
    let Some(tables) = tables else { return false };
    let shape = &cmd["kind"]["graphic"]["shape"];
    let points: Vec<&Value> = if let Some(line) = shape.get("line") {
        vec![&line["a"], &line["b"]]
    } else if let Some(points) = shape["polyline"]["points"].as_array() {
        points.iter().collect()
    } else {
        return false;
    };
    points.iter().any(|p| {
        let at = (p[0].as_i64(), p[1].as_i64());
        tables.cps.iter().any(|c| (Some(c.at.0), Some(c.at.1)) == at)
    })
}

/// The origins (first run's step 2) the tips are for.
const TIP_ORIGINS: [&str; 2] = ["eplan", "qet"];

impl ShellModel {
    /// Shows `tip` once, when the settings allow it: `select` is what Show report block selects (tip 3; a tip 3
    /// without a block is not shown).
    pub(super) fn offer_tip(&mut self, tip: Tip, select: Vec<Uuid>) {
        if !self.settings.tips
            || !TIP_ORIGINS.contains(&self.settings.comes_from.as_str())
            || self.settings.tips_seen.iter().any(|w| w == tip.word())
        {
            return;
        }
        let Some(project) = &self.project else { return };
        let action = match tip {
            Tip::LineOnCp => {
                let h = project.session.history();
                let cursor = h.cursor();
                let Some(rev) = cursor
                    .checked_sub(1)
                    .and_then(|i| h.entries().get(i))
                    .map(|e| e.rev().0)
                else {
                    return;
                };
                (
                    NoteAction::Convert,
                    Validity::Undo {
                        epoch: project.epoch,
                        cursor,
                        rev,
                    },
                )
            }
            Tip::DerivedTag => (
                NoteAction::Renumber,
                Validity::Session {
                    epoch: project.epoch,
                    select: Vec::new(),
                },
            ),
            Tip::GeneratedPage if select.is_empty() => return,
            Tip::GeneratedPage => (
                NoteAction::ShowBlock,
                Validity::Session {
                    epoch: project.epoch,
                    select,
                },
            ),
        };
        self.settings.tips_seen.push(tip.word().to_owned());
        self.save_settings();
        // A tip informs: the info icon, not the check of a finished outcome.
        self.notes.push_toned(
            strings::text(tip.text_key()).to_owned(),
            "tip",
            Some(action),
            Some(NoteTone::Info),
        );
    }

    /// Tip 2 or 3 for an edit the property grid refused with `out` on `row` of `section`, only on the derived or
    /// generated reason (C49-p8#3): the Tag row computed from the drawing (tip 2), a row generated by a report block
    /// (tip 3, its block selected by the action). A read-only project's refusal offers neither.
    pub(super) fn tip_for_refused_row(&mut self, section: &SheetSection, row: &PropRow, out: &Outcome) {
        let computed = *out == Outcome::refused(key!("reason-row-computed"));
        if row.key == "device/derived.tag" && computed {
            self.offer_tip(Tip::DerivedTag, Vec::new());
        } else if matches!(row.provenance, Provenance::Generated { .. })
            && section.kind == "report_block"
            && matches!(out, Outcome::Failed { .. })
        {
            self.offer_tip(Tip::GeneratedPage, section.targets.clone());
        }
    }

    /// Tip 2 or 3 for a cell edit table mode refused on node `i`, column `col` of navigator `kind`, in a project that can
    /// be changed: the Devices table's tag (its key column, tip 2), a generated page's row of the Pages table (tip 3).
    pub(super) fn tip_for_refused_cell(&mut self, kind: NavKind, table: &NavTable, i: u32, col: usize) {
        if !self.is_writable() {
            return;
        }
        let node = &table.nodes[i as usize];
        if kind == NavKind::Devices && table.columns.get(col).is_some_and(|c| c.key_column) {
            self.offer_tip(Tip::DerivedTag, Vec::new());
        } else if kind == NavKind::Pages
            && node.badges.iter().any(|b| b == "generated")
            && let Some(page) = node.open.as_ref().map(|o| o.page)
        {
            // A generated page's row carries no record: the page it opens is the page.
            let block = self.block_of(page);
            self.offer_tip(Tip::GeneratedPage, block);
        }
    }

    /// The report block of the generated page `page`, as the page's property sheet names it (empty when none does).
    fn block_of(&self, page: Uuid) -> Vec<Uuid> {
        self.project
            .as_ref()
            .and_then(|p| {
                p.session
                    .property_sheet(&[page])
                    .sections
                    .into_iter()
                    .find(|s| s.kind == "report_block")
            })
            .map(|s| s.targets)
            .unwrap_or_default()
    }

    /// Tip 3 for an action a generated page refused, in a project that can be changed (C49-p8#3): an editing tool or
    /// Paste refused on the generated page on show (`reason-page-type-no-symbols-report`), and the Pages navigator's
    /// move or reorder refused on a generated page (`error-generated-page-order`; the page the folios of `params`
    /// name, else the marked rows, else the cursor row).
    pub(super) fn tip_for_outcome(&mut self, action: &str, params: &Value, out: &Outcome) {
        if !self.is_writable() {
            return;
        }
        let rows = self.page_rows();
        let generated = |i: usize| rows.get(i).filter(|r| r.generated).map(|r| r.id);
        let page = match out {
            Outcome::Refused { reason_key } if *reason_key == key!("reason-page-type-no-symbols-report") => {
                generated(self.page_index())
            }
            Outcome::Failed { message }
                if matches!(action, "pages.reorder" | "pages.move_up" | "pages.move_down")
                    && message == strings::text(key!("error-generated-page-order")) =>
            {
                let named: Vec<String> = params["pages"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .chain(std::iter::once(&params["before"]))
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                let by_folio = named
                    .iter()
                    .filter_map(|f| rows.iter().position(|r| r.folio == *f))
                    .find_map(generated);
                let marked = || {
                    self.navigator
                        .marked
                        .iter()
                        .cloned()
                        .chain(self.navigator_row_key())
                        .filter_map(|k| crate::navigator::PageNavigator::page_of(&k, rows))
                        .find_map(generated)
                };
                by_folio.or_else(marked)
            }
            _ => None,
        };
        if let Some(page) = page {
            let block = self.block_of(page);
            self.offer_tip(Tip::GeneratedPage, block);
        }
    }

    /// Runs a tip's action (`run_note_action` has judged it valid): the toast goes, then the action's steps.
    pub(super) fn run_tip_action(&mut self, entry: &Notification, action: NoteAction) -> Outcome {
        self.notes.dismiss(entry.serial);
        match Some(action) {
            Some(NoteAction::Convert) => match self.dispatch("edit.undo", &Value::Null) {
                Outcome::Done => self.dispatch("tool.wire", &Value::Null),
                other => other,
            },
            Some(NoteAction::Renumber) => self.dispatch("devices.renumber", &Value::Null),
            Some(NoteAction::ShowBlock) => {
                let ids: Vec<String> = entry.selects(action).iter().map(ToString::to_string).collect();
                match self.dispatch("select.set", &json!({ "ids": ids })) {
                    Outcome::Done => self.dispatch("props.focus", &Value::Null),
                    other => other,
                }
            }
            _ => Outcome::refused(key!("reason-no-toast")),
        }
    }

    /// Whether the one-time tips may be shown: the setting `tips` (FR-UI-048), View ▸ Show tips' check.
    pub fn tips_on(&self) -> bool {
        self.settings.tips
    }

    /// `view.tips {on?}` (View ▸ Show tips, FT-1280): switches the one-time tips on or off (without `on`, the other
    /// way), saved to `tips` in `settings.toml`.
    pub(crate) fn act_tips(&mut self, p: &serde_json::Map<String, Value>) -> Outcome {
        let on = match p.get("on") {
            None => !self.settings.tips,
            Some(v) => match v.as_bool() {
                Some(b) => b,
                None => return super::failed(key!("error-bad-param"), &["on"]),
            },
        };
        if self.settings.tips != on {
            self.settings.tips = on;
            self.save_settings();
        }
        Outcome::Done
    }
}
