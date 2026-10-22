// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Title-block values: `title_block(page)` (doc 09 §2.6–§2.7, FR-REP-033, FR-REP-036, FR-REP-037,
//! FR-REV-003, D-05, D-06, D-30).
//!
//! The query resolves exactly the paths of the pinned profile's binding set
//! ([`crate::TitleBlockBindings`]); a path without a value is absent from [`TitleBlock::values`] and
//! renders empty (decision log ID-40). No value ever carries placeholder text.
//!
//! # Sources
//!
//! - `project.*`: the project row (`MasterData`, `SupplySpec`).
//! - `party.<role>.*`: the first party of that role in `Project.parties` order; roles are `customer`,
//!   `end_user`, `panel_builder`, `author`, `checker` and `engineer`.
//! - `page.*` and `doc.*`: [`PageOrder`] (folio, neighbours, position), [`PageTitle`],
//!   [`PageRefStructure`] and the page row. A page that is not in the page order has none of them.
//!   `page.last_changed_in` (drawn pages only) is the label of the revision that last changed the page
//!   (doc 09 §2.6, doc 15 §5.6; the no-digest branch, page digests do not exist yet): the current open
//!   revision if it names the page in its `page_notes`; else the latest released revision whose changed
//!   pages contain it (`release.changed_pages` if present, else the pages of its `page_notes`); else the
//!   first (lowest ranked) revision. The release record is not typed yet (it comes with
//!   `revision.release`), so `release.changed_pages` is read from the record's `extra` as an array of UUID
//!   strings; malformed entries are skipped.
//! - `revision.current.*`: the typed `revision` records (doc 03 §18, FT-165): `label`, `date`, `state`
//!   (printed as its stored name; a state of a newer format as its text), `author` and `checker`.
//!   Withdrawal records (state `withdrawn`) are ignored, and so is the revision whose label a withdrawal
//!   record names by ID in `withdraws` (it stays `released`, EW-INV-020). The current revision is the greatest by (open
//!   before any other state, undated before dated, `date`, natural order of `label`, `id`): the open
//!   revision (EW-INV-016: at most one) wins over released ones, and a missing date counts as newest
//!   within its state. Author and checker are party roles; the first party with that role gives the name.
//!
//! # Supply text
//!
//! `project.supply` renders as `"<phases> x <u_nom> VAC[ + N][ + PE], <frequency> Hz"` (FR-PRJ-004, D-18):
//! `"1 x 230 VAC + N + PE, 50 Hz"`. A frequency of 0 is DC: `"VDC"` and no frequency part. Numbers print
//! without trailing zeros. The earthing system is not part of the line.
//!
//! # Not resolved yet
//!
//! `project.control_voltage` and `project.consumption` are absent: `ew-schema`'s `Quantity` has no
//! canonical text form yet (doc 09 `qty(unit, decimals)` filter, FR-PRJ-005).
//!
//! # Provisional: company data
//!
//! `project.logo` and `project.notice` (the basic project's company data, doc 07 §2.8, ID-40) have no
//! record member yet. **Provisional** source until that decision is made: `Project.extra["ew.company"]`,
//! an object `{"logo": "sha256:<64 hex>", "notice": <MlText JSON>}`. Each member is parsed on its own; a
//! missing or malformed member leaves its path absent.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{MlText, NaturalKey, natural_key};
use ew_rds::{Aspect, RdsSyntax};
use ew_schema::{Date, Page, Party, PartyRole, Project, Revision, RevisionState, Scale, Sha256, SupplySpec};

use crate::Q;
use crate::pages::{PageOrder, PageRef, PageRefStructure, PageTitle};
use crate::query::{Policy, Query};

/// One bound value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TbValue {
    /// Plain text.
    Text(String),
    /// Multi-language text.
    Ml(MlText),
    /// A date, printed in the project's date format by the form layer.
    Date(Date),
    /// A tag in its two printed forms.
    Tag {
        /// The full tag.
        full: String,
        /// The displayed tag.
        displayed: String,
    },
    /// An image asset by content hash (the logo); prints no text.
    Asset(Sha256),
}

/// The values of every path of the profile's title-block binding set that has a value on one page.
/// A path without a value is absent (it renders empty, decision log ID-40).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TitleBlock {
    /// Path (`page.title`) → value.
    pub values: BTreeMap<String, TbValue>,
}

impl TitleBlock {
    /// The value at a path.
    pub fn get(&self, path: &str) -> Option<&TbValue> {
        self.values.get(path)
    }
}

/// `title_block(page)`.
#[derive(Debug)]
pub struct TitleBlockValues;

impl Query for TitleBlockValues {
    type Key = PageRef;
    type Value = TitleBlock;
    const FAMILY: &'static str = "title_block";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PageRef) -> Arc<TitleBlock> {
        let mut out = Out::default();
        let project = q.project();
        project_scope(&mut out, project);
        party_scope(&mut out, project);
        page_scope(&mut out, q, project, *key);
        revision_scope(&mut out, q, project);

