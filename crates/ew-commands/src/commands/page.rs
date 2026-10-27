// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page commands (doc 06 §3.1): `page.create`, `page.delete`, `page.renumber`.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_model::ModelError;
use ew_schema::{
    ContinuationPolicy, Device, Extra, Folio, FormDef, Function, Kind, Page, PageType, ReportBlock,
    Representation, Scale, Section, StructureAssignment,
};
use ew_units::Grid;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{GridS, IdentS, MlTextS, UuidS};
use crate::tx::TxBuilder;

fn multi_line() -> PageType {
    PageType::MultiLine
}

fn qet_grid() -> Grid {
    Grid::QET_10PX
}

/// `page.create`: a new drawn page (FR-PAG-005, doc 06 §5.6).
///
/// The folio is `{section, number, label}` when `number` is given. Otherwise `after` names the page to
/// insert after and the folio is proposed (doc 06 §5.6): `after.number + 1` if it lies in the section's
/// range (any number when the page has no section) and no page, divider or report block of that section
/// uses it; else the sub-page `{after.number, label}` with the printed label `<after's folio>.k` (`101.1`
/// after `101`, `101.1.1` after `101.1`) for the smallest `k ≥ 1` no page of the section prints. The
/// section is `after`'s; a different explicit `section` is rejected.
///
/// Rejected: neither `number` nor `after`; a printed folio (the label, else the number) another page of the
/// section already prints; kind `report` (report pages are generated, EW-INV-012). An explicit `number` is also
/// rejected (FT-459; FR-PAG-012, FR-REP-041) outside the section's folio range, at the folio of the section's divider
/// page (the range's first folio) and at a folio a page-oriented report block of the same section generates (its
/// first folio; with sub-page continuation also `<first>.<k>`). The section and form must exist.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatePage {
    /// The section of an explicit folio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub section: Option<Id<Section>>,
    /// An explicit folio number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    /// The label of an explicit folio (sub-pages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub label: Option<Ident>,
    /// The page to insert after, when the folio is proposed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub after: Option<Id<Page>>,
    /// The page type; default multi-line.
    #[serde(default = "multi_line")]
    pub kind: PageType,
    /// The page title.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub title: MlText,
    /// The page's structure assignment.
    #[serde(default, skip_serializing_if = "StructureAssignment::is_empty")]
    pub structure: StructureAssignment,
    /// The frame form copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub form: Option<Id<FormDef>>,
    /// The wiring grid; default the QET 10 px grid.
    #[serde(default = "qet_grid")]
    #[schemars(with = "GridS")]
    pub grid: Grid,
}

impl CreatePage {
    /// A page with an explicit folio and defaults elsewhere.
    pub fn at(section: Option<Id<Section>>, number: u32) -> CreatePage {
        CreatePage {
            section,
            number: Some(number),
            label: None,
            after: None,
            kind: PageType::MultiLine,
            title: MlText::default(),
            structure: StructureAssignment::default(),
            form: None,
            grid: Grid::QET_10PX,
        }
    }
}

/// The pages of a section (or of no section), with their folios.
fn section_pages(tx: &TxBuilder<'_>, section: Option<Id<Section>>) -> Vec<(Id<Page>, Folio)> {
    match section {
        Some(s) => tx
            .pages_in_section(s)
            .into_iter()
            .filter_map(|p| tx.get::<Page>(p).map(|page| (p, page.folio.clone())))
            .collect(),
        None => tx
            .iter::<Page>()
            .into_iter()
            .filter(|p| p.folio.section.is_none())
            .map(|p| (p.id, p.folio.clone()))
            .collect(),
    }
}

