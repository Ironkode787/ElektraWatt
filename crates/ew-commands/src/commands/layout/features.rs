// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The holes of a surface (doc 12 §3.7, doc 12-3d §10.6): a hole by hand (`layout.feature.add`,
//! `layout.feature.edit`) and the choice that moves a derived hole or leaves it out (`layout.hole_choice`). A hole's
//! kind and size are read by [`Hole::parse`], the one parser of the part editor's cells, the Add hole form and the
//! command line; its refusal is its words.

use ew_derive::Q;
use ew_ids::Id;
use ew_mltext::MlText;
use ew_schema::{Carrier, Hole, HoleChoice, HoleKind, Kind, MountingSurface, Placement, SurfaceFeature};
use ew_units::{Nm, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{SurfacePt2S, UuidS};
use crate::tx::TxBuilder;

/// The hole of `kind` (a [`HoleKind`] word) and `size` as typed; the refusal in words at `at`.
fn hole_of(name: &'static str, kind: &str, size: &str) -> Result<Hole, CmdError> {
    let Some(k) = HoleKind::of_word(kind.trim()) else {
        return Err(CmdError::invalid_params(
            name,
            "/kind",
            text!("refusal-layout-feature-kind", kind),
        ));
    };
    Hole::parse(k, size).map_err(|e| {
        CmdError::invalid_params(
            name,
            "/size",
            text!("refusal-layout-feature-size", ew_derive::text::hole_error(&e)),
        )
    })
}

/// Refuses a centre `at` off the sheet of `surface`.
fn on_sheet(name: &'static str, surface: &MountingSurface, at: Pt2) -> Result<(), CmdError> {
    let (w, h) = ew_derive::layout::sheet_size(surface).unwrap_or((Nm::ZERO, Nm::ZERO));
    if at.x < Nm::ZERO || at.y < Nm::ZERO || at.x > w || at.y > h {
        let mm = |n: Nm| ew_derive::mm_tenth_short(i128::from(n.0));
        return Err(CmdError::invalid_params(
            name,
            "/at",
            text!("refusal-layout-feature-off", mm(w), mm(h)),
        ));
    }
    Ok(())
}

/// A purpose as typed: an empty text is none.
fn purpose_of(text: Option<&str>) -> MlText {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => MlText::neutral(t),
        None => MlText::default(),
    }
}

/// `layout.feature.add` (`AddFeature`, Add hole here…): a hole by hand on a surface, its centre `at` in the surface's
/// frame, its `kind` and `size` as [`Hole::parse`] reads them ("fixing", "M6"), what it is for. Rejected in words: a
/// surface that does not exist, a kind or a size that does not read, a centre off the sheet. Interactive. Outcome: the
/// feature.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddFeature {
    /// The surface.
    #[schemars(with = "UuidS")]
    pub surface: Id<MountingSurface>,
    /// The centre, in the surface's frame.
    #[schemars(with = "SurfacePt2S")]
    pub at: Pt2,
    /// The kind: `round`, `fixing`, `cutout`, `gland`, `rect`.
    pub kind: String,
    /// The size as typed: "4.5", "M6", "22 mm", "M20", "92 × 92 R 2".
    pub size: String,
    /// What it is for ("Earth stud").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

impl Command for AddFeature {
    const NAME: &'static str = "layout.feature.add";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(surface) = tx.get::<MountingSurface>(self.surface) else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/surface",
                text!("refusal-params-surface-gone"),
            ));
        };
        let hole = hole_of(Self::NAME, &self.kind, &self.size)?;
        on_sheet(Self::NAME, &surface, self.at)?;
        let id: Id<SurfaceFeature> = tx.mint();
        tx.insert(SurfaceFeature {
            id,
            v: 1,
            surface: self.surface,
            at: self.at,
            hole,
            purpose: purpose_of(self.purpose.as_deref()),
            cable: None,
            for_cable: false,
            extra: Default::default(),
        })?;
        Ok(Outcome::primary((Kind::SurfaceFeature, id.cast())))
    }
}

/// `layout.feature.edit` (`EditFeature`): moves a hole by hand (`at`), changes its size (`size`, read with `kind` or
/// its own kind), what it is for (`purpose`; empty: nothing) and whether it is a cable's gland (`for_cable`; `false`
/// keeps it as a hole). Rejected in words as `layout.feature.add` is, and for a feature that does not exist.
/// Interactive. Outcome: the feature.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditFeature {
    /// The feature.
    #[schemars(with = "UuidS")]
    pub feature: Id<SurfaceFeature>,
    /// Its new centre, in the surface's frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<SurfacePt2S>")]
    pub at: Option<Pt2>,
    /// Its kind; absent: its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Its size as typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// What it is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// Whether it is a cable's gland.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub for_cable: Option<bool>,
}

