// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window's List columns and Rows of your own forms read of a list page or a legend (doc 09 §3.8): the report
//! block, every column of its report kind with the block's own choice (printed or not, its heading, its width), its
//! caption and legend, and the rows it holds that the model does not. The forms write them back with `fields.set_many`
//! on the block; this module writes nothing. Also the form a new list starts on and the words of a block's columns.

use ew_derive::{PageOrder, PageRef, text_in};
use ew_ids::{Id, Uuid};
use ew_mltext::{LangTag, MlText};
use ew_reports::Registry;
use ew_schema::{ReportBlock, ReportKind, ReportSource};

use super::view::page_uuid;
use crate::Session;

/// One column a list can print.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ListColumn {
    /// The column's key in its report kind (`terminal`).
    pub key: String,
    /// The kind's heading of the column, in the language asked.
    pub label: String,
    /// Whether the block prints it.
    pub shown: bool,
    /// The block's own heading of the column, in the language asked; empty: the kind's.
    pub heading: String,
    /// Its width: the block's, else the kind's weight.
    pub width: u32,
}

/// A list page's block and what it prints.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ListLook {
    /// The report block.
    pub block: Uuid,
    /// The columns: those the block prints, in its order, then the kind's others; every column of the kind in its
    /// order when the block chooses none.
    pub columns: Vec<ListColumn>,
    /// The caption, in the language asked; empty for none.
    pub caption: String,
    /// The legend, in the language asked; empty for none.
    pub legend: String,
    /// The rows the block holds that the model does not, each its texts by column key, in the language asked.
    pub added: Vec<Vec<(String, String)>>,
    /// Whether the list is a table of contents (which lists the pages and takes no rows of its own).
    pub contents: bool,
}

impl Session {
    /// The form a new list of the project starts on (doc 09 §3.8): the set's own frame
    /// (`ew_reports::layout::set_frame`: the frame defaults' `default`, else the frame most drawn pages print on) when it
    /// is not one of the starter forms; `None`, the starter frame without the grid, for a project on the starter forms
    /// or without drawn pages. A copy made from a starter (the project's own once its title block was first changed,
    /// doc 09 §3.9) stands for that starter by its key: the list starts on the project's copy of the frame without the
    /// grid that prints the set's title block, `None` while the project has no copy of its own.
    pub fn new_list_form(&self) -> Option<Uuid> {
        let q = self.query();
        let frame = ew_reports::layout::set_frame(&q)?;
        let set = q.forms().get(frame)?;
        let starter = |key: Option<&ew_mltext::Ident>| {
            key.is_some_and(|k| ew_library::starter::FORMS.iter().any(|(s, _)| *s == k.as_str()))
        };
        if !starter(set.item.meta.key.as_ref()) {
            return Some(frame.as_uuid());
        }
        let project = q.project().id.as_uuid();
        let plain: Vec<_> = q
            .forms()
            .iter()
            .filter(|(_, c)| {
                c.library_id.as_uuid() == project
                    && c.item.meta.key.as_ref().is_some_and(|k| k.as_str() == ew_schema::starter::NOGRID_FRAME)
            })
            .map(|(id, c)| (id, c.item.title_block))
            .collect();
        plain
            .iter()
            .find(|(_, block)| *block == set.item.title_block)
            .or_else(|| plain.first())
            .map(|(id, _)| id.as_uuid())
    }

    /// The forms a page or a report block prints on, each by its name in words in the reader's language (a frame of
    /// the project's own by its paper and the pages it is on): the Form row's choices (doc 09 §3.8), which the window
    /// shows in its language.
    pub fn frame_forms(&self) -> Vec<(Uuid, crate::Text)> {
        super::navigators::libraries::frame_forms(&self.query())
    }

