// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's entries of the PLC, bus and fluid commands (WP-S2.13): each handler builds one declared command
//! from the row or selection it is aimed at and commits it as one undo step, saying what it did in words.
//!
//! - **Attach cards without a CPU** (FT-1672): a CPU row of the PLC navigator attaches the cards that wait for a CPU
//!   (`Session::plc_cards_without_cpu`) with one `assembly.attach`, after the station's last child position.

use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::strings;

impl ShellModel {
    /// A record named by parameter `name`: its UUID, or a reference (`tag:-201U1`, `id:<uuid>`).
    pub(super) fn record_param(&self, p: &Map<String, Value>, name: &str) -> Result<Uuid, Outcome> {
        let text = match string_param(p, name)? {
            Some(t) => t.trim(),
            None => return Err(failed(key!("error-missing-param"), &[name])),
        };
        if let Ok(id) = text.parse::<Uuid>() {
            return Ok(id);
        }
        let project = self.project.as_ref().ok_or(Outcome::Refused {
            reason_key: key!("reason-no-project"),
        })?;
        project
            .session
            .resolve_reference(text)
            .map_err(|_| failed(key!("error-bad-param"), &[name]))
    }

    /// `plc.attach_cards {station}` (FT-1672): the cards that wait for a CPU become children of `station`, after its
    /// last child position, with one `assembly.attach`; a toast names them.
    pub(crate) fn act_plc_attach_cards(&mut self, p: &Map<String, Value>) -> Outcome {
        let station = or_return!(self.record_param(p, "station"));
        let Some(project) = self.project.as_ref() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let cards = project.session.plc_cards_without_cpu();
        if cards.is_empty() {
            return Outcome::refused(key!("reason-no-cards-without-cpu"));
        }
        let q = project.session.query();
        let Some(id) = q
            .devices()
            .iter()
            .map(|(id, _)| id)
            .find(|id| id.as_uuid() == station)
        else {
            return failed(key!("error-bad-param"), &["station"]);
        };
        let last = q
            .idx()
            .device_children(id)
            .filter_map(|c| q.devices().get(c).and_then(|d| d.position))
            .max()
            .unwrap_or(0);
        let tag = project.session.device_tag(station).unwrap_or_default();
        let children: Vec<Value> = (cards.iter().zip(last + 1..))
            .map(|((card, _), position)| json!({"device": card.to_string(), "position": position}))
            .collect();
        let tags: Vec<String> = cards.into_iter().map(|(_, t)| t).collect();
        let cmd = ew_api::AnyCommand {
            name: "assembly.attach".into(),
            params: json!({"parent": station.to_string(), "children": children}),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            let text = strings::format(key!("toast-cards-attached"), &[&tags.join(", "), &tag]);
            self.notify(text, true);
        }
        out
    }
}