impl Command for EditFeature {
    const NAME: &'static str = "layout.feature.edit";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(feature) = tx.get::<SurfaceFeature>(self.feature) else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/feature",
                text!("refusal-layout-feature-gone"),
            ));
        };
        let hole = match (&self.kind, &self.size) {
            (None, None) => None,
            (kind, size) => {
                let own = feature.hole.kind().map(HoleKind::word).unwrap_or_default();
                let size = size.clone().unwrap_or_else(|| feature.hole.to_string());
                Some(hole_of(Self::NAME, kind.as_deref().unwrap_or(own), &size)?)
            }
        };
        if let Some(at) = self.at {
            let Some(surface) = tx.get::<MountingSurface>(feature.surface) else {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/feature",
                    text!("refusal-params-surface-gone"),
                ));
            };
            on_sheet(Self::NAME, &surface, at)?;
        }
        let edit = self.clone();
        tx.update::<SurfaceFeature>(self.feature, |f| {
            if let Some(at) = edit.at {
                f.at = at;
            }
            if let Some(h) = hole {
                f.hole = h;
            }
            if let Some(p) = &edit.purpose {
                f.purpose = purpose_of(Some(p));
            }
            if let Some(on) = edit.for_cable {
                f.for_cable = on;
            }
        })?;
        Ok(Outcome::primary((Kind::SurfaceFeature, self.feature.cast())))
    }
}

/// `layout.hole_choice` (`HoleChoiceEdit`, a hole's row in Properties): moves the hole at `index` of a placement's or a
/// carrier's pattern by `offset` (in the surface's frame) or leaves it out (`omit`); neither puts it back. Exactly one
/// of `placement` and `carrier`. Rejected in words: an owner that does not exist, an index its pattern has no hole at.
/// Interactive. Outcome: the owner.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HoleChoiceEdit {
    /// The placement whose part's hole it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub placement: Option<Id<Placement>>,
    /// The carrier whose fixing it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub carrier: Option<Id<Carrier>>,
    /// The hole's index in the pattern, from 0.
    pub index: u16,
    /// The hole moved by this offset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<SurfacePt2S>")]
    pub offset: Option<Pt2>,
    /// The hole left out.
    #[serde(default)]
    pub omit: bool,
}

impl Command for HoleChoiceEdit {
    const NAME: &'static str = "layout.hole_choice";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let q = Q::new(tx.base_model());
        let gone = || CmdError::invalid_params(Self::NAME, "/placement", text!("refusal-layout-hole-owner"));
        let (count, choices) = match (self.placement, self.carrier) {
            (Some(p), None) => {
                let placement = tx.get::<Placement>(p).ok_or_else(gone)?;
                (
                    ew_derive::layout::placement_holes(&q, p).len(),
                    placement.hole_choices.clone(),
                )
            }
            (None, Some(c)) => {
                let carrier = tx.get::<Carrier>(c).ok_or_else(gone)?;
                (
                    ew_derive::layout::carrier_fixings(&carrier, &q.project().settings.workshop).len(),
                    carrier.hole_choices.clone(),
                )
            }
            _ => return Err(gone()),
        };
        if usize::from(self.index) >= count {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/index",
                text!(
                    "refusal-layout-hole-index",
                    (u32::from(self.index) + 1).to_string(),
                    count.to_string()
                ),
            ));
        }
        let mut choices: Vec<HoleChoice> = choices.into_iter().filter(|c| c.index != self.index).collect();
        if self.omit || self.offset.is_some() {
            choices.push(HoleChoice {
                index: self.index,
                offset: self.offset.filter(|_| !self.omit),
                omit: self.omit,
                extra: Default::default(),
            });
            choices.sort_by_key(|c| c.index);
        }
        match (self.placement, self.carrier) {
            (Some(p), _) => {
                tx.update::<Placement>(p, |x| x.hole_choices = choices)?;
                Ok(Outcome::primary((Kind::Placement, p.cast())))
            }
            (_, Some(c)) => {
                tx.update::<Carrier>(c, |x| x.hole_choices = choices)?;
                Ok(Outcome::primary((Kind::Carrier, c.cast())))
            }
            _ => Err(gone()),
        }
    }
}