        let wanted = &q.profile().title_block.paths;
        let values = out
            .0
            .into_iter()
            .filter(|(path, _)| wanted.iter().any(|w| w == path))
            .collect();
        Arc::new(TitleBlock { values })
    }
}

/// The candidate values; empty text and empty multi-language text are never stored.
#[derive(Default)]
struct Out(BTreeMap<String, TbValue>);

impl Out {
    fn text(&mut self, path: &str, text: &str) {
        if !text.is_empty() {
            self.0.insert(path.to_owned(), TbValue::Text(text.to_owned()));
        }
    }

    fn ml(&mut self, path: &str, text: &MlText) {
        if !text.is_empty() {
            self.0.insert(path.to_owned(), TbValue::Ml(text.clone()));
        }
    }

    fn date(&mut self, path: &str, date: Date) {
        self.0.insert(path.to_owned(), TbValue::Date(date));
    }
}

fn project_scope(out: &mut Out, project: &Project) {
    let master = &project.master;
    out.ml("project.description", &master.description);
    out.ml("project.long_description", &master.long_description);
    out.ml("project.document_title", &master.document_title);
    if let Some(r) = &master.project_ref {
        out.text("project.reference", r.as_str());
    }
    if let Some(d) = &master.drawing_id {
        out.text("project.drawing_id", d.as_str());
    }
    if let Some(o) = &master.object_id {
        out.text("project.object", o.as_str());
    }
    if let Some(d) = master.created {
        out.date("project.created", d);
    }
    if let Some(d) = master.edited {
        out.date("project.edited", d);
    }
    if let Some(s) = &project.supply {
        out.text("project.supply", &supply_text(s));
    }
    // `project.control_voltage` and `project.consumption`: no canonical text form of `Quantity` yet.
    if let Some(company) = project.extra.get("ew.company") {
        if let Some(logo) = company
            .get("logo")
            .and_then(|v| serde_json::from_value::<Sha256>(v.clone()).ok())
        {
            out.0.insert("project.logo".to_owned(), TbValue::Asset(logo));
        }
        if let Some(notice) = company
            .get("notice")
            .and_then(|v| serde_json::from_value::<MlText>(v.clone()).ok())
        {
            out.ml("project.notice", &notice);
        }
    }
}

/// The supply line: `"1 x 230 VAC + N + PE, 50 Hz"`; frequency 0 is DC (`"VDC"`, no frequency).
fn supply_text(s: &SupplySpec) -> String {
    let ac = s.frequency.0 != 0;
    let mut t = format!("{} x {} {}", s.phases, s.u_nom, if ac { "VAC" } else { "VDC" });
    if s.neutral {
        t.push_str(" + N");
    }
    if s.pe {
        t.push_str(" + PE");
    }
    if ac {
        t.push_str(&format!(", {} Hz", s.frequency));
    }
    t
}

/// The roles of the party scope with their path segment.
const ROLES: [(&str, PartyRole); 6] = [
    ("customer", PartyRole::Customer),
    ("end_user", PartyRole::EndUser),
    ("panel_builder", PartyRole::PanelBuilder),
    ("author", PartyRole::Author),
    ("checker", PartyRole::Checker),
    ("engineer", PartyRole::Engineer),
];

fn party_of<'a>(project: &'a Project, role: &PartyRole) -> Option<&'a Party> {
    project.parties.iter().find(|p| p.role == *role)
}

fn party_scope(out: &mut Out, project: &Project) {
    for (segment, role) in ROLES {
        let Some(party) = party_of(project, &role) else {
            continue;
        };
        out.ml(&format!("party.{segment}.name"), &party.name);
        if let Some(a) = &party.address {
            let fields = [
                ("street", &a.street),
                ("postcode", &a.postcode),
                ("city", &a.city),
                ("country", &a.country),
            ];
            for (field, value) in fields {
                if let Some(v) = value {
                    out.text(&format!("party.{segment}.address.{field}"), v);
                }
            }
        }
    }
}

fn page_scope(out: &mut Out, q: &Q, project: &Project, page: PageRef) {
    let order = q.get::<PageOrder>(());
    let (Some(entry), Some(position)) = (order.entry(page), order.position(page)) else {
        return;
    };
    out.text("page.folio", &entry.folio);
    if let Some(p) = order.prev(page) {
        out.text("page.prev", &p.folio);
    }
    if let Some(n) = order.next(page) {
        out.text("page.next", &n.folio);
    }
    out.text("doc.page_index", &(position + 1).to_string());
    out.text("doc.page_count", &order.len().to_string());
    out.ml("page.title", &q.get::<PageTitle>(page));

    // The description: the page's override, else the project description (doc 09 §2.7).
    let drawn: Option<&Arc<Page>> = match page {
        PageRef::Drawn(id) => q.pages().get(id),
        PageRef::Divider(_) | PageRef::Generated { .. } => None,
    };
    match drawn.and_then(|p| p.description_override.as_ref()) {
        Some(o) if !o.is_empty() => out.ml("page.description", o),
        _ => out.ml("page.description", &project.master.description),
    }
    if let Some(Scale::OneTo(n)) = drawn.map(|p| &p.scale) {
        out.text("page.scale", &format!("1:{n}"));
    }

    last_changed_scope(out, q, page);

    let structure = q.get::<PageRefStructure>(page);
    let syntax = RdsSyntax::default();
    out.text(
        "page.function",
        &structure.render_aspect(Aspect::Function, &syntax),
    );
    out.text(
        "page.location",
        &structure.render_aspect(Aspect::Location, &syntax),
    );
    out.text(
        "page.document_kind",
        &structure.render_aspect(Aspect::DocumentKind, &syntax),
    );
}