/// The folio proposal of doc 06 §5.6 after `after`.
fn propose_folio(tx: &TxBuilder<'_>, after: &Page) -> Result<(u32, Option<Ident>), CmdError> {
    let section = after.folio.section;
    let pages = section_pages(tx, section);
    let sec = match section {
        Some(s) => Some(tx.require::<Section>(s)?),
        None => None,
    };
    if let Some(n) = after.folio.number.checked_add(1) {
        let in_range = sec
            .as_ref()
            .is_none_or(|s| s.folio_range.0 <= n && n <= s.folio_range.1);
        let divider = sec.as_ref().is_some_and(|s| s.divider && s.folio_range.0 == n);
        let page_uses = pages.iter().any(|(_, f)| f.number == n);
        let report_uses = tx
            .iter::<ReportBlock>()
            .iter()
            .any(|b| b.section == section && b.first_folio == Some(n));
        if in_range && !divider && !page_uses && !report_uses {
            return Ok((n, None));
        }
    }
    // The label is the printed folio (doc 06 §5.6, FR-PAG-006): `101.1` after `101`, `101.1.1` after `101.1`.
    let base = match &after.folio.label {
        Some(l) => l.as_str().to_owned(),
        None => after.folio.number.to_string(),
    };
    for k in 1u32.. {
        let text = format!("{base}.{k}");
        let label: Ident = text
            .parse()
            .map_err(|_| CmdError::internal(format!("sub-page label `{text}` is not an identifier")))?;
        let taken = pages.iter().any(|(_, f)| f.label.as_ref() == Some(&label));
        if !taken {
            return Ok((after.folio.number, Some(label)));
        }
    }
    Err(CmdError::internal("no free sub-page label"))
}

/// The printed folio of a drawn page: its label, else its number (`ew_derive::folio_label`).
fn printed_folio(number: u32, label: Option<&Ident>) -> String {
    label.map_or_else(|| number.to_string(), |l| l.as_str().to_owned())
}

/// Why an explicit folio may not be created (FT-459; FR-PAG-012, FR-REP-041, FR-CHK-038), in this order: a number
/// outside the section's folio range; the printed folio of the section's divider page (its range's first folio,
/// FR-PAG-009); the printed folio of a generated page of a page-oriented report block of the same section (its first
/// folio, and with sub-page continuation any `<first>.<k>`; with next-folio continuation only the first folio is
/// known without derivation, the rest stays with the check). `None` when the folio is free of them.
fn explicit_folio_problem(
    tx: &TxBuilder<'_>,
    sec: Option<&Section>,
    section: Option<Id<Section>>,
    number: u32,
    printed: &str,
) -> Option<String> {
    if let Some(s) = sec {
        let (first, last) = s.folio_range;
        if number < first || number > last {
            return Some(format!(
                "folio {printed} lies outside section {}'s folio range {first} to {last}; choose a folio in the range",
                s.number
            ));
        }
        if s.divider && printed == first.to_string() {
            return Some(format!(
                "folio {printed} is the divider page of section {}; choose another folio",
                s.number
            ));
        }
    }
    for b in tx.iter::<ReportBlock>() {
        if b.section != section || b.embedded.is_some() {
            continue;
        }
        let Some(first) = b.first_folio else { continue };
        let head = first.to_string();
        let generated = printed == head
            || (b.continuation == ContinuationPolicy::SubPage
                && printed
                    .strip_prefix(&head)
                    .is_some_and(|rest| rest.starts_with('.')));
        if generated {
            return Some(format!(
                "folio {printed} is a page the report block at folio {first} generates; choose another folio"
            ));
        }
    }
    None
}

