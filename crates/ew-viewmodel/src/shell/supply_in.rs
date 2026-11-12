// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Supply comes in here… (`ip.supply_in`, FT-1699): the selected interruption points are where the supply enters the
//! drawing set from another one (L1, L2, L3, N, PE from another drawing). The supply that comes in is an external
//! reference: the action's form proposes where it comes from (the project supply's origin, else a location named
//! after the supply) and the device there (the feeder's strip), and Apply is one `ip.supply_in`: the location node
//! where the project lacks it, the reference, the points' `ip.external`, and the supply's origin where it has none.
//! The points then print what an external point prints, and pair with nothing, so the unpaired point's finding is
//! gone. On points that all have the mark it is Remove the supply mark, which takes it off.

use ew_api::AnyCommand;
use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use super::{Dialog, FormDialog, FormField, Mode, Outcome, ShellModel, flag};
use crate::actions::ActionItem;
use crate::key;
use crate::strings;
use crate::tools::{EntityKind, selection_kinds};

/// The action.
const SUPPLY_IN: &str = "ip.supply_in";

impl ShellModel {
    /// The selected interruption points of the page shown (their functions) and whether every one has the supply
    /// mark already; `None` when the selection holds anything else or nothing.
    fn selected_supply_points(&self) -> Option<(Vec<Uuid>, bool)> {
        let c = self.canvas()?;
        let t = &c.page().tables;
        let kinds = selection_kinds(&self.tools.selection, t);
        if kinds.is_empty() {
            return None;
        }
        let mut points = Vec::new();
        let mut marked = true;
        for (e, kind) in &kinds {
            if *kind != EntityKind::Rep {
                return None;
            }
            let rep = t.reps.iter().find(|r| r.id == e.0)?;
            marked &= rep.ip.as_ref()?.external;
            points.push(*rep.functions.first()?);
        }
        Some((points, marked))
    }

    /// Supply comes in here…, or Remove the supply mark on points that all have it, for the selected interruption
    /// points (the right-click menu's first entry for a point, and its Properties link); `None` for any other
    /// selection.
    pub fn supply_in_item(&self) -> Option<ActionItem> {
        let (_, marked) = self.selected_supply_points()?;
        let mut item = self.menu_item(SUPPLY_IN)?;
        if marked {
            item.label = strings::text(key!("supply-in-remove")).to_owned();
            item.params = json!({ "remove": true });
        }
        Some(item)
    }

    /// The form's proposals: where the project supply's origin is, when it names one, else a location named after
    /// the supply; and the device there, the origin's, else the feeder's terminal strip.
    fn supply_in_proposal(&self) -> (String, String) {
        let proposal = || {
            (
                strings::text(key!("supply-in-location-proposal")).to_owned(),
                "-X0".to_owned(),
            )
        };
        let Some(project) = &self.project else {
            return proposal();
        };
        let q = project.session.query();
        let origin = q
            .project()
            .supply
            .as_ref()
            .and_then(|s| s.origin.as_ref())
            .and_then(|o| o.source)
            .and_then(|x| q.external_refs().get(x).cloned());
        match origin {
            Some(x) => {
                let location = q
                    .structure_nodes()
                    .get(x.location)
                    .map(|n| n.code.as_str().to_owned())
                    .unwrap_or_default();
                (location, x.device.as_str().to_owned())
            }
            None => proposal(),
        }
    }

    /// `ip.supply_in {location?, device?, remove?}` (see the module documentation). In the window without `location`
    /// and `device` its form opens with the proposals; headless they take the proposals. A selection of anything but
    /// interruption points is refused in words.
    pub(crate) fn act_supply_in(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some((points, _)) = self.selected_supply_points() else {
            return Outcome::refused(key!("reason-supply-in-select"));
        };
        let remove = flag(p, "remove");
        let given = |name: &str| {
            p.get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
        };
        let (mut location, mut device) = (given("location"), given("device"));
        if !remove && (location.is_none() || device.is_none()) {
            let (place, feeder) = self.supply_in_proposal();
            if self.in_form_of(SUPPLY_IN) {
                let missing = if location.is_none() { "location" } else { "device" };
                let message = self.missing_text(missing);
                return self.form_failed(SUPPLY_IN, message);
            }
            if self.mode == Mode::Gui {
                let field = |name, label_key, value: String| FormField {
                    name,
                    label_key,
                    choices: &super::choices::NONE,
                    options: Vec::new(),
                    value,
                };
                let dialog = FormDialog {
                    action: SUPPLY_IN,
                    title_key: key!("supply-in-title"),
                    ok_key: key!("dialog-apply"),
                    fields: vec![
                        field("location", key!("supply-in-location"), place),
                        field("device", key!("supply-in-device"), feeder),
                    ],
                    error: None,
                    table: None,
                };
                return self.open_dialog(Dialog::Form(dialog));
            }
            location = location.or(Some(place));
            device = device.or(Some(feeder));
        }
        let mut params = json!({
            "points": points.iter().map(Uuid::to_string).collect::<Vec<_>>(),
            "remove": remove,
        });
        if !remove {
            params["location"] = Value::from(location);
            params["device"] = Value::from(device);
        }
        let cmd = AnyCommand {
            name: SUPPLY_IN.into(),
            params,
        };
        let out = self.commit_from_host(&cmd);
        self.form_done(SUPPLY_IN, out)
    }
}
