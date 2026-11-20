// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Pages navigator's tree (section view): the sections in page order and their pages.
//!
//! Not here: the table types and the dispatch by kind (`navigators.rs`); the other providers (their own files beside
//! this one).

use std::collections::BTreeMap;

use ew_derive::{PageOrder, PageRef};

use super::{Builder, NavOpen, NavRole};
use crate::Session;
use crate::output::PageRow;
use crate::output::view::PageFrameOf;

impl Session {
    /// The Pages tree as built (section view): section nodes (front matter first, as the page order
    /// meets them), the pages under them.
    pub(super) fn pages_table(&self, b: &mut Builder) {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let rows: Vec<PageRow> = self.page_rows();
        let mut sections: BTreeMap<String, u32> = BTreeMap::new();
        for (row, entry) in rows.iter().zip(order.entries.iter()) {
            let (skey, slabel) = match &row.section {
                None => ("s:front".to_owned(), "Front matter".to_owned()),
                Some((n, title)) => (format!("s:{n}"), format!("Section {n} - {title}")),
            };
            let parent = match sections.get(&skey) {
                Some(p) => *p,
                None => {
                    let p = b.push(
                        None,
                        NavRole::Section,
                        &skey,
                        slabel,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        None,
                        vec![("kind".to_owned(), "section".to_owned())],
                    );
                    sections.insert(skey, p);
                    p
                }
            };
            let mut badges = Vec::new();
            if row.generated {
                badges.push("generated".to_owned());
            }
            if row.qet_grid {
                badges.push("QET grid".to_owned());
            }
            let frame = q
                .get::<PageFrameOf>(entry.page)
                .as_ref()
                .as_ref()
                .and_then(|f| f.form.meta.key.as_ref().map(|k| k.as_str().to_owned()))
                .unwrap_or_default();
            let cells = vec![
                row.folio.clone(),
                row.title.clone(),
                row.kind.clone(),
                frame,
                if row.generated { "yes" } else { "no" }.to_owned(),
                row.changed_in.clone().unwrap_or_default(),
            ];
            let records = match entry.page {
                PageRef::Drawn(p) => vec![p.as_uuid()],
                _ => Vec::new(),
            };
            let at = b.push(
                Some(parent),
                NavRole::Page,
                &format!("p:{}", row.id),
                row.folio.clone(),
                badges,
                records,
                Vec::new(),
                Some(cells),
                vec![
                    ("page".to_owned(), row.folio.to_lowercase()),
                    ("kind".to_owned(), "page".to_owned()),
                ],
            );
            b.nodes[at as usize].open = Some(NavOpen {
                page: row.id,
                select: Vec::new(),
            });
        }
    }
}
