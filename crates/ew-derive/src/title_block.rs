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
//!   `page.title` of a page drawn to scale 1:n reads `"<title> · Scale 1:n"` (WP-S4.01 D9, FR-LAY-010); every other
//!   page prints its plain title.
//!   `page.last_changed_in` (drawn pages only) is the label of the revision that last changed the page
//!   (doc 09 §2.6, doc 15 §5.6; the no-digest branch, page digests do not exist yet): the current open
//!   revision if it names the page in its `page_notes`; else the latest released revision whose changed
//!   pages contain it (`release.changed_pages` if present, else the pages of its `page_notes`); else the
//!   first (lowest ranked) revision. `release.changed_pages` is the typed release record's member (WP-S1.18
//!   part 1, D2); a line whose entries are not page IDs does not load as a record.
//! - `revision.current.*`: the typed `revision` records (doc 03 §18, FT-165): `label`, `date`, `state`
//!   (printed as its stored name; a state of a newer format as its text), `author` and `checker`.
//!   Withdrawal records (state `withdrawn`) are ignored, and so is the revision whose label a withdrawal
//!   record names by ID in `withdraws` (it stays `released`, EW-INV-020). The current revision is the greatest by (open
//!   before any other state, undated before dated, `date`, the label order of [`revision_label_key`], `id`): the open
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
//! an object `{"logo": <AssetRef>, "notice": <MlText JSON>}` (`logo` also in its earlier form, a bare
//! `"sha256:<64 hex>"`, which takes the held asset's format and size, else reads as a PNG of unknown size:
//! [`ew_schema::company_logo`], [`Q::logo_asset`]). Each member is parsed on its own; a missing or malformed member
//! leaves its path absent.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{MlText, NaturalKey, natural_key};
use ew_rds::{Aspect, RdsSyntax};
use ew_schema::{
    Date, LabelFormat, Page, Party, PartyRole, Project, Revision, RevisionState, Scale, SupplySpec,
};

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
    /// An image asset by its reference (hash, format, intrinsic size: the logo); prints no text.
    Asset(ew_schema::AssetRef),
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
        project_scope(&mut out, q, project);
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

fn project_scope(out: &mut Out, q: &Q, project: &Project) {
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
    if let Some(mut logo) = ew_schema::company_logo(&project.extra) {
        // The earlier bare-hash form names no format: the held asset's own reference says it.
        let bare = project
            .extra
            .get(ew_schema::COMPANY_MEMBER)
            .and_then(|c| c.get("logo"))
            .is_some_and(serde_json::Value::is_string);
        if bare {
            if let Some(held) = q.logo_asset(&logo.sha256) {
                logo = held;
            }
        }
        out.0.insert("project.logo".to_owned(), TbValue::Asset(logo));
    }
    if let Some(company) = project.extra.get(ew_schema::COMPANY_MEMBER) {
        if let Some(notice) = company
            .get("notice")
            .and_then(|v| serde_json::from_value::<MlText>(v.clone()).ok())
        {
            out.ml("project.notice", &notice);
        }
    }
}

