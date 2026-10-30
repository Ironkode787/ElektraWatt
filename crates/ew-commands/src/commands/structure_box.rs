// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `box.structure` (`CreateStructureBox`, WP-S2.04 §5.1, §10.1; FR-PRJ-020): a dotted or dash-dot box that assigns
//! a structure to the representations it encloses, and the checks `fields.set` repeats after it writes a box.

use ew_ids::Id;
use ew_mltext::MlText;
use ew_rds::Aspect;
use ew_schema::{
    BoxStyle, ExternalRef, Extra, Open, Page, Slot, StructureAssignment, StructureBox, StructureNode,
};
use ew_units::Pt2;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{MlTextS, Pt2S, UuidS};
use crate::tx::TxBuilder;

/// The line style of a structure box.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BoxStyleParam {
    /// Dotted: 0.5 mm on, 1 mm off.
    Dotted,
    /// Dash-dot: 4 mm on, 1 mm off, 0.5 mm on, 1 mm off.
    DashDot,
}

/// The slots of an assignment with their aspects, in canonical aspect order.
fn slots(a: &StructureAssignment) -> [(Aspect, &'static str, Option<&Slot>); 7] {
    [
        (
            Aspect::FunctionalAssignment,
            "functional_assignment",
            a.functional_assignment.as_ref(),
        ),
        (Aspect::Function, "function", a.function.as_ref()),
        (
            Aspect::InstallationSite,
            "installation_site",
            a.installation_site.as_ref(),
        ),
        (Aspect::Location, "location", a.location.as_ref()),
        (Aspect::DocumentKind, "document_kind", a.document_kind.as_ref()),
        (Aspect::UserDefined, "user_defined", a.user_defined.as_ref()),
        (Aspect::Type, "type", a.type_.as_ref()),
    ]
}

/// Checks a structure box's members (WP-S2.04 §5.1 `box.structure`): at least three outline vertices; every node of
/// `assigns` exists and has the aspect of its slot; the external reference exists; a style this build draws. A
/// refusal is invalid parameters of `command` at `pointer` + the member's path.
pub(crate) fn check_box(
    tx: &TxBuilder<'_>,
    command: &str,
    pointer: &str,
    b: &StructureBox,
) -> Result<(), CmdError> {
    if b.outline.len() < 3 {
        return Err(CmdError::invalid_params(
            command,
            format!("{pointer}/outline"),
            format!(
                "a structure box's outline needs at least three vertices, not {}",
                b.outline.len()
            ),
        ));
    }
    if crate::commands::items::collinear(&b.outline) {
        return Err(CmdError::invalid_params(
            command,
            format!("{pointer}/outline"),
            "a structure box's outline has no area: its vertices lie on one line",
        ));
    }
    if !crate::commands::items::known_box_style(&b.style) {
        return Err(CmdError::invalid_params(
            command,
            format!("{pointer}/style"),
            "a box style this build does not know; use dotted or dash_dot",
        ));
    }
    for (aspect, name, slot) in slots(&b.assigns) {
        let Some(Slot::Node(n)) = slot else {
            continue;
        };
        let at = format!("{pointer}/assigns/{name}");
        let Some(node) = tx.get::<StructureNode>(*n) else {
            return Err(CmdError::invalid_params(
                command,
                at,
                format!("structure node {n} does not exist"),
            ));
        };
        if node.aspect != Open::Known(aspect) {
            return Err(CmdError::invalid_params(
                command,
                at,
                format!("structure node {n} ({}) is not of the aspect {name}", node.code),
            ));
        }
    }
    if let Some(e) = b.external {
        if !tx.exists::<ExternalRef>(e) {
            return Err(CmdError::invalid_params(
                command,
                format!("{pointer}/external"),
                "the external reference is not in the project",
            ));
        }
    }
    Ok(())
}

/// `box.structure` (`CreateStructureBox`, WP-S2.04 §5.1, §10.1; FR-PRJ-020): creates a structure box on a page: its
/// outline (absolute page nm, at least three vertices, which need not lie on the connection grid), its style (`dotted`
/// or `dash_dot`), an optional caption, the structure it assigns (one slot per aspect: a node, `none`, or absent to
/// leave the page's) and the external point it stands for. Which representations it encloses is derived, never
/// stored (`rep_structure_box`). An empty caption is not stored. Rejected: fewer than three vertices (at
/// `/outline`); a node of `assigns` that does not exist or is not of its slot's aspect (at `/assigns/<aspect>`); an
/// external reference that does not exist (at `/external`); a missing page (not found). Interactive. Outcome: the
/// box.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateStructureBox {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The outline vertices, absolute in page nm (at least three).
    #[schemars(with = "Vec<Pt2S>")]
    pub outline: Vec<Pt2>,
    /// The line style.
    pub style: BoxStyleParam,
    /// The caption, printed above the outline's top-left vertex.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub caption: MlText,
    /// The structure the box assigns: per aspect a node, `none`, or absent (the page's).
    pub assigns: StructureAssignment,
    /// The external point the box stands for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub external: Option<Id<ExternalRef>>,
}

impl Command for CreateStructureBox {
    const NAME: &'static str = "box.structure";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        tx.require::<Page>(self.page)?;
        let id = tx.mint::<StructureBox>();
        let b = StructureBox {
            id,
            v: 1,
            page: self.page,
            outline: self.outline.clone(),
            style: match self.style {
                BoxStyleParam::Dotted => BoxStyle::Dotted,
                BoxStyleParam::DashDot => BoxStyle::DashDot,
            },
            caption: (!self.caption.is_empty()).then(|| self.caption.clone()),
            assigns: self.assigns.clone(),
            external: self.external,
            extra: Extra::new(),
        };
        check_box(tx, Self::NAME, "", &b)?;
        let key = tx.insert(b)?;
        Ok(Outcome::primary(<StructureBox as ew_model::Entity>::key(key)))
    }
}