impl Command for CreatePage {
    const NAME: &'static str = "page.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.kind == PageType::Report {
            return tx
                .reject("report pages are generated from report blocks (EW-INV-012); create a report block");
        }
        if let Some(form) = self.form {
            tx.require::<ew_schema::LibCopy<FormDef>>(form)?;
        }
        let (section, number, label) = match (self.number, self.after) {
            (Some(n), _) => (self.section, n, self.label.clone()),
            (None, Some(after)) => {
                let after = tx.require::<Page>(after)?;
                if self.section.is_some() && self.section != after.folio.section {
                    return tx.reject(
                        "`section` differs from the section of `after`; omit it or give an explicit `number`",
                    );
                }
                let (n, label) = propose_folio(tx, &after)?;
                (after.folio.section, n, label)
            }
            (None, None) => {
                return tx.reject("give an explicit folio `number` or the page to insert `after`");
            }
        };
        let sec = match section {
            Some(s) => Some(tx.require::<Section>(s)?),
            None => None,
        };
        let printed = printed_folio(number, label.as_ref());
        if self.number.is_some() {
            if let Some(reason) = explicit_folio_problem(tx, sec.as_deref(), section, number, &printed) {
                return tx.reject(reason);
            }
        }
        if section_pages(tx, section)
            .iter()
            .any(|(_, f)| printed_folio(f.number, f.label.as_ref()) == printed)
        {
            return tx.reject(format!(
                "folio {printed} already exists in this section; choose a free folio"
            ));
        }
        let id: Id<Page> = tx.mint();
        tx.insert(Page {
            id,
            v: 1,
            folio: Folio {
                section,
                number,
                label,
                extra: Extra::new(),
            },
            kind: self.kind.clone(),
            structure: self.structure.clone(),
            form: self.form,
            title: self.title.clone(),
            description_override: None,
            scale: Scale::default(),
            grid: self.grid,
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((ew_schema::Kind::Page, id.cast())))
    }
}

/// `page.delete`: removes pages with everything they own (FR-PAG-025, doc 06 §3.4).
///
/// Each page is removed through the delete policies: its representations, connections and junctions
/// cascade, and an interruption-point function goes with its representation. Devices stay, unplaced,
/// unless `delete_devices` is set: then every device that had a representation of one of its functions on
/// a deleted page and has none left anywhere is removed too (with its functions and their remaining
/// connections). Bulk: it always previews (doc 06 §3.1).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeletePages {
    /// The pages, at least one.
    #[schemars(with = "Vec<UuidS>")]
    pub pages: Vec<Id<Page>>,
    /// Also delete devices left without any representation.
    #[serde(default)]
    pub delete_devices: bool,
}

impl Command for DeletePages {
    const NAME: &'static str = "page.delete";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.pages.is_empty() {
            return tx.reject("no pages given; name at least one page");
        }
        for page in &self.pages {
            tx.require::<Page>(*page)?;
        }
        // Devices with a representation on a deleted page, before the cascade removes the evidence.
        let mut candidates: BTreeSet<Id<Device>> = BTreeSet::new();
        if self.delete_devices {
            for page in &self.pages {
                for rep in tx.reps_on_page(*page) {
                    let Some(rep) = tx.get::<Representation>(rep) else {
                        continue;
                    };
                    for f in &rep.functions {
                        if let Some(device) = tx.get::<Function>(*f).and_then(|f| f.device) {
                            candidates.insert(device);
                        }
                    }
                }
            }
        }
        for page in &self.pages {
            tx.delete_if_exists((Kind::Page, page.cast()))?;
        }
        let mut kept: Vec<Id<Device>> = Vec::new();
        for device in candidates {
            let placed = tx
                .functions_of_device(device)
                .into_iter()
                .any(|f| !tx.reps_of_function(f).is_empty());
            if placed {
                continue;
            }
            match tx.delete_if_exists((Kind::Device, device.cast())) {
                Ok(()) => {}
                // A forbid link (a strip with terminals): the device stays, unplaced.
                Err(CmdError::Model(ModelError::Referenced { .. })) => kept.push(device),
                Err(e) => return Err(e),
            }
        }
        let note = (!kept.is_empty()).then(|| {
            let ids: Vec<String> = kept.iter().map(|d| d.to_string()).collect();
            format!(
                "kept {} device(s) still referenced: {}",
                kept.len(),
                ids.join(", ")
            )
        });
        Ok(Outcome {
            primary: None,
            note,
            notices: Vec::new(),
        })
    }
}