/// What this query reads of one `revision` record.
struct RevisionView<'q> {
    rev: &'q Revision,
    /// The pages named in `page_notes`.
    noted_pages: Vec<Id<Page>>,
    /// `release.changed_pages`, if that member is present (the untyped release record, `extra`).
    changed_pages: Option<Vec<Id<Page>>>,
}

impl<'q> RevisionView<'q> {
    fn read(rev: &'q Revision) -> RevisionView<'q> {
        RevisionView {
            rev,
            noted_pages: rev.page_notes.iter().map(|n| n.page).collect(),
            changed_pages: rev
                .extra
                .get("release")
                .and_then(|r| r.get("changed_pages"))
                .and_then(serde_json::Value::as_array)
                .map(|pages| pages.iter().filter_map(page_id).collect()),
        }
    }

    fn is_open(&self) -> bool {
        self.rev.is_open()
    }

    /// Ranking: open above everything else; within a rank a missing date is newest, then the date, the
    /// natural order of the label and the id.
    fn order_key(&self) -> (bool, bool, Option<Date>, NaturalKey, Id<Revision>) {
        (
            self.is_open(),
            self.rev.date.is_none(),
            self.rev.date,
            natural_key(self.rev.label.as_str()),
            self.rev.id,
        )
    }
}

/// The printed form of a revision state: its stored name, or a newer format's value as text.
fn state_text(state: &RevisionState) -> Option<String> {
    match state {
        RevisionState::Open => Some("open".into()),
        RevisionState::Released => Some("released".into()),
        RevisionState::Withdrawn => Some("withdrawn".into()),
        RevisionState::Unknown(v) => v.as_str().map(str::to_owned),
    }
}

/// A page ID from a UUID string.
fn page_id(v: &serde_json::Value) -> Option<Id<Page>> {
    serde_json::from_value(v.clone()).ok()
}

/// The revisions that are neither withdrawal records nor withdrawn by one (`withdraws` names the ID).
fn live_revisions(q: &Q) -> Vec<RevisionView<'_>> {
    let table = q.table::<Revision>();
    let withdrawn: Vec<Id<Revision>> = table
        .iter()
        .filter(|(_, r)| r.state == RevisionState::Withdrawn)
        .filter_map(|(_, r)| r.withdraws)
        .collect();
    table
        .iter()
        .filter(|(_, r)| r.state != RevisionState::Withdrawn && !withdrawn.contains(&r.id))
        .map(|(_, r)| RevisionView::read(r))
        .collect()
}

fn revision_scope(out: &mut Out, q: &Q, project: &Project) {
    let revisions = live_revisions(q);
    let Some(current) = revisions.iter().max_by_key(|r| r.order_key()) else {
        return;
    };
    let rev = current.rev;
    out.text("revision.current.label", rev.label.as_str());
    if let Some(s) = state_text(&rev.state) {
        out.text("revision.current.state", &s);
    }
    if let Some(d) = rev.date {
        out.date("revision.current.date", d);
    }
    for (path, role) in [
        ("revision.current.author.name", &rev.author),
        ("revision.current.checker.name", &rev.checker),
    ] {
        if let Some(party) = role.as_ref().and_then(|r| party_of(project, r)) {
            out.ml(path, &party.name);
        }
    }
}

/// `page.last_changed_in` of a drawn page (the no-digest branch).
fn last_changed_scope(out: &mut Out, q: &Q, page: PageRef) {
    let PageRef::Drawn(id) = page else {
        return;
    };
    let revisions = live_revisions(q);
    let current = revisions.iter().max_by_key(|r| r.order_key());
    let released = revisions
        .iter()
        .filter(|r| !r.is_open())
        .filter(|r| r.changed_pages.as_ref().unwrap_or(&r.noted_pages).contains(&id))
        .max_by_key(|r| r.order_key());
    let label = match current {
        Some(c) if c.is_open() && c.noted_pages.contains(&id) => Some(c),
        _ => released.or_else(|| revisions.iter().min_by_key(|r| r.order_key())),
    };
    if let Some(r) = label {
        out.text("page.last_changed_in", r.rev.label.as_str());
    }
}
