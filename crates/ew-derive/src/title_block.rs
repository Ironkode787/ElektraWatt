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
//! - `revision.current.*`: the reserved `revision` records (doc 03 §18). Their members are read leniently:
//!   `id`, `label`, `date` (`"YYYY-MM-DD"`), `state` (printed as stored), `author` and `checker`. The
//!   current revision is the one with the greatest (`date`, natural order of `label`, `id`). A party
//!   reference is not yet specified by doc 03 §18; this version accepts the role string of a party
//!   (`"engineer"`) and takes the first party with that role.
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

use ew_mltext::{MlText, NaturalKey, natural_key};
use ew_rds::{Aspect, RdsSyntax};
use ew_schema::{Date, Kind, Page, Party, PartyRole, Project, Scale, Sha256, SupplySpec};

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

fn role_named(name: &str) -> Option<PartyRole> {
    ROLES.iter().find(|(n, _)| *n == name).map(|(_, r)| r.clone())
}

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

/// The members of one `revision` record this query reads.
struct RevisionView {
    id: String,
    label: String,
    date: Option<Date>,
    state: Option<String>,
    author: Option<PartyRole>,
    checker: Option<PartyRole>,
}

impl RevisionView {
    fn read(v: &serde_json::Value) -> Option<RevisionView> {
        let string = |k: &str| v.get(k).and_then(serde_json::Value::as_str).map(str::to_owned);
        let party = |k: &str| v.get(k).and_then(serde_json::Value::as_str).and_then(role_named);
        Some(RevisionView {
            id: string("id").unwrap_or_default(),
            label: string("label")?,
            date: v
                .get("date")
                .and_then(|d| serde_json::from_value::<Date>(d.clone()).ok()),
            state: string("state"),
            author: party("author"),
            checker: party("checker"),
        })
    }

    fn order_key(&self) -> (Option<Date>, NaturalKey, &str) {
        (self.date, natural_key(&self.label), &self.id)
    }
}

fn revision_scope(out: &mut Out, q: &Q, project: &Project) {
    let revisions: Vec<RevisionView> = q
        .reserved(Kind::Revision)
        .filter_map(RevisionView::read)
        .collect();
    let Some(current) = revisions.iter().max_by_key(|r| r.order_key()) else {
        return;
    };
    out.text("revision.current.label", &current.label);
    if let Some(s) = &current.state {
        out.text("revision.current.state", s);
    }
    if let Some(d) = current.date {
        out.date("revision.current.date", d);
    }
    for (path, role) in [
        ("revision.current.author.name", &current.author),
        ("revision.current.checker.name", &current.checker),
    ] {
        if let Some(party) = role.as_ref().and_then(|r| party_of(project, r)) {
            out.ml(path, &party.name);
        }
    }
}
