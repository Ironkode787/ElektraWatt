// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window's List columns and Rows of your own forms read of a list page (doc 09 §3.8): the page's report
//! block, every column of its report kind with the block's own choice (printed or not, its heading, its width), its
//! caption and legend, and the rows it holds that the model does not. The forms write them back with `fields.set_many`
//! on the block; this module writes nothing.

use ew_derive::ew_ids::Uuid;
use ew_derive::ew_mltext::{LangTag, MlText};
use ew_derive::{PageOrder, PageRef, text_in};
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
    /// The look of the list whose page has the ID `page` (a generated page's, [`page_uuid`]), its texts in `lang`
    /// (`nl-NL`); `None` for another page, an embedded block and a list without a report kind of this build (a user
    /// SQL report).
    pub fn list_look(&self, page: Uuid, lang: &str) -> Option<ListLook> {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let block = order.entries.iter().find_map(|e| match e.page {
            PageRef::Generated { block, .. } if page_uuid(e.page) == page => Some(block),
            _ => None,
        })?;
        let b: &ReportBlock = q.report_blocks().get(block)?;
        let ReportSource::Kind(kind) = &b.source else {
            return None;
        };
        let defs = Registry::builtin().get(kind)?.columns();
        let lang: Option<LangTag> = lang.parse().ok();
        let words = |m: &MlText| text_in(m, lang.as_ref()).to_owned();
        let mut columns: Vec<ListColumn> = b
            .columns
            .iter()
            .filter_map(|c| {
                let def = defs.iter().find(|d| d.key == c.key)?;
                Some(ListColumn {
                    key: c.key.clone(),
                    label: words(&def.label),
                    shown: true,
                    heading: words(&c.heading),
                    width: c.weight,
                })
            })
            .collect();
        let chosen = !columns.is_empty();
        for def in &defs {
            if !columns.iter().any(|c| c.key == def.key) {
                columns.push(ListColumn {
                    key: def.key.clone(),
                    label: words(&def.label),
                    shown: !chosen,
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
