// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The display and parse context of the property sheet: how a row's value is shown and read back (a part, a
//! structure node's path, a record's label, the date format, the company colour codes, the slot choices).
//!
//! Not here: the rows themselves (`sections.rs` and the `*_rows.rs` files beside this one); the field types and
//! their texts (`output/fields.rs`).

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_rds::{Aspect, RdsSyntax};
use ew_derive::{Q, ResolvedSlot, ResolvedStructure, SlotSource, StructurePath};
use ew_schema::{Kind, StructureNode};

use super::header::record_title;
use crate::output::fields::{DisplayCx, ParseCx};

/// What the sheet's texts and the editors' parsers read from the project.
pub(crate) struct SheetCx<'q> {
    pub(super) q: &'q Q,
    pub(super) source: Option<String>,
    /// The libraries' parts the project does not hold, listed after its copies (the Parts row).
    library: Vec<crate::output::fields::PartCopyText>,
}

impl<'q> SheetCx<'q> {
    pub(crate) fn new(q: &'q Q) -> SheetCx<'q> {
        let source = q.project().languages.as_ref().map(|l| l.source.to_string());
        SheetCx {
            q,
            source,
            library: Vec::new(),
        }
    }

    /// The context with the libraries' parts that `q` does not hold among the part copies.
    pub(super) fn with_library_parts(mut self, session: &crate::Session) -> SheetCx<'q> {
        // A part's description in the sheet's language, else English, else the first (FT-2066).
        let lang = self.q.project().languages.as_ref().map(|l| l.source.clone());
        self.library = session
            .library_parts()
            .into_iter()
            .filter(|c| !self.q.parts().contains(c.id))
            .map(|c| crate::output::fields::PartCopyText {
                id: c.id.as_uuid().hyphenated().to_string(),
                number: c.item.part_number.as_str().to_owned(),
                manufacturer: session.library_manufacturer(&c.item.manufacturer),
                kind: serde_json::to_value(&c.item.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                description: ew_derive::text_in(&c.item.description, lang.as_ref()).to_owned(),
            })
            .collect();
        self
    }

    fn part(&self, copy: &str) -> Option<(String, String)> {
        let uuid = Uuid::parse_str(copy).ok()?;
        let p = self.q.parts().get(Id::from_uuid(uuid))?;
        Some((
            p.item.part_number.as_str().to_owned(),
            ew_derive::manufacturer_name(self.q, &p.item.manufacturer),
        ))
    }

    /// The printed code path of a structure node (`+J-E-2416`).
    pub(super) fn node_path(&self, node: Id<StructureNode>) -> Option<String> {
        let path = self.q.get::<StructurePath>(node);
        let aspect = path.aspect?;
        let mut s = ResolvedStructure::default();
        s.slots.insert(
            aspect,
            ResolvedSlot {
                node,
                codes: path.codes.clone(),
                source: SlotSource::Explicit,
            },
        );
        Some(s.render_aspect(aspect, &RdsSyntax::default()))
    }
}

impl DisplayCx for SheetCx<'_> {
    fn part_text(&self, copy: &str) -> Option<String> {
        self.part(copy).map(|(pn, mfr)| format!("{pn} ({mfr})"))
    }

    fn part_number(&self, copy: &str) -> Option<String> {
        self.part(copy).map(|(pn, _)| pn)
    }

    fn node_text(&self, node: &str) -> Option<String> {
        let uuid = Uuid::parse_str(node).ok()?;
        self.node_path(Id::from_uuid(uuid))
    }

    fn record_label(&self, id: &str) -> Option<String> {
        let uuid = Uuid::parse_str(id).ok()?;
        record_title(self.q, uuid, self.source.as_deref()).or_else(|| {
            // A library copy (a representation's symbol, a function's definition, a page's form) by its name.
            [Kind::Symbol, Kind::Fndef, Kind::Form]
                .into_iter()
                .find_map(|k| crate::output::navigators::libraries::copy_name(self.q, k, uuid))
        })
    }

    fn source_language(&self) -> Option<String> {
        self.source.clone()
    }

    fn date_format(&self) -> ew_schema::DateFormat {
        self.q.project().settings.formatting.date_format()
    }
}

/// The structure aspect a structure-assignment member names.
fn aspect_of_member(member: &str) -> Option<Aspect> {
    Some(match member {
        "functional_assignment" => Aspect::FunctionalAssignment,
        "function" => Aspect::Function,
        "installation_site" => Aspect::InstallationSite,
        "location" => Aspect::Location,
        "document_kind" => Aspect::DocumentKind,
        "user_defined" => Aspect::UserDefined,
        "type" => Aspect::Type,
        _ => return None,
    })
}

/// The structure-assignment member of an aspect.
pub(super) fn member_of_aspect(aspect: Aspect) -> &'static str {
    match aspect {
        Aspect::FunctionalAssignment => "functional_assignment",
        Aspect::Function => "function",
        Aspect::InstallationSite => "installation_site",
        Aspect::Location => "location",
        Aspect::DocumentKind => "document_kind",
        Aspect::UserDefined => "user_defined",
        Aspect::Type => "type",
        Aspect::Product => "product",
    }
}

impl ParseCx for SheetCx<'_> {
    fn date_format(&self) -> ew_schema::DateFormat {
        self.q.project().settings.formatting.date_format()
    }

    fn frame_forms(&self) -> Vec<(String, crate::Text)> {
        crate::output::navigators::libraries::frame_forms(self.q)
            .into_iter()
            .map(|(id, name)| (id.to_string(), name))
            .collect()
    }

    fn company_codes(&self) -> Vec<(String, String)> {
        self.q
            .project()
            .settings
            .extra
            .get("colour_profile")
            .map(ew_derive::company_codes)
            .unwrap_or_default()
    }

    fn part_copies(&self) -> Vec<crate::output::fields::PartCopyText> {
        self.q
            .parts()
            .iter()
            .map(|(id, p)| crate::output::fields::PartCopyText {
                id: id.as_uuid().hyphenated().to_string(),
                number: p.item.part_number.as_str().to_owned(),
                manufacturer: ew_derive::manufacturer_name(self.q, &p.item.manufacturer),
                kind: serde_json::to_value(&p.item.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                description: p
                    .item
                    .description
                    .neutral_or_first()
                    .unwrap_or_default()
                    .to_owned(),
            })
            .collect()
    }

    fn library_parts(&self) -> Vec<crate::output::fields::PartCopyText> {
        self.library.clone()
    }

    fn slot_nodes(&self, member: &str) -> Vec<(String, String, Vec<String>)> {
        let Some(aspect) = aspect_of_member(member) else {
            return Vec::new();
        };
        self.q
            .structure_nodes()
            .iter()
            .filter(|(id, _)| self.q.get::<StructurePath>(*id).aspect == Some(aspect))
            .filter_map(|(id, n)| {
                let path = self.node_path(id)?;
                let aliases = serde_json::to_value(&n.aliases)
                    .ok()
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|a| a.as_str().map(str::to_owned))
                    .collect();
                Some((id.as_uuid().hyphenated().to_string(), path, aliases))
            })
            .collect()
    }
}
