// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `valve_terminal.generate_overview` (`GenerateOverview`, doc 11 §5.5, FR-FLU-033): creates or refreshes the
//! assembly-overview pages of a valve terminal.

use std::collections::BTreeMap;

use ew_derive::{MainFunction, Q, assembly_members, overview_rep, short_tag};
use ew_ids::Id;
use ew_mltext::MlText;
use ew_schema::{Device, FnRole, Kind, LibCopy, Page, PageType, RepType, Representation, SymbolDef};
use ew_units::{Grid, Nm, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::commands::device::{PlaceSymbol, PlaceTarget};
use crate::commands::page::{CreatePage, printed_folio};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{MlTextS, NmS, UuidS};
use crate::text::text;
use crate::tx::TxBuilder;

/// `valve_terminal.generate_overview` (`GenerateOverview`, doc 11 §5.5, FR-FLU-033): lays out the overview of an
/// assembly (a valve terminal) on its assembly-overview pages.
///
/// The members are the station and its children in position order (`ew_derive::assembly_members`). Each member is
/// drawn by its overview representation (`ew_derive::overview_rep`); a member without one gets one of `symbol`
/// (its main function's, a `device.place` of an existing function). Left to right, each member stands with its
/// symbol's graphics from the cursor on, snapped up to the page's grid, at `y`; one that would pass `right` starts
/// the next page (never inside a module; a member wider than the band alone stands on a page of its own). The pages
/// are those the members' overview representations are on, in member order, then new assembly-overview pages after
/// the last (with its frame, grid, structure and `title`, else, when it is an overview page, its title), the first new one after
/// the station's own page when there is none. A member already drawn in member order inside the band (on a later page
/// than the member before, or on the same page right of it) stays where it stands, so that a refresh of an overview
/// in order, Golden A's 201 and 202 among them, changes nothing; a refresh moves the representations that stand
/// elsewhere; break marks and page-based tags follow from the pages. The notices name each page's members, the pages it creates and a page
/// it empties.
///
/// A member without a main function (a module added without one) is given the symbol's function as its main one.
/// Refused: a station without children and without valve positions of its own; a member without an overview
/// representation when `symbol` is absent; a
/// station without a page to start from. `Bulk`: the window previews it; one undo step. Outcome: the station.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GenerateOverview {
    /// The station (the assembly device).
    #[schemars(with = "UuidS")]
    pub station: Id<Device>,
    /// The left edge of the band the members fill on each page (page coordinates).
    #[schemars(with = "NmS")]
    pub left: Nm,
    /// The right edge of the band.
    #[schemars(with = "NmS")]
    pub right: Nm,
    /// The y of the members' insertion points.
    #[schemars(with = "NmS")]
    pub y: Nm,
    /// The overview symbol copy of a member drawn for the first time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub symbol: Option<Id<SymbolDef>>,
    /// The library documents to embed first (`lib/` JSON form): the symbol and its closure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
    /// The title of a new overview page (the window names the terminal: "Valve terminal -201A1"); empty: a page
    /// after an overview page takes its title, another none.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub title: MlText,
}

/// The first grid position at or right of `x`.
fn snap_up(grid: Grid, x: Nm) -> Option<Nm> {
    let k = grid.snap(x);
    let at = grid.at(k)?;
    if at.0 >= x.0 {
        Some(at)
    } else {
        grid.at(k.checked_add(1)?)
    }
}

