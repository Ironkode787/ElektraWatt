// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Report blocks navigator (doc 16 §5.2): the project's report blocks in section and page order.
//!
//! Not here: the other providers (their own files beside this one).

use std::collections::BTreeMap;

use ew_derive::{PageOrder, PageRef, Q};
use ew_ids::{Id, Uuid};

use super::plain;
use super::{Builder, NavOpen, NavRole, group};
use crate::{Text, TextArg};

pub(super) fn blocks_table(q: &Q, b: &mut Builder) {
    use ew_schema::{ReportBlock, ReportSource, Section};
    let order = q.get::<PageOrder>(());
    // (sort key: section number, first page position, ID), cells, ID, open page, section.
    type BlockRow = (
        (Option<u16>, usize, Uuid),
        Vec<Text>,
        Uuid,
        Option<Uuid>,
        Option<Id<Section>>,
    );
    let mut rows: Vec<BlockRow> = Vec::new();
    for id in q.report_blocks().ids() {
        let Some(block) = q.report_blocks().get(id) else {
            continue;
        };
        let block: &ReportBlock = block;
        let generated: Vec<(usize, &ew_derive::PageEntry)> = order
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e.page, PageRef::Generated { block, .. } if block == id))
            .collect();
        let host = block.embedded.as_ref().and_then(|e| {
            order
                .entries
                .iter()
                .enumerate()
                .find(|(_, e2)| e2.page == PageRef::Drawn(e.page))
        });
        let first = generated.first().copied().or(host);
        let section = first.map_or(block.section, |(_, e)| e.section);
        let number = section.and_then(|s| q.sections().get(s)).map(|s| s.number);
        let kind = match &block.source {
            ReportSource::Kind(k) => k.name().unwrap_or("unknown").to_owned(),
            ReportSource::SqlQuery(_) => "sql".to_owned(),
            _ => "unknown".to_owned(),
        };
        // The kind by the list's name of it in every language of the list texts, else its word; a block without a
        // title by its kind.
        let names = crate::report_kind_names(&kind);
        let kind = if names.is_empty() {
            plain(kind)
        } else {
            let in_each = names
                .into_iter()
                .filter_map(|(tag, name)| Some((tag.parse().ok()?, name)));
            Text::bare(TextArg::Ml(ew_mltext::MlText::from_langs(in_each)))
        };
        let title = crate::output::view::neutral(&block.title);
        let title = if title.is_empty() {
            kind.clone()
        } else {
            plain(title)
        };
        let cells = vec![
            title,
            kind,
            plain(first.map(|(_, e)| e.folio.clone()).unwrap_or_default()),
            plain(number.map(|n| n.to_string()).unwrap_or_default()),
            plain(generated.len().to_string()),
        ];
        let open = generated.first().map(|(_, e)| crate::output::page_uuid(e.page));
        rows.push((
            (number, first.map_or(usize::MAX, |(i, _)| i), id.as_uuid()),
            cells,
            id.as_uuid(),
            open,
            section,
        ));
    }
    rows.sort_by_key(|r| r.0);
    let mut sections: BTreeMap<Option<u16>, u32> = BTreeMap::new();
    for ((number, _, _), cells, id, open, section) in rows {
        let snode = match sections.get(&number) {
            Some(n) => *n,
            None => {
                let (segment, label) = match (number, section.and_then(|s| q.sections().get(s))) {
                    (Some(n), Some(s)) => (
                        format!("section:{n}"),
                        text!(
                            "nav-section",
                            n.to_string(),
                            crate::output::view::neutral(&s.title)
                        ),
                    ),
                    _ => ("section:front".to_owned(), text!("nav-front-matter")),
                };
                let n = group(b, None, NavRole::Section, &segment, label);
                sections.insert(number, n);
                n
            }
        };
        let facets = vec![("kind".to_owned(), "report_block".to_owned())];
        let n = b.push(
            Some(snode),
            NavRole::ReportBlock,
            &format!("report_block:{id}"),
            cells[0].clone(),
            Vec::new(),
            vec![id],
            Vec::new(),
            Some(cells),
            facets,
        );
        // A paged block opens its first generated page with the block selected (R30 F9).
        b.nodes[n as usize].open = open.map(|p| NavOpen {
            page: p,
            select: vec![id],
        });
    }
}
