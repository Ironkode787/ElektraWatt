// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Another cabinet for an enclosure (doc 12 §3.1): `layout.enclosure.set_part` with what its card says before Apply.

use ew_derive::Q;
use ew_ids::{Id, Uuid};
use ew_schema::{Enclosure, LibDocument, Part};

use super::layout_edit::{mm_text, tag};
use crate::{AnyCommand, Session};

/// What giving an enclosure another cabinet part does ([`Session::enclosure_part_plan`]), as its card says it before
/// Apply.
#[derive(Clone, Debug)]
pub struct EnclosurePartPlan {
    /// The `layout.enclosure.set_part`.
    pub command: AnyCommand,
    /// The enclosure's tag (`-UC1`).
    pub tag: String,
    /// The new cabinet's part number.
    pub number: String,
    /// Each surface whose size changes: its words, its size before and after.
    pub sizes: Vec<(crate::Text, String, String)>,
    /// What no longer lies inside its surface, in the layout check's words.
    pub outside: Vec<crate::Text>,
    /// Whether the new cabinet names no plate, so the plate there is stays.
    pub plate_kept: bool,
}

impl Session {
    /// Giving the enclosure of device `device` (an enclosure's device, -UC1) the cabinet part `pick` (doc 12 §3.1): the
    /// `layout.enclosure.set_part` with what it embeds, and what its card says before Apply, read from a preview on a
    /// throwaway engine: each surface whose size changes, and what then no longer lies inside its surface by the
    /// layout's own check (EW-LAY-010). `None` where `device` is no enclosure's device or the part is no cabinet (the
    /// part dialog then gives the device its part as for any other).
    pub fn enclosure_part_plan(
        &self,
        device: Uuid,
        pick: &crate::PartPick,
    ) -> Option<Result<EnclosurePartPlan, crate::ApiError>> {
        let q = self.query();
        let (enclosure, _) = q
            .table::<Enclosure>()
            .iter()
            .find(|(_, e)| e.device.as_uuid() == device)?;
        let (copy, item, copies) = match pick {
            crate::PartPick::Held(c) => (*c, q.parts().get(Id::from_uuid(*c))?.item.clone(), Vec::new()),
            crate::PartPick::Library { library, item } => {
                let r = crate::part_offers::lib_ref(*library, *item);
                let docs = match self.closure_docs(std::slice::from_ref(&r)) {
                    Ok(d) => d,
                    Err(e) => return Some(Err(e)),
                };
                let part = docs.iter().find_map(|d| match d {
                    LibDocument::Part(c) if c.item_id.as_uuid() == *item => Some(c.item.clone()),
                    _ => None,
                })?;
                let copy = ew_ids::v5::lib_copy_id::<Part>(*library, *item).as_uuid();
                (
                    copy,
                    part,
                    docs.iter().filter_map(|d| serde_json::to_value(d).ok()).collect(),
                )
            }
        };
        item.sections.enclosure.as_ref()?;
        let plate_kept = item
            .sections
            .enclosure
            .as_ref()
            .is_some_and(|s| s.plate.is_none());
        let cmd = ew_commands::commands::SetEnclosurePart {
            enclosure,
            part: Id::from_uuid(copy),
            copies,
        };
        let command = match AnyCommand::of(&cmd) {
            Ok(c) => c,
            Err(e) => return Some(Err(crate::ApiError::Cmd(e))),
        };
        let mut engine = ew_commands::Engine::new(
            self.world().model.clone(),
            ew_commands::EngineOptions {
                prevent: Some(Box::new(crate::ShippedPrevent)),
                ..ew_commands::EngineOptions::default()
            },
        );
        let preview = match engine.preview(&command) {
            Ok(p) => p,
            Err(e) => return Some(Err(crate::ApiError::Cmd(e))),
        };
        let after = self.query_over(preview.candidate());
        let size = |q: &Q, record: Uuid| {
            ew_derive::layout::surfaces(q, enclosure)
                .into_iter()
                .find(|f| f.record.map(|r| r.as_uuid()) == Some(record))
                .and_then(|f| f.size)
                .map(|(w, h)| {
                    format!(
                        "{} \u{d7} {} mm",
                        mm_text(q, i128::from(w.0)),
                        mm_text(q, i128::from(h.0))
                    )
                })
                .unwrap_or_default()
        };
        let sizes = ew_derive::layout::surfaces(&q, enclosure)
            .into_iter()
            .filter_map(|f| {
                let record = f.record?.as_uuid();
                let (old, new) = (size(&q, record), size(&after, record));
                (old != new).then(|| (ew_derive::text::surface_word(&f.kind, f.door, f.doors), old, new))
            })
            .collect();
        // What the layout's own check says lies outside its surface after it, and did not before.
        let scheme = self.online_scheme();
        let outside_of = |q: &Q| -> Vec<(String, crate::Text)> {
            self.check_over(q, scheme, crate::checks::CheckMode::Online, Vec::new())
                .findings
                .into_iter()
                .filter(|f| f.rule == "EW-LAY-010")
                .map(|f| (format!("{:?}", f.targets), f.text))
                .collect()
        };
        let before = outside_of(&q);
        let outside = outside_of(&after)
            .into_iter()
            .filter(|(t, _)| !before.iter().any(|(b, _)| b == t))
            .map(|(_, text)| text)
            .collect();
        Some(Ok(EnclosurePartPlan {
            command,
            tag: tag(&q, q.table::<Enclosure>().get(enclosure)?.device),
            number: item.part_number.as_str().to_owned(),
            sizes,
            outside,
            plate_kept,
        }))
    }
}