/// The title-block title of a page drawn to scale 1:n (WP-S4.01 D9): `"<title> · Scale 1:n"` in every language of the
/// title, `"Scale 1:n"` for an empty title.
fn scaled_title(title: &MlText, n: u32) -> MlText {
    let suffix = format!("Scale 1:{n}");
    let with = |t: &str| {
        if t.is_empty() {
            suffix.clone()
        } else {
            format!("{t} · {suffix}")
        }
    };
    match title.view() {
        ew_mltext::MlTextView::Neutral(t) => MlText::neutral(&with(t)),
        ew_mltext::MlTextView::Langs(m) => {
            MlText::from_langs(m.iter().map(|(tag, t)| (tag.clone(), with(t))))
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
    let drawn: Option<&Arc<Page>> = match page {
        PageRef::Drawn(id) => q.pages().get(id),
        PageRef::Divider(_) | PageRef::Generated { .. } => None,
    };
    // WP-S4.01 D9 (FR-LAY-010): a page drawn to a scale 1:n prints "<title> · Scale 1:n" in its title block; the
    // TOC keeps the plain title (it reads `PageTitle`), and a not-to-scale page is unchanged.
    let title = q.get::<PageTitle>(page);
    match drawn.map(|p| &p.scale) {
        Some(Scale::OneTo(n)) => out.ml("page.title", &scaled_title(&title, *n)),
        _ => out.ml("page.title", &title),
    }

    // The description: the page's override, else the project description (doc 09 §2.7).
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

/// The order of revision labels everywhere they are ranked (the title block's current revision, `r_last`, the
/// revision overview's rows; doc 15 §5.2, FR-REV-002): a label in the project's label `format` by
/// [`LabelFormat::order_key`] (`Y < Z < AA` in the `letter` format), after every label outside the format, which
/// keep their natural order ([`natural_key`]) among themselves, as the open-revision normaliser ranks them. Two
/// labels of one position (`V1.0` twice) fall back to the natural order too; callers break the rest by ID.
pub fn revision_label_key(format: &LabelFormat, label: &str) -> (Option<(u64, u64)>, NaturalKey) {
    (format.order_key(label), natural_key(label))
}

/// What this query reads of one `revision` record.
struct RevisionView<'q> {
    rev: &'q Revision,
    /// The label's place in the project's label order ([`revision_label_key`]).
    label: (Option<(u64, u64)>, NaturalKey),
    /// The pages named in `page_notes`.
    noted_pages: Vec<Id<Page>>,
    /// `release.changed_pages`, if that member is present (the typed release record, WP-S1.18 D2).
    changed_pages: Option<Vec<Id<Page>>>,
}

impl<'q> RevisionView<'q> {
    fn read(rev: &'q Revision, format: &LabelFormat) -> RevisionView<'q> {
        RevisionView {
            rev,
            label: revision_label_key(format, rev.label.as_str()),
            noted_pages: rev.page_notes.iter().map(|n| n.page).collect(),
            changed_pages: rev.release.as_ref().and_then(|r| r.changed_pages.clone()),
        }
    }

    fn is_open(&self) -> bool {
        self.rev.is_open()
    }

    /// Ranking: open above everything else; within a rank a missing date is newest, then the date, the label
    /// order ([`revision_label_key`]) and the id.
    #[allow(clippy::type_complexity)]
    fn order_key(
        &self,
    ) -> (
        bool,
        bool,
        Option<Date>,
        (Option<(u64, u64)>, NaturalKey),
        Id<Revision>,
    ) {
        (
            self.is_open(),
            self.rev.date.is_none(),
            self.rev.date,
            self.label.clone(),
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

/// The revisions that are neither withdrawal records nor withdrawn by one (`withdraws` names the ID).
fn live_revisions(q: &Q) -> Vec<RevisionView<'_>> {
    let table = q.table::<Revision>();
    let format = q.project().settings.revision_label_format();
    let withdrawn: Vec<Id<Revision>> = table
        .iter()
        .filter(|(_, r)| r.state == RevisionState::Withdrawn)
        .filter_map(|(_, r)| r.withdraws)
        .collect();
    table
        .iter()
        .filter(|(_, r)| r.state != RevisionState::Withdrawn && !withdrawn.contains(&r.id))
        .map(|(_, r)| RevisionView::read(r, &format))
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

/// `page.last_changed_in` of a drawn page: the no-digest branch of [`last_changed_in`] (page digests are computed in
/// `ew-api`, which hands its digest-based value to the page's bindings; WP-S1.18 part 2).
fn last_changed_scope(out: &mut Out, q: &Q, page: PageRef) {
    let PageRef::Drawn(id) = page else {
        return;
    };
    if let Some(label) = last_changed_in(q, id, None) {
        out.text("page.last_changed_in", &label);
    }
}

// ---------------------------------------------------------------------------------------------------------
// The page change status of doc 15 §5.6 over the revision records (WP-S1.18 part 2, D7). The page digests are not
// here: they are computed in `ew-api` over the page scene and handed to these functions as `changed_now`.

/// The pages a revision records as changed: `release.changed_pages` if its release record has that member, else the
/// pages its `page_notes` name, in their order (doc 15 §5.6: a release without previous digests changes the pages of
/// its notes). A page ID may name a page that no longer exists, or a generated page by its v5 ID.
pub fn recorded_changed_pages(rev: &Revision) -> Vec<Id<Page>> {
    rev.release
        .as_ref()
        .and_then(|r| r.changed_pages.clone())
        .unwrap_or_else(|| rev.page_notes.iter().map(|n| n.page).collect())
}

/// `r_last` of doc 15 §5.6: the latest released revision that no withdrawal record withdraws, in the title block's
/// rank (date, then the label order of [`revision_label_key`], then the ID).
pub fn last_released_revision(q: &Q) -> Option<Arc<Revision>> {
    live_revisions(q)
        .into_iter()
        .filter(|r| r.rev.state == RevisionState::Released)
        .max_by_key(RevisionView::order_key)
        .map(|r| Arc::new(r.rev.clone()))
}

/// The open revision `o` of doc 15 §5.6, if the current revision is open (EW-INV-016: at most one is).
pub fn open_revision(q: &Q) -> Option<Arc<Revision>> {
    live_revisions(q)
        .into_iter()
        .max_by_key(RevisionView::order_key)
        .filter(RevisionView::is_open)
        .map(|r| Arc::new(r.rev.clone()))
}

/// `last_changed_in(p)` of doc 15 §5.6: the open revision's label if `p` is changed now; else the latest released
/// revision whose changed pages ([`recorded_changed_pages`]) contain `p`; else the first (lowest ranked) revision.
/// `None` without revisions.
///
/// `changed_now` is the caller's page status in the digest branch (`Some`: the page's digest differs from `r_last`'s,
/// computed by `ew-api`); `None` is the no-digest branch, where `p` is changed now when the open revision's
/// `page_notes` name it. Withdrawal records, and the revisions they withdraw, take no part.
pub fn last_changed_in(q: &Q, page: Id<Page>, changed_now: Option<bool>) -> Option<String> {
    let revisions = live_revisions(q);
    let current = revisions.iter().max_by_key(|r| r.order_key());
    let open = current.filter(|c| c.is_open());
    let changed = |r: &RevisionView<'_>| match changed_now {
        Some(now) => now,
        None => r.noted_pages.contains(&page),
    };
    let released = revisions
        .iter()
        .filter(|r| !r.is_open())
        .filter(|r| r.changed_pages.as_ref().unwrap_or(&r.noted_pages).contains(&page))
        .max_by_key(|r| r.order_key());
    let label = match open {
        Some(o) if changed(o) => Some(o),
        _ => released.or_else(|| revisions.iter().min_by_key(|r| r.order_key())),
    };
    label.map(|r| r.rev.label.as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_mltext::LangTag;

    // WP-S4.01 part 3 (D9): the title-block value of a page drawn to scale 1:n.
    #[test]
    fn a_scaled_page_title_reads_title_dot_scale_in_every_language() {
        assert_eq!(
            scaled_title(&MlText::neutral("Cabinet Layout"), 10),
            MlText::neutral("Cabinet Layout · Scale 1:10")
        );
        assert_eq!(
            scaled_title(&MlText::neutral(""), 5),
            MlText::neutral("Scale 1:5")
        );
        let en: LangTag = "en-GB".parse().expect("a tag");
        let nl: LangTag = "nl-NL".parse().expect("a tag");
        let title = MlText::from_langs([(en.clone(), "Cabinet Layout"), (nl.clone(), "Kastindeling")]);
        let scaled = scaled_title(&title, 10);
        assert_eq!(scaled.get(&en), Some("Cabinet Layout · Scale 1:10"));
        assert_eq!(scaled.get(&nl), Some("Kastindeling · Scale 1:10"));
    }
}
