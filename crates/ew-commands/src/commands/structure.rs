// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project structure commands: `structure.create_node`, `section.create`, `report_block.create`.

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_rds::Aspect;
use ew_schema::{
    ContinuationPolicy, EmbedSpec, EmptyPolicy, Extra, FormDef, GroupSpec, Kind, LibCopy, ManualSelection,
    Open, Page, ReportBlock, ReportSource, Section, Set, SortKey, StructureNode,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{AspectS, IdentS, MlTextS, UuidS};
use crate::tx::TxBuilder;

/// `structure.create_node`: one identifier in one aspect tree (doc 03 §5, FR-PRJ-017).
///
/// Creates `StructureNode {aspect, code, aliases (sorted set), parent, description, order: 0}`. Rejected:
/// the product aspect (the device tree is never a structure node); a parent of another aspect
/// (EW-INV-011, checked here with a message); a sibling (same aspect and parent) with the same code. The
/// parent must exist. Outcome: the new node.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateStructureNode {
    /// The aspect tree.
    #[schemars(with = "AspectS")]
    pub aspect: Aspect,
    /// The atomic code ("J-E-2416").
    #[schemars(with = "IdentS")]
    pub code: Ident,
    /// The parent node of the same aspect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub parent: Option<Id<StructureNode>>,
    /// Import spellings that map to this node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<IdentS>")]
    pub aliases: Vec<Ident>,
    /// Description.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
}

impl Command for CreateStructureNode {
    const NAME: &'static str = "structure.create_node";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.aspect == Aspect::Product {
            return tx
                .reject("the product aspect is the device tree, not a structure node; pick another aspect");
        }
        let aspect = Open::Known(self.aspect);
        if let Some(parent) = self.parent {
            let parent = tx.require::<StructureNode>(parent)?;
            if parent.aspect != aspect {
                return tx.reject(
                    "the parent belongs to another aspect (EW-INV-011); pick a parent of the same aspect",
                );
            }
        }
        let clash = tx
            .iter::<StructureNode>()
            .iter()
            .any(|n| n.aspect == aspect && n.parent == self.parent && n.code == self.code);
        if clash {
            return tx.reject(format!(
                "a sibling with code `{}` already exists in this aspect; choose another code",
                self.code.as_str()
            ));
        }
        let id: Id<StructureNode> = tx.mint();
        tx.insert(StructureNode {
            id,
            v: 1,
            aspect,
            code: self.code.clone(),
            aliases: Set::from(self.aliases.clone()),
            parent: self.parent,
            description: self.description.clone(),
            order: 0,
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::StructureNode, id.cast())))
    }
}

/// `section.create`: a folio range with a title (doc 03 §6, FR-PAG-008).
///
/// Creates `Section {number, folio_range, title, divider}`. Rejected: `folio_range.0 > folio_range.1`; a
/// section with the same number; a range that overlaps another section's range. Outcome: the new section.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateSection {
    /// Section number.
    pub number: u16,
    /// First and last folio.
    pub folio_range: (u32, u32),
    /// Title.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub title: MlText,
    /// Whether a divider page is generated.
    #[serde(default)]
    pub divider: bool,
}

impl Command for CreateSection {
    const NAME: &'static str = "section.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let (from, to) = self.folio_range;
        if from > to {
            return tx.reject("`folio_range` starts after it ends; give first <= last");
        }
        for other in tx.iter::<Section>() {
            if other.number == self.number {
                return tx.reject(format!(
                    "section number {} already exists; choose another number",
                    self.number
                ));
            }
            let (a, b) = other.folio_range;
            if from <= b && a <= to {
                return tx.reject(format!(
                    "folio range {from}-{to} overlaps section {} ({a}-{b}); choose a free range",
                    other.number
                ));
            }
        }
        let id: Id<Section> = tx.mint();
        tx.insert(Section {
            id,
            v: 1,
            number: self.number,
            folio_range: self.folio_range,
            title: self.title.clone(),
            divider: self.divider,
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Section, id.cast())))
    }
}

/// `report_block.create`: a report block (doc 03 §6, doc 09 §2.2).
///
/// Creates a `ReportBlock` with the given members (`manual`, `sort`, `group`, `continuation`, `empty`
/// default). Rejected with a message: a page-oriented block (no `embedded`) without `first_folio`
/// (EW-INV-014): `first_folio` is required unless `embedded` is set. The form copy, the section and the
/// embedding page must exist (else not found). Outcome: the new block.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateReportBlock {
    /// What the block lists.
    pub source: ReportSource,
    /// The report form copy.
    #[schemars(with = "UuidS")]
    pub form: Id<FormDef>,
    /// The section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub section: Option<Id<Section>>,
    /// The first folio of a page-oriented block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_folio: Option<u32>,
    /// Title.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub title: MlText,
    /// Manual inclusions and exclusions.
    #[serde(default, skip_serializing_if = "ManualSelection::is_empty")]
    pub manual: ManualSelection,
    /// Sort keys.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<SortKey>,
    /// Grouping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupSpec>,
    /// Overflow policy.
    #[serde(default)]
    pub continuation: ContinuationPolicy,
    /// What an empty report prints.
    #[serde(default)]
    pub empty: EmptyPolicy,
    /// Embedding into a drawn page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded: Option<EmbedSpec>,
}

impl Command for CreateReportBlock {
    const NAME: &'static str = "report_block.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.embedded.is_none() && self.first_folio.is_none() {
            return tx.reject(
                "a page-oriented report block needs `first_folio` (EW-INV-014); give one or embed it",
            );
        }
        tx.require::<LibCopy<FormDef>>(self.form)?;
        if let Some(section) = self.section {
            tx.require::<Section>(section)?;
        }
        if let Some(embed) = &self.embedded {
            tx.require::<Page>(embed.page)?;
        }
        let id: Id<ReportBlock> = tx.mint();
        tx.insert(ReportBlock {
            id,
            v: 1,
            source: self.source.clone(),
            manual: self.manual.clone(),
            sort: self.sort.clone(),
            group: self.group.clone(),
            form: self.form,
            section: self.section,
            first_folio: self.first_folio,
            continuation: self.continuation.clone(),
            title: self.title.clone(),
            empty: self.empty.clone(),
            embedded: self.embedded.clone(),
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::ReportBlock, id.cast())))
    }
}
