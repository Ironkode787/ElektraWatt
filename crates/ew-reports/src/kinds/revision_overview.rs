// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `revision_overview`: the revisions and their changed pages (FR-REP-018, golden 911; doc 09 §2.4, doc 15 §5.6;
//! WP-S1.18 part 2, D7 and FT-207).

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Page, PartyRole, ReportBlock, Revision, RevisionState, Section};
use ew_derive::{DrawnPageOrder, PageRef, Q, recorded_changed_pages, revision_label_key};

use super::conn::text_cell;
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowSet};

/// The revision overview: [`revision_rows`].
#[derive(Clone, Copy, Debug)]
pub struct RevisionOverview;

impl ReportKindDef for RevisionOverview {
    fn name(&self) -> &'static str {
        "revision_overview"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("label", CellType::Ident, "Revision"),
            ColumnDef::new("date", CellType::Date, "Date"),
            ColumnDef::new("author", CellType::Text, "Author"),
            ColumnDef::new("checker", CellType::Text, "Checker"),
            ColumnDef::new("state", CellType::Text, "State"),
            ColumnDef::new("pages", CellType::Folios, "Changed pages"),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        revision_rows(q)
    }
}

/// `revision_rows`: one row per `revision` record (FT-207).
///
/// **Order.** The revisions in label order (the project's label order, [`ew_derive::revision_label_key`]: a label in
/// the project's format by its position, `Y < Z < AA`, after the labels outside it in natural order; then the ID);
/// a withdrawal record
/// directly after the revision it withdraws (doc 15 §5.10), several in ID order; a withdrawal record whose revision
/// does not exist in its own label's place, after the revisions of that label.
///
/// **Cells.** `label`; `date` (empty without one); `author` and `checker`, the name of the first party of that role
/// (empty without the role or the party); `state`, the stored name (`open`, `released`, `withdrawn`; a state of a
/// newer format as its text); `pages`, the revision's changed pages in page order: the pages it records
/// ([`ew_derive::recorded_changed_pages`]: `release.changed_pages`, else its page notes) that are drawn pages or section
/// dividers of the project. The row's ID names the revision.
///
/// **Inside the pagination** (this kind's rows are read by `page_order`): the rows read neither the page order nor
/// page digests, and the page cell orders by the drawn page order. Two parts of doc 15 §5.6 are therefore not here
/// but completed by the session at draw time, which sees the page digests: the open revision's pages when the last
/// release carries digests (`changed_now`), and generated pages named by a release (`ew_api` replaces the `pages`
/// cell of each row with its `changed_pages`). One row per revision either way, so the pagination is the same.
pub fn revision_rows(q: &Q) -> RowSet {
    let columns = RevisionOverview.columns();
    let mut set = RowSet::new(columns);
    let table = q.table::<Revision>();
    let format = q.project().settings.revision_label_format();
    let key = |r: &Revision| (revision_label_key(&format, r.label.as_str()), r.id);
    let mut main: Vec<&Revision> = table
        .iter()
        .map(|(_, r)| r.as_ref())
        .filter(|r| !is_withdrawal_of_existing(q, r))
        .collect();
    main.sort_by_key(|r| key(r));
    let mut withdrawals: Vec<&Revision> = table
        .iter()
        .map(|(_, r)| r.as_ref())
        .filter(|r| is_withdrawal_of_existing(q, r))
        .collect();
    withdrawals.sort_by_key(|r| r.id);
    let order = q.get::<DrawnPageOrder>(());
    for rev in main {
        set.rows.push(row(q, &order, rev));
        for w in withdrawals.iter().filter(|w| w.withdraws == Some(rev.id)) {
            set.rows.push(row(q, &order, w));
        }
    }
    set
}

/// A withdrawal record whose withdrawn revision exists: listed after that revision.
fn is_withdrawal_of_existing(q: &Q, r: &Revision) -> bool {
    r.state == RevisionState::Withdrawn && r.withdraws.is_some_and(|w| q.table::<Revision>().contains(w))
}

fn row(q: &Q, order: &ew_derive::PageOrderV, rev: &Revision) -> Row {
    let party = |role: &Option<PartyRole>| -> Cell {
        role.as_ref()
            .and_then(|role| q.project().parties.iter().find(|p| p.role == *role))
            .filter(|p| !p.name.is_empty())
            .map_or(Cell::Empty, |p| Cell::Text(p.name.clone()))
    };
    let state = match &rev.state {
        RevisionState::Open => "open".to_owned(),
        RevisionState::Released => "released".to_owned(),
        RevisionState::Withdrawn => "withdrawn".to_owned(),
        RevisionState::Unknown(v) => v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()),
    };
    let mut pages: Vec<(usize, PageRef)> = recorded_changed_pages(rev)
        .into_iter()
        .filter_map(|p| page_ref(q, p))
        .filter_map(|p| order.position(p).map(|i| (i, p)))
        .collect();
    pages.sort();
    pages.dedup();
    Row::data(
        RowId::entity(rev.id),
        vec![
            Cell::Ident(rev.label.as_str().to_owned()),
            rev.date.map_or(Cell::Empty, Cell::Date),
            party(&rev.author),
            party(&rev.checker),
            text_cell(state),
            Cell::Folios(pages.into_iter().map(|(_, p)| p).collect()),
        ],
    )
}

/// The drawn page or the section divider with this ID; `None` for any other ID (a deleted page, a generated page).
fn page_ref(q: &Q, id: Id<Page>) -> Option<PageRef> {
    if q.pages().contains(id) {
        return Some(PageRef::Drawn(id));
    }
    let section: Id<Section> = Id::from_uuid(id.as_uuid());
    q.sections()
        .contains(section)
        .then_some(PageRef::Divider(section))
}

#[cfg(test)]
mod tests {
    use ew_derive::ew_ids::Uuid;
    use ew_derive::ew_schema::{Project, Record};
    use serde_json::json;

    use super::*;

    fn uid(n: u128) -> String {
        Uuid::from_u128(n).hyphenated().to_string()
    }

    /// Released revisions labelled in the `letter` format past Z, stored out of order, plus a label outside the
    /// format; all undated, so only the label decides.
    fn model() -> ew_model::Model {
        let project: Project = serde_json::from_value(json!({
            "id": uid(1), "derivation_profile": "2027.1", "settings": {"revision_label": "letter"},
        }))
        .unwrap();
        let records: Vec<Record> = [
            (0x10u128, "AA"),
            (0x11, "Z"),
            (0x12, "Y"),
            (0x13, "AB"),
            (0x14, "pre-1"),
        ]
        .into_iter()
        .map(|(n, label)| {
            serde_json::from_value(json!({
                "$k": "revision", "id": uid(n), "label": label, "state": "released",
            }))
            .unwrap()
        })
        .collect();
        ew_model::Model::from_records(project, records, []).unwrap()
    }

    /// FX37a: folio 911 lists the revisions in the project's label order (`Y < Z < AA < AB`), not the natural order
    /// that puts `AA` before `Z`; a label outside the format comes first.
    #[test]
    fn the_rows_follow_the_project_label_order_past_z() {
        let m = model();
        let q = Q::new(&m);
        let labels: Vec<String> = revision_rows(&q)
            .rows
            .iter()
            .map(|r| match &r.cells[0] {
                Cell::Ident(l) => l.to_string(),
                other => panic!("label cell {other:?}"),
            })
            .collect();
        assert_eq!(labels, ["pre-1", "Y", "Z", "AA", "AB"]);
    }
}