    /// The headings of the columns of the report block `block`'s kind in `lang` (`nl-NL`), by key; empty for a block
    /// without a report kind of this build. The window words a block's printed columns and its own rows by them.
    pub fn report_column_words(&self, block: Uuid, lang: &str) -> Vec<(String, String)> {
        let q = self.query();
        let Some(ReportSource::Kind(kind)) = q.report_blocks().get(Id::from_uuid(block)).map(|b| &b.source)
        else {
            return Vec::new();
        };
        let lang: Option<LangTag> = lang.parse().ok();
        Registry::builtin().get(kind).map_or_else(Vec::new, |def| {
            def.columns()
                .into_iter()
                .map(|c| (c.key.clone(), text_in(&c.label, lang.as_ref()).to_owned()))
                .collect()
        })
    }

    /// The look of the list whose page has the ID `page` (a generated page's, [`page_uuid`]), its texts in `lang`
    /// (`nl-NL`); `None` for another page and a list without a report kind of this build (a user SQL report).
    pub fn list_look(&self, page: Uuid, lang: &str) -> Option<ListLook> {
        let order = self.query().get::<PageOrder>(());
        let block = order.entries.iter().find_map(|e| match e.page {
            PageRef::Generated { block, .. } if page_uuid(e.page) == page => Some(block),
            _ => None,
        })?;
        self.block_look(block.as_uuid(), lang)
    }

    /// The look of the report block `block` (a list's, or a legend's embedded on its page), its texts in `lang`
    /// (`nl-NL`); `None` for no such block and one without a report kind of this build. A block that chooses no columns
    /// prints its kind's: the columns a list of its kind prints (`ReportKindDef::printed`, else every column), the columns
    /// of the kind's box of an embedded block (a legend made before 0.3 prints position, tag, quantity and part number,
    /// doc 09 §2.2), the others unticked.
    pub fn block_look(&self, block: Uuid, lang: &str) -> Option<ListLook> {
        let q = self.query();
        let block: Id<ReportBlock> = Id::from_uuid(block);
        let b: &ReportBlock = q.report_blocks().get(block)?;
        let ReportSource::Kind(kind) = &b.source else {
            return None;
        };
        let def = Registry::builtin().get(kind)?;
        let defs = def.columns();
        let lang: Option<LangTag> = lang.parse().ok();
        let words = |m: &MlText| text_in(m, lang.as_ref()).to_owned();
        // The columns the block prints, its own else its kind's, as `(key, width, heading)`.
        let printed: Vec<(String, u32, MlText)> = if !b.columns.is_empty() {
            b.columns
                .iter()
                .map(|c| (c.key.clone(), c.weight, c.heading.clone()))
                .collect()
        } else if b.embedded.is_some() {
            def.embedded_layout()
                .columns
                .into_iter()
                .map(|(key, weight)| (key, weight, MlText::default()))
                .collect()
        } else if let Some(keys) = def.printed() {
            keys.iter()
                .filter_map(|key| defs.iter().find(|d| d.key == *key))
                .map(|d| (d.key.clone(), d.weight, MlText::default()))
                .collect()
        } else {
            defs.iter()
                .map(|d| (d.key.clone(), d.weight, MlText::default()))
                .collect()
        };
        let mut columns: Vec<ListColumn> = printed
            .iter()
            .filter_map(|(key, width, heading)| {
                let def = defs.iter().find(|d| d.key == *key)?;
                Some(ListColumn {
                    key: key.clone(),
                    label: words(&def.label),
                    shown: true,
                    heading: words(heading),
                    width: *width,
                })
            })
            .collect();
        for def in &defs {
            if !columns.iter().any(|c| c.key == def.key) {
                columns.push(ListColumn {
                    key: def.key.clone(),
                    label: words(&def.label),
                    shown: false,
                    heading: String::new(),
                    width: def.weight,
                });
            }
        }
        let added = b
            .added_rows
            .iter()
            .map(|r| r.cells.iter().map(|(k, v)| (k.clone(), words(v))).collect())
            .collect();
        Some(ListLook {
            block: block.as_uuid(),
            columns,
            caption: words(&b.caption),
            legend: words(&b.legend),
            added,
            contents: matches!(kind, ReportKind::Toc),
        })
    }
}