/// Which pages `page.renumber` renumbers.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PageScope {
    /// The listed pages.
    Pages(#[schemars(with = "Vec<UuidS>")] Vec<Id<Page>>),
    /// Every page of a section.
    Section(#[schemars(with = "UuidS")] Id<Section>),
    /// Every page of the project.
    Project,
}

/// `page.renumber`: renumbers folio numbers (FR-PAG-007). References are by ID, so every xref, tag and
/// TOC row follows without further ops.
///
/// The pages in scope are ordered by (section number, folio number, label in natural order, ID); pages
/// sharing a folio number (sub-pages) keep their labels and move together. With `keep_gaps`, a page gets
/// `start + (old − first_old) × step`; without, the i-th distinct old number (from 0) becomes
/// `start + i × step`. `step` is at least 1. A new folio that equals the folio of a page outside the scope
/// in the same section is rejected. Bulk.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenumberPages {
    /// The pages to renumber.
    pub scope: PageScope,
    /// The first new number.
    pub start: u32,
    /// The increment; default 1.
    #[serde(default = "crate::commands::common::one")]
    pub step: u32,
    /// Keep the gaps between the old numbers.
    #[serde(default)]
    pub keep_gaps: bool,
}

impl Command for RenumberPages {
    const NAME: &'static str = "page.renumber";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.step == 0 {
            return tx.reject("`step` must be at least 1");
        }
        let mut scope: Vec<Arc<Page>> = match &self.scope {
            PageScope::Pages(ids) => {
                let mut ids = ids.clone();
                ids.sort();
                ids.dedup();
                ids.into_iter()
                    .map(|id| tx.require::<Page>(id))
                    .collect::<Result<_, _>>()?
            }
            PageScope::Section(s) => {
                tx.require::<Section>(*s)?;
                tx.pages_in_section(*s)
                    .into_iter()
                    .filter_map(|id| tx.get::<Page>(id))
                    .collect()
            }
            PageScope::Project => tx.iter::<Page>(),
        };
        if scope.is_empty() {
            return tx.reject("no pages in scope; name pages, a section or the project with pages");
        }
        let section_number = |p: &Page| {
            p.folio
                .section
                .and_then(|s| tx.get::<Section>(s))
                .map(|s| s.number)
        };
        scope.sort_by(|a, b| {
            section_number(a)
                .cmp(&section_number(b))
                .then(a.folio.number.cmp(&b.folio.number))
                .then_with(|| match (&a.folio.label, &b.folio.label) {
                    (Some(x), Some(y)) => x.natural_cmp(y),
                    (x, y) => x.is_some().cmp(&y.is_some()),
                })
                .then(a.id.cmp(&b.id))
        });
        // The distinct old folio numbers (per section), in order; each gets one new number.
        let first_old = i64::from(scope[0].folio.number);
        let mut new_numbers: Vec<(Id<Page>, u32)> = Vec::with_capacity(scope.len());
        let mut index: i64 = -1;
        let mut last: Option<(Option<Id<Section>>, u32)> = None;
        for p in &scope {
            let key = (p.folio.section, p.folio.number);
            if last != Some(key) {
                index += 1;
                last = Some(key);
            }
            let new = if self.keep_gaps {
                i64::from(self.start) + (i64::from(p.folio.number) - first_old) * i64::from(self.step)
            } else {
                i64::from(self.start) + index * i64::from(self.step)
            };
            let Ok(new) = u32::try_from(new) else {
                return tx.reject(format!(
                    "new folio number {new} is out of range; choose another `start` or `step`"
                ));
            };
            new_numbers.push((p.id, new));
        }
        // The folios of the pages outside the renumbered ones, indexed once: a new number clashes when a
        // page outside the scope holds the same (section, number, label).
        let in_scope: BTreeSet<Id<Page>> = scope.iter().map(|p| p.id).collect();
        let taken: BTreeSet<(Option<Id<Section>>, u32, Option<Ident>)> = tx
            .iter::<Page>()
            .iter()
            .filter(|p| !in_scope.contains(&p.id))
            .map(|p| (p.folio.section, p.folio.number, p.folio.label.clone()))
            .collect();
        for (p, &(_, new)) in scope.iter().zip(&new_numbers) {
            if taken.contains(&(p.folio.section, new, p.folio.label.clone())) {
                return tx.reject(format!(
                    "folio {new} is already used by a page outside the renumbered pages; choose another `start` or include that page"
                ));
            }
        }
        for (id, new) in new_numbers {
            tx.update::<Page>(id, |p| p.folio.number = new)?;
        }
        Ok(Outcome::default())
    }
}