impl Command for GenerateOverview {
    const NAME: &'static str = "valve_terminal.generate_overview";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        // A band outside the drawable range is refused in words (the command fuzz found an internal error).
        if self.left.0 > self.right.0
            || ![self.left, self.right, self.y]
                .into_iter()
                .all(crate::geometry::in_range)
        {
            return tx.reject(text!("refusal-overview-out-of-range"));
        }
        embed_closure(Self::NAME, tx, &self.copies)?;
        tx.require::<Device>(self.station)?;
        let q = Q::new(tx.base_model());
        let members = assembly_members(&q, self.station);
        // A terminal that carries its valves itself (the starter one) is an overview of its own.
        if members.len() < 2 && ew_derive::valve_positions(&q, self.station).is_none() {
            return tx.reject(text!(
                "refusal-overview-no-children",
                crate::words::device(tx, self.station)
            ));
        }
        let tag = |d: Id<Device>| short_tag(&q, d);
        // Each member: its main function, its overview representation and the symbol it is drawn with.
        let mut plan = Vec::new();
        for m in &members {
            let function = *q.get::<MainFunction>(*m);
            let rep = overview_rep(&q, *m);
            let symbol = match rep.and_then(|r| q.reps().get(r).map(|x| x.symbol)) {
                Some(s) => s,
                None => match self.symbol {
                    Some(s) => s,
                    None => {
                        return tx.reject(text!("refusal-overview-no-symbol", crate::words::device(tx, *m)));
                    }
                },
            };
            let def = tx.require::<LibCopy<SymbolDef>>(symbol)?;
            let span = def.item.graphic_span_x().unwrap_or((Nm::ZERO, Nm::ZERO));
            plan.push((*m, function, rep, symbol, span));
        }
        // The pages: those the overview representations are on, in member order.
        let mut pages: Vec<Id<Page>> = Vec::new();
        for (_, _, rep, _, _) in &plan {
            if let Some(page) = rep.and_then(|r| q.reps().get(r).map(|x| x.page)) {
                if !pages.contains(&page) {
                    pages.push(page);
                }
            }
        }
        let used_before = pages.clone();
        let reference = match pages.first() {
            Some(p) => *p,
            None => match (*q.get::<ew_derive::MainRep>(self.station))
                .and_then(|r| q.reps().get(r).map(|x| x.page))
            {
                Some(p) => p,
                None => {
                    return tx.reject(text!(
                        "refusal-overview-unplaced",
                        crate::words::device(tx, self.station)
                    ));
                }
            },
        };
        let mut created: Vec<Id<Page>> = Vec::new();
        let mut on_page: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        let mut index = 0usize;
        let mut cursor = self.left;
        for (m, function, rep, symbol, (x0, x1)) in plan {
            // A member drawn in member order inside the band (on a later page, or on this page right of the one before)
            // stays where it stands, so that a refresh of an overview in order changes nothing.
            let standing = rep
                .and_then(|r| q.reps().get(r))
                .and_then(|r| Some((pages.iter().position(|p| *p == r.page)?, r.pos.x)));
            if let Some((at, x)) = standing
                && x.0.saturating_add(x1.0) <= self.right.0
                && (at > index || (at == index && x.0.saturating_add(x0.0) >= cursor.0))
                && x.0.saturating_add(x0.0) >= self.left.0
            {
                index = at;
                cursor = Nm(x.0.saturating_add(x1.0));
                on_page.entry(index).or_default().push(tag(m));
                continue;
            }
            loop {
                if index >= pages.len() {
                    let page = new_page(tx, pages.last().copied().unwrap_or(reference), &self.title)?;
                    pages.push(page);
                    created.push(page);
                }
                let page = pages[index];
                let grid = tx.require::<Page>(page)?.grid;
                let Some(x) = cursor.0.checked_sub(x0.0).and_then(|x| snap_up(grid, Nm(x))) else {
                    return tx.reject(text!("refusal-overview-out-of-range"));
                };
                let right_edge = x.0.saturating_add(x1.0);
                let first = on_page.get(&index).is_none_or(Vec::is_empty);
                if right_edge > self.right.0 && !first {
                    index += 1;
                    cursor = self.left;
                    continue;
                }
                let y = grid.snap_point(self.y).unwrap_or(self.y);
                // A point beyond what a record holds (a band given at the coordinate bound) is refused in words.
                let bound = crate::geometry::MAX_COORD;
                if x.0.abs() > bound || y.0.abs() > bound {
                    return tx.reject(text!("refusal-overview-out-of-range"));
                }
                let pos = Pt2::new(x, y);
                match rep {
                    Some(r) => {
                        let old = tx.require::<Representation>(r)?;
                        if old.page != page || old.pos != pos {
                            tx.update::<Representation>(r, |x| {
                                x.page = page;
                                x.pos = pos;
                            })?;
                        }
                    }
                    None => {
                        // A member without a main function (a module added without one) gets the symbol's as its main.
                        let target = match function {
                            Some(function) => PlaceTarget::ExistingFunction { function },
                            None => PlaceTarget::JoinDevice {
                                device: m,
                                fndef: None,
                                role: FnRole::Main,
                            },
                        };
                        PlaceSymbol {
                            page,
                            symbol,
                            pos,
                            orient: Orient2::default(),
                            rep_type: Some(RepType::Overview),
                            variant: 0,
                            target,
                            connect: Vec::new(),
                            copies: Vec::new(),
                            into: None,
                            replace: false,
                        }
                        .run(tx)?;
                    }
                }
                on_page.entry(index).or_default().push(tag(m));
                cursor = Nm(right_edge);
                break;
            }
        }
        let folio = |tx: &TxBuilder<'_>, p: Id<Page>| {
            tx.get::<Page>(p)
                .map(|x| printed_folio(x.folio.number, x.folio.label.as_ref()))
                .unwrap_or_default()
        };
        let mut notices = Vec::new();
        for (i, tags) in &on_page {
            let page = pages[*i];
            let new = if created.contains(&page) {
                " (new page)"
            } else {
                ""
            };
            notices.push(format!("page {}{new}: {}", folio(tx, page), tags.join(", ")));
        }
        for (i, page) in used_before.iter().enumerate() {
            if !on_page.contains_key(&i) {
                notices.push(format!(
                    "page {} holds no member of the overview any more",
                    folio(tx, *page)
                ));
            }
        }
        Ok(Outcome {
            notices,
            ..Outcome::primary((Kind::Device, self.station.cast()))
        })
    }
}

/// A new assembly-overview page after `after`, with its frame, grid and structure, titled `title`, else with the
/// title of `after` when it is an assembly-overview page.
fn new_page(tx: &mut TxBuilder<'_>, after: Id<Page>, title: &MlText) -> Result<Id<Page>, CmdError> {
    let before = tx.require::<Page>(after)?;
    let title = if !title.is_empty() {
        title.clone()
    } else if before.kind == PageType::AssemblyOverview {
        before.title.clone()
    } else {
        MlText::default()
    };
    let out = CreatePage {
        section: None,
        number: None,
        label: None,
        after: Some(after),
        kind: PageType::AssemblyOverview,
        title,
        structure: before.structure.clone(),
        form: before.form,
        grid: before.grid,
    }
    .run(tx)?;
    out.primary
        .map(|(_, id)| id.cast())
        .ok_or_else(|| CmdError::internal("page.create made no page"))
}
